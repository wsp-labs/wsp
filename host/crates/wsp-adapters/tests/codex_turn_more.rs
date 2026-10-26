// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-codex/test/adapter.test.ts at a3b870a6e, second half of the turn cases: what ends a turn, a
//! subagent's frames on its lead's stream, a recorded turn whose lead ended first, a turn a later host attaches to, the
//! process a finished turn leaves, the first three rewind cases, the plan's limits, and a thread's own compaction.

#![allow(dead_code, unused_imports)]

#[path = "common/codex_wire.rs"]
mod codex_wire;

use std::sync::{Arc, Mutex};

use futures::future::FutureExt;
use serde_json::{json, Value};
use wsp_adapters::codex::adapter::{create_codex_adapter, AttachedSession, CodexAdapter, CodexAdapterDeps};
use wsp_adapters::codex::turn::CodexSession;
use wsp_adapters::pending::CODEX_LEGACY_HISTORY;
use wsp_adapters::port::{
    AdapterAttachOptions, AdapterEvent, AnswerOutcome, Attached, DeltaKind, HarnessLimit, HarnessStartOptions, PermissionAnswer,
    PermissionOutcome, SteerOutcome, SubagentState, SubagentUsage, TaskStop, TurnDelta, TurnResult, TurnStatus, TurnTokens,
    PERMISSION_ALLOW,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::{BankedResets, HarnessLimitAccount, LimitKind, LimitStatus, LimitWindow, TurnLimit, UsageTokens};

use codex_wire::{
    fixture_lines, launcher, no_events, pending_after, scripted_server, server, sleep_ms, until, wire, Events, Launch, Wire, WireOpts,
    RUN_HANDLE,
};

const THREAD_ID: &str = "01a0e2c1-5d10-7b42-9a6e-3f1c2d4b5a60";
const TURN_ID: &str = "01a0e2c1-5e02-7c11-8d3f-9b2a1c0d4e71";
const RECORDED_THREAD: &str = "01a0e365-72f3-77e3-ba3a-3d18e12e9b95";
const LOGIN: &str = "codex login --device-auth";

#[derive(Default, Clone, Copy)]
struct Extra {
    grace_ms: Option<u64>,
    result_exit_ms: Option<u64>,
    steer_join_ms: Option<u64>,
}

fn adapter_over(launch: &Launch, extra: Extra) -> CodexAdapter {
    create_codex_adapter(CodexAdapterDeps {
        interrupt_grace_ms: extra.grace_ms,
        result_exit_ms: extra.result_exit_ms,
        steer_join_ms: extra.steer_join_ms,
        ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", LOGIN)
    })
    .unwrap()
}

fn go(adapter: &CodexAdapter, prompt: &str, events: &Events) -> CodexSession {
    adapter.start(HarnessStartOptions { prompt: prompt.into(), on_event: Some(events.on_event()), ..Default::default() }).unwrap()
}

fn go_resume(adapter: &CodexAdapter, prompt: &str, events: &Events) -> CodexSession {
    adapter
        .start(HarnessStartOptions {
            prompt: prompt.into(),
            resume: Some(THREAD_ID.into()),
            on_event: Some(events.on_event()),
            ..Default::default()
        })
        .unwrap()
}

fn go_quiet(adapter: &CodexAdapter, prompt: &str) -> CodexSession {
    adapter.start(HarnessStartOptions { prompt: prompt.into(), on_event: Some(no_events()), ..Default::default() }).unwrap()
}

/// The lines a live server prints up to a running turn, for the cases that drive the rest by hand.
fn opened_with(thread_id: &str, legacy: bool) -> Vec<String> {
    let mut thread = json!({ "id": thread_id, "model": "gpt-5.6-sol", "cwd": "/root/app", "turns": [] });
    if legacy {
        thread["historyMode"] = json!("legacy");
    }
    vec![
        json!({ "id": "wsp-initialize", "result": { "userAgent": "wsp/0.155.1", "codexHome": "/root/.codex", "platformFamily": "unix", "platformOs": "linux" } }).to_string(),
        json!({ "id": "wsp-thread", "result": { "thread": thread, "model": "gpt-5.6-sol", "cwd": "/root/app", "approvalPolicy": "on-request", "sandbox": { "type": "workspaceWrite" } } }).to_string(),
    ]
}

fn opened() -> Vec<String> {
    opened_with(THREAD_ID, false)
}

fn turn_started() -> String {
    json!({ "method": "turn/started", "params": { "threadId": THREAD_ID, "turn": { "id": TURN_ID, "items": [], "status": "inProgress" } } })
        .to_string()
}

fn completed(status: &str) -> String {
    json!({ "method": "turn/completed", "params": { "threadId": THREAD_ID, "turn": { "id": TURN_ID, "items": [], "status": status } } })
        .to_string()
}

fn agent_message(id: &str, text: &str) -> String {
    json!({ "method": "item/completed", "params": { "item": { "type": "agentMessage", "id": id, "text": text }, "threadId": THREAD_ID, "turnId": TURN_ID } }).to_string()
}

/// A server that opens the thread and starts the turn, then prints what the case hands it.
fn scripted_turn(rest: Vec<String>, opts: WireOpts) -> impl Fn(&[String]) -> Wire + 'static {
    let mut lines = opened();
    lines.push(turn_started());
    lines.extend(rest);
    server(lines, opts)
}

fn on_write(f: impl Fn(&Value, &Wire) + 'static) -> WireOpts {
    WireOpts { on_write: Some(Arc::new(f)), ..Default::default() }
}

fn hanging(f: impl Fn(&Value, &Wire) + 'static) -> WireOpts {
    WireOpts { hang: true, on_write: Some(Arc::new(f)), ..Default::default() }
}

fn method(m: &Value) -> &str {
    m.get("method").and_then(Value::as_str).unwrap_or("")
}

fn result_to(m: &Value, result: Value) -> String {
    json!({ "id": m["id"].clone(), "result": result }).to_string()
}

fn steer_result(m: &Value) -> String {
    result_to(m, json!({ "turnId": TURN_ID }))
}

fn is_subset(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => e.iter().all(|(k, v)| a.get(k).map(|x| is_subset(x, v)).unwrap_or(false)),
        (a, e) => a == e,
    }
}

/// `toMatchObject`.
fn assert_match(actual: &Value, expected: Value) {
    assert!(is_subset(actual, &expected), "{actual} does not match {expected}");
}

