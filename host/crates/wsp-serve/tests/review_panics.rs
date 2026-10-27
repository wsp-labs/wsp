// SPDX-License-Identifier: AGPL-3.0-only
//! Reader tests for panics a frame, a socket or the environment can reach (review of #2448, branch wt-2448-serve-review).
//!
//! A stack overflow is a process abort that no `catch_unwind` and no task boundary stops, so each of those tests runs
//! its body in a copy of this test binary with an 8 MiB stack (a main thread's) and fails when the copy dies. The
//! red line of such a test is the copy's own `thread '..' has overflowed its stack` and `fatal runtime error`.
mod support;

use std::rc::Rc;

use futures::{SinkExt, StreamExt};
use serde_json::json;
use support::*;
use tokio_tungstenite::tungstenite::Message;
use wsp_serve::serve::{serve_runtime, ServeOptions};

/// Nesting levels of the frames below. The debug build overflows an 8 MiB stack between 1,000 and 5,000 levels; a
/// frame of this depth is 200 KB, and the wire accepts frames up to 16 MiB.
const LEVELS: usize = 100_000;

fn deep(levels: usize) -> String {
    format!("{}0{}", "[".repeat(levels), "]".repeat(levels))
}

/// Runs `body` in a copy of this binary so an abort is a failed assertion here and not a dead test run.
fn isolated(test: &str, body: impl FnOnce()) {
    if std::env::var_os("REVIEW_PANICS_CHILD").is_some() {
        return body();
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env("REVIEW_PANICS_CHILD", "1")
        .env("RUST_MIN_STACK", "8388608")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the copy that ran this test died: {:?}; its stderr ends: {}",
        out.status,
        &err[err.len().saturating_sub(700)..]
    );
}

