// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/snapshot-storage.ts. Spec: spec/sweeps.md.
//!
//! Snapshot storage as the provider bills it, and which of a golden's versions retention may offer to delete.
use std::collections::{HashMap, HashSet};

use wsp_wire::types::JsNumber;
use wsp_wire::views::golden_image::{golden_head, GoldenManifest, GoldenVersion};
use wsp_wire::views::snapshot_lineage::{SnapshotGroup, SnapshotStorage, SnapshotStorageNamed};

use crate::errors::{EngineError, Result};
use crate::machine::{SnapshotRow, SnapshotStoragePricing, TemplateRow};
use crate::orphans::OWN_GRACE_MS;
use crate::snapshot_names::name_owner;

/// snapshot-storage.ts:10. The provider quotes decimal GB.
pub const GB: f64 = 1e9;

/// snapshot-storage.ts:12. `max(0, bytes / GB - free_gb) * usd_per_gb_month + max(0, named - free_named) *
/// usd_per_named_month`, the two absent counts read as 0.
pub fn snapshot_monthly_usd(total_bytes: f64, pricing: &SnapshotStoragePricing, named: u64) -> f64 {
    let free_named = f64::from(pricing.free_named.unwrap_or(0));
    let counted = (named as f64 - free_named).max(0.0) * pricing.usd_per_named_month.unwrap_or(0.0);
    (total_bytes / GB - pricing.wire.free_gb.0).max(0.0) * pricing.wire.usd_per_gb_month.0 + counted
}

/// snapshot-storage.ts:18. No GB price and a price per named snapshot (Boat).
pub fn billed_by_count(pricing: &SnapshotStoragePricing) -> bool {
    pricing.wire.usd_per_gb_month.0 == 0.0 && pricing.usd_per_named_month.is_some()
}

/// snapshot-storage.ts:21. Rows that carry a name.
pub fn named_count(rows: &[SnapshotRow]) -> u64 {
    rows.iter().filter(|r| r.name.is_some()).count() as u64
}

/// snapshot-storage.ts:24. Anything the provider lists that wsp may have made: a snapshot or a template.
pub trait OwnedRow {
    fn id(&self) -> &str;
    fn name(&self) -> Option<&str>;
    fn created_at(&self) -> Option<&str>;
}

impl OwnedRow for SnapshotRow {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    fn created_at(&self) -> Option<&str> {
        self.created_at.as_deref()
    }
}

