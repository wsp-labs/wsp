// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/types/wiring.ts: what a host wires into the runtime, the records the runtime keeps of
//! a workspace, and the small helpers the areas share. Specs: spec/wiring-records.md, spec/wiring-helpers.md. One
//! thread owns every handle here (chapter 03, rule 3.1), so they are `Rc` and the traits are `?Send`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{select, Either, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_engine::machine::{ExecOptions, RunOptions};
use wsp_engine::project_state::merge::{parse_merge_output, MergeOutput};
use wsp_engine::Cancel;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::shell_quote::shell_quote;

use wsp_adapters::pending::CodexClaims;
use wsp_adapters::port::{ExecStream, ExecStreamFactory, TaskStop, TurnImage};
use wsp_engine::errors::EngineError;
use wsp_engine::lifecycle::Workspace;
use wsp_engine::machine::{Machine, MachineBackend, ProviderBoxState};
use wsp_engine::orphans::ReapResult;
use wsp_engine::project_state::listing::UnreadStore;
use wsp_frames::PipeOpen;
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::pull_request::{PullRequestRecord, PullRequestSeen, PullRequestSent};
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::types::start::{ReviewDraft, WorkspaceFrom};
use wsp_wire::types::sub_thread::SubStarted;
use wsp_wire::types::tree::{TreeFact, TreeRecord};
use wsp_wire::types::usage::{AccountsAnswer, ReadingsRange, ResetAnswer, UsageRange, UsageSplit, UsedAnswer};
use wsp_wire::types::workspace_look::{WorkspaceGlyph, WorkspaceTheme};
use wsp_wire::views::golden_image::{GoldenManifest, GoldenVersion, Recipe, SealedImage};
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::place::{MachineBind, WorkspaceCopy};
use wsp_wire::views::preferences::{EditorChoice, EditorId};
use wsp_wire::views::project_bundle::{
    HostFolderListing, ProjectAgentOutcome, ProjectAgentResult, ProjectPlan, TerminalConfig, TerminalScheme,
};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::sub_row::{SubHeld, SubResult, SubSpawn};
use wsp_wire::views::workspace::{BranchOfRecord, Caller, DaemonReachView, ProjectSource, SeedChoice, SeedPlan, WorkspacePhase};
use wsp_wire::views::workspace_events::{WorkspaceCostEvent, WorkspaceCreateStage};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{
    WorkspaceAgents, WorkspaceStatus, WorkspaceView, WorkspaceViewDaemonRefusedAt, WorkspaceViewQueue, WorkspaceViewScreen, WorktreeFolder,
};
use wsp_wire::wire::capabilities::{InitSetup, PauseMode, WorkspaceSize};
use wsp_wire::wire::machine_link::{MachineShape, ReadingsAnswer, SysSample};
use wsp_wire::wire::runtime::{BrokerStatus, SessionAnswerResult, SessionStartOutcome};
use wsp_words::exit::HostError;
use wsp_words::js::{js_trim, js_trim_end};
use wsp_words::words::base::plural;

use super::harness::HarnessAdapterFactory;
use super::pending::{
    AgentsActs, AgentsReader, BuildGoldenOptions, CacheRule, Clock, Copier, DaemonChannel, DaemonChannelOptions, GitHubCache, GoldenImport,
    KillConfirm, MachineExecOptions, PlaceWiring, PluginsActs, ServerIcons, ServersActs, Sighting, SkillsActs, StatusApi,
    StatusListOptions, StatusWatchOptions, Store, TurnWaiting,
};
use crate::console;
use wsp_catalog::plugins::join_path;

/// A future the one owner thread awaits.
pub type Local<'a, T> = LocalBoxFuture<'a, Result<T, HostError>>;

// --- runtime -------------------------------------------------------------------------------------------------------

/// wiring.ts:77-88.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_mb: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envs: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binds: Option<Vec<MachineBind>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<WorkspaceCopy>,
}

/// The four spec keys a record keeps: `Pick<WorkspaceSpec, "envs" | "labels" | "binds" | "copy">`. wiring.ts:324.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RecordSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envs: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<IndexMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binds: Option<Vec<MachineBind>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<WorkspaceCopy>,
}

/// What a create answers: the view, a notice, and a sub-thread's start. wiring.ts:91-95.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreatedWorkspace {
    #[serde(flatten)]
    pub view: WorkspaceView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<SubStarted>,
}

/// wiring.ts:98-105.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubAsk {
    pub on: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
}

/// `idleWindowMs`: absent takes the runtime default, null turns auto-nap off. wiring.ts:126.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum IdleWindow {
    #[default]
    Default,
    Off,
    Ms(JsNumber),
}

/// wiring.ts:108-134.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateWorkspaceOptions {
    pub spec: WorkspaceSpec,
    pub project: String,
    pub golden: Option<String>,
    pub name: String,
    pub sub: Option<SubAsk>,
    pub opens_thread: Option<String>,
    /// `Partial<WorkspaceAgents>`: a key left out takes the default.
    pub agents: Option<serde_json::Map<String, serde_json::Value>>,
    pub idle_window_ms: IdleWindow,
    pub parent: Option<String>,
    pub branch_of_record: Option<BranchOfRecord>,
}

/// A folder's archive as the host packs it. wiring.ts:138-144.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackedProject {
    pub tar: Vec<u8>,
    pub files: u64,
    pub bytes: u64,
    pub cut: Vec<String>,
    pub rewritten: Vec<String>,
}

