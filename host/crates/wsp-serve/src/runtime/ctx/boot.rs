// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::boot::BootRuntime for Ctx {
    fn store(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Store> {
        todo!()
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Clock> {
        todo!()
    }
    fn ids(&self) -> std::rc::Rc<dyn wsp_machines::machines::machines::Ids> {
        todo!()
    }
    fn places(&self) -> std::rc::Rc<dyn wsp_core::types::wiring::PlaceBackends> {
        todo!()
    }
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn pass(&self, event: wsp_wire::wire::events_union::EventUnion) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn owner(&self) -> String {
        todo!()
    }
    fn set_owner(&self, owner: &str) {
        todo!()
    }
    fn reads_since(&self) -> f64 {
        todo!()
    }
    fn set_reads_since(&self, at: f64) {
        todo!()
    }
    fn closed(&self) -> bool {
        todo!()
    }
    fn closing(&self) -> bool {
        todo!()
    }
    fn hostname(&self) -> String {
        todo!()
    }
    fn local_wired(&self) -> bool {
        todo!()
    }
    fn live(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::machines::machines::Entry>>> {
        todo!()
    }
    fn sessions(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::machines::machines::Row>>> {
        todo!()
    }
    fn thread_records(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_core::types::internal::ThreadRecord>>> {
        todo!()
    }
    fn builders(&self) -> wsp_machines::images::Builders {
        todo!()
    }
    fn projects_held(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_wire::views::workspace::ProjectView>>> {
        todo!()
    }
    fn boot_work(
        &self,
    ) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>>>>
    {
        todo!()
    }
    fn rereading(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexSet<String>>> {
        todo!()
    }
    fn viewed_marks(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, indexmap::IndexMap<String, String>>>> {
        todo!()
    }
    fn has_transcript_index(&self, workspace_id: &str) -> bool {
        todo!()
    }
    fn index_pos(&self, workspace_id: &str) -> u64 {
        todo!()
    }
    fn index_children(&self, workspace_id: &str, thread_id: &str) -> Vec<serde_json::Value> {
        todo!()
    }
    fn observed(&self, machine: std::sync::Arc<dyn wsp_engine::machine::Machine>) -> std::sync::Arc<dyn wsp_engine::machine::Machine> {
        todo!()
    }
    fn attach(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        machine: std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> Result<wsp_machines::machines::machines::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    async fn mark_gone(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        machine_id: &str,
        reason: &str,
    ) -> Result<wsp_core::types::wiring::GoneOutcome, wsp_words::exit::HostError> {
        todo!()
    }
    fn idle_touch(&self, workspace_id: &str) {
        todo!()
    }
    fn sync_daemon(&self, entry: &wsp_machines::machines::machines::Entry) {
        todo!()
    }
    fn backend_for(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    fn module_of(&self, kind: &str) -> Result<std::rc::Rc<dyn wsp_core::types::internal::KindModule>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn shape_of(
        &self,
        machine: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> Option<wsp_wire::wire::machine_link::MachineShape> {
        todo!()
    }
    fn size_built(
        &self,
        shape: Option<&wsp_wire::wire::machine_link::MachineShape>,
        asked: wsp_wire::wire::capabilities::WorkspaceSize,
    ) -> wsp_wire::wire::capabilities::WorkspaceSize {
        todo!()
    }
    fn arm_grace(&self, builder_id: &str, sealed_at: &str) {
        todo!()
    }
    fn door_backend_of(&self, place: &str) -> Option<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>> {
        todo!()
    }
    async fn image_kind(&self, golden: &str) -> Result<Option<wsp_wire::views::golden_image::MachineKind>, wsp_words::exit::HostError> {
        todo!()
    }
    fn attach_queued(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn migrate_copies(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn place_door_load(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn git_top_of(&self, path: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn remember_project(&self, project: wsp_wire::views::workspace::ProjectView) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn setups_load(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn move_transcript(&self, record: serde_json::Value) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn move_blobs(&self) {
        todo!()
    }
    async fn load_index(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn wake_at(&self, thread_id: &str, until: f64) {
        todo!()
    }
    fn resume_on_reset(&self, thread_id: &str, at: f64) {
        todo!()
    }
    fn settle_cut(
        &self,
        row: &wsp_machines::machines::machines::Row,
        reason: &str,
        line: &dyn Fn(f64) -> String,
        stopped: Option<bool>,
        unreached: Option<bool>,
    ) {
        todo!()
    }
    async fn persist_sessions(&self, workspace_id: &str) {
        todo!()
    }
    async fn turn_changes_written(&self, workspace_id: &str, thread_id: &str, turn_id: &str) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_turn_changes(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        ask: wsp_machines::machines::boot::CardAsk,
    ) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    async fn reattach(&self, row: &wsp_machines::machines::machines::Row, again: bool) -> wsp_core::types::internal::Reopened {
        todo!()
    }
    async fn devices(&self) -> Result<Vec<wsp_machines::machines::boot::ScopedDevice>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn revoke_device(&self, device_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn revoke_asides(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn shell_held_devices(&self) -> Result<indexmap::IndexSet<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        todo!()
    }
    async fn worktree_mounted(&self, entry: &wsp_machines::machines::machines::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn sweep_runs(&self, entry: &wsp_machines::machines::machines::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn resume_queue(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn notify_end(&self, end: wsp_machines::machines::boot::HeldEnd) {
        todo!()
    }
    fn record(&self, event: wsp_wire::wire::events_union::EventUnion) {
        todo!()
    }
    fn arm_subs(&self) {
        todo!()
    }
    async fn deliver_owed(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn move_old_copies(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn arm_sweep(&self) {
        todo!()
    }
    async fn slates_ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn drives_thread(&self, thread_id: Option<&str>, caller: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn acts_on(&self, record: &wsp_core::types::wiring::WorkspaceRecord, caller: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    fn talks_to_its_tree(&self, caller: Option<&wsp_wire::views::workspace::Caller>) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn refuse_relayed(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
        origin: Option<&wsp_wire::views::workspace::Caller>,
        act: Option<&str>,
        thread: Option<&str>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn resolve_workspace(
        &self,
        reference: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn place_rows(&self) -> Option<Vec<wsp_machines::machines::boot::ComputerRow>> {
        todo!()
    }
    async fn default_image_head(&self) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn place_for(&self, place: &str) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn restarts(&self) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn stopped_behind(&self, row: &wsp_machines::machines::machines::Row) -> Option<serde_json::Value> {
        todo!()
    }
    fn cap_wait(&self, turn_id: &str) -> Option<serde_json::Value> {
        todo!()
    }
    fn agent_pid_of(&self, run: &str) -> Option<u32> {
        todo!()
    }
    fn setup_refusal(&self, workspace_id: &str, harness: &str) -> Option<String> {
        todo!()
    }
    fn setup_place(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn latest_on(&self, thread_id: &str) -> Option<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn rows_on(&self, thread_id: &str) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn sub_row_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<wsp_wire::views::sub_row::SubRow> {
        todo!()
    }
    fn disk_kept_until(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<f64> {
        todo!()
    }
    fn place_id_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    async fn sub_read(&self, workspace_id: &str, since: f64) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn title_rows(&self, views: Vec<wsp_wire::views::session::SessionView>) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn refresh_title(&self, view: wsp_wire::views::session::SessionView) {
        todo!()
    }
    fn reap_kept(&self, thread_id: &str) {
        todo!()
    }
    fn cancel_resume(&self, thread_id: &str) {
        todo!()
    }
    fn lead_asks(&self, thread_id: &str) -> bool {
        todo!()
    }
    async fn ledger_limits(&self) -> Vec<wsp_machines::machines::boot::LimitRow> {
        todo!()
    }
    fn account_key_on(&self, entry: &wsp_machines::machines::machines::Entry, harness: &str) -> String {
        todo!()
    }
    async fn resume_turn(&self, workspace_id: &str, thread_id: &str, after_limit: f64) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
}

impl wsp_machines::machines::create::LandingRoad for Ctx {
    fn kind(&self) -> String {
        todo!()
    }
    fn refusal(&self, project: &wsp_wire::views::workspace::ProjectView, deps: &serde_json::Value) -> Option<String> {
        todo!()
    }
    fn memory_dir(&self, project: &wsp_wire::views::workspace::ProjectView, memory_key: &str, deps: &serde_json::Value) -> String {
        todo!()
    }
    fn workspace_binds(&self, project: &wsp_wire::views::workspace::ProjectView) -> Vec<wsp_wire::views::place::MachineBind> {
        todo!()
    }
}
