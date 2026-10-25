// SPDX-License-Identifier: AGPL-3.0-only
//! What every test of this crate shares: a scripted runtime that records each call, a recording socket sink, and
//! the helpers that drive one socket's frames. Each test file declares `mod support;`.
#![allow(dead_code, unused_imports)]
pub mod fake_bus;
pub mod fake_devices;
pub mod fake_places;
pub mod fake_rt;
pub mod script;
pub mod sink;

use std::future::Future;
use std::rc::Rc;

use serde_json::{json, Value};
use wsp_serve::serve::door::Served;
use wsp_serve::serve::socket::{Incoming, Socket};
use wsp_serve::serve::{ServeOptions, UpgradeHead};

pub use fake_bus::FakeBus;
pub use fake_devices::FakeDevices;
pub use fake_places::FakePlaceDoor;
pub use fake_rt::FakeRuntime;
pub use script::{Call, Script};
pub use sink::RecordingSink;

/// Runs `f` on a current-thread runtime inside a `LocalSet`, as the host runs (chapter 03, one thread).
pub fn block<F: Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, f)
}

/// The host token every test serves with.
pub const TOKEN: &str = "secret";

/// Options with the host token and nothing else wired.
pub fn opts() -> ServeOptions {
    ServeOptions { auth_token: TOKEN.to_string(), ..ServeOptions::default() }
}

/// An upgrade of `/ws` from loopback, with an optional query.
pub fn upgrade(query: &str) -> UpgradeHead {
    UpgradeHead { path_and_query: format!("/ws{query}"), headers: vec![], peer: Some("127.0.0.1:50000".parse().unwrap()) }
}

/// A socket opened on `served`, not let in yet.
pub fn open(served: &Rc<Served>, query: &str) -> (Rc<Socket>, Rc<RecordingSink>) {
    let sink = RecordingSink::new();
    let socket = Socket::opened(served, upgrade(query), sink.clone());
    (socket, sink)
}

/// Sends one text frame and waits for everything it starts to settle as far as one frame goes.
pub async fn send(socket: &Rc<Socket>, frame: Value) {
    socket.on_message(Incoming::Text(frame.to_string())).await;
}

/// Sends a raw text frame.
pub async fn send_raw(socket: &Rc<Socket>, text: &str) {
    socket.on_message(Incoming::Text(text.to_string())).await;
}

/// `{"id": <n>, "op": op, ...fields}`.
pub fn frame(id: u64, op: &str, fields: Value) -> Value {
    let mut frame = json!({ "id": id, "op": op });
    if let Value::Object(more) = fields {
        for (k, v) in more {
            frame[k] = v;
        }
    }
    frame
}

/// Sends `{"id": <n>, "op": op, ...fields}` and answers the reply carrying that id. The frame must pass the wire's
/// schema (`parse_runtime_request`, which is implemented), so a test written with a frame the door would refuse
/// panics here, not inside the crate; a case about a refused frame uses `request_unchecked`.
pub async fn request(socket: &Rc<Socket>, sink: &RecordingSink, id: u64, op: &str, fields: Value) -> Value {
    let frame = frame(id, op, fields);
    if let Err(e) = wsp_wire::wire::runtime_ops::parse_runtime_request(&frame) {
        panic!("the test's frame does not pass the wire's schema: {e:?}; frame {frame}");
    }
    send(socket, frame).await;
    sink.reply(id).unwrap_or_else(|| panic!("no reply to {op} (id {id}); sent: {:?}", sink.frames()))
}

/// `request` without the schema check, for the cases about a frame the schema refuses.
pub async fn request_unchecked(socket: &Rc<Socket>, sink: &RecordingSink, id: u64, op: &str, fields: Value) -> Value {
    send(socket, frame(id, op, fields)).await;
    sink.reply(id).unwrap_or_else(|| panic!("no reply to {op} (id {id}); sent: {:?}", sink.frames()))
}

/// `n` bytes of `fill` in base64: the place link's nonces (32), halves (32), keys (44) and signatures (64) are
/// fixed-size base64 on the wire.
pub fn b64(n: usize, fill: u8) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(vec![fill; n])
}

/// A socket let in with the host's own token.
pub async fn host_socket(served: &Rc<Served>) -> (Rc<Socket>, Rc<RecordingSink>) {
    let (socket, sink) = open(served, "");
    let reply = request(&socket, &sink, 0, "auth", json!({ "token": TOKEN })).await;
    assert_eq!(reply["ok"], json!(true), "the host token lets a socket in: {reply}");
    (socket, sink)
}

/// The served state over a fake runtime with `opts`.
pub fn served_with(rt: &Rc<FakeRuntime>, opts: ServeOptions) -> Rc<Served> {
    Served::new(rt.clone(), opts).expect("a host token serves")
}

/// Captures every line `opts.log` is handed.
pub fn log_lines() -> (wsp_serve::serve::LogFn, Rc<std::cell::RefCell<Vec<String>>>) {
    let lines = Rc::new(std::cell::RefCell::new(Vec::new()));
    let kept = lines.clone();
    (Rc::new(move |line: &str| kept.borrow_mut().push(line.to_string())), lines)
}
