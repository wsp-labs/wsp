// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-mcp.ts:558-1045 (`createSlateMcp`) at 65f6b89c7. Spec: spec/slate_mcp_runs.md.
//!
//! One live connection per thread and server, the tool and resource runs, when to ask the person, the start budgets,
//! scrubbing, the idle sweep, `slate_catalog <server>` and the K713 refusal of a write. One thread owns all of it
//! (rule 3.1); a borrow of the state is never held across an await or a call out (rule 3.3).

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;
use std::sync::OnceLock;

use futures::channel::oneshot;
use futures::future::{join_all, select, Either, LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use regex::{Captures, Regex};
use serde_json::{json, Map, Value};
use tokio::task::spawn_local;

use wsp_core::clock::{Cancel, Clock};
use wsp_wire::types::run_block::RUN_OUTPUT_MAX_CHARS;
use wsp_words::js::{js_trim, utf16};
use wsp_words::run_block::run_output_tail;

use super::client::{
    asks_line, code_key, fold_schema, hints_of, http_client, parsed_text, sentence, stdio_client, McpClient, McpRunDecl, McpServerSpec,
    McpToolAnnotations, McpToolInfo, McpTransport, ToolAsks, CALL_MS, CONNECT_MS, DESCRIBE_MS, IDLE_MS, MARKED, PROTOCOL_VERSION, SWEEP_MS,
};
use crate::pending::slate::problems::{slate_description_missing, slate_problem};
use crate::pending::slate::{SlateDoc, SlateProblem, SlateRunDecl};
use crate::pending::wire::SlateAsk;
use crate::slate_runs::{
    last_result, map_strings, running_record, spent, stable_json, Outcome, Reshape, ReshapeAnswer, RunApprovals, RunBy, RunRecord,
    RunResult, RunStartAnswer, RunState, HELD_APPROVAL, HELD_BUDGET, HELD_PRESSED, SECRET_DOTS,
};

/// Which approval scope the person chose: `Always` is the TS's "thread".
pub use crate::slate_runs::ApproveScope;

/// A start of a tool or resource run (slate-mcp.ts:127).
#[derive(Clone)]
pub struct McpRunStart {
    pub thread_id: String,
    pub run: String,
    pub decl: McpRunDecl,
    pub by: RunBy,
    /// The tool's arguments evaluated now, a secret as `slate_secret_mark(name)` wherever it stands.
    pub args: Rc<dyn Fn() -> Map<String, Value>>,
    /// The run's record before this start, whose result stays in view while this one runs.
    pub last: Option<RunRecord>,
}

/// The secrets a run reads (slate-mcp.ts:150).
pub trait McpSecrets {
    fn plaintext(&self, thread_id: &str, name: &str) -> Option<String>;
    fn scrub(&self, thread_id: &str, text: &str) -> String;
}

/// A release of a process named the thread's.
pub type Release = Box<dyn FnOnce()>;

/// What `create_slate_mcp` is handed (slate-mcp.ts:142).
#[derive(Clone)]
pub struct SlateMcpDeps {
    /// The thread's agent's server by name, resolved; fails with the reason where there is none.
    pub server: Rc<dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<McpServerSpec, String>>>,
    pub on_record: Rc<dyn Fn(&str, &str, &RunRecord)>,
    /// The thread's held asks changed after the fact.
    pub on_asks: Option<Rc<dyn Fn(&str)>>,
    pub asks: Rc<dyn Fn(&str) -> ToolAsks>,
    pub approvals: Rc<dyn RunApprovals>,
    pub secrets: Rc<dyn McpSecrets>,
    /// A run's `then` on this computer, in the thread's folder, the raw result already scrubbed on its stdin.
    pub reshape: Rc<dyn Fn(&str, &str, &str) -> Reshape>,
    pub computer: Rc<dyn Fn(&str) -> String>,
    /// Names a stdio server's process as the thread's tool server until the returned release.
    pub own: Option<Rc<dyn Fn(&str, u32, &str) -> Release>>,
    /// Every time read and timer (rule 3.15); the TS's `now?`.
    pub clock: Rc<dyn Clock>,
    /// How long a connection lives unused while its slate is hidden and no `always` timer keeps it.
    pub idle_ms: Option<u64>,
    /// A call's deadline where the server's config names none.
    pub call_ms: Option<u64>,
}

/// How `stop_all` ends the runs (slate-mcp.ts:179).
#[derive(Debug, Clone, Default)]
pub struct StopAll {
    pub quiet: bool,
    pub why: Option<String>,
}

/// The run a held key asks for, and what it sends (slate-mcp.ts:175).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedAsk {
    pub run: String,
    pub cmd: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeldAsk {
    Code,
    Budget,
    Pressed,
}

#[derive(Clone)]
struct Held {
    req: McpRunStart,
    ask: HeldAsk,
    key: Option<String>,
}

struct Conn {
    client: Rc<dyn McpClient>,
    spec: McpServerSpec,
    resources: bool,
    tools: Option<SharedTools>,
    used_at: i64,
    busy: u32,
}

type ConnRc = Rc<RefCell<Conn>>;
type SharedConn = Shared<LocalBoxFuture<'static, Result<ConnRc, String>>>;
type SharedTools = Shared<LocalBoxFuture<'static, Result<Vec<McpToolInfo>, String>>>;

struct ConnEntry {
    gen: u64,
    fut: SharedConn,
}

struct Live {
    record: RunRecord,
    gen: u64,
    /// The call in flight, for a cancel to tell the server.
    inflight: Option<(Rc<dyn McpClient>, u64)>,
    /// Kills the `then` in flight.
    reshaping: Option<Rc<dyn Fn()>>,
    /// The last call's result, kept in view while the next one runs and through a hold.
    result: Option<RunResult>,
    /// A held start, its arguments read once so the person allows exactly what then goes, and the key it asks by.
    held: Option<Held>,
    starts: Vec<i64>,
    /// The person's own starts, on a budget of their own.
    pressed: Vec<i64>,
}

#[derive(Default)]
struct ThreadMcp {
    runs: IndexMap<String, Rc<RefCell<Live>>>,
    conns: IndexMap<String, ConnEntry>,
    named: IndexSet<String>,
    kept: IndexSet<String>,
    shown: bool,
    pending: Vec<(u64, Shared<LocalBoxFuture<'static, ()>>)>,
}

#[derive(Default)]
struct State {
    threads: IndexMap<String, ThreadMcp>,
    /// Tool lists by thread and server, kept past a closed connection so a sheet and a provisional start can read them.
    lists: HashMap<(String, String), Vec<McpToolInfo>>,
    /// Whether a tool's answers carried a structured result, by thread, server and tool.
    seen: HashMap<(String, String, String), bool>,
    sweep: Option<Cancel>,
    next: u64,
}

/// slate-mcp.ts:163 SlateMcp, made by `create_slate_mcp`.
pub struct SlateMcp {
    deps: SlateMcpDeps,
    state: RefCell<State>,
}

/// Starts the idle sweep on the clock; needs a `LocalSet` for the runs' work.
pub fn create_slate_mcp(deps: SlateMcpDeps) -> Rc<SlateMcp> {
    let mcp = Rc::new(SlateMcp { deps, state: RefCell::new(State::default()) });
    arm_sweep(&mcp);
    mcp
}

fn arm_sweep(mcp: &Rc<SlateMcp>) {
    let every = SWEEP_MS.min(mcp.idle_ms());
    let weak = Rc::downgrade(mcp);
    let cancel = mcp.deps.clock.schedule(
        Box::new(move || {
            if let Some(m) = weak.upgrade() {
                m.sweep();
                arm_sweep(&m);
            }
        }),
        every as i64,
    );
    mcp.state.borrow_mut().sweep = Some(cancel);
}

fn marked() -> &'static Regex {
    static MARK: OnceLock<Regex> = OnceLock::new();
    MARK.get_or_init(|| Regex::new(MARKED).expect("the mark pattern is valid"))
}

fn map_of(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn record_of(state: &str, fields: Vec<(&str, Value)>) -> RunRecord {
    let mut out = Map::new();
    out.insert("state".into(), state.into());
    for (k, v) in fields {
        out.insert(k.into(), v);
    }
    RunRecord(out)
}

fn failed_record(why: String, started_at: i64, ended_at: i64, runs: i64) -> RunRecord {
    record_of(
        "failed",
        vec![
            ("why", why.into()),
            ("exit", Value::Null),
            ("startedAt", started_at.into()),
            ("endedAt", ended_at.into()),
            ("ms", (ended_at - started_at).into()),
            ("runs", runs.into()),
        ],
    )
}

fn answer(outcome: Outcome, record: RunRecord) -> RunStartAnswer {
    RunStartAnswer { outcome, record, ask: None }
}

fn list_key(thread_id: &str, server: &str) -> (String, String) {
    (thread_id.to_string(), server.to_string())
}

fn seen_key(thread_id: &str, server: &str, tool: &str) -> (String, String, String) {
    (thread_id.to_string(), server.to_string(), tool.to_string())
}

fn thread_entry<'a>(st: &'a mut State, thread_id: &str) -> &'a mut ThreadMcp {
    st.threads.entry(thread_id.to_string()).or_default()
}

