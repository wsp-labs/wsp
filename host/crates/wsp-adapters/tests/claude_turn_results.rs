// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/adapter-claude/test/adapter.test.ts lines 1199-2317: how a turn's result is classified, a reply
//! given while background work runs, the agent's tool shell, what a rewind needs, a retried model call, a
//! compaction, the plan's limits, a person's launch setup and what a turn cost.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::sync::{Arc, Mutex};

use futures::future::{LocalBoxFuture, FutureExt};
use regex::Regex;
use serde_json::{json, Map, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps, ClaudeAdapter};
use wsp_adapters::claude::aside::{aside_command, AsideCommandOptions};
use wsp_adapters::claude::chain::{chain_read_command, chain_write_command, copy_cleanup_command};
use wsp_adapters::claude::landmines::{user_message_line, BaseEnv};
use wsp_adapters::claude::turn::ClaudeSession;
use wsp_adapters::pending::{background_tasks_line, notify_tail, LOST_SESSION_NOTE};
use wsp_adapters::port::{agent_word, AdapterEvent, AgentLaunch, DeltaKind, DraftAsk, Env, ExecFailure, ExecFailureKind, ExecStreamFactory, ForkTurn, ForkedSession, HarnessExec, HarnessLimit, HarnessStartOptions, OnEvent, SteerOutcome, TitleTurn, TurnResult, TurnStatus, TurnTokens};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::{LimitKind, LimitStatus, LimitWindow, TurnLimit, UsageTokens};
use wsp_wire::types::workspace_state::CLOUD_SIGN_IN_ROAD;
use wsp_wire::views::session::TurnRefusal;
use wsp_wire::views::session_events::TurnResultModelsItem;

use claude_exec::{collect, fixture_lines, types, until, FakeExec, StreamOpts, FIXTURE_SESSION_ID};

const CFG: &str = "/root/.claude-cfg";
type Events = Arc<Mutex<Vec<AdapterEvent>>>;

// ---------------------------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------------------------

fn n(x: f64) -> JsNumber {
    JsNumber(x)
}

fn snap(events: &Events) -> Vec<AdapterEvent> {
    events.lock().unwrap().clone()
}

fn quiet() -> OnEvent {
    Arc::new(|_| {})
}

fn adapter_with(exec: Arc<dyn ExecStreamFactory>, tweak: impl FnOnce(&mut AdapterDeps)) -> ClaudeAdapter {
    let mut deps = AdapterDeps::new(exec, CFG);
    tweak(&mut deps);
    create_claude_adapter(deps).unwrap()
}

fn adapter_over(exec: &Arc<FakeExec>) -> ClaudeAdapter {
    adapter_with(exec.clone(), |_| {})
}

fn opts(prompt: &str, on_event: OnEvent) -> HarnessStartOptions {
    HarnessStartOptions { prompt: prompt.to_string(), on_event: Some(on_event), ..Default::default() }
}

fn opts_resume(prompt: &str, resume: &str, on_event: OnEvent) -> HarnessStartOptions {
    HarnessStartOptions { resume: Some(resume.to_string()), ..opts(prompt, on_event) }
}

fn script(lines: Vec<String>) -> Arc<FakeExec> {
    FakeExec::scripted(lines, StreamOpts::default())
}

fn script_exit(lines: Vec<String>, exit_code: Option<i32>) -> Arc<FakeExec> {
    FakeExec::scripted(lines, StreamOpts { exit_code: Some(exit_code), ..Default::default() })
}

fn strs(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|s| s.to_string()).collect()
}

fn turn(status: TurnStatus) -> TurnResult {
    TurnResult { status, duration_ms: None, waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: None, error: None, unreached: None, refusal: None, limit: None }
}

fn failed(error: &str) -> TurnResult {
    TurnResult { error: Some(error.to_string()), ..turn(TurnStatus::Failed) }
}

/// The init line most cases in the result block start with.
fn init_fixture() -> String {
    json!({"type": "system", "subtype": "init", "session_id": FIXTURE_SESSION_ID}).to_string()
}

/// Starts one turn of prompt "x" on `exec` and waits for its result.
async fn run_x(exec: &Arc<FakeExec>) -> (Vec<AdapterEvent>, TurnResult) {
    let adapter = adapter_over(exec);
    let (events, on_event) = collect();
    let result = adapter.start(opts("x", on_event)).unwrap().finished().await;
    (snap(&events), result)
}

/// TS `resultRun`: the init line, then one result line.
async fn result_run(result_line: &str) -> (Vec<AdapterEvent>, TurnResult) {
    run_x(&script(vec![init_fixture(), result_line.to_string()])).await
}

/// The `turn.done` events, each with its result and its `held` mark.
fn dones(events: &Events) -> Vec<(TurnResult, Option<bool>)> {
    snap(events).into_iter().filter_map(|e| if let AdapterEvent::TurnDone { result, held, .. } = e { Some((result, held)) } else { None }).collect()
}

/// The last event's exit code and sawResult; panics unless it is a session.end.
fn end_of(events: &[AdapterEvent]) -> (Option<i32>, bool) {
    match events.last() {
        Some(AdapterEvent::SessionEnd { exit_code, saw_result, .. }) => (*exit_code, *saw_result),
        other => panic!("expected session.end, got {other:?}"),
    }
}

fn text_deltas(events: &[AdapterEvent]) -> Vec<String> {
    events.iter().filter_map(|e| if let AdapterEvent::TurnDelta(d) = e { if d.kind == DeltaKind::Text { Some(d.text.clone()) } else { None } } else { None }).collect()
}

fn tokens_of(input: f64, output: f64) -> TurnTokens {
    TurnTokens { input: n(input), output: n(output), cached: None, cache_write: None, reasoning: None, context: None, window: None }
}

/// `toBeCloseTo(expected, digits)`.
fn close(actual: Option<JsNumber>, expected: f64, digits: i32) {
    let got = actual.expect("a number").0;
    assert!((got - expected).abs() < 10f64.powi(-digits) / 2.0, "{got} is not within 10^-{digits}/2 of {expected}");
}

fn sign_in_local() -> String {
    wsp_adapters::pending::sign_in_refusal_line(true)
}

fn sign_in_cloud() -> String {
    format!("{CLOUD_SIGN_IN_ROAD}, then send again")
}

// ---------------------------------------------------------------------------------------------------------------
// result classification
// ---------------------------------------------------------------------------------------------------------------

// TS: adapter.test.ts:1209
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn maps_aborted_streaming_to_interrupted() {
    tokio::task::LocalSet::new().run_until(maps_aborted_streaming_to_interrupted_body()).await
}

async fn maps_aborted_streaming_to_interrupted_body() {
    let line = json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "terminal_reason": "aborted_streaming", "errors": ["[ede_diagnostic] stream abort trace"], "duration_ms": 812, "session_id": FIXTURE_SESSION_ID}).to_string();
    let (_, result) = result_run(&line).await;
    assert_eq!(result.status, TurnStatus::Interrupted);
    assert_eq!(result.error, None);
}

// TS: adapter.test.ts:1217
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn maps_other_execution_errors_to_failed_and_hides_ede_diagnostic_noise() {
    tokio::task::LocalSet::new().run_until(maps_other_execution_errors_to_failed_and_hides_ede_diagnostic_noise_body()).await
}

async fn maps_other_execution_errors_to_failed_and_hides_ede_diagnostic_noise_body() {
    let line = json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "errors": ["[ede_diagnostic] internal", "MCP server exploded"], "duration_ms": 900, "session_id": FIXTURE_SESSION_ID}).to_string();
    let (_, result) = result_run(&line).await;
    assert_eq!(result.status, TurnStatus::Failed);
    assert_eq!(result.error.as_deref(), Some("MCP server exploded"));
}

// TS: adapter.test.ts:1225
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fails_the_turn_when_the_process_dies_without_a_result_event() {
    tokio::task::LocalSet::new().run_until(fails_the_turn_when_the_process_dies_without_a_result_event_body()).await
}

async fn fails_the_turn_when_the_process_dies_without_a_result_event_body() {
    let exec = script_exit(vec![init_fixture()], Some(1));
    let (events, result) = run_x(&exec).await;
    assert_eq!(result.status, TurnStatus::Failed);
    assert!(result.error.as_deref().unwrap().contains("exited with code 1"), "{:?}", result.error);
    assert_eq!(end_of(&events), (Some(1), false));
}

// TS: adapter.test.ts:1241
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_run_killed_by_a_signal_names_the_signal_never_a_bare_code() {
    tokio::task::LocalSet::new().run_until(a_run_killed_by_a_signal_names_the_signal_never_a_bare_code_body()).await
}

async fn a_run_killed_by_a_signal_names_the_signal_never_a_bare_code_body() {
    let exec = FakeExec::scripted(vec![init_fixture()], StreamOpts { exit_code: Some(None), signalled: Some("SIGKILL".to_string()), ..Default::default() });
    let (_, result) = run_x(&exec).await;
    assert_eq!(result, failed("claude was killed (SIGKILL) before it answered"));
}

// TS: adapter.test.ts:1252
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_launch_nothing_of_the_agent_came_back_from_says_so() {
    tokio::task::LocalSet::new().run_until(a_launch_nothing_of_the_agent_came_back_from_says_so_body()).await
}

async fn a_launch_nothing_of_the_agent_came_back_from_says_so_body() {
    let exec = script_exit(vec![], None);
    let (_, result) = run_x(&exec).await;
    assert_eq!(result, failed("the launch never reached claude: its run ended before the agent said a word"));
}

// TS: adapter.test.ts:1262
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_127_before_any_event_names_the_binary_and_the_path_searched() {
    tokio::task::LocalSet::new().run_until(exit_127_before_any_event_names_the_binary_and_the_path_searched_body()).await
}

async fn exit_127_before_any_event_names_the_binary_and_the_path_searched_body() {
    let exec = script_exit(strs(&["bash: line 2: claude: command not found"]), Some(127));
    let adapter = adapter_with(exec.clone(), |d| {
        let base: BaseEnv = [("PATH".to_string(), Some("/root/.local/bin:/usr/bin:/bin".to_string()))].into_iter().collect();
        d.base_env = Some(base);
    });
    let (events, on_event) = collect();
    let result = adapter.start(opts("x", on_event)).unwrap().finished().await;
    assert_eq!(result, failed("claude was not found on PATH (exit 127); PATH searched: /root/.local/bin:/usr/bin:/bin"));
    assert_eq!(end_of(&snap(&events)), (Some(127), false));
    // The PATH the line quotes is the one the launch exported.
    assert_eq!(exec.calls()[0].env.get("PATH").map(String::as_str), Some("/root/.local/bin:/usr/bin:/bin"));
}

// TS: adapter.test.ts:1275
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_127_from_a_launch_that_exported_no_path_says_so() {
    tokio::task::LocalSet::new().run_until(exit_127_from_a_launch_that_exported_no_path_says_so_body()).await
}

async fn exit_127_from_a_launch_that_exported_no_path_says_so_body() {
    let exec = script_exit(vec![], Some(127));
    let (_, result) = run_x(&exec).await;
    assert_eq!(result.error.as_deref(), Some("claude was not found on PATH (exit 127); the launch exported no PATH, the machine's own was searched"));
}

// TS: adapter.test.ts:1285
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn when_the_transport_ends_the_turn_itself_its_line_is_the_turns_error() {
    tokio::task::LocalSet::new().run_until(when_the_transport_ends_the_turn_itself_its_line_is_the_turns_error_body()).await
}

async fn when_the_transport_ends_the_turn_itself_its_line_is_the_turns_error_body() {
    let cut = "stopped after 15m 00s with no output for 10m";
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_over(&exec);
    let (events, on_event) = collect();
    let session = adapter.start(opts("x", on_event)).unwrap();
    exec.push(init_fixture());
    exec.current().exit.resolve(None);
    exec.current().fail(ExecFailure { kind: ExecFailureKind::Other, message: cut.to_string() });
    let result = session.finished().await;
    assert_eq!(result, failed(cut));
    assert_eq!(end_of(&snap(&events)), (None, false));
}

// TS: adapter.test.ts:1316
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_result_with_no_text_and_no_usage_fails_with_the_stderr_tail_after_the_result_included() {
    tokio::task::LocalSet::new().run_until(a_result_with_no_text_and_no_usage_fails_with_the_stderr_tail_after_the_result_included_body()).await
}

async fn a_result_with_no_text_and_no_usage_fails_with_the_stderr_tail_after_the_result_included_body() {
    let usage = json!({"input_tokens": 0, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0, "output_tokens": 0, "service_tier": "standard"});
    let empty = json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 26, "result": "", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 0, "usage": usage}).to_string();
    let exec = script_exit(vec![init_fixture(), "No conversation found with session ID: e16ed170".to_string(), empty, "".to_string(), "Error: transcript ended mid-turn".to_string()], Some(1));
    let adapter = adapter_over(&exec);
    let (events, on_event) = collect();
    let result = adapter.start(opts_resume("x", FIXTURE_SESSION_ID, on_event)).unwrap().finished().await;
    let expected = TurnResult {
        duration_ms: Some(n(26.0)),
        tokens: Some(TurnTokens { cached: Some(n(0.0)), cache_write: Some(n(0.0)), ..tokens_of(0.0, 0.0) }),
        text: Some(String::new()),
        error: Some("claude answered with no output and no usage after 26ms: No conversation found with session ID: e16ed170\nError: transcript ended mid-turn".to_string()),
        ..turn(TurnStatus::Failed)
    };
    assert_eq!(result, expected);
    let events = snap(&events);
    assert_eq!(types(&events), vec!["session.start", "turn.done", "session.end"]);
    match &events[1] {
        AdapterEvent::TurnDone { result: r, .. } => assert_eq!(r, &result),
        other => panic!("expected turn.done, got {other:?}"),
    }
    match &events[2] {
        AdapterEvent::SessionEnd { exit_code, saw_result, .. } => assert_eq!((*exit_code, *saw_result), (Some(1), true)),
        other => panic!("expected session.end, got {other:?}"),
    }
    assert_eq!(exec.order(), vec!["closeInput"]);
}

