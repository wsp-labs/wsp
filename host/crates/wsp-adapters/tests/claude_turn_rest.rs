// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-claude/test/adapter.test.ts:1-1198 at a3b870a6e: the adapter over the recorded fixtures, its
//! launch line and environment, the interrupt policy, the process a finished turn leaves, steering over the stdin
//! channel and a reply while the process keeps running.

#![allow(dead_code, unused_imports)]

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::{LocalBoxFuture, FutureExt};
use serde_json::{json, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps, AttachedSession, ClaudeAdapter};
use wsp_adapters::claude::catalog::claude_screen_commands;
use wsp_adapters::claude::landmines::{user_message_line, BaseEnv};
use wsp_adapters::claude::turn::ClaudeSession;
use wsp_adapters::pending::{internal_tool_result, subagent_asked, NO_REPORT_LINE, SUBAGENT_ASKED_CHARS};
use wsp_adapters::port::{agent_word, AdapterAttachOptions, AdapterEvent, AgentLaunch, DeltaKind, Env, ExecFailure, HarnessExec, HarnessStartOptions, OnEvent, PlanStep, SessionHarness, SteerOutcome, SubagentState, SubagentUsage, TurnDelta, TurnImage, TurnStatus, TurnTokens};
use wsp_wire::types::usage::UsageTokens;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::harness::ScreenControl;
use wsp_wire::views::session_events::PlanStepState;

use claude_exec::{collect, deltas, fixture_lines, guest_env, json as parse_json, types, until, FakeExec, StreamOpts, FIXTURE_SESSION_ID, RUN_HANDLE};

const SID: &str = FIXTURE_SESSION_ID;
const CONFIG_DIR: &str = "/root/.claude-cfg";

type Events = Arc<Mutex<Vec<AdapterEvent>>>;

// ---------------------------------------------------------------- helpers

fn noop() -> OnEvent {
    Arc::new(|_| {})
}

fn adapter_with(deps: AdapterDeps) -> ClaudeAdapter {
    create_claude_adapter(deps).unwrap()
}

fn adapter_for(exec: &Arc<FakeExec>) -> ClaudeAdapter {
    adapter_with(AdapterDeps::new(exec.clone(), CONFIG_DIR))
}

fn go(prompt: &str, on_event: OnEvent) -> HarnessStartOptions {
    HarnessStartOptions { prompt: prompt.to_string(), on_event: Some(on_event), ..Default::default() }
}

fn snap(events: &Events) -> Vec<AdapterEvent> {
    events.lock().unwrap().clone()
}

fn seen(events: &Events, ty: &str) -> bool {
    types(&events.lock().unwrap()).contains(&ty)
}

fn base_env(pairs: &[(&str, &str)]) -> BaseEnv {
    pairs.iter().map(|(k, v)| (k.to_string(), Some(v.to_string()))).collect()
}

fn guest_env_with(pairs: &[(&str, &str)]) -> BaseEnv {
    let mut env = guest_env();
    for (k, v) in pairs {
        env.insert(k.to_string(), Some(v.to_string()));
    }
    env
}

fn stream_lines() -> Vec<String> {
    fixture_lines("stream-session.jsonl")
}

fn init_line() -> String {
    json!({"type": "system", "subtype": "init", "session_id": SID}).to_string()
}

fn pause(ms: u64) -> tokio::time::Sleep {
    tokio::time::sleep(Duration::from_millis(ms))
}

fn env_of<'a>(env: &'a Env, key: &str) -> Option<&'a str> {
    env.get(key).map(String::as_str)
}

fn attach_opts(run: &str, session_id: &str, on_event: OnEvent) -> AdapterAttachOptions {
    AdapterAttachOptions { run: run.to_string(), session_id: session_id.to_string(), started_at: 1.0, model: None, cwd: None, prompt: None, effort: None, from: None, steered: None, on_event }
}

fn recording_exec(ran: Arc<Mutex<Vec<(String, Option<Env>)>>>, answer: String) -> Arc<dyn HarnessExec> {
    Arc::new(move |command: &str, env: Option<&Env>| -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
        ran.lock().unwrap().push((command.to_string(), env.cloned()));
        let answer = answer.clone();
        async move { Ok(answer) }.boxed_local()
    })
}

/// One run of `lines` to its end: the exec, and every event the turn emitted.
async fn run_lines(lines: Vec<String>, prompt: &str) -> (Arc<FakeExec>, Vec<AdapterEvent>) {
    let exec = FakeExec::scripted(lines, StreamOpts::default());
    let adapter = adapter_for(&exec);
    let (events, on_event) = collect();
    adapter.start(go(prompt, on_event)).unwrap().finished().await;
    let all = snap(&events);
    (exec, all)
}

async fn replay(lines: Vec<String>) -> Vec<AdapterEvent> {
    run_lines(lines, "fan out").await.1
}

fn text_deltas_of(events: &[AdapterEvent]) -> Vec<TurnDelta> {
    deltas(events)
}

fn tokens(input: u64, output: u64, cached: u64, cache_write: u64) -> UsageTokens {
    UsageTokens { input: JsNumber(input as f64), output: JsNumber(output as f64), cached: JsNumber(cached as f64), cache_write: JsNumber(cache_write as f64), reasoning: JsNumber(0.0) }
}

fn spent(tool_uses: i64, duration_ms: Option<f64>) -> SubagentUsage {
    SubagentUsage { tokens: None, tool_uses: Some(tool_uses.into()), duration_ms: duration_ms.map(JsNumber) }
}

/// A subagent event, every optional field unset until a builder call sets it.
struct Sub {
    session: String,
    task: String,
    parent: String,
    state: SubagentState,
    title: Option<String>,
    summary: Option<String>,
    depth: Option<u32>,
    model: Option<String>,
    asked: Option<String>,
    usage: Option<SubagentUsage>,
}

fn sub(task: &str, parent: &str, state: SubagentState) -> Sub {
    Sub { session: SID.to_string(), task: task.to_string(), parent: parent.to_string(), state, title: None, summary: None, depth: None, model: None, asked: None, usage: None }
}

impl Sub {
    fn session(mut self, s: &str) -> Self {
        self.session = s.to_string();
        self
    }
    fn title(mut self, s: &str) -> Self {
        self.title = Some(s.to_string());
        self
    }
    fn depth(mut self, d: u32) -> Self {
        self.depth = Some(d);
        self
    }
    fn model(mut self, s: &str) -> Self {
        self.model = Some(s.to_string());
        self
    }
    fn asked(mut self, s: &str) -> Self {
        self.asked = Some(s.to_string());
        self
    }
    fn summary(mut self, s: &str) -> Self {
        self.summary = Some(s.to_string());
        self
    }
    fn usage(mut self, u: SubagentUsage) -> Self {
        self.usage = Some(u);
        self
    }
    fn ev(self) -> AdapterEvent {
        AdapterEvent::Subagent { session_id: self.session, task: self.task, state: self.state, parent_tool_use_id: Some(self.parent), title: self.title, summary: self.summary, depth: self.depth, model: self.model, asked: self.asked, usage: self.usage }
    }
}

