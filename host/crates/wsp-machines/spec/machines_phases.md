# machines_phases
Source: `packages/runtime/src/machines/machines.ts:589-899` (about 310 of 906 lines) at `65f6b89c7`. Tests: integration
cases in `pause.test.ts`, `box-nap.test.ts`, `runtime-gone.test.ts`, `idle.test.ts` and `serve.test.ts` (under "Rules
against tests"), and 28 scenes in `oracle/machines.oracle.ts` ("machines: phases"), written to
`tests/fixtures/oracle/machines_phases.json`. Rust: `src/machines/machines/phases.rs`.

## What it does
The moves of a workspace between running, napping and gone as this host makes them or finds them made: the nap, a pause
the provider made on its own, a machine running under a record that says napping, the one road to gone with its
confirmation, the road back from gone, the idle policy's wiring, and the proof that a machine reading running takes
commands.

## Behaviour
1. `nap_with(id, reason)` (:592): `entry_of(id)`; a napping record over a running machine is adopted first
   (`runs_under_napping`, then `adopt_running`); a nap in flight is joined; a record not `running` answers its view.
   Else, as the joined `napping` future: `phase = pausing`, persist, `emit_status(entry, "napping")`, then `ws.nap()`.
   On success: `end_sessions(id, PAUSED_REASON)`, drop the nap and stop refusals, `phase = napping`, `nappedAt = now`,
   delete `diskGone` and `unchecked`, persist, emit `workspace.napped`, `emit_status(entry, "napping", reason)`, answer
   the view. On failure: a `missing` answer goes to `settle_gone` with `gone_words(machineId, {by: "pause", at: now,
   answer: provider_said(e)})`, and a settled or moot outcome rethrows the error at once; otherwise `phase =
   ws.current_phase`, persist, a `NapRefusedError` warns once per standing refusal (`the provider does not pause <m> of
   <id> (<said>); an idle nap waits for the backstop and exports no vault until a pause lands`) and sets
   `nap_refusals[id]`, a `StopRefusedError` sets `stop_refusals[id]`, `emit_status(entry, reach_of(entry), message)`,
   rethrow. The `napping` mark is cleared in every case.
2. `arm_late_read(entry)` (:640): cancels any armed read, then after `lateReadMs` (180,000) reads
   `runs_under_napping` and adopts a running machine; a failure warns `late read of <id>: <why>`.
3. `adopt_pause(entry)` (:651): only a `running` record with no nap or wake in flight: `ws.note_paused()`, drop the nap
   refusal, `phase = napping`, `nappedAt = now`, delete `diskGone` and `unchecked`, persist, `end_sessions(PAUSED_REASON)`,
   emit `workspace.napped` with `found: true`, `emit_status(entry, "napping", "paused outside wsp")`.
4. `runs_under_napping(entry)` (:667): `false` unless the record is `napping`; then the machine's `state()`, a failure
   reading `paused`; `true` only for `running`.
5. `adopt_running(entry)` (:671): joined on `entry.adopting`; only a `napping` record with no nap or wake in flight:
   `ws.note_running()`, `follow_machine`, `unchecked = true`, persist, emit `workspace.woken {workspaceId, machineId}`,
   `emit_status(entry, reach_of(entry), ALREADY_RUNNING)`.
6. `gone_confirmed(backend, machine)` (:691): waits `goneConfirmMs` when above 0, then `read_gone`; a failure is
   answered as the error.
7. `settle_gone(entry, reason)` (:700): a read not `gone` warns `workspace <id> is not gone: <reason>, and <the state
   read that followed said <state> | the reads that followed failed: <why>>` and answers `not-gone` or `unchecked`;
   `gone` goes to `mark_gone` with the record's machine id read **before** the wait.
8. `mark_gone(entry, machine, reason)` (:712): `moot` when the record is already gone, names another machine, is no
   longer the live entry, or is being deleted; a sub-thread record answers `sub_lost(entry, reason)`; else `phase =
   gone`, delete `unchecked`, `gone = reason`, persist, `end_sessions(GONE_REASON)`, warn `workspace <id> is gone:
   <reason>`, emit `workspace.gone {workspaceId, machineId, reason}`, `emit_status(entry, "gone", reason)`, `settled`.
9. `adopt_gone(entry, reason)` (:732): nothing for a gone record, a nap or wake in flight, or one being re-read (the
   `rereading` set: the second caller returns at once, 03 "drop"); else `settle_gone`, and an `unchecked` outcome
   `emit_status(entry, reach_of(entry), GONE_UNCHECKED)`. The mark is cleared in every case.
10. `recover_gone(entry)` (:745): only a `gone` record with no nap or wake in flight; the machine's `state()` (a
    failure: `None`); `phase_leaving_gone`: `running` gives `ws.note_running()` and `follow_machine` (which keeps
    `diskGone`), `paused` gives `ws.note_paused()`, `phase = napping`, delete `gone` and `diskGone`; persist; emit
    `workspace.woken` or `workspace.napped {found: true}`; `emit_status(entry, reach_of or "napping", NOT_GONE)`;
    answer the phase.
