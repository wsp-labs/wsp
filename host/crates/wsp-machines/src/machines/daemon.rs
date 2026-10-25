// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/daemon.ts` at 65f6b89c7: what the runtime settles with a machine's daemon, its
//! revive, and the detached send. Spec: spec/daemon.md.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;

use wsp_core::types::internal::{KindModule, MachineMoment};
use wsp_core::types::pending::Clock;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::MachineBackend;
use wsp_wire::views::session::{SessionView, ThreadCapWait};
use wsp_wire::views::workspace::{Caller, ReachState};
use wsp_wire::views::workspace_more::WorkspaceStatus;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::HostError;

use super::machines::{Entry, Ids, Row, Unreached};

pub use wsp_core::context::{DetachedOutcome, DetachedSend};

/// What `sessions.start` is asked by a detached send (daemon.ts:32).
pub struct DetachedStart {
    pub prompt: String,
    pub thread: Option<String>,
    pub request_id: String,
    /// `onHeld`: the row and the wait, the moment the computer holds the message back.
    pub on_held: Box<dyn Fn(SessionView, ThreadCapWait)>,
}

/// What `sessions.start` answers once the turn is on its way (its `outcome` and `view()`).
#[derive(Clone, Debug, PartialEq)]
pub struct Started {
    pub outcome: SessionStartOutcome,
    pub view: SessionView,
}

/// Every call the daemon area makes on the rest of the runtime.
#[async_trait(?Send)]
pub trait DaemonRuntime {
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    fn warn(&self, line: &str);
    /// `ctx.daemonHelloTimeoutMs`.
    fn daemon_hello_timeout_ms(&self) -> u64;
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>>;
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>>;
    /// `ctx.daemonNotes`.
    fn daemon_notes(&self) -> Rc<RefCell<IndexMap<String, String>>>;
    /// `ctx.state.closed`.
    fn closed(&self) -> bool;
    async fn sessions_start(&self, workspace_id: &str, ask: DetachedStart, origin: Option<&Caller>) -> Result<Started, HostError>;
    async fn push_status(&self, entry: &Entry) -> Result<(), HostError>;
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError>;
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String>;
    fn module_of(&self, kind: &str) -> Rc<dyn KindModule>;
    /// `ctx.projectHeld(id).path`.
    fn project_path(&self, project_id: &str) -> String;
    fn checkout_of(&self, record: &WorkspaceRecord) -> String;
    fn backend_for(&self, record: &WorkspaceRecord) -> Arc<dyn MachineBackend>;
}

/// The daemon area: the maps and joins daemon.ts keeps in its closure.
pub struct DaemonArea {
    pub(crate) rt: Rc<dyn DaemonRuntime>,
    /// `rootsWrites`: the tail of each machine's roots chain.
    pub(crate) roots_writes: RefCell<IndexMap<String, futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>>>,
    /// `daemonSyncs`: the sync each machine has in flight.
    pub(crate) daemon_syncs: RefCell<IndexMap<String, futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>>>,
    /// `daemonRevivals`.
    pub(crate) daemon_revivals: RefCell<IndexMap<String, futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>>>,
    pub(crate) revived_at: RefCell<IndexMap<String, MachineMoment>>,
    pub(crate) unread_at: RefCell<IndexMap<String, MachineMoment>>,
    pub(crate) unreached: Rc<RefCell<IndexMap<String, Unreached>>>,
    /// The waits on a turn's end and on a queued start's event, woken from `on_event`.
    pub(crate) listeners: RefCell<Vec<Box<dyn Fn(&EventUnion) -> bool>>>,
}

impl DaemonArea {
    /// `daemonArea(ctx)` (daemon.ts:9).
    pub fn new(rt: Rc<dyn DaemonRuntime>) -> Rc<Self> {
        todo!()
    }

    /// The bus events this area waits on (`session.queued`, `session.done`, `session.end`), called by the wiring for
    /// every event in emit order.
    pub fn on_event(&self, event: &EventUnion) {
        todo!()
    }

    /// `sendDetached` (daemon.ts:14), spec rule 1.
    pub async fn send_detached(self: &Rc<Self>, workspace_id: &str, prompt: &str, thread: Option<&str>, origin: Option<&Caller>) -> Result<DetachedSend, HostError> {
        todo!()
    }

    /// `toFirstThread` (daemon.ts:48), spec rule 2.
    pub async fn to_first_thread(self: &Rc<Self>, workspace_id: &str, prompt: &str, origin: Option<&Caller>) -> Result<DetachedSend, HostError> {
        todo!()
    }

    /// `writeDaemonRoots` (daemon.ts:88), spec rule 5.
    pub async fn write_daemon_roots(self: &Rc<Self>, entry: &Entry, strict: bool) -> Result<(), HostError> {
        todo!()
    }

    /// `turnRuns` (daemon.ts:111): a queued row counts.
    pub fn turn_runs(&self, workspace_id: &str) -> bool {
        todo!()
    }

    /// `deployDaemonOn` (daemon.ts:147), spec rule 8: the deploy is the kind module's, called on the entry.
    pub async fn deploy_daemon_on(&self, entry: &Entry) -> Result<Option<String>, HostError> {
        todo!()
    }

    /// `syncDaemon` (daemon.ts:190), spec rule 10; joined per machine. It never rejects for a deploy that failed.
    pub fn sync_daemon(self: &Rc<Self>, entry: &Entry) -> futures::future::LocalBoxFuture<'static, Result<(), HostError>> {
        todo!()
    }

    /// The machines with a sync in flight (`daemonSyncs.keys()`).
    pub fn daemon_syncs(&self) -> Vec<String> {
        todo!()
    }

    /// `daemonSettled` (daemon.ts:262), spec rule 11.
    pub async fn daemon_settled(&self, entry: &Entry) {
        todo!()
    }

    pub fn revived_at(&self) -> IndexMap<String, MachineMoment> {
        todo!()
    }

    pub fn unread_at(&self) -> Rc<RefCell<IndexMap<String, MachineMoment>>> {
        todo!()
    }

    /// `unreached`: the map the machines area writes and this one reads.
    pub fn unreached(&self) -> Rc<RefCell<IndexMap<String, Unreached>>> {
        todo!()
    }

    /// `unreachedOf` (daemon.ts:274), spec rule 12.
    pub fn unreached_of(&self, entry: &Entry) -> Option<String> {
        todo!()
    }

    /// `reviveDaemon` (daemon.ts:292), spec rule 13: starts its work and returns.
    pub fn revive_daemon(self: &Rc<Self>, entry: &Entry, reach: ReachState) {
        todo!()
    }

    /// `offerDaemonAgain` (daemon.ts:343), spec rule 14.
    pub fn offer_daemon_again(self: &Rc<Self>, entry: &Entry, polled: &WorkspaceStatus) {
        todo!()
    }

    /// `readVersionAgain` (daemon.ts:349), spec rule 15.
    pub fn read_version_again(self: &Rc<Self>, entry: &Entry, polled: &WorkspaceStatus) {
        todo!()
    }
}
