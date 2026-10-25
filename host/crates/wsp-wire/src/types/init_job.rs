// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/init-job.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::views::place_kind::PlaceKind;
use indexmap::IndexMap;

/// TS: packages/protocol/src/init-job.ts:93 at a3b870a6e.
pub const LOGIN_CHOICES: &[&str] = &["copy", "machine", "later", "key", "skip", "token"];

/// TS: packages/protocol/src/init-job.ts:106 at a3b870a6e.
pub const SIGN_IN_CODE_MAX: u64 = 256;

/// TS: packages/protocol/src/init-job.ts:147 at a3b870a6e.
pub const INIT_BUILD_STEP: &str = "build";

/// TS: packages/protocol/src/init-job.ts:162 at a3b870a6e.
pub const KEY_REFUSED: &str = "keyRefused";

/// TS: packages/protocol/src/init-job.ts:163 at a3b870a6e.
pub const KEY_UNCHECKED: &str = "keyUnchecked";

/// TS: packages/protocol/src/init-job.ts:13 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitRoad {
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "terminal")]
    Terminal,
}

impl Check for InitRoad {}

/// TS: packages/protocol/src/init-job.ts:19 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitPhase {
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "reading")]
    Reading,
    #[serde(rename = "answering")]
    Answering,
    #[serde(rename = "building")]
    Building,
    #[serde(rename = "signing-in")]
    SigningIn,
    #[serde(rename = "sealing")]
    Sealing,
    #[serde(rename = "finishing")]
    Finishing,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl Check for InitPhase {}

/// TS: packages/protocol/src/init-job.ts:23 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitScreenId {
    #[serde(rename = "agents")]
    Agents,
    #[serde(rename = "tools")]
    Tools,
    #[serde(rename = "also")]
    Also,
    #[serde(rename = "logins")]
    Logins,
    #[serde(rename = "wsp")]
    Wsp,
}

impl Check for InitScreenId {}

/// TS: packages/protocol/src/init-job.ts:26 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitChoice {
    pub value: String,
    pub label: String,
}

impl Check for InitChoice {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitScreenItemLock {
    #[serde(rename = "on")]
    On,
    #[serde(rename = "off")]
    Off,
}

impl Check for InitScreenItemLock {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitScreenItemKey {
    pub name: String,
    pub saved: bool,
}

impl Check for InitScreenItemKey {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:33 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitScreenItem {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub size: Option<Option<JsNumber>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub detail: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<InitScreenItemLock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<InitChoice>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<InitScreenItemKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark: Option<String>,
}

impl Check for InitScreenItem {
    /// Rules: size: >= 0.
    /// Nested checks: choices, key.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitFooterLineTone {
    #[serde(rename = "yellow")]
    Yellow,
    #[serde(rename = "yellowBright")]
    YellowBright,
    #[serde(rename = "red")]
    Red,
}

impl Check for InitFooterLineTone {}

/// TS: packages/protocol/src/init-job.ts:51 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitFooterLine {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<InitFooterLineTone>,
}

impl Check for InitFooterLine {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:54 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitScreen {
    pub id: InitScreenId,
    pub title: String,
    pub top: String,
    pub items: Vec<InitScreenItem>,
    pub ticks: Vec<String>,
    pub answers: IndexMap<String, String>,
    pub footer: Vec<InitFooterLine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tally: Option<String>,
}

impl Check for InitScreen {
    /// Nested checks: id, items, footer.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:78 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitRowKind {
    #[serde(rename = "fact")]
    Fact,
    #[serde(rename = "stage")]
    Stage,
    #[serde(rename = "sign-in")]
    SignIn,
    #[serde(rename = "secret")]
    Secret,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "workspace")]
    Workspace,
    #[serde(rename = "project")]
    Project,
    #[serde(rename = "machine")]
    Machine,
}

impl Check for InitRowKind {}

/// TS: packages/protocol/src/init-job.ts:86 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoginState {
    #[serde(rename = "signed-in")]
    SignedIn,
    #[serde(rename = "not-signed-in")]
    NotSignedIn,
    #[serde(rename = "not-verified")]
    NotVerified,
    #[serde(rename = "skipped")]
    Skipped,
    #[serde(rename = "copied")]
    Copied,
    #[serde(rename = "deferred")]
    Deferred,
}

