// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/status.ts at 65f6b89c7: the rows the status tracker polls and the
//! re-recording of lost machines. Spec: spec/status.md.
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use wsp_core::types::internal::{DROPPED, DROPPED_WATCH_MS, UNRECORDED, UNRECORDED_DELETE_MS, WORKSPACES, WORKSPACE_NAMES};
use wsp_core::types::wiring::{AdoptedMachine, WorkspaceRecord};
use wsp_engine::labels::{CREATED_AT_LABEL, GOLDEN_LABEL, WORKSPACE_LABEL};
use wsp_engine::machine::Machine;
use wsp_engine::orphans::{ReapFailure, ReapReason, ReapedMachine};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_words::date_parse::date_parse;
use wsp_words::exit::HostError;
use wsp_words::js_date::iso_string;
use wsp_words::words::thread::RECORD_RESTORED;

use crate::account::core_types::{AccountResult, Entry};

/// One row of a provider's listing as the sweep hands it (`ListedMachine`): id, state and labels.
#[derive(Clone, Debug, PartialEq)]
pub struct ListedRow {
    pub id: String,
    pub state: String,
    pub labels: IndexMap<String, String>,
}

/// A machine the confirming read found (`ctx.observed(await at.backend.get(id))`).
#[derive(Clone)]
pub struct Confirmed {
    /// `machine.seen?.state ?? await machine.state()`.
    pub state: String,
    pub stream_url: Option<String>,
    /// `ctx.sizeBuilt(await ctx.shapeOf(machine), at.backend.pricing.defaultSize)` as the record writes it.
    pub size: Value,
    /// The machine the read found, which the record is attached to.
    pub machine: Arc<dyn Machine>,
}

/// What a backend read answered: missing (`isMissing`), or another failure's message.
#[derive(Clone, Debug, PartialEq)]
pub enum ReadFailure {
    Missing,
    Failed(String),
    /// The machine was read but its `state()` failed: outside the TS `try` (status.ts:149), so `adoptLost` itself
    /// fails with this message and the sweep's adopt stops there.
    StateFailed(String),
}

/// What the rows and `adoptLost` read off the runtime's context.
#[async_trait(?Send)]
pub trait StatusCtx {
    // The rows (status.ts:13-43).
    async fn ready(&self);
    fn held(&self) -> Vec<Entry>;
    /// `ctx.view(record)` as the TS object.
    fn view(&self, record: &WorkspaceRecord) -> Result<Map<String, Value>, HostError>;
    fn rate_usd_per_hour(&self, record: &WorkspaceRecord) -> f64;
    fn row_reason(&self, entry: &Entry) -> Option<String>;
    fn idle_at(&self, workspace_id: &str) -> Option<f64>;
    fn away_line(&self, record: &WorkspaceRecord) -> Option<String>;
    fn unreached_of(&self, entry: &Entry) -> Option<Value>;
    /// `ctx.reachOf(entry)`.
    fn reach_of(&self, entry: &Entry) -> String;
    fn has_daemon(&self, entry: &Entry) -> bool;
    /// Which optional machine reads the entry's machine offers: `daemonAnswers`, `daemonAnswersPromptMs`, `metrics`,
    /// `facts`, in that order.
    fn machine_reads(&self, entry: &Entry) -> (bool, Option<f64>, bool, bool);

