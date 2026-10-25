// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/golden-image.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::literal;
use crate::types::check::{Check, SchemaError};
use crate::types::init_job::LoginChoice;
use crate::types::init_job::LoginState;
use crate::types::js_number::JsNumber;
use wsp_frames::True;
use wsp_frames::WorkspaceSize;

literal!(CustomLit, "custom");
literal!(LitV1, 1);
literal!(WspVault1Lit, "wsp-vault-1");
literal!(Aes256GcmLit, "aes-256-gcm");

/// TS: packages/protocol/src/views/golden-image.ts:93 at a3b870a6e.
pub const AGENTS_BIN: &str = "/opt/wsp-agents/bin";

/// TS: packages/protocol/src/views/golden-image.ts:203 at a3b870a6e.
pub const RECIPE_TICKS: &[&str] = &["used", "installed", "default"];

/// TS: packages/protocol/src/views/golden-image.ts:255 at a3b870a6e.
pub const ADDED_BY_AGENT: &str = "added by the agent";

/// TS: packages/protocol/src/views/golden-image.ts:419 at a3b870a6e.
pub const IMAGE_PASSPHRASE_MIN: u64 = 12;

/// TS: packages/protocol/src/views/golden-image.ts:438 at a3b870a6e.
pub const ALREADY_APPLIED: &str = "already applied";

/// TS: packages/protocol/src/views/golden-image.ts:440 at a3b870a6e.
pub const MCP_ID_PREFIX: &str = "agents/mcp/";

/// TS: packages/protocol/src/views/golden-image.ts:453 at a3b870a6e.
pub const BREW_ID_PREFIX: &str = "tools/brew/";

/// TS: packages/protocol/src/views/golden-image.ts:10 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::MachineKind;

impl Check for MachineKind {}

/// TS: packages/protocol/src/views/golden-image.ts:13 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenLogin {
    pub name: String,
    pub state: LoginState,
}

impl Check for GoldenLogin {
    /// Nested checks: state.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GoldenMissingToolOutcome {
    #[serde(rename = "skipped")]
    Skipped,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for GoldenMissingToolOutcome {}

/// TS: packages/protocol/src/views/golden-image.ts:18 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenMissingTool {
    pub id: String,
    pub name: String,
    pub outcome: GoldenMissingToolOutcome,
    pub note: String,
}