// TS: adapter.test.ts:1339
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_result_with_no_text_and_a_quiet_stderr_fails_with_the_duration_alone_and_the_tail_keeps_five_lines() {
    tokio::task::LocalSet::new().run_until(a_result_with_no_text_and_a_quiet_stderr_fails_with_the_duration_alone_and_the_tail_keeps_five_lines_body()).await
}

async fn a_result_with_no_text_and_a_quiet_stderr_fails_with_the_duration_alone_and_the_tail_keeps_five_lines_body() {
    let empty = json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 1500, "result": "", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 0, "usage": {"input_tokens": 0, "output_tokens": 0}}).to_string();
    let (_, quiet_run) = result_run(&empty).await;
    assert_eq!(quiet_run.status, TurnStatus::Failed);
    assert_eq!(quiet_run.error.as_deref(), Some("claude answered with no output and no usage after 1.5s"));

    let mut lines = vec![init_fixture()];
    lines.extend(["one", "two", "three", "four", "five", "six"].iter().map(|w| format!("line {w}")));
    lines.push(empty);
    let (_, result) = run_x(&script(lines)).await;
    assert_eq!(result.error.as_deref(), Some("claude answered with no output and no usage after 1.5s: line two\nline three\nline four\nline five\nline six"));
}

// TS: adapter.test.ts:1353
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_resume_after_a_run_that_died_with_background_work_answers_the_persons_message() {
    tokio::task::LocalSet::new().run_until(a_resume_after_a_run_that_died_with_background_work_answers_the_persons_message_body()).await
}

async fn a_resume_after_a_run_that_died_with_background_work_answers_the_persons_message_body() {
    let init = json!({"type": "system", "subtype": "init", "cwd": "/w", "session_id": FIXTURE_SESSION_ID, "model": "claude-haiku-4-5"}).to_string();
    let lines = vec![
        json!({"type": "system", "subtype": "task_notification", "task_id": "b7oi2218c", "tool_use_id": "toolu_1", "status": "stopped", "output_file": "", "summary": "Background shell command didn't finish before the previous session ended", "session_id": FIXTURE_SESSION_ID}).to_string(),
        init.clone(),
        json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 0, "duration_ms": 22, "result": "", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 0.02, "origin": {"kind": "task-notification"}, "usage": {"input_tokens": 0, "output_tokens": 0}}).to_string(),
        init,
        json!({"type": "assistant", "message": {"id": "msg_1", "role": "assistant", "content": [{"type": "text", "text": "hi"}]}, "parent_tool_use_id": null, "session_id": FIXTURE_SESSION_ID}).to_string(),
        json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "duration_ms": 1237, "result": "hi", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 0.024, "usage": {"input_tokens": 10, "output_tokens": 42}}).to_string(),
    ];
    let exec = script(lines);
    let adapter = adapter_over(&exec);
    let (events, on_event) = collect();
    let result = adapter.start(opts_resume("Reply with the single word: hi", FIXTURE_SESSION_ID, on_event)).unwrap().finished().await;
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.text.as_deref(), Some("hi"));
    assert_eq!(result.duration_ms, Some(n(1237.0)));
    let events = snap(&events);
    assert_eq!(events.iter().filter(|e| matches!(e, AdapterEvent::TurnDone { .. })).count(), 1);
    assert_eq!(end_of(&events).1, true);
}

/// The two launches of a resume of a session the machine no longer holds: the first answers the gone session, every
/// later one the fresh session.
struct TwoRuns {
    runs: [Arc<FakeExec>; 2],
    next: Mutex<usize>,
}

impl ExecStreamFactory for TwoRuns {
    fn launch(&self, command: &str, options: wsp_adapters::port::ExecOptions) -> Arc<dyn wsp_adapters::port::ExecStream> {
        let mut next = self.next.lock().unwrap();
        let run = &self.runs[(*next).min(1)];
        *next += 1;
        run.launch(command, options)
    }
}

const FRESH: &str = "0b7f5d0e-3c1a-4e2b-9f6d-2a8c4e6b1d3f";

fn two_runs() -> Arc<TwoRuns> {
    let gone = vec![
        format!("No conversation found with session ID: {FIXTURE_SESSION_ID}"),
        json!({"type": "result", "subtype": "error_during_execution", "duration_ms": 0, "is_error": true, "num_turns": 0, "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 0, "usage": {"input_tokens": 0, "output_tokens": 0}, "errors": [format!("No conversation found with session ID: {FIXTURE_SESSION_ID}")]}).to_string(),
    ];
    let fresh = vec![
        json!({"type": "system", "subtype": "init", "cwd": "/w", "session_id": FRESH, "model": "claude-haiku-4-5"}).to_string(),
        json!({"type": "assistant", "message": {"id": "msg_1", "role": "assistant", "content": [{"type": "text", "text": "picking up"}]}, "parent_tool_use_id": null, "session_id": FRESH}).to_string(),
        json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "duration_ms": 900, "result": "picking up", "session_id": FRESH, "total_cost_usd": 0.01, "usage": {"input_tokens": 10, "output_tokens": 4}}).to_string(),
    ];
    Arc::new(TwoRuns { runs: [script_exit(gone, Some(1)), script(fresh)], next: Mutex::new(0) })
}

// TS: adapter.test.ts:1399
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lost_session_resume_starts_a_new_session_seeded_with_the_thread_so_far() {
    tokio::task::LocalSet::new().run_until(a_lost_session_resume_starts_a_new_session_seeded_with_the_thread_so_far_body()).await
}

async fn a_lost_session_resume_starts_a_new_session_seeded_with_the_thread_so_far_body() {
    let exec = two_runs();
    let adapter = adapter_with(exec.clone(), |_| {});
    let (events, on_event) = collect();
    let seed = "person: fix the login page\nagent: done, pushed to fix/login";
    let session = adapter
        .start(HarnessStartOptions { seed: Some(Arc::new(move || async move { Ok(seed.to_string()) }.boxed_local())), ..opts_resume("now add a test", FIXTURE_SESSION_ID, on_event) })
        .unwrap();
    let result = session.finished().await;
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.text.as_deref(), Some("picking up"));
    let fresh = exec.runs[1].calls()[0].clone();
    assert!(!fresh.command.contains("--resume"), "{}", fresh.command);
    let sent: Value = serde_json::from_str(&fresh.input.as_ref().unwrap()[0]).unwrap();
    let text = sent["message"]["content"].as_array().unwrap().last().unwrap()["text"].as_str().unwrap().to_string();
    assert!(text.contains(seed), "{text}");
    assert!(text.ends_with("now add a test"), "{text}");
    // The failed resume is no turn of the thread's: one turn ends, the fresh session's, and it announced itself.
    let events = snap(&events);
    assert_eq!(events.iter().filter(|e| matches!(e, AdapterEvent::TurnDone { .. })).count(), 1);
    let started = events.iter().position(|e| matches!(e, AdapterEvent::SessionStart { .. })).expect("a session.start");
    match &events[started] {
        AdapterEvent::SessionStart { session_id, .. } => assert_eq!(session_id, &session.claude_session_id()),
        _ => unreachable!(),
    }
    assert_ne!(session.claude_session_id(), FIXTURE_SESSION_ID);
    let note = events[started..].iter().find_map(|e| if let AdapterEvent::TurnDelta(d) = e { if d.kind == DeltaKind::Note { Some(d.text.clone()) } else { None } } else { None });
    assert_eq!(note.as_deref(), Some(LOST_SESSION_NOTE));
}

// TS: adapter.test.ts:1423
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lost_session_resume_fails_with_the_clis_own_sentence_where_nothing_seeds_a_new_one() {
    tokio::task::LocalSet::new().run_until(a_lost_session_resume_fails_with_the_clis_own_sentence_where_nothing_seeds_a_new_one_body()).await
}

async fn a_lost_session_resume_fails_with_the_clis_own_sentence_where_nothing_seeds_a_new_one_body() {
    let exec = two_runs();
    let adapter = adapter_with(exec.clone(), |_| {});
    let result = adapter.start(opts_resume("x", FIXTURE_SESSION_ID, quiet())).unwrap().finished().await;
    assert_eq!(result.status, TurnStatus::Failed);
    assert_eq!(result.error, Some(format!("No conversation found with session ID: {FIXTURE_SESSION_ID}")));
    assert!(exec.runs[1].calls().is_empty());
}

// TS: adapter.test.ts:1433
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reports_answer_that_carries_words_is_the_turns_reply() {
    tokio::task::LocalSet::new().run_until(a_reports_answer_that_carries_words_is_the_turns_reply_body()).await
}

async fn a_reports_answer_that_carries_words_is_the_turns_reply_body() {
    let init = json!({"type": "system", "subtype": "init", "cwd": "/w", "session_id": FIXTURE_SESSION_ID, "model": "claude-haiku-4-5"}).to_string();
    let exec = script(vec![
        init,
        json!({"type": "assistant", "message": {"id": "msg_1", "role": "assistant", "content": [{"type": "text", "text": "hi"}]}, "parent_tool_use_id": null, "session_id": FIXTURE_SESSION_ID}).to_string(),
        json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "duration_ms": 900, "result": "hi", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 0.01, "origin": {"kind": "task-notification"}, "usage": {"input_tokens": 10, "output_tokens": 4}}).to_string(),
    ]);
    let adapter = adapter_over(&exec);
    let result = adapter.start(opts_resume("x", FIXTURE_SESSION_ID, quiet())).unwrap().finished().await;
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.text.as_deref(), Some("hi"));
    assert!(exec.order().contains(&"closeInput".to_string()), "{:?}", exec.order());
}

// TS: adapter.test.ts:1449
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_clis_refusal_for_want_of_a_sign_in_is_a_failed_turn_carrying_its_sentence_once() {
    tokio::task::LocalSet::new().run_until(the_clis_refusal_for_want_of_a_sign_in_is_a_failed_turn_carrying_its_sentence_once_body()).await
}

async fn the_clis_refusal_for_want_of_a_sign_in_is_a_failed_turn_carrying_its_sentence_once_body() {
    let init = json!({"type": "system", "subtype": "init", "session_id": FIXTURE_SESSION_ID, "model": "claude-opus-5[1m]"}).to_string();
    let refusal = json!({"type": "assistant", "message": {"id": "m1", "model": "<synthetic>", "role": "assistant", "type": "message", "content": [{"type": "text", "text": "Not logged in \u{b7} Please run /login"}]}, "session_id": FIXTURE_SESSION_ID, "error": "authentication_failed", "is_api_error_message": true}).to_string();
    let result_line = json!({"type": "result", "subtype": "success", "is_error": true, "terminal_reason": "api_error", "num_turns": 1, "result": "Not logged in \u{b7} Please run /login", "duration_ms": 88, "total_cost_usd": 0, "usage": {"input_tokens": 0, "output_tokens": 0}, "session_id": FIXTURE_SESSION_ID}).to_string();
    let lines = vec![init, refusal, result_line];

    let exec = script_exit(lines.clone(), Some(1));
    let adapter = adapter_with(exec.clone(), |d| d.sign_in_refusal = Some(sign_in_local()));
    let (events, on_event) = collect();
    let done = adapter.start(opts("say hi", on_event)).unwrap().finished().await;
    let expected = TurnResult {
        duration_ms: Some(n(88.0)),
        cost_usd: Some(n(0.0)),
        tokens: Some(tokens_of(0.0, 0.0)),
        model: Some("claude-opus-5[1m]".to_string()),
        error: Some("Not logged in \u{b7} Please run /login; sign in from a terminal on this computer, then send again".to_string()),
        refusal: Some(TurnRefusal::SignIn),
        ..turn(TurnStatus::Failed)
    };
    assert_eq!(done, expected);
    assert_eq!(done.text, None);
    let events = snap(&events);
    assert_eq!(types(&events), vec!["session.start", "turn.done", "session.end"]);
    match &events[1] {
        AdapterEvent::TurnDone { result, .. } => assert_eq!(result, &done),
        other => panic!("expected turn.done, got {other:?}"),
    }

    // The road is the caller's: the same refusal on a machine sends the person to the Machine tab.
    let on_machine = script_exit(lines.clone(), Some(1));
    let there = adapter_with(on_machine.clone(), |d| d.sign_in_refusal = Some(sign_in_cloud()));
    let away = there.start(opts("say hi", quiet())).unwrap().finished().await;
    assert_eq!(away.status, TurnStatus::Failed);
    assert_eq!(away.refusal, Some(TurnRefusal::SignIn));
    assert_eq!(away.error.as_deref(), Some("Not logged in \u{b7} Please run /login; sign this workspace in from the Workspace panel, then send again"));

    // A caller that handed no road leaves the CLI's own sentence to stand alone; the cause still classes the turn.
    let bare = script_exit(lines, Some(1));
    let alone = adapter_over(&bare).start(opts("say hi", quiet())).unwrap().finished().await;
    assert_eq!(alone.status, TurnStatus::Failed);
    assert_eq!(alone.refusal, Some(TurnRefusal::SignIn));
    assert_eq!(alone.error.as_deref(), Some("Not logged in \u{b7} Please run /login"));
}