/// wiring.ts:148-151.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateAgent {
    pub agent: String,
    pub home: String,
    pub present: bool,
}

/// wiring.ts:148-151.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateRequest {
    pub dest: String,
    pub agents: Vec<StateAgent>,
}

/// wiring.ts:156-160.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackedState {
    pub tar: Vec<u8>,
    pub agents: Vec<ProjectAgentResult>,
    /// `{ agent, script }` in archive order.
    pub merges: Vec<(String, String)>,
}

/// A folder on this computer as the host reads it. wiring.ts:166-171.
#[async_trait(?Send)]
pub trait ProjectBundler {
    async fn plan(&self) -> Result<ProjectPlan, HostError>;
    /// `provider` cuts every file of values and every key, whatever `carry` names.
    async fn pack(&self, carry: &[String], rewrite: &[String], provider: bool) -> Result<PackedProject, HostError>;
    async fn pack_state(&self, req: StateRequest) -> Result<PackedState, HostError>;
}

/// wiring.ts:186.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LandState {
    pub archive: String,
    pub homes: IndexMap<String, String>,
    pub agents: Option<Vec<String>>,
}

/// wiring.ts:174-190.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LandRequest {
    pub source: String,
    pub dest: String,
    pub replace: bool,
    pub archive: String,
    pub state: Option<LandState>,
    pub unread: Option<Vec<UnreadStore>>,
}

/// wiring.ts:196.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FolderListAsk {
    pub dir: Option<String>,
    pub hidden: Option<bool>,
    pub repos: Option<bool>,
    pub wide: Option<bool>,
}

/// wiring.ts:194-197.
#[async_trait(?Send)]
pub trait HostFolders {
    async fn list(&self, req: FolderListAsk) -> Result<HostFolderListing, HostError>;
}

/// wiring.ts:201-203.
#[async_trait(?Send)]
pub trait HostTerminalConfig {
    async fn read(&self, scheme: Option<TerminalScheme>) -> Result<TerminalConfig, HostError>;
}

/// wiring.ts:211.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditorOpen {
    pub path: String,
    pub line: Option<u64>,
    pub inside: Vec<String>,
    pub editor: Option<EditorId>,
    pub remote: Option<EditorRemote>,
}

/// wiring.ts:207-216.
#[async_trait(?Send)]
pub trait HostEditor {
    async fn list(&self) -> Result<Vec<EditorChoice>, HostError>;
    async fn open(&self, req: EditorOpen) -> Result<EditorId, HostError>;
    async fn remote_refusal(&self, editor: Option<EditorId>, name: &str) -> Result<Option<String>, HostError>;
}

/// wiring.ts:219-224.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditorRemote {
    pub alias: String,
    pub folder: String,
    pub name: String,
}

/// wiring.ts:227-235.
#[async_trait(?Send)]
pub trait HostSsh {
    async fn port(&self, id: &str, name: &str) -> Result<u16, HostError>;
    async fn include(&self) -> Result<bool, HostError>;
    async fn set_include(&self, on: bool) -> Result<bool, HostError>;
}

/// wiring.ts:239-243.
#[async_trait(?Send)]
pub trait InitDoor {
    async fn get(&self, on: Option<String>) -> Result<InitSetup, HostError>;
    async fn keys(&self, provider: Option<String>, key: Option<String>) -> Result<InitSetup, HostError>;
}

/// A saved recipe as the shelf answers it. wiring.ts:249.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelvedRecipe {
    pub slug: String,
    pub file: RecipeFile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_at: Option<String>,
}

/// wiring.ts:261.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResolvedRecipe {
    pub file: RecipeFile,
    pub items: IndexMap<String, String>,
    pub hash: String,
}

/// wiring.ts:247-264.
#[async_trait(?Send)]
pub trait RecipeShelf {
    async fn list(&self) -> Result<Vec<ShelvedRecipe>, HostError>;
    async fn read(&self, word: &str) -> Result<ShelvedRecipe, HostError>;
    /// The shelved recipe and the hash it resolves to on this computer now.
    async fn get(&self, word: &str) -> Result<(ShelvedRecipe, String), HostError>;
    async fn save(&self, file: serde_json::Value) -> Result<ShelvedRecipe, HostError>;
    async fn remove(&self, word: &str) -> Result<ShelvedRecipe, HostError>;
    async fn options(&self, folders: Option<Vec<(String, String)>>) -> Result<serde_json::Value, HostError>;
    async fn resolve(&self, slug: &str) -> Result<ResolvedRecipe, HostError>;
    async fn resolve_file(&self, file: RecipeFile) -> Result<ResolvedRecipe, HostError>;
}

/// `ProjectAgentResult & { name }`. wiring.ts:267.
#[derive(Clone, Debug, PartialEq)]
pub struct LandedAgent {
    pub result: ProjectAgentResult,
    pub name: String,
}

/// wiring.ts:271-275.
#[derive(Clone, Debug, PartialEq)]
pub struct LandedProject {
    pub files: u64,
    pub bytes: u64,
    pub agents: Vec<LandedAgent>,
}

/// This computer's side of an export. wiring.ts:280-286.
#[async_trait(?Send)]
pub trait ProjectLander {
    fn caches(&self) -> CacheRule;
    async fn probe(&self, dest: &str) -> Result<Option<u64>, HostError>;
    async fn land(&self, req: LandRequest) -> Result<LandedProject, HostError>;
}

