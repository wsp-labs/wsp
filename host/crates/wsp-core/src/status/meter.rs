// SPDX-License-Identifier: AGPL-3.0-only
//! The awake meter and the cost series of packages/runtime/src/status.ts at 65f6b89c7. Spec: spec/status_meter.md.
use std::collections::HashSet;

use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::place_more::{PlaceSpend, PlaceView};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::views::workspace_events::WorkspaceCostEvent;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_words::cost_history::{accrued_past, append_cost_point, day_start, month_start, spent_since};
use wsp_words::date_parse::date_parse;
use wsp_words::js::number_string;
use wsp_words::place_state::{place_spend_limit, workspace_place_id};

use super::{iso, CostHistoryRecord, CostPoint, Meter, StatusTracker, Stood, TrackerState, COST_HISTORIES};
use crate::console::warn;

// The meter's listeners, the load, the writes, the series' end and the cost tick, as the TS has them. Private helpers
// for each go in this file.
//
// /** The total at a tick: the last tick's total plus the awake time since it at the rate that tick carried. A tick
//  * lands at every event that opens or re-prices a stretch (a create, a wake, a size change), so that rate is the one
//  * that held until this tick; only the first tick ever has none behind it, and its stretch ran at the size now. */
// function accrue(last: WorkspaceCostEvent | undefined, awakeMs: number, rateNow: number): number {
//   return accruedPast({ accruedUsd: last?.accruedUsd ?? 0, rateUsdPerHour: last?.rateUsdPerHour ?? rateNow }, awakeMs - (last?.awakeMs ?? 0));
// }
//
//   // Exact awake accounting comes from lifecycle events, not poll edges. A
//   // workspace hydrated already-running starts its meter lazily at first tick.
//   const meter = (id: string): Meter => {
//     let m = meters.get(id);
//     if (!m) {
//       m = { awakeMs: 0 };
//       meters.set(id, m);
//     }
//     return m;
//   };
//   /** Something proved the machine awake just now: a create or a wake here, the guest answering the probe, or the
//    * provider saying running. The record's own phase is a belief, and a belief never moves this. */
//   const sawAwake = (id: string): void => {
//     meter(id).awakeUntil = clock.now();
//   };
//   const beganAwake = (id: string): void => {
//     meter(id).mark = clock.now();
//     sawAwake(id);
//   };
//   o.on("workspace.created", e => {
//     if (e.type === "workspace.created") beganAwake(e.workspace.id);
//   });
//   o.on("workspace.woken", e => {
//     if (e.type !== "workspace.woken") return;
//     const reopened = closedByGone.get(e.workspaceId);
//     if (reopened === undefined) return beganAwake(e.workspaceId);
//     // A record that leaves gone was never paused: the machine went on running and billing through the gap, so the
//     // stretch the verdict closed reopens where it began rather than a fresh one starting now.
//     closedByGone.delete(e.workspaceId);
//     const m = meter(e.workspaceId);
//     m.mark = reopened.mark;
//     m.awakeMs = reopened.awakeMs;
//     sawAwake(e.workspaceId);
//   });
//   // A rebuild put another machine under the record: the stretch the verdict closed was the old one's.
//   o.on("workspace.upgraded", e => {
//     if (e.type === "workspace.upgraded") closedByGone.delete(e.workspaceId);
//   });
//   // Awake time ends where the machine did: a pause this host made ends it now; a pause the provider made and this
//   // host only found, and the moment the provider was found not to know the machine at all, end it at the last
//   // instant anything proved the machine awake, so the hours before we noticed bill nothing.
//   for (const type of ["workspace.napped", "workspace.gone"] as const) {
//     o.on(type, e => {
//       if (e.type !== type) return;
//       const m = meter(e.workspaceId);
//       const found = e.type === "workspace.gone" || e.found === true;
//       const ended = found ? (m.awakeUntil ?? clock.now()) : clock.now();
//       // A machine found paused stopped billing somewhere in the gap and nothing says when, so that stretch stays
//       // closed at the last proof; only a gone verdict is one a later read can unmake.
//       if (e.type === "workspace.napped") closedByGone.delete(e.workspaceId);
//       else if (m.mark !== undefined) closedByGone.set(e.workspaceId, { mark: m.mark, awakeMs: m.awakeMs });
//       if (m.mark !== undefined) m.awakeMs += Math.max(0, ended - m.mark);
//       m.mark = undefined;
//     });
//   }
//   o.on("workspace.deleted", e => {
//     if (e.type !== "workspace.deleted") return;
//     closeSeries(e.workspaceId);
//     meters.delete(e.workspaceId);
//     closedByGone.delete(e.workspaceId);
//     forget(e.workspaceId);
//     lastEmitted.delete(e.workspaceId);
//     sentFigures.delete(e.workspaceId);
//     reconciled.delete(e.workspaceId);
//     goneReasons.delete(e.workspaceId);
//     suspects.delete(e.workspaceId);
//     for (const book of probes.values()) book.delete(e.workspaceId);
//     doubted.delete(e.workspaceId);
//     factsDials.delete(e.workspaceId);
//   });
//
//   // The stored series is as of the tick that last added a point, and a run of one rate is a straight line from
//   // there: a workspace running then resumes its awake stretch at that tick, so the host's downtime is metered as
//   // the provider billed it.
//   const loading = (async () => {
//     const month = monthStart(clock.now());
//     for (const raw of await o.store.list(COST_HISTORIES)) {
//       const doc = raw as CostHistoryRecord;
//       const last = Array.isArray(doc.points) ? doc.points.at(-1) : undefined;
//       if (last === undefined) continue;
//       if (doc.ended === true && Date.parse(last.at) < month) {
//         // A document that will not go is a document that is only read again next month; losing the load with it
//         // would cost every live workspace its meter.
//         await o.store.delete(COST_HISTORIES, doc.workspaceId).catch((e: unknown) => console.warn(`the cost history of the deleted workspace ${doc.workspaceId} could not be dropped`, e));
//         continue;
//       }
//       histories.set(doc.workspaceId, doc.points);
//       if (doc.where !== undefined) stood.set(doc.workspaceId, doc.where);
//       // A deleted workspace's meter is nobody's to carry on: nothing polls it, and a mark on it would bill a
//       // machine that no longer exists if a record ever came back wearing its id.
//       if (doc.ended === true) {
//         ended.add(doc.workspaceId);
//         continue;
//       }
//       const m = meter(doc.workspaceId);
//       m.awakeMs = last.awakeMs;
//       if (last.phase === "running") m.mark = m.awakeUntil = Date.parse(last.at);
//     }
//   })().catch((e: unknown) => console.warn("cost histories not loaded; the series begins at the first tick", e));
//
//   // Writes land in order behind the load, so a save never lands under a delete that followed it.
//   let writes: Promise<void> = loading;
//   const persist = (step: () => Promise<void>): void => {
//     writes = writes.then(step).catch(() => {});
//   };
//
//   /** The document of a workspace as it stands now, which is the series in memory rather than the one on disk: only
//    * a tick that adds a point writes, so a stretch at one rate lives in memory alone until its rate changes. */
//   const document = (id: string): CostHistoryRecord => {
//     const where = stood.get(id);
//     return { workspaceId: id, points: histories.get(id) ?? [], ...(where !== undefined ? { where } : {}), ...(ended.has(id) ? { ended: true } : {}) };
//   };
//
//   /** A deleted workspace's series ends on a tick at the instant it went, closing the stretch it was in: nothing ticks
//    * it again, and a series read later stops at its newest tick. */
//   const closeSeries = (id: string): void => {
//     const before = histories.get(id);
//     const last = before?.at(-1);
//     const m = meters.get(id);
//     if (before === undefined || last === undefined || m === undefined) return;
//     const now = clock.now();
//     const awakeMs = m.awakeMs + (m.mark !== undefined ? Math.max(0, now - m.mark) : 0);
//     histories.set(id, appendCostPoint(before, { ...last, phase: "gone", rateUsdPerHour: 0, awakeMs, accruedUsd: accrue(last, awakeMs, last.rateUsdPerHour), at: new Date(now).toISOString() }));
//   };
//
//   /** A deleted workspace's meter is over, and what it spent this month is not: the series is written where it
//    * stands and marked, so the place it stood on still totals the hours it ran before it went. */
//   const forget = (id: string): void => {
//     if (!histories.has(id)) {
//       stood.delete(id);
//       persist(() => o.store.delete(COST_HISTORIES, id));
//       return;
//     }
//     ended.add(id);
//     persist(() => o.store.put(COST_HISTORIES, id, document(id)));
//   };
//
//   /** The timer's tick samples every workspace and rides the bus, once only for one nobody bills. An event's tick is
//    * one workspace's: it pins a boundary in the meter and rides the bus only when it changed the rate, so a create, a
//    * wake and a size change report at once and a rebuild at the same rate reports nothing. */
//   const costTick = async (only?: string): Promise<void> => {
//     await loading;
//     const now = clock.now();
//     for (const view of await o.records()) {
//       if (only !== undefined && view.id !== only) continue;
//       const m = meter(view.id);
//       if (view.phase === "running") m.mark ??= now;
//       // A gone record's stretch is over and its end unknown (the host may have been down when the provider lost the
//       // machine): the mark goes without folding, so no tick after a rebuild bills the gap.
//       else if (view.phase === "gone" || view.phase === "released") m.mark = undefined;
//       // A pausing machine runs until the provider has paused it, and only workspace.napped closes its stretch.
//       const running = (view.phase === "running" || view.phase === "pausing") && m.mark !== undefined;
//       const rate = view.rateUsdPerHour;
//       const awakeMs = m.awakeMs + (running ? now - m.mark! : 0);
//       const before = histories.get(view.id) ?? [];
//       const last = before.at(-1);
//       const tick: WorkspaceCostEvent = {
//         type: "workspace.cost",
//         workspaceId: view.id,
//         phase: view.phase,
//         rateUsdPerHour: running ? rate : 0,
//         awakeMs,
//         accruedUsd: accrue(last, awakeMs, rate),
//         at: new Date(now).toISOString(),
//       };
//       const next = appendCostPoint(before, tick);
//       histories.set(view.id, next);
//       stood.set(view.id, { kind: view.kind, machineId: view.machineId, ...(view.place !== undefined ? { place: view.place } : {}), ...(view.provider !== undefined ? { provider: view.provider } : {}) });
//       // A tick that replaced the newest point lies on the stored line; only an added point changes the document.
//       const added = before.length === 0 || next[next.length - 2] === before[before.length - 1];
//       if (added) persist(() => o.store.put(COST_HISTORIES, view.id, document(view.id)));
//       // A workspace nobody bills says its zeros once: 125 folders on this computer ticking sent 25 frames a second.
//       // A repeat reaches every listener but stays out of the replay list, which 14 repeats a second filled in 6 minutes.
//       const figures = figuresOf(tick);
//       const unbilled = tick.rateUsdPerHour === 0 && tick.accruedUsd === 0;
//       const moved = sentFigures.get(view.id) !== figures;
//       const due = only === undefined ? !unbilled || moved : last?.rateUsdPerHour !== tick.rateUsdPerHour;
//       if (due) {
//         sentFigures.set(view.id, figures);
//         if (moved) o.emit(tick);
//         else o.pass(tick);
//       }
//     }
//   };
//
//   // A tick at every event that opens, re-prices or ends a stretch: the rate a tick carries then holds until the next
//   // one, so a size change an hour after an unwatched wake still bills that hour at the size it woke at, and a machine
//   // napped or found gone reads rate 0 from that instant rather than from the timer's next tick.
//   const eventTick = (id: string): void => guarded("cost", () => costTick(id))();
//   o.on("workspace.created", e => {
//     if (e.type === "workspace.created") eventTick(e.workspace.id);
//   });
//   for (const type of ["workspace.woken", "workspace.upgraded", "workspace.napped", "workspace.gone"] as const) {
//     o.on(type, e => {
//       if (e.type === type) eventTick(e.workspaceId);
//     });
//   }

