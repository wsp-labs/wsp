// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/types/api.ts: the `Runtime` every door of the host calls, one trait per area as the TS
//! object nests them (`rt.workspaces.create` is `rt.workspaces().create`). Rule 1.5 moves this trait down to
//! wsp-core; wsp-serve implements it. Every verb takes the caller's origin last, None reading here. The behaviour
//! behind each method is its area's spec; this file pins only the shapes. Spec: spec/api.md.

use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_engine::Cancel;

use wsp_engine::machine::MachineBackend;
use wsp_engine::snapshot_storage::RetentionPlan;
use wsp_frames::{GitCommitReply, GitDiscardReply, ProcKind};
use wsp_wire::types::agents_report::{
    AgentRow, AgentsReport, AgentsSignInEvent, AgentsSignInRun, AgentsTarget, PluginAsk, PluginRow, ServerAdd, ServerAsk,
    ServerToolsAnswer, SignInLine, SkillAdded, SkillHit, SkillPreview,
};
use wsp_wire::types::attachments::{Attachment, KeptAttachment};
use wsp_wire::types::bring_back::BringBackResult;
use wsp_wire::types::changes::{CheckoutReply, CommitDraft, ViewedMarks};
use wsp_wire::types::conversations::{ConversationRead, ConversationsAnswer, ResumeAsk};
use wsp_wire::types::pull_request::{
    FixResult, GitPrReactReply, GitPrReplyReply, GitPrResolveReply, GitUpdateReply, MergeMethod, MergeResult, PullRequestItem,
    PullRequestPage, PullRequestSendResult, ReactionContent,
};
use wsp_wire::types::start::{GitPrDiffReply, ReviewDraft};
use wsp_wire::types::thread_defaults::{AccessChoice, AgentSetupSet, ThreadDefaults};
use wsp_wire::types::tree::MergeInResult;
use wsp_wire::types::workspace_look::WorkspaceLook;
use wsp_wire::views::activity::{ActivityEnded, ProcessesAsk};
use wsp_wire::views::golden_image::{
    GoldenBuilderView, GoldenLogin, GoldenManifest, GoldenVersion, MachineKind, RecipeDigest, SealedImage, SealedImageBuilt,
};
use wsp_wire::views::harness::{HarnessCatalog, McpServerSpec, ReviewPostResult, StartResult};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::preferences::{HistoryPage, Preferences, PreferencesPatch, ThreadHead};
use wsp_wire::views::session::{ForkSource, SessionOrigin, SessionView, ThreadCapWait, ThreadMarks};
use wsp_wire::views::session_events_more::{RunStep, SessionEarlierEvent, SessionEvent, SessionRunEvent};
use wsp_wire::views::snapshot_lineage::{SealedImageView, SnapshotStorage};
use wsp_wire::views::workspace::{Caller, DaemonReachView, PortProbeView, PortReachView, ProjectSeed, ProjectView, SeedChoice, SeedPlan};
use wsp_wire::views::workspace_events::WorkspaceCreatingEvent;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ProjectBranch, ThreadScope, WorkspaceView, WorktreeMade};
use wsp_wire::wire::capabilities::Capabilities;
use wsp_wire::wire::machine_link::{ExecResult, SnapshotRow, TemplateRow};
use wsp_wire::wire::runtime::{
    SessionAccessResult, SessionAnswerResult, SessionAsideResult, SessionInterruptResult, SessionRenameResult, SessionRestoreResult,
    SessionRewindResult, SessionSearchResult, SessionSettleResult, SessionSteerResult, SessionWarmResult,
};
use wsp_words::exit::HostError;
use wsp_words::start::ReviewVerdict;

use super::events::EventBus;
use super::harness::StartPicksAsked;
use super::pending::{CallbackForwards, DaemonChannel, DeviceDoor, GoldenDelta, SignInAsk, SkillAsk};
use super::wiring::{
    CreateWorkspaceOptions, CreatedWorkspace, Detach, GoldenBuildRequest, GoldenPromotion, GoldenRecipe, GoldenUpgradeResult,
    OriginStatusApi, ProjectExportOptions, ProjectImportOptions, RecipeShelf, RunningExec, SessionHandle, SizeAsk, SweepResult,
    SysListener, UsageDoor,
};

/// A daemon event as a channel hands it on.
pub type DaemonEventListener = Rc<dyn Fn(serde_json::Map<String, serde_json::Value>)>;