fn subagent_events(events: &[AdapterEvent]) -> Vec<AdapterEvent> {
    events.iter().filter(|e| matches!(e, AdapterEvent::Subagent { .. })).cloned().collect()
}

/// subagent-fixture.ts: the launching calls and handles read off the recorded frames.
struct SubagentFixture {
    lines: Vec<String>,
    a_call: String,
    b_call: String,
    a_id: String,
    b_id: String,
    a_prompt: String,
    b_prompt: String,
}

fn subagent_fixture() -> SubagentFixture {
    let lines = fixture_lines("subagent-stream.jsonl");
    let frames: Vec<Value> = lines.iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    let launches: Vec<&Value> = frames.iter().filter(|e| e["type"] == "system" && e["subtype"] == "task_started").collect();
    let field = |n: usize, key: &str| launches[n][key].as_str().unwrap().to_string();
    let prompt_of = |call: &str| -> String {
        let blocks: Vec<&Value> = frames.iter().flat_map(|e| e["message"]["content"].as_array().into_iter().flatten()).collect();
        blocks.iter().find(|b| b["id"] == call).unwrap()["input"]["prompt"].as_str().unwrap().to_string()
    };
    let (a_call, b_call) = (field(0, "tool_use_id"), field(1, "tool_use_id"));
    SubagentFixture { a_id: field(0, "task_id"), b_id: field(1, "task_id"), a_prompt: prompt_of(&a_call), b_prompt: prompt_of(&b_call), a_call, b_call, lines }
}

fn user_line(text: &str, uuid: &str) -> String {
    user_message_line(text, SID, &[], Some(uuid))
}

fn written_uuid(line: &str) -> String {
    parse_json(line)["uuid"].as_str().unwrap().to_string()
}

/// A steer running on its own task, as the TS promise runs from the call.
struct Steering {
    answer: Arc<Mutex<Option<SteerOutcome>>>,
    task: tokio::task::JoinHandle<()>,
}

fn steering(session: &ClaudeSession, prompt: &'static str) -> Steering {
    let answer: Arc<Mutex<Option<SteerOutcome>>> = Default::default();
    let (s, cell) = (session.clone(), answer.clone());
    let task = tokio::task::spawn_local(async move {
        let outcome = s.steer(prompt, None, vec![]).await.expect("steer");
        *cell.lock().unwrap() = Some(outcome);
    });
    Steering { answer, task }
}

impl Steering {
    fn answer(&self) -> Option<SteerOutcome> {
        *self.answer.lock().unwrap()
    }
    async fn done(self) -> SteerOutcome {
        self.task.await.unwrap();
        let outcome = *self.answer.lock().unwrap();
        outcome.expect("steer answered")
    }
}

fn steer_result_line() -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "result": "ok", "duration_ms": 5, "session_id": SID}).to_string()
}

fn finish_result_line() -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "result": "ok", "duration_ms": 5, "usage": {"output_tokens": 3}, "session_id": SID}).to_string()
}

// ---------------------------------------------------------------- ClaudeAdapter over the recorded fixture

// TS: adapter.test.ts:158


// TS: adapter.test.ts:612
const BG: &str = "toolu_01WspFixHandBg";

const BG_REPORT: &str = "2 + 2 is 4.\n\n3 + 3 is 6.";

const BG_SID: &str = "21921000-0000-4aaa-8bbb-000000000001";

const PLAIN_FG: &str = "2 + 2 is 4, and 3 + 3 is 6.\n\nBoth are basic addition facts, so no tools were needed to answer.";

fn answer(session: &str, call: &str, text: &str) -> TurnDelta {
    TurnDelta { tool_use_id: Some(call.to_string()), is_error: Some(false), ..TurnDelta::new(session, DeltaKind::ToolResult, text) }
}

fn answers(events: &[AdapterEvent], call: &str) -> Vec<TurnDelta> {
    deltas(events).into_iter().filter(|d| d.kind == DeltaKind::ToolResult && d.tool_use_id.as_deref() == Some(call) && d.parent_tool_use_id.is_none()).collect()
}

fn done_summaries(events: &[AdapterEvent]) -> Vec<(SubagentState, Option<String>)> {
    events
        .iter()
        .filter_map(|e| match e {
            AdapterEvent::Subagent { state, summary, .. } if *state != SubagentState::Running => Some((*state, summary.clone())),
            _ => None,
        })
        .collect()
}

/// Every line the fold of one launching call reads: the call's own answers and its subagent's lines.
fn fold_of(events: &[AdapterEvent], call: &str) -> Vec<TurnDelta> {
    deltas(events).into_iter().filter(|d| d.tool_use_id.as_deref() == Some(call) || d.parent_tool_use_id.as_deref() == Some(call)).collect()
}

fn handback_result(f: &Value) -> bool {
    f["type"] == "user" && f.to_string().contains("toolu_01WspFixHandBgCall")
}

fn is_notification(f: &Value) -> bool {
    f["subtype"] == "task_notification"
}

fn kind_text(ds: Vec<TurnDelta>) -> Vec<(DeltaKind, String)> {
    ds.into_iter().map(|d| (d.kind, d.text)).collect()
}

fn not_internal(ds: Vec<TurnDelta>) -> Vec<TurnDelta> {
    ds.into_iter().filter(|d| !internal_tool_result(&d.text)).collect()
}

fn recorded(file: &str) -> Vec<String> {
    fixture_lines(file)
}

fn same(f: Value) -> Value {
    f
}

fn variant(change: impl Fn(Value) -> Vec<Value>) -> Vec<String> {
    recorded("subagent-handback.jsonl").into_iter().flat_map(|l| change(serde_json::from_str(&l).unwrap())).map(|f| f.to_string()).collect()
}

fn with_init(file: &str, change: Value, more: &dyn Fn(Value) -> Value) -> Vec<String> {
    recorded(file)
        .iter()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .map(|mut f| {
            if f["subtype"] == "init" {
                for (k, v) in change.as_object().unwrap() {
                    f[k.as_str()] = v.clone();
                }
            }
            more(f)
        })
        .map(|f| f.to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaves_a_hand_back_the_cli_refused_as_the_failed_call_it_was() {
    tokio::task::LocalSet::new().run_until(leaves_a_hand_back_the_cli_refused_as_the_failed_call_it_was_body()).await
}

async fn leaves_a_hand_back_the_cli_refused_as_the_failed_call_it_was_body() {
    let refused = "Nothing was sent: the agent that spawned you is no longer running.";
    let events = replay(variant(|mut f| {
        if handback_result(&f) {
            *f.pointer_mut("/message/content/0/content/0/text").unwrap() = json!(json!({"success": false, "message": refused}).to_string());
            return vec![f];
        }
        if !is_notification(&f) {
            return vec![f];
        }
        let rest: serde_json::Map<String, Value> = f.as_object().unwrap().iter().filter(|(k, _)| k.as_str() != "handback_report").map(|(k, v)| (k.clone(), v.clone())).collect();
        let mut rest = Value::Object(rest);
        rest["handback"] = json!("withheld");
        rest["summary"] = json!("The subagent ended without delivering a report through SubagentHandback, so no report was delivered. Its unsent text is not shown. Send the agent a message (SendMessage) to ask it to deliver its report.\n");
        vec![rest]
    }))
    .await;
    let fold = fold_of(&events, BG);
    let rows: Vec<(DeltaKind, Option<String>, Option<String>, Option<bool>)> = fold.iter().filter(|d| d.parent_tool_use_id.as_deref() == Some(BG)).map(|d| (d.kind, d.tool_name.clone(), if d.kind == DeltaKind::ToolResult { Some(d.text.clone()) } else { None }, d.is_error)).collect();
    assert_eq!(
        rows,
        vec![
            (DeltaKind::Thinking, None, None, None),
            (DeltaKind::ToolUse, Some("SubagentHandback".to_string()), None, None),
            (DeltaKind::ToolResult, None, Some(refused.to_string()), Some(true)),
            (DeltaKind::Note, None, None, None),
        ]
    );
    assert_eq!(not_internal(answers(&events, BG)), vec![answer(BG_SID, BG, NO_REPORT_LINE)]);
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(NO_REPORT_LINE.to_string()))]);
}

