# sessions_delete: `threads/sessions.ts:1398-1480`

`delete(thread, { processes, check })` and `forget(thread, { check })`.

## `delete` (1398-1451)

- **D1** After `ready`. The workspace (a row's, else the record's) is read first and `spawn_guard("delete_sub" where
  the caller leads the sub, else "delete")` runs before the absence check. None, or not reached:
  `not_found_refusal("no thread <word>")`.
- **D2** A sub-machine's thread: `sub_delete(entry, thread, origin, ask)` answers. A workspace that is no folder:
  `thread_on_machine_line(name)`, usage.
- **D3** The worktree goes with the last thread in it: a made, not gone worktree where every thread of the workspace
  (rows' and records') is this one. A turn running on the workspace then: `WORKTREE_BUSY_LINE`, conflict.
- **D4** A row of the thread running that is not a held send: `THREAD_WORKING_LINE`, conflict.
- **D5** `check`: with processes, `activity.meet([thread], stop as keep, ..., "delete")` (refusals only); answers
  `{ workspaceId, worktree?, threads: 1 }`.
- **D6** Processes met (`meet`) unless none asked. `keep` runs no group end and no shell end.
- **D7** With the worktree: the group ended `{ remove: true }`, the shells ended, the workspace deleted; answers
  `{ workspaceId, worktree, threads: 1, ended? }`.
- **D8** Otherwise: held sends given up and their launches awaited; a thread running now: `THREAD_WORKING_LINE`;
  kept agent reaped; group and shells ended; the transcript read (a transcript that does not read refuses here);
  checkpoints, thread files and sent images dropped; each row of the thread removed (`row_gone`); the record
  removed; the thread dropped from the transcript; rows persisted; `slates.forget(thread, { keep })`. Answers
  `{ workspaceId, threads: 1, ended? }`.

## `forget` (1453-1480)

- **D9** After `ready`. Absent or not reached: `not_found_refusal("no thread <word>")`.
- **D10** A thread whose rows ran (`thread_ran`) or whose record did not say it never worked (`worked` absent counts
  as worked): `thread_forget_refusal(thread)`, conflict.
- **D11** `check`: nothing more. Else: the transcript read (refuses here), files and sent images dropped, rows
  removed (`row_gone`), the record removed, `slates.forget(thread)`, dropped from the transcript, persisted.

## Tests (`tests/sessions_delete.rs`)

| Rule | Test |
|---|---|
| D1 | `a_thread_nobody_holds_is_not_found_after_the_guard_ran` (spawn-guard.test.ts) |
| D2 | `a_sub_threads_delete_is_the_subs`, `a_thread_on_a_machine_is_refused` |
| D3, D7 | `the_last_thread_takes_its_worktree` (folder-threads.test.ts), `a_worktree_with_a_turn_running_is_busy` |
| D4 | `a_working_thread_is_refused` |
| D5 | `check_reads_and_changes_nothing` |
| D6, D8 | `delete_drops_rows_record_transcript_and_slate` (forget.test.ts), `keep_leaves_the_processes_and_the_shells` (activity.test.ts:264) |
| D8 | `a_held_send_goes_with_the_thread` (owed-box.test.ts) |
| D9-D11 | `forget_takes_a_thread_that_never_worked` (forget.test.ts), `forget_refuses_a_thread_that_worked`, `forget_refuses_where_the_record_says_it_worked` |
