// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/snapshot-storage.ts. Spec: spec/sweeps.md.
//!
//! Snapshot storage as the provider bills it, and which of a golden's versions retention may offer to delete.
use std::collections::HashSet;

use wsp_wire::views::golden_image::{GoldenManifest, GoldenVersion};
use wsp_wire::views::snapshot_lineage::SnapshotStorage;

use crate::errors::Result;
use crate::machine::{SnapshotRow, SnapshotStoragePricing};

/// snapshot-storage.ts:10. The provider quotes decimal GB.
pub const GB: f64 = 1e9;

/// snapshot-storage.ts:12. `max(0, bytes / GB - free_gb) * usd_per_gb_month + max(0, named - free_named) *
/// usd_per_named_month`, the two absent counts read as 0.
pub fn snapshot_monthly_usd(total_bytes: f64, pricing: &SnapshotStoragePricing, named: u64) -> f64 {
    todo!()
}

/// snapshot-storage.ts:18. No GB price and a price per named snapshot (Boat).
pub fn billed_by_count(pricing: &SnapshotStoragePricing) -> bool {
    todo!()
}

/// snapshot-storage.ts:21. Rows that carry a name.
pub fn named_count(rows: &[SnapshotRow]) -> u64 {
    todo!()
}

/// snapshot-storage.ts:24. Anything the provider lists that wsp may have made: a snapshot or a template.
pub trait OwnedRow {
    fn id(&self) -> &str;
    fn name(&self) -> Option<&str>;
    fn created_at(&self) -> Option<&str>;
}

impl OwnedRow for SnapshotRow {
    fn id(&self) -> &str {
        todo!()
    }
    fn name(&self) -> Option<&str> {
        todo!()
    }
    fn created_at(&self) -> Option<&str> {
        todo!()
    }
}

impl OwnedRow for crate::machine::TemplateRow {
    fn id(&self) -> &str {
        todo!()
    }
    fn name(&self) -> Option<&str> {
        todo!()
    }
    fn created_at(&self) -> Option<&str> {
        todo!()
    }
}

/// snapshot-storage.ts:35.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ownership {
    Kept,
    Orphan,
    Foreign,
}

/// snapshot-storage.ts:39.
#[derive(Debug, Clone, Default)]
pub struct OwnerRead {
    pub host_id: String,
    pub recorded: HashSet<String>,
    pub now: i64,
}

/// snapshot-storage.ts:55. Recorded is kept; a name whose mark is not this host's is foreign; else kept while younger
/// than `OWN_GRACE_MS` or of no readable age, orphan past it.
pub fn ownership_of(row: &dyn OwnedRow, read: &OwnerRead) -> Ownership {
    todo!()
}

/// snapshot-storage.ts:61.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnerSplit<T> {
    pub kept: Vec<T>,
    pub orphans: Vec<T>,
    pub foreign: Vec<T>,
}

/// snapshot-storage.ts:69. Each row into its group, row order kept. An empty `host_id` fails with
/// `the snapshot split needs this host's mark; without one it could not tell this host's snapshots from another host's`.
pub fn split_by_owner<T: OwnedRow + Clone>(rows: &[T], read: &OwnerRead) -> Result<OwnerSplit<T>> {
    todo!()
}

/// snapshot-storage.ts:81. The listing as the storage view (key order count, totalBytes, freeGb, usdPerGbMonth,
/// billedFrom, monthlyUsd, named where billed by count, kept, orphans, others). The empty-host refusal as the split's.
pub fn snapshot_storage(rows: &[SnapshotRow], pricing: &SnapshotStoragePricing, read: &OwnerRead) -> Result<SnapshotStorage> {
    todo!()
}

/// snapshot-storage.ts:99. Versions the head and its parent.
pub const RETENTION_KEEP: usize = 2;

/// snapshot-storage.ts:109.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardedVersion {
    pub version: GoldenVersion,
    pub workspaces: Vec<String>,
}

/// snapshot-storage.ts:101.
#[derive(Debug, Clone, PartialEq)]
pub struct RetentionPlan {
    /// The head and the ancestors under it that stay, newest first.
    pub keep: Vec<GoldenVersion>,
    /// Older ancestors oldest first, then the abandoned branches by version.
    pub drop: Vec<GoldenVersion>,
    pub abandoned: Vec<GoldenVersion>,
    pub guarded: Vec<GuardedVersion>,
    pub parent_assumed: bool,
    pub freed_bytes: f64,
    pub named_freed: u64,
    pub saves_usd_per_month: f64,
    pub pricing: SnapshotStoragePricing,
}

/// snapshot-storage.ts:127. Rules in spec/sweeps.md; `keep_count` is `RETENTION_KEEP` when `None`.
pub fn retention_plan(
    manifest: &GoldenManifest,
    rows: &[SnapshotRow],
    forked_from: &dyn Fn(&str) -> Vec<String>,
    pricing: &SnapshotStoragePricing,
    keep_count: Option<usize>,
) -> RetentionPlan {
    todo!()
}
