# idle
Source: packages/runtime/src/idle.ts (121 lines) at 65f6b89c7. Tests: packages/runtime/test/idle.test.ts "idle policy mechanics" (9 cases, :22-265). Rust: `src/idle.rs`, tests `tests/idle.rs`.

## What it does
The runtime's own nap timer per workspace. A touch arms a window; holds keep it from firing; when it runs out `on_idle` is asked to nap the workspace. A nap that fails keeps its deadline and is asked again every `retry_ms`. Every arming tells `on_backstop` the provider's stop instant, twice the window or six hours when the window is off.

## Behaviour
1. `IDLE_OFF_BACKSTOP_MS = 21_600_000` (idle.ts:14). `backstop_ms(None) = 21_600_000`, `backstop_ms(Some(w)) = 2 * w` (:16-18).
2. State (:57-60): `armed: IndexMap<id, Armed{at, cancel, serial}>`, `holds: IndexMap<id, count>`, `closed`. Nothing survives a restart; the runtime touches every running workspace again at load (machines.ts, not this unit).
3. `held(id)` is `holds[id] > 0` or `busy(id) == true` (:62). An absent `busy` is never held by it.
4. Private `forget(id)` (:64-69): an armed entry is cancelled and removed; none, nothing.
5. `arm(id)` (:71-97), in this order: `forget(id)`; if closed, stop (no `on_backstop`); `w = window_of(id)`; `on_backstop(id, now + backstop_ms(w))`, synchronously; `w == None`: stop with nothing armed; else an entry at `now + w`, a timer of `w` ms running `fire`, stored under `id`.
6. `fire` (:78-94): `held(id)` re-arms with `arm(id)` (a new full window from now, `on_backstop` again) and stops. Otherwise the entry's cancel becomes a no-op, the entry stays in `armed` (so `idle_at` still reads the deadline while the nap runs), `on_idle(id, w)` is **called inside the timer callback**, and its future is spawned on the local set. On `Ok`: the entry is removed only if `armed[id]` is still this entry. On `Err(e)`: warn `idle nap of <id> failed: <message>` (`e.message` of an Error, else its string), then, only if `armed[id]` is still this entry, its cancel becomes a new `retry_ms` timer running `fire` again. "Still this entry" is identity in the TS (`===`); in Rust a per-entry serial.
7. `touch` is `arm` (:100). `hold` adds one, with no arming and no other effect (:101).
8. `release` (:102-110): `left = holds[id] (0 if absent) - 1`; `left > 0` stores it and stops; otherwise the count is removed and, if `armed` has `id`, `arm(id)`. So a release with no hold also re-arms an armed window, and a release while a nap is in flight replaces that entry (the nap's outcome then finds another entry and touches nothing).
9. `forget` (public, :111-114): private forget, then the hold count is removed too.
10. `idle_at(id)` (:115): `None` while held; else `armed[id].at`; `None` when nothing is armed.
11. `close` (:116-119): `closed = true`, then every armed id forgotten, in map order. Holds stay. After close, `touch` and the re-arm of a `release` cancel what is there and arm nothing, and `on_backstop` is not called.

## Edge cases
- Window off: `on_backstop` hears `now + 21_600_000`, nothing is armed, `idle_at` is `None`, no timer pending (test `off_means_never_no_timer_no_deadline`, `on_backstop_hears_...`).
- A hold over a deadline: the timer fires, finds it held, re-arms from that instant; the last release re-arms from the release (`the_last_release_starts_the_window_over_from_the_release_not_from_the_touch`).
- A touch or forget while a retry timer stands replaces or drops it (`a_touch_or_a_forget_during_the_retries_ends_them...`).
- `on_idle` failing every time: one call per `retry_ms` forever, each warned, deadline unchanged.
- `forget` clears holds: a later `touch` arms a window that is not held (`forget_clears_the_holds_so_the_next_touch_counts_down`).

## Behaviour nobody wrote down
- Rule 3.9 (take and release): the hold count is the runtime's idle refcount (03 table); hold and release do no I/O and never await.
- Rule 3.3: `on_idle`, `on_backstop`, `window_of` and `busy` are called with no borrow of the policy's state held; each may call back into the policy (a nap that forgets the id, a backstop that touches another).
- Rule 3.15: timers come from the injected `Clock`; `unref` is dropped.

## Decisions
1. `on_idle` returns `LocalBoxFuture<'static, Result<(), String>>`; the error string is what the warn line quotes.
2. Warnings go to `crate::console::warn`, which tests capture (`console::capture()`).

## After the port
- A `release` with no matching `hold` re-arms an armed window (Behaviour 8): a stray release reads as activity.

## Not ported here
"idle policy in the runtime" (:267-552) and "provider backstop on the fork spec" (:554-621) drive `createRuntime`; they belong to `machines/machines.ts` in `wsp-machines`. `idle.live.test.ts` is a live provider test.

## Tests
| Rule | Test (`tests/idle.rs`) |
|---|---|
| 5, 6 | `fires_once_after_the_window_with_the_window_it_used_then_waits_for_the_next_touch` |
| 7 | `every_touch_starts_the_window_over` |
| 3, 8 | `a_hold_keeps_the_window_from_firing_the_last_release_starts_it_over`, `the_last_release_starts_the_window_over_from_the_release_not_from_the_touch`, `a_busy_workspace_is_held_without_a_hold`, `a_release_with_no_hold_rearms_an_armed_window` |
| 5 | `off_means_never_no_timer_no_deadline`, `a_per_workspace_window_overrides_the_default` |
| 9 | `forget_cancels_the_window_a_nap_or_a_delete`, `forget_clears_the_holds_so_the_next_touch_counts_down` |
| 6 | `a_nap_that_fails_keeps_its_deadline_and_is_asked_again_every_retry_interval_until_one_lands`, `a_release_during_a_nap_in_flight_keeps_the_new_window_when_the_nap_lands` |
| 1, 5 | `on_backstop_hears_now_plus_twice_the_window_on_every_arming_now_plus_six_hours_when_off_and_nothing_on_forget` |
| 6, 9 | `a_touch_or_a_forget_during_the_retries_ends_them` |
| 11 | `close_forgets_every_window_and_arms_nothing_after` |
| rule 3.3 | `on_idle_may_forget_its_own_workspace_without_a_double_borrow` |
