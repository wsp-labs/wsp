# plan_alerts
Source at 65f6b89c7: packages/runtime/src/plan-alerts.ts (77). Tests: plan-alerts.test.ts (5). Rust:
`src/plan_alerts.rs`; tests `tests/plan_alerts.rs` (14). Reads `wsp_words::plan_alerts::{plan_alerts_of,
plan_blocked, block_ends_at}` (implemented).

## What it does
Each limit reading a turn printed is read against the one it replaced, and the alerts that reading makes are said. A
blocked account is told it is back at its reset by a timer, since no turn runs on an account that cannot run one. A
comeback the timer said is not said again by the next reading.

## Behaviour
- **A1** `read(before, after)` (:56-65): for each alert of `plan_alerts_of(before, after, clock.now())`, in order: emit
  `UsageAlertEvent { key, agent, label }` of **after** with the alert. A `blocked` alert then arms after's comeback.
- **A2** `arm(l)` (:38-53): cancel any timer for `l.key`; `end = block_ends_at(l)`; no end arms nothing. Else one timer
  of `min(max(0, end - now), LONGEST_WAIT_MS)` ms (86,400,000; truncated to whole ms), stored by key.
- **A3** A `back` alert (:58-61) cancels the key's timer first; then, where `before` is present and the comeback of
  `(before.key, block_ends_at(before))` was said by a timer, that mark is taken off and **nothing** is said for this
  alert. Otherwise `back` is said.
- **A4** The wait never runs negative: a reset already past arms a 0 ms timer, which fires on the next clock turn.
- **A5** The timer (:42-51): takes its own key off the timer map first, then reads `limits()`. The account absent from
  the ledger, or no longer blocked there: nothing. Its block ending after now: arm again from that reading, nothing
  said. Else mark `(key, end)` as said by the timer and say `back` with the ledger reading's key, agent and label. A
  `limits()` that fails is swallowed: nothing said, nothing re-armed.
- **A6** `resume()` (:66-71): for every ledger reading whose block ends strictly after now, arm it. Nothing is said.
- **A7** `close()` (:72-75): cancels every timer and clears the map; nothing stops a later `read` or `resume` from
  arming again.
- Nothing survives a restart: the said marks and the timers are memory; a restarted host calls `resume`.

## Decisions
1. `PlanAlerts` is held in an `Rc`; a timer holds a `Weak` of it and spawns its read on the current `LocalSet`.
2. The said marks key on `(account key, the end's f64 bits)` rather than the TS's `${key}@${end}` text: the same
   identity, with no JS number printing.
3. `armed()` answers how many timers stand, so a test can see a re-arm without reading the clock.

## After the port
- A3 keys the said mark on `before`'s end. A reading whose `before` is a later blocked reading than the one the timer
  read (the same account re-blocked to a new end between them) says `back` a second time. Pinned as the TS does it.

## Tests (`tests/plan_alerts.rs`)
| Rule | Test |
|---|---|
| A1, A3, A5 | `says_a_blocked_account_is_back_at_its_reset_by_the_timer_and_the_next_reading_does_not_say_it_again` |
| A3 | `a_reading_free_of_the_block_before_the_reset_says_it_is_back_and_the_timer_says_nothing_after`, `a_new_block_after_a_timed_comeback_says_its_own_comeback_by_reading` |
| A6 | `arms_the_comeback_of_an_account_blocked_when_the_host_starts_and_none_for_one_whose_reset_has_passed`, `resume_arms_nothing_for_a_block_ending_exactly_now_or_an_account_not_blocked` |
| A2, A5 | `a_reset_further_off_than_one_wait_reads_again_on_waking_and_waits_on`, `a_second_blocked_reading_says_nothing_and_the_timer_waits_on_for_the_later_reset_it_reads` |
| A7 | `closing_lets_every_timer_go`, `a_block_read_after_close_arms_again` |
| A1 | `low_alerts_are_said_window_by_window_with_the_accounts_own_names_and_arm_nothing` |
| A2 | `a_block_with_no_reset_named_is_said_and_no_timer_stands` |
| A5 | `the_timer_says_nothing_where_the_ledger_no_longer_holds_the_account_or_reads_it_free`, `a_ledger_read_that_fails_on_waking_says_nothing_and_arms_nothing` |
| A4 | `the_wait_is_the_time_to_the_reset_capped_at_one_day_and_never_negative` |

Every test runs on `FakeClock`, so the clock unit is implemented first.