// TS: adapter.test.ts:1486
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_user_abort_is_read_before_the_error_flag() {
    tokio::task::LocalSet::new().run_until(a_user_abort_is_read_before_the_error_flag_body()).await
}

async fn a_user_abort_is_read_before_the_error_flag_body() {
    let aborted_success = json!({"type": "result", "subtype": "success", "is_error": true, "terminal_reason": "aborted_streaming", "duration_ms": 812, "result": "stopped", "session_id": FIXTURE_SESSION_ID}).to_string();
    let exec = script(vec![init_fixture(), aborted_success]);
    let adapter = adapter_with(exec.clone(), |d| d.sign_in_refusal = Some(sign_in_local()));
    let done = adapter.start(opts("x", quiet())).unwrap().finished().await;
    assert_eq!(done.status, TurnStatus::Interrupted);
    assert_eq!(done.text.as_deref(), Some("stopped"));
    assert_eq!(done.error, None);
    assert_eq!(done.refusal, None);
}

// TS: adapter.test.ts:1499
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refusal_with_no_known_cause_keeps_its_own_sentence_and_a_turn_that_did_work_reads_completed() {
    tokio::task::LocalSet::new().run_until(a_refusal_with_no_known_cause_keeps_its_own_sentence_and_a_turn_that_did_work_reads_completed_body()).await
}

async fn a_refusal_with_no_known_cause_keeps_its_own_sentence_and_a_turn_that_did_work_reads_completed_body() {
    let overloaded = json!({"type": "assistant", "message": {"id": "m1", "model": "<synthetic>", "role": "assistant", "type": "message", "content": [{"type": "text", "text": "API Error: 529 overloaded"}]}, "session_id": FIXTURE_SESSION_ID, "error": "overloaded", "is_api_error_message": true}).to_string();
    let errored = json!({"type": "result", "subtype": "success", "is_error": true, "result": "API Error: 529 overloaded", "duration_ms": 40, "session_id": FIXTURE_SESSION_ID}).to_string();
    let exec = script_exit(vec![init_fixture(), overloaded, errored], Some(1));
    let adapter = adapter_with(exec.clone(), |d| d.sign_in_refusal = Some(sign_in_local()));
    let refused = adapter.start(opts("x", quiet())).unwrap().finished().await;
    assert_eq!(refused.status, TurnStatus::Failed);
    assert_eq!(refused.error.as_deref(), Some("API Error: 529 overloaded"));
    assert_eq!(refused.refusal, None);

    let worked = script(fixture_lines("stream-session.jsonl"));
    let second = adapter_over(&worked);
    let (events, on_event) = collect();
    let done = second.start(opts("x", on_event)).unwrap().finished().await;
    assert_eq!(done.status, TurnStatus::Completed);
    assert_eq!(done.text.as_deref(), Some("Server is live at :3000 and answered: Hello, World!"));
    assert_eq!(done.cost_usd, Some(n(0.0187)));
    assert_eq!(done.error, None);
    assert!(!delta_events(&snap(&events)).is_empty());
}

fn delta_events(events: &[AdapterEvent]) -> Vec<&AdapterEvent> {
    events.iter().filter(|e| matches!(e, AdapterEvent::TurnDelta(_))).collect()
}

// TS: adapter.test.ts:1518
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_completed_result_with_text_or_with_usage_keeps_its_status() {
    tokio::task::LocalSet::new().run_until(a_completed_result_with_text_or_with_usage_keeps_its_status_body()).await
}

async fn a_completed_result_with_text_or_with_usage_keeps_its_status_body() {
    let with_text = result_run(&json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 30, "result": "ok", "session_id": FIXTURE_SESSION_ID, "usage": {"input_tokens": 0, "output_tokens": 0}}).to_string()).await;
    assert_eq!(with_text.1.status, TurnStatus::Completed);
    assert_eq!(with_text.1.text.as_deref(), Some("ok"));
    let with_usage = result_run(&json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 30, "result": "", "session_id": FIXTURE_SESSION_ID, "usage": {"input_tokens": 12, "output_tokens": 0}}).to_string()).await;
    assert_eq!(with_usage.1.status, TurnStatus::Completed);
    assert_eq!(with_usage.1.text.as_deref(), Some(""));
    assert_eq!(with_usage.1.error, None);
}

// TS: adapter.test.ts:1526
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_background_task_that_ended_before_the_result_leaves_the_turn_completed() {
    tokio::task::LocalSet::new().run_until(a_background_task_that_ended_before_the_result_leaves_the_turn_completed_body()).await
}

async fn a_background_task_that_ended_before_the_result_leaves_the_turn_completed_body() {
    let init = init_fixture();
    let one = json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [{"task_id": "a", "task_type": "local_bash", "description": "sleep 5"}], "session_id": FIXTURE_SESSION_ID}).to_string();
    let none = json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [], "session_id": FIXTURE_SESSION_ID}).to_string();
    let success = json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 30, "result": "done", "session_id": FIXTURE_SESSION_ID, "usage": {"input_tokens": 1, "output_tokens": 1}}).to_string();
    let aborted = json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "terminal_reason": "aborted_streaming", "duration_ms": 812, "session_id": FIXTURE_SESSION_ID}).to_string();
    let run = |lines: Vec<String>| async move { adapter_over(&script(lines)).start(opts("x", quiet())).unwrap().finished().await };

    let finished_first = run(vec![init.clone(), one.clone(), none, success]).await;
    assert_eq!(finished_first.status, TurnStatus::Completed);
    assert_eq!(finished_first.text.as_deref(), Some("done"));
    assert_eq!(finished_first.error, None);
    let interrupted = run(vec![init, one, aborted]).await;
    assert_eq!(interrupted.status, TurnStatus::Interrupted);
    assert_eq!(interrupted.error, None);
}

// TS: adapter.test.ts:1546
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skips_lines_that_are_not_stream_json_events() {
    tokio::task::LocalSet::new().run_until(skips_lines_that_are_not_stream_json_events_body()).await
}

async fn skips_lines_that_are_not_stream_json_events_body() {
    let lines = vec![
        "not json at all".to_string(),
        "42".to_string(),
        "{\"type\":123}".to_string(),
        json!({"type": "result", "subtype": "success", "is_error": false, "result": "ok", "duration_ms": 5, "session_id": FIXTURE_SESSION_ID}).to_string(),
    ];
    let (events, result) = run_x(&script(lines)).await;
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(types(&events), vec!["turn.done", "session.end"]);
}

// ---------------------------------------------------------------------------------------------------------------
// a reply given while the agent's background work runs
// ---------------------------------------------------------------------------------------------------------------

const SID: &str = "8ba924b7-5641-4448-a153-9b11d4675297";
const TASK: &str = "bv4027ti3";
const CALL: &str = "toolu_01WspFixCall1";
const SLEEP_DESC: &str = "Sleep 20 seconds then echo done";
const WAITING: &str = "Waiting for the background sleep to finish.";
const WOKE_TEXT: &str = "The background sleep finished with exit code 0 and printed `done`.";

/// Every line of the background block: the ones the CLI printed on a Mac, and the two-task ones.
struct Bg {
    init: String,
    running: String,
    started_task: String,
    handed_back: String,
    reply: String,
    none: String,
    updated: String,
    notified: String,
    woke: String,
    second: String,
    two: String,
    only_b: String,
    notified_b: String,
}

fn bg() -> Bg {
    Bg {
        init: json!({"type": "system", "subtype": "init", "session_id": SID}).to_string(),
        running: json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [{"task_id": TASK, "task_type": "local_bash", "description": SLEEP_DESC}], "uuid": "5007fb59-c70a-4259-9056-e6427fbbf60c", "session_id": SID}).to_string(),
        started_task: json!({"type": "system", "subtype": "task_started", "task_id": TASK, "tool_use_id": CALL, "description": SLEEP_DESC, "is_backgrounded": true, "task_type": "local_bash", "session_id": SID}).to_string(),
        handed_back: json!({"type": "user", "message": {"role": "user", "content": [{"tool_use_id": CALL, "type": "tool_result", "content": format!("Command running in background with ID: {TASK}. Output is being written to: /private/tmp/claude-501/tasks/{TASK}.output. You will be notified when it completes."), "is_error": false}]}, "parent_tool_use_id": null, "session_id": SID}).to_string(),
        reply: json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 10657, "total_cost_usd": 0.26918, "result": WAITING, "session_id": SID, "usage": {"input_tokens": 58, "output_tokens": 188}}).to_string(),
        none: json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [], "uuid": "e6b61619-0394-459a-ad13-d748543abaf0", "session_id": SID}).to_string(),
        updated: json!({"type": "system", "subtype": "task_updated", "task_id": TASK, "patch": {"status": "completed", "end_time": 1789828771480u64}, "session_id": SID}).to_string(),
        notified: json!({"type": "system", "subtype": "task_notification", "task_id": TASK, "tool_use_id": CALL, "status": "completed", "output_file": format!("/private/tmp/claude-501/tasks/{TASK}.output"), "summary": format!("Background command \"{SLEEP_DESC}\" completed (exit code 0)"), "session_id": SID}).to_string(),
        woke: json!({"type": "assistant", "message": {"model": "claude-fable-5-1", "role": "assistant", "content": [{"type": "text", "text": WOKE_TEXT}]}, "parent_tool_use_id": null, "session_id": SID}).to_string(),
        second: json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 3600, "total_cost_usd": 0.28360375, "result": WOKE_TEXT, "origin": {"kind": "task-notification"}, "session_id": SID, "usage": {"input_tokens": 4, "output_tokens": 23}}).to_string(),
        two: json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [{"task_id": TASK, "task_type": "local_bash", "description": SLEEP_DESC}, {"task_id": "b", "task_type": "local_agent", "description": "review"}], "session_id": SID}).to_string(),
        only_b: json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [{"task_id": "b", "task_type": "local_agent", "description": "review"}], "session_id": SID}).to_string(),
        notified_b: json!({"type": "system", "subtype": "task_notification", "task_id": "b", "tool_use_id": "toolu_02", "status": "completed", "summary": "review completed", "session_id": SID}).to_string(),
    }
}

/// Long enough for every line pushed so far to have been read, and for a turn that ended at its reply to have closed
/// its channel and ended its process.
async fn drained() {
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
}

/// A turn whose stream the case feeds, on the exit window every case here runs with. `grace_ms` is how long the
/// process gets after the teardown.
fn held(slow_teardown: bool, grace_ms: Option<u64>) -> (Arc<FakeExec>, Events, ClaudeSession) {
    let m = FakeExec::manual(StreamOpts { slow_teardown, ..Default::default() });
    let adapter = adapter_with(m.clone(), |d| {
        d.result_exit_ms = Some(40);
        d.interrupt_grace_ms = Some(grace_ms.unwrap_or(15));
    });
    let (events, on_event) = collect();
    let session = adapter.start(opts("run it in the background and say you are waiting", on_event)).unwrap();
    (m, events, session)
}

fn push_all(m: &Arc<FakeExec>, lines: &[&String]) {
    for line in lines {
        m.push((*line).clone());
    }
}

/// The lines of a held reply's text after the reply and its blank line.
fn under_reply(result: &TurnResult) -> Vec<String> {
    result.text.as_deref().expect("a text").split('\n').skip(2).map(String::from).collect()
}

fn finished_after(r: &Regex, line: &str) {
    assert!(r.is_match(line), "{line:?} does not match {r}");
}

fn completed_after_reply(task: &str) -> Regex {
    Regex::new(&format!(r"^`{task}` completed, \d+m?s after the reply$")).unwrap()
}

fn task_counts(events: &Events) -> Vec<(u32, Option<String>)> {
    snap(events).into_iter().filter_map(|e| if let AdapterEvent::TurnTasks { running, replied, .. } = e { Some((running, replied.and_then(|r| r.text))) } else { None }).collect()
}

// TS: adapter.test.ts:1603
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_reply_closes_ends_and_delivers_nothing_until_the_task_is_done() {
    tokio::task::LocalSet::new().run_until(held_reply_closes_ends_and_delivers_nothing_until_the_task_is_done_body()).await
}

async fn held_reply_closes_ends_and_delivers_nothing_until_the_task_is_done_body() {
    let b = bg();
    let (m, events, session) = held(true, None);
    push_all(&m, &[&b.init, &b.running, &b.started_task, &b.handed_back, &b.reply]);
    // Past the window a finished turn's process gets: the turn is working, so nothing was delivered and the channel
    // the agent takes a message on is still open.
    drained().await;
    assert_eq!(dones(&events), vec![]);
    assert_eq!(m.order(), Vec::<String>::new());
    // The channel is open, so the turn takes a message.
    assert_eq!(session.steer("still there?", None, vec![]).await.unwrap(), SteerOutcome::Accepted);

    // The task ends and the CLI does not wake its agent: the words it already said are the turn's.
    push_all(&m, &[&b.none, &b.updated, &b.notified]);
    until(|| dones(&events).len() == 1).await;
    let result = dones(&events)[0].0.clone();
    assert_eq!(result.status, TurnStatus::Completed);
    let shape = Regex::new(&format!(r"^Waiting for the background sleep to finish\.\n\n`{SLEEP_DESC}` completed, \d+m?s after the reply$")).unwrap();
    finished_after(&shape, result.text.as_deref().unwrap());
    assert_eq!(result.duration_ms, Some(n(10657.0)));
    assert_eq!(result.cost_usd, Some(n(0.26918)));
    assert_eq!(session.finished().await, result);
    assert_eq!(m.order(), vec!["write", "closeInput", "teardown", "kill"]);
}

