// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/runtime.ts.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
use indexmap::IndexMap;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use super::runtime_ops::{dbl_rule, env_key, env_name_issue, opt_rule};
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
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub item: Option<Option<crate::types::kept::Kept>>,
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
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOkResponse {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub id: Option<ReqId>,
    pub ok: True,
    /// zod passthrough: unknown keys kept, in the order read.
    #[serde(flatten, deserialize_with = "crate::types::kept::record")]
    pub rest: IndexMap<String, Value>,
}

impl Serialize for RuntimeOkResponse {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut entries = vec![
            ("id".to_string(), serde_json::to_value(&self.id).map_err(serde::ser::Error::custom)?),
            ("ok".to_string(), Value::Bool(true)),
        ];
        entries.extend(self.rest.iter().map(|(k, v)| (k.clone(), v.clone())));
        crate::types::kept::serialize_object(&entries, s)
    }
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
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum RuntimeResponse {
    V0(RuntimeOkResponse),
    V1(RuntimeErrorResponse),
}

crate::zod_union!(RuntimeResponse { V0(RuntimeOkResponse), V1(RuntimeErrorResponse) });
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
        Some(key) => Err(env_name_issue(key)),
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
        let Some(tag) = obj.get("op") else {
            return Some(vec![crate::types::check::missing::<Self>("op")]);
        };
        match tag.as_str() {
            Some("auth") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "token", &mut out);
                Some(out)
            }
            Some("ticket.issue") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<TicketPurpose>(obj, "purpose", &mut out);
                Some(out)
            }
            Some("pair.issue") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<True>(obj, "here", &mut out);
                Some(out)
            }
            Some("pair.redeem") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "code", &mut out, |x| text_at_most(x, 64));
                crate::types::keys::req_key_with::<String>(obj, "name", &mut out, |x| text_at_most(x, 200));
                Some(out)
            }
            Some("seal.open") => {
                let mut body = obj.clone();
                body.shift_remove("op");
                Some(crate::types::check::issues_of::<SealOpenRequest>(&serde_json::Value::Object(body)))
            }
            Some("device.auth") => {
                let mut body = obj.clone();
                body.shift_remove("op");
                Some(crate::types::check::issues_of::<DeviceAuthRequest>(&serde_json::Value::Object(body)))
            }
            Some("account.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("devices.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("devices.revoke") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "deviceId", &mut out);
                Some(out)
            }
            Some("place.join") => {
                let mut body = obj.clone();
                body.shift_remove("op");
                Some(crate::types::check::issues_of::<PlaceJoinRequest>(&serde_json::Value::Object(body)))
            }
            Some("place.auth") => {
                let mut body = obj.clone();
                body.shift_remove("op");
                Some(crate::types::check::issues_of::<PlaceAuthRequest>(&serde_json::Value::Object(body)))
            }
            Some("place.prove") => {
                let mut body = obj.clone();
                body.shift_remove("op");
                Some(crate::types::check::issues_of::<PlaceProveRequest>(&serde_json::Value::Object(body)))
            }
            Some("places.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("places.update") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::opt_key::<SudoPassword>(obj, "sudoPassword", &mut out);
                Some(out)
            }
            Some("places.holds") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                Some(out)
            }
            Some("places.remove") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::opt_key::<SudoPassword>(obj, "sudoPassword", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "forget", &mut out);
                Some(out)
            }
            Some("places.doctor") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "doctorId", &mut out, |x| text_at_most(x, 64));
                crate::types::keys::opt_key_with::<String>(obj, "project", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
                Some(out)
            }
            Some("places.dial") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                Some(out)
            }
            Some("places.set") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::opt_key::<Vec<PlaceSettingWord>>(obj, "reset", &mut out);
                crate::types::keys::opt_key_with::<String>(obj, "name", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
                crate::types::keys::opt_key_with::<String>(obj, "ssh", &mut out, |x| opt_rule(x, |v| text_at_most(v, 300)));
                crate::types::keys::opt_key_with::<String>(obj, "recipe", &mut out, |x| {
                    opt_rule(x, |v| text_at_least(v, 1).and_then(|()| text_at_most(v, 200)))
                });
                crate::types::keys::opt_key_with::<JsInt>(obj, "threads", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 1.0)));
                crate::types::keys::opt_key_with::<JsInt>(obj, "machines", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 1.0)));
                crate::types::keys::opt_key_with::<JsNumber>(obj, "spendPerDayUsd", &mut out, |x| opt_rule(x, |v| num_at_least(v.0, 0.0)));
                crate::types::keys::dbl_key_with::<JsInt>(obj, "napMs", &mut out, |x| {
                    dbl_rule(x, |v| num_at_least(v.as_f64(), 60000.0).and_then(|()| num_at_most(v.as_f64(), 10800000.0)))
                });
                crate::types::keys::dbl_key_with::<JsInt>(obj, "turnLimitMs", &mut out, |x| {
                    dbl_rule(x, |v| num_at_least(v.as_f64(), 3600000.0).and_then(|()| num_at_most(v.as_f64(), 86400000.0)))
                });
                crate::types::keys::opt_key::<PlacesSetSpawn>(obj, "spawn", &mut out);
                crate::types::keys::opt_key::<PlacesSetSize>(obj, "size", &mut out);
                crate::types::keys::opt_key_with::<JsInt>(obj, "deleteAfterMs", &mut out, |x| {
                    opt_rule(x, |v| num_at_least(v.as_f64(), 0.0).and_then(|()| num_at_most(v.as_f64(), 86400000.0)))
                });
                Some(out)
            }
            Some("places.follow") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "recipe", &mut out, |x| {
                    text_at_least(x, 1).and_then(|()| text_at_most(x, 200))
                });
                Some(out)
            }
            Some("places.skip") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "row", &mut out, |x| {
                    text_at_least(x, 1).and_then(|()| text_at_most(x, 300))
                });
                Some(out)
            }
            Some("places.setupLog") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::opt_key::<PlaceSetupStep>(obj, "step", &mut out);
                Some(out)
            }
            Some("places.estimate") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "ref", &mut out, |x| text_at_most(x, 200));
                crate::types::keys::req_key::<RecipeFile>(obj, "choices", &mut out);
                Some(out)
            }
            Some("places.loginLanded") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                Some(out)
            }
            Some("places.door") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("places.mint") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("places.sshHosts") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("places.add") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key_with::<String>(obj, "addId", &mut out, |x| opt_rule(x, |v| text_at_most(v, 64)));
                crate::types::keys::req_key_with::<String>(obj, "address", &mut out, |x| text_at_most(x, 200));
                crate::types::keys::opt_key_with::<String>(obj, "name", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
                crate::types::keys::opt_key_with::<JsInt>(obj, "sshPort", &mut out, |x| {
                    opt_rule(x, |v| num_at_least(v.as_f64(), 1.0).and_then(|()| num_at_most(v.as_f64(), 65535.0)))
                });
                crate::types::keys::opt_key_with::<String>(obj, "keyPath", &mut out, |x| opt_rule(x, |v| text_at_most(v, 1024)));
                crate::types::keys::opt_key_with::<String>(obj, "hostKey", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
                crate::types::keys::opt_key::<SudoPassword>(obj, "sudoPassword", &mut out);
                crate::types::keys::opt_key_with::<String>(obj, "recipe", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
                Some(out)
            }
            Some("places.setup") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "ref", &mut out, |x| text_at_most(x, 200));
                crate::types::keys::opt_key_with::<String>(obj, "recipe", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
                crate::types::keys::opt_key::<RecipeFile>(obj, "choices", &mut out);
                crate::types::keys::opt_key_with::<String>(obj, "addId", &mut out, |x| opt_rule(x, |v| text_at_most(v, 64)));
                crate::types::keys::opt_key_with::<String>(obj, "row", &mut out, |x| opt_rule(x, |v| text_at_most(v, 400)));
                Some(out)
            }
            Some("places.choose") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "ref", &mut out, |x| text_at_most(x, 200));
                crate::types::keys::req_key::<RecipeFile>(obj, "choices", &mut out);
                crate::types::keys::opt_key_with::<String>(obj, "recipe", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
                Some(out)
            }
            Some("recipes.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("recipes.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "name", &mut out, |x| text_at_most(x, 200));
                Some(out)
            }
            Some("recipes.save") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "name", &mut out, |x| text_at_most(x, 200));
                crate::types::keys::any_key::<Value>(obj, "file", &mut out);
                crate::types::keys::opt_key_with::<String>(obj, "from", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
                Some(out)
            }
            Some("recipes.remove") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "name", &mut out, |x| text_at_most(x, 200));
                crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
                Some(out)
            }
            Some("recipes.options") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("events.subscribe") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key_with::<JsInt>(obj, "after", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 0.0)));
                crate::types::keys::opt_key::<String>(obj, "stream", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "creates", &mut out);
                Some(out)
            }
            Some("status.subscribe") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("status.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("workspaces.create") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "golden", &mut out);
                crate::types::keys::req_key::<String>(obj, "project", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "cpu", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "memMb", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "envs", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "labels", &mut out);
                crate::types::keys::opt_key::<WorkspacesCreateAgents>(obj, "agents", &mut out);
                crate::types::keys::dbl_key::<JsNumber>(obj, "idleWindowMs", &mut out);
                crate::types::keys::opt_key::<String>(obj, "parent", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "engine", &mut out);
                crate::types::keys::opt_key::<WorkspacesCreateSub>(obj, "sub", &mut out);
                Some(out)
            }
            Some("workspaces.landing") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "project", &mut out);
                crate::types::keys::opt_key::<True>(obj, "sized", &mut out);
                Some(out)
            }
            Some("workspaces.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("workspaces.resolve") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ref", &mut out);
                Some(out)
            }
            Some("workspaces.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("workspaces.nap") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("workspaces.resize") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<WorkspacesResizeSize>(obj, "size", &mut out);
                Some(out)
            }
            Some("workspaces.wake") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("workspaces.restartDaemon") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("workspaces.stopWake") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("workspaces.upgrade") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("workspaces.rename") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                Some(out)
            }
            Some("workspaces.look") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::dbl_key::<WorkspaceTheme>(obj, "theme", &mut out);
                crate::types::keys::dbl_key::<WorkspaceGlyph>(obj, "glyph", &mut out);
                Some(out)
            }
            Some("workspaces.bringBack") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "title", &mut out);
                crate::types::keys::opt_key::<String>(obj, "body", &mut out);
                Some(out)
            }
            Some("worktree.make") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "project", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                Some(out)
            }
            Some("folder.make") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "project", &mut out);
                Some(out)
            }
            Some("worktree.remove") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "project", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
                Some(out)
            }
            Some("workspaces.checkout") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "fresh", &mut out);
                Some(out)
            }
            Some("workspaces.discard") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("workspaces.commit") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "message", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "paths", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("workspaces.commitDraft") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "paths", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("workspaces.viewed") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "path", &mut out);
                crate::types::keys::dbl_key::<String>(obj, "blob", &mut out);
                Some(out)
            }
            Some("workspaces.pullRequestView") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "fresh", &mut out);
                Some(out)
            }
            Some("workspaces.pullRequestDiff") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("workspaces.pullRequestSend") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key_with::<Vec<PullRequestItem>>(obj, "items", &mut out, |x| items_at_least(x.len(), 1));
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("workspaces.pullRequestReply") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key_with::<JsInt>(obj, "replyTo", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 0.0)));
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "body", &mut out, |x| text_at_most(x, 65536));
                Some(out)
            }
            Some("workspaces.pullRequestResolve") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<bool>(obj, "resolved", &mut out);
                Some(out)
            }
            Some("workspaces.pullRequestReact") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "subject", &mut out);
                crate::types::keys::req_key::<ReactionContent>(obj, "content", &mut out);
                crate::types::keys::req_key::<bool>(obj, "on", &mut out);
                Some(out)
            }
            Some("workspaces.fix") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "check", &mut out);
                crate::types::keys::opt_key::<String>(obj, "child", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "childThreadId", &mut out);
                Some(out)
            }
            Some("workspaces.merge") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<MergeMethod>(obj, "method", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "whenChecksPass", &mut out);
                crate::types::keys::opt_key_with::<String>(obj, "head", &mut out, |x| opt_rule(x, |v| text_at_least(v, 1)));
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("workspaces.start") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "url", &mut out);
                crate::types::keys::opt_key::<String>(obj, "project", &mut out);
                crate::types::keys::opt_key::<String>(obj, "agent", &mut out);
                crate::types::keys::opt_key::<String>(obj, "model", &mut out);
                crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
                crate::types::keys::opt_key::<AccessChoice>(obj, "access", &mut out);
                Some(out)
            }
            Some("workspaces.review") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "url", &mut out);
                crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "agent", &mut out);
                crate::types::keys::opt_key::<String>(obj, "model", &mut out);
                crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
                Some(out)
            }
            Some("workspaces.reviewDraft") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "summary", &mut out);
                crate::types::keys::opt_key::<WorkspacesReviewDraftVerdict>(obj, "verdict", &mut out);
                crate::types::keys::opt_key::<Vec<WorkspacesReviewDraftOnItem>>(obj, "on", &mut out);
                Some(out)
            }
            Some("workspaces.reviewPost") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("workspaces.update") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("workspaces.mergeIn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "child", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "childThreadId", &mut out);
                Some(out)
            }
            Some("workspaces.delete") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("workspaces.forget") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("workspaces.snapshot") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("projectGoldens.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("projectGoldens.remove") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "snapshotId", &mut out);
                Some(out)
            }
            Some("workspaces.touch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("daemon.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
                Some(out)
            }
            Some("sys.subscribe") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
                Some(out)
            }
            Some("sys.unsubscribe") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
                Some(out)
            }
            Some("appProcs.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("appProcs.unwatch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("appProcs.offer") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("appProcs.set") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AppProcList>(obj, "procs", &mut out);
                Some(out)
            }
            Some("activity.read") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "computer", &mut out);
                crate::types::keys::opt_key::<String>(obj, "thread", &mut out);
                Some(out)
            }
            Some("activity.end") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "computer", &mut out);
                crate::types::keys::req_key_with::<JsInt>(obj, "pid", &mut out, |x| num_above(x.as_f64(), 0.0));
                crate::types::keys::req_key::<JsNumber>(obj, "startedAt", &mut out);
                Some(out)
            }
            Some("daemon.send") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "channel", &mut out);
                crate::types::keys::req_key::<DaemonFrame>(obj, "frame", &mut out);
                Some(out)
            }
            Some("daemon.close") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "channel", &mut out);
                Some(out)
            }
            Some("sessions.start") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "project", &mut out);
                crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
                crate::types::keys::req_key::<String>(obj, "prompt", &mut out);
                crate::types::keys::opt_key::<String>(obj, "harness", &mut out);
                crate::types::keys::opt_key::<String>(obj, "thread", &mut out);
                crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "model", &mut out);
                crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
                crate::types::keys::opt_key::<String>(obj, "permissionMode", &mut out);
                crate::types::keys::opt_key::<AccessChoice>(obj, "access", &mut out);
                crate::types::keys::opt_key::<String>(obj, "contextWindow", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "fast", &mut out);
                crate::types::keys::opt_key::<SessionOrigin>(obj, "startedBy", &mut out);
                crate::types::keys::opt_key::<String>(obj, "requestId", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "answerHeld", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "followed", &mut out);
                crate::types::keys::opt_key::<String>(obj, "attempt", &mut out);
                crate::types::keys::opt_key_with::<Vec<String>>(obj, "notify", &mut out, |x| opt_rule(x, |v| items_at_least(v.len(), 1)));
                crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
                crate::types::keys::opt_key::<String>(obj, "title", &mut out);
                crate::types::keys::opt_key::<ResumeAsk>(obj, "resume", &mut out);
                crate::types::keys::opt_key::<String>(obj, "replaces", &mut out);
                crate::types::keys::opt_key::<ForkSource>(obj, "fork", &mut out);
                crate::types::keys::opt_key::<Vec<Attachment>>(obj, "attachments", &mut out);
                Some(out)
            }
            Some("harnesses.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "project", &mut out);
                crate::types::keys::opt_key::<String>(obj, "computer", &mut out);
                Some(out)
            }
            Some("sessions.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("sessions.history") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key_with::<JsInt>(obj, "before", &mut out, |x| opt_rule(x, |v| num_above(v.as_f64(), 0.0)));
                crate::types::keys::opt_key_with::<JsInt>(obj, "limit", &mut out, |x| {
                    opt_rule(x, |v| num_above(v.as_f64(), 0.0).and_then(|()| num_at_most(v.as_f64(), 1000.0)))
                });
                crate::types::keys::opt_key::<True>(obj, "whole", &mut out);
                Some(out)
            }
            Some("sessions.head") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<True>(obj, "afterRead", &mut out);
                Some(out)
            }
            Some("sessions.attachment") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<String>(obj, "requestId", &mut out);
                crate::types::keys::req_key_with::<JsInt>(obj, "index", &mut out, |x| num_at_least(x.as_f64(), 0.0));
                Some(out)
            }
            Some("sessions.interrupt") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "task", &mut out);
                Some(out)
            }
            Some("sessions.steer") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
                crate::types::keys::req_key::<String>(obj, "prompt", &mut out);
                crate::types::keys::opt_key::<String>(obj, "requestId", &mut out);
                crate::types::keys::opt_key::<Vec<Attachment>>(obj, "attachments", &mut out);
                Some(out)
            }
            Some("sessions.answer") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
                crate::types::keys::req_key::<String>(obj, "askId", &mut out);
                crate::types::keys::req_key::<String>(obj, "optionId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "reason", &mut out);
                Some(out)
            }
            Some("sessions.access") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
                crate::types::keys::req_key::<String>(obj, "permissionMode", &mut out);
                Some(out)
            }
            Some("sessions.rename") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
                crate::types::keys::req_key::<String>(obj, "title", &mut out);
                Some(out)
            }
            Some("sessions.forget") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
                Some(out)
            }
            Some("sessions.delete") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<ProcessesAsk>(obj, "processes", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
                Some(out)
            }
            Some("sessions.read") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("sessions.replaceable") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("sessions.warm") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "project", &mut out);
                crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "harness", &mut out);
                crate::types::keys::opt_key::<String>(obj, "model", &mut out);
                crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
                crate::types::keys::opt_key::<String>(obj, "permissionMode", &mut out);
                crate::types::keys::opt_key::<AccessChoice>(obj, "access", &mut out);
                crate::types::keys::opt_key::<String>(obj, "contextWindow", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "fast", &mut out);
                Some(out)
            }
            Some("sessions.settle") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<Vec<String>>(obj, "threadIds", &mut out, |x| items_at_least(x.len(), 1));
                crate::types::keys::opt_key::<bool>(obj, "finished", &mut out);
                crate::types::keys::opt_key::<ProcessesAsk>(obj, "processes", &mut out);
                Some(out)
            }
            Some("sessions.mark") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<Vec<String>>(obj, "threadIds", &mut out, |x| items_at_least(x.len(), 1));
                crate::types::keys::req_key::<ThreadMarks>(obj, "marks", &mut out);
                Some(out)
            }
            Some("sessions.restore") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<Vec<String>>(obj, "threadIds", &mut out, |x| items_at_least(x.len(), 1));
                Some(out)
            }
            Some("sessions.search") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "query", &mut out);
                Some(out)
            }
            Some("sessions.aside") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
                crate::types::keys::req_key::<String>(obj, "question", &mut out);
                crate::types::keys::opt_key::<String>(obj, "askId", &mut out);
                Some(out)
            }
            Some("sessions.run") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "runId", &mut out);
                crate::types::keys::req_key::<String>(obj, "block", &mut out);
                crate::types::keys::req_key::<String>(obj, "command", &mut out);
                crate::types::keys::req_key::<RunState>(obj, "state", &mut out);
                crate::types::keys::opt_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::opt_key::<JsInt>(obj, "exitCode", &mut out);
                crate::types::keys::opt_key::<JsInt>(obj, "signal", &mut out);
                crate::types::keys::opt_key::<String>(obj, "output", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<String>(obj, "turnId", &mut out);
                Some(out)
            }
            Some("sessions.rewind") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "turnId", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "files", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "undo", &mut out);
                Some(out)
            }
            Some("golden.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                Some(out)
            }
            Some("capabilities.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("golden.prepare") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<MachineKind>(obj, "kind", &mut out);
                Some(out)
            }
            Some("golden.seal") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "builderId", &mut out);
                Some(out)
            }
            Some("snapshots.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "name", &mut out);
                Some(out)
            }
            Some("snapshots.storage") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("cost.history") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("cost.spend") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("usage.used") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<UsageRange>(obj, "range", &mut out);
                crate::types::keys::req_key::<UsageSplit>(obj, "split", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "outside", &mut out);
                Some(out)
            }
            Some("usage.accounts") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "fresh", &mut out);
                Some(out)
            }
            Some("usage.reset") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "account", &mut out);
                crate::types::keys::opt_key::<String>(obj, "creditId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "on", &mut out);
                Some(out)
            }
            Some("places.readings") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<ReadingsRange>(obj, "range", &mut out);
                Some(out)
            }
            Some("snapshots.rollback") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<JsNumber>(obj, "version", &mut out);
                crate::types::keys::opt_key::<String>(obj, "name", &mut out);
                Some(out)
            }
            Some("workspaces.portReach") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key_with::<JsInt>(obj, "port", &mut out, |x| {
                    num_at_least(x.as_f64(), 1.0).and_then(|()| num_at_most(x.as_f64(), 65535.0))
                });
                Some(out)
            }
            Some("workspaces.portProbe") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key_with::<JsInt>(obj, "port", &mut out, |x| {
                    num_at_least(x.as_f64(), 1.0).and_then(|()| num_at_most(x.as_f64(), 65535.0))
                });
                Some(out)
            }
            Some("workspaces.rebuild") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("forwards.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("forwards.stop") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<RelayPort>(obj, "port", &mut out);
                Some(out)
            }
            Some("workspaces.exec") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key_with::<Vec<String>>(obj, "argv", &mut out, |x| items_at_least(x.len(), 1));
                crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
                Some(out)
            }
            Some("host.folders") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "dir", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "hidden", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "repos", &mut out);
                crate::types::keys::opt_key::<String>(obj, "on", &mut out);
                Some(out)
            }
            Some("host.terminalConfig") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<TerminalScheme>(obj, "scheme", &mut out);
                Some(out)
            }
            Some("editor.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("editor.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key_with::<JsInt>(obj, "line", &mut out, |x| opt_rule(x, |v| num_above(v.as_f64(), 0.0)));
                crate::types::keys::opt_key::<EditorId>(obj, "editor", &mut out);
                Some(out)
            }
            Some("ssh.port") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                Some(out)
            }
            Some("ssh.include") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "on", &mut out);
                Some(out)
            }
            Some("agents.read") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                Some(out)
            }
            Some("servers.tools") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "refresh", &mut out);
                Some(out)
            }
            Some("servers.icon") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "host", &mut out, |x| {
                    text_at_least(x, 1).and_then(|()| text_at_most(x, 260))
                });
                crate::types::keys::opt_key::<bool>(obj, "refresh", &mut out);
                Some(out)
            }
            Some("agents.signIn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "terminal", &mut out);
                Some(out)
            }
            Some("servers.signIn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<McpScope>(obj, "scope", &mut out);
                crate::types::keys::opt_key::<String>(obj, "project", &mut out);
                Some(out)
            }
            Some("agents.signInCode") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "signInId", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "code", &mut out, |x| text_at_least(x, 1));
                Some(out)
            }
            Some("agents.signIns") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("agents.signInStop") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "signInId", &mut out);
                Some(out)
            }
            Some("agents.signInLine") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::opt_key::<String>(obj, "name", &mut out);
                Some(out)
            }
            Some("agents.key") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "key", &mut out, |x| text_at_least(x, 1));
                crate::types::keys::opt_key::<AgentsKeyWay>(obj, "way", &mut out);
                Some(out)
            }
            Some("agents.cloudSignIn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsCloudSignInAgent>(obj, "agent", &mut out);
                Some(out)
            }
            Some("providers.checkKey") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "provider", &mut out, |x| text_at_most(x, 64));
                crate::types::keys::req_key_with::<String>(obj, "key", &mut out, |x| text_at_least(x, 1));
                Some(out)
            }
            Some("broker.status") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("agents.addTools") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                Some(out)
            }
            Some("agents.setup") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "on", &mut out);
                crate::types::keys::dbl_key_with::<String>(obj, "program", &mut out, |x| dbl_rule(x, |v| text_at_least(v, 1)));
                crate::types::keys::dbl_key_with::<String>(obj, "configDir", &mut out, |x| dbl_rule(x, |v| text_at_least(v, 1)));
                crate::types::keys::dbl_key::<Vec<String>>(obj, "args", &mut out);
                env_key(obj, &mut out);
                Some(out)
            }
            Some("skills.search") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "q", &mut out);
                crate::types::keys::opt_key_with::<JsInt>(obj, "limit", &mut out, |x| {
                    opt_rule(x, |v| num_at_least(v.as_f64(), 1.0).and_then(|()| num_at_most(v.as_f64(), 50.0)))
                });
                Some(out)
            }
            Some("skills.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "skill", &mut out);
                Some(out)
            }
            Some("skills.preview") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
                Some(out)
            }
            Some("skills.add") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "skill", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "agents", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
                Some(out)
            }
            Some("skills.remove") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
                Some(out)
            }
            Some("skills.toggle") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
                crate::types::keys::req_key::<bool>(obj, "on", &mut out);
                Some(out)
            }
            Some("servers.add") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
                crate::types::keys::opt_key::<String>(obj, "command", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "args", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "env", &mut out);
                crate::types::keys::opt_key::<String>(obj, "url", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "headers", &mut out);
                Some(out)
            }
            Some("servers.remove") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<McpScope>(obj, "scope", &mut out);
                Some(out)
            }
            Some("servers.toggle") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::req_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<McpScope>(obj, "scope", &mut out);
                crate::types::keys::req_key::<bool>(obj, "on", &mut out);
                Some(out)
            }
            Some("plugins.toggle") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
                crate::types::keys::req_key::<String>(obj, "agent", &mut out);
                crate::types::keys::req_key::<String>(obj, "plugin", &mut out);
                crate::types::keys::req_key::<bool>(obj, "on", &mut out);
                Some(out)
            }
            Some("init.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "on", &mut out);
                Some(out)
            }
            Some("init.keys") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key_with::<String>(obj, "provider", &mut out, |x| opt_rule(x, |v| text_at_most(v, 64)));
                crate::types::keys::opt_key::<String>(obj, "key", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "rows", &mut out);
                Some(out)
            }
            Some("init.start") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<InitRoad>(obj, "road", &mut out);
                crate::types::keys::opt_key::<String>(obj, "harness", &mut out);
                crate::types::keys::opt_key::<String>(obj, "on", &mut out);
                Some(out)
            }
            Some("init.answer") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<InitScreenId>(obj, "screen", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "ticks", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "answers", &mut out);
                Some(out)
            }
            Some("init.step") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<JsInt>(obj, "at", &mut out, |x| num_at_least(x.as_f64(), 0.0));
                Some(out)
            }
            Some("init.draft") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "at", &mut out, |x| text_at_least(x, 1).and_then(|()| text_at_most(x, 64)));
                crate::types::keys::opt_key::<Vec<String>>(obj, "ticks", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "answers", &mut out);
                Some(out)
            }
            Some("init.retry") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "tool", &mut out);
                Some(out)
            }
            Some("init.build") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "firstWorkspace", &mut out);
                crate::types::keys::opt_key::<String>(obj, "importFolder", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "yes", &mut out);
                crate::types::keys::opt_key::<String>(obj, "on", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "rebuild", &mut out);
                Some(out)
            }
            Some("init.signInCode") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "tool", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "code", &mut out, |x| {
                    text_at_least(x, 1).and_then(|()| text_at_most(x, 256))
                });
                Some(out)
            }
            Some("init.cancel") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("preferences.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("preferences.set") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<PreferencesPatch>(obj, "patch", &mut out);
                Some(out)
            }
            Some("release.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("release.check") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
                Some(out)
            }
            Some("host.restart") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("projects.add") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "source", &mut out);
                crate::types::keys::opt_key::<String>(obj, "on", &mut out);
                crate::types::keys::opt_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<String>(obj, "base", &mut out);
                crate::types::keys::opt_key::<String>(obj, "into", &mut out);
                crate::types::keys::opt_key::<SeedChoice>(obj, "seed", &mut out);
                Some(out)
            }
            Some("project.seed.plan") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "source", &mut out);
                crate::types::keys::opt_key::<String>(obj, "on", &mut out);
                Some(out)
            }
            Some("projects.seed") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "project", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "keep", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "cut", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "none", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "save", &mut out);
                Some(out)
            }
            Some("projects.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("conversations.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "project", &mut out);
                crate::types::keys::opt_key::<String>(obj, "agent", &mut out);
                Some(out)
            }
            Some("conversations.read") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "project", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "agent", &mut out, |x| text_at_least(x, 1));
                crate::types::keys::req_key_with::<String>(obj, "conversation", &mut out, |x| {
                    text_at_least(x, 1).and_then(|()| text_at_most(x, 200))
                });
                crate::types::keys::opt_key::<bool>(obj, "preview", &mut out);
                Some(out)
            }
            Some("projects.defaults") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                Some(out)
            }
            Some("projects.resolve") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ref", &mut out);
                Some(out)
            }
            Some("projects.branch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "projectId", &mut out);
                Some(out)
            }
            Some("projects.remove") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "projectId", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
                Some(out)
            }
            Some("projects.icon") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "projectId", &mut out);
                crate::types::keys::req_key_with::<Option<String>>(obj, "png", &mut out, |x| opt_rule(x, |v| text_at_most(v, 327680)));
                Some(out)
            }
            Some("projects.icons") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<Vec<ProjectIconHash>>(obj, "hashes", &mut out, |x| items_at_most(x.len(), 1000));
                Some(out)
            }
            Some("project.plan") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "source", &mut out);
                Some(out)
            }
            Some("project.import") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "source", &mut out);
                crate::types::keys::req_key::<String>(obj, "dest", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "replace", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "carry", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "rewrite", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "agents", &mut out);
                Some(out)
            }
            Some("project.export") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
                crate::types::keys::req_key::<String>(obj, "source", &mut out);
                crate::types::keys::req_key::<String>(obj, "dest", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "replace", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "agents", &mut out);
                Some(out)
            }
            Some("image.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "name", &mut out);
                Some(out)
            }
            Some("image.build") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "place", &mut out, |x| text_at_least(x, 1));
                crate::types::keys::opt_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
                Some(out)
            }
            Some("image.export") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "dest", &mut out, |x| text_at_least(x, 1));
                crate::types::keys::req_key_with::<String>(obj, "passphrase", &mut out, |x| {
                    text_at_least(x, 12).and_then(|()| text_at_most(x, 256))
                });
                crate::types::keys::opt_key::<String>(obj, "name", &mut out);
                Some(out)
            }
            Some("slates.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("slates.write") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
                crate::types::keys::opt_key::<String>(obj, "text", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, Value>>(obj, "document", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
                crate::types::keys::opt_key::<JsInt>(obj, "ifVersion", &mut out);
                crate::types::keys::opt_key::<SlateWireValues>(obj, "values", &mut out);
                crate::types::keys::opt_key::<SlatesWritePress>(obj, "press", &mut out);
                Some(out)
            }
            Some("slates.state") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
                crate::types::keys::opt_key::<SlateWireValues>(obj, "values", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "start", &mut out);
                crate::types::keys::opt_key::<JsInt>(obj, "ifVersion", &mut out);
                Some(out)
            }
            Some("slates.read") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "values", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "text", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "sketch", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "document", &mut out);
                Some(out)
            }
            Some("slates.catalog") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "name", &mut out);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
                Some(out)
            }
            Some("slates.event") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<JsInt>(obj, "version", &mut out);
                crate::types::keys::req_key::<String>(obj, "piece", &mut out);
                crate::types::keys::req_key::<SlatesEventEvent>(obj, "event", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "requestId", &mut out, |x| {
                    text_at_least(x, 1).and_then(|()| text_at_most(x, 200))
                });
                crate::types::keys::opt_key::<SlatesEventScope>(obj, "scope", &mut out);
                crate::types::keys::opt_key_with::<JsInt>(obj, "rowAction", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 0.0)));
                Some(out)
            }
            Some("slates.approve") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<String>(obj, "key", &mut out);
                crate::types::keys::req_key::<SlatesApproveScope>(obj, "scope", &mut out);
                Some(out)
            }
            Some("slates.cancel") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<String>(obj, "run", &mut out);
                Some(out)
            }
            Some("slates.revoke") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<String>(obj, "key", &mut out);
                Some(out)
            }
            Some("slates.shown") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                Some(out)
            }
            Some("slates.subscribe") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<Vec<String>>(obj, "sources", &mut out);
                Some(out)
            }
            Some("slates.unsubscribe") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<Vec<String>>(obj, "sources", &mut out);
                Some(out)
            }
            Some("slates.resolve") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key_with::<Vec<String>>(obj, "paths", &mut out, |x| items_at_most(x.len(), 200));
                Some(out)
            }
            Some("slates.image") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "src", &mut out, |x| text_at_most(x, 4096));
                crate::types::keys::opt_key_with::<String>(obj, "have", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
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
        crate::types::keys::opt_key::<bool>(obj, "spawn", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxMachines", &mut out, Self::rule_max_machines);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxDepth", &mut out, Self::rule_max_depth);
        Some(out)
    }
}
impl PlacesSetSpawn {
    fn rule_max_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_max_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
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
        crate::types::keys::req_key::<JsInt>(obj, "cpu", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "memMb", &mut out);
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
        crate::types::keys::opt_key::<bool>(obj, "spawn", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxMachines", &mut out, Self::rule_max_machines);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxDepth", &mut out, Self::rule_max_depth);
        Some(out)
    }
}
impl WorkspacesCreateAgents {
    fn rule_max_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_max_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
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
        crate::types::keys::req_key::<String>(obj, "on", &mut out);
        crate::types::keys::opt_key::<String>(obj, "size", &mut out);
        crate::types::keys::opt_key::<String>(obj, "from", &mut out);
        crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
        crate::types::keys::opt_key::<String>(obj, "harness", &mut out);
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
        crate::types::keys::req_key_with::<JsInt>(obj, "cpu", &mut out, Self::rule_cpu);
        crate::types::keys::req_key_with::<JsInt>(obj, "memMb", &mut out, Self::rule_mem_mb);
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
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<bool>(obj, "on", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "piece", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "index", &mut out, Self::rule_index);
        crate::types::keys::opt_key_with::<JsInt>(obj, "action", &mut out, Self::rule_action);
        Some(out)
    }
}
impl SlatesWritePress {
    fn rule_index(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_action(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
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
        crate::types::keys::any_key::<Value>(obj, "item", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "index", &mut out, Self::rule_index);
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
        if let Some(named) = obj.get("op").and_then(serde_json::Value::as_str).and_then(super::runtime_ops::strict_keys) {
            let unknown: Vec<String> = obj.keys().filter(|k| !named.contains(&k.as_str())).cloned().collect();
            if !unknown.is_empty() {
                out.push(crate::types::check::unrecognized(Vec::new(), unknown));
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
        let Some(tag) = obj.get("type") else {
            return Some(vec![crate::types::check::missing::<Self>("type")]);
        };
        match tag.as_str() {
            Some("exec.output") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "execId", &mut out);
                crate::types::keys::req_key::<String>(obj, "text", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("exec.exit") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "execId", &mut out);
                crate::types::keys::req_key::<Option<JsInt>>(obj, "exitCode", &mut out);
                crate::types::keys::opt_key::<String>(obj, "error", &mut out);
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
        crate::types::keys::req_key::<Option<ReqId>>(obj, "id", &mut out);
        crate::types::keys::req_key::<True>(obj, "ok", &mut out);
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
        crate::types::keys::req_key::<Option<ReqId>>(obj, "id", &mut out);
        crate::types::keys::req_key::<False>(obj, "ok", &mut out);
        crate::types::keys::req_key::<String>(obj, "error", &mut out);
        crate::types::keys::opt_key::<String>(obj, "kind", &mut out);
        crate::types::keys::opt_key::<String>(obj, "fix", &mut out);
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
        Some(crate::types::check::union_answer(vec![
            crate::types::check::issues_of::<RuntimeOkResponse>(value),
            crate::types::check::issues_of::<RuntimeErrorResponse>(value),
        ]))
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
        crate::types::keys::req_key::<SessionInterruptOutcome>(obj, "outcome", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "under", &mut out);
        crate::types::keys::opt_key::<String>(obj, "error", &mut out);
        crate::types::keys::opt_key::<String>(obj, "left", &mut out);
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
        crate::types::keys::req_key::<SessionSteerOutcome>(obj, "outcome", &mut out);
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
        crate::types::keys::req_key::<SessionAccessOutcome>(obj, "outcome", &mut out);
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
        crate::types::keys::req_key::<SessionAnswerOutcome>(obj, "outcome", &mut out);
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
        crate::types::keys::req_key::<SessionRenameOutcome>(obj, "outcome", &mut out);
        crate::types::keys::opt_key::<String>(obj, "error", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
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
        crate::types::keys::req_key::<Vec<SettledThread>>(obj, "settled", &mut out);
        crate::types::keys::req_key::<Vec<SessionSettleResultLeftItem>>(obj, "left", &mut out);
        crate::types::keys::opt_key::<Vec<ActivityEnded>>(obj, "ended", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "why", &mut out);
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
        crate::types::keys::req_key::<Vec<SettledThread>>(obj, "restored", &mut out);
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
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "snippet", &mut out);
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
        crate::types::keys::req_key::<Vec<SessionSearchHit>>(obj, "hits", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "unsearched", &mut out, Self::rule_unsearched);
        Some(out)
    }
}
impl SessionSearchResult {
    fn rule_unsearched(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
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
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
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
        crate::types::keys::req_key::<JsInt>(obj, "turns", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "files", &mut out);
        crate::types::keys::opt_key::<String>(obj, "kept", &mut out);
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
        crate::types::keys::req_key::<SessionView>(obj, "session", &mut out);
        crate::types::keys::req_key::<SessionStartOutcome>(obj, "outcome", &mut out);
        crate::types::keys::req_key::<String>(obj, "turnId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "notice", &mut out);
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
        crate::types::keys::req_key::<SessionWarmResultWarm>(obj, "warm", &mut out);
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
        crate::types::keys::req_key::<True>(obj, "works", &mut out);
        crate::types::keys::req_key::<String>(obj, "account", &mut out);
        crate::types::keys::req_key::<String>(obj, "plan", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "running", &mut out, Self::rule_running);
        crate::types::keys::req_key_with::<JsInt>(obj, "limit", &mut out, Self::rule_limit);
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
        crate::types::keys::opt_key::<BrokerStatusClaude>(obj, "claude", &mut out);
        crate::types::keys::opt_key::<BrokerStatusCodex>(obj, "codex", &mut out);
        crate::types::keys::opt_key::<String>(obj, "github", &mut out);
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
            Some(Value::Object(inner)) => crate::types::kept::js_order(inner.iter().map(|(k, v)| (k.as_str(), v)))
                .into_iter()
                .filter_map(|(_, v)| v.as_str().map(str::to_string))
                .collect(),
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
