// SPDX-License-Identifier: AGPL-3.0-only
//! spec/waits.md. TS cases: thread-waits.test.ts, subagents.test.ts:341.
mod support;

use std::rc::Rc;

use support::*;

const MIN: f64 = 60_000.0;

/// One workspace with one thread whose turn `turn-1` (session `sess-1`) is running.
fn host() -> (Rc<FakeHost>, WaitsArea) {
    let host = FakeHost::new();
    host.add_entry(entry("w1", json!({})));
    let mut r = row(view("sess-1", "w1", json!({ "threadId": "t1", "claudeSessionId": "sess-1", "startedAt": T0 })), "turn-1");
    r.turn_id = "turn-1".into();
    host.add_row("sess-1", r);
    let area = WaitsArea::new(host.clone());
    (host, area)
}

fn waited(host: &FakeHost) -> Vec<Value> {
    host.recorded_of("thread.waited")
}

fn waited_row(ms: f64, said: Option<&str>) -> Value {
    let mut v = json!({ "type": "thread.waited", "workspaceId": "w1", "sessionId": "sess-1", "turnId": "turn-1", "threadId": "t1", "ms": ms as i64 });
    if let Some(s) = said {
        v["said"] = json!(s);
    }
    v
}

// W6, W7: thread-waits.test.ts:39
#[test]
fn one_row_for_a_14_minute_hold_and_none_for_2_seconds() {
    let (host, area) = host();
    area.note_waited("t1", 2_000.0, None);
    host.clock.advance(WAIT_SETTLE_MS);
    assert_eq!(waited(&host), Vec::<Value>::new());
    area.note_waited("t1", 14.0 * MIN, None);
    assert_eq!(waited(&host), Vec::<Value>::new(), "nothing before the wait settles");
    host.clock.advance(WAIT_SETTLE_MS);
    assert_eq!(waited(&host), vec![waited_row(14.0 * MIN, None)]);
}

// W7: thread-waits.test.ts:55
#[test]
fn one_row_of_the_longest_for_holds_together_and_a_new_row_for_a_later_wait() {
    let (host, area) = host();
    area.note_waited("t1", 2.0 * MIN, None);
    area.note_waited("t1", 14.0 * MIN, None);
    host.clock.advance(WAIT_SETTLE_MS - 1);
    area.note_waited("t1", 9.0 * MIN, None);
    host.clock.advance(1);
    assert_eq!(waited(&host), vec![waited_row(14.0 * MIN, None)]);
    host.clock.advance(20 * 60_000);
    area.note_waited("t1", 3.0 * MIN, None);
    host.clock.advance(WAIT_SETTLE_MS);
    assert_eq!(waited(&host), vec![waited_row(14.0 * MIN, None), waited_row(3.0 * MIN, None)]);
}

// W4, W5: thread-waits.test.ts:70
#[test]
fn a_sentence_is_written_once_per_turn_however_short_the_hold() {
    let (host, area) = host();
    let said = "Codex's sign-in for cloud threads on acme's laptop ended. Sign in again in Settings, Agents.";
    area.note_waited("t1", 0.0, Some(&format!("wsp: {said}")));
    area.note_waited("t1", 0.0, Some(&format!("wsp: {said}")));
    host.clock.advance(WAIT_SETTLE_MS);
    assert_eq!(waited(&host), vec![waited_row(0.0, Some(said))]);
    area.note_waited("t1", 0.0, Some("another sentence"));
    assert_eq!(waited(&host).len(), 2, "another sentence is written at once");
    assert_eq!(waited(&host)[1], waited_row(0.0, Some("another sentence")));
}

// W2: thread-waits.test.ts:78
#[test]
fn nothing_for_a_thread_no_row_holds() {
    let (host, area) = host();
    area.note_waited("thr_gone", 14.0 * MIN, None);
    host.clock.advance(WAIT_SETTLE_MS);
    assert_eq!(host.recorded_json(), Vec::<Value>::new());
}

// W2
#[test]
fn nothing_while_the_host_closes() {
    let (host, area) = host();
    area.note_waited("t1", 14.0 * MIN, None);
    host.closing.set(true);
    host.clock.advance(WAIT_SETTLE_MS);
    area.note_waited("t1", 0.0, Some("a sentence"));
    assert_eq!(host.recorded_json(), Vec::<Value>::new());
}

// W6
#[test]
fn exactly_the_threshold_writes_nothing() {
    let (host, area) = host();
    area.note_waited("t1", 5_000.0, None);
    area.note_waited("t1", f64::NAN, None);
    host.clock.advance(WAIT_SETTLE_MS * 2);
    assert_eq!(waited(&host), Vec::<Value>::new());
    assert_eq!(host.clock.pending(), 0, "no timer is armed for a hold under the threshold");
}

