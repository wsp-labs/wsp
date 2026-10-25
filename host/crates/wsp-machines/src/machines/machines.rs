// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/machines.ts` at 65f6b89c7: the machines area, in three units (specs under `spec/`):
//! `create` (machines_create.md: keyed creates, the replay and stray sweeps, claims, fork), `attach`
//! (machines_attach.md: the live entry, its engine hooks, drop) and `phases` (machines_phases.md: nap, adopt, gone,
//! the idle wiring, the wake proof). The handles every unit of this slice shares are declared here.
//!
//! wsp-core's context traits are empty; each area of this slice takes one trait of its own naming every call it makes on
//! the rest of the runtime (spec/README.md). `ThreadOn`, `Refusal`, `Unreached`, `ImageRead` and `MachinesConfig` are the
//! anonymous object types context.ts and machines.ts write inline; no wsp-core item names them, so they stay here. No borrow of a shared map is held across an `.await` or a
//! call out (chapter 03, rule 3.3).

pub mod attach;
pub mod create;
pub mod phases;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

use wsp_core::types::internal::{SessionEntry, ThreadRecord};
use wsp_core::types::pending::{Clock, KillConfirm, Store};
use wsp_core::types::wiring::{GoldenExec, GoneOutcome, LiveWorkspace, PlaceBackends, StageReport, WorkspaceRecord};
use wsp_core::types::internal::KindModule;
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::views::golden_image::GoldenVersion;
use wsp_wire::views::workspace::ReachState;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::machine_link::MachineShape;
use wsp_words::exit::HostError;

/// A live workspace as every area holds it: one object per record, compared by `Rc::ptr_eq` where the TS compares
/// `live.get(id) !== entry`. Moves to wsp-core's context once it is written.
pub type Entry = Rc<RefCell<LiveWorkspace>>;

/// One row the runtime holds, a turn of a thread, by reference: `end` and the re-key change the row in place.
pub type Row = Rc<RefCell<SessionEntry>>;

/// A thread's record as the runtime's map holds it: `ThreadRecord & { workspaceId }` (TS context.ts:202).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadOn {
    pub workspace_id: String,
    #[serde(flatten)]
    pub record: ThreadRecord,
}

pub use wsp_core::ids::Ids;

/// A refusal the provider stands on for one machine: `{ machineId, said }` (context.ts `napRefusals`, `stopRefusals`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub machine_id: String,
    pub said: String,
}

/// The provider's word that it cannot reach a machine, with the sentence its row carries (daemon.ts:272).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unreached {
    pub machine_id: String,
    pub line: String,
}

/// What `image_of` answers: the image's version where one is recorded (TS `imageOf(...).version`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageRead {
    pub version: Option<GoldenVersion>,
}

/// The numbers the TS area reads off the context once, at its making (machines.ts:16-20).
#[derive(Clone, Debug, PartialEq)]
pub struct MachinesConfig {
    /// `ctx.goneConfirmMs`: the wait before a gone verdict is read again; 0 reads at once.
    pub gone_confirm_ms: u64,
    /// `ctx.lateReadMs`: when a given-up wake reads the provider once more (`WAKE_LATE_READ_MS`, 180,000).
    pub late_read_ms: u64,
    /// `ctx.vaultCapBytes`.
    pub vault_cap_bytes: u64,
    /// `ctx.defaultIdleWindowMs`; `None` is auto-nap off.
    pub default_idle_window_ms: Option<f64>,
    /// `ctx.hostId`.
    pub host_id: String,
    /// `opts.status?.pollIntervalMs ?? POLL_INTERVAL_MS`: the idle policy's retry.
    pub poll_interval_ms: u64,
}

/// The two calls the area makes on the idle policy. wsp-core's `IdlePolicy` (made with this area's callbacks
/// `idle_window_of_id`, `on_idle`, `idle_busy`, `on_backstop`) is one; a test hands in a door that records them, which
/// the real policy cannot (its touches are internal to its timers).
pub trait IdleDoor {
    fn touch(&self, workspace_id: &str);
    fn forget(&self, workspace_id: &str);
}

