// SPDX-License-Identifier: AGPL-3.0-only
//! The engine's image roads (`packages/engine/src/golden.ts`, `image-vault.ts`, `preview.ts` at 65f6b89c7) as the
//! image areas call them. The engine has not ported them (`wsp-engine/src/golden_seam.rs`: "Snapshotting, sealing and
//! image records are not here"), so they are one trait, `GoldenRoads`, reached through each area's context, and their
//! shapes live here. When the engine ports golden.ts the join implements `GoldenRoads` over its functions and these
//! shapes become `pub use` of the engine's.
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;

use wsp_core::types::wiring::DeployDaemon;
use wsp_engine::cancel::Cancel as EngineCancel;
use wsp_engine::errors::EngineError;
use wsp_engine::golden_seam::GoldenImport;
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::{
    GoldenBaseTool, GoldenLeftBehind, GoldenLogin, GoldenManifest, GoldenMissingTool,
    GoldenRetired, GoldenStage, GoldenStep, GoldenVersion, MachineKind,
};
use wsp_wire::wire::machine_link::{PreviewReach, TemplateRow};

/// golden.ts:522 `ImportLedger` and golden.ts:405 `Builder` (replaced whole, never changed in place) live in wsp-core,
/// since `BuilderRecord.import` and `LiveBuilder.builder` carry them.
pub use wsp_core::types::pending::{Builder as EngineBuilder, ImportLedger, ImportStage};

/// image-vault.ts:14 `ImageVault`: the archive a seal took and its hash.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageVault {
    pub tar: Vec<u8>,
    pub sha256: String,
    pub paths: u64,
    pub held: Vec<String>,
}

/// golden.ts:911 `SealResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct SealResult {
    pub manifest: GoldenManifest,
    pub version: GoldenVersion,
    pub builder_kept: bool,
    pub vault: Option<ImageVault>,
}

/// golden.ts:35 `KillConfirm`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KillConfirm {
    pub grace_ms: Option<u64>,
    pub poll_ms: Option<u64>,
}

/// golden.ts:1235 `GoldenDelta`.
#[derive(Clone, Default)]
pub struct GoldenDelta {
    pub import: GoldenImport,
    pub retired: Vec<GoldenRetired>,
    pub retired_on_image: Vec<GoldenRetired>,
}

/// golden.ts:31 `StageListener` as the image areas hand it: `(stage, detail, step, left)`.
pub type StageFn = Rc<dyn Fn(GoldenStage, Option<String>, Option<GoldenStep>, Option<Vec<String>>)>;

/// golden.ts:380 `MachineSize`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MachineSize {
    pub cpu: Option<JsNumber>,
    pub mem_mb: Option<JsNumber>,
    pub envs: Option<IndexMap<String, String>>,
    pub labels: Option<IndexMap<String, String>>,
}

/// golden.ts:1429 `buildGolden`'s options as `golden.build` passes them: the request's own options (`build`), then
/// the claimed backend, the name, the host mark, the labels and the prior manifest.
#[derive(Clone)]
pub struct BuildGoldenAsk {
    pub request: wsp_core::types::pending::BuildGoldenOptions,
    pub backend: Arc<dyn MachineBackend>,
    pub name: String,
    pub host_id: String,
    pub labels: IndexMap<String, String>,
    pub manifest: Option<GoldenManifest>,
}

/// What `buildGolden` answers that `golden.build` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltGolden {
    pub manifest: GoldenManifest,
    pub version: GoldenVersion,
}

/// golden.ts:387 `PrepareBuilderOptions`, as `golden.prepare` fills it (golden.ts:239-247).
#[derive(Clone)]
pub struct PrepareBuilderAsk {
    pub backend: Arc<dyn MachineBackend>,
    pub size: MachineSize,
    pub setup: String,
    pub base_template: Option<String>,
    pub kind: Option<MachineKind>,
    pub deploy_daemon: Option<DeployDaemon>,
    pub import: Option<GoldenImport>,
    pub on_stage: StageFn,
}

/// golden.ts:880 `SealGoldenOptions`, as `sealEntry` fills it (builders.ts:414-433).
#[derive(Clone)]
pub struct SealGoldenAsk {
    pub backend: Arc<dyn MachineBackend>,
    pub smoke: String,
    pub size: MachineSize,
    pub manifest: Option<GoldenManifest>,
    pub on_stage: StageFn,
    pub kill_confirm: Option<KillConfirm>,
    pub snapshot_retry_ms: Option<u64>,
    pub logins: Option<Vec<GoldenLogin>>,
    pub vault_paths: Option<Vec<String>>,
    pub refuse: Option<Vec<String>>,
    pub keep_builder: bool,
    pub name: String,
    pub host_id: String,
}

/// golden.ts:1362 `UpgradeBuilderOptions`, as `golden.upgrade` fills it (golden.ts:359-369).
#[derive(Clone)]
pub struct UpgradeBuilderAsk {
    pub backend: Arc<dyn MachineBackend>,
    pub head: GoldenVersion,
    pub delta: GoldenDelta,
    pub setup: String,
    pub size: MachineSize,
    pub on_stage: StageFn,
}

/// golden.ts:565 `ApplyImportOptions`, as an attach fills it (golden.ts:211).
#[derive(Clone)]
pub struct ApplyImportAsk {
    pub import: GoldenImport,
    pub setup: String,
    pub ledger: Option<ImportLedger>,
    pub on_stage: StageFn,
}

