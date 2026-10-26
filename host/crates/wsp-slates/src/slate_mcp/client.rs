// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-mcp.ts:1-556 at 65f6b89c7. Spec: spec/slate_mcp_client.md.
//!
//! One server of the thread's agent, over a command's stdio or streamable HTTP, every message bounded in size and
//! every parsed answer bounded in depth and width before anything walks it; what a write from a thread off this
//! computer is refused for; the approval key; the folding of a tool's input schema for `slate_catalog`.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Stdio};
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::{select, Either, LocalBoxFuture};
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpStream;
use tokio::task::spawn_local;
use url::Url;

pub use wsp_catalog::mcp::McpTransport;
/// When the thread's agent asks before an MCP tool at its mode now (slate-mcp.ts:140): wsp-core's, which wsp-slates takes.
pub use wsp_core::harness_catalog::ToolAsks;
pub use wsp_wire::wire::daemon::{HttpBearer, EXEC_OUTPUT_MAX};

use wsp_core::clock::{real_clock, Cancel, Clock};
use wsp_words::js::{is_js_whitespace, js_collapse_whitespace, js_round, js_trim, number_string, utf16};

use crate::pending::slate::problems::{slate_host_away, slate_problem, slate_server_not_set_up};
use crate::pending::slate::{SlateDoc, SlateProblem, SlateRunDecl};
use crate::slate_files::WithFiles;

/// The thread's computer where it is not this one, and this one's name (slate-mcp.ts:18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Away {
    pub computer: String,
    pub host: String,
    /// The machine wsp made on that computer the thread runs on, where it is one: it gets no tool run.
    pub machine: Option<String>,
}

pub fn host_away(away: Option<&Away>, run: &str) -> Option<String> {
    away.map(|a| slate_host_away(&format!("${run}"), &a.computer, &a.host))
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

/// What a slate is refused as it is written from a thread off this computer (spec rule 2).
pub async fn refused_away(
    before: Option<&SlateDoc>,
    doc: Option<&SlateDoc>,
    away: Option<&Away>,
    missing_on: Option<MissingOnFn>,
) -> Vec<SlateProblem> {
    let (Some(away), Some(doc)) = (away, doc) else { return Vec::new() };
    let mut problems: Vec<SlateProblem> = doc
        .runs
        .iter()
        .filter_map(|(name, d)| match d {
            SlateRunDecl::Cmd(c) if c.on.as_deref() == Some("host") => {
                Some(slate_problem("K711", &slate_host_away(&format!("${name}"), &away.computer, &away.host), None))
            }
            _ => None,
        })
        .collect();
    let tools: Vec<String> = doc.runs.iter().filter(|(_, d)| !matches!(d, SlateRunDecl::Cmd(_))).map(|(n, _)| n.clone()).collect();
    if let Some(machine) = &away.machine {
        if !tools.is_empty() {
            problems.push(slate_problem("K712", &machine_on_box(&tools, machine, &away.computer), None));
        }
        return problems;
    }
    let known: HashSet<String> = before.map(|b| b.runs.values().filter_map(server_of).collect()).unwrap_or_default();
    let mut asked: IndexSet<String> = IndexSet::new();
    for d in doc.runs.values() {
        if let Some(server) = server_of(d) {
            if !known.contains(&server) {
                asked.insert(server);
            }
        }
    }
    if asked.is_empty() {
        return problems;
    }
    let Some(missing_on) = missing_on else { return problems };
    let got = missing_on(asked.into_iter().collect()).await;
    problems.extend(got.missing.iter().map(|server| {
        slate_problem("K710", &slate_server_not_set_up(server, &away.computer, &got.agent_id, &got.agent_name, &got.thread), None)
    }));
    problems.extend(got.off.iter().map(|why| slate_problem("K714", why, None)));
    problems
}

fn server_of(d: &SlateRunDecl) -> Option<String> {
    match d {
        SlateRunDecl::Tool(t) => Some(t.server.clone()),
        SlateRunDecl::Resource(r) => Some(r.server.clone()),
        SlateRunDecl::Cmd(_) => None,
    }
}

fn machine_on_box(runs: &[String], machine: &str, computer: &str) -> String {
    let names = runs.iter().map(|r| format!("${r}")).collect::<Vec<_>>().join(", ");
    let verb = if runs.len() == 1 { "calls" } else { "call" };
    format!(
        "{names} {verb} an MCP server, and this thread runs on {machine}, a machine wsp made on {computer}, whose server would start on {computer} outside it: a slate's tool run goes only to a thread in a folder on {computer} or on a cloud machine, so start the thread in the project's folder on {computer}"
    )
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
    #[serde(
        rename = "outputSchema",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::pending::null_kept::deserialize"
    )]
    /// A JSON null is kept as a value: the TS tests `outputSchema !== undefined`.
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

