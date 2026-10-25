# boot_rows
Source: `packages/runtime/src/machines/boot.ts:673-1013` (about 340 of 1,071 lines) at `65f6b89c7`. Tests: integration
cases in `session-row.test.ts`, `session-row-oracle.test.ts`, `thread-cap.test.ts`, `sub-disk-keep.test.ts`,
`thread-defaults.test.ts`, `thread-marks.test.ts`, `thread-head.test.ts`, `sub-results.test.ts`,
`thread-keeps-process.test.ts`, `usage-limit-resume.test.ts` (under "Rules against tests"), and 15 scenes in
`oracle/boot_rows.oracle.ts` (`tests/fixtures/oracle/boot_rows.json`). Rust: `src/machines/boot/rows.rs`.

## What it does
The rows every window lists and the pushes that keep them current: a row as a listing answers it, a thread's facts,
whether a thread reads settled, the `session.row` push batched once per tick, and the marks a person puts on threads
(read, settled, pinned, snoozed, Resume at reset) with the snooze's early end and the reset's own resume.

## Behaviour
1. `listed_rows(held)` (:681): each row's view, then in this order: `pid` (running and a pid), `agentPid` (running, a
   pid and a run, `agent_pid_of(run)`), `waitingOn` (running and stopped behind), `capped` (the cap's wait for the
   turn), `subagents` (only on the latest row of its thread, from the transcript index's children, each without
   `turnId` and `startRow`, only when any), `setupRefusal` (the refusal of the row's harness at its workspace's setup
   place), `readAt` (the record's, else the row's `endedAt` when it is before `reads_since`, else `reads_since`),
   `settledAt`, `pinnedAt`, `order`, `foldedAt` (the record's), `replaces` (the record's or the row's, only while
   that thread has a record or a latest row), `replacedBy` (from `restarts()`), `snoozedUntil` while in the future
   else `wokeAt`, `section`, `rewoundAt` (`rewound.at`), `resumeAt` (the arm's `at` when its turn is this row's),
   `forkedFrom` with the source's title as `fold_threads(rows_on(source))` reads it now, else as kept.
2. `thread_facts(thread)` (:739): the rows of that fold key, folded by `fold_threads(listed_rows(...))`; none answers
   `None`. The thread view without `subagents`, then `sub` (the record's sub row), the disk facts (`diskGone`,
   `diskKeptUntil`), the latest row's `model`, `effort`, `contextWindow`, and `turnId` of the last running row.
3. `facts_after_read(thread, origin)` (:760): a sub-thread's facts once `sub_read(ws, since)` ran, `since` being now
   for a running thread else its `endedAt`; anything else, or a caller the row is out of reach for, as they stand.
