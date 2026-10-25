// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-mcp.ts:558-1045 (`createSlateMcp`) at 65f6b89c7. Spec: spec/slate_mcp_runs.md.
//!
//! One live connection per thread and server, the tool and resource runs, when to ask the person, the start budgets,
//! scrubbing, the idle sweep, `slate_catalog <server>` and the K713 refusal of a write. One thread owns all of it
//! (rule 3.1); a borrow of the state is never held across an await or a call out (rule 3.3).
#![allow(unused_variables, dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};

use wsp_core::clock::{Cancel, Clock};

use super::client::{McpClient, McpRunDecl, McpServerSpec, McpToolInfo, ToolAsks};
use crate::pending::slate::{SlateDoc, SlateProblem};
use crate::pending::wire::SlateAsk;
use crate::slate_runs::{Reshape, RunApprovals, RunBy, RunRecord, RunResult, RunStartAnswer};

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

struct Held {
    req: McpRunStart,
    ask: HeldAsk,
    key: Option<String>,
}

struct Conn {
    client: Rc<dyn McpClient>,
    spec: McpServerSpec,
    resources: bool,
    tools: Option<futures::future::Shared<LocalBoxFuture<'static, Result<Vec<McpToolInfo>, String>>>>,
    used_at: i64,
    busy: u32,
}

struct Live {
    record: RunRecord,
    gen: u64,
    inflight: Option<(Rc<RefCell<Conn>>, u64)>,
    reshaping: Option<Reshape>,
    result: Option<RunResult>,
    held: Option<Held>,
    starts: Vec<i64>,
    pressed: Option<Vec<i64>>,
}

type SharedConn = futures::future::Shared<LocalBoxFuture<'static, Result<Rc<RefCell<Conn>>, String>>>;

struct ThreadMcp {
    runs: IndexMap<String, Live>,
    conns: IndexMap<String, SharedConn>,
    named: IndexSet<String>,
    kept: IndexSet<String>,
    shown: bool,
    pending: Vec<futures::future::Shared<LocalBoxFuture<'static, ()>>>,
}

struct State {
    threads: IndexMap<String, ThreadMcp>,
    lists: HashMap<(String, String), Vec<McpToolInfo>>,
    seen: HashMap<(String, String, String), bool>,
    sweep: Option<Cancel>,
}

/// slate-mcp.ts:163 SlateMcp, made by `create_slate_mcp`.
pub struct SlateMcp {
    deps: SlateMcpDeps,
    state: RefCell<State>,
}

// slate-mcp.ts:586 createSlateMcp(deps)
/// Starts the idle sweep on the clock; needs a `LocalSet` for the runs' work.
pub fn create_slate_mcp(deps: SlateMcpDeps) -> Rc<SlateMcp> {
    todo!()
}

impl SlateMcp {
    /// What a start inside a batch will give, before it is started (spec rule 25).
    pub fn provisional(&self, thread_id: &str, decl: &McpRunDecl, last: Option<&RunRecord>, args: &Map<String, Value>) -> RunRecord {
        todo!()
    }

    /// spec rule 11.
    pub fn start(self: &Rc<Self>, asked: McpRunStart) -> RunStartAnswer {
        todo!()
    }

    /// The person's answer to a held run, by the key its ask carried (spec rule 15).
    pub fn approve(self: &Rc<Self>, thread_id: &str, key: &str, scope: ApproveScope) {
        todo!()
    }

    /// spec rule 16.
    pub fn deny(&self, thread_id: &str, key: &str) {
        todo!()
    }

    /// Whether a key is one of this module's.
    pub fn owns(&self, thread_id: &str, key: &str) -> bool {
        todo!()
    }

    /// The code asks held, in run order (spec rule 17).
    pub fn held(&self, thread_id: &str) -> Vec<SlateAsk> {
        todo!()
    }

    pub fn named(&self, thread_id: &str, key: &str) -> Option<NamedAsk> {
        todo!()
    }

    /// Settles once every tool list and connect a start of this thread waits on has, or after `ms` (spec rule 18).
    pub async fn pending(&self, thread_id: &str, ms: Option<u64>) {
        todo!()
    }

    pub fn cancel(&self, thread_id: &str, run: &str) {
        todo!()
    }

    pub fn stop_all(&self, thread_id: &str, opts: StopAll) {
        todo!()
    }

    /// The servers the slate's document names, and which an `always` timer keeps open; others close (spec rule 20).
    pub fn servers(&self, thread_id: &str, names: &[String], kept: &[String]) {
        todo!()
    }

    pub fn shown(&self, thread_id: &str, shown: bool) {
        todo!()
    }

    /// Closes the thread's servers and keeps its runs: its machine napped.
    pub fn close_servers(&self, thread_id: &str) {
        todo!()
    }

    /// `slate_catalog <server>` (spec rule 22).
    pub async fn catalog(self: &Rc<Self>, thread_id: &str, server: &str) -> Result<String, String> {
        todo!()
    }

    /// What a write is refused for its runs (spec rule 23).
    pub async fn refused_runs(self: &Rc<Self>, thread_id: &str, before: Option<&SlateDoc>, doc: Option<&SlateDoc>, away: Vec<SlateProblem>) -> Vec<SlateProblem> {
        todo!()
    }

    /// Forgets the thread's runs and servers; with `keep`, a server and a run going on are left running.
    pub fn drop_thread(&self, thread_id: &str, keep: bool) {
        todo!()
    }

    pub fn close(&self) {
        todo!()
    }
}