fn bare(status: TurnStatus) -> TurnResult {
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

fn tokens_of(result: &TurnResult) -> &TurnTokens {
    result.tokens.as_ref().expect("the result carries tokens")
}

fn wrote(live: &Launch, method: &str) -> bool {
    live.wire_count() > 0 && live.wire(0).wrote(method)
}

fn session_end_saw_result(events: &Events) -> bool {
    matches!(events.all().last(), Some(AdapterEvent::SessionEnd { saw_result: true, .. }))
}

fn tasks_of(events: &Events) -> Vec<u32> {
    events
        .all()
        .iter()
        .filter_map(|e| match e {
            AdapterEvent::TurnTasks { running, .. } => Some(*running),
            _ => None,
        })
        .collect()
}

fn subagents_of(events: &Events) -> Vec<AdapterEvent> {
    events.of("subagent")
}

fn sub_task_state(e: &AdapterEvent) -> (String, SubagentState) {
    match e {
        AdapterEvent::Subagent { task, state, .. } => (task.clone(), *state),
        other => panic!("not a subagent event: {other:?}"),
    }
}

fn assert_last_sub(events: &Events, task: &str, state: SubagentState) {
    let all = subagents_of(events);
    let last = all.last().expect("a subagent event");
    assert_eq!(sub_task_state(last), (task.to_string(), state));
}

fn last_sub_state_is(events: &Events, state: SubagentState) -> bool {
    subagents_of(events).last().map(|e| sub_task_state(e).1 == state).unwrap_or(false)
}

#[derive(Default)]
struct Sub<'a> {
    parent: Option<&'a str>,
    title: Option<&'a str>,
    summary: Option<&'a str>,
    depth: Option<u32>,
    model: Option<&'a str>,
    asked: Option<&'a str>,
    usage: Option<SubagentUsage>,
}

fn sub(session: &str, task: &str, state: SubagentState, s: Sub) -> AdapterEvent {
    AdapterEvent::Subagent {
        session_id: session.into(),
        task: task.into(),
        state,
        parent_tool_use_id: s.parent.map(String::from),
        title: s.title.map(String::from),
        summary: s.summary.map(String::from),
        depth: s.depth.map(|d| JsNumber(f64::from(d))),
        model: s.model.map(String::from),
        asked: s.asked.map(String::from),
        usage: s.usage,
    }
}

fn child_usage(input: u64, cached: u64, output: u64) -> SubagentUsage {
    SubagentUsage {
        tokens: Some(UsageTokens {
            input: JsNumber(input as f64),
            output: JsNumber(output as f64),
            cached: JsNumber(cached as f64),
            cache_write: JsNumber(0.0),
            reasoning: JsNumber(0.0),
        }),
        tool_uses: None,
        duration_ms: None,
    }
}

fn tool_use_delta(session: &str, tool_use_id: &str, prompt: &str) -> TurnDelta {
    TurnDelta {
        tool_name: Some("spawn_agent".into()),
        tool_use_id: Some(tool_use_id.into()),
        ..TurnDelta::new(session, DeltaKind::ToolUse, json!({ "prompt": prompt }).to_string())
    }
}

fn tool_result_delta(session: &str, tool_use_id: &str, is_error: bool) -> TurnDelta {
    TurnDelta { tool_use_id: Some(tool_use_id.into()), is_error: Some(is_error), ..TurnDelta::new(session, DeltaKind::ToolResult, "") }
}

fn deltas_for(events: &Events, tool_use_id: &str) -> Vec<TurnDelta> {
    events.deltas().into_iter().filter(|d| d.tool_use_id.as_deref() == Some(tool_use_id)).collect()
}

fn allow() -> PermissionAnswer {
    PermissionAnswer { option_id: PERMISSION_ALLOW.into(), outcome: PermissionOutcome::Allowed, deny_message: String::new(), reason: None }
}

fn turn_anchor(kept: Option<&str>) -> AdapterEvent {
    AdapterEvent::TurnAnchor { session_id: THREAD_ID.into(), anchor: TURN_ID.into(), kept: kept.map(String::from) }
}

mod what_ends_a_codex_turn {
    use super::*;

    fn status_of(kind: &str) -> String {
        json!({ "method": "thread/status/changed", "params": { "threadId": THREAD_ID, "status": { "type": kind } } }).to_string()
    }

    fn running(on_interrupt: Option<Arc<dyn Fn(&Wire, &Value)>>) -> Launch {
        launcher(scripted_server(on_write(move |m, w| match method(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => w.push1(turn_started()),
            "turn/steer" => w.push1(steer_result(m)),
            "turn/interrupt" => {
                if let Some(f) = &on_interrupt {
                    f(w, m);
                }
            }
            _ => {}
        })))
    }

    #[tokio::test]
    async fn only_turn_completed_the_thread_going_idle_and_its_last_message_leave_the_turn_op() {
        tokio::task::LocalSet::new()
            .run_until(only_turn_completed_the_thread_going_idle_and_its_last_message_leave_the_turn_op_body())
            .await
    }