/// What the person allows of a tool or resource run (spec rule 3).
pub fn code_key(decl: &McpRunDecl, args: &Map<String, Value>) -> String {
    let (kind, server, target, tool_args, then) = match &decl.decl {
        SlateRunDecl::Tool(t) => ("tool", t.server.clone(), t.tool.clone(), Value::Object(args.clone()), t.then.clone()),
        SlateRunDecl::Resource(r) => ("resource", r.server.clone(), r.uri.clone(), Value::Null, r.then.clone()),
        SlateRunDecl::Cmd(c) => ("cmd", String::new(), c.cmd.clone(), Value::Null, c.then.clone()),
    };
    let mut files: Vec<(String, String)> = decl.files.iter().flatten().map(|(k, v)| (k.clone(), v.clone())).collect();
    files.sort_by(|a, b| utf16::cmp(&format!("{},{}", a.0, a.1), &format!("{},{}", b.0, b.1)));
    let files_json = Value::Array(files.into_iter().map(|(name, text)| json!([name, text])).collect());
    let what = json!([kind, server, target, tool_args, then, files_json]);
    let digest = Sha256::digest(crate::slate_runs::stable_json(Some(&what)).as_bytes());
    format!("mcp:{server}#{}", &hex::encode(digest)[..32])
}

/// Where a secret stands in a tool's evaluated arguments (slate-mcp.ts:208).
pub fn slate_secret_mark(name: &str) -> String {
    format!("{SECRET_MARK}{name}\u{0}")
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

/// Nesting past this is refused before any parser sees it: serde_json's own limit is off in this build
/// (`unbounded_depth`, enabled by wsp-engine), so the check is made here.
const PARSE_DEPTH_MAX: usize = 128;
const CLOSED: &str = "the server's connection is closed";
const FETCH_FAILED: &str = "fetch failed";
const TIMED_OUT: &str = "The operation was aborted due to timeout";
const HEAD_MAX: usize = 64 * 1024;

fn depth_words() -> String {
    format!("the server's answer is nested over {DEPTH_MAX} deep")
}

/// Why a parsed answer is too deep or too wide to walk, or nothing (spec rule 6). Iterative, so a deep one cannot take
/// the stack.
pub fn shape_refusal(value: &Value) -> Option<String> {
    let mut stack: Vec<(&Value, usize)> = vec![(value, 0)];
    let mut items = 0usize;
    while let Some((v, depth)) = stack.pop() {
        let children: Vec<&Value> = match v {
            Value::Array(a) => a.iter().collect(),
            Value::Object(m) => m.values().collect(),
            _ => continue,
        };
        if depth >= DEPTH_MAX {
            return Some(depth_words());
        }
        items += children.len();
        if items > ITEMS_MAX {
            return Some(format!("the server's answer holds over {ITEMS_MAX} entries"));
        }
        stack.extend(children.into_iter().map(|c| (c, depth + 1)));
    }
    None
}

/// The deepest nesting of a JSON text's containers, read without parsing it (strings skipped).
fn max_depth(text: &str) -> usize {
    let (mut depth, mut deepest, mut in_string, mut escaped) = (0usize, 0usize, false, false);
    for b in text.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}

/// JSON a tool put in its text, or `None` where it does not parse or is too deep or wide (spec rule 7).
pub fn parsed_text(text: &str) -> Option<Value> {
    if max_depth(text) > DEPTH_MAX {
        return None;
    }
    let value: Value = serde_json::from_str(text).ok()?;
    shape_refusal(&value).is_none().then_some(value)
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

/// The words of a JSON-RPC error answer: `the server answered with error <code or ?>[: <message>]`.
pub fn rpc_failed(error: &Value) -> String {
    let code = match error.get("code") {
        None | Some(Value::Null) => "?".to_string(),
        Some(code) => js_string(code),
    };
    let message = match error.get("message") {
        Some(Value::String(m)) => format!(": {m}"),
        _ => String::new(),
    };
    format!("the server answered with error {code}{message}")
}

/// The server's command on this computer, in a process group of its own (spec rule 9).
pub fn spawn_here() -> McpPipeOpen {
    Rc::new(|t: &McpTransport, spec: &McpServerSpec, on: McpPipeEvents| -> Rc<dyn McpPipe> {
        let McpTransport::Stdio { command, args, env, cwd, .. } = t else { return Rc::new(NoPipe) };
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args)
            .env_clear()
            .envs(&spec.env)
            .envs(env)
            .current_dir(cwd.as_deref().unwrap_or(&spec.cwd))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: between fork and exec the only call is setsid, which is async-signal-safe (as slate_runs' road does).
        unsafe {
            cmd.pre_exec(|| if libc::setsid() == -1 { Err(std::io::Error::last_os_error()) } else { Ok(()) });
        }
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let why = format!("{command} could not be started: {e}");
                spawn_local(async move { (on.exit)(&why) });
                return Rc::new(NoPipe);
            }
        };
        let pid = child.id();
        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        if let Some(mut stdin) = child.stdin.take() {
            spawn_local(async move {
                while let Some(chunk) = stdin_rx.recv().await {
                    if stdin.write_all(&chunk).await.is_err() {
                        break;
                    }
                }
            });
        }
        if let Some(mut stdout) = child.stdout.take() {
            let out = on.out.clone();
            spawn_local(async move {
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    match stdout.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => out(&buf[..n]),
                    }
                }
            });
        }
        if let Some(mut stderr) = child.stderr.take() {
            let err = on.err.clone();
            spawn_local(async move {
                let mut buf = vec![0u8; 64 * 1024];
                let mut pending: Vec<u8> = Vec::new();
                loop {
                    match stderr.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            pending.extend_from_slice(&buf[..n]);
                            let cut = match std::str::from_utf8(&pending) {
                                Ok(_) => pending.len(),
                                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                                Err(_) => pending.len(),
                            };
                            err(&String::from_utf8_lossy(&pending[..cut]));
                            pending.drain(..cut);
                        }
                    }
                }
                if !pending.is_empty() {
                    err(&String::from_utf8_lossy(&pending));
                }
            });
        }
        let exit = on.exit.clone();
        spawn_local(async move {
            let why = match child.wait().await {
                Ok(status) => exit_words(status),
                Err(e) => format!("the server exited: {e}"),
            };
            exit(&why);
        });
        Rc::new(Spawned { stdin: stdin_tx, pid })
    })
}