impl StatusTracker {
    //   // A deleted workspace's series is held for the month's total and for nothing else: it has no pane to draw in and
    //   // no record to read its caller's right to it off, so this read answers for the workspaces that are still here.
    //   const history: StatusApi["history"] = async id => {
    //     await loading;
    //     return ended.has(id) ? [] : (histories.get(id) ?? []);
    //   };
    /// The workspace's cost ticks since metering began, across host restarts; `[]` for a deleted one.
    pub async fn history(&self, workspace_id: &str) -> Vec<CostPoint> {
        self.loaded().await;
        self.st(|s| {
            if s.ended.contains(workspace_id) {
                return Vec::new();
            }
            s.histories.get(workspace_id).map_or_else(Vec::new, |points| points.iter().cloned().map(CostPoint::from).collect())
        })
    }

    //   const spend: StatusApi["spend"] = async (places, at = clock.now()) => {
    //     await loading;
    //     const day = dayStart(at);
    //     const month = monthStart(at);
    //     const zero = (place: string): PlaceSpend => ({ place, todayUsd: 0, monthUsd: 0, rateUsdPerHour: 0 });
    //     const running = new Set((await o.records()).filter(r => r.phase === "running" || r.phase === "pausing").map(r => r.id));
    //     const rows = new Map(places.filter(place => placeSpendLimit(place) !== undefined).map(place => [place.id, zero(place.id)]));
    //     for (const [id, points] of histories) {
    //       const where = stood.get(id);
    //       const place = where === undefined ? undefined : workspacePlaceId(where, places);
    //       if (place === undefined) continue;
    //       const row = rows.get(place) ?? zero(place);
    //       // Only a machine whose record says running or pausing goes on past its newest tick: a deleted one's series ends
    //       // where it went, and a napping record whose stored newest tick still runs (the host died before the nap's tick
    //       // was written) reads nothing past that tick.
    //       const until = running.has(id) ? at : undefined;
    //       row.todayUsd += spentSince(points, day, until);
    //       row.monthUsd += spentSince(points, month, until);
    //       if (until !== undefined) row.rateUsdPerHour += points[points.length - 1]?.rateUsdPerHour ?? 0;
    //       rows.set(place, row);
    //     }
    //     return places.flatMap(place => {
    //       const row = rows.get(place.id);
    //       return row === undefined ? [] : [row];
    //     });
    //   };
    /// What each place has cost since midnight and the first of the month, and burns now; `at` defaults to now.
    pub async fn spend(&self, places: &[PlaceView], at: Option<i64>) -> Result<Vec<PlaceSpend>, String> {
        let at = at.unwrap_or_else(|| self.clock.now());
        self.loaded().await;
        let day = day_start(at as f64, &self.zone);
        let month = month_start(at as f64, &self.zone);
        let running: HashSet<String> = self
            .records()
            .await?
            .into_iter()
            .filter(|r| matches!(r.view.phase, WorkspacePhase::Running | WorkspacePhase::Pausing))
            .map(|r| r.view.id)
            .collect();
        Ok(self.st(|s| {
            let zero = |place: &str| PlaceSpend {
                place: place.to_string(),
                today_usd: JsNumber(0.0),
                month_usd: JsNumber(0.0),
                rate_usd_per_hour: JsNumber(0.0),
            };
            let mut rows: IndexMap<String, PlaceSpend> =
                places.iter().filter(|p| place_spend_limit(p).is_some()).map(|p| (p.id.clone(), zero(&p.id))).collect();
            for (id, points) in &s.histories {
                let Some(place) = s.stood.get(id).and_then(|w| stood_place_id(w, places)) else { continue };
                let until = running.contains(id).then_some(at as f64);
                let row = rows.entry(place.clone()).or_insert_with(|| zero(&place));
                row.today_usd.0 += spent_since(points, day, until);
                row.month_usd.0 += spent_since(points, month, until);
                if until.is_some() {
                    row.rate_usd_per_hour.0 += points.last().map_or(0.0, |p| p.rate_usd_per_hour.0);
                }
            }
            places.iter().filter_map(|p| rows.get(&p.id).cloned()).collect()
        }))
    }
}

