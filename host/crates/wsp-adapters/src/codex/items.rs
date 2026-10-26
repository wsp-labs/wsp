// SPDX-License-Identifier: AGPL-3.0-only
//! The pure readers above `createCodexAdapter`: an item as timeline deltas, a file change's hunks, a turn's tokens, a
//! failure in wsp's words, and the constants the turn reader keys on. TS adapter-codex/src/adapter.ts:251-511. Spec:
//! spec/codex_items.md.

use serde_json::{json, Map, Value};

use super::fields::{count, rec, str};
use super::rpc::{thread_compact_start_line, turn_start_line};
use super::shell_script::shell_script_of;
use crate::pending::{codex_key_refused_line, codex_missing_env_line, codex_not_signed_in_line, unified_hunks, whole_file_hunk};
use crate::port::{AdapterEvent, FilePatch, PlanStep, SubagentState, SubagentUsage, TurnDelta, TurnTokens};
use wsp_wire::text::json_stringify;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::UsageTokens;
use wsp_wire::types::wsp_tools::{MCP_SERVER_NAME, SLATE_SERVER_NAME};
use wsp_wire::views::session::TurnRefusal;
use wsp_wire::views::session_events::{DeltaKind, PlanStepState};
use wsp_words::js::{is_js_whitespace, js_trim, number_string};
use wsp_words::v8_json::parse_json_lax;

/// TS adapter.ts:253.
pub const COMPACT: &str = "/compact";
/// TS adapter.ts:256.
pub const STDERR_TAIL_LINES: usize = 5;
/// TS adapter.ts:263.
pub const RECONNECT_STALL_MS: u64 = 90_000;

/// What a side question's fork is told. TS adapter.ts:484.
pub const ASIDE_INSTRUCTIONS: &str = "The person is asking a side question about this conversation while its work goes on elsewhere. Answer it from the conversation so far, briefly. Run nothing, edit nothing and call no tool.";

/// The two approvals the server raises, with the tool name each is asked under. TS adapter.ts:488.
pub const APPROVALS: [(&str, &str); 2] =
    [("item/commandExecution/requestApproval", "command_execution"), ("item/fileChange/requestApproval", "file_change")];

/// The breakdown fields, by TurnTokens' name and the server's key. TS adapter.ts:347.
pub const BREAKDOWN: [(&str, &str); 5] = [
    ("input", "inputTokens"),
    ("cached", "cachedInputTokens"),
    ("cacheWrite", "cacheWriteInputTokens"),
    ("output", "outputTokens"),
    ("reasoning", "reasoningOutputTokens"),
];

/// A failure in wsp's words, with the cause wsp classes it as where it claims one. TS adapter.ts:288.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureWords {
    pub line: String,
    pub cause: Option<TurnRefusal>,
}

/// Whether a message starts `Reconnecting...`. TS adapter.ts:262.
pub fn is_reconnecting(message: &str) -> bool {
    message.starts_with("Reconnecting...")
}

/// TS adapter.ts:267.
pub fn retry_of(message: &str, info: Option<&Value>, session_id: &str) -> Option<AdapterEvent> {
    let (attempt, of) = retry_counts(message)?;
    let status =
        rec(info).and_then(|i| i.get("responseStreamDisconnected")).and_then(|r| rec(Some(r))).and_then(|r| count(r.get("httpStatusCode")));
    let why = match status {
        Some(code) => format!("the provider answered {}", number_string(code)),
        None => "the connection dropped".to_string(),
    };
    Some(AdapterEvent::TurnRetry { session_id: session_id.to_string(), attempt, of: Some(of), why: Some(why) })
}

/// The `n` and `m` of `Reconnecting... <n>/<m>`, as a prefix match. TS adapter.ts:264.
fn retry_counts(message: &str) -> Option<(f64, f64)> {
    let rest = message.strip_prefix("Reconnecting...")?.strip_prefix(' ')?;
    let (attempt, rest) = leading_digits(rest)?;
    let (of, _) = leading_digits(rest.strip_prefix('/')?)?;
    Some((attempt.parse().ok()?, of.parse().ok()?))
}

