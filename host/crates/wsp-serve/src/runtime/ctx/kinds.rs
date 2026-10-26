// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::kinds::KindsCtx for Ctx {
    fn now(&self) -> f64 {
        todo!()
    }
    fn state_path(&self) -> Option<String> {
        todo!()
    }
    async fn local_machine(&self) -> wsp_machines::machines::area_error::AreaResult<std::sync::Arc<dyn wsp_engine::machine::Machine>> {
        todo!()
    }
    fn own_backend(&self) -> std::sync::Arc<dyn wsp_engine::machine::MachineBackend> {
        todo!()
    }
    fn released_backend(&self) -> std::sync::Arc<dyn wsp_engine::machine::MachineBackend> {
        todo!()
    }
    fn place_name(&self, place: &str) -> String {
        todo!()
    }
    fn wired_place(&self) -> String {
        todo!()
    }
    fn project_held(&self, id: &str) -> wsp_machines::machines::area_error::AreaResult<wsp_wire::views::workspace::ProjectView> {
        todo!()
    }
    fn checkout_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_machines::machines::area_error::AreaResult<String> {
        todo!()
    }
    fn computer_of(&self, entry: &wsp_core::types::wiring::LiveWorkspace) -> String {
        todo!()
    }
    fn served_by_its_computer(&self, entry: &wsp_core::types::wiring::LiveWorkspace) -> bool {
        todo!()
    }
    fn has_place_door(&self) -> bool {
        self.place_door.get().is_some()
    }
    fn door_backend_of(&self, place: &str) -> Option<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>> {
        todo!()
    }
    fn door_name_of(&self, place: &str) -> String {
        todo!()
    }
    fn door_folder_computer(&self, place: &str) -> Option<wsp_machines::places::types::FolderComputer> {
        todo!()
    }
    fn door_joined(&self, place: &str) -> bool {
        todo!()
    }
    fn door_logins_of(&self, place: &str) -> Option<String> {
        todo!()
    }
    fn door_vault_at(&self, place: &str, vault: &indexmap::IndexMap<String, String>) -> Option<indexmap::IndexMap<String, String>> {
        todo!()
    }
    fn door_sign_ins_at(&self, place: &str) -> Option<indexmap::IndexMap<String, wsp_wire::types::agents_report::AgentSignInState>> {
        todo!()
    }
    fn door_github_from_vault(&self, place: &str) -> bool {
        todo!()
    }
    async fn door_exec(
        &self,
        place: &str,
        cmd: &str,
        timeout_ms: u64,
    ) -> wsp_machines::machines::area_error::AreaResult<wsp_engine::machine::ExecResult> {
        todo!()
    }
    async fn door_forward(
        &self,
        place: &str,
        port: u16,
        pane: Option<(String, String)>,
        standing: bool,
    ) -> wsp_machines::machines::area_error::AreaResult<u16> {
        todo!()
    }
    async fn door_report_daemon_version(&self, place: &str) -> Option<u32> {
        todo!()
    }
    fn vault(&self) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn broker_ready(&self) -> bool {
        todo!()
    }
    fn has_broker_status(&self) -> bool {
        todo!()
    }
    async fn broker_status(&self) -> wsp_machines::machines::area_error::AreaResult<wsp_wire::wire::runtime::BrokerStatus> {
        todo!()
    }
    async fn broker_codex_claims(&self) -> wsp_machines::machines::area_error::AreaResult<Option<wsp_adapters::pending::CodexClaims>> {
        todo!()
    }
    fn broker_codex_models(&self) -> Option<wsp_adapters::codex::command::AccountModels> {
        todo!()
    }
    fn is_brokered(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> bool {
        todo!()
    }
    fn codex_broker_of(&self, said: &wsp_machines::machines::kinds::BrokerSaid) -> Option<wsp_adapters::codex::command::CodexBroker> {
        todo!()
    }
    fn unbrokered_refusal(&self, harness: &str) -> Option<wsp_words::exit::HostError> {
        todo!()
    }
    fn codex_login_unread_refusal(&self) -> wsp_words::exit::HostError {
        todo!()
    }
    fn placeholders_of(
        &self,
        said: &wsp_machines::machines::kinds::BrokerSaid,
        token: &str,
    ) -> wsp_machines::machines::brokered::Placeholders {
        todo!()
    }
    fn brokered_env(&self, token: &str, record: &wsp_core::types::wiring::WorkspaceRecord) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn land_on_record(
        &self,
        entry: &wsp_core::types::wiring::LiveWorkspace,
        project: wsp_wire::views::workspace::ProjectView,
        lines: Option<Vec<String>>,
    ) -> futures::future::LocalBoxFuture<'static, wsp_machines::machines::area_error::AreaResult<()>> {
        todo!()
    }
    fn clone_lines(&self, o: serde_json::Value) -> Vec<String> {
        todo!()
    }
    async fn daemon_token_of(
        &self,
        machine: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> wsp_machines::machines::area_error::AreaResult<Option<String>> {
        todo!()
    }
    fn forget_daemon_token(&self, machine_id: &str) {
        todo!()
    }
    fn has_deploy(&self) -> bool {
        todo!()
    }
    async fn deploy_daemon(
        &self,
        machine: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
        token: Option<String>,
    ) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn local_daemon_road(
        &self,
    ) -> Option<wsp_machines::machines::area_error::AreaResult<wsp_wire::views::workspace::DaemonReachView>> {
        todo!()
    }
    fn machine_exec_opts(&self) -> Option<wsp_core::types::pending::MachineExecOptions> {
        todo!()
    }
    fn copy_import(
        &self,
        entry: &wsp_core::types::wiring::LiveWorkspace,
        o: wsp_core::types::wiring::ProjectImportOptions,
        report: wsp_core::types::internal::ImportReport,
    ) -> futures::future::LocalBoxFuture<'static, wsp_machines::machines::area_error::AreaResult<wsp_core::types::internal::ImportLanded>>
    {
        todo!()
    }
    fn hello_version(&self, entry: &wsp_core::types::wiring::LiveWorkspace) -> futures::future::LocalBoxFuture<'static, Option<u32>> {
        todo!()
    }
    fn has_local(&self) -> bool {
        self.opts.local.is_some()
    }
    fn local_backend(&self) -> Option<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>> {
        todo!()
    }
    fn local_home(&self, agent: &str) -> String {
        todo!()
    }
    fn local_home_dir(&self) -> String {
        todo!()
    }
    fn local_env(&self) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn local_roots_path(&self) -> Option<String> {
        todo!()
    }
    fn has_local_daemon_road(&self) -> bool {
        todo!()
    }
    fn has_local_restart(&self) -> bool {
        todo!()
    }
    async fn here_daemon_version(&self) -> Option<wsp_machines::machines::area_error::AreaResult<Option<u32>>> {
        todo!()
    }
    fn local_scratch(&self) -> String {
        todo!()
    }
    fn agents_wsp_mcp(&self) -> Option<wsp_wire::views::harness::McpServerSpec> {
        todo!()
    }
    fn agents_here_url(&self) -> Option<String> {
        todo!()
    }
    async fn door_login_user(&self, place: &str) -> wsp_machines::machines::area_error::AreaResult<String> {
        todo!()
    }
    async fn end_thread(
        &self,
        place: &str,
        thread_id: &str,
        o: wsp_core::types::internal::EndThread,
    ) -> wsp_machines::machines::area_error::AreaResult<Option<String>> {
        todo!()
    }
    async fn end_owed(
        &self,
        place: &str,
        thread_id: &str,
    ) -> wsp_machines::machines::area_error::AreaResult<Option<wsp_machines::machines::thread_ends::OwedEnd>> {
        todo!()
    }
    fn sleep(&self, ms: f64) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
}