    async fn only_turn_completed_the_thread_going_idle_and_its_last_message_leave_the_turn_op_body() {
        let live = running(None);
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra::default()), "x", &events);
        until(|| events.any("turn.anchor")).await;
        live.wire(0).push(&[agent_message("m1", "all done"), status_of("idle")]);
        assert!(pending_after(session.finished(), 50).await);
        assert!(!events.any("turn.done"));
        assert_eq!(session.steer("one more thing".into()).await, SteerOutcome::Accepted);
        live.wire(0).push1(completed("completed"));
        let result = session.finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("all done"));
    }

    #[tokio::test]
    async fn a_steer_sent_before_the_server_named_the_turn_waits_for_turn_started_and_names_t() {
        tokio::task::LocalSet::new()
            .run_until(a_steer_sent_before_the_server_named_the_turn_waits_for_turn_started_and_names_t_body())
            .await
    }

    async fn a_steer_sent_before_the_server_named_the_turn_waits_for_turn_started_and_names_t_body() {
        let live = launcher(scripted_server(on_write(|m, w| match method(m) {
            "thread/start" => w.push(&opened()),
            "turn/steer" => w.push1(steer_result(m)),
            _ => {}
        })));
        let session = go(&adapter_over(&live, Extra::default()), "x", &Events::default());
        until(|| wrote(&live, "turn/start")).await;
        let mut steered = session.steer("one more thing".into());
        assert!(pending_after(&mut steered, 50).await);
        live.wire(0).push1(turn_started());
        assert_eq!(steered.await, SteerOutcome::Accepted);
        let steers = live.wire(0).written_method("turn/steer");
        assert_match(&steers[0]["params"], json!({ "threadId": THREAD_ID, "expectedTurnId": TURN_ID }));
        live.wire(0).push1(completed("completed"));
        session.finished().await;

        let gone = launcher(scripted_server(on_write(|m, w| {
            if method(m) == "thread/start" {
                w.push(&opened());
            }
        })));
        let early = go(&adapter_over(&gone, Extra::default()), "x", &Events::default());
        until(|| wrote(&gone, "turn/start")).await;
        let late = early.steer("too soon".into());
        gone.wire(0).exit(Some(1));
        assert_eq!(late.await, SteerOutcome::NotRunning);
        assert!(!gone.wire(0).wrote("turn/steer"));
    }

    #[tokio::test]
    async fn a_steer_waiting_for_turn_started_answers_not_running_at_a_stop_whose_server_does() {
        tokio::task::LocalSet::new()
            .run_until(a_steer_waiting_for_turn_started_answers_not_running_at_a_stop_whose_server_does_body())
            .await
    }

    async fn a_steer_waiting_for_turn_started_answers_not_running_at_a_stop_whose_server_does_body() {
        let live = launcher(scripted_server(hanging(|m, w| {
            if method(m) == "thread/start" {
                w.push(&opened());
            }
        })));
        let session = go(&adapter_over(&live, Extra { grace_ms: Some(60_000), ..Default::default() }), "x", &Events::default());
        until(|| wrote(&live, "turn/start")).await;
        let mut steered = session.steer("too soon".into());
        let _stopping = tokio::task::spawn_local(session.interrupt());
        assert!(!pending_after(&mut steered, 50).await);
        assert_eq!(steered.await, SteerOutcome::NotRunning);
        assert!(!live.wire(0).wrote("turn/steer"));
        live.wire(0).exit(Some(0));
        session.finished().await;
    }

    #[tokio::test]
    async fn a_steer_waiting_for_a_turn_started_that_never_comes_answers_not_running_once_the() {
        tokio::task::LocalSet::new()
            .run_until(a_steer_waiting_for_a_turn_started_that_never_comes_answers_not_running_once_the_body())
            .await
    }

    async fn a_steer_waiting_for_a_turn_started_that_never_comes_answers_not_running_once_the_body() {
        let live = launcher(scripted_server(hanging(|m, w| {
            if method(m) == "thread/start" {
                w.push(&opened());
            }
        })));
        let session = go(&adapter_over(&live, Extra { steer_join_ms: Some(100), ..Default::default() }), "x", &Events::default());
        until(|| wrote(&live, "turn/start")).await;
        let mut steered = session.steer("too soon".into());
        assert!(pending_after(&mut steered, 50).await);
        assert_eq!(steered.await, SteerOutcome::NotRunning);
        assert!(!live.wire(0).wrote("turn/steer"));
        live.wire(0).exit(Some(0));
        session.finished().await;
    }

    #[tokio::test]
    async fn an_interrupt_s_answer_is_not_the_turn_s_end_it_stays_open_until_turn_completed_s() {
        tokio::task::LocalSet::new()
            .run_until(an_interrupt_s_answer_is_not_the_turn_s_end_it_stays_open_until_turn_completed_s_body())
            .await
    }

    async fn an_interrupt_s_answer_is_not_the_turn_s_end_it_stays_open_until_turn_completed_s_body() {
        let live = running(Some(Arc::new(|w: &Wire, m: &Value| w.push1(result_to(m, json!({}))))));
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra { grace_ms: Some(5_000), ..Default::default() }), "x", &events);
        until(|| events.any("turn.anchor")).await;
        let stopping = tokio::task::spawn_local(session.interrupt());
        until(|| wrote(&live, "turn/interrupt")).await;
        assert!(pending_after(session.finished(), 50).await);
        live.wire(0).push1(completed("interrupted"));
        stopping.await.unwrap();
        assert_eq!(session.finished().await, bare(TurnStatus::Interrupted));
        assert!(session_end_saw_result(&events));
        assert_eq!(live.wire(0).order(), Vec::<String>::new());
    }
}

mod a_subagent_s_frames_on_its_lead_s_stream {
    use super::*;

    // The shape of T3 Code's recorded multi-agent wire (codexMultiAgentWire.json, codex-cli 0.145.0, MIT): a child
    // thread's own turn/started, token usage, items and turn/completed arrive on the lead's stdout under the child's id.
    const CHILD: &str = "019fcfd6-2883-77e0-9013-4410ede70371";
    const CHILD_TURN: &str = "019fcfd6-28bf-7e00-a873-4554526dc845";
    const SPAWN: &str = "call_spawn_1";

    fn usage_of(thread_id: &str, turn_id: &str, total: u64, last: u64) -> String {
        let counts = |n: u64| json!({ "inputTokens": n, "cachedInputTokens": 0, "cacheWriteInputTokens": 0, "outputTokens": 0, "reasoningOutputTokens": 0, "totalTokens": n });
        let window = if thread_id == THREAD_ID { 258_400 } else { 128_000 };
        json!({ "method": "thread/tokenUsage/updated", "params": { "threadId": thread_id, "turnId": turn_id, "tokenUsage": { "total": counts(total), "last": counts(last), "modelContextWindow": window } } }).to_string()
    }

    fn child_frames() -> Vec<String> {
        vec![
            json!({ "method": "thread/status/changed", "params": { "threadId": CHILD, "status": { "type": "idle" } } }).to_string(),
            json!({ "method": "item/completed", "params": { "item": { "type": "subAgentActivity", "id": "call_S2JP", "kind": "started", "agentThreadId": CHILD, "agentPath": "/root/alpha" }, "threadId": THREAD_ID, "turnId": TURN_ID, "completedAtMs": 1785898346687_i64 } }).to_string(),
            json!({ "method": "thread/status/changed", "params": { "threadId": CHILD, "status": { "type": "active", "activeFlags": [] } } }).to_string(),
            json!({ "method": "turn/started", "params": { "threadId": CHILD, "turn": { "id": CHILD_TURN, "items": [], "itemsView": "notLoaded", "status": "inProgress", "error": null } } }).to_string(),
            usage_of(CHILD, CHILD_TURN, 20_756, 20_756),
            json!({ "method": "turn/plan/updated", "params": { "threadId": CHILD, "turnId": CHILD_TURN, "explanation": null, "plan": [{ "step": "child step", "status": "inProgress" }] } }).to_string(),
            json!({ "method": "item/completed", "params": { "item": { "type": "agentMessage", "id": "child_m1", "text": "child says done" }, "threadId": CHILD, "turnId": CHILD_TURN } }).to_string(),
        ]
    }

