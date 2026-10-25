// SPDX-License-Identifier: AGPL-3.0-only
//! The hard cases of unit claude-turn that no ported test pins (spec/claude_turn.md rules T7 to T12): the one request
//! counter, the order of the end when the process goes under open prompts, a call counted once per message, a turn
//! stopped before any result, and the registry key of each kind of start. Every expected value was printed by the TS
//! adapter at a3b870a6e over the same lines.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::sync::Arc;

use claude_exec::{collect, until, FakeExec, StreamOpts};
use serde_json::json;
use wsp_adapters::claude::{create_claude_adapter, AdapterDeps};
use wsp_adapters::port::*;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::UsageTokens;
use wsp_wire::views::session_events::TurnResultModelsItem;

const SID: &str = "e16ed170-8257-4668-879e-fe836341633c";

fn n(x: f64) -> JsNumber {
    JsNumber(x)
}

fn deps(exec: &Arc<FakeExec>) -> AdapterDeps {
    AdapterDeps { result_exit_ms: Some(5), interrupt_grace_ms: Some(5), stop_task_wait_ms: Some(5), steer_join_ms: Some(5), ..AdapterDeps::new(exec.clone(), "/root/.claude-cfg") }
}

fn init() -> String {
    json!({"type": "system", "subtype": "init", "session_id": SID, "model": "claude-x", "cwd": "/w", "tools": []}).to_string()
}

fn ask(id: &str, tool: &str) -> String {
    json!({"type": "control_request", "request_id": id, "request": {"subtype": "can_use_tool", "tool_name": tool, "input": {"command": "ls"}, "tool_use_id": format!("tu-{id}")}}).to_string()
}

fn bare(status: TurnStatus) -> TurnResult {
    TurnResult { status, duration_ms: None, waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: None, error: None, unreached: None, refusal: None, limit: None }
}

fn usage(input: u64, output: u64, cached: u64, cache_write: u64) -> UsageTokens {
    UsageTokens { input, output, cached, cache_write, reasoning: 0 }
}

// T7: set_access, stop_task and interrupt number their requests off one counter per turn; on a kept process that is
// up, a stop is the control line alone and nothing is signalled when the run ends within the grace.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_counter_numbers_every_request_and_a_kept_stop_signals_nothing() {
    tokio::task::LocalSet::new().run_until(one_counter_numbers_every_request_and_a_kept_stop_signals_nothing_body()).await
}

async fn one_counter_numbers_every_request_and_a_kept_stop_signals_nothing_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(AdapterDeps { interrupt_grace_ms: Some(2_000), ..deps(&exec) }).unwrap();
    let (events, on_event) = collect();
    let session = adapter.start(HarnessStartOptions { prompt: "one".into(), keep: true, on_event: Some(on_event), ..Default::default() }).unwrap();
    exec.push(init());
    until(|| !events.lock().unwrap().is_empty()).await;
    let set = tokio::task::spawn_local({
        let session = session.clone();
        async move { session.set_access("plan").await }
    });
    until(|| exec.writes().len() == 1).await;
    exec.push(json!({"type": "control_response", "response": {"subtype": "success", "request_id": "wsp-set-mode-1", "response": {}}}).to_string());
    assert_eq!(set.await.unwrap(), Ok(AccessOutcome::Set));
    let stop = tokio::task::spawn_local({
        let session = session.clone();
        async move { session.stop_task("bg-1").await }
    });
    until(|| exec.writes().len() == 2).await;
    exec.push(json!({"type": "control_response", "response": {"subtype": "error", "request_id": "wsp-stop-task-2", "error": "no such task"}}).to_string());
    assert_eq!(stop.await.unwrap(), Ok(TaskStop::Refused { error: "no such task".into() }));
    let interrupt = tokio::task::spawn_local({
        let session = session.clone();
        async move { session.interrupt().await }
    });
    until(|| exec.writes().len() == 3).await;
    assert_eq!(
        exec.writes().iter().map(|w| claude_exec::json(w)).collect::<Vec<_>>(),
        vec![
            json!({"type": "control_request", "request_id": "wsp-set-mode-1", "request": {"subtype": "set_permission_mode", "mode": "plan"}}),
            json!({"type": "control_request", "request_id": "wsp-stop-task-2", "request": {"subtype": "stop_task", "task_id": "bg-1"}}),
            json!({"type": "control_request", "request_id": "wsp-interrupt-3", "request": {"subtype": "interrupt"}}),
        ]
    );
    exec.end(Some(143));
    interrupt.await.unwrap();
    assert_eq!(exec.order(), vec!["write", "write", "write"], "the stop was the control line; no teardown, no kill");
    assert_eq!(session.finished().await, bare(TurnStatus::Interrupted));
}

