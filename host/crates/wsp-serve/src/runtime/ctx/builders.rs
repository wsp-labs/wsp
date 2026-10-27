// SPDX-License-Identifier: AGPL-3.0-only
//! `BuildersCtx`, `GoldenCtx`, `ImageCtx`, `Templates` and `GoldenRoads` over the one context (builders.ts:55-478,
//! golden.ts:64-615, image.ts:30-272, runtime.ts:472-476 and 850-856, and the engine's golden.ts, image-vault.ts,
//! preview.ts and drop-template.ts, which `GoldenRoads` forwards to).
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::{IndexMap, IndexSet};
use wsp_core::clock::Cancel;
use wsp_core::console;
use wsp_core::types::pending::{Builder as CoreBuilder, Clock, ImportLedger, ImportStage, Store};
use wsp_core::types::wiring::{GoldenRecipe, PlaceBackends};
use wsp_engine::errors::EngineError;
use wsp_engine::golden::{self, GoldenError};
use wsp_engine::golden_seam::GoldenImport;
use wsp_engine::gone::{kill_until_gone, machine_state_word, read_gone, KillConfirm as EngineKill};
use wsp_engine::gone_watch::GoneWatch;
use wsp_engine::image_vault::ImageVault as EngineVault;
use wsp_engine::machine::{Machine, MachineBackend, MachineState};
use wsp_machines::images::builders::life::{CreateStop, Expired, ImportMark};
use wsp_machines::images::builders::places::PlaceAt;
use wsp_machines::images::builders::{BuildPlace, BuildersArea, BuildersOpts, Claim, CopyRecipe, ImageDoor, StageAt};
use wsp_machines::images::engine::{
    Applied, ApplyDeltaAsk, ApplyImportAsk, BuildGoldenAsk, BuiltGolden, GoldenDelta, GoldenRoads, GoneReading, ImageVault, KillConfirm,
    MachineSize, PrepareBuilderAsk, Promoted, SealGoldenAsk, SealResult, StageFn, Templates, UpgradeBuilderAsk,
};
use wsp_machines::images::golden::{GoldenArea, GoldenCtx, Preparing};
use wsp_machines::images::image::{CopyRow, ImageArea, ImageCtx, Stopped};
use wsp_machines::images::records::ImageOf;
use wsp_machines::images::{BuilderRef, Builders, ImageError, ImageResult, Joined};
use wsp_machines::machines::machines::create::Claimed;
use wsp_machines::machines::machines::MachinesArea;
use wsp_wire::views::golden_image::{
    GoldenBuilderView, GoldenLogin, GoldenManifest, GoldenStage, GoldenStageEvent, GoldenStep, GoldenVersion, RecipeDigest, SealedImage,
    SealedImageBuilt,
};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_wire::wire::machine_link::{PreviewReach, TemplateRow, VaultRoad};
use wsp_words::exit::HostError;
use wsp_words::words::computer::no_kind_line;

use super::Ctx;
use crate::runtime::join::{spawn_detached, RuntimeRetryClock};

/// The areas the packet 14 tests read state back from, since the context's own fields are crate-private.
#[doc(hidden)]
pub struct BuildersSeams {
    pub builders_area: Rc<BuildersArea>,
    pub golden: Rc<GoldenArea>,
    pub image: Rc<ImageArea>,
    pub machines: Rc<MachinesArea>,
    pub gone: Rc<GoneWatch>,
}

impl Ctx {
    #[doc(hidden)]
    pub fn builders_seams(&self) -> BuildersSeams {
        BuildersSeams {
            builders_area: self.builders_area.get(),
            golden: self.golden.get(),
            image: self.image.get(),
            machines: self.machines.get(),
            gone: self.gone.clone(),
        }
    }
}

fn kill_of(k: EngineKill) -> KillConfirm {
    KillConfirm { grace_ms: k.grace_ms, poll_ms: k.poll_ms }
}

fn kill_to(k: KillConfirm) -> EngineKill {
    EngineKill { grace_ms: k.grace_ms, poll_ms: k.poll_ms }
}

