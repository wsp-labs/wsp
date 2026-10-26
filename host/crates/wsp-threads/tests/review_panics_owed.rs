// SPDX-License-Identifier: AGPL-3.0-only
//! Review: panics a stored owed line can reach (threads.ts:45-62 readOwed, :677 deliver). Each test is red now.
//! A panic inside a spawned task is caught by tokio, so a hook notes every panic on the test's own thread.
#[path = "support_b/mod.rs"]
mod support;

use serde_json::json;
use support::threads_ctx::{entry, FakeThreadsCtx};
use support::Lend;
use support::{local, row, view};

mod seen {
    use std::sync::{Mutex, Once};
    use std::thread::ThreadId;
    static LOG: Mutex<Vec<(ThreadId, String)>> = Mutex::new(Vec::new());
    static ONCE: Once = Once::new();
    pub fn watch() {
        ONCE.call_once(|| {
            let prior = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
                let p = info.payload();
                let msg =
                    p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
                LOG.lock().unwrap().push((std::thread::current().id(), format!("{msg} at {at}")));
                prior(info);
            }));
        });
    }
    pub fn none() {
        let me = std::thread::current().id();
        let mine: Vec<String> = LOG.lock().unwrap().iter().filter(|(t, _)| *t == me).map(|(_, m)| m.clone()).collect();
        assert!(mine.is_empty(), "panicked: {mine:?}");
    }
}

fn host() -> (std::rc::Rc<FakeThreadsCtx>, std::rc::Rc<wsp_threads::threads::threads::ThreadsArea>) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "phase": "running" }));
    ctx.add_entry(e);
    ctx.sessions.with(|s| {
        s.insert(
            "r-p".into(),
            row(
                view(json!({ "id": "r-p", "threadId": "parent0001", "claudeSessionId": "cs-p", "status": "completed", "startedAt": 1 })),
                "u-p",
            ),
        );
    });
    let area = ctx.area();
    (ctx, area)
}

/// A stored line whose `since` is the largest integer the file can hold. The TS adds an hour to it as a float.
#[test]
fn deliver_owed_survives_a_stored_since_at_the_top_of_i64() {
    seen::watch();
    local(async {
        let (ctx, area) = host();
        ctx.store.seed(
            "notify-owed",
            "L1",
            json!({ "id": "L1", "from": "child0001", "notify": "parent0001", "text": "t", "since": 9223372036854775807_i64 }),
        );
        let _ = area.deliver_owed().await;
        support::settle().await;
    });
    seen::none();
}

/// A stored line whose `since` is the smallest integer: once a start fails on a silent computer, the deadline minus
/// the clock underflows (a real clock reads 1.7e12, the test clock is moved past an hour).
#[test]
fn a_try_survives_a_stored_since_at_the_bottom_of_i64() {
    seen::watch();
    local(async {
        let (ctx, area) = host();
        ctx.clock.advance(5_000_000);
        ctx.store.seed(
            "notify-owed",
            "L1",
            json!({ "id": "L1", "from": "child0001", "notify": "parent0001", "text": "t", "since": -9223372036854775808_i64,
                    "requestId": "req-1" }),
        );
        ctx.starts.borrow_mut().push_back(Err(support::silent("down", "MachineUnreached")));
        let _ = area.deliver_owed().await;
        support::settle().await;
    });
    seen::none();
}

/// A stored line whose `tries` is the top of u32: the next failed try adds one to it.
#[test]
fn a_try_survives_a_stored_tries_at_the_top_of_u32() {
    seen::watch();
    local(async {
        let (ctx, area) = host();
        ctx.store.seed(
            "notify-owed",
            "L1",
            json!({ "id": "L1", "from": "child0001", "notify": "parent0001", "text": "t", "tries": 4294967295_u64,
                    "since": 1_000_000, "next": 1_000_000, "requestId": "req-1" }),
        );
        ctx.starts.borrow_mut().push_back(Err(support::silent("down", "MachineUnreached")));
        let _ = area.deliver_owed().await;
        support::settle().await;
    });
    seen::none();
}

/// `deliver` (owed.rs:272/311/749) starts its detached task with `tokio::task::spawn_local`.
#[test]
fn deliver_runs_on_a_runtime_with_no_local_set() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    rt.block_on(async {
        let (_ctx, area) = host();
        let line =
            wsp_threads::threads::threads::read_owed(&json!({ "id": "L1", "from": "child0001", "notify": "parent0001", "text": "t" }))
                .expect("a line");
        area.deliver(line, false);
    });
}