// T8: a process that goes under open prompts closes each as cancelled in the order they were raised, then the turn's
// end, then session.end last; a request still waiting answers Gone; nothing can be answered or steered after.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_process_going_under_open_prompts_cancels_each_in_order_before_the_end() {
    tokio::task::LocalSet::new().run_until(the_process_going_under_open_prompts_cancels_each_in_order_before_the_end_body()).await
}

async fn the_process_going_under_open_prompts_cancels_each_in_order_before_the_end_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let (events, on_event) = collect();
    let session = adapter.start(HarnessStartOptions { prompt: "one".into(), permission_mode: Some("default".into()), on_event: Some(on_event), ..Default::default() }).unwrap();
    exec.push(init());
    exec.push(ask("r1", "Bash"));
    exec.push(ask("r2", "Write"));
    until(|| events.lock().unwrap().iter().filter(|e| matches!(e, AdapterEvent::PermissionAsk { .. })).count() == 2).await;
    let set = tokio::task::spawn_local({
        let session = session.clone();
        async move { session.set_access("acceptEdits").await }
    });
    until(|| exec.writes().len() == 1).await;
    exec.end(Some(1));
    assert_eq!(set.await.unwrap(), Ok(AccessOutcome::Gone), "its answer can never come");
    let failed = TurnResult { error: Some("claude exited with code 1 before emitting a result".into()), ..bare(TurnStatus::Failed) };
    assert_eq!(session.finished().await, failed);
    let after: Vec<AdapterEvent> = events.lock().unwrap().iter().filter(|e| !matches!(e, AdapterEvent::PermissionAsk { .. } | AdapterEvent::SessionStart { .. })).cloned().collect();
    assert_eq!(
        after,
        vec![
            AdapterEvent::PermissionClose { session_id: SID.into(), ask_id: "r1".into(), outcome: PermissionOutcome::Cancelled, option_id: None },
            AdapterEvent::PermissionClose { session_id: SID.into(), ask_id: "r2".into(), outcome: PermissionOutcome::Cancelled, option_id: None },
            AdapterEvent::TurnDone { session_id: SID.into(), result: failed, held: None },
            AdapterEvent::SessionEnd { session_id: SID.into(), exit_code: Some(1), saw_result: false, failure: None },
        ]
    );
    let allow = PermissionAnswer { option_id: PERMISSION_ALLOW.into(), outcome: PermissionOutcome::Allowed, deny_message: String::new(), reason: None };
    assert_eq!(session.answer("r1", allow.clone()).await, Ok(AnswerOutcome::Gone), "closed at the exit");
    assert_eq!(session.answer("zz", allow).await, Ok(AnswerOutcome::Gone), "never raised");
    assert_eq!(session.steer("x", None, vec![]).await, Ok(SteerOutcome::NotRunning));
    assert_eq!(exec.writes().len(), 1, "nothing written after the exit");
}

// T9: a call is drawn once per message id, its first line timing it where the timestamp parses, with `context` only
// for the lead's own; a run that ends with no result carries every printed call, the output the largest seen, split by
// model, the model that spent most named; the anchor is the lead's last line, never a subagent's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_call_is_drawn_once_per_message_and_a_run_with_no_result_carries_every_call() {
    tokio::task::LocalSet::new().run_until(a_call_is_drawn_once_per_message_and_a_run_with_no_result_carries_every_call_body()).await
}

