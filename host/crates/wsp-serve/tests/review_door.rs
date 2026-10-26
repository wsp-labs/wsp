// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the door against the TS at 65f6b89c7: each test is the TS's answer to the same bytes, read off node 22
//! running serve.ts over a raw socket (ws 8.21.3), and each is red at 5b43883ee.
mod support;

use std::rc::Rc;
use std::time::Duration;

use serde_json::{json, Value};
use support::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use wsp_serve::serve::{serve_runtime, RuntimeServer, ServeOptions, UpgradeHead};

const NO_ACCOUNT: &str = "this host keeps no account records; wsp up serves them";

#[derive(Debug, PartialEq)]
enum Ev {
    Http(String),
    Text(Value),
    Close(Option<u16>, String),
    Pong,
    End,
}

fn masked(opcode: u8, payload: &[u8], mask: bool) -> Vec<u8> {
    let mut out = vec![0x80 | opcode];
    let m = if mask { 0x80 } else { 0 };
    let len = payload.len();
    if len < 126 {
        out.push(m | len as u8);
    } else if len < 65536 {
        out.push(m | 126);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        out.push(m | 127);
        out.extend_from_slice(&(len as u64).to_be_bytes());
    }
    if mask {
        let key = [1u8, 2, 3, 4];
        out.extend_from_slice(&key);
        out.extend(payload.iter().enumerate().map(|(i, b)| b ^ key[i & 3]));
    } else {
        out.extend_from_slice(payload);
    }
    out
}

fn text(frame: &str) -> Vec<u8> {
    masked(1, frame.as_bytes(), true)
}

fn upgrade_bytes(path: &str, version: &str) -> Vec<u8> {
    format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: {version}\r\n\r\n"
    )
    .into_bytes()
}

/// Reads what the host sends until it ends the connection or is quiet for `quiet_ms`.
async fn read_all<S: tokio::io::AsyncRead + Unpin>(stream: &mut S, quiet_ms: u64, head_read: bool) -> Vec<Ev> {
    let mut buf: Vec<u8> = Vec::new();
    let mut evs = Vec::new();
    let mut upgraded = head_read;
    let mut chunk = vec![0u8; 65536];
    loop {
        let n = match tokio::time::timeout(Duration::from_millis(quiet_ms), stream.read(&mut chunk)).await {
            Err(_) => break,
            Ok(Ok(0)) | Ok(Err(_)) => {
                evs.push(Ev::End);
                break;
            }
            Ok(Ok(n)) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        if !upgraded {
            let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
            let head = String::from_utf8_lossy(&buf[..end]).to_string();
            let body = String::from_utf8_lossy(&buf[end + 4..]).to_string();
            upgraded = head.starts_with("HTTP/1.1 101");
            evs.push(Ev::Http(format!(
                "{}{}",
                head.lines().next().unwrap_or(""),
                if upgraded { String::new() } else { format!(" | {body}") }
            )));
            buf.drain(..end + 4);
            if !upgraded {
                buf.clear();
            }
        }
        while upgraded && buf.len() >= 2 {
            let op = buf[0] & 15;
            let mut len = (buf[1] & 127) as usize;
            let mut off = 2;
            if len == 126 {
                if buf.len() < 4 {
                    break;
                }
                len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
                off = 4;
            } else if len == 127 {
                if buf.len() < 10 {
                    break;
                }
                len = u64::from_be_bytes(buf[2..10].try_into().unwrap()) as usize;
                off = 10;
            }
            if buf.len() < off + len {
                break;
            }
            let p = buf[off..off + len].to_vec();
            buf.drain(..off + len);
            match op {
                1 => {
                    evs.push(Ev::Text(serde_json::from_slice(&p).unwrap_or_else(|_| Value::from(String::from_utf8_lossy(&p).to_string()))))
                }
                8 => evs.push(Ev::Close(
                    (p.len() >= 2).then(|| u16::from_be_bytes([p[0], p[1]])),
                    String::from_utf8_lossy(p.get(2..).unwrap_or(&[])).to_string(),
                )),
                10 => evs.push(Ev::Pong),
                _ => {}
            }
        }
    }
    evs
}

/// Reads the upgrade's answer up to its blank line, and no further.
async fn read_head<S: tokio::io::AsyncRead + Unpin>(stream: &mut S) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte)).await.expect("an answer").unwrap();
        assert_eq!(n, 1, "the upgrade is answered");
        head.push(byte[0]);
    }
    assert!(head.starts_with(b"HTTP/1.1 101"), "{}", String::from_utf8_lossy(&head));
}

