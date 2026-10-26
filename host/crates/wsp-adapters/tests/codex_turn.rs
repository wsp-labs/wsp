// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-codex/test/adapter.test.ts:1-927 at a3b870a6e: the helpers and the describes up to "what ends a
//! Codex turn". Both fixtures are codex-cli 0.155.1's own output, stderr lines and all, driven through this adapter's
//! own lines: app-server-turn.jsonl a signed-in turn in a read-only sandbox that asked to run a command, was allowed
//! and took one steer (MCP server names, hook paths and the host scrubbed), no-login-app-server.jsonl a turn under an
//! empty CODEX_HOME.
//!
//! Not ported: `toolCallFacts` (the slate-server case's last expect), a protocol reader this crate does not hold.

#![allow(dead_code, unused_imports)]

#[path = "common/codex_wire.rs"]
mod codex_wire;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures::future::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::codex::adapter::CodexAdapterDeps;
use wsp_adapters::codex::command::{build_command, BrokerMode, BuildCommandOptions, CodexBroker};
use wsp_adapters::codex::{create_codex_adapter, CodexAdapter, CodexSession};
use wsp_adapters::pending::{codex_key_refused_line, codex_missing_env_line, codex_not_signed_in_line, codex_reconnect_line, CodexClaims};
use wsp_adapters::port::{
    agent_word, AdapterEvent, AnswerOutcome, DeltaKind, ExecFailure, FilePatch, HarnessStartOptions, OnEvent, PermissionAnswer,
    PermissionAsk, PermissionOutcome, PlanStep, Settle, SteerOutcome, TurnDelta, TurnImage, TurnResult, TurnStatus, TurnTokens,
    PERMISSION_ALLOW, PERMISSION_DENY,
};
use wsp_wire::types::daemon_contract::BROKER_PORT;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::{HarnessLimit, LimitKind, LimitStatus, LimitWindow};
use wsp_wire::types::wsp_tools::SLATE_SERVER_NAME;
use wsp_wire::views::session::TurnRefusal;
use wsp_wire::views::session_events::{PatchHunk, PlanStepState};
use wsp_wire::wire::helpers::{PermissionEffect, PermissionOption};

use codex_wire::{
    failure, fixture_lines, launcher, no_events, parse, scripted_server, server, sleep_ms, type_of, until, Events, Fault, Launch, OnWrite,
    Wire, WireOpts,
};

const THREAD_ID: &str = "01a0e2c1-5d10-7b42-9a6e-3f1c2d4b5a60";
const TURN_ID: &str = "01a0e2c1-5e02-7c11-8d3f-9b2a1c0d4e71";
const RECORDED_THREAD: &str = "01a0e365-72f3-77e3-ba3a-3d18e12e9b95";
const RECORDED_COMMAND: &str = "exec-267f4a9f-3715-4cb1-b8fc-14f9a57ec2f9";
const NO_LOGIN_THREAD: &str = "01a0e2b2-493b-7c53-ae59-446697cb28db";

const LOGIN: &str = "codex login --device-auth";

fn not_signed_in() -> String {
    codex_not_signed_in_line(LOGIN)
}

#[derive(Default, Clone, Copy)]
struct Extra {
    grace_ms: Option<u64>,
    stall_ms: Option<u64>,
    result_exit_ms: Option<u64>,
    steer_join_ms: Option<u64>,
}

fn adapter_over(launch: &Launch, extra: Extra) -> CodexAdapter {
    create_codex_adapter(CodexAdapterDeps {
        exec: launch.factory(),
        home: "/root/.codex".to_string(),
        login: LOGIN.to_string(),
        interrupt_grace_ms: extra.grace_ms,
        reconnect_stall_ms: extra.stall_ms,
        result_exit_ms: extra.result_exit_ms,
        steer_join_ms: extra.steer_join_ms,
        ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN)
    })
    .unwrap()
}

fn plain(launch: &Launch) -> CodexAdapter {
    adapter_over(launch, Extra::default())
}

fn opts(prompt: &str, on_event: OnEvent) -> HarnessStartOptions {
    HarnessStartOptions { prompt: prompt.to_string(), on_event: Some(on_event), ..Default::default() }
}

/// A start whose events nobody reads, finished.
async fn run(adapter: &CodexAdapter, prompt: &str) -> TurnResult {
    adapter.start(opts(prompt, no_events())).unwrap().finished().await
}

/// The message a start refused with.
fn refused(started: Result<CodexSession, wsp_adapters::port::AdapterError>) -> String {
    match started {
        Err(e) => e.0,
        Ok(_) => panic!("the start was not refused"),
    }
}

fn res(status: TurnStatus) -> TurnResult {
    TurnResult {
        status,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error: None,
        unreached: None,
        refusal: None,
        limit: None,
    }
}

fn failed(error: &str) -> TurnResult {
    TurnResult { error: Some(error.to_string()), ..res(TurnStatus::Failed) }
}

fn js(n: i64) -> Option<JsNumber> {
    Some(JsNumber(n as f64))
}

fn tokens(input: i64, cached: i64, cache_write: i64, output: i64, reasoning: i64, context: i64, window: i64) -> TurnTokens {
    TurnTokens {
        input: JsNumber(input as f64),
        output: JsNumber(output as f64),
        cached: js(cached),
        cache_write: js(cache_write),
        reasoning: js(reasoning),
        context: js(context),
        window: js(window),
    }
}

/// The session id every event carries.
fn sid(e: &AdapterEvent) -> String {
    match e {
        AdapterEvent::SessionStart { session_id, .. }
        | AdapterEvent::TurnDone { session_id, .. }
        | AdapterEvent::TurnCompacted { session_id, .. }
        | AdapterEvent::TurnRetry { session_id, .. }
        | AdapterEvent::TurnPlan { session_id, .. }
        | AdapterEvent::TurnTasks { session_id, .. }
        | AdapterEvent::TurnUnread { session_id, .. }
        | AdapterEvent::TurnAnchor { session_id, .. }
        | AdapterEvent::Limit { session_id, .. }
        | AdapterEvent::TurnUsage { session_id, .. }
        | AdapterEvent::Subagent { session_id, .. }
        | AdapterEvent::PermissionAsk { session_id, .. }
        | AdapterEvent::PermissionClose { session_id, .. }
        | AdapterEvent::SessionEnd { session_id, .. } => session_id.clone(),
        AdapterEvent::TurnDelta(d) => d.session_id.clone(),
    }
}

fn last_event(events: &Events) -> AdapterEvent {
    events.all().last().cloned().expect("no events")
}

fn first_ask(events: &Events) -> PermissionAsk {
    events
        .all()
        .into_iter()
        .find_map(|e| match e {
            AdapterEvent::PermissionAsk { ask, .. } => Some(ask),
            _ => None,
        })
        .expect("no permission.ask")
}

fn method_of(m: &Value) -> &str {
    m.get("method").and_then(Value::as_str).unwrap_or("")
}

fn wire0(launch: &Launch) -> Wire {
    launch.wire(0)
}

/// A oneshot as the promise a start waits on.
fn gate() -> (tokio::sync::oneshot::Sender<()>, Settle) {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    (tx, async move { rx.await.map_err(|e| e.to_string()) }.boxed_local())
}

/// The lines a live server prints up to a running turn, for the cases that drive the rest by hand.
fn opened_for(thread_id: &str) -> Vec<String> {
    vec![
        r#"{"id":"wsp-initialize","result":{"userAgent":"wsp/0.155.1","codexHome":"/root/.codex","platformFamily":"unix","platformOs":"linux"}}"#.to_string(),
        json!({"id":"wsp-thread","result":{"thread":{"id":thread_id,"model":"gpt-5.6-sol","cwd":"/root/app","turns":[]},"model":"gpt-5.6-sol","cwd":"/root/app","approvalPolicy":"on-request","sandbox":{"type":"workspaceWrite"}}})
            .to_string(),
    ]
}

