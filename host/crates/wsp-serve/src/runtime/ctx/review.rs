// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::workspaces::review::ReviewCtx for Ctx {
    fn project_by_repo(
        &self,
        repo: &str,
        named: Option<&str>,
    ) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    async fn issue_of(&self, remote: &str, number: i64) -> Result<wsp_wire::types::start::IssueRead, wsp_words::exit::HostError> {
        todo!()
    }
    async fn pull_request_of(
        &self,
        remote: &str,
        number: i64,
    ) -> Result<wsp_wire::types::pull_request::PullRequest, wsp_words::exit::HostError> {
        todo!()
    }
    fn from_of(
        &self,
        kind: wsp_wire::types::start::WorkspaceFromKind,
        repo: &str,
        read: &wsp_wire::types::start::IssueRead,
        fact: Option<&wsp_wire::types::pull_request::PullRequest>,
    ) -> wsp_wire::types::start::WorkspaceFrom {
        todo!()
    }
    async fn picks_hold(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        picks: &wsp_core::types::harness::StartPicksAsked,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn workspace_from(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        from: wsp_wire::types::start::WorkspaceFrom,
        fact: Option<wsp_wire::types::pull_request::PullRequest>,
        kind: wsp_account::projects::start_from::StartKind,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_account::projects::start_from::Placed, wsp_words::exit::HostError> {
        todo!()
    }
    async fn open_with(
        &self,
        placed: &wsp_account::projects::start_from::Placed,
        o: wsp_account::projects::start_from::OpenAsk,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_wire::views::harness::StartResult, wsp_words::exit::HostError> {
        todo!()
    }
    fn read_only_of(&self, agent: &str) -> Option<String> {
        todo!()
    }
    fn reviewers(&self) -> Vec<String> {
        todo!()
    }
    fn drop_page_cache(&self, remote: &str, number: i64) {
        todo!()
    }
    fn reread_pull_request(&self, entry: &wsp_account::projects::workspaces::seam::Entry) {
        todo!()
    }
    fn label_of(&self, entry: &wsp_account::projects::workspaces::seam::Entry, thread_id: Option<&str>) -> String {
        todo!()
    }
    async fn daemon_road(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
    ) -> Result<wsp_wire::views::workspace::DaemonReachView, wsp_words::exit::HostError> {
        todo!()
    }
    fn forget_daemon_token(&self, machine_id: &str) {
        todo!()
    }
    fn shared_daemon(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    async fn copy_channel(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        on_event: wsp_core::types::api::DaemonEventListener,
    ) -> Result<std::rc::Rc<dyn wsp_core::types::pending::DaemonChannel>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn shared_channel(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        heard: wsp_core::types::api::DaemonEventListener,
    ) -> Result<std::rc::Rc<dyn wsp_core::types::pending::DaemonChannel>, wsp_words::exit::HostError> {
        todo!()
    }
    fn port_pids(&self, workspace_id: &str) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, i64>>> {
        todo!()
    }
    fn port_roots_moved(&self, workspace_id: &str) {
        todo!()
    }
    async fn guest_road_channel(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        on_event: wsp_core::types::api::DaemonEventListener,
    ) -> Result<std::rc::Rc<dyn wsp_core::types::pending::DaemonChannel>, wsp_words::exit::HostError> {
        todo!()
    }
    fn served_by_its_computer(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Option<String> {
        todo!()
    }
    fn metrics_on_host(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    fn machine_word(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> String {
        todo!()
    }
    fn sys_samples(&self, listener: wsp_core::types::wiring::SysListener) -> Option<wsp_core::types::wiring::Detach> {
        todo!()
    }
    async fn kind_port_reach(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        port: u16,
        standing: bool,
    ) -> Option<Result<wsp_wire::views::workspace::PortReachView, wsp_words::exit::HostError>> {
        todo!()
    }
    async fn ws_port_reach(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        port: u16,
    ) -> Result<wsp_wire::views::workspace::PortReachView, wsp_words::exit::HostError> {
        todo!()
    }
    async fn ws_remint_port_reach(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        port: u16,
    ) -> Result<wsp_wire::views::workspace::PortReachView, wsp_words::exit::HostError> {
        todo!()
    }
    async fn probe(&self, url: &str) -> Result<wsp_wire::views::workspace::PortProbeView, wsp_words::exit::HostError> {
        todo!()
    }
    fn refusal_for(
        &self,
        record: Option<&wsp_core::types::wiring::WorkspaceRecord>,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn reaches_row(&self, thread_id: &str, workspace_id: &str, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::projects::ProjectsCtx for Ctx {
    fn state_folder(&self) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn has_state_path(&self) -> bool {
        todo!()
    }
    fn seed_wiring(&self) -> Option<std::rc::Rc<dyn wsp_core::types::wiring::SeedWiring>> {
        todo!()
    }
    fn cloud_projects(&self) -> Option<bool> {
        todo!()
    }
    fn vault_github_token(&self) -> Option<String> {
        todo!()
    }
    fn local_claude_home(&self) -> Option<String> {
        todo!()
    }
    async fn homes_here(&self) -> Result<indexmap::IndexMap<String, String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn refuse_recording(&self, word: &str, origin: Option<&wsp_wire::views::workspace::Caller>) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn computer_rows(&self) -> Result<Vec<wsp_account::projects::projects::add::ComputerRow>, wsp_words::exit::HostError> {
        todo!()
    }
    fn name_of_computer(&self, id: &str, rows: &[wsp_account::projects::projects::add::ComputerRow]) -> String {
        todo!()
    }
    async fn clone_here(
        &self,
        word: &str,
        source: &wsp_wire::views::workspace::ProjectSource,
        into: &str,
    ) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn git_top_of(&self, path: &str) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn remote_here(&self, folder: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn forking_at(
        &self,
        place_id: Option<&str>,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn landing_backend(
        &self,
        place_id: &str,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    fn folder_computer(&self, place_id: &str) -> Option<wsp_account::projects::projects::add::JoinedComputer> {
        todo!()
    }
    fn github_from_vault(&self, place_id: &str) -> bool {
        todo!()
    }
    fn landing_road(
        &self,
        kind: wsp_account::projects::projects::add::LandingKind,
    ) -> std::rc::Rc<dyn wsp_account::projects::projects::add::LandingRoad> {
        todo!()
    }
    fn project_source(
        &self,
        kind: &wsp_wire::views::workspace::ProjectSource,
    ) -> std::rc::Rc<dyn wsp_account::projects::projects::add::ProjectSourceModule> {
        todo!()
    }
    async fn remember_project(&self, project: wsp_wire::views::workspace::ProjectView) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn project_reached(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        project_id: &str,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn project_of_scope(&self, origin: Option<&wsp_wire::views::workspace::Caller>) -> Option<String> {
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
    fn of_threads_repository(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        project_id: &str,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn holds_thread(&self, workspace_id: &str) -> bool {
        todo!()
    }
    async fn delete_workspace(
        &self,
        id: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn copier_worktree_forget(&self, from: &str, home: &str, project: &str) -> Option<Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    async fn forget_icon(&self, project_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn set_icon(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_icons(&self, hashes: Vec<String>) -> Result<indexmap::IndexMap<String, Option<String>>, wsp_words::exit::HostError> {
        todo!()
    }
    fn runs_agent(&self, agent: &str) -> bool {
        todo!()
    }
    async fn agent_lists(
        &self,
        computer: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<Vec<wsp_wire::views::harness::HarnessCatalog>, wsp_words::exit::HostError> {
        todo!()
    }
    fn thread_defaults(
        &self,
        project_id: &str,
        first_agent: &str,
        lists: &[wsp_wire::views::harness::HarnessCatalog],
    ) -> wsp_wire::types::thread_defaults::ThreadDefaults {
        todo!()
    }
    fn first_agent(&self) -> String {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::projects::moves::MoveRoads for Ctx {
    async fn agents_on_machine(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        agents: &[String],
    ) -> Result<Vec<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn guest_agent_homes(&self) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn scratch(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn land_bundle(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        tar: Vec<u8>,
        dest: &str,
        replace: Option<bool>,
        hooks: wsp_account::projects::projects::moves::LandHooks,
    ) -> Result<u64, wsp_words::exit::HostError> {
        todo!()
    }
    async fn import_into(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        tar: Vec<u8>,
        hooks: wsp_account::projects::projects::moves::LandHooks,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn merge_on_machine(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        script: &str,
        agent: wsp_wire::views::project_bundle::ProjectAgentResult,
    ) -> Result<wsp_wire::views::project_bundle::ProjectAgentResult, wsp_words::exit::HostError> {
        todo!()
    }
    fn project_of(&self, dest: &str, bytes: u64) -> Option<serde_json::Value> {
        todo!()
    }
    async fn provider_row(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    async fn export_folder(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        source: &str,
        caches: &wsp_core::types::pending::CacheRule,
        archive: &str,
        timeout_ms: u64,
        on_progress: std::rc::Rc<dyn Fn(wsp_account::projects::projects::moves::Progress)>,
    ) -> Result<Vec<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn export_paths_into(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        groups: Vec<(String, Vec<String>)>,
        archive: &str,
        timeout_ms: u64,
        on_progress: std::rc::Rc<dyn Fn(wsp_account::projects::projects::moves::Progress)>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn machine_run(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        script: &str,
        deadline_ms: u64,
    ) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    async fn machine_exec(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        cmd: &str,
        timeout_ms: u64,
    ) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    async fn kind_import(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        o: wsp_core::types::wiring::ProjectImportOptions,
        report: wsp_core::types::internal::ImportReport,
    ) -> Result<wsp_core::types::internal::ImportLanded, wsp_words::exit::HostError> {
        todo!()
    }
    fn temp_dir(&self) -> String {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::projects::add::LandingRoad for Ctx {
    fn kind(&self) -> wsp_account::projects::projects::add::LandingKind {
        todo!()
    }
    fn path(
        &self,
        name: &str,
        source: &wsp_wire::views::workspace::ProjectSource,
        deps: &wsp_account::projects::projects::add::LandingDeps,
    ) -> String {
        todo!()
    }
    fn memory_dir(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        memory_key: &str,
        deps: &wsp_account::projects::projects::add::LandingDeps,
    ) -> String {
        todo!()
    }
    fn lands_at_add(&self, seeding: bool) -> bool {
        todo!()
    }
    async fn land(
        &self,
        ask: wsp_account::projects::projects::add::LandAsk,
        deps: &wsp_account::projects::projects::add::LandingDeps,
    ) -> Result<wsp_account::projects::projects::add::Landed, wsp_words::exit::HostError> {
        todo!()
    }
    async fn remove(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        deps: &wsp_account::projects::projects::add::LandingDeps,
    ) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn folder_stands(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        deps: &wsp_account::projects::projects::add::LandingDeps,
    ) -> Option<Result<bool, wsp_words::exit::HostError>> {
        todo!()
    }
    fn takes_files(&self) -> bool {
        todo!()
    }
    async fn seed_into(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        tar: Vec<u8>,
        deps: &wsp_account::projects::projects::add::LandingDeps,
    ) -> Option<Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    async fn unsaved(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        deps: &wsp_account::projects::projects::add::LandingDeps,
    ) -> Option<Result<Option<String>, wsp_words::exit::HostError>> {
        todo!()
    }
    async fn branch(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        deps: &wsp_account::projects::projects::add::LandingDeps,
    ) -> Result<wsp_wire::views::workspace_more::ProjectBranch, wsp_words::exit::HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::images::ProjectIconsCtx for Ctx {
    fn state_folder(&self) -> Result<std::path::PathBuf, wsp_words::exit::HostError> {
        todo!()
    }
    fn has_state_path(&self) -> bool {
        todo!()
    }
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn project_held(&self, project_id: &str) -> bool {
        todo!()
    }
    async fn preferences(&self) -> Result<wsp_wire::views::preferences::Preferences, wsp_words::exit::HostError> {
        todo!()
    }
    async fn change_preferences(
        &self,
        change: Box<dyn FnOnce(wsp_wire::views::preferences::Preferences) -> wsp_wire::views::preferences::Preferences>,
    ) -> Result<wsp_wire::views::preferences::Preferences, wsp_words::exit::HostError> {
        todo!()
    }
    fn recipes(&self) -> Option<std::rc::Rc<dyn wsp_core::types::wiring::RecipeShelf>> {
        todo!()
    }
    fn on_recipes_changed(&self, listener: std::rc::Rc<dyn Fn()>) {
        let _ = self.hear("recipes.changed", move |_| listener());
    }
    fn ids(&self) -> std::rc::Rc<dyn wsp_core::ids::Ids> {
        self.ids.clone()
    }
}