fn leading_digits(text: &str) -> Option<(&str, &str)> {
    let end = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
    (end > 0).then(|| text.split_at(end))
}

/// TS adapter.ts:277.
pub fn refused_because(message: &str) -> String {
    const STATUS: &str = "401 Unauthorized";
    let Some(at) = message.find(STATUS) else { return String::new() };
    let tail = message[at + STATUS.len()..].split('\n').next().unwrap_or("");
    let tail = tail.trim_start_matches(|c: char| c == ':' || is_js_whitespace(c));
    let tail = tail.split(", url:").next().unwrap_or("");
    let tail = tail.split(')').next().unwrap_or("");
    js_trim(tail).to_string()
}

/// The name in the first `Missing environment variable` message that names one. TS adapter.ts:275.
fn missing_name(message: &str) -> Option<&str> {
    const PREFIX: &str = "Missing environment variable: `";
    message.match_indices(PREFIX).find_map(|(at, _)| {
        let rest = &message[at + PREFIX.len()..];
        if rest.starts_with('`') {
            return None;
        }
        rest.find('`').map(|end| &rest[..end])
    })
}

/// TS adapter.ts:288.
pub fn failure_words(message: &str, login: &str, key_env: Option<&str>) -> Option<FailureWords> {
    if message.contains("401 Unauthorized") {
        let line = match key_env {
            None => codex_not_signed_in_line(login),
            Some(env) => codex_key_refused_line(env, &refused_because(message), login),
        };
        return Some(FailureWords { line, cause: Some(TurnRefusal::SignIn) });
    }
    missing_name(message).map(|name| FailureWords { line: codex_missing_env_line(name), cause: None })
}

/// An item with a string id and type. TS adapter.ts:297.
pub fn item_of(params: &Map<String, Value>) -> Option<Map<String, Value>> {
    let item = rec(params.get("item"))?;
    str(item.get("id"))?;
    str(item.get("type"))?;
    Some(item.clone())
}

/// One change as `{path, kind}`. TS adapter.ts:305.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub kind: String,
}

/// TS adapter.ts:305.
pub fn changes_of(changes: Option<&Value>) -> Vec<Change> {
    let Some(rows) = changes.and_then(Value::as_array) else { return Vec::new() };
    rows.iter()
        .filter_map(Value::as_object)
        .map(|c| Change {
            path: str(c.get("path")).unwrap_or("").to_string(),
            kind: str(rec(c.get("kind")).and_then(|k| k.get("type"))).or(str(c.get("kind"))).unwrap_or("change").to_string(),
        })
        .collect()
}

/// Where a diff ends in `\n\nMoved to: <path>`: the index the diff is cut at, and the path. TS adapter.ts:317.
fn moved_to_of(diff: &str) -> Option<(usize, &str)> {
    let k = diff.rfind('\n')?;
    let path = diff[k + 1..].strip_prefix("Moved to: ")?;
    if path.is_empty() || !diff[..=k].ends_with("\n\n") {
        return None;
    }
    Some((k - 1, path))
}

/// `None` where no change made a hunk. TS adapter.ts:319.
pub fn file_patches(changes: Option<&Value>) -> Option<Vec<FilePatch>> {
    let mut patches = Vec::new();
    for change in changes.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object) {
        let Some(path) = str(change.get("path")).filter(|p| !p.is_empty()) else { continue };
        let diff = str(change.get("diff")).unwrap_or("");
        let kind = str(rec(change.get("kind")).and_then(|k| k.get("type"))).or(str(change.get("kind")));
        if matches!(kind, Some("add") | Some("delete")) {
            if !diff.is_empty() {
                let sign = if kind == Some("add") { '+' } else { '-' };
                patches.push(FilePatch { path: path.to_string(), hunks: vec![whole_file_hunk(diff, sign)], moved_to: None });
            }
            continue;
        }
        let moved = moved_to_of(diff);
        let hunks = unified_hunks(moved.map_or(diff, |(at, _)| &diff[..at]));
        if hunks.is_empty() && moved.is_none() {
            continue;
        }
        patches.push(FilePatch { path: path.to_string(), hunks, moved_to: moved.map(|(_, to)| to.to_string()) });
    }
    (!patches.is_empty()).then_some(patches)
}

