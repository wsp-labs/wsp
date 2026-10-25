# workspaces_phases

Source: `packages/runtime/src/projects/workspaces.ts:22-65, 289-645` at 65f6b89c7. Tests: `pause`, `box-nap`,
`wake-asks-again`, `disk-gone`, `gone`, `reconcile`, `resize`, `rebuild`, `rename`, `delete` cases of
`packages/runtime/test/` (named per test), ported at the area's seam with a fake `WorkspacesCtx` and a fake clock.

## What it does

A workspace's machine moves: nap, wake (asking the provider again on its cadence, a deleted disk replaced or a
sub-thread released, a stop the person pulls), resize, upgrade, rebuild, and the record's own changes (rename, look),
the daemon (update, restart), delete and forget.

## Behaviour

1. `RESIZE_NAP_MS` = 1800000 (:24).
2. `disk_gone(entry, e)` (:34-65): warn `wake of <id>: <machine> cannot resume: <said>`. A sub-thread: `line` =
   `diskGoneReleasedLine(message)`; `unfork` (a failure warned `wake of <id>: <machine> was not deleted: <why>`);
   `record.diskGone = line`; `subLost(entry, line)`; `tellThreads(id, line)`; fail kind conflict with `line`. Else
   `old` = machine id; `ws.rebuild()`; a failure: `phase` = `ws.currentPhase`, persist, `words` = `<message>, and no new
   machine started: <why>`, `emitStatus(entry, "napping", words)`, fail no kind with `words`. Then `line` =
   `diskGoneReplacedLine(message)`; `followMachine`; drop `wakeRefused`; `diskGone = line`; persist; emit
   `{type: "workspace.upgraded", workspaceId, machineId}`; warn `wake of <id>: <old> replaced by <new>`;
   `tellThreads`; `emitStatus(entry, reachOf(entry), line)`; answer the view.
3. `nap(id, origin)` (:289-293): `spawnGuard("pause")`; `refusePauseless(entryOf(id, origin), "be paused")`;
   `napWith(id)`.
4. `wake(id, origin)` (:295-412), in order:
   1. `talksToTreeOn(id, origin)`: `wake(id)` as the person, answered as `WorkspaceTalked`.
   2. `entryOf(id, origin, {act: "wake"})`; `refuseSubWake`; `copyBlocked`; a wake in flight: join it.
   3. Phase gone: `recoverGone(entry)`; none: no kind `goneRefusal(name, "wake", gone)`; `running`: the view.
   4. A nap in flight is awaited, its failure dropped.
   5. Phase running: `machine.state()`; a failure that is not missing reads `running`; a missing one reads `gone` with
      `answer` = `providerSaid(e)`; gone and `settled(settleGone(entry, goneWords(machineId, {by: "wake", at: now,
      answer?})))`: `goneRefusal`; `paused`: `adoptPause(entry)`. Not running: `runsUnderNapping(entry)` true:
      `adoptRunning(entry)`.
   6. A wake begun meanwhile: join it. Running: `proven(entry)` where `unchecked`, else the view.
   7. `refusePauseless(entry, "be woken")`.
   8. The wake, stored as `entry.waking` (joined by later callers) with a stop token as `entry.wakeStop`: `began`;
      `wakeAsks` = 1, or `wakeAsksIn(resumeAsks.forMs, everyMs)` where the backend's lifecycle names resume asks.
      Drop `deleteSaid`; phase `waking`; persist; `emitStatus(entry, "napping")`; `landed` = false; for ask 1, 2, ...:
      `ws.wake({landed})`; stopped meanwhile: fail `WAKE_STOPPED`; else `followMachine`, drop `wakeRefused`, persist,
      emit `{type: "workspace.woken", workspaceId, machineId}`, warn `wake of <id>: <reason>` where there is one,
      `emitStatus(entry, reachOf, reason)`, answer the view.
      On a failure: not stopped, a `ResumeUnansweredError` and `ask < wakeAsks`: `wakeAsk = {ask, of}`, drop
      `wakeSaid`, `emitStatus(entry, "napping")`, sleep `max(0, began + ask * everyMs - now)` raced against the stop;
      not stopped: `landed` = `tookTheResume(readsState(machine))`, next ask. Then not stopped and a `DiskGoneError`:
      `disk_gone`. Not stopped, missing and settled gone: `goneRefusal`. `gaveUp` = `wakeGaveUpLine(ask, now - began)`
      for an unanswered resume, written as `wakeRefused`; stopped: `ws.notePaused()`; phase = `ws.currentPhase`;
      persist; drop `wakeAsk`; `unfinished` = `noCommandsYetLine(name, message)` for a `RestoreUnfinishedError`;
      `words` = `WAKE_STOPPED` when stopped, else gaveUp, unfinished or the message; warn `wake of <id> failed: <words>`
      unless stopped; `emitStatus(entry, "napping", words)`; `armLateRead(entry)`; fail with `WAKE_STOPPED`, gaveUp,
      unfinished, or the error itself.
      Finally: drop `wakeStop`, `wakeSaid`, `wakeAsk`, `waking`.