/// What an engine road's failure is to the image areas: the engine's own error whole, or the typed failures of
/// golden.ts as the areas read them.
fn builders_image_error(e: GoldenError) -> ImageError {
    match e {
        GoldenError::Engine(e) => ImageError::Engine(e),
        GoldenError::SnapshotFailed { message, builder_state, .. } => ImageError::SnapshotFailed {
            message,
            builder_state: match builder_state {
                wsp_words::words::image::BuilderReading::State(s) => s,
                wsp_words::words::image::BuilderReading::Unread => "unread".to_string(),
            },
        },
        GoldenError::CredentialOnBuilder { message, .. } => {
            ImageError::Host(HostError::with_kind(message, wsp_words::exit::Kind::CredentialOnBuilder))
        }
        GoldenError::DiskSync { line, .. } => ImageError::Host(HostError::new(line)),
    }
}

fn size_to(s: MachineSize) -> golden::MachineSize {
    golden::MachineSize { cpu: s.cpu, mem_mb: s.mem_mb, envs: s.envs, labels: s.labels }
}

/// The areas' stage listener as the engine calls it.
#[allow(clippy::arc_with_non_send_sync)]
fn stage_to(f: StageFn) -> golden::StageFn {
    Arc::new(move |stage: GoldenStage, detail: Option<&str>, step: Option<GoldenStep>, left: Option<&[String]>| {
        f(stage, detail.map(str::to_string), step, left.map(<[String]>::to_vec))
    })
}

fn stage_of_core(s: ImportStage) -> golden::ImportStage {
    match s {
        ImportStage::ApplyingSetup => golden::ImportStage::ApplyingSetup,
        ImportStage::UploadingFiles => golden::ImportStage::UploadingFiles,
        ImportStage::InstallingTools => golden::ImportStage::InstallingTools,
        ImportStage::InstallingHarness => golden::ImportStage::InstallingHarness,
        ImportStage::InstallingMcp => golden::ImportStage::InstallingMcp,
    }
}

fn stage_of_engine(s: golden::ImportStage) -> ImportStage {
    match s {
        golden::ImportStage::ApplyingSetup => ImportStage::ApplyingSetup,
        golden::ImportStage::UploadingFiles => ImportStage::UploadingFiles,
        golden::ImportStage::InstallingTools => ImportStage::InstallingTools,
        golden::ImportStage::InstallingHarness => ImportStage::InstallingHarness,
        golden::ImportStage::InstallingMcp => ImportStage::InstallingMcp,
    }
}

fn ledger_to(l: ImportLedger) -> golden::ImportLedger {
    golden::ImportLedger {
        recipe_hash: l.recipe_hash,
        applied: l.applied.into_iter().map(stage_of_core).collect(),
        smoke: l.smoke,
        recipe: l.recipe,
        missing_tools: l.missing_tools,
        silenced: l.silenced,
        shell_noise: l.shell_noise,
        left_behind: l.left_behind,
    }
}

fn ledger_of(l: golden::ImportLedger) -> ImportLedger {
    ImportLedger {
        recipe_hash: l.recipe_hash,
        recipe: l.recipe,
        applied: l.applied.into_iter().map(stage_of_engine).collect(),
        smoke: l.smoke,
        missing_tools: l.missing_tools,
        silenced: l.silenced,
        shell_noise: l.shell_noise,
        left_behind: l.left_behind,
    }
}

fn builder_to(b: CoreBuilder) -> golden::Builder {
    golden::Builder {
        machine: b.machine,
        kind: b.kind,
        base_template: b.base_template,
        setup_sha: b.setup_sha,
        created_at: b.created_at,
        first_life: b.first_life,
        size: b.size,
        import: b.import.map(ledger_to),
        parent_snapshot_id: b.parent_snapshot_id,
        base: b.base,
        retired: b.retired,
    }
}

fn builder_of(b: golden::Builder) -> CoreBuilder {
    CoreBuilder {
        machine: b.machine,
        kind: b.kind,
        base_template: b.base_template,
        setup_sha: b.setup_sha,
        created_at: b.created_at,
        first_life: b.first_life,
        size: b.size,
        import: b.import.map(ledger_of),
        parent_snapshot_id: b.parent_snapshot_id,
        base: b.base,
        retired: b.retired,
    }
}

fn delta_to(d: GoldenDelta) -> golden::GoldenDelta {
    golden::GoldenDelta { import: d.import, retired: d.retired, retired_on_image: d.retired_on_image }
}

fn vault_of(v: EngineVault) -> ImageVault {
    ImageVault { tar: v.tar, sha256: v.sha256, paths: v.paths, held: v.held }
}

