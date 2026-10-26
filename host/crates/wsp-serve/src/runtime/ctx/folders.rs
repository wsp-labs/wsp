// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::workspaces::seam::Base for Ctx {
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        todo!()
    }
    fn ids(&self) -> std::rc::Rc<dyn wsp_core::ids::Ids> {
        todo!()
    }
    fn store(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Store> {
        todo!()
    }
    fn live(&self) -> wsp_account::projects::workspaces::seam::Live {
        todo!()
    }
    fn projects_held(&self) -> wsp_account::projects::workspaces::seam::ProjectsHeld {
        todo!()
    }
    fn project_held(&self, project_id: &str) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn session_views(&self) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn thread_records(&self) -> Vec<(String, wsp_core::types::internal::ThreadRecord)> {
        todo!()
    }
    async fn persist(&self, record: wsp_core::types::wiring::WorkspaceRecord) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn view(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_wire::views::workspace_more::WorkspaceView {
        todo!()
    }
    fn emit(&self, event: wsp_account::projects::workspaces::seam::Event) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn entry_of(
        &self,
        id: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
        ask: wsp_machines::machines::boot::entries::EntryAsk,
    ) -> Result<wsp_account::account::core_types::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn spawn_guard(
        &self,
        act: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_account::projects::workspaces::seam::FreeSlot, wsp_words::exit::HostError> {
        todo!()
    }
    fn kind_of(&self, computer: &str) -> wsp_wire::views::workspace_kind::WorkspaceKind {
        todo!()
    }
    fn place_name(&self, place_id: &str) -> String {
        todo!()
    }
    async fn copy_blocked(&self, entry: &wsp_account::account::core_types::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn with_daemon(
        &self,
        entry: &wsp_account::account::core_types::Entry,
        work: wsp_account::projects::workspaces::seam::DaemonWork,
    ) -> Result<serde_json::Value, wsp_words::exit::HostError> {
        todo!()
    }
    async fn on_this_computer(
        &self,
        work: wsp_account::projects::workspaces::seam::HereWork,
    ) -> Result<serde_json::Value, wsp_words::exit::HostError> {
        todo!()
    }
    async fn git_here(
        &self,
        ask: wsp_account::projects::workspaces::seam::GitAsk,
    ) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    fn checkout_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn preferences(&self) -> Result<wsp_wire::views::preferences::Preferences, wsp_words::exit::HostError> {
        todo!()
    }
    async fn write_daemon_roots(
        &self,
        entry: &wsp_account::account::core_types::Entry,
        strict: bool,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn home_dir(&self) -> String {
        todo!()
    }
    fn machine_id(&self, entry: &wsp_account::account::core_types::Entry) -> String {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::folders::FoldersCtx for Ctx {
    fn state_folder(&self) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn has_state_path(&self) -> bool {
        todo!()
    }
    fn has_copier(&self) -> bool {
        todo!()
    }
    async fn copier_worktree(
        &self,
        ask: wsp_wire::views::workspace_more::WorktreeAsk,
    ) -> Result<wsp_wire::views::workspace_more::WorktreeReport, wsp_words::exit::HostError> {
        todo!()
    }
    async fn copier_worktree_mount(&self, from: &str, home: &str, path: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn copier_worktree_remove(&self, ask: wsp_engine::copier::WorktreeRemove) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn here_daemon(&self) -> Option<std::rc::Rc<dyn wsp_core::types::wiring::HereDaemon>> {
        todo!()
    }
    async fn admit_folder(
        &self,
        kind: wsp_wire::views::workspace_kind::WorkspaceKind,
        name: &str,
        place: Option<&str>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn folder_machine(
        &self,
        kind: wsp_wire::views::workspace_kind::WorkspaceKind,
        place: Option<&str>,
    ) -> Result<(std::sync::Arc<dyn wsp_engine::machine::Machine>, wsp_wire::wire::capabilities::WorkspaceSize), wsp_words::exit::HostError>
    {
        todo!()
    }
    fn attach(
        &self,
        record: wsp_core::types::wiring::WorkspaceRecord,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> wsp_account::projects::workspaces::seam::Entry {
        todo!()
    }
    fn folders_of(&self, project_id: &str) -> Vec<wsp_account::projects::workspaces::seam::Entry> {
        todo!()
    }
    async fn one_folder(
        &self,
        key: &str,
        make: wsp_account::projects::workspaces::seam::Queued<wsp_account::projects::workspaces::seam::Entry>,
    ) -> Result<wsp_account::projects::workspaces::seam::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    async fn resolve_project(
        &self,
        word: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn elsewhere_refusal(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        project: &wsp_wire::views::workspace::ProjectView,
        word: &str,
    ) -> Result<Option<wsp_words::exit::HostError>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn picks_hold(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        picks: &wsp_core::types::harness::StartPicksAsked,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn branch_at(&self, path: &str) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn turn_runs(&self, workspace_id: &str) -> bool {
        todo!()
    }
    async fn drop_workspace(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn checkpoints_landed(&self, thread_id: &str) {
        todo!()
    }
    fn not_a_repo(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> bool {
        todo!()
    }
    async fn read_pull_request(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        force: bool,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn status_now(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn local_exec(
        &self,
        cmd: &str,
        timeout_ms: u64,
        stdin: Option<Vec<u8>>,
    ) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    fn state(&self) -> wsp_core::context::StateCell<wsp_core::context::RuntimeState> {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::start_from::StartFromCtx for Ctx {
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn places_wired(&self) -> String {
        todo!()
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn landing_kind(&self, computer: &str) -> wsp_account::projects::projects::add::LandingKind {
        todo!()
    }
    fn brokered_start(&self, provider: bool) -> bool {
        todo!()
    }
    async fn pull_request_of_record(
        &self,
        remote: &str,
        number: i64,
        project: &str,
        computer: &str,
        start: bool,
    ) -> Result<Option<wsp_wire::views::workspace::BranchOfRecord>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn create_workspace(
        &self,
        opts: wsp_core::types::wiring::CreateWorkspaceOptions,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_core::types::wiring::CreatedWorkspace, wsp_words::exit::HostError> {
        todo!()
    }
    async fn delete_workspace(
        &self,
        id: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn project_folder(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
    ) -> Result<wsp_account::projects::workspaces::seam::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    async fn worktree_folder(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        top: &str,
        branch: &str,
    ) -> Result<wsp_account::projects::workspaces::seam::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    async fn worktrees_of(
        &self,
        top: &str,
    ) -> Result<Vec<wsp_account::projects::folders::tree::ListedWorktree>, wsp_words::exit::HostError> {
        todo!()
    }
    fn take_setup_line(&self, workspace_id: &str) -> Option<String> {
        todo!()
    }
    fn preferences_held(&self) -> Option<wsp_wire::views::preferences::Preferences> {
        todo!()
    }
    fn default_agent_of(
        &self,
        prefs: &wsp_wire::views::preferences::Preferences,
        entry: &wsp_account::projects::workspaces::seam::Entry,
    ) -> String {
        todo!()
    }
    async fn resolved_picks(
        &self,
        home: &wsp_account::projects::workspaces::seam::Entry,
        harness: &str,
        prefs: &wsp_wire::views::preferences::Preferences,
        project_id: &str,
    ) -> Result<Option<wsp_account::projects::start_from::ResolvedPicks>, wsp_words::exit::HostError> {
        todo!()
    }
    fn access_word_refusal(
        &self,
        catalog: &wsp_wire::views::harness::HarnessCatalog,
        access: &wsp_wire::types::thread_defaults::AccessChoice,
    ) -> Option<wsp_words::exit::HostError> {
        todo!()
    }
    fn named_mode(
        &self,
        catalog: &wsp_wire::views::harness::HarnessCatalog,
        harness: &str,
        access: &wsp_wire::types::thread_defaults::AccessChoice,
    ) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn start_picks_refusal(
        &self,
        catalog: &wsp_wire::views::harness::HarnessCatalog,
        picks: &wsp_core::types::harness::StartPicksAsked,
    ) -> Option<wsp_words::exit::HostError> {
        todo!()
    }
    async fn start_session(
        &self,
        workspace_id: &str,
        opts: serde_json::Value,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_account::projects::start_from::OpenedSession, wsp_words::exit::HostError> {
        todo!()
    }
    fn read_only_modes(&self) -> Vec<(String, Option<String>)> {
        todo!()
    }
    async fn send_detached(&self, workspace_id: &str, prompt: &str, thread: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::workspaces::create::CreateCtx for Ctx {
    fn places_wired(&self) -> String {
        todo!()
    }
    async fn resolve_project(
        &self,
        word: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn landing_capabilities(
        &self,
        place_id: Option<&str>,
    ) -> Result<wsp_wire::wire::capabilities::Capabilities, wsp_words::exit::HostError> {
        todo!()
    }
    fn backend_capabilities(
        &self,
        kind: wsp_wire::views::workspace_kind::WorkspaceKind,
        place: Option<&str>,
    ) -> Result<wsp_wire::wire::capabilities::Capabilities, wsp_words::exit::HostError> {
        todo!()
    }
    fn act_refusal(
        &self,
        scope: &wsp_wire::views::workspace_more::ThreadScope,
        act: &str,
    ) -> Result<wsp_words::exit::HostError, wsp_words::exit::HostError> {
        todo!()
    }
    async fn spawn_sub(
        &self,
        opts: wsp_core::types::wiring::CreateWorkspaceOptions,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_core::types::wiring::CreatedWorkspace, wsp_words::exit::HostError> {
        todo!()
    }
    fn place_of_thread(&self, scope: &wsp_wire::views::workspace_more::ThreadScope) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn name_given(&self, name: &str) -> String {
        todo!()
    }
    fn refuse_recording(&self, word: &str, origin: Option<&wsp_wire::views::workspace::Caller>) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn refuse_relayed(
        &self,
        kind: wsp_wire::views::workspace_kind::WorkspaceKind,
        name: &str,
        project: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn project_folder(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
    ) -> Result<wsp_account::projects::workspaces::seam::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn agents_from(&self, place: &str, agents: &serde_json::Map<String, serde_json::Value>) -> wsp_wire::views::workspace::WorkspaceAgents {
        todo!()
    }
    fn same_repository(
        &self,
        a: &wsp_wire::views::workspace::ProjectView,
        b: &wsp_wire::views::workspace::ProjectView,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> bool {
        todo!()
    }
    async fn place_guard(&self, place_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        todo!()
    }
    fn forking_add(&self, name: &str) {
        todo!()
    }
    fn forking_delete(&self, name: &str) {
        todo!()
    }
    fn supersede_failed(&self, name: &str) {
        todo!()
    }
    fn stage_reporter(
        &self,
        id: &str,
        name: &str,
        began: i64,
        scope: Option<&wsp_wire::views::workspace_more::ThreadScope>,
    ) -> wsp_core::types::wiring::StageReport {
        todo!()
    }
    async fn lead_start(
        &self,
        parent: &wsp_account::projects::workspaces::seam::Entry,
        name: &str,
        how: &str,
    ) -> Result<wsp_account::projects::workspaces::create::LeadStart, wsp_words::exit::HostError> {
        todo!()
    }
    fn show_create(&self, view: wsp_wire::views::workspace_more::WorkspaceView) {
        todo!()
    }
    fn shown_view(
        &self,
        id: &str,
        name: &str,
        kind: wsp_wire::views::workspace_kind::WorkspaceKind,
        golden: &str,
        created_at: &str,
        project: &wsp_wire::views::workspace::ProjectView,
        scope: &wsp_wire::views::workspace_more::ThreadScope,
        parent: Option<&str>,
    ) -> wsp_wire::views::workspace_more::WorkspaceView {
        todo!()
    }
    async fn create_staged(
        &self,
        opts: wsp_core::types::wiring::CreateWorkspaceOptions,
        project: &wsp_wire::views::workspace::ProjectView,
        id: &str,
        report: wsp_core::types::wiring::StageReport,
        scope: Option<wsp_wire::views::workspace_more::ThreadScope>,
        free: wsp_account::projects::workspaces::seam::FreeSlot,
        base: Option<String>,
    ) -> Result<wsp_core::types::wiring::CreatedWorkspace, wsp_words::exit::HostError> {
        todo!()
    }
    fn child_started_line(&self, name: &str, branch: &str) -> String {
        todo!()
    }
    async fn start_child_on(
        &self,
        child: &wsp_account::projects::workspaces::seam::Entry,
        base: &str,
        parent_name: &str,
    ) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn unfork(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_engine::errors::EngineError> {
        todo!()
    }
    fn shown_creates(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_wire::views::workspace_more::WorkspaceView>>> {
        todo!()
    }
    fn failed_creates(
        &self,
    ) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_wire::views::workspace_more::WorkspaceView>>> {
        todo!()
    }
    fn failed_view(
        &self,
        id: &str,
        name: &str,
        kind: wsp_wire::views::workspace_kind::WorkspaceKind,
        golden: &str,
        began: i64,
        project: &wsp_wire::views::workspace::ProjectView,
        said: &str,
    ) -> wsp_wire::views::workspace_more::WorkspaceView {
        todo!()
    }
    async fn boot_work(&self, id: &str) {
        todo!()
    }
    fn listed_for(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<Vec<wsp_account::projects::workspaces::seam::Entry>, wsp_words::exit::HostError> {
        todo!()
    }
    fn held(&self) -> Vec<wsp_account::projects::workspaces::seam::Entry> {
        todo!()
    }
    fn drives(&self, view: &wsp_wire::views::workspace_more::WorkspaceView, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn talks_to_tree_on(&self, id: &str, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn project_of_scope(&self, origin: Option<&wsp_wire::views::workspace::Caller>) -> Option<String> {
        todo!()
    }
    fn refusal_for(
        &self,
        record: Option<&wsp_core::types::wiring::WorkspaceRecord>,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn of_threads_repository(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        project_id: &str,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn elsewhere_refusal(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        project: &wsp_wire::views::workspace::ProjectView,
        word: &str,
    ) -> Result<Option<wsp_words::exit::HostError>, wsp_words::exit::HostError> {
        todo!()
    }
    fn away_for(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        act: &str,
        word: &str,
    ) -> Result<Option<wsp_words::exit::HostError>, wsp_words::exit::HostError> {
        todo!()
    }
    fn project_reached(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        project_id: &str,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    async fn find_places(&self, word: &str) -> Result<Vec<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn create_stages(&self) -> Vec<wsp_wire::views::workspace_events::WorkspaceCreatingEvent> {
        todo!()
    }
}

impl wsp_account::projects::workspaces::WorkspacesCtx for Ctx {}