/// wiring.ts:288-299.
#[derive(Clone)]
pub struct ProjectExportOptions {
    pub workspace_id: String,
    pub source: String,
    pub dest: String,
    pub replace: Option<bool>,
    pub agents: Option<Vec<String>>,
    pub lander: Rc<dyn ProjectLander>,
}

/// wiring.ts:301-316.
#[derive(Clone)]
pub struct ProjectImportOptions {
    pub workspace_id: String,
    pub source: String,
    pub dest: String,
    pub replace: Option<bool>,
    pub carry: Option<Vec<String>>,
    pub rewrite: Option<Vec<String>>,
    pub agents: Option<Vec<String>>,
    pub bundler: Rc<dyn ProjectBundler>,
}

/// wiring.ts:381.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubBase {
    pub branch: String,
    pub sha: String,
}

/// `SubHeld & { at }`. wiring.ts:389.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubHeldAt {
    #[serde(flatten)]
    pub held: SubHeld,
    pub at: JsNumber,
}

/// A sub-machine's record of its one sub-thread. wiring.ts:378-401.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubRecord {
    pub on: String,
    pub repo: String,
    pub base: SubBase,
    pub branch: String,
    pub folder: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_usd_per_hour: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<SubResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<SubHeldAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lost: Option<String>,
    /// `true` where present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_gone: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SubSession>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<SubSpawn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening: Option<String>,
}

/// wiring.ts:348.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonDeployed {
    pub deployed_at: String,
    pub version: JsNumber,
}

/// wiring.ts:354.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BranchThreads {
    pub branch: String,
    pub threads: Vec<String>,
}

/// wiring.ts:373.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadOf {
    pub thread_id: String,
    pub workspace_id: String,
}

/// A workspace's own record, its keys in the order the create literals write them (`machines/create.ts:63`,
/// `sub-spawn.ts:223`): `id`, `name`, `kind`, `machineId`, `phase`, `golden`, `createdAt`, `project`, `spec`,
/// `idleWindowMs`, `place`, `size`, `firstLife`, the tree, `parentWorkspaceId`, `base`, `branchOfRecord`; the rest follow
/// in the TS interface's order. The TS writes a record's keys in the order of the literal that made it, so the folder
/// record (`folders.ts:37`) and the adopted record (`status.ts:155`) differ here: accepted differences, listed in
/// `wsp-account/spec/projects-README.md` and `account-README.md`. A stored record from before `kind` existed reads
/// cloud. wiring.ts:318-402.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
    #[serde(default = "cloud_kind")]
    pub kind: WorkspaceKind,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    pub golden: String,
    pub created_at: String,
    /// The project's id; the view joins its name, path and computer off the projects map.
    pub project: String,
    pub spec: RecordSpec,
    /// Absent: the runtime default. `Some(None)`: auto-nap off (the TS null), written back as null.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "wsp_wire::types::check::double_option")]
    pub idle_window_ms: Option<Option<JsNumber>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    pub size: WorkspaceSize,
    pub first_life: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_of_record: Option<BranchOfRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<WorkspaceViewQueue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<WorkspaceViewScreen>,
    /// With phase gone: the provider's words when the machine was found missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<MachineShape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm_bin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub napped_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon: Option<DaemonDeployed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PullRequestRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_threads: Option<BranchThreads>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<Checkout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<TreeRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_sent: Option<Vec<PullRequestSent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landed: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead: Option<LeadOf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<SubRecord>,
}

fn cloud_kind() -> WorkspaceKind {
    WorkspaceKind::Cloud
}

/// wiring.ts:406-414.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubSession {
    pub agent: String,
    pub id: String,
    /// When its file was last kept, ms epoch; null when it was not.
    pub kept: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// A join on a move in flight: a second caller awaits the same answer. Its error is shared as text, since a
/// `HostError` is cloned to every waiter.
pub type Joined<T> = Shared<LocalBoxFuture<'static, Result<T, HostError>>>;

/// wiring.ts:453.
#[derive(Clone, Debug, PartialEq)]
pub struct DeleteSaid {
    pub phase: WorkspacePhase,
    pub line: String,
}

/// wiring.ts:456.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WakeAsk {
    pub ask: u32,
    pub of: u32,
}

/// `{ cpu, memMb }`. wiring.ts:445.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeAsk {
    pub cpu: JsNumber,
    pub mem_mb: JsNumber,
}

/// A workspace this host holds in memory: its record, its machine, and the joins of its moves. Never stored.
/// wiring.ts:416-477. `attach` makes a new entry that shares the same record object, as the TS does: every entry for
/// a workspace holds one `Rc` of it, a done sees that the record was released while it ran, and entries are told
/// apart by `Rc::ptr_eq` on the record.
pub struct LiveWorkspace {
    pub record: Rc<RefCell<WorkspaceRecord>>,
    pub opens_thread: Option<String>,
    pub of_record_held: bool,
    pub ws: Rc<Workspace>,
    pub machine: Arc<dyn Machine>,
    /// Moves with every write of the record; the status poll drops a row it built under an older one.
    pub generation: u64,
    pub waking: Option<Joined<WorkspaceView>>,
    pub unchecked: bool,
    pub napping: Option<Joined<WorkspaceView>>,
    pub adopting: Option<Joined<()>>,
    pub deleting: Option<Joined<()>>,
    pub late_read: Option<Box<dyn FnOnce()>>,
    pub wake_stop: Option<Cancel>,
    pub resize_to: Option<SizeAsk>,
    pub nap_wait_ms: Option<u64>,
    pub wake_said: Option<String>,
    pub delete_said: Option<DeleteSaid>,
    pub wake_ask: Option<WakeAsk>,
    pub creating: bool,
    pub checkout: Option<Checkout>,
    pub checkout_reading: Option<Joined<Option<Checkout>>>,
    pub tree: Option<TreeFact>,
    pub tree_reading: Option<Joined<Option<TreeFact>>>,
    pub pr: Option<PullRequestSeen>,
    pub pr_reading: Option<Joined<Option<PullRequestSeen>>>,
    pub pr_poll: Option<Box<dyn FnOnce()>>,
    pub pr_poll_ms: Option<u64>,
    pub pr_none_at: Option<String>,
}

