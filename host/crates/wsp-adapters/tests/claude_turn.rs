// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-claude/test/adapter.test.ts:1-1198 at a3b870a6e: the adapter over the recorded fixtures, its
//! launch line and environment, the interrupt policy, the process a finished turn leaves, steering over the stdin
//! channel and a reply while the process keeps running.

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

// ---------------------------------------------------------------- the commands this CLI runs only in its own terminal

// TS: adapter.test.ts:145
#[test]
fn screen_commands_are_one_table_by_name_without_a_slash() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let adapter = adapter_for(&exec);
    let table = claude_screen_commands();
    assert_eq!(adapter.screen_commands(), table);
    assert_eq!(table.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["login", "logout", "model", "permissions", "config", "help"]);
    assert_eq!(table.iter().map(|c| c.control).collect::<Vec<_>>(), vec![ScreenControl::SignIn, ScreenControl::SignIn, ScreenControl::Model, ScreenControl::Access, ScreenControl::Settings, ScreenControl::Docs]);
    for c in &table {
        assert!(!c.name.starts_with('/'), "{}", c.name);
    }
    let unique: std::collections::HashSet<&str> = table.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(unique.len(), table.len());
}

// ---------------------------------------------------------------- ClaudeAdapter over the recorded fixture

// TS: adapter.test.ts:158
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn launches_with_the_picked_model_effort_and_permission_mode() {
    tokio::task::LocalSet::new().run_until(launches_with_the_picked_model_effort_and_permission_mode_body()).await
}

async fn launches_with_the_picked_model_effort_and_permission_mode_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let adapter = adapter_for(&exec);
    let options = HarnessStartOptions { model: Some("claude-opus-5".into()), effort: Some("low".into()), permission_mode: Some("acceptEdits".into()), ..go("go", noop()) };
    adapter.start(options).unwrap().finished().await;
    let command = exec.calls()[0].command.clone();
    assert!(command.contains("--model 'claude-opus-5'"), "{command}");
    assert!(command.contains("--effort 'low'"), "{command}");
    assert!(command.contains("--permission-mode 'acceptEdits'"), "{command}");
}

// TS: adapter.test.ts:168
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keeps_its_auto_memory_in_the_folder_it_was_keyed_to() {
    tokio::task::LocalSet::new().run_until(keeps_its_auto_memory_in_the_folder_it_was_keyed_to_body()).await
}