// TS: adapter.test.ts:635
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ends_a_withheld_subagent_it_waited_on_in_wsps_words() {
    tokio::task::LocalSet::new().run_until(ends_a_withheld_subagent_it_waited_on_in_wsps_words_body()).await
}

async fn ends_a_withheld_subagent_it_waited_on_in_wsps_words_body() {
    let call = "toolu_01WspFixWh1";
    let sid = "21921000-0000-4aaa-8bbb-000000000003";
    let events = replay(recorded("subagent-handback-withheld.jsonl")).await;
    assert_eq!(answers(&events, call), vec![answer(sid, call, NO_REPORT_LINE)]);
    assert_eq!(fold_of(&events, call).last().unwrap(), &answer(sid, call, NO_REPORT_LINE));
    assert!(!deltas(&events).iter().any(|d| d.text.contains("SendMessage")));
    let last_own = fold_of(&events, call).into_iter().rfind(|d| d.parent_tool_use_id.as_deref() == Some(call)).unwrap();
    assert_eq!(last_own.kind, DeltaKind::Note);
    assert_eq!(last_own.text, NO_REPORT_LINE);
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(NO_REPORT_LINE.to_string()))]);
}

// TS: adapter.test.ts:646
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn answers_a_launch_made_inside_a_subagent_with_the_report() {
    tokio::task::LocalSet::new().run_until(answers_a_launch_made_inside_a_subagent_with_the_report_body()).await
}

async fn answers_a_launch_made_inside_a_subagent_with_the_report_body() {
    let (outer, inner) = ("toolu_01WspFixNe1", "toolu_01WspFixNe2");
    let sid = "21921000-0000-4aaa-8bbb-000000000005";
    let outer_report = "I launched one general-purpose subagent (model haiku) with the prompt \"What is 2+2? Answer in one short sentence, no tools.\" It finished and reported: \"2 + 2 = 4.\"";
    let events = replay(recorded("subagent-handback-nested.jsonl")).await;
    let on_page: Vec<String> = deltas(&events).into_iter().filter(|d| d.kind == DeltaKind::ToolResult && d.tool_use_id.as_deref() == Some(inner) && d.parent_tool_use_id.as_deref() == Some(outer)).map(|d| d.text).collect();
    assert_eq!(on_page, vec!["2 + 2 = 4.".to_string()]);
    assert_eq!(answers(&events, inner), vec![answer(sid, inner, "2 + 2 = 4.")]);
    let own: Vec<TurnDelta> = fold_of(&events, inner).into_iter().filter(|d| d.parent_tool_use_id.as_deref() == Some(inner)).collect();
    assert_eq!(kind_text(own), vec![(DeltaKind::Thinking, String::new()), (DeltaKind::Text, "2 + 2 = 4.".to_string())]);
    assert_eq!(fold_of(&events, outer).last().unwrap(), &answer(sid, outer, outer_report));
    assert!(!deltas(&events).iter().any(|d| d.text.contains("delivered to you as a message")));
}

// TS: adapter.test.ts:659
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn says_a_report_once_when_the_subagent_wrote_it_as_text_then_handed_it_back() {
    tokio::task::LocalSet::new().run_until(says_a_report_once_when_the_subagent_wrote_it_as_text_then_handed_it_back_body()).await
}

async fn says_a_report_once_when_the_subagent_wrote_it_as_text_then_handed_it_back_body() {
    let call = "toolu_01WspFixTf1";
    let events = replay(recorded("subagent-handback-text-first.jsonl")).await;
    let texts: Vec<String> = fold_of(&events, call).into_iter().filter(|d| d.parent_tool_use_id.as_deref() == Some(call) && d.kind == DeltaKind::Text).map(|d| d.text).collect();
    assert_eq!(texts, vec!["pong".to_string()]);
    assert_eq!(fold_of(&events, call).last().unwrap(), &answer("21921000-0000-4aaa-8bbb-000000000004", call, "pong"));
}

// TS: adapter.test.ts:666
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lets_a_later_notification_replace_a_subagents_earlier_end() {
    tokio::task::LocalSet::new().run_until(lets_a_later_notification_replace_a_subagents_earlier_end_body()).await
}

async fn lets_a_later_notification_replace_a_subagents_earlier_end_body() {
    let again = "2 + 2 is 4.\n\n3 + 3 is 6.\n\nChecked twice.";
    let events = replay(variant(|f| {
        if is_notification(&f) {
            let mut second = f.clone();
            second["handback_report"] = json!({"text": again});
            vec![f, second]
        } else {
            vec![f]
        }
    }))
    .await;
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(BG_REPORT.to_string())), (SubagentState::Done, Some(again.to_string()))]);
    assert_eq!(not_internal(answers(&events, BG)).last().unwrap(), &answer(BG_SID, BG, again));
}

// TS: adapter.test.ts:679
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ends_a_subagent_it_waited_on_in_auto_mode_with_the_hand_back_flag_off() {
    tokio::task::LocalSet::new().run_until(ends_a_subagent_it_waited_on_in_auto_mode_with_the_hand_back_flag_off_body()).await
}

async fn ends_a_subagent_it_waited_on_in_auto_mode_with_the_hand_back_flag_off_body() {
    let call = "toolu_01WspFixPf1";
    let events = replay(with_init("subagent-bypass-foreground.jsonl", json!({"permissionMode": "auto"}), &same)).await;
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(PLAIN_FG.to_string()))]);
    assert_eq!(answers(&events, call), vec![answer("21921000-0000-4aaa-8bbb-000000000007", call, PLAIN_FG)]);
}

// TS: adapter.test.ts:686
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ends_a_backgrounded_subagent_started_after_a_switch_away_from_auto_mode() {
    tokio::task::LocalSet::new().run_until(ends_a_backgrounded_subagent_started_after_a_switch_away_from_auto_mode_body()).await
}

async fn ends_a_backgrounded_subagent_started_after_a_switch_away_from_auto_mode_body() {
    let call = "toolu_01WspFixPb1";
    let events = replay(with_init("subagent-bypass-background.jsonl", json!({"permissionMode": "auto"}), &same)).await;
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(BG_REPORT.to_string()))]);
    assert_eq!(not_internal(answers(&events, call)), vec![answer("21921000-0000-4aaa-8bbb-000000000006", call, BG_REPORT)]);
}

