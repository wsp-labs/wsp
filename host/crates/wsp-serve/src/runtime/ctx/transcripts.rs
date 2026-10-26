// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

impl wsp_threads::threads::transcripts::TranscriptsCtx for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        todo!()
    }
    fn ids(&self) -> std::rc::Rc<dyn wsp_core::ids::Ids> {
        todo!()
    }
    fn bus(&self) -> std::rc::Rc<dyn wsp_core::types::events::EventBus> {
        self.bus.clone()
    }
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn store(&self) -> std::rc::Rc<dyn wsp_store::store::Store> {
        todo!()
    }
    fn rows(&self) -> std::rc::Rc<dyn wsp_store::sqlite_transcripts::TranscriptRows> {
        todo!()
    }
    fn transcript_state(&self) -> wsp_core::context::StateCell<wsp_threads::threads::transcripts::TranscriptState> {
        todo!()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        todo!()
    }
    fn thread_records(&self) -> wsp_threads::threads::threads::ThreadRecords {
        todo!()
    }
    fn index_flushes(&self) -> wsp_core::context::StateCell<indexmap::IndexMap<String, wsp_core::context::Joined>> {
        todo!()
    }
    fn entry(&self, workspace_id: &str) -> Option<wsp_threads::threads::threads::Entry> {
        todo!()
    }
    fn is_live(&self, workspace_id: &str) -> bool {
        todo!()
    }
    fn closed(&self) -> bool {
        todo!()
    }
    fn push_head(&self, thread_id: &str) {
        todo!()
    }
    fn rows_on(&self, thread_id: &str) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn latest_on(&self, thread_id: &str) -> Option<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn resumed_fact(&self, workspace_id: &str, session: &str, fact: &str) -> Option<String> {
        todo!()
    }
    fn daemon_token_seed(&self) -> Option<String> {
        self.opts.daemon_token.clone()
    }
    fn rotate_daemon_token(
        &self,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
        token: String,
        path: Option<String>,
    ) -> futures::future::LocalBoxFuture<'static, Result<bool, wsp_words::exit::HostError>> {
        todo!()
    }
    fn spawn(&self, work: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::channels::ChannelsCtx for Ctx {
    fn index_start(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        todo!()
    }
    fn index_cut(&self, workspace_id: &str, thread_id: &str) -> Option<bool> {
        todo!()
    }
    fn index_fact(&self, workspace_id: &str, resume: &str, fact: &str) -> Option<String> {
        todo!()
    }
    fn session_rows(&self) -> Vec<wsp_account::account::channels::ChannelRow> {
        todo!()
    }
    fn thread_record(&self, thread_id: &str) -> Option<wsp_core::types::internal::ThreadRecord> {
        todo!()
    }
    fn thread_ids(&self) -> Vec<String> {
        todo!()
    }
    fn latest_harness(&self, thread_id: &str) -> Option<String> {
        todo!()
    }
    fn home_dir(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    async fn daemon_road(
        &self,
        entry: &wsp_account::account::core_types::Entry,
    ) -> wsp_account::account::core_types::AccountResult<wsp_wire::views::workspace::DaemonReachView> {
        todo!()
    }
    fn turn_road(&self, kind: wsp_wire::views::workspace_kind::WorkspaceKind) -> Option<wsp_core::devices::ScopedRoad> {
        todo!()
    }
    fn has_door(&self) -> bool {
        todo!()
    }
    fn offer_of(&self, place_id: &str) -> Option<String> {
        todo!()
    }
    fn wired(&self) -> String {
        todo!()
    }
    fn settings_at(&self, place_id: &str) -> Option<wsp_wire::views::place_more::PlaceSettings> {
        todo!()
    }
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<i64>> {
        todo!()
    }
    fn name_of(&self, place_id: &str) -> String {
        todo!()
    }
    fn joined(&self, place_id: &str) -> bool {
        todo!()
    }
    async fn report_of(&self, place_id: &str) -> Option<wsp_wire::wire::place_link::PlaceReport> {
        todo!()
    }
    async fn rows(&self) -> Vec<wsp_wire::views::place_more::PlaceView> {
        todo!()
    }
    fn link_channel(
        &self,
        place_id: &str,
        on_event: std::rc::Rc<dyn Fn(serde_json::Value)>,
    ) -> Option<std::rc::Rc<dyn wsp_core::daemon_channel::DaemonChannel>> {
        todo!()
    }
    async fn spend_today(&self, rows: &[wsp_wire::views::place_more::PlaceView], now: i64) -> Vec<(String, Option<f64>)> {
        todo!()
    }
    fn projects_held(&self) -> indexmap::IndexMap<String, wsp_wire::views::workspace::ProjectView> {
        todo!()
    }
    async fn open_channel(
        &self,
        url: &str,
        token: &str,
        on_event: std::rc::Rc<dyn Fn(serde_json::Value)>,
    ) -> wsp_account::account::core_types::AccountResult<std::rc::Rc<dyn wsp_core::daemon_channel::DaemonChannel>> {
        todo!()
    }
    fn local_has_daemon(&self) -> bool {
        todo!()
    }
    fn local_home_dir(&self) -> String {
        todo!()
    }
    async fn local_road(&self) -> wsp_account::account::core_types::AccountResult<wsp_wire::views::workspace::DaemonReachView> {
        todo!()
    }
    async fn adopt_pause(&self, entry: &wsp_account::account::core_types::Entry) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn broker_ready(&self) -> bool {
        todo!()
    }
    fn root_of(&self, thread_id: &str) -> String {
        todo!()
    }
    fn device_door(&self) -> std::rc::Rc<dyn wsp_core::devices::DeviceDoor> {
        todo!()
    }
    fn host_pid(&self) -> i64 {
        todo!()
    }
    fn host_parent_pid(&self) -> i64 {
        todo!()
    }
    fn has_state_path(&self) -> bool {
        todo!()
    }
    fn state_folder(&self) -> String {
        todo!()
    }
    fn read_link(&self, path: &str) -> Option<String> {
        todo!()
    }
    fn hostname(&self) -> String {
        todo!()
    }
    fn pid_alive(&self, pid: i64) -> bool {
        todo!()
    }
    fn agent_bins(&self) -> Vec<String> {
        todo!()
    }
    fn agent_pid_of(&self, run: &str) -> Option<i64> {
        todo!()
    }
    fn group_exists(&self, pgid: i64) -> bool {
        todo!()
    }
    fn proc_scopes_moved(&self) {
        todo!()
    }
    fn state(&self) -> wsp_core::context::StateCell<wsp_core::context::RuntimeState> {
        todo!()
    }
    fn box_panels_ctx(&self) -> std::rc::Rc<dyn wsp_account::account::box_panels::BoxPanelsCtx> {
        self.rc()
    }
    async fn store_get(&self, collection: &str, id: &str) -> wsp_account::account::core_types::AccountResult<Option<serde_json::Value>> {
        todo!()
    }
    async fn store_put(&self, collection: &str, id: &str, value: serde_json::Value) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    async fn store_delete(&self, collection: &str, id: &str) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    async fn store_keys(&self, collection: &str) -> wsp_account::account::core_types::AccountResult<Vec<String>> {
        todo!()
    }
    fn on_workspace_deleted(&self, f: std::rc::Rc<dyn Fn(&str)>) {
        self.hear_workspace_deleted(f)
    }
    fn live_has(&self, workspace_id: &str) -> bool {
        todo!()
    }
    async fn thread_launch(
        &self,
        entry: &wsp_account::account::core_types::Entry,
        thread_id: &str,
        root_thread_id: &str,
    ) -> wsp_account::account::core_types::AccountResult<wsp_account::account::channels::ThreadLaunch> {
        todo!()
    }
    fn brokered_env(&self, token: &str, entry: &wsp_account::account::core_types::Entry) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn terminal_sign_in(
        &self,
        harness: &str,
        entry: &wsp_account::account::core_types::Entry,
        token: &str,
    ) -> Option<wsp_machines::machines::brokered::TerminalSignIn> {
        todo!()
    }
    fn terminal_line(&self, harness: &str, program: Option<&str>) -> String {
        todo!()
    }
    async fn thread_folder(
        &self,
        entry: &wsp_account::account::core_types::Entry,
    ) -> wsp_account::account::core_types::AccountResult<String> {
        todo!()
    }
    fn thread_env(&self, entry: &wsp_account::account::core_types::Entry) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn read_checkout_soon(&self, entry: &wsp_account::account::core_types::Entry) {
        todo!()
    }
    async fn delete_workspace(&self, workspace_id: &str) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn git_origin(&self, path: &str, timeout_ms: i64) -> (Option<i32>, String) {
        todo!()
    }
    fn now(&self) -> i64 {
        todo!()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> wsp_core::clock::Cancel {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::proc_scopes::ProcScopesCtx for Ctx {
    fn turn_groups(&self) -> Vec<(String, Vec<(i64, String)>)> {
        todo!()
    }
    fn group_exists(&self, pgid: i64) -> bool {
        todo!()
    }
    fn sessions(&self) -> Vec<wsp_account::account::proc_scopes::ScopeRow> {
        todo!()
    }
    fn kept_agents(&self) -> Vec<(String, wsp_account::account::proc_scopes::KeptRow)> {
        todo!()
    }
    fn latest_on(&self, thread_id: &str) -> Option<wsp_account::account::proc_scopes::LatestRow> {
        todo!()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        todo!()
    }
    fn thread_terminals(&self) -> Vec<wsp_account::account::proc_scopes::ThreadTerminal> {
        todo!()
    }
    fn live_record(&self, workspace_id: &str) -> Option<wsp_account::account::proc_scopes::ScopeRecord> {
        todo!()
    }
    fn shared_daemon(&self, kind: wsp_wire::views::workspace_kind::WorkspaceKind) -> bool {
        todo!()
    }
    async fn channel_here(
        &self,
    ) -> wsp_account::account::core_types::AccountResult<std::rc::Rc<dyn wsp_core::daemon_channel::DaemonChannel>> {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::box_panels::BoxPanelsCtx for Ctx {
    fn on_workspace_deleted(&self, f: std::rc::Rc<dyn Fn(&str)>) {
        self.hear_workspace_deleted(f)
    }
    fn session_rows(&self) -> Vec<wsp_account::account::box_panels::StartedRow> {
        todo!()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        todo!()
    }
    fn link(&self, place_id: &str) -> Option<std::rc::Rc<()>> {
        todo!()
    }
    fn name_of(&self, place_id: &str) -> String {
        todo!()
    }
    async fn daemon_version(&self, place_id: &str) -> Option<i64> {
        todo!()
    }
    async fn watch_sys(&self, place_id: &str) -> Result<wsp_account::account::core_types::Release, String> {
        todo!()
    }
    fn scopes_on(&self, place_id: &str) -> Vec<wsp_wire::wire::daemon::ProcScope> {
        todo!()
    }
    fn thread_env(&self, entry: &wsp_account::account::core_types::Entry) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}

impl wsp_threads::threads::trimmed_row::TrimmedCtx for Ctx {
    fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        todo!()
    }
    fn folder_of(&self, workspace_id: &str, session: &str) -> Option<String> {
        todo!()
    }
    fn thread_words(&self, workspace_id: &str, thread_id: &str) -> Option<wsp_core::types::internal::ThreadWords> {
        todo!()
    }
}

impl wsp_threads::threads::waits::WaitsCtx for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        todo!()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        todo!()
    }
    fn closing(&self) -> bool {
        todo!()
    }
    fn record(&self, event: wsp_wire::views::session_events_more::SessionEvent) {
        todo!()
    }
}
