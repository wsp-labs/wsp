// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use std::rc::Rc;

use wsp_core::types::events::EventBus as _;

use super::Ctx;

/// The areas this packet's tests read at their own seams: `Ctx`'s slots are crate-private.
pub struct PreferencesSeams {
    pub preferences: Rc<wsp_account::account::preferences::PreferencesArea>,
    pub agents_read: Rc<wsp_machines::agents_read::AgentsReads>,
    pub activity: Rc<wsp_threads::activity::Activity>,
    pub slates: Rc<dyn wsp_slates::slates::Slates>,
    pub pipe: Option<Rc<wsp_core::pipe::area::PipeArea>>,
    pub agents: Rc<wsp_threads::threads::agents::AgentsArea>,
    pub threads: Rc<wsp_threads::threads::threads::ThreadsArea>,
    pub transcripts: Rc<wsp_threads::threads::transcripts::TranscriptsArea>,
    pub folders: Rc<wsp_account::projects::folders::Folders>,
}

impl Ctx {
    pub fn preferences_seams(&self) -> PreferencesSeams {
        PreferencesSeams {
            preferences: self.preferences.get(),
            agents_read: self.agents_read(),
            activity: self.activity.get(),
            slates: self.slates.get(),
            pipe: self.pipe.get(),
            agents: self.agents.get(),
            threads: self.threads.get(),
            transcripts: self.transcripts.get(),
            folders: self.folders.get(),
        }
    }