impl Check for LoginState {}

/// TS: packages/protocol/src/init-job.ts:94 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoginChoice {
    #[serde(rename = "copy")]
    Copy,
    #[serde(rename = "machine")]
    Machine,
    #[serde(rename = "later")]
    Later,
    #[serde(rename = "key")]
    Key,
    #[serde(rename = "skip")]
    Skip,
    #[serde(rename = "token")]
    Token,
}

impl Check for LoginChoice {}

/// TS: packages/protocol/src/init-job.ts:101 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignInFinish {
    #[serde(rename = "callback")]
    Callback,
    #[serde(rename = "code")]
    Code,
    #[serde(rename = "none")]
    None,
}

impl Check for SignInFinish {}

/// TS: packages/protocol/src/init-job.ts:108 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitRow {
    pub id: String,
    pub kind: InitRowKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub label: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<LoginState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish: Option<SignInFinish>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<String>>,
}

impl Check for InitRow {
    /// Nested checks: kind, login, finish.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:142 at a3b870a6e.
pub type InitKeys = IndexMap<String, bool>;

/// TS: packages/protocol/src/init-job.ts:152 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitDraft {
    pub at: String,
    pub ticks: Vec<String>,
    pub answers: IndexMap<String, String>,
}

impl Check for InitDraft {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:168 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitNeedsYou {
    pub what: String,
    pub since: i64,
}

impl Check for InitNeedsYou {
    /// Rules: since: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:173 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlace {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PlaceKind>,
}

impl Check for SetupPlace {
    /// Nested checks: kind.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitSaidLineLevel {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "step")]
    Step,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "warn")]
    Warn,
}

impl Check for InitSaidLineLevel {}

/// TS: packages/protocol/src/init-job.ts:177 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitSaidLine {
    pub level: InitSaidLineLevel,
    pub line: String,
}

impl Check for InitSaidLine {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobDisk {
    pub fixed: JsNumber,
    pub total: JsNumber,
}

impl Check for InitJobDisk {
    /// Rules: fixed: >= 0; total: > 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobProgress {
    pub done: i64,
    pub total: i64,
}

impl Check for InitJobProgress {
    /// Rules: done: >= 0; total: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobThread {
    pub id: String,
    pub workspace_id: String,
    pub session: String,
    pub harness: String,
}

impl Check for InitJobThread {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobGolden {
    pub version: i64,
}

impl Check for InitJobGolden {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobWorkspace {
    pub id: String,
    pub name: String,
}

impl Check for InitJobWorkspace {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:180 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJob {
    pub id: String,
    pub road: InitRoad,
    pub phase: InitPhase,
    pub keys: InitKeys,
    pub step: i64,
    pub stoppable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<InitJobDisk>,
    pub screens: Vec<InitScreen>,
    pub rows: Vec<InitRow>,
    pub progress: InitJobProgress,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_you: Option<InitNeedsYou>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drafts: Option<Vec<InitDraft>>,
    pub log: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<InitJobThread>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_refused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub golden: Option<InitJobGolden>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<Vec<InitSaidLine>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<Vec<InitSaidLine>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builders: Option<Vec<InitSaidLine>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<InitJobWorkspace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<SetupPlace>,
}

impl Check for InitJob {
    /// Rules: step: >= 0.
    /// Nested checks: road, phase, keys, disk, screens, rows, progress, needsYou, drafts, thread, golden, cleanup, update, builders, workspace, place.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:237 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitAgent {
    pub id: String,
    pub name: String,
    pub configured: bool,
    pub takes_tools: bool,
}

impl Check for InitAgent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:241 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitJobEvent {
    pub job: InitJob,
}

impl InitJobEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "init.job";
}

impl Check for InitJobEvent {
    /// Nested checks: job.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/init-job.ts:246 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitNeedsYouEvent {
    pub job_id: String,
    pub needs_you: InitNeedsYou,
}

impl InitNeedsYouEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "job.needs-you";
}

impl Check for InitNeedsYouEvent {
    /// Nested checks: needsYou.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
