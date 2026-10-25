// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/create.ts` at 65f6b89c7: the create of one workspace, the name rules, and the
//! creates a window holds before their record lands. Spec: spec/create.md.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};

use wsp_core::types::internal::KindModule;
use wsp_core::types::pending::{Clock, PlaceDoor};
use wsp_core::types::wiring::{CreateWorkspaceOptions, CreatedWorkspace, StageReport, WorkspaceRecord};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::views::place::MachineBind;
use wsp_wire::views::workspace::{BranchOfRecord, ProjectView};
use wsp_wire::views::workspace_events::WorkspaceCreatingEvent;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ProjectRef, ThreadScope, WorkspaceAgents, WorkspaceView};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

use super::machines::{Entry, Ids};

/// A create as a window holds it before its record lands (context.ts:542 `ShownCreate`).
pub type ShownCreate = WorkspaceView;

/// The road that landed a project on its computer, as a create reads it (project-landing.ts `ProjectLanding`; a
/// wsp-core type once its slice lands).
pub trait LandingRoad {
    /// `"provider"`, `"box"`, ...
    fn kind(&self) -> String;
    fn refusal(&self, project: &ProjectView, deps: &serde_json::Value) -> Option<String>;
    /// `places({project, memoryKey, deps}).memoryDir`.
    fn memory_dir(&self, project: &ProjectView, memory_key: &str, deps: &serde_json::Value) -> String;
    fn workspace_binds(&self, project: &ProjectView) -> Vec<MachineBind>;
}

/// `namedOfRecord`'s question (branch-of-record.ts:139).
#[derive(Clone, Debug, PartialEq)]
pub struct RecordBranchAsk {
    pub remote: String,
    pub name: String,
    pub project: String,
    pub on: String,
    pub base: Option<String>,
    pub carried: bool,
}

/// A builder as the cap road reads it (create.ts:131): its id, name, life, the version it sealed and its place.
#[derive(Clone, Debug, PartialEq)]
pub struct KeptBuilder {
    pub id: String,
    pub name: String,
    /// `own`, `reusable`, `stale`, `foreign`, `held`.
    pub life: String,
    pub sealed_version: Option<u32>,
    pub place: Option<String>,
}