/// The meter of a workspace, made at zero on first use.
pub(super) fn meter_of<'a>(s: &'a mut TrackerState, id: &str) -> &'a mut Meter {
    s.meters.entry(id.to_string()).or_default()
}

/// Where a workspace's record would stand, read through `workspace_place_id`, which takes the whole view: the four
/// fields it reads are the stored `Stood`, and the other fields are left empty.
fn stood_place_id(w: &Stood, places: &[PlaceView]) -> Option<String> {
    let mut view = json!({
        "id": "", "name": "", "machineId": w.machine_id, "phase": "gone", "golden": "", "createdAt": "",
        "project": {"id": "", "name": "", "path": "", "computer": ""},
    });
    let fields = view.as_object_mut()?;
    if let Some(kind) = &w.kind {
        fields.insert("kind".into(), serde_json::to_value(kind).ok()?);
    }
    if let Some(place) = &w.place {
        fields.insert("place".into(), json!(place));
    }
    if let Some(provider) = &w.provider {
        fields.insert("provider".into(), json!(provider));
    }
    let view: WorkspaceView = serde_json::from_value(view).ok()?;
    workspace_place_id(&view, places)
}

/// `accrue(last, awakeMs, rateNow)`: the total at a tick, the last tick's total plus the awake time since it.
fn accrue(last: Option<&WorkspaceCostEvent>, awake_ms: f64, rate_now: f64) -> f64 {
    let base = WorkspaceCostEvent {
        workspace_id: String::new(),
        phase: WorkspacePhase::Running,
        rate_usd_per_hour: JsNumber(last.map_or(rate_now, |l| l.rate_usd_per_hour.0)),
        awake_ms: JsNumber(0.0),
        accrued_usd: JsNumber(last.map_or(0.0, |l| l.accrued_usd.0)),
        at: String::new(),
    };
    accrued_past(&base, awake_ms - last.map_or(0.0, |l| l.awake_ms.0))
}