/// The host's core, as every door calls it. api.ts:65-712.
///
/// `places`, `slates` and `activity` are typed by the crates that hold them (wsp-machines' `PlaceDoor`, wsp-slates'
/// `Slates`, wsp-threads' `ActivityDoor`), which sit above this one and cannot be named here, so the runtime says
/// which they are: `dyn Runtime<Places = dyn PlaceDoor, Slates = dyn Slates, Activity = dyn ActivityDoor>`.
#[async_trait(?Send)]
pub trait Runtime {
    type Places: ?Sized;
    type Slates: ?Sized;
    type Activity: ?Sized;
    fn events(&self) -> Rc<dyn EventBus>;
    fn backend(&self) -> std::sync::Arc<dyn MachineBackend>;
    /// None on a runtime wired without places.
    fn places(&self) -> Option<Rc<Self::Places>>;
    fn recipes(&self) -> Option<Rc<dyn RecipeShelf>>;
    async fn here_channel(&self, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError>;
    fn place_kills(&self, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel>;
    async fn watch_place_sys(&self, place_id: &str, listener: SysListener) -> Result<Detach, HostError>;
    /// Answers the release.
    fn own_process(&self, thread_id: &str, pid: u32, kind: ProcKind) -> Detach;
    fn agents(&self) -> &dyn AgentsApi;
    fn workspaces(&self) -> &dyn WorkspacesApi;
    fn projects(&self) -> &dyn ProjectsApi;
    fn sessions(&self) -> &dyn SessionsApi;
    fn harnesses(&self) -> &dyn HarnessesApi;
    fn golden(&self) -> &dyn GoldenApi;
    fn image(&self) -> &dyn ImageApi;
    fn status(&self) -> Rc<dyn OriginStatusApi>;
    fn usage(&self) -> Rc<dyn UsageDoor>;
    fn conversations(&self) -> &dyn ConversationsApi;
    fn slates(&self) -> Rc<Self::Slates>;
    fn activity(&self) -> Rc<Self::Activity>;
    fn devices(&self) -> Rc<dyn DeviceDoor>;
    fn preferences(&self) -> &dyn PreferencesApi;
    /// This state file's owner id, minted on the first read when it has none.
    async fn owner(&self) -> Result<String, HostError>;
    async fn reap(&self, older_than_ms: Option<u64>, say: Option<Rc<dyn Fn(&str)>>) -> Result<SweepResult, HostError>;
    /// Writes every transcript still waiting on its debounce.
    async fn close(&self) -> Result<(), HostError>;
}

/// A sign-in followed: its id, and `leave`, which stops following and never ends it. api.ts:95.
pub struct SignInFollowing {
    pub sign_in_id: String,
    pub leave: Detach,
}

/// api.ts:90.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolsAsk {
    pub agent: String,
    pub name: String,
    pub refresh: Option<bool>,
}

/// api.ts:107: `"token" | "key"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyWay {
    Token,
    Key,
}

/// api.ts:119.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillsAddAsk {
    pub skill: String,
    pub agents: Option<Vec<String>>,
    pub project: Option<bool>,
}

/// `rt.agents`. api.ts:87-140.
#[async_trait(?Send)]
pub trait AgentsApi {
    async fn read(&self, target: AgentsTarget, origin: Option<Caller>) -> Result<AgentsReport, HostError>;
    async fn tools(&self, target: AgentsTarget, ask: ToolsAsk, origin: Option<Caller>) -> Result<ServerToolsAnswer, HostError>;
    async fn sign_in(
        &self,
        target: AgentsTarget,
        ask: SignInAsk,
        emit: Rc<dyn Fn(AgentsSignInEvent)>,
        origin: Option<Caller>,
    ) -> Result<SignInFollowing, HostError>;
    fn sign_ins(&self) -> Vec<AgentsSignInRun>;
    fn sign_in_follow(&self, sign_in_id: &str, emit: Rc<dyn Fn(AgentsSignInEvent)>) -> Result<SignInFollowing, HostError>;
    async fn sign_in_code(&self, sign_in_id: &str, code: &str) -> Result<(), HostError>;
    fn sign_in_stop(&self, sign_in_id: &str);
    async fn sign_in_line(&self, target: AgentsTarget, ask: SignInAsk, origin: Option<Caller>) -> Result<SignInLine, HostError>;
    async fn key(&self, agent: &str, key: &str, way: Option<KeyWay>) -> Result<(), HostError>;
    /// Answers the config file written.
    async fn add_tools(&self, target: AgentsTarget, agent: &str, origin: Option<Caller>) -> Result<String, HostError>;
    fn forwards(&self, relay: Rc<dyn CallbackForwards>) -> Detach;
    async fn skills_search(&self, q: &str, limit: Option<u32>) -> Result<Vec<SkillHit>, HostError>;
    async fn skills_get(&self, skill: &str) -> Result<SkillPreview, HostError>;
    async fn skills_preview(&self, target: AgentsTarget, ask: SkillAsk, origin: Option<Caller>) -> Result<SkillPreview, HostError>;
    async fn skills_add(&self, target: AgentsTarget, ask: SkillsAddAsk, origin: Option<Caller>) -> Result<SkillAdded, HostError>;
    /// Answers the folders removed.
    async fn skills_remove(&self, target: AgentsTarget, ask: SkillAsk, origin: Option<Caller>) -> Result<Vec<String>, HostError>;
    /// Answers the paths renamed.
    async fn skills_toggle(&self, target: AgentsTarget, ask: SkillAsk, on: bool, origin: Option<Caller>) -> Result<Vec<String>, HostError>;
    async fn servers_add(&self, target: AgentsTarget, ask: ServerAdd, origin: Option<Caller>) -> Result<String, HostError>;
    async fn servers_remove(&self, target: AgentsTarget, ask: ServerAsk, origin: Option<Caller>) -> Result<String, HostError>;
    async fn servers_toggle(&self, target: AgentsTarget, ask: ServerAsk, on: bool, origin: Option<Caller>) -> Result<String, HostError>;
    async fn servers_icon(&self, host: &str, refresh: Option<bool>) -> Result<Option<String>, HostError>;
    async fn plugins_toggle(&self, target: AgentsTarget, ask: PluginAsk, on: bool, origin: Option<Caller>) -> Result<PluginRow, HostError>;
    async fn setup(&self, place_id: &str, agent: &str, change: AgentSetupSet, origin: Option<Caller>) -> Result<AgentRow, HostError>;
    async fn homes_here(&self) -> Result<IndexMap<String, String>, HostError>;
}

