// SPDX-License-Identifier: AGPL-3.0-only
//! The listener, the upgrades and the server's own calls over a real websocket on loopback: spec/serve_wire.md.
mod support;

use std::rc::Rc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use support::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::Message;
use wsp_core::devices::HeldDevice;
use wsp_serve::serve::{serve_runtime, Authed, RuntimeServer, ServeOptions, UpgradeHead, STOP_GRACE_MS, TICKET_TTL_MS};
use wsp_wire::wire::limits::HOST_STOPPING_CLOSE;
use wsp_words::exit::HostError;

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

async fn serve(rt: &Rc<FakeRuntime>, o: ServeOptions) -> Rc<dyn RuntimeServer> {
    serve_runtime(rt.clone(), o).await.expect("the runtime serves")
}

async fn dial(port: u16, path: &str) -> Result<Ws, String> {
    tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}{path}")).await.map(|(ws, _)| ws).map_err(|e| e.to_string())
}

async fn ask(ws: &mut Ws, frame: Value) -> Value {
    let id = frame["id"].clone();
    ws.send(Message::Text(frame.to_string().into())).await.unwrap();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await.expect("a reply in time") {
            Some(Ok(Message::Text(t))) => {
                let v: Value = serde_json::from_str(&t).unwrap();
                if v["id"] == id {
                    return v;
                }
            }
            Some(Ok(_)) => continue,
            other => panic!("the socket ended before its reply: {other:?}"),
        }
    }
}

/// The close code the client reads, waiting at most `secs`.
async fn close_code(ws: &mut Ws, secs: u64) -> Option<u16> {
    loop {
        match tokio::time::timeout(Duration::from_secs(secs), ws.next()).await {
            Ok(Some(Ok(Message::Close(Some(frame))))) => return Some(u16::from(frame.code)),
            Ok(Some(Ok(Message::Close(None)))) => return Some(1005),
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(_))) | Ok(None) => return None,
            Err(_) => panic!("no close within {secs} s"),
        }
    }
}

/// A host's own HTTP server, as the host would run it: it reads each request's head and hands the upgrade to the
/// runtime with the bytes after it.
async fn attached(server: Rc<dyn RuntimeServer>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::task::spawn_local(async move {
        while let Ok((mut stream, peer)) = listener.accept().await {
            let server = server.clone();
            tokio::task::spawn_local(async move {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if stream.read(&mut byte).await.unwrap_or(0) == 0 {
                        return;
                    }
                    head.push(byte[0]);
                }
                let text = String::from_utf8_lossy(&head).to_string();
                let mut lines = text.split("\r\n");
                let path = lines.next().unwrap_or("").split(' ').nth(1).unwrap_or("/").to_string();
                let headers = lines.filter_map(|l| l.split_once(": ")).map(|(k, v)| (k.to_string(), v.to_string())).collect();
                server.upgrade(UpgradeHead { path_and_query: path, headers, peer: Some(peer) }, Box::new(stream)).await;
            });
        }
    });
    port
}

#[test]
fn the_defaults_are_the_protocols_numbers() {
    // W1. serve.ts:327-331, 386-389
    assert_eq!(STOP_GRACE_MS, 1_000);
    assert_eq!(TICKET_TTL_MS, 300_000);
    block(async {
        let rt = FakeRuntime::new();
        let devices: Rc<dyn wsp_core::devices::DeviceDoor> = rt.devices.clone();
        rt.script.once::<Result<wsp_core::devices::IssuedCode, HostError>>(
            "devices.issue",
            Ok(wsp_core::devices::IssuedCode { code: "c".into(), expires_at: 1 }),
        );
        let served = served_with(&rt, ServeOptions { devices: Some(devices), ..opts() });
        let (socket, sink) = host_socket(&served).await;
        request(&socket, &sink, 1, "pair.issue", json!({})).await;
        assert_eq!(rt.script.calls_to("devices.issue")[0].args["ttlMs"], json!(600_000), "a pairing code lives PAIR_CODE_TTL_MS");
    })
}

#[test]
fn a_runtime_given_port_zero_binds_one_and_names_it() {
    // W2, W4. serve.test.ts:37
    block(async {
        let rt = FakeRuntime::new();
        let server = serve(&rt, ServeOptions { port: Some(0), ..opts() }).await;
        assert_ne!(server.port(), 0);
        let mut ws = dial(server.port(), "/").await.expect("the own port answers");
        assert_eq!(
            ask(&mut ws, json!({ "id": 1, "op": "auth", "token": "wrong" })).await,
            json!({ "id": 1, "ok": false, "error": "unauthorized", "kind": "auth" })
        );
        assert_eq!(close_code(&mut ws, 5).await, Some(4401));
        server.close().await.unwrap();
    })
}