    /// `agentsReads({ reader, places, workspace, .. })` over the context (preferences.ts:99-151).
    pub(super) fn build_agents_read(&self) -> Rc<wsp_machines::agents_read::AgentsReads> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::preferences::PreferencesCtx for Ctx {
    fn env(&self) -> indexmap::IndexMap<String, String> {
        match &self.opts.env {
            Some(held) => held.iter().filter_map(|(k, v)| v.clone().map(|v| (k.clone(), v))).collect(),
            None => std::env::vars().collect(),
        }
    }
    fn server_icons(&self) -> Option<std::rc::Rc<dyn wsp_account::account::preferences::ServerIcons>> {
        todo!()
    }
    fn home_dir(&self) -> String {
        todo!()
    }
    fn adapter_ids(&self) -> Vec<String> {
        todo!()
    }
    fn access_table(&self, agent: &str) -> wsp_account::account::preferences::AccessTable {
        todo!()
    }
    fn project_held(&self, id: &str) -> bool {
        todo!()
    }
    fn project_of(&self, id: &str) -> Result<(String, String), wsp_words::exit::HostError> {
        todo!()
    }
    fn default_agent_of(&self, prefs: &wsp_wire::views::preferences::Preferences) -> String {
        todo!()
    }
    fn session_thread_keys(&self) -> Vec<String> {
        todo!()
    }
    fn thread_record_held(&self, thread_id: &str) -> bool {
        todo!()
    }
    async fn ready(&self) {
        todo!()
    }
    async fn stored(&self) -> wsp_account::account::core_types::AccountResult<Option<serde_json::Value>> {
        todo!()
    }
    async fn store(&self, prefs: &wsp_wire::views::preferences::Preferences) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn held(&self) -> Option<wsp_wire::views::preferences::Preferences> {
        todo!()
    }
    fn set_held(&self, prefs: wsp_wire::views::preferences::Preferences) {
        todo!()
    }
    fn emit(&self, event: serde_json::Value) {
        todo!()
    }
    fn on_workspace_deleted(&self, f: std::rc::Rc<dyn Fn(&str)>) {
        self.hear_workspace_deleted(f)
    }
    async fn entry_of(
        &self,
        workspace_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_account::account::core_types::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn catalog_agents(&self) -> Vec<wsp_account::account::preferences::CatalogAgentKeys> {
        todo!()
    }
    fn thread_env(&self, entry: &wsp_account::account::core_types::Entry, agent: &str) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn checkout_of(&self, entry: &wsp_account::account::core_types::Entry) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn is_local(&self, entry: &wsp_account::account::core_types::Entry) -> bool {
        todo!()
    }
    fn at_provider(&self, entry: &wsp_account::account::core_types::Entry) -> bool {
        todo!()
    }
    fn pause_mode(&self, entry: &wsp_account::account::core_types::Entry) -> Option<String> {
        todo!()
    }
    fn projects_on(&self, place_id: &str) -> Vec<(String, String, String)> {
        todo!()
    }
    async fn workspace_channel(
        &self,
        workspace_id: &str,
        on_event: std::rc::Rc<dyn Fn(serde_json::Value)>,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> wsp_account::account::core_types::AccountResult<std::rc::Rc<dyn wsp_core::daemon_channel::DaemonChannel>> {
        todo!()
    }
    async fn local_channel(
        &self,
        on_event: std::rc::Rc<dyn Fn(serde_json::Value)>,
    ) -> wsp_account::account::core_types::AccountResult<std::rc::Rc<dyn wsp_core::daemon_channel::DaemonChannel>> {
        todo!()
    }
    fn place_channel(
        &self,
        place_id: &str,
        on_event: std::rc::Rc<dyn Fn(serde_json::Value)>,
    ) -> Option<std::rc::Rc<dyn wsp_core::daemon_channel::DaemonChannel>> {
        todo!()
    }
    fn place_name(&self, place_id: &str) -> Option<String> {
        todo!()
    }
    async fn setup_write(
        &self,
        place_id: &str,
        agent: &str,
        change: serde_json::Value,
    ) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn setup_view(&self, place_id: &str, agent: &str) -> serde_json::Value {
        todo!()
    }
    fn drop_setup_refusal(&self, place_id: &str, agent: &str) -> bool {
        todo!()
    }
    fn setup_refusal_moved(&self, place_id: &str, agent: &str) {
        todo!()
    }
    fn relayed(&self) -> bool {
        todo!()
    }
    fn forget_read(&self, workspace_id: &str) {
        todo!()
    }
}

impl wsp_account::account::preferences::ServerIcons for Ctx {
    fn folder(&self) -> String {
        todo!()
    }
    fn forget(&self) -> Result<(), String> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_threads::activity::ActivityCtx for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        todo!()
    }
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn has_door(&self) -> bool {
        todo!()
    }
    fn place_name(&self, place: &str) -> Option<String> {
        todo!()
    }
    async fn place_rows(&self) -> Result<Vec<wsp_wire::views::place_more::PlaceView>, wsp_words::exit::HostError> {
        todo!()
    }
    fn joined(&self, place: &str) -> bool {
        todo!()
    }
    fn has_link(&self, place: &str) -> bool {
        todo!()
    }
    fn has_local_daemon(&self) -> bool {
        todo!()
    }
    async fn local_channel(
        &self,
        on_event: wsp_core::types::api::DaemonEventListener,
        guarded: bool,
    ) -> Result<std::rc::Rc<dyn wsp_threads::activity::DaemonAsk>, wsp_words::exit::HostError> {
        todo!()
    }
    fn box_channel(
        &self,
        place: &str,
        on_event: wsp_core::types::api::DaemonEventListener,
    ) -> Option<
        futures::future::LocalBoxFuture<'static, Result<std::rc::Rc<dyn wsp_threads::activity::DaemonAsk>, wsp_words::exit::HostError>>,
    > {
        todo!()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        todo!()
    }
    fn records(&self) -> wsp_threads::threads::threads::ThreadRecords {
        todo!()
    }
    fn live_entry(&self, workspace_id: &str) -> Option<wsp_threads::threads::threads::Entry> {
        todo!()
    }
    fn latest_on(&self, thread_id: &str) -> Option<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn thread_facts(&self, thread_id: &str) -> Option<wsp_wire::views::preferences::ThreadFacts> {
        todo!()
    }
    fn scopes_here(&self) -> Vec<wsp_wire::wire::daemon::ProcScope> {
        todo!()
    }
    fn kept_pid(&self, thread_id: &str) -> Option<u32> {
        todo!()
    }
    fn reap_kept_now(&self, thread_id: &str) {
        todo!()
    }
    fn host_pid(&self) -> u32 {
        todo!()
    }
    fn drives(&self, kind: wsp_wire::views::workspace_kind::WorkspaceKind, caller: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn refusal_for(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
        caller: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<Option<wsp_words::exit::HostError>, wsp_words::exit::HostError> {
        todo!()
    }
    fn settles_thread(&self, thread_id: &str, caller: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn end_thread(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_id: &str,
        o: wsp_core::types::internal::EndThread,
    ) -> Option<futures::future::LocalBoxFuture<'static, Result<Option<String>, wsp_words::exit::HostError>>> {
        todo!()
    }
    fn with_daemon(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        frame: serde_json::Value,
    ) -> futures::future::LocalBoxFuture<'static, Result<serde_json::Value, wsp_account::account::core_types::AccountError>> {
        todo!()
    }
    fn close_servers(&self, thread_id: &str) {
        todo!()
    }
}

impl wsp_threads::activity::DaemonAsk for Ctx {
    fn ask(
        &self,
        frame: serde_json::Value,
    ) -> futures::future::LocalBoxFuture<
        'static,
        Result<serde_json::Map<String, serde_json::Value>, wsp_account::account::core_types::AccountError>,
    > {
        todo!()
    }
    fn close(&self) {
        todo!()
    }
}

impl wsp_slates::account::slates::SlatesAreaCtx for Ctx {
    fn store(&self) -> std::rc::Rc<dyn wsp_store::store::Store> {
        todo!()
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        todo!()
    }
    fn record(&self, event: wsp_wire::types::slate_wire::SessionSlateEvent) {
        todo!()
    }
    fn emit(&self, event: wsp_slates::slates::SlateEmit) {
        todo!()
    }
    fn latest_on(&self, thread: &str) -> Option<wsp_slates::slate_sources::context::SessionView> {
        todo!()
    }
    fn thread_record(&self, thread: &str) -> Option<wsp_core::types::internal::ThreadRecord> {
        todo!()
    }
    fn running_turn(&self, thread: &str) -> Option<String> {
        todo!()
    }
    fn live(&self, workspace: &str) -> Option<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn box_of(&self, thread: &str) -> Option<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn place_of(&self, thread: &str) -> Option<std::sync::Arc<wsp_engine::place_machine::PlaceFolderMachine>> {
        todo!()
    }
    fn folder_of(&self, workspace: &str, session: &str) -> Option<String> {
        todo!()
    }
    fn runs_in(&self, entry: &wsp_machines::machines::machines::Entry, ran_in: Option<&str>, fallback: &str) -> String {
        todo!()
    }
    fn kind_folder(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn checkout_of(&self, entry: &wsp_machines::machines::machines::Entry) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn path_exists(&self, path: &str) -> bool {
        todo!()
    }
    fn home_dir(&self) -> String {
        todo!()
    }
    fn computer_of(&self, entry: &wsp_machines::machines::machines::Entry) -> String {
        todo!()
    }
    fn root_of(&self, thread: &str) -> String {
        todo!()
    }
    fn tool_asks(&self, harness: Option<&str>, mode: Option<&str>) -> wsp_slates::slate_mcp::ToolAsks {
        todo!()
    }
    fn entry_rate_usd_per_hour(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<f64> {
        todo!()
    }
    fn at_provider(&self, place: &str) -> bool {
        todo!()
    }
    fn boot_work(&self, workspace: &str) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn wake(&self, workspace: &str) -> futures::future::LocalBoxFuture<'static, Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    fn ready(&self) -> futures::future::LocalBoxFuture<'static, Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    fn settled_now(&self, thread: &str, ms: Option<f64>) -> bool {
        todo!()
    }
    fn settle_after(&self) -> futures::future::LocalBoxFuture<'static, String> {
        todo!()
    }
    fn tree_under(&self, lead: &str) -> Vec<String> {
        todo!()
    }
    fn thread_of_token(&self, token: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn own_process(&self, thread: &str, pid: u32, kind: wsp_frames::ProcKind) -> wsp_core::types::wiring::Detach {
        todo!()
    }
    fn daemon_image(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        path: String,
    ) -> futures::future::LocalBoxFuture<'static, Result<serde_json::Value, wsp_slates::slate_images::ImageRefusal>> {
        todo!()
    }
    fn daemon_hash(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        root: String,
        paths: Vec<String>,
    ) -> futures::future::LocalBoxFuture<'static, Result<serde_json::Value, String>> {
        todo!()
    }
    fn agents_server(
        &self,
        target: wsp_wire::types::agents_report::AgentsTarget,
        agent: &str,
        name: &str,
    ) -> futures::future::LocalBoxFuture<
        'static,
        Result<wsp_slates::account::slates::ReadServer, wsp_slates::account::slates::ServerReadError>,
    > {
        todo!()
    }
    fn agents_reader_server(
        &self,
        projects: Vec<wsp_wire::types::agents_report::AgentsProject>,
        agent: &str,
        name: &str,
    ) -> Option<futures::future::LocalBoxFuture<'static, Result<wsp_slates::slate_mcp::McpServerSpec, wsp_words::exit::HostError>>> {
        todo!()
    }
    fn stdio_road(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        min_version: Option<i64>,
    ) -> futures::future::LocalBoxFuture<'static, Result<std::rc::Rc<dyn wsp_slates::slate_mcp_on::StdioRoad>, String>> {
        todo!()
    }
    fn place_name_here(&self) -> String {
        todo!()
    }
    fn agent_label(&self, agent: &str) -> String {
        todo!()
    }
    fn rows_on(&self, thread: &str) -> Vec<wsp_slates::slate_sources::context::SessionView> {
        todo!()
    }
    fn transcript(&self, workspace: &str) -> Vec<serde_json::Value> {
        todo!()
    }
    fn usage_account_key(&self, entry: &wsp_machines::machines::machines::Entry, harness: &str) -> String {
        todo!()
    }
    fn usage_accounts(&self) -> futures::future::LocalBoxFuture<'static, Vec<wsp_slates::slate_sources::context::AccountRow>> {
        todo!()
    }
    fn project_name(&self, project: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn accrued(&self, workspace: &str) -> Option<f64> {
        todo!()
    }
    fn start_session(
        &self,
        start: wsp_slates::slates::Delivery,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_slates::slates::Delivered, wsp_words::exit::HostError>> {
        todo!()
    }
    fn on_queued(&self, request_id: &str, f: Box<dyn Fn()>) -> Box<dyn FnOnce()> {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn poll_pull_request(&self, entry: &wsp_machines::machines::machines::Entry) {
        todo!()
    }
    fn local_env(&self) -> Option<indexmap::IndexMap<String, String>> {
        todo!()
    }
    fn process_env(&self) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn state_folder(&self) -> Option<String> {
        todo!()
    }
    fn on_napped(&self, f: Box<dyn Fn(&str)>) {
        self.hear_json(
            "workspace.napped",
            Rc::new(move |event| {
                if let Some(workspace_id) = event["workspaceId"].as_str() {
                    f(workspace_id);
                }
            }),
        );
    }
}

impl wsp_core::pipe::area::PipeAreaContext for Ctx {
    fn broker(&self) -> Option<std::rc::Rc<dyn wsp_core::pipe::area::PipeBroker>> {
        self.opts.broker.clone().map(|broker| Rc::new(PipedBroker(broker)) as Rc<dyn wsp_core::pipe::area::PipeBroker>)
    }
    fn live(&self, workspace_id: &str) -> Option<wsp_core::pipe::area::PipeLive> {
        let entry = self.live.borrow().get(workspace_id).cloned()?;
        let entry = entry.borrow();
        let record = entry.record.borrow();
        Some(wsp_core::pipe::area::PipeLive {
            id: record.id.clone(),
            phase: serde_json::to_value(record.phase).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
            machine_id: record.machine_id.clone(),
        })
    }
    fn live_ids(&self) -> Vec<String> {
        self.live.borrow().keys().cloned().collect()
    }
    fn daemon_reach_url(&self, workspace_id: &str) -> futures::future::LocalBoxFuture<'static, Result<String, String>> {
        todo!()
    }
    fn daemon_token_of(&self, workspace_id: &str) -> futures::future::LocalBoxFuture<'static, Result<Option<String>, String>> {
        todo!()
    }
    fn daemon_version(&self, workspace_id: &str) -> futures::future::LocalBoxFuture<'static, Result<Option<i64>, String>> {
        todo!()
    }
    fn forget_daemon_token(&self, machine_id: &str) {
        todo!()
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        self.clock.clone()
    }
    fn on_any(&self, listener: std::rc::Rc<dyn Fn(&wsp_wire::wire::events_union::EventUnion)>) -> Box<dyn FnOnce()> {
        let unlisten = self.bus.on(
            wsp_core::types::events::ANY_EVENT,
            Rc::new(move |e: &wsp_wire::wire::events_union::EventUnion| {
                listener(e);
                Ok(())
            }),
        );
        Box::new(unlisten)
    }
    fn ready(&self) -> futures::future::LocalBoxFuture<'static, Result<(), String>> {
        todo!()
    }
}

