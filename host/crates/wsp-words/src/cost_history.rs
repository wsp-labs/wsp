// SPDX-License-Identifier: AGPL-3.0-only
//! A workspace's cost history: the ticks where its burn rate changed, folded and read by one rule. Port of
//! packages/protocol/src/cost-history.ts at a3b870a6e. See spec/costs_and_alerts.md.

use wsp_wire::types::cost_history::COST_HISTORY_CAP;
use wsp_wire::views::workspace_events::WorkspaceCostEvent;

// cost-history.ts:9
// in wsp_wire::types::cost_history: COST_HISTORY_CAP

/// The series with `tick` appended: a tick that continues the rate of the two before it replaces the newest, so the
/// series holds the first tick, the last tick of each rate, the first tick of the next and the newest. Equal rates
/// stand for one straight run because the accrued total grows at the tick's rate for as long as that rate holds.
// cost-history.ts:14
// export function appendCostPoint(points: readonly WorkspaceCostEvent[], tick: WorkspaceCostEvent): WorkspaceCostEvent[] {
//   const last = points[points.length - 1];
//   const prev = points[points.length - 2];
//   const collinear = last !== undefined && prev !== undefined && prev.rateUsdPerHour === last.rateUsdPerHour && last.rateUsdPerHour === tick.rateUsdPerHour;
//   const next = collinear ? [...points.slice(0, -1), tick] : [...points, tick];
//   return next.length > COST_HISTORY_CAP ? next.slice(next.length - COST_HISTORY_CAP) : next;
// }
pub fn append_cost_point(points: &[WorkspaceCostEvent], tick: &WorkspaceCostEvent) -> Vec<WorkspaceCostEvent> {
    let n = points.len();
    let collinear = n >= 2
        && points[n - 2].rate_usd_per_hour.0 == points[n - 1].rate_usd_per_hour.0
        && points[n - 1].rate_usd_per_hour.0 == tick.rate_usd_per_hour.0;
    let mut next: Vec<WorkspaceCostEvent> = if collinear { points[..n - 1].to_vec() } else { points.to_vec() };
    next.push(tick.clone());
    let cap = COST_HISTORY_CAP as usize;
    if next.len() > cap {
        next.drain(..next.len() - cap);
    }
    next
}

// The tick at or before t and the one after it, when t falls inside the series.
// cost-history.ts:23
// private in the TS: port it as a private helper
// function around(points: readonly WorkspaceCostEvent[], t: number): { before: WorkspaceCostEvent; after: WorkspaceCostEvent | undefined } | null {
//   const first = points[0];
//   if (first === undefined || t < Date.parse(first.at)) return null;
//   let lo = 0;
//   let hi = points.length - 1;
//   while (lo < hi) {
//     const mid = (lo + hi + 1) >> 1;
//     if (Date.parse(points[mid]!.at) <= t) lo = mid;
//     else hi = mid - 1;
//   }
//   return { before: points[lo]!, after: points[lo + 1] };
// }

/// The total `ms` of awake time past a tick adds to it, at the rate that tick carried: the rate a tick carries holds
/// until the next one, so the meter's next tick and a read between ticks both take the total this way.
// cost-history.ts:38
// export function accruedPast(tick: Pick<WorkspaceCostEvent, "accruedUsd" | "rateUsdPerHour">, ms: number): number {
//   return tick.accruedUsd + (tick.rateUsdPerHour * ms) / 3_600_000;
// }
pub fn accrued_past(tick: &WorkspaceCostEvent, ms: f64) -> f64 {
    tick.accrued_usd.0 + (tick.rate_usd_per_hour.0 * ms) / 3_600_000.0
}

/// The total accrued at t: linear between the ticks around it, flat past the newest, unknown before the first.
// cost-history.ts:43
// export function accruedAt(points: readonly WorkspaceCostEvent[], t: number): number | null {
//   const near = around(points, t);
//   if (near === null) return null;
//   const { before, after } = near;
//   if (after === undefined) return before.accruedUsd;
//   const t0 = Date.parse(before.at);
//   const t1 = Date.parse(after.at);
//   if (t1 <= t0) return after.accruedUsd;
//   return before.accruedUsd + ((after.accruedUsd - before.accruedUsd) * (t - t0)) / (t1 - t0);
// }
/// None is the TS null. `at` is read with `Date.parse` of an ISO stamp.
pub fn accrued_at(points: &[WorkspaceCostEvent], t: f64) -> Option<f64> {
    let (before, after) = around(points, t)?;
    let before = &points[before];
    let Some(after) = after.map(|index| &points[index]) else {
        return Some(before.accrued_usd.0);
    };
    let t0 = parse_at(&before.at);
    let t1 = parse_at(&after.at);
    if t1 <= t0 {
        return Some(after.accrued_usd.0);
    }
    Some(before.accrued_usd.0 + ((after.accrued_usd.0 - before.accrued_usd.0) * (t - t0)) / (t1 - t0))
}

