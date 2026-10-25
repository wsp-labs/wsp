// SPDX-License-Identifier: AGPL-3.0-only
//! A host as far as a tool call reaches one, shared by the cases that dial it.
// Each case binary uses its own part of this.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

/// The close a host sends as it stops, which a wait dials through.
const STOPPING_CLOSE: u16 = 4001;

/// What a host answers: each op with its recorded frame, the frames it pushes while an op is under way, before its
/// reply, and the op after whose reply it lets the socket go as it stops. An op with no recorded frame is refused as the
/// recorded host refused it, and the events are subscribed to without one.
#[derive(Default, Clone)]
pub struct Script {
    pub replies: BTreeMap<String, String>,
    pub pushed: BTreeMap<String, Vec<String>>,
    pub closes: Option<String>,
}

/// The frames a scripted host was asked, and how many of its sockets are still open.
pub struct Asked {
    frames: Arc<Mutex<Vec<Value>>>,
    open: watch::Sender<usize>,
}

impl Asked {
    /// Every frame asked once each socket is closed: a call can answer on one reply while a frame it wrote beside it
    /// is still on the wire, and the host reads everything before the close.
    pub async fn settled(&self, named: &str) -> Vec<Value> {
        let mut open = self.open.subscribe();
        let closed = tokio::time::timeout(Duration::from_secs(10), open.wait_for(|n| *n == 0)).await;
        assert!(closed.is_ok(), "{named}: a socket to the host stayed open after the call answered");
        self.frames.lock().unwrap().clone()
    }
}

/// One socket counted open until it goes, however its loop ends.
struct Open(watch::Sender<usize>);

impl Open {
    fn new(open: &watch::Sender<usize>) -> Open {
        open.send_modify(|n| *n += 1);
        Open(open.clone())
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        self.0.send_modify(|n| *n -= 1);
    }
}

/// The release the record names, which a host of this line's own release says it is.
pub fn own_release() -> String {
    let server: Value = serde_json::from_str(include_str!("../../record/server.json")).unwrap();
    server["version"].as_str().unwrap().to_owned()
}

/// The sha256 of a token in hex, as a host's page carries it.
pub fn digest(token: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, token.as_bytes()).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// The page a host on loopback serves at `/`: its one boot line, carrying the digest of `token`, its socket's path and
/// its release, as packages/host/src/server.ts writes it.
pub fn wsp_page(token: &str) -> String {
    let boot = json!({ "tokenHash": digest(token), "wsPath": "/ws", "paired": true, "version": own_release() });
    format!("<!doctype html><html><head><script>window.__WSP__ = {boot};</script></head><body></body></html>")
}

/// A connection to a fake host: one that asks to upgrade comes back to be upgraded; any other request is answered
/// with `page` and closed, as a host answers the GET the token is held behind.
pub async fn upgrade_or_page(mut tcp: TcpStream, page: &str) -> Option<TcpStream> {
    let mut head = [0u8; 2048];
    let read = tcp.peek(&mut head).await.ok()?;
    if String::from_utf8_lossy(&head[..read]).to_ascii_lowercase().contains("upgrade: websocket") {
        return Some(tcp);
    }
    let _ = tcp.read(&mut head).await;
    let answer = format!("HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}", page.len());
    let _ = tcp.write_all(answer.as_bytes()).await;
    let _ = tcp.shutdown().await;
    None
}

/// A host on loopback that takes the token it was given and answers as `script` says, under the id each op was asked
/// with, and every op past the token and the events it was asked, without its id. One port, as the host's: a plain
/// request gets the page a host serves, carrying the digest of the token, and the runtime's socket is upgraded on its
/// path alone.
// The path check is tungstenite's handshake callback, whose error type is tungstenite's to size.
#[allow(clippy::result_large_err)]
pub async fn scripted(token: &'static str, script: Script) -> (u16, Asked) {
    let asked = Asked { frames: Arc::new(Mutex::new(Vec::new())), open: watch::channel(0).0 };
    let noted = asked.frames.clone();
    let open = asked.open.clone();
    // How many times each op was asked, so the frame recorded under `<op> #<n>` answers its nth ask.
    let seen = Arc::new(Mutex::new(std::collections::HashMap::<String, usize>::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let script = script.clone();
            let noted = noted.clone();
            let seen = seen.clone();
            let open = open.clone();
            tokio::spawn(async move {
                let Some(tcp) = upgrade_or_page(tcp, &wsp_page(token)).await else { return };
                let _open = Open::new(&open);
                let on_path = |asked: &Request, answer: Response| {
                    assert_eq!(asked.uri().path(), "/ws", "the runtime's socket is dialled on its path");
                    Ok(answer)
                };
                let mut ws = tokio_tungstenite::accept_hdr_async(tcp, on_path).await.unwrap();
                while let Some(Ok(Message::Text(text))) = ws.next().await {
                    let asked: Value = serde_json::from_str(&text).unwrap();
                    let id = asked["id"].as_u64().unwrap();
                    let op = asked["op"].as_str().unwrap();
                    if op != "auth" && op != "events.subscribe" {
                        let mut frame = asked.clone();
                        frame.as_object_mut().unwrap().remove("id");
                        if frame.get("requestId").is_some() {
                            frame["requestId"] = json!("<request id>");
                        }
                        noted.lock().unwrap().push(frame);
                    }
                    let reply = if op == "auth" {
                        assert_eq!(asked["token"], token);
                        json!({ "id": id, "ok": true }).to_string()
                    } else if let Some(recorded) = {
                        let nth = {
                            let mut seen = seen.lock().unwrap();
                            let count = seen.entry(op.to_owned()).or_insert(0);
                            *count += 1;
                            *count
                        };
                        script.replies.get(&format!("{op} #{nth}")).or_else(|| script.replies.get(op))
                    } {
                        // The recorded frame opens with the id it was recorded under; the rest is the host's bytes.
                        let rest = recorded.strip_prefix(r#"{"id":1,"#).expect("a frame recorded with its id first");
                        format!(r#"{{"id":{id},{rest}"#)
                    } else if op == "events.subscribe" {
                        json!({ "id": id, "ok": true }).to_string()
                    } else {
                        json!({ "id": id, "ok": false, "error": format!("{op} is not in this record") }).to_string()
                    };
                    let mut sent = true;
                    for frame in script.pushed.get(op).into_iter().flatten() {
                        sent = sent && ws.send(Message::text(frame.clone())).await.is_ok();
                    }
                    sent = sent && ws.send(Message::text(reply)).await.is_ok();
                    if !sent {
                        break;
                    }
                    if script.closes.as_deref() == Some(op) {
                        let stopping = CloseFrame { code: CloseCode::from(STOPPING_CLOSE), reason: "".into() };
                        let _ = ws.close(Some(stopping)).await;
                        break;
                    }
                }
            });
        }
    });
    (port, asked)
}