fn decl_server(d: &McpRunDecl) -> String {
    match &d.decl {
        SlateRunDecl::Tool(t) => t.server.clone(),
        SlateRunDecl::Resource(r) => r.server.clone(),
        SlateRunDecl::Cmd(_) => String::new(),
    }
}

fn decl_then(d: &McpRunDecl) -> Option<String> {
    match &d.decl {
        SlateRunDecl::Tool(t) => t.then.clone(),
        SlateRunDecl::Resource(r) => r.then.clone(),
        SlateRunDecl::Cmd(_) => None,
    }
}

fn decl_description(d: &McpRunDecl) -> Option<String> {
    match &d.decl {
        SlateRunDecl::Tool(t) => t.description.clone(),
        SlateRunDecl::Resource(r) => r.description.clone(),
        SlateRunDecl::Cmd(_) => None,
    }
}

fn tool_of(raw: &Value) -> Option<McpToolInfo> {
    let name = raw.get("name")?.as_str()?.to_string();
    let annotations = raw.get("annotations").filter(|a| a.is_object()).map(|a| McpToolAnnotations {
        title: a.get("title").and_then(Value::as_str).map(String::from),
        read_only_hint: a.get("readOnlyHint").cloned(),
        destructive_hint: a.get("destructiveHint").cloned(),
        idempotent_hint: a.get("idempotentHint").cloned(),
        open_world_hint: a.get("openWorldHint").cloned(),
    });
    Some(McpToolInfo {
        name,
        title: raw.get("title").and_then(Value::as_str).map(String::from),
        description: raw.get("description").cloned(),
        input_schema: raw.get("inputSchema").cloned(),
        output_schema: raw.get("outputSchema").cloned(),
        annotations,
    })
}

fn is_true(v: &Option<Value>) -> bool {
    v.as_ref() == Some(&Value::Bool(true))
}

/// Resolves after `fut` or `ms` on `clock`, whichever first; `None` where the deadline came first.
async fn within<F: Future>(clock: Rc<dyn Clock>, ms: u64, fut: F) -> Option<F::Output> {
    let (fire, due) = oneshot::channel::<()>();
    let cancel = clock.schedule(
        Box::new(move || {
            let _ = fire.send(());
        }),
        ms as i64,
    );
    let out = match select(Box::pin(fut), due).await {
        Either::Left((value, _)) => Some(value),
        Either::Right(_) => None,
    };
    cancel();
    out
}

/// The close of a connection once it is up: a connection that never came up has nothing to close.
fn close_conn(fut: SharedConn) {
    spawn_local(async move {
        if let Ok(conn) = fut.await {
            let client = conn.borrow().client.clone();
            client.close();
        }
    });
}

/// A tracked job: its result is shared, and the thread's `pending` drops it once it ends (spec rule 18).
fn track<T: Clone + 'static>(this: &Rc<SlateMcp>, thread_id: &str, body: LocalBoxFuture<'static, T>) -> Shared<LocalBoxFuture<'static, T>> {
    let id = this.fresh();
    let weak = Rc::downgrade(this);
    let thread = thread_id.to_string();
    let job = async move {
        let out = body.await;
        if let Some(m) = weak.upgrade() {
            m.untrack(&thread, id);
        }
        out
    }
    .boxed_local()
    .shared();
    let settled = job.clone().map(|_| ()).boxed_local().shared();
    thread_entry(&mut this.state.borrow_mut(), thread_id).pending.push((id, settled));
    spawn_local(job.clone().map(|_| ()));
    job
}

/// The connection of a thread and server, shared by every caller; made once and kept until it closes (spec rule 3).
fn connect(this: &Rc<SlateMcp>, thread_id: &str, server: &str) -> SharedConn {
    let existing = this.state.borrow().threads.get(thread_id).and_then(|t| t.conns.get(server).map(|e| e.fut.clone()));
    if let Some(fut) = existing {
        return fut;
    }
    let gen = this.fresh();
    let body: LocalBoxFuture<'static, Result<ConnRc, String>> =
        open_conn(this.clone(), thread_id.to_string(), server.to_string(), gen).boxed_local();
    let shared = body.shared();
    thread_entry(&mut this.state.borrow_mut(), thread_id).conns.insert(server.to_string(), ConnEntry { gen, fut: shared.clone() });
    shared
}