/// The host's broker as the pipe pool reads it: a pipe the pool dials is joined to the duplex the broker takes.
struct PipedBroker(Rc<dyn wsp_core::types::wiring::Broker>);

impl wsp_core::pipe::area::PipeBroker for PipedBroker {
    fn ready(&self) -> bool {
        self.0.ready()
    }
    fn pipe(&self) -> Option<wsp_core::pipe::pool::PipeHandler> {
        if !self.0.has_pipe() {
            return None;
        }
        let broker = self.0.clone();
        Some(Rc::new(move |workspace_id: &str, open, io: wsp_core::pipe::pool::PipeDuplex| {
            let open = serde_json::to_value(&open)
                .and_then(serde_json::from_value::<wsp_frames::PipeOpen>)
                .map_err(|e| format!("a pipe's open frame was not read: {e}"))?;
            let (near, mut far) = tokio::io::duplex(64 * 1024);
            broker.pipe(workspace_id, open, near);
            let mut io = io;
            tokio::task::spawn_local(async move {
                let _ = tokio::io::copy_bidirectional(&mut io, &mut far).await;
            });
            Ok(())
        }))
    }
    fn revoke(&self, workspace_id: &str) -> Option<futures::future::LocalBoxFuture<'static, Result<(), String>>> {
        let revoked = self.0.revoke(workspace_id)?;
        Some(Box::pin(async move { revoked.await.map_err(|e| e.message) }))
    }
}
