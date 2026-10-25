// SPDX-License-Identifier: AGPL-3.0-only
//! Spec boot_ready.md: the restart invariants the start holds, each pinned on the state it protects. Runtime invariant 1
//! (packages/runtime/AGENTS.md): a restarted host re-opens each running turn by its handle, and only the machine's own
//! answer ends it. Chapter 02 rule 2.37: every held start fails once, with the restart named, and leaves no row.
mod common;
#[path = "support/boot.rs"]
mod support;

use serde_json::{json, Value};

use common::{T0, flush, run_local};
use support::{Opts, World};
use wsp_wire::views::session::SessionStatus;

const LINK_RETRY_WINDOW_MS: i64 = 60_000;

fn store_with(rows: Value, extra: Value) -> Value {
    let mut store = json!({
        "owner": {"id": {"id": "h_kept"}},
        "reads": {"since": {"at": 5}},
        "workspaces": {"a": {"id": "a", "name": "w-a", "kind": "cloud", "machineId": "m-a", "phase": "running", "golden": "img", "project": "p1"}},
        "sessions": {"a": {"workspaceId": "a", "threads": {}, "sessions": rows}},
    });
    for (k, v) in extra.as_object().into_iter().flatten() {
        store[k] = v.clone();
    }
    store
}

fn running_row(id: &str) -> Value {
    json!({"id": id, "workspaceId": "a", "harness": "claude", "status": "running", "threadId": format!("t-{id}"), "startedAt": T0 - 100_000, "turnId": format!("turn-{id}")})
}

fn entries(log: &[Value], name: &str) -> Vec<Value> {
    log.iter().filter(|e| e[0] == name).cloned().collect()
}

#[test]
fn a_machine_that_answers_nothing_leaves_the_turn_running_and_is_asked_again_a_window_later() {
    run_local(async {
        let w = World::ready(Opts { store: store_with(json!([running_row("r1")]), json!({})), reattach: vec![("turn-r1", "unreached")], ..Opts::default() });
        w.area.ready().await.expect("the start");
        flush().await;
        let log = w.log.take();
        assert_eq!(entries(&log, "reattach"), vec![json!(["reattach", "turn-r1", false])]);
        assert!(entries(&log, "settleCut").is_empty(), "nothing ends a turn its machine said nothing about");
        let row = w.rt.sessions.borrow().get("r1").cloned().expect("the row stands");
        assert_eq!(row.borrow().view.status, SessionStatus::Running, "the row still reads running");
        assert!(row.borrow().unanswered, "the row is marked unanswered between rounds");
        // A round that answered nothing writes nothing.
        assert!(entries(&log, "persistSessions").is_empty(), "a round that answered nothing writes nothing");

        w.log.clear();
        w.clock.advance(LINK_RETRY_WINDOW_MS - 1);
        flush().await;
        assert!(entries(&w.log.take(), "reattach").is_empty(), "not asked again inside the link window");
        w.clock.advance(1);
        flush().await;
        assert_eq!(entries(&w.log.take(), "reattach"), vec![json!(["reattach", "turn-r1", true])], "asked again once the window is out");
        assert_eq!(row.borrow().view.status, SessionStatus::Running);
    });
}

#[test]
fn a_run_the_machine_no_longer_holds_ends_the_turn_on_the_gone_line() {
    run_local(async {
        let w = World::ready(Opts { store: store_with(json!([running_row("r1"), running_row("r2")]), json!({})), reattach: vec![("turn-r1", "gone"), ("turn-r2", "cannot")], ..Opts::default() });
        w.area.ready().await.expect("the start");
        flush().await;
        let log = w.log.take();
        let gone = wsp_words::words::thread::RUN_GONE_LINE;
        assert_eq!(
            entries(&log, "settleCut"),
            vec![
                json!(["settleCut", "turn-r1", gone, gone]),
                json!(["settleCut", "turn-r2", "host restarted while the agent was working", "cut by a host restart after 2m 45s"]),
            ],
            "each answer settles its own row once, with its own words"
        );
        for id in ["r1", "r2"] {
            let row = w.rt.sessions.borrow().get(id).cloned().expect("the row stands");
            assert_eq!(row.borrow().view.status, SessionStatus::Failed);
        }
        assert!(log.contains(&json!(["persistSessions", "a"])), "the settled rows are written");
    });
}

#[test]
fn every_held_start_fails_once_with_the_restart_named_and_no_row() {
    run_local(async {
        let held = json!({
            "turn-h1": {"workspaceId": "a", "threadId": "t-h1", "turnId": "turn-h1", "sessionId": "s-h1", "harness": "claude", "place": "hetzner", "prompt": "do it", "notify": ["lead"]},
            "turn-h2": {"workspaceId": "a", "threadId": "t-h2", "turnId": "turn-h2", "sessionId": "turn-h2", "harness": "codex", "place": "box"},
            "turn-h3": {"workspaceId": "gone", "threadId": "t-h3", "turnId": "turn-h3", "sessionId": "s-h3", "harness": "claude", "place": "box"},
        });
        let w = World::ready(Opts { store: store_with(json!([]), json!({"held-starts": held})), ..Opts::default() });
        w.area.ready().await.expect("the start");
        flush().await;
        let log = w.log.take();
        assert_eq!(w.rt.mem.table("held-starts"), json!([]), "every held start is deleted, the one on no live workspace too");
        let records = entries(&log, "record");
        assert_eq!(records.len(), 2, "one end recorded per held start on a live workspace");
        let first = &records[0][1];
        let keys: Vec<&str> = first.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["type", "workspaceId", "sessionId", "turnId", "threadId", "exitCode", "sawResult", "reason", "unstarted", "prompt"], "rule 2.37's key order");
        assert_eq!(first["reason"], "the host restarted while this turn waited for a slot on hetzner; nothing started, so send it again");
        assert_eq!(first["exitCode"], Value::Null);
        assert_eq!(first["unstarted"], true);
        assert_eq!(records[1][1]["sessionId"], "turn-h2");
        assert!(records[1][1].get("prompt").is_none());
        assert_eq!(entries(&log, "notifyEnd").len(), 1, "only the start with targets tells them");
        assert!(w.rt.sessions.borrow().is_empty(), "no turn row is made for a start that never ran");
    });
}

#[test]
fn a_shell_held_token_outlives_the_restart_revocation() {
    run_local(async {
        let devices = json!([
            {"id": "d-shell", "scope": {"threadId": "t-idle"}},
            {"id": "d-stale", "scope": {"threadId": "t-idle2"}},
            {"id": "d-run", "scope": {"threadId": "t-run"}},
            {"id": "d-person"},
        ]);
        let w = World::ready(Opts { store: store_with(json!([]), json!({})), devices, shells: vec!["d-shell"], running: vec!["t-run"], ..Opts::default() });
        w.area.ready().await.expect("the start");
        flush().await;
        let log = w.log.take();
        assert_eq!(entries(&log, "deviceDoor.revoke"), vec![json!(["deviceDoor.revoke", "d-stale"])], "only the scoped token of a thread not running and held by no shell goes");
        let revoke_at = log.iter().position(|e| e[0] == "deviceDoor.revoke").unwrap();
        let asides_at = log.iter().position(|e| e[0] == "deviceDoor.revokeAsides").expect("the asides go too");
        assert!(revoke_at < asides_at, "the scoped tokens go before the asides");
    });
}
