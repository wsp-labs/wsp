// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/snapshot-lineage.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::snapshot_lineage::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_snapshot_lineage.json");

/// 5 cases: what zod's parse of ProjectGolden wrote, or the first issue it raised.
#[test]
fn project_golden_reads_and_writes_as_zod_does() {
    cases::<ProjectGolden>(FIXTURES, "ProjectGolden");
}

/// 4 cases: what zod's parse of ProjectGoldenRemoved wrote, or the first issue it raised.
#[test]
fn project_golden_removed_reads_and_writes_as_zod_does() {
    cases::<ProjectGoldenRemoved>(FIXTURES, "ProjectGoldenRemoved");
}

/// 5 cases: what zod's parse of SealedImageView wrote, or the first issue it raised.
#[test]
fn sealed_image_view_reads_and_writes_as_zod_does() {
    cases::<SealedImageView>(FIXTURES, "SealedImageView");
}

/// 5 cases: what zod's parse of SealedProjectImage wrote, or the first issue it raised.
#[test]
fn sealed_project_image_reads_and_writes_as_zod_does() {
    cases::<SealedProjectImage>(FIXTURES, "SealedProjectImage");
}

/// 3 cases: what zod's parse of SnapshotGroup wrote, or the first issue it raised.
#[test]
fn snapshot_group_reads_and_writes_as_zod_does() {
    cases::<SnapshotGroup>(FIXTURES, "SnapshotGroup");
}

/// 5 cases: what zod's parse of SnapshotLineage wrote, or the first issue it raised.
#[test]
fn snapshot_lineage_reads_and_writes_as_zod_does() {
    cases::<SnapshotLineage>(FIXTURES, "SnapshotLineage");
}

/// 4 cases: what zod's parse of SnapshotRollbackResult wrote, or the first issue it raised.
#[test]
fn snapshot_rollback_result_reads_and_writes_as_zod_does() {
    cases::<SnapshotRollbackResult>(FIXTURES, "SnapshotRollbackResult");
}

/// 4 cases: what zod's parse of SnapshotStorage wrote, or the first issue it raised.
#[test]
fn snapshot_storage_reads_and_writes_as_zod_does() {
    cases::<SnapshotStorage>(FIXTURES, "SnapshotStorage");
}

/// 4 cases: what zod's parse of WorkspaceCreateResult wrote, or the first issue it raised.
#[test]
fn workspace_create_result_reads_and_writes_as_zod_does() {
    cases::<WorkspaceCreateResult>(FIXTURES, "WorkspaceCreateResult");
}