// TS: adapter.test.ts:693
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ends_a_fork_which_never_hands_back_in_an_auto_run() {
    tokio::task::LocalSet::new().run_until(ends_a_fork_which_never_hands_back_in_an_auto_run_body()).await
}

async fn ends_a_fork_which_never_hands_back_in_an_auto_run_body() {
    let call = "toolu_01WspFixPb1";
    // The TS test spells these with a space after each colon, so against its compact re-stringified frames the rewrite
    // never happened (spec/claude_turn.md rule T13). Here it rewrites the call and its task_started line, as meant.
    let fork = |f: Value| -> Value {
        let text = f.to_string();
        if text.contains(r#""name":"Agent""#) || f["subtype"] == "task_started" {
            serde_json::from_str(&text.replace(r#""subagent_type":"general-purpose""#, r#""subagent_type":"fork""#)).unwrap()
        } else {
            f
        }
    };
    let lines = with_init("subagent-bypass-background.jsonl", json!({"permissionMode": "auto"}), &fork);
    assert_eq!(lines.iter().filter(|l| l.contains(r#""subagent_type":"fork""#)).count(), 2, "the call and its task_started line name a fork");
    let events = replay(lines).await;
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(BG_REPORT.to_string()))]);
    assert_eq!(not_internal(answers(&events, call)), vec![answer("21921000-0000-4aaa-8bbb-000000000006", call, BG_REPORT)]);
    let plain = replay(with_init("subagent-bypass-background.jsonl", json!({"permissionMode": "auto"}), &same)).await;
    let unforked: Vec<AdapterEvent> = events
        .into_iter()
        .map(|e| match e {
            AdapterEvent::TurnDelta(mut d) => {
                d.text = d.text.replace(r#""subagent_type":"fork""#, r#""subagent_type":"general-purpose""#);
                AdapterEvent::TurnDelta(d)
            }
            other => other,
        })
        .collect();
    assert_eq!(unforked, plain, "the adapter reads no subagent_type: a fork differs only in its call's drawn input");
}

// TS: adapter.test.ts:701
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn holds_a_subagents_notification_while_a_command_it_started_runs_and_answers_once() {
    tokio::task::LocalSet::new().run_until(holds_a_subagents_notification_while_a_command_it_started_runs_and_answers_once_body()).await
}

async fn holds_a_subagents_notification_while_a_command_it_started_runs_and_answers_once_body() {
    let call = "toolu_01WspFixOw1";
    let sid = "21921000-0000-4aaa-8bbb-000000000008";
    let report = "The background command `sleep 3; echo hi` ran with run_in_background: true and completed with exit code 0. It printed: hi";
    let waiting = "This agent has not reported yet: it is waiting on its own background work and will deliver its report through SubagentHandback when that finishes.\n";
    let lines: Vec<String> = with_init("subagent-handback-own-work.jsonl", json!({"permissionMode": "default"}), &same)
        .into_iter()
        .flat_map(|line| {
            let f: Value = serde_json::from_str(&line).unwrap();
            if f["subtype"] != "task_started" || f["task_type"] != "local_bash" {
                return vec![line];
            }
            vec![line, json!({"type": "system", "subtype": "task_notification", "task_id": "adaddd87fd4862bf8", "tool_use_id": call, "status": "completed", "summary": waiting, "session_id": sid}).to_string()]
        })
        .collect();
    let events = replay(lines).await;
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(report.to_string()))]);
    assert_eq!(not_internal(answers(&events, call)), vec![answer(sid, call, report)]);
}

// TS: adapter.test.ts:717
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaves_a_background_command_alone_in_auto_mode() {
    tokio::task::LocalSet::new().run_until(leaves_a_background_command_alone_in_auto_mode_body()).await
}

async fn leaves_a_background_command_alone_in_auto_mode_body() {
    let call = "toolu_01WspFixBc1";
    let recorded_lines = recorded("background-command-auto.jsonl");
    let woken = recorded_lines.iter().enumerate().position(|(i, line)| i > 1 && serde_json::from_str::<Value>(line).unwrap()["subtype"] == "init").expect("a second init");
    let finished = regex::Regex::new("^started\n\n`Sleep 4 seconds then echo done` completed, \\d+m?s after the reply$").unwrap();
    for mode in ["auto", "default"] {
        let lines: Vec<String> = with_init("background-command-auto.jsonl", json!({"permissionMode": mode}), &same).into_iter().take(woken).collect();
        let events = replay(lines).await;
        let firsts: Vec<String> = answers(&events, call).iter().map(|d| d.text.split('.').next().unwrap().to_string()).collect();
        assert_eq!(firsts, vec!["Command running in background with ID: bnf8k15go".to_string()], "{mode}");
        let texts: Vec<Option<String>> = events
            .iter()
            .filter_map(|e| match e {
                AdapterEvent::TurnDone { result, .. } => Some(result.text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 1, "{mode}");
        let text = texts[0].clone().expect("the reply's text");
        assert!(finished.is_match(&text), "{mode}: {text:?}");
        assert_eq!(subagent_events(&events), vec![], "{mode}");
    }
}

// TS: adapter.test.ts:732
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn answers_a_launch_the_default_mode_waited_on_with_the_bare_report() {
    tokio::task::LocalSet::new().run_until(answers_a_launch_the_default_mode_waited_on_with_the_bare_report_body()).await
}

async fn answers_a_launch_the_default_mode_waited_on_with_the_bare_report_body() {
    let call = "toolu_01WspFixPf1";
    let events = replay(recorded("subagent-bypass-foreground.jsonl")).await;
    assert_eq!(answers(&events, call), vec![answer("21921000-0000-4aaa-8bbb-000000000007", call, PLAIN_FG)]);
}

// TS: adapter.test.ts:739
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ends_a_backgrounded_subagent_of_the_default_mode_on_its_report() {
    tokio::task::LocalSet::new().run_until(ends_a_backgrounded_subagent_of_the_default_mode_on_its_report_body()).await
}

async fn ends_a_backgrounded_subagent_of_the_default_mode_on_its_report_body() {
    let call = "toolu_01WspFixPb1";
    let events = replay(recorded("subagent-bypass-background.jsonl")).await;
    assert_eq!(not_internal(answers(&events, call)), vec![answer("21921000-0000-4aaa-8bbb-000000000006", call, BG_REPORT)]);
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(BG_REPORT.to_string()))]);
}

// ---------------------------------------------------------------- the rest of ClaudeAdapter over the recorded fixture

// TS: adapter.test.ts:747
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forwards_init_slash_commands_permission_mode_and_agents_as_harness() {
    tokio::task::LocalSet::new().run_until(forwards_init_slash_commands_permission_mode_and_agents_as_harness_body()).await
}

