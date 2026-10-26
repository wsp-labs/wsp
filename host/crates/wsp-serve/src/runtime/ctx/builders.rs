// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use std::rc::Rc;
use std::sync::Arc;

use wsp_engine::machine::MachineBackend;
use wsp_machines::images::builders::Claim;
use wsp_machines::machines::machines::create::Claimed;
use wsp_machines::machines::machines::MachinesArea;

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_machines::images::builders::BuildersCtx for Ctx {
    fn store(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Store> {
        todo!()
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Clock> {
        todo!()
    }
    fn roads(&self) -> std::rc::Rc<dyn wsp_machines::images::engine::GoldenRoads> {
        todo!()
    }
    fn opts(&self) -> wsp_machines::images::builders::BuildersOpts {
        todo!()
    }
    fn places(&self) -> std::rc::Rc<dyn wsp_core::types::wiring::PlaceBackends> {
        todo!()
    }
    fn door(&self) -> Option<std::rc::Rc<dyn wsp_machines::images::builders::ImageDoor>> {
        todo!()
    }
    fn place_door_of(&self) -> Result<std::rc::Rc<dyn wsp_machines::images::builders::ImageDoor>, wsp_words::exit::HostError> {
        todo!()
    }
    fn backend(&self) -> std::sync::Arc<dyn wsp_engine::machine::MachineBackend> {
        todo!()
    }
    fn backend_of_kind_cloud(
        &self,
        place_id: Option<&str>,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    fn broker_ready(&self) -> bool {
        todo!()
    }
    fn host_id(&self) -> String {
        todo!()
    }
    fn pid(&self) -> u32 {
        todo!()
    }
    fn hostname(&self) -> String {
        todo!()
    }
    fn owner(&self) -> String {
        todo!()
    }
    fn closed(&self) -> bool {
        todo!()
    }
    fn beat(&self) -> Option<wsp_core::clock::Cancel> {
        todo!()
    }
    fn set_beat(&self, beat: Option<wsp_core::clock::Cancel>) {
        todo!()
    }
    fn set_ticking(&self, ticking: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn emit_stage(&self, event: wsp_wire::views::golden_image::GoldenStageEvent) {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn builders(&self) -> wsp_machines::images::Builders {
        todo!()
    }
    fn stage_at(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::images::builders::StageAt>>> {
        todo!()
    }
    fn image_mark(&self) -> String {
        todo!()
    }
    async fn refresh_builders(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn gone_stop(
        &self,
        at: std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn claiming(
        &self,
        purpose: &str,
        at: Option<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>>,
        place: Option<&str>,
    ) -> std::rc::Rc<dyn wsp_machines::images::builders::Claim> {
        self.claiming_over(purpose, at, place)
    }
    fn observed(&self, machine: std::sync::Arc<dyn wsp_engine::machine::Machine>) -> std::sync::Arc<dyn wsp_engine::machine::Machine> {
        todo!()
    }
    fn observing(
        &self,
        backend: std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
    ) -> std::sync::Arc<dyn wsp_engine::machine::MachineBackend> {
        todo!()
    }
    async fn record_of(&self, name: &str) -> Result<Option<wsp_wire::views::golden_image::SealedImage>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn copy_of(
        &self,
        place: &str,
        name: &str,
    ) -> Result<Option<wsp_wire::views::golden_image::GoldenManifest>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn put_copy(
        &self,
        place: &str,
        name: &str,
        manifest: &wsp_wire::views::golden_image::GoldenManifest,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn put_copy_recipe(
        &self,
        place: &str,
        name: &str,
        version: u32,
        digest: &wsp_wire::views::golden_image::RecipeDigest,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::images::golden::GoldenCtx for Ctx {
    fn store(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Store> {
        todo!()
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Clock> {
        todo!()
    }
    fn roads(&self) -> std::rc::Rc<dyn wsp_machines::images::engine::GoldenRoads> {
        todo!()
    }
    fn places(&self) -> std::rc::Rc<dyn wsp_core::types::wiring::PlaceBackends> {
        todo!()
    }
    fn door(&self) -> Option<std::rc::Rc<dyn wsp_machines::images::builders::ImageDoor>> {
        todo!()
    }
    fn backend(&self) -> std::sync::Arc<dyn wsp_engine::machine::MachineBackend> {
        todo!()
    }
    fn kill_confirm(&self) -> Option<wsp_machines::images::engine::KillConfirm> {
        todo!()
    }
    fn owner(&self) -> String {
        todo!()
    }
    fn image_mark(&self) -> String {
        todo!()
    }
    fn import_mark(&self, imp: &wsp_engine::golden_seam::GoldenImport) -> wsp_machines::images::builders::life::ImportMark {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn builders(&self) -> wsp_machines::images::Builders {
        todo!()
    }
    fn preparing(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::images::golden::Preparing>>> {
        todo!()
    }
    fn grace_timers(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_core::clock::Cancel>>> {
        todo!()
    }
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn refresh_builders(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn claiming(
        &self,
        purpose: &str,
        at: Option<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>>,
        place: Option<&str>,
    ) -> std::rc::Rc<dyn wsp_machines::images::builders::Claim> {
        self.claiming_over(purpose, at, place)
    }
    async fn daemon_token_of(
        &self,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn conflict(&self, message: &str) -> wsp_words::exit::HostError {
        todo!()
    }
    async fn record_of(&self, name: &str) -> Result<Option<wsp_wire::views::golden_image::SealedImage>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn copy_of(
        &self,
        place: &str,
        name: &str,
    ) -> Result<Option<wsp_wire::views::golden_image::GoldenManifest>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn put_copy(
        &self,
        place: &str,
        name: &str,
        manifest: &wsp_wire::views::golden_image::GoldenManifest,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn copy_recipe_of(
        &self,
        place: &str,
        name: &str,
        version: u32,
    ) -> Result<Option<wsp_wire::views::golden_image::RecipeDigest>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn drop_copy_recipe(&self, place: &str, name: &str, version: u32) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn recorded_images(&self) -> Result<indexmap::IndexSet<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn forked_from(&self, snapshot_id: &str) -> Vec<String> {
        todo!()
    }
    async fn image_place(&self, name: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn place_at(&self, word: &str) -> Result<wsp_machines::images::builders::places::PlaceAt, wsp_words::exit::HostError> {
        todo!()
    }
    fn backend_at(&self, place: &str) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    fn place_name(&self, place: &str) -> String {
        todo!()
    }
    fn place_kind_of(&self, place: &str) -> wsp_wire::views::place_kind::PlaceKind {
        todo!()
    }
    async fn build_places(&self) -> Result<Vec<wsp_machines::images::builders::BuildPlace>, wsp_words::exit::HostError> {
        todo!()
    }
    fn recipe_or_throw(
        &self,
        named: Option<wsp_core::types::wiring::GoldenRecipe>,
    ) -> Result<wsp_core::types::wiring::GoldenRecipe, wsp_words::exit::HostError> {
        todo!()
    }
    fn builder_labels(&self, extra: Option<&indexmap::IndexMap<String, String>>) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> wsp_machines::images::engine::StageFn {
        todo!()
    }
    fn recording_creates(
        &self,
        b: std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
        name: &str,
        imp: Option<wsp_machines::images::builders::life::ImportMark>,
        made: std::rc::Rc<dyn Fn(wsp_machines::images::BuilderRef)>,
        stop: Option<wsp_machines::images::builders::life::CreateStop>,
        place: &str,
    ) -> std::sync::Arc<dyn wsp_engine::machine::MachineBackend> {
        todo!()
    }
    async fn settle_builder(
        &self,
        name: &str,
        builder: wsp_machines::images::engine::EngineBuilder,
        placeholder: Option<wsp_machines::images::BuilderRef>,
        place: &str,
    ) -> Result<wsp_machines::images::BuilderRef, wsp_words::exit::HostError> {
        todo!()
    }
    async fn seal_entry(
        &self,
        entry: &wsp_machines::images::BuilderRef,
        keep: bool,
        logins: Option<Vec<wsp_wire::views::golden_image::GoldenLogin>>,
        copy: Option<wsp_wire::views::golden_image::SealedImage>,
    ) -> wsp_machines::images::ImageResult<wsp_machines::images::engine::SealResult> {
        todo!()
    }
    async fn hold(&self, b: &wsp_machines::images::BuilderRef) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn forget_builder(&self, id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn forget_if_gone(
        &self,
        entry: &wsp_machines::images::BuilderRef,
        at: std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn builder_view(&self, entry: &wsp_machines::images::BuilderRef) -> wsp_wire::views::golden_image::GoldenBuilderView {
        todo!()
    }
    fn refuse_untouchable(&self, entry: &wsp_machines::images::BuilderRef) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn expire_grace(&self) -> wsp_machines::images::ImageResult<wsp_machines::images::builders::life::Expired> {
        todo!()
    }
    fn in_window(&self, sealed_at: &str) -> bool {
        todo!()
    }
    async fn drop_image(
        &self,
        v: &wsp_wire::views::golden_image::GoldenVersion,
        at: Option<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>>,
    ) -> wsp_machines::images::ImageResult<()> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::images::image::ImageCtx for Ctx {
    fn store(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Store> {
        todo!()
    }
    fn roads(&self) -> std::rc::Rc<dyn wsp_machines::images::engine::GoldenRoads> {
        todo!()
    }
    fn places(&self) -> std::rc::Rc<dyn wsp_core::types::wiring::PlaceBackends> {
        todo!()
    }
    fn door(&self) -> Option<std::rc::Rc<dyn wsp_machines::images::builders::ImageDoor>> {
        todo!()
    }
    fn kill_confirm(&self) -> Option<wsp_machines::images::engine::KillConfirm> {
        todo!()
    }
    fn builders(&self) -> wsp_machines::images::Builders {
        todo!()
    }
    fn copy_builds(
        &self,
    ) -> std::rc::Rc<
        std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::images::Joined<wsp_wire::views::golden_image::SealedImageBuilt>>>,
    > {
        todo!()
    }
    fn copy_row(&self, place: &str) -> Option<std::rc::Rc<wsp_machines::images::image::CopyRow>> {
        todo!()
    }
    fn row_says_failure(&self, place: &str, e: &wsp_machines::images::ImageError) -> bool {
        todo!()
    }
    fn place_away(&self, place: &str) -> bool {
        todo!()
    }
    fn frame_stopped(&self, place: &str, name: &str, why: wsp_machines::images::image::Stopped) {
        todo!()
    }
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn record_of(&self, name: &str) -> Result<Option<wsp_wire::views::golden_image::SealedImage>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn copy_of(
        &self,
        place: &str,
        name: &str,
    ) -> Result<Option<wsp_wire::views::golden_image::GoldenManifest>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn copy_recipe_of(
        &self,
        place: &str,
        name: &str,
        version: u32,
    ) -> Result<Option<wsp_wire::views::golden_image::RecipeDigest>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn put_copy_recipe(
        &self,
        place: &str,
        name: &str,
        version: u32,
        digest: &wsp_wire::views::golden_image::RecipeDigest,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn image_of(&self, snapshot_id: &str) -> Result<wsp_machines::images::records::ImageOf, wsp_words::exit::HostError> {
        todo!()
    }
    async fn place_at(&self, word: &str) -> Result<wsp_machines::images::builders::places::PlaceAt, wsp_words::exit::HostError> {
        todo!()
    }
    fn place_name(&self, place: &str) -> String {
        todo!()
    }
    fn place_kind_of(&self, place: &str) -> wsp_wire::views::place_kind::PlaceKind {
        todo!()
    }
    async fn image_place(&self, name: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn copy_recipe_or_throw(&self) -> Result<wsp_machines::images::builders::CopyRecipe, wsp_words::exit::HostError> {
        todo!()
    }
    fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> wsp_machines::images::engine::StageFn {
        todo!()
    }
    async fn forget_if_gone(
        &self,
        entry: &wsp_machines::images::BuilderRef,
        at: std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn seal_entry(
        &self,
        entry: &wsp_machines::images::BuilderRef,
        keep: bool,
        logins: Option<Vec<wsp_wire::views::golden_image::GoldenLogin>>,
        copy: Option<wsp_wire::views::golden_image::SealedImage>,
    ) -> wsp_machines::images::ImageResult<wsp_machines::images::engine::SealResult> {
        todo!()
    }
    async fn golden_prepare(
        &self,
        ask: wsp_core::types::api::PrepareAsk,
    ) -> wsp_machines::images::ImageResult<wsp_wire::views::golden_image::GoldenBuilderView> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::images::engine::GoldenRoads for Ctx {
    async fn build_golden(
        &self,
        ask: wsp_machines::images::engine::BuildGoldenAsk,
    ) -> Result<wsp_machines::images::engine::BuiltGolden, wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn prepare_builder(
        &self,
        ask: wsp_machines::images::engine::PrepareBuilderAsk,
    ) -> Result<wsp_core::types::pending::Builder, wsp_machines::images::ImageError> {
        todo!()
    }
    async fn seal_golden(
        &self,
        builder: wsp_core::types::pending::Builder,
        ask: wsp_machines::images::engine::SealGoldenAsk,
    ) -> Result<wsp_machines::images::engine::SealResult, wsp_machines::images::ImageError> {
        todo!()
    }
    async fn upgrade_builder(
        &self,
        ask: wsp_machines::images::engine::UpgradeBuilderAsk,
    ) -> Result<wsp_core::types::pending::Builder, wsp_machines::images::ImageError> {
        todo!()
    }
    async fn apply_golden_import(
        &self,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
        ask: wsp_machines::images::engine::ApplyImportAsk,
    ) -> Result<wsp_machines::images::engine::Applied, wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn apply_delta(
        &self,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
        delta: wsp_machines::images::engine::GoldenDelta,
        ask: wsp_machines::images::engine::ApplyDeltaAsk,
    ) -> Result<wsp_machines::images::engine::Applied, wsp_engine::errors::EngineError> {
        todo!()
    }
    fn next_setup_sha(&self, previous: &str, setup: &str, import: &wsp_engine::golden_seam::GoldenImport) -> String {
        todo!()
    }
    async fn kill_until_gone(
        &self,
        at: std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
        confirm: Option<wsp_machines::images::engine::KillConfirm>,
    ) -> Result<(), wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn read_gone(
        &self,
        at: std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
        id: &str,
    ) -> Result<wsp_machines::images::engine::GoneReading, wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn drop_template(
        &self,
        at: std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
        template_id: Option<&str>,
    ) -> Result<(), wsp_engine::errors::EngineError> {
        todo!()
    }
    fn templates_of(
        &self,
        at: std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
    ) -> Option<std::rc::Rc<dyn wsp_machines::images::engine::Templates>> {
        todo!()
    }
    async fn promote_version(
        &self,
        templates: std::rc::Rc<dyn wsp_machines::images::engine::Templates>,
        snapshot_id: &str,
        name: &str,
    ) -> Result<wsp_machines::images::engine::Promoted, wsp_engine::errors::EngineError> {
        todo!()
    }
    fn rollback(
        &self,
        manifest: &wsp_wire::views::golden_image::GoldenManifest,
        version: u32,
    ) -> Result<wsp_wire::views::golden_image::GoldenManifest, wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn refresh_preview_token(
        &self,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
        port: u16,
        held: Option<wsp_wire::wire::machine_link::PreviewReach>,
    ) -> Result<wsp_wire::wire::machine_link::PreviewReach, wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn import_image_vault(
        &self,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
        tar: Vec<u8>,
    ) -> Result<(), wsp_engine::errors::EngineError> {
        todo!()
    }
    fn refuse_foreign_members(&self, road: &str, tar: &[u8], held: &[String]) -> Result<(), String> {
        todo!()
    }
    fn image_hash(&self, recipe_hash: &str, vault_sha256: Option<&str>) -> String {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::images::engine::Templates for Ctx {
    async fn promote(&self, snapshot_id: &str, name: &str) -> Result<String, wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn get(&self, id: &str) -> Result<wsp_wire::wire::machine_link::TemplateRow, wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn list(&self) -> Result<Vec<wsp_wire::wire::machine_link::TemplateRow>, wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn delete(&self, id: &str) -> Result<(), wsp_engine::errors::EngineError> {
        todo!()
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
