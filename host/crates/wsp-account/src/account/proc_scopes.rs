// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/proc-scopes.ts at 65f6b89c7: the threads this host names to a computer's
//! daemon in proc.scopes. Spec: spec/proc_scopes.md.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;

use wsp_core::daemon_channel::DaemonChannel;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::daemon::{ProcKind, ProcScope};

use crate::account::core_types::{AccountResult, Release};

/// One session row as the scopes read it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScopeRow {
    pub workspace_id: String,
    pub thread_id: Option<String>,
    /// `view.status`.
    pub status: String,
    pub started_at: Option<f64>,
    /// The agent process the host launched for the turn.
    pub pid: Option<i64>,
}

/// A kept agent (`ctx.keptAgents`): its workspace, its pid, and whether it is a warm one no send has claimed.
#[derive(Clone, Debug, PartialEq)]
pub struct KeptRow {
    pub workspace_id: String,
    pub pid: Option<i64>,
    /// `kept.warm?.claimed === false`.
    pub warm_unclaimed: bool,
}

/// A shell a thread's pane opened (`ctx.threadTerminals()`).
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadTerminal {
    pub thread: String,
    pub workspace: String,
    pub pid: i64,
}

/// A live record as the scopes read it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScopeRecord {
    pub kind: WorkspaceKind,
    pub place: Option<String>,
    /// `ctx.checkoutOf(record)`.
    pub checkout: String,
}

/// `ctx.latestOn(threadId)`: the thread's latest row's workspace and cwd.
#[derive(Clone, Debug, PartialEq)]
pub struct LatestRow {
    pub workspace_id: String,
    pub cwd: Option<String>,
}

/// What the area reads off the runtime's context.
#[async_trait(?Send)]
pub trait ProcScopesCtx {
    /// `ctx.turnGroups`: workspace, then group pid and thread, in map order.
    fn turn_groups(&self) -> Vec<(String, Vec<(i64, String)>)>;
    /// `groupExists(pid)` (wsp_core::local_exec::group_exists).
    fn group_exists(&self, pgid: i64) -> bool;
    /// `ctx.sessions.values()`, in map order.
    fn sessions(&self) -> Vec<ScopeRow>;
    /// `ctx.keptAgents`, in map order.
    fn kept_agents(&self) -> Vec<(String, KeptRow)>;
    fn latest_on(&self, thread_id: &str) -> Option<LatestRow>;
    /// `ctx.threadRecords`: thread and workspace, in map order.
    fn thread_records(&self) -> Vec<(String, String)>;
    fn thread_terminals(&self) -> Vec<ThreadTerminal>;
    /// `ctx.live.get(id)?.record`.
    fn live_record(&self, workspace_id: &str) -> Option<ScopeRecord>;
    /// `ctx.moduleOf(kind).sharedDaemon`.
    fn shared_daemon(&self, kind: WorkspaceKind) -> bool;
    /// `ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, () => {})`: fails where no daemon is wired here.
    async fn channel_here(&self) -> AccountResult<Rc<dyn DaemonChannel>>;
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>);
}

/// proc-scopes.ts:28 `procScopesArea(ctx)`.
pub struct ProcScopesArea {
    pub(crate) ctx: Rc<dyn ProcScopesCtx>,
    /// proc-scopes.ts:30 `owned`: pid → thread and kind.
    pub(crate) owned: RefCell<IndexMap<i64, (String, ProcKind)>>,
    /// proc-scopes.ts:32 `pushes`.
    pub(crate) pushes: RefCell<Vec<Rc<dyn Fn()>>>,
    /// proc-scopes.ts:35 `standing`: the dial to this computer's daemon, shared by every move while it stands, and
    /// the scopes' JSON it was last told.
    pub(crate) standing: RefCell<Option<(futures::future::Shared<futures::future::LocalBoxFuture<'static, Option<Rc<dyn DaemonChannel>>>>, String)>>,
}

impl ProcScopesArea {
    pub fn new(ctx: Rc<dyn ProcScopesCtx>) -> Rc<ProcScopesArea> {
        todo!()
    }

    /// P5, proc-scopes.ts:74.
    pub fn own_process(self: &Rc<Self>, thread_id: &str, pid: i64, kind: ProcKind) -> Release {
        todo!()
    }

    /// P8, proc-scopes.ts:69: the standing channel told, then every watching channel's push.
    pub fn proc_scopes_moved(self: &Rc<Self>) {
        todo!()
    }

    /// P1-P4, proc-scopes.ts:92.
    pub fn scopes_here(&self) -> Vec<ProcScope> {
        todo!()
    }

    /// P6, proc-scopes.ts:149.
    pub fn scopes_on(&self, place_id: &str) -> Vec<ProcScope> {
        todo!()
    }

    /// P7, proc-scopes.ts:176.
    pub fn scoped_procs(self: &Rc<Self>, scopes: Rc<dyn Fn() -> Vec<ProcScope>>, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        todo!()
    }
}