impl IdleDoor for wsp_core::idle::IdlePolicy {
    fn touch(&self, workspace_id: &str) {
        wsp_core::idle::IdlePolicy::touch(self, workspace_id)
    }
    fn forget(&self, workspace_id: &str) {
        wsp_core::idle::IdlePolicy::forget(self, workspace_id)
    }
}

/// Every call the machines area makes on the rest of the runtime, each named as the TS member it stands for.
#[async_trait(?Send)]
pub trait MachinesRuntime {
    // The context's own handles.
    fn store(&self) -> Rc<dyn Store>;
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    /// `backend`: this host's own provider.
    fn backend(&self) -> Arc<dyn MachineBackend>;
    fn places(&self) -> Rc<dyn PlaceBackends>;
    /// `bus.emit`.
    fn emit(&self, event: EventUnion) -> Result<(), HostError>;
    /// `console.warn`.
    fn warn(&self, line: &str);
    /// `ctx.state.owner`.
    fn owner(&self) -> String;
    /// `opts.killConfirm`.
    fn kill_confirm(&self) -> Option<KillConfirm>;
    /// `opts.goldenRecipe?.onExec`; a listener that fails answers `Err` with its message.
    fn on_exec(&self) -> Option<Rc<dyn Fn(&GoldenExec) -> Result<(), String>>>;
    fn idle(&self) -> Rc<dyn IdleDoor>;

    // The shared maps (03, rule 3.3: borrowed inside a call, never across one).
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>>;
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>>;
    fn thread_records(&self) -> Rc<RefCell<IndexMap<String, ThreadOn>>>;
    /// `ctx.unreached` (the daemon area's map).
    fn unreached(&self) -> Rc<RefCell<IndexMap<String, Unreached>>>;
    fn nap_refusals(&self) -> Rc<RefCell<IndexMap<String, Refusal>>>;
    fn stop_refusals(&self) -> Rc<RefCell<IndexMap<String, Refusal>>>;
    /// `ctx.projectsHeld.has(id)`.
    fn project_held(&self, project_id: &str) -> bool;
    /// `ctx.keptAgents`: the thread ids whose kept agent stands on that workspace.
    fn kept_agents_on(&self, workspace_id: &str) -> Vec<String>;

    // Other areas.
    /// `execs`: ends every exec of the workspace with the reason (machines.ts:522).
    fn end_execs(&self, workspace_id: &str, reason: &str);
    async fn entry_of(&self, id: &str) -> Result<Entry, HostError>;
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView;
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError>;
    async fn emit_status(&self, entry: &Entry, reach: ReachState, said: Option<&str>) -> Result<(), HostError>;
    fn reach_of(&self, entry: &Entry) -> ReachState;
    fn nap_refused_of(&self, entry: &Entry) -> Option<String>;
    fn nap_refused_reason(&self, entry: &Entry) -> Option<String>;
    fn stop_refused_reason(&self, entry: &Entry) -> Option<String>;
    async fn says_waking(&self, entry: &Entry, line: &str) -> Result<(), HostError>;
    async fn ping_daemon(&self, entry: &Entry) -> Option<String>;
    /// Started and not awaited, as `void ctx.syncDaemon(entry)`.
    fn sync_daemon(&self, entry: &Entry);
    async fn write_daemon_roots(&self, entry: &Entry) -> Result<(), HostError>;
    async fn sub_lost(&self, entry: &Entry, reason: &str) -> Result<GoneOutcome, HostError>;
    fn backend_for(&self, record: &WorkspaceRecord) -> Arc<dyn MachineBackend>;
    fn module_of(&self, kind: &str) -> Rc<dyn KindModule>;
    fn keeps_images(&self, backend: &Arc<dyn MachineBackend>) -> bool;
    fn names_workspace(&self, backend: &Arc<dyn MachineBackend>) -> bool;
    fn pause_keeps_disk(&self, backend: &Arc<dyn MachineBackend>) -> bool;
    async fn image_of(&self, golden: &str) -> Result<ImageRead, HostError>;
    async fn recorded_images(&self) -> Result<Vec<String>, HostError>;
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String>;
    /// `atProvider(placeId, ctx.places)`.
    fn at_provider(&self, place_id: Option<&str>) -> bool;
    /// `ctx.placeDoorOf().nameOf(place)`.
    fn place_name_of(&self, place: &str) -> String;
    /// `ctx.settingsAt(place)?.napMs`: `None` unset, `Some(None)` off.
    fn place_nap_ms(&self, place_id: Option<&str>) -> Option<Option<f64>>;
    async fn shape_of(&self, machine: &Arc<dyn Machine>) -> Option<MachineShape>;
    fn size_built(&self, shape: Option<&MachineShape>, asked: WorkspaceSize) -> WorkspaceSize;
    async fn read_memory(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: &Arc<dyn Machine>, sizes: &[WorkspaceSize]) -> bool;
    /// `ctx.vaultExport(m, { maxBytes, leave })`.
    async fn vault_export(&self, machine: &Arc<dyn Machine>, max_bytes: u64, leave: Option<Vec<String>>) -> Result<Vec<u8>, HostError>;
    /// `gone.stop(at, machine)`: the delete the gone watch proves.
    async fn gone_stop(&self, at: &Arc<dyn MachineBackend>, machine: &Arc<dyn Machine>) -> Result<(), HostError>;