// TS: adapter.test.ts:1626
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ends_when_the_task_wakes_the_agent_and_the_agents_next_reply_is_the_turns() {
    tokio::task::LocalSet::new().run_until(ends_when_the_task_wakes_the_agent_and_the_agents_next_reply_is_the_turns_body()).await
}

async fn ends_when_the_task_wakes_the_agent_and_the_agents_next_reply_is_the_turns_body() {
    let b = bg();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &b.running, &b.started_task, &b.reply, &b.none, &b.updated, &b.notified, &b.woke, &b.second]);
    until(|| dones(&events).len() == 1).await;
    m.end(Some(0));
    let result = session.finished().await;
    // Timed from the launch: the held reply's figure, then the woken reply's own, since these lines land at once.
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.text.as_deref(), Some(WOKE_TEXT));
    assert_eq!(result.duration_ms, Some(n(10657.0 + 3600.0)));
    assert_eq!(result.cost_usd, Some(n(0.28360375)));
    // One reply, and the woken agent's words reached the pane before it.
    assert_eq!(dones(&events).len(), 1);
    assert_eq!(text_deltas(&snap(&events)), vec![WOKE_TEXT.to_string()]);
    assert_eq!(end_of(&snap(&events)).1, true);
}

// TS: adapter.test.ts:1641
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delivered_once_a_late_wake_is_not_a_second_reply() {
    tokio::task::LocalSet::new().run_until(delivered_once_a_late_wake_is_not_a_second_reply_body()).await
}

async fn delivered_once_a_late_wake_is_not_a_second_reply_body() {
    let b = bg();
    let (m, events, session) = held(true, Some(5_000));
    push_all(&m, &[&b.init, &b.running, &b.started_task, &b.reply, &b.none, &b.updated, &b.notified]);
    // Nothing woke the agent inside the window, so the words it gave are the turn's and the reply is out.
    until(|| dones(&events).len() == 1).await;
    let delivered = dones(&events)[0].0.clone();

    // The CLI wakes its agent anyway, later. Its words reach the pane; the turn's reply does not change.
    m.push(b.woke.clone());
    m.push(b.second.clone());
    until(|| text_deltas(&snap(&events)).iter().any(|t| t.starts_with("The background sleep finished"))).await;
    drained().await;
    assert_eq!(dones(&events).len(), 1);
    assert_eq!(dones(&events)[0].0, delivered);
    assert_eq!(delivered.cost_usd, Some(n(0.26918)));

    m.end(Some(0));
    assert_eq!(session.finished().await, delivered);
    assert_eq!(snap(&events).iter().filter(|e| matches!(e, AdapterEvent::SessionEnd { .. })).count(), 1);
}

// TS: adapter.test.ts:1662
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_send_during_the_hold_steers_the_agent_and_its_next_reply_ends_the_hold() {
    tokio::task::LocalSet::new().run_until(a_send_during_the_hold_steers_the_agent_and_its_next_reply_ends_the_hold_body()).await
}

async fn a_send_during_the_hold_steers_the_agent_and_its_next_reply_ends_the_hold_body() {
    let b = bg();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &b.running, &b.started_task, &b.reply]);
    drained().await;
    assert_eq!(dones(&events), vec![]);
    assert_eq!(session.steer("also say ok", None, vec![]).await.unwrap(), SteerOutcome::Accepted);
    let uuid = claude_exec::json(&m.writes()[0])["uuid"].as_str().unwrap().to_string();
    assert!(m.writes().contains(&user_message_line("also say ok", SID, &[], Some(&uuid))), "{:?}", m.writes());
    assert_eq!(dones(&events), vec![]);

    m.push(b.none.clone());
    m.push(json!({"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "ok"}]}, "session_id": SID}).to_string());
    m.push(json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 900, "result": "ok", "session_id": SID, "usage": {"input_tokens": 2, "output_tokens": 2}}).to_string());
    until(|| dones(&events).len() == 1).await;
    let result = dones(&events)[0].0.clone();
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.text.as_deref(), Some("ok"));
}

// TS: adapter.test.ts:1678
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hands_the_runtime_the_reply_once_over_its_tasks_and_marks_the_turns_end_as_that_reply() {
    tokio::task::LocalSet::new().run_until(hands_the_runtime_the_reply_once_over_its_tasks_and_marks_the_turns_end_as_that_reply_body()).await
}

async fn hands_the_runtime_the_reply_once_over_its_tasks_and_marks_the_turns_end_as_that_reply_body() {
    let b = bg();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &b.running, &b.started_task, &b.reply, &b.two, &b.only_b]);
    until(|| task_counts(&events).len() == 4).await;
    assert_eq!(task_counts(&events), vec![(1, None), (1, Some(WAITING.to_string())), (2, None), (1, None)]);
    push_all(&m, &[&b.none, &b.updated, &b.notified]);
    until(|| dones(&events).len() == 1).await;
    let (result, held_mark) = dones(&events)[0].clone();
    assert_eq!(held_mark, Some(true));
    assert_eq!(result.status, TurnStatus::Completed);
    session.finished().await;
}

// TS: adapter.test.ts:1695
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_woken_agents_own_reply_is_a_new_one_not_marked_held() {
    tokio::task::LocalSet::new().run_until(a_woken_agents_own_reply_is_a_new_one_not_marked_held_body()).await
}

async fn a_woken_agents_own_reply_is_a_new_one_not_marked_held_body() {
    let b = bg();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &b.running, &b.started_task, &b.reply, &b.none, &b.updated, &b.notified, &b.init, &b.woke, &b.second]);
    until(|| dones(&events).len() == 1).await;
    m.end(Some(0));
    session.finished().await;
    assert_eq!(dones(&events)[0].1, None);
    assert_eq!(dones(&events)[0].0.text.as_deref(), Some(WOKE_TEXT));
}

// TS: adapter.test.ts:1705
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_woken_agent_cut_before_its_own_reply_ends_a_turn_not_marked_held_failed_with_the_exit() {
    tokio::task::LocalSet::new().run_until(a_woken_agent_cut_before_its_own_reply_ends_a_turn_not_marked_held_failed_with_the_exit_body()).await
}

async fn a_woken_agent_cut_before_its_own_reply_ends_a_turn_not_marked_held_failed_with_the_exit_body() {
    let b = bg();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &b.running, &b.started_task, &b.reply, &b.init]);
    drained().await;
    m.end(Some(1));
    session.finished().await;
    assert_eq!(dones(&events).len(), 1);
    assert_eq!(dones(&events)[0].1, None);
    assert_eq!(dones(&events)[0].0.status, TurnStatus::Failed);
}

// TS: adapter.test.ts:1716
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reports_the_clis_own_count_to_the_runtime_on_every_change() {
    tokio::task::LocalSet::new().run_until(reports_the_clis_own_count_to_the_runtime_on_every_change_body()).await
}

async fn reports_the_clis_own_count_to_the_runtime_on_every_change_body() {
    let b = bg();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &b.running, &b.two, &b.none]);
    until(|| task_counts(&events).len() == 3).await;
    m.push(json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 5, "result": "done", "session_id": SID, "usage": {"input_tokens": 1, "output_tokens": 1}}).to_string());
    m.end(Some(0));
    session.finished().await;
    assert_eq!(task_counts(&events).iter().map(|(running, _)| *running).collect::<Vec<_>>(), vec![1, 2, 0]);
    // The count rides its own event and no client's: nothing about it reaches the reply.
    let result = dones(&events)[0].0.clone();
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.text.as_deref(), Some("done"));
}

// TS: adapter.test.ts:1728
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn is_held_until_the_last_task_is_gone_and_every_finished_task_is_under_the_reply() {
    tokio::task::LocalSet::new().run_until(is_held_until_the_last_task_is_gone_and_every_finished_task_is_under_the_reply_body()).await
}

async fn is_held_until_the_last_task_is_gone_and_every_finished_task_is_under_the_reply_body() {
    let b = bg();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &b.two, &b.reply, &b.notified, &b.only_b]);
    drained().await;
    assert_eq!(dones(&events), vec![]);

    m.push(b.notified_b.clone());
    m.push(b.none.clone());
    until(|| dones(&events).len() == 1).await;
    m.end(Some(0));
    let result = session.finished().await;
    let under = under_reply(&result);
    assert_eq!(under.len(), 2, "{under:?}");
    finished_after(&completed_after_reply(SLEEP_DESC), &under[0]);
    finished_after(&completed_after_reply("review"), &under[1]);
}

// TS: adapter.test.ts:1745
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ended_with_a_server_still_running_reads_done_and_names_the_running_work_as_its_last_line() {
    tokio::task::LocalSet::new().run_until(ended_with_a_server_still_running_reads_done_and_names_the_running_work_as_its_last_line_body()).await
}

async fn ended_with_a_server_still_running_reads_done_and_names_the_running_work_as_its_last_line_body() {
    let b = bg();
    // A server the agent started for its screenshots never exits, so no wake ever comes: the turn itself went fine.
    let serving = json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [{"task_id": TASK, "task_type": "local_bash", "description": "nohup node server.js"}], "session_id": SID}).to_string();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &serving, &b.started_task, &b.reply]);
    drained().await;
    assert_eq!(dones(&events), vec![]);
    m.end(None);
    let result = session.finished().await;
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.error, None);
    let lines: Vec<&str> = result.text.as_deref().unwrap().split('\n').collect();
    assert_eq!(lines, vec![WAITING, "", background_tasks_line(1).as_str()]);
    // The line a thread's row and a wait read.
    assert_eq!(notify_tail(&result), Some(background_tasks_line(1)));
    assert_eq!(dones(&events).len(), 1);
    assert_eq!(end_of(&snap(&events)).1, true);
}

// TS: adapter.test.ts:1763
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_held_reply_that_failed_stays_failed_with_its_own_error_and_names_no_running_work() {
    tokio::task::LocalSet::new().run_until(a_held_reply_that_failed_stays_failed_with_its_own_error_and_names_no_running_work_body()).await
}

async fn a_held_reply_that_failed_stays_failed_with_its_own_error_and_names_no_running_work_body() {
    let b = bg();
    let errored = json!({"type": "result", "subtype": "success", "is_error": true, "result": "API Error: 529 overloaded", "duration_ms": 40, "session_id": SID}).to_string();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &b.running, &b.started_task, &errored]);
    drained().await;
    assert_eq!(dones(&events), vec![]);
    m.end(None);
    let result = session.finished().await;
    assert_eq!(result.status, TurnStatus::Failed);
    assert_eq!(result.error.as_deref(), Some("API Error: 529 overloaded"));
    assert_eq!(result.text, None);
}

// TS: adapter.test.ts:1775
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ended_with_one_task_running_after_another_finished_names_both_the_running_work_last() {
    tokio::task::LocalSet::new().run_until(ended_with_one_task_running_after_another_finished_names_both_the_running_work_last_body()).await
}

async fn ended_with_one_task_running_after_another_finished_names_both_the_running_work_last_body() {
    let b = bg();
    let (m, events, session) = held(false, None);
    push_all(&m, &[&b.init, &b.two, &b.reply, &b.notified, &b.only_b]);
    drained().await;
    assert_eq!(dones(&events), vec![]);
    m.end(None);
    let result = session.finished().await;
    assert_eq!(result.status, TurnStatus::Completed);
    let under = under_reply(&result);
    assert_eq!(under.len(), 2, "{under:?}");
    finished_after(&completed_after_reply(SLEEP_DESC), &under[0]);
    assert_eq!(under[1], background_tasks_line(1));
}

// TS: adapter.test.ts:1786
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stopped_by_the_person_reads_interrupted_and_keeps_the_reply() {
    tokio::task::LocalSet::new().run_until(stopped_by_the_person_reads_interrupted_and_keeps_the_reply_body()).await
}

async fn stopped_by_the_person_reads_interrupted_and_keeps_the_reply_body() {
    let b = bg();
    let (m, events, session) = held(true, None);
    push_all(&m, &[&b.init, &b.running, &b.started_task, &b.reply]);
    drained().await;
    assert_eq!(dones(&events), vec![]);
    session.interrupt().await;
    let result = session.finished().await;
    assert_eq!(result.status, TurnStatus::Interrupted);
    assert_eq!(result.text.as_deref(), Some(WAITING));
    assert_eq!(m.order(), vec!["teardown", "kill"]);
    assert_eq!(dones(&events).len(), 1);
}

// ---------------------------------------------------------------------------------------------------------------
// the agent's tool shell
// ---------------------------------------------------------------------------------------------------------------

mod shell {
    use super::*;

    const SID: &str = FIXTURE_SESSION_ID;

    pub fn init() -> String {
        json!({"type": "system", "subtype": "init", "cwd": "/root", "session_id": SID, "tools": ["Bash", "Write"], "model": "claude-sonnet-4-5"}).to_string()
    }

    pub fn tool_use(id: &str, name: &str, input: Value) -> String {
        json!({"type": "assistant", "session_id": SID, "message": {"role": "assistant", "content": [{"type": "tool_use", "id": id, "name": name, "input": input}]}}).to_string()
    }

    pub fn tool_result(id: &str) -> String {
        json!({"type": "user", "session_id": SID, "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": id, "content": "ok", "is_error": false}]}}).to_string()
    }

    pub fn result() -> String {
        json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 1, "result": "done", "session_id": SID}).to_string()
    }

    /// Each delta's kind, tool name and folder.
    pub async fn run(lines: Vec<String>) -> Vec<(DeltaKind, Option<String>, Option<String>)> {
        let exec = script(lines);
        let adapter = adapter_over(&exec);
        let (events, on_event) = collect();
        adapter.start(opts("go", on_event)).unwrap().finished().await;
        let events = snap(&events);
        claude_exec::deltas(&events).into_iter().map(|d| (d.kind, d.tool_name, d.cwd)).collect()
    }

    pub fn row(kind: DeltaKind, tool: Option<&str>, cwd: Option<&str>) -> (DeltaKind, Option<String>, Option<String>) {
        (kind, tool.map(String::from), cwd.map(String::from))
    }
}

