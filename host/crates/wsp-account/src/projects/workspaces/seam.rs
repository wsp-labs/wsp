// SPDX-License-Identifier: AGPL-3.0-only
//! What every projects area shares: the live entry handle, the shared maps as the context hands them out, a daemon
//! ask, and the base half of every area's context trait. The entry is wsp-machines' `Entry` over wsp-core's
//! `LiveWorkspace`, the one the account areas hold too.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

pub use crate::account::core_types::Entry;

/// `ctx.live`, in insertion order (rule 3.11).
pub type Live = Rc<RefCell<IndexMap<String, Entry>>>;
/// `ctx.projectsHeld`, in insertion order.
pub type ProjectsHeld = Rc<RefCell<IndexMap<String, ProjectView>>>;
/// What `spawnGuard` hands back: the slot's release, safe to call more than once.
pub type FreeSlot = crate::account::core_types::Release;
/// A bus event as the TS object literal writes it, keys in order; `seq` is the bus's to stamp.
pub type Event = serde_json::Value;

/// One daemon op: `ask({op, ...})` answering the daemon's reply, or its refusal.
pub type DaemonAsk = Rc<dyn Fn(serde_json::Value) -> LocalBoxFuture<'static, Result<serde_json::Value, HostError>>>;
/// The work handed to `withDaemon(entry, ask => ...)`.
pub type DaemonWork = Box<dyn FnOnce(DaemonAsk) -> LocalBoxFuture<'static, Result<serde_json::Value, HostError>>>;
/// The work handed to `onThisComputer((ask, home) => ...)`: the ask and this computer's home folder.
pub type HereWork = Box<dyn FnOnce(DaemonAsk, String) -> LocalBoxFuture<'static, Result<serde_json::Value, HostError>>>;
/// Work queued behind a key (`queued`, `oneFolder`).
pub type Queued<T> = LocalBoxFuture<'static, Result<T, HostError>>;

/// `entryOf`'s options: the thread the caller names, the act it asks for, `now` (skip the boot wait) and `beingMade`.
pub use wsp_machines::machines::boot::entries::EntryAsk;

/// A git run on this computer, `ctx.gitHere(cwd, args, timeoutMs?, input?)`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitAsk {
    pub cwd: String,
    pub args: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub input: Option<String>,
}

/// The calls every projects area makes on the runtime context.
#[async_trait(?Send)]
pub trait Base {
    async fn ready(&self) -> Result<(), HostError>;
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    fn store(&self) -> Rc<dyn Store>;
    fn live(&self) -> Live;
    fn projects_held(&self) -> ProjectsHeld;
    /// `ctx.projectHeld(id)`: the held project; the context refuses one it does not hold.
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError>;
    /// Every session row's view, in insertion order (`ctx.sessions.values()`).
    fn session_views(&self) -> Vec<SessionView>;
    /// Every thread record as `(threadId, record)`, in insertion order (`ctx.threadRecords`); the record names its
    /// workspace.
    fn thread_records(&self) -> Vec<(String, ThreadRecord)>;
    /// `ctx.persist(record)`.
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError>;
    /// `ctx.view(record)`.
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView;
    /// `bus.emit(event)`; a listener's failure reaches the caller (rule 3.6).
    fn emit(&self, event: Event) -> Result<(), HostError>;
    /// `ctx.entryOf(id, origin, opts)`: the live entry the caller may drive, after `ready()` and the boot work.
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError>;
    /// `ctx.spawnGuard(act, origin)`: refuses an act the caller's tree may not take; for `fork` the answer frees the
    /// slot it took.
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError>;
    /// `ctx.kindOf(computer)`.
    fn kind_of(&self, computer: &str) -> WorkspaceKind;
    /// `ctx.placeName(id)`.
    fn place_name(&self, place_id: &str) -> String;
    /// `ctx.copyBlocked(entry)`: refuses acting in a copy whose computer blocks it (#1383).
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError>;
    /// `ctx.withDaemon(entry, work)`.
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<serde_json::Value, HostError>;
    /// `ctx.onThisComputer(work)`.
    async fn on_this_computer(&self, work: HereWork) -> Result<serde_json::Value, HostError>;
    /// `ctx.gitHere(cwd, args, timeoutMs, input)`.
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError>;
    /// `ctx.checkoutOf(record)`.
    fn checkout_of(&self, record: &WorkspaceRecord) -> String;
    async fn preferences(&self) -> Result<Preferences, HostError>;
    /// `ctx.writeDaemonRoots(entry, strict?)`.
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError>;
    /// The home folder of this computer (`os.homedir()`), for `homeShortened`.
    fn home_dir(&self) -> String;
    /// `entry.machine.id`.
    fn machine_id(&self, entry: &Entry) -> String;
}

/// The view of the entry's record as it stands.
pub fn view_of(base: &dyn Base, entry: &Entry) -> WorkspaceView {
    todo!()
}

/// The record as it stands, cloned out of the entry so no borrow crosses an await (rule 3.3).
pub fn record_of(entry: &Entry) -> WorkspaceRecord {
    todo!()
}
