# clock
Source: packages/runtime/src/clock.ts (23 lines) and the test clock packages/runtime/test/fake-clock.ts (39 lines) at 65f6b89c7. Tests: none of their own; 51 runtime test files lean on `fakeClock()`. Rust: `src/clock.rs`, tests `tests/clock.rs`.

## What it does
The one time seam of the runtime (01, seam table; rule 3.15). `now()` is epoch milliseconds; `schedule(f, ms)` runs `f` once after `ms` and hands back a cancel. There is no interval: every repeating timer re-arms itself. `FakeClock` is the test clock every crate's tests use, so it is a public item of this crate, not test-only code.

## Behaviour
1. `Clock::now() -> i64`, epoch ms (clock.ts:6). `RealClock::now` is the system time in whole ms, as `Date.now()` (clock.ts:17).
2. `Clock::schedule(f, ms) -> Cancel` (clock.ts:8). `Cancel` is `Rc<dyn Fn()>`: calling it stops a timer that has not fired; calling it again, or after the timer fired, does nothing (clock.ts:21 `clearTimeout`, fake-clock.ts:14 `timers.delete`).
3. `RealClock::schedule` runs `f` on the current thread's `LocalSet` (`tokio::task::spawn_local` of a sleep then `f`), as Node's timer runs on its one thread (rule 3.1). Called outside a `LocalSet` it panics, as every `spawn_local` does.
4. `RealClock::schedule` clamps as Node's `setTimeout` does: a delay below 1 or above 2,147,483,647 ms fires after 1 ms (Node `timers`: `TIMEOUT_MAX`). A huge window therefore fires at once on the real clock.
5. `ScheduleOptions { unref }` (clock.ts:11-14) is dropped: rule 3.15 says `unref` has no meaning in Rust. `FakeClock::holding()` goes with it.
6. `FakeClock::new(start)` starts at `start`; `FakeClock::at_now()` at the system time (fake-clock.ts:8 `start = Date.now()`). `clock()` hands out an `Rc<dyn Clock>` that shares the fake's state; every handle reads one `now`.
7. `FakeClock` `schedule(f, ms)` stores `{at: now + ms, f}` under an id that grows by one per call, starting at 1 (fake-clock.ts:13-17). A negative `ms` is stored as is (`at` before `now`).
8. `advance(ms)` (fake-clock.ts:20-31): `target = now + ms`; repeat: among the pending timers with `at <= target` take the one with the smallest `at`, ties to the smallest id (the earliest scheduled); remove it; set `now = at`; call `f`. Stop when none is due; then `now = target`. So:
   - a timer `f` schedules or cancels inside the advance is seen by the same loop: one due by `target` runs in this advance;
   - `now()` inside `f` reads that timer's own `at`, even when `at` is before the `now` the advance started from (a negative delay);
   - `advance(0)` runs every timer due at or before `now`.
9. `f` runs with no borrow of the fake's state held (rule 3.3): it may call `now`, `schedule`, a `Cancel` and `pending` on the same clock.
10. `pending()` is the number of timers stored and not yet run or cancelled (fake-clock.ts:34). `due_for(at)` is whether any stored timer has exactly that `at` (fake-clock.ts:37).

## Edge cases
- Cancel of an id already run: nothing, no panic (fake-clock.ts:16).
- Two timers at one `at`: run in the order they were scheduled (`clock_ties_run_in_scheduling_order`).
- A timer that schedules one at `ms = 0` during an advance: it runs in that advance, after the current one (`clock_a_timer_scheduled_inside_advance_runs_in_the_same_advance`).
- `advance` with nothing pending only moves `now`.

## Behaviour nobody wrote down
- Rule 3.15: every time read and every timer of the Rust host goes through this trait; this unit is that trait.
- Rule 3.3: the fake calls `f` after releasing its own `RefCell` borrow, or the first timer that schedules another panics.

## Decisions
1. `now` is `i64`, not `f64`: every TS caller reads `Date.now()`-shaped whole numbers.
2. `Cancel` is a cloneable `Rc<dyn Fn()>` rather than a `FnOnce`, because the TS calls a held cancel more than once (idle.ts:67 after :84 replaced it).

## After the port
- Node's 1 ms clamp (Behaviour 4) makes a window past 24.8 days fire at once; the slate pilot found the same on `setInterval`. Kept.

## Tests
| Rule | Test (`tests/clock.rs`) |
|---|---|
| 2 | `clock_cancel_stops_a_pending_timer_and_is_harmless_twice_or_after_firing` |
| 3, 4 | `real_clock_fires_on_the_local_set`, `real_clock_clamps_a_delay_past_the_timer_range_to_one_ms`, `real_clock_clamps_a_negative_delay_to_one_ms` |
| 6 | `fake_clock_handles_share_one_now` |
| 8 | `fake_clock_runs_due_timers_earliest_first_and_lands_on_the_target`, `clock_ties_run_in_scheduling_order`, `clock_a_timer_scheduled_inside_advance_runs_in_the_same_advance`, `fake_clock_now_inside_a_timer_reads_its_own_at`, `fake_clock_advance_zero_runs_what_is_due_now`, `fake_clock_a_negative_delay_runs_at_the_next_advance_with_its_own_at` |
| 9 | `fake_clock_a_timer_may_schedule_and_cancel_on_its_own_clock` |
| 10 | `fake_clock_pending_and_due_for_count_what_is_stored` |