#[test]
fn a_runtime_with_no_port_and_no_attach_names_port_zero() {
    // W2. serve.ts:1911
    block(async {
        let rt = FakeRuntime::new();
        let server = serve(&rt, opts()).await;
        assert_eq!(server.port(), 0);
        server.close().await.unwrap();
    })
}

#[test]
fn the_own_port_answers_any_path() {
    // W4. serve.ts:470 (no path option on the runtime's own server)
    block(async {
        let rt = FakeRuntime::new();
        let server = serve(&rt, ServeOptions { port: Some(0), ..opts() }).await;
        let mut ws = dial(server.port(), "/anything/here?x=1").await.expect("any path opens");
        assert_eq!(ask(&mut ws, json!({ "id": 1, "op": "auth", "token": TOKEN })).await, json!({ "id": 1, "ok": true }));
        server.close().await.unwrap();
    })
}

#[test]
fn an_upgrade_the_hosts_reading_refuses_is_closed_on_its_own_port_and_an_attached_one() {
    // W4, W5. serve.test.ts:287
    block(async {
        let rt = FakeRuntime::new();
        let allowed = Rc::new(std::cell::Cell::new(false));
        let reading = allowed.clone();
        let server = serve(&rt, ServeOptions { port: Some(0), origin_allowed: Some(Rc::new(move |_| reading.get())), ..opts() }).await;
        let app = attached(server.clone()).await;
        let own = dial(server.port(), "/").await.err().expect("refused on the own port");
        assert!(own.contains("401"), "the ws library's own refusal: {own}");
        let at_app = dial(app, "/ws").await.err().expect("refused on the attached server");
        assert!(at_app.contains("403"), "{at_app}");

        allowed.set(true);
        let mut own = dial(server.port(), "/").await.expect("the door decides, and now takes it");
        let mut at_app = dial(app, "/ws").await.expect("the attached one takes it too");
        assert_eq!(ask(&mut own, json!({ "id": 1, "op": "auth", "token": TOKEN })).await["ok"], json!(true));
        assert_eq!(ask(&mut at_app, json!({ "id": 1, "op": "auth", "token": TOKEN })).await["ok"], json!(true));
        server.close().await.unwrap();
    })
}

#[test]
fn an_attached_upgrade_on_another_path_is_answered_400() {
    // W5. serve.ts:481-488
    block(async {
        let rt = FakeRuntime::new();
        let server = serve(&rt, opts()).await;
        let app = attached(server.clone()).await;
        let refused = dial(app, "/not-ws").await.err().expect("another path is refused");
        assert!(refused.contains("400"), "{refused}");
        let mut ok = dial(app, "/ws?ticket=nothing").await;
        // The query is ignored by the path rule; the unknown ticket then closes the socket 4401.
        let ws = ok.as_mut().expect("the query does not change the path");
        assert_eq!(close_code(ws, 5).await, Some(4401));
        server.close().await.unwrap();
    })
}

#[test]
fn an_unmasked_frame_is_said_in_one_line_and_the_next_socket_is_answered() {
    // W6. serve.test.ts:136
    block(async {
        let rt = FakeRuntime::new();
        let server = serve(&rt, ServeOptions { port: Some(0), ..opts() }).await;
        let warned = wsp_core::console::capture();
        let mut sock = TcpStream::connect(("127.0.0.1", server.port())).await.unwrap();
        sock.write_all(format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n", server.port()).as_bytes()).await.unwrap();
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            sock.read_exact(&mut byte).await.unwrap();
            head.push(byte[0]);
        }
        assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101 "));
        // The four bytes of null as a text frame with its mask bit clear: refused at the wire, before any JSON.
        sock.write_all(&[0x81, 0x04, 0x6e, 0x75, 0x6c, 0x6c]).await.unwrap();
        let mut rest = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(5), sock.read_to_end(&mut rest)).await.expect("the socket is cut");
        assert_eq!(
            warned.lines(),
            vec!["socket from 127.0.0.1 dropped on a wire fault: Invalid WebSocket frame: MASK must be set".to_string()]
        );
        let mut next = dial(server.port(), "/").await.unwrap();
        assert_eq!(ask(&mut next, json!({ "id": 1, "op": "auth", "token": TOKEN })).await["ok"], json!(true));
        server.close().await.unwrap();
    })
}