async fn forwards_init_slash_commands_permission_mode_and_agents_as_harness_body() {
    let (_, events) = run_lines(stream_lines(), "write a hello world server").await;
    match &events[0] {
        AdapterEvent::SessionStart { harness, .. } => assert_eq!(
            harness,
            &Some(SessionHarness { slash_commands: Some(["compact", "context", "cost", "init", "review"].map(String::from).to_vec()), permission_mode: Some("bypassPermissions".into()), agents: Some(vec!["general-purpose".into()]) })
        ),
        other => panic!("expected session.start, got {other:?}"),
    }
}

// TS: adapter.test.ts:763
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaves_harness_unset_when_init_carries_none_of_its_fields() {
    tokio::task::LocalSet::new().run_until(leaves_harness_unset_when_init_carries_none_of_its_fields_body()).await
}

async fn leaves_harness_unset_when_init_carries_none_of_its_fields_body() {
    let (_, events) = run_lines(vec![init_line()], "x").await;
    match &events[0] {
        AdapterEvent::SessionStart { harness, .. } => assert_eq!(harness, &None),
        other => panic!("expected session.start, got {other:?}"),
    }
    assert!(serde_json::to_value(&events[0]).unwrap().get("harness").is_none());
}

// TS: adapter.test.ts:777
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawns_with_the_landmine_safe_command_and_env() {
    tokio::task::LocalSet::new().run_until(spawns_with_the_landmine_safe_command_and_env_body()).await
}

async fn spawns_with_the_landmine_safe_command_and_env_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.base_env = Some(guest_env_with(&[("CLAUDECODE", "1"), ("CLAUDE_CODE_ENTRYPOINT", "cli"), ("PATH", "/usr/bin"), ("HOME", "/Users/z")]));
    let adapter = adapter_with(deps);
    let session = adapter.start(go("say ok", noop())).unwrap();
    session.finished().await;

    let call = exec.calls()[0].clone();
    for flag in ["--output-format stream-json", "--input-format stream-json", "--verbose", "--dangerously-skip-permissions"] {
        assert!(call.command.contains(flag), "{flag}: {}", call.command);
    }
    assert!(call.command.contains(&format!("--session-id {}", session.local_id())), "{}", call.command);
    assert!(!call.command.contains("</dev/null"));
    assert!(!call.command.contains("say ok"));
    assert_eq!(call.input, Some(vec![user_message_line("say ok", &session.local_id(), &[], None)]));
    assert_eq!(env_of(&call.env, "CLAUDECODE"), None);
    assert_eq!(env_of(&call.env, "CLAUDE_CODE_ENTRYPOINT"), None);
    assert_eq!(env_of(&call.env, "CLAUDE_CONFIG_DIR"), Some("/root/.claude-cfg"));
    assert_eq!(env_of(&call.env, "IS_SANDBOX"), Some("1"));
    assert_eq!(env_of(&call.env, "HOME"), Some("/Users/z"));
}

// TS: adapter.test.ts:807
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn with_prompt_after_starts_the_cli_at_once_and_holds_the_seed() {
    tokio::task::LocalSet::new().run_until(with_prompt_after_starts_the_cli_at_once_and_holds_the_seed_body()).await
}

async fn with_prompt_after_starts_the_cli_at_once_and_holds_the_seed_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_for(&exec);
    let (release, released) = futures::channel::oneshot::channel::<()>();
    let prompt_after = async move { released.await.map_err(|e| e.to_string()) }.boxed_local();
    let session = adapter.start(HarnessStartOptions { prompt_after: Some(prompt_after), ..go("say ok", noop()) }).unwrap();
    assert_eq!(exec.calls()[0].input, Some(vec![user_message_line("say ok", &session.local_id(), &[], None)]));
    let held = exec.held.lock().unwrap().pop().expect("the launch held a seed");
    let mut held = held.fuse();
    assert!(futures::poll!(&mut held).is_pending(), "the seed settled before promptAfter did");
    assert_eq!(exec.writes(), Vec::<String>::new());
    assert!(wsp_adapters::port::HarnessAdapter::waits_for_prompt(&adapter));
    exec.end(Some(0));
    session.finished().await;
    release.send(()).unwrap();
    assert_eq!(held.await, Ok(()));
}

// TS: adapter.test.ts:822
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_a_persons_own_computer_exports_no_config_dir_and_no_sandbox_flag() {
    tokio::task::LocalSet::new().run_until(on_a_persons_own_computer_exports_no_config_dir_and_no_sandbox_flag_body()).await
}

async fn on_a_persons_own_computer_exports_no_config_dir_and_no_sandbox_flag_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let mut deps = AdapterDeps::new(exec.clone(), "/Users/z/.claude");
    deps.base_env = Some(base_env(&[("PATH", "/usr/bin"), ("HOME", "/Users/z")]));
    let adapter = adapter_with(deps);
    adapter.start(go("say ok", noop())).unwrap().finished().await;
    let call = exec.calls()[0].clone();
    assert_eq!(env_of(&call.env, "CLAUDE_CONFIG_DIR"), None);
    assert_eq!(env_of(&call.env, "IS_SANDBOX"), None);
    assert_eq!(env_of(&adapter.env(), "CLAUDE_CONFIG_DIR"), None);
    let ran: Arc<Mutex<Vec<(String, Option<Env>)>>> = Default::default();
    adapter.session_title(SID, recording_exec(ran.clone(), String::new())).await.unwrap();
    adapter.probe_catalog(recording_exec(ran.clone(), String::new())).await.unwrap();
    let ran = ran.lock().unwrap().clone();
    assert!(ran[0].0.contains("'/Users/z/.claude/projects'"), "{}", ran[0].0);
    assert!(!ran[1].0.contains("CLAUDE_CONFIG_DIR"), "{}", ran[1].0);
    assert!(!ran[1].0.contains("IS_SANDBOX"), "{}", ran[1].0);
}

// TS: adapter.test.ts:842
#[test]
fn refuses_a_relative_config_dir() {
    let exec = FakeExec::scripted(vec![], StreamOpts::default());
    match create_claude_adapter(AdapterDeps::new(exec, ".claude-cfg")) {
        Err(e) => assert!(e.0.contains("absolute"), "{}", e.0),
        Ok(_) => panic!("a relative config dir was accepted"),
    }
}

// TS: adapter.test.ts:846
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_a_persons_own_computer_whose_shell_names_a_store_exports_theirs() {
    tokio::task::LocalSet::new().run_until(on_a_persons_own_computer_whose_shell_names_a_store_exports_theirs_body()).await
}

async fn on_a_persons_own_computer_whose_shell_names_a_store_exports_theirs_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let mut deps = AdapterDeps::new(exec.clone(), "/Users/z/elsewhere");
    deps.base_env = Some(base_env(&[("PATH", "/usr/bin"), ("HOME", "/Users/z"), ("CLAUDE_CONFIG_DIR", "/Users/z/elsewhere")]));
    adapter_with(deps).start(go("say ok", noop())).unwrap().finished().await;
    let call = exec.calls()[0].clone();
    assert_eq!(env_of(&call.env, "CLAUDE_CONFIG_DIR"), Some("/Users/z/elsewhere"));
    assert_eq!(env_of(&call.env, "IS_SANDBOX"), None);
}

