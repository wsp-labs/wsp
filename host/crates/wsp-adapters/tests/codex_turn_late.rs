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

mod a_codex_turn_that_does_not_complete {
    use super::*;

    fn rest(lines: Vec<String>) -> Launch {
        launcher(scripted(lines, WireOpts::default()))
    }

    #[tokio::test]
    async fn a_turn_with_no_sign_in_as_codex_cli_0_155_1_printed_it_is_the_sign_in_line_with() {
        tokio::task::LocalSet::new().run_until(a_turn_with_no_sign_in_as_codex_cli_0_155_1_printed_it_is_the_sign_in_line_with_body()).await
    }

    async fn a_turn_with_no_sign_in_as_codex_cli_0_155_1_printed_it_is_the_sign_in_line_with_body() {
        let launch = launcher(server(fixture_lines("codex", "no-login-app-server.jsonl"), WireOpts::default()));
        let events = Events::default();
        let result = plain(&launch).start(opts("hi", events.on_event())).unwrap().finished().await;
        assert_eq!(not_signed_in(), "Codex is not signed in where this thread runs; run codex login --device-auth there");
        assert_eq!(result.status, TurnStatus::Failed);
        assert_eq!(result.error, Some(not_signed_in()));
        assert_eq!(result.refusal, Some(TurnRefusal::SignIn));
        // The server tried the 401 over its two roads, with its note between them, before it gave up, and said each try.
        let retries = |n: usize| vec!["turn.retry".to_string(); n];
        let mut expected: Vec<String> = vec!["session.start".into(), "session.start".into(), "turn.anchor".into()];
        expected.extend(retries(4));
        expected.push("turn.delta".into());
        expected.extend(retries(5));
        expected.push("turn.done".into());
        expected.push("session.end".into());
        let types: Vec<String> = events.all().iter().map(type_of).collect();
        assert_eq!(types, expected);
        let tries: Vec<String> = events
            .all()
            .iter()
            .filter_map(|e| match e {
                AdapterEvent::TurnRetry { attempt, of, why, .. } => Some(format!("{}/{} {}", attempt, of.unwrap(), why.clone().unwrap())),
                _ => None,
            })
            .collect();
        let want: Vec<String> = [2, 3, 4, 5, 1, 2, 3, 4, 5].iter().map(|n| format!("{n}/5 the provider answered 401")).collect();
        assert_eq!(tries, want);
        assert_eq!(
            last_event(&events),
            AdapterEvent::SessionEnd { session_id: NO_LOGIN_THREAD.into(), exit_code: Some(0), saw_result: true, failure: None }
        );
        assert!(launch.wire(0).closed());
    }

    #[tokio::test]
    async fn a_failed_turn_carries_the_server_s_own_reason_where_wsp_claims_no_cause_for_it() {
        tokio::task::LocalSet::new().run_until(a_failed_turn_carries_the_server_s_own_reason_where_wsp_claims_no_cause_for_it_body()).await
    }

    async fn a_failed_turn_carries_the_server_s_own_reason_where_wsp_claims_no_cause_for_it_body() {
        let launch = rest(vec![
            error_note("stream disconnected before completion", false, None),
            failed_with("stream disconnected before completion"),
        ]);
        let result = run(&plain(&launch), "x").await;
        assert_eq!(result.status, TurnStatus::Failed);
        assert_eq!(result.error.as_deref(), Some("stream disconnected before completion"));
        assert_eq!(result.refusal, None);
    }

    #[tokio::test]
    async fn a_401_on_a_turn_that_was_handed_the_vault_s_key_is_said_as_a_refused_key_with_th() {
        tokio::task::LocalSet::new()
            .run_until(a_401_on_a_turn_that_was_handed_the_vault_s_key_is_said_as_a_refused_key_with_th_body())
            .await
    }

