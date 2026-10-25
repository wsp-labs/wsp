// SPDX-License-Identifier: AGPL-3.0-only
//! threads_cap (spec/threads_cap.md): a computer's threads at once. Expected values read off threads.ts:364-521
//! at 65f6b89c7 and thread-cap.test.ts.
#[path = "support_b/mod.rs"]
mod support;

use std::rc::Rc;

use futures::FutureExt;
use serde_json::{json, Value};
use support::Lend;
use support::threads_ctx::{entry, FakeThreadsCtx};
use support::{local, row, rows_of, settle, view};
use wsp_threads::threads::threads::cap::CapAt;
use wsp_threads::threads::threads::ThreadsArea;
use wsp_wire::views::session::ThreadCapWait;
use wsp_wire::views::session_events_more::SessionUncappedEventWhy;

/// A host with one local workspace on this computer that runs `at_once` threads at once, and `running` threads
/// already running on it, each its own thread `t<i>` with turn `u<i>`.
fn host(at_once: i64, running: usize) -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>, wsp_core::types::wiring::WorkspaceRecord) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "kind": "local" }));
    let record = e.borrow().record.borrow().clone();
    ctx.add_entry(e);
    ctx.place_ids.borrow_mut().insert("ws1".into(), "here".into());
    ctx.threads_at.borrow_mut().insert("here".into(), at_once);
    ctx.sessions.with(|s| {
        *s = rows_of((0..running).map(|i| row(view(json!({ "id": format!("u{i}"), "threadId": format!("t{i}"), "status": "running" })), &format!("u{i}"))).collect())
    });
    let area = ctx.area();
    (ctx, area, record)
}

/// A start's row for `turn` on thread `thread`, keyed by the turn id as a start keys its held row.
fn start_row(ctx: &FakeThreadsCtx, turn: &str, thread: &str) {
    ctx.sessions.with(|s| {
        s.insert(turn.to_string(), row(view(json!({ "id": turn, "threadId": thread, "status": "running", "harness": "codex" })), turn));
    });
}

fn wait(running: i64, at_once: i64) -> ThreadCapWait {
    serde_json::from_value(json!({ "placeId": "here", "place": "here-name", "running": running, "atOnce": at_once })).expect("a wait")
}

fn at(turn: &str, thread: &str) -> CapAt {
    CapAt { workspace_id: "ws1".into(), thread_id: thread.into(), turn_id: turn.into(), session_id: format!("s-{turn}"), request_id: Some("req1".into()), prompt: "build the thing\nwhole".into() }
}

fn end_of(turn: &str) -> Value {
    json!({ "type": "session.end", "workspaceId": "ws1", "sessionId": "x", "turnId": turn, "exitCode": null, "sawResult": false })
}

#[test]
fn cap_hold_holds_once_and_keeps_its_first_lenders() {
    let (_ctx, area, record) = host(2, 0);
    area.cap_hold(&record, "n1", Some("u0"));
    area.cap_hold(&record, "n1", Some("u9"));
    area.cap_hold(&record, "n2", None);
    let held = area.cap_held.borrow();
    assert_eq!(held.get("n1").map(|h| (h.place_id.clone(), h.lenders.clone(), h.order)), Some((Some("here".to_string()), vec!["u0".to_string()], 0)));
    assert_eq!(held.get("n2").map(|h| (h.lenders.clone(), h.order)), Some((vec![], 1)));
}

#[test]
fn cap_lend_adds_a_lender_once_and_wakes_the_wait() {
    local(async {
        let (ctx, area, record) = host(1, 1);
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        let waiting = area.cap_wait(at("n1", "tn"), wait(1, 1)).shared();
        let watch = waiting.clone();
        tokio::task::spawn_local(async move {
            let _ = watch.await;
        });
        settle().await;
        assert!(waiting.peek().is_none(), "the wait holds");
        area.cap_lend("n1", "u0");
        area.cap_lend("n1", "u0");
        settle().await;
        assert_eq!(area.cap_held.borrow().get("n1").map(|h| h.lenders.clone()), Some(vec!["u0".to_string()]));
        assert!(matches!(waiting.peek(), Some(Ok(()))), "a new lender wakes the wait");
        area.cap_lend("nobody-held", "u0");
    });
}

#[test]
fn cap_full_is_none_on_a_cloud_or_with_no_hold() {
    let (ctx, area, record) = host(1, 3);
    assert_eq!(area.cap_full(&record, "n1"), None, "a turn not held");
    area.cap_hold(&record, "n1", None);
    ctx.threads_at.borrow_mut().shift_remove("here");
    assert_eq!(area.cap_full(&record, "n1"), None, "a place that answers no threads at once (a cloud)");
    ctx.place_ids.borrow_mut().shift_remove("ws1");
    ctx.threads_at.borrow_mut().insert("here".into(), 1);
    assert_eq!(area.cap_full(&record, "n1"), None, "a record with no place id");
}