// TS: adapter.test.ts:1816
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stamps_the_shells_folder_on_the_tool_use_that_moves_it_and_nothing_else() {
    tokio::task::LocalSet::new().run_until(stamps_the_shells_folder_on_the_tool_use_that_moves_it_and_nothing_else_body()).await
}

async fn stamps_the_shells_folder_on_the_tool_use_that_moves_it_and_nothing_else_body() {
    use shell::*;
    let deltas = run(vec![
        init(),
        tool_use("t1", "Bash", json!({"command": "ls"})),
        tool_result("t1"),
        tool_use("t2", "Bash", json!({"command": "cd /root/2048 && npm test"})),
        tool_result("t2"),
        tool_use("t3", "Write", json!({"file_path": "/root/2048/src/game.js", "content": ""})),
        tool_result("t3"),
        tool_use("t4", "Bash", json!({"command": "cd /root/2048"})),
        tool_result("t4"),
        result(),
    ])
    .await;
    assert_eq!(
        deltas,
        vec![
            row(DeltaKind::ToolUse, Some("Bash"), None),
            row(DeltaKind::ToolResult, None, None),
            row(DeltaKind::ToolUse, Some("Bash"), Some("/root/2048")),
            row(DeltaKind::ToolResult, None, None),
            row(DeltaKind::ToolUse, Some("Write"), None),
            row(DeltaKind::ToolResult, None, None),
            row(DeltaKind::ToolUse, Some("Bash"), None),
            row(DeltaKind::ToolResult, None, None),
        ]
    );
}

// TS: adapter.test.ts:1841
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn takes_a_write_under_the_harness_folder_as_the_shells_folder_when_no_cd_came_first() {
    tokio::task::LocalSet::new().run_until(takes_a_write_under_the_harness_folder_as_the_shells_folder_when_no_cd_came_first_body()).await
}

async fn takes_a_write_under_the_harness_folder_as_the_shells_folder_when_no_cd_came_first_body() {
    use shell::*;
    let deltas = run(vec![
        init(),
        tool_use("t1", "Write", json!({"file_path": "/root/2048/index.html", "content": ""})),
        tool_result("t1"),
        tool_use("t2", "Edit", json!({"file_path": "/root/2048/style.css", "old_string": "a", "new_string": "b"})),
        tool_result("t2"),
        tool_use("t3", "Write", json!({"file_path": "/etc/motd", "content": ""})),
        tool_result("t3"),
        result(),
    ])
    .await;
    let uses: Vec<_> = deltas.into_iter().filter(|(kind, _, _)| *kind == DeltaKind::ToolUse).collect();
    assert_eq!(uses, vec![row(DeltaKind::ToolUse, Some("Write"), Some("/root/2048")), row(DeltaKind::ToolUse, Some("Edit"), None), row(DeltaKind::ToolUse, Some("Write"), None)]);
}

// ---------------------------------------------------------------------------------------------------------------
// what a rewind needs of a Claude Code turn
// ---------------------------------------------------------------------------------------------------------------

fn rewind_adapter(exec: &Arc<FakeExec>) -> ClaudeAdapter {
    adapter_with(exec.clone(), |d| d.sign_in_refusal = Some(sign_in_cloud()))
}

// TS: adapter.test.ts:1863
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn names_the_turns_last_message_once_before_the_reply_as_the_point_a_cut_keeps() {
    tokio::task::LocalSet::new().run_until(names_the_turns_last_message_once_before_the_reply_as_the_point_a_cut_keeps_body()).await
}

async fn names_the_turns_last_message_once_before_the_reply_as_the_point_a_cut_keeps_body() {
    let exec = script(fixture_lines("stream-session.jsonl"));
    let (events, on_event) = collect();
    rewind_adapter(&exec).start(opts("go", on_event)).unwrap().finished().await;
    let events = snap(&events);
    let anchors: Vec<AdapterEvent> = events.iter().filter(|e| matches!(e, AdapterEvent::TurnAnchor { .. })).cloned().collect();
    assert_eq!(anchors, vec![AdapterEvent::TurnAnchor { session_id: FIXTURE_SESSION_ID.to_string(), anchor: "1a2b3c4d-0006-4aaa-8bbb-000000000006".to_string(), kept: None }]);
    let at = events.iter().position(|e| matches!(e, AdapterEvent::TurnAnchor { .. })).unwrap();
    let done_at = events.iter().position(|e| matches!(e, AdapterEvent::TurnDone { .. })).unwrap();
    assert!(at < done_at);
}

// TS: adapter.test.ts:1872
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaves_a_subagents_messages_out_of_the_anchor() {
    tokio::task::LocalSet::new().run_until(leaves_a_subagents_messages_out_of_the_anchor_body()).await
}

async fn leaves_a_subagents_messages_out_of_the_anchor_body() {
    let lines = fixture_lines("stream-session.jsonl");
    let at = lines.iter().position(|l| l.contains("\"subtype\":\"success\"")).expect("the success line");
    let sub = json!({"type": "assistant", "uuid": "9f9f9f9f-0000-4000-8000-000000000099", "parent_tool_use_id": "toolu_sub", "session_id": FIXTURE_SESSION_ID, "message": {"id": "m_sub", "role": "assistant", "content": [{"type": "text", "text": "sub"}]}}).to_string();
    let mut all = lines[..at].to_vec();
    all.push(sub);
    all.extend_from_slice(&lines[at..]);
    let exec = script(all);
    let (events, on_event) = collect();
    rewind_adapter(&exec).start(opts("go", on_event)).unwrap().finished().await;
    let anchor = snap(&events).into_iter().find_map(|e| if let AdapterEvent::TurnAnchor { anchor, .. } = e { Some(anchor) } else { None });
    assert_eq!(anchor.as_deref(), Some("1a2b3c4d-0006-4aaa-8bbb-000000000006"));
}

// TS: adapter.test.ts:1882
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forks_a_session_through_a_turns_anchor_on_a_copy_and_never_resumes_at_a_message() {
    tokio::task::LocalSet::new().run_until(forks_a_session_through_a_turns_anchor_on_a_copy_and_never_resumes_at_a_message_body()).await
}

async fn forks_a_session_through_a_turns_anchor_on_a_copy_and_never_resumes_at_a_message_body() {
    // The copy's read answers the fixture's two message lines; the write and the removal print nothing.
    let first = json!({"parentUuid": null, "type": "user", "message": {"role": "user", "content": "hi"}, "uuid": "1a2b3c4d-0001-4aaa-8bbb-000000000001"}).to_string();
    let reply = json!({"parentUuid": "1a2b3c4d-0001-4aaa-8bbb-000000000001", "type": "assistant", "message": {"id": "m1", "content": [{"type": "text", "text": "hello"}]}, "uuid": "1a2b3c4d-0002-4aaa-8bbb-000000000002"}).to_string();
    let exec = script(vec!["2".to_string(), format!("1:{first}"), format!("2:{reply}")]);
    let adapter = rewind_adapter(&exec);
    let forked = adapter.fork_session(FIXTURE_SESSION_ID, ForkTurn::Anchor("1a2b3c4d-0002-4aaa-8bbb-000000000002".to_string())).await;
    let (resume, drop) = match forked {
        Ok(ForkedSession::Resume { resume, drop }) => (resume, drop),
        Ok(ForkedSession::Fork { .. }) => panic!("a Claude Code fork resumes a copy"),
        Err(e) => panic!("fork refused: {e}"),
    };
    assert!(Regex::new(r"^[0-9a-f-]{36}$").unwrap().is_match(&resume), "{resume}");
    assert_ne!(resume, FIXTURE_SESSION_ID);
    let calls = exec.calls();
    assert_eq!(calls[0].command, chain_read_command(FIXTURE_SESSION_ID, CFG).unwrap());
    assert_eq!(calls[1].command, chain_write_command(FIXTURE_SESSION_ID, &resume, CFG, 2).unwrap());
    assert_eq!(calls[1].input, Some(vec!["1-2".to_string()]));
    drop().await.unwrap();
    assert_eq!(exec.calls()[2].command, copy_cleanup_command(&resume, CFG).unwrap());
    match adapter.fork_session(FIXTURE_SESSION_ID, ForkTurn::After(vec![])).await {
        Err(e) => assert!(e.0.contains("named none"), "{e}"),
        Ok(_) => panic!("a turn with no anchor must be refused"),
    }
    adapter.start(opts_resume("next", &resume, quiet())).unwrap().finished().await;
    let calls = exec.calls();
    assert!(calls.last().unwrap().command.contains(&format!("--resume {resume}")), "{}", calls.last().unwrap().command);
    let joined = calls.iter().map(|c| c.command.clone()).collect::<Vec<_>>().join("\n");
    assert!(!joined.contains("--resume-session-at"));
}

// ---------------------------------------------------------------------------------------------------------------
// a model call Claude Code tries again
// ---------------------------------------------------------------------------------------------------------------

/// A recording's lines as Claude Code printed them, its provenance line left out and its session swapped for the
/// fixture's own, so a test resuming FIXTURE_SESSION_ID reads them as that session's.
fn recorded(name: &str) -> Vec<String> {
    let lines: Vec<String> = fixture_lines(&format!("{name}.jsonl")).into_iter().filter(|l| !l.contains("\"_fixture_provenance\"")).collect();
    let first: Value = serde_json::from_str(&lines[0]).unwrap();
    let session = first["session_id"].as_str().expect("a session_id on the first line").to_string();
    lines.iter().map(|l| l.replace(&session, FIXTURE_SESSION_ID)).collect()
}

async fn run_lines(lines: Vec<String>) -> (TurnResult, Vec<AdapterEvent>) {
    let (events, result) = run_x(&script(lines)).await;
    (result, events)
}

fn retry_line(attempt: u32, status: Option<u32>, error: &str) -> String {
    json!({"type": "system", "subtype": "api_retry", "attempt": attempt, "max_retries": 10, "retry_delay_ms": 500, "error_status": status, "error": error, "session_id": FIXTURE_SESSION_ID}).to_string()
}

fn retry_event(attempt: f64, why: &str) -> AdapterEvent {
    AdapterEvent::TurnRetry { session_id: FIXTURE_SESSION_ID.to_string(), attempt, of: Some(10.0), why: Some(why.to_string()) }
}

fn retries(events: &[AdapterEvent]) -> Vec<AdapterEvent> {
    events.iter().filter(|e| matches!(e, AdapterEvent::TurnRetry { .. })).cloned().collect()
}

// TS: adapter.test.ts:1922
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn says_each_retry_which_one_of_how_many_and_why_before_the_reply() {
    tokio::task::LocalSet::new().run_until(says_each_retry_which_one_of_how_many_and_why_before_the_reply_body()).await
}

async fn says_each_retry_which_one_of_how_many_and_why_before_the_reply_body() {
    let (done, events) = run_lines(recorded("api-retry-turn")).await;
    assert_eq!(done.status, TurnStatus::Completed);
    assert_eq!(done.text.as_deref(), Some("hello"));
    assert_eq!(retries(&events), vec![retry_event(1.0, "Anthropic was overloaded"), retry_event(2.0, "Anthropic was overloaded")]);
    let last_retry = events.iter().rposition(|e| matches!(e, AdapterEvent::TurnRetry { .. })).unwrap();
    let first_text = events.iter().position(|e| matches!(e, AdapterEvent::TurnDelta(d) if d.kind == DeltaKind::Text)).expect("a text delta");
    assert!(first_text > last_retry);
}

// TS: adapter.test.ts:1933
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn names_why_off_the_clis_own_error() {
    tokio::task::LocalSet::new().run_until(names_why_off_the_clis_own_error_body()).await
}

async fn names_why_off_the_clis_own_error_body() {
    let rows: [(Option<u32>, &str, &str); 4] = [
        (None, "unknown", "Anthropic did not answer"),
        (Some(500), "server_error", "Anthropic answered with a server error (500)"),
        (Some(429), "rate_limit", "Anthropic's rate limit was reached"),
        (Some(418), "something_new", "Anthropic answered 418"),
    ];
    for (status, error, why) in rows {
        let (_, events) = run_lines(vec![retry_line(1, status, error)]).await;
        assert_eq!(retries(&events), vec![retry_event(1.0, why)], "status {status:?} error {error}");
    }
}

// TS: adapter.test.ts:1941
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn says_a_subagents_retry_off_the_progress_line_its_call_carries() {
    tokio::task::LocalSet::new().run_until(says_a_subagents_retry_off_the_progress_line_its_call_carries_body()).await
}

