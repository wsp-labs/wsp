# tools_loop (golden-tools.ts:237-345, 374-490)

The one tools loop: every install as one guarded run in plan order, the disk read before each against the floor, the
cleanup once at the first reading under it, the checks after the loop, Homebrew's housekeeping and the closing line.
`install_tools_at(machine, tools, on_stage, at, opts)` is `installTools`; `install_tools` is it at `installing-tools`
answering the rows. Every stage line goes out as `(at, detail)`. Nothing survives a restart; the next run's `present`
read is what knows a step stands.

## Before the loop (:380-411)

`path` = `opts.path` or `TOOLS_PATH`; `keep` = `opts.floor` or `TOOLS_DISK_FLOOR`; `sweep()` is nothing when
`keep_caches`, else `sweep_caches(machine, path)`. First call, always (no tools, kept caches alike): exec `rm -f
/tmp/wsp-vault-*.tgz`, timeout 20,000. A step's run is `machine.run(guarded_road(manager, cmd), deadline
guard_deadline_ms(road_limit_s(manager)), on_line: stage("<label>: <line>"))`. `landed(row)` pushes the row, then
awaits `on_tool` with it (a clone: see the open point) before the loop goes on.

## Each step, in order (:412-484)

1. In `present`: installed, note `already on the machine`, no command run, no disk read; it counts as installed.
2. `after` set and not installed: skipped, note `<label of the step with that id, else the id> did not install`.
3. The floor already hit: skipped with the floor note.
4. `free` = the reading the last install left, else `free_bytes(machine, prefix)` (a rejected exec ends the loop with
   its `Err`); the kept reading is used once.
5. Unknown and not warned yet: one stage line `free disk unknown (<reason>); installing without the <fmt keep> floor`,
   once per loop; the step goes on. Free under `keep`: the cleanup (below) with `<fmt free> free, under the <fmt keep>
   floor`. If the cleanup already ran, or reads unknown, or is still under: the floor is hit with `floor_note(words)`,
   words `<fmt free> free` (already ran), `<fmt after> free after cleanup`, or `<fmt free> free before cleanup and
   unknown after (<reason>)`; `floor_note(r)` = `<r>, keeping <fmt keep> free`. This step and every later one that
   reaches step 3 is skipped with it. Else `free` is the reading after.
6. `bytes` set and `free - bytes < keep`: the cleanup with `<label> needs about <fmt bytes> with <fmt free> free above
   a <fmt keep> floor`; a free reading after with room takes over `free`. Still no room: skipped with
   `floor_note("needs about <fmt bytes>, <fmt free> free")`, alone: the floor is not hit and this reading is kept for
   the next step.
7. Stage `<label> (<i+1>/<n>)`; run. Exit not 0 with `has already locked` in stderr: stage `<label>: another brew holds
   its cellar; waiting for it, then once more`, one unguarded run of the lock wait (`for l in <BREW_PREFIX>/var/
   homebrew/locks/*.lock; do [ -e "$l" ] && flock -w 600 "$l" true; done; true`, deadline 660,000, no `on_line`),
   then the run again.
8. Exit 124 on a road whose `road_step(manager).retry` is true: stage `<label>: <step_retry_line(limit)>`, run once
   more; 124 again is two timeouts. A road without retry is never run again.
9. Exit 0: installed (`tools/homebrew` sets the outcome's `homebrew` to `HOMEBREW_TAG`/`HOMEBREW_COMMIT`); `road =
   road_of(stdout)`; read `free_bytes(prefix)` and keep it for the next step; `bytes = max(0, free - left)` when both
   readings are free, else absent; the row carries the step's `note` where it has one, `ms`, `bytes`, `road`.
   Else failed with `timed_out_line(limit, 2)` for two timeouts, else `reason_of(res, limit)`, and `ms`.

The cleanup (once a loop; a second ask answers nothing): stage `<why>; cleaning up before skipping`; Homebrew's
housekeeping when `tools/homebrew` installed in this loop; `sweep()`; `free_bytes(prefix)`; stage `closing(brew,
swept, "<fmt> free" or the unknown reason)`.

## After the loop (:485-489)

1. The PATH check: the installed rows whose step has `bin`, in row order; none asks nothing. `missing_commands(bins,
   path, prefix)`; when it failed, stage `the PATH check failed (<why>): <bins joined ", "> count as failed` and every
   named row fails `<bin> could not be checked on PATH: <why>`; else a missing bin's row fails `<bin> is not on PATH
   after the install`. Either way the failed row loses its `road`.
2. The checks: the rows still installed whose step has `check`, through `paged_reads` with the head
   `path_line(path, prefix)` and per row `if ! out="$( ( <check> ) 2>&1 )"; then printf 'wsp-check %s %s\n' <its index
   in the page> "$(printf '%s' "$out" | tail -1)"; fi`. A page whose exec did not exit 0: stage `the checks could not
   be run (<reason_of(res, 20)>): <name_list(labels)> count as failed`, each of its rows fails `the check could not be
   run: <why>` with a stage line `<label>: its check was <check>`; other pages stand. Else `markers_of(stdout,
   "wsp-check")` by index: an empty mark fails `<label> is not there after the install`, any other its words; stage
   `<label>: the check did not pass: <check>`. A check failure keeps the row's `road`.
3. Housekeeping when `tools/homebrew` installed: `free_bytes(prefix)`, then autoremove and `cleanup -s --prune=all`
   (`brew_housekeeping(path)`) each through the guarded brew run labelled `Homebrew cleanup`, then `free_bytes`.
   Any failure: `Homebrew cleanup failed (<reasons joined "; ">)`; more free after: `Homebrew cleanup freed <fmt>`.
4. One closing stage line: `closing(summary, sweep(), free_note(prefix))`, in that order of calls. The summary:
   `<n> installed`, with ` (<named>)` listing every installed row with a road or a note as `listed_name(label)`, then
   ` <road words>` and ` (<note>)`, joined `, `; then `, <n> failed: <listed_name(label)> (<note>)...`; then `, <n>
   skipped: ` with the skipped labels grouped by note in first-met order, `name_list(labels) (<note>)` joined `; `;
   then `; <housekeeping>` when there is one.

## Open point

The TS hands `onTool` the row object and later corrects that same object in the checks, so a caller holding it sees
the correction. A Rust `on_tool` gets a clone taken when the row lands; the corrected rows are in the answer. Provision
(provision.ts:313) pushes each row to its run as it lands; its port reads the answer for the final rows.

## Rules against tests

| Rule | Tests (`golden_tools_loop_hard.rs` unless named) |
|---|---|
| order, `after`, onTool order, the first exec | `two_steps_one_after_the_other_each_checked_after_the_loop`, `hands_each_row_to_the_caller_watching_the_moment_its_outcome_exists_in_the_order_the_loop_reached_them` |
| a failure, its Error line, what waits skipped | `a_failure_is_named_by_its_error_line_and_skips_what_waits_on_it`, `skipped_rows_share_a_reason_line_in_the_order_first_met` |
| present | `a_step_already_on_the_machine_runs_nothing_and_lets_what_waits_on_it_run`, `pushes_a_step_the_caller_already_found_on_the_machine_as_installed_and_runs_nothing_for_it` |
| the floor and the one cleanup | `under_the_floor_the_sweep_alone_runs_and_the_install_goes_on_once_it_clears`, `still_under_the_floor_after_the_cleanup_every_tool_left_is_skipped_with_the_reading`, `df_unknown_after_the_cleanup_skips_with_both_readings`, `a_full_disk_reads_zero_not_unknown`, `a_homebrew_cleanup_that_fails_at_the_floor_is_named_in_the_rescue_line` |
| unknown df, warned once; junk named | `a_df_that_fails_warns_once_and_installs_without_the_floor`, `a_df_that_prints_junk_names_what_it_printed` |
| a step too big, alone, reading kept | `an_install_too_big_for_the_room_above_the_floor_is_skipped_alone`, `an_install_still_too_big_after_the_cleanup_stays_skipped_with_the_reading_kept`, `skips_an_install_too_big_for_the_room_above_the_floor_alone_and_installs_what_fits_after_it` |
| a caller's floor, a prefix's volume | `a_prefix_reads_the_volume_wsp_installs_onto_under_the_floor_the_caller_holds`, `keeps_its_floor_and_reads_what_is_free_on_the_volume_wsp_installs_onto_not_the_one_root_sits_on` |
| cellar lock; retry roads; compile roads not retried | `a_cellar_lock_waits_for_every_homebrew_lock_then_tries_once_more`, `a_download_road_at_its_limit_is_tried_once_more`, `a_download_road_at_its_limit_twice_fails_with_both_and_a_compile_road_is_not_retried` |
| Homebrew checkout and housekeeping | `homebrew_installed_carries_its_checkout_and_the_housekeeping_says_what_it_freed` |
| PATH check, road dropped; check failing | `an_install_that_leaves_no_command_on_path_fails_by_name_and_loses_its_road`, `a_path_check_that_fails_counts_every_named_tool_as_failed` |
| checks paged, a page that could not run | `checks_read_a_page_at_a_time_and_a_page_that_could_not_run_fails_only_its_rows`, `reads_the_checks_a_page_at_a_time_so_a_recipe_with_a_dozen_never_asks_one_exec_to_run_them_all`, `fails_only_the_rows_of_a_page_it_could_not_run_not_every_checked_row_on_the_machine`, `keeps_the_shared_homebrew_step_as_it_landed_when_a_formula_of_its_own_failed` |
| summary words, road words, a label with a comma | `a_road_install_names_its_road_and_a_label_with_a_comma_is_quoted_in_the_tally` |
| caches kept or swept; the stage named | `keeps_the_caches_where_the_machine_is_somebodys_own_computer_and_sweeps_them_where_it_becomes_an_image`, `the_stage_named_by_the_caller_is_on_every_line`, `no_tools_still_sweeps_and_closes` |
| a rejected call ends the loop with its error | `a_free_read_that_rejects_ends_the_loop_with_its_error_before_anything_installs`, `an_install_run_that_rejects_ends_the_loop_with_its_error` |