async fn open_conn(this: Rc<SlateMcp>, thread_id: String, server: String, gen: u64) -> Result<ConnRc, String> {
    let spec = match (this.deps.server)(&thread_id, &server).await {
        Ok(spec) => spec,
        Err(e) => {
            this.forget_conn(&thread_id, &server, gen);
            return Err(e);
        }
    };
    let release: Rc<RefCell<Option<Release>>> = Rc::new(RefCell::new(None));
    let gone: Box<dyn Fn()> = {
        let (weak, t, s, release) = (Rc::downgrade(&this), thread_id.clone(), server.clone(), release.clone());
        Box::new(move || {
            let taken = release.borrow_mut().take();
            if let Some(taken) = taken {
                taken();
            }
            if let Some(m) = weak.upgrade() {
                m.forget_conn(&t, &s, gen);
            }
        })
    };
    let client: Rc<dyn McpClient> = match &spec.transport {
        McpTransport::Stdio { .. } => stdio_client(&spec.transport, &spec, gone, this.deps.clock.clone()),
        McpTransport::Http { .. } => http_client(&spec.transport, &spec, gone),
    };
    if !client.closed() {
        if let (Some(pid), Some(own)) = (client.pid(), this.deps.own.clone()) {
            let named = own(&thread_id, pid, "tool");
            *release.borrow_mut() = Some(named);
        }
    }
    let (_, initialized) = client.request(
        "initialize",
        map_of(json!({ "protocolVersion": PROTOCOL_VERSION, "capabilities": {}, "clientInfo": { "name": "wsp", "version": "1" } })),
        CONNECT_MS,
    );
    let init = match initialized.await {
        Ok(init) => init,
        Err(e) => {
            let said = client.said();
            let said = js_trim(&said).rsplit('\n').next().unwrap_or("").to_string();
            client.close();
            let mut msg = format!("{server} did not start: {e}");
            if !said.is_empty() {
                msg.push_str(&format!("; it said: {said}"));
            }
            return Err(this.clean(&thread_id, Some(&spec), &msg));
        }
    };
    client.notify("notifications/initialized", None);
    let resources = init.get("capabilities").and_then(|c| c.get("resources")).is_some();
    let used_at = this.now();
    Ok(Rc::new(RefCell::new(Conn { client, spec, resources, tools: None, used_at, busy: 0 })))
}

/// The tool list of a connection, asked once and kept with it (spec rule 4).
fn tools_of(this: &Rc<SlateMcp>, thread_id: &str, server: &str, conn: &ConnRc) -> SharedTools {
    let known = conn.borrow().tools.clone();
    if let Some(tools) = known {
        return tools;
    }
    let client = conn.borrow().client.clone();
    let (weak, held, key) = (Rc::downgrade(this), conn.clone(), list_key(thread_id, server));
    let body: LocalBoxFuture<'static, Result<Vec<McpToolInfo>, String>> = async move {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..20 {
            let mut params = Map::new();
            if let Some(c) = &cursor {
                params.insert("cursor".into(), c.clone().into());
            }
            let (_, asked) = client.request("tools/list", params, CONNECT_MS);
            let got = match asked.await {
                Ok(got) => got,
                Err(e) => {
                    held.borrow_mut().tools = None;
                    return Err(e);
                }
            };
            if let Some(list) = got.get("tools").and_then(Value::as_array) {
                tools.extend(list.iter().filter_map(tool_of));
            }
            match got.get("nextCursor").and_then(Value::as_str) {
                Some(next) => cursor = Some(next.to_string()),
                None => break,
            }
        }
        if let Some(m) = weak.upgrade() {
            m.state.borrow_mut().lists.insert(key, tools.clone());
        }
        Ok(tools)
    }
    .boxed_local();
    let shared = body.shared();
    conn.borrow_mut().tools = Some(shared.clone());
    shared
}

/// A server's tool list, connecting for it; a failure is the list held, else empty: the call itself says why. Tracked.
fn list_for(this: &Rc<SlateMcp>, thread_id: &str, server: &str) -> Shared<LocalBoxFuture<'static, Vec<McpToolInfo>>> {
    let me = this.clone();
    let (t, s) = (thread_id.to_string(), server.to_string());
    let body: LocalBoxFuture<'static, Vec<McpToolInfo>> = async move {
        let conn = connect(&me, &t, &s).await;
        let listed = match conn {
            Ok(conn) => tools_of(&me, &t, &s, &conn).await,
            Err(e) => Err(e),
        };
        match listed {
            Ok(tools) => tools,
            Err(_) => {
                let held = me.state.borrow().lists.get(&list_key(&t, &s)).cloned();
                held.unwrap_or_default()
            }
        }
    }
    .boxed_local();
    track(this, thread_id, body)
}

/// `text` with each secret mark replaced by what `f` says of it.
fn mark_text(text: &str, f: &dyn Fn(&Captures) -> String) -> String {
    marked().replace_all(text, |caps: &Captures| f(caps)).into_owned()
}