    fn child_ask() -> String {
        json!({ "id": 11, "method": "item/commandExecution/requestApproval", "params": { "threadId": CHILD, "turnId": CHILD_TURN, "itemId": "call_c1", "command": "ls", "cwd": "/root/app", "startedAtMs": 1 } }).to_string()
    }

    fn child_completed() -> String {
        json!({ "method": "turn/completed", "params": { "threadId": CHILD, "turn": { "id": CHILD_TURN, "items": [], "status": "completed", "error": null } } }).to_string()
    }

    fn leading() -> Launch {
        launcher(scripted_server(on_write(|m, w| match method(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => {
                let mut lines = vec![turn_started(), usage_of(THREAD_ID, TURN_ID, 18_261, 18_261)];
                lines.extend(child_frames());
                lines.push(child_ask());
                w.push(&lines);
            }
            "turn/steer" => w.push1(steer_result(m)),
            _ => {}
        })))
    }

    fn interrupts_of(live: &Launch) -> Vec<Value> {
        live.wire(0).written_method("turn/interrupt").into_iter().map(|m| m["params"].clone()).collect()
    }

    fn spawn_item(phase: &str) -> String {
        let started = phase == "started";
        json!({
            "method": format!("item/{phase}"),
            "params": {
                "item": {
                    "type": "collabAgentToolCall",
                    "id": SPAWN,
                    "tool": "spawnAgent",
                    "status": if started { "inProgress" } else { "completed" },
                    "senderThreadId": THREAD_ID,
                    "receiverThreadIds": if started { json!([]) } else { json!([CHILD]) },
                    "prompt": "Sleep 20 s and say done.\nEnd without waiting.",
                    "model": null,
                    "reasoningEffort": null,
                    "agentsStates": {}
                },
                "threadId": THREAD_ID,
                "turnId": TURN_ID
            }
        })
        .to_string()
    }

    fn child_started() -> String {
        json!({ "method": "turn/started", "params": { "threadId": CHILD, "turn": { "id": CHILD_TURN, "items": [], "status": "inProgress", "error": null } } }).to_string()
    }

    fn child_ended(status: &str, error: Value) -> String {
        json!({ "method": "turn/completed", "params": { "threadId": CHILD, "turn": { "id": CHILD_TURN, "items": [], "status": status, "error": error } } }).to_string()
    }

    fn child_says(text: &str) -> String {
        json!({ "method": "item/completed", "params": { "item": { "type": "agentMessage", "id": "child_m2", "text": text }, "threadId": CHILD, "turnId": CHILD_TURN } }).to_string()
    }

    fn activity(kind: &str) -> String {
        json!({ "method": "item/completed", "params": { "item": { "type": "subAgentActivity", "id": SPAWN, "kind": kind, "agentThreadId": CHILD, "agentPath": "/root/alpha" }, "threadId": THREAD_ID, "turnId": TURN_ID } }).to_string()
    }

    /// A lead that spawns one child and ends its own turn without waiting, as the person asked it to.
    fn spawning(interrupts: bool, hang: bool) -> Launch {
        launcher(scripted_server(WireOpts {
            hang,
            on_write: Some(Arc::new(move |m, w| match method(m) {
                "thread/start" => w.push(&opened()),
                "turn/start" => w.push(&[
                    turn_started(),
                    spawn_item("started"),
                    spawn_item("completed"),
                    activity("started"),
                    child_started(),
                    usage_of(CHILD, CHILD_TURN, 900, 900),
                ]),
                "turn/interrupt" if interrupts => {
                    w.push1(result_to(m, json!({})));
                    w.push1(if m["params"]["threadId"] == CHILD {
                        child_ended("interrupted", json!(null))
                    } else {
                        completed("interrupted")
                    });
                }
                _ => {}
            })),
            ..Default::default()
        }))
    }

    #[tokio::test]
    async fn never_lets_a_child_s_turn_set_end_or_rename_the_lead_s_the_steer_names_the_lead() {
        tokio::task::LocalSet::new().run_until(never_lets_a_child_s_turn_set_end_or_rename_the_lead_s_the_steer_names_the_lead_body()).await
    }

