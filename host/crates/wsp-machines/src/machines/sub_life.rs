// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/machines/sub-life.ts at 65f6b89c7: a sub-thread's machine from its done to its delete.
//! Specs: spec/sub_life_reads.md (the pure half) and spec/sub_life_timers.md (the area).
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use serde_json::Value;

use wsp_engine::machine::MachineBackend;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::ReachState;
use wsp_words::exit::HostError;

use crate::machines::area_error::AreaResult;
use crate::machines::machines::Entry;
use wsp_core::clock::Cancel;
use wsp_core::types::wiring::WorkspaceRecord;

pub mod reads;
pub mod timers;

pub use reads::*;
pub use timers::SubLifeArea;

/// Where an agent keeps one session's file on a machine (an adapter's `sessionFile(id)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionFile {
    pub folder: String,
    pub name: String,
    pub depth: u32,
}

/// What `backend_for` answers; the one context call `disk_kept_ms` and `disk_kept_until` read.
pub trait BackendFor {
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError>;
}

/// What the sub-life area reads off the runtime's context (sub-life.ts:157 and the `ctx.` calls below it). Every
/// method reads or writes afresh; none hands out a borrow (rule 3.3).
#[async_trait(?Send)]
pub trait SubLifeCtx: BackendFor {
    /// `live.get(id)`.
    fn live(&self, id: &str) -> Option<Entry>;
    /// `live.values()`, in insertion order.
    fn live_all(&self) -> Vec<Entry>;
    /// `(threadId, workspaceId)` of every thread record, in insertion order.
    fn thread_records(&self) -> Vec<(String, String)>;
    /// Every session row's view, in insertion order.
    fn session_views(&self) -> Vec<SessionView>;
    /// `latestOn(threadId)`: the thread's latest row's view.
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn thread_runs(&self, thread_id: &str) -> bool;
    /// `launchingOn(threadId) !== undefined`.
    fn launching_on(&self, thread_id: &str) -> bool;
    fn push_head(&self, thread_id: &str);
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String>;
    /// `settingsAt(place)?.deleteAfterMs`.
    fn delete_after_ms(&self, place_id: Option<&str>) -> Option<f64>;
    /// `adapterFor(entry, harness).adapter.sessionFile?.(id)`.
    fn session_file(&self, entry: &Entry, harness: &str, session_id: &str) -> Option<SessionFile>;
    /// `transcriptReader.read(workspaceId, threadId, { limit, bytes })`'s events, as the TS objects.
    async fn read_transcript(
        &self,
        workspace_id: &str,
        thread_id: &str,
        limit: usize,
        bytes: usize,
    ) -> AreaResult<Vec<Value>>;
    async fn put_blob(&self, collection: &str, key: &str, bytes: Vec<u8>) -> AreaResult<()>;
    async fn delete_blob(&self, collection: &str, key: &str) -> AreaResult<()>;
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()>;
    async fn nap_with(&self, workspace_id: &str, reason: &str) -> AreaResult<()>;
    fn end_sessions(&self, workspace_id: &str, reason: &str);
    /// `attach(record, deadMachine(machineId))`: a new live entry for the record on a machine that answers gone.
    fn attach_dead(&self, entry: &Entry, machine_id: &str) -> Entry;
    /// The id leaves `napRefusals`, `stopRefusals` and `unreached` (sub-life.ts:319-321).
    fn forget_marks(&self, workspace_id: &str);
    fn idle_forget(&self, workspace_id: &str);
    async fn emit_status(&self, entry: &Entry, reach: ReachState) -> AreaResult<()>;
    fn emit(&self, event: Value);
    async fn unfork(&self, entry: &Entry) -> AreaResult<()>;
    /// pipe.ts `cutOff`: the broker's revoke of the machine, its failure warned there.
    async fn cut_off(&self, entry: &Entry);
    /// The broker's `lift(id)` where a broker is wired.
    fn broker_lift(&self, workspace_id: &str);
    /// `workspaces.delete(id)`.
    async fn delete_workspace(&self, workspace_id: &str) -> AreaResult<()>;
    fn now(&self) -> f64;
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> Cancel;
    /// A local task on the host thread (TS `void p`).
    fn spawn(&self, f: LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
    /// `bus.on(type, fn)`; `"*"` for every event.
    fn on_event(&self, kind: &str, f: Rc<dyn Fn(&Value)>);
}