/// What a stage report says beside its line. wiring.ts:482.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StageSaid {
    pub notice: Option<String>,
    pub detail: Option<String>,
    pub waiting: bool,
    pub r#box: Option<ProviderBoxState>,
}

/// Reports one create stage as it is reached; the runtime stamps id, name and elapsed time. wiring.ts:482.
pub type StageReport = Rc<dyn Fn(WorkspaceCreateStage, &str, Option<StageSaid>)>;

/// `"accepted" | "not-running"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SteerTaken {
    Accepted,
    NotRunning,
}

/// `"set" | "refused" | "gone"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessMove {
    Set,
    Refused,
    Gone,
}

/// A started turn as its caller holds it. wiring.ts:484-508. `outcome` is never held.
pub trait SessionHandle {
    fn id(&self) -> String;
    fn workspace_id(&self) -> String;
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>>;
    fn turn_id(&self) -> String;
    fn outcome(&self) -> SessionStartOutcome;
    fn notice(&self) -> Option<String>;
    fn view(&self) -> SessionView;
    fn interrupt(&self) -> LocalBoxFuture<'static, ()>;
    /// None where the harness takes no steer.
    fn steer(&self, prompt: String, id: Option<String>, images: Vec<TurnImage>) -> Option<Local<'static, SteerTaken>>;
    fn tells_unread(&self) -> Option<bool>;
    fn answer(&self, ask_id: String, option_id: String, reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>>;
    fn set_access(&self, mode: String) -> Option<Local<'static, AccessMove>>;
    fn stop_task(&self, task: String) -> Option<Local<'static, TaskStop>>;
}

/// Which backend a place name resolves to. wiring.ts:513-519.
pub trait PlaceBackends {
    /// Read at each call: a host that starts with no key swaps its provider in when one is saved.
    fn wired(&self) -> String;
    fn backend(&self, place: &str) -> Option<Arc<dyn MachineBackend>>;
    fn list(&self) -> Vec<String>;
}

/// The id of the one row `wired_place` answers for: fixed, or read at each call.
pub enum WiredId {
    Fixed(String),
    Read(Rc<dyn Fn() -> String>),
}

/// The one-row table over the runtime's own backend: `wired` and `list` read the id at each call, and `backend`
/// answers the backend for exactly that id and None for any other. wiring.ts:522-531.
pub fn wired_place(id: WiredId, backend: Arc<dyn MachineBackend>) -> Rc<dyn PlaceBackends> {
    Rc::new(WiredRow { id, backend })
}

struct WiredRow {
    id: WiredId,
    backend: Arc<dyn MachineBackend>,
}

impl WiredRow {
    fn at(&self) -> String {
        match &self.id {
            WiredId::Fixed(id) => id.clone(),
            WiredId::Read(read) => read(),
        }
    }
}

impl PlaceBackends for WiredRow {
    fn wired(&self) -> String {
        self.at()
    }
    fn backend(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        (place == self.at()).then(|| self.backend.clone())
    }
    fn list(&self) -> Vec<String> {
        vec![self.at()]
    }
}

/// The daemon deploy a recipe carries: answers the stage's detail, if any. wiring.ts:546.
pub type DeployDaemon = Rc<dyn Fn(Arc<dyn Machine>, Option<String>) -> Local<'static, Option<String>>>;

/// What every golden built by this runtime gets. wiring.ts:535-556.
#[derive(Clone, Default)]
pub struct GoldenRecipe {
    pub setup: String,
    pub smoke: String,
    pub base_template: Option<String>,
    pub cpu: Option<JsNumber>,
    pub mem_mb: Option<JsNumber>,
    pub envs: Option<IndexMap<String, String>>,
    pub labels: Option<IndexMap<String, String>>,
    pub deploy_daemon: Option<DeployDaemon>,
    pub import: Option<GoldenImport>,
    pub on_exec: Option<Rc<dyn Fn(GoldenExec)>>,
    pub vault_paths: Option<Vec<String>>,
    pub source: Option<Recipe>,
}

/// One exec on a golden machine as the run log records it. wiring.ts:559-568.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenExec {
    pub machine_id: String,
    pub cmd: String,
    pub ms: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// An exec stream with the folder it ran in; None where the kind named no folder. wiring.ts:571-573.
pub struct RunningExec {
    pub stream: Arc<dyn ExecStream>,
    pub ran_in: Option<String>,
}

/// `"darwin" | "linux"`. wiring.ts:623.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Darwin,
    Linux,
}

/// The daemon beside a host on its own computer. wiring.ts:632-637.
#[async_trait(?Send)]
pub trait HereDaemon {
    async fn version(&self) -> Result<u32, HostError>;
    fn held(&self) -> Option<Option<u32>> {
        None
    }
    fn fix(&self) -> String;
}