    async fn never_lets_a_child_s_turn_set_end_or_rename_the_lead_s_the_steer_names_the_lead_body() {
        let live = leading();
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra::default()), "spawn one agent", &events);
        until(|| events.any("permission.ask")).await;
        assert_eq!(session.steer("and say when".into()).await, SteerOutcome::Accepted);
        let steers = live.wire(0).written_method("turn/steer");
        assert_match(&steers[0]["params"], json!({ "threadId": THREAD_ID, "expectedTurnId": TURN_ID }));

        live.wire(0).push1(child_completed());
        sleep_ms(10).await;
        assert!(!events.any("turn.done"));
        assert!(!live.wire(0).closed());

        live.wire(0).push(&[
            agent_message("lead_m1", "the lead's reply"),
            usage_of(THREAD_ID, TURN_ID, 36_576, 18_315),
            completed("completed"),
        ]);
        let result = session.finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("the lead's reply"));
        // The child's one call counts in the turn's tokens; what the lead's model held and its window stay the lead's.
        let tokens = tokens_of(&result);
        assert_eq!(tokens.input, JsNumber((36_576 + 20_756) as f64));
        assert_eq!(tokens.context, Some(JsNumber(18_315.0)));
        assert_eq!(tokens.window, Some(JsNumber(258_400.0)));
        assert_eq!(events.of("turn.anchor"), vec![turn_anchor(None)]);
        assert!(!events.any("turn.plan"));
        assert!(!events.deltas().iter().any(|d| d.text == "child says done"));
    }

    #[tokio::test]
    async fn an_interrupt_names_the_lead_s_turn_after_a_child_s_turn_started() {
        tokio::task::LocalSet::new().run_until(an_interrupt_names_the_lead_s_turn_after_a_child_s_turn_started_body()).await
    }

    async fn an_interrupt_names_the_lead_s_turn_after_a_child_s_turn_started_body() {
        let live = leading();
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra { grace_ms: Some(15), ..Default::default() }), "spawn one agent", &events);
        until(|| events.any("permission.ask")).await;
        session.interrupt().await;
        let interrupts = interrupts_of(&live);
        assert!(interrupts.contains(&json!({ "threadId": THREAD_ID, "turnId": TURN_ID })), "{interrupts:?}");
        assert!(!interrupts.contains(&json!({ "threadId": THREAD_ID, "turnId": CHILD_TURN })), "{interrupts:?}");
    }

    #[tokio::test]
    async fn holds_the_lead_s_reply_while_its_child_runs_no_turn_done_stdin_open_one_task_and() {
        tokio::task::LocalSet::new()
            .run_until(holds_the_lead_s_reply_while_its_child_runs_no_turn_done_stdin_open_one_task_and_body())
            .await
    }

    async fn holds_the_lead_s_reply_while_its_child_runs_no_turn_done_stdin_open_one_task_and_body() {
        let live = spawning(false, false);
        let events = Events::default();
        let session = go(
            &adapter_over(&live, Extra { result_exit_ms: Some(15), grace_ms: Some(15), ..Default::default() }),
            "spawn one agent",
            &events,
        );
        until(|| events.any("subagent")).await;
        live.wire(0).push(&[agent_message("lead_m1", "spawned it"), usage_of(THREAD_ID, TURN_ID, 18_000, 18_000), completed("completed")]);
        sleep_ms(40).await;
        assert!(!events.any("turn.done"));
        assert!(!live.wire(0).closed());
        assert_eq!(live.wire(0).order(), Vec::<String>::new());
        assert_eq!(tasks_of(&events).last(), Some(&1));
        assert_eq!(
            subagents_of(&events)[0],
            sub(
                THREAD_ID,
                CHILD,
                SubagentState::Running,
                Sub {
                    parent: Some(SPAWN),
                    title: Some("Sleep 20 s and say done."),
                    asked: Some("Sleep 20 s and say done.\nEnd without waiting."),
                    ..Default::default()
                }
            )
        );
        let without_usage: Vec<AdapterEvent> =
            subagents_of(&events).into_iter().filter(|e| matches!(e, AdapterEvent::Subagent { usage: None, .. })).collect();
        let last = without_usage.last().expect("a subagent event without usage");
        assert_eq!(sub_task_state(last), (CHILD.to_string(), SubagentState::Running));
        assert!(matches!(last, AdapterEvent::Subagent { depth: Some(JsNumber(1.0)), .. }), "{last:?}");
        assert_eq!(
            deltas_for(&events, SPAWN),
            vec![
                tool_use_delta(THREAD_ID, SPAWN, "Sleep 20 s and say done.\nEnd without waiting."),
                tool_result_delta(THREAD_ID, SPAWN, false)
            ]
        );

        live.wire(0).push(&[child_says("done"), child_ended("completed", json!(null))]);
        let result = session.finished().await;
        let usage = child_usage(900, 0, 0);
        let with_usage: Vec<AdapterEvent> =
            subagents_of(&events).into_iter().filter(|e| matches!(e, AdapterEvent::Subagent { usage: Some(_), .. })).collect();
        assert_eq!(
            with_usage,
            vec![
                sub(
                    THREAD_ID,
                    CHILD,
                    SubagentState::Running,
                    Sub { parent: Some(SPAWN), usage: Some(usage.clone()), ..Default::default() }
                ),
                sub(
                    THREAD_ID,
                    CHILD,
                    SubagentState::Done,
                    Sub { parent: Some(SPAWN), summary: Some("done"), usage: Some(usage), ..Default::default() }
                ),
            ]
        );
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("spawned it"));
        let tokens = tokens_of(&result);
        assert_eq!(tokens.input, JsNumber(18_900.0));
        assert_eq!(tokens.context, Some(JsNumber(18_000.0)));
        assert_eq!(tokens.window, Some(JsNumber(258_400.0)));
        assert_eq!(tasks_of(&events).last(), Some(&0));
        assert!(live.wire(0).closed());
        let usages: Vec<(f64, Option<f64>)> = events
            .of("turn.usage")
            .iter()
            .map(|e| match e {
                AdapterEvent::TurnUsage { tokens, context, .. } => (*tokens, *context),
                other => panic!("{other:?}"),
            })
            .collect();
        // The child's call says nothing of what the lead holds.
        assert_eq!(usages, vec![(900.0, None), (18_000.0, Some(18_000.0))]);
    }

    #[tokio::test]
    async fn a_child_whose_turn_failed_reads_failed_and_one_a_server_s_activity_says_complete() {
        tokio::task::LocalSet::new()
            .run_until(a_child_whose_turn_failed_reads_failed_and_one_a_server_s_activity_says_complete_body())
            .await
    }

    async fn a_child_whose_turn_failed_reads_failed_and_one_a_server_s_activity_says_complete_body() {
        let live = spawning(false, false);
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra::default()), "spawn one agent", &events);
        until(|| events.any("subagent")).await;
        live.wire(0).push(&[completed("completed"), child_ended("failed", json!({ "message": "stream disconnected" }))]);
        session.finished().await;
        assert_last_sub(&events, CHILD, SubagentState::Failed);

        let other = spawning(false, false);
        let later = Events::default();
        let next = go(&adapter_over(&other, Extra::default()), "spawn one agent", &later);
        until(|| later.any("subagent")).await;
        other.wire(0).push(&[completed("completed"), activity("completed")]);
        next.finished().await;
        assert_last_sub(&later, CHILD, SubagentState::Done);
    }

    #[tokio::test]
    async fn a_child_known_only_from_its_spawn_holds_the_reply_but_not_the_idle_clock_until_i() {
        tokio::task::LocalSet::new()
            .run_until(a_child_known_only_from_its_spawn_holds_the_reply_but_not_the_idle_clock_until_i_body())
            .await
    }

    async fn a_child_known_only_from_its_spawn_holds_the_reply_but_not_the_idle_clock_until_i_body() {
        let live = launcher(scripted_server(on_write(|m, w| match method(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => w.push(&[turn_started(), spawn_item("completed"), completed("completed")]),
            _ => {}
        })));
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra::default()), "x", &events);
        until(|| events.any("subagent")).await;
        sleep_ms(10).await;
        assert!(!events.any("turn.done"));
        assert_eq!(tasks_of(&events), Vec::<u32>::new());
        live.wire(0).push1(child_started());
        until(|| tasks_of(&events).last() == Some(&1)).await;
        live.wire(0).push1(child_ended("completed", json!(null)));
        assert_eq!(session.finished().await.status, TurnStatus::Completed);
        assert_eq!(tasks_of(&events), vec![1, 0]);
    }

    #[tokio::test]
    async fn a_spawn_the_server_failed_names_no_child_so_nothing_holds_the_lead() {
        tokio::task::LocalSet::new().run_until(a_spawn_the_server_failed_names_no_child_so_nothing_holds_the_lead_body()).await
    }

    async fn a_spawn_the_server_failed_names_no_child_so_nothing_holds_the_lead_body() {
        let failed_spawn = spawn_item("completed").replace(r#""status":"completed""#, r#""status":"failed""#);
        let live = launcher(scripted_server(on_write(move |m, w| match method(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => {
                w.push(&[turn_started(), failed_spawn.clone(), agent_message("lead_m1", "could not spawn"), completed("completed")])
            }
            _ => {}
        })));
        let events = Events::default();
        let result = go(&adapter_over(&live, Extra::default()), "x", &events).finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("could not spawn"));
        assert_eq!(subagents_of(&events), Vec::<AdapterEvent>::new());
        assert_eq!(deltas_for(&events, SPAWN), vec![tool_result_delta(THREAD_ID, SPAWN, true)]);
    }

    #[tokio::test]
    async fn knows_a_child_whose_only_sign_is_its_own_turn_starting_and_holds_the_lead_for_it() {
        tokio::task::LocalSet::new()
            .run_until(knows_a_child_whose_only_sign_is_its_own_turn_starting_and_holds_the_lead_for_it_body())
            .await
    }

    async fn knows_a_child_whose_only_sign_is_its_own_turn_starting_and_holds_the_lead_for_it_body() {
        let live = launcher(scripted_server(on_write(|m, w| match method(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => w.push(&[turn_started(), child_started(), completed("completed")]),
            _ => {}
        })));
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra::default()), "x", &events);
        until(|| events.any("subagent")).await;
        assert_eq!(
            subagents_of(&events)[0],
            sub(THREAD_ID, CHILD, SubagentState::Running, Sub { parent: Some(CHILD), ..Default::default() })
        );
        sleep_ms(10).await;
        assert!(!events.any("turn.done"));
        live.wire(0).push1(child_ended("completed", json!(null)));
        assert_eq!(session.finished().await.status, TurnStatus::Completed);
    }

    #[tokio::test]
    async fn a_stop_with_a_child_running_interrupts_the_child_first_then_the_lead_and_the_chi() {
        tokio::task::LocalSet::new()
            .run_until(a_stop_with_a_child_running_interrupts_the_child_first_then_the_lead_and_the_chi_body())
            .await
    }

    async fn a_stop_with_a_child_running_interrupts_the_child_first_then_the_lead_and_the_chi_body() {
        let live = spawning(true, false);
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra { grace_ms: Some(5_000), ..Default::default() }), "spawn one agent", &events);
        until(|| events.any("subagent")).await;
        session.interrupt().await;
        assert_eq!(
            interrupts_of(&live),
            vec![json!({ "threadId": CHILD, "turnId": CHILD_TURN }), json!({ "threadId": THREAD_ID, "turnId": TURN_ID })]
        );
        assert_eq!(session.finished().await, bare(TurnStatus::Interrupted));
        assert_last_sub(&events, CHILD, SubagentState::Stopped);
        assert_eq!(live.wire(0).order(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn a_stop_after_the_lead_s_reply_interrupts_the_child_alone_and_the_held_reply_stan() {
        tokio::task::LocalSet::new()
            .run_until(a_stop_after_the_lead_s_reply_interrupts_the_child_alone_and_the_held_reply_stan_body())
            .await
    }

    async fn a_stop_after_the_lead_s_reply_interrupts_the_child_alone_and_the_held_reply_stan_body() {
        let live = spawning(true, false);
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra { grace_ms: Some(5_000), ..Default::default() }), "spawn one agent", &events);
        until(|| events.any("subagent")).await;
        live.wire(0).push(&[agent_message("lead_m1", "spawned it"), completed("completed")]);
        sleep_ms(10).await;
        session.interrupt().await;
        assert_eq!(interrupts_of(&live), vec![json!({ "threadId": CHILD, "turnId": CHILD_TURN })]);
        let result = session.finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("spawned it"));
        assert!(last_sub_state_is(&events, SubagentState::Stopped));
    }

    #[tokio::test]
    async fn a_child_s_approval_reaches_the_person_as_an_ask_named_by_the_child_s_spawn() {
        tokio::task::LocalSet::new().run_until(a_child_s_approval_reaches_the_person_as_an_ask_named_by_the_child_s_spawn_body()).await
    }

    async fn a_child_s_approval_reaches_the_person_as_an_ask_named_by_the_child_s_spawn_body() {
        let live = leading();
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra::default()), "spawn one agent", &events);
        until(|| events.any("permission.ask")).await;
        let asks = events.of("permission.ask");
        let AdapterEvent::PermissionAsk { session_id, ask } = &asks[0] else { panic!("{:?}", asks[0]) };
        assert_eq!(session_id, THREAD_ID);
        assert_eq!(ask.ask_id, "11");
        assert_eq!(ask.tool_name, "command_execution");
        assert_eq!(ask.parent_tool_use_id.as_deref(), Some("call_S2JP"));
        assert_eq!(session.answer(ask.ask_id.clone(), allow()).await.unwrap(), AnswerOutcome::Answered);
        assert!(live.wire(0).written().contains(&json!({ "id": 11, "result": { "decision": "accept" } })));
        live.wire(0).push(&[child_completed(), completed("completed")]);
        session.finished().await;
    }

    #[tokio::test]
    async fn stops_one_child_alone_by_its_thread_id_the_lead_running_on_and_its_interrupted_e() {
        tokio::task::LocalSet::new()
            .run_until(stops_one_child_alone_by_its_thread_id_the_lead_running_on_and_its_interrupted_e_body())
            .await
    }

    async fn stops_one_child_alone_by_its_thread_id_the_lead_running_on_and_its_interrupted_e_body() {
        let live = spawning(true, false);
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra::default()), "spawn one agent", &events);
        until(|| events.any("subagent")).await;
        assert_eq!(session.stop_task("no-such-thread".into()).await, TaskStop::NotRunning);
        assert_eq!(session.stop_task(CHILD.into()).await, TaskStop::Accepted);
        assert_eq!(interrupts_of(&live), vec![json!({ "threadId": CHILD, "turnId": CHILD_TURN })]);
        until(|| last_sub_state_is(&events, SubagentState::Stopped)).await;
        assert!(!events.any("turn.done"));
        assert_eq!(session.stop_task(CHILD.into()).await, TaskStop::NotRunning);
        live.wire(0).push1(completed("completed"));
        assert_eq!(session.finished().await.status, TurnStatus::Completed);
    }

    #[tokio::test]
    async fn a_process_that_goes_with_a_child_still_running_ends_the_child_too_stopped_under() {
        tokio::task::LocalSet::new().run_until(a_process_that_goes_with_a_child_still_running_ends_the_child_too_stopped_under_body()).await
    }

    async fn a_process_that_goes_with_a_child_still_running_ends_the_child_too_stopped_under_body() {
        let live = spawning(false, false);
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra::default()), "spawn one agent", &events);
        until(|| events.any("subagent")).await;
        live.wire(0).push(&[agent_message("lead_m1", "spawned it"), completed("completed")]);
        sleep_ms(10).await;
        live.wire(0).exit(Some(1));
        let result = session.finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("spawned it"));
        assert_eq!(live.wire(0).order(), Vec::<String>::new());
        assert_last_sub(&events, CHILD, SubagentState::Failed);
        let kinds = events.kinds();
        assert_eq!(kinds[kinds.len() - 4..].to_vec(), vec!["subagent", "turn.tasks", "turn.done", "session.end"]);

        let stuck = spawning(false, true);
        let stopped = Events::default();
        let cut = go(&adapter_over(&stuck, Extra { grace_ms: Some(15), ..Default::default() }), "spawn one agent", &stopped);
        until(|| stopped.any("subagent")).await;
        cut.interrupt().await;
        assert_eq!(cut.finished().await, bare(TurnStatus::Interrupted));
        assert_eq!(stuck.wire(0).order(), vec!["teardown", "kill"]);
        assert_last_sub(&stopped, CHILD, SubagentState::Stopped);
    }
}