fn exit_words(status: ExitStatus) -> String {
    if let Some(code) = status.code() {
        format!("the server exited with {code}")
    } else if let Some(signal) = status.signal() {
        format!("the server exited on {}", signal_name(signal))
    } else {
        "the server exited".to_string()
    }
}

fn signal_name(signal: i32) -> String {
    match signal {
        1 => "SIGHUP".into(),
        2 => "SIGINT".into(),
        3 => "SIGQUIT".into(),
        6 => "SIGABRT".into(),
        9 => "SIGKILL".into(),
        11 => "SIGSEGV".into(),
        13 => "SIGPIPE".into(),
        15 => "SIGTERM".into(),
        other => format!("signal {other}"),
    }
}

struct Spawned {
    stdin: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    pid: Option<u32>,
}

impl McpPipe for Spawned {
    fn write(&self, text: &str) {
        let _ = self.stdin.send(text.as_bytes().to_vec());
    }
    fn kill(&self) {
        if let Some(pid) = self.pid {
            crate::slate_runs::kill_group(pid);
        }
    }
    fn pid(&self) -> Option<u32> {
        self.pid
    }
}

struct NoPipe;

impl McpPipe for NoPipe {
    fn write(&self, _text: &str) {}
    fn kill(&self) {}
    fn pid(&self) -> Option<u32> {
        None
    }
}

struct Waiting {
    tx: oneshot::Sender<Result<Value, String>>,
    timer: Cancel,
}

struct StdioInner {
    waiting: RefCell<IndexMap<u64, Waiting>>,
    next: Cell<u64>,
    closed: Cell<bool>,
    said: RefCell<String>,
    buffer: RefCell<Vec<u8>>,
    pipe: RefCell<Option<Rc<dyn McpPipe>>>,
    on_close: RefCell<Option<Box<dyn Fn()>>>,
    clock: Rc<dyn Clock>,
}

struct StdioClient(Rc<StdioInner>);

/// A stdio client; `on_close` runs once whenever it closes (spec rule 8). Deadlines go through `clock`.
pub fn stdio_client(t: &McpTransport, spec: &McpServerSpec, on_close: Box<dyn Fn()>, clock: Rc<dyn Clock>) -> Rc<dyn McpClient> {
    let inner = Rc::new(StdioInner {
        waiting: RefCell::new(IndexMap::new()),
        next: Cell::new(1),
        closed: Cell::new(false),
        said: RefCell::new(String::new()),
        buffer: RefCell::new(Vec::new()),
        pipe: RefCell::new(None),
        on_close: RefCell::new(Some(on_close)),
        clock,
    });
    let weak = Rc::downgrade(&inner);
    let (w_out, w_err, w_exit) = (weak.clone(), weak.clone(), weak);
    let events = McpPipeEvents {
        out: Rc::new(move |chunk: &[u8]| {
            if let Some(s) = w_out.upgrade() {
                s.on_out(chunk)
            }
        }),
        err: Rc::new(move |text: &str| {
            if let Some(s) = w_err.upgrade() {
                s.on_err(text)
            }
        }),
        exit: Rc::new(move |why: &str| {
            if let Some(s) = w_exit.upgrade() {
                s.finish(why)
            }
        }),
    };
    let open = spec.open.clone().unwrap_or_else(spawn_here);
    let pipe = open(t, spec, events);
    *inner.pipe.borrow_mut() = Some(pipe);
    Rc::new(StdioClient(inner))
}

impl StdioInner {
    fn send(&self, msg: String) {
        if self.closed.get() {
            return;
        }
        let pipe = self.pipe.borrow().clone();
        if let Some(pipe) = pipe {
            pipe.write(&format!("{msg}\n"));
        }
    }

    fn finish(&self, why: &str) {
        if self.closed.replace(true) {
            return;
        }
        let waiting = std::mem::take(&mut *self.waiting.borrow_mut());
        for (_, w) in waiting {
            (w.timer)();
            let _ = w.tx.send(Err(why.to_string()));
        }
        let pipe = self.pipe.borrow().clone();
        if let Some(pipe) = pipe {
            pipe.kill();
        }
        let on_close = self.on_close.borrow_mut().take();
        if let Some(on_close) = on_close {
            on_close();
        }
    }

    fn expire(&self, id: u64, why: String) {
        let waiting = self.waiting.borrow_mut().shift_remove(&id);
        if let Some(w) = waiting {
            let _ = w.tx.send(Err(why));
        }
    }

