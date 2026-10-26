// SPDX-License-Identifier: AGPL-3.0-only
//! The two writes of the sessions document's own roads, transcripts.ts:607 (the cap drops a row and its thread's record
//! takes the end) and :623 (a settled row that did work marks the thread worked), and the document that carries the
//! record: `{ workspaceId: on, ...held }` (transcripts.ts:649). Expected bytes: fixtures/thread_record_order/gen.mjs,
//! roads `end` and `worked`.
#[path = "support_b/mod.rs"]
mod support;

use serde_json::{json, Map, Value};
use support::transcripts_ctx::{FakeTranscriptsCtx, WithState};
use support::{local, row, sessions_of, view};
use wsp_core::types::internal::ThreadRecord;
use wsp_threads::threads::threads::Row;
use wsp_threads::threads::transcripts::kept::SESSION_INDEX_CAP;

const CASES: &str = include_str!("../../wsp-core/tests/fixtures/thread_record_order/cases.json");

fn cases(road: &str) -> Vec<Value> {
    let all: Vec<Value> = serde_json::from_str(CASES).expect("the cases are JSON");
    all.into_iter().filter(|c| c["road"] == road).collect()
}

fn held(case: &Value) -> ThreadRecord {
    ThreadRecord::from_map(case["before"].as_object().expect("a record").clone()).expect("the record literal")
}

fn sess(id: &str, thread: &str, status: &str, started_at: i64) -> Row {
    row(view(json!({ "id": id, "workspaceId": "w1", "status": status, "threadId": thread, "startedAt": started_at })), &format!("turn-{id}"))
}

fn without_workspace(after: &str) -> String {
    let mut map: Map<String, Value> = serde_json::from_str(after).expect("an object");
    map.shift_remove("workspaceId");
    serde_json::to_string(&map).expect("serializes")
}

fn wrote(ctx: &FakeTranscriptsCtx, thread: &str) -> String {
    ctx.thread_records.with(|t| serde_json::to_string(t.get(thread).expect("the record is held")).expect("serializes"))
}

fn stored(ctx: &FakeTranscriptsCtx, thread: &str) -> String {
    let doc = ctx.store.get_now("sessions", "w1").expect("the document is put");
    serde_json::to_string(&doc["threads"][thread]).expect("serializes")
}

fn live() -> std::rc::Rc<FakeTranscriptsCtx> {
    let ctx = FakeTranscriptsCtx::new();
    ctx.add_live(json!({ "id": "w1" }));
    ctx
}

#[test]
fn a_row_the_cap_drops_ends_its_threads_record_in_the_bytes_node_writes() {
    let all = cases("end");
    assert_eq!(all.len(), 3);
    for case in all {
        local(async move {
            let ctx = live();
            let mut rows = vec![sess("r1", "t1", "failed", 1), sess("r1b", "t1", "completed", 2)];
            rows.extend((0..199).map(|i| sess(&format!("f{i:03}"), &format!("tf{i:03}"), "completed", 100 + i)));
            ctx.sessions.with(|s| *s = sessions_of(rows).take());
            ctx.thread_records.with(|t| {
                t.insert("t1".into(), held(&case));
            });
            let area = ctx.area();
            area.persist_sessions("w1").await;
            assert_eq!(ctx.sessions.with(|s| s.len()), SESSION_INDEX_CAP);
            let after = case["after"].as_str().expect("the bytes");
            assert_eq!(wrote(&ctx, "t1"), after, "{}", case["name"]);
            assert_eq!(stored(&ctx, "t1"), without_workspace(after), "{}: the document", case["name"]);
        });
    }
}

#[test]
fn a_settled_row_that_did_work_marks_its_thread_worked_in_the_bytes_node_writes() {
    let all = cases("worked");
    assert_eq!(all.len(), 2);
    for case in all {
        local(async move {
            let ctx = live();
            let did = sess("r1", "t1", "completed", 1);
            did.borrow_mut().view.claude_session_id = Some("c1".into());
            ctx.sessions.with(|s| *s = sessions_of(vec![did]).take());
            ctx.thread_records.with(|t| {
                t.insert("t1".into(), held(&case));
            });
            let area = ctx.area();
            area.persist_sessions("w1").await;
            let after = case["after"].as_str().expect("the bytes");
            assert_eq!(wrote(&ctx, "t1"), after, "{}", case["name"]);
            assert_eq!(stored(&ctx, "t1"), without_workspace(after), "{}: the document", case["name"]);
        });
    }
}
