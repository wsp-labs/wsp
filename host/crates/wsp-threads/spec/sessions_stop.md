# sessions_stop: `threads/sessions.ts:41-207, 977-990, 1482-1485`

A stop on a turn (`interrupt`), the end it runs on a thread's group, and the holds that keep a thread's next turn
from launching into a group being emptied. State, memory only: `ending` (thread to the end its next turn waits on),
`waits_for` (thread to the computer it waits on), `behind_turn` (thread to the sends waiting on its running turn),
`held_sends` (row id to a send waiting on an owed end). A restart forgets them all; an owed end survives only as the
kind's `end_owed` (sessions_launch N8).

## Holds (sessions.ts:55-68)

- **K1** `hold_next(thread, until, box?)`: the gate is `until`, joined with any gate standing (both must settle, so
  a second stop never releases the first's hold). With `box`, the thread waits on that computer and every send
  waiting behind the thread's running turn is woken. When the gate settles and is still the thread's gate, the
  thread's gate and box are forgotten. `stop_holds(thread)` answers whether a gate stands.

## `stop_unheard(place, stopping)` (sessions.ts:74-100)

- **K2** `true` at once where the place is away or the door has no link to it. Otherwise the first of: the place
  goes absent (`place.absent` for it): true; `STOP_REACH_MS` passes: true; the ping fails: true; the agent's own
  stop settles either way: whether the place is away then. A ping that answers clears the quiet timer, and settles
  false only where there is no agent stop. It settles once; the listener is taken off.

## `stop_turn(marked, row, origin, task)` (sessions.ts:102-148)

- **K3** After `ready`. The row is found by its id, else as the row of that turn id. No row, but a thread record of
  that id: the thread's trimmed row (trimmed_row) with the thread id as turn. Neither: `not-found`.
- **K4** A row the caller cannot reach (`entry_of_row`): `not-found`, the same answer as no row.
- **K5** A send waiting on an owed end (`held_sends`), with no task: given up; `accepted` with `left` its words.
- **K6** A task: the row must be running with a handle, else `not-running`; no `stop_task`: `unsupported` with
  `task_stop_unsupported_line(agent)`; `refused`: `refused` with `task_stop_refused_line(agent, error)`;
  `unsupported` likewise; else the outcome as it came. Nothing else is stopped.
- **K7** A turn held at its computer's cap is marked stopped (`cap_stop(row, cap_stopped_line(wait))`) before
  anything waits; then `marked()`. A turn queued on its machine (`drop_queued`) is dropped: `accepted`.
- **K8** The tree under the thread is stopped first (`stop_under(thread, origin)`); the answer carries `under` only
  where it is not empty. A held turn then waits for its launch to settle: `accepted`.
- **K9** `try_stopped(turn)`, then `end_turn`; the answer is its outcome and `left`.

## `end_turn(row, origin, row_of)` (sessions.ts:151-207)

- **K10** A running row with no handle but a launch in flight waits for it and reads the row again.
- **K11** Where the kind ends a thread's group (`end_thread`), the thread's next turn is held (K1) until this end
  is over, set before the turn is stopped.
- **K12** A row not running, or with no handle and no road to end it (not an unanswered re-open with an `end` that
  is no line's try, and no group end with an `end`): `not-running`; with a group end and no other turn of the
  thread running or launching, the group is ended and its words are `left`.
- **K13** The agent's stop: `interrupt()` then `finished` (failures swallowed).
- **K14** A computer that does not hear the stop (K2): the group end is run `{ away: true }`, the row ended with its
  words else `TURN_STOPPED_LINE` as stopped and unreached, and an owed end (`end_owed`) holds the next turn on the
  computer's name (K1). `accepted` with those words.
- **K15** No agent stop (no handle): with no group end, the row ends `TURN_STOPPED_LINE` stopped and unreached,
  `accepted`; with one, the row ends stopped and the group end's words are `left`.
- **K16** Otherwise the agent's stop is awaited and the group (where one) ended: `accepted` with its words. The hold
  of K11 is released however the end went.

## `interrupt` and `stop_try` (sessions.ts:977-990, 1482-1485)

- **K17** A stop with no task opens a cap stop (`cap_stopping(row)`) released when the stop is done, whichever way.
- **K18** One log line per stop: `stop_log_line(thread, task, outcome, left)`, or on failure with the outcome
  `"failed: <message>"`; the failure is the caller's. The thread is the row's thread, else the id asked.
- **K19** `stop_try(row)`: `end_turn` alone, with no origin, nothing beside (no tree, no cap mark).

## Tests (`tests/sessions_stop.rs`)

| Rule | Test |
|---|---|
| K1 | `a_second_hold_joins_the_first`, `the_hold_is_forgotten_when_its_gate_settles` |
| K2 | `a_computer_away_does_not_hear`, `a_ping_answered_with_no_agent_stop_hears`, `no_answer_in_stop_reach_ms_does_not_hear`, `an_absent_event_does_not_hear` |
| K3, K4 | `a_row_nobody_holds_is_not_found`, `a_row_the_caller_cannot_reach_reads_not_found`, `a_trimmed_thread_is_stopped_by_its_record` |
| K5 | `a_send_waiting_on_an_owed_end_is_given_up` (owed-box.test.ts) |
| K6 | `a_task_stop_stops_only_the_subagent`, `a_task_on_a_turn_not_running_is_not_running`, `a_harness_with_no_task_stop_is_unsupported` |
| K7, K8 | `a_held_turn_is_marked_and_never_launches` (thread-cap.test.ts), `a_queued_turn_is_dropped`, `a_stop_takes_the_tree_under_first` (box-stop.test.ts) |
| K11-K16 | `a_stop_ends_the_turn_then_its_group` (stop-past-grace.test.ts), `a_stop_on_a_box_away_owes_the_end_and_holds_the_next_turn` (owed-box.test.ts), `a_finished_thread_has_its_group_ended`, `another_turn_running_is_not_what_was_stopped` |
| K17, K18 | `every_stop_logs_one_line`, `a_failed_stop_logs_it_and_fails` |
| K19 | `a_lines_try_is_ended_with_no_tree_and_no_mark` |