/// TS adapter.ts:334.
pub fn change_lines(changes: Option<&Value>) -> String {
    changes_of(changes).iter().map(|c| js_trim(&format!("{} {}", c.kind, c.path)).to_string()).collect::<Vec<_>>().join("\n")
}

/// Where the thread's running total stood as the turn began and the server's latest report. TS adapter.ts:357.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageSeen {
    pub before: Map<String, Value>,
    pub total: Map<String, Value>,
    pub last: Map<String, Value>,
    pub window: Option<f64>,
}

/// TS adapter.ts:366.
pub fn turn_tokens_of(seen: &UsageSeen) -> TurnTokens {
    let field = |key: &str| match (count(seen.total.get(key)), count(seen.before.get(key))) {
        (Some(total), Some(before)) => Some(total - before),
        _ => count(seen.last.get(key)),
    };
    let named = |name: &str| BREAKDOWN.iter().find(|(n, _)| *n == name).and_then(|(_, key)| field(key)).map(JsNumber);
    TurnTokens {
        input: named("input").unwrap_or(JsNumber(0.0)),
        output: named("output").unwrap_or(JsNumber(0.0)),
        cached: named("cached"),
        cache_write: named("cacheWrite"),
        reasoning: named("reasoning"),
        context: Some(held_of(&seen.last).into()),
        window: seen.window.map(JsNumber),
    }
}

/// TS adapter.ts:378.
pub fn held_of(last: &Map<String, Value>) -> f64 {
    count(last.get("totalTokens"))
        .unwrap_or_else(|| count(last.get("inputTokens")).unwrap_or(0.0) + count(last.get("outputTokens")).unwrap_or(0.0))
}

/// TS adapter.ts:381.
pub fn total_before(total: &Map<String, Value>, last: &Map<String, Value>) -> Map<String, Value> {
    BREAKDOWN
        .iter()
        .filter_map(|&(_, key)| count(total.get(key)).map(|t| (key.to_string(), Value::from(t - count(last.get(key)).unwrap_or(0.0)))))
        .collect()
}

/// TS adapter.ts:384.
pub fn step_state(status: Option<&Value>) -> PlanStepState {
    match status.and_then(Value::as_str) {
        Some("completed") => PlanStepState::Done,
        Some("inProgress") => PlanStepState::Working,
        _ => PlanStepState::Pending,
    }
}

/// One plan step off `turn/plan/updated`. TS adapter.ts:1018.
pub fn plan_steps(plan: Option<&Value>) -> Vec<PlanStep> {
    plan.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(|p| PlanStep { text: str(p.get("step")).unwrap_or("").to_string(), state: step_state(p.get("status")) })
        .collect()
}

/// One call as a thread's earlier messages show it: `(tool, text)`. TS adapter.ts:387.
pub fn tool_call_of(item: &Map<String, Value>) -> Option<(String, String)> {
    str(item.get("id"))?;
    str(item.get("type"))?;
    [false, true].into_iter().flat_map(|done| item_deltas(done, item, "")).find_map(|event| match event {
        AdapterEvent::TurnDelta(d) if d.kind == DeltaKind::ToolUse => d.tool_name.map(|name| (name, d.text)),
        _ => None,
    })
}

