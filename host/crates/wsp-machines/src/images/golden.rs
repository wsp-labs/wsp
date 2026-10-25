// SPDX-License-Identifier: AGPL-3.0-only
//! `images/golden.ts` at 65f6b89c7, in three units: `prepare` (build, get, buildPlace, prepareAt, prepare, seal,
//! recipe; spec `golden_prepare.md`), `upgrade` (upgrade, builderReach, builders, kill; `golden_upgrade.md`) and
//! `storage` (storage, orphans, deleteOrphans, retention, prune, rollback, promote; `golden_storage.md`).
pub mod prepare;
pub mod storage;
pub mod upgrade;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};

use wsp_core::clock::Cancel;
use wsp_core::types::pending::{Clock, Store};
use wsp_core::types::wiring::{GoldenRecipe, PlaceBackends};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::views::golden_image::{GoldenBuilderView, GoldenLogin, GoldenManifest, GoldenVersion, RecipeDigest, SealedImage};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_words::exit::HostError;

use super::builders::life::{CreateStop, Expired, ImportMark};
use super::builders::places::PlaceAt;
use super::builders::{BuildPlace, Claim, ImageDoor};
use super::engine::{EngineBuilder, GoldenRoads, KillConfirm, SealResult, StageFn};
use super::{BuilderRef, Builders, ImageResult, Joined};

/// `ctx.preparing`'s entry: the import hash a prepare runs for and its future (golden.ts:269).
#[derive(Clone)]
pub struct Preparing {
    pub hash: Option<String>,
    pub run: Joined<GoldenBuilderView>,
}

/// Every call the golden area makes on the rest of the runtime. The builders and records areas' members are here as
/// calls so each unit is tested alone.
#[async_trait(?Send)]
pub trait GoldenCtx {
    fn store(&self) -> Rc<dyn Store>;
    fn clock(&self) -> Rc<dyn Clock>;
    fn roads(&self) -> Rc<dyn GoldenRoads>;
    fn places(&self) -> Rc<dyn PlaceBackends>;
    fn door(&self) -> Option<Rc<dyn ImageDoor>>;
    /// `ctx.backend`.
    fn backend(&self) -> Arc<dyn MachineBackend>;
    /// `opts.killConfirm`.
    fn kill_confirm(&self) -> Option<KillConfirm>;
    /// `ctx.state.owner`.
    fn owner(&self) -> String;
    fn image_mark(&self) -> String;
    /// golden.ts `GoldenImport.recipeHash` and `.recipe`, which wsp-engine's `GoldenImport` does not carry yet; the
    /// join reads them off the engine's import once it does.
    fn import_mark(&self, imp: &wsp_engine::golden_seam::GoldenImport) -> ImportMark;
    fn warn(&self, line: &str);
    fn builders(&self) -> Builders;
    /// `ctx.preparing`, keyed `<place>/<name>`.
    fn preparing(&self) -> Rc<RefCell<IndexMap<String, Preparing>>>;
    /// `ctx.graceTimers` (the builders area's).
    fn grace_timers(&self) -> Rc<RefCell<IndexMap<String, Cancel>>>;
    async fn ready(&self) -> Result<(), HostError>;
    async fn refresh_builders(&self) -> Result<(), HostError>;
    fn claiming(&self, purpose: &str, at: Option<Arc<dyn MachineBackend>>, place: Option<&str>) -> Rc<dyn Claim>;
    /// `ctx.daemonTokenOf(machine)`.
    async fn daemon_token_of(&self, machine: Arc<dyn Machine>) -> Result<Option<String>, HostError>;
    /// `ctx.conflict(message)` (the image area's).
    fn conflict(&self, message: &str) -> HostError;

    // The records area.
    async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError>;
    async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError>;
    async fn put_copy(&self, place: &str, name: &str, manifest: &GoldenManifest) -> Result<(), HostError>;
    async fn copy_recipe_of(&self, place: &str, name: &str, version: u32) -> Result<Option<RecipeDigest>, HostError>;
    async fn drop_copy_recipe(&self, place: &str, name: &str, version: u32) -> Result<(), HostError>;
    async fn recorded_images(&self) -> Result<IndexSet<String>, HostError>;
    fn forked_from(&self, snapshot_id: &str) -> Vec<String>;

    // The builders area.
    async fn image_place(&self, name: &str) -> Result<String, HostError>;
    async fn place_at(&self, word: &str) -> Result<PlaceAt, HostError>;
    fn backend_at(&self, place: &str) -> Result<Arc<dyn MachineBackend>, HostError>;
    fn place_name(&self, place: &str) -> String;
    fn place_kind_of(&self, place: &str) -> PlaceKind;
    async fn build_places(&self) -> Result<Vec<BuildPlace>, HostError>;
    fn recipe_or_throw(&self, named: Option<GoldenRecipe>) -> Result<GoldenRecipe, HostError>;
    fn builder_labels(&self, extra: Option<&IndexMap<String, String>>) -> IndexMap<String, String>;
    fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> StageFn;
    fn recording_creates(&self, b: Arc<dyn MachineBackend>, name: &str, imp: Option<ImportMark>, made: Rc<dyn Fn(BuilderRef)>, stop: Option<CreateStop>, place: &str) -> Arc<dyn MachineBackend>;
    async fn settle_builder(&self, name: &str, builder: EngineBuilder, placeholder: Option<BuilderRef>, place: &str) -> Result<BuilderRef, HostError>;
    async fn seal_entry(&self, entry: &BuilderRef, keep: bool, logins: Option<Vec<GoldenLogin>>, copy: Option<SealedImage>) -> ImageResult<SealResult>;
    async fn hold(&self, b: &BuilderRef) -> Result<(), HostError>;
    async fn forget_builder(&self, id: &str) -> Result<(), HostError>;
    async fn forget_if_gone(&self, entry: &BuilderRef, at: Arc<dyn MachineBackend>) -> Result<(), HostError>;
    fn builder_view(&self, entry: &BuilderRef) -> GoldenBuilderView;
    fn refuse_untouchable(&self, entry: &BuilderRef) -> Result<(), HostError>;
    async fn expire_grace(&self) -> ImageResult<Expired>;
    fn in_window(&self, sealed_at: &str) -> bool;
    async fn drop_image(&self, v: &GoldenVersion, at: Option<Arc<dyn MachineBackend>>) -> ImageResult<()>;
}

/// `goldenArea(ctx)` (golden.ts:64): `Runtime["golden"]`. Holds nothing of its own; the joins it keeps are the
/// context's `preparing`.
pub struct GoldenArea {
    pub ctx: Rc<dyn GoldenCtx>,
}

impl GoldenArea {
    pub fn new(ctx: Rc<dyn GoldenCtx>) -> Rc<GoldenArea> {
        todo!()
    }
}
