// SPDX-License-Identifier: AGPL-3.0-only
//! Spec boot_rows.md: the tests the rules table names beside the scenes, and the listener and subagent rules the
//! scenes do not reach. Each over the fake runtime of
//! `support/boot.rs`; expected values come from `boot.ts`, the TS tests the table cites and the scene fixtures.
mod common;
#[path = "support/boot.rs"]
mod support;

use serde_json::{json, Value};

use common::T0;
use support::{assert_event_key_order, norm, thread_caller, to_json, Opts, World};

async fn world(rows: Value, records: Value) -> World {
    let w = World::rows(Opts { live: json!({"a": {}, "b": {}}), rows, records, ..Opts::default() });
    w.started().await;
    w
}

fn view(id: &str, ws: &str, thread: &str, extra: Value) -> Value {
    let mut v = json!({"id": id, "workspaceId": ws, "threadId": thread});
    for (k, x) in extra.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    json!({"view": v})
}

/// A row as the fixture's `row push` and `mark` scenes pass it: the listing's answer joined with its project and computer.
fn pushed(id: &str, ws: &str, thread: &str, extra: Value) -> Value {
    let mut row = json!({"harness": "claude", "status": "completed", "id": id, "workspaceId": ws, "threadId": thread, "readAt": T0, "project": {"id": "p1", "name": "site"}, "computerName": "hetzner"});
    for (k, x) in extra.as_object().into_iter().flatten() {
        row[k] = x.clone();
    }
    json!(["pass", {"type": "session.row", "workspaceId": ws, "threadId": thread, "id": id, "row": row}])
}

fn gone(id: &str, ws: &str, thread: &str) -> Value {
    json!(["pass", {"type": "session.row", "workspaceId": ws, "threadId": thread, "id": id}])
}

fn assert_log(name: &str, want: Vec<Value>, got: Vec<Value>) {
    let scene = json!({"name": name, "log": want});
    common::assert_log(&scene, &got);
    assert_event_key_order(&scene, &got);
}

fn sub_row() -> wsp_wire::views::sub_row::SubRow {
    serde_json::from_value(json!({"state": "running", "on": "hetzner", "machine": "m-a", "size": {"cpu": 1, "memMb": 1024}, "branch": "sub/one"})).expect("a sub row")
}

/// boot.ts:760: a sub-thread's facts are read again once the sub's own read ran; `since` is now for a running thread
/// and the turn's end for one that ended; a caller the row is out of reach for, and a thread with no sub, get the facts
/// as they stand.
#[test]
fn facts_after_read_waits_for_the_sub_threads_read() {
    common::run_local(async {
        let w = world(json!([view("r1", "a", "t1", json!({"status": "completed", "startedAt": 1, "endedAt": T0 - 5000})), view("r2", "b", "t2", json!({"status": "running", "startedAt": 2})), view("r3", "c", "t3", json!({}))]), json!({})).await;
        w.rt.sub_rows.borrow_mut().insert("a".into(), sub_row());
        w.rt.sub_rows.borrow_mut().insert("b".into(), sub_row());
        let row = w.rt.sessions.borrow().get("r1").cloned().unwrap();
        let row2 = w.rt.sessions.borrow().get("r2").cloned().unwrap();
        *w.rt.on_sub_read.borrow_mut() = Some(Box::new(move || {
            row.borrow_mut().view.model = Some("m-after".into());
            row2.borrow_mut().view.model = Some("m-after".into());
        }));

        let before = w.area.thread_facts("t1").expect("facts");
        assert_eq!(before.model, None, "the facts as they stand carry no model yet");
        let ended = w.area.facts_after_read("t1", None).await.expect("facts").expect("a thread");
        assert_eq!(ended.model.as_deref(), Some("m-after"), "the facts were read again after the sub's read");
        assert!(ended.sub.is_some());
        let running = w.area.facts_after_read("t2", None).await.expect("facts").expect("a thread");
        assert_eq!(running.model.as_deref(), Some("m-after"));
        assert_eq!(
            w.log.take(),
            vec![json!(["refuseRelayed", "a", null, null, null]), json!(["subRead", "a", T0 - 5000]), json!(["refuseRelayed", "b", null, null, null]), json!(["subRead", "b", T0])],
            "entryOf asks refuseRelayed before each read (boot.ts:636, 666); since is the end of an ended turn and now for a running one"
        );

        w.log.clear();
        let none = w.area.facts_after_read("t3", None).await.expect("facts").expect("a thread");
        assert!(none.sub.is_none());
        let out_of_reach = w.area.facts_after_read("t1", Some(&thread_caller())).await.expect("facts").expect("a thread");
        let said = w.log.take();
        let asked: Vec<&str> = said.iter().map(|e| e[0].as_str().unwrap_or("")).collect();
        assert_eq!(asked, ["drivesThread"], "a thread with no sub waits on nothing and a caller out of reach is asked once and read no sub (boot.ts:657): {said:?}");
        assert_eq!(out_of_reach.model.as_deref(), Some("m-after"), "the facts as they stand");
    });
}

