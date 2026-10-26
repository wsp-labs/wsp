// SPDX-License-Identifier: AGPL-3.0-only
//! spec/trimmed_row.md. TS cases: thread-keeps-agent.test.ts:108 and :133, thread-lines.test.ts:170 (a thread listed
//! off its record once its rows fell off the cap).
#[path = "support_a/mod.rs"]
mod support;

use std::rc::Rc;

use support::*;
use wsp_wire::types::js_number::JsNumber;

const THREAD: &str = "thr_1";

fn held(record: Value) -> ThreadRecord {
    ThreadRecord { workspace_id: "w1".into(), ..thread_record(record) }
}

fn words(first: Option<&str>, last: f64) -> ThreadWords {
    ThreadWords { first: first.map(str::to_string), last: JsNumber(last), ..Default::default() }
}

fn row_of(host: &FakeHost, record: Value, ended: Option<TurnStatus>) -> Value {
    serde_json::to_value(trimmed_row(host, THREAD, &held(record), ended)).expect("a row serializes")
}

fn say_session(host: &FakeHost, session: &'static str, folder: Option<&'static str>) {
    host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>(
        "TrimmedCtx::started_as",
        Rc::new(|w: &str, t: &str| (w == "w1" && t == THREAD).then(|| session.to_string())),
    );
    host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>(
        "TrimmedCtx::folder_of",
        Rc::new(move |w: &str, s: &str| if w == "w1" && s == session { folder.map(str::to_string) } else { None }),
    );
}

fn say_words(host: &FakeHost, w: ThreadWords) {
    host.hooks.set::<dyn Fn(&str, &str) -> Option<ThreadWords>>(
        "TrimmedCtx::thread_words",
        Rc::new(move |ws: &str, t: &str| (ws == "w1" && t == THREAD).then(|| w.clone())),
    );
}

// T1, T2: thread-lines.test.ts:183
#[test]
fn a_record_with_no_end_reads_completed_under_the_threads_id() {
    let host = FakeHost::new();
    let row = row_of(&host, json!({ "harness": "codex" }), None);
    assert_eq!(row, json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "codex", "status": "completed" }));
    assert_eq!(host.hooks.calls(), vec!["started_as(w1, thr_1)", "thread_words(w1, thr_1)"]);
}

// T1: trimmed-row.ts:9 (the default parameter yields to an end the caller passes)
#[test]
fn the_end_passed_wins_over_the_records() {
    let host = FakeHost::new();
    let row = row_of(&host, json!({ "harness": "claude", "ended": "completed" }), Some(TurnStatus::Failed));
    assert_eq!(row, json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "failed" }));
    let row = row_of(&host, json!({ "harness": "claude" }), Some(TurnStatus::Interrupted));
    assert_eq!(row["status"], "interrupted");
}

// T1: trimmed-row.ts:9 (sessions.ts:924 passes an undefined end for a thread whose record holds one)
#[test]
fn the_records_own_end_is_the_default() {
    let host = FakeHost::new();
    let row = row_of(&host, json!({ "harness": "claude", "ended": "interrupted" }), None);
    assert_eq!(row, json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "interrupted" }));
    let row = row_of(&host, json!({ "harness": "claude", "ended": "failed" }), None);
    assert_eq!(row["status"], "failed");
}

// T3: trimmed-row.ts:10
#[test]
fn a_thread_with_a_session_names_it_and_its_folder() {
    let host = FakeHost::new();
    say_session(&host, "sess-9", Some("/root/app"));
    let row = row_of(&host, json!({ "harness": "claude" }), None);
    assert_eq!(
        row,
        json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "completed", "claudeSessionId": "sess-9", "cwd": "/root/app" })
    );
    assert_eq!(host.hooks.calls(), vec!["started_as(w1, thr_1)", "folder_of(w1, sess-9)", "thread_words(w1, thr_1)"]);
}

// T3: trimmed-row.ts:11
#[test]
fn no_session_asks_no_folder() {
    let host = FakeHost::new();
    host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>(
        "TrimmedCtx::folder_of",
        Rc::new(|_w: &str, _s: &str| Some("/root/never".to_string())),
    );
    let row = row_of(&host, json!({ "harness": "claude" }), None);
    assert_eq!(row, json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "completed" }));
    assert_eq!(host.hooks.called("folder_of"), Vec::<String>::new());
}