5. `resize(id, size, origin)` (:414-463): `entryOf(.., {act: "wake"})`; `spawnGuard("resize")`; `refusePauseless(..,
   "be moved to another size")`; `refuseSubWake`; `at` = `backendFor(record)`; a machine with no `resumeAt`: kind invalid
   `refusalLine(resizeNoMoveLine(providerKeyName(provider ?? "the provider")), RESIZE_NO_MOVE_FIX)`; `at.limits()`
   (failure ignored); no offered size equal: kind invalid `refusalLine(sizeRefusal(sizeWord(size), sizes),
   SIZE_PICK_FIX)`; the same size: the view; any thread of the workspace running: kind conflict
   `refusalLine(resizeBusyLine(name), RESIZE_BUSY_FIX)`. `napWaitMs` = `RESIZE_NAP_MS` around `napWith(id)` (cleared
   in finally). `resizeTo` = size; record size takes cpu and memMb; drop `shape`. `follow(asked)`: `shape` =
   `shapeOf(machine)` (written where read); `landed` = `sizeBuilt(shape, asked)`; size = landed; a sub-thread's rate
   from the offered size where found; persist. `wake(id, origin)`; a failure: `follow(before)` and the failure
   propagates; finally drop `resizeTo`. Then `follow(size)`; `statusNow(entry)`; landed not the asked size: kind
   conflict `refusalLine(resizeLandedLine(name, landed, size), RESIZE_AGAIN_FIX)`; the view.
6. `stop_wake(id, origin)` (:465-471): `entryOf`; pull the stop; await the wake, failure dropped; the view.
7. `upgrade(id, origin)` (:473-481): `refuseCannot(entry, "replacesMachine", "have its machine replaced")`;
   `ws.upgrade()`; `followMachine`; persist; emit `workspace.upgraded`; the view.
8. `rebuild(id, origin)` (:483-527): `refuseCannot(.., "be rebuilt")`; a wake in flight awaited (failure dropped);
   `recoverGone(entry)` answering: the view. `keeps` = `keepsImages(at)` and `pauseKeepsDisk(at)`; keeps and not
   running: `wake(id, origin)`, a failure: kind conflict `<name> could not be woken to back up before the rebuild:
   <why>`; a machine id changed by that wake (a disk replaced): the view. `fromRunning` = keeps and running;
   `napVault` = not fromRunning and a `vaults` blob for the id. fromRunning: `ws.upgrade()`, a failure warns `rebuild of
   <id>: the running machine gave no vault (<why>); replacing with none` and `ws.rebuild()`; else `ws.rebuild()`.
   `followMachine`; drop `wakeRefused`; persist; emit `workspace.upgraded`; `reason` = `rebuilt: <old> replaced by
   <new>` plus `, nap-time vault imported` or `, no vault to import` when not fromRunning; warn `rebuild of <id>:
   <reason>`; `emitStatus(entry, reachOf, reason)`; the view.
9. `rename(id, typed, origin)` (:529-540): `name` = `nameGiven(typed)`; unchanged: the view; `nameRefusal(name)`: kind
   conflict; record name; persist; store `workspace-names/<id>` = `{workspaceId, name, project}`; emit
   `{type: "workspace.renamed", workspaceId, name}`; the view.
10. `look(id, look, origin)` (:542-549): `putLook(record, "theme", theme)`, then glyph; persist; emit `{type:
    "workspace.look", workspaceId, theme: theme ?? null, glyph: glyph ?? null}`; the view.
11. `update_daemon(id, origin)` (:551-560): served by its computer: no kind `placeServesDaemonLine(name,
    computerOf(entry))`; no `deployDaemon` on the kind: no kind `this runtime cannot deploy a daemon; the host wires
    the bundle`; not running: no kind `wake <name> before updating its daemon`; `deployDaemonOn(entry, deploy)`.