/// boot.ts:778: the head's facts and the transcript index's position go out as `thread.head`, once, and a thread no
/// row holds says nothing.
#[test]
fn push_head_carries_the_facts_and_the_index_pos() {
    common::run_local(async {
        let w = world(json!([view("r1", "a", "t1", json!({"status": "completed", "startedAt": 1}))]), json!({})).await;
        w.rt.index_pos.borrow_mut().insert("a".into(), 7);
        w.area.push_head("t1").expect("push");
        w.area.push_head("nobody").expect("push");
        let facts = json!({"id": "t1", "threadId": "t1", "workspaceId": "a", "harness": "claude", "startedBy": "person", "status": "completed", "title": "r1", "sessionId": "r1", "startedAt": 1, "readAt": T0, "turns": 1, "ran": false});
        let got = w.log.take();
        let scene = json!({"name": "push head", "log": [["emit", {"type": "thread.head", "workspaceId": "a", "threadId": "t1", "facts": facts, "pos": 7}]]});
        common::assert_log(&scene, &got[..1]);
        assert_event_key_order(&scene, &got[..1]);
        assert!(got[1..].iter().all(|e| e[0] == "pass"), "the only other entries are the pushes the head's own event moved: {got:?}");
    });
}

/// boot.ts:834 and session-row.test.ts:204: a batch the bus fails on warns and goes out with the next one. The TS test
/// fails the places read the push joins, which the interface answers without a failure; the bus is the one call left
/// that can fail.
#[test]
fn a_failed_batch_goes_out_with_the_next() {
    common::run_local(async {
        let w = world(json!([view("r1", "a", "t1", json!({})), view("r2", "a", "t2", json!({}))]), json!({})).await;
        w.rt.fail_pass.set(1);
        w.rt.inject(json!({"type": "session.held", "workspaceId": "a", "threadId": "t1"}));
        w.area.rows_sent().await;
        common::flush().await;
        w.rt.inject(json!({"type": "session.held", "workspaceId": "a", "threadId": "t2"}));
        w.area.rows_sent().await;
        common::flush().await;
        assert_log(
            "a failed batch goes out with the next",
            vec![
                json!(["emit", {"type": "session.held", "workspaceId": "a", "threadId": "t1"}]),
                pushed("r1", "a", "t1", json!({})),
                json!(["warn", "a moved row was not pushed: bus down"]),
                json!(["emit", {"type": "session.held", "workspaceId": "a", "threadId": "t2"}]),
                pushed("r1", "a", "t1", json!({})),
                pushed("r2", "a", "t2", json!({})),
            ],
            w.log.take(),
        );
    });
}

/// session-row.test.ts:112 and :145: a turn's start drops the row held under the turn's id and the thread's one row
/// goes out with its opening prompt; its end pushes that row again without the prompt, and drops nothing while a held
/// row still carries the turn.
#[test]
fn a_turn_start_and_end_each_push_that_threads_one_row() {
    common::run_local(async {
        let w = world(json!([{"view": {"id": "s1", "workspaceId": "a", "threadId": "t1", "prompt": "the brief"}, "turnId": "turn-1"}]), json!({})).await;
        w.rt.inject(json!({"type": "thread.marked", "workspaceId": "a", "threadIds": ["t1"]}));
        w.rt.inject(json!({"type": "session.start", "workspaceId": "a", "sessionId": "s1", "turnId": "turn-1", "threadId": "t1"}));
        w.area.rows_sent().await;
        let start = w.log.take();
        assert_log(
            "a turn start",
            vec![
                json!(["emit", {"type": "thread.marked", "workspaceId": "a", "threadIds": ["t1"]}]),
                json!(["emit", {"type": "session.start", "workspaceId": "a", "sessionId": "s1", "turnId": "turn-1", "threadId": "t1"}]),
                gone("turn-1", "a", "t1"),
                pushed("s1", "a", "t1", json!({"prompt": "the brief"})),
            ],
            start,
        );
        w.log.clear();
        w.rt.inject(json!({"type": "thread.marked", "workspaceId": "a", "threadIds": ["t1"]}));
        w.rt.inject(json!({"type": "session.end", "workspaceId": "a", "sessionId": "s1", "turnId": "turn-1", "threadId": "t1"}));
        w.area.rows_sent().await;
        assert_log(
            "a turn end",
            vec![
                json!(["emit", {"type": "thread.marked", "workspaceId": "a", "threadIds": ["t1"]}]),
                json!(["emit", {"type": "session.end", "workspaceId": "a", "sessionId": "s1", "turnId": "turn-1", "threadId": "t1"}]),
                pushed("s1", "a", "t1", json!({})),
            ],
            w.log.take(),
        );
    });
}

