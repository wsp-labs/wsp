// SPDX-License-Identifier: AGPL-3.0-only
//! How a subagent's end reaches the fold on Claude Code 2.1.295. TS: packages/adapter-claude/src/handback.ts at
//! a3b870a6e.

use indexmap::{IndexMap, IndexSet};

use crate::claude::fields::Obj;
use crate::port::{SubagentState, TurnDelta};

/// handback.ts:21.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HandbackBook {
    pub subagents: IndexSet<String>,
    /// Each task a subagent started, until its notification, with that subagent's handle.
    pub owned: IndexMap<String, String>,
    pub notified: IndexSet<String>,
    /// The launching calls whose subagents run in the background.
    pub backgrounded: IndexSet<String>,
    /// Each hand-back call until its result.
    pub held: IndexMap<String, TurnDelta>,
    /// The launching calls a notification answered, with that answer.
    pub answered: IndexMap<String, String>,
    /// The last words each subagent wrote, by its launching call.
    pub last_text: IndexMap<String, String>,
}

/// handback.ts:42.
pub fn note_line(book: &mut HandbackBook, event: &Obj) {
    todo!()
}

/// handback.ts:60. Holds a SubagentHandback call made inside a subagent; false for any other call.
pub fn held_call(book: &mut HandbackBook, call: &TurnDelta) -> bool {
    todo!()
}

/// handback.ts:88. The deltas one tool_result draws, in order.
pub fn read_answer(book: &mut HandbackBook, answer: TurnDelta) -> Vec<TurnDelta> {
    todo!()
}

/// handback.ts:114.
pub fn interim_end(book: &HandbackBook, event: &Obj) -> bool {
    todo!()
}

/// handback.ts:121.
pub fn later_end(book: &HandbackBook, event: &Obj) -> bool {
    todo!()
}

/// A task the CLI says is over.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskEnd {
    pub task: String,
    pub state: SubagentState,
    /// Never `Some("")`.
    pub summary: Option<String>,
}

/// handback.ts:137. completed→done, failed→failed, stopped and killed→stopped, any other word→failed.
pub fn task_ended(event: &Obj) -> Option<TaskEnd> {
    todo!()
}

/// handback.ts:159.
pub fn end_answer(book: &mut HandbackBook, event: &Obj, session_id: &str) -> Vec<TurnDelta> {
    todo!()
}
