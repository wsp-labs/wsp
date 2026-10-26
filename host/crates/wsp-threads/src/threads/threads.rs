// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/threads/threads.ts` at 65f6b89c7: the threads area. One struct, its state, and the
//! context it reads; each unit adds its methods in a file under `threads/`. Specs: `spec/threads-b-README.md`,
//! `spec/threads_{tree,kept,cap,owed,turn_end}.md`.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_account::account::core_types::AccountError;
use wsp_adapters::port::PermissionAsk;
use wsp_core::clock::Clock;
use wsp_core::context::Joined;
use wsp_core::ids::Ids;
use wsp_core::types::events::{EventBus, EventListener};
use wsp_core::types::internal::{KeptProcess, Taken, ThreadRecord, ThreadStamps};
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_store::store::Store;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::usage::AccountLimit;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::{Caller, ProjectView, ReachState};
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::SessionInterruptResult;
use wsp_words::exit::HostError;

pub mod cap;
pub mod kept;
pub mod owed;
pub mod tree;
pub mod turn_end;

pub use cap::{CapAt, CapHeld};
pub use owed::{read_owed, Owed, OwedImages, PersonRow};
pub use tree::{computer_silent, NotifyOn, SILENT_MARKS};
pub use turn_end::{CheckpointAsk, SteerOut, TurnChangesAsk};

/// A live workspace as every area holds it, and a row the runtime holds by reference: the machines area's own items.
pub use wsp_machines::machines::machines::{Entry, Row};

/// The runtime's rows, one per turn, keyed by row id in JS Map order (`context.ts:209`). The map is the one
/// `wsp_machines`' `sessions()` returns; no borrow of it or of a row is held across an await (rule 3.3).
pub type Sessions = Rc<RefCell<IndexMap<String, Row>>>;

/// `threadRecords` (`context.ts:213`): every thread's record by its id, each carrying its `workspace_id`.
pub type ThreadRecords = Rc<RefCell<IndexMap<String, ThreadRecord>>>;

/// The box a turn's exec stream reads to know it waits on something outside its process (`{ on: boolean }`,
/// launch.ts:104); the reader handed to the adapter is `|| waiting.on`.
pub type Waiting = Rc<Cell<bool>>;

/// A place door event the kept agents wait on (`place.present`, `place.removed`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceMove {
    Present(String),
    Removed(String),
    Other,
}

/// Every `ctx.<x>` the threads area calls (README R2). The wsp-threads join implements it on the runtime's context.
pub trait ThreadsCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    fn bus(&self) -> Rc<dyn EventBus>;
    fn store(&self) -> Rc<dyn Store>;
    fn sessions(&self) -> Sessions;
    fn thread_records(&self) -> ThreadRecords;
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
    /// `ctx.sessionsApi.interrupt(row, caller)`, answering its result.
    fn interrupt(&self, row_id: &str, caller: Option<Caller>) -> LocalBoxFuture<'static, Result<SessionInterruptResult, HostError>>;
    /// `ctx.sessionsApi.start(ws, options, caller)`.
    fn start(
        &self,
        workspace_id: &str,
        options: wsp_core::types::api::StartOptions,
        caller: Option<Caller>,
    ) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>>;
    fn stop_try(&self, row_id: &str) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn mark(&self, threads: Vec<String>, mark: ThreadStamps, caller: Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn sub_line(&self, workspace_id: &str, since: i64) -> LocalBoxFuture<'static, Option<String>>;
    fn boot_work(&self, workspace_id: &str) -> Option<Joined>;
    fn kept_writes(&self, workspace_id: &str) -> Option<Joined>;
    fn keep_sent_images(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: Option<String>,
        attachments: Vec<Attachment>,
    ) -> LocalBoxFuture<'static, ()>;
    /// `ctx.record(event)`: the transcripts area stamps, appends and emits it.
    fn record(&self, event: SessionEvent);
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()>;
    fn open_thread(&self, workspace_id: &str, thread_id: &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>;
    fn idle_hold(&self, workspace_id: &str);
    fn idle_release(&self, workspace_id: &str);
    fn port_roots_moved(&self, workspace_id: &str);
    fn end_snooze_for(&self, view: &SessionView);
    /// `ctx.withDaemon(entry, ask => ask(frame))`.
    fn with_daemon(&self, entry: &Entry, frame: Value) -> LocalBoxFuture<'static, Result<Value, AccountError>>;
    fn checkout_of(&self, record: &WorkspaceRecord) -> String;
    fn project_held(&self, project: &str) -> ProjectView;
    /// `ctx.reachOf(entry)`: `reachable` and the other reach words.
    fn reach_of(&self, entry: &Entry) -> ReachState;
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
    /// TS `void promise`: work that outlives its caller. The host runs it on its `LocalSet`, so where it runs is its to say.
    fn spawn(&self, work: LocalBoxFuture<'static, ()>);
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
    pub lead_asks: Rc<RefCell<IndexMap<String, PermissionAsk>>>,
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
    pub checkpoints_landing: Rc<RefCell<IndexMap<String, Joined>>>,
}

impl ThreadsArea {
    /// Builds the area and registers its bus listeners in the TS order: the borrowed slots' release on
    /// `session.end` (threads.ts:385), then the held lines' wake on `workspace.woken` and `workspace.upgraded`
    /// (threads.ts:892).
    pub fn new(ctx: Rc<dyn ThreadsCtx>) -> Rc<ThreadsArea> {
        let bus = ctx.bus();
        let area = Rc::new_cyclic(|me| ThreadsArea {
            ctx: ctx.clone(),
            me: me.clone(),
            kept_agents: RefCell::default(),
            host_writes: RefCell::default(),
            lead_asks: Rc::default(),
            cap_held: RefCell::default(),
            cap_order: Cell::new(0),
            held_writes: RefCell::default(),
            borrowed: RefCell::default(),
            held_lines: RefCell::default(),
            sending: RefCell::default(),
            prompt_waits: RefCell::default(),
            dark_tries: RefCell::default(),
            steers_out: RefCell::default(),
            checkpoints_landing: Rc::default(),
        });
        let weak = Rc::downgrade(&area);
        let borrowed_listener: EventListener = Rc::new(move |e: &EventUnion| {
            let EventUnion::SessionEnd(end) = e else {
                return Ok(());
            };
            let Some(turn) = end.event.turn_id.clone() else {
                return Ok(());
            };
            if let Some(area) = weak.upgrade() {
                let mut borrowed = area.borrowed.borrow_mut();
                borrowed.shift_remove(&turn);
                borrowed.retain(|_, lender| *lender != turn);
            }
            Ok(())
        });
        let _ = bus.on("*", borrowed_listener);
        for ty in ["workspace.woken", "workspace.upgraded"] {
            let weak = Rc::downgrade(&area);
            let held_listener: EventListener = Rc::new(move |e: &EventUnion| {
                let workspace_id = match e {
                    EventUnion::WorkspaceWoken(woken) if ty == "workspace.woken" => woken.event.workspace_id.clone(),
                    EventUnion::WorkspaceUpgraded(upgraded) if ty == "workspace.upgraded" => upgraded.event.workspace_id.clone(),
                    _ => return Ok(()),
                };
                let Some(area) = weak.upgrade() else {
                    return Ok(());
                };
                let lines = area.held_lines.borrow_mut().shift_remove(&workspace_id).unwrap_or_default();
                for line in lines {
                    let owed = line.borrow().clone();
                    area.deliver(owed, true);
                }
                Ok(())
            });
            let _ = bus.on(ty, held_listener);
        }
        area
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