async fn own_port(rt: &Rc<FakeRuntime>) -> Rc<dyn RuntimeServer> {
    serve_runtime(rt.clone(), ServeOptions { port: Some(0), ..opts() }).await.expect("the runtime serves")
}

/// An upgraded, authed raw socket on the own port.
async fn authed_raw(port: u16) -> TcpStream {
    let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s.write_all(&upgrade_bytes("/ws", "13")).await.unwrap();
    read_head(&mut s).await;
    s.write_all(&text(&json!({ "id": "a", "op": "auth", "token": TOKEN }).to_string())).await.unwrap();
    let got = read_all(&mut s, 300, true).await;
    assert_eq!(got.last(), Some(&Ev::Text(json!({ "id": "a", "ok": true }))), "{got:?}");
    s
}

// MAJOR. serve.ts:586-603 adds the pre-auth byte counter only `if (!authed)`, and a socket that redeemed a ticket
// is authed at serve.ts:526; spec 04 rule 4.10: both limits "never apply to a ticket socket". Socket::opened
// (src/serve/socket.rs:160-178) lets a ticket socket in without `through_door()`, so `through` stays false and
// `bytes_arrived` (socket.rs:240-251) cuts it 4401 "too many bytes before the auth frame" once it has sent 4,096
// bytes in all. Node: the 8 requests below are answered, no close.
#[test]
fn a_ticket_socket_has_no_byte_cap() {
    block(async {
        let rt = FakeRuntime::new();
        let server = own_port(&rt).await;
        let mut host = authed_raw(server.port()).await;
        host.write_all(&text(&json!({ "id": 1, "op": "ticket.issue", "purpose": "connect" }).to_string())).await.unwrap();
        let issued = read_all(&mut host, 300, true).await;
        let Some(Ev::Text(reply)) = issued.last() else { panic!("no ticket: {issued:?}") };
        let ticket = reply["ticket"].as_str().expect("a ticket").to_string();
        let mut s = TcpStream::connect(("127.0.0.1", server.port())).await.unwrap();
        s.write_all(&upgrade_bytes(&format!("/ws?ticket={ticket}"), "13")).await.unwrap();
        read_head(&mut s).await;
        let mut frames = Vec::new();
        for id in 0..8 {
            let frame = format!("{:<700}", json!({ "id": id, "op": "account.get" }).to_string());
            frames.extend(text(&frame));
        }
        s.write_all(&frames).await.unwrap();
        let got = read_all(&mut s, 500, true).await;
        let replies = got.iter().filter(|e| matches!(e, Ev::Text(v) if v["error"] == NO_ACCOUNT)).count();
        assert!(!got.iter().any(|e| matches!(e, Ev::Close(..))), "a ticket socket is not held to the pre-auth cap: {got:?}");
        assert_eq!(replies, 8, "{got:?}");
    })
}

