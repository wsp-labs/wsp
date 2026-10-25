// SPDX-License-Identifier: AGPL-3.0-only
//! What `runtimeCore` itself decides (runtime.ts:491-992): the options' defaults, a thread's launch, the timers,
//! what stands on a computer, the place door's recording and the bus wiring. The state is wsp-core's; the calls the
//! core makes on the areas are `CoreCtx`. Spec: spec/runtime_core.md.
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_core::clock::Clock;
use wsp_core::devices::{DeviceDoor, PairedDevice};
use wsp_core::types::events::EventBus;
use wsp_core::types::wiring::RuntimeOptions;
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::HostError;

/// The core's options, defaults filled (spec C2).
#[derive(Clone, Debug, PartialEq)]
pub struct CoreDefaults {
    pub provider_read_ms: u64,
    pub gone_confirm_ms: u64,
    pub late_read_ms: u64,
    pub daemon_hello_timeout_ms: u64,
    pub vault_cap_bytes: u64,
    pub default_idle_window_ms: u64,
    pub host_id: String,
}

/// runtime.ts:498-515. `hostname` is this computer's, read by the caller.
pub fn core_defaults(opts: &RuntimeOptions, hostname: &str) -> CoreDefaults {
    todo!()
}

/// `tookTheResume(reads)`: `running` or `starting` only (spec C3).
pub fn took_the_resume(reads: Option<wsp_engine::machine::MachineState>) -> bool {
    todo!()
}

/// `readsState(machine)`: the provider's state bounded by `provider_read_ms` on the clock; `None` for a read that
/// failed or ran out (spec C3).
pub async fn reads_state(machine: &dyn wsp_engine::machine::Machine, clock: &Rc<dyn Clock>, provider_read_ms: u64) -> Option<wsp_engine::machine::MachineState> {
    todo!()
}

/// The machine's vault paths (spec C5): `given` where the host named them, else the breadcrumb and `ls -A /root`.
pub async fn vault_paths_of(machine: &dyn wsp_engine::machine::Machine, given: Option<&[String]>) -> Result<Vec<String>, HostError> {
    todo!()
}

/// The road a workspace has to this host, as `agentsReach` answers it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchReach {
    pub url: Option<String>,
    pub wsp: Option<McpServerSpec>,
}

/// What a thread's launch is handed (runtime.ts:543): the scoped device, its env, the wsp server.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadLaunch {
    pub scoped: Option<PairedDevice>,
    pub env: IndexMap<String, String>,
    pub wsp: Option<McpServerSpec>,
}

/// One live record, as the core's readings of a computer need it.
#[derive(Clone, Debug, PartialEq)]
pub struct StandingEntry {
    pub id: String,
    pub name: String,
    pub kind: WorkspaceKind,
    pub place: Option<String>,
    pub project: String,
    pub running: bool,
    pub machine_id: String,
    pub sub: bool,
}

/// What `standing_on` answers: the forks, folders and projects on a computer, and the rows a remove names them by.
#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub forks: Vec<StandingEntry>,
    pub folders: Vec<StandingEntry>,
    pub projects: Vec<ProjectView>,
    pub rows: StandingRows,
}

/// `{ forks: { name, threads }[], projects: { name, threads }[] }`, keys in that order.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct StandingRows {
    pub forks: Vec<NamedCount>,
    pub projects: Vec<NamedCount>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NamedCount {
    pub name: String,
    pub threads: usize,
}