// T3: trimmed-row.ts:11
#[test]
fn a_session_with_no_folder_has_no_cwd() {
    let host = FakeHost::new();
    say_session(&host, "sess-9", None);
    let row = row_of(&host, json!({ "harness": "claude" }), None);
    assert_eq!(
        row,
        json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "completed", "claudeSessionId": "sess-9" })
    );
    assert_eq!(host.hooks.called("folder_of"), vec!["folder_of(w1, sess-9)"]);
}

// T4: trimmed-row.ts:14-15
#[test]
fn the_first_words_title_it_and_the_last_place_it_in_time() {
    let host = FakeHost::new();
    say_words(&host, words(Some("fix the build"), 1_760_000_000_500.0));
    let row = row_of(&host, json!({ "harness": "claude" }), None);
    assert_eq!(
        row,
        json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "completed", "prompt": "fix the build", "startedAt": 1_760_000_000_500i64, "endedAt": 1_760_000_000_500i64 })
    );
    // A thread whose first line was never kept still has its place in time, and no prompt.
    say_words(&host, words(None, 42.0));
    let row = row_of(&host, json!({ "harness": "claude" }), None);
    assert_eq!(
        row,
        json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "completed", "startedAt": 42, "endedAt": 42 })
    );
}

// T4: trimmed-row.ts:15 (`words.last > 0`)
#[test]
fn last_zero_places_nothing() {
    let host = FakeHost::new();
    say_words(&host, words(Some("fix the build"), 0.0));
    let row = row_of(&host, json!({ "harness": "claude" }), None);
    assert_eq!(
        row,
        json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "completed", "prompt": "fix the build" })
    );
}

// T4: trimmed-row.ts:14 (`?.words.get(threadId)` answers nothing)
#[test]
fn no_words_no_prompt_and_no_times() {
    let host = FakeHost::new();
    let row = row_of(&host, json!({ "harness": "claude" }), None);
    assert_eq!(row, json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "completed" }));
    assert_eq!(host.hooks.called("thread_words"), vec!["thread_words(w1, thr_1)"]);
}

// T5, T6: trimmed-row.ts:19-20
#[test]
fn a_child_reads_started_by_its_agent_under_its_parent() {
    let host = FakeHost::new();
    let row = row_of(&host, json!({ "harness": "claude", "parentThreadId": "thr_lead", "rootThreadId": "thr_root" }), None);
    assert_eq!(
        row,
        json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "completed", "startedBy": "agent", "parentThreadId": "thr_lead", "rootThreadId": "thr_root" })
    );
    // A root alone says nothing of who started the thread.
    let row = row_of(&host, json!({ "harness": "claude", "rootThreadId": "thr_root" }), None);
    assert_eq!(
        row,
        json!({ "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "claude", "status": "completed", "rootThreadId": "thr_root" })
    );
}

// T6: thread-keeps-agent.test.ts:108 (the agent and the access of a thread read off its record)
#[test]
fn the_records_root_access_and_fork_ride_the_row() {
    let host = FakeHost::new();
    let record = json!({
        "harness": "codex",
        "permissionMode": "read-only",
        "rootThreadId": "thr_root",
        "forkedFrom": { "threadId": "thr_src", "turnId": "turn_3", "title": "the build" },
    });
    let row = row_of(&host, record, None);
    assert_eq!(
        row,
        json!({
            "id": THREAD, "threadId": THREAD, "workspaceId": "w1", "harness": "codex", "status": "completed",
            "rootThreadId": "thr_root", "permissionMode": "read-only",
            "forkedFrom": { "threadId": "thr_src", "turnId": "turn_3", "title": "the build" },
        })
    );
}

// T2: trimmed-row.ts:9 (the workspace is the record's, not a row's)
#[test]
fn the_workspace_is_the_records_and_every_read_asks_it() {
    let host = FakeHost::new();
    let other = ThreadRecord { workspace_id: "w7".into(), ..thread_record(json!({ "harness": "claude" })) };
    let row = serde_json::to_value(trimmed_row(&*host, "thr_9", &other, None)).unwrap();
    assert_eq!(row, json!({ "id": "thr_9", "threadId": "thr_9", "workspaceId": "w7", "harness": "claude", "status": "completed" }));
    assert_eq!(host.hooks.calls(), vec!["started_as(w7, thr_9)", "thread_words(w7, thr_9)"]);
}