impl Check for GoldenMissingTool {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:22 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenLeftBehind {
    pub id: String,
    pub path: String,
    pub note: String,
}

impl Check for GoldenLeftBehind {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:25 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBaseTool {
    pub name: String,
    pub version: String,
}

impl Check for GoldenBaseTool {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:30 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenRetired {
    pub id: String,
    pub name: String,
}

impl Check for GoldenRetired {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenVersionSmoke {
    pub cmd: String,
    pub exit_code: JsNumber,
}

impl Check for GoldenVersionSmoke {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:36 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenVersion {
    pub version: JsNumber,
    pub snapshot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    pub base_template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<MachineKind>,
    pub setup_sha: String,
    pub created_at: String,
    pub smoke: GoldenVersionSmoke,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<WorkspaceSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_shim: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm_bin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logins: Option<Vec<GoldenLogin>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_tools: Option<Vec<GoldenMissingTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silenced: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_noise: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_behind: Option<Vec<GoldenLeftBehind>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_snapshot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Vec<GoldenBaseTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired: Option<Vec<GoldenRetired>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_secrets: Option<bool>,
}

impl Check for GoldenVersion {
    /// Rules: usedBytes: >= 0; imageHash: exactly 64 UTF-16 units.
    /// Nested checks: kind, smoke, size, logins, missingTools, leftBehind, base, retired.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:99 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenManifest {
    pub head: JsNumber,
    pub versions: Vec<GoldenVersion>,
}

impl Check for GoldenManifest {
    /// Nested checks: versions.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolPinArch {
    #[serde(rename = "x86_64")]
    X8664,
    #[serde(rename = "aarch64")]
    Aarch64,
}

impl Check for ToolPinArch {}

/// TS: packages/protocol/src/views/golden-image.ts:126 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPin {
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<ToolPinArch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<True>,
}

impl Check for ToolPin {
    /// Rules: tag: at least 1 UTF-16 units; sha256: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeDigestTicksItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<LoginChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<ToolPin>,
}

impl Check for RecipeDigestTicksItem {
    /// Nested checks: choice, pin.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeDigestFilesItem {
    pub id: String,
    pub path: String,
    pub dest: String,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volatile: Option<bool>,
}

impl Check for RecipeDigestFilesItem {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:134 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeDigest {
    pub ticks: Vec<RecipeDigestTicksItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    pub files: Vec<RecipeDigestFilesItem>,
}

impl Check for RecipeDigest {
    /// Nested checks: ticks, files.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSourceProject {
    pub why: String,
}

impl RecipeSourceProject {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "project";
}

impl Check for RecipeSourceProject {
    /// Rules: why: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSourceInstalled {
    pub paths: Vec<String>,
    pub bin: bool,
}

impl RecipeSourceInstalled {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "installed";
}

impl Check for RecipeSourceInstalled {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSourceUsed {
    pub sessions: i64,
    pub calls: i64,
}

impl RecipeSourceUsed {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "used";
}

impl Check for RecipeSourceUsed {
    /// Rules: sessions: >= 0; calls: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSourcePopular {
    pub sessions: i64,
    pub images: i64,
}

impl RecipeSourcePopular {
    /// The `kind` key's value: the enum holding this struct writes and reads it as its tag.
    pub const KIND: &'static str = "popular";
}

impl Check for RecipeSourcePopular {
    /// Rules: sessions: >= 0; images: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:161 at a3b870a6e.
/// zod's discriminatedUnion on `kind`: each variant's struct leaves that key to this tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RecipeSource {
    #[serde(rename = "project")]
    Project(RecipeSourceProject),
    #[serde(rename = "installed")]
    Installed(RecipeSourceInstalled),
    #[serde(rename = "used")]
    Used(RecipeSourceUsed),
    #[serde(rename = "popular")]
    Popular(RecipeSourcePopular),
}

impl Check for RecipeSource {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecipeRowKind {
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "tool")]
    Tool,
}

impl Check for RecipeRowKind {}

/// TS: packages/protocol/src/views/golden-image.ts:173 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeRow {
    pub id: String,
    pub kind: RecipeRowKind,
    pub on: bool,
    pub source: RecipeSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in: Option<LoginChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<ToolPin>,
}

impl Check for RecipeRow {
    /// Rules: id: at least 1 UTF-16 units; size: >= 0.
    /// Nested checks: source, signIn, pin.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecipeHistoryState {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "empty")]
    Empty,
    #[serde(rename = "unreadable")]
    Unreadable,
    #[serde(rename = "no-reader")]
    NoReader,
}

impl Check for RecipeHistoryState {}

/// TS: packages/protocol/src/views/golden-image.ts:189 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeHistory {
    pub agent: String,
    pub state: RecipeHistoryState,
    pub sessions: i64,
    pub calls: i64,
}

impl Check for RecipeHistory {
    /// Rules: agent: at least 1 UTF-16 units; sessions: >= 0; calls: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:200 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecipeTick {
    #[serde(rename = "used")]
    Used,
    #[serde(rename = "installed")]
    Installed,
    #[serde(rename = "default")]
    Default,
}

impl Check for RecipeTick {}

/// TS: packages/protocol/src/views/golden-image.ts:208 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeCustomRow {
    pub kind: CustomLit,
    pub id: String,
    pub name: String,
    pub install: Vec<String>,
    pub check: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manager: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    pub why: String,
}

impl Check for RecipeCustomRow {
    /// Rules: id: at least 1 UTF-16 units; name: at least 1 UTF-16 units; install[]: at least 1 UTF-16 units; install: at least 1 items; check: at least 1 UTF-16 units; manager: at least 1 UTF-16 units; size: >= 0; why: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:228 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub version: LitV1,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick: Option<RecipeTick>,
    pub histories: Vec<RecipeHistory>,
    pub rows: Vec<RecipeRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<Vec<RecipeCustomRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<bool>,
}

impl Check for Recipe {
    /// Rules: at: at least 1 UTF-16 units.
    /// Nested checks: tick, histories, rows, custom.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderViewSize {
    pub cpu: JsNumber,
    pub mem_mb: JsNumber,
}

impl Check for GoldenBuilderViewSize {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderViewScreen {
    pub stream_url: String,
}

impl Check for GoldenBuilderViewScreen {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderViewHeldBy {
    pub host: String,
    pub pid: JsNumber,
    pub heartbeat: String,
}

impl Check for GoldenBuilderViewHeldBy {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderViewSealed {
    pub at: String,
    pub version: JsNumber,
}

impl Check for GoldenBuilderViewSealed {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:260 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenBuilderView {
    pub id: String,
    pub name: String,
    pub kind: MachineKind,
    pub created_at: String,
    pub place_id: String,
    pub place_name: String,
    pub size: GoldenBuilderViewSize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_usd_per_hour: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<GoldenBuilderViewScreen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<RecipeDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreign_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_by: Option<GoldenBuilderViewHeldBy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub building: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed: Option<GoldenBuilderViewSealed>,
}

impl Check for GoldenBuilderView {
    /// Nested checks: kind, size, screen, recipe, heldBy, sealed.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:293 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GoldenStage {
    #[serde(rename = "creating")]
    Creating,
    #[serde(rename = "deploying-daemon")]
    DeployingDaemon,
    #[serde(rename = "applying-setup")]
    ApplyingSetup,
    #[serde(rename = "uploading-files")]
    UploadingFiles,
    #[serde(rename = "installing-harness")]
    InstallingHarness,
    #[serde(rename = "installing-tools")]
    InstallingTools,
    #[serde(rename = "installing-mcp")]
    InstallingMcp,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "snapshotting")]
    Snapshotting,
    #[serde(rename = "promoting")]
    Promoting,
    #[serde(rename = "smoke-forking")]
    SmokeForking,
    #[serde(rename = "sealed")]
    Sealed,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for GoldenStage {}

/// TS: packages/protocol/src/views/golden-image.ts:312 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenStep {
    pub label: String,
    pub command: String,
}

impl Check for GoldenStep {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:317 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenStageEvent {
    pub name: String,
    pub stage: GoldenStage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<GoldenStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
}

impl GoldenStageEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "golden.stage";
}

impl Check for GoldenStageEvent {
    /// Nested checks: stage, step.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:334 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedVault {
    pub sha256: String,
    pub bytes: i64,
    pub paths: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<Vec<String>>,
    pub taken_at: String,
}

impl Check for SealedVault {
    /// Rules: sha256: exactly 64 UTF-16 units; bytes: >= 0; paths: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:349 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedPin {
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<ToolPinArch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<True>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<String>,
}

impl Check for SealedPin {
    /// Rules: tag: at least 1 UTF-16 units; sha256: at least 1 UTF-16 units; id: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:362 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImage {
    pub name: String,
    pub version: i64,
    pub hash: String,
    pub recipe_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<Recipe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<Vec<SealedPin>>,
    pub logins: Vec<GoldenLogin>,
    pub sealed_at: String,
    pub sealed_from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault: Option<SealedVault>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_secrets: Option<bool>,
}

impl Check for SealedImage {
    /// Rules: version: > 0; hash: exactly 64 UTF-16 units; usedBytes: >= 0.
    /// Nested checks: recipe, pins, logins, vault.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:393 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImageCopy {
    pub place: String,
    pub version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    pub snapshot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    pub built_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
}

impl Check for SealedImageCopy {
    /// Rules: version: > 0; hash: exactly 64 UTF-16 units; sizeBytes: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:411 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImageBuilt {
    pub copy: SealedImageCopy,
    pub built: bool,
}

impl Check for SealedImageBuilt {
    /// Nested checks: copy.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/golden-image.ts:415 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImageExport {
    pub path: String,
    pub bytes: i64,
    pub hash: String,
}

impl Check for SealedImageExport {
    /// Rules: bytes: >= 0; hash: exactly 64 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SealedVaultHeaderTo {
    #[serde(rename = "passphrase")]
    Passphrase,
    #[serde(rename = "key")]
    Key,
}

impl Check for SealedVaultHeaderTo {}

/// TS: packages/protocol/src/views/golden-image.ts:424 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedVaultHeader {
    pub format: WspVault1Lit,
    pub cipher: Aes256GcmLit,
    pub to: SealedVaultHeaderTo,
    pub salt: String,
    pub nonce: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<String>,
    pub image: SealedImage,
}

impl Check for SealedVaultHeader {
    /// Nested checks: image.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// ---- Hand-written below: the functions of the unit.

// views/golden-image.ts:97 at a3b870a6e
// export const onNpmBin = (npmBin: string | undefined, path: string): string => (npmBin === undefined ? path : `${AGENTS_BIN}:${npmBin}:${path}`);
/// `path` alone without an npm bin; else `AGENTS_BIN:<npm bin>:<path>`.
pub fn on_npm_bin(npm_bin: Option<&str>, path: &str) -> String {
    todo!()
}

// views/golden-image.ts:103 at a3b870a6e
// export function goldenHead(manifest: GoldenManifest | undefined): GoldenVersion | undefined {
//   return manifest?.versions.find(v => v.version === manifest.head);
// }
/// The version whose number equals the manifest's head; None without a manifest or with no such version.
pub fn golden_head(manifest: Option<&GoldenManifest>) -> Option<&GoldenVersion> {
    todo!()
}

// views/golden-image.ts:110 at a3b870a6e
// export const holdsNothing = (golden: GoldenManifest | undefined, workspaces: readonly unknown[]): boolean => goldenHead(golden) === undefined && workspaces.length === 0;
pub fn holds_nothing(golden: Option<&GoldenManifest>, workspaces: usize) -> bool {
    todo!()
}

/// What a fork boots from: the version's template where it has one, else its snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageSpec {
    Template(String),
    FromSnapshot(String),
}

/// `golden_image`'s answer: the spec, and `["volatile"]` for a snapshot, `[]` for a template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoldenImage {
    pub spec: ImageSpec,
    pub marks: Vec<&'static str>,
}

// views/golden-image.ts:116 at a3b870a6e
// export function goldenImage(v: Pick<GoldenVersion, "snapshotId" | "templateId">): { spec: { template: string } | { fromSnapshot: string }; marks: readonly "volatile"[] } {
//   return v.templateId !== undefined ? { spec: { template: v.templateId }, marks: [] } : { spec: { fromSnapshot: v.snapshotId }, marks: ["volatile"] };
// }
pub fn golden_image(v: &GoldenVersion) -> GoldenImage {
    todo!()
}

// views/golden-image.ts:245 at a3b870a6e
// export function customRows(recipe: Pick<Recipe, "custom">): readonly RecipeCustomRow[] {
//   return recipe.custom ?? [];
// }
pub fn custom_rows(recipe: &Recipe) -> Vec<RecipeCustomRow> {
    todo!()
}

// views/golden-image.ts:250 at a3b870a6e
// export function commandCheck(bin: string): string {
//   return `command -v ${shellQuote(bin)}`;
// }
/// `command -v <bin quoted by crate::types::shell_quote::shell_quote>`.
pub fn command_check(bin: &str) -> String {
    todo!()
}

// views/golden-image.ts:355 at a3b870a6e
// export function recipePins(digest: Pick<RecipeDigest, "ticks">, key: (id: string) => string = id => id): SealedPin[] {
//   return digest.ticks
//     .flatMap((t): SealedPin[] => (t.pin === undefined ? [] : [{ id: key(t.id), ...t.pin, ...(t.road !== undefined ? { road: t.road } : {}) }]))
//     .sort((a, b) => a.id.localeCompare(b.id));
// }
/// One pin per tick that has one, its id through `key` (the identity where the TS caller passes none), then the pin's
/// own keys, then `road` where the tick has one; sorted by id as `String.prototype.localeCompare` orders them
/// (`crate::text::collate`, which the merge brings down from wsp-words).
pub fn recipe_pins(digest: &RecipeDigest, key: Option<&dyn Fn(&str) -> String>) -> Vec<SealedPin> {
    todo!()
}

// views/golden-image.ts:442 at a3b870a6e (private)
// const AGENTS_PREFIX = "agents/";
const AGENTS_PREFIX: &str = "agents/";

// views/golden-image.ts:445 at a3b870a6e
// export const agentOfRow = (e: { id: string }): string | undefined => (e.id.startsWith(AGENTS_PREFIX) && !e.id.startsWith(MCP_ID_PREFIX) ? e.id.slice(AGENTS_PREFIX.length) : undefined);
/// The agent an `agents/<id>` row is, never an `agents/mcp/` one.
pub fn agent_of_row(id: &str) -> Option<String> {
    todo!()
}

// views/golden-image.ts:448 at a3b870a6e
// export const toolRowPrefix = (manager: string): string => `tools/${manager}/`;
pub fn tool_row_prefix(manager: &str) -> String {
    todo!()
}

// views/golden-image.ts:451 at a3b870a6e
// export const toolRowId = (manager: string, pkg: string): string => `${toolRowPrefix(manager)}${pkg}`;
pub fn tool_row_id(manager: &str, pkg: &str) -> String {
    todo!()
}

// views/golden-image.ts:456 at a3b870a6e
// export const packageOf = (e: { id: string }): string => e.id.split("/").slice(2).join("/");
/// Everything after the second `/` of the id, slashes kept; "" where there is none.
pub fn package_of(id: &str) -> String {
    todo!()
}
