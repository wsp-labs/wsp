// SPDX-License-Identifier: AGPL-3.0-only
//! What every projects area shares: the live entry handle, the shared maps as the context hands them out, a daemon
//! ask, and the base half of every area's context trait. Each name a joined crate will give is here, so the swap is
//! this one file. `LiveEntry` stands in for wsp-core's `LiveWorkspace`: that struct holds the engine's `Workspace`,
//! which no test can build while `Workspace::new` is a stub, and ruling 2 of #2448 (6401022701) changes it to share
//! one record between entries. This one holds the record shared, the join fields the areas read and write, and no
//! machine: every machine and engine call goes through the context (`ws_*`, `machine_*`). The wsp-core join replaces
//! it with the shared-record `LiveWorkspace` and `Entry` with machines a's alias.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::pending::Store;
use tokio_util::sync::CancellationToken;
use wsp_core::types::wiring::{DeleteSaid, Joined, SizeAsk, WakeAsk, WorkspaceRecord};
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::pull_request::PullRequestSeen;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

/// A workspace this host holds in memory (types/wiring.ts:416-477), the fields these areas touch. Never stored.
pub struct LiveEntry {
    /// Shared by every entry of one workspace, as the TS shares one object.
    pub record: Rc<RefCell<WorkspaceRecord>>,
    pub creating: bool,
    pub unchecked: bool,
    pub waking: Option<Joined<WorkspaceView>>,
    pub napping: Option<Joined<WorkspaceView>>,
    pub deleting: Option<Joined<()>>,
    pub wake_stop: Option<CancellationToken>,
    pub wake_said: Option<String>,
    pub wake_ask: Option<WakeAsk>,
    pub delete_said: Option<DeleteSaid>,
    pub resize_to: Option<SizeAsk>,
    pub nap_wait_ms: Option<u64>,
    pub checkout: Option<Checkout>,
    pub pr: Option<PullRequestSeen>,
}

impl LiveEntry {
    /// A fresh entry over a record, every join field empty: what `ctx.attach` makes. A struct literal, so tests can
    /// build entries before any body is written.
    pub fn new(record: WorkspaceRecord) -> Entry {
        Rc::new(RefCell::new(LiveEntry {
            record: Rc::new(RefCell::new(record)),
            creating: false,
            unchecked: false,
            waking: None,
            napping: None,
            deleting: None,
            wake_stop: None,
            wake_said: None,
            wake_ask: None,
            delete_said: None,
            resize_to: None,
            nap_wait_ms: None,
            checkout: None,
            pr: None,
        }))
    }
}

/// A workspace this host holds, shared by every area that reads or writes it.
pub type Entry = Rc<RefCell<LiveEntry>>;
/// `ctx.live`, in insertion order (rule 3.11).
pub type Live = Rc<RefCell<IndexMap<String, Entry>>>;
/// `ctx.projectsHeld`, in insertion order.
pub type ProjectsHeld = Rc<RefCell<IndexMap<String, ProjectView>>>;
/// What `spawnGuard` hands back: the slot's release, safe to call more than once.
pub type FreeSlot = Rc<dyn Fn()>;
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

/// `entryOf`'s options: the thread the caller names, the act it asks for, and `now` (skip the boot wait).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntryAsk {
    pub thread: Option<String>,
    pub act: Option<String>,
    pub now: bool,
}

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
    /// Every thread record as `(threadId, workspaceId)`, in insertion order (`ctx.threadRecords`).
    fn thread_workspaces(&self) -> Vec<(String, String)>;
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
