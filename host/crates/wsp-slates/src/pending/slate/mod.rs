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
pub type SlateResolve = Rc<dyn Fn(&str) -> Option<Value>>;
pub type SlateStart = Rc<dyn Fn(&str, &str) -> Value>;

/// types.ts:7.
pub const SLATE_SCHEMA: i64 = 2;

/// problems.ts:8.
pub const SLATE_SECRET_IN_ARGS: &str = "a secret reaches a command through env or stdin, never as an argument, which ps can read";

/// Every limit a slate is held to (limits.ts).
pub struct SlateLimits {
    pub document_bytes: usize,
    pub pieces: usize,
    pub depth: usize,
    pub children: usize,
    pub values_bytes: usize,
    pub values: usize,
    pub derived: usize,
    pub runs: usize,
    pub reactions: usize,
    pub files: usize,
    pub files_bytes: usize,
    pub steps_per_reaction: usize,
    pub rounds: usize,
    pub running_at_once: usize,
    pub starts_per_minute: usize,
    pub timer_floor_s: i64,
    pub timeout_default_s: i64,
    pub timeout_max_s: i64,
    pub stream_lines: usize,
    pub reaction_sends_per_minute: i64,
    pub expr_chars: usize,
    pub expr_depth: usize,
    pub holes: usize,
    pub pipeline_steps: usize,
    pub eval_steps: usize,
    pub list_items: usize,
    pub images_per_list: usize,
    pub pipeline_visits: usize,
    pub columns: usize,
    pub facts: usize,
    pub row_actions: usize,
    pub errors_per_pass: usize,
    pub pieces_named: usize,
    pub sketch_columns: usize,
    pub sketch_piece_lines: usize,
    pub sketch_rows: usize,
    pub sketch_value_chars: usize,
    pub sketch_tokens: usize,
    pub print_columns: usize,
    pub catalog_index_tokens: usize,
    pub catalog_entry_tokens: usize,
}

pub const SLATE_LIMITS: SlateLimits = SlateLimits {
    document_bytes: 64 * 1024,
    pieces: 300,
    depth: 10,
    children: 100,
    values_bytes: 256 * 1024,
    values: 64,
    derived: 32,
    runs: 16,
    reactions: 32,
    files: 16,
    files_bytes: 64 * 1024,
    steps_per_reaction: 6,
    rounds: 8,
    running_at_once: 4,
    starts_per_minute: 12,
    timer_floor_s: 10,
    timeout_default_s: 60,
    timeout_max_s: 600,
    stream_lines: 500,
    reaction_sends_per_minute: 1,
    expr_chars: 500,
    expr_depth: 16,
    holes: 20,
    pipeline_steps: 12,
    eval_steps: 20_000,
    list_items: 5_000,
    images_per_list: 60,
    pipeline_visits: 100_000,
    columns: 8,
    facts: 12,
    row_actions: 3,
    errors_per_pass: 20,
    pieces_named: 20,
    sketch_columns: 100,
    sketch_piece_lines: 40,
    sketch_rows: 8,
    sketch_value_chars: 80,
    sketch_tokens: 2_000,
    print_columns: 120,
    catalog_index_tokens: 1_500,
    catalog_entry_tokens: 200,
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

/// One step of a path: `.field` or `[index]`, where a negative index counts back from the end.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PathSeg {
    Key(String),
    Index(i64),
}

/// expr.ts's `SlateEvalContext`: the clock, the row scope and how a path reads.
#[derive(Clone)]
pub struct SlateEvalContext {
    pub now: i64,
    pub item: Option<Value>,
    pub index: Option<i64>,
    pub resolve: SlateResolve,
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
    pub start: Option<SlateStart>,
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
    /// `person` or `reaction`.
    pub by: String,
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
    /// The own names whose value moved in this batch, as `$name`.
    pub changed: Vec<String>,
    pub problems: Vec<SlateProblem>,
    pub starts: Vec<BatchStart>,
    pub cancels: Vec<String>,
    pub sends: Vec<BatchSend>,
    /// Steps the window carries out (`open`, `copy`, `pane`, `fill`): each step's `do`.
    pub other: Vec<String>,
    /// The same window steps with their data (`other` is their `do` names).
    pub other_steps: Vec<SlateBatchWindowStep>,
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
    serde_json::json!({"state": "idle", "runs": 0})
}

