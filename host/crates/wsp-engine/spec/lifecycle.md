# lifecycle (lifecycle.ts, 205 lines)

One workspace's machine through naps, wakes, rebuilds and upgrades. Rust: `src/lifecycle.rs`. Every method takes
`&self` and the state sits in cells borrowed only between awaits (rule 3.3), so the runtime holds one `Rc<Workspace>`
and a hook reads the workspace while a wake runs, as the TS object is read mid-wake. `phase_cell()` is the phase as a
cell a reader can hold on its own.

## State, and what survives a restart

`machine`, `phase`, `first_life`, and the routes minted per port with the machine id they were minted under
(:43-50). All memory only: the runtime rebuilds a Workspace at start with `WorkspaceInitial` (phase `None` reads
running, `first_life` `None` reads true, :51-59), and the route cache starts empty.

## Moves

- `move` (:106): the move hook with the machine and the move where there is one, else the machine's own
  `pause(None)` / `resume(None)`. `retire` (:183): the retire hook, else `kill()`.
- `nap` (:99): napping already: nothing at all. Else `stash_vault(machine)` if set (a failure ends the nap there, no
  pause), then the pause move; the phase becomes napping **only after** the pause succeeded, so a failed pause leaves
  the phase as it was.
- `note_paused` / `note_running` (:112, :118): set the phase, nothing else.
- `wake(landed)` (:129): running already: `Ok(WakeResult { reason: None })`, nothing sent. A wake while `Waking`
  proceeds. Otherwise the phase becomes waking, then for attempt `1..=wake_attempts`:
  1. the resume move, except on attempt 1 when `landed`;
  2. clear the port routes (the map only; the machine id stays);
  3. `first_life = false`, before and whatever the check answers;
  4. the wake check (no check reads healthy): healthy ends the wake running with `reason` the faults so far joined by
     `"; "`, or `None` with none;
  5. a fault pushes `attempt <n>: <fault>`; before the last attempt the pause move runs, and a failed pause pushes
     `re-pause failed: <message>` and the loop goes on.
  After the last attempt it fails with `wake_failed_line(machine_id, faults.join("; "))` (wsp-words). **Any**
  failure out of the wake, a resume's or a check's own error included, leaves the phase napping and is answered as
  itself. Zero attempts fail at once with an empty fault list and no move. A wake never replaces the machine.
- `rebuild` (:166): no resurrect hook fails `rebuild requires a resurrect hook` with nothing changed. Else `replace`:
  resurrect with no spec (a failure leaves machine and life as they were); the new machine and `first_life = true`
  stand from here; `restore_vault(new)` if set; retire the old one, where a **missing** answer (`is_missing`) is done
  and any other failure is answered after the swap stood. Only a replace that finished sets the phase running.
- `upgrade(spec)` (:194): no resurrect hook fails `upgrade requires a resurrect hook`. `vault_export(machine)` if set
  (`None` is over the cap); retire the current machine (any failure answered, missing included, machine kept);
  resurrect with `spec`; `vault_import(new, payload)` only where both a payload and the hook are there; then running
  and first life.
- `checkpoint(name)` (:189): the machine's `snapshot(name, MachineLife { first_life }, default options)`; a backend's
  NotFirstLife comes through as itself.

## Routes

`port_reach(port)` (:85): where the cache's machine id is not the machine's, the cache resets under the new id; then
preview.rs's `refresh_preview_token(machine, port, cached)` and the answer is cached. `remint_port_reach` (:94) drops
that port (same machine only) and reaches again; other ports keep theirs. `daemon_reach` is `port_reach(7070)`.

## Rules against tests

