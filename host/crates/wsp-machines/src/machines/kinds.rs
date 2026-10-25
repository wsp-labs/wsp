// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/machines/kinds.ts at 65f6b89c7: the roads every kind of workspace shares and the
//! table of kind modules. Specs: spec/kinds_roads.md and spec/kinds_table.md.
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use futures::future::LocalBoxFuture;
use wsp_adapters::codex::command::{AccountModels, CodexBroker};
use wsp_adapters::pending::CodexClaims;
use wsp_core::types::harness::Brokered;
use wsp_core::types::internal::{EndThread, ImportLanded, ImportReport, KindModule};
use wsp_core::types::pending::MachineExecOptions;
use wsp_core::types::wiring::{LiveWorkspace, ProjectImportOptions, StageReport, WorkspaceRecord};
use wsp_engine::machine::{ExecResult, Machine, MachineBackend};
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::runtime::BrokerStatus;
use wsp_words::exit::HostError;

use crate::machines::area_error::AreaResult;
use crate::machines::brokered::Placeholders;
use crate::machines::thread_ends::OwedEnd;
use crate::places::types::FolderComputer;

pub mod roads;
pub mod table;

/// What the broker last said and the claims of its ChatGPT login (kinds.ts:253-255).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BrokerSaid {
    pub status: Option<BrokerStatus>,
    pub claims: Option<CodexClaims>,
}

/// What a launch on a brokered workspace carries in place of the vault (kinds.ts:278): the shape of
/// `HarnessAdapterContext`'s `brokered` and `codex_broker`.
#[derive(Clone)]
pub struct BrokeredLaunch {
    pub env: IndexMap<String, String>,
    pub vault: IndexMap<String, String>,
    pub claude: Option<Brokered>,
    pub codex: Option<CodexBroker>,
}

