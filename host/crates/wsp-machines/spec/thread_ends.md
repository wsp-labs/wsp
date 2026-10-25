# thread_ends
Source: `packages/runtime/src/machines/thread-ends.ts` (134 lines) at `65f6b89c7`. Tests: no unit test in TS; pinned
through the box suites (`packages/runtime/test/box-stop.test.ts` 12 cases, `box-panels.test.ts:520-570`,
`owed-box.test.ts:41`) and by 18 scenes in `oracle/thread_ends.oracle.ts`, which drive the real TS with a fake store,
runtime and place door and write every call in order to `tests/fixtures/oracle/thread_ends.json`.
Rust: `src/machines/thread_ends.rs`.

## What it does
Ends what a thread left in its cgroup on a computer the person joined, over the place link. A stop or a delete that
cannot finish there writes a debt to the store collection `thread-ends`, keyed by place id, `{threads: [ids]}`; the
next `place.present` of that computer pays every debt it can, and `place.removed` forgets them. `owed` lets a send wait
until the debt of its thread is paid.

## Behaviour
1. The row (:34-42): `owed_on` reads `thread-ends/<place>` and answers its `threads`, or `[]` when the row is absent.
   `keep(place, [])` deletes the row; any other list is put whole. `owe` appends the thread only when absent from the
   list (a second debt of one thread writes nothing, scene "away, linked, already owed").
2. `run` (:45): `door.exec(place, cgroup_end_script(thread_cgroup(thread), {remove}), {timeoutMs: INLINE_EXEC_MS})`
   (20,000). Exit 0 answers `None` (all ended); any other exit answers `survivors_of(stderr)`. `ran` (:69) turns a
   thrown exec into `None` ("did not answer") and an answer into `Some({left})`.
3. `survivors_of(stderr)` (:31): the first match of `/processes((?: \d+)+) of /`; its group trimmed, split on single
   spaces, empties dropped, each `Number(...)`. No match is `[]`, not `None`: a non-zero exit with no pids named still
   counts as an end that ran (scene "stop, linked, nonzero with no pids named": the debt is cleared and the answer is
   `thread_left_line(name, [])`).
4. `busy(thread)` (:51): `running_on(thread).is_some() || (launching_on(thread).is_some() && !stop_holds(thread))`,
   evaluated lazily in that order (the log shows `stopHolds` asked only for a launching thread).
5. `clear` (:63) re-reads the row, writes it without the thread only when the thread was in it (else no write), then
   settles every waiter of `(place, thread)`, after the write.
6. `pay(place)` (:73): awaits `ready()`; reads the row once; an empty list ends there. For each thread in stored order:
   busy skips it (no exec); else `ran(place, thread, {remove: true})`; no answer keeps it; an answer with survivors
   warns `thread_left_line(name, left)`, and any answer clears it (`clear` re-reads the row each time). `paying` runs
   `pay` detached and warns `the threads stopped or deleted while <name> was away were not ended there: <why>` when it
   fails.
7. The door listener (:87): `place.removed` deletes the row (a failure ignored) and settles every waiter whose key starts
   with `<place>\n`; `place.present` starts `paying(place)`; every other event is ignored.
8. `end(place, thread, {remove, away})` (:97), in this order:
   - `away`: `owe` first; then no link answers `thread_end_away_line(name)`; a link starts `paying(place)` and answers
     `thread_end_late_line(name, STOP_REACH_MS / 1000)` (3). `remove` is not read on this road.
   - no link: with `remove`, `owe`; answer `None` either way.
   - linked, not `remove`: `ran`; no answer gives `thread_left_line(name, [])` with nothing cleared; an answer clears,
     then gives `None` for all ended or `thread_left_line(name, left)`.
   - linked, `remove`: an answer with all ended clears and gives `None`; otherwise (no answer, or survivors) `owe`,
     warn `thread_end_owed_line(name)`, answer `None`.
9. `owed(place, thread)` (:125): registers the waiter **before** the read, so a payment landing during the read settles
   it; an owed thread answers `Some({paid})`; none owed settles the waiter at once and answers `None`.

## Edge cases
- The door is asked `link(place)` before anything on the stop and remove roads, and after `owe` on the away road
  (fixture order).
- `place.removed` for one place leaves another place's row and waiters alone (scene "place.removed drops ...").
- A `place.present` with nothing owed awaits `ready()`, reads the row, and asks nothing else.

## Behaviour nobody wrote down
- Rule 3.4: no await lies between `pay`'s busy check and its exec decision; keep that stretch synchronous.
- Rule 3.11: the thread list keeps insertion order; `owe` appends, `clear` filters in place.
- What survives a restart: the `thread-ends` rows. The waiters are memory only; a send after a restart reads the row
  again through `owed`.

## Tapes
None directly; `sessions.interrupt` and the delete ops reach it (their units carry the tapes).

## Decisions
1. The TS takes the whole `RuntimeContext` and a `PlaceDoor`; the Rust takes two narrow traits of this unit,
   `ThreadEndsRuntime` (store row get/put/delete, `ready`, `running_on`, `launching_on`, `stop_holds`, `warn`) and
   `ThreadEndsDoor` (`exec`, `link`, `name_of`), and the door's events come in through `on_place_event`, which the
   wiring calls from the door's listener. The method names are the TS names in Rust case (rule 1.3).
2. `paid` is a future that resolves once; a waiter dropped before it resolves is harmless.

## After the port
- A stop whose end exits non-zero without naming pids clears the debt and tells the person nothing ended (rule 3): the
  end may not have run at all.
- `end` with `away` ignores `remove`; a delete that is late owes the same end as a stop.

## Rules against tests
| Rule | Tests (`tests/thread_ends.rs`) |
|---|---|
| 1 | `owe_appends_once_and_an_emptied_list_deletes_the_row`, scene `away_linked_already_owed_is_not_owed_twice` |
| 2, 3 | `survivors_of_reads_the_pids_the_end_script_names`, scenes `stop_linked_survivors_named`, `stop_linked_nonzero_with_no_pids_named` |
| 4 | scene `pay_keeps_a_busy_thread_and_one_whose_end_did_not_answer` |
| 5, 6 | scenes `owed_then_paid_at_place_present`, `pay_with_nothing_owed_asks_nothing` |
| 7 | scenes `place_removed_drops_the_row_and_settles_every_waiter_of_that_computer`, `other_events_are_ignored` |
| 8 | scenes `remove_computer_away_owed_nothing_said`, `stop_computer_away_nothing_owed_nothing_said`, `away_no_link_owed_and_the_away_line`, `away_linked_owed_the_late_line_then_paid_in_the_background`, `stop_linked_all_ended`, `stop_linked_the_exec_throws_*`, `remove_linked_*` (3) |
| 9 | scene `owed_none_owed_answers_nothing`, `owed_registers_its_waiter_before_the_read` |
| the TS box suites | `a_stop_on_an_away_box_owes_the_end_and_a_present_pays_it` (box-stop.test.ts:122-131), `a_send_after_a_restart_waits_until_the_owed_end_ran` (box-stop.test.ts:283) |
