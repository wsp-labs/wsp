// SPDX-License-Identifier: AGPL-3.0-only
//! The pure readers above `createCodexAdapter`: an item as timeline deltas, a file change's hunks, a turn's tokens, a
//! failure in wsp's words, and the constants the turn reader keys on. TS adapter-codex/src/adapter.ts:251-511. Spec:
//! spec/codex_items.md.

use serde_json::{Map, Value};

use crate::port::{AdapterEvent, FilePatch, PlanStep, SubagentState, SubagentUsage, TurnTokens};
use wsp_wire::views::session::TurnRefusal;
use wsp_wire::views::session_events::PlanStepState;

/// TS adapter.ts:253.
pub const COMPACT: &str = "/compact";
/// TS adapter.ts:256.
pub const STDERR_TAIL_LINES: usize = 5;
/// TS adapter.ts:263.
pub const RECONNECT_STALL_MS: u64 = 90_000;

/// What a side question's fork is told. TS adapter.ts:484.
pub const ASIDE_INSTRUCTIONS: &str = "The person is asking a side question about this conversation while its work goes on elsewhere. Answer it from the conversation so far, briefly. Run nothing, edit nothing and call no tool.";

/// The two approvals the server raises, with the tool name each is asked under. TS adapter.ts:488.
pub const APPROVALS: [(&str, &str); 2] = [("item/commandExecution/requestApproval", "command_execution"), ("item/fileChange/requestApproval", "file_change")];

/// The breakdown fields, by TurnTokens' name and the server's key. TS adapter.ts:347.
pub const BREAKDOWN: [(&str, &str); 5] = [("input", "inputTokens"), ("cached", "cachedInputTokens"), ("cacheWrite", "cacheWriteInputTokens"), ("output", "outputTokens"), ("reasoning", "reasoningOutputTokens")];

/// A failure in wsp's words, with the cause wsp classes it as where it claims one. TS adapter.ts:288.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureWords {
    pub line: String,
    pub cause: Option<TurnRefusal>,
}

/// Whether a message starts `Reconnecting...`. TS adapter.ts:262.
pub fn is_reconnecting(message: &str) -> bool {
    todo!()
}

/// TS adapter.ts:267.
pub fn retry_of(message: &str, info: Option<&Value>, session_id: &str) -> Option<AdapterEvent> {
    todo!()
}

/// TS adapter.ts:277.
pub fn refused_because(message: &str) -> String {
    todo!()
}

/// TS adapter.ts:288.
pub fn failure_words(message: &str, login: &str, key_env: Option<&str>) -> Option<FailureWords> {
    todo!()
}

/// An item with a string id and type. TS adapter.ts:297.
pub fn item_of(params: &Map<String, Value>) -> Option<Map<String, Value>> {
    todo!()
}

/// One change as `{path, kind}`. TS adapter.ts:305.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub kind: String,
}

/// TS adapter.ts:305.
pub fn changes_of(changes: Option<&Value>) -> Vec<Change> {
    todo!()
}

/// `None` where no change made a hunk. TS adapter.ts:319.
pub fn file_patches(changes: Option<&Value>) -> Option<Vec<FilePatch>> {
    todo!()
}

/// TS adapter.ts:334.
pub fn change_lines(changes: Option<&Value>) -> String {
    todo!()
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
    todo!()
}

/// TS adapter.ts:378.
pub fn held_of(last: &Map<String, Value>) -> f64 {
    todo!()
}

/// TS adapter.ts:381.
pub fn total_before(total: &Map<String, Value>, last: &Map<String, Value>) -> Map<String, Value> {
    todo!()
}

/// TS adapter.ts:384.
pub fn step_state(status: Option<&Value>) -> PlanStepState {
    todo!()
}

/// One plan step off `turn/plan/updated`. TS adapter.ts:1018.
pub fn plan_steps(plan: Option<&Value>) -> Vec<PlanStep> {
    todo!()
}

/// One call as a thread's earlier messages show it: `(tool, text)`. TS adapter.ts:387.
pub fn tool_call_of(item: &Map<String, Value>) -> Option<(String, String)> {
    todo!()
}

/// Each item as the deltas a timeline draws. TS adapter.ts:397.
pub fn item_deltas(done: bool, item: &Map<String, Value>, session_id: &str) -> Vec<AdapterEvent> {
    todo!()
}

/// TS adapter.ts:464.
pub fn child_end(status: Option<&str>) -> SubagentState {
    todo!()
}

/// TS adapter.ts:467.
pub fn depth_of(path: &str) -> u32 {
    todo!()
}

/// TS adapter.ts:470.
pub fn child_usage_of(total: Option<&Map<String, Value>>) -> Option<SubagentUsage> {
    todo!()
}

/// The child tokens a turn summed, by TurnTokens' field name, in the order first added.
pub type ChildTokens = indexmap::IndexMap<String, f64>;

/// TS adapter.ts:476.
pub fn with_child_tokens(lead: Option<TurnTokens>, children: &ChildTokens) -> Option<TurnTokens> {
    todo!()
}

/// TS adapter.ts:495.
pub fn turn_line_for(thread_id: &str, prompt: &str, images: &[String], effort: Option<&str>) -> String {
    todo!()
}

/// TS adapter.ts:501.
pub fn is_turn_start(line: &str) -> bool {
    todo!()
}

/// A thread's rollout: `<home>/sessions/YYYY/MM/DD/rollout-<stamp>-<thread id>.jsonl`. TS adapter.ts:511.
pub fn rollout_in(home: &str, thread_id: &str) -> crate::port::SessionFile {
    todo!()
}