async fn says_a_subagents_retry_off_the_progress_line_its_call_carries_body() {
    // A subagent the agent waits on, its call held past the CLI's wait: a heartbeat, the retry, then the call through.
    let progress = |more: Value| {
        let mut line = json!({"type": "tool_progress", "tool_use_id": "agent_msg_12", "tool_name": "Agent", "parent_tool_use_id": "toolu_one", "elapsed_time_seconds": 0, "session_id": FIXTURE_SESSION_ID, "uuid": "66f6638f-09b9-4699-bc14-552076ae3cec", "subagent_type": "general-purpose"});
        if let (Some(base), Value::Object(extra)) = (line.as_object_mut(), more) {
            base.extend(extra);
        }
        line.to_string()
    };
    let (_, events) = run_lines(vec![
        json!({"type": "tool_progress", "tool_use_id": "toolu_one-heartbeat-0", "tool_name": "Agent", "parent_tool_use_id": "toolu_one", "elapsed_time_seconds": 30, "heartbeat": true, "session_id": FIXTURE_SESSION_ID, "uuid": "66f6638f-09b9-4699-bc14-552076ae3cec"}).to_string(),
        progress(json!({"subagent_retry": {"agent_id": "a306bf0cc23c2cec4", "attempt": 1, "max_retries": 10, "retry_delay_ms": 580, "error_status": null, "error_category": "unknown"}})),
        progress(json!({"subagent_retry": {"agent_id": "a306bf0cc23c2cec4", "attempt": 2, "max_retries": 10, "retry_delay_ms": 1100, "error_status": 529, "error_category": "overloaded"}})),
        progress(json!({})),
    ])
    .await;
    assert_eq!(retries(&events), vec![retry_event(1.0, "Anthropic did not answer"), retry_event(2.0, "Anthropic was overloaded")]);
}

// ---------------------------------------------------------------------------------------------------------------
// a compaction Claude Code ran by itself mid-session
// ---------------------------------------------------------------------------------------------------------------

fn compacted_event(before: f64, after: f64) -> AdapterEvent {
    AdapterEvent::TurnCompacted { session_id: FIXTURE_SESSION_ID.to_string(), before: Some(before), after: Some(after) }
}

// TS: adapter.test.ts:1958
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn says_the_compaction_with_both_figures_and_the_reply_after_it_is_what_the_model_then_held() {
    tokio::task::LocalSet::new().run_until(says_the_compaction_with_both_figures_and_the_reply_after_it_is_what_the_model_then_held_body()).await
}

async fn says_the_compaction_with_both_figures_and_the_reply_after_it_is_what_the_model_then_held_body() {
    let exec = script(recorded("compact-turn"));
    let adapter = adapter_over(&exec);
    let (events, on_event) = collect();
    let result = adapter.start(opts_resume("keep going", FIXTURE_SESSION_ID, on_event)).unwrap().finished().await;
    let compacted: Vec<AdapterEvent> = snap(&events).into_iter().filter(|e| matches!(e, AdapterEvent::TurnCompacted { .. })).collect();
    assert_eq!(compacted, vec![compacted_event(106_883.0, 15_322.0)]);
    let tokens = result.tokens.expect("tokens");
    assert_eq!(tokens.context, Some(n(20_995.0)));
    assert_eq!(tokens.window, Some(n(200_000.0)));
}

// ---------------------------------------------------------------------------------------------------------------
// the plan's limits Claude Code prints
// ---------------------------------------------------------------------------------------------------------------

mod limits {
    use super::*;

    pub const SID: &str = "4b9c1d2e-0a1f-4c3b-9d8e-7f6a5b4c3d2e";

    pub fn init() -> String {
        json!({"type": "system", "subtype": "init", "cwd": "/root", "session_id": SID, "tools": [], "model": "claude-opus-5"}).to_string()
    }

    pub fn result() -> String {
        json!({"type": "result", "subtype": "success", "is_error": false, "duration_ms": 1, "result": "done", "session_id": SID}).to_string()
    }

    /// The shape Claude Code's own stream-json schema declares for the event.
    pub fn limit_line(info: Value) -> String {
        json!({"type": "rate_limit_event", "rate_limit_info": info, "uuid": "7c1e0f5e-3d44-4b0a-9a1c-2e6f8d9b0a11", "session_id": SID}).to_string()
    }

    pub async fn limits(lines: Vec<String>) -> Vec<(String, HarnessLimit)> {
        let mut all = vec![init()];
        all.extend(lines);
        all.push(result());
        let exec = script(all);
        let (events, on_event) = collect();
        adapter_over(&exec).start(opts("go", on_event)).unwrap().finished().await;
        snap(&events).into_iter().filter_map(|e| if let AdapterEvent::Limit { session_id, limit } = e { Some((session_id, limit)) } else { None }).collect()
    }

    pub fn window(kind: LimitKind, used_percent: f64, resets_at: f64) -> LimitWindow {
        LimitWindow { kind, used_percent: n(used_percent), resets_at: Some(n(resets_at)) }
    }

    pub fn limit(windows: Vec<LimitWindow>, status: Option<LimitStatus>, keyed: Option<bool>) -> HarnessLimit {
        HarnessLimit { windows, plan: None, status, account: None, keyed, credits: None }
    }
}

// TS: adapter.test.ts:1984
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reads_every_window_the_event_tracks_as_a_percent_and_ms_and_names_no_account() {
    tokio::task::LocalSet::new().run_until(reads_every_window_the_event_tracks_as_a_percent_and_ms_and_names_no_account_body()).await
}

async fn reads_every_window_the_event_tracks_as_a_percent_and_ms_and_names_no_account_body() {
    use limits::*;
    let seen = limits(vec![limit_line(json!({
        "status": "allowed",
        "rateLimitType": "five_hour",
        "utilization": 0.62,
        "resetsAt": 1_790_700_000u64,
        "unifiedWindows": {"five_hour": {"utilization": 0.62, "resetsAt": 1_790_700_000u64}, "seven_day": {"utilization": 0.18, "resetsAt": 1_791_200_000u64}},
    }))])
    .await;
    assert_eq!(seen, vec![(SID.to_string(), limit(vec![window(LimitKind::Session, 62.0, 1_790_700_000_000.0), window(LimitKind::Week, 18.0, 1_791_200_000_000.0)], Some(LimitStatus::Ok), None))]);
}

// TS: adapter.test.ts:2009
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn maps_each_of_the_six_limit_types_to_its_kind_and_the_status_words() {
    tokio::task::LocalSet::new().run_until(maps_each_of_the_six_limit_types_to_its_kind_and_the_status_words_body()).await
}

async fn maps_each_of_the_six_limit_types_to_its_kind_and_the_status_words_body() {
    use limits::*;
    let one = |rate_limit_type: &str, status: &str| limit_line(json!({"status": status, "rateLimitType": rate_limit_type, "utilization": 0.5, "resetsAt": 1_790_700_000u64}));
    let seen = limits(vec![
        one("five_hour", "allowed"),
        one("seven_day", "allowed_warning"),
        one("seven_day_opus", "allowed"),
        one("seven_day_sonnet", "allowed"),
        one("seven_day_overage_included", "allowed"),
        one("overage", "rejected"),
    ])
    .await;
    let read: Vec<(Vec<LimitKind>, Option<LimitStatus>)> = seen.iter().map(|(_, l)| (l.windows.iter().map(|w| w.kind).collect(), l.status)).collect();
    assert_eq!(
        read,
        vec![
            (vec![LimitKind::Session], Some(LimitStatus::Ok)),
            (vec![LimitKind::Week], Some(LimitStatus::Warning)),
            (vec![LimitKind::WeekOpus], Some(LimitStatus::Ok)),
            (vec![LimitKind::WeekSonnet], Some(LimitStatus::Ok)),
            (vec![LimitKind::Overage], Some(LimitStatus::Ok)),
            (vec![LimitKind::Overage], Some(LimitStatus::Reached)),
        ]
    );
}

// TS: adapter.test.ts:2029
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn says_a_turn_signed_in_with_a_key_has_no_plan_window_off_the_key_source_the_init_line_names() {
    tokio::task::LocalSet::new().run_until(says_a_turn_signed_in_with_a_key_has_no_plan_window_off_the_key_source_the_init_line_names_body()).await
}

async fn says_a_turn_signed_in_with_a_key_has_no_plan_window_off_the_key_source_the_init_line_names_body() {
    use limits::*;
    let run = |api_key_source: Option<&'static str>| async move {
        let mut init = json!({"type": "system", "subtype": "init", "cwd": "/root", "session_id": SID, "tools": [], "model": "claude-opus-5"});
        if let Some(source) = api_key_source {
            init.as_object_mut().unwrap().insert("apiKeySource".to_string(), json!(source));
        }
        let exec = script(vec![init.to_string(), result()]);
        let (events, on_event) = collect();
        adapter_over(&exec).start(opts("go", on_event)).unwrap().finished().await;
        snap(&events).into_iter().filter_map(|e| if let AdapterEvent::Limit { limit, .. } = e { Some(limit) } else { None }).collect::<Vec<HarnessLimit>>()
    };
    for keyed in ["ANTHROPIC_API_KEY", "apiKeyHelper", "/login managed key"] {
        assert_eq!(run(Some(keyed)).await, vec![limit(vec![], None, Some(true))], "key source {keyed}");
    }
    assert_eq!(run(Some("none")).await, vec![]);
    assert_eq!(run(None).await, vec![]);
}

// TS: adapter.test.ts:2042
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drops_an_event_that_reads_no_utilization_for_any_window() {
    tokio::task::LocalSet::new().run_until(drops_an_event_that_reads_no_utilization_for_any_window_body()).await
}

async fn drops_an_event_that_reads_no_utilization_for_any_window_body() {
    use limits::*;
    let seen = limits(vec![limit_line(json!({"status": "allowed", "rateLimitType": "five_hour", "resetsAt": 1_790_700_000u64})), limit_line(json!({"status": "allowed"}))]).await;
    assert_eq!(seen, vec![]);
}

/// A turn the plan's usage limit stopped: a spent window as the event's declared shape gives it, then a result
/// flagged as an error with the sentence the owner saw.
async fn stopped_turn(lines: Vec<String>) -> TurnResult {
    use limits::*;
    let stopped = json!({"type": "result", "subtype": "success", "is_error": true, "duration_ms": 1, "result": "You've hit your limit \u{b7} resets 1pm", "session_id": SID}).to_string();
    let mut all = vec![init()];
    all.extend(lines);
    all.push(stopped);
    adapter_over(&script(all)).start(opts("go", quiet())).unwrap().finished().await
}

// TS: adapter.test.ts:2054
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_usage_limit_stop_carries_the_limit_with_the_last_reset_of_the_rejected_windows() {
    tokio::task::LocalSet::new().run_until(a_usage_limit_stop_carries_the_limit_with_the_last_reset_of_the_rejected_windows_body()).await
}

async fn a_usage_limit_stop_carries_the_limit_with_the_last_reset_of_the_rejected_windows_body() {
    use limits::*;
    let done = stopped_turn(vec![
        limit_line(json!({"status": "rejected", "rateLimitType": "five_hour", "utilization": 1, "resetsAt": 1_790_700_000u64, "overageStatus": "rejected", "isUsingOverage": false})),
        limit_line(json!({"status": "rejected", "rateLimitType": "seven_day", "resetsAt": 1_791_200_000u64})),
    ])
    .await;
    assert_eq!(done.status, TurnStatus::Failed);
    assert_eq!(done.limit, Some(TurnLimit { resets_at: Some(n(1_791_200_000_000.0)) }));
}

// TS: adapter.test.ts:2062
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_usage_limit_stop_carries_the_limit_with_no_reset_where_a_rejected_window_named_none() {
    tokio::task::LocalSet::new().run_until(a_usage_limit_stop_carries_the_limit_with_no_reset_where_a_rejected_window_named_none_body()).await
}

async fn a_usage_limit_stop_carries_the_limit_with_no_reset_where_a_rejected_window_named_none_body() {
    use limits::*;
    let done = stopped_turn(vec![limit_line(json!({"status": "rejected", "rateLimitType": "five_hour", "resetsAt": 1_790_700_000u64})), limit_line(json!({"status": "rejected", "rateLimitType": "seven_day"}))]).await;
    assert_eq!(done.limit, Some(TurnLimit { resets_at: None }));
}

// TS: adapter.test.ts:2067
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_usage_limit_stop_carries_none_where_overage_runs_on_or_a_later_reading_allowed_it_again() {
    tokio::task::LocalSet::new().run_until(a_usage_limit_stop_carries_none_where_overage_runs_on_or_a_later_reading_allowed_it_again_body()).await
}

async fn a_usage_limit_stop_carries_none_where_overage_runs_on_or_a_later_reading_allowed_it_again_body() {
    use limits::*;
    let overage = stopped_turn(vec![limit_line(json!({"status": "rejected", "rateLimitType": "five_hour", "resetsAt": 1_790_700_000u64, "overageStatus": "allowed"}))]).await;
    assert_eq!(overage.status, TurnStatus::Failed);
    assert_eq!(overage.limit, None);
    let allowed_again = stopped_turn(vec![limit_line(json!({"status": "rejected", "rateLimitType": "five_hour", "resetsAt": 1_790_700_000u64})), limit_line(json!({"status": "allowed", "rateLimitType": "five_hour", "utilization": 0.1}))]).await;
    assert_eq!(allowed_again.status, TurnStatus::Failed);
    assert_eq!(allowed_again.limit, None);
    let no_reading = stopped_turn(vec![]).await;
    assert_eq!(no_reading.status, TurnStatus::Failed);
    assert_eq!(no_reading.limit, None);
}

// ---------------------------------------------------------------------------------------------------------------
// a person's setup for Claude Code on a computer
// ---------------------------------------------------------------------------------------------------------------

/// A HarnessExec answering "" to every command and noting each.
struct Asked(Arc<Mutex<Vec<String>>>);

impl HarnessExec for Asked {
    fn exec(&self, command: &str, _env: Option<&Env>) -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
        self.0.lock().unwrap().push(command.to_string());
        async { Ok(String::new()) }.boxed_local()
    }
}

fn setup() -> AgentLaunch {
    AgentLaunch { program: Some("/opt/my claude/bin/claude".to_string()), args: Some(vec!["--debug".to_string(), "a b".to_string()]) }
}

