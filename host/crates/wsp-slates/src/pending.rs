// SPDX-License-Identifier: AGPL-3.0-only
//! Stand-ins for what this crate names from crates that do not hold it yet. Spec: spec/slates-a-README.md, "What this
//! crate takes from crates that do not have it yet". Each module becomes `pub use` of the real items once they land:
//! `slate` of `wsp-slate` (protocol/src/slate/ less wire.ts), `wire` of `wsp_wire::types::slate_wire`, `siblings` of
//! this crate's other slice (slate-mcp.ts, slate-files.ts, slate-box.ts, slate-mcp-on.ts, slate-images.ts,
//! slate-sources/). Every function body is `todo!()`: none of them is this slice's to implement.

use wsp_words::exit::HostError;

/// `e.code`, the code a slate refusal stamps (`Z803`, `V750`, ...). HostError gains `code` in the words fix round
/// (ruling 6402177374 point 3); until then the tests read it through this.
pub fn code_of(e: &HostError) -> Option<String> {
    todo!()
}

/// Serde plumbing for a field the TS writes as absent, a value, or `null`, all three apart: `None` is absent,
/// `Some(Value::Null)` (or `Some(None)`) is a JSON null. serde's own `Option` reads `null` as absent.
pub mod null_kept {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
        T::deserialize(d).map(Some)
    }
}

/// protocol/src/slate/ at 65f6b89c7, the names this slice calls. Types are ported whole where the stored record holds
/// them; functions are stubs.
pub mod slate {
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
}

/// protocol/src/slate/wire.ts:33-123, 207-269 at 65f6b89c7: the view and the answers, for `wsp_wire::types::slate_wire`.
pub mod wire {
    use indexmap::IndexMap;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    use super::slate::SlateProblem;

    /// wire.ts:37.
    pub const SLATE_HELD_APPROVAL: &str = "needs your approval";
    /// wire.ts:94.
    pub const SLATE_SEND_KEY: &str = "send";