/// The calls the core makes on the areas and the state it reads (context.ts's members runtime.ts names). The
/// assembly implements it on the context; tests fake it.
#[async_trait(?Send)]
pub trait CoreCtx {
    fn bus(&self) -> Rc<dyn EventBus>;
    fn clock(&self) -> Rc<dyn Clock>;
    fn devices(&self) -> Rc<dyn DeviceDoor>;
    /// The place door's host key, where a place door was built.
    fn host_key(&self) -> Option<String>;
    async fn ready(&self) -> Result<(), HostError>;
    fn agents_reach(&self, workspace_id: &str) -> Option<LaunchReach>;
    fn brokered(&self, workspace_id: &str) -> bool;
    fn record_name(&self, workspace_id: &str) -> String;
    async fn reread_public(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn read_broker(&self) -> Result<(), HostError>;
    async fn daemon_settled(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn read_public_marketplaces(&self) -> Result<(), HostError>;
    fn turn_road(&self, kind: WorkspaceKind) -> WorkspaceOrigin;
    /// The live records, in the map's order.
    fn live(&self) -> Vec<StandingEntry>;
    fn projects_held(&self) -> Vec<ProjectView>;
    /// Every thread record's (thread id, workspace id), in the map's order.
    fn thread_records(&self) -> Vec<(String, String)>;
    /// Every session's (workspace id, thread id, title, turn id), in the map's order.
    fn session_views(&self) -> Vec<wsp_wire::views::session::SessionView>;
    fn cap_held(&self, turn_id: &str) -> bool;
    fn snoozed_until(&self, thread_id: &str) -> Option<f64>;
    fn limit_resume_at(&self, thread_id: &str) -> Option<f64>;
    fn thread_workspace(&self, thread_id: &str) -> Option<String>;
    async fn resume_after_limit(&self, thread_id: &str) -> Result<(), HostError>;
    async fn wake(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn exec(&self, workspace_id: &str, cmd: &str, timeout_ms: u64) -> Result<wsp_wire::wire::machine_link::ExecResult, HostError>;
    async fn delete_workspace(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn unsaved_fork(&self, workspace_id: &str, name: &str) -> Result<Option<String>, HostError>;
    async fn unsaved_project(&self, project: &ProjectView) -> Result<Option<String>, HostError>;
    async fn remove_project(&self, project_id: &str, force: bool, keep_look: bool) -> Result<(), HostError>;
    fn end_sessions(&self, workspace_id: &str, reason: &str);
    async fn drop_record(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn store_delete_project(&self, project_id: &str) -> Result<(), HostError>;
    async fn forget_icon(&self, project_id: &str) -> Result<(), HostError>;
}

/// The core's timers and readings over a `CoreCtx` (runtime.ts:860-939). One per runtime.
pub struct Core {
    pub(crate) ctx: Rc<dyn CoreCtx>,
    pub(crate) snooze_timers: std::cell::RefCell<IndexMap<String, wsp_core::clock::Cancel>>,
    pub(crate) resume_timers: std::cell::RefCell<IndexMap<String, wsp_core::clock::Cancel>>,
}

impl Core {
    pub fn new(ctx: Rc<dyn CoreCtx>) -> Rc<Core> {
        todo!()
    }
    /// `threadLaunch(entry, threadId, rootThreadId, o)` (spec C6-C9).
    pub async fn thread_launch(&self, workspace_id: &str, kind: WorkspaceKind, thread_id: &str, root_thread_id: &str, aside: bool) -> Result<ThreadLaunch, HostError> {
        todo!()
    }
    /// `wakeAt(threadId, until)` (spec C10).
    pub fn wake_at(self: &Rc<Self>, thread_id: &str, until: f64) {
        todo!()
    }
    /// `resumeOnReset(threadId, at)` (spec C11).
    pub fn resume_on_reset(self: &Rc<Self>, thread_id: &str, at: f64) {
        todo!()
    }
    /// The armed snooze wakes and resumes, by thread, for the close and the tests.
    pub fn armed(&self) -> (Vec<String>, Vec<String>) {
        todo!()
    }
    /// `standingOn(placeId)` (spec C12).
    pub async fn standing_on(&self, place_id: &str) -> Result<Standing, HostError> {
        todo!()
    }
    /// `forkName(entry)` (spec C13).
    pub fn fork_name(&self, entry: &StandingEntry) -> String {
        todo!()
    }
    /// `forgetProject(projectId)` (spec C14). The held map is the context's.
    pub async fn forget_project(&self, project_id: &str) -> Result<(), HostError> {
        todo!()
    }
    /// `recording.holdsOn(placeId, read)` (spec C16): the rows, with `unsaved`.
    pub async fn holds_on(&self, place_id: &str, read: bool) -> Result<(StandingRows, Vec<String>), HostError> {
        todo!()
    }
    /// `recording.dropOn(placeId)` (spec C17).
    pub async fn drop_on(&self, place_id: &str) -> Result<StandingRows, HostError> {
        todo!()
    }
    /// `recording.forgetOn(placeId)` (spec C18).
    pub async fn forget_on(&self, place_id: &str) -> Result<StandingRows, HostError> {
        todo!()
    }
    /// `recording.machinesOn(placeId)` (spec C19).
    pub async fn machines_on(&self, place_id: &str) -> Result<Vec<String>, HostError> {
        todo!()
    }
    /// `recording.removeFolder(placeId, projectId)` (spec C20).
    pub async fn remove_folder(&self, place_id: &str, project_id: &str) -> Result<(), HostError> {
        todo!()
    }
}

