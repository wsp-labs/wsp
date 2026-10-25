// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-mcp.ts:1-556 at 65f6b89c7. Spec: spec/slate_mcp_client.md.
//!
//! One server of the thread's agent, over a command's stdio or streamable HTTP, every message bounded in size and
//! every parsed answer bounded in depth and width before anything walks it; what a write from a thread off this
//! computer is refused for; the approval key; the folding of a tool's input schema for `slate_catalog`.
#![allow(unused_variables)]

use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use wsp_catalog::mcp::McpTransport;
pub use wsp_wire::wire::daemon::{HttpBearer, EXEC_OUTPUT_MAX};

use wsp_core::clock::Clock;

use crate::pending::{SlateDoc, SlateProblem};
use crate::slate_files::WithFiles;

/// The thread's computer where it is not this one, and this one's name (slate-mcp.ts:18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Away {
    pub computer: String,
    pub host: String,
    /// The machine wsp made on that computer the thread runs on, where it is one: it gets no tool run.
    pub machine: Option<String>,
}

// slate-mcp.ts:27 hostAway(away, run)
pub fn host_away(away: Option<&Away>, run: &str) -> Option<String> {
    todo!()
}

/// What the thread's computer said of the servers a write asks about (slate-mcp.ts:37).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingOn {
    pub agent_id: String,
    pub agent_name: String,
    pub thread: String,
    pub missing: Vec<String>,
    pub off: Vec<String>,
}

pub type MissingOnFn = Rc<dyn Fn(Vec<String>) -> LocalBoxFuture<'static, MissingOn>>;

// slate-mcp.ts:33 refusedAway(before, doc, away, missingOn)
/// What a slate is refused as it is written from a thread off this computer (spec rule 2).
pub async fn refused_away(before: Option<&SlateDoc>, doc: Option<&SlateDoc>, away: Option<&Away>, missing_on: Option<MissingOnFn>) -> Vec<SlateProblem> {
    todo!()
}

/// A tool or resource run as declared, with the text of each slate file its then reads (slate-mcp.ts:52). Its `decl`
/// is always a tool or a resource.
pub type McpRunDecl = WithFiles;

/// An http server's sign-in the thread's agent keeps on its own computer (slate-mcp.ts:73).
#[derive(Clone)]
pub struct McpSignIn {
    pub bearer: HttpBearer,
    /// The agent's own word on it asked again there: true where it holds one, renewing a token near its end.
    pub renew: Rc<dyn Fn() -> LocalBoxFuture<'static, bool>>,
    /// The line the person signs in with there.
    pub line: String,
    /// On this computer, the bearer read here at each message; absent where only that computer's daemon reads it.
    pub read: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, Option<String>>>>,
}

/// What an http server answered one message (slate-mcp.ts:82).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpHttpAnswer {
    pub status: u16,
    pub content_type: Option<String>,
    pub session_id: Option<String>,
    pub body: String,
}

/// One message to an http server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpHttpRequest {
    pub headers: IndexMap<String, String>,
    pub body: String,
    pub ms: u64,
}

/// One message to an http server and the end of the road it goes by (slate-mcp.ts:90). `t` is always the http
/// transport.
pub trait McpPost {
    fn send(&self, t: &McpTransport, req: McpHttpRequest) -> LocalBoxFuture<'static, Result<McpHttpAnswer, String>>;
    fn close(&self);
}

/// What a command server's process says, wherever it runs; `exit` is said with why it ended (slate-mcp.ts:96).
#[derive(Clone)]
pub struct McpPipeEvents {
    pub out: Rc<dyn Fn(&[u8])>,
    pub err: Rc<dyn Fn(&str)>,
    pub exit: Rc<dyn Fn(&str)>,
}

/// A command server's stdin and the stop of its whole process group (slate-mcp.ts:103).
pub trait McpPipe {
    fn write(&self, text: &str);
    fn kill(&self);
    /// Where it runs on this computer.
    fn pid(&self) -> Option<u32>;
}

/// Opens a command server; `t` is always the stdio transport (slate-mcp.ts:109).
pub type McpPipeOpen = Rc<dyn Fn(&McpTransport, &McpServerSpec, McpPipeEvents) -> Rc<dyn McpPipe>>;

/// Wraps this computer's own post in the road a connection asks by (slate-mcp.ts:65).
pub type McpPostRoad = Rc<dyn Fn(Rc<dyn McpPost>) -> Rc<dyn McpPost>>;

/// A server as the thread's agent would start it there (slate-mcp.ts:56).
#[derive(Clone)]
pub struct McpServerSpec {
    pub transport: McpTransport,
    pub cwd: String,
    pub env: IndexMap<String, String>,
    /// Values handed to it, hidden from whatever it says back.
    pub secrets: Vec<String>,
    /// Where a command server runs; absent runs it on this computer.
    pub open: Option<McpPipeOpen>,
    /// Where an http server is asked from; absent asks it from this computer.
    pub post: Option<McpPostRoad>,
    pub sign_in: Option<McpSignIn>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct McpToolAnnotations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "readOnlyHint", default, skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<Value>,
    #[serde(rename = "destructiveHint", default, skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<Value>,
    #[serde(rename = "idempotentHint", default, skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<Value>,
    #[serde(rename = "openWorldHint", default, skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<Value>,
}

