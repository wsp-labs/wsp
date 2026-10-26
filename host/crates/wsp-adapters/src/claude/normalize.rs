// SPDX-License-Identifier: AGPL-3.0-only
//! The pure half of the turn reader: what one stream-json line says, and what a result costs. TS:
//! packages/adapter-claude/src/adapter.ts:213-625 at a3b870a6e.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::claude::fields::{harness_of, num, rec, str_arr, string, Obj};
use crate::claude::handback::{held_call, read_answer, HandbackBook};
use crate::claude::limits::limit_of;
use crate::claude::plans::{read_plan_call, PlanBook, PlanCall};
use crate::claude::tokens::{plus_tokens, usage_of, usage_tokens};
use crate::claude::tool_results::{flatten_content, result_facts};
use crate::pending::{anthropic_retry_why, base_model, fmt_duration, refused_turn, Refusal};
use crate::port::{AdapterEvent, DeltaKind, HarnessLimit, SessionFile, TurnDelta, TurnResult, TurnStatus, TurnTokens};
use wsp_wire::text::json_stringify;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::UsageTokens;
use wsp_wire::views::session::TurnRefusal;
use wsp_wire::views::session_events::TurnResultModelsItem;
use wsp_words::js::js_trim;
use wsp_words::v8_json::parse_json_lax;

static NAMED_TASK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#([0-9]+)").expect("the task id pattern is valid"));

/// One model's use: `{ model, tokens, costUsd? }`. adapter.ts:242.
pub type ModelUse = TurnResultModelsItem;

/// JSON.parse of a tool_use delta's text; `None` where it does not parse. adapter.ts:213.
pub fn parse_input(text: &str) -> Option<Value> {
    parse_json_lax(text)
}

/// adapter.ts:221. `errors_text` is the result's errors joined by " " and lower-cased.
pub fn result_status(event: &Obj, errors_text: &str) -> TurnStatus {
    let subtype = string(event.get("subtype"));
    if matches!(string(event.get("terminal_reason")), Some("aborted_tools") | Some("aborted_streaming")) {
        return TurnStatus::Interrupted;
    }
    if subtype == Some("success") {
        return if event.get("is_error") == Some(&Value::Bool(true)) { TurnStatus::Failed } else { TurnStatus::Completed };
    }
    if errors_text.contains("interrupt") || errors_text.contains("cancel") {
        return TurnStatus::Interrupted;
    }
    if subtype == Some("error_during_execution") && event.get("is_error") == Some(&Value::Bool(false)) && errors_text.contains("aborted") {
        return TurnStatus::Interrupted;
    }
    TurnStatus::Failed
}

/// adapter.ts:248. Each entry of a modelUsage object, in its key order; non-object entries skipped.
pub fn models_of(raw: Option<&Value>) -> Vec<ModelUse> {
    rec(raw)
        .into_iter()
        .flat_map(|models| models.iter())
        .filter_map(|(model, value)| {
            let row = rec(Some(value))?;
            let cached = num(row.get("cacheReadInputTokens")).unwrap_or(0.0);
            let cache_write = num(row.get("cacheCreationInputTokens")).unwrap_or(0.0);
            let input = num(row.get("inputTokens")).unwrap_or(0.0) + cached + cache_write;
            let tokens = UsageTokens {
                input: input.into(),
                output: num(row.get("outputTokens")).unwrap_or(0.0).into(),
                cached: cached.into(),
                cache_write: cache_write.into(),
                reasoning: num(row.get("thinkingTokens")).unwrap_or(0.0).into(),
            };
            Some(ModelUse { model: model.clone(), tokens, cost_usd: num(row.get("costUSD")).map(JsNumber) })
        })
        .collect()
}

fn start_at(models: &[ModelUse], model: &str) -> Option<usize> {
    models.iter().position(|m| m.model == model).or_else(|| {
        let base = base_model(model);
        models.iter().position(|m| base_model(&m.model) == base)
    })
}

/// adapter.ts:262. By its own name first, then by base_model on both sides.
pub fn start_of<'a>(models: &'a [ModelUse], model: &str) -> Option<&'a ModelUse> {
    start_at(models, model).map(|at| &models[at])
}

fn as_turn(t: &UsageTokens) -> TurnTokens {
    TurnTokens {
        input: t.input,
        output: t.output,
        cached: Some(t.cached),
        cache_write: Some(t.cache_write),
        reasoning: Some(t.reasoning),
        context: None,
        window: None,
    }
}

