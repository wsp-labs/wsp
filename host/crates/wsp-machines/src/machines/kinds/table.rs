// SPDX-License-Identifier: AGPL-3.0-only
//! kinds.ts:317-505: the three kind modules. Spec: spec/kinds_table.md.
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_engine::machine::MachineBackend;
use wsp_wire::views::workspace::DaemonReachView;
use wsp_words::exit::HostError;

use super::KindsArea;
use crate::machines::core_types::{KindModule, Live, StageReport, WorkspaceRecord};

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

#[async_trait(?Send)]
impl KindModule for CloudKind {
    fn backend(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }
    fn folder(&self, _record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn home(&self, _entry: &Live, _agent: &str) -> Result<String, HostError> {
        todo!()
    }
    fn home_dir(&self, _record: &WorkspaceRecord) -> Result<String, HostError> {
        todo!()
    }
    fn env(&self, _entry: &Live, _agent: Option<&str>, _home_of: &dyn Fn(&str) -> String) -> Result<IndexMap<String, String>, HostError> {
        todo!()
    }
    fn login_stands(&self, _entry: &Live, _agent: &str) -> bool {
        todo!()
    }
    fn relayed(&self) -> bool {
        todo!()
    }
    fn wsp_mcp(&self, _entry: &Live) -> Option<Value> {
        todo!()
    }
    fn turn_reach(&self) -> Option<IndexMap<String, String>> {
        todo!()
    }
    fn turn_road(&self) -> &'static str {
        todo!()
    }
    fn keeps_agents(&self) -> bool {
        todo!()
    }
    fn server_values(&self) -> &'static str {
        todo!()
    }
    fn has_daemon(&self, _entry: &Live) -> bool {
        todo!()
    }
    fn shared_daemon(&self) -> bool {
        todo!()
    }
    async fn daemon_road(&self, _entry: &Live) -> Result<DaemonReachView, HostError> {
        todo!()
    }
    fn scratch(&self, _entry: &Live) -> Result<String, HostError> {
        todo!()
    }
    async fn daemon_version(&self, _entry: &Live) -> Result<Option<f64>, HostError> {
        todo!()
    }
    async fn dropped(&self, _entry: &Live) -> Result<(), HostError> {
        todo!()
    }
    fn deploys_daemon(&self) -> bool {
        todo!()
    }
    async fn deploy_daemon(&self, _entry: &Live) -> Result<(), HostError> {
        todo!()
    }
    fn restarts_daemon(&self) -> bool {
        todo!()
    }
    async fn import(&self, _entry: &Live, _o: Value, _report: Rc<dyn Fn(&str, &str)>) -> Result<Value, HostError> {
        todo!()
    }
    async fn roots(&self, _entry: &Live, _dests: &[String]) -> Result<(), HostError> {
        todo!()
    }
    async fn land_project(&self, _entry: &Live, _project: &Value, _report: StageReport) -> Result<(), HostError> {
        todo!()
    }
    fn memory_key(&self, _entry: &Live, _agent: &str) -> Option<String> {
        todo!()
    }
    async fn admit_folder(&self, _record: &WorkspaceRecord) -> Result<(), HostError> {
        todo!()
    }
    async fn port_reach(&self, _entry: &Live, _port: u16, _standing: bool) -> Result<Option<DaemonReachView>, HostError> {
        todo!()
    }
}

