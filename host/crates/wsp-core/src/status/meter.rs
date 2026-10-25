// SPDX-License-Identifier: AGPL-3.0-only
//! The awake meter and the cost series of packages/runtime/src/status.ts at 65f6b89c7. Spec: spec/status_meter.md.
use wsp_wire::views::place_more::{PlaceSpend, PlaceView};
use wsp_wire::views::workspace_events::WorkspaceCostEvent;

use super::StatusTracker;

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
    pub async fn history(&self, workspace_id: &str) -> Vec<WorkspaceCostEvent> {
        todo!()
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
        todo!()
    }
}
