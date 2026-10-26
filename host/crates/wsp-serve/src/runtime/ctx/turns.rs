// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_threads::threads::turns::TurnsCtx for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
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
    fn closing(&self) -> bool {
        todo!()
    }
    fn record(&self, event: wsp_wire::views::session_events_more::SessionEvent) {
        todo!()
    }
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) {
        todo!()
    }
    fn pass(&self, event: wsp_wire::wire::events_union::EventUnion) {
        todo!()
    }
    fn persist_sessions(&self, workspace_id: &str) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    async fn persist_record(&self, record: wsp_core::types::wiring::WorkspaceRecord) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn held_at_close(&self) -> wsp_threads::threads::turns::HeldAtClose {
        todo!()
    }
    async fn slates_turn_ended(&self, thread_id: &str, turn_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn keep_checkpoint(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        ask: wsp_threads::threads::threads::CheckpointAsk,
    ) -> wsp_core::context::Joined {
        todo!()
    }
    fn checkpoints_landing(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_core::context::Joined>>> {
        todo!()
    }
    async fn read_turn_changes(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        ask: wsp_threads::threads::threads::TurnChangesAsk,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn lead_asks(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_adapters::port::PermissionAsk>>> {
        todo!()
    }
    fn mode_label(&self, harness: &str, mode: &str) -> Option<String> {
        todo!()
    }
    fn settle_cut(&self, row: &wsp_threads::threads::threads::Row, reason: &str, by_stop: bool, unreached: bool) {
        todo!()
    }
    fn line_try(&self, row: &wsp_threads::threads::threads::Row) -> bool {
        todo!()
    }
    fn prompt_held(&self, request_id: &str) {
        todo!()
    }
    async fn name_in_harness(&self, view: wsp_wire::views::session::SessionView, title: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn source_of(&self, view: &wsp_wire::views::session::SessionView) -> Option<String> {
        todo!()
    }
    fn hold_ask(&self, ask: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    async fn refresh_title(&self, row: &wsp_threads::threads::threads::Row, ended: bool) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn make_title(&self, row: &wsp_threads::threads::threads::Row) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn usage_account_of(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: &str,
        account: Option<(String, Option<String>)>,
    ) -> wsp_threads::threads::turns::UsageAccount {
        todo!()
    }
    fn usage_computer_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> String {
        todo!()
    }
    async fn ledger_add(&self, row: wsp_threads::threads::turns::LedgerUse) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn ledger_limit(
        &self,
        ask: wsp_threads::threads::turns::LimitAsk,
    ) -> Result<(serde_json::Value, serde_json::Value), wsp_words::exit::HostError> {
        todo!()
    }
    async fn alerts_read(&self, before: serde_json::Value, after: serde_json::Value) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn usage_accounts(&self) -> Result<wsp_wire::types::usage::AccountsAnswer, wsp_words::exit::HostError> {
        todo!()
    }
    fn burn_add(&self, ask: wsp_threads::threads::turns::BurnAsk) {
        todo!()
    }
    fn notify_end(
        &self,
        row: &wsp_threads::threads::threads::Row,
        notify: &[String],
        result: wsp_wire::views::session_events::TurnResult,
        open: bool,
    ) {
        todo!()
    }
    fn agent_retried(&self, turn_id: &str, why: Option<&str>) {
        todo!()
    }
    fn end_snooze_for(&self, view: &wsp_wire::views::session::SessionView) {
        todo!()
    }
    fn idle_hold(&self, workspace_id: &str) {
        todo!()
    }
    fn idle_release(&self, workspace_id: &str) {
        todo!()
    }
    fn arm_disk_watch(&self) {
        todo!()
    }
    async fn read_disk(&self, entry: &wsp_threads::threads::threads::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn harness_exec(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        timeout_ms: u64,
    ) -> std::sync::Arc<dyn wsp_adapters::port::HarnessExec> {
        todo!()
    }
    fn shared_daemon(&self, entry: &wsp_threads::threads::threads::Entry) -> bool {
        todo!()
    }
    fn turn_groups(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, indexmap::IndexMap<i64, String>>>> {
        todo!()
    }
    fn arm_roots_recheck(&self) {
        todo!()
    }
    fn port_roots_moved(&self, workspace_id: &str) {
        todo!()
    }
    fn hold_kept(&self, thread_id: &str, hold: wsp_threads::threads::threads::KeptHand) -> Option<wsp_threads::threads::threads::KeptHand> {
        todo!()
    }
    async fn revoke_device(&self, device_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_checkout(&self, entry: &wsp_threads::threads::threads::Entry, fresh: bool) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn ran_on_branch(&self, entry: &wsp_threads::threads::threads::Entry, thread_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_pull_request(&self, entry: &wsp_threads::threads::threads::Entry, fresh: bool) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_lead_of(&self, entry: &wsp_threads::threads::threads::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn pull_request_state(&self, entry: &wsp_threads::threads::threads::Entry) -> Option<String> {
        todo!()
    }
    async fn settle_tree(&self, entry: &wsp_threads::threads::threads::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn drop_images(&self, entry: &wsp_threads::threads::threads::Entry, dir: &str) {
        todo!()
    }
    fn send_back(&self, row: &wsp_threads::threads::threads::Row, unread: Vec<String>, stopped: bool) {
        todo!()
    }
    async fn take_review(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_id: &str,
        text: &str,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn is_held_away(&self, workspace_id: &str) -> bool {
        todo!()
    }
    fn exec_sweep(&self, entry: &wsp_threads::threads::threads::Entry) -> Option<wsp_threads::threads::turns::Sweep> {
        todo!()
    }
    fn adapter_for(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        ask: wsp_threads::threads::agents::launch::AdapterAsk,
    ) -> Result<wsp_threads::threads::agents::launch::BuiltAdapter, wsp_words::exit::HostError> {
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
    fn spawn(&self, work: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}

impl wsp_threads::threads::forks::ForksCtx for Ctx {
    fn ready(&self) -> futures::future::LocalBoxFuture<'static, Result<(), wsp_words::exit::HostError>> {
        todo!()
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
    fn latest_on(&self, thread_id: &str) -> Option<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn rows_on(&self, thread_id: &str) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn running_turn(&self, thread_id: &str) -> Option<String> {
        todo!()
    }
    fn launching_turn(&self, thread_id: &str) -> Option<String> {
        todo!()
    }
    fn entry_of_row(
        &self,
        at: &wsp_machines::machines::boot::entries::RowAt,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_threads::threads::threads::Entry, wsp_words::exit::HostError>> {
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
    fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        todo!()
    }
    fn folder_of(&self, workspace_id: &str, session: &str) -> Option<String> {
        todo!()
    }
    fn agent_label(&self, agent: &str) -> String {
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
    fn persist_sessions(&self, workspace_id: &str) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn project_held(&self, project: &str) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn place_name(&self, computer: &str) -> String {
        todo!()
    }
    fn git_here(
        &self,
        top: &str,
        args: Vec<String>,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError>> {
        todo!()
    }
    fn live_ids(&self) -> Vec<String> {
        todo!()
    }
    fn worktree_folder(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        top: &str,
        branch: &str,
        from: &str,
        root: Option<String>,
        at_ref: &str,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_threads::threads::threads::Entry, wsp_words::exit::HostError>> {
        todo!()
    }
    fn checkout_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn take_setup_line(&self, workspace_id: &str) -> Option<String> {
        todo!()
    }
    fn remove_worktree(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        force: bool,
    ) -> futures::future::LocalBoxFuture<'static, Result<(), wsp_words::exit::HostError>> {
        todo!()
    }
    fn spawn(&self, work: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
}

impl wsp_threads::threads::conversations::ConversationsCtx for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        todo!()
    }
    fn bus(&self) -> std::rc::Rc<dyn wsp_core::types::events::EventBus> {
        self.bus.clone()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        todo!()
    }
    fn transcript_state(&self) -> wsp_core::context::StateCell<wsp_threads::threads::transcripts::TranscriptState> {
        todo!()
    }
    fn live_project(&self, workspace_id: &str) -> Option<String> {
        todo!()
    }
    fn with_daemon(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        frame: serde_json::Value,
    ) -> futures::future::LocalBoxFuture<'static, Result<serde_json::Value, wsp_account::account::core_types::AccountError>> {
        todo!()
    }
    fn harness_exec(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        timeout_ms: u64,
    ) -> std::sync::Arc<dyn wsp_adapters::port::HarnessExec> {
        todo!()
    }
    fn project_held(&self, project: &str) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn worktrees_of(
        &self,
        top: &str,
    ) -> futures::future::LocalBoxFuture<
        'static,
        Result<Vec<wsp_account::projects::folders::tree::ListedWorktree>, wsp_words::exit::HostError>,
    > {
        todo!()
    }
    fn folders_of(&self, project: &str) -> Vec<wsp_core::types::wiring::WorkspaceRecord> {
        todo!()
    }
    fn has_adapter(&self, harness: &str) -> bool {
        todo!()
    }
    fn agent_off(&self, entry: &wsp_threads::threads::threads::Entry, agent: &str) -> bool {
        todo!()
    }
    fn conversation_store(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: &str,
    ) -> Option<std::sync::Arc<dyn wsp_adapters::port::ConversationStore>> {
        todo!()
    }
    fn agent_label(&self, agent: &str) -> String {
        todo!()
    }
    fn computer_of(&self, entry: &wsp_threads::threads::threads::Entry) -> String {
        todo!()
    }
    fn latest_on(&self, thread_id: &str) -> Option<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn reaches_row(&self, row: &wsp_wire::views::session::SessionView, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn folder_for(
        &self,
        project: Option<String>,
        cwd: Option<String>,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_account::projects::folders::place::FolderPlaced, wsp_words::exit::HostError>>
    {
        todo!()
    }
    fn entry_of(
        &self,
        workspace_id: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_threads::threads::threads::Entry, wsp_words::exit::HostError>> {
        todo!()
    }
    fn preferences(
        &self,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_wire::views::preferences::Preferences, wsp_words::exit::HostError>> {
        todo!()
    }
    fn default_agent_of(&self, prefs: &wsp_wire::views::preferences::Preferences, entry: &wsp_threads::threads::threads::Entry) -> String {
        todo!()
    }
}
