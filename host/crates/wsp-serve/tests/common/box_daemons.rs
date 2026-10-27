// SPDX-License-Identifier: AGPL-3.0-only
//! What box-nap.test.ts (packages/runtime/test/box-nap.test.ts at 65f6b89c7) stands behind a box's preview route: a daemon
//! that takes the socket at once and holds its answer to the auth frame until `up()` (`slowDaemon`), and an edge that
//! refuses every upgrade with a 502 until `up()` and from then on hands the socket to a daemon (`lateDaemon`).
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::{SinkExt as _, StreamExt as _};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http;
use tokio_tungstenite::tungstenite::Message;

pub struct BoxDaemon {
    pub port: u16,
    answering: Rc<Cell<bool>>,
    held: Rc<RefCell<Vec<futures::channel::mpsc::UnboundedSender<()>>>>,
    dials: Rc<Cell<u32>>,
    task: JoinHandle<()>,
    sockets: Rc<RefCell<Vec<JoinHandle<()>>>>,
}

impl Drop for BoxDaemon {
    fn drop(&mut self) {
        self.task.abort();
        for socket in self.sockets.borrow_mut().drain(..) {
            socket.abort();
        }
    }
}

impl BoxDaemon {
    /// The route as the machine's `previewUrl` hands it: `ws://127.0.0.1:<port>`.
    pub fn url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }

    /// `up()`: the daemon answers, and the answers held back go out.
    pub fn up(&self) {
        self.answering.set(true);
        for say in self.held.borrow_mut().drain(..) {
            let _ = say.unbounded_send(());
        }
    }

    /// Every upgrade the edge was asked for, the refused ones too (`lateDaemon`'s `dials()`).
    pub fn dials(&self) -> u32 {
        self.dials.get()
    }
}

async fn start(late: bool) -> BoxDaemon {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
    let port = listener.local_addr().expect("an address").port();
    let answering = Rc::new(Cell::new(false));
    let held: Rc<RefCell<Vec<futures::channel::mpsc::UnboundedSender<()>>>> = Rc::default();
    let dials = Rc::new(Cell::new(0u32));
    let sockets: Rc<RefCell<Vec<JoinHandle<()>>>> = Rc::default();
    let (up, kept, counted, handles) = (answering.clone(), held.clone(), dials.clone(), sockets.clone());
    let task = tokio::task::spawn_local(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let (up, kept, counted) = (up.clone(), kept.clone(), counted.clone());
            handles.borrow_mut().push(tokio::task::spawn_local(async move {
                let gate = up.clone();
                let seen = counted.clone();
                let admit = move |_: &Request, response: Response| -> Result<Response, ErrorResponse> {
                    seen.set(seen.get() + 1);
                    if late && !gate.get() {
                        return Err(http::Response::builder().status(502).body(None).expect("a 502"));
                    }
                    Ok(response)
                };
                let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(stream, admit).await else { return };
                let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
                let mut waiting: Vec<Value> = vec![];
                loop {
                    tokio::select! {
                        got = ws.next() => {
                            let Some(Ok(Message::Text(raw))) = got else { return };
                            let Ok(frame) = serde_json::from_str::<Value>(raw.as_str()) else { continue };
                            let held_back = !late && frame["op"] == "auth" && !up.get();
                            if held_back {
                                waiting.push(frame["id"].clone());
                                kept.borrow_mut().push(tx.clone());
                            } else if ws.send(Message::text(json!({ "id": frame["id"], "ok": true }).to_string())).await.is_err() {
                                return;
                            }
                        }
                        Some(()) = rx.next() => {
                            for id in waiting.drain(..) {
                                if ws.send(Message::text(json!({ "id": id, "ok": true }).to_string())).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
            }));
        }
    });
    BoxDaemon { port, answering, held, dials, task, sockets }
}

/// `slowDaemon()`.
pub async fn slow_daemon() -> BoxDaemon {
    start(false).await
}

/// `lateDaemon()`.
pub async fn late_daemon() -> BoxDaemon {
    start(true).await
}
