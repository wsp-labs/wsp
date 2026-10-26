// SPDX-License-Identifier: AGPL-3.0-only
//! What `runtimeOf` decides (runtime.ts:1017-1277): the public members that are more than an area's own, the sweep
//! and the close. The calls it makes on the core and the areas are `PublicCtx`. Spec: spec/runtime_of.md.
use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::api::CatalogAt;
use wsp_core::types::api::{
    AgentsApi, ConversationsApi, DaemonEventListener, GoldenApi, HarnessesApi, ImageApi, PreferencesApi, ProjectsApi, SessionsApi,
    WorkspacesApi,
};
use wsp_core::types::events::EventBus;
use wsp_core::types::internal::BuilderLife;
use wsp_core::types::pending::{DaemonChannel, DeviceDoor};
use wsp_core::types::wiring::{AdoptedMachine, SweepResult};
use wsp_core::types::wiring::{Detach, OriginStatusApi, RecipeShelf, SysListener, UsageDoor};
use wsp_engine::machine::MachineBackend;
use wsp_engine::orphans::{ListedMachine, ReapFailure, ReapResult, ReapedMachine};
use wsp_frames::ProcKind;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_words::exit::HostError;

/// A builder as the sweep reads it (runtime.ts:1122-1140).
#[derive(Clone, Debug, PartialEq)]
pub struct SweptBuilder {
    pub id: String,
    pub life: BuilderLife,
    /// The machine's `CREATED_AT_LABEL`, where it carries one.
    pub created_label: Option<String>,
    /// The record's `createdAt`.
    pub created_at: String,
    /// The record's `building` flag.
    pub building: bool,
    /// The place it was made at; the wired place where none.
    pub place: Option<String>,
    pub heartbeat_held: bool,
}

/// A live record as the sweep reads it (runtime.ts:1171-1191).
#[derive(Clone, Debug, PartialEq)]
pub struct SweptEntry {
    pub id: String,
    pub machine_id: String,
    pub entry_machine_id: String,
    pub creating: bool,
    pub napping: bool,
    pub waking: bool,
    pub deleting: bool,
    pub absent_machine: bool,
    pub phase: WorkspacePhase,
}

/// What a read of a record's machine came to: its state, missing (with the provider's answer), or another failure.
#[derive(Clone, Debug, PartialEq)]
pub enum MachineRead {
    State(wsp_engine::machine::MachineState),
    Missing { answer: Option<String> },
    Failed,
}

/// The calls `runtimeOf` makes, in the order the sweep and the close make them (spec U9-U17).
#[async_trait(?Send)]
pub trait PublicCtx {
    async fn ready(&self) -> Result<(), HostError>;
    fn set_say_stops(&self, say: Rc<dyn Fn(&str)>);
    fn wall_now(&self) -> i64;
    fn clock_now(&self) -> i64;
    async fn refresh_builders(&self) -> Result<(), HostError>;
    async fn expire_grace(&self) -> Result<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>;
    fn builders(&self) -> Vec<SweptBuilder>;
    async fn hold(&self, builder_id: &str) -> Result<(), HostError>;
    async fn stop_builder(&self, builder_id: &str, place: &str) -> Result<(), HostError>;
    async fn forget_builder(&self, builder_id: &str) -> Result<(), HostError>;
    async fn replay_creates(&self, failed: &mut Vec<ReapFailure>) -> Result<Vec<ReapedMachine>, HostError>;
    /// The wired place, then every other in `places.list()` order, each with whether a backend answers there.
    fn sweep_places(&self) -> Vec<(String, bool)>;
    fn wired_place(&self) -> String;
    fn place_name(&self, place: &str) -> String;
    async fn list_at(&self, place: &str) -> Result<Vec<ListedMachine>, HostError>;
    fn entries(&self) -> Vec<SweptEntry>;
    async fn reread_held(&self, workspace_id: &str, why: &str) -> Result<(), HostError>;
    async fn recover_gone(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn read_machine(&self, workspace_id: &str) -> MachineRead;
    async fn adopt_gone(&self, workspace_id: &str, words: String) -> Result<(), HostError>;
    /// The ids something claims: every record's machine ids, the builders, the in-flight creates, the gone watch's.
    fn known_ids(&self) -> Vec<String>;
    async fn adopt_lost(
        &self,
        place: &str,
        rows: &[ListedMachine],
        claimed: &mut Vec<String>,
        failed: &mut Vec<ReapFailure>,
    ) -> Result<(Vec<AdoptedMachine>, Vec<ReapedMachine>), HostError>;
    async fn engine_reap(
        &self,
        place: &str,
        rows: Vec<ListedMachine>,
        known: Vec<String>,
        older_than_ms: Option<i64>,
    ) -> Result<ReapResult, HostError>;
    async fn sweep_strays(
        &self,
        known: Vec<String>,
        failed: &mut Vec<ReapFailure>,
        places: &[String],
    ) -> Result<Vec<ReapedMachine>, HostError>;
    /// The close's steps, each named by the TS member it runs (spec U16), in the order `close` calls them.
    async fn close_step(&self, step: CloseStep) -> Result<(), HostError>;
    /// Thread ids of the kept agents, for the close's wait.
    fn kept_agents(&self) -> Vec<String>;
    /// Reaps one kept agent at once; answers its exit.
    fn reap_kept(&self, thread_id: &str) -> futures::future::LocalBoxFuture<'static, ()>;
}

/// One step of `close()`, in the order spec U16 lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseStep {
    MarkClosing,
    ClearRootsRecheck,
    RunHeldAtClose,
    AwaitCopiesMoving,
    StopSweep,
    AwaitSweeping,
    StopDiskWatch,
    CloseIdle,
    CloseAlerts,
    CloseSlates,
    CloseAgentsReader,
    SettleDaemonSyncs,
    SettleKeptWrites,
    SettleAgentAsks,
    StopMachineReadings,
    ClosePipes,
    CloseLocal,
    ClosePlaceDoor,
    MarkClosed,
    CancelBeat,
    CancelTimers,
    StopPrPolls,
    CloseGoneWatch,
    AwaitTicking,
    FreeOwnBuilders,
    FlushTranscripts,
    AwaitTranscriptWrites,
}