/// The text parts of a call's content, in order.
fn text_of_content(got: &Value) -> Vec<String> {
    got.get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|c| c.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|c| c.get("text").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn hold(this: &Rc<SlateMcp>, req: &McpRunStart, l: &Rc<RefCell<Live>>, ask: HeldAsk, runs: i64, beside: bool) -> RunStartAnswer {
    let key = (ask == HeldAsk::Code).then(|| code_key(&req.decl, &(req.args)()));
    l.borrow_mut().held = Some(Held { req: req.clone(), ask, key });
    let record = if beside {
        l.borrow().record.clone()
    } else {
        let mut fields = l.borrow().result.clone().unwrap_or_default();
        let why = match ask {
            HeldAsk::Code => HELD_APPROVAL,
            HeldAsk::Pressed => HELD_PRESSED,
            HeldAsk::Budget => HELD_BUDGET,
        };
        fields.insert("state".into(), "held".into());
        fields.insert("why".into(), why.into());
        fields.insert("runs".into(), runs.into());
        this.write(&req.thread_id, &req.run, l, RunRecord(fields))
    };
    let server = decl_server(&req.decl);
    if ask == HeldAsk::Code && !this.state.borrow().lists.contains_key(&list_key(&req.thread_id, &server)) {
        let listing = list_for(this, &req.thread_id, &server);
        let (me, thread) = (this.clone(), req.thread_id.clone());
        spawn_local(async move {
            listing.await;
            if let Some(on_asks) = me.deps.on_asks.clone() {
                on_asks(&thread);
            }
        });
    }
    answer(Outcome::Held, record)
}

/// The record a call's answer gives, and the raw result a `then` reads: the structured result as JSON, else the text.
fn result_of(this: &Rc<SlateMcp>, thread_id: &str, spec: &McpServerSpec, decl: &McpRunDecl, got: &Value) -> (Map<String, Value>, String) {
    let scrub = |text: &str| this.clean(thread_id, Some(spec), text);
    if let SlateRunDecl::Resource(_) = &decl.decl {
        let texts: Vec<&str> = got
            .get("contents")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(|c| c.get("text").and_then(Value::as_str)).collect())
            .unwrap_or_default();
        let out = scrub(&texts.join("\n"));
        let json = (texts.len() == 1).then(|| parsed_text(texts[0])).flatten().map(|v| map_strings(&v, &|s: &str| scrub(s)));
        let mut record = Map::new();
        record.insert("state".into(), "done".into());
        record.insert("exit".into(), 0.into());
        record.insert("out".into(), run_output_tail(&out, RUN_OUTPUT_MAX_CHARS as usize).into());
        if let Some(json) = json {
            record.insert("json".into(), json);
        }
        return (record, out);
    }
    let texts = text_of_content(got);
    let text = scrub(&texts.join("\n"));
    if got.get("isError") == Some(&Value::Bool(true)) {
        let mut record = Map::new();
        record.insert("state".into(), "failed".into());
        record.insert("why".into(), "the tool said it failed".into());
        record.insert("exit".into(), 1.into());
        record.insert("out".into(), "".into());
        record.insert("err".into(), run_output_tail(&text, RUN_OUTPUT_MAX_CHARS as usize).into());
        return (record, text);
    }
    let structured_value = got.get("structuredContent").filter(|v| !v.is_null());
    let structured = structured_value.is_some();
    if let SlateRunDecl::Tool(t) = &decl.decl {
        let key = seen_key(thread_id, &t.server, &t.tool);
        let prior = this.state.borrow().seen.get(&key).copied().unwrap_or(false);
        this.state.borrow_mut().seen.insert(key, prior || structured);
    }
    let parsed: Option<Value> = match structured_value {
        Some(v) => Some(v.clone()),
        None if !texts.is_empty() => parsed_text(&texts.join("\n")),
        None => None,
    };
    let json = parsed.map(|v| map_strings(&v, &|s: &str| scrub(s)));
    let mut record = Map::new();
    record.insert("state".into(), "done".into());
    record.insert("exit".into(), 0.into());
    record.insert("out".into(), run_output_tail(&text, RUN_OUTPUT_MAX_CHARS as usize).into());
    let raw = match &json {
        Some(json) if structured => json.to_string(),
        _ => text.clone(),
    };
    if let Some(json) = json {
        record.insert("json".into(), json);
    }
    if !structured {
        record.insert("text".into(), Value::Bool(true));
    }
    (record, raw)
}

/// The run goes running now and its call follows; a tool found on the way to be one that asks holds it for the person.
fn launch(this: &Rc<SlateMcp>, req: McpRunStart, l: Rc<RefCell<Live>>, confirmed: bool) -> RunStartAnswer {
    let (thread_id, run) = (req.thread_id.clone(), req.run.clone());
    let running = l.borrow().record.state() == Some(RunState::Running);
    if running {
        this.stop(&thread_id, &run, &l, None);
    }
    let gen = {
        let mut live = l.borrow_mut();
        live.gen += 1;
        live.gen
    };
    let runs = l.borrow().record.runs() + 1;
    let started_at = this.now();
    let result = l.borrow().result.clone();
    let record = this.write(&thread_id, &run, &l, running_record(result.as_ref(), runs, started_at));
    let body: LocalBoxFuture<'static, ()> = {
        let (this, l, req) = (this.clone(), l.clone(), req.clone());
        async move {
            let mut spec = None;
            if let Err(why) = run_work(&this, &req, &l, gen, runs, started_at, confirmed, &mut spec).await {
                let current = l.borrow().gen == gen;
                if current {
                    l.borrow_mut().inflight = None;
                    let ended_at = this.now();
                    let why = this.clean(&req.thread_id, spec.as_ref(), &why);
                    this.write(&req.thread_id, &req.run, &l, failed_record(why, started_at, ended_at, runs));
                }
            }
        }
        .boxed_local()
    };
    let _ = track(this, &thread_id, body);
    answer(Outcome::Running, record)
}

async fn run_work(
    this: &Rc<SlateMcp>,
    req: &McpRunStart,
    l: &Rc<RefCell<Live>>,
    gen: u64,
    runs: i64,
    started_at: i64,
    confirmed: bool,
    spec_out: &mut Option<McpServerSpec>,
) -> Result<(), String> {
    let conn = connect(this, &req.thread_id, &decl_server(&req.decl)).await?;
    let spec = conn.borrow().spec.clone();
    *spec_out = Some(spec);
    if l.borrow().gen != gen {
        return Ok(());
    }
    conn.borrow_mut().busy += 1;
    let outcome = call(this, req, l, gen, runs, started_at, confirmed, &conn).await;
    conn.borrow_mut().busy -= 1;
    let used_at = this.now();
    conn.borrow_mut().used_at = used_at;
    outcome
}

async fn call(
    this: &Rc<SlateMcp>,
    req: &McpRunStart,
    l: &Rc<RefCell<Live>>,
    gen: u64,
    runs: i64,
    started_at: i64,
    confirmed: bool,
    conn: &ConnRc,
) -> Result<(), String> {
    let decl = &req.decl;
    let server = decl_server(decl);
    let (client, spec) = {
        let c = conn.borrow();
        (c.client.clone(), c.spec.clone())
    };
    let (method, params) = match &decl.decl {
        SlateRunDecl::Tool(t) => {
            let tools = tools_of(this, &req.thread_id, &server, conn).await?;
            if l.borrow().gen != gen {
                return Ok(());
            }
            let info = tools.iter().find(|x| x.name == t.tool).cloned();
            let Some(info) = info else {
                return Err(format!("{server} has no tool {}; slate_catalog {server} lists its tools", t.tool));
            };
            let args = (req.args)();
            if !confirmed && this.asks_now(&req.thread_id, decl, Some(&info), &args) == Some(true) {
                let _ = hold(this, req, l, HeldAsk::Code, runs - 1, false);
                if let Some(on_asks) = this.deps.on_asks.clone() {
                    on_asks(&req.thread_id);
                }
                return Ok(());
            }
            let real = this.real_args(&req.thread_id, &args).map_err(|name| format!("the secret ${name} is not filled"))?;
            let mut params = Map::new();
            params.insert("name".into(), t.tool.clone().into());
            params.insert("arguments".into(), Value::Object(real));
            ("tools/call", params)
        }
        SlateRunDecl::Resource(r) => ("resources/read", map_of(json!({ "uri": r.uri }))),
        SlateRunDecl::Cmd(_) => return Err(format!("{server} is not an MCP server run")),
    };
    let timeout_ms = match &spec.transport {
        McpTransport::Stdio { tool_timeout_sec, .. } => tool_timeout_sec.map(|s| s * 1000),
        McpTransport::Http { .. } => None,
    }
    .unwrap_or_else(|| this.call_ms());
    let (id, asked) = client.request(method, params, timeout_ms);
    l.borrow_mut().inflight = Some((client.clone(), id));
    let got = asked.await;
    if l.borrow().gen != gen {
        return Ok(());
    }
    l.borrow_mut().inflight = None;
    let got = got?;
    let (mut record, raw) = result_of(this, &req.thread_id, &spec, decl, &got);
    if let Some(then) = decl_then(decl) {
        if record.get("state") == Some(&json!("done")) {
            let Reshape { done, kill } = (this.deps.reshape)(&req.thread_id, &then, &raw);
            l.borrow_mut().reshaping = Some(kill);
            let shaped = done.await;
            if l.borrow().gen != gen {
                return Ok(());
            }
            l.borrow_mut().reshaping = None;
            let mut kept = record.clone();
            kept.remove("json");
            kept.remove("text");
            record = match shaped {
                ReshapeAnswer::Json(value) => {
                    kept.insert("json".into(), value);
                    kept
                }
                ReshapeAnswer::Failed { why, exit, err } => {
                    kept.insert("state".into(), "failed".into());
                    kept.insert("why".into(), why.into());
                    kept.insert("exit".into(), exit.map_or(Value::Null, Value::from));
                    kept.insert("err".into(), err.into());
                    kept
                }
            };
        }
    }
    let ended_at = this.now();
    record.insert("startedAt".into(), started_at.into());
    record.insert("endedAt".into(), ended_at.into());
    record.insert("ms".into(), (ended_at - started_at).into());
    record.insert("runs".into(), runs.into());
    this.write(&req.thread_id, &req.run, l, RunRecord(record));
    Ok(())
}

