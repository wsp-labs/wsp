// SPDX-License-Identifier: AGPL-3.0-only
//! Rules CT1 to CT8 of spec/codex_turn.md, "Rules added for the port": the end of a turn on each road, the order of
//! what is said there, and what each session method answers once the turn is over. Every expected value was printed
//! by the TS adapter at 65f6b89c7 driven through the same lines (adapter.test.ts's own wire).

#[path = "common/codex_wire.rs"]
mod codex_wire;

use std::sync::Arc;

use codex_wire::*;
use serde_json::Value;
use wsp_adapters::codex::adapter::{create_codex_adapter, CodexAdapter, CodexAdapterDeps};
use wsp_adapters::codex::CodexSession;
use wsp_adapters::port::{AdapterEvent, AnswerOutcome, HarnessStartOptions, PermissionAnswer, PermissionOutcome, SteerOutcome, TaskStop, TurnResult, TurnStatus, PERMISSION_ALLOW};
use wsp_wire::views::session::TurnRefusal;

const THREAD: &str = "01a0e2c1-5d10-7b42-9a6e-3f1c2d4b5a60";
const TURN: &str = "01a0e2c1-5e02-7c11-8d3f-9b2a1c0d4e71";
const CHILD: &str = "019fcfd6-2883-77e0-9013-4410ede70371";
const CHILD_TURN: &str = "019fcfd6-28bf-7e00-a873-4554526dc845";
const NOT_SIGNED_IN: &str = "Codex is not signed in where this thread runs; run codex login --device-auth there";