// MAJOR. ws's default `maxPayload` is 100 MiB (ws 8.21.3 websocket-server.js:74), and serve.ts:470-472 sets none;
// spec 04 rule 4.5: "The largest frame is ws's default 100 MiB, past which ws closes 1009". tokio-tungstenite's
// defaults (16 MiB a frame, 64 MiB a message, src/serve/listener.rs:237 and :271 take them) drop the socket at 16 MiB
// with no close frame. Node answers this 20 MiB frame with its schema refusal.
#[test]
fn a_twenty_mib_frame_is_read() {
    block(async {
        let rt = FakeRuntime::new();
        let server = own_port(&rt).await;
        let mut s = authed_raw(server.port()).await;
        let frame = format!("{{\"id\":2,\"op\":\"nope\",\"pad\":\"{}\"}}", "x".repeat(20 * 1024 * 1024));
        s.write_all(&text(&frame)).await.expect("ws reads a frame up to 100 MiB; the host reset the connection while it was sent");
        let got = read_all(&mut s, 3000, true).await;
        let reply = got.iter().find_map(|e| match e {
            Ev::Text(v) if v["id"] == json!(2) => Some(v.clone()),
            _ => None,
        });
        let reply = reply.unwrap_or_else(|| panic!("a 20 MiB frame is answered, as ws reads up to 100 MiB: {got:?}"));
        assert_eq!(reply["ok"], json!(false));
        assert!(reply["error"].as_str().unwrap().contains("invalid_union_discriminator"), "{reply}");
    })
}

// MAJOR. Spec 08 rule 8.26 (Proposal 11, accepted by the owner): "A frame holding an unpaired surrogate escape MUST
// be read, not refused", each unpaired half read as U+FFFD. The TS's JSON.parse takes it (node echoes the id back as
// "\ud800"). src/serve/flat.rs refuses it as `invalid json`, and before auth that also closes the socket 4401.
#[test]
fn a_lone_surrogate_is_read_as_a_replacement_character() {
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = host_socket(&served).await;
        send_raw(&socket, r#"{"id":"a\ud800b","op":"account.get"}"#).await;
        assert_eq!(
            sink.frames().last().map(|f| f.to_string()),
            Some(json!({ "id": "a\u{fffd}b", "ok": false, "error": NO_ACCOUNT }).to_string()),
            "the frame is read with U+FFFD for the half, not refused as invalid json"
        );
        let (door, door_sink) = open(&served, "");
        send_raw(&door, &format!(r#"{{"id":1,"op":"auth","token":"{TOKEN}","note":"\ud83d"}}"#)).await;
        assert_eq!(door_sink.reply(1), Some(json!({ "id": 1, "ok": true })), "an auth frame with a lone half still lets the socket in");
    })
}

// MINOR. Spec 08 rule 8.27 and 04 rule 4.31: "a number past the double's range is Infinity (JSON.parse("1e400"))",
// and "1e999 parses as Infinity and comes back null". Node answers the op with id null; src/serve/flat.rs refuses
// the frame as `invalid json`.
#[test]
fn a_number_past_the_double_reads_as_infinity() {
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = host_socket(&served).await;
        send_raw(&socket, r#"{"id":1e400,"op":"account.get"}"#).await;
        assert_eq!(sink.frames().last().map(|f| f.to_string()), Some(json!({ "id": null, "ok": false, "error": NO_ACCOUNT }).to_string()));
    })
}

// MINOR. serve.ts:539-540, 709 and spec 04 rule 4.9 (09 rule 9.18): "a frame that arrives while the door is still
// deciding the last one is refused like a wrong op". ws emits both messages of one read synchronously
// (allowSynchronousEvents, ws 8.21.3), so the second frame finds `deciding` set while `whoIs` is awaited: node answers
// it `{id: 2, ok: false, error: "unauthorized", kind: "auth"}` and closes 4401, and the auth reply is never sent. The
// Rust door runs each frame as its own task (src/serve/listener.rs:320) and `auth` never yields for the host's token
// (src/serve/door.rs:475-509), so the first frame is decided before the second is read and both are served.
#[test]
fn a_second_frame_in_the_same_read_as_the_auth_is_refused() {
    block(async {
        let rt = FakeRuntime::new();
        let server = own_port(&rt).await;
        let mut s = TcpStream::connect(("127.0.0.1", server.port())).await.unwrap();
        s.write_all(&upgrade_bytes("/ws", "13")).await.unwrap();
        read_head(&mut s).await;
        let mut both = text(&json!({ "id": "a", "op": "auth", "token": TOKEN }).to_string());
        both.extend(text(&json!({ "id": 2, "op": "account.get" }).to_string()));
        s.write_all(&both).await.unwrap();
        let got = read_all(&mut s, 500, true).await;
        assert_eq!(
            got.iter().filter(|e| !matches!(e, Ev::End)).collect::<Vec<_>>(),
            vec![
                &Ev::Text(json!({ "id": 2, "ok": false, "error": "unauthorized", "kind": "auth" })),
                &Ev::Close(Some(4401), "unauthorized".into())
            ],
            "{got:?}"
        );
    })
}

