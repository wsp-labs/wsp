// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/threads/threads.ts` at 65f6b89c7: the threads area. One struct, its state, and the
//! context it reads; each unit adds its methods in a file under `threads/`. Specs: `spec/threads-b-README.md`,
//! `spec/threads_{tree,kept,cap,owed,turn_end}.md`.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_core::clock::Clock;
use wsp_core::context::{Joined, StateCell};
use wsp_core::ids::Ids;
use wsp_core::types::events::EventBus;
use wsp_core::types::internal::{KeptProcess, SessionEntry, Taken, ThreadRecord, TurnLive};
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_engine::machine::Machine;
use wsp_store::store::Store;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::usage::AccountLimit;
use wsp_wire::views::session::{SessionView, ThreadPrompt};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

pub mod cap;
pub mod kept;
pub mod owed;
pub mod tree;
pub mod turn_end;

pub use cap::CapHeld;
pub use owed::{read_owed, Owed, OwedImages, PersonRow};
pub use tree::{computer_silent, SILENT_MARKS};
pub use turn_end::SteerOut;

/// The runtime's rows, one per turn, keyed by row id (`context.ts:209`).
pub type Sessions = IndexMap<String, SessionEntry>;

/// A thread's record as the runtime holds it: the stored record and the workspace it is on (`context.ts:213`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeldThread {
    pub workspace_id: String,
    pub record: ThreadRecord,
}

/// `threadRecords` (`context.ts:213`).
pub type ThreadRecords = IndexMap<String, HeldThread>;

/// What an area reads off a live workspace at the moment it asks: its record and its machine. The join builds it
/// from wsp-core's `LiveWorkspace`.
#[derive(Clone)]
pub struct Entry {
    pub record: WorkspaceRecord,
    pub machine: Arc<dyn Machine>,
}

/// What a project held reads as here (`projectHeld`): the fields these areas read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeldProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub computer: String,
    /// `git.top`; `None` where git holds no repo in the folder (`git` absent).
    pub git_top: Option<String>,
}

/// A daemon ask that failed: the error, and the daemon's own refusal code where it refused (`DaemonRefusal.code`).
#[derive(Clone, Debug)]
pub struct AskFailure {
    pub error: HostError,
    pub code: Option<String>,
}

/// One turn's row as a function reads it, copied out of `sessions` so no borrow is held across a call out.
#[derive(Clone, Debug)]
pub struct TurnRow {
    pub view: SessionView,
    pub turn_id: String,
    pub turn_live: Option<TurnLive>,
}

/// A place door event the kept agents wait on (`place.present`, `place.removed`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceMove {
    Present(String),
    Removed(String),
    Other,
}

/// `{readAt}` for `mark`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadMark {
    pub read_at: i64,
}