/// adapter.ts:266.
pub fn with_calls(models: &[ModelUse], calls: &[ModelUse]) -> Vec<ModelUse> {
    let mut out = models.to_vec();
    for call in calls {
        match start_at(&out, &call.model) {
            None => out.push(call.clone()),
            Some(at) => {
                let joined = plus_tokens(Some(&as_turn(&out[at].tokens)), &as_turn(&call.tokens));
                out[at].tokens = usage_of(&joined);
            }
        }
    }
    out
}

/// The turn's tokens and running totals off its result. adapter.ts:278.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResultTokens {
    pub tokens: Option<TurnTokens>,
    pub models: Option<Vec<ModelUse>>,
}

/// adapter.ts:278.
pub fn result_tokens(event: &Obj) -> ResultTokens {
    let windows: Vec<f64> = rec(event.get("modelUsage"))
        .into_iter()
        .flat_map(|models| models.values())
        .filter_map(|raw| num(rec(Some(raw)).and_then(|r| r.get("contextWindow"))))
        .collect();
    let tokens = rec(event.get("usage")).map(|usage| {
        let mut tokens = usage_tokens(usage);
        if !windows.is_empty() {
            tokens.window = Some(JsNumber(windows.iter().copied().fold(f64::NEG_INFINITY, f64::max)));
        }
        tokens
    });
    ResultTokens { tokens, models: event.get("modelUsage").map(|raw| models_of(Some(raw))) }
}

/// What a session's file saved of its running totals before a turn. adapter.ts:292.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavedUse {
    pub cost_usd: f64,
    pub models: Vec<ModelUse>,
    /// `None` and `Some(vec![])` read the same.
    pub unpriced: Option<Vec<String>>,
}

/// adapter.ts:301.
pub fn saved_use_of(event: &Obj, session_id: &str) -> Option<SavedUse> {
    if string(event.get("sessionId")) != Some(session_id) || rec(event.get("modelUsage")).is_none() {
        return None;
    }
    let cost_usd = num(event.get("totalCostUSD"))?;
    Some(SavedUse { cost_usd, models: models_of(event.get("modelUsage")), unpriced: None })
}

fn spent_by(m: &ModelUse) -> f64 {
    m.tokens.input.0 + m.tokens.output.0
}

/// adapter.ts:311.
pub fn gained(now: &ModelUse, before: Option<&ModelUse>) -> Option<ModelUse> {
    let Some(before) = before else {
        return Some(now.clone());
    };
    let (n, b) = (&now.tokens, &before.tokens);
    let fell = [(n.input, b.input), (n.output, b.output), (n.cached, b.cached), (n.cache_write, b.cache_write), (n.reasoning, b.reasoning)]
        .iter()
        .any(|(now, was)| now < was);
    if fell {
        return None;
    }
    let tokens = UsageTokens {
        input: JsNumber(n.input.0 - b.input.0),
        output: JsNumber(n.output.0 - b.output.0),
        cached: JsNumber(n.cached.0 - b.cached.0),
        cache_write: JsNumber(n.cache_write.0 - b.cache_write.0),
        reasoning: JsNumber(n.reasoning.0 - b.reasoning.0),
    };
    let cost_usd = now.cost_usd.map(|c| JsNumber((c.0 - before.cost_usd.map_or(0.0, |w| w.0)).max(0.0)));
    Some(ModelUse { model: now.model.clone(), tokens, cost_usd })
}