4. `settled_now(thread, settle_ms)` (:771): with no row, the record's `settledAt` alone; else `thread_settled({working,
   asking, failed, startedAt, endedAt, readAt, settledAt}, now, settle_ms)`.
5. `push_head(thread)` (:778): emits `thread.head {workspaceId, threadId, facts, pos}` when facts stand (`pos` the
   index's, else 0).
6. `placed_rows(listed)` (:783): reads `computer_rows()` once when any row is given; each row gets `sub` and the disk
   facts, and, when its project is held, `project: {id, name}` and `computerName` (the computer row's table name, or
   for a sub `cloud_said(sub.on)`, or the project's computer id).
7. The row push (:801-865): `row_moved(ws, thread)` and `row_gone(id, at)` collect into two maps and queue one push at
   the end of the current task (03 rule 3.12). A push takes the moved threads, drops gone ids still held in
   `sessions`, clears both maps, builds the latest listed row of each moved thread (none skipped), leaves out `prompt`
   for a row id told before, notes rows stopped behind a prompt, asks for each moved workspace's titles (not while its
   boot work runs), then on the `rows_out` chain: passes `{type: "session.row", workspaceId, threadId?, id}` for each
   gone id, then each placed row as `{type, workspaceId, threadId?, id, row}`, marks told ids, unmarks gone ids, and
   prunes told ids not held once the set passes `2 * sessions + 64`. A failed batch warns `a moved row was not pushed:
   <why>` and puts its threads back for the next. `rows_sent()` runs a queued push now and awaits the chain.
8. The `*` listener (:873): `thread.head` moves its thread and every told-behind row stopped behind it; `session.held`,
   `session.capped`, `session.uncapped`, `session.subagent` move their thread; `thread.marked` moves each thread;
   `workspace.napped`, `workspace.woken`, `workspace.upgraded` move every row on the workspace; `session.start` with a
   turn id unlike the session id drops the turn id's row; `session.end` drops the session id's row when no held row
   has that turn; `session.permission` and `.closed` move their thread and every running row told behind or stopped
   behind; `session.delta` moves its thread for a `tool_use` that follows threads, or a `tool_result` of a turn told
   behind.
9. `mark(threads, stamped, origin)` (:923): all threads are checked first (a thread with no row and no record, or out of
   the caller's reach, refuses `no thread <thread_word>` kind `not-found` and nothing moves). Then each thread's
   record (or one made from its latest row) takes the stamps, an `undefined` stamp deleting its key; a settle reaps the
   kept agent; a snooze arms `wake_at`; a `limitResume` stamp arms `resume_on_reset` or cancels the resume timer. Each
   workspace touched is persisted and told `thread.marked` once, in first-touched order.
10. `end_snooze_for(row)` (:959): the thread and its root (deduped) whose snooze stands past now are marked
    `snoozedUntil: now` (not awaited; a failure warns `snooze not ended for <ids>: <why>`).
11. `arm_resume(threads, on, origin)` (:966): off is `mark(threads, {limitResume: undefined})`. On: every thread's latest
    row checked first (no row or out of reach: `no thread <word>`, not-found; no `limit.resetsAt`: `refusal("thread
    <word> is not stopped at a usage limit with a known reset,", "so there is nothing to resume at; send it a message
    to go on now.", "usage")`), then each marked `{limitResume: {at, turnId}}`.
12. `resume_after_limit(thread)` (:984): nothing without a record and an arm. It stands when not settled, the workspace
    is live, the latest row is the armed turn and carries a limit; it waits while the lead asks, or the facts ask or
    wait. Standing and not waiting, `held_past(entry, harness)` reads the reset the account is still held behind. Only
    if the arm is still the same object: drop it, take a held reset onto the latest row's limit, persist, emit
    `thread.marked`; then, standing, not waiting and not held, `sessions_api.start(ws, {prompt: LIMIT_RESUME_PROMPT,
    thread, afterLimit: arm.at})`.
13. `held_past(entry, harness)` (:1008): `held_until` of the windows of the ledger's limit row keyed by
    `account_on_computer(...)`.

## Edge cases
- A `session.end` whose turn a held row still carries drops nothing.
- The first push under a row id carries its prompt; a launch's move to the harness id carries it again.
- `mark` with an unknown thread among known ones moves none of them (scene).

## Behaviour nobody wrote down
- Rule 3.12 and 3.24: rows are passed, never emitted, after the event that moved them is out, with the head's `seq`.
- Rule 3.10: `mark` writes the record from a copy read before its awaits (unguarded in the TS; kept).
- Rule 8.3: `thread_word` cuts ids by UTF-16 units.

## After the port
- The arm-resume refusal reads `... with a known reset,. so there is nothing ...`: the first half ends in a comma and
  `refusal` adds `. `.

## Rules against tests
| Rule | Tests (`tests/boot_rows_scenes.rs` replays a scene by name; `tests/boot_rows.rs`) |
|---|---|
| 1 | scene `listed_rows_every_fact_a_listing_carries_and_none_a_row_does_not` |
| 2, 4 | scenes `thread_facts_*`, `settled_now_*` |
| 3, 5, 6 | `facts_after_read_waits_for_the_sub_threads_read`, `push_head_carries_the_facts_and_the_index_pos` |
| 7, 8 | scene `row_push_one_row_per_moved_thread_*`; `a_failed_batch_goes_out_with_the_next`, `a_turn_start_and_end_each_push_that_threads_one_row` |
| 9, 10 | scenes `mark_*`, `end_snooze_*` |
| 11 | scene `arm_resume_off_clears_on_needs_a_known_reset_on_the_latest_row` |
| 12, 13 | scenes `resume_after_limit_*` (6) |
