// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/machines/reach.ts at 65f6b89c7: a row's status outside the poll, the refusals and
//! the disk reading it carries, and the roads to a machine's own daemon. Specs: spec/reach_disk.md and
//! spec/reach_daemon.md.
#![allow(clippy::result_large_err)]
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_core::reach::DaemonReach;
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{DaemonReachView, ProjectView, ReachState, WorkspacePhase};
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_words::exit::HostError;

use crate::machines::area_error::AreaResult;
use crate::machines::machines::Entry;
use wsp_core::clock::Cancel;
use wsp_core::types::internal::SessionEntry;
use wsp_core::types::wiring::WorkspaceRecord;

pub mod daemon;
pub mod disk;

pub use disk::{DISK_CLOSE_WAIT_MS, DISK_READ_MS};

/// What the reach area reads off the runtime's context (reach.ts:22 and the `ctx.` calls below it).
#[async_trait(?Send)]
pub trait ReachCtx {
    fn live(&self, id: &str) -> Option<Entry>;
    fn live_all(&self) -> Vec<Entry>;
    fn session_entries(&self) -> Vec<SessionEntry>;
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError>;
    fn pause_keeps_disk(&self, backend: &dyn MachineBackend) -> bool;
    /// engine `sweepLeftovers(machine)`.
    async fn sweep_leftovers(&self, machine: &Arc<dyn Machine>) -> AreaResult<()>;
    /// `ctx.state.closing`.
    fn closing(&self) -> bool;
    fn provider_of(&self, record: &WorkspaceRecord) -> Option<String>;
    /// `ctx.rowReason(entry)`.
    fn row_reason(&self, entry: &Entry) -> Option<String>;
    /// `ctx.unreachedOf(entry)`.
    fn unreached_of(&self, entry: &Entry) -> Option<String>;
    /// `ctx.moduleOf(kind).hasDaemon(entry)`.
    fn kind_has_daemon(&self, entry: &Entry) -> AreaResult<bool>;
    /// `ctx.view(record)` as the TS object.
    fn view(&self, record: &WorkspaceRecord) -> Value;
    /// status.ts `machineStateOf(phase)` as its word.
    fn machine_state_of(&self, phase: &WorkspacePhase) -> String;
    /// `ctx.idle.idleAt(id)`.
    fn idle_at(&self, workspace_id: &str) -> Option<f64>;
    /// `lifecycleOf(entry).budgets.daemonAnswersMs`.
    fn daemon_answers_ms(&self, entry: &Entry) -> AreaResult<f64>;
    fn daemon_hello_timeout_ms(&self) -> f64;
    async fn daemon_token_of(&self, machine: &Arc<dyn Machine>) -> AreaResult<Option<String>>;
    /// RT/reach.ts `connectDaemon({previewUrl, token, onEvent, heartbeatMs})`.
    fn connect_daemon(
        &self,
        preview_url: &str,
        token: &str,
        on_event: Rc<dyn Fn(DaemonEvent)>,
        heartbeat_ms: Option<i64>,
    ) -> Rc<dyn DaemonReach>;
    fn project_held(&self, id: &str) -> AreaResult<ProjectView>;
    /// `withDaemon(entry, ask => ask(frame))`.
    async fn with_daemon(&self, entry: &Entry, frame: Value) -> AreaResult<Value>;
    /// `onThisComputer((ask, home) => ask(frame(home)))`.
    async fn on_this_computer(&self, frame: Box<dyn Fn(&str) -> Value>) -> AreaResult<Value>;
    fn checkout_of(&self, record: &WorkspaceRecord) -> AreaResult<String>;
    /// `servedByItsComputer(entry) !== undefined`.
    fn served_by_its_computer(&self, entry: &Entry) -> bool;
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()>;
    fn emit(&self, event: Value);
    fn record(&self, event: Value);
    fn now(&self) -> f64;
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> Cancel;
    /// A future that settles after `ms` on the clock.
    fn sleep(&self, ms: f64) -> LocalBoxFuture<'static, ()>;
    fn spawn(&self, f: LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
}

/// A fact held for one machine (reach.ts:31, :34, :48).
#[derive(Clone, Debug, PartialEq)]
pub struct ForMachine<T> {
    pub machine_id: String,
    pub value: T,
}

/// reach.ts:55: how full the disk last read, and whether its crossing was said.
#[derive(Clone, Debug, PartialEq)]
pub struct DiskHeld {
    pub machine_id: String,
    pub pct: f64,
    pub told: bool,
}

/// reach.ts:21 `reachArea(ctx)`.
pub struct ReachArea {
    pub(crate) ctx: Rc<dyn ReachCtx>,
    pub(crate) me: Weak<ReachArea>,
    pub polled_reach: RefCell<IndexMap<String, ForMachine<ReachState>>>,
    pub nap_refusals: Rc<RefCell<IndexMap<String, ForMachine<String>>>>,
    pub stop_refusals: Rc<RefCell<IndexMap<String, ForMachine<String>>>>,
    pub(crate) disks: RefCell<IndexMap<String, DiskHeld>>,
    pub(crate) disk_reads: RefCell<IndexMap<String, wsp_core::context::Joined>>,
    pub(crate) disk_timer: RefCell<Option<Cancel>>,
}

impl ReachArea {
    pub fn new(ctx: Rc<dyn ReachCtx>) -> Rc<ReachArea> {
        Rc::new_cyclic(|me| ReachArea {
            ctx,
            me: me.clone(),
            polled_reach: RefCell::default(),
            nap_refusals: Rc::default(),
            stop_refusals: Rc::default(),
            disks: RefCell::default(),
            disk_reads: RefCell::default(),
            disk_timer: RefCell::default(),
        })
    }
}

#[allow(dead_code)]
fn _uses(_: DaemonReachView, _: SessionView) {}