    // What drop takes away beside the record (machines.ts:540-576), each a call of its own TS line.
    /// `revivedAt`, `unreadAt`, `unreached`, `polledReach`, `napRefusals`, `transcripts`, `transcriptBytes`,
    /// `pendingEvents`, `pendingBytes`, `transcriptIndex`, `daemonNotes` lose the id (machines.ts:540-550).
    fn forget_workspace_maps(&self, workspace_id: &str);
    fn reap_kept(&self, thread_id: &str);
    async fn forget_slate(&self, thread_id: &str) -> Result<(), HostError>;
    /// `ctx.viewedMarks.delete(id)`.
    fn forget_viewed(&self, workspace_id: &str);
    fn cancel_flush(&self, workspace_id: &str);
    /// Awaits and drops the workspace's transcript queue, then its index flush (machines.ts:565-568).
    async fn settle_flushes(&self, workspace_id: &str);
    /// `rows?.clear(id)` then `unreadIndexes.delete(id)` (machines.ts:571-572).
    fn clear_rows(&self, workspace_id: &str);
    async fn drop_sent_images(&self, workspace_id: &str) -> Result<(), HostError>;
    /// `ctx.keptWrites.delete(id)`.
    fn forget_kept_writes(&self, workspace_id: &str);
}

/// The machines area: the state machines.ts keeps in its closure, over its runtime.
pub struct MachinesArea {
    pub(crate) rt: Rc<dyn MachinesRuntime>,
    pub(crate) config: MachinesConfig,
    /// `holding`: the purposes a create or a replay holds now, each with the waiters to wake at its release.
    pub(crate) holding: RefCell<IndexMap<String, Vec<futures::channel::oneshot::Sender<()>>>>,
    /// `inflight`: ids this process created and has not recorded yet.
    pub(crate) inflight: RefCell<IndexSet<String>>,
    /// `rereading`: records whose gone verdict is being read again.
    pub(crate) rereading: RefCell<IndexSet<String>>,
}

impl MachinesArea {
    /// `machinesArea(ctx)` (machines.ts:15): the area over its runtime, with nothing held.
    pub fn new(rt: Rc<dyn MachinesRuntime>, config: MachinesConfig) -> Rc<Self> {
        todo!()
    }

    /// The ids this process created and has not recorded, which the sweep must not read as lost.
    pub fn inflight(&self) -> Vec<String> {
        todo!()
    }

    /// The records whose gone verdict is being read again now.
    pub fn rereading(&self) -> Vec<String> {
        todo!()
    }

    /// The area's bus listeners, called by the wiring for every event in emit order: the `workspace.status` adoption
    /// (machines.ts:765) and the idle touches and forgets (:840-846).
    pub fn on_event(self: &Rc<Self>, event: &EventUnion) {
        todo!()
    }
}

/// `report`'s shape for fork's stages, re-exported for the units.
pub type Report = StageReport;