/// A host-wired daemon deploy as the engine calls it: the machine alone, where the host's own closure also takes the
/// token, which it reads for itself.
#[allow(clippy::arc_with_non_send_sync)]
fn deploy_to(d: wsp_core::types::wiring::DeployDaemon) -> golden::DeployDaemon {
    Arc::new(move |machine: Arc<dyn Machine>| {
        let run = d(machine, None);
        async move { run.await.map_err(|e| EngineError::Other { message: e.message, code: None }) }.boxed_local()
    })
}

/// golden.ts:303 `Templates` as the engine's `promoteVersion` reads it, over the areas' own handle.
struct EngineTemplates(Rc<dyn Templates>);

#[async_trait::async_trait(?Send)]
impl golden::Templates for EngineTemplates {
    async fn promote(&self, snapshot_id: &str, name: &str) -> Result<String, EngineError> {
        self.0.promote(snapshot_id, name).await
    }
    async fn get(&self, id: &str) -> Result<TemplateRow, EngineError> {
        self.0.get(id).await
    }
    async fn list(&self) -> Result<Vec<TemplateRow>, EngineError> {
        self.0.list().await
    }
    async fn delete(&self, id: &str) -> Result<(), EngineError> {
        self.0.delete(id).await
    }
}

/// The areas' `Templates` over what the engine's `templatesOf` made of a backend.
struct AreaTemplates(Rc<dyn golden::Templates>);

#[async_trait::async_trait(?Send)]
impl Templates for AreaTemplates {
    async fn promote(&self, snapshot_id: &str, name: &str) -> Result<String, EngineError> {
        self.0.promote(snapshot_id, name).await
    }
    async fn get(&self, id: &str) -> Result<TemplateRow, EngineError> {
        self.0.get(id).await
    }
    async fn list(&self) -> Result<Vec<TemplateRow>, EngineError> {
        self.0.list().await
    }
    async fn delete(&self, id: &str) -> Result<(), EngineError> {
        self.0.delete(id).await
    }
}

impl Ctx {
    /// `claiming(purpose, run, at, place)` without the closure (machines.ts:183-230): the creates through the handle's
    /// backend stay in flight until the handle is dropped, which is when the TS run settles.
    fn claiming_over(&self, purpose: &str, at: Option<Arc<dyn MachineBackend>>, place: Option<&str>) -> Rc<dyn Claim> {
        let area = self.machines.get();
        let at = at.unwrap_or_else(|| self.backend.clone());
        let claim = area.begin_claim(purpose, at, place);
        Rc::new(Claiming { area, claim })
    }

    /// `ctx.placeAway(place)` (runtime.ts:853): the host holds a door, no place of its own answers for it and no link
    /// to it stands.
    fn builders_place_away(&self, place: &str) -> bool {
        self.wired_door().is_some_and(|door| self.places.backend(place).is_none() && door.link(place).is_none())
    }

    fn builders_ledger(&self) -> RuntimeRetryClock {
        RuntimeRetryClock(self.clock.clone())
    }
}

struct Claiming {
    area: Rc<MachinesArea>,
    claim: Claimed,
}

impl Claim for Claiming {
    fn backend(&self) -> Arc<dyn MachineBackend> {
        self.claim.backend()
    }
}