#[test]
fn cap_full_counts_running_threads_and_the_turns_ahead() {
    let (ctx, area, record) = host(2, 2);
    start_row(&ctx, "n1", "tn");
    area.cap_hold(&record, "n1", None);
    assert_eq!(area.cap_full(&record, "n1"), Some(wait(2, 2)), "two running of two");
    ctx.threads_at.borrow_mut().insert("here".into(), 3);
    assert_eq!(area.cap_full(&record, "n1"), None, "room for a third");
    // A turn held before this one on the same place stands ahead of it.
    start_row(&ctx, "n0", "t-ahead");
    let (ctx2, area2, record2) = host(3, 2);
    start_row(&ctx2, "n0", "t-ahead");
    start_row(&ctx2, "n1", "tn");
    area2.cap_hold(&record2, "n0", None);
    area2.cap_hold(&record2, "n1", None);
    assert_eq!(area2.cap_full(&record2, "n1"), Some(wait(2, 3)), "two running plus one ahead fills three; the held turn is not counted running");
    assert_eq!(area2.cap_full(&record2, "n0"), None, "the first in line has room");
    drop((ctx, area, record));
}

#[test]
fn cap_full_lets_a_turn_through_in_its_lenders_slot() {
    let (ctx, area, record) = host(2, 2);
    start_row(&ctx, "n1", "tn");
    area.cap_hold(&record, "n1", Some("u0"));
    assert_eq!(area.cap_full(&record, "n1"), None, "u0 runs here and lends its slot");
    assert_eq!(area.borrowed.borrow().get("n1").map(String::as_str), Some("u0"));
}

#[test]
fn cap_full_lends_one_slot_to_one_turn_at_a_time() {
    let (ctx, area, record) = host(2, 2);
    start_row(&ctx, "n1", "tn");
    area.cap_hold(&record, "n1", Some("u0"));
    assert_eq!(area.cap_full(&record, "n1"), None);
    area.cap_left("n1");
    start_row(&ctx, "n2", "tm");
    area.cap_hold(&record, "n2", Some("u0"));
    assert_eq!(area.cap_full(&record, "n2"), Some(wait(3, 2)), "u0 already lends to n1, which now runs: three running, one lent, so full");
    assert_eq!(area.borrowed.borrow().get("n2"), None);
}

#[test]
fn cap_full_says_raised_or_freed_when_it_lets_a_waiting_turn_through() {
    local(async {
        let (ctx, area, record) = host(1, 1);
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        let _w = area.cap_wait(at("n1", "tn"), wait(1, 1));
        ctx.threads_at.borrow_mut().insert("here".into(), 2);
        assert_eq!(area.cap_full(&record, "n1"), None);
        assert_eq!(area.cap_held.borrow().get("n1").and_then(|h| h.through), Some((SessionUncappedEventWhy::Raised, 2)));
        ctx.threads_at.borrow_mut().insert("here".into(), 1);
        ctx.sessions.with(|s| s.shift_remove("u0"));
        assert_eq!(area.cap_full(&record, "n1"), None);
        assert_eq!(area.cap_held.borrow().get("n1").and_then(|h| h.through), Some((SessionUncappedEventWhy::Freed, 1)));
    });
}

#[test]
fn cap_wait_writes_the_held_start_once_and_says_the_wait() {
    local(async {
        let (ctx, area, record) = host(1, 1);
        start_row(&ctx, "n1", "tn");
        ctx.sessions.with(|s| s.get("n1").expect("row").borrow_mut().notify = Some(vec!["lead".into()]));
        area.cap_hold(&record, "n1", None);
        let _w = area.cap_wait(at("n1", "tn"), wait(1, 1));
        settle().await;
        let held = ctx.store.get_now("held-starts", "n1").expect("the held start is written down");
        assert_eq!(
            held,
            json!({ "workspaceId": "ws1", "threadId": "tn", "turnId": "n1", "sessionId": "s-n1", "harness": "codex", "place": "here-name", "prompt": "build the thing\nwhole", "notify": ["lead"] })
        );
        let keys: Vec<&String> = held.as_object().expect("an object").keys().collect();
        assert_eq!(keys, vec!["workspaceId", "threadId", "turnId", "sessionId", "harness", "place", "prompt", "notify"]);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!(said[0]["type"], "session.capped");
        for (k, v) in [("workspaceId", json!("ws1")), ("sessionId", json!("s-n1")), ("turnId", json!("n1")), ("threadId", json!("tn")), ("placeId", json!("here")), ("place", json!("here-name")), ("running", json!(1)), ("atOnce", json!(1)), ("requestId", json!("req1")), ("prompt", json!("build the thing\nwhole"))] {
            assert_eq!(said[0][k], v, "{k}");
        }
        assert!(ctx.called().contains(&"idle.hold ws1".to_string()));
    });
}