/// `KEPT_CLOSE_WAIT_MS` as the close reads it (types/internal.ts).
pub use wsp_core::types::internal::KEPT_CLOSE_WAIT_MS;

/// `rt.reap(olderThanMs, say)` (runtime.ts:1105-1216, spec U9-U15).
pub async fn reap(ctx: &dyn PublicCtx, older_than_ms: Option<i64>, say: Option<Rc<dyn Fn(&str)>>) -> Result<SweepAnswer, HostError> {
    todo!()
}

/// `{ reaped, spared, failed?, adopted? }` as the sweep answers it (runtime.ts:1142-1145).
#[derive(Clone, Debug, PartialEq)]
pub struct SweepAnswer {
    pub reaped: Vec<ReapedMachine>,
    pub spared: Vec<wsp_engine::orphans::SparedMachine>,
    pub failed: Option<Vec<ReapFailure>>,
    pub adopted: Option<Vec<AdoptedMachine>>,
}

impl From<SweepAnswer> for SweepResult {
    fn from(answer: SweepAnswer) -> SweepResult {
        todo!()
    }
}

/// `rt.close()` (runtime.ts:1217-1275, spec U16-U17).
pub async fn close(ctx: &dyn PublicCtx) -> Result<(), HostError> {
    todo!()
}