/// What `workspaces.landing` answers. api.ts:150.
#[derive(Clone, Debug, PartialEq)]
pub struct Landing {
    pub place: Option<String>,
    pub name: String,
    pub capabilities: Capabilities,
    pub kind: Option<WorkspaceKind>,
}

/// api.ts:224.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FolderForAsk {
    pub project: Option<String>,
    pub branch: Option<String>,
    pub cwd: Option<String>,
    pub picks: Option<StartPicksAsked>,
    pub opens: bool,
}

/// api.ts:224.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderFor {
    pub workspace: WorkspaceView,
    pub cwd: Option<String>,
    pub setup: Option<String>,
}

/// api.ts:236.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorktreeRemoveAsk {
    pub project: String,
    pub branch: String,
    pub force: Option<bool>,
    pub check: Option<bool>,
}

/// api.ts:243.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiscardAsk {
    pub workspace_id: String,
    pub path: String,
    pub check: Option<bool>,
    pub thread_id: Option<String>,
}

/// api.ts:246.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommitAsk {
    pub workspace_id: String,
    pub message: String,
    pub paths: Option<Vec<String>>,
    pub thread_id: Option<String>,
}

/// api.ts:250.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommitDraftAsk {
    pub workspace_id: String,
    pub paths: Option<Vec<String>>,
    pub thread_id: Option<String>,
}

/// The mark on one file: set against a blob, taken off at None. api.ts:252.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewedMark {
    pub path: String,
    pub blob: Option<String>,
}

/// api.ts:263.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PullRequestReplyAsk {
    pub workspace_id: String,
    pub reply_to: Option<u64>,
    pub thread_id: Option<String>,
    pub body: String,
}

/// api.ts:271.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FixAsk {
    pub workspace_id: String,
    pub check: Option<String>,
    pub child: Option<String>,
    pub thread_id: Option<String>,
    pub child_thread_id: Option<String>,
}

/// api.ts:275.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MergeAsk {
    pub workspace_id: String,
    pub method: Option<MergeMethod>,
    pub when_checks_pass: Option<bool>,
    pub head: Option<String>,
    pub thread_id: Option<String>,
}

/// api.ts:283.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MergeInAsk {
    pub workspace_id: String,
    pub child: String,
    pub thread_id: Option<String>,
    pub child_thread_id: Option<String>,
}

/// api.ts:287.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StartFromLinkAsk {
    pub url: String,
    pub project: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub access: Option<AccessChoice>,
}

/// api.ts:291.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReviewAsk {
    pub url: Option<String>,
    pub workspace_id: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// api.ts:293.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReviewDraftAsk {
    pub workspace_id: String,
    pub summary: Option<String>,
    pub verdict: Option<ReviewVerdict>,
    /// Comment id and whether it stays ticked.
    pub on: Option<Vec<(String, bool)>>,
}

