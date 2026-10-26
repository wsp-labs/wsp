// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::create::CreateRuntime for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Clock> {
        todo!()
    }
    fn ids(&self) -> std::rc::Rc<dyn wsp_machines::machines::machines::Ids> {
        todo!()
    }
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn live(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::machines::machines::Entry>>> {
        todo!()
    }
    fn wired(&self) -> String {
        todo!()
    }
    fn place_door(&self) -> Option<std::rc::Rc<dyn wsp_machines::places::types::PlaceDoor>> {
        todo!()
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn place_refuses(&self, place_id: Option<&str>) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn landing_backend(
        &self,
        place_id: Option<&str>,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    fn landing_kind(&self, computer: &str, at: &std::sync::Arc<dyn wsp_engine::machine::MachineBackend>) -> String {
        todo!()
    }
    async fn landing_deps(&self, computer: &str) -> Result<serde_json::Value, wsp_words::exit::HostError> {
        todo!()
    }
    fn project_landing(
        &self,
        kind: &str,
    ) -> Result<std::rc::Rc<dyn wsp_machines::machines::create::LandingRoad>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn remember_project(&self, project: wsp_wire::views::workspace::ProjectView) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn project_held(&self, project_id: &str) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn keeps_images(&self, at: &std::sync::Arc<dyn wsp_engine::machine::MachineBackend>) -> bool {
        todo!()
    }
    async fn copy_for_fork(
        &self,
        golden: &str,
        place_id: Option<&str>,
        building: &dyn Fn(&str, f64),
    ) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn image_head(&self) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn image_size(&self, golden: &str) -> Result<Option<wsp_wire::wire::capabilities::WorkspaceSize>, wsp_words::exit::HostError> {
        todo!()
    }
    fn brokered_start(&self, provider: bool) -> bool {
        todo!()
    }
    async fn named_of_record(
        &self,
        ask: wsp_machines::machines::create::RecordBranchAsk,
    ) -> Result<wsp_wire::views::workspace::BranchOfRecord, wsp_words::exit::HostError> {
        todo!()
    }
    fn place_name(&self, place: &str) -> String {
        todo!()
    }
    fn place_door_name(&self, place: &str) -> String {
        todo!()
    }
    fn spawn_at(&self, place: &str) -> wsp_wire::views::workspace_more::WorkspaceAgents {
        todo!()
    }
    fn tree_of(&self, spawned: Option<&wsp_wire::views::workspace_more::ThreadScope>) -> serde_json::Map<String, serde_json::Value> {
        todo!()
    }
    fn attach(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> Result<wsp_machines::machines::machines::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    async fn fork(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        bind: &dyn for<'m, 'r> Fn(
            &'m std::sync::Arc<dyn wsp_engine::machine::Machine>,
            &'r wsp_core::types::wiring::WorkspaceRecord,
        ) -> Result<(), wsp_words::exit::HostError>,
        report: wsp_core::types::wiring::StageReport,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::Machine>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn refresh_builders(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn builders(&self) -> Vec<wsp_machines::machines::create::KeptBuilder> {
        todo!()
    }
    fn cancel_grace(&self, builder_id: &str) {
        todo!()
    }
    async fn kill_builder(&self, builder_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn forget_builder(&self, builder_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn holds_slot(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> bool {
        todo!()
    }
    fn served_by_its_computer(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn daemon_answers_ms(&self, entry: &wsp_machines::machines::machines::Entry) -> Result<u64, wsp_words::exit::HostError> {
        todo!()
    }
    async fn daemon_reach(&self, entry: &wsp_machines::machines::machines::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn ping_daemon(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn sync_daemon(&self, entry: &wsp_machines::machines::machines::Entry) {
        todo!()
    }
    async fn read_memory(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        machine: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
        sizes: &[wsp_wire::wire::capabilities::WorkspaceSize],
    ) -> bool {
        todo!()
    }
    fn module_of(&self, kind: &str) -> Result<std::rc::Rc<dyn wsp_core::types::internal::KindModule>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn write_daemon_roots(&self, entry: &wsp_machines::machines::machines::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn mark_used(&self, place_id: Option<&str>) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn view(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_wire::views::workspace_more::WorkspaceView {
        todo!()
    }
    fn ref_of(&self, project: &wsp_wire::views::workspace::ProjectView) -> wsp_wire::views::workspace_more::ProjectRef {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::sub_spawn::SubSpawnCtx for Ctx {
    fn live(&self, id: &str) -> Option<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn remove_live(&self, id: &str) {
        todo!()
    }
    fn is_absent_machine(&self, entry: &wsp_machines::machines::machines::Entry) -> bool {
        todo!()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        todo!()
    }
    fn push_head(&self, thread_id: &str) {
        todo!()
    }
    async fn local_machine(&self) -> wsp_machines::machines::area_error::AreaResult<std::sync::Arc<dyn wsp_engine::machine::Machine>> {
        todo!()
    }
    fn provider_backend(&self, id: &str) -> Option<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>> {
        todo!()
    }
    async fn computer_rows(&self) -> wsp_machines::machines::area_error::AreaResult<Vec<wsp_machines::machines::sub_spawn::ProviderRow>> {
        todo!()
    }
    async fn landing_place(&self, row_id: &str) -> wsp_machines::machines::area_error::AreaResult<Option<String>> {
        todo!()
    }
    fn place_settings(&self, place: &str) -> Option<serde_json::Map<String, serde_json::Value>> {
        todo!()
    }
    fn delete_after_ms(&self, place: &str) -> Option<f64> {
        todo!()
    }
    fn place_name(&self, computer: &str) -> String {
        todo!()
    }
    fn place_id_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn backend_for(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    fn name_given(&self, name: &str) -> String {
        todo!()
    }
    fn spawn_guard(&self, origin: Option<&wsp_wire::views::workspace::Caller>) -> Result<Box<dyn FnOnce()>, wsp_words::exit::HostError> {
        todo!()
    }
    fn project_held(&self, id: &str) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    async fn copy_blocked(&self, lead: &wsp_machines::machines::machines::Entry) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn daemon_frame(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        frame: serde_json::Value,
    ) -> wsp_machines::machines::area_error::AreaResult<serde_json::Value> {
        todo!()
    }
    fn checkout_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn brokered(&self, kind: &str, place: Option<&str>) -> bool {
        todo!()
    }
    fn brokered_start(&self) -> bool {
        todo!()
    }
    async fn repo_of_record(
        &self,
        remote: &str,
        project: &str,
        on: &str,
    ) -> wsp_machines::machines::area_error::AreaResult<serde_json::Map<String, serde_json::Value>> {
        todo!()
    }
    fn free_branch_name(&self, wanted: &str, heads: Option<&str>) -> String {
        todo!()
    }
    async fn copy_of(
        &self,
        place: &str,
        image: &str,
    ) -> wsp_machines::machines::area_error::AreaResult<Option<wsp_wire::views::golden_image::GoldenManifest>> {
        todo!()
    }
    async fn record_of(
        &self,
        image: &str,
    ) -> wsp_machines::machines::area_error::AreaResult<Option<wsp_machines::machines::sub_spawn::ImageRecord>> {
        todo!()
    }
    async fn place_guard(&self, place: &str) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn default_agent_of(
        &self,
        lead: &wsp_machines::machines::machines::Entry,
    ) -> wsp_machines::machines::area_error::AreaResult<String> {
        todo!()
    }
    fn unbrokered_refusal(&self, harness: &str) -> Option<wsp_words::exit::HostError> {
        todo!()
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        todo!()
    }
    fn tree_of(&self, scope: &wsp_wire::views::workspace_more::ThreadScope) -> indexmap::IndexMap<String, serde_json::Value> {
        todo!()
    }
    fn forking_add(&self, name: &str) {
        todo!()
    }
    fn forking_remove(&self, name: &str) {
        todo!()
    }
    fn supersede_failed(&self, name: &str) {
        todo!()
    }
    async fn enqueue(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        wait: Option<wsp_wire::views::session::ThreadCapWait>,
    ) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn queue_wait(
        &self,
        place: &str,
        limits: Option<&wsp_engine::box_backend::BoxLimits>,
    ) -> Option<wsp_wire::views::session::ThreadCapWait> {
        todo!()
    }
    fn view(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> serde_json::Value {
        todo!()
    }
    fn stage_reporter(
        &self,
        id: &str,
        name: &str,
        started_at: f64,
        asked_by: Option<serde_json::Value>,
    ) -> wsp_core::types::wiring::StageReport {
        todo!()
    }
    async fn fork(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        on_machine: Box<dyn Fn(std::sync::Arc<dyn wsp_engine::machine::Machine>) -> Result<(), wsp_words::exit::HostError>>,
        report: wsp_core::types::wiring::StageReport,
    ) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn attach(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> Result<wsp_machines::machines::machines::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn attach_dead(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
    ) -> Result<wsp_machines::machines::machines::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    async fn ping_daemon(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn sync_daemon(&self, entry: &wsp_machines::machines::machines::Entry) {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn emit_status_reachable(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
    ) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn unfork(&self, entry: &wsp_machines::machines::machines::Entry) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn broker_lift(&self, workspace_id: &str) {
        todo!()
    }
    async fn rows_sent(&self) {
        todo!()
    }
    fn session_file(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        agent: &str,
        session_id: &str,
    ) -> Option<wsp_machines::machines::sub_life::SessionFile> {
        todo!()
    }
    async fn get_seed_choice(
        &self,
        folder: &str,
    ) -> wsp_machines::machines::area_error::AreaResult<Option<wsp_wire::views::workspace::SeedChoice>> {
        todo!()
    }
    async fn get_blob(&self, collection: &str, key: &str) -> wsp_machines::machines::area_error::AreaResult<Option<Vec<u8>>> {
        todo!()
    }
    async fn delete_create(&self, key: &str) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn seeds(&self) -> bool {
        todo!()
    }
    async fn homes_here(&self) -> wsp_machines::machines::area_error::AreaResult<serde_json::Value> {
        todo!()
    }
    async fn seed_plan(
        &self,
        folder: &str,
        homes: &serde_json::Value,
    ) -> wsp_machines::machines::area_error::AreaResult<wsp_wire::views::workspace::SeedPlan> {
        todo!()
    }
    async fn seed_pack(
        &self,
        plan: &wsp_wire::views::workspace::SeedPlan,
        files: &[String],
        homes: &serde_json::Value,
    ) -> wsp_machines::machines::area_error::AreaResult<wsp_machines::machines::sub_spawn::Packed> {
        todo!()
    }
    fn folder_named(&self, path: &str) -> String {
        todo!()
    }
    fn emit(&self, event: serde_json::Value) {
        todo!()
    }
    fn now(&self) -> f64 {
        todo!()
    }
    fn random_hex(&self, bytes: usize) -> String {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
}