| Rule | Tests |
|---|---|
| checkpoint passes the life; a refusal comes through | `lifecycle::checkpoint_hands_the_machine_the_life_it_tracked_true_before_a_wake_and_false_after`, `lifecycle::a_backends_refusal_of_a_resumed_machine_comes_through_checkpoint_as_itself` |
| a resume's own failure is answered as itself, napping kept | `lifecycle::a_wake_of_a_machine_that_vanished_while_paused_throws_the_providers_404_and_forks_nothing`, `lifecycle::a_resume_that_ends_with_the_guest_unusable_fails_the_wake_with_the_providers_words` |
| routes dropped at a wake, per port, remint one, voided by a new machine, stale remints | `lifecycle::the_wake_check_dials_a_route_minted_after_the_resume_not_the_one_cached_before_the_nap`, `lifecycle_hard::a_port_route_is_kept_per_port_and_a_remint_drops_only_that_port`, `lifecycle_routes::*` (7, preview.test.ts:61-144) |
| phase waking during the check | `lifecycle::holds_the_phase_until_the_check_passes_then_stays_on_the_same_machine`, `lifecycle_hard::a_reader_holding_the_phase_sees_waking_while_the_check_runs_and_running_after` |
| attempts, re-pause, faults in order | `lifecycle::a_check_that_fails_once_is_paused_and_resumed_again_and_a_passing_second_try_keeps_the_machine`, `lifecycle::two_failed_checks_fail_the_wake_with_both_faults_and_the_machine_stays`, `lifecycle::a_workspace_given_wake_attempts_1_never_re_pauses`, `lifecycle_hard::a_failed_re_pause_is_one_more_fault_and_the_wake_goes_on`, `lifecycle_hard::a_wake_with_zero_attempts_fails_at_once_with_no_faults_and_no_move` |
| landed skips only the first resume | `lifecycle_hard::a_landed_wake_sends_no_resume_on_its_first_attempt_but_does_on_its_second` |
| null moves: wake when running, nap when napping | `lifecycle_hard::a_wake_of_a_running_workspace_does_nothing_and_a_nap_of_a_napping_one_does_nothing` |
| nap order, phase follows facts | `lifecycle::nap_stashes_the_vault_before_the_pause`, `lifecycle::note_running_puts_a_napping_phase_back_to_running_so_the_next_nap_pauses_the_machine_for_real`, `lifecycle::a_failed_wake_with_no_resurrect_hook_throws_and_leaves_the_workspace_napping` |
| move hook for every move | `lifecycle::nap_and_wake_run_their_pause_and_resume_through_the_move_hook_with_the_machine`, `lifecycle::a_wakes_re_pause_goes_through_the_hook_too_and_a_move_the_hook_gives_up_on_leaves_the_phase_where_it_was` |
| rebuild order, missing old is done, other failure after the swap | `lifecycle::rebuild_replaces_a_running_machine_with_a_golden_fork_restores_the_vault_kills_the_old_one`, `lifecycle::a_napping_workspace_rebuilds_too_and_comes_back_running`, `lifecycle::rebuild_refuses_without_a_resurrect_hook_and_keeps_the_machine`, `lifecycle_hard::a_rebuild_whose_old_machine_is_already_gone_still_lands_and_any_other_retire_failure_is_thrown_after_the_swap` |
| upgrade order, cap skips import | `lifecycle_hard::an_upgrade_exports_retires_resurrects_imports_in_that_order_and_lands_running_first_life`, `lifecycle_hard::an_upgrade_over_the_vault_cap_goes_on_with_no_import` |

## Pins (wave 2)

Scenes in `oracle/lifecycle_scenes.oracle.ts`, replayed by `tests/lifecycle_scenes.rs::<name>` against
`tests/fixtures/scenes/lifecycle.json`: one log holds every machine call and hook in order, and after the act the
machine id, the phase, the first life and the failure's words must be the TS's.

- W-1 The wake check's answer is tested against `None` only: `Some("")` is a fault, pushed as `attempt 1: ` (with the
  trailing space), so the machine is paused and resumed again (`an_empty_fault_is_still_a_fault`).
- W-2 rebuild: once resurrect answered, the new machine and `first_life = true` stand even when `restore_vault` then
  fails; that failure is answered, the old machine is never retired, and the phase is not moved
  (`a_rebuild_whose_restore_fails_keeps_the_new_machine_and_leaves_the_old_alive`).
- W-3 upgrade: an export failure is answered with nothing changed and nothing retired
  (`an_upgrade_whose_export_fails_changes_nothing`).
- W-4 upgrade: a resurrect that fails after the retire leaves the workspace holding the machine it just killed, phase
  and first life unmoved (`an_upgrade_whose_resurrect_fails_after_the_retire_keeps_the_killed_machine`).
- W-5 upgrade: an import that fails leaves the new machine in place, but phase and first life move only after the
  import, so neither does (`an_upgrade_whose_import_fails_keeps_the_new_machine_and_the_old_phase`).
- W-6 An empty payload is a payload: `Some(vec![])` is imported (`an_empty_payload_is_still_imported`).
- W-7 A resume that fails is answered as itself at once, with no further attempt; first life ends only after a
  resume that answered, so it stays true; the phase goes back to napping
  (`a_failed_resume_keeps_the_first_life_and_naps`).
- W-8 A wake's re-pause is the pause move alone; only `nap` calls `stash_vault` (`a_wakes_re_pause_stashes_no_vault`).
- W-9 `checkpoint` reads no phase and moves none: a napping workspace snapshots with its life
  (`a_checkpoint_of_a_napping_workspace_snapshots_and_moves_nothing`).
- W-10 A wake that runs out fails with a plain error, `EngineError::Other` with no code, its words
  `wake_failed_line(id, faults)` (`a_wake_that_runs_out_fails_with_plain_words`).
- W-11 The route cache on a failed mint, from lifecycle.ts:85-97 (no fake here fails a mint, so no test): `port_reach`
  resets the cache under the new machine id **before** it mints, so a failed mint still leaves the cache reset;
  `remint_port_reach` drops the port **before** it mints, so a failed remint leaves that port with no route. rebuild
  and upgrade never clear the cache: only a machine id that differs voids it.