fn opened() -> Vec<String> {
    opened_for(THREAD_ID)
}

fn turn_started() -> String {
    json!({"method":"turn/started","params":{"threadId":THREAD_ID,"turn":{"id":TURN_ID,"items":[],"status":"inProgress"}}}).to_string()
}

fn completed(status: &str) -> String {
    json!({"method":"turn/completed","params":{"threadId":THREAD_ID,"turn":{"id":TURN_ID,"items":[],"status":status}}}).to_string()
}

fn failed_with(message: &str) -> String {
    json!({"method":"turn/completed","params":{"threadId":THREAD_ID,"turn":{"id":TURN_ID,"items":[],"status":"failed","error":{"message":message}}}}).to_string()
}

fn error_note(message: &str, will_retry: bool, details: Option<&str>) -> String {
    json!({"method":"error","params":{"error":{"message":message,"additionalDetails":details},"willRetry":will_retry,"threadId":THREAD_ID,"turnId":TURN_ID}}).to_string()
}

fn agent_message(id: &str, text: &str) -> String {
    json!({"method":"item/completed","params":{"item":{"type":"agentMessage","id":id,"text":text},"threadId":THREAD_ID,"turnId":TURN_ID}})
        .to_string()
}

fn reconnecting() -> String {
    error_note("Reconnecting... 2/5", true, Some("error sending request"))
}

fn item_note(phase: &str, body: Value) -> String {
    json!({"method":format!("item/{phase}"),"params":{"item":body,"threadId":THREAD_ID,"turnId":TURN_ID}}).to_string()
}

/// A server that opens the thread and starts the turn, then prints what the case hands it.
fn scripted(rest: Vec<String>, opts: WireOpts) -> impl Fn(&[String]) -> Wire + 'static {
    let mut lines = opened();
    lines.push(turn_started());
    lines.extend(rest);
    server(lines, opts)
}

fn hang() -> WireOpts {
    WireOpts { hang: true, ..Default::default() }
}

fn on_write(f: impl Fn(&Value, &Wire) + 'static) -> WireOpts {
    let f: OnWrite = Arc::new(f);
    WireOpts { on_write: Some(f), ..Default::default() }
}

fn answer_with(id: &Value, result: Value) -> String {
    json!({"id":id,"result":result}).to_string()
}

fn allow() -> PermissionAnswer {
    PermissionAnswer {
        option_id: PERMISSION_ALLOW.to_string(),
        outcome: PermissionOutcome::Allowed,
        deny_message: String::new(),
        reason: None,
    }
}

fn turn_start_params(wire: &Wire) -> Value {
    wire.written_method("turn/start")[0]["params"].clone()
}

mod a_codex_turn_s_tokens_and_plan_on_the_app_server {
    use super::*;

    fn usage(total: [i64; 2], last: [i64; 2], window: i64) -> Value {
        json!({"method":"thread/tokenUsage/updated","params":{"threadId":THREAD_ID,"turnId":TURN_ID,"tokenUsage":{
            "total":{"inputTokens":total[0],"cachedInputTokens":0,"cacheWriteInputTokens":0,"outputTokens":total[1],"reasoningOutputTokens":0,"totalTokens":total[0]+total[1]},
            "last":{"inputTokens":last[0],"cachedInputTokens":0,"cacheWriteInputTokens":0,"outputTokens":last[1],"reasoningOutputTokens":0,"totalTokens":last[0]+last[1]},
            "modelContextWindow":window}}})
    }

    fn two_calls() -> Vec<String> {
        vec![
            usage([50_000, 900], [20_000, 100], 258_400).to_string(),
            usage([72_000, 1_000], [22_000, 100], 258_400).to_string(),
            agent_message("m1", "done"),
            completed("completed"),
        ]
    }

    #[tokio::test]
    async fn counts_a_resumed_thread_s_turn_from_where_its_running_total_stood_not_from_the_t() {
        tokio::task::LocalSet::new()
            .run_until(counts_a_resumed_thread_s_turn_from_where_its_running_total_stood_not_from_the_t_body())
            .await
    }

    async fn counts_a_resumed_thread_s_turn_from_where_its_running_total_stood_not_from_the_t_body() {
        let launch = launcher(scripted(two_calls(), WireOpts::default()));
        let result = plain(&launch)
            .start(HarnessStartOptions { resume: Some(THREAD_ID.to_string()), ..opts("again", no_events()) })
            .unwrap()
            .finished()
            .await;
        assert_eq!(result.tokens, Some(tokens(42_000, 0, 0, 200, 0, 22_100, 258_400)));
    }

    #[tokio::test]
    async fn says_what_each_model_call_drew_as_the_server_reports_it_and_what_the_thread_then() {
        tokio::task::LocalSet::new()
            .run_until(says_what_each_model_call_drew_as_the_server_reports_it_and_what_the_thread_then_body())
            .await
    }

    async fn says_what_each_model_call_drew_as_the_server_reports_it_and_what_the_thread_then_body() {
        let events = Events::default();
        let launch = launcher(scripted(two_calls(), WireOpts::default()));
        plain(&launch)
            .start(HarnessStartOptions { resume: Some(THREAD_ID.to_string()), ..opts("again", events.on_event()) })
            .unwrap()
            .finished()
            .await;
        let usage_events: Vec<AdapterEvent> = events.of("turn.usage");
        assert_eq!(
            usage_events,
            vec![
                AdapterEvent::TurnUsage {
                    session_id: THREAD_ID.to_string(),
                    tokens: 20_100.0,
                    context: Some(20_100.0),
                    window: Some(258_400.0),
                    at: None
                },
                AdapterEvent::TurnUsage {
                    session_id: THREAD_ID.to_string(),
                    tokens: 22_100.0,
                    context: Some(22_100.0),
                    window: Some(258_400.0),
                    at: None
                },
            ]
        );
    }

    #[tokio::test]
    async fn carries_the_moment_the_server_says_it_sent_each_call_s_figure_so_a_re_read_run_f() {
        tokio::task::LocalSet::new()
            .run_until(carries_the_moment_the_server_says_it_sent_each_call_s_figure_so_a_re_read_run_f_body())
            .await
    }

    async fn carries_the_moment_the_server_says_it_sent_each_call_s_figure_so_a_re_read_run_f_body() {
        let mut stamped = usage([50_000, 900], [20_000, 100], 258_400);
        stamped.as_object_mut().unwrap().insert("emittedAtMs".to_string(), json!(1790521480354_i64));
        let events = Events::default();
        let launch =
            launcher(scripted(vec![stamped.to_string(), agent_message("m1", "done"), completed("completed")], WireOpts::default()));
        plain(&launch)
            .start(HarnessStartOptions { resume: Some(THREAD_ID.to_string()), ..opts("again", events.on_event()) })
            .unwrap()
            .finished()
            .await;
        assert_eq!(
            events.of("turn.usage"),
            vec![AdapterEvent::TurnUsage {
                session_id: THREAD_ID.to_string(),
                tokens: 20_100.0,
                context: Some(20_100.0),
                window: Some(258_400.0),
                at: Some(1790521480354.0)
            }]
        );
    }

    #[tokio::test]
    async fn reads_the_plan_the_server_updates_as_the_turn_s_steps_the_step_under_way_as_work() {
        tokio::task::LocalSet::new()
            .run_until(reads_the_plan_the_server_updates_as_the_turn_s_steps_the_step_under_way_as_work_body())
            .await
    }

