# machines_attach
Source: `packages/runtime/src/machines/machines.ts:270-301,364-587` (about 260 of 906 lines) at `65f6b89c7`. Tests:
integration cases in `pause.test.ts`, `box-nap.test.ts`, `runtime-comeback.test.ts`, `machine-gates.test.ts` and the
delete suites (under "Rules against tests"), and the scene "drop: ..." in `oracle/machines.oracle.ts`, written to
`tests/fixtures/oracle/machines_phases.json`. Rust: `src/machines/machines/attach.rs`.

## What it does
How a workspace record and its machine become a live entry: the machine wrapped so the provider's word that it cannot
reach or has stopped the machine reaches the record, the engine `Workspace` given its hooks (vaults, moves, the wake
check), the engine's three phases read off the record's seven, the end of every session on a machine that went, and
`drop`, which takes away everything a workspace left on this host.

## Behaviour
1. `watched(entry, machine)` (:274): `exec` and `run` go through the machine. A `MachineUnreachableError` while the
   entry still holds this machine sets `unreached[ws] = {machineId, line: message}`; when no mark of this machine stood
   before and the record is `running`, `emit_status(entry, "unreachable", message)`. A `MachineStoppedError` while the
   entry holds this machine runs `adopt_pause(entry)`. The error is rethrown. Any answer deletes a mark of this machine
   (with no push). Every other member is the machine's own.
2. `engine_phase_of(phase)` (:367): `running`, `napping`, `waking` map to themselves; `pausing`, `gone`, `queued`,
   `released` to `napping`.
3. `follow_machine(entry)` (:386): record `phase = running`, `machineId = ws.machine_id`, `firstLife =
   ws.is_first_life`, delete `gone` and the entry's `unchecked`, then start `sync_daemon(entry)` (not awaited).
4. `attach(record, machine)` (:395): a new entry with `generation` one past the live entry's (0 for the first), its
   machine `watched`. A kept pull request that is not `open` becomes `entry.pr` with `readAt = mergedAt ?? closedAt ??
   0`; a kept `checkout` is copied. When the backend keeps no images (`keeps_images` false) the record loses
   `vaultedAt` and `vaultRefused`, and the engine gets none of the four vault hooks. The engine `Workspace` gets:
   `goldenSnapshot: record.golden`; `wakeAttempts: at.lifecycle.budgets.wakeAttempts ?? 0`; `retire: gone.stop(at,
   m)`; `resurrect(override)`: `fork(record, m => entry.machine = watched(entry, m), override)`; with images, the
   vault hooks of rule 5; `move` of rule 6; `wakeCheck` of rule 7; initial `{phase: engine_phase_of(record.phase),
   firstLife: record.firstLife}`. The entry is set in `live` (replacing any) and returned.
5. Vault hooks (:427-472). `leave` is `{leave: LOGIN_PATHS}` when the record's place is at a provider, read at each
   call. `vaultExport(m)`: `vault_export(m, {maxBytes: vaultCapBytes, ...leave})`; a failure of kind `vaultTooLarge`
   warns `vault for <id> not taken before the replacement: <why>; replacing with no backup` and answers `None`; any
   other is rethrown. `vaultImport(m, payload)`: `import_into(m, payload, "/", leave)`. `stashVault(m)`: nothing while
   a nap refusal stands; `sync_disk(entry.machine)` with a failure warned (`disk sync before the nap of <id> failed:
   <answer or message>; napping anyway`); nothing more when the backend's pause keeps the disk; else put the blob
   `vaults/<id>`, set `vaultedAt = ISO(now)` and delete `vaultRefused`; a failure sets `vaultRefused = <why>` and warns
   `nap vault for <id> not stored, previous kept: <why>`. `restoreVault(m)`: the blob, imported where present.
6. `move(m, move)` (:478): `resume` calls `m.resume_at(entry.resizeTo, wakeStop.signal)` when a resize is asked and the
   machine has `resume_at`, else `m.resume(wakeStop.signal)`; on success, unless the backend names the workspace,
   `set_hostname(m, record.name)`; a `ResumeUnansweredError` first runs `says_waking(entry, RESUME_UNANSWERED)`, then
   rethrows. `pause` calls `m.pause({waitMs: entry.napWaitMs})` when set, else `m.pause(None)`.
