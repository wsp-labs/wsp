// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

impl wsp_threads::threads::agents::AgentsCtx for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        todo!()
    }
    fn store(&self) -> std::rc::Rc<dyn wsp_store::store::Store> {
        todo!()
    }
    fn bus(&self) -> std::rc::Rc<dyn wsp_core::types::events::EventBus> {
        self.bus.clone()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        todo!()
    }
    fn thread_records(&self) -> wsp_threads::threads::threads::ThreadRecords {
        todo!()
    }
    fn entry(&self, workspace_id: &str) -> Option<wsp_threads::threads::threads::Entry> {
        todo!()
    }
    fn closing(&self) -> bool {
        todo!()
    }
    fn adapters(&self) -> indexmap::IndexMap<String, wsp_core::types::harness::HarnessAdapterFactory> {
        todo!()
    }
    fn local_home_dir(&self) -> Option<String> {
        todo!()
    }
    fn local_home(&self, agent: &str) -> Option<String> {
        todo!()
    }
    fn process_home(&self) -> String {
        todo!()
    }
    fn state_path(&self) -> Option<String> {
        todo!()
    }
    fn setup_off(&self, place: &str, agent: &str) -> bool {
        todo!()
    }
    fn setup_config_dir(&self, place: &str, agent: &str) -> Option<String> {
        todo!()
    }
    fn setup_launch(&self, place: &str, agent: &str) -> wsp_threads::threads::agents::SetupLaunch {
        todo!()
    }
    fn setup_refusal_moved(&self, place: &str, harness: &str) {
        todo!()
    }
    fn place_exec(
        &self,
        place: &str,
        script: &str,
        timeout_ms: u64,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError>> {
        todo!()
    }
    fn place_name(&self, place: &str) -> String {
        todo!()
    }
    fn place_id_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn at_provider(&self, place_id: Option<&str>) -> bool {
        todo!()
    }
    fn place_agent_home(&self, place: &str, home: &str, agent: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn ready(&self) -> futures::future::LocalBoxFuture<'static, Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    fn know_broker(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: &str,
    ) -> futures::future::LocalBoxFuture<'static, Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    fn unreached_of(&self, entry: &wsp_threads::threads::threads::Entry) -> Option<String> {
        todo!()
    }
    fn persist_sessions(&self, workspace_id: &str) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn push_head(&self, thread_id: &str) {
        todo!()
    }
    fn write_session(
        &self,
        session: &str,
        write: futures::future::LocalBoxFuture<'static, Result<wsp_adapters::port::SessionRenameWrite, wsp_words::exit::HostError>>,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_adapters::port::SessionRenameWrite, wsp_words::exit::HostError>> {
        todo!()
    }
    fn kind_home(&self, entry: &wsp_threads::threads::threads::Entry, agent: &str) -> String {
        todo!()
    }
    fn kind_env(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: Option<&str>,
        home: &dyn Fn(&str) -> String,
    ) -> wsp_adapters::port::Env {
        todo!()
    }
    fn kind_server_values(&self, entry: &wsp_threads::threads::threads::Entry) -> Option<wsp_core::types::internal::ServerValuesRoad> {
        todo!()
    }
    fn kind_ends_threads(&self, entry: &wsp_threads::threads::threads::Entry) -> bool {
        todo!()
    }
    fn kind_memory_key(&self, entry: &wsp_threads::threads::threads::Entry, harness: &str) -> Option<String> {
        todo!()
    }
    fn kind_login_stands(&self, entry: &wsp_threads::threads::threads::Entry, agent: &str) -> bool {
        todo!()
    }
    fn computer_runs_in_folder(&self, computer: &str) -> bool {
        todo!()
    }
    fn brokered_launch(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: &str,
        token: Option<&str>,
    ) -> Result<Option<wsp_machines::machines::kinds::BrokeredLaunch>, wsp_words::exit::HostError> {
        todo!()
    }
    fn vault(&self) -> wsp_adapters::port::Env {
        todo!()
    }
    fn vault_on(&self, entry: &wsp_threads::threads::threads::Entry) -> wsp_adapters::port::Env {
        todo!()
    }
    fn exec_factory_for(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        turn: wsp_threads::threads::agents::TurnLaunch,
        waiting: Option<wsp_core::machine_exec::TurnWaiting>,
    ) -> std::sync::Arc<dyn wsp_adapters::port::ExecStreamFactory> {
        todo!()
    }
    fn signs_in(&self, addresses: Vec<String>) -> futures::future::LocalBoxFuture<'static, Option<wsp_threads::threads::agents::SignsIn>> {
        todo!()
    }
    fn resolve_project(
        &self,
        reference: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError>> {
        todo!()
    }
    fn list_projects(
        &self,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> futures::future::LocalBoxFuture<'static, Result<Vec<wsp_wire::views::workspace::ProjectView>, wsp_words::exit::HostError>> {
        todo!()
    }
    fn project_folder(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_threads::threads::threads::Entry, wsp_words::exit::HostError>> {
        todo!()
    }
    fn spawn(&self, work: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_threads::threads::launch::LaunchCtx for Ctx {
    fn ids(&self) -> std::rc::Rc<dyn wsp_core::ids::Ids> {
        todo!()
    }
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn entry_of(
        &self,
        workspace_id: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_threads::threads::threads::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn keeps_agents(&self, entry: &wsp_threads::threads::threads::Entry) -> bool {
        todo!()
    }
    fn kept_box_cap(&self, workspace_id: &str) -> Option<u32> {
        todo!()
    }
    fn closing(&self) -> bool {
        todo!()
    }
    fn preferences_held(&self) -> Option<wsp_wire::views::preferences::Preferences> {
        todo!()
    }
    async fn preferences(&self) -> Result<wsp_wire::views::preferences::Preferences, wsp_words::exit::HostError> {
        todo!()
    }
    fn default_agent_of(&self, prefs: &wsp_wire::views::preferences::Preferences, entry: &wsp_threads::threads::threads::Entry) -> String {
        todo!()
    }
    fn agent_off(&self, entry: &wsp_threads::threads::threads::Entry, harness: &str) -> bool {
        todo!()
    }
    fn adapter_for(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        ask: wsp_threads::threads::agents::launch::AdapterAsk,
    ) -> Result<wsp_threads::threads::agents::launch::BuiltAdapter, wsp_words::exit::HostError> {
        todo!()
    }
    fn adapter_warms(&self, adapter: &dyn wsp_adapters::port::HarnessAdapter) -> bool {
        todo!()
    }
    async fn confine_setup(&self, entry: &wsp_threads::threads::threads::Entry, harness: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn catalog_on(
        &self,
        harness: &str,
        entry: &wsp_threads::threads::threads::Entry,
        adapter: &dyn wsp_adapters::port::HarnessAdapter,
    ) -> Result<Option<wsp_wire::views::harness::HarnessCatalog>, wsp_words::exit::HostError> {
        todo!()
    }
    fn defaults_on(
        &self,
        catalog: wsp_wire::views::harness::HarnessCatalog,
        prefs: &wsp_wire::views::preferences::Preferences,
        project: &str,
    ) -> wsp_threads::threads::agents::launch::DefaultsOn {
        todo!()
    }
    fn named_mode(
        &self,
        catalog: Option<&wsp_wire::views::harness::HarnessCatalog>,
        harness: &str,
        access: &wsp_wire::types::thread_defaults::AccessChoice,
    ) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn access_of(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> Option<String> {
        todo!()
    }
    fn ran_on(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> wsp_wire::types::harness_picks::RanPicks {
        todo!()
    }
    fn resumed_fact(&self, workspace_id: &str, session: &str, fact: &str) -> Option<String> {
        todo!()
    }
    fn not_run_refused(
        &self,
        catalog: Option<&wsp_wire::views::harness::HarnessCatalog>,
        model: Option<&str>,
    ) -> Option<wsp_words::exit::HostError> {
        todo!()
    }
    async fn thread_folder(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        cwd: Option<&str>,
    ) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn folder_exists(&self, path: &str) -> bool {
        todo!()
    }
    fn runs_in(&self, entry: &wsp_threads::threads::threads::Entry, ran_in: Option<&str>, folder: &str) -> String {
        todo!()
    }
    fn setup_place(&self, entry: &wsp_threads::threads::threads::Entry) -> Option<String> {
        todo!()
    }
    fn launch_of(&self, place: &str, harness: &str) -> Option<wsp_adapters::port::AgentLaunch> {
        todo!()
    }
    fn warm_on(&self, workspace_id: &str, harness: &str) -> Option<(String, wsp_core::types::internal::KeptLaunch)> {
        todo!()
    }
    fn rewarm(&self, thread_id: &str) {
        todo!()
    }
    fn reap_kept(&self, thread_id: &str) {
        todo!()
    }
    async fn thread_launch(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_id: &str,
        root_thread_id: &str,
        aside: bool,
    ) -> Result<wsp_threads::threads::launch::ThreadLaunch, wsp_words::exit::HostError> {
        todo!()
    }
    fn vault(&self) -> wsp_adapters::port::Env {
        todo!()
    }
    async fn launch_servers_for(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: &str,
        adapter: &dyn wsp_adapters::port::HarnessAdapter,
        cwd: &str,
    ) -> Result<wsp_threads::threads::agents::launch::LaunchServers, wsp_words::exit::HostError> {
        todo!()
    }
    fn hold_warm(&self, thread_id: &str, hold: wsp_threads::threads::threads::KeptHand) {
        todo!()
    }
    async fn revoke_device(&self, device_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn spawn(&self, work: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}

impl wsp_threads::threads::threads::ThreadsCtx for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        todo!()
    }
    fn ids(&self) -> std::rc::Rc<dyn wsp_core::ids::Ids> {
        todo!()
    }
    fn bus(&self) -> std::rc::Rc<dyn wsp_core::types::events::EventBus> {
        self.bus.clone()
    }
    fn store(&self) -> std::rc::Rc<dyn wsp_store::store::Store> {
        todo!()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        todo!()
    }
    fn thread_records(&self) -> wsp_threads::threads::threads::ThreadRecords {
        todo!()
    }
    fn closing(&self) -> bool {
        todo!()
    }
    fn entry(&self, workspace_id: &str) -> Option<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn live_records(&self) -> Vec<wsp_core::types::wiring::WorkspaceRecord> {
        todo!()
    }
    fn taken(&self, workspace_id: &str, request_id: &str) -> Option<wsp_core::types::internal::Taken> {
        todo!()
    }
    fn has_place_door(&self) -> bool {
        todo!()
    }
    fn place_linked(&self, place: &str) -> bool {
        todo!()
    }
    fn on_place_move(&self, listener: std::rc::Rc<dyn Fn(&wsp_threads::threads::threads::PlaceMove)>) -> Option<Box<dyn Fn()>> {
        todo!()
    }
    fn place_mem_mb(&self, place: &str) -> Option<f64> {
        todo!()
    }
    fn place_threads_at(&self, place: &str) -> Option<i64> {
        todo!()
    }
    fn place_name(&self, place: &str) -> String {
        todo!()
    }
    fn place_id_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn provider_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn revoke_device(&self, device_id: &str) -> futures::future::LocalBoxFuture<'static, Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    fn proc_scopes_moved(&self) {
        todo!()
    }
    fn root_of(&self, thread_id: &str) -> Option<String> {
        todo!()
    }
    fn interrupt(
        &self,
        row_id: &str,
        caller: Option<wsp_wire::views::workspace::Caller>,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_wire::wire::runtime::SessionInterruptResult, wsp_words::exit::HostError>> {
        todo!()
    }
    fn start(
        &self,
        workspace_id: &str,
        options: wsp_core::types::api::StartOptions,
        caller: Option<wsp_wire::views::workspace::Caller>,
    ) -> futures::future::LocalBoxFuture<'static, Result<std::rc::Rc<dyn wsp_core::types::wiring::SessionHandle>, wsp_words::exit::HostError>>
    {
        todo!()
    }
    fn stop_try(&self, row_id: &str) -> futures::future::LocalBoxFuture<'static, Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    fn mark(
        &self,
        threads: Vec<String>,
        mark: wsp_core::types::internal::ThreadStamps,
        caller: Option<wsp_wire::views::workspace::Caller>,
    ) -> futures::future::LocalBoxFuture<'static, Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    fn sub_line(&self, workspace_id: &str, since: i64) -> futures::future::LocalBoxFuture<'static, Option<String>> {
        todo!()
    }
    fn boot_work(&self, workspace_id: &str) -> Option<wsp_core::context::Joined> {
        todo!()
    }
    fn kept_writes(&self, workspace_id: &str) -> Option<wsp_core::context::Joined> {
        todo!()
    }
    fn keep_sent_images(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: Option<String>,
        attachments: Vec<wsp_wire::types::attachments::Attachment>,
    ) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn record(&self, event: wsp_wire::views::session_events_more::SessionEvent) {
        todo!()
    }
    fn persist_sessions(&self, workspace_id: &str) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn open_thread(
        &self,
        workspace_id: &str,
        thread_id: &str,
    ) -> futures::future::LocalBoxFuture<'static, Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, wsp_words::exit::HostError>>
    {
        todo!()
    }
    fn idle_hold(&self, workspace_id: &str) {
        todo!()
    }
    fn idle_release(&self, workspace_id: &str) {
        todo!()
    }
    fn port_roots_moved(&self, workspace_id: &str) {
        todo!()
    }
    fn end_snooze_for(&self, view: &wsp_wire::views::session::SessionView) {
        todo!()
    }
    fn with_daemon(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        frame: serde_json::Value,
    ) -> futures::future::LocalBoxFuture<'static, Result<serde_json::Value, wsp_account::account::core_types::AccountError>> {
        todo!()
    }
    fn checkout_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn project_held(&self, project: &str) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn reach_of(&self, entry: &wsp_machines::machines::machines::Entry) -> wsp_wire::views::workspace::ReachState {
        todo!()
    }
    fn turn_snapshot_ms(&self) -> Option<i64> {
        todo!()
    }
    fn vault(&self) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn vault_on(&self, entry: &wsp_machines::machines::machines::Entry) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn login_stands(&self, entry: &wsp_machines::machines::machines::Entry, harness: &str) -> bool {
        todo!()
    }
    fn computer_of(&self, entry: &wsp_machines::machines::machines::Entry) -> String {
        todo!()
    }
    fn ledger_limits(
        &self,
    ) -> futures::future::LocalBoxFuture<'static, Result<Vec<wsp_wire::types::usage::AccountLimit>, wsp_words::exit::HostError>> {
        todo!()
    }
    fn spawn(&self, work: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}
