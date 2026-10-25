// SPDX-License-Identifier: AGPL-3.0-only
//! A daemon's inbound door on its own thread, answering as the real one does where the links read it: the auth frame
//! first (a wrong token, another first frame or no frame inside the deadline closes 4401 with the daemon's sentence),
//! the reply then the hello, ping, the port and inbox watches, the inbox rescan (files pushed before the reply), and
//! a pty whose writes come back as `pty.data`.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;
use wsp_wire::types::daemon_contract::{DAEMON_AUTH_DEADLINE_PASSED, DAEMON_FIRST_FRAME_NOT_AUTH, DAEMON_TOKEN_REFUSED};

#[derive(Default)]
struct Shared {
    /// Every frame a socket sent, in order.
    frames: Vec<Value>,
    /// path -> bytes.
    inbox: BTreeMap<String, u64>,
    /// port -> pid.
    listeners: BTreeMap<u64, u64>,
    ports_watchers: Vec<UnboundedSender<String>>,
    inbox_watchers: Vec<UnboundedSender<String>>,
    sockets: usize,
}

pub struct FakeDaemon {
    pub port: u16,
    pub token: String,
    shared: Arc<Mutex<Shared>>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FakeDaemon {
    pub fn start(token: &str) -> FakeDaemon {
        FakeDaemon::start_with(token, 10_000)
    }

    pub fn start_with(token: &str, auth_deadline_ms: u64) -> FakeDaemon {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let (port_tx, port_rx) = std::sync::mpsc::channel();
        let sh = shared.clone();
        let token_s = token.to_string();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                port_tx.send(listener.local_addr().unwrap().port()).unwrap();
                let serve = async {
                    while let Ok((sock, _)) = listener.accept().await {
                        tokio::spawn(serve(sock, sh.clone(), token_s.clone(), auth_deadline_ms));
                    }
                };
                tokio::select! {
                    _ = serve => {}
                    _ = stopped => {}
                }
            });
        });
        let port = port_rx.recv().unwrap();
        FakeDaemon { port, token: token.to_string(), shared, stop: Some(stop), thread: Some(thread) }
    }

    pub fn url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }

    /// A file lands in the inbox and settles: every socket watching the inbox hears it.
    pub fn add_inbox_file(&self, path: &str, bytes: u64) {
        let mut s = self.shared.lock().unwrap();
        s.inbox.insert(path.to_string(), bytes);
        let event = json!({"type": "inbox.file", "path": path, "bytes": bytes}).to_string();
        s.inbox_watchers.retain(|w| w.send(event.clone()).is_ok());
    }

    /// A process starts listening: every socket watching ports hears it.
    pub fn set_listener(&self, port: u64, pid: u64) {
        let mut s = self.shared.lock().unwrap();
        s.listeners.insert(port, pid);
        let event = json!({"type": "port.open", "port": port, "pid": pid}).to_string();
        s.ports_watchers.retain(|w| w.send(event.clone()).is_ok());
    }

    pub fn frames(&self) -> Vec<Value> {
        self.shared.lock().unwrap().frames.clone()
    }

    pub fn ops(&self) -> Vec<String> {
        self.frames().iter().filter_map(|f| f["op"].as_str().map(str::to_string)).collect()
    }

    /// Sockets accepted so far.
    pub fn sockets(&self) -> usize {
        self.shared.lock().unwrap().sockets
    }

    /// Stops listening and drops every socket.
    pub fn close(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for FakeDaemon {
    fn drop(&mut self) {
        self.close();
    }
}

fn close_frame(reason: &str) -> Message {
    Message::Close(Some(CloseFrame { code: CloseCode::from(4401), reason: reason.to_string().into() }))
}

async fn serve(sock: tokio::net::TcpStream, shared: Arc<Mutex<Shared>>, token: String, deadline_ms: u64) {
    let Ok(ws) = tokio_tungstenite::accept_async(sock).await else { return };
    shared.lock().unwrap().sockets += 1;
    let (mut tx, mut rx) = ws.split();
    let first = tokio::time::timeout(Duration::from_millis(deadline_ms), rx.next()).await;
    let first: Value = match first {
        Err(_) => {
            let _ = tx.send(close_frame(DAEMON_AUTH_DEADLINE_PASSED)).await;
            return;
        }
        Ok(Some(Ok(Message::Text(t)))) => serde_json::from_str(&t).unwrap_or(Value::Null),
        Ok(_) => return,
    };
    shared.lock().unwrap().frames.push(first.clone());
    if first["op"] != "auth" {
        let _ = tx.send(close_frame(DAEMON_FIRST_FRAME_NOT_AUTH)).await;
        return;
    }
    if first["token"] != token.as_str() {
        let _ = tx.send(close_frame(DAEMON_TOKEN_REFUSED)).await;
        return;
    }
    let (push, mut pushed) = unbounded_channel::<String>();
    let _ = tx.send(Message::text(json!({"id": first["id"], "ok": true}).to_string())).await;
    let _ = tx.send(Message::text(json!({"type": "daemon.hello", "root": "/root", "version": 1}).to_string())).await;
    loop {
        tokio::select! {
            out = pushed.recv() => {
                let Some(out) = out else { return };
                if tx.send(Message::text(out)).await.is_err() { return; }
            }
            frame = rx.next() => {
                let Some(Ok(frame)) = frame else { return };
                let text = match frame {
                    Message::Text(t) => t.to_string(),
                    Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
                    Message::Close(_) => return,
                    _ => continue,
                };
                let Ok(f) = serde_json::from_str::<Value>(&text) else { continue };
                shared.lock().unwrap().frames.push(f.clone());
                let id = f["id"].clone();
                let op = f["op"].as_str().unwrap_or("").to_string();
                let mut after: Vec<String> = Vec::new();
                let mut before: Vec<String> = Vec::new();
                let reply = match op.as_str() {
                    "ping" | "pty.attach" => json!({"id": id, "ok": true}),
                    "ports.watch" => {
                        shared.lock().unwrap().ports_watchers.push(push.clone());
                        json!({"id": id, "ok": true})
                    }
                    "inbox.watch" => {
                        shared.lock().unwrap().inbox_watchers.push(push.clone());
                        json!({"id": id, "ok": true})
                    }
                    "inbox.rescan" => {
                        let s = shared.lock().unwrap();
                        before = s.inbox.iter().map(|(p, b)| json!({"type": "inbox.file", "path": p, "bytes": b}).to_string()).collect();
                        json!({"id": id, "ok": true})
                    }
                    "pty.create" => json!({"id": id, "ok": true, "ptyId": "pty_1", "pid": 4242}),
                    "pty.write" => {
                        after.push(json!({"type": "pty.data", "ptyId": f["ptyId"], "data": f["data"]}).to_string());
                        json!({"id": id, "ok": true})
                    }
                    "fs.list" if !f["path"].is_string() => {
                        json!({"id": id, "ok": false, "error": "fs.list needs a path", "code": "bad-request"})
                    }
                    _ => json!({"id": id, "ok": false, "error": format!("unknown op: {op}")}),
                };
                for e in before { let _ = tx.send(Message::text(e)).await; }
                if tx.send(Message::text(reply.to_string())).await.is_err() { return; }
                for e in after { let _ = tx.send(Message::text(e)).await; }
            }
        }
    }
}