/// boot.ts:873: a nap, a wake or an upgrade moves every row on the machine, and no row of another workspace.
#[test]
fn a_nap_moves_every_row_on_the_workspace_and_none_elsewhere() {
    common::run_local(async {
        let w = world(json!([view("r1", "a", "t1", json!({})), view("r2", "a", "t2", json!({})), view("r3", "b", "t3", json!({}))]), json!({})).await;
        w.rt.inject(json!({"type": "workspace.napped", "workspaceId": "a"}));
        w.area.rows_sent().await;
        assert_log(
            "a nap",
            vec![json!(["emit", {"type": "workspace.napped", "workspaceId": "a"}]), pushed("r1", "a", "t1", json!({})), pushed("r2", "a", "t2", json!({}))],
            w.log.take(),
        );
        w.log.clear();
        w.rt.inject(json!({"type": "workspace.woken", "workspaceId": "b", "machineId": "m-b"}));
        w.area.rows_sent().await;
        assert_log("a wake", vec![json!(["emit", {"type": "workspace.woken", "workspaceId": "b", "machineId": "m-b"}]), pushed("r3", "b", "t3", json!({}))], w.log.take());
    });
}

/// boot.ts:681 `subagents`: a thread's subagents ride its latest row alone, from the transcript index's children, and
/// only when there are any.
#[test]
fn listed_rows_carry_subagents_on_the_latest_row_of_a_thread_only() {
    common::run_local(async {
        let w = world(json!([view("r1", "a", "t1", json!({})), view("r2", "a", "t1", json!({})), view("r3", "a", "t2", json!({}))]), json!({})).await;
        let child = json!({"id": "c1", "title": "reads the log", "state": "running", "startedAt": 5});
        w.rt.index_children.borrow_mut().insert(("a".into(), "t1".into()), vec![child.clone()]);
        let held: Vec<_> = w.rt.sessions.borrow().values().cloned().collect();
        let listed = norm(to_json(&w.area.listed_rows(&held)));
        assert!(listed[0].get("subagents").is_none(), "the earlier row of the thread carries none");
        assert_eq!(listed[1]["subagents"], json!([child]));
        assert!(listed[2].get("subagents").is_none(), "a thread with no children lists none");
    });
}

/// boot.ts:783: the computers are read once for a listing, and a row of a held project is joined with its project and
/// its computer's table name where the table has the row; a listing of no rows reads nothing.
#[test]
fn placed_rows_join_the_project_and_the_computers_table_name() {
    common::run_local(async {
        let w = world(json!([view("r1", "a", "t1", json!({})), view("r2", "a", "t2", json!({}))]), json!({})).await;
        *w.rt.door_rows.borrow_mut() = Some(vec![wsp_machines::machines::boot::ComputerRow { id: "hetzner".into(), name: "Hetzner box".into(), kind: wsp_wire::views::place_kind::PlaceKind::Computer }]);
        let held: Vec<_> = w.rt.sessions.borrow().values().cloned().collect();
        let listed = w.area.listed_rows(&held);
        let placed = to_json(&w.area.placed_rows(listed).await);
        assert_eq!(placed[0]["project"], json!({"id": "p1", "name": "site"}));
        assert_eq!(placed[0]["computerName"], json!("Hetzner box"));
        assert_eq!(placed[1]["computerName"], json!("Hetzner box"));
        assert_eq!(to_json(&w.area.placed_rows(vec![]).await), json!([]));
    });
}