mod a_recorded_codex_turn_whose_lead_spawned_an_agent_and_ended_first {
    use super::*;

    // codex-cli 0.155.1 on gpt-5.6-luna at effort low, recorded through a throwaway host: the lead's turn/completed comes
    // before its child's. MCP startup, hook and stderr lines are left out; the account, paths and warnings are scrubbed.
    const LEAD: &str = "01a100dc-e1f0-7183-94f1-048611cff500";
    const CHILD: &str = "01a100dd-0379-7f91-bd7d-76a0d1bcb962";
    const SPAWN: &str = "exec-c2242f64-3c99-4c9b-be0d-2132497b24ef";
    const ASKED: &str = "Reply with the single word done and nothing else.";

    #[tokio::test]
    async fn holds_the_lead_s_reply_until_the_child_s_own_turn_completes_and_says_the_child_s() {
        tokio::task::LocalSet::new()
            .run_until(holds_the_lead_s_reply_until_the_child_s_own_turn_completes_and_says_the_child_s_body())
            .await
    }

    async fn holds_the_lead_s_reply_until_the_child_s_own_turn_completes_and_says_the_child_s_body() {
        let launch = launcher(server(fixture_lines("codex", "app-server-subagent.jsonl"), WireOpts::default()));
        let events = Events::default();
        let result = go(&adapter_over(&launch, Extra::default()), "spawn one agent and end your turn", &events).finished().await;
        let said = subagents_of(&events);
        // The child's row carries its own thread's total; the lead's turn still counts the child's calls in its tokens.
        let usage = child_usage(19_515, 19_200, 5);
        let expected = vec![
            sub(
                LEAD,
                CHILD,
                SubagentState::Running,
                Sub { parent: Some(SPAWN), title: Some(ASKED), model: Some("gpt-5.6-luna"), asked: Some(ASKED), ..Default::default() },
            ),
            sub(LEAD, CHILD, SubagentState::Running, Sub { parent: Some(SPAWN), usage: Some(usage.clone()), ..Default::default() }),
            sub(
                LEAD,
                CHILD,
                SubagentState::Done,
                Sub { parent: Some(SPAWN), summary: Some("done"), usage: Some(usage), ..Default::default() },
            ),
        ];
        assert_eq!(said, expected);
        let all = events.all();
        let done_at = all.iter().position(|e| matches!(e, AdapterEvent::TurnDone { .. })).expect("a turn.done");
        let last_said = all.iter().rposition(|e| e == &expected[2]).expect("the done event");
        assert!(done_at > last_said, "{done_at} {last_said}");
        assert_eq!(tasks_of(&events), vec![1, 0]);
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("spawned"));
        let tokens = tokens_of(&result);
        assert_eq!(tokens.input, JsNumber((77_535 + 19_515) as f64));
        assert_eq!(tokens.window, Some(JsNumber(258_400.0)));
        assert!(!events.deltas().iter().any(|d| d.text == "done"));
        let spawn_deltas: Vec<(DeltaKind, Option<String>, String)> =
            deltas_for(&events, SPAWN).into_iter().map(|d| (d.kind, d.tool_name, d.text)).collect();
        assert_eq!(
            spawn_deltas,
            vec![
                (DeltaKind::ToolUse, Some("spawn_agent".to_string()), json!({ "prompt": ASKED }).to_string()),
                (DeltaKind::ToolResult, None, String::new())
            ]
        );
        assert!(launch.wire(0).closed());
        let last = all.last().expect("events");
        assert!(matches!(last, AdapterEvent::SessionEnd { session_id, saw_result: true, .. } if session_id == LEAD), "{last:?}");
    }
}