12. `delete(id, origin)` (:562-612): `spawnGuard("delete")`; the person, an id not live, and a failed create removed:
    emit `workspace.deleted` and answer. `entryOf`; a delete in flight: join it. Queued: `deleteQueued(id)` answering
    a future: that. `takes` = a made, standing worktree on disk; takes: `refuseChanged(path)` (awaited before the join
    is set: rule 3.10's gap). The delete (stored as `deleting`): drop `deleteSaid`; `cutOff(entry)`;
    `endSessions(id, DELETED_REASON)`; `threads` = thread records of the workspace, then session rows' threads; a folder
    kind: `dropCheckpoints` per thread in order; takes: `removeWorktree(entry, false, {ending: true})`;
    `dropThreadFiles(entry, thread records' ids)`; `standing` = phase not released or queued and the sub-thread's cloud
    not gone; standing and not driven: `machine.kill()`, missing ignored; standing and driven: `unfork(entry)`, a
    `MachineAliveError` sets `deleteSaid = {phase, line: deleteRefusedLine(name, machineId, state)}`, warns the line and
    fails with its kind; any other error propagates; `drop(id)`. Finally drop `deleting`.
13. `forget(id, origin)` (:614-624): not driven: kind conflict `forgetUndrivenRefusal(name, machineWord(kind))`;
    `readGone(backend, machine id)` not gone: kind conflict `<name>'s machine <id> is still <state>; pause it or
    delete it at the provider first`; `endSessions(id, DELETED_REASON)`; `drop(id)`.
14. `touch(id, origin)`: `entryOf`, then `idle.touch(id)`.
15. `restart_daemon(id, origin)` (:631-645): no `restartDaemon` on the kind: no kind `<name>'s daemon runs on
    <machineWord(kind)>, which this host does not hold the process of`; run it; finally, a shared daemon:
    `sharedDaemonReplaced()`; then forget `polledReach[id]` and `pushStatus(entry)`.

## Edge cases

- A wake with no resume asks named asks once; a stop between asks answers `WAKE_STOPPED` at once.
- `delete` of a failed create by a thread goes through `entryOf` (and so refuses) rather than the failed list.
- `rename` to the same typed name after `nameGiven` writes nothing and emits nothing.

## Behaviour nobody wrote down

- Rule 3.9, join: `waking` and `deleting` hand later callers the first call's future, its error cloned.
- Rule 3.10: two deletes of a worktree workspace can both pass `refuseChanged` before either sets `deleting`; kept.
- Rule 3.15: the wake's cadence sleep is on the clock (`sleeps`).

## Decisions

1. The stop is a `CancellationToken`; `orStopped` races the sleep against `cancelled()`.

## After the port

- `rebuild` reads `napVault` before the rebuild and reports it whether or not the rebuild imported it (:506). Kept.

## Rules against tests

| Rule | Tests |
|---|---|
| 2 | `disk_gone_releases_a_sub_thread`, `disk_gone_rebuilds_from_the_image_and_says_replaced`, `disk_gone_with_no_new_machine_keeps_the_phase_and_says_why` |
| 3 | `nap_refuses_a_pauseless_kind_and_naps` |
| 4.1-4.7 | `wake_of_a_tree_workspace_answers_talked_fields`, `wake_joins_a_wake_in_flight`, `wake_of_gone_refuses_unless_recovered`, `wake_of_running_reads_the_machine_and_settles_gone`, `wake_of_running_adopts_a_provider_pause`, `wake_of_running_proves_an_unchecked_machine` |
| 4.8 | `wake_resumes_persists_and_says_woken`, `wake_asks_again_on_the_cadence_from_its_start`, `wake_reads_the_machine_before_asking_again`, `wake_gives_up_after_every_ask_and_writes_wake_refused`, `wake_stopped_between_asks_answers_wake_stopped_and_notes_paused`, `wake_says_no_commands_yet_for_a_restore_unfinished`, `wake_clears_its_join_fields_however_it_ends` |
| 5 | `resize_refuses_in_the_ts_order`, `resize_to_the_same_size_does_nothing`, `resize_naps_with_the_long_wait_then_wakes_at_the_new_size`, `resize_that_landed_elsewhere_follows_the_machine_and_refuses`, `resize_whose_wake_fails_follows_the_old_size` |
| 6, 7 | `stop_wake_pulls_the_stop_and_waits`, `upgrade_follows_the_new_machine_and_says_upgraded` |
| 8 | `rebuild_of_a_napped_keeping_backend_wakes_first`, `rebuild_from_running_takes_a_vault_then_falls_back`, `rebuild_reports_whether_a_nap_vault_was_there` |
| 9, 10 | `rename_writes_the_record_and_the_name_row_and_emits`, `rename_refuses_a_name_taken`, `look_sets_theme_and_glyph_and_emits_nulls` |
| 11, 15 | `update_daemon_refuses_in_the_ts_order`, `restart_daemon_drops_the_polled_reach_and_pushes_status` |
| 12 | `delete_of_a_failed_create_emits_deleted`, `delete_joins_a_delete_in_flight`, `delete_of_a_queued_spawn_stops_it`, `delete_refuses_a_worktree_with_changes_before_anything`, `delete_takes_everything_away_in_the_ts_order`, `delete_refused_by_a_machine_alive_says_why_on_the_record` |
| 13, 14 | `forget_refuses_an_undriven_kind_and_a_machine_still_standing`, `touch_touches_the_idle_window` |
