// SPDX-License-Identifier: AGPL-3.0-only
//! The join's own pieces: a `void f()` starts `f` with nothing polling it, the engine's retry clock moves with the
//! runtime's, and `ring` stays the one TLS crypto provider compiled in (wsp-core's re-review).
mod support;

use std::cell::Cell;
use std::rc::Rc;

use wsp_core::clock::FakeClock;
use wsp_engine::errors::RetryClock;
use wsp_serve::runtime::join::{spawn_detached, RuntimeRetryClock};

#[test]
fn a_void_future_runs_with_nobody_awaiting_it() {
    // wsp-account review item 4: the sweep, the pull request rereads and the icon sweep are never awaited.
    support::block(async {
        let ran = Rc::new(Cell::new(false));
        let seen = ran.clone();
        assert!(spawn_detached(async move { seen.set(true) }));
        tokio::task::yield_now().await;
        assert!(ran.get(), "a started future runs to its end, awaited or not");
    })
}

#[test]
fn a_void_future_with_no_local_set_says_so() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async { assert!(!spawn_detached(async {}), "nothing here runs it, and the caller is told") });
}

#[test]
fn the_retry_clock_sleeps_on_the_runtimes_clock() {
    support::block(async {
        let fake = FakeClock::new(1_000);
        let clock = RuntimeRetryClock(fake.clock());
        assert_eq!(clock.now(), 1_000);
        let woke = Rc::new(Cell::new(false));
        let seen = woke.clone();
        let sleeping = Rc::new(clock);
        let waiter = sleeping.clone();
        let handle = tokio::task::spawn_local(async move {
            waiter.sleep(500).await;
            seen.set(true);
        });
        tokio::task::yield_now().await;
        assert!(!woke.get());
        fake.advance(499);
        tokio::task::yield_now().await;
        assert!(!woke.get(), "a moment early is not the time");
        fake.advance(1);
        handle.await.unwrap();
        assert!(woke.get());
        assert_eq!(sleeping.now(), 1_500);
    })
}

#[test]
fn ring_is_the_only_tls_crypto_provider() {
    // wsp-core's re-review (6410742924): a second provider compiled in makes rustls pick none at run time.
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock")).ok();
    let Some(lock) = lock else { return };
    assert!(!lock.contains("name = \"aws-lc-rs\""), "aws-lc-rs is compiled in beside ring");
}

struct SlowAccount {
    finished: Rc<Cell<bool>>,
}

#[async_trait::async_trait(?Send)]
impl wsp_serve::serve::AccountDoor for SlowAccount {
    async fn read(&self) -> Result<wsp_wire::wire::devices::AccountView, wsp_words::exit::HostError> {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        self.finished.set(true);
        Err(wsp_words::exit::HostError::new("read to its end"))
    }
}

#[test]
fn a_request_runs_to_its_end_after_its_socket_is_gone() {
    // wsp-account review item 4: the TS cannot drop a promise, so a create whose socket went away still frees its
    // landing slot and its name; here the op the dropped socket asked for is read to its end.
    support::block(async {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;
        let rt = support::FakeRuntime::new();
        let finished = Rc::new(Cell::new(false));
        let opts = wsp_serve::serve::ServeOptions {
            port: Some(0),
            account: Some(Rc::new(SlowAccount { finished: finished.clone() })),
            ..support::opts()
        };
        let server = wsp_serve::serve::serve_runtime(rt.clone(), opts).await.expect("the runtime serves");
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/ws", server.port())).await.expect("a socket");
        ws.send(Message::Text(serde_json::json!({ "id": 1, "op": "auth", "token": support::TOKEN }).to_string().into())).await.unwrap();
        ws.send(Message::Text(serde_json::json!({ "id": 2, "op": "account.get" }).to_string().into())).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(ws);
        assert!(!finished.get(), "the read was still out when the socket went");
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(finished.get(), "a request whose socket is gone still runs to its end");
    })
}