    async fn a_401_on_a_turn_that_was_handed_the_vault_s_key_is_said_as_a_refused_key_with_th_body() {
        let with_key = |launch: &Launch| {
            let mut deps = CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN);
            deps.api_key = Some("sk-x-not-a-key".into());
            deps.key_env = Some("OPENAI_API_KEY".into());
            create_codex_adapter(deps).unwrap()
        };
        let refused_launch = rest(vec![failed_with(
            "unexpected status 401 Unauthorized: token expired, url: https://api.openai.com/v1/responses, cf-ray: x",
        )]);
        let refused = run(&with_key(&refused_launch), "x").await;
        assert_eq!(refused.status, TurnStatus::Failed);
        assert_eq!(refused.error, Some(codex_key_refused_line("OPENAI_API_KEY", "token expired", LOGIN)));
        assert_eq!(refused.refusal, Some(TurnRefusal::SignIn));
        let none_launch = rest(vec![failed_with("unexpected status 401 Unauthorized: token expired")]);
        let none = run(&plain(&none_launch), "x").await;
        assert_eq!(none.status, TurnStatus::Failed);
        assert_eq!(none.error, Some(not_signed_in()));
        assert_eq!(none.refusal, Some(TurnRefusal::SignIn));
    }

    #[tokio::test]
    async fn a_provider_whose_env_key_variable_is_unset_fails_the_turn_naming_that_variable_i() {
        tokio::task::LocalSet::new()
            .run_until(a_provider_whose_env_key_variable_is_unset_fails_the_turn_naming_that_variable_i_body())
            .await
    }

    async fn a_provider_whose_env_key_variable_is_unset_fails_the_turn_naming_that_variable_i_body() {
        let launch = rest(vec![failed_with("Missing environment variable: `FAKE_API_KEY`.")]);
        let result = run(&plain(&launch), "x").await;
        assert_eq!(result.status, TurnStatus::Failed);
        assert_eq!(result.error, Some(codex_missing_env_line("FAKE_API_KEY")));
    }

    #[tokio::test]
    async fn the_last_failure_seen_wins_a_401_in_an_early_retry_then_a_turn_that_fails_for_an() {
        tokio::task::LocalSet::new()
            .run_until(the_last_failure_seen_wins_a_401_in_an_early_retry_then_a_turn_that_fails_for_an_body())
            .await
    }

    async fn the_last_failure_seen_wins_a_401_in_an_early_retry_then_a_turn_that_fails_for_an_body() {
        let launch = rest(vec![
            error_note("Reconnecting... 1/5", true, Some("unexpected status 401 Unauthorized: token expired")),
            agent_message("msg_1", "partway"),
            failed_with("Missing environment variable: `LATER_KEY`."),
        ]);
        let result = run(&plain(&launch), "x").await;
        assert_eq!(result.status, TurnStatus::Failed);
        assert_eq!(result.error, Some(codex_missing_env_line("LATER_KEY")));
        assert_eq!(result.refusal, None);
    }

    #[tokio::test]
    async fn a_server_that_turns_the_thread_down_fails_the_turn_in_its_words_and_sends_no_tur() {
        tokio::task::LocalSet::new()
            .run_until(a_server_that_turns_the_thread_down_fails_the_turn_in_its_words_and_sends_no_tur_body())
            .await
    }

    async fn a_server_that_turns_the_thread_down_fails_the_turn_in_its_words_and_sends_no_tur_body() {
        let launch = launcher(scripted_server(on_write(|m, w| {
            if method_of(m) == "thread/resume" {
                w.push(&[
                    r#"{"id":"wsp-initialize","result":{}}"#.to_string(),
                    r#"{"error":{"code":-32600,"message":"no rollout found for thread id 01a0"},"id":"wsp-thread"}"#.to_string(),
                ]);
            }
        })));
        let events = Events::default();
        let result = plain(&launch)
            .start(HarnessStartOptions { resume: Some(THREAD_ID.into()), ..opts("x", events.on_event()) })
            .unwrap()
            .finished()
            .await;
        assert_eq!(result, failed("codex could not open the thread: no rollout found for thread id 01a0"));
        assert!(!launch.wire(0).wrote("turn/start"));
        match last_event(&events) {
            AdapterEvent::SessionEnd { saw_result, .. } => assert!(saw_result),
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn a_process_that_dies_mid_turn_fails_under_the_thread_s_id_with_its_last_stderr_li() {
        tokio::task::LocalSet::new()
            .run_until(a_process_that_dies_mid_turn_fails_under_the_thread_s_id_with_its_last_stderr_li_body())
            .await
    }

    async fn a_process_that_dies_mid_turn_fails_under_the_thread_s_id_with_its_last_stderr_li_body() {
        let launch = launcher(scripted_server(on_write(|m, w| match method_of(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => {
                w.push(&[turn_started(), "2026-09-27T00:51:50Z ERROR codex_core: sandbox unavailable".to_string()]);
                let dying = w.clone();
                tokio::task::spawn_local(async move {
                    sleep_ms(1).await;
                    dying.exit(Some(2));
                });
            }
            _ => {}
        })));
        let events = Events::default();
        let result = plain(&launch).start(opts("x", events.on_event())).unwrap().finished().await;
        assert_eq!(
            result,
            failed("codex exited with code 2 before its turn ended: 2026-09-27T00:51:50Z ERROR codex_core: sandbox unavailable")
        );
        let got: Vec<(String, String)> = events.all().iter().map(|e| (type_of(e), sid(e))).collect();
        let want: Vec<(String, String)> = ["session.start", "session.start", "turn.anchor", "turn.done", "session.end"]
            .iter()
            .map(|t| (t.to_string(), THREAD_ID.to_string()))
            .collect();
        assert_eq!(got, want);
    }

    #[tokio::test]
    async fn a_transport_that_ends_the_turn_itself_makes_its_message_the_turn_s_error() {
        tokio::task::LocalSet::new().run_until(a_transport_that_ends_the_turn_itself_makes_its_message_the_turn_s_error_body()).await
    }

    async fn a_transport_that_ends_the_turn_itself_makes_its_message_the_turn_s_error_body() {
        let launch = launcher(|_seed: &[String]| {
            let w = codex_wire::wire(WireOpts::default());
            w.push1(opened()[0].clone());
            w.fail(failure("turn cut: idle 10m"));
            w.exit(None);
            w
        });
        let result = run(&plain(&launch), "x").await;
        assert_eq!(result, failed("turn cut: idle 10m"));
    }

    #[tokio::test]
    async fn a_provider_that_never_answers_the_server_retries_forever_so_the_adapter_ends_the() {
        tokio::task::LocalSet::new()
            .run_until(a_provider_that_never_answers_the_server_retries_forever_so_the_adapter_ends_the_body())
            .await
    }

    async fn a_provider_that_never_answers_the_server_retries_forever_so_the_adapter_ends_the_body() {
        let launch = launcher(scripted(vec![reconnecting(), reconnecting()], hang()));
        let events = Events::default();
        let result = adapter_over(&launch, Extra { grace_ms: Some(10), stall_ms: Some(20), ..Default::default() })
            .start(opts("x", events.on_event()))
            .unwrap()
            .finished()
            .await;
        assert_eq!(launch.wire(0).order(), vec!["teardown", "kill"]);
        assert_eq!(result.status, TurnStatus::Failed);
        let error = result.error.clone().expect("no error");
        let shape = regex::Regex::new(r"^stopped after \d+m \d\ds of Codex reconnecting to its model provider with no answer$").unwrap();
        assert!(shape.is_match(&error), "{error}");
        assert_eq!(codex_reconnect_line(90_000.0), "stopped after 1m 30s of Codex reconnecting to its model provider with no answer");
        match last_event(&events) {
            AdapterEvent::SessionEnd { exit_code, saw_result, .. } => {
                assert_eq!(exit_code, None);
                assert!(!saw_result);
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn a_retry_run_that_recovers_into_the_turn_is_not_cut() {
        tokio::task::LocalSet::new().run_until(a_retry_run_that_recovers_into_the_turn_is_not_cut_body()).await
    }

    async fn a_retry_run_that_recovers_into_the_turn_is_not_cut_body() {
        let launch = rest(vec![reconnecting(), agent_message("msg_1", "back"), completed("completed")]);
        let result = adapter_over(&launch, Extra { grace_ms: Some(10), stall_ms: Some(20), ..Default::default() })
            .start(opts("x", no_events()))
            .unwrap()
            .finished()
            .await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("back"));
        assert_eq!(launch.wire(0).order(), Vec::<String>::new());
    }
}

mod a_model_call_codex_tries_again {
    use super::*;

    // As codex 0.155.1's app-server printed them on 2026-10-16 against a provider answering every call 500, and as the
    // no-login fixture holds them for a provider answering 401.
    fn retrying(n: i64, status: Option<i64>) -> String {
        json!({"method":"error","params":{
            "error":{"message":format!("Reconnecting... {n}/5"),"codexErrorInfo":{"responseStreamDisconnected":{"httpStatusCode":status}},"additionalDetails":"We\u{2019}re currently experiencing high demand, which may cause temporary errors.","misalignment":null},
            "willRetry":true,"threadId":THREAD_ID,"turnId":TURN_ID}})
        .to_string()
    }

    #[tokio::test]
    async fn says_each_retry_which_one_of_how_many_and_why_before_the_reply_the_call_that_got() {
        tokio::task::LocalSet::new()
            .run_until(says_each_retry_which_one_of_how_many_and_why_before_the_reply_the_call_that_got_body())
            .await
    }

    async fn says_each_retry_which_one_of_how_many_and_why_before_the_reply_the_call_that_got_body() {
        let events = Events::default();
        let launch = launcher(scripted(
            vec![retrying(1, None), retrying(2, Some(401)), agent_message("msg_1", "back"), completed("completed")],
            WireOpts::default(),
        ));
        let result = plain(&launch).start(opts("x", events.on_event())).unwrap().finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("back"));
        assert_eq!(
            events.of("turn.retry"),
            vec![
                AdapterEvent::TurnRetry {
                    session_id: THREAD_ID.into(),
                    attempt: 1.0,
                    of: Some(5.0),
                    why: Some("the connection dropped".into())
                },
                AdapterEvent::TurnRetry {
                    session_id: THREAD_ID.into(),
                    attempt: 2.0,
                    of: Some(5.0),
                    why: Some("the provider answered 401".into())
                },
            ]
        );
        let kinds = events.kinds();
        let text = kinds.iter().position(|k| k == "delta:text").expect("no text delta");
        let last_retry = kinds.iter().rposition(|k| k == "turn.retry").expect("no retry");
        assert!(text > last_retry);
    }

    #[tokio::test]
    async fn says_nothing_of_an_error_it_will_not_retry_the_turn_s_own_end_says_that_one() {
        tokio::task::LocalSet::new().run_until(says_nothing_of_an_error_it_will_not_retry_the_turn_s_own_end_says_that_one_body()).await
    }

    async fn says_nothing_of_an_error_it_will_not_retry_the_turn_s_own_end_says_that_one_body() {
        let events = Events::default();
        let last = json!({"method":"error","params":{"error":{"message":"We\u{2019}re currently experiencing high demand, which may cause temporary errors.","codexErrorInfo":"internalServerError","additionalDetails":null},"willRetry":false,"threadId":THREAD_ID,"turnId":TURN_ID}})
            .to_string();
        let launch = launcher(scripted(vec![last, completed("failed")], WireOpts::default()));
        plain(&launch).start(opts("x", events.on_event())).unwrap().finished().await;
        assert_eq!(events.of("turn.retry"), Vec::<AdapterEvent>::new());
    }
}

mod interrupt {
    use super::*;

    #[tokio::test]
    async fn asks_the_server_to_interrupt_the_running_turn_which_then_completes_as_interrupte() {
        tokio::task::LocalSet::new()
            .run_until(asks_the_server_to_interrupt_the_running_turn_which_then_completes_as_interrupte_body())
            .await
    }

    async fn asks_the_server_to_interrupt_the_running_turn_which_then_completes_as_interrupte_body() {
        let live = launcher(scripted_server(on_write(|m, w| match method_of(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => w.push1(turn_started()),
            "turn/interrupt" => w.push(&[answer_with(&m["id"], json!({})), completed("interrupted")]),
            _ => {}
        })));
        let events = Events::default();
        let session = adapter_over(&live, Extra { grace_ms: Some(5_000), ..Default::default() })
            .start(opts("loop forever", events.on_event()))
            .unwrap();
        until(|| live.wire_count() > 0 && wire0(&live).wrote("turn/start")).await;
        sleep_ms(5).await;
        session.interrupt().await;
        assert_eq!(session.finished().await, res(TurnStatus::Interrupted));
        assert_eq!(wire0(&live).written_method("turn/interrupt")[0]["params"], json!({"threadId":THREAD_ID,"turnId":TURN_ID}));
        assert_eq!(wire0(&live).order(), Vec::<String>::new());
        match last_event(&events) {
            AdapterEvent::SessionEnd { exit_code, saw_result, .. } => {
                assert_eq!(exit_code, Some(0));
                assert!(saw_result);
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn ends_the_process_with_its_tree_when_the_server_does_not_stop_inside_the_grace_wi() {
        tokio::task::LocalSet::new()
            .run_until(ends_the_process_with_its_tree_when_the_server_does_not_stop_inside_the_grace_wi_body())
            .await
    }

    async fn ends_the_process_with_its_tree_when_the_server_does_not_stop_inside_the_grace_wi_body() {
        let launch = launcher(scripted(vec![], hang()));
        let session =
            adapter_over(&launch, Extra { grace_ms: Some(15), ..Default::default() }).start(opts("loop forever", no_events())).unwrap();
        until(|| launch.wire_count() > 0 && wire0(&launch).wrote("turn/start")).await;
        sleep_ms(5).await;
        session.interrupt().await;
        assert_eq!(session.finished().await, res(TurnStatus::Interrupted));
        assert_eq!(wire0(&launch).order(), vec!["teardown", "kill"]);
    }

    #[tokio::test]
    async fn an_interrupt_before_the_server_started_the_turn_ends_the_process_at_once_with_no() {
        tokio::task::LocalSet::new()
            .run_until(an_interrupt_before_the_server_started_the_turn_ends_the_process_at_once_with_no_body())
            .await
    }

    async fn an_interrupt_before_the_server_started_the_turn_ends_the_process_at_once_with_no_body() {
        let launch = launcher(scripted_server(hang()));
        let session = adapter_over(&launch, Extra { grace_ms: Some(15), ..Default::default() }).start(opts("x", no_events())).unwrap();
        session.interrupt().await;
        assert_eq!(session.finished().await, res(TurnStatus::Interrupted));
        assert!(!wire0(&launch).wrote("turn/interrupt"));
        assert_eq!(wire0(&launch).order(), vec!["teardown", "kill"]);
    }

    #[tokio::test]
    async fn a_stop_before_the_prompt_was_handed_over_sends_no_turn_then_or_once_the_prompt_i() {
        tokio::task::LocalSet::new()
            .run_until(a_stop_before_the_prompt_was_handed_over_sends_no_turn_then_or_once_the_prompt_i_body())
            .await
    }

    async fn a_stop_before_the_prompt_was_handed_over_sends_no_turn_then_or_once_the_prompt_i_body() {
        let launch = launcher(server(fixture_lines("codex", "app-server-turn.jsonl"), WireOpts::default()));
        let events = Events::default();
        let (due, after) = gate();
        let session = adapter_over(&launch, Extra { grace_ms: Some(15), ..Default::default() })
            .start(HarnessStartOptions { prompt_after: Some(after), ..opts("x", events.on_event()) })
            .unwrap();
        until(|| events.any("session.start")).await;
        session.interrupt().await;
        due.send(()).unwrap();
        assert_eq!(session.finished().await, res(TurnStatus::Interrupted));
        sleep_ms(5).await;
        assert!(!wire0(&launch).wrote("turn/start"));
    }

    #[tokio::test]
    async fn an_interrupt_after_the_process_is_gone_sends_nothing_and_ends_nothing() {
        tokio::task::LocalSet::new().run_until(an_interrupt_after_the_process_is_gone_sends_nothing_and_ends_nothing_body()).await
    }

    async fn an_interrupt_after_the_process_is_gone_sends_nothing_and_ends_nothing_body() {
        let launch = launcher(server(fixture_lines("codex", "app-server-turn.jsonl"), WireOpts::default()));
        let session = adapter_over(&launch, Extra { grace_ms: Some(5_000), ..Default::default() }).start(opts("x", no_events())).unwrap();
        session.finished().await;
        let before = wire0(&launch).written().len();
        session.interrupt().await;
        assert_eq!(wire0(&launch).written().len(), before);
        assert_eq!(wire0(&launch).order(), Vec::<String>::new());
    }
}