/// `figuresOf(tick)`: the phase, rate and total a cost frame carries, the words a repeat is judged by.
fn figures_of(tick: &WorkspaceCostEvent) -> String {
    let phase = serde_json::to_value(tick.phase).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    format!("{} {} {}", phase, number_string(tick.rate_usd_per_hour.0), number_string(tick.accrued_usd.0))
}

/// Whether a tick continues the rate of the two points before it, so it replaces the newest instead of adding one.
fn collinear(points: &[WorkspaceCostEvent], tick: &WorkspaceCostEvent) -> bool {
    let n = points.len();
    n >= 2
        && points[n - 2].rate_usd_per_hour.0 == points[n - 1].rate_usd_per_hour.0
        && points[n - 1].rate_usd_per_hour.0 == tick.rate_usd_per_hour.0
}

/// A write of the document chain, built when it runs.
enum Persist {
    Put(String),
    Delete(String),
}

impl StatusTracker {
    /// `o.on("workspace.created")`: a create opens a stretch.
    pub(super) fn meter_created(&self, e: &EventUnion) {
        if let EventUnion::WorkspaceCreated(c) = e {
            self.began_awake(&c.event.workspace.id);
        }
    }

    /// `beganAwake(id)`: a stretch starts now, and proves the machine awake.
    fn began_awake(&self, id: &str) {
        let now = self.clock.now();
        self.st(|s| {
            let m = meter_of(s, id);
            m.mark = Some(now);
            m.awake_until = Some(now);
        });
    }