// W1
#[test]
fn the_row_started_latest_takes_the_write_and_a_tie_goes_to_the_later_row() {
    let (host, area) = host();
    let mut older = row(view("sess-0", "w1", json!({ "threadId": "t1", "startedAt": T0 - 10 })), "turn-0");
    older.turn_id = "turn-0".into();
    host.add_row("sess-0", older);
    let mut tie = row(view("turn-2", "w1", json!({ "threadId": "t1", "startedAt": T0 })), "turn-2");
    tie.turn_id = "turn-2".into();
    host.add_row("turn-2", tie);
    area.note_waited("t1", 0.0, Some("held"));
    let rows = waited(&host);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["turnId"], "turn-2", "the later of two rows started in one ms");
    assert_eq!(rows[0]["sessionId"], "turn-2", "a row with no agent session is named by its turn");
}

// W3
#[test]
fn ms_is_rounded_and_never_negative() {
    let (host, area) = host();
    area.note_waited("t1", 6_000.5, None);
    host.clock.advance(WAIT_SETTLE_MS);
    area.note_waited("t1", -3.0, Some("a"));
    area.note_waited("t1", 2.4, Some("b"));
    let ms: Vec<Value> = waited(&host).iter().map(|e| e["ms"].clone()).collect();
    assert_eq!(ms, vec![json!(6001), json!(0), json!(2)]);
}

// W3
#[test]
fn the_session_id_is_the_agents_else_the_turns() {
    let (host, area) = host();
    area.note_waited("t1", 0.0, Some("one"));
    host.rows.borrow()["sess-1"].view.borrow_mut().claude_session_id = None;
    area.note_waited("t1", 0.0, Some("two"));
    let ids: Vec<Value> = waited(&host).iter().map(|e| e["sessionId"].clone()).collect();
    assert_eq!(ids, vec![json!("sess-1"), json!("turn-1")]);
}

// W5
#[test]
fn only_the_leading_wsp_prefix_is_cut() {
    let (host, area) = host();
    area.note_waited("t1", 0.0, Some("wsp: wsp: twice"));
    area.note_waited("t1", 0.0, Some("says wsp: inside"));
    let said: Vec<Value> = waited(&host).iter().map(|e| e["said"].clone()).collect();
    assert_eq!(said, vec![json!("wsp: twice"), json!("says wsp: inside")]);
}

fn retries(host: &FakeHost) -> Vec<Value> {
    host.recorded_of("session.retry")
}

fn retry_row(attempt: u32, why: &str) -> Value {
    json!({ "type": "session.retry", "workspaceId": "w1", "sessionId": "sess-1", "turnId": "turn-1", "threadId": "t1", "attempt": attempt, "why": why })
}

// W9
#[test]
fn a_retry_on_a_thread_not_running_is_dropped() {
    let (host, area) = host();
    host.rows.borrow()["sess-1"].view.borrow_mut().status = serde_json::from_value(json!("completed")).unwrap();
    area.note_retry("t1", RetryNote { attempt: 1, why: "Anthropic did not answer".into() });
    assert_eq!(host.clock.pending(), 0, "no timer for a thread not running");
    host.clock.advance(RETRY_SETTLE_MS);
    assert_eq!(retries(&host), Vec::<Value>::new());
}

// W10, W11: subagents.test.ts:341
#[test]
fn a_retry_the_agent_says_in_the_window_is_no_row_and_one_it_never_says_is_written() {
    let (host, area) = host();
    area.note_retry("t1", RetryNote { attempt: 1, why: "Anthropic did not answer".into() });
    area.agent_retried("turn-1", Some("Anthropic did not answer"));
    host.clock.advance(1_000);
    area.note_retry("t1", RetryNote { attempt: 3, why: "Anthropic answered with a server error (500)".into() });
    host.clock.advance(850);
    area.note_retry("t1", RetryNote { attempt: 1, why: "Anthropic was overloaded".into() });
    host.clock.advance(300);
    area.agent_retried("turn-1", Some("Anthropic was overloaded"));
    host.clock.advance(RETRY_SETTLE_MS);
    assert_eq!(retries(&host), vec![retry_row(3, "Anthropic answered with a server error (500)")]);
}

// W10
#[test]
fn a_retry_whose_turn_ended_before_the_fire_writes_nothing() {
    let (host, area) = host();
    area.note_retry("t1", RetryNote { attempt: 2, why: "Anthropic was overloaded".into() });
    host.rows.borrow()["sess-1"].view.borrow_mut().status = serde_json::from_value(json!("completed")).unwrap();
    host.clock.advance(RETRY_SETTLE_MS);
    assert_eq!(retries(&host), Vec::<Value>::new());
}

// W11
#[test]
fn the_agents_retry_takes_the_newest_note_of_its_reason_else_the_newest() {
    let (host, area) = host();
    for (attempt, why) in [(1, "a"), (2, "b"), (3, "a"), (4, "c")] {
        area.note_retry("t1", RetryNote { attempt, why: why.into() });
    }
    area.agent_retried("turn-1", Some("a"));
    area.agent_retried("turn-1", Some("nothing like it"));
    area.agent_retried("turn-1", None);
    area.agent_retried("turn-other", Some("a"));
    host.clock.advance(RETRY_SETTLE_MS);
    assert_eq!(retries(&host), vec![retry_row(1, "a")], "attempt 3 went to its reason, 4 then 2 to the newest");
}