#[test]
fn authorize_names_nobody_for_an_empty_token_and_the_host_for_its_own() {
    // W7. serve.ts:1917
    block(async {
        let rt = FakeRuntime::new();
        let devices: Rc<dyn wsp_core::devices::DeviceDoor> = rt.devices.clone();
        let server = serve(&rt, ServeOptions { devices: Some(devices), ..opts() }).await;
        assert_eq!(server.authorize(None).await, None);
        assert_eq!(server.authorize(Some("")).await, None);
        assert!(rt.script.calls_to("devices.match").is_empty(), "an empty token asks the device door nothing");
        assert_eq!(server.authorize(Some(TOKEN)).await, Some(Authed::Host));
        let device: HeldDevice = HeldDevice {
            view: serde_json::from_value(json!({ "id": "d-1", "name": "laptop", "createdAt": "x", "lastSeenAt": "x" })).unwrap(),
            road: None,
        };
        rt.script.once::<Result<Option<HeldDevice>, HostError>>("devices.match", Ok(Some(device.clone())));
        assert_eq!(server.authorize(Some("device-token")).await, Some(Authed::Device(device)));
        assert!(rt.script.calls_to("devices.seen").is_empty(), "a route reading a token moves no last seen");
        server.close().await.unwrap();
    })
}

#[test]
fn revoke_device_cuts_every_socket_that_device_holds() {
    // W8. pairing.test.ts
    block(async {
        let rt = FakeRuntime::new();
        let devices: Rc<dyn wsp_core::devices::DeviceDoor> = rt.devices.clone();
        let server = serve(&rt, ServeOptions { port: Some(0), devices: Some(devices), ..opts() }).await;
        let device: HeldDevice = HeldDevice {
            view: serde_json::from_value(json!({ "id": "d-1", "name": "laptop", "createdAt": "x", "lastSeenAt": "x" })).unwrap(),
            road: None,
        };
        rt.script.on("devices.match", move |_| Ok::<Option<HeldDevice>, HostError>(Some(device.clone())));
        let mut a = dial(server.port(), "/").await.unwrap();
        assert_eq!(ask(&mut a, json!({ "id": 1, "op": "auth", "token": "device-token" })).await["ok"], json!(true));
        let mut host = dial(server.port(), "/").await.unwrap();
        assert_eq!(ask(&mut host, json!({ "id": 1, "op": "auth", "token": TOKEN })).await["ok"], json!(true));
        rt.script.once::<Result<bool, HostError>>("devices.revoke", Ok(true));
        assert!(server.revoke_device("d-1").await.unwrap());
        assert_eq!(close_code(&mut a, 5).await, Some(4401));
        assert_eq!(
            ask(&mut host, json!({ "id": 2, "op": "auth", "token": TOKEN })).await["ok"],
            json!(true),
            "the host's own socket stays"
        );
        server.close().await.unwrap();
    })
}

#[test]
fn a_stop_closes_every_client_with_4001() {
    // W9. serve.test.ts:44
    block(async {
        let rt = FakeRuntime::new();
        let server = serve(&rt, ServeOptions { port: Some(0), ..opts() }).await;
        let mut c = dial(server.port(), "/").await.unwrap();
        ask(&mut c, json!({ "id": 1, "op": "auth", "token": TOKEN })).await;
        let closing = tokio::task::spawn_local({
            let server = server.clone();
            async move { server.close().await }
        });
        assert_eq!(close_code(&mut c, 5).await, Some(HOST_STOPPING_CLOSE as u16));
        drop(c);
        closing.await.unwrap().unwrap();
        assert!(dial(server.port(), "/").await.is_err(), "a closed server takes no new socket");
        let _ = CloseCode::Normal;
    })
}

#[test]
fn a_client_that_never_answers_the_close_is_cut_after_the_grace() {
    // W9. serve.ts:1923-1926
    block(async {
        let rt = FakeRuntime::new();
        let server = serve(&rt, ServeOptions { port: Some(0), ..opts() }).await;
        let mut sock = TcpStream::connect(("127.0.0.1", server.port())).await.unwrap();
        sock.write_all(format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n", server.port()).as_bytes()).await.unwrap();
        let started = std::time::Instant::now();
        let closing = tokio::task::spawn_local({
            let server = server.clone();
            async move { server.close().await }
        });
        // Read everything the server writes and never answer: the socket ends once the grace runs out.
        let mut rest = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(5), sock.read_to_end(&mut rest)).await.expect("the socket is cut");
        let took = started.elapsed();
        assert!(took >= Duration::from_millis(900), "not cut before the grace: {took:?}");
        assert!(took < Duration::from_millis(3_000), "cut soon after the grace: {took:?}");
        closing.await.unwrap().unwrap();
    })
}