    /// `o.on("workspace.woken")`: a stretch a gone verdict closed reopens where it began; otherwise a new one starts.
    pub(super) fn meter_woken(&self, e: &EventUnion) {
        let EventUnion::WorkspaceWoken(w) = e else { return };
        let id = w.event.workspace_id.clone();
        let now = self.clock.now();
        let reopened = self.st(|s| s.closed_by_gone.shift_remove(&id));
        match reopened {
            None => self.began_awake(&id),
            Some((mark, awake_ms)) => self.st(|s| {
                let m = meter_of(s, &id);
                m.mark = Some(mark);
                m.awake_ms = awake_ms;
                m.awake_until = Some(now);
            }),
        }
    }

    /// `o.on("workspace.upgraded")`: a rebuild put another machine under the record.
    pub(super) fn meter_upgraded(&self, e: &EventUnion) {
        if let EventUnion::WorkspaceUpgraded(u) = e {
            self.st(|s| {
                s.closed_by_gone.shift_remove(&u.event.workspace_id);
            });
        }
    }

    /// `o.on("workspace.napped")` and `o.on("workspace.gone")`: the awake stretch ends where the machine did.
    pub(super) fn meter_stopped(&self, e: &EventUnion) {
        let (id, found, napped) = match e {
            EventUnion::WorkspaceNapped(n) => (n.event.workspace_id.clone(), n.event.found == Some(true), true),
            EventUnion::WorkspaceGone(g) => (g.event.workspace_id.clone(), true, false),
            _ => return,
        };
        let now = self.clock.now();
        self.st(|s| {
            let m = meter_of(s, &id).clone();
            let ended = if found { m.awake_until.unwrap_or(now) } else { now };
            if napped {
                s.closed_by_gone.shift_remove(&id);
            } else if let Some(mark) = m.mark {
                s.closed_by_gone.insert(id.clone(), (mark, m.awake_ms));
            }
            let m = meter_of(s, &id);
            if let Some(mark) = m.mark {
                m.awake_ms += (ended - mark).max(0) as f64;
            }
            m.mark = None;
        });
    }

