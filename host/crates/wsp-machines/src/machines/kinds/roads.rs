// SPDX-License-Identifier: AGPL-3.0-only
//! kinds.ts:26-316, 506-563: the roads every kind shares. Spec: spec/kinds_roads.md.
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use wsp_engine::machine::{ExecResult, MachineBackend};
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::exit::HostError;

use super::{BrokeredLaunch, FolderComputer, KindsArea};
use crate::machines::core_types::{AreaResult, Live, StageReport, WorkspaceRecord};

/// kinds.ts:27. How long a launch waits on the broker's answer.
pub const BROKER_READ_MS: f64 = 5_000.0;

/// The import's report and answer (types/internal.ts `ImportLanded`) as the TS objects.
pub type ImportLanded = Value;

impl KindsArea {
    /// kinds.ts:32 (rule 1).
    pub fn project_of(&self, dest: &str, size: f64) -> Value {
        todo!()
    }
    /// kinds.ts:36 (rule 2).
    pub(crate) fn clone_on_machine(&self, project: &ProjectView, computer: &str) -> String {
        todo!()
    }
    /// kinds.ts:46 (rule 3).
    pub(crate) async fn clone_project(&self, entry: &Live, project: &ProjectView, report: StageReport) -> AreaResult<()> {
        todo!()
    }
    /// kinds.ts:80 (rule 4). `(remote, defaultBranch)`.
    pub async fn remote_here(&self, path: &str) -> AreaResult<(String, String)> {
        todo!()
    }
    /// kinds.ts:93 (rule 5).
    pub async fn git_here(&self, cwd: &str, args: &[&str], timeout_ms: Option<u64>, stdin: Option<&str>) -> AreaResult<ExecResult> {
        todo!()
    }
    /// kinds.ts:99 (rule 5).
    pub async fn git_top_of(&self, path: &str) -> AreaResult<Option<String>> {
        todo!()
    }
    /// kinds.ts:105 (rule 5).
    pub async fn branch_at(&self, path: &str) -> AreaResult<Option<String>> {
        todo!()
    }
    /// kinds.ts:112 (rule 6).
    pub fn state_folder(&self) -> Result<String, HostError> {
        todo!()
    }
    /// kinds.ts:124 (rule 7). `source` as the TS `ProjectSource` object; answers the folder as it resolved.
    pub async fn clone_here(&self, word: &str, source: &Value, into: &str) -> Result<String, HostError> {
        todo!()
    }
    /// kinds.ts:171 (rule 8).
    pub(crate) async fn register_import(&self, o: &Value, report: &dyn Fn(&str, &str)) -> AreaResult<ImportLanded> {
        todo!()
    }
    /// kinds.ts:181 (rule 9).
    pub(crate) async fn write_roots(&self, entry: &Live, dests: &[String], roots_path: Option<&str>) -> AreaResult<()> {
        todo!()
    }
    /// kinds.ts:185 (rule 10).
    pub(crate) fn cloud_home(&self, id: &str) -> Result<String, HostError> {
        todo!()
    }
    /// kinds.ts:192 (rule 10).
    pub(crate) fn cloud_env(&self, record: &WorkspaceRecord, id: Option<&str>) -> IndexMap<String, String> {
        todo!()
    }
    /// kinds.ts:197 (rule 11).
    pub(crate) async fn cloud_road(&self, entry: &Live) -> AreaResult<DaemonReachView> {
        todo!()
    }
    /// kinds.ts:202 (rule 11).
    pub async fn local_road(&self) -> AreaResult<DaemonReachView> {
        todo!()
    }
    /// kinds.ts:209 (rule 12).
    pub(crate) async fn cloud_deploy(&self, entry: &Live) -> AreaResult<()> {
        todo!()
    }
    /// kinds.ts:217 (rule 13).
    pub(crate) fn shared_login_under(&self, home: &str, agent: &str) -> bool {
        todo!()
    }
    /// kinds.ts:227 (rule 13).
    pub(crate) fn place_of(&self, record: &WorkspaceRecord) -> Result<FolderComputer, HostError> {
        todo!()
    }
    /// kinds.ts:235 (rule 14).
    pub fn place_agent_home(&self, place: Option<&str>, home: &str, id: &str) -> Result<String, HostError> {
        todo!()
    }
    /// kinds.ts:242 (rule 15).
    pub fn vault_on(&self, entry: &Live) -> IndexMap<String, String> {
        todo!()
    }
    /// kinds.ts:248 (rule 16).
    pub fn brokered(&self, record: &WorkspaceRecord) -> bool {
        todo!()
    }
    /// kinds.ts:256 (rule 17).
    pub async fn read_broker(&self) {
        todo!()
    }
    /// kinds.ts:278 (rule 18).
    pub fn brokered_launch(&self, entry: &Live, harness: &str, token: Option<&str>) -> Result<Option<BrokeredLaunch>, HostError> {
        todo!()
    }
    /// kinds.ts:291 (rule 19).
    pub async fn know_broker(&self, entry: &Live, harness: &str) -> Result<(), HostError> {
        todo!()
    }
    /// kinds.ts:297.
    pub(crate) fn place_login_stands(&self, entry: &Live, id: &str) -> bool {
        todo!()
    }
    /// kinds.ts:304 (rule 20).
    pub(crate) fn place_env(&self, entry: &Live, home_of: &dyn Fn(&str) -> String) -> Result<IndexMap<String, String>, HostError> {
        todo!()
    }
    /// kinds.ts:508 (rule 21).
    pub fn kind_of(&self, computer: &str) -> WorkspaceKind {
        todo!()
    }
    /// kinds.ts:514 (rule 22).
    pub fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }
    /// kinds.ts:519 (rule 22).
    pub fn backend_of_kind(&self, kind: WorkspaceKind, place: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError> {
        todo!()
    }
    /// kinds.ts:525-531 (rule 22).
    pub fn keeps_images(&self, at: &dyn MachineBackend) -> bool {
        todo!()
    }
    pub fn pause_keeps_disk(&self, at: &dyn MachineBackend) -> bool {
        todo!()
    }
    pub fn names_workspace(&self, at: &dyn MachineBackend) -> bool {
        todo!()
    }
    /// kinds.ts:534 (rule 22): the lifecycle's budgets' daemon answer time is what readers take.
    pub fn lifecycle_of(&self, entry: &Live) -> Result<Arc<dyn wsp_engine::machine::Lifecycle>, HostError> {
        todo!()
    }
    /// kinds.ts:539 (rule 23): the options the kind's exec stream is made with.
    pub fn exec_options_for(&self, o: Option<serde_json::Map<String, Value>>) -> Option<serde_json::Map<String, Value>> {
        todo!()
    }
    /// kinds.ts:546 (rule 24).
    pub fn project_memory_key(&self, agent: &str, project: &ProjectView) -> Option<String> {
        todo!()
    }
    /// kinds.ts:556 (rule 25).
    pub async fn thread_folder(&self, entry: &Live, cwd: Option<&str>) -> Result<String, HostError> {
        todo!()
    }
}
