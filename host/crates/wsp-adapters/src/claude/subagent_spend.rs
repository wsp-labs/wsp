// SPDX-License-Identifier: AGPL-3.0-only
//! What each of a turn's subagents spent. TS: packages/adapter-claude/src/subagent-spend.ts at a3b870a6e.

use indexmap::IndexMap;

use crate::claude::fields::{num, rec, string, Obj};
use crate::claude::tokens::{usage_of, usage_tokens};
use crate::port::SubagentUsage;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::usage::UsageTokens;

/// The tool calls and run time a subagent's latest progress or end line named.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Said {
    pub tool_uses: Option<f64>,
    pub duration_ms: Option<f64>,
}

/// subagent-spend.ts:12.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpendBook {
    /// Each subagent's calls by message id, each held at the largest output any of its lines showed.
    pub calls: IndexMap<String, IndexMap<String, UsageTokens>>,
    /// Each subagent's calls summed, moved by each line.
    pub totals: IndexMap<String, UsageTokens>,
    pub said: IndexMap<String, Said>,
}

fn zero() -> UsageTokens {
    let n = JsNumber(0.0);
    UsageTokens { input: n, output: n, cached: n, cache_write: n, reasoning: n }
}

/// subagent-spend.ts:26.
pub fn note_child_call(book: &mut SpendBook, task: &str, event: &Obj) {
    let message = rec(event.get("message"));
    let id = string(message.and_then(|m| m.get("id")));
    let usage = rec(message.and_then(|m| m.get("usage")));
    let (Some(id), Some(usage)) = (id, usage) else {
        return;
    };
    let held = book.calls.get(task).and_then(|calls| calls.get(id)).cloned().unwrap_or_else(zero);
    let tokens = usage_of(&usage_tokens(usage));
    let call = UsageTokens { output: JsNumber(f64::max(held.output.0, tokens.output.0)), ..tokens };
    let total = book.totals.get(task).cloned().unwrap_or_else(zero);
    let moved = |t: JsNumber, h: JsNumber, c: JsNumber| JsNumber(t.0 - h.0 + c.0);
    let next = UsageTokens {
        input: moved(total.input, held.input, call.input),
        output: moved(total.output, held.output, call.output),
        cached: moved(total.cached, held.cached, call.cached),
        cache_write: moved(total.cache_write, held.cache_write, call.cache_write),
        reasoning: moved(total.reasoning, held.reasoning, call.reasoning),
    };
    book.totals.insert(task.to_string(), next);
    book.calls.entry(task.to_string()).or_default().insert(id.to_string(), call);
}

/// subagent-spend.ts:43. `is_subagent` answers whether a task id is one of this turn's subagents.
pub fn note_progress(book: &mut SpendBook, event: &Obj, is_subagent: &dyn Fn(&str) -> bool) -> Option<String> {
    let subtype = string(event.get("subtype"));
    if string(event.get("type")) != Some("system") || !matches!(subtype, Some("task_progress" | "task_notification")) {
        return None;
    }
    let task = string(event.get("task_id"))?;
    let usage = rec(event.get("usage"))?;
    if !is_subagent(task) {
        return None;
    }
    let said = Said { tool_uses: num(usage.get("tool_uses")), duration_ms: num(usage.get("duration_ms")) };
    book.said.insert(task.to_string(), said);
    Some(task.to_string())
}

/// subagent-spend.ts:56. Keys in the TS order: tokens, then toolUses, then durationMs.
pub fn subagent_spent(book: &SpendBook, task: &str) -> Option<SubagentUsage> {
    let tokens = book.totals.get(task);
    let said = book.said.get(task);
    if tokens.is_none() && said.is_none() {
        return None;
    }
    Some(SubagentUsage {
        tokens: tokens.cloned(),
        tool_uses: said.and_then(|s| s.tool_uses).map(|n| JsInt(JsNumber(n))),
        duration_ms: said.and_then(|s| s.duration_ms).map(JsNumber),
    })
}

/// subagent-spend.ts:65.
pub fn subagent_spent_at_end(book: &SpendBook, task: &str, end: &Obj) -> Option<SubagentUsage> {
    let mut spent = subagent_spent(book, task)?;
    spent.duration_ms = rec(end.get("usage")).and_then(|u| num(u.get("duration_ms"))).map(JsNumber);
    Some(spent)
}