fn thread_answer() -> String {
    format!(r#"{{"id":"wsp-thread","result":{{"thread":{{"id":"{THREAD}","model":"gpt-5.6-sol","cwd":"/root/app","turns":[]}},"model":"gpt-5.6-sol","cwd":"/root/app"}}}}"#)
}
fn turn_started() -> String {
    format!(r#"{{"method":"turn/started","params":{{"threadId":"{THREAD}","turn":{{"id":"{TURN}","items":[],"status":"inProgress"}}}}}}"#)
}
fn completed(status: &str) -> String {
    format!(r#"{{"method":"turn/completed","params":{{"threadId":"{THREAD}","turn":{{"id":"{TURN}","items":[],"status":"{status}"}}}}}}"#)
}
fn child_started() -> String {
    format!(r#"{{"method":"turn/started","params":{{"threadId":"{CHILD}","turn":{{"id":"{CHILD_TURN}","items":[],"status":"inProgress"}}}}}}"#)
}
fn agent_message(text: &str) -> String {
    format!(r#"{{"method":"item/completed","params":{{"item":{{"type":"agentMessage","id":"m1","text":"{text}"}},"threadId":"{THREAD}","turnId":"{TURN}"}}}}"#)
}
fn approval(id: &str) -> String {
    format!(r#"{{"id":{id},"method":"item/commandExecution/requestApproval","params":{{"threadId":"{THREAD}","turnId":"{TURN}","itemId":"call_1","command":"/bin/zsh -lc 'ls'","cwd":"/root/app"}}}}"#)
}
fn error_note(message: &str) -> String {
    format!(r#"{{"method":"error","params":{{"error":{{"message":"{message}","additionalDetails":null}},"willRetry":false,"threadId":"{THREAD}","turnId":"{TURN}"}}}}"#)
}

fn method_of(m: &Value) -> String {
    m.get("method").and_then(Value::as_str).unwrap_or("").to_string()
}

/// A server that answers the thread request, then prints `rest` once the turn starts. With `hang` it exits only when
/// the case says so.
fn scripted(rest: Vec<String>, hang: bool) -> Launch {
    launcher(scripted_server(WireOpts {
        hang,
        on_write: Some(Arc::new(move |m: &Value, w: &Wire| match method_of(m).as_str() {
            "thread/start" | "thread/resume" => w.push(&[thread_answer()]),
            "turn/start" => {
                w.push(&[turn_started()]);
                w.push(&rest);
            }
            _ => {}
        })),
        ..Default::default()
    }))
}

fn adapter(launch: &Launch) -> CodexAdapter {
    create_codex_adapter(CodexAdapterDeps { interrupt_grace_ms: Some(10), result_exit_ms: Some(10), ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", "codex login --device-auth") }).unwrap()
}

fn start(adapter: &CodexAdapter, events: &Events) -> CodexSession {
    adapter.start(HarnessStartOptions { prompt: "go".into(), on_event: Some(events.on_event()), ..Default::default() }).unwrap()
}

fn failed(error: &str) -> TurnResult {
    TurnResult { status: TurnStatus::Failed, duration_ms: None, waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: None, error: Some(error.into()), unreached: None, refusal: None, limit: None }
}

fn allow() -> PermissionAnswer {
    PermissionAnswer { option_id: PERMISSION_ALLOW.into(), outcome: PermissionOutcome::Allowed, deny_message: String::new(), reason: None }
}

fn session_end(events: &Events) -> (Option<i32>, bool, Option<String>) {
    match events.all().last() {
        Some(AdapterEvent::SessionEnd { exit_code, saw_result, failure, .. }) => (*exit_code, *saw_result, failure.as_ref().map(|f| f.message.clone())),
        other => panic!("the last event is session.end, not {other:?}"),
    }
}

#[tokio::test]
async fn the_stderr_tail_keeps_the_last_five_non_blank_trimmed_lines_unreadable_json_among_them() {
    // CT1; adapter.ts:1080-1085, 1117-1118. A line read_message refuses is stderr, a JSON array or an object with
    // neither method nor id included; blank lines are dropped before the count.
    let rest = ["  first  ", "", "   ", "two", r#"{"neither":true}"#, "three", "[1,2]", "four", "five"].map(String::from).to_vec();
    let launch = scripted(rest, true);
    let events = Events::default();
    let session = start(&adapter(&launch), &events);
    until(|| events.any("turn.anchor")).await;
    sleep_ms(5).await;
    launch.wire(0).exit(Some(3));
    assert_eq!(session.finished().await, failed("codex exited with code 3 before its turn ended: {\"neither\":true}\nthree\n[1,2]\nfour\nfive"));
    assert_eq!(events.kinds(), ["session.start", "session.start", "turn.anchor", "turn.done", "session.end"]);
    assert_eq!(session_end(&events), (Some(3), false, None));
}

#[tokio::test]
async fn an_error_with_no_message_leaves_an_empty_reason_and_a_signal_reads_code_null() {
    // CT2; adapter.ts:1047-1048, 1117-1118. `lastError` is "" (not absent), so the stderr tail is not read and the
    // words end in ": ". A process ended by a signal has no code: JS writes `null`.
    let launch = scripted(vec![format!(r#"{{"method":"error","params":{{"error":{{}},"willRetry":false,"threadId":"{THREAD}"}}}}"#), "a stderr line".into()], true);
    let session = start(&adapter(&launch), &Events::default());
    sleep_ms(20).await;
    launch.wire(0).exit(Some(1));
    assert_eq!(session.finished().await, failed("codex exited with code 1 before its turn ended: "));

    let launch = scripted(vec![], true);
    let events = Events::default();
    let session = start(&adapter(&launch), &events);
    until(|| events.any("turn.anchor")).await;
    launch.wire(0).exit(None);
    assert_eq!(session.finished().await, failed("codex exited with code null before its turn ended"));
    assert_eq!(session_end(&events), (None, false, None));
}

#[tokio::test]
async fn failure_words_said_before_the_stream_throws_beat_the_stream_s_message_which_rides_session_end() {
    // CT3; adapter.ts:1102-1105, 1119-1121, 1124. The exit road's failed result carries no durationMs.
    let launch = scripted(vec![error_note("unexpected status 401 Unauthorized: Missing bearer")], true);
    let events = Events::default();
    let session = start(&adapter(&launch), &events);
    until(|| events.any("turn.anchor")).await;
    sleep_ms(5).await;
    let w = launch.wire(0);
    w.fail(failure("the machine went away"));
    w.exit(Some(255));
    let mut expected = failed(NOT_SIGNED_IN);
    expected.refusal = Some(TurnRefusal::SignIn);
    assert_eq!(session.finished().await, expected);
    assert_eq!(session_end(&events), (Some(255), false, Some("the machine went away".into())));
}

#[tokio::test]
async fn a_process_that_goes_mid_turn_cancels_its_asks_then_fails_its_children_then_ends_the_turn() {
    // CT4; adapter.ts:1108-1125. Order: each open ask closes cancelled, every awaited request settles gone (a steer
    // waiting on its answer reads not-running), each running child ends failed with the count after it, turn.done,
    // session.end. Afterwards every method answers as over and nothing more is written.
    let launch = scripted(vec![child_started(), approval("7")], true);
    let events = Events::default();
    let session = start(&adapter(&launch), &events);
    until(|| events.any("permission.ask")).await;
    let steered = session.steer("more".into());
    sleep_ms(5).await;
    let w = launch.wire(0);
    w.exit(Some(1));
    assert_eq!(session.finished().await, failed("codex exited with code 1 before its turn ended"));
    assert_eq!(steered.await, SteerOutcome::NotRunning);
    assert_eq!(
        events.kinds(),
        ["session.start", "session.start", "turn.anchor", "subagent", "turn.tasks", "permission.ask", "permission.close", "subagent", "turn.tasks", "turn.done", "session.end"]
    );
    let all = events.all();
    assert!(matches!(&all[6], AdapterEvent::PermissionClose { ask_id, outcome: PermissionOutcome::Cancelled, option_id: None, .. } if ask_id == "7"));
    assert!(matches!(&all[7], AdapterEvent::Subagent { task, state: wsp_adapters::port::SubagentState::Failed, parent_tool_use_id: Some(parent), title: None, summary: None, usage: None, .. } if task == CHILD && parent == CHILD));
    assert!(matches!(&all[8], AdapterEvent::TurnTasks { running: 0, .. }));
    assert_eq!(session_end(&events), (Some(1), false, None));

    let written = w.written().len();
    assert_eq!(session.answer("7".into(), allow()).await, Ok(AnswerOutcome::Gone));
    assert_eq!(session.steer("again".into()).await, SteerOutcome::NotRunning);
    assert_eq!(session.stop_task(CHILD.into()).await, TaskStop::NotRunning);
    session.interrupt().await;
    assert_eq!(w.written().len(), written, "nothing reaches a process that is gone");
}

#[tokio::test]
async fn a_turn_that_completes_under_an_open_ask_says_turn_done_first_then_cancels_the_ask() {
    // CT5; adapter.ts:763-767. The finish road is the other way round from the exit road: turn.done, then each open
    // ask closed cancelled, then stdin closed. A finished turn stays in the registry under its local id (never removed),
    // and its methods write nothing.
    let launch = scripted(vec![approval(r#""ask-1""#), agent_message("done"), completed("completed")], false);
    let a = adapter(&launch);
    let events = Events::default();
    let session = start(&a, &events);
    let result = session.finished().await;
    assert_eq!((result.status, result.text.as_deref(), result.model.as_deref()), (TurnStatus::Completed, Some("done"), Some("gpt-5.6-sol")));
    assert_eq!(events.kinds(), ["session.start", "session.start", "turn.anchor", "permission.ask", "delta:text", "turn.done", "permission.close", "session.end"]);
    let w = launch.wire(0);
    assert!(w.closed());
    assert_eq!(session.steer("x".into()).await, SteerOutcome::NotRunning);
    assert_eq!(session.answer("ask-1".into(), allow()).await, Ok(AnswerOutcome::Gone));
    assert_eq!(session.stop_task("nobody".into()).await, TaskStop::NotRunning);
    let methods: Vec<String> = w.written().iter().map(method_of).collect();
    assert_eq!(methods, ["initialize", "initialized", "account/read", "account/rateLimits/read", "thread/start", "turn/start"]);
    let keys: Vec<String> = a.sessions().keys().cloned().collect();
    assert_eq!(keys, vec![session.local_id()]);
    assert_ne!(session.local_id(), THREAD, "a new thread's local id is a fresh uuid, not the id the server names");
}

#[tokio::test]
async fn a_lead_end_with_no_status_or_an_unknown_one_fails_in_the_default_words_and_another_turn_s_end_is_ignored() {
    // CT6; adapter.ts:1053-1056, 778. The lead's failed result carries durationMs; one ended only by the exit does not.
    for line in [format!(r#"{{"method":"turn/completed","params":{{"threadId":"{THREAD}","turn":{{"id":"{TURN}","items":[]}}}}}}"#), completed("weird")] {
        let launch = scripted(vec![line], true);
        let session = start(&adapter(&launch), &Events::default());
        let result = session.finished().await;
        assert!(result.duration_ms.is_some());
        assert_eq!(TurnResult { duration_ms: None, ..result }, failed("codex reported a failed turn"));
    }
    let other = format!(r#"{{"method":"turn/completed","params":{{"threadId":"{THREAD}","turn":{{"id":"other","status":"completed"}}}}}}"#);
    let launch = scripted(vec![other], true);
    let session = start(&adapter(&launch), &Events::default());
    sleep_ms(20).await;
    launch.wire(0).exit(Some(0));
    assert_eq!(session.finished().await, failed("codex exited with code 0 before its turn ended"));
}

#[tokio::test]
async fn a_thread_or_handshake_the_server_refuses_ends_the_turn_under_its_local_id_with_no_session_start() {
    // CT7; adapter.ts:938-948. The refusal comes before any announce, so the turn says turn.done and session.end alone,
    // under the local id; a refused turn/start comes after it, so session.start (unprompted) is said once.
    let cases = [
        (r#"{"id":"wsp-thread","error":{"code":-32600,"message":"thread 0199 already has an active writer"}}"#, None, "Codex is writing this conversation in another app, so this turn did not run. Close it there and send again. Codex lets go about a minute after its window closes."),
        (r#"{"id":"wsp-thread","error":{"code":-32600,"message":"no such thing"}}"#, None, "codex could not open the thread: no such thing"),
        (r#"{"id":"wsp-initialize","error":{"code":-32600,"message":"too old"}}"#, None, "codex could not start the turn: too old"),
        ("", Some(r#"{"id":"wsp-turn","error":{"code":-32600,"message":"bad input"}}"#), "codex could not start the turn: bad input"),
    ];
    for (on_thread, on_turn, words) in cases {
        let launch = launcher(scripted_server(WireOpts {
            on_write: Some(Arc::new(move |m: &Value, w: &Wire| match method_of(m).as_str() {
                "thread/start" if on_turn.is_some() => w.push(&[thread_answer()]),
                "thread/start" => w.push(&[on_thread]),
                "turn/start" => w.push(&[on_turn.unwrap_or_default()]),
                _ => {}
            })),
            ..Default::default()
        }));
        let events = Events::default();
        let session = start(&adapter(&launch), &events);
        assert_eq!(session.finished().await, failed(words), "{words}");
        let all = events.all();
        if on_turn.is_some() {
            assert_eq!(events.kinds(), ["session.start", "turn.done", "session.end"]);
            assert!(matches!(&all[0], AdapterEvent::SessionStart { prompted: Some(false), .. }));
        } else {
            assert_eq!(events.kinds(), ["turn.done", "session.end"]);
            assert!(all.iter().all(|e| e.session_id() == session.local_id()), "nothing named the thread, so its events go under the local id");
            assert!(!launch.wire(0).wrote("turn/start"));
        }
    }
}

#[tokio::test]
async fn a_lead_that_ended_while_its_child_ran_is_completed_when_the_process_goes_and_its_stdin_never_closed() {
    // CT8; adapter.ts:785-788, 1110-1114. The lead's end is held while the child runs; the exit fails the child, then
    // the held end is the result, said with turn.done, and session.end reads sawResult. Nothing closed stdin first.
    let launch = scripted(vec![child_started(), agent_message("lead said"), completed("completed")], true);
    let events = Events::default();
    let session = start(&adapter(&launch), &events);
    until(|| events.any("delta:text")).await;
    sleep_ms(10).await;
    assert!(!events.any("turn.done"));
    let w = launch.wire(0);
    assert!(!w.closed());
    w.exit(Some(0));
    let result = session.finished().await;
    assert!(result.duration_ms.is_some());
    assert_eq!((result.status, result.text.as_deref(), result.model.as_deref(), result.error.as_deref()), (TurnStatus::Completed, Some("lead said"), Some("gpt-5.6-sol"), None));
    assert_eq!(events.kinds()[6..], ["subagent", "turn.tasks", "turn.done", "session.end"]);
    assert_eq!(session_end(&events), (Some(0), true, None));
}

#[tokio::test]
async fn a_launched_start_s_model_rides_the_thread_line_but_the_turn_names_the_model_the_server_answered() {
    // CT10; adapter.ts:1283 hands `follow` the cwd alone, so `announce` (:642) takes the server's model.
    let launch = scripted(vec![agent_message("ok"), completed("completed")], false);
    let events = Events::default();
    let session = adapter(&launch)
        .start(HarnessStartOptions { prompt: "go".into(), model: Some("my-model".into()), cwd: Some("/my/cwd".into()), on_event: Some(events.on_event()), ..Default::default() })
        .unwrap();
    let result = session.finished().await;
    assert_eq!(result.model.as_deref(), Some("gpt-5.6-sol"));
    let starts: Vec<(Option<String>, Option<String>, Option<bool>)> = events
        .all()
        .into_iter()
        .filter_map(|e| match e {
            AdapterEvent::SessionStart { model, cwd, prompted, .. } => Some((model, cwd, prompted)),
            _ => None,
        })
        .collect();
    let said = (Some("gpt-5.6-sol".to_string()), Some("/my/cwd".to_string()));
    assert_eq!(starts, vec![(said.0.clone(), said.1.clone(), Some(false)), (said.0, said.1, None)]);
    assert_eq!(launch.wire(0).written_method("thread/start")[0]["params"]["model"], "my-model");
}

#[tokio::test]
async fn failure_words_said_earlier_beat_the_completed_turn_s_own_error() {
    // CT11; adapter.ts:770-771 (`failureWords(message) ?? words`).
    let failed_with = format!(r#"{{"method":"turn/completed","params":{{"threadId":"{THREAD}","turn":{{"id":"{TURN}","items":[],"status":"failed","error":{{"message":"other boom"}}}}}}}}"#);
    let launch = scripted(vec![error_note("Missing environment variable: `FOO_KEY`"), failed_with], false);
    let result = start(&adapter(&launch), &Events::default()).finished().await;
    assert!(result.duration_ms.is_some());
    assert_eq!(TurnResult { duration_ms: None, ..result }, failed("Codex's model provider reads its key from the environment variable FOO_KEY, which is not set on this machine"));
}

#[tokio::test]
async fn a_later_turn_started_takes_the_turn_id_with_no_new_anchor_and_the_old_turn_s_end_is_ignored() {
    // CT12; adapter.ts:966-973, 1053-1054.
    let started = |id: &str| format!(r#"{{"method":"turn/started","params":{{"threadId":"{THREAD}","turn":{{"id":"{id}"}}}}}}"#);
    let done = |id: &str| format!(r#"{{"method":"turn/completed","params":{{"threadId":"{THREAD}","turn":{{"id":"{id}","status":"completed"}}}}}}"#);
    let (one, two) = (started("tt1"), started("tt2"));
    let launch = launcher(scripted_server(WireOpts {
        hang: true,
        on_write: Some(Arc::new(move |m: &Value, w: &Wire| match method_of(m).as_str() {
            "thread/start" => w.push(&[thread_answer()]),
            "turn/start" => w.push(&[one.clone(), two.clone()]),
            "turn/steer" => w.push(&[format!(r#"{{"id":{},"result":{{}}}}"#, m["id"])]),
            _ => {}
        })),
        ..Default::default()
    }));
    let events = Events::default();
    let session = start(&adapter(&launch), &events);
    until(|| events.any("turn.anchor")).await;
    sleep_ms(5).await;
    assert_eq!(session.steer("more".into()).await, SteerOutcome::Accepted);
    let w = launch.wire(0);
    assert_eq!(w.written_method("turn/steer")[0]["params"]["expectedTurnId"], "tt2");
    w.push1(done("tt1"));
    sleep_ms(10).await;
    assert!(!events.any("turn.done"), "the first turn's end names a turn that is no longer this one");
    w.push1(done("tt2"));
    assert_eq!(session.finished().await.status, TurnStatus::Completed);
    let anchors: Vec<String> = events.all().into_iter().filter_map(|e| match e { AdapterEvent::TurnAnchor { anchor, .. } => Some(anchor), _ => None }).collect();
    assert_eq!(anchors, ["tt1"]);
}

#[tokio::test]
async fn an_answer_s_close_carries_the_caller_s_outcome_and_option_and_a_deny_s_reason_is_steered_after_the_decision() {
    // CT13; adapter.ts:1169-1183. Only PERMISSION_ALLOW writes accept; anything else declines, whatever outcome is named.
    let launch = scripted(vec![approval("13"), approval("14")], true);
    let events = Events::default();
    let session = start(&adapter(&launch), &events);
    until(|| events.of("permission.ask").len() == 2).await;
    let odd = PermissionAnswer { option_id: "whatever".into(), outcome: PermissionOutcome::Allowed, deny_message: String::new(), reason: None };
    assert_eq!(session.answer("13".into(), odd).await, Ok(AnswerOutcome::Answered));
    let deny = PermissionAnswer { option_id: "deny".into(), outcome: PermissionOutcome::Denied, deny_message: String::new(), reason: Some("use the other file".into()) };
    assert_eq!(session.answer("14".into(), deny).await, Ok(AnswerOutcome::Answered));
    sleep_ms(5).await;
    let closes: Vec<(String, PermissionOutcome, Option<String>)> = events
        .all()
        .into_iter()
        .filter_map(|e| match e {
            AdapterEvent::PermissionClose { ask_id, outcome, option_id, .. } => Some((ask_id, outcome, option_id)),
            _ => None,
        })
        .collect();
    assert_eq!(closes, [("13".into(), PermissionOutcome::Allowed, Some("whatever".into())), ("14".into(), PermissionOutcome::Denied, Some("deny".into()))]);
    let w = launch.wire(0);
    let written = w.written();
    assert_eq!(
        written[written.len() - 3..],
        [
            serde_json::json!({"id": 13, "result": {"decision": "decline"}}),
            serde_json::json!({"id": 14, "result": {"decision": "decline"}}),
            serde_json::json!({"id": "wsp-steer-1", "method": "turn/steer", "params": {"threadId": THREAD, "expectedTurnId": TURN, "input": [{"type": "text", "text": "use the other file"}]}}),
        ]
    );
    w.exit(Some(0));
    let _ = session.finished().await;
}
