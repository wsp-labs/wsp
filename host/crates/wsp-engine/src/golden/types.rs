// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of golden.ts (packages/engine/src/golden.ts at 65f6b89c7): the options each road takes, the builder, the
//! import ledger and result, the stage listener, and the typed failures a seal can end with.
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde::Serialize;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::{
    GoldenBaseTool, GoldenLeftBehind, GoldenLogin, GoldenManifest, GoldenMissingTool, GoldenRetired, GoldenStage, GoldenStep, GoldenVersion,
    MachineKind, RecipeDigest,
};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_words::words::image::{BuilderReading, ProviderAnswer};

use crate::errors::EngineError;
use crate::fetch::Fetch;
use crate::golden_seam::{
    CutNames, GoldenImport, HomebrewCheckout, McpResult, SkippedPath, ToolResult,
};
use crate::gone::KillConfirm;
use crate::machine::{Machine, MachineBackend};
use crate::machine_context::ContextResult;

/// golden.ts:31 `StageListener`: `(stage, detail, step, left)`. `left` names the machines the stage made and could not
/// remove; only a failure that tried to clean up carries it. The engine's tools loop cannot carry a `step` (its own
/// listener has two arguments), so the frames it writes carry none.
pub type StageFn = Arc<dyn Fn(GoldenStage, Option<&str>, Option<GoldenStep>, Option<&[String]>)>;

/// golden.ts:380 `MachineSize`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MachineSize {
    pub cpu: Option<JsNumber>,
    pub mem_mb: Option<JsNumber>,
    pub envs: Option<IndexMap<String, String>>,
    pub labels: Option<IndexMap<String, String>>,
}

/// golden.ts:405 `Builder`: a first-life machine with the harness installed, waiting to be sealed. Replaced whole, never
/// changed in place.
#[derive(Clone)]
pub struct Builder {
    pub machine: Arc<dyn Machine>,
    pub kind: MachineKind,
    pub base_template: String,
    pub setup_sha: String,
    pub created_at: String,
    /// False once the machine has ever been resumed; `seal_golden` refuses it then.
    pub first_life: bool,
    /// What the provider built, read back after create (it may clamp the request); the sealed version records it.
    pub size: WorkspaceSize,
    pub import: Option<ImportLedger>,
    pub parent_snapshot_id: Option<String>,
    pub base: Option<Vec<GoldenBaseTool>>,
    pub retired: Option<Vec<GoldenRetired>>,
}

/// golden.ts:520 `ImportStage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ImportStage {
    #[serde(rename = "applying-setup")]
    ApplyingSetup,
    #[serde(rename = "uploading-files")]
    UploadingFiles,
    #[serde(rename = "installing-tools")]
    InstallingTools,
    #[serde(rename = "installing-harness")]
    InstallingHarness,
    #[serde(rename = "installing-mcp")]
    InstallingMcp,
}

/// golden.ts:522 `ImportLedger`: what of the recipe is on a machine. Serializes with the TS's keys in its order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLedger {
    pub recipe_hash: String,
    pub applied: Vec<ImportStage>,
    /// The version checks of the agents that installed, joined; the seal runs this on the fork.
    pub smoke: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe: Option<RecipeDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_tools: Option<Vec<GoldenMissingTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silenced: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_noise: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_behind: Option<Vec<GoldenLeftBehind>>,
}

/// golden.ts:539 `AgentResult`. `outcome` is `installed`, `failed` or `skipped`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentResult {
    pub id: String,
    pub name: String,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<i64>,
}

/// golden.ts:549 `ImportResult.files`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResultFiles {
    pub bytes: u64,
    pub skipped: Vec<SkippedPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cut: Option<Vec<CutNames>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blanked: Option<Vec<SkippedPath>>,
}

/// golden.ts:547 `ImportResult`. The engine's own `golden_seam::ImportResult` is the part of it machine context reads;
/// `to_context` is that part. Fields are in the order the TS assigns them, which is the order its JSON writes them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportResult {
    pub recipe_hash: String,
    pub tools: Vec<ToolResult>,
    pub agents: Vec<AgentResult>,
    pub base: Option<Vec<ToolResult>>,
    pub retired: Option<Vec<GoldenRetired>>,
    pub files: Option<ResultFiles>,
    pub homebrew: Option<HomebrewCheckout>,
    pub mcp: Option<Vec<McpResult>>,
    pub context: Option<Vec<ContextResult>>,
    pub context_failure: Option<String>,
}