/// adapter.ts:325. Key order of the answer: the result's own keys less costUsd and models, then tokens, costUsd,
/// models, model (each where present), a key already there keeping its place.
pub fn own_use(result: TurnResult, saved: Option<&SavedUse>, fresh: bool, spent: Option<&TurnTokens>) -> TurnResult {
    let total = result.cost_usd.map(|c| c.0);
    let running = result.models.clone();
    let window = result.tokens.as_ref().and_then(|t| t.window);
    let mut rest = TurnResult { cost_usd: None, models: None, ..result };
    if let Some(spent) = spent {
        let mut tokens = spent.clone();
        if window.is_some() {
            tokens.window = window;
        }
        rest.tokens = Some(tokens);
    }
    if !fresh && saved.is_none() {
        return rest;
    }
    let before = saved.cloned().unwrap_or_default();
    let restarted = total.is_some_and(|t| t < before.cost_usd);
    let paid = total.map(|t| if restarted { t } else { t - before.cost_usd });
    let counted: Option<Vec<ModelUse>> = running
        .as_ref()
        .and_then(|ms| {
            ms.iter()
                .map(|m| if restarted { Some(m.clone()) } else { gained(m, start_of(&before.models, &m.model)) })
                .collect::<Option<Vec<_>>>()
        })
        .map(|own| own.into_iter().filter(|m| spent_by(m) > 0.0).collect());
    let unpriced: Vec<String> = if restarted { Vec::new() } else { before.unpriced.iter().flatten().map(|m| base_model(m)).collect() };
    let withheld = |m: &ModelUse| unpriced.contains(&base_model(&m.model));
    let cost_sum = counted.iter().flatten().filter(|m| withheld(m)).fold(0.0, |sum, m| sum + m.cost_usd.map_or(0.0, |c| c.0));
    let models: Option<Vec<ModelUse>> =
        counted.map(|ms| ms.into_iter().map(|m| if withheld(&m) { ModelUse { cost_usd: None, ..m } } else { m }).collect());
    let priced_any = models.iter().flatten().any(|m| m.cost_usd.is_some());
    let cost_usd = match paid {
        Some(paid) if !unpriced.is_empty() => {
            if priced_any {
                Some((paid - cost_sum).max(0.0))
            } else {
                None
            }
        }
        other => other,
    };
    let model = models
        .iter()
        .flatten()
        .fold(None::<&ModelUse>, |best, m| match best {
            Some(b) if spent_by(m) <= spent_by(b) => Some(b),
            _ => Some(m),
        })
        .map(|m| m.model.clone());
    let mut split: Option<TurnTokens> = None;
    for m in models.iter().flatten() {
        split = Some(plus_tokens(split.as_ref(), &as_turn(&m.tokens)));
    }
    if let Some(mut tokens) = split {
        if window.is_some() {
            tokens.window = window;
        }
        rest.tokens = Some(tokens);
    }
    rest.cost_usd = cost_usd.map(JsNumber);
    rest.models = models;
    if model.is_some() {
        rest.model = model;
    }
    rest
}

/// adapter.ts:348.
pub fn normalize_result(event: &Obj, refusal: Option<&Refusal>) -> TurnResult {
    let errors = str_arr(event.get("errors")).unwrap_or_default();
    let tokens = result_tokens(event);
    let result = TurnResult {
        status: result_status(event, &errors.join(" ").to_lowercase()),
        duration_ms: num(event.get("duration_ms")).map(JsNumber),
        waited_ms: None,
        cost_usd: num(event.get("total_cost_usd")).map(JsNumber),
        tokens: tokens.tokens,
        model: None,
        models: tokens.models,
        text: string(event.get("result")).map(String::from),
        error: errors.iter().find(|e| !e.starts_with("[ede_diagnostic]")).cloned(),
        unreached: None,
        refusal: None,
        limit: None,
    };
    if event.get("is_error") == Some(&Value::Bool(true)) && result.status == TurnStatus::Failed {
        refused_turn(result, refusal)
    } else {
        result
    }
}

/// adapter.ts:365: `authentication_failed` is `sign-in`; nothing else is claimed.
pub fn refusal_cause(error: &str) -> Option<TurnRefusal> {
    (error == "authentication_failed").then_some(TurnRefusal::SignIn)
}

/// What an API error line the CLI wrote names. adapter.ts:370: `undefined` is NotOne, `null` Unclaimed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiErrorCause {
    NotOne,
    Unclaimed,
    Cause(TurnRefusal),
}

/// adapter.ts:370.
pub fn api_error_cause(event: &Obj) -> ApiErrorCause {
    if string(event.get("type")) != Some("assistant") || event.get("is_api_error_message") != Some(&Value::Bool(true)) {
        return ApiErrorCause::NotOne;
    }
    match refusal_cause(string(event.get("error")).unwrap_or("")) {
        Some(cause) => ApiErrorCause::Cause(cause),
        None => ApiErrorCause::Unclaimed,
    }
}

/// adapter.ts:377.
pub const STOP_TASK_UNSUPPORTED: &str = "stop_task is not supported in this context";
/// adapter.ts:380.
pub const NO_CONVERSATION: &str = "No conversation found with session ID:";