    /// `o.on("workspace.deleted")`: the series ends, and every per-workspace entry the tracker holds is dropped.
    pub(super) fn drop_workspace(&self, e: &EventUnion) {
        let EventUnion::WorkspaceDeleted(d) = e else { return };
        let id = d.event.workspace_id.clone();
        self.close_series(&id);
        self.st(|s| {
            s.meters.shift_remove(&id);
            s.closed_by_gone.shift_remove(&id);
        });
        self.forget(&id);
        self.st(|s| {
            s.last_emitted.shift_remove(&id);
            s.sent_figures.shift_remove(&id);
            s.reconciled.shift_remove(&id);
            s.gone_reasons.shift_remove(&id);
            s.suspects.shift_remove(&id);
            for book in s.probes.values_mut() {
                book.shift_remove(&id);
            }
            s.doubted.shift_remove(&id);
            s.facts_dials.shift_remove(&id);
        });
    }

    /// `closeSeries(id)`: a deleted workspace's series gets a closing point at the instant it went.
    fn close_series(&self, id: &str) {
        let now = self.clock.now();
        self.st(|s| {
            let (Some(before), Some(m)) = (s.histories.get(id).cloned(), s.meters.get(id).cloned()) else { return };
            let Some(last) = before.last().cloned() else { return };
            let awake_ms = m.awake_ms + m.mark.map_or(0.0, |mark| (now - mark).max(0) as f64);
            let tick = WorkspaceCostEvent {
                workspace_id: last.workspace_id.clone(),
                phase: WorkspacePhase::Gone,
                rate_usd_per_hour: JsNumber(0.0),
                awake_ms: JsNumber(awake_ms),
                accrued_usd: JsNumber(accrue(Some(&last), awake_ms, last.rate_usd_per_hour.0)),
                at: iso(now),
            };
            s.histories.insert(id.to_string(), append_cost_point(&before, &tick));
        });
    }

