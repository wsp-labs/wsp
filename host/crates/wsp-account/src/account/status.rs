// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/status.ts at 65f6b89c7: the rows the status tracker polls and the
//! re-recording of lost machines. Spec: spec/status.md.
use std::collections::BTreeSet;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{Map, Value};

use wsp_core::types::wiring::{AdoptedMachine, WorkspaceRecord};
use wsp_engine::orphans::{ReapFailure, ReapedMachine};
use wsp_words::exit::HostError;

use crate::account::core_types::{AccountResult, Live};

/// One row of a provider's listing as the sweep hands it (`ListedMachine`): id, state and labels.
#[derive(Clone, Debug, PartialEq)]
pub struct ListedRow {
    pub id: String,
    pub state: String,
    pub labels: IndexMap<String, String>,
}

/// A machine the confirming read found (`ctx.observed(await at.backend.get(id))`).
#[derive(Clone, Debug, PartialEq)]
pub struct Confirmed {
    /// `machine.seen?.state ?? await machine.state()`.
    pub state: String,
    pub stream_url: Option<String>,
    /// `ctx.sizeBuilt(await ctx.shapeOf(machine), at.backend.pricing.defaultSize)` as the record writes it.
    pub size: Value,
}

/// What a backend read answered: missing (`isMissing`), or another failure's message.
#[derive(Clone, Debug, PartialEq)]
pub enum ReadFailure {
    Missing,
    Failed(String),
}

/// What the rows and `adoptLost` read off the runtime's context.
#[async_trait(?Send)]
pub trait StatusCtx {
    // The rows (status.ts:13-43).
    async fn ready(&self);
    fn held(&self) -> Vec<Live>;
    /// `ctx.view(record)` as the TS object.
    fn view(&self, record: &WorkspaceRecord) -> Map<String, Value>;
    fn rate_usd_per_hour(&self, record: &WorkspaceRecord) -> f64;
    fn row_reason(&self, entry: &Live) -> Option<String>;
    fn idle_at(&self, workspace_id: &str) -> Option<f64>;
    fn away_line(&self, record: &WorkspaceRecord) -> Option<String>;
    fn unreached_of(&self, entry: &Live) -> Option<Value>;
    /// `ctx.reachOf(entry)`.
    fn reach_of(&self, entry: &Live) -> String;
    fn has_daemon(&self, entry: &Live) -> bool;
    /// Which optional machine reads the entry's machine offers: `daemonAnswers`, `daemonAnswersPromptMs`, `metrics`,
    /// `facts`, in that order.
    fn machine_reads(&self, entry: &Live) -> (bool, Option<f64>, bool, bool);

    // The poll (status.ts:49-66).
    fn polled_reach_set(&self, workspace_id: &str, machine_id: &str, reach: &str);
    fn polled_reach_delete(&self, workspace_id: &str);
    fn boot_work_has(&self, workspace_id: &str) -> bool;
    fn revive_daemon(&self, entry: &Live, reach: &str);
    fn offer_daemon_again(&self, entry: &Live, status: &Value);
    fn read_version_again(&self, entry: &Live, status: &Value);
    async fn adopt_gone(&self, entry: &Live, reason: &str) -> AccountResult<()>;
    fn live(&self, workspace_id: &str) -> Option<Live>;

    // adoptLost (status.ts:81-180).
    async fn store_get(&self, collection: &str, id: &str) -> AccountResult<Option<Value>>;
    async fn store_put(&self, collection: &str, id: &str, value: Value) -> AccountResult<()>;
    async fn store_list(&self, collection: &str) -> AccountResult<Vec<Value>>;
    async fn store_delete(&self, collection: &str, id: &str) -> AccountResult<()>;
    /// `clock.now()`.
    fn now(&self) -> i64;
    /// `Date.now()` and `new Date()`: the same fake reading in tests (README A7).
    fn wall_now(&self) -> i64;
    fn owner(&self) -> String;
    /// `lostWorkspace(row, owner, now)` (wsp_engine::orphans).
    fn lost_workspace(&self, row: &ListedRow, owner: &str, now: i64) -> bool;
    fn project_held(&self, id: &str) -> bool;
    fn name_refusal(&self, name: &str) -> Option<String>;
    fn places_wired(&self) -> String;
    /// `goldenHead(await ctx.golden.get())?.snapshotId`.
    async fn golden_head(&self) -> Option<String>;
    /// `at.backend.get(id)` observed and its state, shape and size read.
    async fn confirm(&self, place: &str, machine_id: &str) -> Result<Confirmed, ReadFailure>;
    /// `ctx.gone.stop(at.backend, await at.backend.get(id))`.
    async fn stop_unrecorded(&self, place: &str, machine_id: &str) -> Result<(), ReadFailure>;
    /// `ctx.attach(record, machine)` for the machine `confirm` read.
    fn attach(&self, record: WorkspaceRecord) -> Live;
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()>;
    fn sync_daemon(&self, entry: &Live);
    fn emit(&self, event: Value);
    async fn emit_status(&self, entry: &Live, reach: &str, reason: &str);
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
}

/// status.ts:9 `statusArea(ctx)`: the status tracker itself is wsp-core's (`createStatusTracker`); this area hands it
/// the rows and the callbacks.
pub struct StatusArea {
    pub(crate) ctx: Rc<dyn StatusCtx>,
}

impl StatusArea {
    pub fn new(ctx: Rc<dyn StatusCtx>) -> Rc<StatusArea> {
        todo!()
    }

    /// S1, status.ts:13: every held entry's row as the TS object, its callbacks left out (the tracker's join adds them).
    pub async fn records(&self) -> Vec<Map<String, Value>> {
        todo!()
    }

    /// S2, status.ts:49. Each status as the TS object (`id`, `machineId`, `phase`, `reach.state`, ...).
    pub fn on_polled(&self, statuses: &[Value]) {
        todo!()
    }

    /// S3, status.ts:62.
    pub fn on_gone(&self, workspace_id: &str, machine_id: &str, reason: &str) {
        todo!()
    }

    /// S4-S10, status.ts:81. `known` gains every row the engine must spare; `failed` gains the reports.
    pub async fn adopt_lost(&self, listing: &[ListedRow], known: &mut BTreeSet<String>, failed: &mut Vec<ReapFailure>, place: &str) -> Result<(Vec<AdoptedMachine>, Vec<ReapedMachine>), HostError> {
        todo!()
    }
}