/// A tool as `tools/list` lists it (slate-mcp.ts:111). A hint counts only when it is exactly `true`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Value>,
    #[serde(rename = "inputSchema", default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<Value>,
    #[serde(rename = "outputSchema", default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<McpToolAnnotations>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpResourceInfo {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "mimeType", default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Value>,
}

/// When the thread's agent asks before an MCP tool at its mode now (slate-mcp.ts:140).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolAsks {
    Never,
    Every,
    UnlessReadOnly,
}

// slate-mcp.ts:200 codeKey(decl, args)
/// What the person allows of a tool or resource run (spec rule 3).
pub fn code_key(decl: &McpRunDecl, args: &Map<String, Value>) -> String {
    todo!()
}

// slate-mcp.ts:208 slateSecretMark(name)
pub fn slate_secret_mark(name: &str) -> String {
    todo!()
}

pub const SECRET_MARK: &str = "\u{0}wsp-secret:";
/// The pattern of a mark inside a string (slate-mcp.ts:209).
pub const MARKED: &str = "\u{0}wsp-secret:([A-Za-z_][A-Za-z0-9_-]*)\u{0}";

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const CONNECT_MS: u64 = 20_000;
pub const CALL_MS: u64 = 60_000;
pub const IDLE_MS: u64 = 5 * 60_000;
pub const SWEEP_MS: u64 = 30_000;
pub const ERR_KEPT: usize = 4_000;
pub const DESCRIBE_MS: u64 = 10_000;
pub const MESSAGE_MAX_BYTES: usize = 2_097_152;
pub const TOO_BIG: &str = "the server sent a message over 2 MB";
pub const DEPTH_MAX: usize = 64;
pub const ITEMS_MAX: usize = 100_000;

// slate-mcp.ts:228 shapeRefusal(value)
/// Why a parsed answer is too deep or too wide to walk; iterative, so a deep one cannot take the stack (spec rule 6).
pub fn shape_refusal(value: &Value) -> Option<String> {
    todo!()
}

// slate-mcp.ts:244 parsedText(text)
/// JSON a tool put in its text, or `None` where it does not parse or is too deep or wide (spec rule 7).
pub fn parsed_text(text: &str) -> Option<Value> {
    todo!()
}

/// A request's answer: the result, or the failure's words, which become a run's `why`.
pub type McpAnswer = LocalBoxFuture<'static, Result<Value, String>>;

/// One live connection to a server (slate-mcp.ts:294).
pub trait McpClient {
    /// The id the request went with, and its answer.
    fn request(&self, method: &str, params: Map<String, Value>, ms: u64) -> (u64, McpAnswer);
    fn notify(&self, method: &str, params: Option<Map<String, Value>>);
    fn close(&self);
    fn closed(&self) -> bool;
    /// The tail of what the server said on stderr, for a failure's why.
    fn said(&self) -> String;
    /// The server's process on this computer, for a stdio server.
    fn pid(&self) -> Option<u32>;
}

// slate-mcp.ts:305 RpcFailed
/// The words of a JSON-RPC error answer: `the server answered with error <code or ?>[: <message>]`.
pub fn rpc_failed(error: &Value) -> String {
    todo!()
}

// slate-mcp.ts:312 spawnHere
/// The server's command on this computer, in a process group of its own (spec rule 9).
pub fn spawn_here() -> McpPipeOpen {
    todo!()
}

// slate-mcp.ts:334 stdioClient(t, spec, onClose)
/// A stdio client; `on_close` runs once whenever it closes (spec rule 8). Deadlines go through `clock`.
pub fn stdio_client(t: &McpTransport, spec: &McpServerSpec, on_close: Box<dyn Fn()>, clock: Rc<dyn Clock>) -> Rc<dyn McpClient> {
    todo!()
}

// slate-mcp.ts:414 fetchHere
/// This computer's own fetch: no redirect followed, a failed answer's body never read, a body over 2 MiB cut.
pub fn fetch_here() -> Rc<dyn McpPost> {
    todo!()
}

// slate-mcp.ts:426 httpClient(t, spec, onClose)
/// Streamable HTTP: one POST per message (spec rule 10).
pub fn http_client(t: &McpTransport, spec: &McpServerSpec, on_close: Box<dyn Fn()>) -> Rc<dyn McpClient> {
    todo!()
}

// slate-mcp.ts:491 sentence(text, max)
/// A description cut to its first sentence and at most `max` characters (spec rule 12).
pub fn sentence(text: Option<&Value>, max: usize) -> Option<String> {
    todo!()
}

// slate-mcp.ts:530 foldSchema(schema)
/// One line per property, nested objects and objects in arrays as dotted names, three deep.
pub fn fold_schema(schema: &Value) -> Vec<String> {
    todo!()
}

// slate-mcp.ts:551 hintsOf(tool, structured)
pub fn hints_of(tool: &McpToolInfo, structured: Option<bool>) -> Vec<String> {
    todo!()
}

// slate-mcp.ts:281 ASKS_LINE
pub fn asks_line(asks: ToolAsks) -> &'static str {
    todo!()
}
