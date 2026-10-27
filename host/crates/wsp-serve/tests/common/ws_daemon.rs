// SPDX-License-Identifier: AGPL-3.0-only
//! runtime-fixture.ts:23 `helloingDaemon` and :87 `withDaemon`: a daemon on a loopback port that answers every op ok and
//! announces the version it is set to right after the auth reply, as the real one does. A plain GET is the reach probe,
//! answered as the daemon's own ws server answers it (426). `with_daemon` is the same stand-in holding the first
//! machine's own token, which refuses an auth that carries another.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::{SinkExt as _, StreamExt as _};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

struct Shared {
    announced: Cell<i64>,
    held: Cell<bool>,
    waiting: RefCell<Vec<futures::channel::mpsc::UnboundedSender<()>>>,
    hits: Cell<u32>,
    dials: Cell<u32>,
    token: Option<String>,
}

pub struct HelloingDaemon {
    pub port: u16,
    shared: Rc<Shared>,
    task: JoinHandle<()>,
    sockets: Rc<RefCell<Vec<JoinHandle<()>>>>,
}

impl Drop for HelloingDaemon {
    fn drop(&mut self) {
        self.task.abort();
        for socket in self.sockets.borrow_mut().drain(..) {
            socket.abort();
        }
    }
}

impl HelloingDaemon {
    /// `announce(v)`: the version the next hello carries.
    pub fn announce(&self, version: i64) {
        self.shared.announced.set(version);
    }

    /// `release()`: the hellos held back go out.
    pub fn release(&self) {
        self.shared.held.set(false);
        for say in self.shared.waiting.borrow_mut().drain(..) {
            let _ = say.unbounded_send(());
        }
    }

    /// Plain GETs answered (the reach probe).
    pub fn hits(&self) -> u32 {
        self.shared.hits.get()
    }

    /// Sockets dialed.
    pub fn dials(&self) -> u32 {
        self.shared.dials.get()
    }
}

/// `helloingDaemon(version, holdHello)`; `token` makes it refuse an auth that names another.
pub async fn helloing_daemon(version: i64, hold_hello: bool, token: Option<String>) -> HelloingDaemon {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
    let port = listener.local_addr().expect("an address").port();
    let shared = Rc::new(Shared {
        announced: Cell::new(version),
        held: Cell::new(hold_hello),
        waiting: RefCell::default(),
        hits: Cell::new(0),
        dials: Cell::new(0),
        token,
    });
    let sockets: Rc<RefCell<Vec<JoinHandle<()>>>> = Rc::default();
    let (kept, held_sockets) = (shared.clone(), sockets.clone());
    let task = tokio::task::spawn_local(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let (shared, sockets) = (kept.clone(), held_sockets.clone());
            held_sockets.borrow_mut().push(tokio::task::spawn_local(async move { talk(stream, shared, sockets).await }));
        }
    });
    HelloingDaemon { port, shared, task, sockets }
}

/// `withDaemon`: a daemon that holds the first machine's own token under `seed`, at the current daemon version.
pub async fn daemon_of(seed: &str, machine: &str) -> HelloingDaemon {
    let token = wsp_core::daemon_token::daemon_token_for(seed, machine).expect("a token");
    helloing_daemon(wsp_wire::wire::daemon_version::DAEMON_VERSION, false, Some(token)).await
}

async fn talk(stream: TcpStream, shared: Rc<Shared>, _sockets: Rc<RefCell<Vec<JoinHandle<()>>>>) {
    let mut head = [0u8; 2048];
    let n = stream.peek(&mut head).await.unwrap_or(0);
    let text = String::from_utf8_lossy(&head[..n]).to_ascii_lowercase();
    if !text.contains("upgrade: websocket") {
        shared.hits.set(shared.hits.get() + 1);
        let mut stream = stream;
        let _ = stream.write_all(b"HTTP/1.1 426 Upgrade Required\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
        let _ = stream.shutdown().await;
        return;
    }
    let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else { return };
    shared.dials.set(shared.dials.get() + 1);
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
    loop {
        let hello = |version: i64| Message::text(json!({ "type": "daemon.hello", "root": "/root", "version": version }).to_string());
        tokio::select! {
            got = ws.next() => {
                let Some(Ok(Message::Text(raw))) = got else { return };
                let Ok(frame) = serde_json::from_str::<Value>(raw.as_str()) else { continue };
                let (id, op) = (frame["id"].clone(), frame["op"].as_str().unwrap_or_default().to_string());
                let refused = op == "auth" && shared.token.as_ref().is_some_and(|t| frame["token"].as_str() != Some(t.as_str()));
                let reply = if refused { json!({ "id": id, "ok": false, "error": "bad token" }) } else { json!({ "id": id, "ok": true }) };
                if ws.send(Message::text(reply.to_string())).await.is_err() || op != "auth" || refused {
                    continue;
                }
                if shared.held.get() {
                    shared.waiting.borrow_mut().push(tx.clone());
                } else if ws.send(hello(shared.announced.get())).await.is_err() {
                    return;
                }
            }
            Some(()) = rx.next() => {
                if ws.send(hello(shared.announced.get())).await.is_err() {
                    return;
                }
            }
        }
    }
}
