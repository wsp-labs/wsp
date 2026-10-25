// SPDX-License-Identifier: AGPL-3.0-only
//! kinds.ts:317-505: the three kind modules. Spec: spec/kinds_table.md.
use std::rc::Weak;
use std::sync::Arc;

use indexmap::IndexMap;

use wsp_adapters::port::ExecStreamFactory;
use wsp_core::types::internal::{EndThread, FolderAdmit, ImportLanded, ImportReport, KindModule, PortRoute, ServerValuesRoad};
use wsp_core::types::pending::{MachineExecOptions, ScopedRoad, TurnWaiting};
use wsp_core::types::wiring::{LiveWorkspace, Local, ProjectImportOptions, StageReport, WorkspaceRecord};
use wsp_engine::machine::MachineBackend;
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_words::exit::HostError;

use super::KindsArea;

/// kinds.ts:323. A fork at a provider or on a computer somebody joined.
pub struct CloudKind {
    pub(crate) area: Weak<KindsArea>,
}

/// kinds.ts:373. A folder on this computer.
pub struct LocalKind {
    pub(crate) area: Weak<KindsArea>,
}

/// kinds.ts:434. A project folder on a joined computer.
pub struct PlaceKind {
    pub(crate) area: Weak<KindsArea>,
}

impl KindModule for CloudKind {
    fn backend(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }
    fn exec_stream(&self, entry: &LiveWorkspace, opts: Option<MachineExecOptions>, waiting: Option<TurnWaiting>) -> Arc<dyn ExecStreamFactory> {
        todo!()
    }
    fn admit_folder(&self, record: FolderAdmit) -> Option<Local<'static, ()>> {
        todo!()
    }
    fn folder(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn home(&self, entry: &LiveWorkspace, agent_id: &str) -> String {
        todo!()
    }
    fn home_dir(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn env(&self, entry: &LiveWorkspace, agent_id: Option<&str>, home_of: &dyn Fn(&str) -> String) -> IndexMap<String, String> {
        todo!()
    }
    fn login_stands(&self, entry: &LiveWorkspace, agent_id: &str) -> bool {
        todo!()
    }
    fn import(&self, entry: &LiveWorkspace, o: ProjectImportOptions, report: ImportReport) -> Local<'static, ImportLanded> {
        todo!()
    }
    fn roots(&self, entry: &LiveWorkspace, dests: &[String]) -> Local<'static, ()> {
        todo!()
    }
    fn land_project(&self, entry: &LiveWorkspace, project: ProjectView, report: StageReport) -> Local<'static, ()> {
        todo!()
    }
    fn memory_key(&self, entry: &LiveWorkspace, agent_id: &str) -> Option<String> {
        todo!()
    }
    fn end_thread(&self, entry: &LiveWorkspace, thread_id: &str, o: EndThread) -> Option<Local<'static, Option<String>>> {
        todo!()
    }
    fn end_owed(&self, entry: &LiveWorkspace, thread_id: &str) -> Option<Local<'static, Option<Local<'static, ()>>>> {
        todo!()
    }
    fn port_reach(&self, entry: &LiveWorkspace, port: u16, standing: bool) -> Option<Local<'static, PortRoute>> {
        todo!()
    }
    fn relayed(&self, machine_id: Option<&str>) -> bool {
        todo!()
    }
    fn wsp_mcp(&self, entry: &LiveWorkspace) -> Option<McpServerSpec> {
        todo!()
    }
    fn turn_reach(&self, entry: &LiveWorkspace) -> Option<Option<String>> {
        todo!()
    }
    fn turn_road(&self) -> ScopedRoad {
        todo!()
    }
    fn keeps_agents(&self) -> bool {
        todo!()
    }
    fn server_values(&self) -> ServerValuesRoad {
        todo!()
    }
    fn has_daemon(&self, entry: &LiveWorkspace) -> bool {
        todo!()
    }
    fn shared_daemon(&self) -> bool {
        todo!()
    }
    fn daemon_road(&self, entry: &LiveWorkspace) -> Local<'static, DaemonReachView> {
        todo!()
    }
    fn scratch(&self, entry: &LiveWorkspace) -> String {
        todo!()
    }
    fn dropped(&self, entry: &LiveWorkspace) -> Local<'static, ()> {
        todo!()
    }
    fn daemon_version(&self, entry: &LiveWorkspace) -> Local<'static, Option<u32>> {
        todo!()
    }
    fn restart_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, ()>> {
        todo!()
    }
    fn deploy_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>> {
        todo!()
    }
}