// TS: adapter.test.ts:855
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn starts_claude_in_the_guest_home_unless_a_cwd_is_named() {
    tokio::task::LocalSet::new().run_until(starts_claude_in_the_guest_home_unless_a_cwd_is_named_body()).await
}

async fn starts_claude_in_the_guest_home_unless_a_cwd_is_named_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let adapter = adapter_for(&exec);
    adapter.start(go("say ok", noop())).unwrap().finished().await;
    adapter.start(HarnessStartOptions { cwd: Some("/root/app".into()), ..go("say ok", noop()) }).unwrap().finished().await;
    let calls = exec.calls();
    assert!(calls[0].command.starts_with(&format!("cd ~ && {} -p", agent_word("claude", None))), "{}", calls[0].command);
    assert!(calls[1].command.starts_with(&format!("cd '/root/app' && {} -p", agent_word("claude", None))), "{}", calls[1].command);
}

// TS: adapter.test.ts:867
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_generates_the_session_uuid_and_registers_the_session_before_any_output() {
    tokio::task::LocalSet::new().run_until(self_generates_the_session_uuid_and_registers_the_session_before_any_output_body()).await
}

async fn self_generates_the_session_uuid_and_registers_the_session_before_any_output_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let adapter = adapter_for(&exec);
    let session = adapter.start(go("say ok", noop())).unwrap();
    let id = session.local_id();
    assert!(regex::Regex::new("^[0-9a-f-]{36}$").unwrap().is_match(&id), "{id}");
    let registered = adapter.sessions();
    assert!(registered.get(&id).is_some_and(|s| s.same(&session)), "the registry holds this very session");
}

// TS: adapter.test.ts:878
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumes_with_the_prior_session_id_as_the_registry_key() {
    tokio::task::LocalSet::new().run_until(resumes_with_the_prior_session_id_as_the_registry_key_body()).await
}

async fn resumes_with_the_prior_session_id_as_the_registry_key_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let adapter = adapter_for(&exec);
    let session = adapter.start(HarnessStartOptions { resume: Some(SID.into()), ..go("next turn", noop()) }).unwrap();
    session.finished().await;
    assert_eq!(session.local_id(), SID);
    let command = exec.calls()[0].command.clone();
    assert!(command.contains(&format!("--resume {SID}")), "{command}");
}

// ---------------------------------------------------------------- interrupt policy

// TS: adapter.test.ts:892
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn escalates_to_kill_when_teardown_does_not_end_the_process() {
    tokio::task::LocalSet::new().run_until(escalates_to_kill_when_teardown_does_not_end_the_process_body()).await
}

async fn escalates_to_kill_when_teardown_does_not_end_the_process_body() {
    let exec = FakeExec::scripted(vec![init_line()], StreamOpts { hang: true, ..Default::default() });
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.interrupt_grace_ms = Some(15);
    let adapter = adapter_with(deps);
    let (events, on_event) = collect();
    let session = adapter.start(go("loop forever", on_event)).unwrap();
    session.interrupt().await;
    let result = session.finished().await;
    assert_eq!(exec.order(), vec!["teardown", "kill"]);
    assert_eq!(result.status, TurnStatus::Interrupted);
    match snap(&events).last().unwrap() {
        AdapterEvent::SessionEnd { saw_result, exit_code, .. } => {
            assert!(!*saw_result);
            assert_eq!(*exit_code, None);
        }
        other => panic!("expected session.end, got {other:?}"),
    }
}

// TS: adapter.test.ts:914
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn does_not_kill_when_teardown_ends_the_process_within_the_grace_window() {
    tokio::task::LocalSet::new().run_until(does_not_kill_when_teardown_ends_the_process_within_the_grace_window_body()).await
}

async fn does_not_kill_when_teardown_ends_the_process_within_the_grace_window_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.interrupt_grace_ms = Some(5_000);
    let adapter = adapter_with(deps);
    let session = adapter.start(go("say ok", noop())).unwrap();
    session.finished().await;
    session.interrupt().await;
    assert_eq!(exec.order(), vec!["closeInput", "teardown"]);
}

// ---------------------------------------------------------------- the process a finished turn leaves

// TS: adapter.test.ts:935
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cli_that_does_not_go_on_the_eof_is_ended_with_its_tree() {
    tokio::task::LocalSet::new().run_until(a_cli_that_does_not_go_on_the_eof_is_ended_with_its_tree_body()).await
}

async fn a_cli_that_does_not_go_on_the_eof_is_ended_with_its_tree_body() {
    let exec = FakeExec::manual(StreamOpts { slow_teardown: true, ..Default::default() });
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.result_exit_ms = Some(15);
    deps.interrupt_grace_ms = Some(15);
    let adapter = adapter_with(deps);
    let (events, on_event) = collect();
    let session = adapter.start(go("go", on_event)).unwrap();
    exec.push(init_line());
    exec.push(finish_result_line());
    let turn = session.finished().await;
    assert_eq!(exec.order(), vec!["closeInput", "teardown", "kill"]);
    assert_eq!(turn.status, TurnStatus::Completed);
    assert_eq!(turn.text.as_deref(), Some("ok"));
    match snap(&events).last().unwrap() {
        AdapterEvent::SessionEnd { saw_result, .. } => assert!(*saw_result),
        other => panic!("expected session.end, got {other:?}"),
    }
}

// TS: adapter.test.ts:950
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cli_that_goes_on_its_own_is_left_alone() {
    tokio::task::LocalSet::new().run_until(a_cli_that_goes_on_its_own_is_left_alone_body()).await
}

async fn a_cli_that_goes_on_its_own_is_left_alone_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.result_exit_ms = Some(15);
    deps.interrupt_grace_ms = Some(15);
    let adapter = adapter_with(deps);
    let (events, on_event) = collect();
    let session = adapter.start(go("go", on_event)).unwrap();
    exec.push(init_line());
    exec.push(finish_result_line());
    until(|| seen(&events, "turn.done")).await;
    exec.end(Some(0));
    assert_eq!(session.finished().await.status, TurnStatus::Completed);
    pause(60).await;
    assert_eq!(exec.order(), vec!["closeInput"]);
}

// ---------------------------------------------------------------- steer over the stdin channel

// TS: adapter.test.ts:971
#[test]
fn declares_that_its_sessions_take_a_message_mid_turn() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_for(&exec);
    assert!(wsp_adapters::port::HarnessAdapter::steers(&adapter));
}

// TS: adapter.test.ts:975
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn before_init_a_steer_waits_for_it_and_writes_under_the_session_it_names() {
    tokio::task::LocalSet::new().run_until(before_init_a_steer_waits_for_it_and_writes_under_the_session_it_names_body()).await
}

async fn before_init_a_steer_waits_for_it_and_writes_under_the_session_it_names_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_for(&exec);
    let session = adapter.start(go("go", noop())).unwrap();
    let steer = steering(&session, "also this");
    pause(50).await;
    assert_eq!(steer.answer(), None);
    assert_eq!(exec.writes(), Vec::<String>::new());
    exec.push(init_line());
    assert_eq!(steer.done().await, SteerOutcome::Accepted);
    let writes = exec.writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes, vec![user_line("also this", &written_uuid(&writes[0]))]);
    exec.push(steer_result_line());
    exec.end(Some(0));
    session.finished().await;
}