/// golden.ts:1246 `ApplyDeltaOptions`, as a kept builder's update fills it (golden.ts:327).
#[derive(Clone)]
pub struct ApplyDeltaAsk {
    pub setup: String,
    pub previous_smoke: String,
    pub previous_missing: Option<Vec<GoldenMissingTool>>,
    pub previous_left_behind: Option<Vec<GoldenLeftBehind>>,
    pub previous_base: Option<Vec<GoldenBaseTool>>,
    pub on_stage: StageFn,
}

/// What an apply answers that the image areas read: the ledger now on the machine.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    pub ledger: ImportLedger,
}

/// golden.ts:343 `promoteVersion`'s answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Promoted {
    pub template_id: String,
    pub sharing: Option<u32>,
}

/// golden.ts:303 `Templates`: a backend's template road.
#[async_trait(?Send)]
pub trait Templates {
    async fn promote(&self, snapshot_id: &str, name: &str) -> Result<String, EngineError>;
    async fn get(&self, id: &str) -> Result<TemplateRow, EngineError>;
    async fn list(&self) -> Result<Vec<TemplateRow>, EngineError>;
    async fn delete(&self, id: &str) -> Result<(), EngineError>;
}

/// What `readGone` answers (golden.ts): `gone`, or the machine still standing in some state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GoneReading {
    Gone,
    Standing(String),
}

/// The engine's image roads, one method per TS function. Each failure is the engine's own error, so a reader can
/// tell `NotFirstLife`, a missing machine or the place-absent error apart; `seal_golden`'s `SnapshotFailedError`
/// arrives as `crate::images::ImageError::SnapshotFailed`.
#[async_trait(?Send)]
pub trait GoldenRoads {
    /// golden.ts:1429.
    async fn build_golden(&self, ask: BuildGoldenAsk) -> Result<BuiltGolden, EngineError>;
    /// golden.ts:989.
    async fn prepare_builder(
        &self,
        ask: PrepareBuilderAsk,
    ) -> Result<EngineBuilder, crate::images::ImageError>;
    /// golden.ts:1071.
    async fn seal_golden(
        &self,
        builder: EngineBuilder,
        ask: SealGoldenAsk,
    ) -> Result<SealResult, crate::images::ImageError>;
    /// golden.ts:1375.
    async fn upgrade_builder(
        &self,
        ask: UpgradeBuilderAsk,
    ) -> Result<EngineBuilder, crate::images::ImageError>;
    /// golden.ts:598.
    async fn apply_golden_import(
        &self,
        machine: Arc<dyn Machine>,
        ask: ApplyImportAsk,
    ) -> Result<Applied, EngineError>;
    /// golden.ts:1326.
    async fn apply_delta(
        &self,
        machine: Arc<dyn Machine>,
        delta: GoldenDelta,
        ask: ApplyDeltaAsk,
    ) -> Result<Applied, EngineError>;
    /// golden.ts:868.
    fn next_setup_sha(&self, previous: &str, setup: &str, import: &GoldenImport) -> String;
    /// golden.ts:119.
    async fn kill_until_gone(
        &self,
        at: Arc<dyn MachineBackend>,
        machine: Arc<dyn Machine>,
        confirm: Option<KillConfirm>,
    ) -> Result<(), EngineError>;
    /// golden.ts `readGone`.
    async fn read_gone(
        &self,
        at: Arc<dyn MachineBackend>,
        id: &str,
    ) -> Result<GoneReading, EngineError>;
    /// drop-template.ts `dropTemplate`: nothing for no template id.
    async fn drop_template(
        &self,
        at: Arc<dyn MachineBackend>,
        template_id: Option<&str>,
    ) -> Result<(), EngineError>;
    /// golden.ts:291: none where the backend has no templates.
    fn templates_of(&self, at: Arc<dyn MachineBackend>) -> Option<Rc<dyn Templates>>;
    /// golden.ts:343.
    async fn promote_version(
        &self,
        templates: Rc<dyn Templates>,
        snapshot_id: &str,
        name: &str,
    ) -> Result<Promoted, EngineError>;
    /// golden.ts:1471.
    fn rollback(
        &self,
        manifest: &GoldenManifest,
        version: u32,
    ) -> Result<GoldenManifest, EngineError>;
    /// preview.ts:25.
    async fn refresh_preview_token(
        &self,
        machine: Arc<dyn Machine>,
        port: u16,
        held: Option<PreviewReach>,
    ) -> Result<PreviewReach, EngineError>;
    /// image-vault.ts:52.
    async fn import_image_vault(
        &self,
        machine: Arc<dyn Machine>,
        tar: Vec<u8>,
    ) -> Result<(), EngineError>;
    /// image-vault.ts:236: the refusal's message where the archive carries a member the list does not.
    fn refuse_foreign_members(&self, road: &str, tar: &[u8], held: &[String])
        -> Result<(), String>;
    /// image-vault.ts:249.
    fn image_hash(&self, recipe_hash: &str, vault_sha256: Option<&str>) -> String;
}

/// A seal's cancel: the engine's `Cancel`, which the TS `AbortSignal` becomes.
pub type Signal = EngineCancel;