/// adapter.ts:384.
pub fn answered_nothing(result: &TurnResult) -> bool {
    if result.status != TurnStatus::Completed || !js_trim(result.text.as_deref().unwrap_or("")).is_empty() {
        return false;
    }
    let (input, output) = result.tokens.as_ref().map_or((0.0, 0.0), |t| (t.input.0, t.output.0));
    input + output == 0.0
}

/// adapter.ts:391.
pub fn with_context(result: TurnResult, context: Option<f64>, init_model: Option<&str>) -> TurnResult {
    let model = result.model.clone().or_else(|| init_model.map(String::from));
    let tokens = match (result.tokens.clone(), context) {
        (Some(tokens), Some(context)) => Some(TurnTokens { context: Some(JsNumber(context)), ..tokens }),
        (tokens, _) => tokens,
    };
    TurnResult { tokens, model, ..result }
}

/// What the model held before and after a compaction. adapter.ts:398.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Compacted {
    pub before: Option<f64>,
    pub after: f64,
}

/// adapter.ts:398.
pub fn compacted_to(event: &Obj) -> Option<Compacted> {
    if string(event.get("type")) != Some("system") || string(event.get("subtype")) != Some("compact_boundary") {
        return None;
    }
    let metadata = rec(event.get("compact_metadata"));
    let after = num(metadata.and_then(|m| m.get("post_tokens")))?;
    let before = num(metadata.and_then(|m| m.get("pre_tokens")));
    Some(Compacted { before, after })
}

/// adapter.ts:408. A `turn.retry`; `why` is always present.
pub fn retry_of(event: &Obj, session_id: &str) -> Option<AdapterEvent> {
    if string(event.get("type")) != Some("system") || string(event.get("subtype")) != Some("api_retry") {
        return None;
    }
    let attempt = num(event.get("attempt"))?;
    let why = anthropic_retry_why(string(event.get("error")), num(event.get("error_status")));
    Some(AdapterEvent::TurnRetry { session_id: session_id.into(), attempt, of: num(event.get("max_retries")), why: Some(why) })
}

/// adapter.ts:419.
pub fn subagent_retry_of(event: &Obj, session_id: &str) -> Option<AdapterEvent> {
    let retry = rec(event.get("subagent_retry"));
    let attempt = num(retry.and_then(|r| r.get("attempt")))?;
    let why = anthropic_retry_why(string(retry.and_then(|r| r.get("error_category"))), num(retry.and_then(|r| r.get("error_status"))));
    Some(AdapterEvent::TurnRetry {
        session_id: session_id.into(),
        attempt,
        of: num(retry.and_then(|r| r.get("max_retries"))),
        why: Some(why),
    })
}

/// adapter.ts:428.
pub const STDERR_TAIL_LINES: usize = 5;

/// adapter.ts:430.
pub fn no_output_error(result: &TurnResult, stderr_tail: &[String]) -> String {
    let head = format!("claude answered with no output and no usage after {}", fmt_duration(result.duration_ms.map_or(0.0, |d| d.0)));
    if stderr_tail.is_empty() {
        head
    } else {
        format!("{head}: {}", stderr_tail.join("\n"))
    }
}

/// adapter.ts:437.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubagentLaunch {
    pub agent_id: String,
    pub tool_use_id: String,
}

/// adapter.ts:437.
pub fn subagent_launch(event: &Obj) -> Option<SubagentLaunch> {
    if string(event.get("type")) != Some("system") || string(event.get("subtype")) != Some("task_started") {
        return None;
    }
    Some(SubagentLaunch { agent_id: string(event.get("task_id"))?.to_string(), tool_use_id: string(event.get("tool_use_id"))?.to_string() })
}

/// adapter.ts:445.
pub const SUBAGENT_TOOLS: [&str; 2] = ["Agent", "Task"];

/// adapter.ts:448. (call id, prompt) pairs in block order.
pub fn subagent_prompts(event: &Obj) -> Vec<(String, String)> {
    if string(event.get("type")) != Some("assistant") || string(event.get("parent_tool_use_id")).is_some() {
        return Vec::new();
    }
    let Some(blocks) = rec(event.get("message")).and_then(|m| m.get("content")).and_then(Value::as_array) else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter_map(|raw| {
            let block = rec(Some(raw));
            let id = string(block.and_then(|b| b.get("id")))?;
            let prompt = string(rec(block.and_then(|b| b.get("input"))).and_then(|input| input.get("prompt")))?;
            let is_subagent = string(block.and_then(|b| b.get("type"))) == Some("tool_use")
                && SUBAGENT_TOOLS.contains(&string(block.and_then(|b| b.get("name"))).unwrap_or(""));
            is_subagent.then(|| (id.to_string(), prompt.to_string()))
        })
        .collect()
}