/// A sample listener and the detach its watch answers.
pub type SysListener = Rc<dyn Fn(SysSample)>;
/// Detaches a watch.
pub type Detach = Box<dyn FnOnce()>;

/// What a host wires for the one local workspace this computer can be. wiring.ts:579-626.
pub struct LocalWiring {
    pub backend: Arc<dyn MachineBackend>,
    /// The folder this computer's backend runs commands in.
    pub folder: String,
    pub exec_stream: Rc<dyn Fn(Option<MachineExecOptions>, Option<TurnWaiting>) -> Arc<dyn ExecStreamFactory>>,
    pub home: Rc<dyn Fn(&str) -> String>,
    pub home_dir: String,
    pub roots_path: String,
    /// Asked every time, never copied.
    pub env: Rc<dyn Fn() -> IndexMap<String, String>>,
    pub daemon_road: Option<Rc<dyn Fn() -> Local<'static, DaemonReachView>>>,
    pub here_daemon: Option<Rc<dyn HereDaemon>>,
    pub restart_daemon: Option<Rc<dyn Fn() -> Local<'static, ()>>>,
    pub sys_samples: Option<Rc<dyn Fn(SysListener) -> Local<'static, Detach>>>,
    pub copier: Option<Rc<dyn Copier>>,
    pub platform: Platform,
    pub close: Option<Rc<dyn Fn() -> Local<'static, ()>>>,
}

/// What a seed pack answers. wiring.ts:644.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeedPack {
    pub tar: Vec<u8>,
    pub files: u64,
    pub bytes: u64,
    pub commits: u64,
    pub left: Vec<String>,
}

/// wiring.ts:641-645.
#[async_trait(?Send)]
pub trait SeedWiring {
    async fn plan(&self, folder: &str, homes: &IndexMap<String, String>) -> Result<SeedPlan, HostError>;
    async fn pack(&self, plan: SeedPlan, choice: SeedChoice, homes: &IndexMap<String, String>) -> Result<SeedPack, HostError>;
}

/// The host's broker. wiring.ts:770-786.
#[async_trait(?Send)]
pub trait Broker {
    fn ready(&self) -> bool;
    async fn codex_claims(&self) -> Result<Option<CodexClaims>, HostError>;
    /// None where the broker has no such read.
    fn codex_models(&self, client_version: &str) -> Option<Local<'static, serde_json::Value>> {
        let _ = client_version;
        None
    }
    fn has_pipe(&self) -> bool {
        false
    }
    fn pipe(&self, workspace_id: &str, open: PipeOpen, duplex: tokio::io::DuplexStream) {
        let _ = (workspace_id, open, duplex);
    }
    fn status(&self) -> Option<Local<'static, BrokerStatus>> {
        None
    }
    fn revoke(&self, workspace_id: &str) -> Option<Local<'static, ()>> {
        let _ = workspace_id;
        None
    }
    fn has_lift(&self) -> bool {
        false
    }
    fn lift(&self, workspace_id: &str) {
        let _ = workspace_id;
    }
}

/// How a turn here reaches back into this host. wiring.ts:730.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentsReach {
    pub here_url: Option<String>,
    pub wsp_mcp: Option<McpServerSpec>,
}

/// What a host hands `createRuntime`. wiring.ts:647-787. Every `Option` is the TS optional, absent where the host
/// wired none; the defaults each stands for are spec/wiring-records.md and spec/wiring-helpers.md's table.
#[derive(Default)]
pub struct RuntimeOptions {
    pub backend: Option<Arc<dyn MachineBackend>>,
    pub state_path: Option<String>,
    pub local: Option<LocalWiring>,
    pub place_links: Option<Rc<dyn PlaceWiring>>,
    pub place_join_wait_ms: Option<u64>,
    pub place_update_wait_ms: Option<u64>,
    pub place_dial_wait_ms: Option<u64>,
    pub place_frame_wait_ms: Option<u64>,
    pub place_relink_wait_ms: Option<u64>,
    pub store: Option<Rc<dyn Store>>,
    pub adapters: IndexMap<String, HarnessAdapterFactory>,
    pub vault: Option<Rc<dyn Fn() -> IndexMap<String, String>>>,
    pub recipes: Option<Rc<dyn RecipeShelf>>,
    pub prices_fetch: Option<Rc<dyn Fn() -> Local<'static, serde_json::Value>>>,
    pub seed: Option<Rc<dyn SeedWiring>>,
    pub golden_recipe: Option<GoldenRecipe>,
    pub copy_recipe: Option<Rc<dyn Fn(SealedImage, bool) -> Local<'static, GoldenRecipe>>>,
    pub places: Option<Rc<dyn PlaceBackends>>,
    pub vault_paths: Option<Vec<String>>,
    pub vault_caches: Option<CacheRule>,
    pub status: Option<StatusWatchOptions>,
    pub machine_exec: Option<MachineExecOptions>,
    pub idle_default_window_ms: Option<u64>,
    pub clock: Option<Rc<dyn Clock>>,
    pub github_cache: Option<Rc<dyn GitHubCache>>,
    pub kill_confirm: Option<KillConfirm>,
    pub snapshot_retry_ms: Option<u64>,
    pub host_id: Option<String>,
    pub wake: Option<WakeOptions>,
    pub provider_read_ms: Option<u64>,
    pub gone_confirm_ms: Option<u64>,
    pub agents: Option<AgentsReach>,
    pub daemon_token: Option<String>,
    pub daemon_channel: Option<Rc<dyn Fn(DaemonChannelOptions) -> Local<'static, Rc<dyn DaemonChannel>>>>,
    pub turn_snapshot_ms: Option<u64>,
    pub daemon_hello_timeout_ms: Option<u64>,
    pub agents_reader: Option<Rc<dyn AgentsReader>>,
    pub agents_acts: Option<Rc<dyn AgentsActs>>,
    pub skills_acts: Option<Rc<dyn SkillsActs>>,
    pub servers_acts: Option<Rc<dyn ServersActs>>,
    pub plugins_acts: Option<Rc<dyn PluginsActs>>,
    pub server_icons: Option<Rc<dyn ServerIcons>>,
    pub env: Option<IndexMap<String, Option<String>>>,
    pub no_machines_line: Option<String>,
    pub cloud_projects: Option<bool>,
    pub broker: Option<Rc<dyn Broker>>,
}