impl Drop for Claiming {
    fn drop(&mut self) {
        self.area.end_claim(&self.claim);
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::images::builders::BuildersCtx for Ctx {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn roads(&self) -> Rc<dyn GoldenRoads> {
        self.rc()
    }
    fn opts(&self) -> BuildersOpts {
        BuildersOpts {
            no_machines_line: self.opts.no_machines_line.clone(),
            golden_recipe: self.opts.golden_recipe.clone(),
            copy_recipe: self.opts.copy_recipe.clone().map(|recipe| -> CopyRecipe { Rc::new(move |image, force| recipe(image, force)) }),
            kill_confirm: self.opts.kill_confirm.map(kill_of),
            snapshot_retry_ms: self.opts.snapshot_retry_ms,
        }
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn door(&self) -> Option<Rc<dyn ImageDoor>> {
        self.wired_door().map(|_| -> Rc<dyn ImageDoor> { self.rc() })
    }
    fn place_door_of(&self) -> Result<Rc<dyn ImageDoor>, HostError> {
        match self.wired_door() {
            Some(_) => Ok(self.rc()),
            None => Err(HostError::new(no_kind_line("place"))),
        }
    }
    fn backend(&self) -> Arc<dyn MachineBackend> {
        self.backend.clone()
    }
    fn backend_of_kind_cloud(&self, place_id: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.kinds.get().backend_of_kind(WorkspaceKind::Cloud, place_id)
    }
    fn broker_ready(&self) -> bool {
        self.opts.broker.as_ref().is_some_and(|broker| broker.ready())
    }
    fn host_id(&self) -> String {
        self.defaults.host_id.clone()
    }
    fn pid(&self) -> u32 {
        std::process::id()
    }
    fn hostname(&self) -> String {
        self.hostname.clone()
    }
    fn owner(&self) -> String {
        self.state.with(|s| s.owner.clone())
    }
    fn closed(&self) -> bool {
        self.state.with(|s| s.closed)
    }
    fn beat(&self) -> Option<Cancel> {
        self.state.with(|s| s.beat.clone())
    }
    fn set_beat(&self, beat: Option<Cancel>) {
        self.state.with(|s| s.beat = beat);
    }
    fn set_ticking(&self, ticking: LocalBoxFuture<'static, ()>) {
        let run = ticking.shared();
        spawn_detached(run.clone());
        self.state.with(|s| s.ticking = Some(run));
    }
    fn warn(&self, line: &str) {
        console::warn(line);
    }
    fn emit_stage(&self, event: GoldenStageEvent) {
        let _ = self.bus.emit(EventUnion::GoldenStage(Sequenced { event, seq: None }));
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn builders(&self) -> Builders {
        self.builders.clone()
    }
    fn stage_at(&self) -> Rc<RefCell<IndexMap<String, StageAt>>> {
        self.stage_at.clone()
    }
    fn image_mark(&self) -> String {
        self.machines.get().image_mark()
    }
    async fn refresh_builders(&self) -> Result<(), HostError> {
        self.boot.get().refresh_builders().await
    }
    async fn gone_stop(&self, at: Arc<dyn MachineBackend>, machine: Arc<dyn Machine>) -> Result<(), HostError> {
        self.gone.stop(at, machine).await.map_err(|e| ImageError::Engine(e).into_host())
    }
    fn claiming(&self, purpose: &str, at: Option<Arc<dyn MachineBackend>>, place: Option<&str>) -> Rc<dyn Claim> {
        self.claiming_over(purpose, at, place)
    }
    fn observed(&self, machine: Arc<dyn Machine>) -> Arc<dyn Machine> {
        self.machines.get().observed(machine)
    }
    fn observing(&self, backend: Arc<dyn MachineBackend>) -> Arc<dyn MachineBackend> {
        self.machines.get().observing(backend)
    }
    async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError> {
        self.records.get().record_of(name).await
    }
    async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError> {
        self.records.get().copy_of(place, name).await
    }
    async fn put_copy(&self, place: &str, name: &str, manifest: &GoldenManifest) -> Result<(), HostError> {
        self.records.get().put_copy(place, name, manifest).await
    }
    async fn put_copy_recipe(&self, place: &str, name: &str, version: u32, digest: &RecipeDigest) -> Result<(), HostError> {
        self.records.get().put_copy_recipe(place, name, version, digest).await
    }
}

#[async_trait::async_trait(?Send)]
impl GoldenCtx for Ctx {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn roads(&self) -> Rc<dyn GoldenRoads> {
        self.rc()
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn door(&self) -> Option<Rc<dyn ImageDoor>> {
        self.wired_door().map(|_| -> Rc<dyn ImageDoor> { self.rc() })
    }
    fn backend(&self) -> Arc<dyn MachineBackend> {
        self.backend.clone()
    }
    fn kill_confirm(&self) -> Option<KillConfirm> {
        self.opts.kill_confirm.map(kill_of)
    }
    fn owner(&self) -> String {
        self.state.with(|s| s.owner.clone())
    }
    fn image_mark(&self) -> String {
        self.machines.get().image_mark()
    }
    fn import_mark(&self, imp: &GoldenImport) -> ImportMark {
        ImportMark { recipe_hash: imp.recipe_hash.clone(), recipe: imp.recipe.clone() }
    }
    fn warn(&self, line: &str) {
        console::warn(line);
    }
    fn builders(&self) -> Builders {
        self.builders.clone()
    }
    fn preparing(&self) -> Rc<RefCell<IndexMap<String, Preparing>>> {
        self.preparing.clone()
    }
    fn grace_timers(&self) -> Rc<RefCell<IndexMap<String, Cancel>>> {
        self.builders_area.get().grace_timers.clone()
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    async fn refresh_builders(&self) -> Result<(), HostError> {
        self.boot.get().refresh_builders().await
    }
    fn claiming(&self, purpose: &str, at: Option<Arc<dyn MachineBackend>>, place: Option<&str>) -> Rc<dyn Claim> {
        self.claiming_over(purpose, at, place)
    }
    async fn daemon_token_of(&self, machine: Arc<dyn Machine>) -> Result<Option<String>, HostError> {
        self.transcripts.get().daemon_token_of(machine, None).await
    }
    fn conflict(&self, message: &str) -> HostError {
        self.image.get().conflict(message)
    }
    async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError> {
        self.records.get().record_of(name).await
    }
    async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError> {
        self.records.get().copy_of(place, name).await
    }
    async fn put_copy(&self, place: &str, name: &str, manifest: &GoldenManifest) -> Result<(), HostError> {
        self.records.get().put_copy(place, name, manifest).await
    }
    async fn copy_recipe_of(&self, place: &str, name: &str, version: u32) -> Result<Option<RecipeDigest>, HostError> {
        self.records.get().copy_recipe_of(place, name, version).await
    }
    async fn drop_copy_recipe(&self, place: &str, name: &str, version: u32) -> Result<(), HostError> {
        self.records.get().drop_copy_recipe(place, name, version).await
    }
    async fn recorded_images(&self) -> Result<IndexSet<String>, HostError> {
        self.records.get().recorded_images().await
    }
    fn forked_from(&self, snapshot_id: &str) -> Vec<String> {
        self.records.get().forked_from(snapshot_id)
    }
    async fn image_place(&self, name: &str) -> Result<String, HostError> {
        self.builders_area.get().image_place(name).await
    }
    async fn place_at(&self, word: &str) -> Result<PlaceAt, HostError> {
        self.builders_area.get().place_at(word).await
    }
    fn backend_at(&self, place: &str) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.builders_area.get().backend_at(place)
    }
    fn place_name(&self, place: &str) -> String {
        self.builders_area.get().place_name(place)
    }
    fn place_kind_of(&self, place: &str) -> PlaceKind {
        self.builders_area.get().place_kind_of(place)
    }
    async fn build_places(&self) -> Result<Vec<BuildPlace>, HostError> {
        self.builders_area.get().build_places().await
    }
    fn recipe_or_throw(&self, named: Option<GoldenRecipe>) -> Result<GoldenRecipe, HostError> {
        self.builders_area.get().recipe_or_throw(named)
    }
    fn builder_labels(&self, extra: Option<&IndexMap<String, String>>) -> IndexMap<String, String> {
        self.builders_area.get().builder_labels(extra)
    }
    fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> StageFn {
        self.builders_area.get().stage_of(name, on, named)
    }
    fn recording_creates(
        &self,
        b: Arc<dyn MachineBackend>,
        name: &str,
        imp: Option<ImportMark>,
        made: Rc<dyn Fn(BuilderRef)>,
        stop: Option<CreateStop>,
        place: &str,
    ) -> Arc<dyn MachineBackend> {
        self.builders_area.get().recording_creates(b, name, imp, made, stop, place)
    }
    async fn settle_builder(
        &self,
        name: &str,
        builder: CoreBuilder,
        placeholder: Option<BuilderRef>,
        place: &str,
    ) -> Result<BuilderRef, HostError> {
        self.builders_area.get().settle_builder(name, builder, placeholder, place).await
    }
    async fn seal_entry(
        &self,
        entry: &BuilderRef,
        keep: bool,
        logins: Option<Vec<GoldenLogin>>,
        copy: Option<SealedImage>,
    ) -> ImageResult<SealResult> {
        self.builders_area.get().seal_entry(entry, keep, logins, copy).await
    }
    async fn hold(&self, b: &BuilderRef) -> Result<(), HostError> {
        self.builders_area.get().hold(b).await
    }
    async fn forget_builder(&self, id: &str) -> Result<(), HostError> {
        self.builders_area.get().forget_builder(id).await
    }
    async fn forget_if_gone(&self, entry: &BuilderRef, at: Arc<dyn MachineBackend>) -> Result<(), HostError> {
        self.builders_area.get().forget_if_gone(entry, at).await
    }
    fn builder_view(&self, entry: &BuilderRef) -> GoldenBuilderView {
        let held = entry.borrow();
        self.builders_area.get().builder_view(&held.record, &held)
    }
    fn refuse_untouchable(&self, entry: &BuilderRef) -> Result<(), HostError> {
        self.builders_area.get().refuse_untouchable(entry)
    }
    async fn expire_grace(&self) -> ImageResult<Expired> {
        self.builders_area.get().expire_grace().await
    }
    fn in_window(&self, sealed_at: &str) -> bool {
        self.builders_area.get().in_window(sealed_at)
    }
    async fn drop_image(&self, v: &GoldenVersion, at: Option<Arc<dyn MachineBackend>>) -> ImageResult<()> {
        self.builders_area.get().drop_image(v, at).await
    }
}

#[async_trait::async_trait(?Send)]
impl ImageCtx for Ctx {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn roads(&self) -> Rc<dyn GoldenRoads> {
        self.rc()
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn door(&self) -> Option<Rc<dyn ImageDoor>> {
        self.wired_door().map(|_| -> Rc<dyn ImageDoor> { self.rc() })
    }
    fn kill_confirm(&self) -> Option<KillConfirm> {
        self.opts.kill_confirm.map(kill_of)
    }
    fn builders(&self) -> Builders {
        self.builders.clone()
    }
    fn copy_builds(&self) -> Rc<RefCell<IndexMap<String, Joined<SealedImageBuilt>>>> {
        self.copy_builds.clone()
    }
    fn copy_row(&self, place: &str) -> Option<Rc<CopyRow>> {
        self.copy_rows.borrow().get(place).cloned()
    }
    fn row_says_failure(&self, place: &str, e: &ImageError) -> bool {
        let said = e.clone().into_host();
        !(wsp_machines::places::types::is_place_forks_nowhere(&said)
            || wsp_machines::places::types::is_place_provisioning(&said)
            || e.is_place_absent()
            || said.kind.as_ref().is_some_and(|kind| kind.as_str() == "absent")
            || self.builders_place_away(place))
    }
    fn place_away(&self, place: &str) -> bool {
        self.builders_place_away(place)
    }
    fn frame_stopped(&self, place: &str, name: &str, why: Stopped) {
        let said = match why {
            Stopped::Line(line) => line,
            Stopped::Error(e) => e.message(),
        };
        let stage = self.builders_area.get().stage_of(name, Some(place), true);
        stage(GoldenStage::Failed, Some(said), None, None);
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.booted().await
    }
    async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError> {
        self.records.get().record_of(name).await
    }
    async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError> {
        self.records.get().copy_of(place, name).await
    }
    async fn copy_recipe_of(&self, place: &str, name: &str, version: u32) -> Result<Option<RecipeDigest>, HostError> {
        self.records.get().copy_recipe_of(place, name, version).await
    }
    async fn put_copy_recipe(&self, place: &str, name: &str, version: u32, digest: &RecipeDigest) -> Result<(), HostError> {
        self.records.get().put_copy_recipe(place, name, version, digest).await
    }
    async fn image_of(&self, snapshot_id: &str) -> Result<ImageOf, HostError> {
        self.records.get().image_of(snapshot_id).await
    }
    async fn place_at(&self, word: &str) -> Result<PlaceAt, HostError> {
        self.builders_area.get().place_at(word).await
    }
    fn place_name(&self, place: &str) -> String {
        self.builders_area.get().place_name(place)
    }
    fn place_kind_of(&self, place: &str) -> PlaceKind {
        self.builders_area.get().place_kind_of(place)
    }
    async fn image_place(&self, name: &str) -> Result<String, HostError> {
        self.builders_area.get().image_place(name).await
    }
    fn copy_recipe_or_throw(&self) -> Result<CopyRecipe, HostError> {
        self.builders_area.get().copy_recipe_or_throw()
    }
    fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> StageFn {
        self.builders_area.get().stage_of(name, on, named)
    }
    async fn forget_if_gone(&self, entry: &BuilderRef, at: Arc<dyn MachineBackend>) -> Result<(), HostError> {
        self.builders_area.get().forget_if_gone(entry, at).await
    }
    async fn seal_entry(
        &self,
        entry: &BuilderRef,
        keep: bool,
        logins: Option<Vec<GoldenLogin>>,
        copy: Option<SealedImage>,
    ) -> ImageResult<SealResult> {
        self.builders_area.get().seal_entry(entry, keep, logins, copy).await
    }
    async fn golden_prepare(&self, ask: wsp_core::types::api::PrepareAsk) -> ImageResult<GoldenBuilderView> {
        self.golden.get().prepare(ask).await
    }
}

#[async_trait::async_trait(?Send)]
impl GoldenRoads for Ctx {
    /// golden.ts:1429. The request `golden.build` hands is wsp-core's `BuildGoldenOptions`, an empty marker that
    /// carries neither the setup, the smoke nor the size the engine's road runs, so there is nothing to build from.
    async fn build_golden(&self, ask: BuildGoldenAsk) -> Result<BuiltGolden, EngineError> {
        let _ = ask;
        Err(EngineError::Other {
            message: "this build carries no setup or smoke to run: the request's options are not wired through to the engine yet"
                .to_string(),
            code: None,
        })
    }
    async fn prepare_builder(&self, ask: PrepareBuilderAsk) -> Result<CoreBuilder, ImageError> {
        let opts = golden::PrepareBuilderOptions {
            backend: ask.backend,
            size: size_to(ask.size),
            kind: ask.kind,
            base_template: ask.base_template,
            deploy_daemon: ask.deploy_daemon.map(deploy_to),
            setup: ask.setup,
            import: ask.import,
            fetch: None,
            on_stage: Some(stage_to(ask.on_stage)),
        };
        golden::prepare_builder(opts, &self.builders_ledger()).await.map(builder_of).map_err(builders_image_error)
    }
    async fn seal_golden(&self, builder: CoreBuilder, ask: SealGoldenAsk) -> Result<SealResult, ImageError> {
        let opts = golden::SealGoldenOptions {
            backend: ask.backend,
            size: size_to(ask.size),
            smoke: ask.smoke,
            smoke_timeout_ms: None,
            manifest: ask.manifest,
            on_stage: Some(stage_to(ask.on_stage)),
            kill_confirm: ask.kill_confirm.map(kill_to),
            logins: ask.logins,
            keep_builder: ask.keep_builder,
            snapshot_retry_ms: ask.snapshot_retry_ms,
            name: Some(ask.name),
            host_id: ask.host_id,
            template_ready_ms: None,
            template_poll_ms: None,
            refuse: ask.refuse,
            vault_paths: ask.vault_paths,
            fetch: None,
        };
        let sealed = golden::seal_golden(builder_to(builder), opts, &self.builders_ledger()).await.map_err(builders_image_error)?;
        Ok(SealResult {
            manifest: sealed.manifest,
            version: sealed.version,
            builder_kept: sealed.builder_kept,
            vault: sealed.vault.map(vault_of),
        })
    }
    async fn upgrade_builder(&self, ask: UpgradeBuilderAsk) -> Result<CoreBuilder, ImageError> {
        let opts = golden::UpgradeBuilderOptions {
            backend: ask.backend,
            size: size_to(ask.size),
            head: ask.head,
            delta: delta_to(ask.delta),
            setup: ask.setup,
            fetch: None,
            on_stage: Some(stage_to(ask.on_stage)),
        };
        golden::upgrade_builder(opts, &self.builders_ledger()).await.map(builder_of).map_err(builders_image_error)
    }
    async fn apply_golden_import(&self, machine: Arc<dyn Machine>, ask: ApplyImportAsk) -> Result<Applied, EngineError> {
        let opts = golden::ApplyImportOptions {
            import: Some(ask.import),
            setup: ask.setup,
            base: None,
            ledger: ask.ledger.map(ledger_to),
            retired: None,
            fetch: None,
            on_stage: Some(stage_to(ask.on_stage)),
        };
        let applied = golden::apply_golden_import(&machine, opts, &self.builders_ledger()).await?;
        Ok(Applied { ledger: ledger_of(applied.ledger) })
    }
    async fn apply_delta(&self, machine: Arc<dyn Machine>, delta: GoldenDelta, ask: ApplyDeltaAsk) -> Result<Applied, EngineError> {
        let opts = golden::ApplyDeltaOptions {
            setup: ask.setup,
            previous_smoke: ask.previous_smoke,
            previous_missing: ask.previous_missing,
            previous_left_behind: ask.previous_left_behind,
            previous_base: ask.previous_base,
            fetch: None,
            on_stage: Some(stage_to(ask.on_stage)),
        };
        let applied = golden::apply_delta(&machine, delta_to(delta), opts, &self.builders_ledger()).await?;
        Ok(Applied { ledger: ledger_of(applied.ledger) })
    }
    fn next_setup_sha(&self, previous: &str, setup: &str, import: &GoldenImport) -> String {
        golden::next_setup_sha(previous, setup, import)
    }
    async fn kill_until_gone(
        &self,
        at: Arc<dyn MachineBackend>,
        machine: Arc<dyn Machine>,
        confirm: Option<KillConfirm>,
    ) -> Result<(), EngineError> {
        kill_until_gone(&*at, &*machine, confirm.map(kill_to).unwrap_or_default(), &self.builders_ledger()).await
    }
    async fn read_gone(&self, at: Arc<dyn MachineBackend>, id: &str) -> Result<GoneReading, EngineError> {
        Ok(match read_gone(&*at, id).await? {
            MachineState::Gone => GoneReading::Gone,
            state => GoneReading::Standing(machine_state_word(state).to_string()),
        })
    }
    async fn drop_template(&self, at: Arc<dyn MachineBackend>, template_id: Option<&str>) -> Result<(), EngineError> {
        wsp_engine::drop_template::drop_template(&*at, template_id).await
    }
    fn templates_of(&self, at: Arc<dyn MachineBackend>) -> Option<Rc<dyn Templates>> {
        golden::templates_of(at).map(|templates| -> Rc<dyn Templates> { Rc::new(AreaTemplates(templates)) })
    }
    async fn promote_version(&self, templates: Rc<dyn Templates>, snapshot_id: &str, name: &str) -> Result<Promoted, EngineError> {
        let promoted = golden::promote_version(
            &EngineTemplates(templates),
            snapshot_id,
            name,
            &golden::TemplateWait::default(),
            &self.builders_ledger(),
        )
        .await?;
        Ok(Promoted { template_id: promoted.template_id, sharing: promoted.sharing })
    }
    fn rollback(&self, manifest: &GoldenManifest, version: u32) -> Result<GoldenManifest, EngineError> {
        golden::rollback(manifest, version)
    }
    async fn refresh_preview_token(
        &self,
        machine: Arc<dyn Machine>,
        port: u16,
        held: Option<PreviewReach>,
    ) -> Result<PreviewReach, EngineError> {
        wsp_engine::preview::refresh_preview_token(&*machine, port, held.as_ref(), self.clock.now()).await
    }
    async fn import_image_vault(&self, machine: Arc<dyn Machine>, tar: Vec<u8>) -> Result<(), EngineError> {
        wsp_engine::image_vault::import_image_vault(machine, &tar, &Default::default()).await
    }
    fn refuse_foreign_members(&self, road: &str, tar: &[u8], held: &[String]) -> Result<(), String> {
        let road = if road == "import" { VaultRoad::Import } else { VaultRoad::Seal };
        wsp_engine::image_vault::refuse_foreign_members(road, tar, held).map_err(|e| e.to_string())
    }
    fn image_hash(&self, recipe_hash: &str, vault_sha256: Option<&str>) -> String {
        wsp_engine::image_vault::image_hash(recipe_hash, vault_sha256)
    }
}

/// golden.ts:291 `templatesOf(backend)`: the runtime's own backend's four template calls.
#[async_trait::async_trait(?Send)]
impl Templates for Ctx {
    async fn promote(&self, snapshot_id: &str, name: &str) -> Result<String, EngineError> {
        self.backend.promote_snapshot(snapshot_id, name).await
    }
    async fn get(&self, id: &str) -> Result<TemplateRow, EngineError> {
        self.backend.get_template(id).await
    }
    async fn list(&self) -> Result<Vec<TemplateRow>, EngineError> {
        self.backend.list_templates().await
    }
    async fn delete(&self, id: &str) -> Result<(), EngineError> {
        self.backend.delete_template(id).await
    }
}
