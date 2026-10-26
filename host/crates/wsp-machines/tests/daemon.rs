// SPDX-License-Identifier: AGPL-3.0-only
//! The tests of the daemon area the oracle's scenes cannot make: the roots chain's order under a held write, and the
//! hour a refusal on the record holds across a restart. Spec: spec/daemon.md rules 4, 5, 9 and 10.

mod common;
#[path = "support/daemon.rs"]
mod support;

use serde_json::{json, Value};
use support::{same, spawn, World};

fn entries_of(log: &[Value], kind: &str, ids: &[&str]) -> Vec<Value> {
    log.iter().filter(|e| e[0] == kind && ids.iter().any(|id| e[1] == *id)).cloned().collect()
}

/// The oracle's roots failure `x` * 250 + " roots refused", cut to its last 200 characters.
fn warn_tail() -> String {
    let said = format!("{} roots refused", "x".repeat(250));
    said[said.len() - 200..].to_string()
}

#[test]
fn roots_writes_of_one_machine_land_in_call_order() {
    common::run_local(async {
        // a and b share the machine m1; c is on m2. The module's roots call on a is held open, and every roots call fails.
        let w = World::new(&json!({"entries": [
            {"record": {"id": "a", "kind": "cloud", "machineId": "m1", "phase": "running", "project": "p"}, "mod": {"roots": "fails", "rootsHold": "a"}},
            {"record": {"id": "b", "kind": "cloud", "machineId": "m1", "phase": "running", "project": "q"}},
            {"record": {"id": "c", "kind": "cloud", "machineId": "m2", "phase": "running", "project": "r"}},
        ]}));
        let both = json!(["/work/p", "/work/p/a", "/work/q", "/work/q/b"]);
        let (a, b, c) = (w.entry("a"), w.entry("b"), w.entry("c"));
        let area = w.area.clone();
        let first = spawn({
            let (area, a) = (area.clone(), a.clone());
            async move { area.write_daemon_roots(&a, true).await }
        });
        let second = spawn({
            let (area, b) = (area.clone(), b.clone());
            async move { area.write_daemon_roots(&b, false).await }
        });
        let other = spawn({
            let (area, c) = (area.clone(), c.clone());
            async move { area.write_daemon_roots(&c, false).await }
        });
        common::flush().await;

        let held = w.log.take();
        assert_eq!(entries_of(&held, "roots", &["a", "b"]), vec![json!(["roots", "a", both])], "only the first write on m1 has started; the second waits for it");
        assert!(held.iter().all(|e| e[0] != "warn" || !e[1].as_str().unwrap_or("").contains("for a ") && !e[1].as_str().unwrap_or("").contains("for b ")), "nothing has settled on m1 yet: {held:?}");
        assert_eq!(entries_of(&held, "roots", &["c"]), vec![json!(["roots", "c", ["/work/r", "/work/r/c"]])], "a write on another machine does not wait for m1's chain");
        assert!(!second.is_finished(), "the second write has not settled");

        w.open_roots();
        common::flush().await;
        let after = w.log.take();

        let a_settled = after.iter().position(|e| *e == json!(["roots settled", "a"])).expect("the first write settled");
        let b_started = after.iter().position(|e| *e == json!(["roots", "b", both])).expect("the second write ran after the first failed");
        assert!(a_settled < b_started, "the second write starts only after the first settled: {after:?}");
        assert!(
            !after.iter().any(|e| e[0] == "warn" && e[1].as_str().unwrap_or("").contains("for a ")),
            "a strict write hands its failure back instead of warning: {after:?}"
        );
        let warned: Vec<&Value> = after.iter().filter(|e| e[0] == "warn" && e[1].as_str().unwrap_or("").contains("for b ")).collect();
        assert_eq!(warned, vec![&json!(["warn", format!("browsable folders for b not written on m1: {}", warn_tail())])], "a failed first still lets the second run, and the non-strict second warns");

        let failed = first.await.expect("the first task").expect_err("a strict write hands its own failure back");
        assert_eq!(failed.message, format!("{} roots refused", "x".repeat(250)));
        second.await.expect("the second task").expect("a non-strict write resolves");
        other.await.expect("the third task").expect("a non-strict write resolves");
    })
}

#[test]
fn a_lacks_refusal_on_the_record_holds_the_sync_after_a_restart() {
    common::run_local(async {
        // Written as the runtime wrote it 59 minutes before the scene clock: T0 is 2026-09-21T14:13:20.000Z.
        let marked = json!({"machineId": "m-a", "at": "2026-09-21T13:14:20.000Z", "why": "gcc"});
        let w = World::new(&json!({"entries": [{
            "record": {"id": "a", "kind": "cloud", "machineId": "m-a", "phase": "running", "project": "p", "daemonRefusedAt": marked},
            "mod": {"version": 1}
        }]}));
        let fresh = w.fresh_area();
        fresh.sync_daemon(&w.entry("a")).await.expect("a sync that holds ends well");
        common::flush().await;

        assert_eq!(w.log.take(), vec![json!(["hasDaemon", "a"])], "a refusal under the hour old asks no version and writes no roots");
        let records = w.records();
        assert_eq!(records.len(), 1);
        assert!(same(&records[0], &json!({"id": "a", "kind": "cloud", "machineId": "m-a", "phase": "running", "project": "p", "daemonRefusedAt": marked})), "the mark stays on the record: {}", records[0]);
        assert!(fresh.unread_at().borrow().is_empty());
        assert!(fresh.daemon_syncs().is_empty(), "the join is dropped when the sync ends");

        w.log.clear();
        // A minute on the mark is an hour old, and a third area offers the machine again exactly as the oracle's "the hour out" scene.
        w.clock.advance(60_000);
        let hour = w.fresh_area();
        hour.sync_daemon(&w.entry("a")).await.expect("the sync");
        common::flush().await;
        let fixture = common::fixture("daemon");
        let scene = common::scene(&fixture, "sync: the hour out, it is offered again and a deploy that lands clears the mark");
        let want = scene["log"].as_array().expect("a log");
        let got = w.log.take();
        assert_eq!(got.len(), want.len(), "log length; got {got:#?}");
        for (g, e) in got.iter().zip(want) {
            assert!(same(g, e), "log entry differs\n got {g}\nwant {e}");
        }
        assert_eq!(w.records()[0].get("daemonRefusedAt"), None, "the deploy that landed cleared the mark");
    })
}
