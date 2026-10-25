// SPDX-License-Identifier: AGPL-3.0-only
//! A turn's subagents: more threads on the same server, which its lead spawned. TS adapter-codex/src/adapter.ts:445-461
//! and 664-841 (`learn`, `endChild`, `childSigns`, `sayChild`, `sayTasks`, `onChild`). Spec: spec/codex_turn.md rule 12.
//! Each method pushes the events it says onto `out`, in order, and emits nothing itself (rule 3.3).

use indexmap::IndexMap;
use serde_json::{Map, Value};

use super::items::ChildTokens;
use crate::port::{AdapterEvent, SubagentState, SubagentUsage};

/// TS adapter.ts:446.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub state: SubagentState,
    /// The turn it has in progress, which a stop names.
    pub turn_id: Option<String>,
    /// The call that spawned it; its own thread id until one is seen.
    pub parent: String,
    pub title: Option<String>,
    pub prompt: Option<String>,
    pub model: Option<String>,
    pub depth: Option<u32>,
    pub summary: Option<String>,
    pub usage: Option<SubagentUsage>,
}

/// A sign of a subagent and what it says of it. TS adapter.ts:691's `said`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Said {
    pub parent: Option<String>,
    pub prompt: Option<String>,
    pub model: Option<String>,
    pub name: Option<String>,
    pub depth: Option<u32>,
}

/// The turn's subagents by thread id, in the order first seen, the tokens their calls drew, and the task count last said.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Children {
    pub children: IndexMap<String, Child>,
    pub child_tokens: ChildTokens,
    pub tasks_said: u32,
}

impl Children {
    /// Running children, a turn id or not. TS adapter.ts:664.
    pub fn running(&self) -> usize {
        todo!()
    }

    /// Each running child with a turn in progress, as `(thread id, turn id)`, in order. TS adapter.ts:1198.
    pub fn running_turns(&self) -> Vec<(String, String)> {
        todo!()
    }

    /// `turn.tasks` where the count of running children with a turn changed. TS adapter.ts:667.
    pub fn say_tasks(&mut self, lead: &str, out: &mut Vec<AdapterEvent>) {
        todo!()
    }

    /// The `subagent` event for one child as it stands. TS adapter.ts:673.
    pub fn say_child(&self, lead: &str, task: &str, out: &mut Vec<AdapterEvent>) {
        todo!()
    }

    /// TS adapter.ts:691. A task equal to the lead's thread id is ignored.
    pub fn learn(&mut self, lead: &str, task: &str, said: Said, out: &mut Vec<AdapterEvent>) {
        todo!()
    }

    /// TS adapter.ts:713. True where the child was running and has now ended, so the caller settles the lead.
    pub fn end_child(&mut self, lead: &str, task: &str, state: SubagentState, out: &mut Vec<AdapterEvent>) -> bool {
        todo!()
    }

    /// The subagents a collab call or an activity item names. TS adapter.ts:723. True where any child ended.
    pub fn child_signs(&mut self, lead: &str, item: &Map<String, Value>, done: bool, out: &mut Vec<AdapterEvent>) -> bool {
        todo!()
    }

    /// Everything else a subagent's thread prints on the lead's stdout: its turn, its tokens, its summary and the signs
    /// in its items. The caller has already counted its commands and kept its changes. TS adapter.ts:791. True where a
    /// child ended.
    pub fn on_child(&mut self, lead: &str, task: &str, method: &str, params: &Map<String, Value>, emitted_at_ms: Option<f64>, out: &mut Vec<AdapterEvent>) -> bool {
        todo!()
    }
}