impl wsp_machines::machines::sub_life::BackendFor for Ctx {
    fn backend_for(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::thread_ends::ThreadEndsRuntime for Ctx {
    async fn store_get(&self, collection: &str, id: &str) -> Result<Option<serde_json::Value>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn store_put(&self, collection: &str, id: &str, value: serde_json::Value) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn store_delete(&self, collection: &str, id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn running_on(&self, thread_id: &str) -> bool {
        todo!()
    }
    fn launching_on(&self, thread_id: &str) -> bool {
        todo!()
    }
    fn stop_holds(&self, thread_id: &str) -> bool {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::thread_ends::ThreadEndsDoor for Ctx {
    async fn exec(
        &self,
        place_id: &str,
        script: &str,
        timeout_ms: u64,
    ) -> Result<wsp_engine::machine::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    fn link(&self, place_id: &str) -> bool {
        todo!()
    }
    fn name_of(&self, place_id: &str) -> String {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::rules::RulesCtx for Ctx {
    fn no_machines_line(&self) -> Option<String> {
        todo!()
    }
    async fn store_put_project(&self, project: &wsp_wire::views::workspace::ProjectView) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn door_name_of(&self, place_id: &str) -> Option<String> {
        todo!()
    }
    fn landing(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, i64>>> {
        todo!()
    }
    fn live(&self) -> Vec<wsp_account::account::core_types::Entry> {
        todo!()
    }
    fn live_get(&self, id: &str) -> Option<wsp_account::account::core_types::Entry> {
        todo!()
    }
    fn projects_held(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_wire::views::workspace::ProjectView>>> {
        todo!()
    }
    fn sessions_size(&self) -> usize {
        todo!()
    }
    fn thread_records_size(&self) -> usize {
        todo!()
    }
    fn thread_record(&self, thread_id: &str) -> Option<wsp_core::types::internal::ThreadRecord> {
        todo!()
    }
    fn rows_on(&self, thread_id: &str) -> Vec<wsp_account::account::rules::TreeFacts> {
        todo!()
    }
    async fn target_workspace(&self) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn set_preferences(&self, patch: serde_json::Value) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn capability(&self, record: &wsp_core::types::wiring::WorkspaceRecord, can: &str) -> bool {
        todo!()
    }
    fn pause_mode_declared(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> bool {
        todo!()
    }
    fn forks_no_machines(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> bool {
        todo!()
    }
    fn relayed(&self, kind: wsp_wire::views::workspace_kind::WorkspaceKind, machine_id: Option<&str>) -> bool {
        todo!()
    }
    fn turn_reach(&self, entry: &wsp_account::account::core_types::Entry) -> Option<serde_json::Map<String, serde_json::Value>> {
        todo!()
    }
    fn wsp_mcp(&self, entry: &wsp_account::account::core_types::Entry) -> Option<serde_json::Value> {
        todo!()
    }
    fn same_repository(
        &self,
        a: &wsp_wire::views::workspace::ProjectView,
        b: &wsp_wire::views::workspace::ProjectView,
        caller: Option<&wsp_wire::views::workspace::Caller>,
    ) -> bool {
        todo!()
    }
    fn project_held(&self, id: &str) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn kind_of(&self, computer: &str) -> wsp_wire::views::workspace_kind::WorkspaceKind {
        todo!()
    }
    fn place_name(&self, id: &str) -> String {
        todo!()
    }
    fn agents_held(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_wire::views::workspace_more::WorkspaceAgents {
        todo!()
    }
    fn place_id_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::images::records::RecordsCtx for Ctx {
    fn store(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Store> {
        todo!()
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Clock> {
        todo!()
    }
    fn wired(&self) -> String {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn module_folder(&self, r: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn home_of(&self, r: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn project_ref(&self, project: &str) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn daemon_note(&self, workspace_id: &str) -> Option<String> {
        todo!()
    }
    fn agents_of(&self, r: &wsp_core::types::wiring::WorkspaceRecord) -> Option<serde_json::Value> {
        todo!()
    }
    fn provider_of(&self, r: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn backend_for(
        &self,
        r: &wsp_core::types::wiring::WorkspaceRecord,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    fn live_records(&self) -> Vec<std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>> {
        todo!()
    }
    fn bump_generation(&self, workspace_id: &str) -> bool {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::images::held::HeldCtx for Ctx {
    fn store(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Store> {
        todo!()
    }
    fn wired(&self) -> String {
        todo!()
    }
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn drop_copy_recipe(&self, place: &str, name: &str, version: u32) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::projects::branch_of_record::BranchCtx for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Clock> {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn broker_ready(&self) -> bool {
        todo!()
    }
    async fn exec_here(&self, cmd: &str, timeout_ms: u64) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn live_records(&self) -> Vec<std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>> {
        todo!()
    }
    async fn mint(
        &self,
        name: &str,
        scope: wsp_wire::views::workspace_more::ThreadScope,
        now: i64,
        more: wsp_core::devices::MintMore,
    ) -> Result<wsp_core::devices::PairedDevice, wsp_words::exit::HostError> {
        todo!()
    }
    async fn revoke(&self, id: &str) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn turn_road_of(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
    ) -> Result<wsp_core::devices::MintMore, wsp_words::exit::HostError> {
        todo!()
    }
    fn place_name(&self, place: &str) -> String {
        todo!()
    }
    fn wired(&self) -> String {
        todo!()
    }
    fn home_dir(&self) -> String {
        todo!()
    }
    async fn read_file(&self, path: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn marketplace_files(&self) -> Vec<wsp_machines::projects::branch_of_record::MarketplaceFile> {
        todo!()
    }
    fn brokered_git_env(
        &self,
        token: &str,
        of_record: Option<&wsp_wire::views::workspace::BranchOfRecord>,
    ) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn on_record_lines(
        &self,
        checkout: &str,
        of_record: &wsp_wire::views::workspace::BranchOfRecord,
        origin: &str,
        held: bool,
    ) -> Vec<String> {
        todo!()
    }
    async fn env_exec(
        &self,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
        script: &str,
        env: indexmap::IndexMap<String, String>,
        timeout_ms: u64,
    ) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::images::builders::ImageDoor for Ctx {
    fn backend_of(&self, place_id: &str) -> Option<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>> {
        todo!()
    }
    async fn forking_backend(
        &self,
        place_id: &str,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    fn name_of(&self, place_id: &str) -> Option<String> {
        todo!()
    }
    async fn place_for(&self, word: &str) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn default_place(&self) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn mark_default_if_none(&self, place_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn list(&self, now: i64) -> Result<Vec<wsp_wire::views::place_more::PlaceView>, wsp_words::exit::HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::agents_read::AgentsPlaces for Ctx {
    async fn list(&self, now: i64) -> Result<Vec<wsp_wire::views::place_more::PlaceView>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn report_of(&self, place_id: &str) -> Result<Option<wsp_wire::wire::place_link::PlaceReport>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn logins_again(&self, place_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn sign_ins_at(
        &self,
        place_id: &str,
        agents: &[String],
    ) -> Option<indexmap::IndexMap<String, wsp_wire::types::agents_report::AgentSignInState>> {
        todo!()
    }
    async fn exec(
        &self,
        place_id: &str,
        cmd: &str,
        timeout_ms: Option<u64>,
        stdin: Option<Vec<u8>>,
    ) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    async fn gh_there(
        &self,
        place_id: &str,
        on_step: wsp_machines::agents_read::GhStep,
    ) -> Result<Vec<wsp_wire::views::place::PlaceProvisionRow>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn key_landed(&self, agent: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn login_landed(&self, place_id: &str, agent: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
}