/// adapter.ts:462.
#[derive(Clone, Debug, PartialEq)]
pub struct SubagentStarted {
    pub task: String,
    pub parent: Option<String>,
    pub title: Option<String>,
    pub depth: Option<f64>,
}

/// adapter.ts:462.
pub fn subagent_started(event: &Obj) -> Option<SubagentStarted> {
    if string(event.get("type")) != Some("system")
        || string(event.get("subtype")) != Some("task_started")
        || string(event.get("task_type")) != Some("local_agent")
    {
        return None;
    }
    Some(SubagentStarted {
        task: string(event.get("task_id"))?.to_string(),
        parent: string(event.get("tool_use_id")).map(String::from),
        title: string(event.get("description")).map(String::from),
        depth: num(event.get("spawn_depth")),
    })
}

/// One background task the CLI reports running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundTask {
    pub id: String,
    pub description: String,
}

/// adapter.ts:476.
pub fn background_tasks_of(event: &Obj) -> Option<Vec<BackgroundTask>> {
    if string(event.get("type")) != Some("system") || string(event.get("subtype")) != Some("background_tasks_changed") {
        return None;
    }
    let Some(tasks) = event.get("tasks").and_then(Value::as_array) else {
        return Some(Vec::new());
    };
    Some(
        tasks
            .iter()
            .filter_map(|raw| {
                let task = rec(Some(raw));
                let id = string(task.and_then(|t| t.get("task_id")))?;
                let description = string(task.and_then(|t| t.get("description"))).unwrap_or(id);
                Some(BackgroundTask { id: id.to_string(), description: description.to_string() })
            })
            .collect(),
    )
}

/// adapter.ts:487. (id, status).
pub fn task_finished_of(event: &Obj) -> Option<(String, String)> {
    if string(event.get("type")) != Some("system") || string(event.get("subtype")) != Some("task_notification") {
        return None;
    }
    Some((string(event.get("task_id"))?.to_string(), string(event.get("status"))?.to_string()))
}

/// adapter.ts:497.
pub fn drained_notice(event: &Obj) -> bool {
    if string(event.get("type")) != Some("result")
        || string(rec(event.get("origin")).and_then(|o| o.get("kind"))) != Some("task-notification")
        || num(event.get("num_turns")) != Some(0.0)
    {
        return false;
    }
    answered_nothing(&normalize_result(event, None))
}