/// golden.ts:1246 `ApplyDeltaOptions`.
#[derive(Clone)]
pub struct ApplyDeltaOptions {
    pub setup: String,
    /// The smoke of the version being updated; removed agents leave it, added ones join it.
    pub previous_smoke: String,
    pub previous_missing: Option<Vec<GoldenMissingTool>>,
    pub previous_left_behind: Option<Vec<GoldenLeftBehind>>,
    /// The base tools read on the version being updated; a version sealed before they existed has none and is refused.
    pub previous_base: Option<Vec<GoldenBaseTool>>,
    pub fetch: Option<Arc<dyn Fetch>>,
    pub on_stage: Option<StageFn>,
}

/// golden.ts:1235 `GoldenDelta`.
#[derive(Clone, Default)]
pub struct GoldenDelta {
    pub import: GoldenImport,
    /// The rows this delta takes out of the recipe. Nothing is taken off the image for them.
    pub retired: Vec<GoldenRetired>,
    /// Every row the next version's image carries that its recipe does not ask for.
    pub retired_on_image: Vec<GoldenRetired>,
}

/// golden.ts:565 `ApplyImportOptions`.
#[derive(Clone, Default)]
pub struct ApplyImportOptions {
    pub import: Option<GoldenImport>,
    pub setup: String,
    /// What the base stage did on this builder, so a floor row that did not land is in the result with the person's tools.
    pub base: Option<Vec<ToolResult>>,
    /// Stages this builder already carries; those for the same recipe hash are skipped.
    pub ledger: Option<ImportLedger>,
    /// Only on an update: the rows the recipe stopped asking for.
    pub retired: Option<Vec<GoldenRetired>>,
    pub fetch: Option<Arc<dyn Fetch>>,
    pub on_stage: Option<StageFn>,
}

/// What `applyGoldenImport` answers: `{ ledger, result }`.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    pub ledger: ImportLedger,
    pub result: ImportResult,
}

/// golden.ts:387 `PrepareBuilderOptions`.
#[derive(Clone)]
pub struct PrepareBuilderOptions {
    pub backend: Arc<dyn MachineBackend>,
    pub size: MachineSize,
    /// Default sandbox; desktop streams a display instead.
    pub kind: Option<MachineKind>,
    pub base_template: Option<String>,
    /// Host-owned step (the daemon bundle lives outside the engine); skipped when absent. A returned string is
    /// reported as the deploying-daemon detail.
    pub deploy_daemon: Option<DeployDaemon>,
    /// Harness install; its sha is recorded in the manifest.
    pub setup: String,
    pub import: Option<GoldenImport>,
    pub fetch: Option<Arc<dyn Fetch>>,
    pub on_stage: Option<StageFn>,
}

/// golden.ts:394 `deployDaemon`.
pub type DeployDaemon = Arc<dyn Fn(Arc<dyn Machine>) -> LocalBoxFuture<'static, Result<Option<String>, EngineError>>>;

/// golden.ts:1362 `UpgradeBuilderOptions`.
#[derive(Clone)]
pub struct UpgradeBuilderOptions {
    pub backend: Arc<dyn MachineBackend>,
    pub size: MachineSize,
    /// The version being updated; the fork boots from its image at its size and kind.
    pub head: GoldenVersion,
    pub delta: GoldenDelta,
    pub setup: String,
    pub fetch: Option<Arc<dyn Fetch>>,
    pub on_stage: Option<StageFn>,
}

/// golden.ts:880 `SealGoldenOptions`.
#[derive(Clone)]
pub struct SealGoldenOptions {
    pub backend: Arc<dyn MachineBackend>,
    pub size: MachineSize,
    /// Runs on a fork of the fresh snapshot; a non-zero exit means no version is sealed.
    pub smoke: String,
    pub smoke_timeout_ms: Option<u64>,
    pub manifest: Option<GoldenManifest>,
    pub on_stage: Option<StageFn>,
    pub kill_confirm: Option<KillConfirm>,
    /// How each sign-in asked of the builder ended; stamped on the version as given.
    pub logins: Option<Vec<GoldenLogin>>,
    /// Leave the builder running after the snapshot, so one more change can re-snapshot it.
    pub keep_builder: bool,
    /// How long the seal waits between snapshot attempts the provider refused (tests shrink it).
    pub snapshot_retry_ms: Option<u64>,
    /// The golden the version belongs to, which names its template; the store's default key when absent.
    pub name: Option<String>,
    /// The host sealing it, in lowercase letters and digits, which names its template too.
    pub host_id: String,
    /// How long the seal waits for the promoted template to read ready, and how often it reads.
    pub template_ready_ms: Option<u64>,
    pub template_poll_ms: Option<u64>,
    /// Absolute guest paths whose presence on the builder refuses the seal, each named; `NEVER_IN_IMAGE` when absent.
    pub refuse: Option<Vec<String>>,
    /// Absolute guest paths to archive off the builder before the snapshot. Absent, no vault is taken.
    pub vault_paths: Option<Vec<String>>,
    /// The vault archive's trip off the builder takes this; the platform fetch when absent.
    pub fetch: Option<Arc<dyn Fetch>>,
}

