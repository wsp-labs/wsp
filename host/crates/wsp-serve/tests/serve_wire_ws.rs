// SPDX-License-Identifier: AGPL-3.0-only
//! The websocket the runtime's own port speaks, read off raw bytes the way ws (8.21.3) reads and writes them:
//! spec/serve_wire.md, W4, W11-W13. Each test names the ws source it follows.
mod support;

use std::time::Duration;

use serde_json::{json, Value};
use support::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use wsp_serve::serve::{serve_runtime, RuntimeServer, ServeOptions};

const NO_ACCOUNT: &str = "this host keeps no account records; wsp up serves them";

fn frame(opcode: u8, fin: bool, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![if fin { 0x80 } else { 0 } | opcode];
    let len = payload.len();
    if len < 126 {
        out.push(0x80 | len as u8);
    } else {
        out.push(0x80 | 126);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    }
    let key = [9u8, 8, 7, 6];
    out.extend_from_slice(&key);
    out.extend(payload.iter().enumerate().map(|(i, b)| b ^ key[i & 3]));
    out
}

fn upgrade(method: &str, extra: &str) -> Vec<u8> {
    format!(
        "{method} /ws HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n{extra}\r\n"
    )
    .into_bytes()
}

/// Everything the host writes until it ends the connection or is quiet for `quiet_ms`.
async fn heard(stream: &mut TcpStream, quiet_ms: u64) -> Vec<u8> {
    let mut all = Vec::new();
    let mut chunk = vec![0u8; 65536];
    loop {
        match tokio::time::timeout(Duration::from_millis(quiet_ms), stream.read(&mut chunk)).await {
            Ok(Ok(n)) if n > 0 => all.extend_from_slice(&chunk[..n]),
            _ => return all,
        }
    }
}

/// The frames in the bytes after the head: (opcode, payload).
fn frames_of(bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 2 <= bytes.len() {
        let op = bytes[at] & 15;
        let mut len = (bytes[at + 1] & 127) as usize;
        let mut off = at + 2;
        if len == 126 {
            len = u16::from_be_bytes([bytes[off], bytes[off + 1]]) as usize;
            off += 2;
        }
        out.push((op, bytes[off..off + len].to_vec()));
        at = off + len;
    }
    out
}

async fn on_own_port(server: &dyn RuntimeServer) -> TcpStream {
    TcpStream::connect(("127.0.0.1", server.port())).await.unwrap()
}

async fn serving() -> std::rc::Rc<dyn RuntimeServer> {
    let rt = FakeRuntime::new();
    serve_runtime(rt, ServeOptions { port: Some(0), ..opts() }).await.expect("the runtime serves")
}

