// SPDX-License-Identifier: AGPL-3.0-only
//! What each of a turn's subagents spent. TS: packages/adapter-claude/src/subagent-spend.ts at a3b870a6e.

use indexmap::IndexMap;

use crate::claude::fields::Obj;
use crate::port::SubagentUsage;
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

/// subagent-spend.ts:26.
pub fn note_child_call(book: &mut SpendBook, task: &str, event: &Obj) {
    todo!()
}

/// subagent-spend.ts:43. `is_subagent` answers whether a task id is one of this turn's subagents.
pub fn note_progress(book: &mut SpendBook, event: &Obj, is_subagent: &dyn Fn(&str) -> bool) -> Option<String> {
    todo!()
}

/// subagent-spend.ts:56. Keys in the TS order: tokens, then toolUses, then durationMs.
pub fn subagent_spent(book: &SpendBook, task: &str) -> Option<SubagentUsage> {
    todo!()
}

/// subagent-spend.ts:65.
pub fn subagent_spent_at_end(book: &SpendBook, task: &str, end: &Obj) -> Option<SubagentUsage> {
    todo!()
}
