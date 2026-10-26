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
        depth: s.depth,
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
        tokio::task::LocalSet::new().run_until(asks_the_account_and_its_limits_once_as_the_turn_opens_and_reads_the_windows_by_body()).await
    }

    async fn asks_the_account_and_its_limits_once_as_the_turn_opens_and_reads_the_windows_by_body() {
        let (launch, limits) = limits_after(vec![
            plus_account(),
            rate_limits(
                snapshot(Snapshot {
                    primary: Some(window(34.5, 300, 1_790_700_000)),
                    secondary: Some(window(12.0, 10_080, 1_791_200_000)),
                    plan_type: Some("plus"),
                    ..Default::default()
                }),
                Some("acct_7f3a"),
            ),
            agent_message("m1", "done"),
            completed("completed"),
        ])
        .await;
        let seed: Vec<String> = launch
            .call(0)
            .input
            .unwrap_or_default()
            .iter()
            .map(|l| method(&serde_json::from_str::<Value>(l).unwrap()).to_string())
            .collect();
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
        tokio::task::LocalSet::new().run_until(merges_a_rolling_update_into_the_last_reading_a_window_it_leaves_out_kept_and_a_body()).await
    }

    async fn merges_a_rolling_update_into_the_last_reading_a_window_it_leaves_out_kept_and_a_body() {
        let (_, limits) = limits_after(vec![
            plus_account(),
            rate_limits(
                snapshot(Snapshot {
                    primary: Some(window(34.5, 300, 1_790_700_000)),
                    secondary: Some(window(12.0, 10_080, 1_791_200_000)),
                    plan_type: Some("plus"),
                    ..Default::default()
                }),
                Some("acct_7f3a"),
            ),
            updated(snapshot(Snapshot {
                primary: Some(window(100.0, 300, 1_790_700_000)),
                reached: Some("rate_limit_reached"),
                ..Default::default()
            })),
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
        tokio::task::LocalSet::new().run_until(reads_a_window_longer_than_a_week_as_the_month_body()).await
    }

    async fn reads_a_window_longer_than_a_week_as_the_month_body() {
        let (_, limits) =
            limits_after(vec![rate_limits(primary_only(window(5.0, 43_200, 1_792_000_000)), None), completed("completed")]).await;
        assert_eq!(
            limits,
            vec![HarnessLimit { status: Some(LimitStatus::Ok), ..limit(vec![win(LimitKind::Month, 5.0, 1_792_000_000_000.0)]) }]
        );
    }

    #[tokio::test]
    async fn says_a_sign_in_by_api_key_has_no_plan_window() {
        tokio::task::LocalSet::new().run_until(says_a_sign_in_by_api_key_has_no_plan_window_body()).await
    }

    async fn says_a_sign_in_by_api_key_has_no_plan_window_body() {
        let (_, limits) = limits_after(vec![account(json!({ "type": "apiKey" })), completed("completed")]).await;
        assert_eq!(limits, vec![HarnessLimit { keyed: Some(true), ..limit(vec![]) }]);
    }

    fn banked(credits: Value) -> Value {
        json!({ "availableCount": 2, "credits": credits })
    }

    #[tokio::test]
    async fn asks_for_each_banked_reset_in_full_on_a_turn_the_runtime_says_is_due_for_it_and() {
        tokio::task::LocalSet::new().run_until(asks_for_each_banked_reset_in_full_on_a_turn_the_runtime_says_is_due_for_it_and_body()).await
    }

    async fn asks_for_each_banked_reset_in_full_on_a_turn_the_runtime_says_is_due_for_it_and_body() {
        async fn params_of(limit_details: bool) -> Option<Value> {
            let launch = launcher(scripted_turn(vec![completed("completed")], WireOpts::default()));
            let adapter = adapter_over(&launch, Extra::default());
            adapter
                .start(HarnessStartOptions { prompt: "hi".into(), limit_details, on_event: Some(no_events()), ..Default::default() })
                .unwrap()
                .finished()
                .await;
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
        tokio::task::LocalSet::new().run_until(reads_how_many_resets_the_plan_has_banked_off_the_turn_s_read_where_the_details_body()).await
    }

    async fn reads_how_many_resets_the_plan_has_banked_off_the_turn_s_read_where_the_details_body() {
        let (_, limits) = limits_after(vec![
            read(primary_only(window(34.5, 300, 1_790_700_000)), json!({ "rateLimitResetCredits": banked(json!(null)) })),
            completed("completed"),
        ])
        .await;
        let credits: Vec<Option<BankedResets>> = limits.into_iter().map(|l| l.credits).collect();
        assert_eq!(credits, vec![Some(BankedResets { count: 2.into(), credits: None })]);
    }

    #[tokio::test]
    async fn keeps_the_banked_resets_off_a_rolling_update_which_never_carries_them_so_the_las() {
        tokio::task::LocalSet::new()
            .run_until(keeps_the_banked_resets_off_a_rolling_update_which_never_carries_them_so_the_las_body())
            .await
    }

    async fn keeps_the_banked_resets_off_a_rolling_update_which_never_carries_them_so_the_las_body() {
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
        tokio::task::LocalSet::new()
            .run_until(reads_the_windows_off_the_codex_bucket_where_the_answer_names_buckets_and_merges_body())
            .await
    }

    async fn reads_the_windows_off_the_codex_bucket_where_the_answer_names_buckets_and_merges_body() {
        let mut other = primary_only(window(90.0, 300, 1_790_700_000));
        other["limitId"] = json!("codex_other");
        let (_, limits) = limits_after(vec![
            read(
                primary_only(window(70.0, 300, 1_790_700_000)),
                json!({ "rateLimitsByLimitId": { "codex": primary_only(window(10.0, 300, 1_790_700_000)), "codex_other": other.clone() } }),
            ),
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
            tokio::task::LocalSet::new()
                .run_until(carries_the_limit_with_the_reset_of_the_window_the_latest_reading_has_at_its_cap_body())
                .await
        }

        async fn carries_the_limit_with_the_reset_of_the_window_the_latest_reading_has_at_its_cap_body() {
            let hour = in_an_hour();
            let result = run(vec![
                read(
                    snapshot(Snapshot {
                        primary: Some(window(97.0, 300, hour)),
                        secondary: Some(window(40.0, 10_080, hour + 86_400)),
                        ..Default::default()
                    }),
                    json!({}),
                ),
                updated(snapshot(Snapshot {
                    primary: Some(window(100.0, 300, hour)),
                    reached: Some("rate_limit_reached"),
                    ..Default::default()
                })),
                error("usageLimitExceeded"),
                failed("usageLimitExceeded"),
            ])
            .await;
            assert_eq!(result.status, TurnStatus::Failed);
            assert_eq!(result.limit, Some(TurnLimit { resets_at: Some(JsNumber((hour * 1000) as f64)) }));
        }

        #[tokio::test]
        async fn carries_the_reset_of_the_fullest_window_ahead_where_the_server_stopped_the_turn() {
            tokio::task::LocalSet::new()
                .run_until(carries_the_reset_of_the_fullest_window_ahead_where_the_server_stopped_the_turn_body())
                .await
        }

        async fn carries_the_reset_of_the_fullest_window_ahead_where_the_server_stopped_the_turn_body() {
            let hour = in_an_hour();
            let result = run(vec![
                read(
                    snapshot(Snapshot {
                        primary: Some(window(97.0, 300, hour)),
                        secondary: Some(window(40.0, 10_080, hour + 86_400)),
                        ..Default::default()
                    }),
                    json!({}),
                ),
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
            tokio::task::LocalSet::new().run_until(carries_the_limit_with_no_reset_where_no_window_names_one_ahead_body()).await
        }

        async fn carries_the_limit_with_no_reset_where_no_window_names_one_ahead_body() {
            let result = run(vec![read(primary_only(window(99.0, 300, now_secs() - 60)), json!({})), failed("usageLimitExceeded")]).await;
            assert_eq!(result.status, TurnStatus::Failed);
            assert_eq!(result.limit, Some(TurnLimit { resets_at: None }));
        }

        #[tokio::test]
        async fn carries_none_on_a_turn_that_failed_another_way_whatever_the_reading_says() {
            tokio::task::LocalSet::new().run_until(carries_none_on_a_turn_that_failed_another_way_whatever_the_reading_says_body()).await
        }

        async fn carries_none_on_a_turn_that_failed_another_way_whatever_the_reading_says_body() {
            let result = run(vec![
                updated(snapshot(Snapshot {
                    primary: Some(window(100.0, 300, in_an_hour())),
                    reached: Some("rate_limit_reached"),
                    ..Default::default()
                })),
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

    fn compacting_server() -> impl Fn(&[String]) -> Wire + 'static {
        scripted_server(on_write(|m, w| match method(m) {
            "thread/resume" => w.push(&opened()),
            "thread/compact/start" => w.push(&compaction()),
            _ => {}
        }))
    }

    #[tokio::test]
    async fn runs_compact_as_the_server_s_own_compaction_of_the_thread_not_as_a_message_to_th() {
        tokio::task::LocalSet::new()
            .run_until(runs_compact_as_the_server_s_own_compaction_of_the_thread_not_as_a_message_to_th_body())
            .await
    }

    async fn runs_compact_as_the_server_s_own_compaction_of_the_thread_not_as_a_message_to_th_body() {
        let launch = launcher(compacting_server());
        let adapter = adapter_over(&launch, Extra::default());
        assert_eq!(adapter.compacts(), "/compact");
        let result = go_resume(&adapter, "/compact", &Events::default()).finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert!(launch.wire(0).wrote("thread/compact/start"));
        assert!(!launch.wire(0).wrote("turn/start"));
        assert_eq!(
            launch.wire(0).written_method("thread/compact/start")[0],
            json!({ "id": "wsp-turn", "method": "thread/compact/start", "params": { "threadId": THREAD_ID } })
        );
    }

    #[tokio::test]
    async fn reads_what_the_thread_holds_after_the_compaction_off_the_usage_line_s_total_wher() {
        tokio::task::LocalSet::new()
            .run_until(reads_what_the_thread_holds_after_the_compaction_off_the_usage_line_s_total_wher_body())
            .await
    }

    async fn reads_what_the_thread_holds_after_the_compaction_off_the_usage_line_s_total_wher_body() {
        let launch = launcher(compacting_server());
        let result = go_resume(&adapter_over(&launch, Extra::default()), "/compact", &Events::default()).finished().await;
        let tokens = tokens_of(&result);
        assert_eq!(tokens.context, Some(JsNumber(4_607.0)));
        assert_eq!(tokens.window, Some(JsNumber(258_400.0)));
    }

    #[tokio::test]
    async fn says_the_compact_turn_s_compaction_once_with_what_the_thread_holds_after_it() {
        tokio::task::LocalSet::new().run_until(says_the_compact_turn_s_compaction_once_with_what_the_thread_holds_after_it_body()).await
    }

    async fn says_the_compact_turn_s_compaction_once_with_what_the_thread_holds_after_it_body() {
        let launch = launcher(compacting_server());
        let events = Events::default();
        go_resume(&adapter_over(&launch, Extra::default()), "/compact", &events).finished().await;
        assert_eq!(
            events.of("turn.compacted"),
            vec![AdapterEvent::TurnCompacted { session_id: THREAD_ID.into(), before: None, after: Some(4_607.0) }]
        );
    }

    #[tokio::test]
    async fn says_a_compaction_the_server_ran_by_itself_at_a_turn_s_start_with_what_the_threa() {
        tokio::task::LocalSet::new()
            .run_until(says_a_compaction_the_server_ran_by_itself_at_a_turn_s_start_with_what_the_threa_body())
            .await
    }

    async fn says_a_compaction_the_server_ran_by_itself_at_a_turn_s_start_with_what_the_threa_body() {
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
        assert_eq!(
            events.of("turn.compacted"),
            vec![AdapterEvent::TurnCompacted { session_id: THREAD_ID.into(), before: Some(31_250.0), after: Some(18_935.0) }]
        );
    }

    #[tokio::test]
    async fn sends_any_other_message_as_a_turn_compact_with_words_after_it_included() {
        tokio::task::LocalSet::new().run_until(sends_any_other_message_as_a_turn_compact_with_words_after_it_included_body()).await
    }

    async fn sends_any_other_message_as_a_turn_compact_with_words_after_it_included_body() {
        let launch = launcher(scripted_turn(vec![agent_message("m1", "done"), completed("completed")], WireOpts::default()));
        go_resume(&adapter_over(&launch, Extra::default()), "/compact the notes please", &Events::default()).finished().await;
        assert!(launch.wire(0).wrote("turn/start"));
    }
}