// TS: adapter.test.ts:2078
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runs_the_program_everywhere_and_adds_the_launch_words_to_a_turn_and_its_side_question_alone() {
    tokio::task::LocalSet::new().run_until(runs_the_program_everywhere_and_adds_the_launch_words_to_a_turn_and_its_side_question_alone_body()).await
}

async fn runs_the_program_everywhere_and_adds_the_launch_words_to_a_turn_and_its_side_question_alone_body() {
    let exec = script(fixture_lines("stream-session.jsonl"));
    let launch = setup();
    let adapter = adapter_with(exec.clone(), |d| d.launch = Some(launch.clone()));
    adapter.start(opts("go", quiet())).unwrap().finished().await;
    assert!(exec.calls()[0].command.contains("'/opt/my claude/bin/claude' -p '--debug' 'a b' --input-format stream-json"), "{}", exec.calls()[0].command);
    let asked: Arc<Mutex<Vec<String>>> = Default::default();
    let answer: Arc<dyn HarnessExec> = Arc::new(Asked(asked.clone()));
    adapter.probe_catalog(answer.clone()).await.unwrap();
    adapter.title_for(TitleTurn { opening: "fix the build".to_string(), reply: Some("done".to_string()), model: None }, answer.clone()).await.unwrap();
    adapter.draft_for(DraftAsk { prompt_file: "/tmp/ask".to_string(), model: None }, answer.clone()).await.unwrap();
    let asked = asked.lock().unwrap().clone();
    assert!(!asked.is_empty());
    let bare = Regex::new(r"(^|[;|&] *)claude ").unwrap();
    for command in &asked {
        assert!(command.contains("'/opt/my claude/bin/claude' -p"), "{command}");
        assert!(!bare.is_match(command), "{command}");
        assert!(!command.contains("--debug"), "{command}");
    }
    // The side question sends the request a turn sends, and the person's words can shape it, so it carries them too.
    let aside = aside_command(&AsideCommandOptions { fork: "11111111-2222-4333-8444-555555555555".to_string(), launch: Some(launch), ..Default::default() }).unwrap();
    assert!(aside.contains("'/opt/my claude/bin/claude' -p '--debug' 'a b' --input-format stream-json"), "{aside}");
}

// TS: adapter.test.ts:2100
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn with_no_setup_the_line_is_the_clis_own() {
    tokio::task::LocalSet::new().run_until(with_no_setup_the_line_is_the_clis_own_body()).await
}

async fn with_no_setup_the_line_is_the_clis_own_body() {
    let exec = script(fixture_lines("stream-session.jsonl"));
    adapter_over(&exec).start(opts("go", quiet())).unwrap().finished().await;
    let command = exec.calls()[0].command.clone();
    let expected = format!("&& {} -p --input-format stream-json", agent_word("claude", None));
    assert!(command.contains(&expected), "{command}");
}

// ---------------------------------------------------------------------------------------------------------------
// what a turn cost, off totals the CLI keeps for the whole session
// ---------------------------------------------------------------------------------------------------------------

mod cost {
    use super::*;

    pub fn init() -> String {
        json!({"type": "system", "subtype": "init", "cwd": "/w", "session_id": FIXTURE_SESSION_ID, "model": "claude-opus-5-5"}).to_string()
    }

    pub fn saved(total: f64) -> String {
        saved_of(total, FIXTURE_SESSION_ID)
    }

    pub fn saved_of(total: f64, session: &str) -> String {
        json!({"type": "cost-state", "sessionId": session, "totalCostUSD": total, "modelUsage": {"claude-opus-5-5": {"inputTokens": 9, "outputTokens": 9, "cacheReadInputTokens": 9, "cacheCreationInputTokens": 9, "webSearchRequests": 0, "costUSD": total}}}).to_string()
    }

    pub fn result(total: f64) -> String {
        json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "duration_ms": 900, "result": "done", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": total, "usage": {"input_tokens": 10, "output_tokens": 4}}).to_string()
    }

    pub struct Ran {
        pub done: TurnResult,
        pub events: Vec<AdapterEvent>,
        pub command: String,
    }

    pub async fn run(lines: Vec<String>, resume: Option<&str>) -> Ran {
        let exec = script(lines);
        let adapter = adapter_over(&exec);
        let (events, on_event) = collect();
        let options = match resume {
            Some(r) => opts_resume("x", r, on_event),
            None => opts("x", on_event),
        };
        let done = adapter.start(options).unwrap().finished().await;
        let command = exec.calls().first().map(|c| c.command.clone()).unwrap_or_default();
        Ran { done, events: snap(&events), command }
    }

    /// One model's running totals as the CLI keeps them; `think` is the optional thinkingTokens key.
    pub fn use_full(input: u64, out: u64, think: Option<u64>, read: u64, write: u64, cost: f64, max_output: bool) -> Value {
        let mut m = Map::new();
        m.insert("inputTokens".into(), json!(input));
        m.insert("outputTokens".into(), json!(out));
        if let Some(t) = think {
            m.insert("thinkingTokens".into(), json!(t));
        }
        m.insert("cacheReadInputTokens".into(), json!(read));
        m.insert("cacheCreationInputTokens".into(), json!(write));
        m.insert("webSearchRequests".into(), json!(0));
        m.insert("costUSD".into(), json!(cost));
        m.insert("contextWindow".into(), json!(1_000_000));
        if max_output {
            m.insert("maxOutputTokens".into(), json!(64_000));
        }
        Value::Object(m)
    }

    /// The shorter `use` the later cases share: contextWindow but no maxOutputTokens.
    pub fn use_(input: u64, read: u64, write: u64, out: u64, cost: f64) -> Value {
        use_full(input, out, None, read, write, cost, false)
    }

    pub fn call(id: &str, usage: Value, model: &str, parent: Option<&str>) -> String {
        json!({"type": "assistant", "message": {"id": id, "model": model, "role": "assistant", "content": [{"type": "text", "text": "x"}], "usage": usage}, "parent_tool_use_id": parent, "session_id": FIXTURE_SESSION_ID}).to_string()
    }

    pub fn ended(total: f64, usage: Value, models: Value) -> String {
        json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "duration_ms": 900, "result": "done", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": total, "usage": usage, "modelUsage": models}).to_string()
    }

    /// A haiku turn whose Task subagent ran in the background: the CLI printed two results.
    pub fn woke_lines(first: Option<Value>, second: Option<Value>) -> Vec<String> {
        let task_set = json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [{"task_id": "a1", "task_type": "local_agent", "description": "echo there"}], "session_id": FIXTURE_SESSION_ID}).to_string();
        let task_none = json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [], "session_id": FIXTURE_SESSION_ID}).to_string();
        let task_done = json!({"type": "system", "subtype": "task_notification", "task_id": "a1", "status": "completed", "summary": "echo there completed", "session_id": FIXTURE_SESSION_ID}).to_string();
        let mut waiting = json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 3, "duration_ms": 900, "result": "waiting", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 0.08, "usage": {"input_tokens": 18, "cache_creation_input_tokens": 10_166, "cache_read_input_tokens": 44_218, "output_tokens": 1_535}});
        if let Some(models) = first {
            waiting.as_object_mut().unwrap().insert("modelUsage".into(), models);
        }
        let mut done = json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "duration_ms": 300, "result": "done", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 0.1, "usage": {"input_tokens": 10, "cache_creation_input_tokens": 887, "cache_read_input_tokens": 27_898, "output_tokens": 144}, "origin": {"kind": "task-notification"}});
        if let Some(models) = second {
            done.as_object_mut().unwrap().insert("modelUsage".into(), models);
        }
        vec![init(), task_set, waiting.to_string(), task_none, task_done, json!({"type": "system", "subtype": "init", "cwd": "/w", "session_id": FIXTURE_SESSION_ID, "model": "claude-opus-5-5"}).to_string(), done.to_string()]
    }

    pub fn split(model: &str, input: u64, output: u64, cached: u64, cache_write: u64, reasoning: u64, cost: Option<f64>) -> TurnResultModelsItem {
        TurnResultModelsItem { model: model.to_string(), tokens: UsageTokens { input, output, cached, cache_write, reasoning }, cost_usd: cost.map(n) }
    }
}

// TS: adapter.test.ts:2123
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_resumed_turn_costs_its_results_total_less_the_total_the_transcript_saved() {
    tokio::task::LocalSet::new().run_until(a_resumed_turn_costs_its_results_total_less_the_total_the_transcript_saved_body()).await
}

async fn a_resumed_turn_costs_its_results_total_less_the_total_the_transcript_saved_body() {
    use cost::*;
    let ran = run(vec![saved(352.3259844), init(), result(370.679875)], Some(FIXTURE_SESSION_ID)).await;
    close(ran.done.cost_usd, 18.3538906, 6);
    // The launch prints the saved line ahead of the CLI, read from the same file the CLI restores from.
    let head = Regex::new(r"^wsp_saved=\$\(ls -1td '/root/\.claude-cfg/projects'/\*/'e16ed170-8257-4668-879e-fe836341633c\.jsonl'").unwrap();
    assert!(head.is_match(&ran.command), "{}", ran.command);
    assert!(ran.command.contains("grep -F '\"type\":\"cost-state\"'"), "{}", ran.command);
}

// TS: adapter.test.ts:2131
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_saved_line_of_another_session_is_not_this_sessions_total() {
    tokio::task::LocalSet::new().run_until(a_saved_line_of_another_session_is_not_this_sessions_total_body()).await
}

async fn a_saved_line_of_another_session_is_not_this_sessions_total_body() {
    use cost::*;
    let ran = run(vec![saved_of(352.3259844, "11111111-2222-4333-8444-555555555555"), init(), result(370.679875)], Some(FIXTURE_SESSION_ID)).await;
    assert_eq!(ran.done.cost_usd, None);
    let tokens = ran.done.tokens.expect("tokens");
    assert_eq!((tokens.input, tokens.output), (n(10.0), n(4.0)));
}

// TS: adapter.test.ts:2137
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_resume_with_nothing_saved_leaves_the_turns_cost_out() {
    tokio::task::LocalSet::new().run_until(a_resume_with_nothing_saved_leaves_the_turns_cost_out_body()).await
}

async fn a_resume_with_nothing_saved_leaves_the_turns_cost_out_body() {
    use cost::*;
    let ran = run(vec![init(), result(370.679875)], Some(FIXTURE_SESSION_ID)).await;
    assert_eq!(ran.done.cost_usd, None);
    assert_eq!(ran.done.models, None);
}

// TS: adapter.test.ts:2143
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_saved_line_that_does_not_read_leaves_the_turns_cost_out() {
    tokio::task::LocalSet::new().run_until(a_saved_line_that_does_not_read_leaves_the_turns_cost_out_body()).await
}

async fn a_saved_line_that_does_not_read_leaves_the_turns_cost_out_body() {
    use cost::*;
    let broken = json!({"type": "cost-state", "sessionId": FIXTURE_SESSION_ID, "totalCostUSD": "352.33", "modelUsage": {}}).to_string();
    assert_eq!(run(vec![broken, init(), result(370.679875)], Some(FIXTURE_SESSION_ID)).await.done.cost_usd, None);
    assert_eq!(run(vec!["{\"type\":\"cost-state\",\"sessionId\":\"e16ed170".to_string(), init(), result(370.679875)], Some(FIXTURE_SESSION_ID)).await.done.cost_usd, None);
}

// TS: adapter.test.ts:2149
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_compaction_inside_the_turn_leaves_the_running_total_running() {
    tokio::task::LocalSet::new().run_until(a_compaction_inside_the_turn_leaves_the_running_total_running_body()).await
}

async fn a_compaction_inside_the_turn_leaves_the_running_total_running_body() {
    use cost::*;
    let compacted = recorded("compact-turn").into_iter().find(|l| l.contains("\"compact_boundary\"")).expect("a compact_boundary line");
    let ran = run(vec![saved(352.3259844), init(), compacted, result(370.679875)], Some(FIXTURE_SESSION_ID)).await;
    close(ran.done.cost_usd, 18.3538906, 6);
}

// TS: adapter.test.ts:2155
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_compact_turn_says_what_the_model_holds_after_the_compaction_though_its_result_reports_no_call() {
    tokio::task::LocalSet::new().run_until(a_compact_turn_says_what_the_model_holds_after_the_compaction_though_its_result_reports_no_call_body()).await
}

async fn a_compact_turn_says_what_the_model_holds_after_the_compaction_though_its_result_reports_no_call_body() {
    use cost::*;
    // 2.1.291's answer to a headless /compact on a resumed session: the status lines, init, the boundary with what
    // the model held before and after it, the CLI's own line, and a result whose usage is all zeros.
    let ran = run(recorded("compact-command"), Some(FIXTURE_SESSION_ID)).await;
    assert_eq!(ran.done.status, TurnStatus::Completed);
    assert_eq!(ran.done.tokens.expect("tokens").context, Some(n(1_152.0)));
    let compacted: Vec<AdapterEvent> = ran.events.iter().filter(|e| matches!(e, AdapterEvent::TurnCompacted { .. })).cloned().collect();
    assert_eq!(compacted, vec![compacted_event(44_560.0, 1_152.0)]);
}

// TS: adapter.test.ts:2164
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_total_under_the_saved_one_started_again_from_nothing_so_the_turn_cost_the_whole_of_it() {
    tokio::task::LocalSet::new().run_until(a_total_under_the_saved_one_started_again_from_nothing_so_the_turn_cost_the_whole_of_it_body()).await
}