    // The poll (status.ts:49-66).
    fn polled_reach_set(&self, workspace_id: &str, machine_id: &str, reach: &str);
    fn polled_reach_delete(&self, workspace_id: &str);
    fn boot_work_has(&self, workspace_id: &str) -> bool;
    fn revive_daemon(&self, entry: &Entry, reach: &str);
    fn offer_daemon_again(&self, entry: &Entry, status: &Value);
    fn read_version_again(&self, entry: &Entry, status: &Value);
    async fn adopt_gone(&self, entry: &Entry, reason: &str) -> AccountResult<()>;
    fn live(&self, workspace_id: &str) -> Option<Entry>;

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
    /// `ctx.attach(record, machine)`.
    fn attach(&self, record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Result<Entry, HostError>;
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()>;
    fn sync_daemon(&self, entry: &Entry);
    fn emit(&self, event: Value);
    async fn emit_status(&self, entry: &Entry, reach: &str, reason: &str);
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
}

/// status.ts:9 `statusArea(ctx)`: the status tracker itself is wsp-core's (`createStatusTracker`); this area hands it
/// the rows and the callbacks.
pub struct StatusArea {
    pub(crate) ctx: Rc<dyn StatusCtx>,
}

fn record_of_entry(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

/// `Date.parse(text)` as the TS reads it: NaN where the text is no date.
fn parse_date(text: &str) -> f64 {
    date_parse(text)
}

/// `new Date(ms).toISOString()`.
fn iso_of(ms: i64) -> String {
    iso_string(ms as f64).unwrap_or_default()
}

impl StatusArea {
    pub fn new(ctx: Rc<dyn StatusCtx>) -> Rc<StatusArea> {
        Rc::new(StatusArea { ctx })
    }

    /// S1, status.ts:13: every held entry's row as the TS object, keys in the TS order. A key whose TS value is a
    /// function (`daemonReach`, `daemonAnswers`, `providerState`, `metrics`, `facts`, `exec`) holds `null`: the tracker's
    /// join puts the callback there, so the key's presence and place are what the area decides.
    pub async fn records(&self) -> Result<Vec<Map<String, Value>>, HostError> {
        self.ctx.ready().await;
        let mut rows = Vec::new();
        for entry in self.ctx.held() {
            let record = record_of_entry(&entry);
            let mut row = self.ctx.view(&record)?;
            row.insert("size".to_string(), serde_json::to_value(&record.size).unwrap_or(Value::Null));
            row.insert("rateUsdPerHour".to_string(), json!(self.ctx.rate_usd_per_hour(&record)));
            let (generation, wake_ask, checkout, pr) = {
                let live = entry.borrow();
                (live.generation, live.wake_ask, live.checkout.clone(), live.pr.clone())
            };
            row.insert("generation".to_string(), json!(generation));
            if let Some(reason) = self.ctx.row_reason(&entry) {
                row.insert("reason".to_string(), json!(reason));
            }
            if let Some(ask) = wake_ask {
                row.insert("wakeAsk".to_string(), json!({ "ask": ask.ask, "of": ask.of }));
            }
            if let Some(checkout) = checkout {
                row.insert("checkout".to_string(), serde_json::to_value(checkout).unwrap_or(Value::Null));
            }
            if let Some(pr) = pr {
                row.insert("pr".to_string(), serde_json::to_value(pr).unwrap_or(Value::Null));
            }
            if record.phase == WorkspacePhase::Running {
                if let Some(at) = self.ctx.idle_at(&record.id) {
                    row.insert("idleAt".to_string(), json!(at));
                }
            }
            if let Some(away) = self.ctx.away_line(&record) {
                row.insert("away".to_string(), json!(away));
            }
            if let Some(unreached) = self.ctx.unreached_of(&entry) {
                row.insert("unreached".to_string(), unreached);
            }
            if self.ctx.has_daemon(&entry) {
                row.insert("daemonReach".to_string(), Value::Null);
            }
            let (answers, prompt_ms, metrics, facts) = self.ctx.machine_reads(&entry);
            if answers {
                row.insert("daemonAnswers".to_string(), Value::Null);
            }
            if let Some(ms) = prompt_ms {
                row.insert("daemonAnswersPromptMs".to_string(), json!(ms));
            }
            row.insert("providerState".to_string(), Value::Null);
            if metrics {
                row.insert("metrics".to_string(), Value::Null);
            }
            if facts && self.ctx.reach_of(&entry) != "unreachable" {
                row.insert("facts".to_string(), Value::Null);
            }
            row.insert("exec".to_string(), Value::Null);
            rows.push(row);
        }
        Ok(rows)
    }

    /// S2, status.ts:49. Each status as the TS object (`id`, `machineId`, `phase`, `reach.state`, ...).
    pub fn on_polled(&self, statuses: &[Value]) {
        for status in statuses {
            let id = status["id"].as_str().unwrap_or_default();
            let Some(entry) = self.ctx.live(id) else {
                continue;
            };
            let current = entry.borrow().machine.id().to_string();
            let machine_id = status["machineId"].as_str();
            if machine_id != Some(current.as_str()) {
                continue;
            }
            let reach = status["reach"]["state"].as_str().unwrap_or_default();
            if status["phase"].as_str() == Some("running") {
                self.ctx.polled_reach_set(id, &current, reach);
            } else {
                self.ctx.polled_reach_delete(id);
            }
            if self.ctx.boot_work_has(id) {
                continue;
            }
            self.ctx.revive_daemon(&entry, reach);
            self.ctx.offer_daemon_again(&entry, status);
            self.ctx.read_version_again(&entry, status);
        }
    }

    /// S3, status.ts:62.
    pub fn on_gone(&self, workspace_id: &str, machine_id: &str, reason: &str) {
        let Some(entry) = self.ctx.live(workspace_id) else {
            return;
        };
        let record = record_of_entry(&entry);
        if record.machine_id != machine_id {
            return;
        }
        let ctx = self.ctx.clone();
        let reason = reason.to_string();
        self.ctx.spawn(
            async move {
                if let Err(e) = ctx.adopt_gone(&entry, &reason).await {
                    ctx.warn(&format!("{}'s gone reading was not confirmed: {}", record.id, e.message()));
                }
            }
            .boxed_local(),
        );
    }

    /// S4-S10, status.ts:81. `known` gains every row the engine must spare; `failed` gains the reports.
    pub async fn adopt_lost(
        &self,
        listing: &[ListedRow],
        known: &mut BTreeSet<String>,
        failed: &mut Vec<ReapFailure>,
        place: &str,
    ) -> Result<(Vec<AdoptedMachine>, Vec<ReapedMachine>), HostError> {
        let ctx = &*self.ctx;
        let mut adopted = Vec::new();
        let mut reaped = Vec::new();
        let mut unreached: BTreeSet<String> = BTreeSet::new();
        let now = ctx.wall_now();
        let mut dropped: BTreeSet<String> = BTreeSet::new();
        for d in ctx.store_list(DROPPED).await.map_err(HostError::from)? {
            let machine_id = d["machineId"].as_str().unwrap_or_default().to_string();
            let at = parse_date(d["at"].as_str().unwrap_or_default());
            if (ctx.now() as f64) - at < DROPPED_WATCH_MS as f64 {
                dropped.insert(machine_id);
            } else {
                ctx.store_delete(DROPPED, &machine_id).await.map_err(HostError::from)?;
            }
        }
        let owner = ctx.owner();
        for row in listing {
            if known.contains(&row.id) || (dropped.contains(&row.id) && row.state == "running") || !ctx.lost_workspace(row, &owner, now) {
                continue;
            }
            let stamped: Option<String> = row.labels.get(WORKSPACE_LABEL).cloned();
            let holder: Option<Entry> = stamped.as_deref().and_then(|id| ctx.live(id));
            if let (Some(stamped), None) = (&stamped, &holder) {
                if ctx.store_get(WORKSPACES, stamped).await.map_err(HostError::from)?.is_some() {
                    known.insert(row.id.clone());
                    failed.push(ReapFailure {
                        id: Some(row.id.clone()),
                        message: format!("not recorded: {stamped} is stored here but was not read at start, so its {} machine is left as it is until the host starts again", row.state),
                    });
                    continue;
                }
            }
            if let Some(holder) = &holder {
                let live = holder.borrow();
                if row.state == "running" || live.creating || live.waking.is_some() || live.deleting.is_some() || live.napping.is_some() {
                    continue;
                }
            }
            let kept: Option<Value> = match (&stamped, &holder) {
                (Some(stamped), None) => ctx.store_get(WORKSPACE_NAMES, stamped).await.map_err(HostError::from)?,
                _ => None,
            };
            if let (Some(stamped), None) = (&stamped, &kept) {
                known.insert(row.id.clone());
                unreached.insert(row.id.clone());
                let mut seen_at: Option<String> = ctx
                    .store_get(UNRECORDED, &row.id)
                    .await
                    .map_err(HostError::from)?
                    .and_then(|u| u["seenAt"].as_str().map(str::to_string));
                if seen_at.is_none() {
                    let stamp = iso_of(ctx.now());
                    ctx.store_put(UNRECORDED, &row.id, json!({ "machineId": row.id, "place": place, "seenAt": stamp }))
                        .await
                        .map_err(HostError::from)?;
                    seen_at = Some(stamp);
                }
                let goes_at = parse_date(seen_at.as_deref().unwrap_or_default()) + UNRECORDED_DELETE_MS as f64;
                if (ctx.now() as f64) < goes_at {
                    let whose = match &holder {
                        None => format!("no workspace of this host is {stamped}"),
                        Some(holder) => {
                            let machine_id = holder.borrow().record.borrow().machine_id.clone();
                            if machine_id.is_empty() {
                                format!("{stamped} holds no machine")
                            } else {
                                format!("{stamped} runs on {machine_id}")
                            }
                        }
                    };
                    failed.push(ReapFailure {
                        id: Some(row.id.clone()),
                        message: format!(
                            "not recorded: {whose}, so nothing here reaches this {} machine of yours; deleted at {}, a day after this host first saw it",
                            row.state,
                            iso_string(goes_at).unwrap_or_default()
                        ),
                    });
                    continue;
                }
                match ctx.stop_unrecorded(place, &row.id).await {
                    Ok(()) | Err(ReadFailure::Missing) => {}
                    Err(ReadFailure::Failed(message)) | Err(ReadFailure::StateFailed(message)) => {
                        failed.push(ReapFailure {
                            id: Some(row.id.clone()),
                            message: format!("could not delete: {message}; no record of this host's reaches it, retried next sweep"),
                        });
                        continue;
                    }
                }
                ctx.store_delete(UNRECORDED, &row.id).await.map_err(HostError::from)?;
                let born = row.labels.get(CREATED_AT_LABEL).map(|b| parse_date(b)).filter(|b| !b.is_nan());
                reaped.push(ReapedMachine {
                    id: row.id.clone(),
                    labels: Some(row.labels.clone()),
                    builder: false,
                    reason: ReapReason::Unrecorded,
                    age_ms: born.map(|b| now - b as i64),
                    name: None,
                });
                continue;
            }
            let project = kept.as_ref().and_then(|k| k["project"].as_str().map(str::to_string));
            let (stamped, kept, project) = match (stamped, kept, project) {
                (Some(stamped), Some(kept), Some(project)) if ctx.project_held(&project) => (stamped, kept, project),
                _ => {
                    known.insert(row.id.clone());
                    failed.push(ReapFailure {
                        id: Some(row.id.clone()),
                        message: format!("not recorded: this host holds no project for it, and a workspace is one project's copy; it is a machine of yours still {}", row.state),
                    });
                    continue;
                }
            };
            let confirmed = match ctx.confirm(place, &row.id).await {
                Ok(confirmed) => confirmed,
                Err(ReadFailure::Missing) => continue,
                Err(ReadFailure::Failed(message)) => {
                    known.insert(row.id.clone());
                    failed.push(ReapFailure { id: Some(row.id.clone()), message: format!("not recorded: {message}; retried next sweep") });
                    continue;
                }
                Err(ReadFailure::StateFailed(message)) => return Err(HostError::new(message)),
            };
            if confirmed.state != "running" && confirmed.state != "paused" {
                known.insert(row.id.clone());
                continue;
            }
            let kept_name = kept["name"].as_str().unwrap_or_default().to_string();
            let name = if ctx.name_refusal(&kept_name).is_none() { kept_name } else { row.id.clone() };
            let created_at = match row.labels.get(CREATED_AT_LABEL) {
                Some(born) if !parse_date(born).is_nan() => born.clone(),
                _ => iso_of(ctx.wall_now()),
            };
            let golden = match row.labels.get(GOLDEN_LABEL) {
                Some(golden) => golden.clone(),
                None => ctx.golden_head().await.unwrap_or_default(),
            };
            let phase = if confirmed.state == "paused" { "napping" } else { "running" };
            let mut doc = Map::new();
            doc.insert("id".to_string(), json!(stamped));
            doc.insert("name".to_string(), json!(name));
            doc.insert("kind".to_string(), json!("cloud"));
            doc.insert("project".to_string(), json!(project));
            if place != ctx.places_wired() {
                doc.insert("place".to_string(), json!(place));
            }
            doc.insert("machineId".to_string(), json!(row.id));
            doc.insert("phase".to_string(), json!(phase));
            doc.insert("golden".to_string(), json!(golden));
            doc.insert("createdAt".to_string(), json!(created_at));
            doc.insert("spec".to_string(), json!({ "labels": row.labels }));
            doc.insert("size".to_string(), confirmed.size.clone());
            doc.insert("firstLife".to_string(), json!(false));
            if let Some(url) = &confirmed.stream_url {
                doc.insert("screen".to_string(), json!({ "streamUrl": url }));
            }
            let record: WorkspaceRecord = serde_json::from_value(Value::Object(doc)).map_err(|e| HostError::new(e.to_string()))?;
            let entry = ctx.attach(record.clone(), confirmed.machine.clone())?;
            ctx.persist(&record).await.map_err(HostError::from)?;
            if phase == "running" {
                ctx.sync_daemon(&entry);
            }
            ctx.emit(json!({ "type": "workspace.created", "workspace": Value::Object(ctx.view(&record)?) }));
            let reach = if phase == "running" { ctx.reach_of(&entry) } else { "napping".to_string() };
            ctx.emit_status(&entry, &reach, RECORD_RESTORED).await;
            adopted.push(AdoptedMachine { id: row.id.clone(), workspace_id: stamped, name: record.name.clone(), phase: record.phase });
        }
        for u in ctx.store_list(UNRECORDED).await.map_err(HostError::from)? {
            let machine_id = u["machineId"].as_str().unwrap_or_default().to_string();
            if u["place"].as_str() == Some(place) && !unreached.contains(&machine_id) {
                ctx.store_delete(UNRECORDED, &machine_id).await.map_err(HostError::from)?;
            }
        }
        Ok((adopted, reaped))
    }
}