    fn on_err(&self, text: &str) {
        let joined = format!("{}{}", self.said.borrow(), text);
        *self.said.borrow_mut() = utf16::tail(&joined, ERR_KEPT);
    }

    fn on_out(&self, chunk: &[u8]) {
        if self.closed.get() {
            return;
        }
        self.buffer.borrow_mut().extend_from_slice(chunk);
        loop {
            if self.closed.get() {
                return;
            }
            let found = {
                let mut buf = self.buffer.borrow_mut();
                buf.iter().position(|&b| b == b'\n').map(|at| (at, buf.drain(..=at).collect::<Vec<u8>>()))
            };
            let Some((at, line)) = found else { break };
            if at > MESSAGE_MAX_BYTES {
                self.finish(TOO_BIG);
                return;
            }
            let text = String::from_utf8_lossy(&line[..at]);
            let line = js_trim(&text);
            if !line.is_empty() {
                self.handle(line);
            }
        }
        if self.buffer.borrow().len() > MESSAGE_MAX_BYTES {
            self.finish(TOO_BIG);
        }
    }

    fn handle(&self, line: &str) {
        let Some(msg) = read_message(line) else { return };
        if let Some(Value::String(method)) = &msg.method {
            if let Some(id) = &msg.id {
                let reply = if method == "ping" {
                    json!({ "jsonrpc": "2.0", "id": id, "result": {} })
                } else {
                    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "not offered" } })
                };
                self.send(reply.to_string());
            }
            return;
        }
        let Some(id) = msg.id.as_ref().and_then(id_of) else { return };
        let waiting = self.waiting.borrow_mut().shift_remove(&id);
        let Some(w) = waiting else { return };
        (w.timer)();
        let answer = if let Some(error) = &msg.error {
            Err(rpc_failed(error))
        } else if msg.result_deep {
            Err(depth_words())
        } else {
            let result = msg.result.unwrap_or(Value::Null);
            match shape_refusal(&result) {
                Some(bad) => Err(bad),
                None => Ok(result),
            }
        };
        let _ = w.tx.send(answer);
    }
}

impl McpClient for StdioClient {
    fn request(&self, method: &str, params: Map<String, Value>, ms: u64) -> (u64, McpAnswer) {
        let inner = &self.0;
        let id = inner.next.get();
        inner.next.set(id + 1);
        let (tx, rx) = oneshot::channel::<Result<Value, String>>();
        let answer: McpAnswer = async move { rx.await.unwrap_or_else(|_| Err(CLOSED.to_string())) }.boxed_local();
        if inner.closed.get() {
            let _ = tx.send(Err(CLOSED.to_string()));
            return (id, answer);
        }
        let why = format!("the server did not answer {method} within {} s", number_string(js_round(ms as f64 / 1000.0)));
        let weak = Rc::downgrade(inner);
        let timer = inner.clock.schedule(
            Box::new(move || {
                if let Some(s) = weak.upgrade() {
                    s.expire(id, why)
                }
            }),
            ms as i64,
        );
        inner.waiting.borrow_mut().insert(id, Waiting { tx, timer });
        inner.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": Value::Object(params) }).to_string());
        (id, answer)
    }

    fn notify(&self, method: &str, params: Option<Map<String, Value>>) {
        let mut msg = Map::new();
        msg.insert("jsonrpc".into(), "2.0".into());
        msg.insert("method".into(), method.into());
        if let Some(params) = params {
            msg.insert("params".into(), Value::Object(params));
        }
        self.0.send(Value::Object(msg).to_string());
    }

    fn close(&self) {
        self.0.finish("the connection was closed");
    }

    fn closed(&self) -> bool {
        self.0.closed.get()
    }

    fn said(&self) -> String {
        self.0.said.borrow().clone()
    }

    fn pid(&self) -> Option<u32> {
        self.0.pipe.borrow().as_ref().and_then(|p| p.pid())
    }
}

/// One line of a server's output as a message: its method, id, error and result. A part too deep for the parser is
/// marked, not parsed: a result that is so is refused by its depth.
struct Msg {
    method: Option<Value>,
    id: Option<Value>,
    error: Option<Value>,
    result: Option<Value>,
    result_deep: bool,
}

fn read_message(line: &str) -> Option<Msg> {
    if max_depth(line) <= PARSE_DEPTH_MAX {
        return match serde_json::from_str::<Value>(line).ok()? {
            Value::Object(mut m) => Some(Msg {
                method: m.remove("method"),
                id: m.remove("id"),
                error: m.remove("error"),
                result: m.remove("result"),
                result_deep: false,
            }),
            _ => None,
        };
    }
    let mut msg = Msg { method: None, id: None, error: None, result: None, result_deep: false };
    for (key, span) in top_members(line)? {
        let parsed = if max_depth(span) > PARSE_DEPTH_MAX { None } else { Some(serde_json::from_str::<Value>(span).ok()?) };
        match (key.as_str(), parsed) {
            ("method", v) => msg.method = v,
            ("id", v) => msg.id = v,
            ("error", v) => msg.error = Some(v.unwrap_or(Value::Null)),
            ("result", None) => msg.result_deep = true,
            ("result", v) => msg.result = v,
            _ => {}
        }
    }
    Some(msg)
}