impl KindModule for LocalKind {
    fn backend(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }
    fn exec_stream(&self, entry: &LiveWorkspace, opts: Option<MachineExecOptions>, waiting: Option<TurnWaiting>) -> Arc<dyn ExecStreamFactory> {
        todo!()
    }
    fn admit_folder(&self, record: FolderAdmit) -> Option<Local<'static, ()>> {
        todo!()
    }
    fn folder(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn home(&self, entry: &LiveWorkspace, agent_id: &str) -> String {
        todo!()
    }
    fn home_dir(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn env(&self, entry: &LiveWorkspace, agent_id: Option<&str>, home_of: &dyn Fn(&str) -> String) -> IndexMap<String, String> {
        todo!()
    }
    fn login_stands(&self, entry: &LiveWorkspace, agent_id: &str) -> bool {
        todo!()
    }
    fn import(&self, entry: &LiveWorkspace, o: ProjectImportOptions, report: ImportReport) -> Local<'static, ImportLanded> {
        todo!()
    }
    fn roots(&self, entry: &LiveWorkspace, dests: &[String]) -> Local<'static, ()> {
        todo!()
    }
    fn land_project(&self, entry: &LiveWorkspace, project: ProjectView, report: StageReport) -> Local<'static, ()> {
        todo!()
    }
    fn memory_key(&self, entry: &LiveWorkspace, agent_id: &str) -> Option<String> {
        todo!()
    }
    fn end_thread(&self, entry: &LiveWorkspace, thread_id: &str, o: EndThread) -> Option<Local<'static, Option<String>>> {
        todo!()
    }
    fn end_owed(&self, entry: &LiveWorkspace, thread_id: &str) -> Option<Local<'static, Option<Local<'static, ()>>>> {
        todo!()
    }
    fn port_reach(&self, entry: &LiveWorkspace, port: u16, standing: bool) -> Option<Local<'static, PortRoute>> {
        todo!()
    }
    fn relayed(&self, machine_id: Option<&str>) -> bool {
        todo!()
    }
    fn wsp_mcp(&self, entry: &LiveWorkspace) -> Option<McpServerSpec> {
        todo!()
    }
    fn turn_reach(&self, entry: &LiveWorkspace) -> Option<Option<String>> {
        todo!()
    }
    fn turn_road(&self) -> ScopedRoad {
        todo!()
    }
    fn keeps_agents(&self) -> bool {
        todo!()
    }
    fn server_values(&self) -> ServerValuesRoad {
        todo!()
    }
    fn has_daemon(&self, entry: &LiveWorkspace) -> bool {
        todo!()
    }
    fn shared_daemon(&self) -> bool {
        todo!()
    }
    fn daemon_road(&self, entry: &LiveWorkspace) -> Local<'static, DaemonReachView> {
        todo!()
    }
    fn scratch(&self, entry: &LiveWorkspace) -> String {
        todo!()
    }
    fn dropped(&self, entry: &LiveWorkspace) -> Local<'static, ()> {
        todo!()
    }
    fn daemon_version(&self, entry: &LiveWorkspace) -> Local<'static, Option<u32>> {
        todo!()
    }
    fn restart_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, ()>> {
        todo!()
    }
    fn deploy_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>> {
        todo!()
    }
}

impl KindModule for PlaceKind {
    fn backend(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }
    fn exec_stream(&self, entry: &LiveWorkspace, opts: Option<MachineExecOptions>, waiting: Option<TurnWaiting>) -> Arc<dyn ExecStreamFactory> {
        todo!()
    }
    fn admit_folder(&self, record: FolderAdmit) -> Option<Local<'static, ()>> {
        todo!()
    }
    fn folder(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn home(&self, entry: &LiveWorkspace, agent_id: &str) -> String {
        todo!()
    }
    fn home_dir(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn env(&self, entry: &LiveWorkspace, agent_id: Option<&str>, home_of: &dyn Fn(&str) -> String) -> IndexMap<String, String> {
        todo!()
    }
    fn login_stands(&self, entry: &LiveWorkspace, agent_id: &str) -> bool {
        todo!()
    }
    fn import(&self, entry: &LiveWorkspace, o: ProjectImportOptions, report: ImportReport) -> Local<'static, ImportLanded> {
        todo!()
    }
    fn roots(&self, entry: &LiveWorkspace, dests: &[String]) -> Local<'static, ()> {
        todo!()
    }
    fn land_project(&self, entry: &LiveWorkspace, project: ProjectView, report: StageReport) -> Local<'static, ()> {
        todo!()
    }
    fn memory_key(&self, entry: &LiveWorkspace, agent_id: &str) -> Option<String> {
        todo!()
    }
    fn end_thread(&self, entry: &LiveWorkspace, thread_id: &str, o: EndThread) -> Option<Local<'static, Option<String>>> {
        todo!()
    }
    fn end_owed(&self, entry: &LiveWorkspace, thread_id: &str) -> Option<Local<'static, Option<Local<'static, ()>>>> {
        todo!()
    }
    fn port_reach(&self, entry: &LiveWorkspace, port: u16, standing: bool) -> Option<Local<'static, PortRoute>> {
        todo!()
    }
    fn relayed(&self, machine_id: Option<&str>) -> bool {
        todo!()
    }
    fn wsp_mcp(&self, entry: &LiveWorkspace) -> Option<McpServerSpec> {
        todo!()
    }
    fn turn_reach(&self, entry: &LiveWorkspace) -> Option<Option<String>> {
        todo!()
    }
    fn turn_road(&self) -> ScopedRoad {
        todo!()
    }
    fn keeps_agents(&self) -> bool {
        todo!()
    }
    fn server_values(&self) -> ServerValuesRoad {
        todo!()
    }
    fn has_daemon(&self, entry: &LiveWorkspace) -> bool {
        todo!()
    }
    fn shared_daemon(&self) -> bool {
        todo!()
    }
    fn daemon_road(&self, entry: &LiveWorkspace) -> Local<'static, DaemonReachView> {
        todo!()
    }
    fn scratch(&self, entry: &LiveWorkspace) -> String {
        todo!()
    }
    fn dropped(&self, entry: &LiveWorkspace) -> Local<'static, ()> {
        todo!()
    }
    fn daemon_version(&self, entry: &LiveWorkspace) -> Local<'static, Option<u32>> {
        todo!()
    }
    fn restart_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, ()>> {
        todo!()
    }
    fn deploy_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>> {
        todo!()
    }
}
