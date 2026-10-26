// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use std::rc::Rc;

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::reach::ReachCtx for Ctx {
    fn live(&self, id: &str) -> Option<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn live_all(&self) -> Vec<wsp_machines::machines::machines::Entry> {
        todo!()
    }
    fn session_entries(&self) -> Vec<wsp_core::types::internal::SessionEntry> {
        todo!()
    }
    fn backend_for(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
    fn pause_keeps_disk(&self, backend: &dyn wsp_engine::machine::MachineBackend) -> bool {
        todo!()
    }
    async fn sweep_leftovers(
        &self,
        machine: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn closing(&self) -> bool {
        todo!()
    }
    fn provider_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn row_reason(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn unreached_of(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn kind_has_daemon(&self, entry: &wsp_machines::machines::machines::Entry) -> wsp_machines::machines::area_error::AreaResult<bool> {
        todo!()
    }
    fn view(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> serde_json::Value {
        todo!()
    }
    fn machine_state_of(&self, phase: &wsp_wire::views::workspace::WorkspacePhase) -> String {
        todo!()
    }
    fn idle_at(&self, workspace_id: &str) -> Option<f64> {
        todo!()
    }
    fn daemon_answers_ms(&self, entry: &wsp_machines::machines::machines::Entry) -> wsp_machines::machines::area_error::AreaResult<f64> {
        todo!()
    }
    fn daemon_hello_timeout_ms(&self) -> f64 {
        todo!()
    }
    async fn daemon_token_of(
        &self,
        machine: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> wsp_machines::machines::area_error::AreaResult<Option<String>> {
        todo!()
    }
    fn connect_daemon(
        &self,
        preview_url: &str,
        token: &str,
        on_event: std::rc::Rc<dyn Fn(wsp_wire::wire::daemon_events::DaemonEvent)>,
        heartbeat_ms: Option<i64>,
    ) -> wsp_machines::machines::area_error::AreaResult<std::rc::Rc<dyn wsp_core::reach::DaemonReach>> {
        todo!()
    }
    fn project_held(&self, id: &str) -> wsp_machines::machines::area_error::AreaResult<wsp_wire::views::workspace::ProjectView> {
        todo!()
    }
    async fn with_daemon(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        frame: serde_json::Value,
    ) -> wsp_machines::machines::area_error::AreaResult<serde_json::Value> {
        todo!()
    }
    async fn on_this_computer(
        &self,
        frame: Box<dyn Fn(&str) -> serde_json::Value>,
    ) -> wsp_machines::machines::area_error::AreaResult<serde_json::Value> {
        todo!()
    }
    fn checkout_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_machines::machines::area_error::AreaResult<String> {
        todo!()
    }
    fn served_by_its_computer(&self, entry: &wsp_machines::machines::machines::Entry) -> bool {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_machines::machines::area_error::AreaResult<()> {
        todo!()
    }
    fn emit(&self, event: serde_json::Value) {
        todo!()
    }
    fn record(&self, event: serde_json::Value) {
        todo!()
    }
    fn now(&self) -> f64 {
        todo!()
    }
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> wsp_core::clock::Cancel {
        todo!()
    }
    fn sleep(&self, ms: f64) -> futures::future::LocalBoxFuture<'static, ()> {
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
impl wsp_machines::machines::daemon::DaemonRuntime for Ctx {
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Clock> {
        todo!()
    }
    fn ids(&self) -> std::rc::Rc<dyn wsp_machines::machines::machines::Ids> {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn daemon_hello_timeout_ms(&self) -> u64 {
        todo!()
    }
    fn live(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::machines::machines::Entry>>> {
        todo!()
    }
    fn sessions(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::machines::machines::Row>>> {
        todo!()
    }
    fn daemon_notes(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, String>>> {
        todo!()
    }
    fn closed(&self) -> bool {
        todo!()
    }
    async fn sessions_start(
        &self,
        workspace_id: &str,
        ask: wsp_machines::machines::daemon::DetachedStart,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<wsp_machines::machines::daemon::Started, wsp_words::exit::HostError> {
        todo!()
    }
    async fn push_status(&self, entry: &wsp_machines::machines::machines::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn served_by_its_computer(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn module_of(&self, kind: &str) -> Result<std::rc::Rc<dyn wsp_core::types::internal::KindModule>, wsp_words::exit::HostError> {
        todo!()
    }
    fn project_path(&self, project_id: &str) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn checkout_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    fn backend_for(
        &self,
        record: &wsp_core::types::wiring::WorkspaceRecord,
    ) -> Result<std::sync::Arc<dyn wsp_engine::machine::MachineBackend>, wsp_words::exit::HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_machines::machines::machines::MachinesRuntime for Ctx {
    fn store(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Store> {
        todo!()
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Clock> {
        todo!()
    }
    fn ids(&self) -> std::rc::Rc<dyn wsp_core::ids::Ids> {
        todo!()
    }
    fn backend(&self) -> std::sync::Arc<dyn wsp_engine::machine::MachineBackend> {
        todo!()
    }
    fn places(&self) -> std::rc::Rc<dyn wsp_core::types::wiring::PlaceBackends> {
        todo!()
    }
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
    fn owner(&self) -> String {
        todo!()
    }
    fn kill_confirm(&self) -> Option<wsp_core::types::pending::KillConfirm> {
        todo!()
    }
    fn on_exec(&self) -> Option<std::rc::Rc<dyn Fn(&wsp_core::types::wiring::GoldenExec) -> Result<(), String>>> {
        todo!()
    }
    fn idle(&self) -> std::rc::Rc<dyn wsp_machines::machines::machines::IdleDoor> {
        Rc::new(self.idle.get())
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
    fn unreached(&self) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::machines::machines::Unreached>>> {
        todo!()
    }
    fn nap_refusals(
        &self,
    ) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::machines::reach::ForMachine<String>>>> {
        todo!()
    }
    fn stop_refusals(
        &self,
    ) -> std::rc::Rc<std::cell::RefCell<indexmap::IndexMap<String, wsp_machines::machines::reach::ForMachine<String>>>> {
        todo!()
    }
    fn project_held(&self, project_id: &str) -> bool {
        todo!()
    }
    fn kept_agents_on(&self, workspace_id: &str) -> Vec<String> {
        todo!()
    }
    fn end_execs(&self, workspace_id: &str, reason: &str) {
        todo!()
    }
    async fn entry_of(&self, id: &str) -> Result<wsp_machines::machines::machines::Entry, wsp_words::exit::HostError> {
        todo!()
    }
    fn view(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_wire::views::workspace_more::WorkspaceView {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn emit_status(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        reach: wsp_wire::views::workspace::ReachState,
        said: Option<&str>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn reach_of(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
    ) -> Result<wsp_wire::views::workspace::ReachState, wsp_words::exit::HostError> {
        todo!()
    }
    fn nap_refused_of(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn nap_refused_reason(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn stop_refused_reason(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    async fn says_waking(&self, entry: &wsp_machines::machines::machines::Entry, line: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn ping_daemon(&self, entry: &wsp_machines::machines::machines::Entry) -> Option<String> {
        todo!()
    }
    fn sync_daemon(&self, entry: &wsp_machines::machines::machines::Entry) {
        todo!()
    }
    async fn write_daemon_roots(&self, entry: &wsp_machines::machines::machines::Entry) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn sub_lost(
        &self,
        entry: &wsp_machines::machines::machines::Entry,
        reason: &str,
    ) -> Result<wsp_core::types::wiring::GoneOutcome, wsp_words::exit::HostError> {
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
    fn keeps_images(&self, backend: &std::sync::Arc<dyn wsp_engine::machine::MachineBackend>) -> bool {
        todo!()
    }
    fn names_workspace(&self, backend: &std::sync::Arc<dyn wsp_engine::machine::MachineBackend>) -> bool {
        todo!()
    }
    fn pause_keeps_disk(&self, backend: &std::sync::Arc<dyn wsp_engine::machine::MachineBackend>) -> bool {
        todo!()
    }
    async fn image_of(&self, golden: &str) -> Result<wsp_machines::machines::machines::ImageRead, wsp_words::exit::HostError> {
        todo!()
    }
    async fn recorded_images(&self) -> Result<Vec<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn place_id_of(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn at_provider(&self, place_id: Option<&str>) -> bool {
        todo!()
    }
    fn place_name_of(&self, place: &str) -> String {
        todo!()
    }
    fn place_nap_ms(&self, place_id: Option<&str>) -> Option<Option<f64>> {
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
    async fn read_memory(
        &self,
        record: &std::rc::Rc<std::cell::RefCell<wsp_core::types::wiring::WorkspaceRecord>>,
        machine: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
        sizes: &[wsp_wire::wire::capabilities::WorkspaceSize],
    ) -> bool {
        todo!()
    }
    async fn vault_export(
        &self,
        machine: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
        max_bytes: u64,
        leave: Option<Vec<String>>,
    ) -> Result<Vec<u8>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn gone_stop(
        &self,
        at: &std::sync::Arc<dyn wsp_engine::machine::MachineBackend>,
        machine: &std::sync::Arc<dyn wsp_engine::machine::Machine>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn forget_workspace_maps(&self, workspace_id: &str) {
        todo!()
    }
    fn reap_kept(&self, thread_id: &str) {
        todo!()
    }
    async fn forget_slate(&self, thread_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn forget_viewed(&self, workspace_id: &str) {
        todo!()
    }
    fn cancel_flush(&self, workspace_id: &str) {
        todo!()
    }
    async fn settle_flushes(&self, workspace_id: &str) {
        todo!()
    }
    fn clear_rows(&self, workspace_id: &str) {
        todo!()
    }
    async fn drop_sent_images(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn forget_kept_writes(&self, workspace_id: &str) {
        todo!()
    }
}

impl wsp_machines::machines::machines::IdleDoor for Ctx {
    fn touch(&self, workspace_id: &str) {
        todo!()
    }
    fn forget(&self, workspace_id: &str) {
        todo!()
    }
}