/// The top-level members of a JSON object as (key, value text), found without recursion.
fn top_members(line: &str) -> Option<Vec<(String, &str)>> {
    let b = line.as_bytes();
    let mut i = skip_ws(b, 0);
    if *b.get(i)? != b'{' {
        return None;
    }
    i += 1;
    let mut out = Vec::new();
    loop {
        i = skip_ws(b, i);
        match *b.get(i)? {
            b'}' => return Some(out),
            b'"' => {}
            _ => return None,
        }
        let key_end = string_end(b, i)?;
        let key: String = serde_json::from_str(&line[i..key_end]).ok()?;
        i = skip_ws(b, key_end);
        if *b.get(i)? != b':' {
            return None;
        }
        i = skip_ws(b, i + 1);
        let end = value_end(b, i)?;
        out.push((key, &line[i..end]));
        i = skip_ws(b, end);
        match *b.get(i)? {
            b',' => i += 1,
            b'}' => return Some(out),
            _ => return None,
        }
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while matches!(b.get(i), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        i += 1;
    }
    i
}

/// The index just past the string that opens at `i`.
fn string_end(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i + 1;
    loop {
        match *b.get(j)? {
            b'\\' => j += 2,
            b'"' => return Some(j + 1),
            _ => j += 1,
        }
    }
}

/// The index where the value that starts at `i` ends: a `,` or a closing bracket at its own level.
fn value_end(b: &[u8], i: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut j = i;
    loop {
        match *b.get(j)? {
            b'"' => {
                j = string_end(b, j)?;
                continue;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' if depth == 0 => return Some(j),
            b'}' | b']' => depth -= 1,
            b',' if depth == 0 => return Some(j),
            _ => {}
        }
        j += 1;
    }
}

fn id_of(v: &Value) -> Option<u64> {
    let n = v.as_f64()?;
    (n.fract() == 0.0 && n >= 0.0).then_some(n as u64)
}

/// This computer's own fetch: no redirect followed, a failed answer's body never read, a body over 2 MiB cut.
pub fn fetch_here() -> Rc<dyn McpPost> {
    Rc::new(Here)
}

struct Here;

impl McpPost for Here {
    fn send(&self, t: &McpTransport, req: McpHttpRequest) -> LocalBoxFuture<'static, Result<McpHttpAnswer, String>> {
        let url = match t {
            McpTransport::Http { url, .. } => url.clone(),
            McpTransport::Stdio { .. } => String::new(),
        };
        async move { fetch_bounded(url, req).await }.boxed_local()
    }

    fn close(&self) {}
}

/// One exchange, aborted once `ms` pass (AbortSignal.timeout), the timer on the process clock.
async fn fetch_bounded(url: String, req: McpHttpRequest) -> Result<McpHttpAnswer, String> {
    let ms = req.ms;
    let (fire, due) = oneshot::channel::<()>();
    let cancel = real_clock().schedule(
        Box::new(move || {
            let _ = fire.send(());
        }),
        ms as i64,
    );
    let outcome = match select(Box::pin(exchange(url, req)), due).await {
        Either::Left((answer, _)) => answer,
        Either::Right(_) => Err(TIMED_OUT.to_string()),
    };
    cancel();
    outcome
}

async fn exchange(url: String, req: McpHttpRequest) -> Result<McpHttpAnswer, String> {
    let parsed = Url::parse(&url).map_err(|_| FETCH_FAILED.to_string())?;
    match parsed.scheme() {
        "http" => {}
        "https" => return Err("https is not linked into this build".to_string()),
        _ => return Err(FETCH_FAILED.to_string()),
    }
    let host = parsed.host_str().unwrap_or("").to_string();
    let connect_host = host.trim_start_matches('[').trim_end_matches(']').to_string();
    let port = parsed.port_or_known_default().unwrap_or(80);
    let mut target = parsed.path().to_string();
    if let Some(query) = parsed.query() {
        target.push('?');
        target.push_str(query);
    }
    let host_line = match parsed.port() {
        Some(p) => format!("{host}:{p}"),
        None => host,
    };
    let stream = TcpStream::connect((connect_host.as_str(), port)).await.map_err(|_| FETCH_FAILED.to_string())?;
    let (read, mut write) = stream.into_split();
    let mut head = format!("POST {target} HTTP/1.1\r\nHost: {host_line}\r\n");
    for (name, value) in &req.headers {
        if name.contains(['\r', '\n', ':']) || value.contains(['\r', '\n']) {
            return Err(FETCH_FAILED.to_string());
        }
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", req.body.len()));
    write.write_all(head.as_bytes()).await.map_err(|_| FETCH_FAILED.to_string())?;
    write.write_all(req.body.as_bytes()).await.map_err(|_| FETCH_FAILED.to_string())?;
    let mut reader = BufReader::new(read);
    let (status, headers) = read_head(&mut reader).await?;
    let content_type = header_value(&headers, "content-type");
    let session_id = header_value(&headers, "mcp-session-id");
    if matches!(status, 301 | 302 | 303 | 307 | 308) && header_value(&headers, "location").is_some() {
        return Err(FETCH_FAILED.to_string());
    }
    let body = if (200..=299).contains(&status) { read_body(&mut reader, &headers).await? } else { Vec::new() };
    Ok(McpHttpAnswer { status, content_type, session_id, body: String::from_utf8_lossy(&body).into_owned() })
}

async fn read_head(reader: &mut BufReader<OwnedReadHalf>) -> Result<(u16, Vec<(String, String)>), String> {
    let mut line = String::new();
    let mut total = reader.read_line(&mut line).await.map_err(|_| FETCH_FAILED.to_string())?;
    if total == 0 {
        return Err(FETCH_FAILED.to_string());
    }
    let status = line.split_whitespace().nth(1).and_then(|s| s.parse::<u16>().ok()).ok_or_else(|| FETCH_FAILED.to_string())?;
    let mut headers = Vec::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await.map_err(|_| FETCH_FAILED.to_string())?;
        total += n;
        if n == 0 || total > HEAD_MAX {
            return Err(FETCH_FAILED.to_string());
        }
        let text = line.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            break;
        }
        if let Some((name, value)) = text.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    Ok((status, headers))
}

/// A body read no further than one message may be; a declared length over the cap is refused before a byte is read.
async fn read_body(reader: &mut BufReader<OwnedReadHalf>, headers: &[(String, String)]) -> Result<Vec<u8>, String> {
    let io = |_: std::io::Error| FETCH_FAILED.to_string();
    let mut body = Vec::new();
    let chunked = header_value(headers, "transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
    if chunked {
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.map_err(io)? == 0 {
                return Err(FETCH_FAILED.to_string());
            }
            let size = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16).map_err(|_| FETCH_FAILED.to_string())?;
            if size == 0 {
                loop {
                    let mut trailer = String::new();
                    if reader.read_line(&mut trailer).await.map_err(io)? == 0 || trailer.trim_end_matches(['\r', '\n']).is_empty() {
                        break;
                    }
                }
                break;
            }
            if body.len().saturating_add(size) > MESSAGE_MAX_BYTES {
                return Err(TOO_BIG.to_string());
            }
            let start = body.len();
            body.resize(start + size, 0);
            reader.read_exact(&mut body[start..]).await.map_err(io)?;
            let mut crlf = [0u8; 2];
            reader.read_exact(&mut crlf).await.map_err(io)?;
        }
    } else if let Some(len) = header_value(headers, "content-length").and_then(|v| v.parse::<usize>().ok()) {
        if len > MESSAGE_MAX_BYTES {
            return Err(TOO_BIG.to_string());
        }
        body.resize(len, 0);
        reader.read_exact(&mut body).await.map_err(io)?;
    } else {
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            let n = reader.read(&mut chunk).await.map_err(io)?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
            if body.len() > MESSAGE_MAX_BYTES {
                return Err(TOO_BIG.to_string());
            }
        }
    }
    Ok(body)
}