    /// `forgetSeries(id)`: a workspace with no series loses its document; one with a series is marked ended.
    fn forget(&self, id: &str) {
        let write = self.st(|s| {
            if s.histories.contains_key(id) {
                s.ended.insert(id.to_string());
                Persist::Put(id.to_string())
            } else {
                s.stood.shift_remove(id);
                Persist::Delete(id.to_string())
            }
        });
        self.persist(write);
    }

    /// Queues one document write behind the ones before it. Failures are swallowed.
    fn persist(&self, step: Persist) {
        let prev = self.writes.borrow().clone();
        let me = self.me.clone();
        let write: Shared<LocalBoxFuture<'static, ()>> = (async move {
            if let Some(prev) = prev {
                prev.await;
            }
            let Some(t) = me.upgrade() else { return };
            match step {
                Persist::Put(id) => {
                    let doc = t.document(&id);
                    if let Ok(value) = serde_json::to_value(&doc) {
                        let _ = t.options.store.put(COST_HISTORIES, &id, value).await;
                    }
                }
                Persist::Delete(id) => {
                    let _ = t.options.store.delete(COST_HISTORIES, &id).await;
                }
            }
        })
        .boxed_local()
        .shared();
        *self.writes.borrow_mut() = Some(write.clone());
        tokio::task::spawn_local(write);
    }

    /// The document of a workspace as it stands in memory now.
    fn document(&self, id: &str) -> CostHistoryRecord {
        self.st(|s| CostHistoryRecord {
            workspace_id: id.to_string(),
            points: s.histories.get(id).map_or_else(Vec::new, |points| points.iter().cloned().map(CostPoint::from).collect()),
            where_: s.stood.get(id).cloned(),
            ended: s.ended.contains(id).then_some(true),
        })
    }

    /// The load of the stored series (status_meter Behaviour 7).
    pub(super) async fn load_stored(&self) {
        let month = month_start(self.clock.now() as f64, &self.zone);
        let docs = match self.options.store.list(COST_HISTORIES).await {
            Ok(docs) => docs,
            Err(e) => {
                warn(&format!("cost histories not loaded; the series begins at the first tick {}", e.message));
                return;
            }
        };
        for raw in docs {
            let Some(last_raw) = raw.get("points").and_then(Value::as_array).and_then(|points| points.last()).cloned() else { continue };
            let Ok(last) = serde_json::from_value::<WorkspaceCostEvent>(last_raw) else { continue };
            let Ok(doc) = serde_json::from_value::<CostHistoryRecord>(raw) else { continue };
            if doc.ended == Some(true) && date_parse(&last.at) < month {
                if let Err(e) = self.options.store.delete(COST_HISTORIES, &doc.workspace_id).await {
                    warn(&format!("the cost history of the deleted workspace {} could not be dropped {}", doc.workspace_id, e.message));
                }
                continue;
            }
            self.st(|s| {
                let id = doc.workspace_id.clone();
                s.histories.insert(id.clone(), doc.points.iter().map(|p| p.event.clone()).collect());
                if let Some(where_) = &doc.where_ {
                    s.stood.insert(id.clone(), where_.clone());
                }
                if doc.ended == Some(true) {
                    s.ended.insert(id);
                    return;
                }
                let m = meter_of(s, &id);
                m.awake_ms = last.awake_ms.0;
                if last.phase == WorkspacePhase::Running {
                    let at = date_parse(&last.at) as i64;
                    m.mark = Some(at);
                    m.awake_until = Some(at);
                }
            });
        }
    }

    /// `costTick(only)` (status_meter Behaviour 10 and 11): one point per workspace, and the bus told of it when its
    /// figures moved, or passed quietly when they did not.
    pub(super) async fn cost_tick(&self, only: Option<&str>) -> Result<(), String> {
        self.loaded().await;
        let now = self.clock.now();
        for r in self.records().await? {
            let id = r.view.id.clone();
            if only.is_some_and(|only| only != id) {
                continue;
            }
            let (tick, added, last_rate) = self.st(|s| {
                let m = meter_of(s, &id);
                if r.view.phase == WorkspacePhase::Running && m.mark.is_none() {
                    m.mark = Some(now);
                } else if matches!(r.view.phase, WorkspacePhase::Gone | WorkspacePhase::Released) {
                    m.mark = None;
                }
                let running = matches!(r.view.phase, WorkspacePhase::Running | WorkspacePhase::Pausing) && m.mark.is_some();
                let awake_ms = match (running, m.mark) {
                    (true, Some(mark)) => m.awake_ms + (now - mark) as f64,
                    _ => m.awake_ms,
                };
                let rate = r.rate_usd_per_hour;
                let before = s.histories.get(&id).cloned().unwrap_or_default();
                let last = before.last().cloned();
                let tick = WorkspaceCostEvent {
                    workspace_id: id.clone(),
                    phase: r.view.phase,
                    rate_usd_per_hour: JsNumber(if running { rate } else { 0.0 }),
                    awake_ms: JsNumber(awake_ms),
                    accrued_usd: JsNumber(accrue(last.as_ref(), awake_ms, rate)),
                    at: iso(now),
                };
                let next = append_cost_point(&before, &tick);
                let added = before.is_empty() || !collinear(&before, &tick);
                s.histories.insert(id.clone(), next);
                s.stood.insert(
                    id.clone(),
                    Stood {
                        kind: r.view.kind,
                        machine_id: r.view.machine_id.clone(),
                        place: r.view.place.clone(),
                        provider: r.view.provider.clone(),
                    },
                );
                (tick, added, last.map(|l| l.rate_usd_per_hour.0))
            });
            if added {
                self.persist(Persist::Put(id.clone()));
            }
            let figures = figures_of(&tick);
            let unbilled = tick.rate_usd_per_hour.0 == 0.0 && tick.accrued_usd.0 == 0.0;
            let (due, moved) = self.st(|s| {
                let moved = s.sent_figures.get(&id) != Some(&figures);
                let due = if only.is_none() { !unbilled || moved } else { last_rate != Some(tick.rate_usd_per_hour.0) };
                if due {
                    s.sent_figures.insert(id.clone(), figures);
                }
                (due, moved)
            });
            if due {
                let event = EventUnion::WorkspaceCost(Sequenced { event: tick, seq: None });
                if moved {
                    (self.options.emit)(event);
                } else {
                    (self.options.pass)(event);
                }
            }
        }
        Ok(())
    }

    /// The bus listeners that tick the cost of one workspace at the event that opens or re-prices its stretch.
    pub(super) fn tick_created(&self, e: &EventUnion) {
        if let EventUnion::WorkspaceCreated(c) = e {
            self.guard_cost(Some(c.event.workspace.id.clone()));
        }
    }

    pub(super) fn tick_moved(&self, e: &EventUnion) {
        let id = match e {
            EventUnion::WorkspaceWoken(w) => w.event.workspace_id.clone(),
            EventUnion::WorkspaceUpgraded(u) => u.event.workspace_id.clone(),
            EventUnion::WorkspaceNapped(n) => n.event.workspace_id.clone(),
            EventUnion::WorkspaceGone(g) => g.event.workspace_id.clone(),
            _ => return,
        };
        self.guard_cost(Some(id));
    }
}
