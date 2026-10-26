// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/threads/transcripts.ts` at 65f6b89c7, the rows road only (Proposal 3): the
//! transcripts area. Specs: `spec/transcripts_rows.md`, `spec/transcripts_kept.md`.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use wsp_core::clock::{Cancel, Clock};
use wsp_core::daemon_token::assert_token_shape;
use wsp_core::context::{Joined, StateCell};
use wsp_core::ids::Ids;
use wsp_core::types::events::EventBus;
use wsp_core::types::internal::TranscriptIndex;
use wsp_engine::machine::Machine;
use wsp_store::sqlite_transcripts::TranscriptRows;
use wsp_store::store::Store;
use wsp_wire::views::session::SessionView;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

use super::threads::{Entry, Sessions, ThreadRecords};

pub mod kept;
pub mod rows;

/// The transcript state every area shares (`context.ts`: `transcriptIndex`, `pendingEvents`, `pendingBytes`,
/// `unreadIndexes`).
#[derive(Clone, Debug, Default)]
pub struct TranscriptState {
    pub index: IndexMap<String, TranscriptIndex>,
    pub pending: IndexMap<String, Vec<SessionEvent>>,
    pub pending_bytes: IndexMap<String, u64>,
    pub unread: IndexSet<String>,
}

/// Every `ctx.<x>` the transcripts area calls (README R2).
pub trait TranscriptsCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    fn bus(&self) -> Rc<dyn EventBus>;
    /// `bus.emit(event)` (the read side `EventBus` has no emit).
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) -> Result<(), HostError>;
    fn store(&self) -> Rc<dyn Store>;
    /// `store.transcripts`: always present on the Rust host.
    fn rows(&self) -> Rc<dyn TranscriptRows>;
    fn transcript_state(&self) -> StateCell<TranscriptState>;
    fn sessions(&self) -> Sessions;
    fn thread_records(&self) -> ThreadRecords;
    /// `ctx.indexFlushes`: the sessions document's chain per workspace.
    fn index_flushes(&self) -> StateCell<IndexMap<String, Joined>>;
    fn entry(&self, workspace_id: &str) -> Option<Entry>;
    fn is_live(&self, workspace_id: &str) -> bool;
    /// `ctx.state.closed`.
    fn closed(&self) -> bool;
    fn push_head(&self, thread_id: &str);
    /// `ctx.rowsOn(thread)`.
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView>;
    /// `ctx.latestOn(thread)`.
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    /// `ctx.resumedFact(ws, session, fact)`.
    fn resumed_fact(&self, workspace_id: &str, session: &str, fact: &str) -> Option<String>;
    /// `opts.daemonToken`.
    fn daemon_token_seed(&self) -> Option<String>;
    /// `rotateDaemonToken(machine, token, daemonTokenPathOf(machine, path))`: whether the machine has a daemon.
    fn rotate_daemon_token(&self, machine: Arc<dyn Machine>, token: String, path: Option<String>) -> LocalBoxFuture<'static, Result<bool, HostError>>;
}

/// `{ hasDaemon, at }` (transcripts.ts:529).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DaemonAnswered {
    pub has_daemon: bool,
    pub at: i64,
}

/// The transcripts area (`transcriptsArea`, transcripts.ts:17).
pub struct TranscriptsArea {
    pub(crate) ctx: Rc<dyn TranscriptsCtx>,
    pub(crate) me: Weak<TranscriptsArea>,
    /// transcripts.ts:22
    pub kept_writes: RefCell<IndexMap<String, Joined>>,
    /// transcripts.ts:292, one chain per workspace.
    pub transcript_queue: RefCell<IndexMap<String, Joined>>,
    /// transcripts.ts:513
    pub transcript_timers: RefCell<IndexMap<String, Cancel>>,
    /// transcripts.ts:519
    pub machine_tokens: RefCell<IndexMap<String, String>>,
    /// transcripts.ts:529
    pub daemon_tokens: RefCell<IndexMap<String, DaemonAnswered>>,
    /// transcripts.ts:531
    pub writing: RefCell<IndexMap<String, Shared<LocalBoxFuture<'static, Result<bool, HostError>>>>>,
    /// transcripts.ts:552
    pub flushes_queued: RefCell<IndexMap<String, Joined>>,
    /// transcripts.ts:484
    pub warming: Cell<bool>,
    /// transcripts.ts:615
    pub viewed_marks: RefCell<IndexMap<String, IndexMap<String, String>>>,
    /// transcripts.ts:717
    pub accrued: RefCell<IndexMap<String, f64>>,
}

impl TranscriptsArea {
    /// Builds the area: checks the daemon token seed's shape (a bad one fails the build, transcripts.ts:518), then
    /// registers the `workspace.cost` listener (transcripts.ts:718).
    pub fn new(ctx: Rc<dyn TranscriptsCtx>) -> Result<Rc<TranscriptsArea>, HostError> {
        if let Some(seed) = ctx.daemon_token_seed() {
            assert_token_shape(&seed)?;
        }
        let area = Rc::new_cyclic(|me| TranscriptsArea {
            ctx: ctx.clone(),
            me: me.clone(),
            kept_writes: RefCell::default(),
            transcript_queue: RefCell::default(),
            transcript_timers: RefCell::default(),
            machine_tokens: RefCell::default(),
            daemon_tokens: RefCell::default(),
            writing: RefCell::default(),
            flushes_queued: RefCell::default(),
            warming: Cell::new(false),
            viewed_marks: RefCell::default(),
            accrued: RefCell::default(),
        });
        let weak = Rc::downgrade(&area);
        let _ = area.ctx.bus().on(
            "workspace.cost",
            Rc::new(move |e: &EventUnion| {
                if let EventUnion::WorkspaceCost(v) = e {
                    if let Some(area) = weak.upgrade() {
                        area.accrued.borrow_mut().insert(v.event.workspace_id.clone(), v.event.accrued_usd.0);
                    }
                }
                Ok(())
            }),
        );
        Ok(area)
    }
}