#[test]
fn cap_wait_says_the_wait_again_only_when_it_moved() {
    local(async {
        let (ctx, area, record) = host(1, 1);
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        let first = area.cap_wait(at("n1", "tn"), wait(1, 1)).shared();
        let f = first.clone();
        tokio::task::spawn_local(async move {
            let _ = f.await;
        });
        ctx.bus.emit_json(end_of("other"));
        settle().await;
        assert!(matches!(first.peek(), Some(Ok(()))), "an end anywhere wakes the wait");
        let _second = area.cap_wait(at("n1", "tn"), wait(1, 1));
        let _third = area.cap_wait(at("n1", "tn"), wait(2, 2));
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 2, "the same wait is not said twice; a moved count is");
        assert_eq!(said[1]["running"], 2);
        assert_eq!(said[1].get("prompt"), None, "the prompt rides the first line alone");
        assert_eq!(ctx.store.logged().iter().filter(|l| l.starts_with("put held-starts")).count(), 1, "the held start is written once");
    });
}

#[test]
fn cap_wait_holds_the_machine_awake_until_it_settles() {
    local(async {
        let (ctx, area, record) = host(1, 1);
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        let waiting = area.cap_wait(at("n1", "tn"), wait(1, 1)).shared();
        let w = waiting.clone();
        tokio::task::spawn_local(async move {
            let _ = w.await;
        });
        settle().await;
        assert_eq!(ctx.called().iter().filter(|c| c.starts_with("idle.")).cloned().collect::<Vec<_>>(), vec!["idle.hold ws1"]);
        assert_eq!(ctx.bus.listening("*"), 2, "the area's own end listener and this wait's");
        ctx.bus.emit_json(json!({ "type": "workspace.deleted", "workspaceId": "ws9" }));
        settle().await;
        ctx.bus.emit_json(end_of("other"));
        settle().await;
        assert_eq!(ctx.called().iter().filter(|c| c.starts_with("idle.")).cloned().collect::<Vec<_>>(), vec!["idle.hold ws1", "idle.release ws1"], "released once");
        assert_eq!(ctx.bus.listening("*"), 1, "the wait's listener is gone");
    });
}

#[test]
fn cap_wait_wakes_on_an_end_a_place_change_or_a_delete() {
    for wake in [end_of("other"), json!({ "type": "workspace.deleted", "workspaceId": "ws9" })] {
        local(async move {
            let (ctx, area, record) = host(1, 1);
            start_row(&ctx, "n1", "tn");
            area.cap_hold(&record, "n1", None);
            let waiting = area.cap_wait(at("n1", "tn"), wait(1, 1)).shared();
            let w = waiting.clone();
            tokio::task::spawn_local(async move {
                let _ = w.await;
            });
            ctx.bus.emit_json(json!({ "type": "workspace.renamed", "workspaceId": "ws1", "name": "x" }));
            settle().await;
            assert!(waiting.peek().is_none(), "another event does not wake it");
            ctx.bus.emit_json(wake.clone());
            settle().await;
            assert!(matches!(waiting.peek(), Some(Ok(()))), "{wake}");
        });
    }
}

#[test]
fn cap_stop_ends_a_wait_with_the_reason() {
    local(async {
        let (ctx, area, record) = host(1, 1);
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        let waiting = area.cap_wait(at("n1", "tn"), wait(1, 1)).shared();
        let w = waiting.clone();
        tokio::task::spawn_local(async move {
            let _ = w.await;
        });
        settle().await;
        assert!(area.cap_stop("n1", "the person stopped it"));
        assert!(!area.cap_stop("not-held", "x"));
        settle().await;
        match waiting.peek() {
            Some(Err(e)) => assert_eq!(e.message, "the person stopped it"),
            _ => panic!("the wait failed with the stop's reason"),
        }
        assert!(ctx.called().contains(&"idle.release ws1".to_string()));
    });
}

#[test]
fn a_stop_before_the_wait_fails_it_at_once_and_writes_nothing() {
    local(async {
        let (ctx, area, record) = host(1, 1);
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        assert!(area.cap_stop("n1", "stopped before it waited"));
        let failed = area.cap_wait(at("n1", "tn"), wait(1, 1)).await;
        assert_eq!(failed.expect_err("fails at once").message, "stopped before it waited");
        settle().await;
        assert!(ctx.recorded.borrow().is_empty(), "no wait is said");
        assert!(ctx.store.get_now("held-starts", "n1").is_none(), "no held start is written");
        assert!(!ctx.called().contains(&"idle.hold ws1".to_string()));
    });
}