11. The `workspace.status` listener (:765): a status whose phase is `running` and machine state `paused`, on a live
    entry, starts `adopt_pause` (not awaited); nothing ends a session on a reach verdict.
12. `quiet_of(id)` (:777): `None` for no entry or a record not `running`; else the backend lifecycle's
    `quiet_for_ms(machine)`, a failure warned `quiet figure of <id> not read: <why>` and read as `None`.
13. The idle policy (:789): `window_of(id)` is `None` for no entry, else `idle_window_of(record)`, raised to
    `backstop_ms(window)` while a nap refusal stands (an off window stays off). `on_idle(id, window)`: a quiet figure
    under the window touches and ends; else `nap_with(id, IDLE_REASON.of(window))`; a `MoveUnansweredError` is
    rethrown (the policy retries after `retryMs`); a `missing` failure ends quietly; any other touches the window,
    `emit_status(entry, reach_of, nap_refused_reason ?? stop_refused_reason ?? message)`, and, unless it is a
    `NapRefusedError`, warns `idle nap of <id> was answered with <provider_said(e)>; a full <round(window / 60000)> min
    window starts over`. `busy(id)`: any running row on the workspace. `retryMs = opts.status.pollIntervalMs ??
    POLL_INTERVAL_MS`. `on_backstop(id, until)`: only a live `running` record without `sub.deleteAt`; the backend
    lifecycle's `backstop(machine, until)`, a failure warned `backstop of <id> on <machine> not set: <why>`. Touch on
    `session.start`, `session.delta`, `session.done`, `session.end`, `session.steer`, `inbox.file`, `workspace.woken`,
    `workspace.upgraded`, `project.import`, `project.export` and `workspace.created` (its `workspace.id`); forget on
    `workspace.napped`, `workspace.gone`, `workspace.deleted`.
14. `proven(entry)` (:853): joined on `entry.waking`. No `prove_road`: delete `unchecked`, answer the view. Else a new
    stop (`entry.wakeStop`); `still_live()` (await a delete in flight, then a record no longer the live entry throws
    `not_found_refusal(no_workspace_refusal(name))`); `phase = waking`, persist, `emit_status(entry, "napping")`; race
    the proof against the stop (a stop already fired is `stopped` at once); `still_live()`; a `missing` failure sets
    `phase = running`, settles gone with `{by: "wake"}`, `still_live()`, and a settled outcome throws
    `gone_refusal(name, "wake", record.gone)`; `proven` deletes `unchecked`; `said` is none when proven,
    `WAKE_STOPPED_UP` when stopped, `no_commands_yet_line(name, message)` for a `RestoreUnfinishedError`, else the
    message; `phase = running`, persist, `emit_status(entry, reach_of(entry), said)`; answer the view when proven, throw
    `said` for stopped or unfinished, else the error itself. `waking` is cleared, and `wakeStop` when it is still
    this one.

## Edge cases
- A nap's 404 that the reads confirm throws the pause's own error (`404 not found`), not a gone sentence.
- A delete mid-proof leaves the record reading `waking` and throws kind `not-found` (scene "a delete that took ...").
- `runs_under_napping` on a running record reads nothing.

## Behaviour nobody wrote down
- Rule 3.4: `nap_with` writes `pausing` and pushes before the provider is asked; `proven` re-checks liveness after
  every await before writing.
- Rule 3.9: `napping`, `adopting`, `waking` are joins; `rereading` is a drop.
- Restart: the record's phase, `nappedAt`, `gone` survive; nap and stop refusals, `rereading`, the late read and idle
  windows are memory only, re-armed by touches.

## Decisions
1. `wakeStop` is a `tokio_util::sync::CancellationToken` held on the entry; `stop.signal.aborted` is
   `is_cancelled()`.

## After the port
- A delete that lands during a proof leaves the dropped record's phase `waking` (it is no longer live, so nothing
  reads it).
- `nap_with` joins only after its own `entry_of` await, so two naps called in one tick each pass the guard before
  either sets `napping`; the second still joins because the first sets it synchronously after its await.

## Rules against tests
| Rule | Tests (`tests/machines_phases_scenes.rs` replays a scene by name; `tests/machines_phases.rs`) |
|---|---|
| 1 | scenes `nap_*` (7) |
| 2 | `a_late_read_after_a_given_up_wake_adopts_a_running_machine` |
| 3, 11 | scenes `adopt_pause_*` (2), `status_a_running_row_the_poll_reads_paused_is_adopted_as_a_pause` |
| 4, 5 | scenes `adopt_running_*`, `runs_under_napping_*` |
| 6-8 | scenes `settle_gone_*` (3), `mark_gone_*` (2), `the_gone_confirmation_waits_gone_confirm_ms_on_the_clock` |
| 9 | scene `adopt_gone_*` |
| 10 | scenes `recover_gone_*` (3) |
| 12, 13 | `idle_*` (7, from idle.test.ts and pause.test.ts: quiet resets, refusal backstop, move unanswered retried, missing quiet, busy holds, backstop pushed only for running, the touch and forget events) |
| 14 | scenes `proven_*` (7) |