7. `wakeCheck(m)` (:490): with a recorded `shape` and a machine that describes itself: a describe failure answers
   `provider view of <id> unavailable (<why>)`; `both = "created as <json(expected)>, provider view <json(actual)>"`;
   a `shape_fault` warns `wake check on <id>: <fault>; <both>` and answers `<fault> on <id> (<both>)`. Then
   `fault = ping_daemon(entry)`: answered as `<fault> (<both>)` when both are set, else as it is (`None` passes).
8. `idle_window_of(r)` (:517): `setting_for(r.idleWindowMs, settings_at(place_id_of(r)).napMs, defaultIdleWindowMs)`;
   `null` (off) is a value, not absent.
9. `end_sessions(ws, reason)` (:520): `end(reason)` of every session row on the workspace in map order, then every exec.
10. `drop(id)` (:526), in this order (scene "drop"): the entry's late read cancelled; the kind module's `dropped(entry)`
    (a failure warns `<name>'s machine <id> kept something of this host's: <why>`); `live` loses it; a folder kind
    rewrites the daemon roots from the entry; the maps `revived_at`, `unread_at`, `unreached`, `polled_reach`,
    `nap_refusals`, the transcript maps and `daemon_notes` lose it; the threads it drops are the thread records on it
    and every session row's thread on it; the session rows on it go; each kept agent on it is reaped; its thread
    records go; each dropped thread's slate is forgotten; a sub-thread record deletes each dropped thread's
    `sub-sessions` blob; `viewed_marks` loses it; `live.get(id)?.prPoll()` (see After the port); `cancel_flush`; the
    transcript queue and index flush are awaited and removed; store deletes in this order: `workspaces`, blob
    `transcripts`, `rows.clear`, `unread_indexes`, blobs `transcript-heads`, `transcript-index`,
    `drop_sent_images(id, "all")`, `kept_writes`, `workspace-names`, `sessions`, `creates/workspace/<id>`, blob
    `vaults`; then, when no live entry is on the same machine, every `agent-lists` key starting `<machineId>:`; last
    `workspace.deleted`.

## Edge cases
- `attach` over a live entry replaces it; anything still holding the old entry sees `live.get(id) != entry`.
- A `MachineUnreachableError` from a machine the entry no longer holds changes nothing.

## Behaviour nobody wrote down
- Rule 3.3: `emit_status`, `adopt_pause`, `fork`, `says_waking`, `ping_daemon` and every store call are calls out.
- Restart: the record and the `vaults` blob survive; `unreached`, `generation` and the engine phase are rebuilt by
  `attach` at hydration.

## Decisions
1. The engine `Workspace` is `wsp_engine::lifecycle::Workspace` with `WorkspaceHooks`; the hooks capture an `Rc` of
   the context and the entry id, never a borrow.

## After the port
- `drop` reads `live.get(id)?.prPoll` after `live.delete(id)`, so a workspace's pull-request poll is never cancelled
  by its drop (the scene's log has no `prPoll cancelled`).

## Rules against tests
| Rule | Tests (`tests/machines_attach.rs`) |
|---|---|
| 1 | `an_unreachable_answer_marks_the_record_once_and_any_answer_clears_it`, `a_stopped_answer_adopts_the_pause`, `a_replaced_machines_errors_mark_nothing` |
| 2 | `engine_phase_of_maps_every_phase` |
| 3 | `follow_machine_moves_the_record_onto_the_engines_machine` |
| 4 | `attach_counts_generations_and_keeps_a_closed_pull_request`, `a_computer_that_keeps_no_image_gets_no_vault_hooks_and_loses_its_vault_marks` |
| 5 | `a_vault_over_the_cap_replaces_with_no_backup`, `stash_vault_skips_while_a_refusal_stands_and_a_disk_keeping_pause`, `a_stash_that_fails_marks_vault_refused` |
| 6 | `a_resume_sets_the_hostname_again_and_an_unanswered_one_says_waking` |
| 7 | `the_wake_check_names_both_shapes_and_the_daemons_fault` |
| 8 | `idle_window_of_takes_the_record_then_the_place_then_the_default` |
| 9, 10 | scene `drop_everything_the_workspace_left_goes_in_order_and_the_event_last`, `drop_keeps_the_agent_lists_while_another_record_shares_the_machine` |