    /// wire.ts:33.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum SlateEmpty {
        #[serde(rename = "none")]
        None,
        #[serde(rename = "cleared")]
        Cleared,
        #[serde(rename = "rewound-before")]
        RewoundBefore,
    }

    /// wire.ts:40-66, keys in the order slates.ts:648-664 writes them.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SlateCmdAsk {
        pub key: String,
        pub run: String,
        pub cmd: String,
        pub env: IndexMap<String, String>,
        pub args: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub stdin: Option<String>,
        pub computer: String,
        pub folder: String,
        #[serde(rename = "timeoutS")]
        pub timeout_s: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub confirm: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub then: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub files: Option<IndexMap<String, String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub no_always: Option<bool>,
        pub why: String,
    }

    /// wire.ts:40-90: `kind` is written after `run` for a command (slates.ts:651); a tool ask is the MCP table's own
    /// JSON (wire.ts:69-88), kept as it comes.
    #[derive(Clone, Debug, PartialEq)]
    pub enum SlateAsk {
        Cmd(SlateCmdAsk),
        Tool(Value),
    }

    impl SlateAsk {
        pub fn key(&self) -> String {
            todo!()
        }
        pub fn run(&self) -> String {
            todo!()
        }
        /// The ask as the wire carries it: `{key, run, kind:"cmd", cmd, ...}` or the tool ask's JSON.
        pub fn to_json(&self) -> Value {
            todo!()
        }
    }

    /// wire.ts:97: an approval as a window reads it.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SlateApprovalView {
        pub state: String,
        pub at: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub run: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cmd: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub scripts: Option<IndexMap<String, String>>,
    }

    /// wire.ts:101-122, keys in the order slates.ts:669-684 writes them.
    #[derive(Clone, Debug, PartialEq)]
    pub struct SlateView {
        pub thread_id: String,
        pub workspace_id: String,
        pub version: i64,
        pub revision: i64,
        pub document: Option<Value>,
        pub values: IndexMap<String, Value>,
        pub empty: Option<SlateEmpty>,
        pub comments: Vec<Value>,
        pub approvals: IndexMap<String, SlateApprovalView>,
        pub asks: Vec<SlateAsk>,
        pub problems: Vec<SlateProblem>,
        pub shown_once: bool,
        pub can_undo: bool,
        pub updated_at: i64,
    }

    impl SlateView {
        /// The view as the wire carries it, in the TS's key order.
        pub fn to_json(&self) -> Value {
            todo!()
        }
    }

    /// wire.ts:212.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SlateWriteAnswer {
        pub version: i64,
        pub text: String,
        pub warnings: Vec<SlateProblem>,
        pub problems: Vec<SlateProblem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waiting: Option<Vec<String>>,
    }

    /// wire.ts:216.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SlateStateAnswer {
        pub version: i64,
        pub text: String,
        pub problems: Vec<SlateProblem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waiting: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub not_started: Option<Vec<String>>,
    }

    /// wire.ts:220-236, keys in the order slates.ts:1247-1260 writes them.
    #[derive(Clone, Debug, PartialEq)]
    pub struct SlateReadAnswer {
        pub version: i64,
        pub text: String,
        /// `Some` only with `document: true`; `Some(None)` when there is no document.
        pub document: Option<Option<Value>>,
        pub values: IndexMap<String, Value>,
        pub state: IndexMap<String, Value>,
        pub derived: IndexMap<String, Value>,
        pub runs: IndexMap<String, Value>,
        pub problems: Vec<SlateProblem>,
        pub waiting: Vec<String>,
        pub comments: Vec<Value>,
        pub approvals: IndexMap<String, String>,
    }

    /// wire.ts:243.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum SlateEventOutcome {
        Started,
        Steered,
        Queued,
        Held,
        Done,
    }

    /// wire.ts:246-253.
    #[derive(Clone, Debug, PartialEq)]
    pub struct SlateEventAnswer {
        pub outcome: SlateEventOutcome,
        pub said: String,
        pub turn_id: Option<String>,
        pub ask: Option<SlateAsk>,
    }

    /// wire.ts:263-268, as JSON.
    pub type SlatesImageAnswer = Value;
}

/// What `slates.ts` calls of the other wsp-slates slice, by the TS names in Rust case.
pub mod siblings {
    use std::future::Future;
    use std::pin::Pin as StdPin;
    use std::rc::Rc;

    use indexmap::IndexMap;
    use serde_json::Value;
    use wsp_words::exit::HostError;

    use super::slate::{SlateDoc, SlateProblem, SlateRunDecl};
    use super::wire::SlateAsk;
    use crate::slate_runs::{Reshape, RunRecord, RunRoad, RunStartAnswer};

    pub type LocalFuture<'a, T> = StdPin<Box<dyn Future<Output = T> + 'a>>;

    /// slate-sources/context.ts:7: what the source resolvers read for one thread.
    pub trait SlateSourceContext {}

    /// slate-sources/index.ts:15: the host sources' names, in order.
    pub const HOST_SLATE_SOURCES: [&str; 6] = ["thread", "usage", "cost", "time", "git", "pr"];

    /// slate-sources/index.ts:43.
    pub fn view_sources<'a>(names: Vec<String>, ctx: Rc<dyn SlateSourceContext>) -> LocalFuture<'a, IndexMap<String, Option<Value>>> {
        todo!()
    }
    /// slate-sources/index.ts:54.
    pub fn resolve_in(views: &IndexMap<String, Option<Value>>, path: &str) -> Option<Value> {
        todo!()
    }