// TS: adapter.test.ts:993
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn before_init_on_a_cli_that_exits_without_one_answers_not_running() {
    tokio::task::LocalSet::new().run_until(before_init_on_a_cli_that_exits_without_one_answers_not_running_body()).await
}

async fn before_init_on_a_cli_that_exits_without_one_answers_not_running_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_for(&exec);
    let session = adapter.start(go("go", noop())).unwrap();
    let steer = steering(&session, "also this");
    exec.end(Some(1));
    assert_eq!(steer.done().await, SteerOutcome::NotRunning);
    assert_eq!(exec.writes(), Vec::<String>::new());
    session.finished().await;
}

// TS: adapter.test.ts:1004
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn before_init_at_a_stop_on_a_cli_whose_stream_does_not_end_answers_not_running() {
    tokio::task::LocalSet::new().run_until(before_init_at_a_stop_on_a_cli_whose_stream_does_not_end_answers_not_running_body()).await
}

async fn before_init_at_a_stop_on_a_cli_whose_stream_does_not_end_answers_not_running_body() {
    let exec = FakeExec::manual(StreamOpts { slow_teardown: true, ..Default::default() });
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.interrupt_grace_ms = Some(60_000);
    let adapter = adapter_with(deps);
    let session = adapter.start(go("go", noop())).unwrap();
    let steer = steering(&session, "also this");
    let stopper = session.clone();
    let _interrupting = tokio::task::spawn_local(async move { stopper.interrupt().await });
    pause(50).await;
    assert_eq!(steer.answer(), Some(SteerOutcome::NotRunning));
    assert_eq!(exec.writes(), Vec::<String>::new());
    exec.end(Some(0));
    assert_eq!(steer.done().await, SteerOutcome::NotRunning);
    session.finished().await;
}

// TS: adapter.test.ts:1019
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn before_init_on_a_cli_that_stays_silent_past_the_bound_answers_not_running() {
    tokio::task::LocalSet::new().run_until(before_init_on_a_cli_that_stays_silent_past_the_bound_answers_not_running_body()).await
}

async fn before_init_on_a_cli_that_stays_silent_past_the_bound_answers_not_running_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.steer_join_ms = Some(100);
    let adapter = adapter_with(deps);
    let session = adapter.start(go("go", noop())).unwrap();
    let steer = steering(&session, "also this");
    pause(50).await;
    assert_eq!(steer.answer(), None);
    assert_eq!(steer.done().await, SteerOutcome::NotRunning);
    assert_eq!(exec.writes(), Vec::<String>::new());
    exec.end(Some(0));
    session.finished().await;
}

// TS: adapter.test.ts:1034
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn between_init_and_result_writes_one_user_line_and_answers_accepted() {
    tokio::task::LocalSet::new().run_until(between_init_and_result_writes_one_user_line_and_answers_accepted_body()).await
}

async fn between_init_and_result_writes_one_user_line_and_answers_accepted_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_for(&exec);
    let (events, on_event) = collect();
    let session = adapter.start(go("go", on_event)).unwrap();
    exec.push(init_line());
    until(|| seen(&events, "session.start")).await;
    assert_eq!(session.steer("also this", None, vec![]).await.unwrap(), SteerOutcome::Accepted);
    let writes = exec.writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes, vec![user_line("also this", &written_uuid(&writes[0]))]);
    assert_eq!(exec.order(), vec!["write"]);
    exec.push(steer_result_line());
    exec.end(Some(0));
    assert_eq!(session.finished().await.status, TurnStatus::Completed);
}

// TS: adapter.test.ts:1049
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn carries_a_steered_messages_images_as_base64_blocks_ahead_of_its_words() {
    tokio::task::LocalSet::new().run_until(carries_a_steered_messages_images_as_base64_blocks_ahead_of_its_words_body()).await
}

async fn carries_a_steered_messages_images_as_base64_blocks_ahead_of_its_words_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_for(&exec);
    assert!(wsp_adapters::port::HarnessAdapter::steers_images(&adapter));
    let (events, on_event) = collect();
    let session = adapter.start(go("go", on_event)).unwrap();
    exec.push(init_line());
    until(|| seen(&events, "session.start")).await;
    let image = TurnImage { media_type: "image/png".into(), bytes: "iVBORw0KGgo=".into(), path: None };
    assert_eq!(session.steer("what is this?", Some("steer-1".into()), vec![image]).await.unwrap(), SteerOutcome::Accepted);
    let line = parse_json(&exec.writes()[0]);
    assert_eq!(line["type"], "user");
    assert_eq!(line["uuid"], "steer-1");
    assert_eq!(line["message"]["role"], "user");
    assert_eq!(line["message"]["content"], json!([{"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBORw0KGgo="}}, {"type": "text", "text": "what is this?"}]));
    exec.push(steer_result_line());
    exec.end(Some(0));
    assert_eq!(session.finished().await.status, TurnStatus::Completed);
}

// TS: adapter.test.ts:1068
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn after_result_answers_not_running_and_result_closed_the_channel_once() {
    tokio::task::LocalSet::new().run_until(after_result_answers_not_running_and_result_closed_the_channel_once_body()).await
}

async fn after_result_answers_not_running_and_result_closed_the_channel_once_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_for(&exec);
    let (events, on_event) = collect();
    let session = adapter.start(go("go", on_event)).unwrap();
    exec.push(init_line());
    exec.push(steer_result_line());
    until(|| seen(&events, "turn.done")).await;
    assert_eq!(exec.order(), vec!["closeInput"]);
    assert_eq!(session.steer("too late", None, vec![]).await.unwrap(), SteerOutcome::NotRunning);
    assert_eq!(exec.writes(), Vec::<String>::new());
    exec.end(Some(0));
    session.finished().await;
    assert_eq!(exec.order(), vec!["closeInput"]);
}

// TS: adapter.test.ts:1084
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn after_the_process_exited_without_a_result_answers_not_running() {
    tokio::task::LocalSet::new().run_until(after_the_process_exited_without_a_result_answers_not_running_body()).await
}

async fn after_the_process_exited_without_a_result_answers_not_running_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_for(&exec);
    let session = adapter.start(go("go", noop())).unwrap();
    exec.push(init_line());
    exec.end(Some(1));
    session.finished().await;
    assert_eq!(session.steer("too late", None, vec![]).await.unwrap(), SteerOutcome::NotRunning);
    assert_eq!(exec.writes(), Vec::<String>::new());
}

// TS: adapter.test.ts:1095
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn when_the_guest_says_the_process_is_already_gone_answers_not_running_and_no_line_landed() {
    tokio::task::LocalSet::new().run_until(when_the_guest_says_the_process_is_already_gone_answers_not_running_and_no_line_landed_body()).await
}