/// Every call the create area makes on the rest of the runtime.
#[async_trait(?Send)]
pub trait CreateRuntime {
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    fn emit(&self, event: EventUnion) -> Result<(), HostError>;
    fn warn(&self, line: &str);
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>>;
    /// `places.wired`.
    fn wired(&self) -> String;
    fn place_door(&self) -> Option<Rc<dyn PlaceDoor>>;
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError>;
    async fn place_refuses(&self, place_id: Option<&str>) -> Result<(), HostError>;
    async fn landing_backend(&self, place_id: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError>;
    fn landing_kind(&self, computer: &str, at: &Arc<dyn MachineBackend>) -> String;
    async fn landing_deps(&self, computer: &str) -> Result<serde_json::Value, HostError>;
    fn project_landing(&self, kind: &str) -> Result<Rc<dyn LandingRoad>, HostError>;
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError>;
    fn project_held(&self, project_id: &str) -> ProjectView;
    fn keeps_images(&self, at: &Arc<dyn MachineBackend>) -> bool;
    async fn copy_for_fork(&self, golden: &str, place_id: Option<&str>, building: &dyn Fn(&str, f64)) -> Result<String, HostError>;
    async fn image_head(&self) -> Result<String, HostError>;
    /// `imageOf(golden).version?.size`.
    async fn image_size(&self, golden: &str) -> Result<Option<wsp_wire::wire::capabilities::WorkspaceSize>, HostError>;
    fn brokered_start(&self, provider: bool) -> bool;
    async fn named_of_record(&self, ask: RecordBranchAsk) -> Result<BranchOfRecord, HostError>;
    fn place_name(&self, place: &str) -> String;
    /// `ctx.placeDoorOf().nameOf(place)`.
    fn place_door_name(&self, place: &str) -> String;
    fn spawn_at(&self, place: &str) -> WorkspaceAgents;
    /// `ctx.treeOf(spawned)`: `{rootThreadId, threadId}` written on the record, or nothing.
    fn tree_of(&self, spawned: Option<&ThreadScope>) -> serde_json::Map<String, serde_json::Value>;
    /// The machines area's `attach`.
    fn attach(&self, record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry;
    /// The machines area's `fork` with no override and `readsMemory` false; `bind` runs before the claim is let go.
    async fn fork(&self, record: &mut WorkspaceRecord, bind: &dyn for<'m, 'r> Fn(&'m Arc<dyn Machine>, &'r WorkspaceRecord), report: StageReport) -> Result<Arc<dyn Machine>, HostError>;
    async fn refresh_builders(&self) -> Result<(), HostError>;
    /// The builders in map order, as the cap road reads them.
    fn builders(&self) -> Vec<KeptBuilder>;
    /// `ctx.graceTimers.get(id)?.(); ctx.graceTimers.delete(id)`.
    fn cancel_grace(&self, builder_id: &str);
    /// `killUntilGone(ctx.backendAt(place ?? wired), machine, opts.killConfirm)`.
    async fn kill_builder(&self, builder_id: &str) -> Result<(), HostError>;
    async fn forget_builder(&self, builder_id: &str) -> Result<(), HostError>;
    fn holds_slot(&self, record: &WorkspaceRecord) -> bool;
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String>;
    /// `ctx.lifecycleOf(entry).budgets.daemonAnswersMs`.
    fn daemon_answers_ms(&self, entry: &Entry) -> u64;
    /// `entry.ws.daemonReach()`.
    async fn daemon_reach(&self, entry: &Entry) -> Result<(), HostError>;
    async fn ping_daemon(&self, entry: &Entry) -> Option<String>;
    fn sync_daemon(&self, entry: &Entry);
    async fn read_memory(&self, record: &mut WorkspaceRecord, machine: &Arc<dyn Machine>, sizes: &[wsp_wire::wire::capabilities::WorkspaceSize]) -> bool;
    fn module_of(&self, kind: &str) -> Rc<dyn KindModule>;
    async fn write_daemon_roots(&self, entry: &Entry) -> Result<(), HostError>;
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError>;
    async fn mark_used(&self, place_id: Option<&str>) -> Result<(), HostError>;
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView;
    fn ref_of(&self, project: &ProjectView) -> ProjectRef;
}

/// The create area: the name holds and the creates create.ts keeps in its closure.
pub struct CreateArea {
    pub(crate) rt: Rc<dyn CreateRuntime>,
    /// `forking`: names whose fork is between its check and its first machine.
    pub forking: RefCell<IndexSet<String>>,
    /// `failedCreates`: creates that failed before any machine was recorded.
    pub failed_creates: RefCell<IndexMap<String, ShownCreate>>,
    /// `shownCreates`: forks a thread asked for, shown under it before their record lands.
    pub shown_creates: RefCell<IndexMap<String, ShownCreate>>,
    /// `createStages`: each create in flight's last stage, without its `seq`.
    pub create_stages: RefCell<IndexMap<String, WorkspaceCreatingEvent>>,
    pub(crate) folder_making: RefCell<IndexMap<String, futures::future::Shared<futures::future::LocalBoxFuture<'static, Result<Entry, HostError>>>>>,
}

impl CreateArea {
    /// `createArea(ctx)` (create.ts:11).
    pub fn new(rt: Rc<dyn CreateRuntime>) -> Rc<Self> {
        todo!()
    }

    /// `createStaged` (create.ts:13), spec rule 1.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_staged(
        self: &Rc<Self>,
        o: &CreateWorkspaceOptions,
        project: &ProjectView,
        id: &str,
        report: StageReport,
        spawned: Option<&ThreadScope>,
        landed: Option<Box<dyn Fn()>>,
        child_base: Option<&str>,
    ) -> Result<CreatedWorkspace, HostError> {
        todo!()
    }

    /// `nameGiven` (create.ts:207): JavaScript's `trim()`.
    pub fn name_given(name: &str) -> String {
        todo!()
    }

    /// `nameRefusal` (create.ts:271), spec rule 3.
    pub fn name_refusal(&self, name: &str) -> Option<String> {
        todo!()
    }

    /// `failedView` (create.ts:229), spec rule 4. `began` is epoch ms.
    pub fn failed_view(&self, id: &str, name: &str, kind: WorkspaceKind, golden: &str, began: i64, project: &ProjectView, said: &str) -> ShownCreate {
        todo!()
    }

    /// `supersedeFailed` (create.ts:241), spec rule 5.
    pub fn supersede_failed(&self, name: &str) {
        todo!()
    }

    /// `stageReporter` (create.ts:251), spec rule 6.
    pub fn stage_reporter(self: &Rc<Self>, id: &str, name: &str, began: i64, spawned: Option<&ThreadScope>) -> StageReport {
        todo!()
    }

    /// The `*` listener (create.ts:221), spec rule 7: the wiring calls it for every event in emit order.
    pub fn on_event(&self, event: &EventUnion) {
        todo!()
    }

    /// `showCreate` (create.ts:216), spec rule 8.
    pub fn show_create(&self, view: ShownCreate) {
        todo!()
    }

    /// `foldersOf` (create.ts:279), spec rule 9.
    pub fn folders_of(&self, project_id: &str) -> Vec<Entry> {
        todo!()
    }

    /// `oneFolder` (create.ts:283), spec rule 10: a join per key.
    pub fn one_folder(&self, key: &str, make: Box<dyn FnOnce() -> futures::future::LocalBoxFuture<'static, Result<Entry, HostError>>>) -> futures::future::LocalBoxFuture<'static, Result<Entry, HostError>> {
        todo!()
    }
}
