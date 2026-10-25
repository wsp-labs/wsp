# status_meter
Source: packages/runtime/src/status.ts, the awake meter and cost series of `createStatusTracker`: :230-255, :282-287, :343-500, :788-847, :891-923 at 65f6b89c7. Tests: status.test.ts "status.watch cost events", "a pause the provider made", "accrued cost", "status.history", "spend today", "gone machines and the meter", "serveRuntime status.subscribe" (the tracker's half). Rust: `src/status/meter.rs`, an `impl` block of `StatusTracker`; tests `tests/status_meter.rs`.

## What it does
Bills awake time. Lifecycle events open and close awake stretches; a cost tick folds each workspace's stretch into a `workspace.cost` point, appends it to a folded series (`append_cost_point`) and stores the series, so the chart and the month's spend survive a restart. A deleted workspace's series is kept, ended, for the month's total on the place that billed it, and dropped once its last point leaves the month.

## Behaviour
1. State, per workspace (:343-363): `meters {awake_ms, mark?, awake_until?}`, `histories` (points), `stood {kind, machineId, place?, provider?}`, `ended` ids, `closed_by_gone {mark, awake_ms}`, `sent_figures`. A meter is made at first use with `awake_ms: 0`.
2. `saw_awake(id)` sets `awake_until = now`; `began_awake(id)` sets `mark = now` then `saw_awake` (:377-383). The record's phase alone never moves `awake_until`.
3. Bus listeners, registered at construction in this order (:384-434, :784, :840-847): created (meter), woken (meter), upgraded (meter), napped (meter), gone (meter), deleted, `workspace.status` (status_poll), then created, woken, upgraded, napped, gone (event tick). A meter listener of a type runs before that type's tick.
4. `workspace.created`: `began_awake(e.workspace.id)`. `workspace.woken`: a stretch closed by a gone verdict reopens (`mark` and `awake_ms` put back, the entry removed, `saw_awake`); else `began_awake` (:387-398). `workspace.upgraded`: the closed-by-gone entry is dropped (:400-402).
5. `workspace.napped` and `workspace.gone` (:406-419): `found` is a gone event or a napped with `found: true`; the stretch ends at `awake_until` (or now if none) when found, else now. Napped drops the closed-by-gone entry; gone with a `mark` first stores `{mark, awake_ms}` as closed-by-gone (the values before folding). Then, with a `mark`, `awake_ms += max(0, end - mark)`; `mark` cleared.
6. `workspace.deleted` (:420-434): `close_series(id)`, then the meter, closed-by-gone, every status_poll map entry for the id (reconciled, gone reasons, suspects, every reader's probes, doubted, facts dials, last emitted) and `sent_figures` are dropped, and `forget_series(id)` runs.
7. Load (:439-463), started at construction; `cost_tick`, `history` and `spend` wait for it. `month = month_start(now)`. For each `cost-histories` document in store order: no last point (or `points` not an array) is skipped and kept; an `ended` document whose last point's `at` is before `month` is deleted (a failure warned `the cost history of the deleted workspace <id> could not be dropped`, the load goes on) and skipped; else its points and `where` are held; an `ended` one joins `ended` and gets no meter; any other gets `awake_ms = last.awake_ms` and, when `last.phase == running`, `mark = awake_until = parse(last.at)` (pausing is not running here). A failed listing warns `cost histories not loaded; the series begins at the first tick` and the tracker starts empty.
8. Writes (:466-469) run one at a time behind the load, each after the one before, failures swallowed (rule 3.9, chain of the second form). A queued write builds its document **when it runs**, from the state then (:499, :821): `{workspaceId, points, where?, ended?: true}` (:473-476), keys in that order.
9. `accrue(last, awake_ms, rate_now)` (:285-287): `accrued_past({accrued: last.accrued or 0, rate: last.rate or rate_now}, awake_ms - (last.awake_ms or 0))`.
10. `cost_tick(only)` (:791-834): after the load, `now` is read **before** the records are listed. For each record (only `only` when given): running sets `mark` if unset; gone or released clears `mark` without folding; `running = (phase running or pausing) and mark set`; `awake_ms = meter.awake_ms + (running ? now - mark : 0)`; the tick is `{type: "workspace.cost", workspaceId, phase, rateUsdPerHour: running ? rate : 0, awakeMs, accruedUsd: accrue(last, awake_ms, rate), at: iso(now)}`; the series becomes `append_cost_point(before, tick)`; `stood` is set from the record (`kind`, `machineId`, then `place`, `provider` where set). The tick **added** a point unless the series had two or more points and the last two and the tick share one rate (that fold replaces the newest point); an added point queues a document write.
11. Who hears a tick (:822-832): `unbilled` is rate 0 and accrued 0; `moved` is figures differing from `sent_figures`. The timer's tick (`only` absent) is due when `!unbilled || moved`; an event's tick when the rate differs from the last point's (no last point: due). A due tick records its figures, then `emit`s when moved and `pass`es when not. So a workspace nobody bills says its zeros once, and an event at an unchanged rate says nothing.
12. Event ticks (:839-847): created, woken, upgraded, napped, gone each run `cost_tick(id)` as a guarded tick (a failure warns `cost tick failed`, never reaches the caller of `emit`), spawned, not awaited.
13. `close_series(id)` (:480-488): with a series, a last point and a meter, appends a point copied from the last with `phase: "gone"`, `rateUsdPerHour: 0`, `awakeMs = awake_ms + (mark ? max(0, now - mark) : 0)`, `accruedUsd = accrue(last, awakeMs, last.rate)`, `at: iso(now)`. No write of its own.
14. `forget_series(id)` (:492-500): no series: `stood` dropped and the document deleted (queued); else the id joins `ended` and the document is written (queued).
15. `history(id)` (:893-896): after the load; an ended id gives `[]`; else the series or `[]`.
16. `spend(places, at = now)` (:898-923): after the load; `day = day_start(at)`, `month = month_start(at)`; running ids are records with phase running or pausing. Rows start, in `places` order, as zero rows for every place with a spend limit (`place_spend_limit`). For each series in insertion order with a `stood` that `workspace_place_id` maps to a place: `until = at` when the id is running, else none; `today += spent_since(points, day, until)`, `month += spent_since(points, month, until)`, and when running `rate += last point's rate`. The answer is one row per entry of `places` that has one, in `places` order.

## Edge cases
- A tick for a workspace napping since the last tick: rate 0, awake time unchanged, no new point when the two before were also 0.
- A host that died between a nap and its tick: the stored last point says running, the record says napping; the load sets a `mark` and the next timer tick clears nothing (napping is not gone), so `running` is false and nothing is billed; the wake's `began_awake` starts afresh ("a host that died between a nap and its tick does not bill the nap").
- A gone record at hydrate: the tick clears `mark` without folding, the frozen total stays, and the first tick after a rebuild bills from the rebuild only.
- A gone verdict the next read unmakes: the woken event reopens the stretch at the old mark, so the gap is billed.
- `spend` with no series and a place with a limit: a zero row; a place with no limit and no series: no row.

## Behaviour nobody wrote down
- Rule 3.11: `histories` is an `IndexMap`, insertion order, removals `shift_remove`; `spend` folds in that order.
- Rule 3.9: the document writes are one chain per tracker, second form (never surfaces a failure).
- Rule 2.27: the stored document's key order is Behaviour 8's.
- What survives a restart: the series, `where` and `ended` (the store); the meter is rebuilt from the last point; `closed_by_gone`, `sent_figures` and everything in status_poll are lost.

## Decisions
1. The tracker needs a time zone for `day_start` and `month_start` (the TS reads the process zone); `StatusTrackerOptions::zone` defaults to the system zone and tests pin UTC.
2. `workspace_place_id` (wsp-words) takes a whole `WorkspaceView` in Rust where the TS takes four fields; the meter needs a narrow form (`Stood`). Filed for the wsp-words owner; until it lands the implementer builds the call from `Stood`.
3. "Added" (Behaviour 10) is decided by the fold rule, not object identity, which Rust does not have; the two agree, the cap of `COST_HISTORY_CAP` included.

## After the port
- `cost_tick` computes `now - mark` without `max(0, ...)`, so a `mark` set from a stored point later than the clock (a clock that moved back) bills negative time.

## Tests
| Rule | Test (`tests/status_meter.rs`) |
|---|---|
| 2, 4, 10, 11 | `emits_workspace_cost_on_a_timer_with_awake_time_accrual_zero_rate_while_napping`, `keeps_the_total_and_keeps_ticking_through_a_pause_the_provider_takes_100_ms_over` |
| 11 | `sends_a_folder_nobody_bills_once_and_none_of_the_timers_ticks_after`, `a_rebuild_at_an_unchanged_rate_lands_no_cost_event`, `an_event_tick_at_a_changed_rate_is_emitted_at_once` |
| 5 | `a_pause_found_after_a_gap_ends_the_awake_stretch_at_the_last_proof`, `a_pause_this_host_made_ends_the_stretch_now`, `a_gone_verdict_unmade_by_a_wake_reopens_the_stretch_it_closed`, `a_rebuild_after_gone_starts_a_fresh_stretch` |
| 9, 10 | `is_each_awake_stretch_billed_at_the_rate_that_held_over_it` |
| 7, 8, 14 | `survives_a_host_restart_the_series_reloads_and_meters_on`, `a_workspace_napping_across_the_restart_keeps_its_total_and_accrues_nothing`, `a_host_that_died_between_a_nap_and_its_tick_does_not_bill_the_nap` |
| 7 | `drops_a_deleted_workspaces_document_once_its_last_tick_falls_out_of_the_month`, `a_document_with_no_points_is_skipped_and_kept`, `a_failed_load_starts_empty_and_warns` |
| 6, 13, 14, 15 | `hands_a_deleted_workspaces_chart_to_nobody_and_keeps_what_it_spent_on_the_place`, `a_deleted_workspace_with_no_series_has_its_document_deleted` |
| 8, 10 | `only_an_added_point_writes_the_document_and_the_write_carries_the_state_when_it_runs` |
| 10 | `a_gone_record_drops_its_mark_without_folding_and_a_rebuild_does_not_bill_the_downtime` |
| 16 | `totals_each_place_over_the_workspaces_metered_on_it_and_burns_only_for_the_ones_awake`, `counts_a_month_from_its_first_day`, `counts_of_a_stretch_that_began_yesterday_only_the_part_since_midnight`, `reads_a_running_machine_live_between_ticks`, `answers_every_limited_place_a_row_and_leaves_the_rest_off` |
| 12 | `a_tick_that_throws_is_warned_and_never_reaches_the_emitter` |
