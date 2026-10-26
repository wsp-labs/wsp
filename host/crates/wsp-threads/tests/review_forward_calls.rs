// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448), path 1: the tool calls a turn is inside while its harness is being opened.
//! turns.ts:168 makes `calls` a local map when the turn starts, so every `turn.delta` of kind `tool_use` that the
//! harness hands over inside `t.open(forward)` (turns.ts:576) is in the map when `sessions.set(rowId, { ..., calls })`
//! puts it on the row (:650). A re-opened turn does exactly this: turns.ts:854-858 forwards every event it held while
//! the row did not exist from inside `open`. `stoppedBehind` (threads.ts:527) reads those calls to say a lead is
//! stopped on a question of a thread it follows.
//! The port writes the call into `row.calls` (forward.rs:315-319) and then replaces `row.calls` with an empty map
//! once `open` has returned (run.rs:897).
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("turns_run/world.rs");

fn tool_use(id: &str, name: &str, input: &str) -> Value {
    json!({ "type": "turn.delta", "sessionId": SESS, "kind": "tool_use", "text": input, "toolName": name, "toolUseId": id })
}

fn tool_result(id: &str) -> Value {
    json!({ "type": "turn.delta", "sessionId": SESS, "kind": "tool_result", "text": "ok", "toolUseId": id })
}

fn open_calls(host: &FakeHost) -> Vec<(String, String)> {
    let row = host.rows.borrow().get(SESS).cloned().expect("the turn's row under its session id");
    let row = row.borrow();
    row.calls.as_ref().map(|calls| calls.iter().map(|(id, c)| (id.clone(), c.tool_name.clone())).collect()).unwrap_or_default()
}

/// A harness that hands its events over before `open` returns, as a re-opened run's held events are.
fn open_with(session: Arc<FakeSession>, events: Vec<Value>) -> Open {
    Box::new(move |forward: OnEvent| {
        for doc in events {
            forward(adapter_event(doc));
        }
        Ok(session as Arc<dyn HarnessSession>)
    })
}

/// A call opened while the turn opens stays open on the row: the lead is inside `wsp_wait` the whole time.
#[test]
fn a_call_the_harness_opened_before_open_returned_is_open_on_the_row() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let session = FakeSession::new(SESS);
        let slot = Rc::new(RefCell::new(None));
        let mut t = turn_start(&entry, &turn, &session, &slot);
        t.open = open_with(session.clone(), vec![tool_use("toolu_1", "wsp_wait", "{\"threads\":[\"abc\"]}")]);
        let _handle = TurnsArea::new(host.clone()).run_turn(t).expect("the turn opens");
        assert_eq!(open_calls(&host), vec![("toolu_1".to_string(), "wsp_wait".to_string())]);
    });
}

/// The same for a call that closed during open: only the call with no result yet is left, and it is not lost.
#[test]
fn only_the_call_still_open_is_left_after_open_has_read_the_run_so_far() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let session = FakeSession::new(SESS);
        let slot = Rc::new(RefCell::new(None));
        let mut t = turn_start(&entry, &turn, &session, &slot);
        t.open = open_with(
            session.clone(),
            vec![tool_use("toolu_1", "Bash", "{}"), tool_use("toolu_2", "wsp_wait", "{}"), tool_result("toolu_1")],
        );
        let _handle = TurnsArea::new(host.clone()).run_turn(t).expect("the turn opens");
        assert_eq!(open_calls(&host), vec![("toolu_2".to_string(), "wsp_wait".to_string())]);
    });
}