#[async_trait(?Send)]
impl KindModule for LocalKind {
    fn backend(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }
    fn folder(&self, _record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn home(&self, _entry: &Live, _agent: &str) -> Result<String, HostError> {
        todo!()
    }
    fn home_dir(&self, _record: &WorkspaceRecord) -> Result<String, HostError> {
        todo!()
    }
    fn env(&self, _entry: &Live, _agent: Option<&str>, _home_of: &dyn Fn(&str) -> String) -> Result<IndexMap<String, String>, HostError> {
        todo!()
    }
    fn login_stands(&self, _entry: &Live, _agent: &str) -> bool {
        todo!()
    }
    fn relayed(&self) -> bool {
        todo!()
    }
    fn wsp_mcp(&self, _entry: &Live) -> Option<Value> {
        todo!()
    }
    fn turn_reach(&self) -> Option<IndexMap<String, String>> {
        todo!()
    }
    fn turn_road(&self) -> &'static str {
        todo!()
    }
    fn keeps_agents(&self) -> bool {
        todo!()
    }
    fn server_values(&self) -> &'static str {
        todo!()
    }
    fn has_daemon(&self, _entry: &Live) -> bool {
        todo!()
    }
    fn shared_daemon(&self) -> bool {
        todo!()
    }
    async fn daemon_road(&self, _entry: &Live) -> Result<DaemonReachView, HostError> {
        todo!()
    }
    fn scratch(&self, _entry: &Live) -> Result<String, HostError> {
        todo!()
    }
    async fn daemon_version(&self, _entry: &Live) -> Result<Option<f64>, HostError> {
        todo!()
    }
    async fn dropped(&self, _entry: &Live) -> Result<(), HostError> {
        todo!()
    }
    fn deploys_daemon(&self) -> bool {
        todo!()
    }
    async fn deploy_daemon(&self, _entry: &Live) -> Result<(), HostError> {
        todo!()
    }
    fn restarts_daemon(&self) -> bool {
        todo!()
    }
    async fn import(&self, _entry: &Live, _o: Value, _report: Rc<dyn Fn(&str, &str)>) -> Result<Value, HostError> {
        todo!()
    }
    async fn roots(&self, _entry: &Live, _dests: &[String]) -> Result<(), HostError> {
        todo!()
    }
    async fn land_project(&self, _entry: &Live, _project: &Value, _report: StageReport) -> Result<(), HostError> {
        todo!()
    }
    fn memory_key(&self, _entry: &Live, _agent: &str) -> Option<String> {
        todo!()
    }
    async fn admit_folder(&self, _record: &WorkspaceRecord) -> Result<(), HostError> {
        todo!()
    }
    async fn port_reach(&self, _entry: &Live, _port: u16, _standing: bool) -> Result<Option<DaemonReachView>, HostError> {
        todo!()
    }
}

#[async_trait(?Send)]
impl KindModule for PlaceKind {
    fn backend(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }
    fn folder(&self, _record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn home(&self, _entry: &Live, _agent: &str) -> Result<String, HostError> {
        todo!()
    }
    fn home_dir(&self, _record: &WorkspaceRecord) -> Result<String, HostError> {
        todo!()
    }
    fn env(&self, _entry: &Live, _agent: Option<&str>, _home_of: &dyn Fn(&str) -> String) -> Result<IndexMap<String, String>, HostError> {
        todo!()
    }
    fn login_stands(&self, _entry: &Live, _agent: &str) -> bool {
        todo!()
    }
    fn relayed(&self) -> bool {
        todo!()
    }
    fn wsp_mcp(&self, _entry: &Live) -> Option<Value> {
        todo!()
    }
    fn turn_reach(&self) -> Option<IndexMap<String, String>> {
        todo!()
    }
    fn turn_road(&self) -> &'static str {
        todo!()
    }
    fn keeps_agents(&self) -> bool {
        todo!()
    }
    fn server_values(&self) -> &'static str {
        todo!()
    }
    fn has_daemon(&self, _entry: &Live) -> bool {
        todo!()
    }
    fn shared_daemon(&self) -> bool {
        todo!()
    }
    async fn daemon_road(&self, _entry: &Live) -> Result<DaemonReachView, HostError> {
        todo!()
    }
    fn scratch(&self, _entry: &Live) -> Result<String, HostError> {
        todo!()
    }
    async fn daemon_version(&self, _entry: &Live) -> Result<Option<f64>, HostError> {
        todo!()
    }
    async fn dropped(&self, _entry: &Live) -> Result<(), HostError> {
        todo!()
    }
    fn deploys_daemon(&self) -> bool {
        todo!()
    }
    async fn deploy_daemon(&self, _entry: &Live) -> Result<(), HostError> {
        todo!()
    }
    fn restarts_daemon(&self) -> bool {
        todo!()
    }
    async fn import(&self, _entry: &Live, _o: Value, _report: Rc<dyn Fn(&str, &str)>) -> Result<Value, HostError> {
        todo!()
    }
    async fn roots(&self, _entry: &Live, _dests: &[String]) -> Result<(), HostError> {
        todo!()
    }
    async fn land_project(&self, _entry: &Live, _project: &Value, _report: StageReport) -> Result<(), HostError> {
        todo!()
    }
    fn memory_key(&self, _entry: &Live, _agent: &str) -> Option<String> {
        todo!()
    }
    async fn admit_folder(&self, _record: &WorkspaceRecord) -> Result<(), HostError> {
        todo!()
    }
    async fn port_reach(&self, _entry: &Live, _port: u16, _standing: bool) -> Result<Option<DaemonReachView>, HostError> {
        todo!()
    }
}
