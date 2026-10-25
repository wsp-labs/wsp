# costs_and_alerts
Source: at a3b870a6e, `packages/protocol/src/cost-history.ts` (84 lines), the functions of `plan-alerts.ts` (68), `outside-line.ts` (66) and `notifyBy` of `general-prefs.ts`; their shapes and constants (`COST_HISTORY_CAP`, `PlanAlert`, `PLAN_ALERT_STEPS`, `OutsideLine`, `OUTSIDE_HELD`) are `wsp_wire::types::<file>`. Tests: `cost-history.test.ts`, `plan-alerts.test.ts`, `outside-line.test.ts` (25 cases ported; 1 schema-only case skipped), plus `pinned` in `tests/costs_and_alerts.rs` (every value printed by the TS under node with `TZ=UTC`).

## What it does
A workspace's cost history as the ticks where its rate changed, and the totals read off it at an instant, since an instant, from local midnight; when an account's plan running low is worth telling the person, and what a moment says outside the app under the person's notify choices.

## Behaviour
1. `append_cost_point(points, tick)`: when the last two points and the tick share one rate, the tick replaces the last point; else it is appended; past `COST_HISTORY_CAP` (4096) the oldest are dropped (cost-history.ts:10-16). A new list; the given one is untouched.
2. `accrued_past(tick, ms)`: `accruedUsd + rate * ms / 3_600_000` (:31-33). `accrued_at(points, t)`: None before the first tick or with no ticks; the newest's total past it; between two ticks, linear, and the later's total where the two share an instant (:35-44). `rate_at`: the rate of the tick at or before t (:46-48). Ticks are found by binary search on `Date.parse(at)`.
3. `month_start(at, zone)` and `day_start(at, zone)`: local midnight of the first of the month or of the day in the host's zone, as ms (:50-58); the clocks changing keeps midnight (a day of 25 hours).
4. `spent_since(points, from, until)`: 0 for no points; end = until, else the newest tick; the total at an instant past the newest tick runs on at its rate; the result is `max(0, total(end) - total(min(from, end)))`, a total that is None counting 0 (:60-66).
5. `plan_blocked(r)`: status reached, or a counted window (not overage) at 100% or more (plan-alerts.ts:21). `block_ends_at(r)`: None unless blocked; the latest reset of the used-up counted windows, else of every counted window; None without resets (:23-29).
6. `plan_alerts_of(before, after, at)`: blocked after: `[blocked]` unless it was blocked before, then `[]`; blocked before and not now: `[back]`; else per counted window the highest step of 90 and 70 that the window crossed since the reading before (a window whose reset is at or before `at` counts from 0): `{low, window, step}` (:37-47). `plan_alert_line` per :51-55 with the window words session, week, Opus week, Sonnet week, month, extra usage.
7. `outside_line(moment, choices)`: the needs-you choice for asks, a sign-in, setup failed and setup needs you; the done choice for finished, failed, awake, setup ready and image ready; a plan alert shows without sound when planAlerts is on, else nothing; titles and bodies per outside-line.ts:45-66; a failure's body is its error on one line where it has one (`one_line`). `notify_by`: off None; show unless "sound"; sound unless "notify" (general-prefs.ts:25-28).

## Edge cases
- `accrued_at` one unit before the first tick is None; at the newest tick and after, the newest total (pinned in `a_total_reads_off_the_line_between_ticks`).
- `spent_since` with from after until reads 0; sums written by `number_string` keep their float noise ("0.30000000000000004").
- A window that reset since the last reading at 95% says 90 again; an overage window never alerts (`a_plan_is_blocked_and_alerts_only_as_it_gets_worse`).

## Behaviour nobody wrote down
- Rule 8.16: the month and day starts read the host's zone, never UTC.
- Rule 8.17: tick times are `toISOString` text read by `Date.parse`.

## Tapes
`cost.history`, `cost.spend`, `usage.alert` events; the desktop's notifications.

## Decisions
1. `month_start` and `day_start` take the zone (`jiff::tz::TimeZone`); the host passes `TimeZone::system()`.
2. `OutsideMoment` is an enum, one variant per TS kind.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `cost_history_tests::append_cost_point_tests::*`, `a_series_keeps_only_the_ticks_where_the_rate_changes` |
| 2, 4 | `cost_history_tests::what_a_series_says_at_one_instant::*`, `a_total_reads_off_the_line_between_ticks` |
| 3 | `cost_history_tests::what_a_month_took::*`, `what_a_day_took::*`, `months_and_days_start_at_local_midnight` |
| 5, 6 | `plan_alerts_tests::*`, `a_plan_is_blocked_and_alerts_only_as_it_gets_worse` |
| 7 | `outside_line_tests::*`, `setup_lines_and_notify_choices` |
