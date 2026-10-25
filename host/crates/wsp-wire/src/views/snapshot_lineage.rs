// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/snapshot-lineage.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::literal;
use crate::types::js_number::JsNumber;
use crate::views::golden_image::GoldenVersion;
use crate::views::golden_image::SealedImage;
use crate::views::golden_image::SealedImageCopy;
use crate::views::workspace::WorkspaceProject;
use crate::views::workspace::WorkspaceView;

literal!(UntouchedLit, "untouched");

/// TS: packages/protocol/src/views/snapshot-lineage.ts:12 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLineage {
    pub name: String,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub head: Option<JsNumber>,
    pub versions: Vec<GoldenVersion>,
}

impl Check for SnapshotLineage {
    /// Nested checks: versions.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:22 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGolden {
    pub snapshot_id: String,
    pub projects: Vec<WorkspaceProject>,
    pub golden: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i64>,
    pub workspace_id: String,
    pub workspace_name: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
}

impl Check for ProjectGolden {
    /// Nested checks: projects.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:39 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGoldenRemoved {
    pub project_golden: ProjectGolden,
    pub already_gone: bool,
}

impl Check for ProjectGoldenRemoved {
    /// Nested checks: projectGolden.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:43 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedProjectImage {
    pub snapshot_id: String,
    pub projects: Vec<WorkspaceProject>,
    pub golden: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i64>,
    pub workspace_id: String,
    pub workspace_name: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<JsNumber>,
}

impl Check for SealedProjectImage {
    /// Nested checks: projects.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:48 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImageView {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub image: Option<SealedImage>,
    pub copies: Vec<SealedImageCopy>,
    pub projects: Vec<SealedProjectImage>,
}

impl Check for SealedImageView {
    /// Nested checks: image, copies, projects.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:56 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotGroup {
    pub count: JsNumber,
    pub bytes: JsNumber,
}

impl Check for SnapshotGroup {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotStorageNamed {
    pub count: JsNumber,
    pub free: JsNumber,
}

impl Check for SnapshotStorageNamed {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:63 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotStorage {
    pub count: JsNumber,
    pub total_bytes: JsNumber,
    pub free_gb: JsNumber,
    pub usd_per_gb_month: JsNumber,
    pub billed_from: String,
    pub monthly_usd: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named: Option<SnapshotStorageNamed>,
    pub kept: SnapshotGroup,
    pub orphans: SnapshotGroup,
    pub others: SnapshotGroup,
}

impl Check for SnapshotStorage {
    /// Nested checks: named, kept, orphans, others.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:85 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRollbackResult {
    pub lineage: SnapshotLineage,
    pub existing_workspaces: UntouchedLit,
}

impl Check for SnapshotRollbackResult {
    /// Nested checks: lineage.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/views/snapshot-lineage.ts:93 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreateResult {
    pub workspace: WorkspaceView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

impl Check for WorkspaceCreateResult {
    /// Nested checks: workspace.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