/// What `harnesses.list` reads (runtime.ts:1078-1097, spec U7).
#[async_trait(?Send)]
pub trait HarnessesCtx {
    async fn preferences(&self) -> Result<wsp_wire::views::preferences::Preferences, HostError>;
    fn talks_to_tree_on(&self, workspace_id: &str, origin: Option<&Caller>) -> bool;
    /// The entry the lists are read for: its record's id and project, or none.
    async fn entry_of(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError>;
    async fn project_home(&self, project: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError>;
    async fn computer_home(&self, computer: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError>;
    async fn agent_lists(&self, entry: Option<&(String, String)>) -> Result<Vec<HarnessCatalog>, HostError>;
    fn default_agent_of(&self, prefs: &wsp_wire::views::preferences::Preferences, entry: Option<&(String, String)>) -> String;
    /// `defaultsOn(c, prefs, project).catalog` then `shapeModels(.., own.models)`.
    fn shaped(
        &self,
        catalog: &HarnessCatalog,
        prefs: &wsp_wire::views::preferences::Preferences,
        project: Option<&str>,
    ) -> serde_json::Map<String, serde_json::Value>;
}

/// `harnesses.list(workspaceId, origin, at)` (spec U7): each catalog as a JSON object in the TS's key order.
pub async fn harnesses_list(
    ctx: &dyn HarnessesCtx,
    workspace_id: Option<String>,
    origin: Option<Caller>,
    at: Option<CatalogAt>,
) -> Result<Vec<serde_json::Map<String, serde_json::Value>>, HostError> {
    todo!()
}

/// What `status.list` and `status.history` read (spec U5-U6).
#[async_trait(?Send)]
pub trait StatusCtx {
    async fn probe_list(
        &self,
        o: Option<wsp_core::status::StatusListOptions>,
    ) -> Result<Vec<wsp_wire::views::workspace_more::WorkspaceStatus>, HostError>;
    fn listed_for(&self, origin: Option<&Caller>) -> Vec<String>;
    fn refuse_named(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    async fn history(&self, workspace_id: &str) -> Result<Vec<serde_json::Value>, HostError>;
}

/// `status.list(o, origin)` with the caller's rows read after the probes (spec U5).
pub async fn status_list(
    ctx: &dyn StatusCtx,
    o: Option<wsp_core::status::StatusListOptions>,
    origin: Option<Caller>,
) -> Result<Vec<wsp_wire::views::workspace_more::WorkspaceStatus>, HostError> {
    todo!()
}

/// `status.history(workspaceId, origin)` (spec U6).
pub async fn status_history(ctx: &dyn StatusCtx, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<serde_json::Value>, HostError> {
    todo!()
}

/// What `watchPlaceSys` refuses this computer with when it reads nothing of itself (runtime.ts:1053).
pub const NO_SAMPLES_HERE: &str = "this host reads nothing of the computer it runs on";

/// The assembled runtime: the core, every area, and the public members over them. Implements wsp-core's `Runtime`,
/// the one a socket serves (spec U1).
pub struct AssembledRuntime {
    pub(crate) core: Rc<super::core::Core>,
}

#[async_trait(?Send)]
impl wsp_core::types::api::Runtime for AssembledRuntime {
    type Places = dyn wsp_machines::places::types::PlaceDoor;
    type Slates = dyn wsp_slates::slates::Slates;
    type Activity = dyn wsp_threads::activity::ActivityDoor;
    fn events(&self) -> Rc<dyn EventBus> {
        todo!()
    }
    fn backend(&self) -> std::sync::Arc<dyn MachineBackend> {
        todo!()
    }
    fn places(&self) -> Option<Rc<Self::Places>> {
        todo!()
    }
    fn recipes(&self) -> Option<Rc<dyn RecipeShelf>> {
        todo!()
    }
    async fn here_channel(&self, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        todo!()
    }
    fn place_kills(&self, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        todo!()
    }
    async fn watch_place_sys(&self, place_id: &str, listener: SysListener) -> Result<Detach, HostError> {
        todo!()
    }
    fn own_process(&self, thread_id: &str, pid: u32, kind: ProcKind) -> Detach {
        todo!()
    }
    fn agents(&self) -> &dyn AgentsApi {
        todo!()
    }
    fn workspaces(&self) -> &dyn WorkspacesApi {
        todo!()
    }
    fn projects(&self) -> &dyn ProjectsApi {
        todo!()
    }
    fn sessions(&self) -> &dyn SessionsApi {
        todo!()
    }
    fn harnesses(&self) -> &dyn HarnessesApi {
        todo!()
    }
    fn golden(&self) -> &dyn GoldenApi {
        todo!()
    }
    fn image(&self) -> &dyn ImageApi {
        todo!()
    }
    fn status(&self) -> Rc<dyn OriginStatusApi> {
        todo!()
    }
    fn usage(&self) -> Rc<dyn UsageDoor> {
        todo!()
    }
    fn conversations(&self) -> &dyn ConversationsApi {
        todo!()
    }
    fn slates(&self) -> Rc<Self::Slates> {
        todo!()
    }
    fn activity(&self) -> Rc<Self::Activity> {
        todo!()
    }
    fn devices(&self) -> Rc<dyn DeviceDoor> {
        todo!()
    }
    fn preferences(&self) -> &dyn PreferencesApi {
        todo!()
    }
    async fn owner(&self) -> Result<String, HostError> {
        todo!()
    }
    async fn reap(&self, older_than_ms: Option<u64>, say: Option<Rc<dyn Fn(&str)>>) -> Result<SweepResult, HostError> {
        todo!()
    }
    async fn close(&self) -> Result<(), HostError> {
        todo!()
    }
}
