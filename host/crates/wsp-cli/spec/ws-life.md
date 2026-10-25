# ws_life: renaming, waking, commits, fixes, merges, starts, reviews, stops, forgets, settles

Source: `packages/host/src/verbs/workspaces-help.ts` at 65f6b89c7, lines 311-598. Rust:
`src/verbs/workspaces_help/life.rs`. Recorded in `tests/fixtures/cli_c/ws_life.json` (386 cases, probe
`record/ws_life.probe.ts`), drivers `tests/cli_c/drivers_ws_life.rs`, hand tests `tests/cli_c/ws_life_rules.rs`.

## Scope

`renameWorkspace` 319, `renamedWorkspaceLine` 325, `Woken` 331, `awake` 347, `noDraftLine` 360, `NO_DRAFT_FIX` 361,
`committed` 365, `askedToFix` 378, `mergedIn` 387, `fixLine` 394, `startedFrom` 402, `reviewStarted` 409,
`reviewPosted` 416, `startedLines` 423, `reviewLines` 428, `threadLabel` 433, `threadCwd` 437, `folderOf` 443,
`withShared` 446, `sharedOf` 449, `VERDICTS` 452, `mergedPr` 455, `updateLine` 460, `napAfterDeadLaunch` 469,
`withLine` 494, `Stopped` 501, `stop` 517, `stopLine` 525, `forgetThread` 537, `threadForgotLine` 543, `settleThreads`
549, `restoreThreads` 554, `settledLines` 559, `restoredLines` 565, `Renamed` 571, `rename` 581, `renameLine` 588.
`STOP_WORDS` (523) is private and reached through `stopLine`. Not here: `onDelete` and `madeWorktree` (602, `ws_rows`).

## Behaviour

- `renameWorkspace`: `workspaceOf` then `workspaces.rename {workspaceId, name}`; answers the old name and the record as the
  host wrote it. A blank or taken name is the runtime's refusal; nothing is checked here.
- `awake`: the state is the phase alone. Gone: plain error `goneRefusal(name, action, gone)`, no frame, nothing said. Released
  with action `send`: the workspace as given, `woke` false, no frame. Otherwise, unless running, `waking <name>` is told;
  `workspaces.wake {workspaceId}` is asked in every state (the runtime alone sees a provider-side pause). `woke` is true
  only when the state was paused or pausing and the record answered reads `running`. From those two states the record's
  `diskGone` is told too and kept as `said` (an empty one is told and kept). The workspace answered is the wake's record.
- `committed`: a message (an empty one too) goes straight to `workspaces.commit {workspaceId, message, paths?, threadId?}`.
  With none, `workspaces.commitDraft {workspaceId, paths?, threadId?}` first; a null message is a usage refusal of
  `noDraftLine(note)` with `NO_DRAFT_FIX`; any other draft is said and used. The reply is parsed (`oid`, `subject`,
  `filesChanged`, `insertions`, `deletions`).
- `askedToFix`: `workspaces.fix {workspaceId, check?, child?, childThreadId?, threadId}` in that order; an unreadable answer
  is the plain error `otherVersion("workspaces.fix")`. `fixLine`: `updated` is the nothing-to-fix line; otherwise the line
  by child, else check, else conflicts, with the agent's name; a `held` answer with `capped` adds a line.
- `mergedIn`: `awake(lead, "merge in")` then `workspaces.mergeIn {workspaceId (the woken record's), child, threadId,
  childThreadId}`. A gone lead fails before any frame.
- `startedFrom`: the access word is checked first (`ask`, `auto-edit`, `full`, `plan` only); `workspaces.start` carries the
  fields given in the order url, project, agent, model, effort, access. `reviewStarted`: `workspaces.review`, order url,
  workspaceId, agent, model, effort. `reviewPosted`: a verdict or summary (an empty summary counts) sends
  `workspaces.reviewDraft` first; a refusal of it ends the call; then `workspaces.reviewPost`.
- `startedLines`, `reviewLines`, `threadLabel`, `withShared`, `sharedOf`, `VERDICTS`, `updateLine`, `settledLines`,
  `restoredLines`, `stopLine`, `renameLine`, `threadForgotLine`, `renamedWorkspaceLine`: pure lines, pinned by matrices.
- `threadCwd`: the turn's folder, absent when the worktree is gone and the folder is the worktree's path or inside it.
  `folderOf`: the folder (else the project path) shortened by the home folder; `home` is a parameter here.