pub fn slate_bytes(text: &str) -> usize {
    text.len()
}

pub fn is_slate_binding(v: &Value) -> bool {
    v.as_object().is_some_and(|m| m.len() == 1 && m.get("bind").is_some_and(Value::is_string))
}

pub mod batch;
pub mod catalog;
pub mod chart;
pub mod examples;
pub mod expr;
pub mod image;
pub mod kit;
pub mod patch;
pub mod paths;
pub mod problems;
pub mod shape;
pub mod sketch;
pub mod sources;
pub mod steps;
pub mod syntax;
pub mod validate;
pub use batch::{run_slate_batch, SlateBatchSend, SlateBatchWindowStep, SlateWriter};
pub use catalog::slate_catalog;
pub use catalog::{slate_takes_figures, slate_tokens, SLATE_CHAPTERS, SLATE_INDEX_EXAMPLE, SLATE_RULES};
pub use chart::{
    slate_axis_word, slate_chart_axis, slate_shares, slate_span_state, slate_time, SlateChartAxis, SlateShare, SLATE_DONUT_PARTS,
    SLATE_SPAN_STATES, SLATE_TIME_FLOOR,
};
pub use examples::{SlateExample, SLATE_EXAMPLES};
pub use expr::{
    check_slate_expression, closing_brace, evaluate_slate_ast, evaluate_slate_expression, parse_slate_expression, parse_slate_format,
    resolve_slate_prop, slate_ast_dependencies, slate_dependencies, slate_path_text, slate_prop_dependencies, slate_text, slate_truthy,
    SlateBinOp, SlateCheckScope, SlateExpr, SlateExprCheck, SlateExprParse, SlateFormatParse, SlateFormatPart, SlateFunction, SlatePipeArg,
    SlatePipeStep, SlatePipeStepNode, SlateScopeProblem, SlateTemplatePart, SlateType, SlateTypeName, SLATE_FUNCTIONS, SLATE_PIPE_STEPS,
};
pub use image::{
    slate_address_refused, slate_image_missing, slate_image_not_a_file, slate_image_source, slate_image_too_big, slate_list_too_long,
    slate_looks_svg, slate_not_an_image, SlateImageSource, SLATE_IMAGE_LAYOUTS, SLATE_IMAGE_MAX_BYTES,
};
pub use kit::{
    slate_figure, slate_held_text, slate_piece_module, slate_reserved_prop, SlateBinds, SlateChildLimit, SlateItemSpec, SlateMost,
    SlatePieceCheck, SlatePieceModule, SlatePropSpec, SlatePropType, SlateSketchFn, SlateSketchView, SLATE_EVENTS, SLATE_FIGURES,
    SLATE_ITEM_KINDS, SLATE_PIECES, SLATE_RESERVED_PROPS, SLATE_TONES,
};
pub use patch::{apply_slate_patch, carry_slate_values, slate_start_values};
pub use paths::{get_slate_value, parse_slate_own_path, set_slate_value, slate_equal, slate_own_name, slate_segments, slate_step};
pub use problems::{
    or_list, slate_description_missing, slate_host_away, slate_nearest, slate_problem, slate_server_not_set_up, slate_unknown_piece,
    UnknownPiece, SLATE_CODES, SLATE_WARNINGS,
};
pub use shape::{
    sketch_slate_result, slate_field_words, slate_result_shape, SlateColumn, SlateFact, SlateNested, SlateResultShape, SLATE_RESULT_ROWS,
};
pub use sketch::{sketch_slate, SlateSketchContext};
pub use sources::{
    is_slate_source, slate_is_series, slate_shape_text, slate_shape_type, slate_source_type, SlateShape, SlateSourceModule, SLATE_SOURCES,
    SLATE_SOURCE_NAMES,
};
pub use steps::{slate_domain_key, slate_link_domain, slate_step_module, SlateStepModule, SLATE_STEPS};
pub use syntax::{compile_slate_text, parse_slate, parse_slate_patch, print_slate, SlateCompiled};
pub use validate::{validate_document, validate_slate, SlateLines, SlateValidated};
