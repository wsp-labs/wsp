// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-codex/test/adapter.test.ts at a3b870a6e, second half of the turn cases: what ends a turn, a
//! subagent's frames on its lead's stream, a recorded turn whose lead ended first, a turn a later host attaches to, the
//! process a finished turn leaves, the first three rewind cases, the plan's limits, and a thread's own compaction.

#[path = "common/codex_wire.rs"]
mod codex_wire;

use std::sync::{Arc, Mutex};

use futures::future::FutureExt;
use serde_json::{json, Value};
use wsp_adapters::codex::adapter::{create_codex_adapter, AttachedSession, CodexAdapter, CodexAdapterDeps};
use wsp_adapters::codex::turn::CodexSession;
use wsp_adapters::pending::CODEX_LEGACY_HISTORY;
use wsp_adapters::port::{
    AdapterAttachOptions, AdapterEvent, AnswerOutcome, Attached, DeltaKind, HarnessLimit, HarnessStartOptions, PermissionAnswer, PermissionOutcome,
    SteerOutcome, SubagentState, SubagentUsage, TaskStop, TurnDelta, TurnResult, TurnStatus, TurnTokens, PERMISSION_ALLOW,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::{BankedResets, HarnessLimitAccount, LimitKind, LimitStatus, LimitWindow, TurnLimit, UsageTokens};

use codex_wire::{fixture_lines, launcher, no_events, pending_after, scripted_server, server, sleep_ms, until, wire, Events, Launch, Wire, WireOpts, RUN_HANDLE};

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
    adapter.start(HarnessStartOptions { prompt: prompt.into(), resume: Some(THREAD_ID.into()), on_event: Some(events.on_event()), ..Default::default() }).unwrap()
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
    json!({ "method": "turn/started", "params": { "threadId": THREAD_ID, "turn": { "id": TURN_ID, "items": [], "status": "inProgress" } } }).to_string()
}

fn completed(status: &str) -> String {
    json!({ "method": "turn/completed", "params": { "threadId": THREAD_ID, "turn": { "id": TURN_ID, "items": [], "status": status } } }).to_string()
}

fn agent_message(id: &str, text: &str) -> String {
    json!({ "method": "item/completed", "params": { "item": { "type": "agentMessage", "id": id, "text": text }, "threadId": THREAD_ID, "turnId": TURN_ID } }).to_string()
}

/// A server that opens the thread and starts the turn, then prints what the case hands it.
fn scripted_turn(rest: Vec<String>, opts: WireOpts) -> impl Fn(&[String]) -> Wire + Send + Sync + 'static {
    let mut lines = opened();
    lines.push(turn_started());
    lines.extend(rest);
    server(lines, opts)
}

fn on_write(f: impl Fn(&Value, &Wire) + Send + Sync + 'static) -> WireOpts {
    WireOpts { on_write: Some(Arc::new(f)), ..Default::default() }
}

fn hanging(f: impl Fn(&Value, &Wire) + Send + Sync + 'static) -> WireOpts {
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
    TurnResult { status, duration_ms: None, waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: None, error: None, unreached: None, refusal: None, limit: None }
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
        depth: s.depth,
        model: s.model.map(String::from),
        asked: s.asked.map(String::from),
        usage: s.usage,
    }
}

