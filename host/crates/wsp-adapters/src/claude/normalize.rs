// SPDX-License-Identifier: AGPL-3.0-only
//! The pure half of the turn reader: what one stream-json line says, and what a result costs. TS:
//! packages/adapter-claude/src/adapter.ts:213-625 at a3b870a6e.

use serde_json::Value;

use crate::claude::fields::Obj;
use crate::claude::handback::HandbackBook;
use crate::claude::plans::PlanBook;
use crate::pending::Refusal;
use crate::port::{AdapterEvent, SessionFile, TurnResult, TurnStatus, TurnTokens};
use wsp_wire::views::session::TurnRefusal;
use wsp_wire::views::session_events::TurnResultModelsItem;

/// One model's use: `{ model, tokens, costUsd? }`. adapter.ts:242.
pub type ModelUse = TurnResultModelsItem;

/// JSON.parse of a tool_use delta's text; `None` where it does not parse. adapter.ts:213.
pub fn parse_input(text: &str) -> Option<Value> {
    todo!()
}

/// adapter.ts:221. `errors_text` is the result's errors joined by " " and lower-cased.
pub fn result_status(event: &Obj, errors_text: &str) -> TurnStatus {
    todo!()
}

/// adapter.ts:248. Each entry of a modelUsage object, in its key order; non-object entries skipped.
pub fn models_of(raw: Option<&Value>) -> Vec<ModelUse> {
    todo!()
}

/// adapter.ts:262. By its own name first, then by base_model on both sides.
pub fn start_of<'a>(models: &'a [ModelUse], model: &str) -> Option<&'a ModelUse> {
    todo!()
}

/// adapter.ts:266.
pub fn with_calls(models: &[ModelUse], calls: &[ModelUse]) -> Vec<ModelUse> {
    todo!()
}

/// The turn's tokens and running totals off its result. adapter.ts:278.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResultTokens {
    pub tokens: Option<TurnTokens>,
    pub models: Option<Vec<ModelUse>>,
}

/// adapter.ts:278.
pub fn result_tokens(event: &Obj) -> ResultTokens {
    todo!()
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
    todo!()
}

/// adapter.ts:311.
pub fn gained(now: &ModelUse, before: Option<&ModelUse>) -> Option<ModelUse> {
    todo!()
}

/// adapter.ts:325. Key order of the answer: the result's own keys less costUsd and models, then tokens, costUsd,
/// models, model (each where present), a key already there keeping its place.
pub fn own_use(result: TurnResult, saved: Option<&SavedUse>, fresh: bool, spent: Option<&TurnTokens>) -> TurnResult {
    todo!()
}

/// adapter.ts:348.
pub fn normalize_result(event: &Obj, refusal: Option<&Refusal>) -> TurnResult {
    todo!()
}

/// adapter.ts:365: `authentication_failed` is `sign-in`; nothing else is claimed.
pub fn refusal_cause(error: &str) -> Option<TurnRefusal> {
    todo!()
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
    todo!()
}

/// adapter.ts:377.
pub const STOP_TASK_UNSUPPORTED: &str = "stop_task is not supported in this context";
/// adapter.ts:380.
pub const NO_CONVERSATION: &str = "No conversation found with session ID:";

/// adapter.ts:384.
pub fn answered_nothing(result: &TurnResult) -> bool {
    todo!()
}

/// adapter.ts:391.
pub fn with_context(result: TurnResult, context: Option<f64>, init_model: Option<&str>) -> TurnResult {
    todo!()
}

/// What the model held before and after a compaction. adapter.ts:398.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Compacted {
    pub before: Option<f64>,
    pub after: f64,
}

/// adapter.ts:398.
pub fn compacted_to(event: &Obj) -> Option<Compacted> {
    todo!()
}

/// adapter.ts:408. A `turn.retry`; `why` is always present.
pub fn retry_of(event: &Obj, session_id: &str) -> Option<AdapterEvent> {
    todo!()
}

/// adapter.ts:419.
pub fn subagent_retry_of(event: &Obj, session_id: &str) -> Option<AdapterEvent> {
    todo!()
}

/// adapter.ts:428.
pub const STDERR_TAIL_LINES: usize = 5;

/// adapter.ts:430.
pub fn no_output_error(result: &TurnResult, stderr_tail: &[String]) -> String {
    todo!()
}

/// adapter.ts:437.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubagentLaunch {
    pub agent_id: String,
    pub tool_use_id: String,
}

/// adapter.ts:437.
pub fn subagent_launch(event: &Obj) -> Option<SubagentLaunch> {
    todo!()
}

/// adapter.ts:445.
pub const SUBAGENT_TOOLS: [&str; 2] = ["Agent", "Task"];

/// adapter.ts:448. (call id, prompt) pairs in block order.
pub fn subagent_prompts(event: &Obj) -> Vec<(String, String)> {
    todo!()
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
    todo!()
}

/// One background task the CLI reports running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundTask {
    pub id: String,
    pub description: String,
}

/// adapter.ts:476.
pub fn background_tasks_of(event: &Obj) -> Option<Vec<BackgroundTask>> {
    todo!()
}

/// adapter.ts:487. (id, status).
pub fn task_finished_of(event: &Obj) -> Option<(String, String)> {
    todo!()
}

/// adapter.ts:497.
pub fn drained_notice(event: &Obj) -> bool {
    todo!()
}

/// adapter.ts:502. Mutates `plans` and `book` as the TS does.
pub fn normalize_event(
    event: &Obj,
    fallback_session_id: &str,
    refusal: Option<&Refusal>,
    plans: &mut PlanBook,
    book: &mut HandbackBook,
) -> Vec<AdapterEvent> {
    todo!()
}

/// adapter.ts:625.
pub fn session_file_in(config_dir: &str, session_id: &str) -> SessionFile {
    todo!()
}
