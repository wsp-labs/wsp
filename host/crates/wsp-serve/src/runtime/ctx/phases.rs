// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::workspaces::phases::PhasesCtx for Ctx {
    async fn unfork(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn sub_lost(&self, entry: &wsp_account::projects::workspaces::seam::Entry, line: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn tell_threads(&self, workspace_id: &str, line: &str) {
        todo!()
    }
    async fn emit_status(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        reach: &str,
        words: Option<&str>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn follow_machine(&self, entry: &wsp_account::projects::workspaces::seam::Entry) {
        todo!()
    }
    fn reach_of(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> String {
        todo!()
    }
    async fn nap_with(&self, id: &str) -> Result<wsp_wire::views::workspace_more::WorkspaceView, wsp_words::exit::HostError> {
        todo!()
    }
    fn refuse_pauseless(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        act: &str,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn talks_to_tree_on(&self, id: &str, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn refuse_sub_wake(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn recover_gone(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
    ) -> Result<Option<wsp_wire::views::workspace::WorkspacePhase>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn machine_state(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
    ) -> Result<wsp_account::projects::workspaces::phases::MachineRead, wsp_engine::errors::EngineError> {
        todo!()
    }
    fn provider_said(&self, e: &wsp_engine::errors::EngineError) -> String {
        todo!()
    }
    async fn settle_gone(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        by: &str,
        at: i64,
        answer: Option<String>,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    async fn adopt_pause(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn runs_under_napping(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    async fn adopt_running(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn proven(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
    ) -> Result<wsp_wire::views::workspace_more::WorkspaceView, wsp_words::exit::HostError> {
        todo!()
    }
    fn resume_asks(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
    ) -> Option<wsp_account::projects::workspaces::phases::ResumeAsks> {
        todo!()
    }
    async fn sleeps(&self, ms: i64) {
        todo!()
    }
    async fn took_the_resume(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    async fn ws_wake(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        landed: bool,
    ) -> Result<wsp_engine::lifecycle::WakeResult, wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn ws_rebuild(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn ws_upgrade(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_engine::errors::EngineError> {
        todo!()
    }
    fn ws_current_phase(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> wsp_wire::views::workspace::WorkspacePhase {
        todo!()
    }
    fn ws_note_paused(&self, entry: &wsp_account::projects::workspaces::seam::Entry) {
        todo!()
    }
    fn arm_late_read(&self, entry: &wsp_account::projects::workspaces::seam::Entry) {
        todo!()
    }
    fn can_resume_at(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    fn provider_of(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Option<String> {
        todo!()
    }
    async fn backend_capabilities(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
    ) -> Result<wsp_wire::wire::capabilities::Capabilities, wsp_words::exit::HostError> {
        todo!()
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        todo!()
    }
    async fn shape_of(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Option<wsp_wire::wire::machine_link::MachineShape> {
        todo!()
    }
    fn size_built(
        &self,
        shape: Option<&wsp_wire::wire::machine_link::MachineShape>,
        asked: wsp_core::types::wiring::SizeAsk,
    ) -> wsp_core::types::wiring::SizeAsk {
        todo!()
    }
    async fn status_now(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn refuse_cannot(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        cap: &str,
        act: &str,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn keeps_images(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    fn pause_keeps_disk(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    fn name_given(&self, name: &str) -> String {
        todo!()
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        todo!()
    }
    fn put_look(&self, entry: &wsp_account::projects::workspaces::seam::Entry, key: &str, value: Option<serde_json::Value>) {
        todo!()
    }
    fn served_by_its_computer(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Option<String> {
        todo!()
    }
    fn computer_of(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> String {
        todo!()
    }
    fn has_deploy_daemon(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    async fn deploy_daemon_on(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn delete_queued(&self, id: &str) -> Option<Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    async fn refuse_changed(&self, path: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn path_exists(&self, path: &str) -> bool {
        todo!()
    }
    async fn cut_off(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn end_sessions(&self, id: &str, reason: &str) {
        todo!()
    }
    async fn drop_checkpoints(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        thread_id: &str,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn remove_worktree_ending(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn drop_thread_files(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        thread_ids: Vec<String>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn driven(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    async fn machine_kill(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_engine::errors::EngineError> {
        todo!()
    }
    async fn drop_workspace(&self, id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_gone(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn idle_touch(&self, id: &str) {
        todo!()
    }
    fn has_restart_daemon(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    async fn restart_daemon_of(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn shared_daemon(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> bool {
        todo!()
    }
    fn shared_daemon_replaced(&self) {
        todo!()
    }
    fn forget_polled_reach(&self, id: &str) {
        todo!()
    }
    async fn push_status(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn forget_failed_create(&self, id: &str) -> bool {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::projects::workspaces::git::GitCtx for Ctx {
    async fn worktree_mounted(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn machine_exec(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        cmd: &str,
        timeout_ms: Option<u64>,
    ) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    async fn exec_stream_of(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        argv: Vec<String>,
        cwd: Option<String>,
    ) -> Result<wsp_core::types::wiring::RunningExec, wsp_words::exit::HostError> {
        todo!()
    }
    async fn queued(
        &self,
        id: &str,
        work: wsp_account::projects::workspaces::seam::Queued<serde_json::Value>,
    ) -> Result<serde_json::Value, wsp_words::exit::HostError> {
        todo!()
    }
    async fn keep_tree(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        tree: wsp_wire::types::tree::TreeRecord,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn take_pull_request(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        pr: serde_json::Value,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn read_lead_of(&self, entry: &wsp_account::projects::workspaces::seam::Entry) {
        todo!()
    }
    async fn folder_for(
        &self,
        ask: wsp_core::types::api::FolderForAsk,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<(wsp_account::projects::workspaces::seam::Entry, Option<String>, Option<String>), wsp_words::exit::HostError> {
        todo!()
    }
    fn take_setup_line(&self, id: &str) -> Option<String> {
        todo!()
    }
    async fn resolve_project(
        &self,
        word: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn refuse_recording(&self, word: &str, origin: Option<&wsp_wire::views::workspace::Caller>) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn refuse_relayed(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
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
    async fn worktree_folder(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        top: &str,
        branch: &str,
        made_for: Option<String>,
    ) -> Result<wsp_account::projects::workspaces::seam::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    async fn worktrees_of(
        &self,
        top: &str,
    ) -> Result<Vec<wsp_account::projects::folders::tree::ListedWorktree>, wsp_words::exit::HostError> {
        todo!()
    }
    fn folders_of(&self, project_id: &str) -> Vec<wsp_account::projects::workspaces::seam::Entry> {
        todo!()
    }
    async fn remove_worktree(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        force: bool,
        ask: wsp_account::projects::folders::tree::RemoveAsk,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_checkout(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        fresh: bool,
    ) -> Result<Option<wsp_wire::types::changes::Checkout>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_pull_request(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        force: bool,
        whole: bool,
    ) -> Result<Option<serde_json::Value>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_tree(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn label_of(&self, entry: &wsp_account::projects::workspaces::seam::Entry, thread_id: Option<&str>) -> String {
        todo!()
    }
    async fn open_thread(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<serde_json::Value>, wsp_words::exit::HostError> {
        todo!()
    }
    fn default_agent_of(&self, entry: &wsp_account::projects::workspaces::seam::Entry) -> String {
        todo!()
    }
    fn agent_off(&self, entry: &wsp_account::projects::workspaces::seam::Entry, agent: &str) -> bool {
        todo!()
    }
    fn has_adapter(&self, agent: &str) -> bool {
        todo!()
    }
    async fn drafter(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        agent: &str,
    ) -> Result<Option<Option<String>>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn put_files(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        path: &str,
        text: &str,
        before: &[&str],
    ) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    async fn draft_for(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        agent: &str,
        prompt_file: &str,
        model: Option<&str>,
    ) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn viewed_marks(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, indexmap::IndexMap<String, String>>>> {
        todo!()
    }
    async fn persist_sessions(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn pull_request_on(
        &self,
        workspace_id: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_account::projects::workspaces::git::PullRequestOn, wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_page(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        remote: &str,
        number: i64,
        fresh: bool,
    ) -> Result<(wsp_wire::types::pull_request::PullRequestPage, bool), wsp_words::exit::HostError> {
        todo!()
    }
    async fn merge_settings(&self, remote: &str) -> Result<serde_json::Value, wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_host(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
        frame: wsp_account::account::pull_requests::FrameFor,
    ) -> Result<serde_json::Value, wsp_words::exit::HostError> {
        todo!()
    }
    async fn post_as_person(
        &self,
        remote: &str,
        number: i64,
        ask: serde_json::Value,
    ) -> Result<serde_json::Value, wsp_words::exit::HostError> {
        todo!()
    }
    async fn to_first_thread(
        &self,
        workspace_id: &str,
        prompt: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_account::projects::workspaces::git::SentTo, wsp_words::exit::HostError> {
        todo!()
    }
    async fn send_detached(
        &self,
        workspace_id: &str,
        prompt: &str,
        thread: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_account::projects::workspaces::git::SentTo, wsp_words::exit::HostError> {
        todo!()
    }
    async fn child_of(
        &self,
        lead: &wsp_account::projects::workspaces::seam::Entry,
        child: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
        lead_thread: Option<&str>,
        child_thread: Option<&str>,
    ) -> Result<wsp_account::projects::workspaces::seam::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    async fn update_copy(
        &self,
        entry: &wsp_account::projects::workspaces::seam::Entry,
    ) -> Result<wsp_wire::types::pull_request::GitUpdateReply, wsp_words::exit::HostError> {
        todo!()
    }
    fn drop_page_cache(&self, remote: &str, number: i64) {
        todo!()
    }
    fn is_local_workspace(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> bool {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}
