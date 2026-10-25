// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/runtime.ts.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub max_machines: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub max_depth: Option<JsInt>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacesSetSize {
    pub cpu: JsInt,
    pub mem_mb: JsInt,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesCreateAgents {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub max_machines: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub max_depth: Option<JsInt>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesCreateSub {
    pub on: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub harness: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesResizeSize {
    pub cpu: JsInt,
    pub mem_mb: JsInt,
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub index: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub action: Option<JsInt>,
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
    pub index: JsInt,
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
        exit_code: Option<JsInt>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
        error: Option<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOkResponse {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub id: Option<ReqId>,
    pub ok: True,
    /// zod passthrough: unknown keys kept, in the order read.
    #[serde(flatten, serialize_with = "crate::types::check::js_record")]
    pub rest: IndexMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeErrorResponse {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub id: Option<ReqId>,
    pub ok: False,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub under: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub unsearched: Option<JsInt>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAsideResult {
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRewindResult {
    pub turns: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub files: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderKeyWorks {
    pub works: True,
    pub account: String,
    pub plan: String,
    pub running: JsInt,
    pub limit: JsInt,
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude: Option<BrokerStatusClaude>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub codex: Option<BrokerStatusCodex>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
                    num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("threads"))?;
                }
                if let Some(v) = machines {
                    num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("machines"))?;
                }
                if let Some(v) = spend_per_day_usd {
                    num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
                }
                spend_per_day_usd.check().map_err(|e| e.under("spendPerDayUsd"))?;
                if let Some(Some(v)) = nap_ms {
                    num_at_least(v.as_f64(), 60000.0).map_err(|e| e.under("napMs"))?;
                }
                if let Some(Some(v)) = nap_ms {
                    num_at_most(v.as_f64(), 10800000.0).map_err(|e| e.under("napMs"))?;
                }
                if let Some(Some(v)) = turn_limit_ms {
                    num_at_least(v.as_f64(), 3600000.0).map_err(|e| e.under("turnLimitMs"))?;
                }
                if let Some(Some(v)) = turn_limit_ms {
                    num_at_most(v.as_f64(), 86400000.0).map_err(|e| e.under("turnLimitMs"))?;
                }
                spawn.check().map_err(|e| e.under("spawn"))?;
                size.check().map_err(|e| e.under("size"))?;
                if let Some(v) = delete_after_ms {
                    num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("deleteAfterMs"))?;
                }
                if let Some(v) = delete_after_ms {
                    num_at_most(v.as_f64(), 86400000.0).map_err(|e| e.under("deleteAfterMs"))?;
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
                    num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("sshPort"))?;
                }
                if let Some(v) = ssh_port {
                    num_at_most(v.as_f64(), 65535.0).map_err(|e| e.under("sshPort"))?;
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
                    num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("after"))?;
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
                    num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("replyTo"))?;
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
                num_above(pid.as_f64(), 0.0).map_err(|e| e.under("pid"))?;
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
                    num_above(v.as_f64(), 0.0).map_err(|e| e.under("before"))?;
                }
                if let Some(v) = limit {
                    num_above(v.as_f64(), 0.0).map_err(|e| e.under("limit"))?;
                }
                if let Some(v) = limit {
                    num_at_most(v.as_f64(), 1000.0).map_err(|e| e.under("limit"))?;
                }
                Ok(())
            }
            Self::SessionsHead { .. } => Ok(()),
            Self::SessionsAttachment { index, .. } => {
                num_at_least(index.as_f64(), 0.0).map_err(|e| e.under("index"))?;
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
                num_at_least(port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
                num_at_most(port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
                Ok(())
            }
            Self::WorkspacesPortProbe { port, .. } => {
                num_at_least(port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
                num_at_most(port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
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
                    num_above(v.as_f64(), 0.0).map_err(|e| e.under("line"))?;
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
                    num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("limit"))?;
                }
                if let Some(v) = limit {
                    num_at_most(v.as_f64(), 50.0).map_err(|e| e.under("limit"))?;
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
                num_at_least(at.as_f64(), 0.0).map_err(|e| e.under("at"))?;
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
                    num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("rowAction"))?;
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

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::tag_describe(path, "op", RUNTIME_OPS)
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("op") else { return Some(vec![crate::types::check::missing::<Self>("op")]); };
        match tag.as_str() {
            Some("auth") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("token") { None => out.push(crate::types::check::required::<String>("token")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("token"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("ticket.issue") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("purpose") { None => out.push(crate::types::check::required::<TicketPurpose>("purpose")), Some(v) => { match crate::types::check::read::<TicketPurpose>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("purpose"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pair.issue") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("here") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("here"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pair.redeem") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("code") { None => out.push(crate::types::check::required::<String>("code")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("code"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("seal.open") => { let mut body = obj.clone(); body.shift_remove("op"); Some(crate::types::check::issues_of::<SealOpenRequest>(&serde_json::Value::Object(body))) }
            Some("device.auth") => { let mut body = obj.clone(); body.shift_remove("op"); Some(crate::types::check::issues_of::<DeviceAuthRequest>(&serde_json::Value::Object(body))) }
            Some("account.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("devices.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("devices.revoke") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("deviceId") { None => out.push(crate::types::check::required::<String>("deviceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("deviceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("place.join") => { let mut body = obj.clone(); body.shift_remove("op"); Some(crate::types::check::issues_of::<PlaceJoinRequest>(&serde_json::Value::Object(body))) }
            Some("place.auth") => { let mut body = obj.clone(); body.shift_remove("op"); Some(crate::types::check::issues_of::<PlaceAuthRequest>(&serde_json::Value::Object(body))) }
            Some("place.prove") => { let mut body = obj.clone(); body.shift_remove("op"); Some(crate::types::check::issues_of::<PlaceProveRequest>(&serde_json::Value::Object(body))) }
            Some("places.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.update") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                if let Some(v) = obj.get("sudoPassword") { match crate::types::check::read_present::<SudoPassword>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sudoPassword"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.holds") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.remove") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                if let Some(v) = obj.get("sudoPassword") { match crate::types::check::read_present::<SudoPassword>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sudoPassword"))), } }
                if let Some(v) = obj.get("force") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("force"))), } }
                if let Some(v) = obj.get("forget") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("forget"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.doctor") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                match obj.get("doctorId") { None => out.push(crate::types::check::required::<String>("doctorId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("doctorId"))), } } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.dial") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.set") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                if let Some(v) = obj.get("reset") { match crate::types::check::read_present::<Vec<PlaceSettingWord>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reset"))), } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if let Some(v) = obj.get("ssh") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ssh"))), } }
                if let Some(v) = obj.get("recipe") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipe"))), } }
                if let Some(v) = obj.get("threads") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threads"))), } }
                if let Some(v) = obj.get("machines") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("machines"))), } }
                if let Some(v) = obj.get("spendPerDayUsd") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spendPerDayUsd"))), } }
                if let Some(v) = obj.get("napMs") { match crate::types::check::read_double::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("napMs"))), } }
                if let Some(v) = obj.get("turnLimitMs") { match crate::types::check::read_double::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnLimitMs"))), } }
                if let Some(v) = obj.get("spawn") { match crate::types::check::read_present::<PlacesSetSpawn>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawn"))), } }
                if let Some(v) = obj.get("size") { match crate::types::check::read_present::<PlacesSetSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } }
                if let Some(v) = obj.get("deleteAfterMs") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("deleteAfterMs"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.follow") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                match obj.get("recipe") { None => out.push(crate::types::check::required::<String>("recipe")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipe"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.skip") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                match obj.get("row") { None => out.push(crate::types::check::required::<String>("row")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("row"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.setupLog") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                if let Some(v) = obj.get("step") { match crate::types::check::read_present::<PlaceSetupStep>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("step"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.estimate") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("ref") { None => out.push(crate::types::check::required::<String>("ref")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ref"))), } } }
                match obj.get("choices") { None => out.push(crate::types::check::required::<RecipeFile>("choices")), Some(v) => { match crate::types::check::read::<RecipeFile>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("choices"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.loginLanded") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.door") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.mint") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.sshHosts") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.add") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("addId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("addId"))), } }
                match obj.get("address") { None => out.push(crate::types::check::required::<String>("address")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("address"))), } } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if let Some(v) = obj.get("sshPort") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sshPort"))), } }
                if let Some(v) = obj.get("keyPath") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("keyPath"))), } }
                if let Some(v) = obj.get("hostKey") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hostKey"))), } }
                if let Some(v) = obj.get("sudoPassword") { match crate::types::check::read_present::<SudoPassword>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sudoPassword"))), } }
                if let Some(v) = obj.get("recipe") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipe"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.setup") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("ref") { None => out.push(crate::types::check::required::<String>("ref")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ref"))), } } }
                if let Some(v) = obj.get("recipe") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipe"))), } }
                if let Some(v) = obj.get("choices") { match crate::types::check::read_present::<RecipeFile>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("choices"))), } }
                if let Some(v) = obj.get("addId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("addId"))), } }
                if let Some(v) = obj.get("row") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("row"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.choose") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("ref") { None => out.push(crate::types::check::required::<String>("ref")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ref"))), } } }
                match obj.get("choices") { None => out.push(crate::types::check::required::<RecipeFile>("choices")), Some(v) => { match crate::types::check::read::<RecipeFile>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("choices"))), } } }
                if let Some(v) = obj.get("recipe") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("recipe"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("recipes.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("recipes.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("recipes.save") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("file") { match crate::types::check::read_present::<Value>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("file"))), } }
                if let Some(v) = obj.get("from") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("recipes.remove") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("check") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("check"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("recipes.options") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("events.subscribe") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("after") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("after"))), } }
                if let Some(v) = obj.get("stream") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stream"))), } }
                if let Some(v) = obj.get("creates") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("creates"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("status.subscribe") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("status.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.create") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("golden") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("golden"))), } }
                match obj.get("project") { None => out.push(crate::types::check::required::<String>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("cpu") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cpu"))), } }
                if let Some(v) = obj.get("memMb") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memMb"))), } }
                if let Some(v) = obj.get("envs") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("envs"))), } }
                if let Some(v) = obj.get("labels") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("labels"))), } }
                if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<WorkspacesCreateAgents>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
                if let Some(v) = obj.get("idleWindowMs") { match crate::types::check::read_double::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("idleWindowMs"))), } }
                if let Some(v) = obj.get("parent") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("parent"))), } }
                if let Some(v) = obj.get("engine") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("engine"))), } }
                if let Some(v) = obj.get("sub") { match crate::types::check::read_present::<WorkspacesCreateSub>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sub"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.landing") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("project") { None => out.push(crate::types::check::required::<String>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
                if let Some(v) = obj.get("sized") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sized"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.resolve") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("ref") { None => out.push(crate::types::check::required::<String>("ref")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ref"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.nap") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.resize") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("size") { None => out.push(crate::types::check::required::<WorkspacesResizeSize>("size")), Some(v) => { match crate::types::check::read::<WorkspacesResizeSize>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.wake") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.restartDaemon") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.stopWake") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.upgrade") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.rename") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.look") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("theme") { match crate::types::check::read_double::<WorkspaceTheme>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("theme"))), } }
                if let Some(v) = obj.get("glyph") { match crate::types::check::read_double::<WorkspaceGlyph>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("glyph"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.bringBack") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("title") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("title"))), } }
                if let Some(v) = obj.get("body") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("body"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("worktree.make") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("project") { None => out.push(crate::types::check::required::<String>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
                match obj.get("branch") { None => out.push(crate::types::check::required::<String>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("folder.make") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("project") { None => out.push(crate::types::check::required::<String>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("worktree.remove") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("project") { None => out.push(crate::types::check::required::<String>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
                match obj.get("branch") { None => out.push(crate::types::check::required::<String>("branch")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
                if let Some(v) = obj.get("force") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("force"))), } }
                if let Some(v) = obj.get("check") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("check"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.checkout") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("fresh") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fresh"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.discard") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("path") { None => out.push(crate::types::check::required::<String>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
                if let Some(v) = obj.get("check") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("check"))), } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.commit") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("message") { None => out.push(crate::types::check::required::<String>("message")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("message"))), } } }
                if let Some(v) = obj.get("paths") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("paths"))), } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.commitDraft") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("paths") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("paths"))), } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.viewed") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("path") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } }
                if let Some(v) = obj.get("blob") { match crate::types::check::read_double::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("blob"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.pullRequestView") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("fresh") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fresh"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.pullRequestDiff") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.pullRequestSend") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("items") { None => out.push(crate::types::check::required::<Vec<PullRequestItem>>("items")), Some(v) => { match crate::types::check::read::<Vec<PullRequestItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("items"))), } } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.pullRequestReply") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("replyTo") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replyTo"))), } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                match obj.get("body") { None => out.push(crate::types::check::required::<String>("body")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("body"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.pullRequestResolve") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("resolved") { None => out.push(crate::types::check::required::<bool>("resolved")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("resolved"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.pullRequestReact") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("subject") { None => out.push(crate::types::check::required::<String>("subject")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("subject"))), } } }
                match obj.get("content") { None => out.push(crate::types::check::required::<ReactionContent>("content")), Some(v) => { match crate::types::check::read::<ReactionContent>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("content"))), } } }
                match obj.get("on") { None => out.push(crate::types::check::required::<bool>("on")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.fix") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("check") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("check"))), } }
                if let Some(v) = obj.get("child") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("child"))), } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if let Some(v) = obj.get("childThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("childThreadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.merge") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("method") { match crate::types::check::read_present::<MergeMethod>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("method"))), } }
                if let Some(v) = obj.get("whenChecksPass") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("whenChecksPass"))), } }
                if let Some(v) = obj.get("head") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("head"))), } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.start") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("url") { None => out.push(crate::types::check::required::<String>("url")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if let Some(v) = obj.get("agent") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } }
                if let Some(v) = obj.get("model") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } }
                if let Some(v) = obj.get("effort") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("effort"))), } }
                if let Some(v) = obj.get("access") { match crate::types::check::read_present::<AccessChoice>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("access"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.review") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("url") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } }
                if let Some(v) = obj.get("workspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } }
                if let Some(v) = obj.get("agent") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } }
                if let Some(v) = obj.get("model") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } }
                if let Some(v) = obj.get("effort") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("effort"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.reviewDraft") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("summary") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("summary"))), } }
                if let Some(v) = obj.get("verdict") { match crate::types::check::read_present::<WorkspacesReviewDraftVerdict>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("verdict"))), } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<Vec<WorkspacesReviewDraftOnItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.reviewPost") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.update") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.mergeIn") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("child") { None => out.push(crate::types::check::required::<String>("child")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("child"))), } } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if let Some(v) = obj.get("childThreadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("childThreadId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.delete") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.forget") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.snapshot") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projectGoldens.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projectGoldens.remove") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("snapshotId") { None => out.push(crate::types::check::required::<String>("snapshotId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snapshotId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.touch") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("daemon.open") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("workspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } }
                if let Some(v) = obj.get("placeId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.subscribe") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("workspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } }
                if let Some(v) = obj.get("placeId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.unsubscribe") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("workspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } }
                if let Some(v) = obj.get("placeId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("appProcs.watch") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("appProcs.unwatch") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("appProcs.offer") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("appProcs.set") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("procs") { None => out.push(crate::types::check::required::<AppProcList>("procs")), Some(v) => { match crate::types::check::read::<AppProcList>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("procs"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("activity.read") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("computer") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computer"))), } }
                if let Some(v) = obj.get("thread") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("thread"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("activity.end") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("computer") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computer"))), } }
                match obj.get("pid") { None => out.push(crate::types::check::required::<JsInt>("pid")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } } }
                match obj.get("startedAt") { None => out.push(crate::types::check::required::<JsNumber>("startedAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedAt"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("daemon.send") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("channel") { None => out.push(crate::types::check::required::<String>("channel")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("channel"))), } } }
                match obj.get("frame") { None => out.push(crate::types::check::required::<DaemonFrame>("frame")), Some(v) => { match crate::types::check::read::<DaemonFrame>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("frame"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("daemon.close") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("channel") { None => out.push(crate::types::check::required::<String>("channel")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("channel"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.start") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("workspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if let Some(v) = obj.get("branch") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } }
                match obj.get("prompt") { None => out.push(crate::types::check::required::<String>("prompt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("prompt"))), } } }
                if let Some(v) = obj.get("harness") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harness"))), } }
                if let Some(v) = obj.get("thread") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("thread"))), } }
                if let Some(v) = obj.get("cwd") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cwd"))), } }
                if let Some(v) = obj.get("model") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } }
                if let Some(v) = obj.get("effort") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("effort"))), } }
                if let Some(v) = obj.get("permissionMode") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("permissionMode"))), } }
                if let Some(v) = obj.get("access") { match crate::types::check::read_present::<AccessChoice>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("access"))), } }
                if let Some(v) = obj.get("contextWindow") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("contextWindow"))), } }
                if let Some(v) = obj.get("fast") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fast"))), } }
                if let Some(v) = obj.get("startedBy") { match crate::types::check::read_present::<SessionOrigin>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedBy"))), } }
                if let Some(v) = obj.get("requestId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("requestId"))), } }
                if let Some(v) = obj.get("answerHeld") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("answerHeld"))), } }
                if let Some(v) = obj.get("followed") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("followed"))), } }
                if let Some(v) = obj.get("attempt") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("attempt"))), } }
                if let Some(v) = obj.get("notify") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("notify"))), } }
                if let Some(v) = obj.get("turnToken") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnToken"))), } }
                if let Some(v) = obj.get("title") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("title"))), } }
                if let Some(v) = obj.get("resume") { match crate::types::check::read_present::<ResumeAsk>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("resume"))), } }
                if let Some(v) = obj.get("replaces") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replaces"))), } }
                if let Some(v) = obj.get("fork") { match crate::types::check::read_present::<ForkSource>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fork"))), } }
                if let Some(v) = obj.get("attachments") { match crate::types::check::read_present::<Vec<Attachment>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("attachments"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("harnesses.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("workspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if let Some(v) = obj.get("computer") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("computer"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("workspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.history") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if let Some(v) = obj.get("before") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("before"))), } }
                if let Some(v) = obj.get("limit") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("limit"))), } }
                if let Some(v) = obj.get("whole") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("whole"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.head") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                if let Some(v) = obj.get("afterRead") { match crate::types::check::read_present::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("afterRead"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.attachment") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("requestId") { None => out.push(crate::types::check::required::<String>("requestId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("requestId"))), } } }
                match obj.get("index") { None => out.push(crate::types::check::required::<JsInt>("index")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("index"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.interrupt") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("sessionId") { None => out.push(crate::types::check::required::<String>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
                if let Some(v) = obj.get("task") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("task"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.steer") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("sessionId") { None => out.push(crate::types::check::required::<String>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
                match obj.get("prompt") { None => out.push(crate::types::check::required::<String>("prompt")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("prompt"))), } } }
                if let Some(v) = obj.get("requestId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("requestId"))), } }
                if let Some(v) = obj.get("attachments") { match crate::types::check::read_present::<Vec<Attachment>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("attachments"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.answer") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("sessionId") { None => out.push(crate::types::check::required::<String>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
                match obj.get("askId") { None => out.push(crate::types::check::required::<String>("askId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("askId"))), } } }
                match obj.get("optionId") { None => out.push(crate::types::check::required::<String>("optionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("optionId"))), } } }
                if let Some(v) = obj.get("reason") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reason"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.access") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("sessionId") { None => out.push(crate::types::check::required::<String>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
                match obj.get("permissionMode") { None => out.push(crate::types::check::required::<String>("permissionMode")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("permissionMode"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.rename") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("sessionId") { None => out.push(crate::types::check::required::<String>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
                match obj.get("title") { None => out.push(crate::types::check::required::<String>("title")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("title"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.forget") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                if let Some(v) = obj.get("check") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("check"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.delete") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                if let Some(v) = obj.get("processes") { match crate::types::check::read_present::<ProcessesAsk>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("processes"))), } }
                if let Some(v) = obj.get("check") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("check"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.read") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.replaceable") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.warm") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("workspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if let Some(v) = obj.get("cwd") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cwd"))), } }
                if let Some(v) = obj.get("harness") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harness"))), } }
                if let Some(v) = obj.get("model") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("model"))), } }
                if let Some(v) = obj.get("effort") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("effort"))), } }
                if let Some(v) = obj.get("permissionMode") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("permissionMode"))), } }
                if let Some(v) = obj.get("access") { match crate::types::check::read_present::<AccessChoice>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("access"))), } }
                if let Some(v) = obj.get("contextWindow") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("contextWindow"))), } }
                if let Some(v) = obj.get("fast") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fast"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.settle") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadIds") { None => out.push(crate::types::check::required::<Vec<String>>("threadIds")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadIds"))), } } }
                if let Some(v) = obj.get("finished") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("finished"))), } }
                if let Some(v) = obj.get("processes") { match crate::types::check::read_present::<ProcessesAsk>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("processes"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.mark") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadIds") { None => out.push(crate::types::check::required::<Vec<String>>("threadIds")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadIds"))), } } }
                match obj.get("marks") { None => out.push(crate::types::check::required::<ThreadMarks>("marks")), Some(v) => { match crate::types::check::read::<ThreadMarks>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("marks"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.restore") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadIds") { None => out.push(crate::types::check::required::<Vec<String>>("threadIds")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadIds"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.search") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("query") { None => out.push(crate::types::check::required::<String>("query")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("query"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.aside") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("sessionId") { None => out.push(crate::types::check::required::<String>("sessionId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sessionId"))), } } }
                match obj.get("question") { None => out.push(crate::types::check::required::<String>("question")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("question"))), } } }
                if let Some(v) = obj.get("askId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("askId"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.run") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("runId") { None => out.push(crate::types::check::required::<String>("runId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("runId"))), } } }
                match obj.get("block") { None => out.push(crate::types::check::required::<String>("block")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("block"))), } } }
                match obj.get("command") { None => out.push(crate::types::check::required::<String>("command")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("command"))), } } }
                match obj.get("state") { None => out.push(crate::types::check::required::<RunState>("state")), Some(v) => { match crate::types::check::read::<RunState>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
                if let Some(v) = obj.get("ptyId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ptyId"))), } }
                if let Some(v) = obj.get("exitCode") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exitCode"))), } }
                if let Some(v) = obj.get("signal") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signal"))), } }
                if let Some(v) = obj.get("output") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("output"))), } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("turnId") { None => out.push(crate::types::check::required::<String>("turnId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sessions.rewind") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                if let Some(v) = obj.get("turnId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } }
                if let Some(v) = obj.get("files") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } }
                if let Some(v) = obj.get("undo") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("undo"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("golden.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("capabilities.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("golden.prepare") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("kind") { match crate::types::check::read_present::<MachineKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("golden.seal") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("builderId") { None => out.push(crate::types::check::required::<String>("builderId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("builderId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("snapshots.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("snapshots.storage") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("cost.history") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("cost.spend") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("usage.used") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("range") { None => out.push(crate::types::check::required::<UsageRange>("range")), Some(v) => { match crate::types::check::read::<UsageRange>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("range"))), } } }
                match obj.get("split") { None => out.push(crate::types::check::required::<UsageSplit>("split")), Some(v) => { match crate::types::check::read::<UsageSplit>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("split"))), } } }
                if let Some(v) = obj.get("outside") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outside"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("usage.accounts") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("fresh") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fresh"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("usage.reset") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("account") { None => out.push(crate::types::check::required::<String>("account")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("account"))), } } }
                if let Some(v) = obj.get("creditId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("creditId"))), } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("places.readings") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("placeId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } }
                if let Some(v) = obj.get("workspaceId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } }
                match obj.get("range") { None => out.push(crate::types::check::required::<ReadingsRange>("range")), Some(v) => { match crate::types::check::read::<ReadingsRange>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("range"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("snapshots.rollback") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("version") { None => out.push(crate::types::check::required::<JsNumber>("version")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.portReach") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("port") { None => out.push(crate::types::check::required::<JsInt>("port")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.portProbe") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("port") { None => out.push(crate::types::check::required::<JsInt>("port")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.rebuild") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("forwards.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("forwards.stop") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("port") { None => out.push(crate::types::check::required::<RelayPort>("port")), Some(v) => { match crate::types::check::read::<RelayPort>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("workspaces.exec") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("argv") { None => out.push(crate::types::check::required::<Vec<String>>("argv")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("argv"))), } } }
                if let Some(v) = obj.get("cwd") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cwd"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("host.folders") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("dir") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dir"))), } }
                if let Some(v) = obj.get("hidden") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hidden"))), } }
                if let Some(v) = obj.get("repos") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("repos"))), } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("host.terminalConfig") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("scheme") { match crate::types::check::read_present::<TerminalScheme>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scheme"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("editor.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("editor.open") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("path") { None => out.push(crate::types::check::required::<String>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
                if let Some(v) = obj.get("line") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("line"))), } }
                if let Some(v) = obj.get("editor") { match crate::types::check::read_present::<EditorId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("editor"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("ssh.port") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("ssh.include") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.read") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("servers.tools") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("refresh") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("refresh"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("servers.icon") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("host") { None => out.push(crate::types::check::required::<String>("host")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("host"))), } } }
                if let Some(v) = obj.get("refresh") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("refresh"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.signIn") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                if let Some(v) = obj.get("terminal") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("terminal"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("servers.signIn") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("scope") { match crate::types::check::read_present::<McpScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.signInCode") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("signInId") { None => out.push(crate::types::check::required::<String>("signInId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signInId"))), } } }
                match obj.get("code") { None => out.push(crate::types::check::required::<String>("code")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("code"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.signIns") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.signInStop") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("signInId") { None => out.push(crate::types::check::required::<String>("signInId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signInId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.signInLine") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.key") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                match obj.get("key") { None => out.push(crate::types::check::required::<String>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
                if let Some(v) = obj.get("way") { match crate::types::check::read_present::<AgentsKeyWay>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("way"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.cloudSignIn") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<AgentsCloudSignInAgent>("agent")), Some(v) => { match crate::types::check::read::<AgentsCloudSignInAgent>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("providers.checkKey") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("provider") { None => out.push(crate::types::check::required::<String>("provider")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("provider"))), } } }
                match obj.get("key") { None => out.push(crate::types::check::required::<String>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("broker.status") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.addTools") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("agents.setup") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("placeId") { None => out.push(crate::types::check::required::<String>("placeId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("placeId"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if let Some(v) = obj.get("program") { match crate::types::check::read_double::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("program"))), } }
                if let Some(v) = obj.get("configDir") { match crate::types::check::read_double::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("configDir"))), } }
                if let Some(v) = obj.get("args") { match crate::types::check::read_double::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("args"))), } }
                if let Some(v) = obj.get("env") { match crate::types::check::read_present::<IndexMap<String, Option<String>>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("env"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("skills.search") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("q") { None => out.push(crate::types::check::required::<String>("q")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("q"))), } } }
                if let Some(v) = obj.get("limit") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("limit"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("skills.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("skill") { None => out.push(crate::types::check::required::<String>("skill")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("skill"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("skills.preview") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("skills.add") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("skill") { None => out.push(crate::types::check::required::<String>("skill")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("skill"))), } } }
                if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("skills.remove") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("skills.toggle") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                match obj.get("on") { None => out.push(crate::types::check::required::<bool>("on")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("servers.add") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if let Some(v) = obj.get("command") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("command"))), } }
                if let Some(v) = obj.get("args") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("args"))), } }
                if let Some(v) = obj.get("env") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("env"))), } }
                if let Some(v) = obj.get("url") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("url"))), } }
                if let Some(v) = obj.get("headers") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("headers"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("servers.remove") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("scope") { match crate::types::check::read_present::<McpScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("servers.toggle") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                match obj.get("name") { None => out.push(crate::types::check::required::<String>("name")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } } }
                if let Some(v) = obj.get("scope") { match crate::types::check::read_present::<McpScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } }
                match obj.get("on") { None => out.push(crate::types::check::required::<bool>("on")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("plugins.toggle") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("target") { None => out.push(crate::types::check::required::<AgentsTarget>("target")), Some(v) => { match crate::types::check::read::<AgentsTarget>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("target"))), } } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                match obj.get("plugin") { None => out.push(crate::types::check::required::<String>("plugin")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plugin"))), } } }
                match obj.get("on") { None => out.push(crate::types::check::required::<bool>("on")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.keys") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("provider") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("provider"))), } }
                if let Some(v) = obj.get("key") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } }
                if let Some(v) = obj.get("rows") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rows"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.start") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("road") { None => out.push(crate::types::check::required::<InitRoad>("road")), Some(v) => { match crate::types::check::read::<InitRoad>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("road"))), } } }
                if let Some(v) = obj.get("harness") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harness"))), } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.answer") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("screen") { None => out.push(crate::types::check::required::<InitScreenId>("screen")), Some(v) => { match crate::types::check::read::<InitScreenId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("screen"))), } } }
                if let Some(v) = obj.get("ticks") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ticks"))), } }
                if let Some(v) = obj.get("answers") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("answers"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.step") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("at") { None => out.push(crate::types::check::required::<JsInt>("at")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.draft") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("at") { None => out.push(crate::types::check::required::<String>("at")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("at"))), } } }
                if let Some(v) = obj.get("ticks") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ticks"))), } }
                if let Some(v) = obj.get("answers") { match crate::types::check::read_present::<IndexMap<String, String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("answers"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.retry") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("tool") { None => out.push(crate::types::check::required::<String>("tool")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tool"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.build") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("firstWorkspace") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("firstWorkspace"))), } }
                if let Some(v) = obj.get("importFolder") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("importFolder"))), } }
                if let Some(v) = obj.get("yes") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("yes"))), } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if let Some(v) = obj.get("rebuild") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rebuild"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.signInCode") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("tool") { None => out.push(crate::types::check::required::<String>("tool")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("tool"))), } } }
                match obj.get("code") { None => out.push(crate::types::check::required::<String>("code")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("code"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("init.cancel") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("preferences.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("preferences.set") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("patch") { None => out.push(crate::types::check::required::<PreferencesPatch>("patch")), Some(v) => { match crate::types::check::read::<PreferencesPatch>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("patch"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("release.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("release.check") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("force") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("force"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("host.restart") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projects.add") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("source") { None => out.push(crate::types::check::required::<String>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if let Some(v) = obj.get("base") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("base"))), } }
                if let Some(v) = obj.get("into") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("into"))), } }
                if let Some(v) = obj.get("seed") { match crate::types::check::read_present::<SeedChoice>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("seed"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("project.seed.plan") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("source") { None => out.push(crate::types::check::required::<String>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
                if let Some(v) = obj.get("on") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projects.seed") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("project") { None => out.push(crate::types::check::required::<String>("project")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } } }
                if let Some(v) = obj.get("keep") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("keep"))), } }
                if let Some(v) = obj.get("cut") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cut"))), } }
                if let Some(v) = obj.get("none") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("none"))), } }
                if let Some(v) = obj.get("save") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("save"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projects.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("conversations.list") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                if let Some(v) = obj.get("agent") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("conversations.read") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("project") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("project"))), } }
                match obj.get("agent") { None => out.push(crate::types::check::required::<String>("agent")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agent"))), } } }
                match obj.get("conversation") { None => out.push(crate::types::check::required::<String>("conversation")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("conversation"))), } } }
                if let Some(v) = obj.get("preview") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("preview"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projects.defaults") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projects.resolve") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("ref") { None => out.push(crate::types::check::required::<String>("ref")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ref"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projects.branch") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("projectId") { None => out.push(crate::types::check::required::<String>("projectId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projectId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projects.remove") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("projectId") { None => out.push(crate::types::check::required::<String>("projectId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projectId"))), } } }
                if let Some(v) = obj.get("force") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("force"))), } }
                if let Some(v) = obj.get("check") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("check"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projects.icon") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("projectId") { None => out.push(crate::types::check::required::<String>("projectId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("projectId"))), } } }
                match obj.get("png") { None => out.push(crate::types::check::required::<Option<String>>("png")), Some(v) => { match crate::types::check::read::<Option<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("png"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("projects.icons") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("hashes") { None => out.push(crate::types::check::required::<Vec<ProjectIconHash>>("hashes")), Some(v) => { match crate::types::check::read::<Vec<ProjectIconHash>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hashes"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("project.plan") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("source") { None => out.push(crate::types::check::required::<String>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("project.import") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("source") { None => out.push(crate::types::check::required::<String>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
                match obj.get("dest") { None => out.push(crate::types::check::required::<String>("dest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dest"))), } } }
                if let Some(v) = obj.get("replace") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replace"))), } }
                if let Some(v) = obj.get("carry") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("carry"))), } }
                if let Some(v) = obj.get("rewrite") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rewrite"))), } }
                if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("project.export") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("workspaceId") { None => out.push(crate::types::check::required::<String>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
                match obj.get("source") { None => out.push(crate::types::check::required::<String>("source")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("source"))), } } }
                match obj.get("dest") { None => out.push(crate::types::check::required::<String>("dest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dest"))), } } }
                if let Some(v) = obj.get("replace") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("replace"))), } }
                if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("image.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("image.build") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("place") { None => out.push(crate::types::check::required::<String>("place")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("place"))), } } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if let Some(v) = obj.get("force") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("force"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("image.export") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("dest") { None => out.push(crate::types::check::required::<String>("dest")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("dest"))), } } }
                match obj.get("passphrase") { None => out.push(crate::types::check::required::<String>("passphrase")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("passphrase"))), } } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.get") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.write") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if let Some(v) = obj.get("turnToken") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnToken"))), } }
                if let Some(v) = obj.get("text") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } }
                if let Some(v) = obj.get("document") { match crate::types::check::read_present::<IndexMap<String, Value>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("document"))), } }
                if let Some(v) = obj.get("check") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("check"))), } }
                if let Some(v) = obj.get("ifVersion") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ifVersion"))), } }
                if let Some(v) = obj.get("values") { match crate::types::check::read_present::<SlateWireValues>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("values"))), } }
                if let Some(v) = obj.get("press") { match crate::types::check::read_present::<SlatesWritePress>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("press"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.state") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if let Some(v) = obj.get("turnToken") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnToken"))), } }
                if let Some(v) = obj.get("values") { match crate::types::check::read_present::<SlateWireValues>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("values"))), } }
                if let Some(v) = obj.get("start") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("start"))), } }
                if let Some(v) = obj.get("ifVersion") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ifVersion"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.read") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if let Some(v) = obj.get("turnToken") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnToken"))), } }
                if let Some(v) = obj.get("values") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("values"))), } }
                if let Some(v) = obj.get("text") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } }
                if let Some(v) = obj.get("sketch") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sketch"))), } }
                if let Some(v) = obj.get("document") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("document"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.catalog") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                if let Some(v) = obj.get("name") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("name"))), } }
                if let Some(v) = obj.get("threadId") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } }
                if let Some(v) = obj.get("turnToken") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnToken"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.event") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("version") { None => out.push(crate::types::check::required::<JsInt>("version")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("version"))), } } }
                match obj.get("piece") { None => out.push(crate::types::check::required::<String>("piece")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("piece"))), } } }
                match obj.get("event") { None => out.push(crate::types::check::required::<SlatesEventEvent>("event")), Some(v) => { match crate::types::check::read::<SlatesEventEvent>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("event"))), } } }
                match obj.get("requestId") { None => out.push(crate::types::check::required::<String>("requestId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("requestId"))), } } }
                if let Some(v) = obj.get("scope") { match crate::types::check::read_present::<SlatesEventScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } }
                if let Some(v) = obj.get("rowAction") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rowAction"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.approve") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("key") { None => out.push(crate::types::check::required::<String>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
                match obj.get("scope") { None => out.push(crate::types::check::required::<SlatesApproveScope>("scope")), Some(v) => { match crate::types::check::read::<SlatesApproveScope>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("scope"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.cancel") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("run") { None => out.push(crate::types::check::required::<String>("run")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("run"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.revoke") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("key") { None => out.push(crate::types::check::required::<String>("key")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("key"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.shown") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.subscribe") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("sources") { None => out.push(crate::types::check::required::<Vec<String>>("sources")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sources"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.unsubscribe") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("sources") { None => out.push(crate::types::check::required::<Vec<String>>("sources")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("sources"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.resolve") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("paths") { None => out.push(crate::types::check::required::<Vec<String>>("paths")), Some(v) => { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("paths"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("slates.image") => {
                let mut out = Vec::new();
                match obj.get("id") { None => out.push(crate::types::check::required::<ReqId>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
                match obj.get("threadId") { None => out.push(crate::types::check::required::<String>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
                match obj.get("src") { None => out.push(crate::types::check::required::<String>("src")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("src"))), } } }
                if let Some(v) = obj.get("have") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("have"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("op", tag)]),
        }
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `maxMachines`: int min 0
/// - `maxDepth`: int min 1
impl Check for PlacesSetSpawn {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.max_machines {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("maxMachines"))?;
        }
        if let Some(v) = self.max_depth {
            num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("maxDepth"))?;
        }
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("spawn") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawn"))), } }
        if let Some(v) = obj.get("maxMachines") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_max_machines(&x) { out.push(e.under("maxMachines")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("maxMachines"))), } }
        if let Some(v) = obj.get("maxDepth") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_max_depth(&x) { out.push(e.under("maxDepth")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("maxDepth"))), } }
        Some(out)
    }

}
impl PlacesSetSpawn {
    fn rule_max_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_max_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 1.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlacesSetSize {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("cpu") { None => out.push(crate::types::check::missing::<Self>("cpu")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cpu"))), } } }
        match obj.get("memMb") { None => out.push(crate::types::check::missing::<Self>("memMb")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("memMb"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `maxMachines`: int min 0
/// - `maxDepth`: int min 1
impl Check for WorkspacesCreateAgents {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.max_machines {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("maxMachines"))?;
        }
        if let Some(v) = self.max_depth {
            num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("maxDepth"))?;
        }
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("spawn") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("spawn"))), } }
        if let Some(v) = obj.get("maxMachines") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_max_machines(&x) { out.push(e.under("maxMachines")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("maxMachines"))), } }
        if let Some(v) = obj.get("maxDepth") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_max_depth(&x) { out.push(e.under("maxDepth")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("maxDepth"))), } }
        Some(out)
    }

}
impl WorkspacesCreateAgents {
    fn rule_max_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_max_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 1.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspacesCreateSub {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("on") { None => out.push(crate::types::check::missing::<Self>("on")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } } }
        if let Some(v) = obj.get("size") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } }
        if let Some(v) = obj.get("from") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("from"))), } }
        if let Some(v) = obj.get("branch") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } }
        if let Some(v) = obj.get("harness") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("harness"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `cpu`: int min 0 exclusive
/// - `memMb`: int min 0 exclusive
impl Check for WorkspacesResizeSize {
    fn check(&self) -> Result<(), SchemaError> {
        num_above(self.cpu.as_f64(), 0.0).map_err(|e| e.under("cpu"))?;
        num_above(self.mem_mb.as_f64(), 0.0).map_err(|e| e.under("memMb"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("cpu") { None => out.push(crate::types::check::missing::<Self>("cpu")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_cpu(&x) { out.push(e.under("cpu")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("cpu"))), } } }
        match obj.get("memMb") { None => out.push(crate::types::check::missing::<Self>("memMb")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_mem_mb(&x) { out.push(e.under("memMb")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("memMb"))), } } }
        Some(out)
    }

}
impl WorkspacesResizeSize {
    fn rule_cpu(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_above((*x).as_f64(), 0.0)?;
        Ok(())
    }
    fn rule_mem_mb(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_above((*x).as_f64(), 0.0)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspacesReviewDraftOnItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("on") { None => out.push(crate::types::check::missing::<Self>("on")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("on"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `index`: int min 0
/// - `action`: int min 0
impl Check for SlatesWritePress {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.index {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("index"))?;
        }
        if let Some(v) = self.action {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("action"))?;
        }
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("piece") { None => out.push(crate::types::check::missing::<Self>("piece")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("piece"))), } } }
        if let Some(v) = obj.get("index") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_index(&x) { out.push(e.under("index")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("index"))), } }
        if let Some(v) = obj.get("action") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_action(&x) { out.push(e.under("action")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("action"))), } }
        Some(out)
    }

}
impl SlatesWritePress {
    fn rule_index(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_action(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `index`: int min 0
impl Check for SlatesEventScope {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.index.as_f64(), 0.0).map_err(|e| e.under("index"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("item") { None => out.push(crate::types::check::missing::<Self>("item")), Some(v) => { match crate::types::check::read::<Value>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("item"))), } } }
        match obj.get("index") { None => out.push(crate::types::check::missing::<Self>("index")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_index(&x) { out.push(e.under("index")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("index"))), } } }
        Some(out)
    }

}
impl SlatesEventScope {
    fn rule_index(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
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

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let mut out = crate::types::check::issues_of::<RuntimeOp>(value);
        let serde_json::Value::Object(obj) = value else {
            return Some(out);
        };
        if out.is_empty() {
            if let Some(named) = obj.get("op").and_then(serde_json::Value::as_str).and_then(super::runtime_ops::strict_keys) {
                let unknown: Vec<String> = obj.keys().filter(|k| !named.contains(&k.as_str())).cloned().collect();
                if !unknown.is_empty() {
                    out.push(crate::types::check::unrecognized(Vec::new(), unknown));
                }
            }
        }
        if let Some(origin) = obj.get("origin") {
            if let Err(es) = crate::types::check::read_present::<crate::views::workspace::WorkspaceOrigin>(origin) {
                out.extend(es.into_iter().map(|e| e.under("origin")));
            }
        }
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ExecEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::tag_describe(path, "type", &["exec.output", "exec.exit"])
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("type") else { return Some(vec![crate::types::check::missing::<Self>("type")]); };
        match tag.as_str() {
            Some("exec.output") => {
                let mut out = Vec::new();
                match obj.get("execId") { None => out.push(crate::types::check::required::<String>("execId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("execId"))), } } }
                match obj.get("text") { None => out.push(crate::types::check::required::<String>("text")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("exec.exit") => {
                let mut out = Vec::new();
                match obj.get("execId") { None => out.push(crate::types::check::required::<String>("execId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("execId"))), } } }
                match obj.get("exitCode") { None => out.push(crate::types::check::required::<Option<JsInt>>("exitCode")), Some(v) => { match crate::types::check::read::<Option<JsInt>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exitCode"))), } } }
                if let Some(v) = obj.get("error") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("error"))), } }
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("type", tag)]),
        }
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeOkResponse {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<Option<ReqId>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("ok") { None => out.push(crate::types::check::missing::<Self>("ok")), Some(v) => { match crate::types::check::read::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ok"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeErrorResponse {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<Option<ReqId>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("ok") { None => out.push(crate::types::check::missing::<Self>("ok")), Some(v) => { match crate::types::check::read::<False>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ok"))), } } }
        match obj.get("error") { None => out.push(crate::types::check::missing::<Self>("error")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("error"))), } } }
        if let Some(v) = obj.get("kind") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } }
        if let Some(v) = obj.get("fix") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("fix"))), } }
        Some(out)
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
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![crate::types::check::issues_of::<RuntimeOkResponse>(value), crate::types::check::issues_of::<RuntimeErrorResponse>(value)]))
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionInterruptResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("outcome") { None => out.push(crate::types::check::missing::<Self>("outcome")), Some(v) => { match crate::types::check::read::<SessionInterruptOutcome>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outcome"))), } } }
        if let Some(v) = obj.get("under") { match crate::types::check::read_present::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("under"))), } }
        if let Some(v) = obj.get("error") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("error"))), } }
        if let Some(v) = obj.get("left") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("left"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSteerResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("outcome") { None => out.push(crate::types::check::missing::<Self>("outcome")), Some(v) => { match crate::types::check::read::<SessionSteerOutcome>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outcome"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAccessResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("outcome") { None => out.push(crate::types::check::missing::<Self>("outcome")), Some(v) => { match crate::types::check::read::<SessionAccessOutcome>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outcome"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAnswerResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("outcome") { None => out.push(crate::types::check::missing::<Self>("outcome")), Some(v) => { match crate::types::check::read::<SessionAnswerOutcome>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outcome"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRenameResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("outcome") { None => out.push(crate::types::check::missing::<Self>("outcome")), Some(v) => { match crate::types::check::read::<SessionRenameOutcome>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outcome"))), } } }
        if let Some(v) = obj.get("error") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("error"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SettledThread {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        match obj.get("title") { None => out.push(crate::types::check::missing::<Self>("title")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("title"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSettleResult {
    fn check(&self) -> Result<(), SchemaError> {
        self.settled.check().map_err(|e| e.under("settled"))?;
        self.ended.check().map_err(|e| e.under("ended"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("settled") { None => out.push(crate::types::check::missing::<Self>("settled")), Some(v) => { match crate::types::check::read::<Vec<SettledThread>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("settled"))), } } }
        match obj.get("left") { None => out.push(crate::types::check::missing::<Self>("left")), Some(v) => { match crate::types::check::read::<Vec<SessionSettleResultLeftItem>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("left"))), } } }
        if let Some(v) = obj.get("ended") { match crate::types::check::read_present::<Vec<ActivityEnded>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ended"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSettleResultLeftItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        match obj.get("why") { None => out.push(crate::types::check::missing::<Self>("why")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("why"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRestoreResult {
    fn check(&self) -> Result<(), SchemaError> {
        self.restored.check().map_err(|e| e.under("restored"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("restored") { None => out.push(crate::types::check::missing::<Self>("restored")), Some(v) => { match crate::types::check::read::<Vec<SettledThread>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("restored"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSearchHit {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("workspaceId") { None => out.push(crate::types::check::missing::<Self>("workspaceId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspaceId"))), } } }
        match obj.get("threadId") { None => out.push(crate::types::check::missing::<Self>("threadId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threadId"))), } } }
        match obj.get("snippet") { None => out.push(crate::types::check::missing::<Self>("snippet")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("snippet"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `unsearched`: int min 0 exclusive
impl Check for SessionSearchResult {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.unsearched {
            num_above(v.as_f64(), 0.0).map_err(|e| e.under("unsearched"))?;
        }
        self.hits.check().map_err(|e| e.under("hits"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("hits") { None => out.push(crate::types::check::missing::<Self>("hits")), Some(v) => { match crate::types::check::read::<Vec<SessionSearchHit>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("hits"))), } } }
        if let Some(v) = obj.get("unsearched") { match crate::types::check::read_present::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_unsearched(&x) { out.push(e.under("unsearched")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("unsearched"))), } }
        Some(out)
    }

}
impl SessionSearchResult {
    fn rule_unsearched(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = (*x) {
            num_above(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAsideResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("text") { None => out.push(crate::types::check::missing::<Self>("text")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRewindResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("turns") { None => out.push(crate::types::check::missing::<Self>("turns")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turns"))), } } }
        if let Some(v) = obj.get("files") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } }
        if let Some(v) = obj.get("kept") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kept"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionStartResult {
    fn check(&self) -> Result<(), SchemaError> {
        self.session.check().map_err(|e| e.under("session"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("session") { None => out.push(crate::types::check::missing::<Self>("session")), Some(v) => { match crate::types::check::read::<SessionView>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("session"))), } } }
        match obj.get("outcome") { None => out.push(crate::types::check::missing::<Self>("outcome")), Some(v) => { match crate::types::check::read::<SessionStartOutcome>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("outcome"))), } } }
        match obj.get("turnId") { None => out.push(crate::types::check::missing::<Self>("turnId")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("turnId"))), } } }
        if let Some(v) = obj.get("notice") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("notice"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionWarmResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("warm") { None => out.push(crate::types::check::missing::<Self>("warm")), Some(v) => { match crate::types::check::read::<SessionWarmResultWarm>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("warm"))), } } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `running`: int min 0
/// - `limit`: int min 0
impl Check for ProviderKeyWorks {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.running.as_f64(), 0.0).map_err(|e| e.under("running"))?;
        num_at_least(self.limit.as_f64(), 0.0).map_err(|e| e.under("limit"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("works") { None => out.push(crate::types::check::missing::<Self>("works")), Some(v) => { match crate::types::check::read::<True>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("works"))), } } }
        match obj.get("account") { None => out.push(crate::types::check::missing::<Self>("account")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("account"))), } } }
        match obj.get("plan") { None => out.push(crate::types::check::missing::<Self>("plan")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("plan"))), } } }
        match obj.get("running") { None => out.push(crate::types::check::missing::<Self>("running")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_running(&x) { out.push(e.under("running")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("running"))), } } }
        match obj.get("limit") { None => out.push(crate::types::check::missing::<Self>("limit")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(x) => { if let Err(e) = Self::rule_limit(&x) { out.push(e.under("limit")); } }, Err(es) => out.extend(es.into_iter().map(|e| e.under("limit"))), } } }
        Some(out)
    }

}
impl ProviderKeyWorks {
    fn rule_running(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
    fn rule_limit(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for BrokerStatus {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("claude") { match crate::types::check::read_present::<BrokerStatusClaude>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("claude"))), } }
        if let Some(v) = obj.get("codex") { match crate::types::check::read_present::<BrokerStatusCodex>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("codex"))), } }
        if let Some(v) = obj.get("github") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("github"))), } }
        Some(out)
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
