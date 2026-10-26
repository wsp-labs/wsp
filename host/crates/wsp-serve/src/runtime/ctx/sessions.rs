// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_threads::threads::sessions::SessionsCtx for Ctx {
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
    fn rows_on(&self, thread_id: &str) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        todo!()
    }
    fn running_on(&self, thread_id: &str) -> Option<wsp_threads::threads::threads::Row> {
        todo!()
    }
    fn launching_on(&self, thread_id: &str) -> Option<wsp_threads::threads::threads::Row> {
        todo!()
    }
    async fn entry_of_row(
        &self,
        at: &wsp_machines::machines::boot::entries::RowAt,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        talk: Option<wsp_core::types::internal::TreeTalk>,
        being_made: bool,
    ) -> Result<Option<wsp_threads::threads::threads::Entry>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn entry_of_with(
        &self,
        workspace_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        ask: wsp_machines::machines::boot::entries::EntryAsk,
    ) -> Result<wsp_threads::threads::threads::Entry, wsp_words::exit::HostError> {
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
    fn on_turn_settled(&self, turn_id: &str, f: std::rc::Rc<dyn Fn()>) -> std::rc::Rc<dyn Fn()> {
        todo!()
    }
    fn persist_sessions(&self, workspace_id: &str) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn push_head(&self, thread_id: &str) {
        todo!()
    }
    fn agent_label(&self, harness: &str) -> String {
        todo!()
    }
    fn waits(&self) -> wsp_threads::threads::waits::WaitsArea {
        todo!()
    }
    fn activity(&self) -> std::rc::Rc<dyn wsp_threads::activity::ActivityDoor> {
        todo!()
    }
    fn place_away(&self, place: &str) -> bool {
        todo!()
    }
    fn has_link(&self, place: &str) -> bool {
        todo!()
    }
    async fn ping(&self, place: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn on_place_absent(&self, place: &str, f: std::rc::Rc<dyn Fn()>) -> std::rc::Rc<dyn Fn()> {
        todo!()
    }
    fn place_name_of(&self, place: &str) -> String {
        todo!()
    }
    fn workspace_place(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn cap_held(&self, turn_id: &str) -> Option<wsp_threads::threads::threads::CapHeld> {
        todo!()
    }
    fn cap_stop(&self, turn_id: &str, line: &str) {
        todo!()
    }
    fn cap_stopping(&self, row_id: &str) -> std::rc::Rc<dyn Fn()> {
        todo!()
    }
    async fn drop_queued(&self, turn_id: &str) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    async fn stop_under(
        &self,
        thread_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<Vec<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn try_stopped(&self, turn_id: &str) {
        todo!()
    }
    fn line_try(&self, row: &wsp_threads::threads::threads::Row) -> bool {
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
    async fn end_owed(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_id: &str,
    ) -> Result<Option<Option<wsp_core::context::Joined>>, wsp_words::exit::HostError> {
        todo!()
    }
    fn taken(&self, workspace_id: &str, request_id: &str) -> Option<wsp_core::types::internal::Taken> {
        todo!()
    }
    async fn taken_turn(
        &self,
        workspace_id: &str,
        taken: wsp_core::types::internal::Taken,
    ) -> Result<Option<std::rc::Rc<dyn wsp_core::types::wiring::SessionHandle>>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn fork_opened(
        &self,
        workspace_id: &str,
        opened: wsp_core::types::api::StartOptions,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<(wsp_core::types::api::StartOptions, Option<wsp_threads::threads::forks::ForkPlan>), wsp_words::exit::HostError> {
        todo!()
    }
    fn claim_warm(&self, workspace_id: &str, harness: &str) -> Option<String> {
        todo!()
    }
    fn opens_in(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn of_threads_repository(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        project: &str,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn elsewhere_refusal(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        project: &str,
        name: &str,
    ) -> Result<Option<wsp_words::exit::HostError>, wsp_words::exit::HostError> {
        todo!()
    }
    fn away_for(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        act: &str,
        workspace_id: &str,
    ) -> Result<Option<wsp_words::exit::HostError>, wsp_words::exit::HostError> {
        todo!()
    }
    fn acts_on(&self, record: &wsp_core::types::wiring::WorkspaceRecord, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn project_computer_name(&self, project: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn worktree_gone(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        why: &str,
        starting: bool,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn computer_of(&self, entry: &wsp_threads::threads::threads::Entry) -> String {
        todo!()
    }
    fn refuse_sub_send(&self, entry: &wsp_threads::threads::threads::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn proven(&self, entry: &wsp_threads::threads::threads::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn copy_blocked(&self, entry: &wsp_threads::threads::threads::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn worktree_mounted(&self, entry: &wsp_threads::threads::threads::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn kept_check(&self, thread_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn spawn_guard(&self, act: &str, origin: Option<&wsp_wire::views::workspace::Caller>) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn parent_of(&self, thread_id: &str) -> Option<String> {
        todo!()
    }
    fn root_of(&self, thread_id: &str) -> String {
        todo!()
    }
    fn tree_of(&self, scope: Option<&wsp_wire::views::workspace_more::ThreadScope>) -> wsp_account::account::rules::TreeOf {
        todo!()
    }
    fn thread_of_token(&self, token: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn reaches_row(
        &self,
        view: &wsp_wire::views::session::SessionView,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        talk: Option<wsp_core::types::internal::TreeTalk>,
    ) -> bool {
        todo!()
    }
    fn notify_reach(&self, notify: &[String]) -> indexmap::IndexSet<String> {
        todo!()
    }
    fn notify_on(&self, thread_id: &str) -> Option<wsp_threads::threads::threads::NotifyOn> {
        todo!()
    }
    async fn resume_sub(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_id: &str,
        harness: &str,
        prompt: &str,
    ) -> Result<wsp_machines::machines::sub_spawn::machine::Resumed, wsp_words::exit::HostError> {
        todo!()
    }
    async fn place_rows(&self) -> Option<Vec<wsp_wire::views::place_more::PlaceView>> {
        todo!()
    }
    async fn hold_queued(
        &self,
        workspace_id: &str,
        thread_id: &str,
        tree: wsp_account::account::rules::TreeOf,
        opts: wsp_core::types::api::StartOptions,
        notify: Option<Vec<String>>,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<std::rc::Rc<dyn wsp_core::types::wiring::SessionHandle>, wsp_words::exit::HostError> {
        todo!()
    }
    fn launch_area(&self) -> wsp_threads::threads::launch::LaunchArea {
        todo!()
    }
    fn fork_turn_of(
        &self,
        plan: &wsp_threads::threads::forks::ForkPlan,
        by_count: bool,
    ) -> Result<Option<wsp_adapters::port::ForkTurn>, wsp_words::exit::HostError> {
        todo!()
    }
    fn sub_done(&self, workspace_id: &str) {
        todo!()
    }
    fn kept_warm(&self, thread_id: &str) -> bool {
        todo!()
    }
    async fn fork_start(
        &self,
        opts: wsp_core::types::api::ForkOptions,
        start: wsp_threads::threads::forks::StartFn,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<std::rc::Rc<dyn wsp_core::types::wiring::SessionHandle>, wsp_words::exit::HostError> {
        todo!()
    }
    fn carried_title(&self, thread_id: &str) -> (Option<String>, Option<wsp_wire::views::session::TitleSource>) {
        todo!()
    }
    fn cap_hold(&self, record: &wsp_core::types::wiring::WorkspaceRecord, turn_id: &str, lender: Option<&str>) {
        todo!()
    }
    fn cap_full(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
        turn_id: &str,
    ) -> Option<wsp_wire::views::session::ThreadCapWait> {
        todo!()
    }
    fn cap_wait(
        &self,
        ask: wsp_threads::threads::threads::CapAt,
        full: wsp_wire::views::session::ThreadCapWait,
    ) -> wsp_core::context::Joined {
        todo!()
    }
    fn cap_left(&self, turn_id: &str) {
        todo!()
    }
    fn cap_lend(&self, turn_id: &str, lender: &str) {
        todo!()
    }
    async fn first_run_here(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: &str,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    async fn limit_details_due(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: &str,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn host_writes(&self, session: &str) -> Option<wsp_core::context::Joined> {
        todo!()
    }
    async fn land_images(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        adapter: &dyn wsp_adapters::port::HarnessAdapter,
        landing: &str,
        dir: &str,
        attachments: Vec<wsp_wire::types::attachments::Attachment>,
    ) -> Result<wsp_threads::threads::agents::launch::LandedImages, wsp_words::exit::HostError> {
        todo!()
    }
    async fn land_files(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        landing: &str,
        dir: &str,
        attachments: Vec<wsp_wire::types::attachments::Attachment>,
    ) -> Result<Vec<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn ready_fork(
        &self,
        adapter: std::sync::Arc<dyn wsp_adapters::port::HarnessAdapter>,
        session: &str,
        turn: wsp_adapters::port::ForkTurn,
        harness: &str,
    ) -> Result<wsp_adapters::port::ForkedSession, wsp_words::exit::HostError> {
        todo!()
    }
    async fn copy_images(
        &self,
        plan: &wsp_threads::threads::forks::ForkPlan,
        workspace_id: &str,
        thread_id: &str,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn pay_cut_owed(
        &self,
        adapter: std::sync::Arc<dyn wsp_adapters::port::HarnessAdapter>,
        workspace_id: &str,
        thread_id: &str,
        resume: &str,
    ) -> String {
        todo!()
    }
    fn snapshot_of(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        cwd: &str,
    ) -> futures::future::Shared<futures::future::LocalBoxFuture<'static, Option<String>>> {
        todo!()
    }
    async fn keep_steer(
        &self,
        running: &wsp_threads::threads::threads::Row,
        opts: &wsp_core::types::api::StartOptions,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        steer_id: &str,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn steer_lost(&self, running: &wsp_threads::threads::threads::Row, steer_id: &str) {
        todo!()
    }
    fn steer_answered(
        &self,
        running: &wsp_threads::threads::threads::Row,
        handle_id: &str,
        opts: &wsp_core::types::api::StartOptions,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        steer_id: &str,
        accepted: bool,
    ) -> bool {
        todo!()
    }
    fn cut_before(&self, workspace_id: &str, thread_id: &str) -> bool {
        todo!()
    }
    fn take_kept(
        &self,
        thread_id: &str,
        ask: Option<wsp_core::types::internal::KeptLaunch>,
        resume: Option<&str>,
        launch_key: Option<&wsp_core::types::internal::KeptLaunch>,
    ) -> Option<std::rc::Rc<std::cell::RefCell<wsp_core::types::internal::KeptProcess>>> {
        todo!()
    }
    fn record_copies(&self, plan: &wsp_threads::threads::forks::ForkPlan, workspace_id: &str, thread_id: &str) {
        todo!()
    }
    fn run_turn(
        &self,
        t: wsp_threads::threads::turns::TurnStart,
    ) -> Result<std::rc::Rc<dyn wsp_core::types::wiring::SessionHandle>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn keep_sent_images(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: Option<&str>,
        attachments: Vec<wsp_wire::types::attachments::Attachment>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn cancel_resume_timer(&self, thread_id: &str) {
        todo!()
    }
    async fn remember_target(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn notify_end(
        &self,
        row: &wsp_threads::threads::threads::Row,
        notify: &[String],
        by: Option<&wsp_wire::views::workspace_more::ThreadScope>,
        road: Option<&wsp_wire::views::workspace_more::WorkspaceOrigin>,
        result: wsp_wire::views::session_events::TurnResult,
    ) {
        todo!()
    }
    fn drop_images(&self, entry: &wsp_threads::threads::threads::Entry, dir: &str) {
        todo!()
    }
    async fn drop_sent_images(&self, workspace_id: &str, thread_ids: &[String]) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn end_kept(&self, thread_id: &str, kept: &std::rc::Rc<std::cell::RefCell<wsp_core::types::internal::KeptProcess>>) {
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
    fn tree_stands_on(
        &self,
        workspace_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        talk: Option<wsp_core::types::internal::TreeTalk>,
    ) -> bool {
        todo!()
    }
    fn refuse_named(
        &self,
        workspace_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn title_rows(&self, rows: Vec<wsp_wire::views::session::SessionView>) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn booting(&self, workspace_id: &str) -> bool {
        todo!()
    }
    async fn refresh_title(&self, view: wsp_wire::views::session::SessionView, ended: bool) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn tree_records(
        &self,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        talk: wsp_core::types::internal::TreeTalk,
        workspace_id: Option<&str>,
    ) -> Vec<(String, wsp_core::types::internal::ThreadRecord)> {
        todo!()
    }
    fn listed_rows(&self, rows: Vec<wsp_wire::views::session::SessionView>) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn placed_rows(&self, rows: Vec<wsp_wire::views::session::SessionView>) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn drives_thread(&self, thread_id: Option<&str>, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    async fn open_transcript(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, wsp_words::exit::HostError> {
        todo!()
    }
    fn opened_words(&self, workspace_id: &str, thread_id: &str) {
        todo!()
    }
    fn index_pos(&self, workspace_id: &str) -> Option<u64> {
        todo!()
    }
    async fn read_page(
        &self,
        workspace_id: &str,
        thread_id: &str,
        window: wsp_store::transcript_reader::ThreadRead,
    ) -> Result<wsp_wire::views::preferences::HistoryPage, wsp_words::exit::HostError> {
        todo!()
    }
    fn thread_facts(&self, thread_id: &str) -> Option<wsp_wire::views::preferences::ThreadFacts> {
        todo!()
    }
    async fn facts_after_read(
        &self,
        thread_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<Option<wsp_wire::views::preferences::ThreadFacts>, wsp_words::exit::HostError> {
        todo!()
    }
    fn kept_writes(&self, workspace_id: &str) -> Option<wsp_core::context::Joined> {
        todo!()
    }
    async fn store_get(&self, collection: &str, id: &str) -> Result<Option<serde_json::Value>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn store_get_blob(&self, collection: &str, key: &str) -> Result<Option<Vec<u8>>, wsp_words::exit::HostError> {
        todo!()
    }
    fn index_workspaces(&self) -> Vec<String> {
        todo!()
    }
    fn search_in(
        &self,
        workspace_id: &str,
        words: &str,
        drives: &dyn Fn(&str) -> bool,
        found: &mut Vec<wsp_core::types::internal::SearchedHit>,
    ) -> u32 {
        todo!()
    }
    fn record_steer(
        &self,
        row: &wsp_threads::threads::threads::Row,
        row_id: &str,
        ask: &wsp_core::types::api::SteerAsk,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        steer_id: &str,
    ) {
        todo!()
    }
    async fn launch_adapter_for(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: &str,
        server_values: Option<wsp_adapters::port::ServerValues>,
    ) -> Result<wsp_threads::threads::agents::launch::BuiltAdapter, wsp_words::exit::HostError> {
        todo!()
    }
    fn harness_exec(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        timeout_ms: u64,
    ) -> std::sync::Arc<dyn wsp_adapters::port::HarnessExec> {
        todo!()
    }
    async fn write_session(
        &self,
        session: &str,
        entry: &wsp_threads::threads::threads::Entry,
        adapter: std::sync::Arc<dyn wsp_adapters::port::HarnessAdapter>,
        title: &str,
    ) -> Result<wsp_adapters::port::SessionRenameWrite, wsp_words::exit::HostError> {
        todo!()
    }
    fn brokered(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> bool {
        todo!()
    }
    async fn server_values_for(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        harness: &str,
        cwd: &str,
    ) -> Result<Option<wsp_adapters::port::ServerValues>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn mark(
        &self,
        thread_ids: &[String],
        stamps: wsp_threads::threads::sessions::StampsOf,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn arm_resume(
        &self,
        thread_ids: &[String],
        on: bool,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn aside(
        &self,
        adapter: &dyn wsp_adapters::port::HarnessAdapter,
        ask: wsp_adapters::port::AsideQuestion,
    ) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn subagent_known(&self, id: &str) -> bool {
        todo!()
    }
    fn settles_thread(&self, thread_id: &str, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn settled_now(&self, thread_id: &str, settle_ms: Option<f64>) -> bool {
        todo!()
    }
    fn tree_under(&self, thread_id: &str) -> Vec<String> {
        todo!()
    }
    fn restarts(&self) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn reap_kept(&self, thread_id: &str) {
        todo!()
    }
    fn checkout_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn wake(
        &self,
        workspace_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn git_restore(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        cwd: &str,
        checkpoint: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn flush_transcript(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn move_session(&self, thread_id: &str, from: &str, to: &str) {
        todo!()
    }
    async fn drop_from_transcript(
        &self,
        workspace_id: &str,
        thread_id: &str,
        turns: Option<&[String]>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn slates_rewound(&self, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn lead_of_sub(&self, workspace_id: Option<&str>, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    async fn sub_delete(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        ask: wsp_core::types::api::DeleteThreadAsk,
    ) -> Result<wsp_core::types::api::ThreadDeleted, wsp_words::exit::HostError> {
        todo!()
    }
    fn turn_runs(&self, workspace_id: &str) -> bool {
        todo!()
    }
    async fn end_thread_shells(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_id: &str,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn delete_workspace(
        &self,
        workspace_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn drop_checkpoints(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_id: &str,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn drop_thread_files(
        &self,
        entry: &wsp_threads::threads::threads::Entry,
        thread_ids: &[String],
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn row_gone(&self, row_id: &str, workspace_id: &str, thread_id: &str) {
        todo!()
    }
    async fn slates_forget(&self, thread_id: &str, keep: bool) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
}