async fn when_the_guest_says_the_process_is_already_gone_answers_not_running_and_no_line_landed_body() {
    let exec = FakeExec::manual(StreamOpts { gone: true, ..Default::default() });
    let adapter = adapter_for(&exec);
    let (events, on_event) = collect();
    let session = adapter.start(go("go", on_event)).unwrap();
    exec.push(init_line());
    until(|| seen(&events, "session.start")).await;
    assert_eq!(session.steer("after exit", None, vec![]).await.unwrap(), SteerOutcome::NotRunning);
    assert_eq!(exec.order(), vec!["write"]);
    assert_eq!(exec.writes(), Vec::<String>::new());
    exec.end(Some(1));
    assert_eq!(session.finished().await.status, TurnStatus::Failed);
}

// TS: adapter.test.ts:1109
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn once_interrupt_was_asked_answers_not_running_and_writes_nothing() {
    tokio::task::LocalSet::new().run_until(once_interrupt_was_asked_answers_not_running_and_writes_nothing_body()).await
}

async fn once_interrupt_was_asked_answers_not_running_and_writes_nothing_body() {
    let exec = FakeExec::manual(StreamOpts { slow_teardown: true, ..Default::default() });
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.interrupt_grace_ms = Some(30);
    let adapter = adapter_with(deps);
    let (events, on_event) = collect();
    let session = adapter.start(go("go", on_event)).unwrap();
    exec.push(init_line());
    until(|| seen(&events, "session.start")).await;
    let stopper = session.clone();
    let interrupting = tokio::task::spawn_local(async move { stopper.interrupt().await });
    until(|| exec.order() == vec!["teardown"]).await;
    assert_eq!(session.steer("into a stopping turn", None, vec![]).await.unwrap(), SteerOutcome::NotRunning);
    assert_eq!(exec.writes(), Vec::<String>::new());
    interrupting.await.unwrap();
    assert_eq!(exec.order(), vec!["teardown", "kill"]);
    assert_eq!(session.finished().await.status, TurnStatus::Interrupted);
}

// TS: adapter.test.ts:1125
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_write_that_lands_after_the_turn_ended_answers_not_running() {
    tokio::task::LocalSet::new().run_until(a_write_that_lands_after_the_turn_ended_answers_not_running_body()).await
}

async fn a_write_that_lands_after_the_turn_ended_answers_not_running_body() {
    let (events, on_event) = collect();
    let feed: Arc<Mutex<Option<Arc<FakeExec>>>> = Default::default();
    let (feed_in, events_in) = (feed.clone(), events.clone());
    let before_write: Arc<dyn Fn() -> LocalBoxFuture<'static, ()>> = Arc::new(move || {
        let (feed, events) = (feed_in.clone(), events_in.clone());
        async move {
            let turn_ends = feed.lock().unwrap().clone();
            if let Some(exec) = turn_ends {
                exec.push(steer_result_line());
            }
            until(|| seen(&events, "turn.done")).await;
        }
        .boxed_local()
    });
    let exec = FakeExec::manual(StreamOpts { before_write: Some(before_write), ..Default::default() });
    let adapter = adapter_for(&exec);
    let session = adapter.start(go("go", on_event)).unwrap();
    exec.push(init_line());
    until(|| seen(&events, "session.start")).await;
    *feed.lock().unwrap() = Some(exec.clone());
    assert_eq!(session.steer("racing", None, vec![]).await.unwrap(), SteerOutcome::NotRunning);
    let writes = exec.writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes, vec![user_line("racing", &written_uuid(&writes[0]))]);
    exec.end(Some(0));
    session.finished().await;
}

// TS: adapter.test.ts:1145
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_result_read_while_a_steers_write_travels_does_not_end_the_turn_under_it() {
    tokio::task::LocalSet::new().run_until(a_result_read_while_a_steers_write_travels_does_not_end_the_turn_under_it_body()).await
}

async fn a_result_read_while_a_steers_write_travels_does_not_end_the_turn_under_it_body() {
    let (events, on_event) = collect();
    let feed: Arc<Mutex<Option<Arc<FakeExec>>>> = Default::default();
    let feed_in = feed.clone();
    let before_write: Arc<dyn Fn() -> LocalBoxFuture<'static, ()>> = Arc::new(move || {
        let feed = feed_in.clone();
        async move {
            let result_lands = feed.lock().unwrap().clone();
            if let Some(exec) = result_lands {
                exec.push(steer_result_line());
            }
            pause(20).await;
        }
        .boxed_local()
    });
    let exec = FakeExec::manual(StreamOpts { before_write: Some(before_write), ..Default::default() });
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.result_exit_ms = Some(60_000);
    let adapter = adapter_with(deps);
    let session = adapter.start(go("go", on_event)).unwrap();
    exec.push(json!({"type": "system", "subtype": "init", "session_id": SID, "capabilities": ["msg_lifecycle_v1"]}).to_string());
    until(|| seen(&events, "session.start")).await;
    *feed.lock().unwrap() = Some(exec.clone());
    assert_eq!(session.steer("racing", None, vec![]).await.unwrap(), SteerOutcome::Accepted);
    assert!(!seen(&events, "turn.done"));
    let uuid = written_uuid(&exec.writes()[0]);
    let lifecycle = |state: &str| json!({"type": "command_lifecycle", "command_uuid": uuid, "state": state, "session_id": SID}).to_string();
    for line in [lifecycle("queued"), lifecycle("started"), init_line(), steer_result_line().replace("\"result\":\"ok\"", "\"result\":\"raced\""), lifecycle("completed")] {
        exec.push(line);
    }
    until(|| seen(&events, "turn.done")).await;
    exec.end(Some(0));
    let turn = session.finished().await;
    assert_eq!(turn.status, TurnStatus::Completed);
    assert_eq!(turn.text.as_deref(), Some("raced"));
}

// ---------------------------------------------------------------- a reply while the process keeps running

// TS: adapter.test.ts:1171
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_the_reply_at_the_result_but_leaves_the_turn_open_until_the_process_exits() {
    tokio::task::LocalSet::new().run_until(emits_the_reply_at_the_result_but_leaves_the_turn_open_until_the_process_exits_body()).await
}

async fn emits_the_reply_at_the_result_but_leaves_the_turn_open_until_the_process_exits_body() {
    let m = FakeExec::manual(StreamOpts::default());
    let adapter = adapter_for(&m);
    let (events, on_event) = collect();
    let session = adapter.start(go("x", on_event)).unwrap();
    let settled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (flag, finished) = (settled.clone(), session.finished());
    tokio::task::spawn_local(async move {
        finished.await;
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let is_settled = || settled.load(std::sync::atomic::Ordering::SeqCst);

    m.push(init_line());
    m.push(json!({"type": "result", "subtype": "success", "result": "the reply", "duration_ms": 1000, "usage": {"output_tokens": 5}, "session_id": SID}).to_string());
    until(|| seen(&events, "turn.done")).await;
    assert!(!is_settled());

    m.push(json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "still tidying up"}]}, "session_id": SID}).to_string());
    until(|| text_deltas_of(&snap(&events)).iter().any(|d| d.text == "still tidying up")).await;
    assert!(!is_settled());

    m.end(Some(0));
    let result = session.finished().await;
    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.text.as_deref(), Some("the reply"));
    match snap(&events).last().unwrap() {
        AdapterEvent::SessionEnd { saw_result, .. } => assert!(*saw_result),
        other => panic!("expected session.end, got {other:?}"),
    }
}