/// wiring.ts:791-796.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WakeOptions {
    pub late_read_ms: Option<u64>,
    pub vault_cap_bytes: Option<u64>,
}

/// wiring.ts:799.
pub const PROVIDER_READ_MS: u64 = 30_000;
/// wiring.ts:803.
pub const GONE_CONFIRM_MS: u64 = 5_000;

/// What a gone sighting came to. wiring.ts:807.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoneOutcome {
    Settled,
    NotGone,
    Unchecked,
    Moot,
}

/// A read that found the machine, handed to the load in place of its own get. wiring.ts:809.
pub struct FoundMachine {
    pub sighting: Sighting,
    pub machine: Arc<dyn Machine>,
}

/// Whether the record is gone after the sighting: settled or moot. wiring.ts:811.
pub fn settled(o: GoneOutcome) -> bool {
    matches!(o, GoneOutcome::Settled | GoneOutcome::Moot)
}

/// wiring.ts:813.
pub const WAKE_LATE_READ_MS: u64 = 3 * 60_000;
/// wiring.ts:815.
pub const DAEMON_HELLO_TIMEOUT_MS: u64 = 5_000;
/// wiring.ts:817.
pub const PORT_PROBE_TIMEOUT_MS: u64 = 10_000;
/// wiring.ts:821.
pub const CLONE_MS: u64 = 300_000;
/// wiring.ts:825.
pub const ADD_IS_A_COMPUTER_LINE: &str =
    "that is a computer, not a project; wsp add user@host joins it, and a project is a folder here or a repo's url with --on <computer>";

/// The folder a person named as this computer sees it: `~` and `~/...` under `home`, an absolute path as it is, any
/// other word resolved against `cwd`; then through the links, the joined path itself where that fails. The TS reads
/// the process's home and cwd; they are arguments here so a test names them. wiring.ts:829-838.
pub fn folder_named(path: &str, home: &str, cwd: &str) -> String {
    let named = if path == "~" || path.starts_with("~/") {
        let joined = join_path(&[home, &path[1..]]);
        if path.ends_with('/') && joined != "/" {
            format!("{joined}/")
        } else {
            joined
        }
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        join_path(&[cwd, path])
    };
    std::fs::canonicalize(&named).map(|real| real.to_string_lossy().into_owned()).unwrap_or(named)
}

/// Two git sources are one when their urls are equal, two folders when their paths are; a git and a folder never.
/// wiring.ts:842.
pub fn same_source(a: &ProjectSource, b: &ProjectSource) -> bool {
    match (a, b) {
        (ProjectSource::Git(x), ProjectSource::Git(y)) => x.url == y.url,
        (ProjectSource::Folder(x), ProjectSource::Folder(y)) => x.path == y.path,
        _ => false,
    }
}

/// wiring.ts:845.
pub const NO_COPIER_HERE: &str =
    "this host has no daemon binary beside it to make a worktree with; stage this wsp's daemon binary and try again";

/// The last line a command said, trimmed; "" for nothing. wiring.ts:849.
pub fn last_line_of(said: &str) -> String {
    js_trim(js_trim_end(said).rsplit('\n').next().unwrap_or("")).to_string()
}

/// How the import's done line reads each outcome but failed. wiring.ts:851-856.
pub fn outcome_word(outcome: ProjectAgentOutcome) -> Option<&'static str> {
    match outcome {
        ProjectAgentOutcome::Moved => Some("moved"),
        ProjectAgentOutcome::TranscriptOnly => Some("transcripts landed but not yet in its session list"),
        ProjectAgentOutcome::Carried => Some("carried unchanged since it is not on the machine"),
        ProjectAgentOutcome::Nothing => Some("had nothing to carry"),
        ProjectAgentOutcome::Failed => None,
    }
}

/// One agent's import outcome in words. wiring.ts:857-864.
pub fn outcome_words(a: &ProjectAgentResult) -> String {
    let Some(word) = outcome_word(a.outcome) else {
        return format!("failed: {}", a.error.as_deref().unwrap_or("no reason given"));
    };
    let note = a.note.as_ref().map_or(String::new(), |n| format!(" ({n})"));
    match (a.outcome, a.rows) {
        (ProjectAgentOutcome::Moved, Some(rows)) => {
            let merged = if rows.0 > 0.0 { format!("{} merged", plural(rows.0, "row")) } else { "its rows already there".to_string() };
            format!("{word}, {merged}{note}")
        }
        (ProjectAgentOutcome::TranscriptOnly, _) => format!("{word}{note}"),
        _ => word.to_string(),
    }
}

/// wiring.ts:866.
pub const MERGE_DEADLINE_MS: u64 = 120_000;
/// wiring.ts:868.
pub const LISTING_DEADLINE_MS: u64 = 120_000;