mod a_turn_a_later_host_attaches_to {
    use super::*;

    #[tokio::test]
    async fn re_opens_the_run_with_its_channel_replays_the_log_and_writes_nothing_in_answer_t() {
        tokio::task::LocalSet::new()
            .run_until(re_opens_the_run_with_its_channel_replays_the_log_and_writes_nothing_in_answer_t_body())
            .await
    }

    async fn re_opens_the_run_with_its_channel_replays_the_log_and_writes_nothing_in_answer_t_body() {
        let launch = launcher(server(fixture_lines("codex", "app-server-turn.jsonl"), WireOpts::default()));
        let attached: Arc<Mutex<Vec<(String, bool)>>> = Arc::new(Mutex::new(Vec::new()));
        let replay = wire(WireOpts::default());
        replay.push(&fixture_lines("codex", "app-server-turn.jsonl"));
        replay.exit(Some(0));
        let (seen, stream) = (attached.clone(), replay.clone());
        launch.set_attach(move |run, options| {
            seen.lock().unwrap().push((run.to_string(), options.input));
            let stream = stream.stream();
            async move { Ok(Attached::Stream(stream)) }.boxed_local()
        });
        let adapter = adapter_over(&launch, Extra::default());
        assert_eq!(go_quiet(&adapter, "go").run().as_deref(), Some(RUN_HANDLE));
        let events = Events::default();
        let found = adapter
            .attach(AdapterAttachOptions {
                run: RUN_HANDLE.into(),
                session_id: RECORDED_THREAD.into(),
                started_at: 1.0,
                model: Some("gpt-5.5".into()),
                cwd: Some("/root/app".into()),
                prompt: None,
                effort: None,
                from: None,
                steered: None,
                on_event: events.on_event(),
            })
            .expect("the factory attaches")
            .await
            .unwrap();
        let AttachedSession::Session(session) = found else { panic!("the run was gone") };
        let result = session.finished().await;
        assert_eq!(*attached.lock().unwrap(), vec![(RUN_HANDLE.to_string(), true)]);
        assert_eq!(session.command(), None);
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(replay.written(), Vec::<Value>::new());
        let all = events.all();
        assert_eq!(
            all[0],
            AdapterEvent::SessionStart {
                session_id: RECORDED_THREAD.into(),
                model: Some("gpt-5.5".into()),
                cwd: Some("/root/app".into()),
                tools: None,
                harness: None,
                prompted: None
            }
        );
        let tail = &all[all.len() - 2..];
        assert!(matches!(&tail[0], AdapterEvent::TurnDone { .. }), "{:?}", tail[0]);
        assert!(matches!(&tail[1], AdapterEvent::SessionEnd { exit_code: Some(0), saw_result: true, .. }), "{:?}", tail[1]);
    }
}