/// What the kinds area reads off the runtime's context (kinds.ts:30 and every `ctx.`, `opts.` and door call).
#[async_trait(?Send)]
pub trait KindsCtx {
    fn now(&self) -> f64;
    /// `opts.statePath`.
    fn state_path(&self) -> Option<String>;
    /// The local backend's machine, where the host wired the local road.
    async fn local_machine(&self) -> AreaResult<Arc<dyn Machine>>;
    /// The host's own provider backend (`ctx.backend`).
    fn own_backend(&self) -> Arc<dyn MachineBackend>;
    fn released_backend(&self) -> Arc<dyn MachineBackend>;
    fn place_name(&self, place: &str) -> String;
    /// `places.wired`.
    fn wired_place(&self) -> String;
    fn project_held(&self, id: &str) -> ProjectView;
    fn checkout_of(&self, record: &WorkspaceRecord) -> String;
    fn computer_of(&self, entry: &LiveWorkspace) -> String;
    fn served_by_its_computer(&self, entry: &LiveWorkspace) -> bool;
    /// Whether the host wired a place door at all.
    fn has_place_door(&self) -> bool;
    fn door_backend_of(&self, place: &str) -> Option<Arc<dyn MachineBackend>>;
    fn door_name_of(&self, place: &str) -> String;
    fn door_folder_computer(&self, place: &str) -> Option<FolderComputer>;
    fn door_joined(&self, place: &str) -> bool;
    fn door_logins_of(&self, place: &str) -> Option<String>;
    fn door_vault_at(&self, place: &str, vault: &IndexMap<String, String>) -> Option<IndexMap<String, String>>;
    fn door_sign_ins_at(&self, place: &str) -> Option<IndexMap<String, AgentSignInState>>;
    fn door_github_from_vault(&self, place: &str) -> bool;
    async fn door_exec(&self, place: &str, cmd: &str, timeout_ms: u64) -> AreaResult<ExecResult>;
    async fn door_forward(&self, place: &str, port: u16, pane: Option<(String, String)>, standing: bool) -> AreaResult<u16>;
    async fn door_report_daemon_version(&self, place: &str) -> Option<u32>;
    /// `opts.vault?.()`.
    fn vault(&self) -> IndexMap<String, String>;
    /// `opts.broker?.ready() === true`.
    fn broker_ready(&self) -> bool;
    fn has_broker_status(&self) -> bool;
    async fn broker_status(&self) -> AreaResult<BrokerStatus>;
    /// `broker.codexClaims()`: none where the broker holds no ChatGPT login.
    async fn broker_codex_claims(&self) -> AreaResult<Option<CodexClaims>>;
    /// `opts.broker.codexModels` bound, handed to `codex_broker_of` as the models read.
    fn broker_codex_models(&self) -> Option<AccountModels>;
    /// protocol `isBrokered(record, provider)`, `atProvider` read for it.
    fn is_brokered(&self, record: &WorkspaceRecord) -> bool;
    /// brokered.ts `codexBrokerOf(status, claims, models)`.
    fn codex_broker_of(&self, said: &BrokerSaid) -> Option<CodexBroker>;
    fn unbrokered_refusal(&self, harness: &str) -> Option<HostError>;
    fn codex_login_unread_refusal(&self) -> HostError;
    /// brokered.ts `placeholdersOf(status, token)`.
    fn placeholders_of(&self, said: &BrokerSaid, token: &str) -> Placeholders;
    /// brokered.ts `brokeredEnv(token, branchOfRecord, publicMarketplaces(ctx))`.
    fn brokered_env(&self, token: &str, record: &WorkspaceRecord) -> IndexMap<String, String>;
    /// branch-of-record.ts `landOnRecord(ctx, entry, project, lines)`. The answer owns what it read of the entry.
    fn land_on_record(&self, entry: &LiveWorkspace, project: ProjectView, lines: Option<Vec<String>>) -> LocalBoxFuture<'static, AreaResult<()>>;
    /// project-landing.ts `cloneLines(...)` as the TS object it takes.
    fn clone_lines(&self, o: Value) -> Vec<String>;
    async fn daemon_token_of(&self, machine: &Arc<dyn Machine>) -> AreaResult<Option<String>>;
    fn forget_daemon_token(&self, machine_id: &str);
    fn has_deploy(&self) -> bool;
    async fn deploy_daemon(&self, machine: &Arc<dyn Machine>, token: Option<String>) -> AreaResult<()>;
    /// The local wiring's daemon road, where it wired one.
    async fn local_daemon_road(&self) -> Option<AreaResult<DaemonReachView>>;
    /// `opts.machineExec`, merged under the caller's keys by `exec_factory_for`.
    fn machine_exec_opts(&self) -> Option<MachineExecOptions>;
    /// The copy import of a fork; the answer owns what it read of the entry.
    fn copy_import(&self, entry: &LiveWorkspace, o: ProjectImportOptions, report: ImportReport) -> LocalBoxFuture<'static, AreaResult<ImportLanded>>;
    /// The daemon's hello version; the answer owns what it read of the entry.
    fn hello_version(&self, entry: &LiveWorkspace) -> LocalBoxFuture<'static, Option<u32>>;
    /// Whether the host wired the local road (`local !== undefined`), which the local kind needs.
    fn has_local(&self) -> bool;
    /// The local wiring's backend.
    fn local_backend(&self) -> Option<Arc<dyn MachineBackend>>;
    /// `local.home(id)`.
    fn local_home(&self, agent: &str) -> String;
    /// `local.homeDir`.
    fn local_home_dir(&self) -> String;
    /// `local.env()`.
    fn local_env(&self) -> IndexMap<String, String>;
    /// `local.rootsPath`.
    fn local_roots_path(&self) -> Option<String>;
    /// `local.daemonRoad !== undefined`.
    fn has_local_daemon_road(&self) -> bool;
    /// `local.restartDaemon !== undefined`.
    fn has_local_restart(&self) -> bool;
    /// `local.hereDaemon?.version()`; `None` where there is no here daemon.
    async fn here_daemon_version(&self) -> Option<AreaResult<Option<u32>>>;
    /// `local.backend.folder`.
    fn local_scratch(&self) -> String;
    /// `opts.agents?.wspMcp`.
    fn agents_wsp_mcp(&self) -> Option<McpServerSpec>;
    /// `opts.agents?.here?.url`.
    fn agents_here_url(&self) -> Option<String>;
    /// The folder computer's machine's `loginOf().user` (`PlaceFolderMachine.loginOf`).
    async fn door_login_user(&self, place: &str) -> AreaResult<String>;
    /// The thread-ends area's `end` and `owed` on a place.
    async fn end_thread(&self, place: &str, thread_id: &str, o: EndThread) -> AreaResult<Option<String>>;
    async fn end_owed(&self, place: &str, thread_id: &str) -> AreaResult<Option<OwedEnd>>;
    fn now_ms(&self) -> f64 {
        self.now()
    }
    /// A future that settles after `ms` on the clock (the broker race's timer, rule 17).
    fn sleep(&self, ms: f64) -> futures::future::LocalBoxFuture<'static, ()>;
    fn warn(&self, line: &str);
}

/// kinds.ts:29 `kindsArea(ctx)`.
pub struct KindsArea {
    pub(crate) ctx: Rc<dyn KindsCtx>,
    pub(crate) me: Weak<KindsArea>,
    /// kinds.ts:321.
    pub machine_reading: RefCell<IndexSet<usize>>,
    pub(crate) modules: RefCell<IndexMap<String, Option<Rc<dyn KindModule>>>>,
    pub(crate) broker_said: RefCell<BrokerSaid>,
}

impl KindsArea {
    pub fn new(ctx: Rc<dyn KindsCtx>) -> Rc<KindsArea> {
        todo!()
    }

    /// kinds.ts:509 (roads rule 21).
    pub fn module_of(&self, kind: WorkspaceKind) -> Result<Rc<dyn KindModule>, HostError> {
        todo!()
    }
}

#[allow(dead_code)]
fn _uses(_: StageReport) {}