    /// slate-mcp.ts:18: the computer a thread runs on where that is not this one.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Away {
        pub computer: String,
        pub host: String,
    }
    /// slate-mcp.ts:56.
    pub type McpServerSpec = Value;
    /// slate-mcp.ts:140: `never`, `every`, `unless-read-only`.
    pub type ToolAsks = String;

    /// slate-mcp.ts:27.
    pub fn host_away(away: Option<&Away>, run: &str) -> Option<String> {
        todo!()
    }
    /// What `refusedAway`'s `missingOn` answers (slate-mcp.ts:37).
    pub type MissingOn = Rc<dyn Fn(Vec<String>) -> LocalFuture<'static, Result<Value, HostError>>>;
    /// slate-mcp.ts:33.
    pub fn refused_away<'a>(before: Option<&'a SlateDoc>, doc: Option<&'a SlateDoc>, away: Option<Away>, missing_on: Option<MissingOn>) -> LocalFuture<'a, Vec<SlateProblem>> {
        todo!()
    }
    /// slate-mcp.ts:200: a tool or resource run's approval key over its arguments as written.
    pub fn code_key(decl: &SlateRunDecl, files: Option<&IndexMap<String, String>>, args: &IndexMap<String, Value>) -> String {
        todo!()
    }
    /// slate-mcp.ts:208.
    pub fn slate_secret_mark(name: &str) -> String {
        todo!()
    }

    /// slate-mcp.ts:127: a tool or resource run's start.
    pub struct McpRunStart {
        pub thread_id: String,
        pub run: String,
        pub decl: SlateRunDecl,
        pub files: Option<IndexMap<String, String>>,
        pub confirm: Option<String>,
        pub by: String,
        pub args: Rc<dyn Fn() -> IndexMap<String, Value>>,
        pub last: Option<RunRecord>,
    }

    /// slate-mcp.ts:142-161, what the slate area hands the MCP table.
    pub trait SlateMcpDeps {
        fn server(&self, thread_id: &str, name: &str) -> LocalFuture<'static, Result<McpServerSpec, HostError>>;
        fn on_record(&self, thread_id: &str, run: &str, record: RunRecord);
        fn on_asks(&self, thread_id: &str);
        fn reshape(&self, thread_id: &str, cmd: &str, input: &str) -> Reshape;
        fn computer(&self, thread_id: &str) -> String;
        fn asks(&self, thread_id: &str) -> ToolAsks;
        fn now(&self) -> i64;
    }

    /// slate-mcp.ts:163-195.
    pub trait SlateMcp {
        fn provisional(&self, thread_id: &str, decl: &SlateRunDecl, files: Option<&IndexMap<String, String>>, last: Option<&RunRecord>, args: &IndexMap<String, Value>) -> RunRecord;
        fn start(&self, req: McpRunStart) -> RunStartAnswer;
        fn approve(&self, thread_id: &str, key: &str, scope: &str);
        fn deny(&self, thread_id: &str, key: &str);
        fn owns(&self, thread_id: &str, key: &str) -> bool;
        fn held(&self, thread_id: &str) -> Vec<SlateAsk>;
        /// `{run, cmd}` of a key it asked.
        fn named(&self, thread_id: &str, key: &str) -> Option<(String, String)>;
        fn pending(&self, thread_id: &str, ms: i64) -> LocalFuture<'static, ()>;
        fn cancel(&self, thread_id: &str, run: &str);
        fn stop_all(&self, thread_id: &str, quiet: bool, why: Option<&str>);
        fn servers(&self, thread_id: &str, names: &[String], kept: &[String]);
        fn shown(&self, thread_id: &str, shown: bool);
        fn close_servers(&self, thread_id: &str);
        fn catalog(&self, thread_id: &str, server: &str) -> LocalFuture<'static, Result<String, HostError>>;
        fn refused_runs<'a>(&'a self, thread_id: &'a str, before: Option<&'a SlateDoc>, doc: Option<&'a SlateDoc>, away: Vec<SlateProblem>) -> LocalFuture<'a, Vec<SlateProblem>>;
        fn drop_thread(&self, thread_id: &str, keep: bool);
        fn close(&self);
    }

    /// slate-mcp.ts:586.
    pub fn create_slate_mcp(deps: Rc<dyn SlateMcpDeps>) -> Rc<dyn SlateMcp> {
        todo!()
    }

    /// slate-files.ts:55: hashes the named paths under a root on the thread's machine.
    pub type HashOn = Rc<dyn Fn(String, Vec<String>) -> LocalFuture<'static, Result<Value, HostError>>>;

    /// slate-files.ts:60-64: a path the pin could not cover, and why (`unread`, `outside`, ...).
    #[derive(Clone, Debug, PartialEq)]
    pub struct Unpinned {
        pub path: String,
        pub why: String,
    }
    /// slate-files.ts:64.
    #[derive(Clone, Debug, PartialEq, Default)]
    pub struct Pin {
        pub scripts: Option<IndexMap<String, String>>,
        pub unpinned: Vec<Unpinned>,
    }
    /// slate-files.ts:20: writes the files into the folder, 0700 and 0600, removing what is not declared; answers the folder.
    pub fn write_slate_files(dir: &str, files: &IndexMap<String, String>) -> Result<String, HostError> {
        todo!()
    }
    /// slate-files.ts:32: the slate files a declaration reads.
    pub fn with_files(doc: Option<&SlateDoc>, decl: &SlateRunDecl) -> Option<IndexMap<String, String>> {
        todo!()
    }
    /// slate-files.ts:68.
    pub fn keyed(pin: &Pin) -> Option<IndexMap<String, String>> {
        todo!()
    }
    /// slate-files.ts:87.
    pub fn cannot_pin(pin: &Pin) -> Option<String> {
        todo!()
    }
    /// slate-files.ts:93.
    pub fn always_refused(run: &str, pin: &Pin) -> Option<String> {
        todo!()
    }
    /// slate-files.ts:217.
    pub fn pin_here(folder: Option<&str>, decl: &SlateRunDecl) -> Pin {
        todo!()
    }
    /// slate-files.ts:231: the pins of the thread's machine as its daemon last read them.
    pub trait BoxPins {
        fn pin(&self, thread_id: &str, folder: Option<&str>, decl: &SlateRunDecl) -> Pin;
        fn read(&self, thread_id: &str, there: Vec<(Option<String>, SlateRunDecl)>, asleep: bool) -> LocalFuture<'static, ()>;
        fn drop_thread(&self, thread_id: &str);
    }
    pub fn box_pins(hash_on: Rc<dyn Fn(&str) -> Option<HashOn>>) -> Rc<dyn BoxPins> {
        todo!()
    }

    /// slate-box.ts:20-28.
    pub trait BoxLedger {
        fn remove(&self, machine: Rc<dyn wsp_engine::machine::Machine>, path: &str) -> LocalFuture<'static, bool>;
        fn close(&self);
    }
    /// slate-box.ts:30.
    pub fn box_ledger() -> Rc<dyn BoxLedger> {
        todo!()
    }
    /// slate-box.ts:98: the road of a run on the thread's own machine.
    pub fn box_road(
        machine: Rc<dyn wsp_engine::machine::Machine>,
        thread_id: &str,
        files: Rc<dyn Fn() -> IndexMap<String, String>>,
        ready: Rc<dyn Fn() -> LocalFuture<'static, ()>>,
        ledger: Rc<dyn BoxLedger>,
    ) -> Rc<dyn RunRoad> {
        todo!()
    }
    /// slate-mcp-on.ts:31.
    pub fn box_slate_dir(thread_id: &str) -> String {
        todo!()
    }

    /// slate-images.ts:18: reads an image file on the thread's computer through its daemon.
    pub type ImageOn = Value;
    /// slate-images.ts:23-33.
    pub struct SlateImageRoads {
        pub thread: String,
        pub have: Option<String>,
        pub folder: Option<String>,
        pub on: Option<ImageOn>,
        pub allowed: Rc<dyn Fn(&str) -> bool>,
    }
    /// slate-images.ts:52.
    pub fn slate_image<'a>(src: &'a str, roads: SlateImageRoads) -> LocalFuture<'a, Result<Value, HostError>> {
        todo!()
    }
}