mod the_process_a_finished_turn_leaves {
    use super::*;

    #[tokio::test]
    async fn a_server_that_does_not_go_after_its_turn_completed_is_ended_with_its_tree_and_th() {
        tokio::task::LocalSet::new()
            .run_until(a_server_that_does_not_go_after_its_turn_completed_is_ended_with_its_tree_and_th_body())
            .await
    }

    async fn a_server_that_does_not_go_after_its_turn_completed_is_ended_with_its_tree_and_th_body() {
        let launch = launcher(scripted_turn(
            vec![agent_message("msg_1", "done"), completed("completed")],
            WireOpts { hang: true, ..Default::default() },
        ));
        let events = Events::default();
        let result = go(&adapter_over(&launch, Extra { grace_ms: Some(15), result_exit_ms: Some(15), ..Default::default() }), "x", &events)
            .finished()
            .await;
        assert_eq!(launch.wire(0).order(), vec!["teardown", "kill"]);
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("done"));
        assert!(session_end_saw_result(&events));
    }

    #[tokio::test]
    async fn a_server_that_exits_on_eof_is_left_alone() {
        tokio::task::LocalSet::new().run_until(a_server_that_exits_on_eof_is_left_alone_body()).await
    }

    async fn a_server_that_exits_on_eof_is_left_alone_body() {
        let launch = launcher(server(fixture_lines("codex", "app-server-turn.jsonl"), WireOpts::default()));
        let result = go_quiet(&adapter_over(&launch, Extra { grace_ms: Some(15), result_exit_ms: Some(15), ..Default::default() }), "x")
            .finished()
            .await;
        assert_eq!(result.status, TurnStatus::Completed);
        sleep_ms(60).await;
        assert_eq!(launch.wire(0).order(), Vec::<String>::new());
    }
}

mod what_a_rewind_needs_of_a_codex_turn {
    use super::*;

    #[tokio::test]
    async fn names_the_turn_by_the_id_the_server_gave_it_at_turn_started_once() {
        tokio::task::LocalSet::new().run_until(names_the_turn_by_the_id_the_server_gave_it_at_turn_started_once_body()).await
    }

    async fn names_the_turn_by_the_id_the_server_gave_it_at_turn_started_once_body() {
        let launch = launcher(scripted_turn(vec![agent_message("msg_1", "done"), completed("completed")], WireOpts::default()));
        let events = Events::default();
        go(&adapter_over(&launch, Extra::default()), "x", &events).finished().await;
        assert_eq!(events.of("turn.anchor"), vec![turn_anchor(None)]);
    }

    #[tokio::test]
    async fn on_a_thread_an_older_codex_made_names_each_turn_s_anchor_with_why_its_history_ca() {
        tokio::task::LocalSet::new()
            .run_until(on_a_thread_an_older_codex_made_names_each_turn_s_anchor_with_why_its_history_ca_body())
            .await
    }

    async fn on_a_thread_an_older_codex_made_names_each_turn_s_anchor_with_why_its_history_ca_body() {
        let head = opened_with(THREAD_ID, true);
        let legacy = server(
            vec![head[0].clone(), head[1].clone(), turn_started(), agent_message("msg_1", "done"), completed("completed")],
            WireOpts::default(),
        );
        let events = Events::default();
        go_resume(&adapter_over(&launcher(legacy), Extra::default()), "x", &events).finished().await;
        assert_eq!(events.of("turn.anchor"), vec![turn_anchor(Some(CODEX_LEGACY_HISTORY))]);
    }

    #[tokio::test]
    async fn reads_the_history_mode_off_this_thread_s_own_announcement_alone_never_a_later_th() {
        tokio::task::LocalSet::new()
            .run_until(reads_the_history_mode_off_this_thread_s_own_announcement_alone_never_a_later_th_body())
            .await
    }

    async fn reads_the_history_mode_off_this_thread_s_own_announcement_alone_never_a_later_th_body() {
        let other = json!({ "method": "thread/started", "params": { "thread": { "id": "01a0e2c1-0000-7000-8000-00000000c41d", "historyMode": "legacy" } } }).to_string();
        let mut lines = opened();
        lines.extend([other, turn_started(), agent_message("msg_1", "done"), completed("completed")]);
        let events = Events::default();
        go(&adapter_over(&launcher(server(lines, WireOpts::default())), Extra::default()), "x", &events).finished().await;
        assert_eq!(events.of("turn.anchor"), vec![turn_anchor(None)]);
    }
}
