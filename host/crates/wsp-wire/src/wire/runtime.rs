// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/runtime.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::agents_report::{AgentsTarget, McpScope};
use crate::types::attachments::Attachment;
use crate::types::check::{
    double_option, env_name, items_at_least, items_at_most, leaf, num_above, num_at_least, num_at_most, text_at_least, text_at_most, Zod,
};
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
/// Read as a plain struct first: serde's tagged enums lose the path of a field that fails, and the tests read it by path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", try_from = "ExecEventFrame")]
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
/// zod's z.record(EnvName, ...): each key is a variable name, and a refused key is refused on its own path.
fn env_names(env: &IndexMap<String, Option<String>>) -> Result<(), SchemaError> {
    match env.keys().find(|key| !env_name(key)) {
        None => Ok(()),
        Some(key) => Err(SchemaError {
            code: "invalid_string".into(),
            path: vec![Value::from(key.as_str())],
            message: "a variable's name is letters, digits and _, not starting with a digit".into(),
            detail: IndexMap::from([("validation".to_string(), Value::from("regex"))]),
        }),
    }
}

/// zod's `z.literal(true)`: a false is invalid_literal, which serde's own words for the literal would not answer.
fn literal_true<'de, D: Deserializer<'de>>(d: D) -> Result<True, D::Error> {
    if bool::deserialize(d)? {
        Ok(True)
    } else {
        Err(D::Error::custom("Invalid literal value, expected true"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderKeyWorks {
    #[serde(deserialize_with = "literal_true")]
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
        match self {
            Self::Auth { .. } => Ok(()),
            Self::TicketIssue { .. } => Ok(()),
            Self::PairIssue { .. } => Ok(()),
            Self::PairRedeem { code, name, .. } => {
                text_at_most(code, 64).map_err(|e| e.under("code"))?;
                text_at_most(name, 200).map_err(|e| e.under("name"))?;
                Ok(())
            }
            Self::SealOpen(r) => r.check(),
            Self::DeviceAuth(r) => r.check(),
            Self::AccountGet { .. } => Ok(()),
            Self::DevicesList { .. } => Ok(()),
            Self::DevicesRevoke { .. } => Ok(()),
            Self::PlaceJoin(r) => r.check(),
            Self::PlaceAuth(r) => r.check(),
            Self::PlaceProve(r) => r.check(),
            Self::PlacesList { .. } => Ok(()),
            Self::PlacesUpdate { .. } => Ok(()),
            Self::PlacesHolds { .. } => Ok(()),
            Self::PlacesRemove { .. } => Ok(()),
            Self::PlacesDoctor { doctor_id, project, .. } => {
                text_at_most(doctor_id, 64).map_err(|e| e.under("doctorId"))?;
                if let Some(v) = project {
                    text_at_most(v, 200).map_err(|e| e.under("project"))?;
                }
                Ok(())
            }
            Self::PlacesDial { .. } => Ok(()),
            Self::PlacesSet {
                name,
                ssh,
                recipe,
                threads,
                machines,
                spend_per_day_usd,
                nap_ms,
                turn_limit_ms,
                spawn,
                size,
                delete_after_ms,
                ..
            } => {
                if let Some(v) = name {
                    text_at_most(v, 200).map_err(|e| e.under("name"))?;
                }
                if let Some(v) = ssh {
                    text_at_most(v, 300).map_err(|e| e.under("ssh"))?;
                }
                if let Some(v) = recipe {
                    text_at_least(v, 1).map_err(|e| e.under("recipe"))?;
                }
                if let Some(v) = recipe {
                    text_at_most(v, 200).map_err(|e| e.under("recipe"))?;
                }
                if let Some(v) = threads {
                    num_at_least(*v as f64, 1.0).map_err(|e| e.under("threads"))?;
                }
                if let Some(v) = machines {
                    num_at_least(*v as f64, 1.0).map_err(|e| e.under("machines"))?;
                }
                if let Some(v) = spend_per_day_usd {
                    num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
                }
                spend_per_day_usd.check().map_err(|e| e.under("spendPerDayUsd"))?;
                if let Some(Some(v)) = nap_ms {
                    num_at_least(*v as f64, 60000.0).map_err(|e| e.under("napMs"))?;
                }
                if let Some(Some(v)) = nap_ms {
                    num_at_most(*v as f64, 10800000.0).map_err(|e| e.under("napMs"))?;
                }
                if let Some(Some(v)) = turn_limit_ms {
                    num_at_least(*v as f64, 3600000.0).map_err(|e| e.under("turnLimitMs"))?;
                }
                if let Some(Some(v)) = turn_limit_ms {
                    num_at_most(*v as f64, 86400000.0).map_err(|e| e.under("turnLimitMs"))?;
                }
                spawn.check().map_err(|e| e.under("spawn"))?;
                size.check().map_err(|e| e.under("size"))?;
                if let Some(v) = delete_after_ms {
                    num_at_least(*v as f64, 0.0).map_err(|e| e.under("deleteAfterMs"))?;
                }
                if let Some(v) = delete_after_ms {
                    num_at_most(*v as f64, 86400000.0).map_err(|e| e.under("deleteAfterMs"))?;
                }
                Ok(())
            }
            Self::PlacesFollow { recipe, .. } => {
                text_at_least(recipe, 1).map_err(|e| e.under("recipe"))?;
                text_at_most(recipe, 200).map_err(|e| e.under("recipe"))?;
                Ok(())
            }
            Self::PlacesSkip { row, .. } => {
                text_at_least(row, 1).map_err(|e| e.under("row"))?;
                text_at_most(row, 300).map_err(|e| e.under("row"))?;
                Ok(())
            }
            Self::PlacesSetupLog { .. } => Ok(()),
            Self::PlacesEstimate { r#ref, choices, .. } => {
                text_at_most(r#ref, 200).map_err(|e| e.under("ref"))?;
                choices.check().map_err(|e| e.under("choices"))?;
                Ok(())
            }
            Self::PlacesLoginLanded { .. } => Ok(()),
            Self::PlacesDoor { .. } => Ok(()),
            Self::PlacesMint { .. } => Ok(()),
            Self::PlacesSshHosts { .. } => Ok(()),
            Self::PlacesAdd { add_id, address, name, ssh_port, key_path, host_key, recipe, .. } => {
                if let Some(v) = add_id {
                    text_at_most(v, 64).map_err(|e| e.under("addId"))?;
                }
                text_at_most(address, 200).map_err(|e| e.under("address"))?;
                if let Some(v) = name {
                    text_at_most(v, 200).map_err(|e| e.under("name"))?;
                }
                if let Some(v) = ssh_port {
                    num_at_least(*v as f64, 1.0).map_err(|e| e.under("sshPort"))?;
                }
                if let Some(v) = ssh_port {
                    num_at_most(*v as f64, 65535.0).map_err(|e| e.under("sshPort"))?;
                }
                if let Some(v) = key_path {
                    text_at_most(v, 1024).map_err(|e| e.under("keyPath"))?;
                }
                if let Some(v) = host_key {
                    text_at_most(v, 200).map_err(|e| e.under("hostKey"))?;
                }
                if let Some(v) = recipe {
                    text_at_most(v, 200).map_err(|e| e.under("recipe"))?;
                }
                Ok(())
            }
            Self::PlacesSetup { r#ref, recipe, choices, add_id, row, .. } => {
                text_at_most(r#ref, 200).map_err(|e| e.under("ref"))?;
                if let Some(v) = recipe {
                    text_at_most(v, 200).map_err(|e| e.under("recipe"))?;
                }
                choices.check().map_err(|e| e.under("choices"))?;
                if let Some(v) = add_id {
                    text_at_most(v, 64).map_err(|e| e.under("addId"))?;
                }
                if let Some(v) = row {
                    text_at_most(v, 400).map_err(|e| e.under("row"))?;
                }
                Ok(())
            }
            Self::PlacesChoose { r#ref, choices, recipe, .. } => {
                text_at_most(r#ref, 200).map_err(|e| e.under("ref"))?;
                choices.check().map_err(|e| e.under("choices"))?;
                if let Some(v) = recipe {
                    text_at_most(v, 200).map_err(|e| e.under("recipe"))?;
                }
                Ok(())
            }
            Self::RecipesList { .. } => Ok(()),
            Self::RecipesGet { name, .. } => {
                text_at_most(name, 200).map_err(|e| e.under("name"))?;
                Ok(())
            }
            Self::RecipesSave { name, from, .. } => {
                text_at_most(name, 200).map_err(|e| e.under("name"))?;
                if let Some(v) = from {
                    text_at_most(v, 200).map_err(|e| e.under("from"))?;
                }
                Ok(())
            }
            Self::RecipesRemove { name, .. } => {
                text_at_most(name, 200).map_err(|e| e.under("name"))?;
                Ok(())
            }
            Self::RecipesOptions { .. } => Ok(()),
            Self::EventsSubscribe { after, .. } => {
                if let Some(v) = after {
                    num_at_least(*v as f64, 0.0).map_err(|e| e.under("after"))?;
                }
                Ok(())
            }
            Self::StatusSubscribe { .. } => Ok(()),
            Self::StatusList { .. } => Ok(()),
            Self::WorkspacesCreate { cpu, mem_mb, agents, idle_window_ms, sub, .. } => {
                cpu.check().map_err(|e| e.under("cpu"))?;
                mem_mb.check().map_err(|e| e.under("memMb"))?;
                agents.check().map_err(|e| e.under("agents"))?;
                idle_window_ms.check().map_err(|e| e.under("idleWindowMs"))?;
                sub.check().map_err(|e| e.under("sub"))?;
                Ok(())
            }
            Self::WorkspacesLanding { .. } => Ok(()),
            Self::WorkspacesList { .. } => Ok(()),
            Self::WorkspacesResolve { .. } => Ok(()),
            Self::WorkspacesGet { .. } => Ok(()),
            Self::WorkspacesNap { .. } => Ok(()),
            Self::WorkspacesResize { size, .. } => {
                size.check().map_err(|e| e.under("size"))?;
                Ok(())
            }
            Self::WorkspacesWake { .. } => Ok(()),
            Self::WorkspacesRestartDaemon { .. } => Ok(()),
            Self::WorkspacesStopWake { .. } => Ok(()),
            Self::WorkspacesUpgrade { .. } => Ok(()),
            Self::WorkspacesRename { .. } => Ok(()),
            Self::WorkspacesLook { theme, .. } => {
                theme.check().map_err(|e| e.under("theme"))?;
                Ok(())
            }
            Self::WorkspacesBringBack { .. } => Ok(()),
            Self::WorktreeMake { .. } => Ok(()),
            Self::FolderMake { .. } => Ok(()),
            Self::WorktreeRemove { .. } => Ok(()),
            Self::WorkspacesCheckout { .. } => Ok(()),
            Self::WorkspacesDiscard { .. } => Ok(()),
            Self::WorkspacesCommit { .. } => Ok(()),
            Self::WorkspacesCommitDraft { .. } => Ok(()),
            Self::WorkspacesViewed { .. } => Ok(()),
            Self::WorkspacesPullRequestView { .. } => Ok(()),
            Self::WorkspacesPullRequestDiff { .. } => Ok(()),
            Self::WorkspacesPullRequestSend { items, .. } => {
                items_at_least(items.len(), 1).map_err(|e| e.under("items"))?;
                items.check().map_err(|e| e.under("items"))?;
                Ok(())
            }
            Self::WorkspacesPullRequestReply { reply_to, body, .. } => {
                if let Some(v) = reply_to {
                    num_at_least(*v as f64, 0.0).map_err(|e| e.under("replyTo"))?;
                }
                text_at_most(body, 65536).map_err(|e| e.under("body"))?;
                Ok(())
            }
            Self::WorkspacesPullRequestResolve { .. } => Ok(()),
            Self::WorkspacesPullRequestReact { content, .. } => {
                content.check().map_err(|e| e.under("content"))?;
                Ok(())
            }
            Self::WorkspacesFix { .. } => Ok(()),
            Self::WorkspacesMerge { method, head, .. } => {
                method.check().map_err(|e| e.under("method"))?;
                if let Some(v) = head {
                    text_at_least(v, 1).map_err(|e| e.under("head"))?;
                }
                Ok(())
            }
            Self::WorkspacesStart { .. } => Ok(()),
            Self::WorkspacesReview { .. } => Ok(()),
            Self::WorkspacesReviewDraft { on, .. } => {
                on.check().map_err(|e| e.under("on"))?;
                Ok(())
            }
            Self::WorkspacesReviewPost { .. } => Ok(()),
            Self::WorkspacesUpdate { .. } => Ok(()),
            Self::WorkspacesMergeIn { .. } => Ok(()),
            Self::WorkspacesDelete { .. } => Ok(()),
            Self::WorkspacesForget { .. } => Ok(()),
            Self::WorkspacesSnapshot { .. } => Ok(()),
            Self::ProjectGoldensList { .. } => Ok(()),
            Self::ProjectGoldensRemove { .. } => Ok(()),
            Self::WorkspacesTouch { .. } => Ok(()),
            Self::DaemonOpen { .. } => Ok(()),
            Self::SysSubscribe { .. } => Ok(()),
            Self::SysUnsubscribe { .. } => Ok(()),
            Self::AppProcsWatch { .. } => Ok(()),
            Self::AppProcsUnwatch { .. } => Ok(()),
            Self::AppProcsOffer { .. } => Ok(()),
            Self::AppProcsSet { procs, .. } => {
                procs.check().map_err(|e| e.under("procs"))?;
                Ok(())
            }
            Self::ActivityRead { .. } => Ok(()),
            Self::ActivityEnd { pid, started_at, .. } => {
                num_above(*pid as f64, 0.0).map_err(|e| e.under("pid"))?;
                started_at.check().map_err(|e| e.under("startedAt"))?;
                Ok(())
            }
            Self::DaemonSend { frame, .. } => {
                frame.check().map_err(|e| e.under("frame"))?;
                Ok(())
            }
            Self::DaemonClose { .. } => Ok(()),
            Self::SessionsStart { notify, resume, fork, attachments, .. } => {
                if let Some(v) = notify {
                    items_at_least(v.len(), 1).map_err(|e| e.under("notify"))?;
                }
                resume.check().map_err(|e| e.under("resume"))?;
                fork.check().map_err(|e| e.under("fork"))?;
                attachments.check().map_err(|e| e.under("attachments"))?;
                Ok(())
            }
            Self::HarnessesList { .. } => Ok(()),
            Self::SessionsList { .. } => Ok(()),
            Self::SessionsHistory { before, limit, .. } => {
                if let Some(v) = before {
                    num_above(*v as f64, 0.0).map_err(|e| e.under("before"))?;
                }
                if let Some(v) = limit {
                    num_above(*v as f64, 0.0).map_err(|e| e.under("limit"))?;
                }
                if let Some(v) = limit {
                    num_at_most(*v as f64, 1000.0).map_err(|e| e.under("limit"))?;
                }
                Ok(())
            }
            Self::SessionsHead { .. } => Ok(()),
            Self::SessionsAttachment { index, .. } => {
                num_at_least(*index as f64, 0.0).map_err(|e| e.under("index"))?;
                Ok(())
            }
            Self::SessionsInterrupt { .. } => Ok(()),
            Self::SessionsSteer { attachments, .. } => {
                attachments.check().map_err(|e| e.under("attachments"))?;
                Ok(())
            }
            Self::SessionsAnswer { .. } => Ok(()),
            Self::SessionsAccess { .. } => Ok(()),
            Self::SessionsRename { .. } => Ok(()),
            Self::SessionsForget { .. } => Ok(()),
            Self::SessionsDelete { .. } => Ok(()),
            Self::SessionsRead { .. } => Ok(()),
            Self::SessionsReplaceable { .. } => Ok(()),
            Self::SessionsWarm { .. } => Ok(()),
            Self::SessionsSettle { thread_ids, .. } => {
                items_at_least(thread_ids.len(), 1).map_err(|e| e.under("threadIds"))?;
                Ok(())
            }
            Self::SessionsMark { thread_ids, marks, .. } => {
                items_at_least(thread_ids.len(), 1).map_err(|e| e.under("threadIds"))?;
                marks.check().map_err(|e| e.under("marks"))?;
                Ok(())
            }
            Self::SessionsRestore { thread_ids, .. } => {
                items_at_least(thread_ids.len(), 1).map_err(|e| e.under("threadIds"))?;
                Ok(())
            }
            Self::SessionsSearch { .. } => Ok(()),
            Self::SessionsAside { .. } => Ok(()),
            Self::SessionsRun { .. } => Ok(()),
            Self::SessionsRewind { .. } => Ok(()),
            Self::GoldenGet { .. } => Ok(()),
            Self::CapabilitiesGet { .. } => Ok(()),
            Self::GoldenPrepare { kind, .. } => {
                kind.check().map_err(|e| e.under("kind"))?;
                Ok(())
            }
            Self::GoldenSeal { .. } => Ok(()),
            Self::SnapshotsList { .. } => Ok(()),
            Self::SnapshotsStorage { .. } => Ok(()),
            Self::CostHistory { .. } => Ok(()),
            Self::CostSpend { .. } => Ok(()),
            Self::UsageUsed { .. } => Ok(()),
            Self::UsageAccounts { .. } => Ok(()),
            Self::UsageReset { .. } => Ok(()),
            Self::PlacesReadings { .. } => Ok(()),
            Self::SnapshotsRollback { version, .. } => {
                version.check().map_err(|e| e.under("version"))?;
                Ok(())
            }
            Self::WorkspacesPortReach { port, .. } => {
                num_at_least(*port as f64, 1.0).map_err(|e| e.under("port"))?;
                num_at_most(*port as f64, 65535.0).map_err(|e| e.under("port"))?;
                Ok(())
            }
            Self::WorkspacesPortProbe { port, .. } => {
                num_at_least(*port as f64, 1.0).map_err(|e| e.under("port"))?;
                num_at_most(*port as f64, 65535.0).map_err(|e| e.under("port"))?;
                Ok(())
            }
            Self::WorkspacesRebuild { .. } => Ok(()),
            Self::ForwardsList { .. } => Ok(()),
            Self::ForwardsStop { port, .. } => {
                port.check().map_err(|e| e.under("port"))?;
                Ok(())
            }
            Self::WorkspacesExec { argv, .. } => {
                items_at_least(argv.len(), 1).map_err(|e| e.under("argv"))?;
                Ok(())
            }
            Self::HostFolders { .. } => Ok(()),
            Self::HostTerminalConfig { .. } => Ok(()),
            Self::EditorList { .. } => Ok(()),
            Self::EditorOpen { line, .. } => {
                if let Some(v) = line {
                    num_above(*v as f64, 0.0).map_err(|e| e.under("line"))?;
                }
                Ok(())
            }
            Self::SshPort { .. } => Ok(()),
            Self::SshInclude { .. } => Ok(()),
            Self::AgentsRead { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::ServersTools { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::ServersIcon { host, .. } => {
                text_at_least(host, 1).map_err(|e| e.under("host"))?;
                text_at_most(host, 260).map_err(|e| e.under("host"))?;
                Ok(())
            }
            Self::AgentsSignIn { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::ServersSignIn { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::AgentsSignInCode { code, .. } => {
                text_at_least(code, 1).map_err(|e| e.under("code"))?;
                Ok(())
            }
            Self::AgentsSignIns { .. } => Ok(()),
            Self::AgentsSignInStop { .. } => Ok(()),
            Self::AgentsSignInLine { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::AgentsKey { key, .. } => {
                text_at_least(key, 1).map_err(|e| e.under("key"))?;
                Ok(())
            }
            Self::AgentsCloudSignIn { .. } => Ok(()),
            Self::ProvidersCheckKey { provider, key, .. } => {
                text_at_most(provider, 64).map_err(|e| e.under("provider"))?;
                text_at_least(key, 1).map_err(|e| e.under("key"))?;
                Ok(())
            }
            Self::BrokerStatus { .. } => Ok(()),
            Self::AgentsAddTools { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::AgentsSetup { program, config_dir, env, .. } => {
                if let Some(Some(v)) = program {
                    text_at_least(v, 1).map_err(|e| e.under("program"))?;
                }
                if let Some(Some(v)) = config_dir {
                    text_at_least(v, 1).map_err(|e| e.under("configDir"))?;
                }
                if let Some(v) = env {
                    env_names(v).map_err(|e| e.under("env"))?;
                }
                Ok(())
            }
            Self::SkillsSearch { limit, .. } => {
                if let Some(v) = limit {
                    num_at_least(*v as f64, 1.0).map_err(|e| e.under("limit"))?;
                }
                if let Some(v) = limit {
                    num_at_most(*v as f64, 50.0).map_err(|e| e.under("limit"))?;
                }
                Ok(())
            }
            Self::SkillsGet { .. } => Ok(()),
            Self::SkillsPreview { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::SkillsAdd { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::SkillsRemove { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::SkillsToggle { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::ServersAdd { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::ServersRemove { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::ServersToggle { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::PluginsToggle { target, .. } => {
                target.check().map_err(|e| e.under("target"))?;
                Ok(())
            }
            Self::InitGet { .. } => Ok(()),
            Self::InitKeys { provider, .. } => {
                if let Some(v) = provider {
                    text_at_most(v, 64).map_err(|e| e.under("provider"))?;
                }
                Ok(())
            }
            Self::InitStart { .. } => Ok(()),
            Self::InitAnswer { .. } => Ok(()),
            Self::InitStep { at, .. } => {
                num_at_least(*at as f64, 0.0).map_err(|e| e.under("at"))?;
                Ok(())
            }
            Self::InitDraft { at, .. } => {
                text_at_least(at, 1).map_err(|e| e.under("at"))?;
                text_at_most(at, 64).map_err(|e| e.under("at"))?;
                Ok(())
            }
            Self::InitRetry { .. } => Ok(()),
            Self::InitBuild { .. } => Ok(()),
            Self::InitSignInCode { code, .. } => {
                text_at_least(code, 1).map_err(|e| e.under("code"))?;
                text_at_most(code, 256).map_err(|e| e.under("code"))?;
                Ok(())
            }
            Self::InitCancel { .. } => Ok(()),
            Self::PreferencesGet { .. } => Ok(()),
            Self::PreferencesSet { patch, .. } => {
                patch.check().map_err(|e| e.under("patch"))?;
                Ok(())
            }
            Self::ReleaseGet { .. } => Ok(()),
            Self::ReleaseCheck { .. } => Ok(()),
            Self::HostRestart { .. } => Ok(()),
            Self::ProjectsAdd { seed, .. } => {
                seed.check().map_err(|e| e.under("seed"))?;
                Ok(())
            }
            Self::ProjectSeedPlan { .. } => Ok(()),
            Self::ProjectsSeed { .. } => Ok(()),
            Self::ProjectsList { .. } => Ok(()),
            Self::ConversationsList { .. } => Ok(()),
            Self::ConversationsRead { agent, conversation, .. } => {
                text_at_least(agent, 1).map_err(|e| e.under("agent"))?;
                text_at_least(conversation, 1).map_err(|e| e.under("conversation"))?;
                text_at_most(conversation, 200).map_err(|e| e.under("conversation"))?;
                Ok(())
            }
            Self::ProjectsDefaults { .. } => Ok(()),
            Self::ProjectsResolve { .. } => Ok(()),
            Self::ProjectsBranch { .. } => Ok(()),
            Self::ProjectsRemove { .. } => Ok(()),
            Self::ProjectsIcon { png, .. } => {
                if let Some(v) = png {
                    text_at_most(v, 327680).map_err(|e| e.under("png"))?;
                }
                Ok(())
            }
            Self::ProjectsIcons { hashes, .. } => {
                items_at_most(hashes.len(), 1000).map_err(|e| e.under("hashes"))?;
                Ok(())
            }
            Self::ProjectPlan { .. } => Ok(()),
            Self::ProjectImport { .. } => Ok(()),
            Self::ProjectExport { .. } => Ok(()),
            Self::ImageGet { .. } => Ok(()),
            Self::ImageBuild { place, .. } => {
                text_at_least(place, 1).map_err(|e| e.under("place"))?;
                Ok(())
            }
            Self::ImageExport { dest, passphrase, .. } => {
                text_at_least(dest, 1).map_err(|e| e.under("dest"))?;
                text_at_least(passphrase, 12).map_err(|e| e.under("passphrase"))?;
                text_at_most(passphrase, 256).map_err(|e| e.under("passphrase"))?;
                Ok(())
            }
            Self::SlatesGet { .. } => Ok(()),
            Self::SlatesWrite { press, .. } => {
                press.check().map_err(|e| e.under("press"))?;
                Ok(())
            }
            Self::SlatesState { .. } => Ok(()),
            Self::SlatesRead { .. } => Ok(()),
            Self::SlatesCatalog { .. } => Ok(()),
            Self::SlatesEvent { request_id, scope, row_action, .. } => {
                text_at_least(request_id, 1).map_err(|e| e.under("requestId"))?;
                text_at_most(request_id, 200).map_err(|e| e.under("requestId"))?;
                scope.check().map_err(|e| e.under("scope"))?;
                if let Some(v) = row_action {
                    num_at_least(*v as f64, 0.0).map_err(|e| e.under("rowAction"))?;
                }
                Ok(())
            }
            Self::SlatesApprove { .. } => Ok(()),
            Self::SlatesCancel { .. } => Ok(()),
            Self::SlatesRevoke { .. } => Ok(()),
            Self::SlatesShown { .. } => Ok(()),
            Self::SlatesSubscribe { .. } => Ok(()),
            Self::SlatesUnsubscribe { .. } => Ok(()),
            Self::SlatesResolve { paths, .. } => {
                items_at_most(paths.len(), 200).map_err(|e| e.under("paths"))?;
                Ok(())
            }
            Self::SlatesImage { src, have, .. } => {
                text_at_most(src, 4096).map_err(|e| e.under("src"))?;
                if let Some(v) = have {
                    text_at_most(v, 200).map_err(|e| e.under("have"))?;
                }
                Ok(())
            }
        }
    }

    /// serde reads the op as {word: fields}, so a path starts with the op word; the id is read by name.
    fn describe(path: &[Value]) -> Option<Zod> {
        let Some((_, rest)) = path.split_first() else {
            return Some(Zod::Word("object"));
        };
        match rest.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, tail)) => match key.as_str()? {
                "id" => leaf(tail, Zod::Union),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `maxMachines`: int min 0
/// - `maxDepth`: int min 1
impl Check for PlacesSetSpawn {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.max_machines {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("maxMachines"))?;
        }
        if let Some(v) = self.max_depth {
            num_at_least(v as f64, 1.0).map_err(|e| e.under("maxDepth"))?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlacesSetSize {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `maxMachines`: int min 0
/// - `maxDepth`: int min 1
impl Check for WorkspacesCreateAgents {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.max_machines {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("maxMachines"))?;
        }
        if let Some(v) = self.max_depth {
            num_at_least(v as f64, 1.0).map_err(|e| e.under("maxDepth"))?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspacesCreateSub {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `cpu`: int min 0 exclusive
/// - `memMb`: int min 0 exclusive
impl Check for WorkspacesResizeSize {
    fn check(&self) -> Result<(), SchemaError> {
        num_above(self.cpu as f64, 0.0).map_err(|e| e.under("cpu"))?;
        num_above(self.mem_mb as f64, 0.0).map_err(|e| e.under("memMb"))?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspacesReviewDraftOnItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `index`: int min 0
/// - `action`: int min 0
impl Check for SlatesWritePress {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.index {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("index"))?;
        }
        if let Some(v) = self.action {
            num_at_least(v as f64, 0.0).map_err(|e| e.under("action"))?;
        }
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `index`: int min 0
impl Check for SlatesEventScope {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.index as f64, 0.0).map_err(|e| e.under("index"))?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeRequest {
    fn check(&self) -> Result<(), SchemaError> {
        self.op.check()?;
        self.origin.check().map_err(|e| e.under("origin"))?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecEventFrame {
    #[serde(rename = "type")]
    kind: ExecEventKind,
    exec_id: String,
    text: Option<String>,
    #[serde(default, deserialize_with = "double_option::deserialize")]
    exit_code: Option<Option<i64>>,
    error: Option<String>,
}

#[derive(Deserialize)]
enum ExecEventKind {
    #[serde(rename = "exec.output")]
    Output,
    #[serde(rename = "exec.exit")]
    Exit,
}

impl TryFrom<ExecEventFrame> for ExecEvent {
    type Error = String;

    fn try_from(frame: ExecEventFrame) -> Result<Self, Self::Error> {
        Ok(match frame.kind {
            ExecEventKind::Output => ExecEvent::ExecOutput { exec_id: frame.exec_id, text: frame.text.ok_or("missing field `text`")? },
            ExecEventKind::Exit => ExecEvent::ExecExit {
                exec_id: frame.exec_id,
                exit_code: frame.exit_code.ok_or("missing field `exitCode`")?,
                error: frame.error,
            },
        })
    }
}

impl Check for ExecEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.first().and_then(Value::as_str) {
            Some("type") => Some(Zod::Tags("type", &["exec.output", "exec.exit"])),
            _ => None,
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeOkResponse {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeErrorResponse {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeResponse {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            Self::V0(r) => r.check(),
            Self::V1(r) => r.check(),
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionInterruptResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSteerResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAccessResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAnswerResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRenameResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SettledThread {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSettleResult {
    fn check(&self) -> Result<(), SchemaError> {
        self.settled.check().map_err(|e| e.under("settled"))?;
        self.ended.check().map_err(|e| e.under("ended"))?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSettleResultLeftItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRestoreResult {
    fn check(&self) -> Result<(), SchemaError> {
        self.restored.check().map_err(|e| e.under("restored"))?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSearchHit {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `unsearched`: int min 0 exclusive
impl Check for SessionSearchResult {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.unsearched {
            num_above(v as f64, 0.0).map_err(|e| e.under("unsearched"))?;
        }
        self.hits.check().map_err(|e| e.under("hits"))?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAsideResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRewindResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionStartResult {
    fn check(&self) -> Result<(), SchemaError> {
        self.session.check().map_err(|e| e.under("session"))?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionWarmResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `running`: int min 0
/// - `limit`: int min 0
impl Check for ProviderKeyWorks {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.running as f64, 0.0).map_err(|e| e.under("running"))?;
        num_at_least(self.limit as f64, 0.0).map_err(|e| e.under("limit"))?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BrokerStatus {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
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
    let Some(map) = frame.as_object() else {
        return Vec::new();
    };
    SECRET_REQUEST_FIELDS
        .iter()
        .flat_map(|field| match map.get(*field) {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Object(inner)) => inner.values().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            Some(Value::Array(items)) => items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            _ => Vec::new(),
        })
        .collect()
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
    [
        event.get("workspaceId"),
        event.get("workspace").and_then(|w| w.get("id")),
        event.get("status").and_then(|s| s.get("id")),
        event.get("forward").and_then(|f| f.get("workspaceId")),
    ]
    .into_iter()
    .flatten()
    .find_map(|v| v.as_str().map(str::to_string))
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
    let asked = event.get("askedBy")?;
    let thread_id = asked.get("threadId")?.as_str()?;
    let root_thread_id = asked.get("rootThreadId")?.as_str()?;
    Some(EventAsker { thread_id: thread_id.to_string(), root_thread_id: root_thread_id.to_string() })
}
// runtime.ts:1318
// /** What a stop of one subagent the agent refused says, in the agent's own words; the line it stands in names the task. */
// export const taskStopRefusedLine = (agent: string, why: string): string => `${agent} would not stop it: ${why}`;
pub fn task_stop_refused_line(agent: &str, why: &str) -> String {
    format!("{agent} would not stop it: {why}")
}
// runtime.ts:1320
// /** What a stop of one subagent says where the agent offers none. */
// export const taskStopUnsupportedLine = (agent: string): string => `Stop is not available for ${agent} subagents; stop the thread to stop them all`;
pub fn task_stop_unsupported_line(agent: &str) -> String {
    format!("Stop is not available for {agent} subagents; stop the thread to stop them all")
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for WorkspacesReviewDraftVerdict {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for AgentsKeyWay {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for AgentsCloudSignInAgent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SlatesEventEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SlatesApproveScope {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SessionInterruptOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SessionSteerOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SessionAccessOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SessionAnswerOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SessionRenameOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SessionStartOutcome {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SessionWarmResultWarm {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for BrokerStatusClaude {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for BrokerStatusCodex {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

pub use super::runtime_ops::{parse_runtime_request, RuntimeOp, RuntimeRequest};
