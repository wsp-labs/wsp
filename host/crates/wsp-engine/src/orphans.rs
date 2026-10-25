// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/orphans.ts. Spec: spec/sweeps.md.
//!
//! Kills running machines that carry our label but that nothing claims; reports another owner's, never kills them.
use std::collections::HashSet;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use crate::errors::Result;
use crate::machine::{Machine, MachineBackend, MachineListRow, NamedMachine};

/// orphans.ts:9. A machine of this owner this young is spared on age alone.
pub const OWN_GRACE_MS: i64 = 60_000;
/// orphans.ts:97. How old a machine named after one of this host's images must be before the sweep deletes it.
pub const STRAY_GRACE_MS: i64 = 10 * 60_000;
/// orphans.ts:129. The default `older_than_ms`.
pub const UNOWNED_BACKSTOP_MS: i64 = 10 * 60_000;

/// orphans.ts:12. One row of the provider's listing.
pub type ListedMachine = MachineListRow;

/// orphans.ts:22 `stop`: stops one machine the sweep kills.
pub type StopMachine = Arc<dyn Fn(Arc<dyn Machine>) -> LocalBoxFuture<'static, Result<()>>>;

/// orphans.ts:14.
pub struct ReapOptions {
    pub backend: Arc<dyn MachineBackend>,
    /// The listing the caller already fetched; `None` reads `backend.list(None)`.
    pub listing: Option<Vec<ListedMachine>>,
    /// Ids something claims. Read once, after the listing returns.
    pub known_ids: Box<dyn Fn() -> Vec<String>>,
    /// The sweeping state file's id.
    pub owner: String,
    pub stop: StopMachine,
    /// Age an unclaimed unowned non-builder must reach; `UNOWNED_BACKSTOP_MS` when `None`.
    pub older_than_ms: Option<i64>,
    /// Epoch ms, read once; the wall clock when `None`.
    pub now: Option<Box<dyn Fn() -> i64>>,
}

/// orphans.ts:30.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Whose {
    Own,
    Foreign,
    None,
}

/// orphans.ts:34. By the `wsp-owner` label alone: absent is `None`, equal to `owner` is `Own`, else `Foreign`.
pub fn whose_machine(labels: &IndexMap<String, String>, owner: &str) -> Whose {
    todo!()
}

/// orphans.ts:51.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReapReason {
    Recorded,
    Unfinished,
    Expired,
    Grace,
    Own,
    Orphan,
    Lost,
    Stray,
    Unrecorded,
}

/// orphans.ts:39.
#[derive(Debug, Clone, PartialEq)]
pub struct ReapedMachine {
    pub id: String,
    pub labels: Option<IndexMap<String, String>>,
    pub builder: bool,
    pub reason: ReapReason,
    pub age_ms: Option<i64>,
    pub name: Option<String>,
}

/// orphans.ts:58.
#[derive(Debug, Clone, PartialEq)]
pub struct SparedMachine {
    pub id: String,
    pub labels: IndexMap<String, String>,
    pub builder: bool,
    pub whose: Whose,
    pub owner: Option<String>,
    pub age_ms: Option<i64>,
    pub backstop_ms: i64,
    pub rate_usd_per_hour: f64,
}

/// orphans.ts:71.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReapFailure {
    /// `None` when the listing itself failed.
    pub id: Option<String>,
    pub message: String,
}

/// orphans.ts:77. `failed` is `None` where nothing failed, never an empty list.
#[derive(Debug, Clone, PartialEq)]
pub struct ReapResult {
    pub reaped: Vec<ReapedMachine>,
    pub spared: Vec<SparedMachine>,
    pub failed: Option<Vec<ReapFailure>>,
}

/// orphans.ts:87. Running or paused; not reserved; wears `wsp=1` and this owner's label; not a builder or smoke fork
/// (`=1`); and its `createdAt` label unparseable (or absent) or at least `OWN_GRACE_MS` old.
pub fn lost_workspace(m: &ListedMachine, owner: &str, now: i64) -> bool {
    todo!()
}

/// orphans.ts:102 `read`.
#[derive(Debug, Clone, Default)]
pub struct StrayRead {
    pub known: HashSet<String>,
    pub asked: HashSet<String>,
    pub now: i64,
}

/// orphans.ts:102. In row order, each with its age: not gone, id not known, name not asked, `created_at` parseable and
/// at least `STRAY_GRACE_MS` old.
pub fn strays(rows: &[NamedMachine], read: &StrayRead) -> Vec<(NamedMachine, i64)> {
    todo!()
}

/// orphans.ts:111. `age unknown`; under a minute `<floor s> s old`; under an hour `<floor min> min old`; else
/// `<h with one decimal, JS toFixed> h old`. A negative age reads as 0.
pub fn describe_age(ms: Option<i64>) -> String {
    todo!()
}

/// orphans.ts:127. An empty owner fails before anything is read, with
/// `reap needs the owner id; an empty one would claim every unowned machine`. Rules in spec/sweeps.md; a listing that
/// fails is the failure (the TS `await` throws it).
pub async fn reap(opts: ReapOptions) -> Result<ReapResult> {
    todo!()
}
