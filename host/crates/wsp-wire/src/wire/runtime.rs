// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/runtime.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::agents_report::{AgentsTarget, McpScope};
use crate::types::attachments::Attachment;
use crate::types::conversations::ResumeAsk;
use crate::types::init_job::{InitRoad, InitScreenId};
use crate::types::project_look::ProjectIconHash;
use crate::types::pull_request::{MergeMethod, PullRequestItem, ReactionContent};
use crate::types::recipe_file::RecipeFile;
use crate::types::slate_wire::SlateWireValues;
use crate::types::thread_defaults::AccessChoice;
use crate::types::usage::{ReadingsRange, UsageRange, UsageSplit};
use crate::types::workspace_look::{WorkspaceGlyph, WorkspaceTheme};
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::activity::{ActivityEnded, ProcessesAsk};
use crate::views::app_procs::AppProcList;
use crate::views::golden_image::MachineKind;
use crate::views::place::{PlaceSettingWord, PlaceSetupStep, SudoPassword};
use crate::views::preferences::{EditorId, PreferencesPatch};
use crate::views::project_bundle::TerminalScheme;
use crate::views::session::{ForkSource, SessionOrigin, SessionView, ThreadMarks};
use crate::views::session_events::RunState;
use crate::views::workspace::{SeedChoice, WorkspaceOrigin};
use crate::wire::devices::{DaemonFrame, TicketPurpose};
use crate::wire::limits::RelayPort;
use crate::wire::place_link::{DeviceAuthRequest, PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, SealOpenRequest};
use wsp_frames::{False, True};

use crate::views::workspace::EventAsker;