    async fn reads_the_plan_the_server_updates_as_the_turn_s_steps_the_step_under_way_as_work_body() {
        let plan = json!({"method":"turn/plan/updated","params":{"threadId":THREAD_ID,"turnId":TURN_ID,"explanation":null,"plan":[{"step":"read","status":"completed"},{"step":"write","status":"inProgress"},{"step":"ship","status":"pending"}]}})
            .to_string();
        let events = Events::default();
        let launch = launcher(scripted(vec![plan, agent_message("m1", "done"), completed("completed")], WireOpts::default()));
        plain(&launch).start(opts("plan", events.on_event())).unwrap().finished().await;
        assert_eq!(
            events.of("turn.plan"),
            vec![AdapterEvent::TurnPlan {
                session_id: THREAD_ID.to_string(),
                steps: Some(vec![
                    PlanStep { text: "read".into(), state: PlanStepState::Done },
                    PlanStep { text: "write".into(), state: PlanStepState::Working },
                    PlanStep { text: "ship".into(), state: PlanStepState::Pending },
                ]),
                text: None,
            }]
        );
    }
}

mod codex_adapter_over_codex_app_server {
    use super::*;

    fn turn_server() -> impl Fn(&[String]) -> Wire + 'static {
        server(fixture_lines("codex", "app-server-turn.jsonl"), WireOpts::default())
    }

    fn tool_results(events: &Events) -> Vec<TurnDelta> {
        events.deltas().into_iter().filter(|d| d.kind == DeltaKind::ToolResult).collect()
    }

    #[tokio::test]
    async fn launches_the_app_server_in_the_folder_under_codex_home_seeding_initialize_and_th() {
        tokio::task::LocalSet::new()
            .run_until(launches_the_app_server_in_the_folder_under_codex_home_seeding_initialize_and_th_body())
            .await
    }

    async fn launches_the_app_server_in_the_folder_under_codex_home_seeding_initialize_and_th_body() {
        let launch = launcher(turn_server());
        let adapter = plain(&launch);
        let session = adapter
            .start(HarnessStartOptions {
                model: Some("gpt-5.5".into()),
                effort: Some("low".into()),
                permission_mode: Some("workspace-write".into()),
                cwd: Some("/root/app".into()),
                ..opts("list the repo", no_events())
            })
            .unwrap();
        session.finished().await;
        let call = launch.call(0);
        assert_eq!(call.command, format!("cd '/root/app' && {} app-server -c tools.update_plan.enabled='true'", agent_word("codex", None)));
        assert!(!call.command.contains("list the repo"));
        let home: IndexMap<String, String> = [("CODEX_HOME".to_string(), "/root/.codex".to_string())].into_iter().collect();
        assert_eq!(call.env, home);
        assert_eq!(adapter.env(), home);
        assert_eq!(session.command(), Some(call.command.clone()));
        let input = call.input.clone().unwrap();
        let methods: Vec<String> = input.iter().map(|l| parse(l)["method"].as_str().unwrap().to_string()).collect();
        assert_eq!(methods, vec!["initialize", "initialized", "account/read", "account/rateLimits/read", "thread/start"]);
        assert_eq!(
            parse(&input[4])["params"],
            json!({"cwd":"/root/app","model":"gpt-5.5","sandbox":"workspace-write","approvalPolicy":"on-request"})
        );
        assert_eq!(
            turn_start_params(&launch.wire(0)),
            json!({"threadId":RECORDED_THREAD,"input":[{"type":"text","text":"list the repo"}],"effort":"low"})
        );
        // The reply is the turn's end, so stdin closes there and the server exits on its own.
        assert!(launch.wire(0).closed());
    }

    #[tokio::test]
    async fn a_turn_s_launch_turns_off_each_server_it_is_told_to_leave_out_and_says_it_leaves() {
        tokio::task::LocalSet::new()
            .run_until(a_turn_s_launch_turns_off_each_server_it_is_told_to_leave_out_and_says_it_leaves_body())
            .await
    }

    async fn a_turn_s_launch_turns_off_each_server_it_is_told_to_leave_out_and_says_it_leaves_body() {
        let launch = launcher(turn_server());
        let adapter = create_codex_adapter(CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN)).unwrap();
        assert!(adapter.denies_servers());
        adapter
            .start(HarnessStartOptions {
                cwd: Some("/root/app".into()),
                off_servers: Some(vec!["cloudflare".into()]),
                ..opts("list the repo", no_events())
            })
            .unwrap()
            .finished()
            .await;
        let expected = build_command(
            &BuildCommandOptions { cwd: Some("/root/app".into()), off_servers: Some(vec!["cloudflare".into()]), ..Default::default() },
            0,
        )
        .unwrap();
        assert_eq!(launch.call(0).command, expected);
        assert!(launch.call(0).command.contains("-c mcp_servers.cloudflare.enabled=false"));
    }

    #[tokio::test]
    async fn a_turn_on_a_machine_whose_credentials_the_host_keeps_writes_its_placeholder_logi() {
        tokio::task::LocalSet::new()
            .run_until(a_turn_on_a_machine_whose_credentials_the_host_keeps_writes_its_placeholder_logi_body())
            .await
    }

    async fn a_turn_on_a_machine_whose_credentials_the_host_keeps_writes_its_placeholder_logi_body() {
        let launch = launcher(turn_server());
        let broker = CodexBroker {
            mode: BrokerMode::Chatgpt {
                claims: CodexClaims {
                    account_id: "11111111-2222-4333-8444-555555555555".into(),
                    plan_type: Some("plus".into()),
                    user_id: Some("user-acme0001".into()),
                    fedramp: Some(false),
                },
                models: None,
            },
            providers: None,
        };
        // Pinned, so the line the turn ran and the one built here stamp the same exp and last_refresh.
        let mut deps = CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN);
        deps.broker = Some(broker.clone());
        deps.now = Some(Arc::new(|| 1_791_900_000_000));
        create_codex_adapter(deps)
            .unwrap()
            .start(HarnessStartOptions { cwd: Some("/root/app".into()), ..opts("list the repo", no_events()) })
            .unwrap()
            .finished()
            .await;
        let expected = build_command(
            &BuildCommandOptions { cwd: Some("/root/app".into()), broker: Some(broker), ..Default::default() },
            1_791_900_000_000,
        )
        .unwrap();
        let call = launch.call(0);
        assert_eq!(call.command, expected);
        let written = call.command.find("> \"$CODEX_HOME/auth.json\" ) && ").expect("no auth.json write");
        assert!(written > 0);
        assert!(written < call.command.find("app-server").expect("no app-server"));
        let env: IndexMap<String, String> = [
            ("CODEX_HOME".to_string(), "/root/.codex".to_string()),
            ("CODEX_REFRESH_TOKEN_URL_OVERRIDE".to_string(), format!("http://127.0.0.1:{BROKER_PORT}/oauth/token")),
            ("CODEX_REVOKE_TOKEN_URL_OVERRIDE".to_string(), format!("http://127.0.0.1:{BROKER_PORT}/oauth/revoke")),
        ]
        .into_iter()
        .collect();
        assert_eq!(call.env, env);
        assert!(call.env.get("CODEX_API_KEY").is_none());
    }

    #[tokio::test]
    async fn takes_its_prompt_late_the_server_starts_and_opens_the_thread_at_once_and_the_tur() {
        tokio::task::LocalSet::new()
            .run_until(takes_its_prompt_late_the_server_starts_and_opens_the_thread_at_once_and_the_tur_body())
            .await
    }

    async fn takes_its_prompt_late_the_server_starts_and_opens_the_thread_at_once_and_the_tur_body() {
        // The launch's snapshot of the folder is what the prompt waits on, and it took 0.46 to 1.6 s on a one-file repo
        // (measured 2026-10-05); before the server boots in that window, every send paid it in full first.
        let launch = launcher(turn_server());
        let adapter = plain(&launch);
        assert!(adapter.waits_for_prompt());
        let (due, after) = gate();
        let session = adapter.start(HarnessStartOptions { prompt_after: Some(after), ..opts("list the repo", no_events()) }).unwrap();
        until(|| launch.wire_count() > 0 && wire0(&launch).wrote("thread/start")).await;
        sleep_ms(5).await;
        assert!(!wire0(&launch).wrote("turn/start"));
        due.send(()).unwrap();
        assert_eq!(session.finished().await.status, TurnStatus::Completed);
        let params = turn_start_params(&wire0(&launch));
        assert_eq!(params["threadId"], json!(RECORDED_THREAD));
        assert_eq!(params["input"], json!([{"type":"text","text":"list the repo"}]));
    }

    #[tokio::test]
    async fn reads_a_recorded_turn_session_start_unprompted_as_the_thread_opens_and_again_onc() {
        tokio::task::LocalSet::new()
            .run_until(reads_a_recorded_turn_session_start_unprompted_as_the_thread_opens_and_again_onc_body())
            .await
    }

    async fn reads_a_recorded_turn_session_start_unprompted_as_the_thread_opens_and_again_onc_body() {
        let launch = launcher(turn_server());
        let events = Events::default();
        let slot: Arc<Mutex<Option<CodexSession>>> = Arc::new(Mutex::new(None));
        let base = events.on_event();
        let held = slot.clone();
        // Answered as the recording's person answered it: the command ran once it was allowed.
        let answering: OnEvent = Arc::new(move |e| {
            let ask_id = match &e {
                AdapterEvent::PermissionAsk { ask, .. } => Some(ask.ask_id.clone()),
                _ => None,
            };
            base(e);
            if let Some(ask_id) = ask_id {
                let session = held.lock().unwrap().clone().expect("the session is not held yet");
                tokio::task::spawn_local(session.answer(ask_id, allow()));
            }
        });
        let session = plain(&launch)
            .start(HarnessStartOptions {
                permission_mode: Some("read-only".into()),
                ..opts("Run the shell command `touch hi.txt` in this folder, then reply with one short line.", answering)
            })
            .unwrap();
        *slot.lock().unwrap() = Some(session.clone());
        let result = session.finished().await;

        assert_eq!(
            events.kinds(),
            vec![
                "session.start",
                "delta:note",
                "session.start",
                "turn.anchor",
                "delta:text",
                "delta:tool_use",
                "permission.ask",
                "permission.close",
                "delta:tool_result",
                "turn.usage",
                "limit",
                "delta:text",
                "turn.usage",
                "limit",
                "turn.done",
                "session.end",
            ]
        );
        let all = events.all();
        assert_eq!(
            all[0],
            AdapterEvent::SessionStart {
                session_id: RECORDED_THREAD.into(),
                model: Some("gpt-5.6-sol".into()),
                cwd: Some("/private/tmp/b7-real".into()),
                tools: None,
                harness: None,
                prompted: Some(false)
            }
        );
        assert_eq!(
            all[2],
            AdapterEvent::SessionStart {
                session_id: RECORDED_THREAD.into(),
                model: Some("gpt-5.6-sol".into()),
                cwd: Some("/private/tmp/b7-real".into()),
                tools: None,
                harness: None,
                prompted: None
            }
        );
        let deltas = events.deltas();
        assert!(deltas[0].text.starts_with("loading hooks from both "), "{}", deltas[0].text);
        assert_eq!(deltas[1].text, "I\u{2019}ll create `hi.txt` in the current folder.");
        assert_eq!(deltas[1].message_id.as_deref(), Some("msg_0567d7bf2c7ea0de016ab930865e9087d09774946388412886"));
        assert_eq!(deltas[2].tool_name.as_deref(), Some("command_execution"));
        assert_eq!(deltas[2].tool_use_id.as_deref(), Some(RECORDED_COMMAND));
        assert_eq!(deltas[2].text, json!({"command":"touch hi.txt"}).to_string());
        // The recording's two rolling updates, each a plan window pair on a Plus plan, epoch seconds read as ms.
        let limit = all
            .iter()
            .filter_map(|e| match e {
                AdapterEvent::Limit { limit, .. } => Some(limit.clone()),
                _ => None,
            })
            .next_back()
            .expect("no limit");
        assert_eq!(
            limit,
            HarnessLimit {
                windows: vec![
                    LimitWindow { kind: LimitKind::Session, used_percent: JsNumber(9.0), resets_at: js(1_790_539_090_000) },
                    LimitWindow { kind: LimitKind::Week, used_percent: JsNumber(5.0), resets_at: js(1_791_094_756_000) },
                ],
                plan: Some("plus".into()),
                status: Some(LimitStatus::Ok),
                account: None,
                keyed: None,
                credits: None,
            }
        );
        let ask = first_ask(&events);
        assert_eq!(ask.ask_id, "0");
        assert_eq!(ask.tool_name, "command_execution");
        assert_eq!(ask.tool_use_id.as_deref(), Some(RECORDED_COMMAND));
        assert_eq!(ask.detail.as_deref(), Some("Allow me to create hi.txt in the current folder?"));
        assert_eq!(serde_json::from_str::<Value>(&ask.input).unwrap(), json!({"command":"touch hi.txt","cwd":"/private/tmp/b7-real"}));
        assert!(launch.wire(0).written().contains(&json!({"id":0,"result":{"decision":"accept"}})));
        // The server's own resolved notice for that request finds nothing open, so the prompt closes once.
        assert_eq!(
            events.of("permission.close"),
            vec![AdapterEvent::PermissionClose {
                session_id: RECORDED_THREAD.into(),
                ask_id: "0".into(),
                outcome: PermissionOutcome::Allowed,
                option_id: Some(PERMISSION_ALLOW.into())
            }]
        );
        assert_eq!(deltas[3].kind, DeltaKind::ToolResult);
        assert_eq!(deltas[3].tool_use_id.as_deref(), Some(RECORDED_COMMAND));
        assert_eq!(deltas[3].text, "");
        assert_eq!(deltas[3].is_error, Some(false));
        assert_eq!(deltas[3].exit_code, Some(0));
        assert_eq!(deltas[3].duration_ms, Some(0.0));
        assert_eq!(deltas[4].kind, DeltaKind::Text);
        assert_eq!(deltas[4].text, "Created hi.txt in this folder.");
        for d in &deltas {
            assert_eq!(d.session_id, RECORDED_THREAD);
        }

        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("Created hi.txt in this folder."));
        // The turn's own counts are the thread's running total less what it stood at before the turn's first call; what
        // the model held is the last call's, and the window is the one the server names.
        assert_eq!(result.tokens, Some(tokens(41_624, 31_744, 0, 124, 31, 20_913, 258_400)));
        assert_eq!(result.model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(
            last_event(&events),
            AdapterEvent::SessionEnd { session_id: RECORDED_THREAD.into(), exit_code: Some(0), saw_result: true, failure: None }
        );
        assert_eq!(session.thread_id(), RECORDED_THREAD);
        assert_ne!(session.local_id(), RECORDED_THREAD);
    }

    #[tokio::test]
    async fn an_mcp_call_the_tool_failed_keeps_the_tool_s_words_one_that_never_reached_it_kee() {
        tokio::task::LocalSet::new()
            .run_until(an_mcp_call_the_tool_failed_keeps_the_tool_s_words_one_that_never_reached_it_kee_body())
            .await
    }

    async fn an_mcp_call_the_tool_failed_keeps_the_tool_s_words_one_that_never_reached_it_kee_body() {
        let said = json!([{"type":"text","text":"slate refused: 1 error: P100 bad-syntax"}]);
        let call = |id: &str, result: Value, error: Value| {
            item_note(
                "completed",
                json!({"type":"mcpToolCall","id":id,"server":"wsp","tool":"slate_write","arguments":{},"status":"failed","result":result,"error":error}),
            )
        };
        let launch = launcher(scripted(
            vec![
                call("call_4", json!({"content":said}), Value::Null),
                call("call_5", Value::Null, json!({"message":"server gone"})),
                call("call_6", Value::Null, Value::Null),
                completed("completed"),
            ],
            WireOpts::default(),
        ));
        let events = Events::default();
        plain(&launch).start(opts("x", events.on_event())).unwrap().finished().await;
        let got: Vec<(Option<String>, String, Option<bool>)> =
            tool_results(&events).into_iter().map(|d| (d.tool_use_id, d.text, d.is_error)).collect();
        assert_eq!(
            got,
            vec![
                (Some("call_4".to_string()), serde_json::to_string(&said).unwrap(), Some(true)),
                (Some("call_5".to_string()), "server gone".to_string(), Some(true)),
                (Some("call_6".to_string()), String::new(), Some(true)),
            ]
        );
    }

    #[tokio::test]
    async fn file_changes_mcp_calls_and_web_searches_which_that_turn_made_none_of_draw_as_cod() {
        tokio::task::LocalSet::new()
            .run_until(file_changes_mcp_calls_and_web_searches_which_that_turn_made_none_of_draw_as_cod_body())
            .await
    }

    async fn file_changes_mcp_calls_and_web_searches_which_that_turn_made_none_of_draw_as_cod_body() {
        // Written from the 0.155.1 schema's ThreadItem variants, since the recorded turn ran one command and no other tool.
        let change = json!({"type":"fileChange","id":"call_2","changes":[{"path":"README.md","kind":{"type":"update","move_path":null},"diff":""},{"path":"docs/new.md","kind":{"type":"add"},"diff":""}]});
        let mcp = json!({"type":"mcpToolCall","id":"call_3","server":"wsp","tool":"threads","arguments":{"workspace":"first"}});
        let search = json!({"type":"webSearch","id":"ws_1","query":"codex app-server"});
        let with = |base: &Value, extra: Value| {
            let mut v = base.clone();
            for (k, x) in extra.as_object().unwrap() {
                v.as_object_mut().unwrap().insert(k.clone(), x.clone());
            }
            v
        };
        let launch = launcher(scripted(
            vec![
                item_note("started", with(&change, json!({"status":"inProgress"}))),
                item_note("completed", with(&change, json!({"status":"completed"}))),
                item_note("started", with(&mcp, json!({"status":"inProgress","result":null,"error":null}))),
                item_note(
                    "completed",
                    with(&mcp, json!({"status":"completed","result":{"content":[{"type":"text","text":"{\"threads\":[]}"}]},"error":null})),
                ),
                item_note("completed", json!({"type":"reasoning","id":"rs_1","summary":["Listing the repository first."],"content":[]})),
                item_note("started", search.clone()),
                item_note("completed", search),
                completed("completed"),
            ],
            WireOpts::default(),
        ));
        let events = Events::default();
        plain(&launch).start(opts("x", events.on_event())).unwrap().finished().await;
        let deltas = events.deltas();
        // Changes whose diff the schema let be empty made no hunks to carry.
        assert!(deltas
            .iter()
            .find(|d| d.kind == DeltaKind::ToolResult && d.tool_use_id.as_deref() == Some("call_2"))
            .and_then(|d| d.patch.clone())
            .is_none());
        let got: Vec<(DeltaKind, Option<String>, Option<String>, String)> =
            deltas.into_iter().map(|d| (d.kind, d.tool_name, d.tool_use_id, d.text)).collect();
        let s = |v: &str| Some(v.to_string());
        assert_eq!(
            got,
            vec![
                (
                    DeltaKind::ToolUse,
                    s("file_change"),
                    s("call_2"),
                    json!({"changes":[{"path":"README.md","kind":"update"},{"path":"docs/new.md","kind":"add"}]}).to_string()
                ),
                (DeltaKind::ToolResult, None, s("call_2"), "update README.md\nadd docs/new.md".to_string()),
                (DeltaKind::ToolUse, s("mcp__wsp__threads"), s("call_3"), json!({"workspace":"first"}).to_string()),
                (DeltaKind::ToolResult, None, s("call_3"), json!([{"type":"text","text":"{\"threads\":[]}"}]).to_string()),
                (DeltaKind::Thinking, None, None, "Listing the repository first.".to_string()),
                (DeltaKind::ToolUse, s("web_search"), s("ws_1"), json!({"query":"codex app-server"}).to_string()),
                (DeltaKind::ToolResult, None, s("ws_1"), "codex app-server".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn a_finished_command_carries_its_exit_code_and_how_long_it_ran_and_each_file_chang() {
        tokio::task::LocalSet::new()
            .run_until(a_finished_command_carries_its_exit_code_and_how_long_it_ran_and_each_file_chang_body())
            .await
    }

    async fn a_finished_command_carries_its_exit_code_and_how_long_it_ran_and_each_file_chang_body() {
        // Written from the 0.162.1 schema: an update's diff as `similar` writes it (format_file_change_diff), an add's
        // and a delete's the whole file, and a rename's update followed by its Moved to line.
        let failing = json!({"type":"commandExecution","id":"call_8","command":"/bin/zsh -lc 'npm test'","cwd":"/root/lab","status":"failed","commandActions":[],"aggregatedOutput":"1 failing\n","exitCode":1,"durationMs":1534});
        let changes = json!([
            {"path":"/root/lab/f.txt","kind":{"type":"update","move_path":null},"diff":"@@ -1,3 +1,3 @@\n alpha\n-beta\n+BETA\n gamma\n"},
            {"path":"/root/lab/new.txt","kind":{"type":"add"},"diff":"hi\nthere\n"},
            {"path":"/root/lab/old.txt","kind":{"type":"delete"},"diff":"gone\n"},
            {"path":"/root/lab/a.txt","kind":{"type":"update","move_path":"/root/lab/b.txt"},"diff":"@@ -2 +2 @@\n-x\n+y\n\n\nMoved to: /root/lab/b.txt"},
        ]);
        let launch = launcher(scripted(
            vec![
                item_note("completed", failing),
                item_note("completed", json!({"type":"fileChange","id":"call_9","changes":changes,"status":"completed"})),
                item_note("completed", json!({"type":"fileChange","id":"call_10","changes":changes,"status":"declined"})),
                completed("completed"),
            ],
            WireOpts::default(),
        ));
        let events = Events::default();
        plain(&launch).start(opts("x", events.on_event())).unwrap().finished().await;
        let results = tool_results(&events);
        let (ran, changed, declined) = (&results[0], &results[1], &results[2]);
        assert_eq!(declined.tool_use_id.as_deref(), Some("call_10"));
        assert_eq!(declined.is_error, Some(true));
        assert!(declined.patch.is_none());
        assert_eq!(ran.tool_use_id.as_deref(), Some("call_8"));
        assert_eq!(ran.text, "1 failing\n");
        assert_eq!(ran.is_error, Some(true));
        assert_eq!(ran.exit_code, Some(1));
        assert_eq!(ran.duration_ms, Some(1534.0));
        let lines = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        assert_eq!(
            changed.patch,
            Some(vec![
                FilePatch {
                    path: "/root/lab/f.txt".into(),
                    hunks: vec![PatchHunk {
                        old_start: 1.into(),
                        old_lines: 3.into(),
                        new_start: 1.into(),
                        new_lines: 3.into(),
                        lines: lines(&[" alpha", "-beta", "+BETA", " gamma"])
                    }],
                    moved_to: None
                },
                FilePatch {
                    path: "/root/lab/new.txt".into(),
                    hunks: vec![PatchHunk {
                        old_start: 0.into(),
                        old_lines: 0.into(),
                        new_start: 1.into(),
                        new_lines: 2.into(),
                        lines: lines(&["+hi", "+there"])
                    }],
                    moved_to: None
                },
                FilePatch {
                    path: "/root/lab/old.txt".into(),
                    hunks: vec![PatchHunk {
                        old_start: 1.into(),
                        old_lines: 1.into(),
                        new_start: 0.into(),
                        new_lines: 0.into(),
                        lines: lines(&["-gone"])
                    }],
                    moved_to: None
                },
                FilePatch {
                    path: "/root/lab/a.txt".into(),
                    hunks: vec![PatchHunk {
                        old_start: 2.into(),
                        old_lines: 1.into(),
                        new_start: 2.into(),
                        new_lines: 1.into(),
                        lines: lines(&["-x", "+y"])
                    }],
                    moved_to: Some("/root/lab/b.txt".into())
                },
            ])
        );
    }

    #[tokio::test]
    async fn names_a_call_to_the_slate_s_second_server_by_wsp_s_own_name_which_is_the_server() {
        tokio::task::LocalSet::new().run_until(names_a_call_to_the_slate_s_second_server_by_wsp_s_own_name_which_is_the_server_body()).await
    }

    async fn names_a_call_to_the_slate_s_second_server_by_wsp_s_own_name_which_is_the_server_body() {
        let slate = json!({"type":"mcpToolCall","id":"call_7","server":SLATE_SERVER_NAME,"tool":"slate_write","arguments":{"text":"<slate/>"},"status":"inProgress","result":null,"error":null});
        let launch = launcher(scripted(vec![item_note("started", slate), completed("completed")], WireOpts::default()));
        let events = Events::default();
        plain(&launch).start(opts("x", events.on_event())).unwrap().finished().await;
        let uses: Vec<Option<String>> = events.deltas().into_iter().filter(|d| d.kind == DeltaKind::ToolUse).map(|d| d.tool_name).collect();
        assert_eq!(uses, vec![Some("mcp__wsp__slate_write".to_string())]);
    }

    #[tokio::test]
    async fn resumes_a_thread_by_the_id_the_row_holds_the_registry_key_is_that_id_and_the_see() {
        tokio::task::LocalSet::new()
            .run_until(resumes_a_thread_by_the_id_the_row_holds_the_registry_key_is_that_id_and_the_see_body())
            .await
    }

    async fn resumes_a_thread_by_the_id_the_row_holds_the_registry_key_is_that_id_and_the_see_body() {
        let launch = launcher(turn_server());
        let adapter = plain(&launch);
        let events = Events::default();
        let session = adapter
            .start(HarnessStartOptions {
                resume: Some(RECORDED_THREAD.into()),
                permission_mode: Some("read-only".into()),
                cwd: Some("/root/app".into()),
                ..opts("next", events.on_event())
            })
            .unwrap();
        session.finished().await;
        assert_eq!(session.local_id(), RECORDED_THREAD);
        let registered = adapter.sessions().get(RECORDED_THREAD).cloned().expect("not in the registry");
        assert!(Arc::ptr_eq(&registered.core, &session.core));
        let input = launch.call(0).input.unwrap();
        assert_eq!(
            parse(&input[4]),
            json!({"id":"wsp-thread","method":"thread/resume","params":{"threadId":RECORDED_THREAD,"excludeTurns":true,"cwd":"/root/app","sandbox":"read-only","approvalPolicy":"on-request"}})
        );
        let ids: std::collections::HashSet<String> = events.all().iter().map(sid).collect();
        assert_eq!(ids, [RECORDED_THREAD.to_string()].into_iter().collect());
    }

    #[tokio::test]
    async fn images_ride_the_turn_as_local_paths_and_one_with_no_path_on_the_machine_is_refus() {
        tokio::task::LocalSet::new()
            .run_until(images_ride_the_turn_as_local_paths_and_one_with_no_path_on_the_machine_is_refus_body())
            .await
    }

    async fn images_ride_the_turn_as_local_paths_and_one_with_no_path_on_the_machine_is_refus_body() {
        let launch = launcher(turn_server());
        let image =
            TurnImage { media_type: "image/png".into(), bytes: String::new(), path: Some("/root/.wsp/threads/thr_1/images/1.png".into()) };
        plain(&launch).start(HarnessStartOptions { images: vec![image], ..opts("what is this?", no_events()) }).unwrap().finished().await;
        assert_eq!(
            turn_start_params(&launch.wire(0))["input"],
            json!([{"type":"text","text":"what is this?"},{"type":"localImage","path":"/root/.wsp/threads/thr_1/images/1.png"}])
        );

        let none = launcher(turn_server());
        let no_path = TurnImage { media_type: "image/png".into(), bytes: "aGk=".into(), path: None };
        let message = refused(plain(&none).start(HarnessStartOptions { images: vec![no_path], ..opts("x", no_events()) }));
        assert!(message.contains("no path on it"), "{message}");
        let relative = TurnImage { media_type: "image/png".into(), bytes: String::new(), path: Some("shot.png".into()) };
        let message = refused(plain(&none).start(HarnessStartOptions { images: vec![relative], ..opts("x", no_events()) }));
        assert!(message.contains("must be one absolute path"), "{message}");
        assert!(none.calls().is_empty());
    }

    #[test]
    fn refuses_a_context_window_which_codex_has_no_setting_for() {
        let launch = launcher(server(vec![], WireOpts::default()));
        let message = refused(plain(&launch).start(HarnessStartOptions { context_window: Some("1m".into()), ..opts("x", no_events()) }));
        assert!(message.contains("codex takes no context window"), "{message}");
    }
}

mod a_message_sent_while_a_codex_turn_runs {
    use super::*;

    #[tokio::test]
    async fn steers_the_running_turn_by_its_id_and_reads_accepted_off_the_server_s_answer() {
        tokio::task::LocalSet::new().run_until(steers_the_running_turn_by_its_id_and_reads_accepted_off_the_server_s_answer_body()).await
    }

    async fn steers_the_running_turn_by_its_id_and_reads_accepted_off_the_server_s_answer_body() {
        let live = launcher(scripted_server(on_write(|m, w| match method_of(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => w.push1(turn_started()),
            "turn/steer" => w.push1(answer_with(&m["id"], json!({"turnId":TURN_ID}))),
            _ => {}
        })));
        let adapter = plain(&live);
        assert!(adapter.steers());
        let session = adapter.start(opts("count to 40", no_events())).unwrap();
        until(|| live.wire_count() > 0 && wire0(&live).wrote("turn/start")).await;
        sleep_ms(5).await;
        assert_eq!(session.steer("stop at 12".to_string()).await, SteerOutcome::Accepted);
        let steer = wire0(&live).written_method("turn/steer")[0].clone();
        assert_eq!(steer["params"], json!({"threadId":THREAD_ID,"expectedTurnId":TURN_ID,"input":[{"type":"text","text":"stop at 12"}]}));
        wire0(&live).push(&[agent_message("msg_1", "stopped at 12"), completed("completed")]);
        let result = session.finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("stopped at 12"));
    }

    #[tokio::test]
    async fn reads_not_running_when_the_server_turns_the_steer_down_and_after_the_turn_ended() {
        tokio::task::LocalSet::new().run_until(reads_not_running_when_the_server_turns_the_steer_down_and_after_the_turn_ended_body()).await
    }

    async fn reads_not_running_when_the_server_turns_the_steer_down_and_after_the_turn_ended_body() {
        let live = launcher(scripted_server(on_write(|m, w| match method_of(m) {
            "thread/start" => w.push(&opened()),
            "turn/steer" => w.push1(json!({"error":{"code":-32600,"message":"no active turn"},"id":m["id"]}).to_string()),
            _ => {}
        })));
        let session = plain(&live).start(opts("x", no_events())).unwrap();
        wire0(&live).push1(turn_started());
        sleep_ms(5).await;
        assert_eq!(session.steer("refused".to_string()).await, SteerOutcome::NotRunning);
        wire0(&live).push1(completed("completed"));
        session.finished().await;
        assert_eq!(session.steer("after the end".to_string()).await, SteerOutcome::NotRunning);
    }

    #[tokio::test]
    async fn a_steer_whose_write_lost_its_answer_reads_the_server_s_own_answer_accepted_where() {
        tokio::task::LocalSet::new()
            .run_until(a_steer_whose_write_lost_its_answer_reads_the_server_s_own_answer_accepted_where_body())
            .await
    }

    async fn a_steer_whose_write_lost_its_answer_reads_the_server_s_own_answer_accepted_where_body() {
        let losing: Arc<Mutex<Option<Fault>>> = Arc::new(Mutex::new(None));
        let lose = losing.clone();
        let fault: codex_wire::WriteFault = Arc::new(move |m: &Value| {
            let loses = if method_of(m) == "turn/steer" { *lose.lock().unwrap() } else { None };
            loses.map(|f| (f, failure("remote write failed on m1: nothing came back saying WSP_OK")))
        });
        let live = launcher(scripted_server(WireOpts {
            fault: Some(fault),
            ..on_write(|m, w| match method_of(m) {
                "thread/start" => w.push(&opened()),
                "turn/start" => w.push1(turn_started()),
                "turn/steer" => w.push1(answer_with(&m["id"], json!({"turnId":TURN_ID}))),
                _ => {}
            })
        }));
        let session = plain(&live).start(opts("count to 40", no_events())).unwrap();
        until(|| live.wire_count() > 0 && wire0(&live).wrote("turn/start")).await;
        sleep_ms(5).await;
        *losing.lock().unwrap() = Some(Fault::Landed);
        assert_eq!(session.steer("stop at 12".to_string()).await, SteerOutcome::Accepted);
        *losing.lock().unwrap() = Some(Fault::Lost);
        let unanswered = session.steer("stop at 20".to_string());
        wire0(&live).push(&[agent_message("msg_1", "stopped at 12"), completed("completed")]);
        assert_eq!(unanswered.await, SteerOutcome::NotRunning);
        assert_eq!(wire0(&live).written_method("turn/steer").len(), 1);
        session.finished().await;
    }
}

mod an_approval_codex_asks_for {
    use super::*;

    fn approving(request: Value, also: Option<OnWrite>) -> Launch {
        let request = request.to_string();
        let started = item_note(
            "started",
            json!({"type":"fileChange","id":"call_9","changes":[{"path":"hi.txt","kind":{"type":"add"},"diff":"+hi\n"}],"status":"inProgress"}),
        );
        launcher(scripted_server(on_write(move |m, w| {
            if let Some(also) = &also {
                also(m, w);
            }
            match method_of(m) {
                "thread/start" => w.push(&opened()),
                "turn/start" => w.push(&[turn_started(), started.clone(), request.clone()]),
                _ => {}
            }
        })))
    }

    fn file_ask() -> Value {
        json!({"id":0,"method":"item/fileChange/requestApproval","params":{"threadId":THREAD_ID,"turnId":TURN_ID,"itemId":"call_9","reason":"write outside the sandbox","startedAtMs":1}})
    }

    fn command_ask() -> Value {
        json!({"id":"ask-7","method":"item/commandExecution/requestApproval","params":{"threadId":THREAD_ID,"turnId":TURN_ID,"itemId":"call_4","command":"rm -rf build","cwd":"/root/app","startedAtMs":1,"proposedExecpolicyAmendment":["rm"]}})
    }

    fn deny(reason: Option<&str>, message: &str) -> PermissionAnswer {
        PermissionAnswer {
            option_id: PERMISSION_DENY.into(),
            outcome: PermissionOutcome::Denied,
            deny_message: message.into(),
            reason: reason.map(String::from),
        }
    }

    fn close(ask_id: &str, outcome: PermissionOutcome, option: Option<&str>) -> AdapterEvent {
        AdapterEvent::PermissionClose { session_id: THREAD_ID.into(), ask_id: ask_id.into(), outcome, option_id: option.map(String::from) }
    }

    #[tokio::test]
    async fn raises_a_file_change_as_a_prompt_with_allow_and_deny_the_change_s_paths_as_its_i() {
        tokio::task::LocalSet::new()
            .run_until(raises_a_file_change_as_a_prompt_with_allow_and_deny_the_change_s_paths_as_its_i_body())
            .await
    }

    async fn raises_a_file_change_as_a_prompt_with_allow_and_deny_the_change_s_paths_as_its_i_body() {
        let live = approving(file_ask(), None);
        let events = Events::default();
        let session = plain(&live)
            .start(HarnessStartOptions { permission_mode: Some("read-only".into()), ..opts("write hi into hi.txt", events.on_event()) })
            .unwrap();
        until(|| events.any("permission.ask")).await;
        assert_eq!(
            first_ask(&events),
            PermissionAsk {
                ask_id: "0".into(),
                tool_name: "file_change".into(),
                tool_use_id: Some("call_9".into()),
                parent_tool_use_id: None,
                input: json!({"changes":[{"path":"hi.txt","kind":"add"}]}).to_string(),
                detail: Some("write outside the sandbox".into()),
                options: vec![
                    PermissionOption { id: PERMISSION_ALLOW.into(), label: "Allow".into(), effect: PermissionEffect::Allow, mode: None },
                    PermissionOption { id: PERMISSION_DENY.into(), label: "Deny".into(), effect: PermissionEffect::Deny, mode: None },
                ],
            }
        );
        assert_eq!(session.answer("0".to_string(), allow()).await.unwrap(), AnswerOutcome::Answered);
        assert_eq!(wire0(&live).written().last().cloned(), Some(json!({"id":0,"result":{"decision":"accept"}})));
        assert_eq!(last_event(&events), close("0", PermissionOutcome::Allowed, Some(PERMISSION_ALLOW)));
        assert_eq!(session.answer("0".to_string(), allow()).await.unwrap(), AnswerOutcome::Gone);
        wire0(&live).push1(completed("completed"));
        session.finished().await;
    }

    #[tokio::test]
    async fn a_deny_the_person_gave_a_reason_for_declines_the_call_and_puts_the_reason_to_the() {
        tokio::task::LocalSet::new()
            .run_until(a_deny_the_person_gave_a_reason_for_declines_the_call_and_puts_the_reason_to_the_body())
            .await
    }

    async fn a_deny_the_person_gave_a_reason_for_declines_the_call_and_puts_the_reason_to_the_body() {
        let also: OnWrite = Arc::new(|m, w| {
            if method_of(m) == "turn/steer" {
                w.push1(answer_with(&m["id"], json!({"turnId":TURN_ID})));
            }
        });
        let live = approving(command_ask(), Some(also));
        let events = Events::default();
        let session = plain(&live)
            .start(HarnessStartOptions { permission_mode: Some("workspace-write".into()), ..opts("clean", events.on_event()) })
            .unwrap();
        until(|| events.any("permission.ask")).await;
        assert_eq!(session.answer("ask-7".to_string(), deny(Some("Only clean dist"), "unused")).await.unwrap(), AnswerOutcome::Answered);
        assert!(wire0(&live).written().contains(&json!({"id":"ask-7","result":{"decision":"decline"}})));
        until(|| wire0(&live).wrote("turn/steer")).await;
        let steer = wire0(&live).written_method("turn/steer")[0].clone();
        assert!(steer["params"].to_string().contains("Only clean dist"));
        wire0(&live).push1(completed("completed"));
        session.finished().await;
    }

    #[tokio::test]
    async fn answers_a_command_s_deny_with_decline_under_the_server_s_own_id_never_widening_t() {
        tokio::task::LocalSet::new()
            .run_until(answers_a_command_s_deny_with_decline_under_the_server_s_own_id_never_widening_t_body())
            .await
    }

    async fn answers_a_command_s_deny_with_decline_under_the_server_s_own_id_never_widening_t_body() {
        let live = approving(command_ask(), None);
        let events = Events::default();
        let session = plain(&live)
            .start(HarnessStartOptions { permission_mode: Some("workspace-write".into()), ..opts("clean", events.on_event()) })
            .unwrap();
        until(|| events.any("permission.ask")).await;
        let ask = first_ask(&events);
        assert_eq!(ask.ask_id, "ask-7");
        assert_eq!(ask.tool_name, "command_execution");
        assert_eq!(ask.tool_use_id.as_deref(), Some("call_4"));
        assert_eq!(ask.input, json!({"command":"rm -rf build","cwd":"/root/app"}).to_string());
        assert_eq!(session.answer("ask-7".to_string(), deny(None, "not now")).await.unwrap(), AnswerOutcome::Answered);
        assert_eq!(wire0(&live).written().last().cloned(), Some(json!({"id":"ask-7","result":{"decision":"decline"}})));
        wire0(&live).push1(completed("completed"));
        session.finished().await;
        let widening = regex::Regex::new("acceptForSession|Execpolicy|NetworkPolicy").unwrap();
        for m in wire0(&live).written() {
            if let Some(result) = m.get("result") {
                assert!(!widening.is_match(&result.to_string()), "{result}");
            }
        }
    }

    #[tokio::test]
    async fn an_answer_whose_write_failed_leaves_the_request_open_so_answering_again_lands_it() {
        tokio::task::LocalSet::new()
            .run_until(an_answer_whose_write_failed_leaves_the_request_open_so_answering_again_lands_it_body())
            .await
    }

    async fn an_answer_whose_write_failed_leaves_the_request_open_so_answering_again_lands_it_body() {
        let refused = failure("Boat is refusing requests from this computer's address");
        let failing = Arc::new(AtomicBool::new(true));
        let error = refused.clone();
        let fault: codex_wire::WriteFault = Arc::new(move |m: &Value| {
            if m.get("result").is_some() && failing.swap(false, Ordering::SeqCst) {
                Some((Fault::Lost, error.clone()))
            } else {
                None
            }
        });
        let live = launcher(scripted_server(WireOpts {
            fault: Some(fault),
            ..on_write(|m, w| match method_of(m) {
                "thread/start" => w.push(&opened()),
                "turn/start" => w.push(&[turn_started(), command_ask().to_string()]),
                _ => {}
            })
        }));
        let events = Events::default();
        let session = plain(&live)
            .start(HarnessStartOptions { permission_mode: Some("workspace-write".into()), ..opts("clean", events.on_event()) })
            .unwrap();
        until(|| events.any("permission.ask")).await;
        let err: ExecFailure = session.answer("ask-7".to_string(), allow()).await.unwrap_err();
        assert_eq!(err, refused);
        let results = |w: &Wire| w.written().into_iter().filter(|m| m.get("result").is_some()).collect::<Vec<Value>>();
        assert_eq!(results(&wire0(&live)), Vec::<Value>::new());
        assert!(!events.any("permission.close"));

        assert_eq!(session.answer("ask-7".to_string(), allow()).await.unwrap(), AnswerOutcome::Answered);
        assert_eq!(results(&wire0(&live)), vec![json!({"id":"ask-7","result":{"decision":"accept"}})]);
        assert_eq!(events.of("permission.close"), vec![close("ask-7", PermissionOutcome::Allowed, Some(PERMISSION_ALLOW))]);
        wire0(&live).push1(completed("completed"));
        session.finished().await;
    }

    #[tokio::test]
    async fn closes_a_prompt_the_server_resolved_itself_and_every_prompt_still_open_when_the() {
        tokio::task::LocalSet::new().run_until(closes_a_prompt_the_server_resolved_itself_and_every_prompt_still_open_when_the_body()).await
    }

    async fn closes_a_prompt_the_server_resolved_itself_and_every_prompt_still_open_when_the_body() {
        let live = approving(file_ask(), None);
        let events = Events::default();
        let session = plain(&live).start(opts("x", events.on_event())).unwrap();
        until(|| events.any("permission.ask")).await;
        wire0(&live).push1(json!({"method":"serverRequest/resolved","params":{"threadId":THREAD_ID,"requestId":0}}).to_string());
        until(|| events.any("permission.close")).await;
        match &events.of("permission.close")[0] {
            AdapterEvent::PermissionClose { ask_id, outcome, .. } => {
                assert_eq!(ask_id, "0");
                assert_eq!(*outcome, PermissionOutcome::Cancelled);
            }
            other => panic!("{other:?}"),
        }

        let second = approving(command_ask(), None);
        let later = Events::default();
        let cut = plain(&second).start(opts("x", later.on_event())).unwrap();
        until(|| later.any("permission.ask")).await;
        wire0(&second).exit(Some(1));
        cut.finished().await;
        match &later.of("permission.close")[0] {
            AdapterEvent::PermissionClose { ask_id, outcome, .. } => {
                assert_eq!(ask_id, "ask-7");
                assert_eq!(*outcome, PermissionOutcome::Cancelled);
            }
            other => panic!("{other:?}"),
        }
        wire0(&live).push1(completed("completed"));
        session.finished().await;
    }

    #[tokio::test]
    async fn refuses_a_server_request_it_does_not_serve_with_an_error_so_the_turn_is_not_left() {
        tokio::task::LocalSet::new()
            .run_until(refuses_a_server_request_it_does_not_serve_with_an_error_so_the_turn_is_not_left_body())
            .await
    }

    async fn refuses_a_server_request_it_does_not_serve_with_an_error_so_the_turn_is_not_left_body() {
        let live = approving(
            json!({"id":5,"method":"item/tool/requestUserInput","params":{"threadId":THREAD_ID,"turnId":TURN_ID,"itemId":"call_5","questions":[]}}),
            None,
        );
        let events = Events::default();
        let session = plain(&live).start(opts("x", events.on_event())).unwrap();
        let answered = |w: &Wire| w.written().into_iter().find(|m| m.get("id") == Some(&json!(5)));
        until(|| live.wire_count() > 0 && answered(&wire0(&live)).is_some()).await;
        assert_eq!(
            answered(&wire0(&live)),
            Some(json!({"id":5,"error":{"code":-32601,"message":"wsp does not answer item/tool/requestUserInput"}}))
        );
        assert!(!events.any("permission.ask"));
        wire0(&live).push1(completed("completed"));
        session.finished().await;
    }
}