- `mergedPr`: `workspaces.merge {workspaceId, method?, whenChecksPass? (only true), threadId}`, parsed.
- `napAfterDeadLaunch`: nothing when this call did not wake the machine or the turn's result names a refusal. Else
  `sessions.list {workspaceId}`; nothing when the turn's thread did work. Another thread running: `status.list` and the
  stays-awake line, with minutes to `idleAt` (floored at zero) where the entry carries one. Else `workspaces.nap` and
  the asleep-again line. Any failure after the first checks is the stays-awake line with no countdown; it never throws.
- `withLine`: a new error with `\n<line>` under the message and only the `kind` carried over. No line, the error as it was.
- `stop`: `threadOf`, `sessions.interrupt {sessionId, task?}` (the thread's latest turn). The answer drops an empty `under`.
- `forgetThread`: a row with no thread id is refused (plain error) before any frame; else `sessions.forget {threadId,
  check?: true}`. `settleThreads`, `restoreThreads`, `rename`: `sessions.settle` and `sessions.restore` send each row's `id`,
  `sessions.rename` the session id; `error` rides a rename answer whatever the outcome.

## Limits and caps

Minutes of the countdown: 0m, 1m, 20m, 59m, `1h 0m`, `23h 59m`, `1d 0h`, `2d 1h` (hand test, clock read). Ids of 3, 8, 9
and 12 characters through `threadWord` (`threadLabel`, 8 kept). One thread and two under a stopped one (plural word).
`threadCwd` with a sibling folder sharing a prefix, and a worktree path with a trailing slash.

## Null and empty inputs

Empty message, empty summary, empty check, empty note, empty title, empty disk line, empty action, empty access, empty
picks, no threads to settle or restore, an empty `under`, empty `shared`, no turn and no listing for `napAfterDeadLaunch`,
no line for `withLine`, an empty folder.

## Order

Frames: draft before commit; wake before merge-in; resolve before rename; thread read before stop; draft edit before
post; thread list before status list before nap. Lines: `waking <name>` before the disk line. Settle lines: settled first, then left.

## Invariants held

Host invariant 7: no word of this unit moves with the cloud setting (`invariant_7_the_cloud_setting_moves_no_word_of_ws_life`).
The guest rule is not reached: nothing here reads this computer's disk (`folderOf` takes the home it is given and reads no
file) or resolves a folder. The exit class is read off the kind stamped on an error: `with_line_keeps_the_exit_class_...`.

## What the TS leaves unclear

- The refusals of `awake` and `rebuild` for a gone or a not-gone workspace are plain errors, so they exit as the provider's class.
- `withLine` loses the structured `fix`, `errors`, `left` and key of an error; only the message and kind survive.
- `stopLine` ignores the outcome, the tree and `left` once an `error` stands.
- `commit`, `merge`, `review post`, `settle`, `restore`, `rename` and `stop` parse their answers with zod `parse`, so an
  unreadable one throws zod's text; `askedToFix` says `otherVersion`. Not recorded.
- `awake` tells the disk line even when the wake did not bring the machine up.
- `fixLine`'s `child` argument is the child's label; `asked.child` is a workspace id.

## Rule -> test table

| Rule | Test |
|---|---|
| frames, params and order of each helper | the recorded case of each helper |
| a wake belongs to the caller that took it off its nap | `ts_a_wake_belongs_to_the_caller_only_from_down_to_running` (`verbs-turns.test.ts`) |
| waking is said on every state but running | `ts_it_says_waking_on_every_state_that_is_not_running` |
| gone is refused before any frame | `awake_gone_is_refused_before_any_frame` |
| released send asks nothing | `awake_released_send_asks_nothing_but_any_other_action_wakes` |
| the disk line only from a down state | `awake_tells_the_disk_line_only_from_a_down_state` |
| nap where nothing else runs | `ts_a_dead_launch_naps_the_machine_where_nothing_else_runs` |
| no countdown, no host | `ts_a_dead_launch_with_no_nap_time_or_no_host_says_stays_awake` |
| a countdown from `idleAt` | `ts_a_dead_launch_says_when_the_idle_window_takes_the_machine` |
| nothing owed to a wake not made | `nap_after_dead_launch_owes_nothing_to_a_wake_it_did_not_make_or_a_turn_that_ran` |
| the kind survives `withLine` | `with_line_keeps_the_exit_class_of_the_error_it_adds_to` |
| the id each op takes | `threads_are_named_to_the_host_by_the_id_each_op_takes` |
| refusals before the host send no later frame | `refusals_before_the_host_send_no_later_frame`, `forget_thread_refuses_a_row_without_a_thread_id_before_any_frame` |
| caps and boundaries | `threadCwd` cases, `threadLabel / an id of exactly 8`, the countdown table |
| null and empty inputs | the empty cases named above |
| both cloud settings | `invariant_7_the_cloud_setting_moves_no_word_of_ws_life` and every `(cloud)` twin |