#[test]
fn cap_stopping_holds_a_promise_until_released() {
    let (ctx, area, record) = host(1, 1);
    start_row(&ctx, "n1", "tn");
    area.cap_hold(&record, "n1", None);
    let release = area.cap_stopping("n1");
    let stops = area.cap_held.borrow().get("n1").map(|h| h.stops.clone()).expect("held");
    assert_eq!(stops.len(), 1);
    assert!(stops[0].peek().is_none(), "unsettled while the stop is read");
    release();
    assert!(area.cap_held.borrow().get("n1").map(|h| h.stops.is_empty()).expect("held"));
    assert!(stops[0].clone().now_or_never().is_some(), "settled by its release");
    area.cap_stopping("not-held")();
}

#[test]
fn cap_left_deletes_the_held_start_says_why_and_wakes_the_rest() {
    local(async {
        let (ctx, area, record) = host(1, 1);
        start_row(&ctx, "n1", "tn");
        start_row(&ctx, "n2", "tm");
        area.cap_hold(&record, "n1", None);
        area.cap_hold(&record, "n2", None);
        let _first = area.cap_wait(at("n1", "tn"), wait(1, 1));
        let second = area.cap_wait(at("n2", "tm"), wait(1, 1)).shared();
        let s = second.clone();
        tokio::task::spawn_local(async move {
            let _ = s.await;
        });
        ctx.threads_at.borrow_mut().insert("here".into(), 2);
        assert_eq!(area.cap_full(&record, "n1"), None);
        area.cap_left("n1");
        settle().await;
        assert!(ctx.store.get_now("held-starts", "n1").is_none(), "its held start is deleted");
        assert!(ctx.store.get_now("held-starts", "n2").is_some());
        let said = ctx.recorded_json();
        let uncapped = said.iter().find(|e| e["type"] == "session.uncapped").expect("why it went through");
        for (k, v) in [("workspaceId", json!("ws1")), ("threadId", json!("tn")), ("sessionId", json!("s-n1")), ("turnId", json!("n1")), ("why", json!("raised")), ("atOnce", json!(2))] {
            assert_eq!(uncapped[k], v, "{k}");
        }
        assert!(matches!(second.peek(), Some(Ok(()))), "the turn behind it looks again");
        assert!(area.cap_held.borrow().get("n1").is_none());
    });
}

#[test]
fn cap_left_of_a_turn_that_never_waited_writes_nothing() {
    local(async {
        let (ctx, area, record) = host(2, 0);
        start_row(&ctx, "n1", "tn");
        area.cap_hold(&record, "n1", None);
        area.cap_left("n1");
        area.cap_left("n1");
        settle().await;
        assert!(ctx.store.logged().is_empty(), "nothing was written down, so nothing is deleted");
        assert!(ctx.recorded.borrow().is_empty());
    });
}

#[test]
fn a_failed_held_start_write_warns_and_the_chain_goes_on() {
    local(async {
        let (ctx, area, record) = host(1, 1);
        start_row(&ctx, "n1", "tn");
        start_row(&ctx, "n2", "tm");
        area.cap_hold(&record, "n1", None);
        area.cap_hold(&record, "n2", None);
        let warned = wsp_core::console::capture();
        ctx.store.fail_put.set(true);
        let _a = area.cap_wait(at("n1", "tn"), wait(1, 1));
        settle().await;
        ctx.store.fail_put.set(false);
        let _b = area.cap_wait(at("n2", "tm"), wait(1, 1));
        settle().await;
        assert_eq!(warned.lines(), vec!["a held start was not written down: the disk is full"]);
        assert!(ctx.store.get_now("held-starts", "n2").is_some(), "the next write still lands");
        assert_eq!(ctx.store.logged(), vec!["put held-starts/n1", "put held-starts/n2"], "in call order");
    });
}

#[test]
fn an_end_frees_its_borrowed_slots_both_ways() {
    let (ctx, area, _record) = host(2, 2);
    area.borrowed.borrow_mut().insert("n1".into(), "u0".into());
    area.borrowed.borrow_mut().insert("n2".into(), "u0".into());
    area.borrowed.borrow_mut().insert("n3".into(), "u1".into());
    ctx.bus.emit_json(end_of("u0"));
    assert_eq!(area.borrowed.borrow().keys().cloned().collect::<Vec<_>>(), vec!["n3"], "every turn u0 lent to holds its own slot now");
    ctx.bus.emit_json(end_of("n3"));
    assert!(area.borrowed.borrow().is_empty(), "the borrower's own end frees its entry");
    ctx.bus.emit_json(json!({ "type": "session.end", "workspaceId": "ws1", "sessionId": "x", "exitCode": null, "sawResult": false }));
}