// MINOR. On a frame wrong at the wire, ws closes with the fault's own code before it emits `error`
// (ws 8.21.3 websocket.js receiverOnError: `websocket.close(err[kStatusCode])`), and serve.ts:500-503 then terminates:
// node sends close 1002 for an unmasked frame and a reserved opcode, 1007 for invalid UTF-8 in a text frame, and an
// empty reason for a close frame carrying 1005. src/serve/listener.rs:333-341 breaks on the error with no close frame
// (the client reads 1006), and answers the 1005 close with the reason "Protocol violation".
#[test]
fn a_wire_fault_is_closed_with_its_code() {
    block(async {
        let rt = FakeRuntime::new();
        let server = own_port(&rt).await;
        let cases: [(&str, Vec<u8>, Ev); 4] = [
            ("unmasked", masked(1, br#"{"id":2,"op":"account.get"}"#, false), Ev::Close(Some(1002), String::new())),
            ("reserved opcode", masked(3, b"x", true), Ev::Close(Some(1002), String::new())),
            ("invalid utf-8", masked(1, &[0xff, 0xfe, 0xfd], true), Ev::Close(Some(1007), String::new())),
            ("close 1005", masked(8, &1005u16.to_be_bytes(), true), Ev::Close(Some(1002), String::new())),
        ];
        for (name, bytes, want) in cases {
            let mut s = authed_raw(server.port()).await;
            s.write_all(&bytes).await.unwrap();
            let got = read_all(&mut s, 1000, true).await;
            assert_eq!(got.first(), Some(&want), "{name}: {got:?}");
        }
    })
}

/// Hands one upgrade to the runtime as the host's HTTP server would, and reads what the runtime answers.
async fn attached_answer(server: &Rc<dyn RuntimeServer>, headers: Vec<(&str, &str)>) -> String {
    let (mut client, io) = tokio::io::duplex(4096);
    let head = UpgradeHead {
        path_and_query: "/ws".into(),
        headers: headers.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        peer: Some("127.0.0.1:50000".parse().unwrap()),
    };
    let s = server.clone();
    tokio::task::spawn_local(async move { s.upgrade(head, Box::new(io)).await });
    let mut buf = vec![0u8; 4096];
    let n = tokio::time::timeout(Duration::from_secs(2), client.read(&mut buf)).await.expect("an answer").unwrap();
    let text = String::from_utf8_lossy(&buf[..n]).to_string();
    let status = text.lines().next().unwrap_or("").to_string();
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    format!("{status} | {body}")
}

// MINOR. An attached upgrade goes through ws's `handleUpgrade` (serve.ts:489), which refuses before any handshake
// (ws 8.21.3 websocket-server.js handleUpgrade): a missing or malformed Sec-WebSocket-Key, a Sec-WebSocket-Version
// other than 8 or 13, and an Upgrade header that is not `websocket` each get 400 with ws's sentence as the body (and a
// method other than GET 405, which UpgradeHead cannot carry). src/serve/listener.rs:257-273 checks only that a key is
// present and answers 101 to the rest; its 400 for a missing key carries no body.
#[test]
fn an_upgrade_ws_refuses_is_refused_in_its_words() {
    block(async {
        let rt = FakeRuntime::new();
        let server = serve_runtime(rt.clone(), opts()).await.expect("the runtime serves");
        let key = ("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==");
        let upgrade = ("Upgrade", "websocket");
        let v13 = ("Sec-WebSocket-Version", "13");
        let cases: [(&str, Vec<(&str, &str)>, &str); 5] = [
            ("no key", vec![upgrade, v13], "HTTP/1.1 400 Bad Request | Missing or invalid Sec-WebSocket-Key header"),
            (
                "short key",
                vec![upgrade, ("Sec-WebSocket-Key", "short"), v13],
                "HTTP/1.1 400 Bad Request | Missing or invalid Sec-WebSocket-Key header",
            ),
            (
                "version 12",
                vec![upgrade, key, ("Sec-WebSocket-Version", "12")],
                "HTTP/1.1 400 Bad Request | Missing or invalid Sec-WebSocket-Version header",
            ),
            ("no version", vec![upgrade, key], "HTTP/1.1 400 Bad Request | Missing or invalid Sec-WebSocket-Version header"),
            ("upgrade h2c", vec![("Upgrade", "h2c"), key, v13], "HTTP/1.1 400 Bad Request | Invalid Upgrade header"),
        ];
        for (name, headers, want) in cases {
            assert_eq!(attached_answer(&server, headers).await, want, "{name}");
        }
    })
}

// NIT. A request answered without an await and a close frame in the same read: ws emits the message first and the
// handler's reply goes out before the close is echoed (node: the account refusal, then close 1000). The Rust pump
// spawns the frame's task (src/serve/listener.rs:320), reads the close in the same turn, marks the sink shut and
// breaks, so the reply is dropped.
#[test]
fn a_reply_before_a_close_in_the_same_read_is_sent() {
    block(async {
        let rt = FakeRuntime::new();
        let server = own_port(&rt).await;
        let mut s = authed_raw(server.port()).await;
        let mut both = text(&json!({ "id": 2, "op": "account.get" }).to_string());
        both.extend(masked(8, &1000u16.to_be_bytes(), true));
        s.write_all(&both).await.unwrap();
        let got = read_all(&mut s, 500, true).await;
        assert_eq!(
            got.iter().filter(|e| !matches!(e, Ev::End)).collect::<Vec<_>>(),
            vec![&Ev::Text(json!({ "id": 2, "ok": false, "error": NO_ACCOUNT })), &Ev::Close(Some(1000), String::new())],
            "{got:?}"
        );
    })
}

// NIT. On the runtime's own port, ws takes the bytes that came in the same read as the upgrade request (its `head`)
// as the first frames. tungstenite's server handshake refuses them (ProtocolError::JunkAfterRequest), so
// src/serve/listener.rs:237-241 drops the connection without a 101. Node: 101, then the auth reply. The host serves
// through `attach` (packages/host/src/server.ts:625), so only a runtime on its own port meets this.
#[test]
fn the_own_port_reads_a_frame_sent_with_the_upgrade() {
    block(async {
        let rt = FakeRuntime::new();
        let server = own_port(&rt).await;
        let mut s = TcpStream::connect(("127.0.0.1", server.port())).await.unwrap();
        let mut bytes = upgrade_bytes("/ws", "13");
        bytes.extend(text(&json!({ "id": "a", "op": "auth", "token": TOKEN }).to_string()));
        s.write_all(&bytes).await.unwrap();
        let got = read_all(&mut s, 500, false).await;
        assert_eq!(got.first(), Some(&Ev::Http("HTTP/1.1 101 Switching Protocols".into())), "{got:?}");
        assert!(got.contains(&Ev::Text(json!({ "id": "a", "ok": true }))), "{got:?}");
    })
}

// MINOR. serve.ts:1919-1932 and spec 04 rule 4.41: every client gets close 4001 `stopping`, and "a 1,000 ms timer
// terminates whatever has not finished its close", which fires the socket's `close` and so its detaches. The Rust
// pump writes each frame inside its select arm (src/serve/listener.rs:290-299, `ws.send(message).await`), so a client
// that has stopped reading holds the pump in that write: the close and the cut queued behind it are never read,
// `close()` returns after its two waits (listener.rs:138-155) with the connection open, and the socket's event
// listener stays on the bus.
async fn stop_with_backlog(events: u64) -> usize {
    {
        let rt = FakeRuntime::new();
        let server = serve_runtime(rt.clone(), opts()).await.expect("the runtime serves");
        let (mut client, io) = tokio::io::duplex(1024);
        let head = UpgradeHead {
            path_and_query: "/ws".into(),
            headers: vec![
                ("Upgrade".into(), "websocket".into()),
                ("Sec-WebSocket-Key".into(), "dGhlIHNhbXBsZSBub25jZQ==".into()),
                ("Sec-WebSocket-Version".into(), "13".into()),
            ],
            peer: Some("127.0.0.1:50000".parse().unwrap()),
        };
        let s = server.clone();
        tokio::task::spawn_local(async move { s.upgrade(head, Box::new(io)).await });
        read_head(&mut client).await;
        client.write_all(&text(&json!({ "id": "a", "op": "auth", "token": TOKEN }).to_string())).await.unwrap();
        client.write_all(&text(&json!({ "id": 1, "op": "events.subscribe" }).to_string())).await.unwrap();
        let got = read_all(&mut client, 300, true).await;
        assert!(got.contains(&Ev::Text(json!({ "id": 1, "ok": true, "seq": 0, "stream": "stream-a" }))), "{got:?}");
        assert!(rt.bus.listening() > 0);
        for seq in 0..events {
            rt.bus.emit(&FakeBus::event(json!({ "type": "workspace.napped", "workspaceId": "ws_1", "seq": seq })));
        }
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        server.close().await.unwrap();
        rt.bus.listening()
    }
}

#[test]
fn a_stop_cuts_a_client_that_stopped_reading() {
    let left = block(stop_with_backlog(200));
    assert_eq!(left, 0, "the stop cut the socket that never read its close, so its detaches ran");
}

/// Control, green at HEAD: the same client with nothing queued for it is cut after the grace.
#[test]
fn control_a_stop_cuts_a_quiet_client() {
    assert_eq!(block(stop_with_backlog(0)), 0);
}

// MAJOR. ws destroys a socket 30 s after its own close frame when the peer never answers (`closeTimeout`, ws 8.21.3
// websocket.js `setCloseTimer`; spec 04's limits table: "close handshake wait | 30 s | always"), so a client the door
// timed out (4408) or refused (4401) is cut. Node, client silent after the 4408: TCP end 30 s after the close. The
// Rust pump (src/serve/listener.rs:283-346) ends only on the peer's close, its EOF, an error or a stop's cut, so a
// client that never signs in and never answers the close keeps its connection, its task and its socket forever.
#[test]
fn a_client_that_never_answers_the_close_is_cut_after_thirty_seconds() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().start_paused(true).build().unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async {
        let fake = FakeRuntime::new();
        let server = serve_runtime(fake.clone(), ServeOptions { auth_deadline_ms: Some(100), ..opts() }).await.expect("the runtime serves");
        let (mut client, io) = tokio::io::duplex(4096);
        let head = UpgradeHead {
            path_and_query: "/ws".into(),
            headers: vec![
                ("Upgrade".into(), "websocket".into()),
                ("Sec-WebSocket-Key".into(), "dGhlIHNhbXBsZSBub25jZQ==".into()),
                ("Sec-WebSocket-Version".into(), "13".into()),
            ],
            peer: Some("127.0.0.1:50000".parse().unwrap()),
        };
        let s = server.clone();
        tokio::task::spawn_local(async move { s.upgrade(head, Box::new(io)).await });
        read_head(&mut client).await;
        let got = read_all(&mut client, 1_000, true).await;
        assert_eq!(got, vec![Ev::Close(Some(4408), "no auth frame arrived in time".into())]);
        let mut buf = [0u8; 64];
        let ended = tokio::time::timeout(Duration::from_secs(31), client.read(&mut buf)).await;
        assert!(matches!(ended, Ok(Ok(0))), "the host cuts a client that never answered its close 30 s after it: {ended:?}");
    });
}
