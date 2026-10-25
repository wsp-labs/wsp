// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/machines/sub-queue.ts at 65f6b89c7: the queue every sub-thread's spawn goes through.
//! Spec: spec/sub_queue.md.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_engine::box_backend::BoxLimits;
use wsp_wire::views::session::{SessionView, ThreadCapWait};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use crate::machines::core_types::{AreaResult, Cancel, Joined, Live, SessionEntry, SubStartRecord, WorkspaceRecord};

/// sub-queue.ts:21. How often a held spawn reads its cloud's limits again for a slot.
pub const QUEUE_SLOT_READ_MS: f64 = 30_000.0;
/// sub-queue.ts:22. How often for the next minute of starts.
pub const QUEUE_MINUTE_READ_MS: f64 = 10_000.0;
/// sub-queue.ts:25. How long a queued record waits for its first turn.
pub const QUEUE_START_WAIT_MS: f64 = 30_000.0;
/// sub-queue.ts:29. What of a start rides the written record, in this order.
pub const KEPT: [&str; 14] = ["prompt", "harness", "model", "effort", "permissionMode", "access", "contextWindow", "fast", "startedBy", "requestId", "attempt", "title", "cwd", "attachments"];

/// The handle a started turn answers (`SessionHandle`): what the queue hands its caller.
#[derive(Clone, Debug, PartialEq)]
pub struct StartedTurn {
    pub id: String,
    pub workspace_id: String,
    pub turn_id: String,
}

/// What `onHeld` is told once a held start is written down (sub-queue.ts:205).
#[derive(Clone, Debug)]
pub struct HeldTold {
    pub view: SessionView,
    pub turn_id: String,
    pub wait: ThreadCapWait,
}

/// The start a queued record's first turn asks (sub-queue.ts:624 `at`): its options as the TS object, the
/// `onHeld` callback beside them since it never rides the written record.
pub struct HoldAsk {
    pub workspace_id: String,
    pub thread_id: String,
    pub tree: IndexMap<String, Value>,
    pub o: serde_json::Map<String, Value>,
    pub on_held: Option<Rc<dyn Fn(HeldTold)>>,
    pub notify: Option<Vec<String>>,
}

/// What the queue reads off the runtime's context (sub-queue.ts:50 and the `ctx.` calls below it).
#[async_trait(?Send)]
pub trait SubQueueCtx {
    fn live(&self, id: &str) -> Option<Live>;
    fn live_all(&self) -> Vec<Live>;
    fn session(&self, turn_id: &str) -> Option<SessionEntry>;
    fn set_session(&self, turn_id: &str, entry: SessionEntry);
    /// Changes the row in place, keeping its map position.
    fn update_session(&self, turn_id: &str, f: &mut dyn FnMut(&mut SessionEntry));
    fn remove_session(&self, turn_id: &str);
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String>;
    fn holds_slot(&self, record: &WorkspaceRecord) -> bool;
    /// `placeDoor?.nameOf(placeId)`.
    fn place_name_of(&self, place_id: &str) -> Option<String>;
    /// `placeDoor?.machinesAt(placeId)`.
    fn machines_at(&self, place_id: &str) -> Option<f64>;
    /// `backendFor(record).limits()` where the backend offers it; `Ok(None)` where it does not.
    async fn limits_of(&self, record: &WorkspaceRecord) -> AreaResult<Option<BoxLimits>>;
    /// `ctx.view(record)` as the TS object.
    fn view(&self, record: &WorkspaceRecord) -> Value;
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()>;
    async fn persist_sessions(&self, workspace_id: &str) -> AreaResult<()>;
    fn emit(&self, event: Value);
    /// `ctx.record(event)`: a transcript event, stamped and emitted.
    fn record(&self, event: Value);
    /// `attach(record, absentMachine("", "sandbox", () => throw said))`.
    fn attach_absent(&self, record: &Rc<RefCell<WorkspaceRecord>>, said: &str) -> Live;
    /// `attach(record, deadMachine(""))`.
    fn attach_dead(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Live;
    /// The record's shared cell, for a record live or just read.
    fn record_cell(&self, workspace_id: &str) -> Option<Rc<RefCell<WorkspaceRecord>>>;
    /// `ctx.drop(id)`: the record goes.
    async fn drop_workspace(&self, workspace_id: &str) -> AreaResult<()>;
    /// `notifyEnd({view, turnId}, notify, tellAs({notifyBy?, notifyRoad?}), {status: "failed", error})`.
    fn notify_failed(&self, view: &SessionView, turn_id: &str, notify: &[String], by: Option<&wsp_wire::views::workspace_more::ThreadScope>, road: Option<&Value>, error: &str);
    fn push_head(&self, thread_id: &str);
    async fn rows_sent(&self);
    async fn put_start(&self, record: &SubStartRecord) -> AreaResult<()>;
    async fn delete_start(&self, workspace_id: &str) -> AreaResult<()>;
    async fn list_starts(&self) -> AreaResult<Vec<SubStartRecord>>;
    /// The sub-spawn area's `standUpQueued`.
    async fn stand_up_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>, posted: Rc<dyn Fn()>, waited: bool) -> AreaResult<()>;
    /// `sessionsApi.start(workspaceId, o, origin)`.
    async fn start_turn(&self, workspace_id: &str, o: serde_json::Map<String, Value>, origin: Option<Caller>) -> AreaResult<StartedTurn>;
    async fn cut_off(&self, entry: &Live);
    async fn unfork(&self, entry: &Live) -> AreaResult<()>;
    fn now(&self) -> f64;
    /// A new UUID (the `Ids` trait).
    fn uuid(&self) -> String;
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> Cancel;
    fn spawn(&self, f: LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
    fn on_event(&self, kind: &str, f: Rc<dyn Fn(&Value)>);
}

/// sub-queue.ts:32 `Held`: one held spawn's start, with the caller still waiting where there is one.
pub(crate) struct Held {
    pub record: SubStartRecord,
    pub name: String,
    pub started: RefCell<Option<Box<dyn FnOnce(StartedTurn)>>>,
    pub failed: RefCell<Option<Box<dyn FnOnce(HostError)>>>,
    pub tell: RefCell<Option<Box<dyn FnOnce(ThreadCapWait)>>>,
    pub written: Cell<bool>,
    pub left: RefCell<Option<Box<dyn FnOnce()>>>,
    pub gone: Joined<()>,
    pub stopped: RefCell<Option<String>>,
}

/// A claim of a slot and a start (sub-queue.ts:106).
pub struct StartClaim {
    pub posted: Rc<dyn Fn()>,
    pub release: Rc<dyn Fn()>,
}

/// sub-queue.ts:49 `subQueueArea(ctx)`.
pub struct SubQueueArea {
    pub(crate) ctx: Rc<dyn SubQueueCtx>,
    pub(crate) me: Weak<SubQueueArea>,
    pub(crate) held: RefCell<IndexMap<String, Rc<Held>>>,
    pub(crate) last_wait: RefCell<IndexMap<String, ThreadCapWait>>,
    pub(crate) starting: RefCell<IndexMap<String, (String, bool)>>,
    pub(crate) refused_until: RefCell<IndexMap<String, f64>>,
    pub(crate) queued_at: RefCell<IndexMap<String, f64>>,
    pub(crate) pumping: RefCell<Option<Joined<()>>>,
    pub(crate) again: Cell<bool>,
    pub(crate) timer: RefCell<Option<Cancel>>,
}

impl SubQueueArea {
    /// sub-queue.ts:49. Builds the area and registers its `"*"` listener (rule 23).
    pub fn new(ctx: Rc<dyn SubQueueCtx>) -> Rc<SubQueueArea> {
        todo!()
    }