/// golden.ts:911 `SealResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct SealResult {
    pub manifest: GoldenManifest,
    pub version: GoldenVersion,
    /// True when `keep_builder` was asked and the builder is still running.
    pub builder_kept: bool,
    /// The archive and its hash when `vault_paths` was given.
    pub vault: Option<crate::image_vault::ImageVault>,
}

/// golden.ts:920 `BuildGoldenOptions`.
#[derive(Clone)]
pub struct BuildGoldenOptions {
    pub backend: Arc<dyn MachineBackend>,
    pub size: MachineSize,
    pub setup: String,
    pub smoke: String,
    pub kind: Option<MachineKind>,
    pub base_template: Option<String>,
    pub manifest: Option<GoldenManifest>,
    pub smoke_timeout_ms: Option<u64>,
    pub on_stage: Option<StageFn>,
    pub name: Option<String>,
    pub host_id: String,
}

/// What `buildGolden` answers: `{ manifest, version }`.
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltGolden {
    pub manifest: GoldenManifest,
    pub version: GoldenVersion,
}

/// Every failure a golden road answers: the engine's own, or one of the typed failures of golden.ts the wizard words
/// differently. A plain `new Error(words)` is `Engine(EngineError::Other)`.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GoldenError {
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// golden.ts:65 `SnapshotFailedError`, kind "snapshotFailed".
    #[error("{message}")]
    SnapshotFailed {
        message: String,
        machine_id: String,
        attempts: u32,
        answer: ProviderAnswer,
        builder_state: BuilderReading,
        read_error: Option<String>,
    },
    /// golden.ts:82 `CredentialOnBuilderError`, kind "credentialOnBuilder".
    #[error("{message}")]
    CredentialOnBuilder { message: String, paths: Vec<String> },
    /// disk-sync.ts:22 `DiskSyncError`: the guest's sync answered non-zero.
    #[error("{line}")]
    DiskSync { line: String, answer: String },
}

impl GoldenError {
    /// `e instanceof Error ? e.message : String(e)`.
    pub fn message(&self) -> String {
        self.to_string()
    }
}

pub(crate) fn plain(message: impl Into<String>) -> EngineError {
    EngineError::Other { message: message.into(), code: None }
}

/// A stage listener that is never absent: the TS's `opts.onStage ?? (() => {})`.
#[derive(Clone)]
pub(crate) struct Stage(StageFn);

impl Stage {
    pub(crate) fn new(on_stage: Option<StageFn>) -> Self {
        Stage(on_stage.unwrap_or_else(|| Arc::new(|_, _, _, _| {})))
    }
    /// `stage(name, detail)`.
    pub(crate) fn say(&self, stage: GoldenStage, detail: &str) {
        (self.0)(stage, Some(detail), None, None);
    }
    /// The listener itself, for a call that takes the TS's `onStage`.
    pub(crate) fn as_fn(&self) -> StageFn {
        self.0.clone()
    }
    /// `stage(name)`: no detail.
    pub(crate) fn bare(&self, stage: GoldenStage) {
        (self.0)(stage, None, None, None);
    }
    /// `stage("failed", detail, undefined, left)`.
    pub(crate) fn failed(&self, detail: &str, left: &[String]) {
        (self.0)(GoldenStage::Failed, Some(detail), None, Some(left));
    }
    /// The two-argument listener the tools loop, the MCP round and the plugins take.
    pub(crate) fn listener(&self) -> crate::golden_seam::StageListener {
        let f = self.0.clone();
        Arc::new(move |stage, detail| f(stage, detail, None, None))
    }
}

/// golden.ts:962 `staged`: the listener with the stage it last named kept beside it, so a rollback's own line lands on
/// that stage.
pub(crate) struct Staged {
    pub(crate) stage: Stage,
    current: std::rc::Rc<std::cell::Cell<Option<GoldenStage>>>,
}

impl Staged {
    pub(crate) fn new(on_stage: Option<StageFn>) -> Self {
        let told = Stage::new(on_stage);
        let current: std::rc::Rc<std::cell::Cell<Option<GoldenStage>>> = Default::default();
        let (c, t) = (current.clone(), told.0.clone());
        let wrapped: StageFn = Arc::new(move |name, detail, step, left| {
            if name != GoldenStage::Failed {
                c.set(Some(name));
            }
            t(name, detail, step, left);
        });
        Staged { stage: Stage(wrapped), current }
    }
    /// `current()`: the last stage named that was not `failed`.
    pub(crate) fn current(&self) -> Option<GoldenStage> {
        self.current.get()
    }
}