impl SlateMcp {
    fn fresh(&self) -> u64 {
        let mut st = self.state.borrow_mut();
        st.next += 1;
        st.next
    }

    fn now(&self) -> i64 {
        self.deps.clock.now()
    }

    fn idle_ms(&self) -> u64 {
        self.deps.idle_ms.unwrap_or(IDLE_MS)
    }

    fn call_ms(&self) -> u64 {
        self.deps.call_ms.unwrap_or(CALL_MS)
    }

    /// Every secret's plaintext and the server's own values put out of what it said (spec rule 2).
    fn clean(&self, thread_id: &str, spec: Option<&McpServerSpec>, text: &str) -> String {
        let mut out = self.deps.secrets.scrub(thread_id, text);
        let mut values: Vec<&String> = spec.map(|s| s.secrets.iter().collect()).unwrap_or_default();
        values.sort_by(|a, b| utf16::len(b).cmp(&utf16::len(a)));
        for v in values {
            if utf16::len(v) >= 4 && out.contains(v.as_str()) {
                out = out.split(v.as_str()).collect::<Vec<_>>().join("***");
            }
        }
        out
    }

    fn live(&self, thread_id: &str, run: &str, last: Option<&RunRecord>) -> Rc<RefCell<Live>> {
        let mut st = self.state.borrow_mut();
        let t = thread_entry(&mut st, thread_id);
        if let Some(l) = t.runs.get(run) {
            return l.clone();
        }
        let l = Rc::new(RefCell::new(Live {
            record: record_of("idle", vec![("runs", last.map_or(0, RunRecord::runs).into())]),
            gen: 0,
            inflight: None,
            reshaping: None,
            result: last.and_then(last_result),
            held: None,
            starts: Vec::new(),
            pressed: Vec::new(),
        }));
        t.runs.insert(run.to_string(), l.clone());
        l
    }

    fn get_live(&self, thread_id: &str, run: &str) -> Option<Rc<RefCell<Live>>> {
        self.state.borrow().threads.get(thread_id).and_then(|t| t.runs.get(run).cloned())
    }