async fn a_total_under_the_saved_one_started_again_from_nothing_so_the_turn_cost_the_whole_of_it_body() {
    use cost::*;
    let ran = run(vec![saved(352.3259844), init(), result(2.5)], Some(FIXTURE_SESSION_ID)).await;
    assert_eq!(ran.done.cost_usd, Some(n(2.5)));
}

// TS: adapter.test.ts:2169
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn files_each_model_a_turn_used_with_its_own_tokens_and_cost() {
    tokio::task::LocalSet::new().run_until(files_each_model_a_turn_used_with_its_own_tokens_and_cost_body()).await
}

async fn files_each_model_a_turn_used_with_its_own_tokens_and_cost_body() {
    use cost::*;
    let before = json!({"type": "cost-state", "sessionId": FIXTURE_SESSION_ID, "totalCostUSD": 10.5, "modelUsage": {
        "claude-opus-5-5[1m]": use_full(100, 1_000, None, 50_000, 5_000, 10.0, true),
        "claude-haiku-4-5-20251001": use_full(10, 100, None, 0, 2_000, 0.5, true),
    }}).to_string();
    let after = json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "duration_ms": 900, "result": "done", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 13.75,
        "usage": {"input_tokens": 5, "cache_read_input_tokens": 20_000, "cache_creation_input_tokens": 1_000, "output_tokens": 300},
        "modelUsage": {
            "claude-opus-5-5[1m]": use_full(105, 1_300, Some(40), 70_000, 6_000, 12.5, true),
            "claude-haiku-4-5-20251001": use_full(10, 100, None, 0, 2_000, 0.5, true),
            "claude-sonnet-5": use_full(7, 70, None, 3_000, 500, 0.75, true),
        }}).to_string();
    let ran = run(vec![before, init(), after], Some(FIXTURE_SESSION_ID)).await;
    close(ran.done.cost_usd, 3.25, 9);
    assert_eq!(ran.done.model.as_deref(), Some("claude-opus-5-5[1m]"));
    assert_eq!(
        ran.done.models,
        Some(vec![split("claude-opus-5-5[1m]", 21_005, 300, 20_000, 1_000, 40, Some(2.5)), split("claude-sonnet-5", 3_507, 70, 3_000, 500, 0, Some(0.75))])
    );
}

// TS: adapter.test.ts:2190
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn says_what_each_model_call_drew_once_per_message_and_what_the_agents_own_call_left_it_holding() {
    tokio::task::LocalSet::new().run_until(says_what_each_model_call_drew_once_per_message_and_what_the_agents_own_call_left_it_holding_body()).await
}

async fn says_what_each_model_call_drew_once_per_message_and_what_the_agents_own_call_left_it_holding_body() {
    use cost::*;
    let said = |id: &str, usage: Value, parent: Option<&str>| {
        let last = &id[id.len() - 1..];
        json!({"type": "assistant", "message": {"id": id, "role": "assistant", "content": [{"type": "text", "text": "x"}], "usage": usage}, "parent_tool_use_id": parent, "session_id": FIXTURE_SESSION_ID, "timestamp": format!("2026-10-01T10:00:0{last}.000Z")}).to_string()
    };
    let exec = script(vec![
        init(),
        said("msg_1", json!({"input_tokens": 5, "cache_read_input_tokens": 1_000, "cache_creation_input_tokens": 200, "output_tokens": 40}), None),
        said("msg_1", json!({"input_tokens": 5, "cache_read_input_tokens": 1_000, "cache_creation_input_tokens": 200, "output_tokens": 40}), None),
        said("msg_2", json!({"input_tokens": 3, "output_tokens": 7}), Some("toolu_1")),
        result(1.0),
    ]);
    let (events, on_event) = collect();
    adapter_over(&exec).start(opts("x", on_event)).unwrap().finished().await;
    let usage: Vec<AdapterEvent> = snap(&events).into_iter().filter(|e| matches!(e, AdapterEvent::TurnUsage { .. })).collect();
    // Date.parse of the two timestamps, in ms since the epoch.
    assert_eq!(
        usage,
        vec![
            AdapterEvent::TurnUsage { session_id: FIXTURE_SESSION_ID.to_string(), tokens: 1_245.0, context: Some(1_245.0), window: None, at: Some(1_790_848_801_000.0) },
            AdapterEvent::TurnUsage { session_id: FIXTURE_SESSION_ID.to_string(), tokens: 10.0, context: None, window: None, at: Some(1_790_848_802_000.0) },
        ]
    );
}

// TS: adapter.test.ts:2209
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_models_totals_under_their_saved_ones_leaves_the_split_by_model_unknown() {
    tokio::task::LocalSet::new().run_until(one_models_totals_under_their_saved_ones_leaves_the_split_by_model_unknown_body()).await
}

async fn one_models_totals_under_their_saved_ones_leaves_the_split_by_model_unknown_body() {
    use cost::*;
    let use_ = |input: u64, cost: f64| json!({"inputTokens": input, "outputTokens": 10, "cacheReadInputTokens": 0, "cacheCreationInputTokens": 0, "webSearchRequests": 0, "costUSD": cost});
    let before = json!({"type": "cost-state", "sessionId": FIXTURE_SESSION_ID, "totalCostUSD": 10, "modelUsage": {"claude-opus-5-5": use_(1_000, 9.0), "claude-haiku-4-5-20251001": use_(500, 1.0)}}).to_string();
    let after = json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "duration_ms": 900, "result": "done", "session_id": FIXTURE_SESSION_ID, "total_cost_usd": 12, "usage": {"input_tokens": 10, "output_tokens": 4}, "modelUsage": {"claude-opus-5-5": use_(1_400, 11.9), "claude-haiku-4-5-20251001": use_(200, 0.1)}}).to_string();
    let ran = run(vec![before, init(), after], Some(FIXTURE_SESSION_ID)).await;
    assert_eq!(ran.done.cost_usd, Some(n(2.0)));
    assert_eq!(ran.done.models, None);
}

// TS: adapter.test.ts:2218
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_session_has_nothing_saved_so_the_launch_reads_no_file_and_the_turn_cost_its_whole_total() {
    tokio::task::LocalSet::new().run_until(a_new_session_has_nothing_saved_so_the_launch_reads_no_file_and_the_turn_cost_its_whole_total_body()).await
}

async fn a_new_session_has_nothing_saved_so_the_launch_reads_no_file_and_the_turn_cost_its_whole_total_body() {
    use cost::*;
    let ran = run(vec![init(), result(4.25)], None).await;
    assert_eq!(ran.done.cost_usd, Some(n(4.25)));
    assert!(!ran.command.contains("cost-state"), "{}", ran.command);
}

// TS: adapter.test.ts:2230
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn files_the_split_by_model_whatever_the_stream_printed() {
    tokio::task::LocalSet::new().run_until(files_the_split_by_model_whatever_the_stream_printed_body()).await
}

async fn files_the_split_by_model_whatever_the_stream_printed_body() {
    use cost::*;
    let lines = vec![
        init(),
        call("msg_1", json!({"input_tokens": 5, "cache_read_input_tokens": 1_000, "cache_creation_input_tokens": 200, "output_tokens": 1}), "claude-opus-5-5", None),
        call("msg_2", json!({"input_tokens": 3, "cache_read_input_tokens": 500, "output_tokens": 1}), "claude-opus-5-5", Some("toolu_1")),
        ended(
            0.5,
            json!({"input_tokens": 5, "cache_read_input_tokens": 1_000, "cache_creation_input_tokens": 200, "output_tokens": 40}),
            json!({"claude-opus-5-5[1m]": use_(8, 1_500, 200, 47, 0.45), "claude-haiku-4-5-20251001": use_(700, 0, 0, 30, 0.05)}),
        ),
    ];
    let ran = run(lines, None).await;
    assert_eq!(ran.done.cost_usd, Some(n(0.5)));
    let models: Vec<String> = ran.done.models.expect("a split by model").into_iter().map(|m| m.model).collect();
    assert_eq!(models, vec!["claude-opus-5-5[1m]".to_string(), "claude-haiku-4-5-20251001".to_string()]);
}

// TS: adapter.test.ts:2246
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_turn_whose_cli_made_a_call_it_never_printed_keeps_its_cost_and_split() {
    tokio::task::LocalSet::new().run_until(a_turn_whose_cli_made_a_call_it_never_printed_keeps_its_cost_and_split_body()).await
}

async fn a_turn_whose_cli_made_a_call_it_never_printed_keeps_its_cost_and_split_body() {
    use cost::*;
    // Recorded on 2.1.280: three printed haiku calls read 81,881 tokens, modelUsage 82,084.
    let ran = run(fixture_lines("webfetch-turn.jsonl"), None).await;
    close(ran.done.cost_usd, 0.04197715, 9);
    let models = ran.done.models.clone().expect("a split by model");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model, "claude-haiku-4-5-20251001");
    assert_eq!(models[0].tokens, UsageTokens { input: 82_084, output: 346, cached: 54_174, cache_write: 27_679, reasoning: 162 });
    close(models[0].cost_usd, 0.04197715, 9);
    let tokens = ran.done.tokens.expect("tokens");
    assert_eq!((tokens.input, tokens.output), (n(82_084.0), n(346.0)));
}

// TS: adapter.test.ts:2272
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_turn_the_cli_woke_for_its_background_work_counts_every_token_since_its_start() {
    tokio::task::LocalSet::new().run_until(a_turn_the_cli_woke_for_its_background_work_counts_every_token_since_its_start_body()).await
}

async fn a_turn_the_cli_woke_for_its_background_work_counts_every_token_since_its_start_body() {
    use cost::*;
    let lines = woke_lines(
        Some(json!({"claude-haiku-4-5-20251001": use_(18, 70_000, 17_218, 1_681, 0.08)})),
        Some(json!({"claude-haiku-4-5-20251001": use_(28, 97_898, 18_105, 1_825, 0.1)})),
    );
    let ran = run(lines, None).await;
    assert_eq!(ran.done.text.as_deref(), Some("done"));
    let models = ran.done.models.clone().expect("a split by model");
    assert_eq!(models.len(), 1);
    let tokens = ran.done.tokens.expect("tokens");
    assert_eq!((tokens.input, tokens.output, tokens.cached, tokens.cache_write), (n(116_031.0), n(1_825.0), Some(n(97_898.0)), Some(n(18_105.0))));
    let split = &models[0].tokens;
    assert_eq!((tokens.input, tokens.output, tokens.cached, tokens.cache_write), (n(split.input as f64), n(split.output as f64), Some(n(split.cached as f64)), Some(n(split.cache_write as f64))));
}

// TS: adapter.test.ts:2281
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_woken_agent_cut_before_its_reply_adds_the_calls_printed_after_the_held_reply_as_an_entry_with_no_cost() {
    tokio::task::LocalSet::new().run_until(a_woken_agent_cut_before_its_reply_adds_the_calls_printed_after_the_held_reply_as_an_entry_with_no_cost_body()).await
}

async fn a_woken_agent_cut_before_its_reply_adds_the_calls_printed_after_the_held_reply_as_an_entry_with_no_cost_body() {
    use cost::*;
    let mut lines = woke_lines(Some(json!({"claude-haiku-4-5-20251001": use_(18, 70_000, 17_218, 1_681, 0.08)})), None);
    lines.pop();
    lines.push(call("msg_9", json!({"input_tokens": 2, "cache_read_input_tokens": 60_000, "output_tokens": 8}), "claude-haiku-4-5-20251001", None));
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_over(&exec);
    let session = adapter.start(opts("x", quiet())).unwrap();
    for line in lines {
        exec.push(line);
    }
    exec.current().exit.resolve(None);
    exec.current().fail(ExecFailure { kind: ExecFailureKind::Other, message: "stopped after 6:00:00 at the 6h turn limit".to_string() });
    let done = session.finished().await;
    assert_eq!(done.status, TurnStatus::Failed);
    assert_eq!(done.text.as_deref(), Some("waiting"));
    assert_eq!(done.error.as_deref(), Some("stopped after 6:00:00 at the 6h turn limit"));
    assert_eq!(done.cost_usd, Some(n(0.08)));
    let tokens = done.tokens.clone().expect("tokens");
    assert_eq!((tokens.input, tokens.output, tokens.cached, tokens.cache_write), (n((87_236 + 60_002) as f64), n((1_681 + 8) as f64), Some(n((70_000 + 60_000) as f64)), Some(n(17_218.0))));
    // The reply's cost covers the reply's tokens alone, so the calls after it come with none and take the list price.
    assert_eq!(
        done.models,
        Some(vec![split("claude-haiku-4-5-20251001", 87_236, 1_681, 70_000, 17_218, 0, Some(0.08)), split("claude-haiku-4-5-20251001", 60_002, 8, 60_000, 0, 0, None)])
    );
}

// TS: adapter.test.ts:2312
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_woken_turn_with_no_split_by_model_still_counts_both_of_its_results_tokens() {
    tokio::task::LocalSet::new().run_until(a_woken_turn_with_no_split_by_model_still_counts_both_of_its_results_tokens_body()).await
}

async fn a_woken_turn_with_no_split_by_model_still_counts_both_of_its_results_tokens_body() {
    use cost::*;
    let ran = run(woke_lines(None, None), Some(FIXTURE_SESSION_ID)).await;
    assert_eq!(ran.done.models, None);
    let tokens = ran.done.tokens.expect("tokens");
    assert_eq!(
        (tokens.input, tokens.output, tokens.cached, tokens.cache_write),
        (n((18 + 10_166 + 44_218 + 10 + 887 + 27_898) as f64), n((1_535 + 144) as f64), Some(n((44_218 + 27_898) as f64)), Some(n((10_166 + 887) as f64)))
    );
}
