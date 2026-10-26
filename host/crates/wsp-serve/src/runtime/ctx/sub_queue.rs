// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::sub_queue::SubQueueCtx for Ctx {
    fn live(&self, id: &str) -> Option<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn live_all(&self) -> Vec<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn session(&self, turn_id: &str) -> Option<wsp_core::types::internal::SessionEntry> {
        todo!()
    }
    fn set_session(&self, turn_id: &str, entry: wsp_core::types::internal::SessionEntry) {
        todo!()
    }
    fn update_session(&self, turn_id: &str, f: &mut dyn FnMut(&mut wsp_core::types::internal::SessionEntry)) {
        todo!()
    }
    fn remove_session(&self, turn_id: &str) {
        todo!()
    }
    fn place_id_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn holds_slot(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> bool {
        todo!()
    }
    fn place_name_of(&self, place_id: &str) -> Option<String> {
        todo!()
    }
    fn machines_at(&self, place_id: &str) -> Option<f64> {
        todo!()
    }
    async fn limits_of(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
    ) -> wsp_machines::machines::area_error::AreaResult<Option<wsp_engine::box_backend::BoxLimits>> {
        todo!()
    }
    fn view(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> serde_json::Value {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn persist_sessions(&self, workspace_id: &str) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn emit(&self, event: serde_json::Value) {
        todo!()
    }
    fn record(&self, event: serde_json::Value) {
        todo!()
    }
    fn attach_absent(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        said: &str,
    ) -> Result<wsp_machines::machines::machines::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn attach_dead(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
    ) -> Result<wsp_machines::machines::machines::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn record_cell(&self, workspace_id: &str) -> Option<std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>> {
        todo!()
    }
    async fn drop_workspace(&self, workspace_id: &str) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn notify_failed(
        &self,
        view: &wsp_wire::views::session::SessionView,
        turn_id: &str,
        notify: &[String],
        by: Option<&wsp_wire::views::workspace_more::ThreadScope>,
        road: Option<&serde_json::Value>,
        error: &str,
    ) {
        todo!()
    }
    fn push_head(&self, thread_id: &str) {
        todo!()
    }
    async fn rows_sent(&self) {
        todo!()
    }
    async fn put_start(&self, record: &wsp_core::types::internal::SubStartRecord) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn delete_start(&self, workspace_id: &str) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn list_starts(&self) -> wsp_machines::machines::area_error::AreaResult<Vec<wsp_core::types::internal::SubStartRecord>> {
        todo!()
    }
    async fn stand_up_queued(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        posted: std::rc::Rc<dyn Fn()>,
        waited: bool,
    ) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn start_turn(
        &self,
        workspace_id: &str,
        o: serde_json::Map<String, serde_json::Value>,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> wsp_machines::machines::area_error::AreaResult<wsp_machines::machines::sub_queue::StartedTurn> {
        todo!()
    }
    async fn cut_off(&self, entry: &wsp_machines::machines::machines::Entry) {
        todo!()
    }
    async fn unfork(&self, entry: &wsp_machines::machines::machines::Entry) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn now(&self) -> f64 {
        todo!()
    }
    fn uuid(&self) -> String {
        todo!()
    }
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> wsp_core::clock::Cancel {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn on_event(&self, kind: &str, f: std::rc::Rc<dyn Fn(&serde_json::Value)>) {
        self.hear_json(kind, f)
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::sub_life::SubLifeCtx for Ctx {
    fn live(&self, id: &str) -> Option<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn live_all(&self) -> Vec<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        todo!()
    }
    fn session_views(&self) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn latest_on(&self, thread_id: &str) -> Option<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        todo!()
    }
    fn launching_on(&self, thread_id: &str) -> bool {
        todo!()
    }
    fn push_head(&self, thread_id: &str) {
        todo!()
    }
    fn place_id_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn delete_after_ms(&self, place_id: Option<&str>) -> Option<f64> {
        todo!()
    }
    fn session_file(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        harness: &str,
        session_id: &str,
    ) -> Option<wsp_machines::machines::sub_life::SessionFile> {
        todo!()
    }
    async fn read_transcript(
        &self,
        workspace_id: &str,
        thread_id: &str,
        limit: usize,
        bytes: usize,
    ) -> wsp_machines::machines::area_error::AreaResult<Vec<serde_json::Value>> {
        todo!()
    }
    async fn put_blob(&self, collection: &str, key: &str, bytes: Vec<u8>) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn delete_blob(&self, collection: &str, key: &str) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn nap_with(&self, workspace_id: &str, reason: &str) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn end_sessions(&self, workspace_id: &str, reason: &str) {
        todo!()
    }
    fn attach_dead(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        machine_id: &str,
    ) -> Result<wsp_machines::machines::machines::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn forget_marks(&self, workspace_id: &str) {
        todo!()
    }
    fn idle_forget(&self, workspace_id: &str) {
        todo!()
    }
    async fn emit_status(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        reach: wsp_wire::views::workspace::ReachState,
    ) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn emit(&self, event: serde_json::Value) {
        todo!()
    }
    async fn unfork(&self, entry: &wsp_machines::machines::machines::Entry) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    async fn cut_off(&self, entry: &wsp_machines::machines::machines::Entry) {
        todo!()
    }
    fn broker_lift(&self, workspace_id: &str) {
        todo!()
    }
    async fn delete_workspace(&self, workspace_id: &str) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn now(&self) -> f64 {
        todo!()
    }
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> wsp_core::clock::Cancel {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn on_event(&self, kind: &str, f: std::rc::Rc<dyn Fn(&serde_json::Value)>) {
        self.hear_json(kind, f)
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::pull_requests::PullRequestsCtx for Ctx {
    fn now(&self) -> i64 {
        todo!()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> wsp_core::clock::Cancel {
        todo!()
    }
    fn cache_get(&self, key: &str) -> Option<wsp_account::account::pull_requests::CacheEntry> {
        todo!()
    }
    fn cache_set(&self, key: &str, entry: wsp_account::account::pull_requests::CacheEntry) {
        todo!()
    }
    fn cache_delete(&self, key: &str) {
        todo!()
    }
    fn live(&self, workspace_id: &str) -> Option<wsp_account::account::core_types::Entry> {
        todo!()
    }
    fn session_views(&self) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    async fn on_this_computer(
        &self,
        work: Box<
            dyn FnOnce(
                wsp_account::account::core_types::Ask,
                String,
            )
                -> futures::future::LocalBoxFuture<'static, wsp_account::account::core_types::AccountResult<serde_json::Value>>,
        >,
    ) -> wsp_account::account::core_types::AccountResult<serde_json::Value> {
        todo!()
    }
    async fn with_daemon(
        &self,
        entry: &wsp_account::account::core_types::Entry,
        work: wsp_account::account::core_types::Work<serde_json::Value>,
    ) -> wsp_account::account::core_types::AccountResult<serde_json::Value> {
        todo!()
    }
    fn checkout_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn entry_of(
        &self,
        workspace_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_account::account::core_types::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn project_held(&self, id: &str) -> Result<wsp_wire::views::workspace::ProjectView, wsp_words::exit::HostError> {
        todo!()
    }
    fn status_watched(&self) -> bool {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    async fn status_now(&self, entry: &wsp_account::account::core_types::Entry) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    async fn read_checkout(
        &self,
        entry: &wsp_account::account::core_types::Entry,
        force: bool,
    ) -> wsp_account::account::core_types::AccountResult<Option<wsp_wire::types::changes::Checkout>> {
        todo!()
    }
    fn slates_watch_pr(&self, workspace_id: &str) -> bool {
        todo!()
    }
    async fn mark_settled(&self, threads: Vec<String>, at: i64) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn pauses(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> bool {
        todo!()
    }
    async fn nap_with(&self, workspace_id: &str) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn read_lead_of(&self, child: &wsp_account::account::core_types::Entry) {
        todo!()
    }
    async fn queued(
        &self,
        workspace_id: &str,
        work: futures::future::LocalBoxFuture<'static, wsp_account::account::core_types::AccountResult<serde_json::Value>>,
    ) -> wsp_account::account::core_types::AccountResult<serde_json::Value> {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
}