fn child_usage(input: u64, cached: u64, output: u64) -> SubagentUsage {
    SubagentUsage { tokens: Some(UsageTokens { input, output, cached, cache_write: 0, reasoning: 0 }), tool_uses: None, duration_ms: None }
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

    fn running(on_interrupt: Option<Arc<dyn Fn(&Wire, &Value) + Send + Sync>>) -> Launch {
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
        let live = launcher(scripted_server(hanging(|m, w| {
            if method(m) == "thread/start" {
                w.push(&opened());
            }
        })));
        let session = go(&adapter_over(&live, Extra { grace_ms: Some(60_000), ..Default::default() }), "x", &Events::default());
        until(|| wrote(&live, "turn/start")).await;
        let mut steered = session.steer("too soon".into());
        let _stopping = tokio::spawn(session.interrupt());
        assert!(!pending_after(&mut steered, 50).await);
        assert_eq!(steered.await, SteerOutcome::NotRunning);
        assert!(!live.wire(0).wrote("turn/steer"));
        live.wire(0).exit(Some(0));
        session.finished().await;
    }

    #[tokio::test]
    async fn a_steer_waiting_for_a_turn_started_that_never_comes_answers_not_running_once_the() {
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
        let live = running(Some(Arc::new(|w: &Wire, m: &Value| w.push1(result_to(m, json!({}))))));
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra { grace_ms: Some(5_000), ..Default::default() }), "x", &events);
        until(|| events.any("turn.anchor")).await;
        let stopping = tokio::spawn(session.interrupt());
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
                "turn/start" => w.push(&[turn_started(), spawn_item("started"), spawn_item("completed"), activity("started"), child_started(), usage_of(CHILD, CHILD_TURN, 900, 900)]),
                "turn/interrupt" if interrupts => {
                    w.push1(result_to(m, json!({})));
                    w.push1(if m["params"]["threadId"] == CHILD { child_ended("interrupted", json!(null)) } else { completed("interrupted") });
                }
                _ => {}
            })),
            ..Default::default()
        }))
    }

    #[tokio::test]
    async fn never_lets_a_child_s_turn_set_end_or_rename_the_lead_s_the_steer_names_the_lead() {
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

        live.wire(0).push(&[agent_message("lead_m1", "the lead's reply"), usage_of(THREAD_ID, TURN_ID, 36_576, 18_315), completed("completed")]);
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
        let live = spawning(false, false);
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra { result_exit_ms: Some(15), grace_ms: Some(15), ..Default::default() }), "spawn one agent", &events);
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
                Sub { parent: Some(SPAWN), title: Some("Sleep 20 s and say done."), asked: Some("Sleep 20 s and say done.\nEnd without waiting."), ..Default::default() }
            )
        );
        let without_usage: Vec<AdapterEvent> = subagents_of(&events).into_iter().filter(|e| matches!(e, AdapterEvent::Subagent { usage: None, .. })).collect();
        let last = without_usage.last().expect("a subagent event without usage");
        assert_eq!(sub_task_state(last), (CHILD.to_string(), SubagentState::Running));
        assert!(matches!(last, AdapterEvent::Subagent { depth: Some(1), .. }), "{last:?}");
        assert_eq!(
            deltas_for(&events, SPAWN),
            vec![tool_use_delta(THREAD_ID, SPAWN, "Sleep 20 s and say done.\nEnd without waiting."), tool_result_delta(THREAD_ID, SPAWN, false)]
        );

        live.wire(0).push(&[child_says("done"), child_ended("completed", json!(null))]);
        let result = session.finished().await;
        let usage = child_usage(900, 0, 0);
        let with_usage: Vec<AdapterEvent> = subagents_of(&events).into_iter().filter(|e| matches!(e, AdapterEvent::Subagent { usage: Some(_), .. })).collect();
        assert_eq!(
            with_usage,
            vec![
                sub(THREAD_ID, CHILD, SubagentState::Running, Sub { parent: Some(SPAWN), usage: Some(usage.clone()), ..Default::default() }),
                sub(THREAD_ID, CHILD, SubagentState::Done, Sub { parent: Some(SPAWN), summary: Some("done"), usage: Some(usage), ..Default::default() }),
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
        let failed_spawn = spawn_item("completed").replace(r#""status":"completed""#, r#""status":"failed""#);
        let live = launcher(scripted_server(on_write(move |m, w| match method(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => w.push(&[turn_started(), failed_spawn.clone(), agent_message("lead_m1", "could not spawn"), completed("completed")]),
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
        let live = launcher(scripted_server(on_write(|m, w| match method(m) {
            "thread/start" => w.push(&opened()),
            "turn/start" => w.push(&[turn_started(), child_started(), completed("completed")]),
            _ => {}
        })));
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra::default()), "x", &events);
        until(|| events.any("subagent")).await;
        assert_eq!(subagents_of(&events)[0], sub(THREAD_ID, CHILD, SubagentState::Running, Sub { parent: Some(CHILD), ..Default::default() }));
        sleep_ms(10).await;
        assert!(!events.any("turn.done"));
        live.wire(0).push1(child_ended("completed", json!(null)));
        assert_eq!(session.finished().await.status, TurnStatus::Completed);
    }

    #[tokio::test]
    async fn a_stop_with_a_child_running_interrupts_the_child_first_then_the_lead_and_the_chi() {
        let live = spawning(true, false);
        let events = Events::default();
        let session = go(&adapter_over(&live, Extra { grace_ms: Some(5_000), ..Default::default() }), "spawn one agent", &events);
        until(|| events.any("subagent")).await;
        session.interrupt().await;
        assert_eq!(interrupts_of(&live), vec![json!({ "threadId": CHILD, "turnId": CHILD_TURN }), json!({ "threadId": THREAD_ID, "turnId": TURN_ID })]);
        assert_eq!(session.finished().await, bare(TurnStatus::Interrupted));
        assert_last_sub(&events, CHILD, SubagentState::Stopped);
        assert_eq!(live.wire(0).order(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn a_stop_after_the_lead_s_reply_interrupts_the_child_alone_and_the_held_reply_stan() {
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
        let launch = launcher(server(fixture_lines("codex", "app-server-subagent.jsonl"), WireOpts::default()));
        let events = Events::default();
        let result = go(&adapter_over(&launch, Extra::default()), "spawn one agent and end your turn", &events).finished().await;
        let said = subagents_of(&events);
        // The child's row carries its own thread's total; the lead's turn still counts the child's calls in its tokens.
        let usage = child_usage(19_515, 19_200, 5);
        let expected = vec![
            sub(LEAD, CHILD, SubagentState::Running, Sub { parent: Some(SPAWN), title: Some(ASKED), model: Some("gpt-5.6-luna"), asked: Some(ASKED), ..Default::default() }),
            sub(LEAD, CHILD, SubagentState::Running, Sub { parent: Some(SPAWN), usage: Some(usage.clone()), ..Default::default() }),
            sub(LEAD, CHILD, SubagentState::Done, Sub { parent: Some(SPAWN), summary: Some("done"), usage: Some(usage), ..Default::default() }),
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
        let spawn_deltas: Vec<(DeltaKind, Option<String>, String)> = deltas_for(&events, SPAWN).into_iter().map(|d| (d.kind, d.tool_name, d.text)).collect();
        assert_eq!(
            spawn_deltas,
            vec![(DeltaKind::ToolUse, Some("spawn_agent".to_string()), json!({ "prompt": ASKED }).to_string()), (DeltaKind::ToolResult, None, String::new())]
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
        let launch = launcher(server(fixture_lines("codex", "app-server-turn.jsonl"), WireOpts::default()));
        let attached: Arc<Mutex<Vec<(String, bool)>>> = Arc::new(Mutex::new(Vec::new()));
        let replay = wire(WireOpts::default());
        replay.push(&fixture_lines("codex", "app-server-turn.jsonl"));
        replay.exit(Some(0));
        let (seen, stream) = (attached.clone(), replay.clone());
        launch.set_attach(move |run, options| {
            seen.lock().unwrap().push((run.to_string(), options.input));
            let stream = stream.stream();
            async move { Ok(Attached::Stream(stream)) }.boxed()
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
            AdapterEvent::SessionStart { session_id: RECORDED_THREAD.into(), model: Some("gpt-5.5".into()), cwd: Some("/root/app".into()), tools: None, harness: None, prompted: None }
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
        let launch = launcher(scripted_turn(vec![agent_message("msg_1", "done"), completed("completed")], WireOpts { hang: true, ..Default::default() }));
        let events = Events::default();
        let result = go(&adapter_over(&launch, Extra { grace_ms: Some(15), result_exit_ms: Some(15), ..Default::default() }), "x", &events).finished().await;
        assert_eq!(launch.wire(0).order(), vec!["teardown", "kill"]);
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text.as_deref(), Some("done"));
        assert!(session_end_saw_result(&events));
    }

    #[tokio::test]
    async fn a_server_that_exits_on_eof_is_left_alone() {
        let launch = launcher(server(fixture_lines("codex", "app-server-turn.jsonl"), WireOpts::default()));
        let result = go_quiet(&adapter_over(&launch, Extra { grace_ms: Some(15), result_exit_ms: Some(15), ..Default::default() }), "x").finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        sleep_ms(60).await;
        assert_eq!(launch.wire(0).order(), Vec::<String>::new());
    }
}

mod what_a_rewind_needs_of_a_codex_turn {
    use super::*;

    #[tokio::test]
    async fn names_the_turn_by_the_id_the_server_gave_it_at_turn_started_once() {
        let launch = launcher(scripted_turn(vec![agent_message("msg_1", "done"), completed("completed")], WireOpts::default()));
        let events = Events::default();
        go(&adapter_over(&launch, Extra::default()), "x", &events).finished().await;
        assert_eq!(events.of("turn.anchor"), vec![turn_anchor(None)]);
    }

    #[tokio::test]
    async fn on_a_thread_an_older_codex_made_names_each_turn_s_anchor_with_why_its_history_ca() {
        let head = opened_with(THREAD_ID, true);
        let legacy = server(vec![head[0].clone(), head[1].clone(), turn_started(), agent_message("msg_1", "done"), completed("completed")], WireOpts::default());
        let events = Events::default();
        go_resume(&adapter_over(&launcher(legacy), Extra::default()), "x", &events).finished().await;
        assert_eq!(events.of("turn.anchor"), vec![turn_anchor(Some(CODEX_LEGACY_HISTORY))]);
    }

    #[tokio::test]
    async fn reads_the_history_mode_off_this_thread_s_own_announcement_alone_never_a_later_th() {
        let other = json!({ "method": "thread/started", "params": { "thread": { "id": "01a0e2c1-0000-7000-8000-00000000c41d", "historyMode": "legacy" } } }).to_string();
        let mut lines = opened();
        lines.extend([other, turn_started(), agent_message("msg_1", "done"), completed("completed")]);
        let events = Events::default();
        go(&adapter_over(&launcher(server(lines, WireOpts::default())), Extra::default()), "x", &events).finished().await;
        assert_eq!(events.of("turn.anchor"), vec![turn_anchor(None)]);
    }
}

mod a_codex_account_s_plan_limits_on_the_app_server {
    use super::*;

    // Shapes from `codex app-server generate-ts` on codex-cli 0.157.1: GetAccountResponse, GetAccountRateLimitsResponse,
    // AccountRateLimitsUpdatedNotification.
    fn account(value: Value) -> String {
        json!({ "id": "wsp-account", "result": { "account": value, "requiresOpenaiAuth": true } }).to_string()
    }

    fn window(used_percent: f64, window_duration_mins: i64, resets_at: i64) -> Value {
        json!({ "usedPercent": used_percent, "windowDurationMins": window_duration_mins, "resetsAt": resets_at })
    }

    #[derive(Default)]
    struct Snapshot {
        primary: Option<Value>,
        secondary: Option<Value>,
        plan_type: Option<&'static str>,
        reached: Option<&'static str>,
    }

    fn snapshot(o: Snapshot) -> Value {
        json!({
            "limitId": "codex",
            "limitName": null,
            "normalModelSlug": null,
            "primary": o.primary,
            "secondary": o.secondary,
            "credits": null,
            "individualLimit": null,
            "spendControlReached": null,
            "planType": o.plan_type,
            "rateLimitReachedType": o.reached
        })
    }

    fn primary_only(primary: Value) -> Value {
        snapshot(Snapshot { primary: Some(primary), ..Default::default() })
    }

    fn rate_limits(s: Value, account_id: Option<&str>) -> String {
        json!({ "id": "wsp-rate-limits", "result": { "ordinaryUsageAllowed": true, "rateLimits": s, "rateLimitsByLimitId": null, "rateLimitResetCredits": null, "accountId": account_id, "rateLimitUpsell": null } }).to_string()
    }

    fn updated(s: Value) -> String {
        json!({ "method": "account/rateLimits/updated", "params": { "rateLimits": s } }).to_string()
    }

    fn read(s: Value, extra: Value) -> String {
        let mut result = json!({ "ordinaryUsageAllowed": true, "rateLimits": s, "rateLimitsByLimitId": null, "rateLimitResetCredits": null, "accountId": "acct_7f3a", "rateLimitUpsell": null });
        for (k, v) in extra.as_object().expect("extra is an object") {
            result[k] = v.clone();
        }
        json!({ "id": "wsp-rate-limits", "result": result }).to_string()
    }

    fn limits_of(events: &Events) -> Vec<HarnessLimit> {
        events
            .all()
            .into_iter()
            .filter_map(|e| match e {
                AdapterEvent::Limit { limit, .. } => Some(limit),
                _ => None,
            })
            .collect()
    }

    fn win(kind: LimitKind, used: f64, resets_at_ms: f64) -> LimitWindow {
        LimitWindow { kind, used_percent: JsNumber(used), resets_at: Some(JsNumber(resets_at_ms)) }
    }

    fn limit(windows: Vec<LimitWindow>) -> HarnessLimit {
        HarnessLimit { windows, plan: None, status: None, account: None, keyed: None, credits: None }
    }

    fn dev_account() -> Option<HarnessLimitAccount> {
        Some(HarnessLimitAccount { id: "acct_7f3a".into(), label: Some("dev@example.com".into()) })
    }

    fn plus_account() -> String {
        account(json!({ "type": "chatgpt", "email": "dev@example.com", "planType": "plus" }))
    }

    async fn limits_after(lines: Vec<String>) -> (Launch, Vec<HarnessLimit>) {
        let launch = launcher(scripted_turn(lines, WireOpts::default()));
        let events = Events::default();
        go(&adapter_over(&launch, Extra::default()), "hi", &events).finished().await;
        let limits = limits_of(&events);
        (launch, limits)
    }

    #[tokio::test]
    async fn asks_the_account_and_its_limits_once_as_the_turn_opens_and_reads_the_windows_by() {
        let (launch, limits) = limits_after(vec![
            plus_account(),
            rate_limits(snapshot(Snapshot { primary: Some(window(34.5, 300, 1_790_700_000)), secondary: Some(window(12.0, 10_080, 1_791_200_000)), plan_type: Some("plus"), ..Default::default() }), Some("acct_7f3a")),
            agent_message("m1", "done"),
            completed("completed"),
        ])
        .await;
        let seed: Vec<String> = launch.call(0).input.unwrap_or_default().iter().map(|l| method(&serde_json::from_str::<Value>(l).unwrap()).to_string()).collect();
        assert_eq!(seed, vec!["initialize", "initialized", "account/read", "account/rateLimits/read", "thread/start"]);
        assert_eq!(
            limits,
            vec![HarnessLimit {
                windows: vec![win(LimitKind::Session, 34.5, 1_790_700_000_000.0), win(LimitKind::Week, 12.0, 1_791_200_000_000.0)],
                plan: Some("plus".into()),
                status: Some(LimitStatus::Ok),
                account: dev_account(),
                keyed: None,
                credits: None,
            }]
        );
    }

    #[tokio::test]
    async fn merges_a_rolling_update_into_the_last_reading_a_window_it_leaves_out_kept_and_a() {
        let (_, limits) = limits_after(vec![
            plus_account(),
            rate_limits(snapshot(Snapshot { primary: Some(window(34.5, 300, 1_790_700_000)), secondary: Some(window(12.0, 10_080, 1_791_200_000)), plan_type: Some("plus"), ..Default::default() }), Some("acct_7f3a")),
            updated(snapshot(Snapshot { primary: Some(window(100.0, 300, 1_790_700_000)), reached: Some("rate_limit_reached"), ..Default::default() })),
            completed("completed"),
        ])
        .await;
        assert_eq!(
            limits.last(),
            Some(&HarnessLimit {
                windows: vec![win(LimitKind::Session, 100.0, 1_790_700_000_000.0), win(LimitKind::Week, 12.0, 1_791_200_000_000.0)],
                plan: Some("plus".into()),
                status: Some(LimitStatus::Reached),
                account: dev_account(),
                keyed: None,
                credits: None,
            })
        );
    }

    #[tokio::test]
    async fn reads_a_window_longer_than_a_week_as_the_month() {
        let (_, limits) = limits_after(vec![rate_limits(primary_only(window(5.0, 43_200, 1_792_000_000)), None), completed("completed")]).await;
        assert_eq!(limits, vec![HarnessLimit { status: Some(LimitStatus::Ok), ..limit(vec![win(LimitKind::Month, 5.0, 1_792_000_000_000.0)]) }]);
    }

    #[tokio::test]
    async fn says_a_sign_in_by_api_key_has_no_plan_window() {
        let (_, limits) = limits_after(vec![account(json!({ "type": "apiKey" })), completed("completed")]).await;
        assert_eq!(limits, vec![HarnessLimit { keyed: Some(true), ..limit(vec![]) }]);
    }

    fn banked(credits: Value) -> Value {
        json!({ "availableCount": 2, "credits": credits })
    }

    #[tokio::test]
    async fn asks_for_each_banked_reset_in_full_on_a_turn_the_runtime_says_is_due_for_it_and() {
        async fn params_of(limit_details: bool) -> Option<Value> {
            let launch = launcher(scripted_turn(vec![completed("completed")], WireOpts::default()));
            let adapter = adapter_over(&launch, Extra::default());
            adapter.start(HarnessStartOptions { prompt: "hi".into(), limit_details, on_event: Some(no_events()), ..Default::default() }).unwrap().finished().await;
            launch
                .call(0)
                .input
                .unwrap_or_default()
                .iter()
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .find(|m| method(m) == "account/rateLimits/read")
                .map(|m| m["params"].clone())
        }
        assert_eq!(params_of(true).await, Some(json!({})));
        assert_eq!(params_of(false).await, Some(json!({ "excludeResetCreditDetails": true })));
    }

    #[tokio::test]
    async fn reads_how_many_resets_the_plan_has_banked_off_the_turn_s_read_where_the_details() {
        let (_, limits) = limits_after(vec![read(primary_only(window(34.5, 300, 1_790_700_000)), json!({ "rateLimitResetCredits": banked(json!(null)) })), completed("completed")]).await;
        let credits: Vec<Option<BankedResets>> = limits.into_iter().map(|l| l.credits).collect();
        assert_eq!(credits, vec![Some(BankedResets { count: 2, credits: None })]);
    }

    #[tokio::test]
    async fn keeps_the_banked_resets_off_a_rolling_update_which_never_carries_them_so_the_las() {
        let (_, limits) = limits_after(vec![
            read(primary_only(window(34.5, 300, 1_790_700_000)), json!({ "rateLimitResetCredits": banked(json!(null)) })),
            updated(primary_only(window(40.0, 300, 1_790_700_000))),
            completed("completed"),
        ])
        .await;
        let has_credits: Vec<bool> = limits.iter().map(|l| l.credits.is_some()).collect();
        assert_eq!(has_credits, vec![true, false]);
    }

    #[tokio::test]
    async fn reads_the_windows_off_the_codex_bucket_where_the_answer_names_buckets_and_merges() {
        let mut other = primary_only(window(90.0, 300, 1_790_700_000));
        other["limitId"] = json!("codex_other");
        let (_, limits) = limits_after(vec![
            read(primary_only(window(70.0, 300, 1_790_700_000)), json!({ "rateLimitsByLimitId": { "codex": primary_only(window(10.0, 300, 1_790_700_000)), "codex_other": other.clone() } })),
            updated(other),
            completed("completed"),
        ])
        .await;
        let used: Vec<Vec<JsNumber>> = limits.iter().map(|l| l.windows.iter().map(|w| w.used_percent).collect()).collect();
        assert_eq!(used, vec![vec![JsNumber(10.0)]]);
    }

    mod a_turn_the_plan_s_usage_limit_stopped {
        use super::*;

        // The sentence the owner saw when a Plus window was spent, under the code the app server's schema names for it;
        // not a recording of a real limit.
        const SAID: &str = "You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing) or try again at 1:13 PM.";

        fn error(code: &str) -> String {
            json!({ "method": "error", "params": { "threadId": THREAD_ID, "turnId": TURN_ID, "willRetry": false, "error": { "message": SAID, "codexErrorInfo": code, "additionalDetails": null } } }).to_string()
        }

        fn failed(code: &str) -> String {
            json!({ "method": "turn/completed", "params": { "threadId": THREAD_ID, "turn": { "id": TURN_ID, "items": [], "status": "failed", "error": { "message": SAID, "codexErrorInfo": code, "additionalDetails": null } } } }).to_string()
        }

        fn now_secs() -> i64 {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
        }

        fn in_an_hour() -> i64 {
            now_secs() + 3600
        }

        async fn run(lines: Vec<String>) -> TurnResult {
            let launch = launcher(scripted_turn(lines, WireOpts::default()));
            go_quiet(&adapter_over(&launch, Extra::default()), "hi").finished().await
        }

        #[tokio::test]
        async fn carries_the_limit_with_the_reset_of_the_window_the_latest_reading_has_at_its_cap() {
            let hour = in_an_hour();
            let result = run(vec![
                read(snapshot(Snapshot { primary: Some(window(97.0, 300, hour)), secondary: Some(window(40.0, 10_080, hour + 86_400)), ..Default::default() }), json!({})),
                updated(snapshot(Snapshot { primary: Some(window(100.0, 300, hour)), reached: Some("rate_limit_reached"), ..Default::default() })),
                error("usageLimitExceeded"),
                failed("usageLimitExceeded"),
            ])
            .await;
            assert_eq!(result.status, TurnStatus::Failed);
            assert_eq!(result.limit, Some(TurnLimit { resets_at: Some(JsNumber((hour * 1000) as f64)) }));
        }

        #[tokio::test]
        async fn carries_the_reset_of_the_fullest_window_ahead_where_the_server_stopped_the_turn() {
            let hour = in_an_hour();
            let result = run(vec![
                read(snapshot(Snapshot { primary: Some(window(97.0, 300, hour)), secondary: Some(window(40.0, 10_080, hour + 86_400)), ..Default::default() }), json!({})),
                updated(primary_only(window(99.0, 300, hour))),
                error("usageLimitExceeded"),
                failed("usageLimitExceeded"),
            ])
            .await;
            assert_eq!(result.status, TurnStatus::Failed);
            assert_eq!(result.limit, Some(TurnLimit { resets_at: Some(JsNumber((hour * 1000) as f64)) }));
        }

        #[tokio::test]
        async fn carries_the_limit_with_no_reset_where_no_window_names_one_ahead() {
            let result = run(vec![read(primary_only(window(99.0, 300, now_secs() - 60)), json!({})), failed("usageLimitExceeded")]).await;
            assert_eq!(result.status, TurnStatus::Failed);
            assert_eq!(result.limit, Some(TurnLimit { resets_at: None }));
        }

        #[tokio::test]
        async fn carries_none_on_a_turn_that_failed_another_way_whatever_the_reading_says() {
            let result = run(vec![
                updated(snapshot(Snapshot { primary: Some(window(100.0, 300, in_an_hour())), reached: Some("rate_limit_reached"), ..Default::default() })),
                error("serverOverloaded"),
                failed("serverOverloaded"),
            ])
            .await;
            assert_eq!(result.status, TurnStatus::Failed);
            assert_eq!(result.limit, None);
        }
    }
}

mod a_codex_thread_s_own_compaction {
    use super::*;

    // codex-cli 0.155.1's answer to thread/compact/start on a thread holding 20,964 tokens (recorded 2026-10-05, ids
    // swapped for the test's): a turn of its own whose one item is the compaction, and a usage line whose last call
    // reports no input or output but the context the thread now holds as its total.
    fn compaction() -> Vec<String> {
        vec![
            json!({ "id": "wsp-turn", "result": {} }).to_string(),
            turn_started(),
            json!({ "method": "item/started", "params": { "item": { "type": "contextCompaction", "id": "cc1" }, "threadId": THREAD_ID, "turnId": TURN_ID } }).to_string(),
            json!({ "method": "item/completed", "params": { "item": { "type": "contextCompaction", "id": "cc1" }, "threadId": THREAD_ID, "turnId": TURN_ID } }).to_string(),
            json!({
                "method": "thread/tokenUsage/updated",
                "params": {
                    "threadId": THREAD_ID,
                    "turnId": TURN_ID,
                    "tokenUsage": {
                        "total": { "totalTokens": 20964, "inputTokens": 20959, "cachedInputTokens": 11136, "cacheWriteInputTokens": 0, "outputTokens": 5, "reasoningOutputTokens": 0 },
                        "last": { "totalTokens": 4607, "inputTokens": 0, "cachedInputTokens": 0, "cacheWriteInputTokens": 0, "outputTokens": 0, "reasoningOutputTokens": 0 },
                        "modelContextWindow": 258400
                    }
                },
                "emittedAtMs": 1791150174233_i64
            })
            .to_string(),
            json!({ "method": "turn/completed", "params": { "threadId": THREAD_ID, "turn": { "id": TURN_ID, "items": [], "status": "completed", "durationMs": 3086 } } }).to_string(),
        ]
    }

    fn compacting_server() -> impl Fn(&[String]) -> Wire + Send + Sync + 'static {
        scripted_server(on_write(|m, w| match method(m) {
            "thread/resume" => w.push(&opened()),
            "thread/compact/start" => w.push(&compaction()),
            _ => {}
        }))
    }

    #[tokio::test]
    async fn runs_compact_as_the_server_s_own_compaction_of_the_thread_not_as_a_message_to_th() {
        let launch = launcher(compacting_server());
        let adapter = adapter_over(&launch, Extra::default());
        assert_eq!(adapter.compacts(), "/compact");
        let result = go_resume(&adapter, "/compact", &Events::default()).finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert!(launch.wire(0).wrote("thread/compact/start"));
        assert!(!launch.wire(0).wrote("turn/start"));
        assert_eq!(launch.wire(0).written_method("thread/compact/start")[0], json!({ "id": "wsp-turn", "method": "thread/compact/start", "params": { "threadId": THREAD_ID } }));
    }

    #[tokio::test]
    async fn reads_what_the_thread_holds_after_the_compaction_off_the_usage_line_s_total_wher() {
        let launch = launcher(compacting_server());
        let result = go_resume(&adapter_over(&launch, Extra::default()), "/compact", &Events::default()).finished().await;
        let tokens = tokens_of(&result);
        assert_eq!(tokens.context, Some(JsNumber(4_607.0)));
        assert_eq!(tokens.window, Some(JsNumber(258_400.0)));
    }

    #[tokio::test]
    async fn says_the_compact_turn_s_compaction_once_with_what_the_thread_holds_after_it() {
        let launch = launcher(compacting_server());
        let events = Events::default();
        go_resume(&adapter_over(&launch, Extra::default()), "/compact", &events).finished().await;
        assert_eq!(events.of("turn.compacted"), vec![AdapterEvent::TurnCompacted { session_id: THREAD_ID.into(), before: None, after: Some(4_607.0) }]);
    }

    #[tokio::test]
    async fn says_a_compaction_the_server_ran_by_itself_at_a_turn_s_start_with_what_the_threa() {
        // The recording's first line names its own thread and turn, swapped here for the test's.
        let recorded = fixture_lines("codex", "auto-compaction.jsonl");
        let ids: Value = codex_wire::parse(&recorded[0]);
        let (old_thread, old_turn) = (ids["threadId"].as_str().unwrap().to_string(), ids["turnId"].as_str().unwrap().to_string());
        let swapped: Vec<String> = recorded[1..].iter().map(|l| l.replace(&old_thread, THREAD_ID).replace(&old_turn, TURN_ID)).collect();
        let (before, turn) = (swapped[0].clone(), swapped[1..].to_vec());
        let launch = launcher(scripted_server(on_write(move |m, w| match method(m) {
            "thread/resume" => {
                let mut lines = opened();
                lines.push(before.clone());
                w.push(&lines);
            }
            "turn/start" => {
                let mut lines = vec![json!({ "id": "wsp-turn", "result": {} }).to_string()];
                lines.extend(turn.clone());
                w.push(&lines);
            }
            _ => {}
        })));
        let events = Events::default();
        let result = go_resume(&adapter_over(&launch, Extra::default()), "go", &events).finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(events.of("turn.compacted"), vec![AdapterEvent::TurnCompacted { session_id: THREAD_ID.into(), before: Some(31_250.0), after: Some(18_935.0) }]);
    }

    #[tokio::test]
    async fn sends_any_other_message_as_a_turn_compact_with_words_after_it_included() {
        let launch = launcher(scripted_turn(vec![agent_message("m1", "done"), completed("completed")], WireOpts::default()));
        go_resume(&adapter_over(&launch, Extra::default()), "/compact the notes please", &Events::default()).finished().await;
        assert!(launch.wire(0).wrote("turn/start"));
    }
}
