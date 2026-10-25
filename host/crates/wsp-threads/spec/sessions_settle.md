# sessions_settle: `threads/sessions.ts:209-243, 1112-1173`

Settling threads into the Settled fold, marking them, and restoring them; and the check a restart runs.

## `settle_reads(ids, origin)` (213-234)

- **E1** After `ready`, each id in order: no facts and no record: a subagent's id (any index child set holds it)
  `refusal(subagent_settle_line(id), SUBAGENT_SETTLE_FIX, usage)`, else `not_found_refusal("no thread <word>")`. One
  the caller may not settle: `refusal(not_under_line(id), NOT_UNDER_FIX, usage)` where it drives it, else not found.
  The whole list is refused before anything moves.
- **E2** `busy`: facts say running, asking, or waiting on something. `settled`: `settled_now(id, settle_ms)` with
  `SETTLE_MS[prefs.settleAfter]`. `finished`: completed or interrupted. `titled`: `{ threadId, title (facts' else
  thread_word) }`.

## `replaceable(thread, origin)` (238-243)

- **E3** After E1: the thread or anything under it busy: `refusal(replaces_working_line(t), replaces_working_fix(t),
  usage)`; a restart already standing: `refusal(replaced_already_line(t, r), replaced_already_fix(r), usage)`.

## `settle(ids, origin, { finished, processes })` (1112-1143)

- **E4** Each distinct id in order. `finished`: the finished, unsettled threads under it (not itself) are taken,
  except those whose own tree is busy, which are left `SETTLE_WORKING`. Otherwise: a busy tree leaves the id
  `SETTLE_WORKING`; else an already settled id is left `SETTLE_ALREADY` and its unsettled tree is taken.
- **E5** An id that took any thread is named. With `processes`, `activity.meet(take, processes, "settle", origin)`
  first; its ends ride the answer only where any.
- **E6** The stamp is `max(now, last + 1)`, kept in memory. Taken threads get `readAt` and `settledAt`; named ids
  get `settleNamedAt`; one `mark` for all, only where anything was taken.
- **E7** Answers `{ settled: titled(taken), left, ended? }`.

## `mark(ids, marks)` (1145-1158)

- **E8** `pinned` or `order` that is a number but not finite: `usage_refusal("a thread's place in the sidebar is a
  number, and <n> is not one.", "Send a finite number, or leave the key out.")`.
- **E9** `resumeAtReset` arms the resume first. Stamps: `pinnedAt` (a number as given, true now, false off),
  `order` (null off), `foldedAt` (true now, false off), `snoozedUntil` (null off; a time also marks read now),
  `section` (null off). A mark with only `resumeAtReset` writes no stamps.

## `restore(ids)` (1160-1173)

- **E10** Each id with the threads under it whose `settledAt` equals the id's `max(settledAt, settleNamedAt)`
  (only where that is above 0), deduplicated, then only those settled now. They take `readAt: now` and lose
  `settledAt`. Answers `{ restored: titled }`.

## Tests (`tests/sessions_settle.rs`)

| Rule | Test |
|---|---|
| E1 | `a_subagents_id_refuses_the_whole_list`, `a_thread_not_under_the_caller_refuses_and_a_stranger_is_not_found` (settle-tree.test.ts) |
| E3 | `a_restart_of_a_working_tree_is_refused`, `a_thread_restarted_once_is_refused` (replaces.test.ts) |
| E4-E7 | `settle_takes_the_tree_and_names_the_lead` (settle-tree.test.ts), `settle_leaves_a_working_tree`, `settle_finished_takes_only_finished_children`, `settle_with_processes_meets_them_first` (activity.test.ts:225) |
| E6 | `two_settles_in_one_ms_get_two_stamps` |
| E8, E9 | `a_sidebar_place_that_is_no_number_is_refused`, `pin_fold_snooze_and_section_stamps` (thread-marks.test.ts) |
| E10 | `restore_takes_back_what_one_settle_took` (settle-tree.test.ts) |