/// adapter.ts:502. Mutates `plans` and `book` as the TS does.
pub fn normalize_event(
    event: &Obj,
    fallback_session_id: &str,
    refusal: Option<&Refusal>,
    plans: &mut PlanBook,
    book: &mut HandbackBook,
) -> Vec<AdapterEvent> {
    let session_id = string(event.get("session_id")).unwrap_or(fallback_session_id);
    let parent = string(event.get("parent_tool_use_id"));
    match string(event.get("type")) {
        Some("system") => {
            if let Some(retry) = retry_of(event, session_id) {
                return vec![retry];
            }
            if string(event.get("subtype")) != Some("init") {
                return Vec::new();
            }
            let mut out = vec![AdapterEvent::SessionStart {
                session_id: session_id.into(),
                model: string(event.get("model")).map(String::from),
                cwd: string(event.get("cwd")).map(String::from),
                tools: str_arr(event.get("tools")),
                harness: harness_of(event),
                prompted: None,
            }];
            if string(event.get("apiKeySource")).is_some_and(|source| source != "none") {
                out.push(AdapterEvent::Limit {
                    session_id: session_id.into(),
                    limit: HarnessLimit { windows: vec![], plan: None, status: None, account: None, keyed: Some(true), credits: None },
                });
            }
            out
        }
        Some("assistant") => {
            let message = rec(event.get("message"));
            let Some(blocks) = message.and_then(|m| m.get("content")).and_then(Value::as_array) else {
                return Vec::new();
            };
            let message_id = message.and_then(|m| string(m.get("id")));
            let mut deltas = Vec::new();
            for block in blocks.iter().filter_map(|raw| rec(Some(raw))) {
                match string(block.get("type")) {
                    Some("text") => deltas.push(AdapterEvent::TurnDelta(TurnDelta {
                        message_id: message_id.map(String::from),
                        parent_tool_use_id: parent.map(String::from),
                        ..TurnDelta::new(session_id, DeltaKind::Text, string(block.get("text")).unwrap_or(""))
                    })),
                    Some("thinking") => deltas.push(AdapterEvent::TurnDelta(TurnDelta {
                        parent_tool_use_id: parent.map(String::from),
                        ..TurnDelta::new(session_id, DeltaKind::Thinking, string(block.get("thinking")).unwrap_or(""))
                    })),
                    Some("tool_use") => {
                        let id = string(block.get("id"));
                        let plan = if parent.is_none() {
                            read_plan_call(string(block.get("name")), block.get("input"), id, plans)
                        } else {
                            PlanCall::NotPlan
                        };
                        if plan != PlanCall::NotPlan {
                            if let Some(id) = id {
                                plans.calls.insert(id.to_string());
                            }
                            match plan {
                                PlanCall::Steps(steps) => {
                                    deltas.push(AdapterEvent::TurnPlan { session_id: session_id.into(), steps: Some(steps), text: None })
                                }
                                PlanCall::Text(text) => {
                                    deltas.push(AdapterEvent::TurnPlan { session_id: session_id.into(), steps: None, text: Some(text) })
                                }
                                PlanCall::Nothing | PlanCall::NotPlan => {}
                            }
                            continue;
                        }
                        let input = block.get("input").cloned().unwrap_or(Value::Null);
                        let call = TurnDelta {
                            tool_name: string(block.get("name")).map(String::from),
                            tool_use_id: id.map(String::from),
                            parent_tool_use_id: parent.map(String::from),
                            ..TurnDelta::new(session_id, DeltaKind::ToolUse, json_stringify(&input))
                        };
                        if !held_call(book, &call) {
                            deltas.push(AdapterEvent::TurnDelta(call));
                        }
                    }
                    _ => {}
                }
            }
            deltas
        }
        Some("user") => {
            let Some(blocks) = rec(event.get("message")).and_then(|m| m.get("content")).and_then(Value::as_array) else {
                return Vec::new();
            };
            let mut deltas = Vec::new();
            for block in blocks.iter().filter_map(|raw| rec(Some(raw))) {
                if string(block.get("type")) != Some("tool_result") {
                    continue;
                }
                let answered = string(block.get("tool_use_id"));
                if let Some(id) = answered {
                    if plans.calls.shift_remove(id) {
                        let at = plans.unnamed.get(id).copied();
                        let named = NAMED_TASK.captures(&flatten_content(block.get("content"))).map(|c| c[1].to_string());
                        if let (Some(at), Some(named)) = (at, named) {
                            if let Some(task) = plans.tasks.get_mut(at) {
                                task.id = Some(named);
                            }
                        }
                        plans.unnamed.shift_remove(id);
                        continue;
                    }
                }
                let text = flatten_content(block.get("content"));
                let is_error = block.get("is_error") == Some(&Value::Bool(true));
                let facts = result_facts(blocks, &text, is_error, event.get("tool_use_result"));
                let answer = TurnDelta {
                    tool_use_id: answered.map(String::from),
                    is_error: Some(is_error),
                    parent_tool_use_id: parent.map(String::from),
                    bytes: facts.bytes,
                    exit_code: facts.exit_code,
                    patch: facts.patch,
                    ..TurnDelta::new(session_id, DeltaKind::ToolResult, facts.text.unwrap_or(text))
                };
                deltas.extend(read_answer(book, answer).into_iter().map(AdapterEvent::TurnDelta));
            }
            deltas
        }
        Some("result") => {
            vec![AdapterEvent::TurnDone { session_id: session_id.into(), result: normalize_result(event, refusal), held: None }]
        }
        Some("rate_limit_event") => match limit_of(rec(event.get("rate_limit_info"))) {
            Some(limit) => vec![AdapterEvent::Limit { session_id: session_id.into(), limit }],
            None => Vec::new(),
        },
        Some("tool_progress") => subagent_retry_of(event, session_id).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// adapter.ts:625.
pub fn session_file_in(config_dir: &str, session_id: &str) -> SessionFile {
    SessionFile { folder: format!("{config_dir}/projects"), name: format!("{session_id}.jsonl"), depth: 1 }
}