/// Runs one agent's merge script on the machine and folds what it printed into the agent's result; the script and its
/// folder are removed by their own exec whatever the run did, that exec's failure ignored; the run's own failure is
/// returned as it came. wiring.ts:873-892.
pub async fn merge_on_machine(machine: &dyn Machine, script: &str, agent: ProjectAgentResult) -> Result<ProjectAgentResult, EngineError> {
    let dir = match script.rfind('/') {
        Some(i) => script[..i].to_string(),
        None => {
            let mut dir = script.to_string();
            dir.pop();
            dir
        }
    };
    let run =
        machine.run(&format!("python3 {}", shell_quote(script)), RunOptions { deadline_ms: MERGE_DEADLINE_MS, ..Default::default() }).await;
    let cleanup = format!("rm -f {}; rmdir {} 2>/dev/null", shell_quote(script), shell_quote(&dir));
    let _ = machine.exec(&cleanup, ExecOptions::default()).await;
    let run = run?;
    if run.exit_code.get() != 0 {
        let stderr = js_trim_end(&run.stderr);
        let last = stderr.rsplit('\n').next().unwrap_or("");
        let why = if last.is_empty() { "no output" } else { last };
        let error = format!("the merge on the machine failed (exit {}): {why}", run.exit_code.get());
        return Ok(ProjectAgentResult { outcome: ProjectAgentOutcome::Failed, error: Some(error), ..agent });
    }
    match parse_merge_output(&run.stdout) {
        Ok(MergeOutput::Waiting { waiting }) => Ok(ProjectAgentResult { note: Some(waiting), ..agent }),
        Ok(MergeOutput::Merged { merged, note, .. }) => {
            let mut out = ProjectAgentResult { outcome: ProjectAgentOutcome::Moved, rows: Some(JsNumber(merged)), ..agent };
            if let Some(note) = note {
                out.note = Some(note);
            }
            Ok(out)
        }
        Err(e) => Ok(ProjectAgentResult { outcome: ProjectAgentOutcome::Failed, error: Some(e.to_string()), ..agent }),
    }
}

/// The export's done line words, where the state lands on this computer. wiring.ts:895-900.
pub fn home_word(outcome: ProjectAgentOutcome) -> Option<&'static str> {
    match outcome {
        ProjectAgentOutcome::Moved => Some("moved"),
        ProjectAgentOutcome::TranscriptOnly => Some("transcripts landed but not yet in its session list here"),
        ProjectAgentOutcome::Carried => Some("carried unchanged"),
        ProjectAgentOutcome::Nothing => Some("had nothing to bring"),
        ProjectAgentOutcome::Failed => None,
    }
}

/// One agent's export outcome in words. wiring.ts:903-907.
pub fn home_outcome(a: &LandedAgent) -> String {
    let r = &a.result;
    let counted = r.sessions.map_or(String::new(), |n| format!(" ({})", plural(n.0, "session")));
    let skipped = match r.skipped {
        Some(n) if n.0 != 0.0 => format!(", {} not under sessions/ skipped", plural(n.0, "indexed rollout")),
        _ => String::new(),
    };
    let tail = match home_word(r.outcome) {
        Some(word) => word.to_string(),
        None => format!("failed: {}", r.error.as_deref().unwrap_or("no reason given")),
    };
    format!("{}{counted} {tail}{skipped}", a.name)
}

/// wiring.ts:910.
pub const PORT_PROBE_BODY_CAP: usize = 2048;
/// wiring.ts:911.
pub const VAULT_CAP_BYTES: u64 = 200 * 1024 * 1024;
/// Blob collection: the latest nap-time vault per workspace id. wiring.ts:913.
pub const VAULTS: &str = "vaults";

/// What `until` fails with when the deadline passed and not the future: `<what> timed out after <ms> ms`, where ms is
/// the wait it was armed with. wiring.ts:917.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct DeadlineError(pub String);

/// `until`'s failure: the deadline's, or the future's own.
#[derive(Debug)]
pub enum UntilError<E> {
    Deadline(DeadlineError),
    Failed(E),
}

/// Fails once `deadline` (epoch ms on `clock`) passes; the future is left to settle on its own. The wait is
/// `max(0, deadline - clock.now())`, armed with `clock.schedule` and cancelled when the future settles first.
/// wiring.ts:921-930.
pub async fn until<T, E>(
    work: LocalBoxFuture<'_, Result<T, E>>,
    deadline: i64,
    what: &str,
    clock: Rc<dyn Clock>,
) -> Result<T, UntilError<E>> {
    let ms = (deadline - clock.now()).max(0);
    let (fire, timer) = futures::channel::oneshot::channel::<()>();
    let cancel = clock.schedule(
        Box::new(move || {
            let _ = fire.send(());
        }),
        ms,
    );
    match select(work, timer).await {
        Either::Left((done, _)) => {
            cancel();
            done.map_err(UntilError::Failed)
        }
        Either::Right(_) => Err(UntilError::Deadline(DeadlineError(format!("{what} timed out after {ms} ms")))),
    }
}

/// The size fields the provider reports that differ from what was created, `"<key> <got> != <want>"` joined with
/// ", ", cpu then memMb; None where none differs. wiring.ts:936-944.
pub fn shape_fault(expected: &MachineShape, actual: &MachineShape) -> Option<String> {
    let mut diffs = Vec::new();
    for (key, want, got) in [("cpu", expected.cpu, actual.cpu), ("memMb", expected.mem_mb, actual.mem_mb)] {
        if let (Some(want), Some(got)) = (want, got) {
            if want != got {
                diffs.push(format!("{key} {} != {}", got.js_string(), want.js_string()));
            }
        }
    }
    (!diffs.is_empty()).then(|| diffs.join(", "))
}