/// `res.headers.get(name)`: case-insensitive, a name sent twice joined with ", ".
fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    let values: Vec<&str> = headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).collect();
    (!values.is_empty()).then(|| values.join(", "))
}

/// Streamable HTTP: one POST per message (spec rule 10).
pub fn http_client(t: &McpTransport, spec: &McpServerSpec, on_close: Box<dyn Fn()>) -> Rc<dyn McpClient> {
    let own = match t {
        McpTransport::Http { headers, .. } => headers.clone(),
        McpTransport::Stdio { .. } => IndexMap::new(),
    };
    let road: Rc<dyn McpPost> = match &spec.post {
        Some(road) => road(fetch_here()),
        None => fetch_here(),
    };
    Rc::new(HttpClient(Rc::new(HttpInner {
        transport: t.clone(),
        own,
        road,
        session: RefCell::new(None),
        next: Cell::new(1),
        closed: Cell::new(false),
        on_close: RefCell::new(Some(on_close)),
    })))
}

struct HttpInner {
    transport: McpTransport,
    own: IndexMap<String, String>,
    road: Rc<dyn McpPost>,
    session: RefCell<Option<String>>,
    next: Cell<u64>,
    closed: Cell<bool>,
    on_close: RefCell<Option<Box<dyn Fn()>>>,
}

struct HttpClient(Rc<HttpInner>);

/// Sets a header, replacing one of the same name in any letter case where it stands (later keys override earlier).
fn set_header(headers: &mut IndexMap<String, String>, name: &str, value: &str) {
    match headers.keys().position(|k| k.eq_ignore_ascii_case(name)) {
        Some(at) => {
            headers.shift_remove_index(at);
            headers.shift_insert(at, name.to_string(), value.to_string());
        }
        None => {
            headers.insert(name.to_string(), value.to_string());
        }
    }
}

async fn post(inner: Rc<HttpInner>, body: String, ms: u64) -> Result<McpHttpAnswer, String> {
    let mut headers = inner.own.clone();
    set_header(&mut headers, "Content-Type", "application/json");
    set_header(&mut headers, "Accept", "application/json, text/event-stream");
    set_header(&mut headers, "MCP-Protocol-Version", PROTOCOL_VERSION);
    let session = inner.session.borrow().clone();
    if let Some(session) = session {
        set_header(&mut headers, "Mcp-Session-Id", &session);
    }
    let road = inner.road.clone();
    let res = road.send(&inner.transport, McpHttpRequest { headers, body, ms }).await?;
    if let Some(session) = res.session_id.clone() {
        *inner.session.borrow_mut() = Some(session);
    }
    Ok(res)
}

