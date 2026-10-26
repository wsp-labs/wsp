// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

#[async_trait::async_trait(?Send)]
impl crate::runtime::core::CoreCtx for Ctx {
    fn bus(&self) -> std::rc::Rc<dyn wsp_core::types::events::EventBus> {
        todo!()
    }
    fn clock(&self) -> std::rc::Rc<dyn wsp_core::clock::Clock> {
        todo!()
    }
    fn devices(&self) -> std::rc::Rc<dyn wsp_core::devices::DeviceDoor> {
        todo!()
    }
    fn host_key(&self) -> Option<String> {
        todo!()
    }
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn agents_reach(&self, workspace_id: &str) -> Option<crate::runtime::core::LaunchReach> {
        todo!()
    }
    fn brokered(&self, workspace_id: &str) -> bool {
        todo!()
    }
    fn record_name(&self, workspace_id: &str) -> String {
        todo!()
    }
    async fn reread_public(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_broker(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn daemon_settled(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_public_marketplaces(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn turn_road(&self, kind: wsp_wire::views::workspace_kind::WorkspaceKind) -> wsp_wire::views::workspace_more::WorkspaceOrigin {
        todo!()
    }
    fn live(&self) -> Vec<crate::runtime::core::StandingEntry> {
        todo!()
    }
    fn projects_held(&self) -> Vec<wsp_wire::views::workspace::ProjectView> {
        todo!()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        todo!()
    }
    fn session_views(&self) -> Vec<wsp_wire::views::session::SessionView> {
        todo!()
    }
    fn cap_held(&self, turn_id: &str) -> bool {
        todo!()
    }
    fn snoozed_until(&self, thread_id: &str) -> Option<f64> {
        todo!()
    }
    fn limit_resume_at(&self, thread_id: &str) -> Option<f64> {
        todo!()
    }
    fn thread_workspace(&self, thread_id: &str) -> Option<String> {
        todo!()
    }
    async fn resume_after_limit(&self, thread_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn wake(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn exec(
        &self,
        workspace_id: &str,
        cmd: &str,
        timeout_ms: u64,
    ) -> Result<wsp_wire::wire::machine_link::ExecResult, wsp_words::exit::HostError> {
        todo!()
    }
    async fn delete_workspace(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn unsaved_fork(&self, workspace_id: &str, name: &str) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn unsaved_project(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
    ) -> Result<Option<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn remove_project(&self, project_id: &str, force: bool, keep_look: bool) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn end_sessions(&self, workspace_id: &str, reason: &str) {
        todo!()
    }
    async fn drop_record(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn store_delete_project(&self, project_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn forget_icon(&self, project_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn project_look(
        &self,
        project_id: &str,
    ) -> Result<Option<wsp_wire::views::preferences::ProjectLook>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn set_project_look(
        &self,
        project_id: &str,
        look: Option<wsp_wire::views::preferences::ProjectLook>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn set_icon(&self, project_id: &str, image: Option<String>) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn keep_current(&self, place_id: &str) {
        todo!()
    }
    fn place_name(&self, place_id: &str) -> String {
        todo!()
    }
    async fn held_away_on(&self, place_id: &str) -> Result<Vec<String>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn hydrate(&self, workspace_id: &str) -> Result<bool, wsp_words::exit::HostError> {
        todo!()
    }
    fn sync_daemon(&self, workspace_id: &str) {
        todo!()
    }
    async fn reread_held(&self, workspace_id: &str, why: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn held_on(&self, place_id: &str) -> Vec<String> {
        todo!()
    }
    async fn poll(&self, workspace_ids: Vec<String>) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn place_away(&self, place_id: &str) -> bool {
        todo!()
    }
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl crate::runtime::public::PublicCtx for Ctx {
    async fn ready(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn set_say_stops(&self, say: std::rc::Rc<dyn Fn(&str)>) {
        todo!()
    }
    fn wall_now(&self) -> i64 {
        todo!()
    }
    fn clock_now(&self) -> i64 {
        todo!()
    }
    async fn refresh_builders(&self) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn expire_grace(
        &self,
    ) -> Result<(Vec<wsp_engine::orphans::ReapedMachine>, Vec<wsp_engine::orphans::ReapFailure>), wsp_words::exit::HostError> {
        todo!()
    }
    fn builders(&self) -> Vec<crate::runtime::public::SweptBuilder> {
        todo!()
    }
    async fn hold(&self, builder_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn stop_builder(&self, builder_id: &str, place: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn forget_builder(&self, builder_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn replay_creates(
        &self,
        known: &dyn Fn() -> Vec<String>,
        failed: &mut Vec<wsp_engine::orphans::ReapFailure>,
    ) -> Result<Vec<wsp_engine::orphans::ReapedMachine>, wsp_words::exit::HostError> {
        todo!()
    }
    fn sweep_places(&self) -> Vec<(String, bool)> {
        todo!()
    }
    fn wired_place(&self) -> String {
        todo!()
    }
    fn place_name(&self, place: &str) -> String {
        todo!()
    }
    async fn list_at(&self, place: &str) -> Result<Vec<wsp_engine::orphans::ListedMachine>, wsp_words::exit::HostError> {
        todo!()
    }
    fn entries(&self) -> Vec<crate::runtime::public::SweptEntry> {
        todo!()
    }
    async fn reread_held(&self, workspace_id: &str, why: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn recover_gone(&self, workspace_id: &str) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn read_machine(&self, workspace_id: &str) -> crate::runtime::public::MachineRead {
        todo!()
    }
    async fn adopt_gone(&self, workspace_id: &str, words: String) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn known_ids(&self) -> Vec<String> {
        todo!()
    }
    async fn adopt_lost(
        &self,
        place: &str,
        rows: &[wsp_engine::orphans::ListedMachine],
        claimed: &mut Vec<String>,
        failed: &mut Vec<wsp_engine::orphans::ReapFailure>,
    ) -> Result<(Vec<wsp_core::types::wiring::AdoptedMachine>, Vec<wsp_engine::orphans::ReapedMachine>), wsp_words::exit::HostError> {
        todo!()
    }
    async fn engine_reap(
        &self,
        place: &str,
        rows: Vec<wsp_engine::orphans::ListedMachine>,
        known: Vec<String>,
        older_than_ms: Option<i64>,
    ) -> Result<wsp_engine::orphans::ReapResult, wsp_words::exit::HostError> {
        todo!()
    }
    async fn sweep_strays(
        &self,
        known: Vec<String>,
        failed: &mut Vec<wsp_engine::orphans::ReapFailure>,
        places: &[String],
    ) -> Result<Vec<wsp_engine::orphans::ReapedMachine>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn close_step(&self, step: crate::runtime::public::CloseStep) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    fn kept_agents(&self) -> Vec<String> {
        todo!()
    }
    fn reap_kept(&self, thread_id: &str) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl crate::runtime::public::HarnessesCtx for Ctx {
    async fn preferences(&self) -> Result<wsp_wire::views::preferences::Preferences, wsp_words::exit::HostError> {
        todo!()
    }
    fn talks_to_tree_on(&self, workspace_id: &str, origin: Option<&wsp_wire::views::workspace::Caller>) -> bool {
        todo!()
    }
    async fn entry_of(
        &self,
        workspace_id: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<Option<(String, String)>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn project_home(
        &self,
        project: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<Option<(String, String)>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn computer_home(
        &self,
        computer: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> Result<Option<(String, String)>, wsp_words::exit::HostError> {
        todo!()
    }
    async fn agent_lists(
        &self,
        entry: Option<&(String, String)>,
    ) -> Result<Vec<wsp_wire::views::harness::HarnessCatalog>, wsp_words::exit::HostError> {
        todo!()
    }
    fn default_agent_of(&self, prefs: &wsp_wire::views::preferences::Preferences, entry: Option<&(String, String)>) -> String {
        todo!()
    }
    fn shaped(
        &self,
        catalog: &wsp_wire::views::harness::HarnessCatalog,
        prefs: &wsp_wire::views::preferences::Preferences,
        project: Option<&str>,
    ) -> serde_json::Map<String, serde_json::Value> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl crate::runtime::public::StatusCtx for Ctx {
    async fn probe_list(
        &self,
        o: Option<wsp_core::status::StatusListOptions>,
    ) -> Result<Vec<wsp_wire::views::workspace_more::WorkspaceStatus>, wsp_words::exit::HostError> {
        todo!()
    }
    fn listed_for(&self, origin: Option<&wsp_wire::views::workspace::Caller>) -> Result<Vec<String>, wsp_words::exit::HostError> {
        todo!()
    }
    fn refuse_named(
        &self,
        workspace_id: &str,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> Result<(), wsp_words::exit::HostError> {
        todo!()
    }
    async fn history(&self, workspace_id: &str) -> Result<Vec<serde_json::Value>, wsp_words::exit::HostError> {
        todo!()
    }
}