/// The strings of an array field, the others skipped. TS adapter.ts:407.
fn string_list(value: Option<&Value>) -> Vec<&str> {
    value.and_then(Value::as_array).map(|rows| rows.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

/// Each item as the deltas a timeline draws. TS adapter.ts:397.
pub fn item_deltas(done: bool, item: &Map<String, Value>, session_id: &str) -> Vec<AdapterEvent> {
    let id = str(item.get("id")).unwrap_or("").to_string();
    let failed = str(item.get("status")) != Some("completed");
    let field = |key: &str| str(item.get(key)).unwrap_or("").to_string();
    let delta = |kind: DeltaKind, text: String| TurnDelta::new(session_id, kind, text);
    let deltas = match str(item.get("type")) {
        Some("agentMessage") => {
            if done {
                vec![TurnDelta { message_id: Some(id.clone()), ..delta(DeltaKind::Text, field("text")) }]
            } else {
                Vec::new()
            }
        }
        Some("reasoning") => {
            let summary = string_list(item.get("summary"));
            let text = (if summary.is_empty() { string_list(item.get("content")) } else { summary }).join("\n");
            if done && !text.is_empty() {
                vec![delta(DeltaKind::Thinking, text)]
            } else {
                Vec::new()
            }
        }
        Some("commandExecution") => {
            if done {
                vec![TurnDelta {
                    tool_use_id: Some(id.clone()),
                    is_error: Some(failed),
                    exit_code: count(item.get("exitCode")).map(|n| n as i64),
                    duration_ms: count(item.get("durationMs")),
                    ..delta(DeltaKind::ToolResult, field("aggregatedOutput"))
                }]
            } else {
                let command = json!({"command": shell_script_of(&field("command"))});
                vec![TurnDelta {
                    tool_name: Some("command_execution".into()),
                    tool_use_id: Some(id.clone()),
                    ..delta(DeltaKind::ToolUse, json_stringify(&command))
                }]
            }
        }
        Some("fileChange") => {
            if done {
                vec![TurnDelta {
                    tool_use_id: Some(id.clone()),
                    is_error: Some(failed),
                    patch: if failed { None } else { file_patches(item.get("changes")) },
                    ..delta(DeltaKind::ToolResult, change_lines(item.get("changes")))
                }]
            } else {
                let changes: Vec<Value> =
                    changes_of(item.get("changes")).into_iter().map(|c| json!({"path": c.path, "kind": c.kind})).collect();
                vec![TurnDelta {
                    tool_name: Some("file_change".into()),
                    tool_use_id: Some(id.clone()),
                    ..delta(DeltaKind::ToolUse, json_stringify(&json!({"changes": changes})))
                }]
            }
        }
        Some("mcpToolCall") => {
            let server = match str(item.get("server")) {
                Some(name) if name == SLATE_SERVER_NAME => MCP_SERVER_NAME,
                Some(name) => name,
                None => "mcp",
            };
            let tool_name = format!("mcp__{server}__{}", str(item.get("tool")).unwrap_or("tool"));
            if done {
                let said = rec(item.get("error")).and_then(|e| str(e.get("message")));
                let content = rec(item.get("result")).and_then(|r| r.get("content"));
                let text = if failed && said.is_some_and(|s| !s.is_empty()) {
                    said.unwrap_or("").to_string()
                } else if failed && content.is_none() {
                    String::new()
                } else {
                    let listed = match content {
                        Some(content) if !content.is_null() => content.clone(),
                        _ => json!([]),
                    };
                    json_stringify(&listed)
                };
                vec![TurnDelta { tool_use_id: Some(id.clone()), is_error: Some(failed), ..delta(DeltaKind::ToolResult, text) }]
            } else {
                let arguments = match item.get("arguments") {
                    None | Some(Value::Null) => json!({}),
                    Some(arguments) => arguments.clone(),
                };
                vec![TurnDelta {
                    tool_name: Some(tool_name),
                    tool_use_id: Some(id.clone()),
                    ..delta(DeltaKind::ToolUse, json_stringify(&arguments))
                }]
            }
        }
        Some("collabAgentToolCall") => {
            if str(item.get("tool")) != Some("spawnAgent") {
                Vec::new()
            } else if done {
                vec![TurnDelta { tool_use_id: Some(id.clone()), is_error: Some(failed), ..delta(DeltaKind::ToolResult, String::new()) }]
            } else {
                vec![TurnDelta {
                    tool_name: Some("spawn_agent".into()),
                    tool_use_id: Some(id.clone()),
                    ..delta(DeltaKind::ToolUse, json_stringify(&json!({"prompt": field("prompt")})))
                }]
            }
        }
        Some("webSearch") => {
            if done {
                vec![
                    TurnDelta {
                        tool_name: Some("web_search".into()),
                        tool_use_id: Some(id.clone()),
                        ..delta(DeltaKind::ToolUse, json_stringify(&json!({"query": field("query")})))
                    },
                    TurnDelta { tool_use_id: Some(id.clone()), is_error: Some(false), ..delta(DeltaKind::ToolResult, field("query")) },
                ]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    };
    deltas.into_iter().map(AdapterEvent::TurnDelta).collect()
}

/// TS adapter.ts:464.
pub fn child_end(status: Option<&str>) -> SubagentState {
    match status {
        Some("completed") => SubagentState::Done,
        Some("interrupted") => SubagentState::Stopped,
        _ => SubagentState::Failed,
    }
}

/// TS adapter.ts:467.
pub fn depth_of(path: &str) -> u32 {
    let segments = path.split('/').filter(|s| !s.is_empty()).count() as u32;
    segments.saturating_sub(1).max(1)
}

/// TS adapter.ts:470.
pub fn child_usage_of(total: Option<&Map<String, Value>>) -> Option<SubagentUsage> {
    let total = total?;
    if BREAKDOWN.iter().all(|(_, key)| count(total.get(*key)).is_none()) {
        return None;
    }
    let value = |key: &str| JsNumber(count(total.get(key)).unwrap_or(0.0));
    Some(SubagentUsage {
        tokens: Some(UsageTokens {
            input: value("inputTokens"),
            output: value("outputTokens"),
            cached: value("cachedInputTokens"),
            cache_write: value("cacheWriteInputTokens"),
            reasoning: value("reasoningOutputTokens"),
        }),
        tool_uses: None,
        duration_ms: None,
    })
}

/// The child tokens a turn summed, by TurnTokens' field name, in the order first added.
pub type ChildTokens = indexmap::IndexMap<String, f64>;

/// TS adapter.ts:476.
pub fn with_child_tokens(lead: Option<TurnTokens>, children: &ChildTokens) -> Option<TurnTokens> {
    if children.is_empty() {
        return lead;
    }
    let mut sum = lead.unwrap_or(TurnTokens {
        input: JsNumber(0.0),
        output: JsNumber(0.0),
        cached: None,
        cache_write: None,
        reasoning: None,
        context: Some(JsNumber(0.0)),
        window: None,
    });
    if let Some(add) = children.get("input") {
        sum.input = JsNumber(sum.input.0 + add);
    }
    if let Some(add) = children.get("output") {
        sum.output = JsNumber(sum.output.0 + add);
    }
    for (name, slot) in [("cached", &mut sum.cached), ("cacheWrite", &mut sum.cache_write), ("reasoning", &mut sum.reasoning)] {
        if let Some(add) = children.get(name) {
            *slot = Some(JsNumber(slot.as_ref().map_or(0.0, |x| x.0) + add));
        }
    }
    Some(sum)
}

/// TS adapter.ts:495.
pub fn turn_line_for(thread_id: &str, prompt: &str, images: &[String], effort: Option<&str>) -> String {
    if js_trim(prompt) == COMPACT {
        thread_compact_start_line(thread_id)
    } else {
        turn_start_line(thread_id, prompt, images, effort)
    }
}

/// TS adapter.ts:501.
pub fn is_turn_start(line: &str) -> bool {
    parse_json_lax(line)
        .as_ref()
        .and_then(|v| v.get("method"))
        .and_then(Value::as_str)
        .is_some_and(|method| method == "turn/start" || method == "thread/compact/start")
}

/// A thread's rollout: `<home>/sessions/YYYY/MM/DD/rollout-<stamp>-<thread id>.jsonl`. TS adapter.ts:511.
pub fn rollout_in(home: &str, thread_id: &str) -> crate::port::SessionFile {
    crate::port::SessionFile { folder: format!("{home}/sessions"), name: format!("-{thread_id}.jsonl"), depth: 3 }
}
