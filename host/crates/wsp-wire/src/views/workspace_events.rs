// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/workspace-events.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;
use crate::types::workspace_look::WorkspaceGlyph;
use crate::types::workspace_look::WorkspaceTheme;
use crate::views::workspace::EventAsker;
use crate::views::workspace::WorkspacePhase;
use crate::views::workspace::WorkspaceStatus;
use crate::views::workspace::WorkspaceView;
use indexmap::IndexMap;
use wsp_frames::True;

/// TS: packages/protocol/src/views/workspace-events.ts:13 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreatedEvent {
    pub workspace: WorkspaceView,
}

impl WorkspaceCreatedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.created";
}

impl Check for WorkspaceCreatedEvent {
    /// Nested checks: workspace.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:15 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceCreateStage {
    #[serde(rename = "fork-requested")]
    ForkRequested,
    #[serde(rename = "hostname-set")]
    HostnameSet,
    #[serde(rename = "preview-route")]
    PreviewRoute,
    #[serde(rename = "daemon-answering")]
    DaemonAnswering,
    #[serde(rename = "project-cloned")]
    ProjectCloned,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "failed")]
    Failed,
}

impl Check for WorkspaceCreateStage {}

/// TS: packages/protocol/src/views/workspace-events.ts:25 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreatingEvent {
    pub workspace_id: String,
    pub name: String,
    pub stage: WorkspaceCreateStage,
    pub message: String,
    pub elapsed_ms: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked_by: Option<EventAsker>,
}

impl WorkspaceCreatingEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.creating";
}

impl Check for WorkspaceCreatingEvent {
    /// Nested checks: stage, askedBy.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:47 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceNappedEvent {
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found: Option<bool>,
}

impl WorkspaceNappedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.napped";
}

impl Check for WorkspaceNappedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:48 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWokenEvent {
    pub workspace_id: String,
    pub machine_id: String,
}

impl WorkspaceWokenEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.woken";
}

impl Check for WorkspaceWokenEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:55 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUpgradedEvent {
    pub workspace_id: String,
    pub machine_id: String,
}

impl WorkspaceUpgradedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.upgraded";
}

impl Check for WorkspaceUpgradedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:62 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRenamedEvent {
    pub workspace_id: String,
    pub name: String,
}

impl WorkspaceRenamedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.renamed";
}

impl Check for WorkspaceRenamedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:65 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLookEvent {
    pub workspace_id: String,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub glyph: Option<WorkspaceGlyph>,
}

impl WorkspaceLookEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.look";
}

impl Check for WorkspaceLookEvent {
    /// Nested checks: theme, glyph.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:71 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeletedEvent {
    pub workspace_id: String,
}

impl WorkspaceDeletedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.deleted";
}

impl Check for WorkspaceDeletedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:74 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGoneEvent {
    pub workspace_id: String,
    pub machine_id: String,
    pub reason: String,
}

impl WorkspaceGoneEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.gone";
}

impl Check for WorkspaceGoneEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:82 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReleasedEvent {
    pub workspace_id: String,
}

impl WorkspaceReleasedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.released";
}

impl Check for WorkspaceReleasedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:84 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatusEvent {
    pub status: WorkspaceStatus,
}

impl WorkspaceStatusEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.status";
}

impl Check for WorkspaceStatusEvent {
    /// Nested checks: status.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:86 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReviewEvent {
    pub workspace_id: String,
}

impl WorkspaceReviewEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.review";
}

impl Check for WorkspaceReviewEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:90 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewedEvent {
    pub workspace_id: String,
    pub viewed: IndexMap<String, String>,
}

impl WorkspaceViewedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.viewed";
}

impl Check for WorkspaceViewedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:94 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCostEvent {
    pub workspace_id: String,
    pub phase: WorkspacePhase,
    pub rate_usd_per_hour: JsNumber,
    pub awake_ms: JsNumber,
    pub accrued_usd: JsNumber,
    pub at: String,
}

impl WorkspaceCostEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "workspace.cost";
}

impl Check for WorkspaceCostEvent {
    /// Nested checks: phase.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:109 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortOpenEvent {
    pub workspace_id: String,
    pub port: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
}

impl PortOpenEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "port.open";
}

impl Check for PortOpenEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:118 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortCloseEvent {
    pub workspace_id: String,
    pub port: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exited: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<String>,
}

impl PortCloseEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "port.close";
}

impl Check for PortCloseEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/workspace-events.ts:119 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxFileEvent {
    pub workspace_id: String,
    pub path: String,
    pub bytes: JsNumber,
}

impl InboxFileEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "inbox.file";
}

impl Check for InboxFileEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