    fn runs_of(&self, thread_id: &str) -> Vec<(String, Rc<RefCell<Live>>)> {
        self.state.borrow().threads.get(thread_id).map_or_else(Vec::new, |t| t.runs.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    fn write(&self, thread_id: &str, run: &str, l: &Rc<RefCell<Live>>, record: RunRecord) -> RunRecord {
        {
            let mut live = l.borrow_mut();
            live.record = record.clone();
            if record.state() != Some(RunState::Running) {
                if let Some(result) = last_result(&record) {
                    live.result = Some(result);
                }
            }
        }
        (self.deps.on_record)(thread_id, run, &record);
        record
    }

    fn forget_conn(&self, thread_id: &str, server: &str, gen: u64) {
        let removed = {
            let mut st = self.state.borrow_mut();
            match st.threads.get_mut(thread_id) {
                Some(t) if t.conns.get(server).is_some_and(|e| e.gen == gen) => t.conns.shift_remove(server),
                _ => None,
            }
        };
        drop(removed);
    }

    fn untrack(&self, thread_id: &str, id: u64) {
        let dropped: Vec<_> = {
            let mut st = self.state.borrow_mut();
            match st.threads.get_mut(thread_id) {
                Some(t) => {
                    let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut t.pending).into_iter().partition(|(i, _)| *i == id);
                    t.pending = kept;
                    gone
                }
                None => Vec::new(),
            }
        };
        drop(dropped);
    }

    fn shown_args(&self, thread_id: &str, args: &Map<String, Value>) -> Value {
        map_strings(&Value::Object(args.clone()), &|s: &str| {
            mark_text(s, &|caps: &Captures| match self.deps.secrets.plaintext(thread_id, &caps[1]) {
                None => format!("{SECRET_DOTS} (not filled)"),
                Some(text) => format!("{SECRET_DOTS} ({})", utf16::len(&text)),
            })
        })
    }

    /// The arguments with every secret in place, for the call alone; the first secret not filled is the error.
    fn real_args(&self, thread_id: &str, args: &Map<String, Value>) -> Result<Map<String, Value>, String> {
        let missing: RefCell<Option<String>> = RefCell::new(None);
        let out = map_strings(&Value::Object(args.clone()), &|s: &str| {
            mark_text(s, &|caps: &Captures| match self.deps.secrets.plaintext(thread_id, &caps[1]) {
                Some(text) => text,
                None => {
                    missing.borrow_mut().get_or_insert_with(|| caps[1].to_string());
                    String::new()
                }
            })
        });
        let unfilled = missing.borrow().clone();
        match unfilled {
            Some(name) => Err(name),
            None => Ok(map_of(out)),
        }
    }

    fn listed(&self, thread_id: &str, decl: &McpRunDecl) -> Option<McpToolInfo> {
        let SlateRunDecl::Tool(t) = &decl.decl else { return None };
        let st = self.state.borrow();
        st.lists.get(&list_key(thread_id, &t.server)).and_then(|tools| tools.iter().find(|x| x.name == t.tool).cloned())
    }

    /// Whether this start waits for the person (spec rule 8); `None` where only an unlisted tool's hints could decide.
    fn asks_now(&self, thread_id: &str, decl: &McpRunDecl, info: Option<&McpToolInfo>, args: &Map<String, Value>) -> Option<bool> {
        if matches!(&decl.decl, SlateRunDecl::Tool(t) if t.confirm.is_some()) {
            return Some(true);
        }
        let rule = (self.deps.asks)(thread_id);
        if rule == ToolAsks::Never || self.deps.approvals.has(thread_id, &code_key(decl, args)) {
            return Some(false);
        }
        if rule == ToolAsks::Every {
            return Some(true);
        }
        if let SlateRunDecl::Resource(_) = &decl.decl {
            return Some(false);
        }
        info.map(|i| i.annotations.as_ref().and_then(|a| a.read_only_hint.as_ref()) != Some(&Value::Bool(true)))
    }

    /// The sheet a held code ask shows (spec rule 9), keys in the TS's order.
    fn ask_of(&self, thread_id: &str, run: &str, req: &McpRunStart, key: &str) -> SlateAsk {
        let d = &req.decl;
        let description = decl_description(d).or_else(|| sentence(self.listed(thread_id, d).and_then(|t| t.description).as_ref(), 240));
        let (tool, args) = match &d.decl {
            SlateRunDecl::Tool(t) => (t.tool.clone(), self.shown_args(thread_id, &(req.args)())),
            SlateRunDecl::Resource(r) => (r.uri.clone(), Value::Object(Map::new())),
            SlateRunDecl::Cmd(_) => (String::new(), Value::Object(Map::new())),
        };
        let mut ask = Map::new();
        ask.insert("key".into(), key.into());
        ask.insert("run".into(), run.into());
        ask.insert("kind".into(), "tool".into());
        ask.insert("server".into(), decl_server(d).into());
        ask.insert("tool".into(), tool.into());
        if let SlateRunDecl::Resource(_) = &d.decl {
            ask.insert("resource".into(), Value::Bool(true));
        }
        ask.insert("computer".into(), (self.deps.computer)(thread_id).into());
        ask.insert("why".into(), HELD_APPROVAL.into());
        if let Some(description) = description {
            ask.insert("description".into(), description.into());
        }
        ask.insert("args".into(), args);
        if let SlateRunDecl::Tool(t) = &d.decl {
            if let Some(Value::String(confirm)) = &t.confirm {
                ask.insert("confirm".into(), confirm.clone().into());
            }
        }
        if let Some(then) = decl_then(d) {
            ask.insert("then".into(), then.into());
        }
        if let Some(files) = &d.files {
            ask.insert("files".into(), Value::Object(files.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect()));
        }
        SlateAsk::Tool(Value::Object(ask))
    }

    fn held_on(&self, thread_id: &str, key: &str) -> Vec<(String, Rc<RefCell<Live>>)> {
        self.runs_of(thread_id)
            .into_iter()
            .filter(|(_, l)| l.borrow().held.as_ref().is_some_and(|h| h.ask == HeldAsk::Code && h.key.as_deref() == Some(key)))
            .collect()
    }

    fn stop(&self, thread_id: &str, run: &str, l: &Rc<RefCell<Live>>, why: Option<&str>) {
        let (was, inflight, reshaping) = {
            let mut live = l.borrow_mut();
            let was = live.record.state();
            live.gen += 1;
            live.held = None;
            (was, live.inflight.take(), live.reshaping.take())
        };
        if let Some((client, id)) = inflight {
            let mut params = Map::new();
            params.insert("requestId".into(), id.into());
            params.insert("reason".into(), why.unwrap_or("cancelled").into());
            client.notify("notifications/cancelled", Some(params));
        }
        if let Some(kill) = reshaping {
            kill();
        }
        let Some(why) = why else { return };
        if was != Some(RunState::Running) && was != Some(RunState::Held) {
            return;
        }
        let runs = l.borrow().record.runs();
        let ended_at = self.now();
        self.write(
            thread_id,
            run,
            l,
            record_of("cancelled", vec![("why", why.into()), ("exit", Value::Null), ("runs", runs.into()), ("endedAt", ended_at.into())]),
        );
    }

    fn sweep(self: &Rc<Self>) {
        let at = self.now();
        let idle = self.idle_ms() as i64;
        let entries: Vec<(String, String, u64, SharedConn)> = {
            let st = self.state.borrow();
            st.threads
                .iter()
                .flat_map(|(thread, t)| t.conns.iter().map(move |(server, e)| (thread.clone(), server.clone(), e.gen, e.fut.clone())))
                .collect()
        };
        for (thread, server, gen, fut) in entries {
            let weak = Rc::downgrade(self);
            spawn_local(async move {
                let Ok(conn) = fut.await else { return };
                let Some(m) = weak.upgrade() else { return };
                let (busy, used_at, client) = {
                    let c = conn.borrow();
                    (c.busy, c.used_at, c.client.clone())
                };
                let removed = {
                    let mut st = m.state.borrow_mut();
                    let Some(t) = st.threads.get_mut(&thread) else { return };
                    let same = t.conns.get(&server).is_some_and(|e| e.gen == gen);
                    let quiet = busy == 0 && !t.kept.contains(&server) && !t.shown && at - used_at >= idle;
                    if same && quiet {
                        t.conns.shift_remove(&server)
                    } else {
                        None
                    }
                };
                if removed.is_some() {
                    client.close();
                }
            });
        }
    }

    /// The person's answer to a held run, by the key its ask carried (spec rule 15).
    pub fn approve(self: &Rc<Self>, thread_id: &str, key: &str, scope: ApproveScope) {
        if scope == ApproveScope::Always {
            self.deps.approvals.allow(thread_id, key);
        }
        for (_, l) in self.held_on(thread_id, key) {
            let req = l.borrow_mut().held.take().map(|h| h.req);
            if let Some(req) = req {
                let _ = launch(self, req, l, true);
            }
        }
    }

    /// What a start inside a batch will give, before it is started (spec rule 25).
    pub fn provisional(&self, thread_id: &str, decl: &McpRunDecl, last: Option<&RunRecord>, args: &Map<String, Value>) -> RunRecord {
        let runs = last.map_or(0, RunRecord::runs);
        let info = self.listed(thread_id, decl);
        if self.asks_now(thread_id, decl, info.as_ref(), args) == Some(true) {
            return record_of("held", vec![("why", HELD_APPROVAL.into()), ("runs", runs.into())]);
        }
        let result = last.and_then(last_result);
        running_record(result.as_ref(), runs + 1, self.now())
    }

    /// spec rule 11.
    pub fn start(self: &Rc<Self>, asked: McpRunStart) -> RunStartAnswer {
        // Read once, so the arguments the person allows are the ones the call sends.
        let sent = (asked.args)();
        let fixed = sent.clone();
        let req = McpRunStart { args: Rc::new(move || fixed.clone()), ..asked };
        let l = self.live(&req.thread_id, &req.run, req.last.as_ref());
        let beside = l.borrow().record.state() == Some(RunState::Running);
        if beside && matches!(&req.decl.decl, SlateRunDecl::Tool(t) if t.once == Some(true)) {
            let record = l.borrow().record.clone();
            return answer(Outcome::Noop, record);
        }
        l.borrow_mut().held = None;
        let person = req.by == RunBy::Person;
        let now = self.now();
        let times = {
            let live = l.borrow();
            spent(Some(if person { live.pressed.as_slice() } else { live.starts.as_slice() }), now)
        };
        let runs = l.borrow().record.runs();
        match times {
            None => return hold(self, &req, &l, if person { HeldAsk::Pressed } else { HeldAsk::Budget }, runs, beside),
            Some(times) if person => l.borrow_mut().pressed = times,
            Some(times) => l.borrow_mut().starts = times,
        }
        let info = self.listed(&req.thread_id, &req.decl);
        if self.asks_now(&req.thread_id, &req.decl, info.as_ref(), &sent) == Some(true) {
            let asked = hold(self, &req, &l, HeldAsk::Code, runs, beside);
            if beside {
                if let Some(on_asks) = self.deps.on_asks.clone() {
                    on_asks(&req.thread_id);
                }
            }
            return asked;
        }
        launch(self, req, l, false)
    }

    /// spec rule 16.
    pub fn deny(&self, thread_id: &str, key: &str) {
        for (run, l) in self.held_on(thread_id, key) {
            let running = l.borrow().record.state() == Some(RunState::Running);
            if running {
                l.borrow_mut().held = None;
            } else {
                self.stop(thread_id, &run, &l, Some("you said not to run it"));
            }
        }
    }

    /// Whether a key is one of this module's: a tool or resource run's code.
    pub fn owns(&self, _thread_id: &str, key: &str) -> bool {
        key.starts_with("mcp:")
    }

    /// The code asks held, in run order (spec rule 17).
    pub fn held(&self, thread_id: &str) -> Vec<SlateAsk> {
        let mut out = Vec::new();
        for (run, l) in self.runs_of(thread_id) {
            let held = l.borrow().held.as_ref().filter(|h| h.ask == HeldAsk::Code).and_then(|h| h.key.clone().map(|k| (h.req.clone(), k)));
            if let Some((req, key)) = held {
                out.push(self.ask_of(thread_id, &run, &req, &key));
            }
        }
        out
    }

    pub fn named(&self, thread_id: &str, key: &str) -> Option<NamedAsk> {
        for asked in self.held(thread_id) {
            let SlateAsk::Tool(v) = asked else { continue };
            if v.get("key").and_then(Value::as_str) != Some(key) {
                continue;
            }
            let server = v["server"].as_str().unwrap_or("");
            let tool = v["tool"].as_str().unwrap_or("");
            let cmd = format!("{server}.{tool} {}", v["args"]);
            return Some(NamedAsk { run: v["run"].as_str().unwrap_or("").to_string(), cmd: utf16::cut(&cmd, 120) });
        }
        None
    }

    /// Settles once every tool list and connect a start of this thread is waiting on has, or after `ms` (spec rule 18).
    pub async fn pending(&self, thread_id: &str, ms: Option<u64>) {
        let ms = ms.unwrap_or(CONNECT_MS);
        for _ in 0..4 {
            let waits: Vec<Shared<LocalBoxFuture<'static, ()>>> =
                self.state.borrow().threads.get(thread_id).map_or_else(Vec::new, |t| t.pending.iter().map(|(_, f)| f.clone()).collect());
            if waits.is_empty() {
                return;
            }
            within(self.deps.clock.clone(), ms, join_all(waits)).await;
        }
    }

    pub fn cancel(&self, thread_id: &str, run: &str) {
        if let Some(l) = self.get_live(thread_id, run) {
            self.stop(thread_id, run, &l, Some("cancelled"));
        }
    }

    pub fn stop_all(&self, thread_id: &str, opts: StopAll) {
        for (run, l) in self.runs_of(thread_id) {
            let why = if opts.quiet { None } else { Some(opts.why.as_deref().unwrap_or("cancelled")) };
            self.stop(thread_id, &run, &l, why);
        }
    }

    /// The servers the slate's document names, and which an `always` timer keeps open; others close (spec rule 20).
    pub fn servers(&self, thread_id: &str, names: &[String], kept: &[String]) {
        let closing: Vec<SharedConn> = {
            let mut st = self.state.borrow_mut();
            let t = thread_entry(&mut st, thread_id);
            t.named = names.iter().cloned().collect();
            t.kept = kept.iter().cloned().collect();
            let stale: Vec<String> = t.conns.keys().filter(|s| !t.named.contains(*s)).cloned().collect();
            stale.iter().filter_map(|s| t.conns.shift_remove(s).map(|e| e.fut)).collect()
        };
        for fut in closing {
            close_conn(fut);
        }
    }

    pub fn shown(&self, thread_id: &str, shown: bool) {
        let (was, conns): (bool, Vec<SharedConn>) = {
            let mut st = self.state.borrow_mut();
            let t = thread_entry(&mut st, thread_id);
            let was = t.shown;
            t.shown = shown;
            let conns = if was && !shown { t.conns.values().map(|e| e.fut.clone()).collect() } else { Vec::new() };
            (was, conns)
        };
        if was && !shown {
            for fut in conns {
                let clock = self.deps.clock.clone();
                spawn_local(async move {
                    if let Ok(conn) = fut.await {
                        conn.borrow_mut().used_at = clock.now();
                    }
                });
            }
        }
    }

    /// Closes the thread's servers and keeps its runs: its machine napped.
    pub fn close_servers(&self, thread_id: &str) {
        let closing: Vec<SharedConn> = {
            let mut st = self.state.borrow_mut();
            match st.threads.get_mut(thread_id) {
                Some(t) => t.conns.drain(..).map(|(_, e)| e.fut).collect(),
                None => Vec::new(),
            }
        };
        for fut in closing {
            close_conn(fut);
        }
    }

    /// `slate_catalog <server>` (spec rule 22).
    pub async fn catalog(self: &Rc<Self>, thread_id: &str, server: &str) -> Result<String, String> {
        let conn = connect(self, thread_id, server).await?;
        let tools = tools_of(self, thread_id, server, &conn).await?;
        let (client, has_resources, spec) = {
            let c = conn.borrow();
            (c.client.clone(), c.resources, c.spec.clone())
        };
        let mut resources: Vec<Value> = Vec::new();
        if has_resources {
            let (_, asked) = client.request("resources/list", Map::new(), CONNECT_MS);
            if let Ok(got) = asked.await {
                resources = got
                    .get("resources")
                    .and_then(Value::as_array)
                    .map(|items| items.iter().filter(|r| r.get("uri").is_some_and(Value::is_string)).cloned().collect())
                    .unwrap_or_default();
            }
        }
        let used_at = self.now();
        conn.borrow_mut().used_at = used_at;
        let n = tools.len();
        let mut head = format!("{server}: an MCP server of this thread's agent, {n} tool{}", if n == 1 { "" } else { "s" });
        if !resources.is_empty() {
            let r = resources.len();
            head.push_str(&format!(", {r} resource{}", if r == 1 { "" } else { "s" }));
        }
        head.push('.');
        let mut lines = vec![
            head,
            asks_line(self.asks_of(thread_id)).to_string(),
            "A run on a tool not marked read-only takes description=\"what it does, in one plain line\", which the person reads before allowing it; a write without one is refused.".to_string(),
            format!("A run calls a tool: <run name=\"x\" tool=\"{server}.<tool>\" args={{{{ <property>: <expression> }}}} every={{60}} />; a resource: <run name=\"x\" resource=\"{server}:<uri>\" />. $x.json holds the structured result (else the text parsed as JSON), $x.out the text, $x.err the error."),
            "Each tool says whether it returns data (a structured result fills $x.json) or text only (prose for a model: draw $x.out, or reshape it with then=).".to_string(),
            String::new(),
        ];
        for tool in &tools {
            let hints = hints_of(tool, self.seen_of(thread_id, server, &tool.name));
            let about = sentence(tool.description.as_ref(), 240);
            let mut first = tool.name.clone();
            if !hints.is_empty() {
                first.push_str(&format!(" [{}]", hints.join(", ")));
            }
            if let Some(about) = about {
                first.push_str(&format!(": {about}"));
            }
            lines.push(first);
            let props = fold_schema(tool.input_schema.as_ref().unwrap_or(&Value::Null));
            if props.is_empty() {
                lines.push("  (no arguments)".to_string());
            } else {
                lines.extend(props);
            }
        }
        if !resources.is_empty() {
            lines.push(String::new());
            for r in resources.iter().take(20) {
                let uri = r.get("uri").and_then(Value::as_str).unwrap_or("");
                let mime =
                    r.get("mimeType").map_or(String::new(), |m| format!(" ({})", m.as_str().map_or_else(|| m.to_string(), String::from)));
                let about = sentence(r.get("description"), 160).map_or(String::new(), |a| format!(": {a}"));
                lines.push(format!("resource {uri}{mime}{about}"));
            }
        }
        Ok(self.clean(thread_id, Some(&spec), &lines.join("\n")))
    }

    fn asks_of(&self, thread_id: &str) -> ToolAsks {
        (self.deps.asks)(thread_id)
    }

    fn seen_of(&self, thread_id: &str, server: &str, tool: &str) -> Option<bool> {
        self.state.borrow().seen.get(&seen_key(thread_id, server, tool)).copied()
    }

    /// What a write is refused for its runs (spec rule 23).
    pub async fn refused_runs(
        self: &Rc<Self>,
        thread_id: &str,
        before: Option<&SlateDoc>,
        doc: Option<&SlateDoc>,
        away: Vec<SlateProblem>,
    ) -> Vec<SlateProblem> {
        let Some(doc) = doc else { return away };
        if !away.is_empty() {
            return away;
        }
        let bare: Vec<(String, String, String)> = doc
            .runs
            .iter()
            .filter_map(|(name, d)| {
                let SlateRunDecl::Tool(t) = d else { return None };
                if t.description.is_some() {
                    return None;
                }
                let then_json = serde_json::to_value(d).ok();
                let before_json = before.and_then(|b| b.runs.get(name)).and_then(|v| serde_json::to_value(v).ok());
                (stable_json(before_json.as_ref()) != stable_json(then_json.as_ref()))
                    .then(|| (name.clone(), t.server.clone(), t.tool.clone()))
            })
            .collect();
        if bare.is_empty() {
            return Vec::new();
        }
        let mut servers: IndexSet<String> = IndexSet::new();
        for (_, server, _) in &bare {
            servers.insert(server.clone());
        }
        let clock = self.deps.clock.clone();
        let lookups = join_all(servers.into_iter().map(|server| {
            let clock = clock.clone();
            async move {
                let known = self.state.borrow().lists.get(&list_key(thread_id, &server)).cloned();
                let tools = match known {
                    Some(tools) => tools,
                    None => {
                        let listing = list_for(self, thread_id, &server);
                        within(clock, DESCRIBE_MS, listing).await.unwrap_or_default()
                    }
                };
                (server, tools)
            }
        }))
        .await;
        let tools: HashMap<String, Vec<McpToolInfo>> = lookups.into_iter().collect();
        bare.into_iter()
            .filter(|(_, server, tool)| {
                !tools
                    .get(server)
                    .and_then(|ts| ts.iter().find(|t| t.name == *tool))
                    .and_then(|t| t.annotations.as_ref())
                    .is_some_and(|a| is_true(&a.read_only_hint))
            })
            .map(|(name, server, tool)| slate_problem("K713", &slate_description_missing(&format!("${name}"), &server, &tool), None))
            .collect()
    }

    /// Forgets the thread's runs and servers; with `keep`, a server and a run going on are left running.
    pub fn drop_thread(&self, thread_id: &str, keep: bool) {
        if !self.state.borrow().threads.contains_key(thread_id) {
            return;
        }
        let runs = self.runs_of(thread_id);
        if !keep {
            for (run, l) in &runs {
                self.stop(thread_id, run, l, None);
            }
            let closing: Vec<SharedConn> = {
                let mut st = self.state.borrow_mut();
                st.threads.get_mut(thread_id).map_or_else(Vec::new, |t| t.conns.drain(..).map(|(_, e)| e.fut).collect())
            };
            for fut in closing {
                close_conn(fut);
            }
        }
        let removed = self.state.borrow_mut().threads.shift_remove(thread_id);
        drop(removed);
        let mut st = self.state.borrow_mut();
        st.lists.retain(|k, _| k.0 != thread_id);
        st.seen.retain(|k, _| k.0 != thread_id);
    }

    pub fn close(&self) {
        let sweep = self.state.borrow_mut().sweep.take();
        if let Some(sweep) = sweep {
            sweep();
        }
        let threads: Vec<String> = self.state.borrow().threads.keys().cloned().collect();
        for thread_id in threads {
            self.drop_thread(&thread_id, false);
        }
    }
}