/// `rt.workspaces`. api.ts:144-341.
#[async_trait(?Send)]
pub trait WorkspacesApi {
    async fn create(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError>;
    async fn landing(&self, project: &str, sized: bool, origin: Option<Caller>) -> Result<Landing, HostError>;
    async fn get(&self, id: &str, origin: Option<Caller>, thread_id: Option<&str>) -> Result<WorkspaceView, HostError>;
    async fn held_or_creating(&self, id: &str) -> Result<Option<WorkspaceView>, HostError>;
    fn thread_place(&self, scope: &ThreadScope) -> Option<String>;
    async fn list(&self, origin: Option<Caller>) -> Result<Vec<WorkspaceView>, HostError>;
    async fn resolve(&self, reference: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn nap(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn wake(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn resize(&self, id: &str, size: SizeAsk, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn stop_wake(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn upgrade(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn rebuild(&self, id: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn restart_daemon(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError>;
    async fn rename(&self, id: &str, name: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn look(&self, id: &str, look: WorkspaceLook, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn update_daemon(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError>;
    async fn delete(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError>;
    async fn forget(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError>;
    async fn touch(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError>;
    async fn exec(&self, id: &str, cmd: &str, timeout_ms: Option<u64>, origin: Option<Caller>) -> Result<ExecResult, HostError>;
    async fn exec_stream(&self, id: &str, argv: Vec<String>, cwd: Option<String>, origin: Option<Caller>)
        -> Result<RunningExec, HostError>;
    async fn bring_back(
        &self,
        workspace_id: &str,
        title: Option<String>,
        body: Option<String>,
        origin: Option<Caller>,
    ) -> Result<BringBackResult, HostError>;
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError>;
    async fn take_setup_line(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError>;
    async fn folder(&self, project: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError>;
    async fn worktree(&self, project: &str, branch: &str, origin: Option<Caller>) -> Result<WorktreeMade, HostError>;
    async fn worktree_remove(&self, ask: WorktreeRemoveAsk, origin: Option<Caller>) -> Result<(), HostError>;
    async fn checkout(&self, id: &str, origin: Option<Caller>, fresh: Option<bool>) -> Result<CheckoutReply, HostError>;
    async fn discard(&self, ask: DiscardAsk, origin: Option<Caller>) -> Result<GitDiscardReply, HostError>;
    async fn commit(&self, ask: CommitAsk, origin: Option<Caller>) -> Result<GitCommitReply, HostError>;
    async fn commit_draft(&self, ask: CommitDraftAsk, origin: Option<Caller>) -> Result<CommitDraft, HostError>;
    async fn viewed(&self, workspace_id: &str, mark: Option<ViewedMark>, origin: Option<Caller>) -> Result<ViewedMarks, HostError>;
    async fn pull_request_view(
        &self,
        workspace_id: &str,
        fresh: Option<bool>,
        origin: Option<Caller>,
    ) -> Result<PullRequestPage, HostError>;
    async fn pull_request_diff(&self, workspace_id: &str, origin: Option<Caller>) -> Result<GitPrDiffReply, HostError>;
    async fn pull_request_send(
        &self,
        workspace_id: &str,
        items: Vec<PullRequestItem>,
        thread_id: Option<String>,
        origin: Option<Caller>,
    ) -> Result<PullRequestSendResult, HostError>;
    async fn pull_request_reply(&self, ask: PullRequestReplyAsk, origin: Option<Caller>) -> Result<GitPrReplyReply, HostError>;
    async fn pull_request_resolve(
        &self,
        workspace_id: &str,
        thread_id: &str,
        resolved: bool,
        origin: Option<Caller>,
    ) -> Result<GitPrResolveReply, HostError>;
    async fn pull_request_react(
        &self,
        workspace_id: &str,
        subject: &str,
        content: ReactionContent,
        on: bool,
        origin: Option<Caller>,
    ) -> Result<GitPrReactReply, HostError>;
    async fn fix(&self, ask: FixAsk, origin: Option<Caller>) -> Result<FixResult, HostError>;
    async fn merge(&self, ask: MergeAsk, origin: Option<Caller>) -> Result<MergeResult, HostError>;
    async fn update(&self, workspace_id: &str, thread_id: Option<String>, origin: Option<Caller>) -> Result<GitUpdateReply, HostError>;
    async fn merge_in(&self, ask: MergeInAsk, origin: Option<Caller>) -> Result<MergeInResult, HostError>;
    async fn start(&self, ask: StartFromLinkAsk, origin: Option<Caller>) -> Result<StartResult, HostError>;
    async fn review(&self, ask: ReviewAsk, origin: Option<Caller>) -> Result<StartResult, HostError>;
    async fn review_draft(&self, ask: ReviewDraftAsk, origin: Option<Caller>) -> Result<Option<ReviewDraft>, HostError>;
    async fn review_post(
        &self,
        workspace_id: &str,
        thread_id: Option<String>,
        origin: Option<Caller>,
    ) -> Result<ReviewPostResult, HostError>;
    async fn daemon_reach(&self, id: &str, origin: Option<Caller>) -> Result<DaemonReachView, HostError>;
    async fn token_refused(&self, id: &str) -> Result<(), HostError>;
    async fn daemon_channel(
        &self,
        id: &str,
        on_event: DaemonEventListener,
        origin: Option<Caller>,
    ) -> Result<Rc<dyn DaemonChannel>, HostError>;
    async fn guest_channel(&self, id: &str, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError>;
    async fn served_by_its_computer(&self, id: &str, origin: Option<Caller>) -> Result<bool, HostError>;
    async fn watch_sys(&self, id: &str, listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError>;
    async fn port_reach(&self, id: &str, port: u16, origin: Option<Caller>) -> Result<PortReachView, HostError>;
    async fn port_probe(&self, id: &str, port: u16, origin: Option<Caller>) -> Result<PortProbeView, HostError>;
    async fn origin_refusal(&self, id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError>;
    /// Synchronous: the one road that cannot await. api.ts:334.
    fn seen_by(&self, event: &serde_json::Value, origin: Option<&Caller>) -> bool;
    /// api.ts:337.
    fn shown_to(&self, event: serde_json::Value, origin: Option<&Caller>) -> serde_json::Value;
    /// The last stage of every create in flight and every refused one still held, oldest first.
    fn creating(&self) -> Vec<WorkspaceCreatingEvent>;
}

/// api.ts:346.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectAddAsk {
    pub source: String,
    pub on: Option<String>,
    pub name: Option<String>,
    pub base: Option<String>,
    pub into: Option<String>,
    pub seed: Option<SeedChoice>,
}

/// `ProjectView & { notice? }`. api.ts:346.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectAdded {
    pub project: ProjectView,
    pub notice: Option<String>,
}

/// api.ts:353.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeedAsk {
    pub keep: Option<Vec<String>>,
    pub cut: Option<Vec<String>>,
    pub none: Option<bool>,
    pub save: Option<bool>,
}

/// api.ts:360.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputerName {
    pub id: String,
    pub name: String,
}

/// api.ts:369.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRemoved {
    pub said: String,
    pub unsaved: Option<String>,
}

/// `rt.projects`. api.ts:342-378.
#[async_trait(?Send)]
pub trait ProjectsApi {
    async fn add(&self, ask: ProjectAddAsk, origin: Option<Caller>) -> Result<ProjectAdded, HostError>;
    async fn seed_plan(&self, source: &str, on: Option<String>) -> Result<SeedPlan, HostError>;
    async fn seed(&self, reference: &str, ask: SeedAsk, origin: Option<Caller>) -> Result<ProjectSeed, HostError>;
    async fn list(&self, origin: Option<Caller>) -> Result<Vec<ProjectView>, HostError>;
    async fn defaults(&self, origin: Option<Caller>) -> Result<IndexMap<String, ThreadDefaults>, HostError>;
    async fn computers(&self) -> Result<Vec<ComputerName>, HostError>;
    async fn resolve(&self, reference: &str, origin: Option<Caller>) -> Result<ProjectView, HostError>;
    async fn branch(&self, reference: &str, origin: Option<Caller>) -> Result<ProjectBranch, HostError>;
    async fn remove(&self, id: &str, origin: Option<Caller>, force: Option<bool>, check: Option<bool>)
        -> Result<ProjectRemoved, HostError>;
    /// Answers the kept hash, None where the image was taken off.
    async fn icon(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, HostError>;
    async fn icons(&self, hashes: Vec<String>) -> Result<IndexMap<String, Option<String>>, HostError>;
    async fn import(
        &self,
        opts: ProjectImportOptions,
        origin: Option<Caller>,
    ) -> Result<wsp_wire::views::project_bundle::ProjectImportResult, HostError>;
    async fn export(
        &self,
        opts: ProjectExportOptions,
        origin: Option<Caller>,
    ) -> Result<wsp_wire::views::project_bundle::ProjectExportResult, HostError>;
}

/// A start's `as`. api.ts:405.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartAs {
    pub thread_id: String,
    pub turn_id: String,
}

/// What `on_held` is called with. api.ts:459.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldStart {
    pub view: SessionView,
    pub turn_id: String,
    pub wait: ThreadCapWait,
}

/// `sessions.start`'s options. api.ts:385-467.
#[derive(Clone, Default)]
pub struct StartOptions {
    pub prompt: String,
    pub harness: Option<String>,
    pub thread: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub access: Option<AccessChoice>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
    pub started_by: Option<SessionOrigin>,
    pub r#as: Option<StartAs>,
    pub request_id: Option<String>,
    /// `"slate"` where present.
    pub via_slate: bool,
    pub attempt: Option<String>,
    pub notify: Option<Vec<String>>,
    pub turn_token: Option<String>,
    pub title: Option<String>,
    pub replaces: Option<String>,
    pub fork: Option<ForkSource>,
    pub attachments: Option<Vec<Attachment>>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub behind: Option<String>,
    pub setup: Option<String>,
    pub after_limit: Option<f64>,
    pub wakes_lead: bool,
    pub owed: bool,
    pub asks_until_stopped: bool,
    pub on_held: Option<Rc<dyn Fn(HeldStart)>>,
    pub followed: Option<bool>,
    pub outside: Option<OutsideOpening>,
}

/// `sessions.threadHistory`'s ask. api.ts:478.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryAsk {
    pub thread_id: Option<String>,
    pub before: Option<u64>,
    pub limit: Option<u64>,
}

/// `sessions.page`'s window. api.ts:481.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageWindow {
    pub thread_id: String,
    pub before: Option<u64>,
    pub limit: Option<u64>,
}

/// api.ts:494.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SteerAsk {
    pub prompt: String,
    pub request_id: Option<String>,
    pub attachments: Option<Vec<Attachment>>,
}

/// api.ts:500.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnswerAsk {
    pub ask_id: String,
    pub option_id: String,
    pub reason: Option<String>,
}

/// `sessions.warm`'s picks. api.ts:533.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WarmAsk {
    pub harness: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub access: Option<AccessChoice>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
}

/// api.ts:538.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettleAsk {
    pub finished: Option<bool>,
    pub processes: Option<ProcessesAsk>,
}

/// api.ts:556.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RewindAsk {
    pub turn_id: Option<String>,
    pub files: Option<bool>,
    pub undo: Option<bool>,
}

/// api.ts:572.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeleteThreadAsk {
    pub processes: Option<ProcessesAsk>,
    pub check: Option<bool>,
}

/// What `sessions.delete` answers. api.ts:572.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadDeleted {
    pub workspace_id: String,
    pub worktree: Option<String>,
    pub threads: u64,
    pub ended: Option<Vec<ActivityEnded>>,
    /// `"deleted"` where present.
    pub machine_deleted: bool,
    pub on: Option<String>,
    pub question: Option<String>,
}

/// `sessions.fork`'s options: a start without fork, thread, cwd and replaces, plus the fork and a branch. api.ts:527.
#[derive(Clone)]
pub struct ForkOptions {
    pub start: StartOptions,
    pub fork: ForkSource,
    pub branch: Option<String>,
}

/// `rt.sessions`. api.ts:379-573.
#[async_trait(?Send)]
pub trait SessionsApi {
    async fn start(&self, workspace_id: &str, opts: StartOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError>;
    async fn list(&self, workspace_id: Option<String>, origin: Option<Caller>) -> Result<Vec<SessionView>, HostError>;
    async fn history(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<SessionEvent>, HostError>;
    async fn thread_history(&self, workspace_id: &str, asked: HistoryAsk, origin: Option<Caller>) -> Result<Vec<SessionEvent>, HostError>;
    async fn page(&self, workspace_id: &str, window: PageWindow, origin: Option<Caller>) -> Result<HistoryPage, HostError>;
    async fn head(&self, thread_id: &str, origin: Option<Caller>, after_read: Option<bool>) -> Result<ThreadHead, HostError>;
    async fn attachment(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: &str,
        index: u32,
        origin: Option<Caller>,
    ) -> Result<KeptAttachment, HostError>;
    async fn interrupt(&self, session_id: &str, origin: Option<Caller>, task: Option<String>) -> Result<SessionInterruptResult, HostError>;
    async fn steer(&self, session_id: &str, ask: SteerAsk, origin: Option<Caller>) -> Result<SessionSteerResult, HostError>;
    async fn answer(&self, session_id: &str, ask: AnswerAsk, origin: Option<Caller>) -> Result<SessionAnswerResult, HostError>;
    async fn access(&self, session_id: &str, permission_mode: &str, origin: Option<Caller>) -> Result<SessionAccessResult, HostError>;
    async fn rename(&self, session_id: &str, title: &str, origin: Option<Caller>) -> Result<SessionRenameResult, HostError>;
    async fn read(&self, thread_id: &str, origin: Option<Caller>) -> Result<(), HostError>;
    async fn note_waited(&self, thread_id: &str, ms: f64, said: Option<String>) -> Result<(), HostError>;
    fn note_retry(&self, thread_id: &str, attempt: u32, why: &str);
    async fn replaceable(&self, thread_id: &str, origin: Option<Caller>) -> Result<(), HostError>;
    async fn fork(&self, opts: ForkOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError>;
    async fn warm(&self, workspace_id: &str, ask: WarmAsk, origin: Option<Caller>) -> Result<SessionWarmResult, HostError>;
    async fn settle(&self, thread_ids: Vec<String>, origin: Option<Caller>, ask: SettleAsk) -> Result<SessionSettleResult, HostError>;
    async fn mark(&self, thread_ids: Vec<String>, marks: ThreadMarks, origin: Option<Caller>) -> Result<(), HostError>;
    async fn restore(&self, thread_ids: Vec<String>, origin: Option<Caller>) -> Result<SessionRestoreResult, HostError>;
    async fn search(&self, query: &str, origin: Option<Caller>) -> Result<SessionSearchResult, HostError>;
    async fn aside(
        &self,
        session_id: &str,
        question: &str,
        origin: Option<Caller>,
        ask_id: Option<String>,
    ) -> Result<SessionAsideResult, HostError>;
    async fn rewind(&self, thread_id: &str, ask: RewindAsk, origin: Option<Caller>) -> Result<SessionRewindResult, HostError>;
    async fn run(&self, step: RunStep, origin: Option<Caller>) -> Result<SessionRunEvent, HostError>;
    async fn forget(&self, thread_id: &str, origin: Option<Caller>, check: Option<bool>) -> Result<(), HostError>;
    async fn delete(&self, thread_id: &str, origin: Option<Caller>, ask: DeleteThreadAsk) -> Result<ThreadDeleted, HostError>;
}

/// Where the harness lists are read for: a project or a computer, with no workspace. api.ts:581.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CatalogAt {
    pub project: Option<String>,
    pub computer: Option<String>,
}

/// `rt.harnesses`. api.ts:574-582.
#[async_trait(?Send)]
pub trait HarnessesApi {
    async fn list(
        &self,
        workspace_id: Option<String>,
        origin: Option<Caller>,
        at: Option<CatalogAt>,
    ) -> Result<Vec<HarnessCatalog>, HostError>;
}

/// What `golden.build` and `golden.seal` answer. api.ts:584.
#[derive(Clone, Debug, PartialEq)]
pub struct Sealed {
    pub manifest: GoldenManifest,
    pub version: GoldenVersion,
}

/// What `golden.buildPlace` answers. api.ts:593.
pub struct BuildPlace {
    pub place: String,
    pub name: String,
    pub kind: PlaceKind,
    pub backend: std::sync::Arc<dyn MachineBackend>,
}

/// api.ts:601.
#[derive(Clone, Default)]
pub struct PrepareAsk {
    pub name: Option<String>,
    pub kind: Option<MachineKind>,
    pub signal: Option<Cancel>,
    pub recipe: Option<GoldenRecipe>,
    pub place: Option<String>,
    pub copy: Option<bool>,
}

/// api.ts:616.
#[derive(Clone, Default)]
pub struct UpgradeAsk {
    pub name: Option<String>,
    pub delta: GoldenDelta,
    pub keep_previous: Option<bool>,
    pub logins: Option<Vec<GoldenLogin>>,
    pub recipe: Option<GoldenRecipe>,
}

/// What `golden.prune` answers. api.ts:649.
#[derive(Clone, Debug, PartialEq)]
pub struct Pruned {
    pub dropped: Vec<GoldenVersion>,
    /// Version and the provider's words.
    pub failed: Vec<(u32, String)>,
    pub storage: Option<SnapshotStorage>,
}

/// `rt.golden`. api.ts:583-650.
#[async_trait(?Send)]
pub trait GoldenApi {
    async fn build(&self, opts: GoldenBuildRequest) -> Result<Sealed, HostError>;
    async fn get(&self, name: Option<String>) -> Result<Option<GoldenManifest>, HostError>;
    async fn build_place(&self, word: Option<String>) -> Result<BuildPlace, HostError>;
    async fn prepare(&self, ask: PrepareAsk) -> Result<GoldenBuilderView, HostError>;
    /// Answers the place id and its name.
    async fn prepare_at(&self, name: Option<String>, place: Option<String>) -> Result<(String, String), HostError>;
    async fn seal(&self, builder_id: &str, logins: Option<Vec<GoldenLogin>>, keep_builder: Option<bool>) -> Result<Sealed, HostError>;
    async fn recipe(&self, name: Option<String>) -> Result<Option<RecipeDigest>, HostError>;
    async fn upgrade(&self, ask: UpgradeAsk) -> Result<GoldenUpgradeResult, HostError>;
    async fn builder_reach(&self, builder_id: &str) -> Result<DaemonReachView, HostError>;
    async fn builders(&self) -> Result<Vec<GoldenBuilderView>, HostError>;
    async fn kill(&self, builder_id: &str) -> Result<(), HostError>;
    async fn rollback(&self, version: u32, name: Option<String>) -> Result<GoldenManifest, HostError>;
    async fn promote(&self, name: Option<String>) -> Result<Option<Vec<GoldenPromotion>>, HostError>;
    async fn storage(&self) -> Result<Option<SnapshotStorage>, HostError>;
    async fn orphans(&self) -> Result<Option<AccountOrphans>, HostError>;
    async fn delete_orphans(&self) -> Result<Option<OrphansDeleted>, HostError>;
    async fn retention_places(&self, name: Option<String>) -> Result<Vec<String>, HostError>;
    async fn retention(&self, name: Option<String>, place: Option<String>) -> Result<Option<PlacedRetention>, HostError>;
    async fn prune(&self, name: Option<String>, place: Option<String>) -> Result<Pruned, HostError>;
}

/// api.ts:664.
#[derive(Clone, Default)]
pub struct ImageBuildAsk {
    pub place: String,
    pub name: Option<String>,
    pub force: Option<bool>,
    pub signal: Option<Cancel>,
    pub starting: Option<Rc<dyn Fn()>>,
}

/// `rt.image`. api.ts:652-672.
#[async_trait(?Send)]
pub trait ImageApi {
    async fn get(&self, name: Option<String>) -> Result<SealedImageView, HostError>;
    /// The record and the vault's bytes.
    async fn vault(&self, name: Option<String>) -> Result<(SealedImage, Vec<u8>), HostError>;
    async fn build(&self, ask: ImageBuildAsk) -> Result<SealedImageBuilt, HostError>;
    /// Never fails.
    async fn keep_current(&self, place: &str, name: Option<String>);
}

/// `conversations.read`'s ask. api.ts:684.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConversationAsk {
    pub project: Option<String>,
    pub agent: String,
    pub id: String,
    pub preview: Option<bool>,
}

/// `conversations.placed`'s start. api.ts:688.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlacedStart {
    pub workspace_id: Option<String>,
    pub project: Option<String>,
    pub harness: Option<String>,
    pub thread: Option<String>,
    pub branch: Option<String>,
    pub cwd: Option<String>,
}

/// What `conversations.placed` answers. api.ts:688.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub workspace_id: String,
    pub cwd: Option<String>,
    pub outside: OutsideOpening,
}

/// `rt.conversations`. api.ts:681-689.
#[async_trait(?Send)]
pub trait ConversationsApi {
    async fn list(&self, project: Option<String>, agent: Option<String>, origin: Option<Caller>) -> Result<ConversationsAnswer, HostError>;
    async fn read(&self, ask: ConversationAsk, origin: Option<Caller>) -> Result<ConversationRead, HostError>;
    async fn opening(
        &self,
        workspace_id: &str,
        harness: Option<String>,
        resume: ResumeAsk,
        origin: Option<Caller>,
    ) -> Result<OutsideOpening, HostError>;
    async fn placed(&self, start: PlacedStart, resume: ResumeAsk, origin: Option<Caller>) -> Result<Placed, HostError>;
}

/// What `preferences.set` answers. api.ts:701.
#[derive(Clone, Debug, PartialEq)]
pub struct PreferencesSet {
    pub preferences: Preferences,
    pub notice: Option<String>,
}

/// `rt.preferences`. api.ts:698-702.
#[async_trait(?Send)]
pub trait PreferencesApi {
    async fn get(&self) -> Result<Preferences, HostError>;
    async fn set(&self, patch: PreferencesPatch) -> Result<PreferencesSet, HostError>;
}

/// Both lists of an account's rows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountRows {
    pub snapshots: Vec<SnapshotRow>,
    pub templates: Vec<TemplateRow>,
}

/// What this host left on the account that nothing here records. api.ts:717-725.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountOrphans {
    pub snapshots: Vec<SnapshotRow>,
    pub templates: Vec<TemplateRow>,
    pub freed_bytes: f64,
    pub saves_usd_per_month: f64,
    pub others: AccountRows,
}

/// A retention plan with the place it was read at. api.ts:729.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedRetention {
    pub plan: RetentionPlan,
    pub place: String,
    pub place_name: String,
    pub copy: bool,
}

/// One delete the provider refused. api.ts:735.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrphanFailed {
    pub id: String,
    pub name: Option<String>,
    pub message: String,
}

/// api.ts:731-736.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrphansDeleted {
    pub snapshots: Vec<SnapshotRow>,
    pub templates: Vec<TemplateRow>,
    pub failed: Vec<OrphanFailed>,
}

/// A conversation the start of a thread opens on, read before the thread exists. api.ts:740-748.
#[derive(Clone, Debug, PartialEq)]
pub struct OutsideOpening {
    pub harness: String,
    pub id: String,
    pub copy: bool,
    pub project: String,
    pub cwd: Option<String>,
    pub title: String,
    /// `Pick<SessionEarlierEvent, "who" | "text" | "at">` each.
    pub earlier: Vec<SessionEarlierEvent>,
}