/// Every `ctx.<x>` the threads area calls (README R2). The wsp-threads join implements it on the runtime's context.
pub trait ThreadsCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    fn bus(&self) -> Rc<dyn EventBus>;
    fn store(&self) -> Rc<dyn Store>;
    fn sessions(&self) -> StateCell<Sessions>;
    fn thread_records(&self) -> StateCell<ThreadRecords>;
    /// `ctx.state.closing`.
    fn closing(&self) -> bool;
    /// `live.get(ws)` as an entry; `None` when the workspace is not live.
    fn entry(&self, workspace_id: &str) -> Option<Entry>;
    /// Every live record, in `live`'s order.
    fn live_records(&self) -> Vec<WorkspaceRecord>;
    /// `ctx.transcriptIndex.get(ws)?.taken.get(request)`.
    fn taken(&self, workspace_id: &str, request_id: &str) -> Option<Taken>;

    // The place door (`ctx.placeDoor`), `None`/false where the host has none.
    fn has_place_door(&self) -> bool;
    /// `door.link(place) !== undefined`.
    fn place_linked(&self, place: &str) -> bool;
    /// `door.on(listener)`; the answer removes it. `None` where there is no door.
    fn on_place_move(&self, listener: Rc<dyn Fn(&PlaceMove)>) -> Option<Box<dyn Fn()>>;
    /// `door.folderComputer(place)?.shape.memMb`.
    fn place_mem_mb(&self, place: &str) -> Option<f64>;
    /// `door.threadsAt(place)`.
    fn place_threads_at(&self, place: &str) -> Option<i64>;
    /// `door.nameOf(place)`.
    fn place_name(&self, place: &str) -> String;
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String>;
    fn provider_of(&self, record: &WorkspaceRecord) -> Option<String>;

    fn revoke_device(&self, device_id: &str) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn proc_scopes_moved(&self);
    fn root_of(&self, thread_id: &str) -> Option<String>;
    /// `ctx.sessionsApi.interrupt(row, caller)`, answering its `outcome` word.
    fn interrupt(&self, row_id: &str, caller: Option<Caller>) -> LocalBoxFuture<'static, Result<String, HostError>>;
    /// `ctx.sessionsApi.start(ws, options, caller)`.
    fn start(
        &self,
        workspace_id: &str,
        options: wsp_core::types::api::StartOptions,
        caller: Option<Caller>,
    ) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>>;
    fn stop_try(&self, row_id: &str) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn mark(&self, threads: Vec<String>, mark: ReadMark, caller: Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn sub_line(&self, workspace_id: &str, since: i64) -> LocalBoxFuture<'static, Option<String>>;
    fn boot_work(&self, workspace_id: &str) -> Option<Joined>;
    fn kept_writes(&self, workspace_id: &str) -> Option<Joined>;
    fn keep_sent_images(&self, workspace_id: &str, thread_id: &str, request_id: Option<String>, attachments: Vec<Attachment>) -> LocalBoxFuture<'static, ()>;
    /// `ctx.record(event)`: the transcripts area stamps, appends and emits it.
    fn record(&self, event: SessionEvent);
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()>;
    fn open_thread(&self, workspace_id: &str, thread_id: &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>;
    fn idle_hold(&self, workspace_id: &str);
    fn idle_release(&self, workspace_id: &str);
    fn port_roots_moved(&self, workspace_id: &str);
    fn end_snooze_for(&self, view: &SessionView);
    /// `ctx.withDaemon(entry, ask => ask(frame))`.
    fn with_daemon(&self, entry: &Entry, frame: Value) -> LocalBoxFuture<'static, Result<Value, AskFailure>>;
    fn checkout_of(&self, record: &WorkspaceRecord) -> String;
    fn project_held(&self, project: &str) -> HeldProject;
    /// `ctx.reachOf(entry)`: `reachable` and the other reach words.
    fn reach_of(&self, entry: &Entry) -> String;
    /// `opts.turnSnapshotMs`.
    fn turn_snapshot_ms(&self) -> Option<i64>;
    /// `opts.vault?.()`, empty where none is wired.
    fn vault(&self) -> IndexMap<String, String>;
    fn vault_on(&self, entry: &Entry) -> IndexMap<String, String>;
    /// `ctx.moduleOf(kind).loginStands(entry, harness)`.
    fn login_stands(&self, entry: &Entry, harness: &str) -> bool;
    fn computer_of(&self, entry: &Entry) -> String;
    /// `ctx.ledger.limits()`.
    fn ledger_limits(&self) -> LocalBoxFuture<'static, Result<Vec<AccountLimit>, HostError>>;
}

/// The threads area (`threadsArea`, `threads.ts:70`). Every field is the TS closure's state of the same name.
pub struct ThreadsArea {
    pub(crate) ctx: Rc<dyn ThreadsCtx>,
    /// The area itself, for the timers and tasks it starts (`Rc::new_cyclic`).
    pub(crate) me: Weak<ThreadsArea>,
    /// threads.ts:102
    pub kept_agents: RefCell<IndexMap<String, Rc<RefCell<KeptProcess>>>>,
    /// threads.ts:173
    pub host_writes: RefCell<IndexMap<String, Joined>>,
    /// threads.ts:367
    pub lead_asks: RefCell<IndexMap<String, ThreadPrompt>>,
    /// threads.ts:374
    pub cap_held: RefCell<IndexMap<String, CapHeld>>,
    /// threads.ts:375
    pub cap_order: Cell<u64>,
    /// threads.ts:376, one chain for the host.
    pub held_writes: RefCell<Option<Joined>>,
    /// threads.ts:384
    pub borrowed: RefCell<IndexMap<String, String>>,
    /// threads.ts:588
    pub held_lines: RefCell<IndexMap<String, Vec<Rc<RefCell<Owed>>>>>,
    /// threads.ts:591
    pub sending: RefCell<IndexMap<String, Rc<RefCell<Owed>>>>,
    /// threads.ts:636
    pub prompt_waits: RefCell<IndexMap<String, Rc<dyn Fn()>>>,
    /// threads.ts:640
    pub dark_tries: RefCell<IndexMap<String, Rc<dyn Fn()>>>,
    /// threads.ts:864
    pub steers_out: RefCell<IndexMap<String, SteerOut>>,
    /// threads.ts:962
    pub checkpoints_landing: RefCell<IndexMap<String, Joined>>,
}

impl ThreadsArea {
    /// Builds the area and registers its bus listeners in the TS order: the borrowed slots' release on
    /// `session.end` (threads.ts:385), then the held lines' wake on `workspace.woken` and `workspace.upgraded`
    /// (threads.ts:892).
    pub fn new(ctx: Rc<dyn ThreadsCtx>) -> Rc<ThreadsArea> {
        todo!()
    }
}

/// `Omit<KeptProcess, "file" | "boxFile" | "usedAt" | "cancel">`: what a turn hands `hold_kept` and `hold_warm`.
pub struct KeptHand {
    pub workspace_id: String,
    pub agent: Box<dyn wsp_adapters::port::KeptAgent>,
    pub pid: Option<u32>,
    pub launch: wsp_core::types::internal::KeptLaunch,
    pub session: Option<String>,
    pub warm: Option<wsp_core::types::internal::WarmKeep>,
    pub turn_token: String,
    pub scope_device_id: Option<String>,
    pub waiting: Rc<Cell<bool>>,
}

/// A future the area answers that fails with the error its caller sees.
pub type Answer<T> = LocalBoxFuture<'static, Result<T, HostError>>;