fn answer_of(method: &str, id: u64, res: McpHttpAnswer) -> Result<Value, String> {
    if res.status == 401 || res.status == 403 {
        return Err(format!("its address wants a sign-in ({}); sign in where the agent keeps it", res.status));
    }
    if !(200..=299).contains(&res.status) {
        return Err(format!("its address answered {method} with {}", res.status));
    }
    let event_stream = res.content_type.as_deref().unwrap_or("").contains("event-stream");
    let lines: Vec<String> = if event_stream {
        res.body.split('\n').filter(|l| l.starts_with("data:")).map(|l| js_trim(&l[5..]).to_string()).collect()
    } else {
        vec![res.body]
    };
    for line in lines {
        let Some(msg) = read_message(&line) else { continue };
        if msg.id.as_ref().and_then(id_of) != Some(id) {
            continue;
        }
        if let Some(error) = &msg.error {
            return Err(rpc_failed(error));
        }
        if msg.result_deep {
            return Err(depth_words());
        }
        let result = msg.result.unwrap_or(Value::Null);
        if let Some(bad) = shape_refusal(&result) {
            return Err(bad);
        }
        return Ok(result);
    }
    Err(format!("its answer to {method} could not be read"))
}

impl McpClient for HttpClient {
    fn request(&self, method: &str, params: Map<String, Value>, ms: u64) -> (u64, McpAnswer) {
        let inner = self.0.clone();
        let id = inner.next.get();
        inner.next.set(id + 1);
        let method = method.to_string();
        let answer = async move {
            if inner.closed.get() {
                return Err(CLOSED.to_string());
            }
            let body = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": Value::Object(params) }).to_string();
            let res = post(inner.clone(), body, ms).await?;
            answer_of(&method, id, res)
        }
        .boxed_local();
        (id, answer)
    }

    fn notify(&self, method: &str, params: Option<Map<String, Value>>) {
        if self.0.closed.get() {
            return;
        }
        let mut msg = Map::new();
        msg.insert("jsonrpc".into(), "2.0".into());
        msg.insert("method".into(), method.into());
        if let Some(params) = params {
            msg.insert("params".into(), Value::Object(params));
        }
        let body = Value::Object(msg).to_string();
        let inner = self.0.clone();
        spawn_local(async move {
            let _ = post(inner, body, CONNECT_MS).await;
        });
    }

    fn close(&self) {
        if self.0.closed.replace(true) {
            return;
        }
        self.0.road.close();
        let on_close = self.0.on_close.borrow_mut().take();
        if let Some(on_close) = on_close {
            on_close();
        }
    }

    fn closed(&self) -> bool {
        self.0.closed.get()
    }

    fn said(&self) -> String {
        String::new()
    }

    fn pid(&self) -> Option<u32> {
        None
    }
}

/// The schema folded to lines (spec rule 12): a sentence cut at its first full stop, `!` or `?` followed by a capital or
/// an open bracket, or at `max` characters with `…`.
pub fn sentence(text: Option<&Value>, max: usize) -> Option<String> {
    let Some(Value::String(text)) = text else { return None };
    let one = js_trim(&js_collapse_whitespace(text)).to_string();
    if one.is_empty() {
        return None;
    }
    let cut = first_sentence(&one);
    if utf16::len(cut) > max {
        Some(format!("{}…", utf16::cut(cut, max.saturating_sub(1))))
    } else {
        Some(cut.to_string())
    }
}

fn first_sentence(one: &str) -> &str {
    for (i, c) in one.char_indices() {
        if !matches!(c, '.' | '!' | '?') {
            continue;
        }
        let end = i + 1;
        let rest = &one[end..];
        if rest.is_empty() {
            return &one[..end];
        }
        if rest.starts_with(is_js_whitespace)
            && rest.trim_start_matches(is_js_whitespace).starts_with(|c: char| c.is_ascii_uppercase() || c == '(')
        {
            return &one[..end];
        }
    }
    one
}

fn is_object_like(v: &Value) -> bool {
    matches!(v, Value::Object(_) | Value::Array(_))
}