async fn a_call_is_drawn_once_per_message_and_a_run_with_no_result_carries_every_call_body() {
    let lines = vec![
        init(),
        json!({"type": "assistant", "uuid": "u1", "session_id": SID, "timestamp": "2026-10-25T10:00:00.000Z", "message": {"id": "m1", "model": "claude-x", "content": [{"type": "thinking", "thinking": ""}], "usage": {"input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 7}}}).to_string(),
        json!({"type": "assistant", "uuid": "u2", "session_id": SID, "timestamp": "not a date", "message": {"id": "m1", "model": "claude-x", "content": [{"type": "text", "text": "a"}], "usage": {"input_tokens": 10, "output_tokens": 9, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 7}}}).to_string(),
        json!({"type": "assistant", "uuid": "u3", "session_id": SID, "parent_tool_use_id": "toolu_x", "message": {"id": "m2", "model": "claude-y", "content": [{"type": "text", "text": "b"}], "usage": {"input_tokens": 4, "output_tokens": 3}}}).to_string(),
    ];
    let exec = FakeExec::scripted(lines, StreamOpts { exit_code: Some(Some(1)), ..Default::default() });
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let (events, on_event) = collect();
    let result = adapter.start(HarnessStartOptions { prompt: "x".into(), on_event: Some(on_event), ..Default::default() }).unwrap().finished().await;
    let events = events.lock().unwrap().clone();
    let drawn: Vec<_> = events.iter().filter(|e| matches!(e, AdapterEvent::TurnUsage { .. })).cloned().collect();
    assert_eq!(
        drawn,
        vec![
            AdapterEvent::TurnUsage { session_id: SID.into(), tokens: 122.0, context: Some(122.0), window: None, at: Some(1_792_922_400_000.0) },
            AdapterEvent::TurnUsage { session_id: SID.into(), tokens: 7.0, context: None, window: None, at: None },
        ]
    );
    assert_eq!(
        result,
        TurnResult {
            error: Some("claude exited with code 1 before emitting a result".into()),
            tokens: Some(TurnTokens { input: n(121.0), output: n(12.0), cached: Some(n(100.0)), cache_write: Some(n(7.0)), reasoning: Some(n(0.0)), context: None, window: None }),
            models: Some(vec![
                TurnResultModelsItem { model: "claude-x".into(), tokens: usage(117, 9, 100, 7), cost_usd: None },
                TurnResultModelsItem { model: "claude-y".into(), tokens: usage(4, 3, 0, 0), cost_usd: None },
            ]),
            model: Some("claude-x".into()),
            ..bare(TurnStatus::Failed)
        }
    );
    let anchors: Vec<_> = events.iter().filter(|e| matches!(e, AdapterEvent::TurnAnchor { .. })).cloned().collect();
    assert_eq!(anchors, vec![AdapterEvent::TurnAnchor { session_id: SID.into(), anchor: "u2".into(), kept: None }]);
}

// T10: a stop before any result reads interrupted with the calls printed so far; a run that is not kept is torn down,
// and one that goes on the SIGTERM is never killed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stop_before_any_result_reads_interrupted_with_the_calls_so_far() {
    tokio::task::LocalSet::new().run_until(a_stop_before_any_result_reads_interrupted_with_the_calls_so_far_body()).await
}

async fn a_stop_before_any_result_reads_interrupted_with_the_calls_so_far_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let (events, on_event) = collect();
    let session = adapter.start(HarnessStartOptions { prompt: "x".into(), on_event: Some(on_event), ..Default::default() }).unwrap();
    exec.push(init());
    for id in ["m1", "m2"] {
        exec.push(json!({"type": "assistant", "uuid": format!("u-{id}"), "session_id": SID, "message": {"id": id, "model": "claude-x", "content": [{"type": "text", "text": "..."}], "usage": {"input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 7}}}).to_string());
    }
    until(|| events.lock().unwrap().iter().filter(|e| matches!(e, AdapterEvent::TurnUsage { .. })).count() == 2).await;
    session.interrupt().await;
    let result = session.finished().await;
    assert_eq!(
        result,
        TurnResult {
            tokens: Some(TurnTokens { input: n(234.0), output: n(10.0), cached: Some(n(200.0)), cache_write: Some(n(14.0)), reasoning: Some(n(0.0)), context: None, window: None }),
            models: Some(vec![TurnResultModelsItem { model: "claude-x".into(), tokens: usage(234, 10, 200, 14), cost_usd: None }]),
            model: Some("claude-x".into()),
            ..bare(TurnStatus::Interrupted)
        }
    );
    assert_eq!(exec.order(), vec!["teardown"]);
    assert_eq!(events.lock().unwrap().last(), Some(&AdapterEvent::SessionEnd { session_id: SID.into(), exit_code: Some(143), saw_result: false, failure: None }));
}

// T11: the registry key is the resume for a resume, a new id for a copy (which forks the session it names) and for a
// new session; only a resume's launch reads the saved spend first.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_start_is_registered_under_its_own_local_id() {
    tokio::task::LocalSet::new().run_until(each_start_is_registered_under_its_own_local_id_body()).await
}

async fn each_start_is_registered_under_its_own_local_id_body() {
    let exec = FakeExec::scripted(vec![init()], StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let start = |resume: Option<&str>, copy: bool| adapter.start(HarnessStartOptions { prompt: "x".into(), resume: resume.map(String::from), copy, on_event: Some(Arc::new(|_| {})), ..Default::default() }).unwrap();
    let fresh = start(None, false);
    let resumed = start(Some("22222222-2222-4222-8222-222222222222"), false);
    let copied = start(Some("33333333-3333-4333-8333-333333333333"), true);
    for s in [&fresh, &resumed, &copied] {
        s.finished().await;
    }
    let keys: Vec<String> = adapter.sessions().keys().cloned().collect();
    assert_eq!(keys, vec![fresh.local_id(), "22222222-2222-4222-8222-222222222222".to_string(), copied.local_id()]);
    assert_ne!(copied.local_id(), "33333333-3333-4333-8333-333333333333");
    let word = r#"cd ~ && sh -c '[ -z "${WSP_AGENT_PID_FILE:-}" ] || echo $$ > "$WSP_AGENT_PID_FILE"; unset WSP_AGENT_PID_FILE; exec "$0" "$@"' claude -p --input-format stream-json --output-format stream-json --verbose --dangerously-skip-permissions --permission-prompt-tool stdio"#;
    assert_eq!(copied.command(), Some(format!("{word} --session-id {} --resume 33333333-3333-4333-8333-333333333333 --fork-session", copied.local_id())));
    assert_eq!(
        resumed.command(),
        Some(format!(r#"wsp_saved=$(ls -1td '/root/.claude-cfg/projects'/*/'22222222-2222-4222-8222-222222222222.jsonl' 2>/dev/null | head -n 1); [ -z "$wsp_saved" ] || tail -c 8388608 "$wsp_saved" | grep -F '"type":"cost-state"' | tail -n 1; {word} --resume 22222222-2222-4222-8222-222222222222"#))
    );
}

// T12: the adapter never reads a transport failure's kind: whatever the kind, its words are the turn's error and the
// failure rides session.end as it came, so the runtime can tell a silent computer from the rest.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_transport_failure_kind_rides_session_end_as_it_came() {
    tokio::task::LocalSet::new().run_until(every_transport_failure_kind_rides_session_end_as_it_came_body()).await
}

async fn every_transport_failure_kind_rides_session_end_as_it_came_body() {
    for kind in [ExecFailureKind::Unreached, ExecFailureKind::Unreachable, ExecFailureKind::PlaceAbsent, ExecFailureKind::LaunchUnanswered, ExecFailureKind::Deadline, ExecFailureKind::NoAnswer, ExecFailureKind::Other] {
        let exec = FakeExec::manual(StreamOpts::default());
        let adapter = create_claude_adapter(deps(&exec)).unwrap();
        let (events, on_event) = collect();
        let session = adapter.start(HarnessStartOptions { prompt: "go".into(), on_event: Some(on_event), ..Default::default() }).unwrap();
        exec.push(init());
        until(|| !events.lock().unwrap().is_empty()).await;
        let failure = ExecFailure { kind, message: format!("cut: {kind:?}") };
        exec.current().fail(failure.clone());
        exec.current().exit.resolve(None);
        let result = session.finished().await;
        assert_eq!(result, TurnResult { error: Some(format!("cut: {kind:?}")), ..bare(TurnStatus::Failed) }, "{kind:?}");
        assert_eq!(events.lock().unwrap().last(), Some(&AdapterEvent::SessionEnd { session_id: SID.into(), exit_code: None, saw_result: false, failure: Some(failure) }), "{kind:?}");
    }
}

// A3: a launch line that refuses a value is the start's (and the warm's) Err, before anything is launched.
#[test]
fn a_refused_value_fails_the_start_before_any_launch() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let start = adapter.start(HarnessStartOptions { prompt: "x".into(), model: Some("bad model".into()), on_event: Some(Arc::new(|_| {})), ..Default::default() });
    assert_eq!(start.err().map(|e| e.0), Some(r#"model must be a plain slug, got "bad model""#.to_string()));
    let warm = adapter.warm(HarnessWarmOptions { model: Some("bad model".into()), context_window: Some("1m".into()), ..Default::default() });
    assert_eq!(warm.err().map(|e| e.0), Some(r#"model must be a plain slug, got "bad model[1m]""#.to_string()), "the slug is checked after the window's suffix");
    assert!(exec.calls().is_empty());
    assert!(adapter.sessions().is_empty());
}