impl OwnedRow for TemplateRow {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> Option<&str> {
        Some(&self.name)
    }
    fn created_at(&self) -> Option<&str> {
        self.created_at.as_deref()
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
    if read.recorded.contains(row.id()) {
        return Ownership::Kept;
    }
    if name_owner(row.name()).as_deref() != Some(read.host_id.as_str()) {
        return Ownership::Foreign;
    }
    let at = crate::orphans::date_parse(row.created_at().unwrap_or(""));
    if at.is_nan() || (read.now as f64 - at) < OWN_GRACE_MS as f64 {
        Ownership::Kept
    } else {
        Ownership::Orphan
    }
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
    if read.host_id.is_empty() {
        return Err(EngineError::Other {
            message: "the snapshot split needs this host's mark; without one it could not tell this host's snapshots from another host's"
                .into(),
            code: None,
        });
    }
    let mut split = OwnerSplit { kept: Vec::new(), orphans: Vec::new(), foreign: Vec::new() };
    for row in rows {
        match ownership_of(row, read) {
            Ownership::Kept => split.kept.push(row.clone()),
            Ownership::Orphan => split.orphans.push(row.clone()),
            Ownership::Foreign => split.foreign.push(row.clone()),
        }
    }
    Ok(split)
}

fn group(rows: &[SnapshotRow]) -> SnapshotGroup {
    SnapshotGroup { count: JsNumber(rows.len() as f64), bytes: JsNumber(rows.iter().map(|r| r.size_bytes.0).sum()) }
}

/// snapshot-storage.ts:81. The listing as the storage view (key order count, totalBytes, freeGb, usdPerGbMonth,
/// billedFrom, monthlyUsd, named where billed by count, kept, orphans, others). The empty-host refusal as the split's.
pub fn snapshot_storage(rows: &[SnapshotRow], pricing: &SnapshotStoragePricing, read: &OwnerRead) -> Result<SnapshotStorage> {
    let total_bytes: f64 = rows.iter().map(|r| r.size_bytes.0).sum();
    let split = split_by_owner(rows, read)?;
    let named = named_count(rows);
    Ok(SnapshotStorage {
        count: JsNumber(rows.len() as f64),
        total_bytes: JsNumber(total_bytes),
        free_gb: pricing.wire.free_gb,
        usd_per_gb_month: pricing.wire.usd_per_gb_month,
        billed_from: pricing.wire.billed_from.clone(),
        monthly_usd: JsNumber(snapshot_monthly_usd(total_bytes, pricing, named)),
        named: billed_by_count(pricing)
            .then(|| SnapshotStorageNamed { count: JsNumber(named as f64), free: JsNumber(f64::from(pricing.free_named.unwrap_or(0))) }),
        kept: group(&split.kept),
        orphans: group(&split.orphans),
        others: group(&split.foreign),
    })
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

/// The version a version's parent is: its recorded parent snapshot, else the highest version below it, which is
/// assumed where the version has no base recorded.
fn parent_of<'a>(
    manifest: &'a GoldenManifest,
    by_snapshot: &HashMap<&str, &'a GoldenVersion>,
    v: &GoldenVersion,
) -> (Option<&'a GoldenVersion>, bool) {
    if let Some(parent) = &v.parent_snapshot_id {
        return (by_snapshot.get(parent.as_str()).copied(), false);
    }
    let mut below: Vec<&GoldenVersion> = manifest.versions.iter().filter(|x| x.version.0 < v.version.0).collect();
    below.sort_by(|a, b| b.version.0.total_cmp(&a.version.0));
    (below.first().copied(), !below.is_empty() && v.base.is_none())
}

/// snapshot-storage.ts:127. Rules in spec/sweeps.md; `keep_count` is `RETENTION_KEEP` when `None`.
pub fn retention_plan(
    manifest: &GoldenManifest,
    rows: &[SnapshotRow],
    forked_from: &dyn Fn(&str) -> Vec<String>,
    pricing: &SnapshotStoragePricing,
    keep_count: Option<usize>,
) -> RetentionPlan {
    let keep_count = keep_count.unwrap_or(RETENTION_KEEP);
    let head = golden_head(Some(manifest));
    let mut by_snapshot: HashMap<&str, &GoldenVersion> = HashMap::new();
    for v in &manifest.versions {
        by_snapshot.insert(v.snapshot_id.as_str(), v);
    }
    let same = |a: &GoldenVersion, b: &GoldenVersion| std::ptr::eq(a, b);

    let mut chain: Vec<&GoldenVersion> = head.into_iter().collect();
    let mut parent_assumed = false;
    let mut cur = head;
    while let Some(version) = cur {
        let (parent, assumed) = parent_of(manifest, &by_snapshot, version);
        let Some(parent) = parent else { break };
        if chain.iter().any(|c| same(c, parent)) {
            break;
        }
        if chain.len() == 1 {
            parent_assumed = assumed;
        }
        chain.push(parent);
        cur = Some(parent);
    }
    let kept_count = keep_count.min(chain.len());
    let keep: Vec<GoldenVersion> = chain[..kept_count].iter().map(|v| (*v).clone()).collect();
    let older: Vec<&GoldenVersion> = chain[kept_count..].iter().rev().copied().collect();
    let mut off_chain: Vec<&GoldenVersion> = match head {
        None => Vec::new(),
        Some(head) => manifest.versions.iter().filter(|v| v.version.0 < head.version.0 && !chain.iter().any(|c| same(c, v))).collect(),
    };
    off_chain.sort_by(|a, b| a.version.0.total_cmp(&b.version.0));

    let mut drop: Vec<GoldenVersion> = Vec::new();
    let mut abandoned: Vec<GoldenVersion> = Vec::new();
    let mut guarded: Vec<GuardedVersion> = Vec::new();
    for version in older.iter().chain(off_chain.iter()) {
        let workspaces = forked_from(&version.snapshot_id);
        if !workspaces.is_empty() {
            guarded.push(GuardedVersion { version: (*version).clone(), workspaces });
            continue;
        }
        drop.push((*version).clone());
        if off_chain.iter().any(|o| same(o, version)) {
            abandoned.push((*version).clone());
        }
    }

    let size_of = |id: &str| rows.iter().find(|r| r.id == id).map_or(0.0, |r| r.size_bytes.0);
    let total_bytes: f64 = rows.iter().map(|r| r.size_bytes.0).sum();
    let freed_bytes: f64 = drop.iter().map(|v| size_of(&v.snapshot_id)).sum();
    let named = named_count(rows);
    let dropped_rows: Vec<SnapshotRow> = rows.iter().filter(|r| drop.iter().any(|v| v.snapshot_id == r.id)).cloned().collect();
    let named_freed = named_count(&dropped_rows);
    let saves_usd_per_month = snapshot_monthly_usd(total_bytes, pricing, named)
        - snapshot_monthly_usd(total_bytes - freed_bytes, pricing, named.saturating_sub(named_freed));

    RetentionPlan {
        keep,
        drop,
        abandoned,
        guarded,
        parent_assumed,
        freed_bytes,
        named_freed,
        saves_usd_per_month,
        pricing: pricing.clone(),
    }
}