    /// sub-queue.ts:66 (rule 2).
    pub(crate) fn queued(&self) -> Vec<Live> {
        todo!()
    }

    /// sub-queue.ts:70 (rule 3).
    pub fn slots(&self, place_id: &str) -> ThreadCapWait {
        todo!()
    }

    /// sub-queue.ts:82 (rule 4).
    pub fn wait_on(&self, place_id: &str, limits: Option<&BoxLimits>) -> Option<ThreadCapWait> {
        todo!()
    }

    /// sub-queue.ts:96 (rule 5).
    pub fn queue_wait(&self, place_id: &str, limits: Option<&BoxLimits>) -> Option<ThreadCapWait> {
        todo!()
    }

    /// sub-queue.ts:106 (rule 6).
    pub fn claim_start(&self, workspace_id: &str, place_id: &str) -> StartClaim {
        todo!()
    }

    /// sub-queue.ts:119 (rule 7).
    pub fn cloud_full(&self, place_id: &str, limits: Option<&BoxLimits>) -> ThreadCapWait {
        todo!()
    }

    /// sub-queue.ts:127 (rule 8).
    pub async fn enqueue(&self, record: &Rc<RefCell<WorkspaceRecord>>, wait: Option<ThreadCapWait>, refused: bool) -> AreaResult<()> {
        todo!()
    }

    /// sub-queue.ts:143 (rule 9).
    pub fn attach_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>, wait: Option<&ThreadCapWait>) {
        todo!()
    }

    /// sub-queue.ts:151 (rule 10).
    pub(crate) fn show_row(&self, h: &Held, wait: Option<&ThreadCapWait>) {
        todo!()
    }

    /// sub-queue.ts:172.
    pub(crate) fn hold_of(&self, record: SubStartRecord) -> Rc<Held> {
        todo!()
    }

    /// sub-queue.ts:183 (rule 11).
    pub fn hold_queued(&self, at: HoldAsk, origin: Option<&Caller>) -> LocalBoxFuture<'static, Result<StartedTurn, HostError>> {
        todo!()
    }

    /// sub-queue.ts:220 (rule 12).
    pub(crate) fn forget(&self, workspace_id: &str) {
        todo!()
    }

    /// sub-queue.ts:234 (rule 14).
    pub(crate) async fn drop_held(&self, h: &Rc<Held>, reason: &str, failed_spawn: bool) -> AreaResult<()> {
        todo!()
    }

    /// sub-queue.ts:250 (rule 15).
    pub(crate) async fn fail_first(&self, h: &Rc<Held>, record: &Rc<RefCell<WorkspaceRecord>>, e: crate::machines::core_types::AreaError) -> AreaResult<()> {
        todo!()
    }

    /// sub-queue.ts:280 (rule 16).
    pub async fn drop_queued(&self, turn_id: &str) -> bool {
        todo!()
    }

    /// sub-queue.ts:294 (rule 17). `None` for a record the queue holds no start of.
    pub fn delete_queued(&self, workspace_id: &str) -> Option<LocalBoxFuture<'static, ()>> {
        todo!()
    }

    /// sub-queue.ts:305 (rule 18).
    pub fn pump_queue(&self) -> Joined<()> {
        todo!()
    }

    /// sub-queue.ts:332 (rule 20).
    pub(crate) async fn pump_once(&self) -> AreaResult<bool> {
        todo!()
    }

    /// sub-queue.ts:388 (rule 21).
    pub(crate) async fn let_through(&self, entry: Live, h: Rc<Held>) {
        todo!()
    }

    /// sub-queue.ts:454 (rule 22).
    pub async fn resume_queue(&self) -> AreaResult<()> {
        todo!()
    }
}
