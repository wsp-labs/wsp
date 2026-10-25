// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/ at 65f6b89c7, the names this slice calls. Types are ported whole where the stored record holds
//! them; functions are stubs.
use std::rc::Rc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_wire::types::js_number::JsNumber;

/// types.ts:8: any JSON.
pub type SlateJson = Value;
/// types.ts:262: the live values by name without the `$`.
pub type SlateValues = IndexMap<String, Value>;
/// types.ts:31: a literal, `{bind}` or `{format}`, as JSON.
pub type SlatePropValue = Value;

/// types.ts:7.
pub const SLATE_SCHEMA: i64 = 2;

/// problems.ts:8.
pub const SLATE_SECRET_IN_ARGS: &str = "a secret reaches a command through env or stdin, never as an argument, which ps can read";

/// limits.ts:3-45, the entries this slice reads, with their values.
pub struct SlateLimits {
    pub document_bytes: usize,
    pub values_bytes: usize,
    pub running_at_once: usize,
    pub starts_per_minute: usize,
    pub timer_floor_s: i64,
    pub timeout_default_s: i64,
    pub timeout_max_s: i64,
    pub stream_lines: usize,
    pub reaction_sends_per_minute: i64,
    pub errors_per_pass: usize,
    pub pieces_named: usize,
}

pub const SLATE_LIMITS: SlateLimits = SlateLimits {
    document_bytes: 64 * 1024,
    values_bytes: 256 * 1024,
    running_at_once: 4,
    starts_per_minute: 12,
    timer_floor_s: 10,
    timeout_default_s: 60,
    timeout_max_s: 600,
    stream_lines: 500,
    reaction_sends_per_minute: 1,
    errors_per_pass: 20,
    pieces_named: 20,
};

/// types.ts:270-281: the one shape of an error, a warning and a runtime problem, keys in this order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SlateProblem {
    pub code: String,
    pub name: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

/// types.ts:98-105.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlateValueDecl {
    pub start: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<bool>,
}

/// types.ts:115-121, the `cmd` member.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlateCmdDecl {
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<IndexMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<Value>>,
    /// A literal `null` stdin is kept apart from none: it changes the approval key.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "super::null_kept::deserialize")]
    pub stdin: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// A literal, `{bind}` or `{format}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub once: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
}

/// types.ts:122, the `tool` member.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlateToolDecl {
    pub server: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<IndexMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub once: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
}

/// types.ts:123, the `resource` member.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlateResourceDecl {
    pub server: String,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// types.ts:114-123.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SlateRunDecl {
    Cmd(SlateCmdDecl),
    Tool(SlateToolDecl),
    Resource(SlateResourceDecl),
}

/// types.ts:132-139.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlatePiece {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props: Option<IndexMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Value>,
}

/// types.ts:142-154: the stored document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlateDoc {
    pub schema: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub root: String,
    pub values: IndexMap<String, SlateValueDecl>,
    pub derived: IndexMap<String, String>,
    pub runs: IndexMap<String, SlateRunDecl>,
    /// `SlateReactionDecl[]`, read by the module only.
    pub reactions: Vec<Value>,
    pub pieces: IndexMap<String, SlatePiece>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<IndexMap<String, String>>,
}

/// A parsed own path: `$name` and the steps below it (paths.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct SlateOwnPath {
    pub name: String,
    pub segs: Vec<PathSeg>,
}

/// One step of a path: `.field` or `[index]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PathSeg {
    Key(String),
    Index(usize),
}

/// expr.ts's `SlateEvalContext`: the clock, the row scope and how a path reads.
#[derive(Clone)]
pub struct SlateEvalContext {
    pub now: i64,
    pub item: Option<Value>,
    pub index: Option<i64>,
    pub resolve: Rc<dyn Fn(&str) -> Option<Value>>,
}

/// The event a batch runs for (slates.ts:200-206).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEvent {
    pub piece: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_action: Option<i64>,
}

/// batch.ts's context: the eval context and what the batch adds (slates.ts:904-918).
#[derive(Clone)]
pub struct SlateBatchContext {
    pub eval: SlateEvalContext,
    /// `person`, `agent`, `reaction`, `run` or `host`; a timer's batch reads as `run`.
    pub by: String,
    pub event: Option<BatchEvent>,
    pub reaction_sends: bool,
    /// `Some(false)` from a paired computer: no reaction fires.
    pub react: Option<bool>,
    /// A start the batch asks for: answers the record it will read until the start runs.
    pub start: Option<Rc<dyn Fn(&str, &str) -> Value>>,
}

/// One write into a batch: a path and its value.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateWrite {
    pub path: String,
    pub value: Value,
}

