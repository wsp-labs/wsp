// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/adapter-claude/test/adapter.test.ts lines 1199-2317: how a turn's result is classified, a reply
//! given while background work runs, the agent's tool shell, what a rewind needs, a retried model call, a
//! compaction, the plan's limits, a person's launch setup and what a turn cost.

#![allow(dead_code, unused_imports)]

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
fn compacted_event(before: f64, after: f64) -> AdapterEvent {
    AdapterEvent::TurnCompacted { session_id: FIXTURE_SESSION_ID.to_string(), before: Some(before), after: Some(after) }
}

/// A recording's lines as Claude Code printed them, its provenance line left out and its session swapped for the
/// fixture's own, so a test resuming FIXTURE_SESSION_ID reads them as that session's.
fn recorded(name: &str) -> Vec<String> {
    let lines: Vec<String> = fixture_lines(&format!("{name}.jsonl")).into_iter().filter(|l| !l.contains("\"_fixture_provenance\"")).collect();
    let first: Value = serde_json::from_str(&lines[0]).unwrap();
    let session = first["session_id"].as_str().expect("a session_id on the first line").to_string();
    lines.iter().map(|l| l.replace(&session, FIXTURE_SESSION_ID)).collect()
}

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
        TurnResultModelsItem { model: model.to_string(), tokens: UsageTokens { input: n(input as f64), output: n(output as f64), cached: n(cached as f64), cache_write: n(cache_write as f64), reasoning: n(reasoning as f64) }, cost_usd: cost.map(n) }
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
    assert_eq!(models[0].tokens, UsageTokens { input: 82_084.into(), output: 346.into(), cached: 54_174.into(), cache_write: 27_679.into(), reasoning: 162.into() });
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
    assert_eq!((tokens.input, tokens.output, tokens.cached, tokens.cache_write), (split.input, split.output, Some(split.cached), Some(split.cache_write)));
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