pub const RUNTIME_OPS: &[&str] = &[
    "auth",
    "ticket.issue",
    "pair.issue",
    "pair.redeem",
    "seal.open",
    "device.auth",
    "account.get",
    "devices.list",
    "devices.revoke",
    "place.join",
    "place.auth",
    "place.prove",
    "places.list",
    "places.update",
    "places.holds",
    "places.remove",
    "places.doctor",
    "places.dial",
    "places.set",
    "places.follow",
    "places.skip",
    "places.setupLog",
    "places.estimate",
    "places.loginLanded",
    "places.door",
    "places.mint",
    "places.sshHosts",
    "places.add",
    "places.setup",
    "places.choose",
    "recipes.list",
    "recipes.get",
    "recipes.save",
    "recipes.remove",
    "recipes.options",
    "events.subscribe",
    "status.subscribe",
    "status.list",
    "workspaces.create",
    "workspaces.landing",
    "workspaces.list",
    "workspaces.resolve",
    "workspaces.get",
    "workspaces.nap",
    "workspaces.resize",
    "workspaces.wake",
    "workspaces.restartDaemon",
    "workspaces.stopWake",
    "workspaces.upgrade",
    "workspaces.rename",
    "workspaces.look",
    "workspaces.bringBack",
    "worktree.make",
    "folder.make",
    "worktree.remove",
    "workspaces.checkout",
    "workspaces.discard",
    "workspaces.commit",
    "workspaces.commitDraft",
    "workspaces.viewed",
    "workspaces.pullRequestView",
    "workspaces.pullRequestDiff",
    "workspaces.pullRequestSend",
    "workspaces.pullRequestReply",
    "workspaces.pullRequestResolve",
    "workspaces.pullRequestReact",
    "workspaces.fix",
    "workspaces.merge",
    "workspaces.start",
    "workspaces.review",
    "workspaces.reviewDraft",
    "workspaces.reviewPost",
    "workspaces.update",
    "workspaces.mergeIn",
    "workspaces.delete",
    "workspaces.forget",
    "workspaces.snapshot",
    "projectGoldens.list",
    "projectGoldens.remove",
    "workspaces.touch",
    "daemon.open",
    "sys.subscribe",
    "sys.unsubscribe",
    "appProcs.watch",
    "appProcs.unwatch",
    "appProcs.offer",
    "appProcs.set",
    "activity.read",
    "activity.end",
    "daemon.send",
    "daemon.close",
    "sessions.start",
    "harnesses.list",
    "sessions.list",
    "sessions.history",
    "sessions.head",
    "sessions.attachment",
    "sessions.interrupt",
    "sessions.steer",
    "sessions.answer",
    "sessions.access",
    "sessions.rename",
    "sessions.forget",
    "sessions.delete",
    "sessions.read",
    "sessions.replaceable",
    "sessions.warm",
    "sessions.settle",
    "sessions.mark",
    "sessions.restore",
    "sessions.search",
    "sessions.aside",
    "sessions.run",
    "sessions.rewind",
    "golden.get",
    "capabilities.get",
    "golden.prepare",
    "golden.seal",
    "snapshots.list",
    "snapshots.storage",
    "cost.history",
    "cost.spend",
    "usage.used",
    "usage.accounts",
    "usage.reset",
    "places.readings",
    "snapshots.rollback",
    "workspaces.portReach",
    "workspaces.portProbe",
    "workspaces.rebuild",
    "forwards.list",
    "forwards.stop",
    "workspaces.exec",
    "host.folders",
    "host.terminalConfig",
    "editor.list",
    "editor.open",
    "ssh.port",
    "ssh.include",
    "agents.read",
    "servers.tools",
    "servers.icon",
    "agents.signIn",
    "servers.signIn",
    "agents.signInCode",
    "agents.signIns",
    "agents.signInStop",
    "agents.signInLine",
    "agents.key",
    "agents.cloudSignIn",
    "providers.checkKey",
    "broker.status",
    "agents.addTools",
    "agents.setup",
    "skills.search",
    "skills.get",
    "skills.preview",
    "skills.add",
    "skills.remove",
    "skills.toggle",
    "servers.add",
    "servers.remove",
    "servers.toggle",
    "plugins.toggle",
    "init.get",
    "init.keys",
    "init.start",
    "init.answer",
    "init.step",
    "init.draft",
    "init.retry",
    "init.build",
    "init.signInCode",
    "init.cancel",
    "preferences.get",
    "preferences.set",
    "release.get",
    "release.check",
    "host.restart",
    "projects.add",
    "project.seed.plan",
    "projects.seed",
    "projects.list",
    "conversations.list",
    "conversations.read",
    "projects.defaults",
    "projects.resolve",
    "projects.branch",
    "projects.remove",
    "projects.icon",
    "projects.icons",
    "project.plan",
    "project.import",
    "project.export",
    "image.get",
    "image.build",
    "image.export",
    "slates.get",
    "slates.write",
    "slates.state",
    "slates.read",
    "slates.catalog",
    "slates.event",
    "slates.approve",
    "slates.cancel",
    "slates.revoke",
    "slates.shown",
    "slates.subscribe",
    "slates.unsubscribe",
    "slates.resolve",
    "slates.image",
];
pub const SECRET_REQUEST_FIELDS: &[&str] = &["token", "key", "rows", "code", "passphrase", "env", "envs", "headers", "sudoPassword"];
pub const THREAD_OPS: &[&str] = &[
    "auth",
    "events.subscribe",
    "status.list",
    "status.subscribe",
    "capabilities.get",
    "workspaces.landing",
    "workspaces.create",
    "projects.list",
    "places.list",
    "cost.spend",
    "conversations.list",
    "projects.defaults",
    "projects.resolve",
    "projects.seed",
    "workspaces.list",
    "workspaces.resolve",
    "workspaces.get",
    "workspaces.touch",
    "workspaces.wake",
    "workspaces.exec",
    "worktree.make",
    "worktree.remove",
    "workspaces.checkout",
    "workspaces.commit",
    "workspaces.commitDraft",
    "workspaces.pullRequestView",
    "workspaces.fix",
    "workspaces.update",
    "workspaces.mergeIn",
    "harnesses.list",
    "sessions.start",
    "sessions.replaceable",
    "sessions.list",
    "sessions.history",
    "sessions.head",
    "sessions.attachment",
    "sessions.interrupt",
    "sessions.steer",
    "sessions.rename",
    "sessions.read",
    "sessions.settle",
    "sessions.restore",
    "sessions.delete",
    "sessions.search",
    "activity.read",
    "activity.end",
    "slates.write",
    "slates.state",
    "slates.read",
    "slates.catalog",
];
pub const DEVICE_OPS: &[&str] = &[
    "auth",
    "events.subscribe",
    "status.subscribe",
    "status.list",
    "capabilities.get",
    "ticket.issue",
    "places.list",
    "account.get",
    "devices.list",
    "devices.revoke",
    "workspaces.landing",
    "workspaces.create",
    "folder.make",
    "workspaces.list",
    "workspaces.resolve",
    "workspaces.get",
    "workspaces.nap",
    "workspaces.wake",
    "workspaces.stopWake",
    "workspaces.restartDaemon",
    "workspaces.upgrade",
    "workspaces.rename",
    "workspaces.look",
    "workspaces.delete",
    "workspaces.forget",
    "workspaces.snapshot",
    "workspaces.touch",
    "workspaces.portReach",
    "workspaces.portProbe",
    "workspaces.rebuild",
    "workspaces.checkout",
    "workspaces.viewed",
    "workspaces.pullRequestView",
    "workspaces.pullRequestDiff",
    "workspaces.reviewDraft",
    "projects.list",
    "conversations.list",
    "conversations.read",
    "projects.defaults",
    "projects.resolve",
    "projects.branch",
    "projects.remove",
    "projects.icon",
    "projects.icons",
    "projectGoldens.list",
    "projectGoldens.remove",
    "sys.subscribe",
    "sys.unsubscribe",
    "harnesses.list",
    "sessions.replaceable",
    "sessions.list",
    "sessions.history",
    "sessions.head",
    "sessions.attachment",
    "sessions.forget",
    "sessions.read",
    "sessions.settle",
    "sessions.mark",
    "sessions.restore",
    "sessions.search",
    "golden.get",
    "image.get",
    "snapshots.list",
    "snapshots.storage",
    "snapshots.rollback",
    "cost.history",
    "cost.spend",
    "usage.used",
    "usage.accounts",
    "broker.status",
    "places.readings",
    "forwards.list",
    "forwards.stop",
    "preferences.get",
    "preferences.set",
    "release.get",
    "release.check",
    "host.terminalConfig",
    "init.get",
    "slates.get",
    "slates.state",
    "slates.catalog",
    "slates.shown",
    "slates.subscribe",
    "slates.unsubscribe",
    "slates.resolve",
    "slates.image",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacesSetSpawn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_machines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacesSetSize {
    pub cpu: i64,
    pub mem_mb: i64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesCreateAgents {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_machines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesCreateSub {
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesResizeSize {
    pub cpu: i64,
    pub mem_mb: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspacesReviewDraftVerdict {
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "approve")]
    Approve,
    #[serde(rename = "request_changes")]
    RequestChanges,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesReviewDraftOnItem {
    pub id: String,
    pub on: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentsKeyWay {
    #[serde(rename = "token")]
    Token,
    #[serde(rename = "key")]
    Key,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentsCloudSignInAgent {
    #[serde(rename = "claude")]
    Claude,
    #[serde(rename = "codex")]
    Codex,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlatesWritePress {
    pub piece: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<i64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlatesEventEvent {
    #[serde(rename = "press")]
    Press,
    #[serde(rename = "submit")]
    Submit,
    #[serde(rename = "change")]
    Change,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlatesEventScope {
    pub item: Value,
    pub index: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlatesApproveScope {
    #[serde(rename = "once")]
    Once,
    #[serde(rename = "thread")]
    Thread,
    #[serde(rename = "refuse")]
    Refuse,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExecEvent {
    #[serde(rename = "exec.output", rename_all = "camelCase")]
    ExecOutput { exec_id: String, text: String },
    #[serde(rename = "exec.exit", rename_all = "camelCase")]
    ExecExit {
        exec_id: String,
        exit_code: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOkResponse {
    pub id: Option<ReqId>,
    pub ok: True,
    /// zod passthrough: unknown keys kept, in the order read.
    #[serde(flatten)]
    pub rest: IndexMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeErrorResponse {
    pub id: Option<ReqId>,
    pub ok: False,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuntimeResponse {
    V0(RuntimeOkResponse),
    V1(RuntimeErrorResponse),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionInterruptOutcome {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "not-running")]
    NotRunning,
    #[serde(rename = "not-found")]
    NotFound,
    #[serde(rename = "refused")]
    Refused,
    #[serde(rename = "unsupported")]
    Unsupported,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInterruptResult {
    pub outcome: SessionInterruptOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionSteerOutcome {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "not-running")]
    NotRunning,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "not-found")]
    NotFound,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSteerResult {
    pub outcome: SessionSteerOutcome,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionAccessOutcome {
    #[serde(rename = "set")]
    Set,
    #[serde(rename = "not-running")]
    NotRunning,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "not-found")]
    NotFound,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAccessResult {
    pub outcome: SessionAccessOutcome,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionAnswerOutcome {
    #[serde(rename = "answered")]
    Answered,
    #[serde(rename = "gone")]
    Gone,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "not-found")]
    NotFound,
    #[serde(rename = "no-option")]
    NoOption,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAnswerResult {
    pub outcome: SessionAnswerOutcome,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionRenameOutcome {
    #[serde(rename = "renamed")]
    Renamed,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "no-session")]
    NoSession,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "not-found")]
    NotFound,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRenameResult {
    pub outcome: SessionRenameOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettledThread {
    pub thread_id: String,
    pub title: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSettleResultLeftItem {
    pub thread_id: String,
    pub why: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSettleResult {
    pub settled: Vec<SettledThread>,
    pub left: Vec<SessionSettleResultLeftItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<Vec<ActivityEnded>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRestoreResult {
    pub restored: Vec<SettledThread>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchHit {
    pub workspace_id: String,
    pub thread_id: String,
    pub snippet: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchResult {
    pub hits: Vec<SessionSearchHit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsearched: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAsideResult {
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRewindResult {
    pub turns: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionStartOutcome {
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "steered")]
    Steered,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "held")]
    Held,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartResult {
    pub session: SessionView,
    pub outcome: SessionStartOutcome,
    pub turn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionWarmResultWarm {
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "standing")]
    Standing,
    #[serde(rename = "none")]
    None,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWarmResult {
    pub warm: SessionWarmResultWarm,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderKeyWorks {
    pub works: True,
    pub account: String,
    pub plan: String,
    pub running: i64,
    pub limit: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrokerStatusClaude {
    #[serde(rename = "token")]
    Token,
    #[serde(rename = "key")]
    Key,
    #[serde(rename = "gateway")]
    Gateway,
    #[serde(rename = "login")]
    Login,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrokerStatusCodex {
    #[serde(rename = "chatgpt")]
    Chatgpt,
    #[serde(rename = "access-token")]
    AccessToken,
    #[serde(rename = "key")]
    Key,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokerStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude: Option<BrokerStatusClaude>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex: Option<BrokerStatusCodex>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeOp {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `maxMachines`: int min 0
/// - `maxDepth`: int min 1
impl Check for PlacesSetSpawn {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlacesSetSize {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `maxMachines`: int min 0
/// - `maxDepth`: int min 1
impl Check for WorkspacesCreateAgents {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspacesCreateSub {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `cpu`: int min 0 exclusive
/// - `memMb`: int min 0 exclusive
impl Check for WorkspacesResizeSize {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspacesReviewDraftOnItem {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `index`: int min 0
/// - `action`: int min 0
impl Check for SlatesWritePress {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `index`: int min 0
impl Check for SlatesEventScope {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ExecEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeOkResponse {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeErrorResponse {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeResponse {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionInterruptResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSteerResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAccessResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAnswerResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRenameResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SettledThread {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSettleResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSettleResultLeftItem {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRestoreResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSearchHit {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `unsearched`: int min 0 exclusive
impl Check for SessionSearchResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAsideResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRewindResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionStartResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionWarmResult {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `running`: int min 0
/// - `limit`: int min 0
impl Check for ProviderKeyWorks {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BrokerStatus {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// Refinements on the variants of RuntimeOp (runtime_ops.rs), each an issue when it fails:
// - RuntimeOp::PairRedeem.code: string max 64
// - RuntimeOp::PairRedeem.name: string max 200
// - RuntimeOp::PlacesDoctor.doctorId: string max 64
// - RuntimeOp::PlacesDoctor.project: string max 200
// - RuntimeOp::PlacesSet.name: string max 200
// - RuntimeOp::PlacesSet.ssh: string max 300
// - RuntimeOp::PlacesSet.recipe: string min 1
// - RuntimeOp::PlacesSet.recipe: string max 200
// - RuntimeOp::PlacesSet.threads: int min 1
// - RuntimeOp::PlacesSet.machines: int min 1
// - RuntimeOp::PlacesSet.spendPerDayUsd: number min 0
// - RuntimeOp::PlacesSet.napMs: int min 60000
// - RuntimeOp::PlacesSet.napMs: int max 10800000
// - RuntimeOp::PlacesSet.napMs: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::PlacesSet.turnLimitMs: int min 3600000
// - RuntimeOp::PlacesSet.turnLimitMs: int max 86400000
// - RuntimeOp::PlacesSet.turnLimitMs: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::PlacesSet.deleteAfterMs: int min 0
// - RuntimeOp::PlacesSet.deleteAfterMs: int max 86400000
// - RuntimeOp::PlacesFollow.recipe: string min 1
// - RuntimeOp::PlacesFollow.recipe: string max 200
// - RuntimeOp::PlacesSkip.row: string min 1
// - RuntimeOp::PlacesSkip.row: string max 300
// - RuntimeOp::PlacesEstimate.ref: string max 200
// - RuntimeOp::PlacesAdd.addId: string max 64
// - RuntimeOp::PlacesAdd.address: string max 200
// - RuntimeOp::PlacesAdd.name: string max 200
// - RuntimeOp::PlacesAdd.sshPort: int min 1
// - RuntimeOp::PlacesAdd.sshPort: int max 65535
// - RuntimeOp::PlacesAdd.keyPath: string max 1024
// - RuntimeOp::PlacesAdd.hostKey: string max 200
// - RuntimeOp::PlacesAdd.recipe: string max 200
// - RuntimeOp::PlacesSetup.ref: string max 200
// - RuntimeOp::PlacesSetup.recipe: string max 200
// - RuntimeOp::PlacesSetup.addId: string max 64
// - RuntimeOp::PlacesSetup.row: string max 400
// - RuntimeOp::PlacesChoose.ref: string max 200
// - RuntimeOp::PlacesChoose.recipe: string max 200
// - RuntimeOp::RecipesGet.name: string max 200
// - RuntimeOp::RecipesSave.name: string max 200
// - RuntimeOp::RecipesSave.from: string max 200
// - RuntimeOp::RecipesRemove.name: string max 200
// - RuntimeOp::EventsSubscribe.after: int min 0
// - RuntimeOp::WorkspacesCreate.idleWindowMs: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::WorkspacesLook.theme: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::WorkspacesLook.glyph: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::WorkspacesViewed.blob: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::WorkspacesPullRequestSend.items: array min 1
// - RuntimeOp::WorkspacesPullRequestReply.replyTo: int min 0
// - RuntimeOp::WorkspacesPullRequestReply.body: string max 65536
// - RuntimeOp::WorkspacesMerge.head: string min 1
// - RuntimeOp::ActivityEnd.pid: int min 0 exclusive
// - RuntimeOp::SessionsStart.notify: array min 1
// - RuntimeOp::SessionsHistory.before: int min 0 exclusive
// - RuntimeOp::SessionsHistory.limit: int min 0 exclusive
// - RuntimeOp::SessionsHistory.limit: int max 1000
// - RuntimeOp::SessionsAttachment.index: int min 0
// - RuntimeOp::SessionsSettle.threadIds: array min 1
// - RuntimeOp::SessionsMark.threadIds: array min 1
// - RuntimeOp::SessionsRestore.threadIds: array min 1
// - RuntimeOp::WorkspacesPortReach.port: int min 1
// - RuntimeOp::WorkspacesPortReach.port: int max 65535
// - RuntimeOp::WorkspacesPortProbe.port: int min 1
// - RuntimeOp::WorkspacesPortProbe.port: int max 65535
// - RuntimeOp::WorkspacesExec.argv: array min 1
// - RuntimeOp::EditorOpen.line: int min 0 exclusive
// - RuntimeOp::ServersIcon.host: string min 1
// - RuntimeOp::ServersIcon.host: string max 260
// - RuntimeOp::AgentsSignInCode.code: string min 1
// - RuntimeOp::AgentsKey.key: string min 1
// - RuntimeOp::ProvidersCheckKey.provider: string max 64
// - RuntimeOp::ProvidersCheckKey.key: string min 1
// - RuntimeOp::AgentsSetup.program: string min 1
// - RuntimeOp::AgentsSetup.program: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::AgentsSetup.configDir: string min 1
// - RuntimeOp::AgentsSetup.configDir: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::AgentsSetup.args: absent, null or a value: Option<Option<_>> keeps the three apart
// - RuntimeOp::SkillsSearch.limit: int min 1
// - RuntimeOp::SkillsSearch.limit: int max 50
// - RuntimeOp::InitKeys.provider: string max 64
// - RuntimeOp::InitStep.at: int min 0
// - RuntimeOp::InitDraft.at: string min 1
// - RuntimeOp::InitDraft.at: string max 64
// - RuntimeOp::InitSignInCode.code: string min 1
// - RuntimeOp::InitSignInCode.code: string max 256
// - RuntimeOp::ConversationsRead.agent: string min 1
// - RuntimeOp::ConversationsRead.conversation: string min 1
// - RuntimeOp::ConversationsRead.conversation: string max 200
// - RuntimeOp::ProjectsIcon.png: string max 327680
// - RuntimeOp::ProjectsIcons.hashes: array max 1000
// - RuntimeOp::ImageBuild.place: string min 1
// - RuntimeOp::ImageExport.dest: string min 1
// - RuntimeOp::ImageExport.passphrase: string min 12
// - RuntimeOp::ImageExport.passphrase: string max 256
// - RuntimeOp::SlatesEvent.requestId: string min 1
// - RuntimeOp::SlatesEvent.requestId: string max 200
// - RuntimeOp::SlatesEvent.rowAction: int min 0
// - RuntimeOp::SlatesResolve.paths: array max 200
// - RuntimeOp::SlatesImage.src: string max 4096
// - RuntimeOp::SlatesImage.have: string max 200

// runtime.ts:1035
// /** The secret values a request frame carries, read one level into a record and no deeper: a record of logins is as
//  * deep as a secret field goes, and the frame may be a stranger's. */
// export function requestSecrets(frame: unknown): string[] {
//   if (typeof frame !== "object" || frame === null) return [];
//   return SECRET_REQUEST_FIELDS.flatMap(field => {
//     const v = (frame as Record<string, unknown>)[field];
//     return typeof v === "string" ? [v] : typeof v === "object" && v !== null ? Object.values(v).filter((x): x is string => typeof x === "string") : [];
//   });
// }
pub fn request_secrets(frame: &Value) -> Vec<String> {
    todo!()
}
// runtime.ts:1251
// /** The workspace an event is about, for the one reading every door that hides a workspace from a caller shares: the
//  * id on the event itself, else the id of the record or the status it carries. An event that names none is about
//  * this host rather than about any workspace, which is why a caller that may see only its own tree is sent none. */
// export function workspaceIdOf(event: unknown): string | undefined {
//   const e = event as { workspaceId?: unknown; workspace?: { id?: unknown }; status?: { id?: unknown }; forward?: { workspaceId?: unknown } };
//   for (const found of [e.workspaceId, e.workspace?.id, e.status?.id, e.forward?.workspaceId]) if (typeof found === "string") return found;
//   return undefined;
// }
pub fn workspace_id_of(event: &Value) -> Option<String> {
    todo!()
}
// runtime.ts:1260
// /** The thread an event is on behalf of, beside the reading above and for the same door: an event about a workspace
//  * that has no record yet is nobody's by the reading above, so the caller it was asked for by is named on it. */
// export function askerOf(event: unknown): EventAsker | undefined {
//   const asked = (event as { askedBy?: { threadId?: unknown; rootThreadId?: unknown } }).askedBy;
//   if (typeof asked?.threadId !== "string" || typeof asked.rootThreadId !== "string") return undefined;
//   return { threadId: asked.threadId, rootThreadId: asked.rootThreadId };
// }
pub fn asker_of(event: &Value) -> Option<EventAsker> {
    todo!()
}
// runtime.ts:1318
// /** What a stop of one subagent the agent refused says, in the agent's own words; the line it stands in names the task. */
// export const taskStopRefusedLine = (agent: string, why: string): string => `${agent} would not stop it: ${why}`;
pub fn task_stop_refused_line(agent: &str, why: &str) -> String {
    todo!()
}
// runtime.ts:1320
// /** What a stop of one subagent says where the agent offers none. */
// export const taskStopUnsupportedLine = (agent: string): string => `Stop is not available for ${agent} subagents; stop the thread to stop them all`;
pub fn task_stop_unsupported_line(agent: &str) -> String {
    todo!()
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for WorkspacesReviewDraftVerdict {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for AgentsKeyWay {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for AgentsCloudSignInAgent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SlatesEventEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SlatesApproveScope {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SessionInterruptOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SessionSteerOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SessionAccessOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SessionAnswerOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SessionRenameOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SessionStartOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SessionWarmResultWarm {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for BrokerStatusClaude {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for BrokerStatusCodex {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

pub use super::runtime_ops::{parse_runtime_request, RuntimeOp, RuntimeRequest};