/// A start a batch asked for, with why (`a press on pick`).
#[derive(Clone, Debug, PartialEq)]
pub struct BatchStart {
    pub run: String,
    pub why: String,
}

/// A send step as the batch hands it over (slates.ts:208).
#[derive(Clone, Debug, PartialEq)]
pub struct BatchSend {
    /// `send`, `steer` or `queue`.
    pub kind: String,
    pub text: String,
    pub with: Option<Vec<String>>,
    pub values: Option<IndexMap<String, Value>>,
    pub piece: Option<String>,
    pub reaction: Option<String>,
    /// `person` for a press.
    pub by: Option<String>,
}

/// batch.ts's result.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateBatchResult {
    pub values: SlateValues,
    pub problems: Vec<SlateProblem>,
    pub starts: Vec<BatchStart>,
    pub cancels: Vec<String>,
    pub sends: Vec<BatchSend>,
    /// Steps the window carries out (`open`, `copy`, `pane`, `fill`): each step's `do`.
    pub other: Vec<String>,
}

/// What `parseSlate` and `validateSlate` answer.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateChecked {
    pub document: Option<SlateDoc>,
    pub errors: Vec<SlateProblem>,
    pub warnings: Vec<SlateProblem>,
}

/// patch.ts's parse: the ops as JSON (`{op, id?, ...}`) and the source lines the apply reads.
#[derive(Clone, Debug, PartialEq)]
pub struct SlatePatchParsed {
    pub ops: Option<Vec<Value>>,
    pub lines: Value,
    pub errors: Vec<SlateProblem>,
}

/// patch.ts's apply: `document` is `None` when refused, `Some(None)` for a clear.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateApplied {
    pub document: Option<Option<SlateDoc>>,
    pub values: Option<SlateValues>,
    pub errors: Vec<SlateProblem>,
    pub warnings: Vec<SlateProblem>,
}

/// sketch.ts's options (slates.ts:586).
#[derive(Clone)]
pub struct SketchOptions {
    pub eval: SlateEvalContext,
    pub version: i64,
    pub warnings: Vec<SlateProblem>,
    pub check: bool,
    pub problems: Vec<SlateProblem>,
    pub waiting: Vec<String>,
}

pub fn slate_run_idle() -> Value {
    todo!()
}
pub fn slate_problem(code: &str, message: &str, fix: Option<&str>) -> SlateProblem {
    todo!()
}
pub fn is_slate_source(name: &str) -> bool {
    todo!()
}
pub fn slate_bytes(text: &str) -> usize {
    todo!()
}
pub fn parse_slate(text: &str) -> SlateChecked {
    todo!()
}
pub fn validate_slate(document: &Value) -> SlateChecked {
    todo!()
}
pub fn parse_slate_patch(text: &str, doc: Option<&SlateDoc>) -> SlatePatchParsed {
    todo!()
}
pub fn apply_slate_patch(doc: Option<&SlateDoc>, values: &SlateValues, ops: &[Value], lines: &Value) -> SlateApplied {
    todo!()
}
pub fn print_slate(doc: &SlateDoc) -> String {
    todo!()
}
pub fn sketch_slate(doc: Option<&SlateDoc>, values: &SlateValues, o: SketchOptions) -> String {
    todo!()
}
pub fn run_slate_batch(doc: &SlateDoc, values: &SlateValues, input: Vec<SlateWrite>, ctx: SlateBatchContext) -> SlateBatchResult {
    todo!()
}
pub fn evaluate_slate_expression(expr: &str, ctx: &SlateEvalContext) -> Option<Value> {
    todo!()
}
pub fn resolve_slate_prop(v: &Value, ctx: &SlateEvalContext) -> Option<Value> {
    todo!()
}
pub fn get_slate_value(values: &SlateValues, path: &str) -> Option<Value> {
    todo!()
}
pub fn set_slate_value(values: &SlateValues, path: &str, value: Value) -> Option<SlateValues> {
    todo!()
}
pub fn parse_slate_own_path(path: &str) -> Option<SlateOwnPath> {
    todo!()
}
pub fn slate_step(at: Option<Value>, seg: &PathSeg) -> Option<Value> {
    todo!()
}
pub fn is_slate_binding(v: &Value) -> bool {
    todo!()
}
pub fn slate_dependencies(expr: &str) -> Vec<String> {
    todo!()
}
pub fn slate_equal(a: Option<&Value>, b: Option<&Value>) -> bool {
    todo!()
}
pub fn slate_nearest(word: &str, options: &[String]) -> Option<String> {
    todo!()
}
pub fn slate_domain_key(domain: &str) -> String {
    todo!()
}
pub fn slate_text(v: &Value) -> String {
    todo!()
}
pub fn slate_catalog(name: Option<&str>) -> String {
    todo!()
}
