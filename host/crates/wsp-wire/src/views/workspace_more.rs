// SPDX-License-Identifier: AGPL-3.0-only
//! The later shapes of packages/protocol/src/views/workspace.ts at a3b870a6e, apart only for the repo's file-size law; workspace.rs re-exports them.

#![allow(unused_imports)]

use serde::{Deserialize, Serialize};

use super::workspace::*;
use crate::literal;
use crate::types::changes::Checkout;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::types::pull_request::PullRequestSeen;
use crate::types::start::ReviewDraft;
use crate::types::start::WorkspaceFrom;
use crate::types::tree::TreeFact;
use crate::types::workspace_look::WorkspaceGlyph;
use crate::types::workspace_look::WorkspaceTheme;
use crate::views::workspace_kind::WorkspaceKind;
use wsp_frames::PauseMode;
use wsp_frames::True;
use wsp_frames::WorkspaceSize;

/// TS: packages/protocol/src/views/workspace.ts:250 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBranch {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub branch: Option<String>,
    pub folder: bool,
}

impl Check for ProjectBranch {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:255 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
    pub path: String,
    pub computer: String,
}

impl Check for ProjectRef {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:262 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DaemonKind {
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "place")]
    Place,
}

impl Check for DaemonKind {}

/// TS: packages/protocol/src/views/workspace.ts:270 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceOrigin {
    #[serde(rename = "here")]
    Here,
    #[serde(rename = "relayed")]
    Relayed,
    #[serde(rename = "paired")]
    Paired,
}

impl Check for WorkspaceOrigin {}

/// TS: packages/protocol/src/views/workspace.ts:276 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadScope {
    pub kind: ThreadLit,
    pub thread_id: String,
    pub workspace_id: String,
    pub root_thread_id: String,
}

impl Check for ThreadScope {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:287 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventAsker {
    pub thread_id: String,
    pub root_thread_id: String,
}

impl Check for EventAsker {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:304 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceAgents {
    pub spawn: bool,
    pub max_machines: i64,
    pub max_depth: i64,
}

impl Check for WorkspaceAgents {
    /// Rules: maxMachines: >= 0; maxDepth: >= 1.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:316 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CopyRoad {
    #[serde(rename = "clonefile")]
    Clonefile,
    #[serde(rename = "worktree")]
    Worktree,
}

impl Check for CopyRoad {}

/// TS: packages/protocol/src/views/workspace.ts:321 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::Carried;

impl Check for Carried {}

/// TS: packages/protocol/src/views/workspace.ts:327 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyAsk {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    pub exclude: Vec<String>,
    pub size_line_bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<CopyRoad>,
}

impl Check for CopyAsk {
    /// Rules: sizeLineBytes: >= 0.
    /// Nested checks: road.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:343 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyReport {
    pub road: CopyRoad,
    pub path: String,
    pub base: String,
    pub branch: String,
    pub fetched: bool,
    pub carried: Carried,
    pub excluded: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<Vec<String>>,
    pub bytes: i64,
    pub ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fell_back: Option<String>,
}

impl Check for CopyReport {
    /// Rules: bytes: >= 0; ms: >= 0.
    /// Nested checks: road, carried.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:364 at a3b870a6e.
/// The daemon's own shape: wsp-frames declares it, with the same keys in the same order.
pub use wsp_frames::CarryModule;

impl Check for CarryModule {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:371 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeAsk {
    pub from: String,
    pub home: String,
    pub project: String,
    pub branch: String,
    pub modules: Vec<CarryModule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<String>,
}

impl Check for WorktreeAsk {
    /// Nested checks: modules.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:385 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopy {
    pub path: String,
    pub base: String,
    pub branch: String,
    pub carried: Carried,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fell_back: Option<String>,
    pub road: CopyRoad,
    pub source: String,
}

impl Check for ProjectCopy {
    /// Nested checks: carried, road.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorktreeSettledWhy {
    #[serde(rename = "merged")]
    Merged,
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "removed")]
    Removed,
}

impl Check for WorktreeSettledWhy {}

/// TS: packages/protocol/src/views/workspace.ts:394 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSettled {
    pub at: JsNumber,
    pub why: WorktreeSettledWhy,
}

impl Check for WorktreeSettled {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:402 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeFolder {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub made: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub made_for: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled: Option<WorktreeSettled>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove_failed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<True>,
}

impl Check for WorktreeFolder {
    /// Nested checks: settled.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:416 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundModule {
    pub id: String,
    pub folder: String,
    pub rebuild: bool,
}

impl Check for FoundModule {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:424 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeReport {
    pub path: String,
    pub branch: String,
    pub made: bool,
    pub carried: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plain: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlaid: Option<Vec<String>>,
    pub fresh: bool,
    pub modules: Vec<FoundModule>,
    pub ms: i64,
}

impl Check for WorktreeReport {
    /// Rules: ms: >= 0.
    /// Nested checks: modules.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:439 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemoval {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rescued: Option<String>,
}

impl Check for WorktreeRemoval {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:444 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMade {
    pub path: String,
    pub branch: String,
    pub made: bool,
}

impl Check for WorktreeMade {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceViewQueueWhy {
    #[serde(rename = "slot")]
    Slot,
    #[serde(rename = "minute")]
    Minute,
}

impl Check for WorkspaceViewQueueWhy {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewQueue {
    pub since: String,
    pub why: WorkspaceViewQueueWhy,
}

impl Check for WorkspaceViewQueue {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewScreen {
    pub stream_url: String,
}

impl Check for WorkspaceViewScreen {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewDaemonRefusedAt {
    pub machine_id: String,
    pub at: String,
    pub why: String,
}

impl Check for WorkspaceViewDaemonRefusedAt {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:454 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<WorkspaceViewQueue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<WorkspaceViewScreen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_of_record: Option<BranchOfRecord>,
}

impl Check for WorkspaceView {
    /// Nested checks: phase, kind, project, queue, screen, theme, glyph, daemonRefusedAt, agents, worktree, pauseMode, from, review, branchOfRecord.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:553 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineFacts {
    pub os: String,
    pub uptime_ms: JsNumber,
    pub folder: String,
}

impl Check for MachineFacts {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatusWakeAsk {
    pub ask: JsNumber,
    pub of: JsNumber,
}

impl Check for WorkspaceStatusWakeAsk {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:557 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<WorkspaceViewQueue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<WorkspaceViewScreen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_of_record: Option<BranchOfRecord>,
    pub machine_state: MachineState,
    pub reach: ReachStatus,
    pub size: WorkspaceSize,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<MachineFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_ask: Option<WorkspaceStatusWakeAsk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<Checkout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PullRequestSeen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<TreeFact>,
}

impl Check for WorkspaceStatus {
    /// Nested checks: phase, kind, project, theme, glyph, agents, worktree, pauseMode, from, review, branchOfRecord, machineState, reach, size, facts, wakeAsk, checkout, pr, tree.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:596 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOut {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDraft>,
}

impl Check for WorkspaceOut {
    /// Nested checks: phase, kind, project, theme, glyph, agents, pauseMode, worktree, from, review.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:602 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTalked {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<String>,
}

impl Check for WorkspaceTalked {
    /// Nested checks: phase, kind, project.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace.ts:608 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListing {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDraft>,
    pub machine_state: MachineState,
    pub size: WorkspaceSize,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<MachineFacts>,
    pub reach: ReachView,
}

impl Check for WorkspaceListing {
    /// Nested checks: phase, kind, project, theme, glyph, agents, pauseMode, worktree, from, review, machineState, size, facts, reach.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