async fn keeps_its_auto_memory_in_the_folder_it_was_keyed_to_body() {
    let mut lines = stream_lines();
    lines.extend(stream_lines());
    let exec = FakeExec::scripted(lines, StreamOpts::default());
    let mut deps = AdapterDeps::new(exec.clone(), "/Users/z/.claude");
    deps.base_env = Some(base_env(&[("PATH", "/usr/bin"), ("HOME", "/Users/z")]));
    deps.project_dir_name = Some("-Users-z-wt".into());
    let adapter = adapter_with(deps);
    adapter.start(go("go", noop())).unwrap().finished().await;
    let first = exec.calls()[0].clone();
    assert!(first.command.contains(r#"--settings '{"autoMemoryDirectory":"/Users/z/.claude/projects/-Users-z-wt/memory"}'"#), "{}", first.command);
    assert_eq!(env_of(&first.env, "CLAUDE_CODE_PROJECT_DIR_NAME"), Some("-Users-z-wt"));
    assert_eq!(env_of(&first.env, "CLAUDE_CONFIG_DIR"), None);
    adapter.start(HarnessStartOptions { fast: Some(true), ..go("go", noop()) }).unwrap().finished().await;
    let second = exec.calls()[1].command.clone();
    assert_eq!(second.matches("--settings").count(), 1, "{second}");
    assert!(second.contains(r#"--settings '{"fastMode":true,"autoMemoryDirectory":"/Users/z/.claude/projects/-Users-z-wt/memory"}'"#), "{second}");
    let bare = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let adapter = adapter_with(AdapterDeps::new(bare.clone(), "/Users/z/.claude"));
    adapter.start(go("go", noop())).unwrap().finished().await;
    assert!(!bare.calls()[0].command.contains("autoMemoryDirectory"));
}

// TS: adapter.test.ts:187
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn folds_the_persons_own_settings_into_its_one_flag() {
    tokio::task::LocalSet::new().run_until(folds_the_persons_own_settings_into_its_one_flag_body()).await
}

async fn folds_the_persons_own_settings_into_its_one_flag_body() {
    let memory = "/Users/z/.claude/projects/-Users-z-wt/memory";
    async fn launched(args: Vec<String>) -> String {
        let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
        let mut deps = AdapterDeps::new(exec.clone(), "/Users/z/.claude");
        deps.project_dir_name = Some("-Users-z-wt".into());
        deps.launch = Some(AgentLaunch { program: None, args: Some(args) });
        adapter_with(deps).start(HarnessStartOptions { fast: Some(true), ..go("go", noop()) }).unwrap().finished().await;
        exec.calls()[0].command.clone()
    }
    let inline = launched(vec!["--debug".into(), "--settings".into(), json!({"fastMode": false, "theme": "dark"}).to_string()]).await;
    assert_eq!(inline.matches("--settings").count(), 1, "{inline}");
    assert!(inline.contains(&format!(r#"--settings '{{"fastMode":false,"autoMemoryDirectory":"{memory}","theme":"dark"}}'"#)), "{inline}");
    assert!(inline.contains("-p '--debug' --input-format"), "{inline}");
    let joined = launched(vec![format!("--settings={}", json!({"autoMemoryDirectory": "/mine"}))]).await;
    assert_eq!(joined.matches("--settings").count(), 1, "{joined}");
    assert!(joined.contains(r#"--settings '{"fastMode":true,"autoMemoryDirectory":"/mine"}'"#), "{joined}");
    let file = launched(vec!["--settings".into(), "/Users/z/mine.json".into()]).await;
    assert_eq!(file.matches("--settings").count(), 1, "{file}");
    assert!(file.contains("-p '--settings' '/Users/z/mine.json' --input-format"), "{file}");
}

// TS: adapter.test.ts:209
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_launched_session_carries_the_run_its_stream_reported() {
    tokio::task::LocalSet::new().run_until(a_launched_session_carries_the_run_its_stream_reported_body()).await
}

async fn a_launched_session_carries_the_run_its_stream_reported_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let adapter = adapter_for(&exec);
    let session = adapter.start(go("go", noop())).unwrap();
    session.finished().await;
    assert_eq!(session.run().as_deref(), Some(RUN_HANDLE));
}

// TS: adapter.test.ts:217
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attaches_to_a_run_an_earlier_process_launched() {
    tokio::task::LocalSet::new().run_until(attaches_to_a_run_an_earlier_process_launched_body()).await
}

async fn attaches_to_a_run_an_earlier_process_launched_body() {
    let exec = FakeExec::attaching(stream_lines(), false);
    let adapter = adapter_for(&exec);
    let (events, on_event) = collect();
    let attached = adapter.attach(attach_opts(RUN_HANDLE, "e16ed170-8257-4668-879e-fe836341633c", on_event)).expect("attach offered").await.unwrap();
    let session = match attached {
        AttachedSession::Session(s) => s,
        AttachedSession::Gone => panic!("expected a session, got gone"),
    };
    let result = session.finished().await;
    let asked: Vec<(String, bool)> = exec.attached.lock().unwrap().iter().map(|(run, o)| (run.clone(), o.input)).collect();
    assert_eq!(asked, vec![(RUN_HANDLE.to_string(), true)]);
    assert_eq!(session.command(), None);
    assert_eq!(result.status, TurnStatus::Completed);
    let events = snap(&events);
    match &events[0] {
        AdapterEvent::SessionStart { session_id, .. } => assert_eq!(session_id, "e16ed170-8257-4668-879e-fe836341633c"),
        other => panic!("expected session.start, got {other:?}"),
    }
    assert!(!deltas(&events).is_empty());
    match &events[events.len() - 2..] {
        [AdapterEvent::TurnDone { .. }, AdapterEvent::SessionEnd { exit_code, saw_result, .. }] => {
            assert_eq!(*exit_code, Some(0));
            assert!(*saw_result);
        }
        other => panic!("expected turn.done then session.end, got {other:?}"),
    }
}

// TS: adapter.test.ts:237
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_machine_that_no_longer_holds_the_run_is_passed_through_as_gone() {
    tokio::task::LocalSet::new().run_until(a_machine_that_no_longer_holds_the_run_is_passed_through_as_gone_body()).await
}

async fn a_machine_that_no_longer_holds_the_run_is_passed_through_as_gone_body() {
    let exec = FakeExec::attaching(stream_lines(), true);
    let (events, on_event) = collect();
    let adapter = adapter_for(&exec);
    let attached = adapter.attach(attach_opts(RUN_HANDLE, "s", on_event)).expect("attach offered").await.unwrap();
    assert!(matches!(attached, AttachedSession::Gone));
    assert_eq!(snap(&events), vec![]);
    assert!(exec.calls().is_empty());
}

// TS: adapter.test.ts:247
#[test]
fn an_adapter_whose_exec_cannot_attach_has_no_attach_of_its_own() {
    let exec = FakeExec::scripted(vec![], StreamOpts::default());
    let adapter = adapter_for(&exec);
    assert!(adapter.attach(attach_opts(RUN_HANDLE, "s", noop())).is_none());
}

// TS: adapter.test.ts:253
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probes_the_catalog_through_the_exec_it_is_handed() {
    tokio::task::LocalSet::new().run_until(probes_the_catalog_through_the_exec_it_is_handed_body()).await
}

async fn probes_the_catalog_through_the_exec_it_is_handed_body() {
    let exec = FakeExec::scripted(vec![], StreamOpts::default());
    let mut deps = AdapterDeps::new(exec.clone(), CONFIG_DIR);
    deps.base_env = Some(guest_env());
    let adapter = adapter_with(deps);
    let ran: Arc<Mutex<Vec<(String, Option<Env>)>>> = Default::default();
    let probe_text = std::fs::read_to_string(format!("{}/tests/fixtures/claude/catalog-probe.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let probe = adapter.probe_catalog(recording_exec(ran.clone(), probe_text)).await.unwrap();
    let ran_now = ran.lock().unwrap().clone();
    assert_eq!(ran_now.len(), 1);
    assert_eq!(env_of(ran_now[0].1.as_ref().expect("env rides the command"), "CLAUDE_CONFIG_DIR"), Some("/root/.claude-cfg"));
    assert!(!ran_now[0].0.contains("CLAUDE_CONFIG_DIR"), "{}", ran_now[0].0);
    let probe = probe.expect("a probe");
    assert_eq!(probe.version.as_deref(), Some("2.1.257"));
    assert!(probe.models.iter().any(|m| m.slug == "claude-opus-5"));
    let garbage = adapter.probe_catalog(recording_exec(Default::default(), "garbage\n".to_string())).await.unwrap();
    assert_eq!(garbage, None);
}

// TS: adapter.test.ts:268
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn normalizes_the_stream_into_session_start_delta_done_end() {
    tokio::task::LocalSet::new().run_until(normalizes_the_stream_into_session_start_delta_done_end_body()).await
}

async fn normalizes_the_stream_into_session_start_delta_done_end_body() {
    let exec = FakeExec::scripted(stream_lines(), StreamOpts::default());
    let adapter = adapter_for(&exec);
    let (events, on_event) = collect();
    let session = adapter.start(go("write a hello world server", on_event)).unwrap();
    let result = session.finished().await;
    let events = snap(&events);

    assert_eq!(types(&events), vec!["session.start", "limit", "turn.usage", "turn.delta", "turn.usage", "turn.delta", "turn.delta", "turn.usage", "turn.delta", "turn.usage", "turn.delta", "turn.anchor", "turn.done", "session.end"]);

    match &events[0] {
        AdapterEvent::SessionStart { session_id, model, cwd, tools, .. } => {
            assert_eq!(session_id, SID);
            assert_eq!(model.as_deref(), Some("claude-sonnet-4-5"));
            assert_eq!(cwd.as_deref(), Some("/root"));
            assert!(tools.as_ref().expect("tools").iter().any(|t| t == "Bash"));
        }
        other => panic!("expected session.start, got {other:?}"),
    }

    let ds = deltas(&events);
    assert_eq!(ds.iter().map(|d| d.kind).collect::<Vec<_>>(), vec![DeltaKind::Text, DeltaKind::ToolUse, DeltaKind::ToolResult, DeltaKind::Thinking, DeltaKind::Text]);
    assert_eq!(ds[1].tool_name.as_deref(), Some("Bash"));
    assert_eq!(ds[1].tool_use_id.as_deref(), Some("toolu_01WspFixBash1"));
    assert!(ds[1].text.contains("curl -s http://localhost:3000"), "{}", ds[1].text);
    assert_eq!(ds[2].text, "Hello, World!");
    assert_eq!(ds[2].is_error, Some(false));
    assert!(ds[3].text.contains("server is live"), "{}", ds[3].text);

    assert_eq!(result.status, TurnStatus::Completed);
    assert_eq!(result.duration_ms, Some(JsNumber(10458.0)));
    assert_eq!(result.cost_usd, Some(JsNumber(0.0187)));
    assert_eq!(result.text.as_deref(), Some("Server is live at :3000 and answered: Hello, World!"));
    assert_eq!(result.model.as_deref(), Some("claude-sonnet-4-5"));
    assert_eq!(result.tokens, Some(TurnTokens { input: JsNumber(22_564.0), output: JsNumber(251.0), cached: Some(JsNumber(18_435.0)), cache_write: Some(JsNumber(4_113.0)), reasoning: Some(JsNumber(0.0)), context: Some(JsNumber(4_269.0)), window: Some(JsNumber(200_000.0)) }));

    match events.last().unwrap() {
        AdapterEvent::SessionEnd { exit_code, saw_result, .. } => {
            assert_eq!(*exit_code, Some(0));
            assert!(*saw_result);
        }
        other => panic!("expected session.end, got {other:?}"),
    }
    assert_eq!(session.claude_session_id(), SID);
}

// TS: adapter.test.ts:333
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn carries_the_clis_own_id_for_each_message_its_text_came_out_of() {
    tokio::task::LocalSet::new().run_until(carries_the_clis_own_id_for_each_message_its_text_came_out_of_body()).await
}

async fn carries_the_clis_own_id_for_each_message_its_text_came_out_of_body() {
    let (_, events) = run_lines(stream_lines(), "write a hello world server").await;
    let ids: Vec<Option<String>> = deltas(&events).into_iter().filter(|d| d.kind == DeltaKind::Text).map(|d| d.message_id).collect();
    assert_eq!(ids, vec![Some("msg_01WspFixA1".to_string()), Some("msg_01WspFixA4".to_string())]);
}

fn tool_call_line(id: &str, name: &str, input: Value) -> String {
    json!({"type": "assistant", "session_id": SID, "parent_tool_use_id": null, "message": {"id": format!("msg_{id}"), "role": "assistant", "content": [{"type": "tool_use", "id": id, "name": name, "input": input}]}}).to_string()
}

fn tool_answer_line(id: &str, text: &str) -> String {
    json!({"type": "user", "session_id": SID, "parent_tool_use_id": null, "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": id, "content": text}]}}).to_string()
}

fn step(text: &str, state: PlanStepState) -> PlanStep {
    PlanStep { text: text.to_string(), state }
}

fn tool_rows(events: &[AdapterEvent]) -> Vec<TurnDelta> {
    deltas(events).into_iter().filter(|d| d.kind == DeltaKind::ToolUse || d.kind == DeltaKind::ToolResult).collect()
}

// TS: adapter.test.ts:343
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reads_the_todo_list_and_the_proposed_plan_as_the_turns_plan() {
    tokio::task::LocalSet::new().run_until(reads_the_todo_list_and_the_proposed_plan_as_the_turns_plan_body()).await
}

async fn reads_the_todo_list_and_the_proposed_plan_as_the_turns_plan_body() {
    let todos = |states: &[&str]| json!({"todos": states.iter().enumerate().map(|(n, status)| json!({"content": format!("step {}", n + 1), "activeForm": format!("doing step {}", n + 1), "status": status})).collect::<Vec<_>>()});
    let lines = stream_lines();
    let script = vec![
        lines[1].clone(),
        tool_call_line("toolu_todo1", "TodoWrite", todos(&["in_progress", "pending", "pending"])),
        tool_answer_line("toolu_todo1", "Todos have been modified successfully"),
        tool_call_line("toolu_todo2", "TodoWrite", todos(&["completed", "in_progress", "pending"])),
        tool_answer_line("toolu_todo2", "Todos have been modified successfully"),
        tool_call_line("toolu_plan", "ExitPlanMode", json!({"plan": "# Add --quiet\n\n1. Parse the flag"})),
        tool_answer_line("toolu_plan", "Todos have been modified successfully"),
        lines.last().unwrap().clone(),
    ];
    let (_, events) = run_lines(script, "plan it").await;
    let plans: Vec<AdapterEvent> = events.iter().filter(|e| matches!(e, AdapterEvent::TurnPlan { .. })).cloned().collect();
    assert_eq!(
        plans,
        vec![
            AdapterEvent::TurnPlan { session_id: SID.into(), steps: Some(vec![step("step 1", PlanStepState::Working), step("step 2", PlanStepState::Pending), step("step 3", PlanStepState::Pending)]), text: None },
            AdapterEvent::TurnPlan { session_id: SID.into(), steps: Some(vec![step("step 1", PlanStepState::Done), step("step 2", PlanStepState::Working), step("step 3", PlanStepState::Pending)]), text: None },
            AdapterEvent::TurnPlan { session_id: SID.into(), steps: None, text: Some("# Add --quiet\n\n1. Parse the flag".into()) },
        ]
    );
    assert_eq!(tool_rows(&events), vec![]);
}

// TS: adapter.test.ts:371
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keeps_the_task_list_built_call_by_call_as_the_turns_plan() {
    tokio::task::LocalSet::new().run_until(keeps_the_task_list_built_call_by_call_as_the_turns_plan_body()).await
}

async fn keeps_the_task_list_built_call_by_call_as_the_turns_plan_body() {
    let lines = stream_lines();
    let script = vec![
        lines[1].clone(),
        tool_call_line("toolu_c1", "TaskCreate", json!({"subject": "Write line 1", "description": "first", "activeForm": "Writing line 1"})),
        tool_answer_line("toolu_c1", "Task #1 created successfully: Write line 1"),
        tool_call_line("toolu_c2", "TaskCreate", json!({"subject": "Write line 2", "description": "second", "activeForm": "Writing line 2"})),
        tool_answer_line("toolu_c2", "Task #2 created successfully: Write line 2"),
        tool_call_line("toolu_u1", "TaskUpdate", json!({"taskId": "1", "status": "in_progress"})),
        tool_answer_line("toolu_u1", "Updated task #1 status"),
        tool_call_line("toolu_u2", "TaskUpdate", json!({"taskId": "1", "status": "completed"})),
        tool_answer_line("toolu_u2", "Updated task #1 status"),
        tool_call_line("toolu_l", "TaskList", json!({})),
        tool_answer_line("toolu_l", "#1 [completed] Write line 1\n#2 [pending] Write line 2"),
        lines.last().unwrap().clone(),
    ];
    let (_, events) = run_lines(script, "list it").await;
    let one = |state| step("Write line 1", state);
    let two = step("Write line 2", PlanStepState::Pending);
    let plans: Vec<Option<Vec<PlanStep>>> = events.iter().filter_map(|e| if let AdapterEvent::TurnPlan { steps, .. } = e { Some(steps.clone()) } else { None }).collect();
    assert_eq!(
        plans,
        vec![Some(vec![one(PlanStepState::Pending)]), Some(vec![one(PlanStepState::Pending), two.clone()]), Some(vec![one(PlanStepState::Working), two.clone()]), Some(vec![one(PlanStepState::Done), two])]
    );
    assert_eq!(tool_rows(&events), vec![]);
}

// TS: adapter.test.ts:400
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stamps_every_line_a_subagent_wrote_with_the_call_that_launched_it() {
    tokio::task::LocalSet::new().run_until(stamps_every_line_a_subagent_wrote_with_the_call_that_launched_it_body()).await
}

async fn stamps_every_line_a_subagent_wrote_with_the_call_that_launched_it_body() {
    let fx = subagent_fixture();
    let (_, events) = run_lines(fx.lines.clone(), "fan out").await;
    let ds = deltas(&events);
    let (a, b) = (Some(fx.a_call.clone()), Some(fx.b_call.clone()));
    assert_eq!(ds.iter().map(|d| d.parent_tool_use_id.clone()).collect::<Vec<_>>(), vec![None, None, None, a.clone(), b.clone(), a.clone(), b.clone(), b, a, None, None]);
    assert_eq!(ds[3].text, ds[4].text);
    assert_ne!(ds[3].parent_tool_use_id, ds[4].parent_tool_use_id);
    assert!(!ds.iter().any(|d| d.text.contains("task_started")));
}

// TS: adapter.test.ts:422
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn says_each_subagents_start_model_spend_and_end_once_each() {
    tokio::task::LocalSet::new().run_until(says_each_subagents_start_model_spend_and_end_once_each_body()).await
}

async fn says_each_subagents_start_model_spend_and_end_once_each_body() {
    let fx = subagent_fixture();
    let (_, events) = run_lines(fx.lines.clone(), "fan out").await;

    let asked_b = subagent_asked(&fx.b_prompt);
    assert!(fx.b_prompt.chars().count() > SUBAGENT_ASKED_CHARS);
    assert!(asked_b.chars().count() <= SUBAGENT_ASKED_CHARS);
    let cut: String = asked_b.chars().take(asked_b.chars().count() - 1).collect();
    assert!(fx.b_prompt.starts_with(&cut));
    let (ac, bc) = (fx.a_call.as_str(), fx.b_call.as_str());
    let (ai, bi) = (fx.a_id.as_str(), fx.b_id.as_str());
    assert_eq!(
        subagent_events(&events),
        vec![
            sub(ai, ac, SubagentState::Running).title("count alpha files").depth(1).asked(&fx.a_prompt).ev(),
            sub(bi, bc, SubagentState::Running).title("read beta hostname").depth(1).asked(&asked_b).ev(),
            sub(ai, ac, SubagentState::Running).model("claude-haiku-4-5").ev(),
            sub(bi, bc, SubagentState::Running).model("claude-sonnet-4-5").ev(),
            sub(ai, ac, SubagentState::Running).usage(spent(1, Some(3_228.0))).ev(),
            sub(bi, bc, SubagentState::Running).usage(spent(1, Some(3_228.0))).ev(),
            sub(ai, ac, SubagentState::Done).summary("acpi, adduser.conf, alsa").usage(spent(1, Some(4_120.0))).ev(),
            sub(bi, bc, SubagentState::Stopped).usage(spent(1, None)).ev(),
        ]
    );
    assert!(!deltas(&events).iter().any(|d| d.text.contains("task_progress") || d.text.contains("Running ")));
}

const SPEND_ONE: &str = "ac063c143bb6a84b3";
const SPEND_TWO: &str = "a0046736e356b5076";

fn spend_lines() -> Vec<String> {
    fixture_lines("subagent-spend-2.1.296.jsonl")
}

async fn spent_rows(lines: Vec<String>) -> Vec<(String, SubagentState, SubagentUsage)> {
    let (_, events) = run_lines(lines, "two in the background").await;
    events
        .into_iter()
        .filter_map(|e| match e {
            AdapterEvent::Subagent { task, state, usage: Some(usage), .. } => Some((task, state, usage)),
            _ => None,
        })
        .collect()
}

fn spend_expected() -> Vec<(String, SubagentState, SubagentUsage)> {
    let one_first = tokens(2 + 10_308, 16, 0, 10_308);
    let one_all = tokens(2 + 10_308 + 2 + 654 + 10_308, 19, 10_308, 10_308 + 654);
    let two_first = tokens(2 + 3_913 + 6_395, 16, 6_395, 3_913);
    let two_all = tokens(2 + 3_913 + 6_395 + 2 + 654 + 10_308, 19, 6_395 + 10_308, 3_913 + 654);
    let row = |task: &str, state, tokens, ms| (task.to_string(), state, SubagentUsage { tokens: Some(tokens), tool_uses: Some(1.into()), duration_ms: Some(JsNumber(ms)) });
    vec![
        row(SPEND_ONE, SubagentState::Running, one_first, 1_085.0),
        row(SPEND_TWO, SubagentState::Running, two_first, 1_129.0),
        row(SPEND_ONE, SubagentState::Done, one_all, 14_370.0),
        row(SPEND_TWO, SubagentState::Done, two_all, 13_748.0),
    ]
}

// TS: adapter.test.ts:474
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sums_a_subagents_own_calls_into_a_row_at_each_progress_line_and_its_end() {
    tokio::task::LocalSet::new().run_until(sums_a_subagents_own_calls_into_a_row_at_each_progress_line_and_its_end_body()).await
}

async fn sums_a_subagents_own_calls_into_a_row_at_each_progress_line_and_its_end_body() {
    assert_eq!(spent_rows(spend_lines()).await, spend_expected());
}

// TS: adapter.test.ts:478
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn counts_a_call_once_however_many_of_its_blocks_the_cli_writes() {
    tokio::task::LocalSet::new().run_until(counts_a_call_once_however_many_of_its_blocks_the_cli_writes_body()).await
}

async fn counts_a_call_once_however_many_of_its_blocks_the_cli_writes_body() {
    let mut lines = spend_lines();
    let at = lines.iter().position(|l| l.contains(&format!("\"agent_id\":\"{SPEND_ONE}\"")) && l.contains("\"type\":\"assistant\"")).expect("an assistant line of the first subagent");
    let mut early: Value = serde_json::from_str(&lines[at]).unwrap();
    early["message"]["usage"]["output_tokens"] = json!(4);
    lines.insert(at, early.to_string());
    let again = lines[at + 1].clone();
    lines.insert(at + 2, again);
    assert_eq!(spent_rows(lines).await, spend_expected());
}

// TS: adapter.test.ts:490
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forwards_a_subagents_own_text_only_from_a_cli_that_takes_the_flag() {
    tokio::task::LocalSet::new().run_until(forwards_a_subagents_own_text_only_from_a_cli_that_takes_the_flag_body()).await
}

async fn forwards_a_subagents_own_text_only_from_a_cli_that_takes_the_flag_body() {
    async fn launched(version: Option<&str>) -> (String, usize) {
        let exec = FakeExec::scripted(subagent_fixture().lines, StreamOpts::default());
        let adapter = adapter_for(&exec);
        let (events, on_event) = collect();
        adapter.start(HarnessStartOptions { version: version.map(String::from), ..go("fan out", on_event) }).unwrap().finished().await;
        let command = exec.calls()[0].command.clone();
        (command, subagent_events(&snap(&events)).len())
    }
    for version in ["2.1.288", "2.1.270"] {
        let (command, children) = launched(Some(version)).await;
        assert!(command.contains("--forward-subagent-text"), "{version}: {command}");
        assert_eq!(children, 8);
    }
    for older in [Some("2.1.269"), None] {
        let (command, children) = launched(older).await;
        assert!(!command.contains("--forward-subagent-text"), "{older:?}: {command}");
        assert_eq!(children, 8);
    }
}

// TS: adapter.test.ts:508
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_subagents_model_comes_off_its_first_line_never_an_api_error() {
    tokio::task::LocalSet::new().run_until(a_subagents_model_comes_off_its_first_line_never_an_api_error_body()).await
}

async fn a_subagents_model_comes_off_its_first_line_never_an_api_error_body() {
    let lines = vec![
        json!({"type": "system", "subtype": "init", "session_id": SID}).to_string(),
        json!({"type": "assistant", "session_id": SID, "parent_tool_use_id": null, "message": {"id": "m1", "model": "claude-sonnet-4-5", "content": [{"type": "tool_use", "id": "toolu_t", "name": "Task", "input": {"description": "count", "prompt": "Count to three."}}]}}).to_string(),
        json!({"type": "system", "subtype": "task_started", "task_id": "a1", "tool_use_id": "toolu_t", "description": "count", "task_type": "local_agent", "session_id": SID}).to_string(),
        json!({"type": "assistant", "session_id": SID, "parent_tool_use_id": "toolu_t", "is_api_error_message": true, "message": {"id": "m2", "model": "<synthetic>", "content": [{"type": "text", "text": "API Error: 529"}]}}).to_string(),
        json!({"type": "assistant", "session_id": SID, "parent_tool_use_id": "toolu_t", "message": {"id": "m3", "model": "claude-haiku-4-5", "content": [{"type": "text", "text": "1, 2, 3"}]}}).to_string(),
        json!({"type": "assistant", "session_id": SID, "parent_tool_use_id": "toolu_t", "message": {"id": "m4", "model": "claude-opus-4-1", "content": [{"type": "text", "text": "done"}]}}).to_string(),
        json!({"type": "result", "subtype": "success", "session_id": SID, "result": "ok", "usage": {"output_tokens": 1}}).to_string(),
    ];
    let (_, events) = run_lines(lines, "count").await;
    assert_eq!(
        subagent_events(&events),
        vec![sub("a1", "toolu_t", SubagentState::Running).title("count").asked("Count to three.").ev(), sub("a1", "toolu_t", SubagentState::Running).model("claude-haiku-4-5").ev()]
    );
}

// TS: adapter.test.ts:528
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_background_command_the_cli_tracks_as_a_task_is_no_subagent() {
    tokio::task::LocalSet::new().run_until(a_background_command_the_cli_tracks_as_a_task_is_no_subagent_body()).await
}

async fn a_background_command_the_cli_tracks_as_a_task_is_no_subagent_body() {
    let lines = vec![
        json!({"type": "system", "subtype": "init", "session_id": SID}).to_string(),
        json!({"type": "system", "subtype": "task_started", "task_id": "b1", "tool_use_id": "toolu_bash", "description": "npm run dev", "task_type": "local_bash", "session_id": SID}).to_string(),
        json!({"type": "system", "subtype": "task_notification", "task_id": "b1", "tool_use_id": "toolu_bash", "status": "completed", "summary": "exited 0", "session_id": SID}).to_string(),
        json!({"type": "result", "subtype": "success", "session_id": SID, "result": "ok", "usage": {"output_tokens": 1}}).to_string(),
    ];
    let (_, events) = run_lines(lines, "serve").await;
    assert_eq!(subagent_events(&events), vec![]);
}

// ---------------------------------------------------------------- a subagent's end on 2.1.295

fn recorded(file: &str) -> Vec<String> {
    fixture_lines(file)
}

/// Every line the fold of one launching call reads: the call's own answers and its subagent's lines.
fn fold_of(events: &[AdapterEvent], call: &str) -> Vec<TurnDelta> {
    deltas(events).into_iter().filter(|d| d.tool_use_id.as_deref() == Some(call) || d.parent_tool_use_id.as_deref() == Some(call)).collect()
}

fn handed_back(events: &[AdapterEvent], handback: &str) -> Vec<TurnDelta> {
    deltas(events).into_iter().filter(|d| d.tool_name.as_deref() == Some("SubagentHandback") || d.tool_use_id.as_deref() == Some(handback)).collect()
}

fn answer(session: &str, call: &str, text: &str) -> TurnDelta {
    TurnDelta { tool_use_id: Some(call.to_string()), is_error: Some(false), ..TurnDelta::new(session, DeltaKind::ToolResult, text) }
}

fn answers(events: &[AdapterEvent], call: &str) -> Vec<TurnDelta> {
    deltas(events).into_iter().filter(|d| d.kind == DeltaKind::ToolResult && d.tool_use_id.as_deref() == Some(call) && d.parent_tool_use_id.is_none()).collect()
}

fn kind_text(ds: Vec<TurnDelta>) -> Vec<(DeltaKind, String)> {
    ds.into_iter().map(|d| (d.kind, d.text)).collect()
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

fn not_internal(ds: Vec<TurnDelta>) -> Vec<TurnDelta> {
    ds.into_iter().filter(|d| !internal_tool_result(&d.text)).collect()
}

fn variant(change: impl Fn(Value) -> Vec<Value>) -> Vec<String> {
    recorded("subagent-handback.jsonl").into_iter().flat_map(|l| change(serde_json::from_str(&l).unwrap())).map(|f| f.to_string()).collect()
}

fn is_notification(f: &Value) -> bool {
    f["subtype"] == "task_notification"
}

fn handback_result(f: &Value) -> bool {
    f["type"] == "user" && f.to_string().contains("toolu_01WspFixHandBgCall")
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

fn same(f: Value) -> Value {
    f
}

const BG: &str = "toolu_01WspFixHandBg";
const BG_SID: &str = "21921000-0000-4aaa-8bbb-000000000001";
const BG_REPORT: &str = "2 + 2 is 4.\n\n3 + 3 is 6.";
const PLAIN_FG: &str = "2 + 2 is 4, and 3 + 3 is 6.\n\nBoth are basic addition facts, so no tools were needed to answer.";

// TS: adapter.test.ts:557
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn answers_a_backgrounded_launch_with_the_report_and_draws_no_hand_back() {
    tokio::task::LocalSet::new().run_until(answers_a_backgrounded_launch_with_the_report_and_draws_no_hand_back_body()).await
}

async fn answers_a_backgrounded_launch_with_the_report_and_draws_no_hand_back_body() {
    let report = "2 + 2 is 4.\n\n3 + 3 is 6.";
    let events = replay(recorded("subagent-handback.jsonl")).await;
    let fold = fold_of(&events, BG);
    assert_eq!(fold.last().unwrap(), &answer(BG_SID, BG, report));
    let tool_results: Vec<String> = fold.iter().filter(|d| d.kind == DeltaKind::ToolResult && d.tool_use_id.as_deref() == Some(BG)).map(|d| if internal_tool_result(&d.text) { "note".to_string() } else { d.text.clone() }).collect();
    assert_eq!(tool_results, vec!["note".to_string(), report.to_string()]);
    let own: Vec<TurnDelta> = fold.iter().filter(|d| d.parent_tool_use_id.as_deref() == Some(BG)).cloned().collect();
    assert_eq!(kind_text(own), vec![(DeltaKind::Thinking, String::new()), (DeltaKind::Text, report.to_string())]);
    assert_eq!(handed_back(&events, "toolu_01WspFixHandBgCall"), vec![]);
    let usage = SubagentUsage { tokens: Some(tokens(2 + 21_895, 5, 0, 21_895)), tool_uses: Some(1.into()), duration_ms: Some(JsNumber(2_000.0)) };
    let ended: Vec<AdapterEvent> = events.iter().filter(|e| matches!(e, AdapterEvent::Subagent { state: SubagentState::Done, .. })).cloned().collect();
    assert_eq!(ended, vec![sub("aafea93324a2f8ca1", BG, SubagentState::Done).session(BG_SID).summary(report).usage(usage).ev()]);
}

// TS: adapter.test.ts:573
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn answers_a_launch_it_waited_on_with_the_report_once_never_the_note() {
    tokio::task::LocalSet::new().run_until(answers_a_launch_it_waited_on_with_the_report_once_never_the_note_body()).await
}

async fn answers_a_launch_it_waited_on_with_the_report_once_never_the_note_body() {
    let sid = "21921000-0000-4aaa-8bbb-000000000002";
    let report = "2+2 is 4.\n\n3+3 is 6.";
    let call = "toolu_01WspFixHandFg";
    let events = replay(recorded("subagent-handback-foreground.jsonl")).await;
    let fold = fold_of(&events, call);
    let top: Vec<TurnDelta> = fold.iter().filter(|d| d.kind == DeltaKind::ToolResult && d.parent_tool_use_id.is_none()).cloned().collect();
    assert_eq!(top, vec![answer(sid, call, report)]);
    assert_eq!(fold.last().unwrap(), &answer(sid, call, report));
    let own: Vec<TurnDelta> = fold.iter().filter(|d| d.parent_tool_use_id.as_deref() == Some(call)).cloned().collect();
    assert_eq!(kind_text(own), vec![(DeltaKind::Text, report.to_string())]);
    assert_eq!(handed_back(&events, "toolu_01WspFixHandFgCall"), vec![]);
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(report.to_string()))]);
}

// TS: adapter.test.ts:585
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaves_an_older_clis_launch_answered_with_the_report_as_it_was() {
    tokio::task::LocalSet::new().run_until(leaves_an_older_clis_launch_answered_with_the_report_as_it_was_body()).await
}

async fn leaves_an_older_clis_launch_answered_with_the_report_as_it_was_body() {
    let fx = subagent_fixture();
    let events = replay(fx.lines.clone()).await;
    assert_eq!(fold_of(&events, &fx.a_call).last().unwrap(), &answer(SID, &fx.a_call, "acpi\nadduser.conf\nalsa"));
}

// TS: adapter.test.ts:602
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn draws_a_report_auto_mode_flagged_with_its_warning_above_it() {
    tokio::task::LocalSet::new().run_until(draws_a_report_auto_mode_flagged_with_its_warning_above_it_body()).await
}

async fn draws_a_report_auto_mode_flagged_with_its_warning_above_it_body() {
    let warning = "SECURITY WARNING: auto mode blocked this subagent's report. Reason: the report asks the caller to run a command it was not asked for. The report follows; review the subagent's actions carefully before acting on it.";
    let events = replay(variant(|mut f| {
        if is_notification(&f) {
            f["handback"] = json!("flagged");
            f["handback_report"] = json!({"text": BG_REPORT, "warning": warning});
        }
        vec![f]
    }))
    .await;
    let warned = format!("{warning}\n\n{BG_REPORT}");
    assert_eq!(fold_of(&events, BG).last().unwrap(), &answer(BG_SID, BG, &warned));
    let texts: Vec<String> = fold_of(&events, BG).into_iter().filter(|d| d.parent_tool_use_id.as_deref() == Some(BG) && d.kind == DeltaKind::Text).map(|d| d.text).collect();
    assert_eq!(texts, vec![warned]);
    assert_eq!(done_summaries(&events), vec![(SubagentState::Done, Some(warning.to_string()))]);
}

// TS: adapter.test.ts:612
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