/// A new socket on the same server is let in and answered: the host is still alive and serving.
async fn another_socket_is_served(port: u16) {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/")).await.expect("the server takes a new socket");
    ws.send(Message::Text(json!({ "id": 1, "op": "auth", "token": TOKEN }).to_string().into())).await.unwrap();
    match tokio::time::timeout(std::time::Duration::from_secs(5), ws.next()).await.expect("the server answers") {
        Some(Ok(Message::Text(t))) => assert!(t.contains(r#""ok":true"#), "{t}"),
        other => panic!("{other:?}"),
    }
}

// BLOCKER: one authed frame takes down the host and every socket it holds, with no catch. The frame is a valid
// `workspaces.list` that carries one key the op does not name; the schema drops such a key, and the TS answers it.
// Rust: host/crates/wsp-wire/src/wire/runtime_ops.rs:714-715 (`RuntimeRequest::deserialize` reads the frame into a
// `Value`, clones it and hands the clone to the internally tagged `RuntimeOp`, which buffers every key, unknown ones
// too, recursively; the buffer and the clone drop recursively), reached by host/crates/wsp-wire/src/types/check.rs:649
// from host/crates/wsp-serve/src/serve/door.rs:238. flat.rs reads and drops the frame without recursion, and this
// read undoes it.
// TS: packages/runtime/src/serve.ts:669 (`RuntimeRequest.safeParse`, which strips unknown keys without descending);
// run under node at 20,000, 200,000 and 2,000,000 levels it answers {"id":5,"ok":true,"workspaces":[]} each time.
#[test]
fn a_valid_frame_with_a_deeply_nested_unknown_key_is_answered() {
    isolated("a_valid_frame_with_a_deeply_nested_unknown_key_is_answered", || {
        block(async {
            let rt = FakeRuntime::new();
            rt.script
                .on("workspaces.list", |_| Ok::<Vec<wsp_wire::views::workspace_more::WorkspaceView>, wsp_words::exit::HostError>(vec![]));
            let served = served_with(&rt, opts());
            let (socket, sink) = host_socket(&served).await;
            // Ruling 2 of fix round 3: the intake reads a frame to 506 levels and refuses a deeper one as a frame it cannot
            // read ("invalid json", serve.ts:640), never a shortened value. The frame at 100,000 levels, which node answers
            // ok:true because it reads whole and the schema strips the key, is refused; the host keeps serving.
            send_raw(&socket, &format!(r#"{{"id":5,"op":"workspaces.list","x":{}}}"#, deep(LEVELS))).await;
            assert_eq!(sink.frames().last().unwrap(), &json!({ "id": null, "ok": false, "error": "invalid json" }));
            // The deepest frame the intake reads: 506 levels with the frame's own, answered as node answers it.
            send_raw(&socket, &format!(r#"{{"id":6,"op":"workspaces.list","x":{}}}"#, deep(505))).await;
            assert_eq!(sink.reply(6).unwrap()["ok"], json!(true));
        })
    })
}

// BLOCKER: the same frame from a client that has not signed in, over a real socket, takes the host down. The schema
// read at door.rs:238 runs before the door looks at `authed` (door.rs:247), so no token is needed. The pre-auth cap
// of 4,096 bytes closes the socket 4401 (this test sees that close first) but the frame it was counting is still
// handed to `on_message` (listener.rs:320), which reads it. Rust: runtime_ops.rs:714-715 as above.
// TS: serve.ts:580-584 and 669; the same frame at 20,000, 200,000 and 2,000,000 levels closes the socket 4401 "too
// many bytes before the auth frame" and the host keeps serving.
#[test]
fn an_unauthenticated_deeply_nested_frame_over_a_real_socket_does_not_take_the_host_down() {
    isolated("an_unauthenticated_deeply_nested_frame_over_a_real_socket_does_not_take_the_host_down", || {
        block(async {
            let rt = FakeRuntime::new();
            let server = serve_runtime(rt.clone(), ServeOptions { port: Some(0), ..opts() }).await.unwrap();
            let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/", server.port())).await.unwrap();
            let frame = format!(r#"{{"id":5,"op":"auth","token":"x","pad":{}}}"#, deep(LEVELS));
            ws.send(Message::Text(frame.into())).await.unwrap();
            let closed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    match ws.next().await {
                        Some(Ok(Message::Close(frame))) => return frame.map(|f| u16::from(f.code)),
                        Some(Ok(_)) => continue,
                        _ => return None,
                    }
                }
            })
            .await
            .expect("the server closes the socket");
            assert_eq!(closed, Some(4401));
            another_socket_is_served(server.port()).await;
            server.close().await.unwrap();
        })
    })
}

// BLOCKER: a deeply nested `id` takes the host down before the schema is read, so repairing the read above does not
// close it. Three independent sites, each overflowing alone (checked one at a time at 20,000 levels on 8 MiB):
// host/crates/wsp-serve/src/serve/door.rs:222 (`frame.get("id").cloned()` clones the value recursively, for every
// frame, signed in or not); host/crates/wsp-serve/src/serve/socket.rs:219 (`Value::Object(payload).to_string()`
// writes the refusal that echoes the id, recursively); runtime_ops.rs:714-715 (the schema read, from 5,000 levels).
// TS: serve.ts:655 reads the same id and the refusal's `send` (serve.ts:616-620) throws RangeError out of the frame's
// async handler; the host's own handler (packages/host/src/cli/serve.ts:124) logs "unhandled rejection, kept serving"
// and the same socket answers its next frame. Run under node at 20,000, 200,000 and 2,000,000 levels: no reply to the
// deep frame, a RangeError, and {"id":6,"ok":true,"workspaces":[]} to the frame after it.
#[test]
fn a_deeply_nested_id_leaves_the_host_serving_the_next_frame() {
    isolated("a_deeply_nested_id_leaves_the_host_serving_the_next_frame", || {
        block(async {
            let rt = FakeRuntime::new();
            rt.script
                .on("workspaces.list", |_| Ok::<Vec<wsp_wire::views::workspace_more::WorkspaceView>, wsp_words::exit::HostError>(vec![]));
            let served = served_with(&rt, opts());
            let (socket, sink) = host_socket(&served).await;
            send_raw(&socket, &format!(r#"{{"id":{},"op":"workspaces.list"}}"#, deep(LEVELS))).await;
            send_raw(&socket, r#"{"id":6,"op":"workspaces.list"}"#).await;
            assert!(sink.texts.borrow().iter().any(|t| t.starts_with(r#"{"id":6,"ok":true"#)), "the frame after it is answered");
        })
    })
}

// NIT (host option only; no host code in this tree passes `ticket_ttl_ms`, only tests do): `now() + ttl as i64` is
// unchecked. A ttl past i64::MAX - now panics in a debug build; a ttl past u64::MAX / 2 casts negative and mints a
// ticket that is already expired, silently, in a release build. Rust: host/crates/wsp-serve/src/serve/places.rs:391.
// TS: packages/runtime/src/serve.ts:1165 (`now() + ticketTtlMs`, a float add: no overflow, a valid ticket).
#[test]
fn a_ticket_ttl_near_the_top_of_the_range_still_mints_a_ticket() {
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, ServeOptions { ticket_ttl_ms: Some(i64::MAX as u64), now: Some(Rc::new(|| 1_000)), ..opts() });
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "ticket.issue", json!({ "purpose": "relay" })).await;
        assert_eq!(reply["ok"], json!(true));
    })
}