/// Dirs the golden image already provides on every fresh fork. wiring.ts:949.
pub const VAULT_SKIP: [&str; 3] = [".local", ".cache", ".npm"];

/// Whether the place a workspace stands on is a provider. wiring.ts:953.
pub fn at_provider(place_id: Option<&str>, places: &dyn PlaceBackends) -> bool {
    place_id.is_some_and(|id| places.list().iter().any(|p| p == id))
}

/// One RFC 1123 label, "wsp" when nothing is left. wiring.ts:956-964.
pub fn hostname_for(name: &str) -> String {
    let mut out = String::new();
    let mut in_run = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('-');
            in_run = true;
        }
    }
    let label: String = out.trim_matches('-').chars().take(63).collect();
    let label = label.trim_end_matches('-');
    if label.is_empty() {
        "wsp".to_string()
    } else {
        label.to_string()
    }
}

/// The name a hostname set came to, and the refusal where the guest refused. wiring.ts:968.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostnameSet {
    pub host: String,
    pub refused: Option<String>,
}

/// Names a fresh fork; a refusal (a non-zero exit, or an exec that failed, read as exit -1 with its message as stderr)
/// is logged and answered, never thrown. wiring.ts:968-977.
pub async fn set_hostname(machine: &dyn Machine, name: &str) -> HostnameSet {
    let host = hostname_for(name);
    let (code, stderr) = match machine.exec(&format!("hostname {host} && echo {host} > /etc/hostname"), ExecOptions::default()).await {
        Ok(res) => (res.exit_code.get(), res.stderr),
        Err(e) => (-1, e.to_string()),
    };
    if code == 0 {
        return HostnameSet { host, refused: None };
    }
    let refused = format!("hostname {host} on {} failed: {}", machine.id(), js_trim(&stderr));
    console::warn(&refused);
    HostnameSet { host, refused: Some(refused) }
}

/// `Omit<BuildGoldenOptions, "backend" | "manifest" | "hostId"> & { name? }`. wiring.ts:979-982.
#[derive(Clone, Debug, Default)]
pub struct GoldenBuildRequest {
    pub options: BuildGoldenOptions,
    pub name: Option<String>,
}

/// What the promote road did with one version. wiring.ts:987.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenPromotion {
    pub golden: String,
    pub version: u32,
    #[serde(flatten)]
    pub done: PromotionDone,
}

/// wiring.ts:987.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PromotionDone {
    #[serde(rename_all = "camelCase")]
    Promoted {
        template_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sharing: Option<u32>,
    },
    Failed {
        error: String,
    },
}

/// `"builder" | "fork"`. wiring.ts:994.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpgradeRoad {
    Builder,
    Fork,
}

/// wiring.ts:989-998.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenUpgradeResult {
    pub manifest: GoldenManifest,
    pub version: GoldenVersion,
    pub road: UpgradeRoad,
    pub previous_dropped: bool,
    pub builder_kept: bool,
}

/// wiring.ts:1002.
pub const GRACE_MS: u64 = 10 * 60_000;
/// wiring.ts:1007.
pub const DAEMON_REVIVE_AGAIN_MS: u64 = 5 * 60_000;
/// wiring.ts:1013.
pub const DAEMON_LACKS_AGAIN_MS: u64 = 60 * 60_000;

/// wiring.ts:1016-1021.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptedMachine {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub phase: WorkspacePhase,
}

/// wiring.ts:1024-1026.
#[derive(Clone, Debug)]
pub struct SweepResult {
    pub reaped: ReapResult,
    pub adopted: Option<Vec<AdoptedMachine>>,
}

/// The status tracker as the runtime serves it, the caller's origin last on its two workspace reads. wiring.ts:1031-1034.
#[async_trait(?Send)]
pub trait OriginStatusApi: StatusApi {
    async fn list(&self, opts: Option<StatusListOptions>, origin: Option<Caller>) -> Result<Vec<WorkspaceStatus>, HostError>;
    async fn history(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<WorkspaceCostEvent>, HostError>;
}

/// wiring.ts:1037.
pub const LOG_READ_EVERY_MS: u64 = 60_000;
/// wiring.ts:1041.
pub const LOG_READ_MS: u64 = 30_000;
/// wiring.ts:1044.
pub const RESET_EXEC_MS: u64 = 30_000;
/// wiring.ts:1047.
pub const PLAN_READ_EVERY_MS: u64 = 60_000;

/// A readings target: a computer or a workspace's own machine. wiring.ts:1056.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadingsTarget {
    Place(String),
    Workspace(String),
}

/// wiring.ts:1049-1060.
#[async_trait(?Send)]
pub trait UsageDoor {
    async fn used(&self, range: UsageRange, split: UsageSplit, outside: Option<bool>) -> Result<UsedAnswer, HostError>;
    async fn accounts(&self, fresh: Option<bool>) -> Result<AccountsAnswer, HostError>;
    async fn readings(&self, target: ReadingsTarget, range: ReadingsRange, origin: Option<Caller>) -> Result<ReadingsAnswer, HostError>;
    async fn reset(&self, account: String, credit_id: Option<String>, on: Option<String>) -> Result<ResetAnswer, HostError>;
}