/// The burn rate at t: the rate of the tick at or before it, which holds until the next tick.
// cost-history.ts:55
// export function rateAt(points: readonly WorkspaceCostEvent[], t: number): number | null {
//   return around(points, t)?.before.rateUsdPerHour ?? null;
// }
pub fn rate_at(points: &[WorkspaceCostEvent], t: f64) -> Option<f64> {
    around(points, t).map(|(before, _)| points[before].rate_usd_per_hour.0)
}

/// The first instant of the month `at` falls in, in the zone the computer is set to, which is the zone every
/// figure a person reads is in. One reading, so the month the chart draws and the month the places list totals
/// begin at the same instant.
// cost-history.ts:62
// export function monthStart(at: number): number {
//   const d = new Date(at);
//   return new Date(d.getFullYear(), d.getMonth(), 1).getTime();
// }
/// Local midnight of the first of `at`'s month in `zone` (the host's zone, rule 8.16), as ms epoch.
pub fn month_start(at: f64, zone: &jiff::tz::TimeZone) -> f64 {
    let local = jiff::Timestamp::from_millisecond(at as i64).unwrap().to_zoned(zone.clone());
    let start = local.date().first_of_month().at(0, 0, 0, 0).to_zoned(zone.clone()).unwrap();
    start.timestamp().as_millisecond() as f64
}

/// The first instant of the day `at` falls in, in the zone the computer is set to, as monthStart reads the month.
// cost-history.ts:68
// export function dayStart(at: number): number {
//   const d = new Date(at);
//   return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
// }
/// Local midnight of `at`'s day in `zone`, as ms epoch.
pub fn day_start(at: f64, zone: &jiff::tz::TimeZone) -> f64 {
    let local = jiff::Timestamp::from_millisecond(at as i64).unwrap().to_zoned(zone.clone());
    let start = local.date().at(0, 0, 0, 0).to_zoned(zone.clone()).unwrap();
    start.timestamp().as_millisecond() as f64
}

/// What a series accrued between `from` and its newest tick, or up to `until` where one is given: past the newest
/// tick the series goes on at that tick's rate, so a machine still running reads what it has cost since, and one
/// whose newest tick is at rate 0 adds nothing. Nothing for a series that ends before `from`, and the whole of it for
/// one that began after. A workspace deleted mid-month keeps its own share this way, since its series stops where it
/// was deleted and its caller gives it no `until`.
// cost-history.ts:78
// export function spentSince(points: readonly WorkspaceCostEvent[], from: number, until?: number): number {
//   const last = points[points.length - 1];
//   if (last === undefined) return 0;
//   const end = until ?? Date.parse(last.at);
//   const total = (t: number): number | null => (t > Date.parse(last.at) ? accruedPast(last, t - Date.parse(last.at)) : accruedAt(points, t));
//   return Math.max(0, (total(end) ?? 0) - (total(Math.min(from, end)) ?? 0));
// }
pub fn spent_since(points: &[WorkspaceCostEvent], from: f64, until: Option<f64>) -> f64 {
    let Some(last) = points.last() else {
        return 0.0;
    };
    let last_at = parse_at(&last.at);
    let end = until.unwrap_or(last_at);
    let total = |t: f64| {
        if t > last_at {
            Some(accrued_past(last, t - last_at))
        } else {
            accrued_at(points, t)
        }
    };
    let start = if from.is_nan() || end.is_nan() { f64::NAN } else { from.min(end) };
    let spent = total(end).unwrap_or(0.0) - total(start).unwrap_or(0.0);
    if spent.is_nan() {
        spent
    } else {
        spent.max(0.0)
    }
}

// Date.parse of a tick's stamp: NaN where it reads no date, so every comparison with it is false, as in the TS.
fn parse_at(at: &str) -> f64 {
    at.parse::<jiff::Timestamp>().map(|t| t.as_millisecond() as f64).unwrap_or(f64::NAN)
}

// The index of the tick at or before t and the one after it, when t falls inside the series.
fn around(points: &[WorkspaceCostEvent], t: f64) -> Option<(usize, Option<usize>)> {
    let first = points.first()?;
    if t < parse_at(&first.at) {
        return None;
    }
    let (mut lo, mut hi) = (0usize, points.len() - 1);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if parse_at(&points[mid].at) <= t {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Some((lo, (lo + 1 < points.len()).then_some(lo + 1)))
}