/// An upgraded socket that has signed in; what it heard so far is dropped.
async fn signed_in(server: &dyn RuntimeServer) -> TcpStream {
    let mut s = on_own_port(server).await;
    s.write_all(&upgrade("GET", "")).await.unwrap();
    let head = heard(&mut s, 200).await;
    assert!(head.starts_with(b"HTTP/1.1 101"), "{}", String::from_utf8_lossy(&head));
    s.write_all(&frame(1, true, json!({ "id": "a", "op": "auth", "token": TOKEN }).to_string().as_bytes())).await.unwrap();
    let got = heard(&mut s, 200).await;
    assert_eq!(frames_of(&got), vec![(1, br#"{"id":"a","ok":true}"#.to_vec())]);
    s
}

// ws websocket-server.js: a request with no upgrade reaches the server's own request listener, which answers 426
// with the body "Upgrade Required".
#[test]
fn a_plain_request_on_the_own_port_is_answered_426() {
    block(async {
        let server = serving().await;
        let mut s = on_own_port(&*server).await;
        s.write_all(b"GET /ws HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await.unwrap();
        let got = String::from_utf8_lossy(&heard(&mut s, 300).await).to_string();
        assert!(got.starts_with("HTTP/1.1 426 Upgrade Required\r\n"), "{got}");
        assert!(got.ends_with("\r\n\r\nUpgrade Required"), "{got}");
    })
}

// ws websocket-server.js handleUpgrade: a method other than GET is 405 with the body "Invalid HTTP method", on the
// runtime's own port as on an attached one.
#[test]
fn a_post_with_upgrade_headers_on_the_own_port_is_refused_405() {
    block(async {
        let server = serving().await;
        let mut s = on_own_port(&*server).await;
        s.write_all(&upgrade("POST", "")).await.unwrap();
        let got = String::from_utf8_lossy(&heard(&mut s, 300).await).to_string();
        assert!(got.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"), "{got}");
        assert!(got.ends_with("\r\n\r\nInvalid HTTP method"), "{got}");
    })
}

// ws websocket-server.js completeUpgrade: with no handleProtocols the first subprotocol offered is the one named.
#[test]
fn the_first_subprotocol_offered_is_named_in_the_101() {
    block(async {
        let server = serving().await;
        let mut s = on_own_port(&*server).await;
        s.write_all(&upgrade("GET", "Sec-WebSocket-Protocol: wsp, other\r\n")).await.unwrap();
        let got = String::from_utf8_lossy(&heard(&mut s, 300).await).to_string();
        assert!(got.starts_with("HTTP/1.1 101 Switching Protocols\r\n"), "{got}");
        assert!(got.contains("\r\nSec-WebSocket-Protocol: wsp\r\n"), "{got}");
    })
}

// ws websocket.js receiverOnPing: an open socket answers a ping with a pong of the same bytes.
#[test]
fn a_ping_is_answered_with_a_pong_of_its_bytes() {
    block(async {
        let server = serving().await;
        let mut s = signed_in(&*server).await;
        s.write_all(&frame(9, true, b"hi")).await.unwrap();
        assert_eq!(frames_of(&heard(&mut s, 200).await), vec![(10, b"hi".to_vec())]);
    })
}

// ws receiver.js: the fragments of one message are one message, and a control frame may sit between them.
#[test]
fn the_fragments_of_one_message_are_read_as_one_frame() {
    block(async {
        let server = serving().await;
        let mut s = signed_in(&*server).await;
        let whole = json!({ "id": 3, "op": "account.get" }).to_string();
        let (a, rest) = whole.as_bytes().split_at(5);
        let (b, c) = rest.split_at(7);
        let mut bytes = frame(1, false, a);
        bytes.extend(frame(9, true, b"p"));
        bytes.extend(frame(0, false, b));
        bytes.extend(frame(0, true, c));
        s.write_all(&bytes).await.unwrap();
        let got = frames_of(&heard(&mut s, 300).await);
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(got[0], (10, b"p".to_vec()));
        let reply: Value = serde_json::from_slice(&got[1].1).unwrap();
        assert_eq!(reply, json!({ "id": 3, "ok": false, "error": NO_ACCOUNT }));
    })
}

// ws websocket.js receiverOnConclude: the peer's close is answered with its own code and reason, and the host ends the
// connection.
#[test]
fn a_close_is_echoed_with_its_code_and_reason() {
    block(async {
        let server = serving().await;
        let mut s = signed_in(&*server).await;
        let mut payload = 1001u16.to_be_bytes().to_vec();
        payload.extend_from_slice(b"bye");
        s.write_all(&frame(8, true, &payload)).await.unwrap();
        let got = heard(&mut s, 300).await;
        assert_eq!(frames_of(&got), vec![(8, payload)]);
    })
}

// ws receiver.js haveLength: a frame that declares more than maxPayload (100 MiB) is closed 1009 as its length is read,
// before any of its payload comes.
#[test]
fn a_frame_declared_past_the_ceiling_is_closed_1009() {
    block(async {
        let server = serving().await;
        let mut s = signed_in(&*server).await;
        let mut head = vec![0x81, 0x80 | 127];
        head.extend_from_slice(&((100u64 * 1024 * 1024) + 1).to_be_bytes());
        s.write_all(&head).await.unwrap();
        let got = frames_of(&heard(&mut s, 300).await);
        assert_eq!(got, vec![(8, 1009u16.to_be_bytes().to_vec())]);
    })
}