/// `Object.assign({}, base, over)` without the key `drop` of `over`.
fn merged(base: &Value, over: &Value, drop: &str) -> Value {
    let mut out = Map::new();
    if let Value::Object(b) = base {
        for (k, v) in b {
            out.insert(k.clone(), v.clone());
        }
    }
    if let Value::Object(o) = over {
        for (k, v) in o {
            if k != drop {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    Value::Object(out)
}

/// A local `$ref` followed, and an `allOf` of one schema taken as that schema, as pydantic writes a model field.
fn deref(p: &Value, root: &Value, seen: u32) -> Value {
    if let Some(Value::String(reference)) = p.get("$ref") {
        if let Some(path) = reference.strip_prefix("#/") {
            if seen < 8 {
                let mut at: Option<&Value> = Some(root);
                for seg in path.split('/') {
                    let key = seg.replace("~1", "/").replace("~0", "~");
                    at = at.and_then(|v| v.get(key.as_str()));
                }
                if let Some(target) = at.filter(|t| t.is_object()) {
                    return deref(&merged(target, p, "$ref"), root, seen + 1);
                }
            }
        }
    }
    if let Some(Value::Array(all)) = p.get("allOf") {
        if all.len() == 1 && all[0].is_object() {
            return deref(&merged(&all[0], p, "allOf"), root, seen + 1);
        }
    }
    p.clone()
}

/// `JSON.stringify` of a value: keys in their order, numbers as JavaScript writes them.
fn js_json(v: &Value) -> String {
    match v {
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::Array(items) => format!("[{}]", items.iter().map(js_json).collect::<Vec<_>>().join(",")),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter().map(|(k, v)| format!("{}:{}", js_json(&Value::String(k.clone())), js_json(v))).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}

/// `String(v)` of a value.
fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(items) => items.iter().map(|x| if x.is_null() { String::new() } else { js_string(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn type_of(raw: &Value, root: &Value) -> String {
    let p = deref(raw, root, 0);
    if let Some(Value::Array(values)) = p.get("enum") {
        return values.iter().map(js_json).collect::<Vec<_>>().join(" | ");
    }
    if let Some(c) = p.get("const") {
        return js_json(c);
    }
    let named = match p.get("type") {
        Some(Value::String(t)) => Some(t.clone()),
        Some(Value::Array(types)) => Some(types.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" | ")),
        _ => None,
    };
    if named.as_deref() == Some("array") {
        return match p.get("items") {
            Some(items) if is_object_like(items) => format!("{}[]", type_of(items, root)),
            _ => "array".to_string(),
        };
    }
    if let Some(named) = named {
        if !named.is_empty() {
            return named;
        }
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(Value::Array(options)) = p.get(key) {
            return options.iter().map(|o| type_of(o, root)).collect::<Vec<_>>().join(" | ");
        }
    }
    "any".to_string()
}

/// One line per property, nested objects (and objects inside arrays) as dotted names, three deep.
pub fn fold_schema(schema: &Value) -> Vec<String> {
    fold(schema, "", 0, schema)
}

fn fold(schema: &Value, prefix: &str, depth: u32, root: &Value) -> Vec<String> {
    if !is_object_like(schema) {
        return Vec::new();
    }
    let p = deref(schema, root, 0);
    let Some(Value::Object(properties)) = p.get("properties") else { return Vec::new() };
    let needed: Vec<&str> = match p.get("required") {
        Some(Value::Array(r)) => r.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    let mut lines = Vec::new();
    for (name, raw) in properties {
        if !is_object_like(raw) {
            continue;
        }
        let prop = deref(raw, root, 0);
        let mut facts = vec![type_of(&prop, root)];
        if needed.contains(&name.as_str()) {
            facts.push("required".to_string());
        }
        if let Some(default) = prop.get("default") {
            facts.push(format!("default {}", js_json(default)));
        }
        for (key, word) in [("minimum", "min"), ("maximum", "max"), ("maxLength", "max length"), ("format", "format")] {
            if let Some(v) = prop.get(key) {
                facts.push(format!("{word} {}", js_string(v)));
            }
        }
        let about = sentence(prop.get("description"), 160).map(|a| format!(". {a}")).unwrap_or_default();
        lines.push(format!("  {prefix}{name}: {}{about}", facts.join(", ")));
        if depth >= 2 {
            continue;
        }
        if prop.get("properties").is_some() {
            lines.extend(fold(&prop, &format!("{prefix}{name}."), depth + 1, root));
        }
        if let Some(items) = prop.get("items").filter(|i| is_object_like(i)).map(|i| deref(i, root, 0)) {
            if items.get("properties").is_some() {
                lines.extend(fold(&items, &format!("{prefix}{name}[]."), depth + 1, root));
            }
        }
    }
    lines
}

/// What a tool says of itself to the agent: whether it returns data, and each hint that is exactly `true`.
pub fn hints_of(tool: &McpToolInfo, structured: Option<bool>) -> Vec<String> {
    let shape = if tool.output_schema.is_some() || structured == Some(true) { "returns data" } else { "text only" };
    let Some(a) = &tool.annotations else { return vec![shape.to_string()] };
    let truthy = |h: &Option<Value>| h.as_ref() == Some(&Value::Bool(true));
    let mut hints = vec![shape.to_string()];
    if truthy(&a.read_only_hint) {
        hints.push("read-only".to_string());
    }
    if truthy(&a.destructive_hint) {
        hints.push("destructive".to_string());
    }
    if truthy(&a.idempotent_hint) {
        hints.push("idempotent".to_string());
    }
    if truthy(&a.open_world_hint) {
        hints.push("reaches outside".to_string());
    }
    hints
}

/// How the thread's agent asks before an MCP tool, as slate_catalog tells the agent (slate-mcp.ts:281).
pub fn asks_line(asks: ToolAsks) -> &'static str {
    match asks {
        ToolAsks::Never => "This thread's agent runs every tool without asking, and so do its slate's runs.",
        ToolAsks::Every => "This thread's agent asks before every tool, so the person allows each run's code once, on the slate: new arguments, a new then or a new tool ask again.",
        ToolAsks::UnlessReadOnly => "This thread's agent asks before a tool not marked read-only, so the person allows each such run's code once, on the slate: new arguments, a new then or a new tool ask again.",
    }
}
