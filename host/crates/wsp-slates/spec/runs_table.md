# runs_table
Source: slate-runs.ts:613-983 (`createSlateRuns`), 133-163 (`RunApprovals`, `SlateRunsDeps`), 210-242 (`SlateRuns`) at
65f6b89c7. Tests: slate-runs.test.ts (28 cases); the large-module pilot's 8 review tests (#2440, 6396255198).

## What it does
The process table of every thread's slate: one live entry per run with its record, a generation that drops the end
of a replaced instance, the last result kept through holds, the hold it waits in (approval, four running, the start
budgets), a FIFO queue for runs past four at once, timers per declared `every`, and the secrets. Every record change
goes to `deps.on_record` at once. It starts nothing by itself but through a timer's `on_timer`.

## Behaviour
1. `live(thread, run, last)` (:627-636) makes an entry once: record `{state:"idle", runs: last.runs ?? 0}`, gen 0,
   no starts, and `last_result(last)` as its kept result. A later `last` is ignored.
2. `write` (:637-643): sets the record; a record that is not running and holds a result (`last_result`) replaces the
   kept result, else the kept result stays (a hold never wipes it); then `on_record(thread, run, record)`.
3. `start(req)` (:883-904), in order: (a) `asleep` and `by: timer`: a running entry answers `noop`; else drops any
   hold and writes held `[result], state, why: <asleep>, runs` and answers `held` with no ask. (b) `on: "host"` with
   `host_refused` saying why: `fail`. (c) `beside` = the entry reads running; `beside` with `once` answers `noop`.
   (d) drops any hold. (e) spends one start against the person's own budget for `by: person`, else the shared one;
   past it, `hold(pressed | budget, beside)`. (f) a `confirm`, or no approval for `run_key(decl)`, is
   `hold(approval, beside)`. (g) else `launch`.
4. `hold(req, entry, for, beside)` (:704-713): keeps `{req, for}`; `busy` joins the queue once. Beside a running
   instance the record is left as it is (not written) and the answer is `held` with that record (and the ask for
   `approval`). Otherwise writes `{state:"held", why, runs}` with why `needs your approval`, `4 runs are already
   running`, the pressed or the budget sentence; answers `held`, with the ask only for `approval`.
5. `fail(req, entry, why)` (:715-719): writes `{state:"failed", why: streak(failed, why), exit:null, runs, endedAt}`.
6. `launch(req, entry)` (:733-834), in order: a running instance is stopped with no reason; four running in the
   thread is `hold(busy)`; `inputs()` is read; a secret in `args` fails `SLATE_SECRET_IN_ARGS`; `road(thread, on ??
   "thread")`; env is `login_env(thread)` here (every `WSP_` name dropped, `SLATE_DIR` from `dir(thread)`, whose
   failure fails `the slate's files were not written: <message>`) or `{SLATE_DIR: road.slate_dir, WSP_THREAD:
   thread}` on a road; each env input in order: a name not matching the env pattern fails `<name> is not a name an
   environment variable can have`; a secret's plaintext, a value's `as_text`, and a secret not set or a null value
   removes the name; args are `as_text` (null is empty); stdin a secret's plaintext or empty, or `as_text`; cwd the
   folder, or the folder resolved with `cwd` (`path.resolve` here, posix on a road: an absolute `cwd` wins, `..`
   folds); here a cwd that is not a folder fails `the folder <cwd> does not exist`. Then gen += 1, `runs` + 1,
   writes the running record (rule `runs_parts` 1) and starts the road.
7. While it runs (:768-774): each line scrubbed, the last 500 kept, `on_line` only for the current gen.
8. Its end (:775-821), dropped when the gen moved: out and err scrubbed then `run_output_tail`; `json` is the out
   parsed when its trimmed text is not empty and parses; exit 124 on timeout else the code; why the scrubbed spawn
   error, `timed out after <s> s`, none on exit 0, `ended by <signal or "a signal">` on no code, `exited with <n>`.
   A failure or no `then` finishes at once; else the `then` runs on the raw scrubbed out (here through `reshape`,
   on a road through `road.reshape` with only `SLATE_DIR` and `WSP_THREAD` and 60 s), and its answer finishes with
   `done` and its json, or `failed` with its why, exit and err. Finishing writes the finished record (`runs_parts`
   1), with `lines` only for a `stream` run, then runs the queue.
9. Process naming (:822-830): `own(thread, pid, "slate")` once the road answers a pid and the run has not ended; its
   release runs at the end. The kill is kept only when the gen is current and the record still reads running.
10. `stop(thread, run, entry, why)` (:836-856): gen += 1, kill, kill the reshape, drop the hold, leave the queue. A
    reason and a record that read running or held writes the cancelled record; otherwise nothing is written.
11. `approve(thread, run, scope)` (:905-912): only an entry held for approval; `always` allows the key first; then
    `launch`. Else none. `deny` (:913-923): with an instance still running or reshaping, only the new start is
    dropped (no record); else `stop` with `you said not to run it`.
12. `cancel` stops with `cancelled` then runs the queue (:929-934). `stop_all(quiet | why)` stops each run with no
    reason when quiet, else the why or `cancelled`, and does not run the queue (:935-939).
13. The queue (:721-731): while runs wait and fewer than four run, the first is taken; one no longer held as busy is
    skipped; the rest launch.
14. Timers (:858-879, 940-952): `timers(thread, list)` keeps each timer whose run, key, every and always are
    unchanged (its period and failures run on), drops the rest, adds the new ones, then ticks. A tick arms each
    timer that should run (always, or the slate is shown) and is not armed: it fires at once, then every
    `max(every, 10)` seconds; it disarms the ones that should not. A fire of an `always` timer with five or more
    failures in a row, within 300,000 ms of the last, is skipped. `streak` (:694-702): `done` clears the failures,
    `failed` adds one and stamps the time, and on an `always` timer at five or more adds `; <BACKED_OFF>` to the why
    (`it failed` when there was none).
15. `release(thread)` empties the shared start budget of each entry, never the person's (:954-956).
16. `held(thread)` lists each entry held for approval, in entry order, its ask built again (inputs read now) (:926).
    The ask (:673-690): `run, key, cmd, env, args, [stdin], folder, on, timeout, [confirm], [then], [files]`; a set
    secret shows `••••`, an unset one `•••• (not filled)`, null `(unset)`; stdin its first line only.
17. `drop(thread, keep)` (:965-974): without `keep` every run is stopped with no reason; with it the running ones are
    left running; timers cleared and the thread forgotten either way; then every approval the approvals list holds
    for the thread is revoked, and the thread's secrets forgotten, `keep` or not.
18. `close()` (:975-981): every run of every thread stopped with no reason, timers cleared, all forgotten (rule R8).
19. `reshape(thread, cmd, input, folder, timeout_s = 60)` (:651-659): `login_env` failing answers at once
    `{why: "the slate's files were not written: ...", exit: null, err: ""}`; the cwd is the folder when it exists,
    else the host process's own.

## Edge cases
- A hold never wipes the kept result: done, then a held start, then approved, gives a running record with the old
  out and `refreshing` (pilot review).
- The budget is spent before the approval check: a start held for approval still counts.
- A thirteenth reaction or timer start in a minute holds `budget`; the person's start still runs; `release` frees the
  shared budget; the person's twelve a minute are freed by nothing (tests `reactions and timers ...`, `the person's
  starts ...`).
- A fifth start waits held `4 runs are already running` and runs when one ends (test `a fifth run waits`).
- `stop_all(quiet)` writes nothing, and the killed run's end writes nothing either (gen moved).
- A held start beside a running confirm run: the record stays running with its runs; `deny` drops the new start and
  the first one ends `done` with `runs: 1` (slates.test.ts `a second press on a confirmed run`).
- The asleep tick keeps the last out and exit on its held record and lists no ask.
- `every` past 2,147,483 s fires every 1 ms, as node clamps (rule 8.18); it never panics. The table asks the clock
  for `max(every, 10) * 1000` ms as it is (saturating, never wrapping) and the clock clamps, as node's timers do.
- An `always` timer failing every 10 s: five starts by 40 s, none more until 300 s after the fifth, the sixth then;
  once one succeeds every tick starts again (test `an always timer that fails five ...`).
- Shown-only timers: none fire until shown; shown fires at once and each 10 s floor after; hidden again stops them;
  `always` ones fire regardless (test `tick only while shown`).
- On a road the env is exactly `{SLATE_DIR, WSP_THREAD}` for the command and for its `then` (test `on the thread's
  own machine ...`).

## Behaviour nobody wrote down
- Rule R1 (3.3): `on_record`, `on_line`, `on_timer`, `own`'s release and a road's `kill` and `start` are called with
  no borrow of the table held; `on_record` re-enters the slate area, which may call `start` or `cancel` again.
- Rule 3.15: `now` and every timer through the clock. Rule 8.18: the interval clamp.
- Invariant 2: one end per instance, enforced by the generation.

## Decisions
1. `RunApprovals` and `SlateRunsDeps` are traits; `SlateRuns` is a trait so the slate area's tests can swap it, as
   `deps.runs` lets the TS.
2. `on_line`'s stream is `Stream::Out | Stream::Err`.

## After the port
- `stop_all` never runs the queue; a restart while four run holds itself busy until another ends (pilot).
- A timer's `every` past node's limit fires every millisecond (8.18).
- `drop(keep)` leaves running processes untracked: their ends still call `on_record`.

## Tests

45 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| runs_table Behaviour 6: slate-runs.test.ts:83 | `runs_table::values_reach_a_command_as_data::an_env_value_with_shell_metacharacters_is_one_word_and_runs_nothing` |
| runs_table Behaviour 6: slate-runs.test.ts:92 | `runs_table::values_reach_a_command_as_data::a_positional_parameter_and_stdin_carry_it_the_same_way` |
| runs_table Behaviour 6: slate-runs.test.ts:103 | `runs_table::values_reach_a_command_as_data::null_is_unset_a_list_is_json_and_the_run_sees_the_login_environment_with_no_wsp` |
| runs_table Behaviour 6: slate-runs.test.ts:115 | `runs_table::values_reach_a_command_as_data::a_cwd_is_read_against_the_thread_s_folder_and_a_missing_one_fails_with_why` |
| runs_table Behaviour 6: an absolute cwd wins and `..` folds, as path.resolve does (pilot finding) | `runs_table::values_reach_a_command_as_data::an_absolute_cwd_wins_and_dot_dot_folds` |
| runs_table Behaviour 6: an env name that is not one fails before anything starts (slate-runs.ts:751) | `runs_table::values_reach_a_command_as_data::an_env_name_that_is_not_a_name_fails_the_run` |
| runs_table Behaviour 6-8, runs_secrets 7: slate-runs.test.ts:127 | `runs_table::values_reach_a_command_as_data::secrets::reach_a_run_through_env_and_stdin_and_are_scrubbed_from_out_err_lines_and_the_record` |
| runs_table Behaviour 6: slate-runs.test.ts:162 | `runs_table::values_reach_a_command_as_data::secrets::are_refused_as_a_positional_parameter_which_ps_can_read` |
| runs_table Behaviour 16: slate-runs.test.ts:170 | `runs_table::values_reach_a_command_as_data::secrets::the_sheet_shows_dots_never_the_value` |
| runs_table Behaviour 3, 11: slate-runs.test.ts:205 | `runs_table::values_reach_a_command_as_data::secrets::consent::holds_until_approved_once_is_not_remembered_always_is_and_an_edited_command_is_held` |
| runs_table Behaviour 3, 11: slate-runs.test.ts:235 | `runs_table::values_reach_a_command_as_data::secrets::consent::a_confirm_asks_every_time_don_t_ends_it_cancelled_and_a_revoke_asks_again` |
| runs_table Behaviour 3e, 15: slate-runs.test.ts:253 | `runs_table::values_reach_a_command_as_data::secrets::consent::reactions_and_timers_start_a_run_at_most_12_times_a_minute_a_press_is_not_counted` |
| runs_table Behaviour 3e, 15: slate-runs.test.ts:270 | `runs_table::values_reach_a_command_as_data::secrets::consent::the_person_s_starts_have_a_budget_of_their_own_12_a_minute_which_no_press_or` |
| runs_table Edge case: the budget is spent before the approval check (slate-runs.ts:898-902) | `runs_table::values_reach_a_command_as_data::secrets::consent::a_start_held_for_approval_still_spends_the_budget` |
| runs_table Behaviour 3b: a stored host run of a thread now away fails before anything asks (slate-runs.ts:891-892) | `runs_table::values_reach_a_command_as_data::secrets::consent::a_host_run_the_host_refuses_fails_before_it_asks` |
| runs_table Behaviour 10, 12: slate-runs.test.ts:305 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::a_cancel_kills_the_whole_process_group_and_writes_cancelled` |
| runs_table Behaviour 3c, 6: slate-runs.test.ts:326 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::a_start_while_it_runs_restarts_it_with_once_it_is_a_no_op` |
| runs_table Behaviour 8: slate-runs.test.ts:339 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::a_timeout_ends_the_run_failed_with_exit_124_and_takes_its_group` |
| runs_table Behaviour 8: slate-runs.test.ts:348 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::a_non_zero_exit_fails_with_its_code_and_a_json_out_is_parsed` |
| runs_table Behaviour 8: slate-runs.test.ts:354 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::keeps_the_64_kb_tail_at_a_line_start_and_cuts_past_2_mb` |
| runs_table Behaviour 13: slate-runs.test.ts:366 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::a_fifth_run_waits_for_one_of_four_to_end` |
| runs_table Behaviour 12: slate-runs.test.ts:377 (the stop_all half; the records half is runs_parts') | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::stop_all_quiet_drops_completions_and_writes_nothing` |
| runs_table Behaviour 12: stop_all with a why writes it on each running and held run, and not on a done one | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::stop_all_with_a_why_writes_it_on_running_and_held_runs_only` |
| runs_table Behaviour 8: slate-runs.test.ts:395 (the run half; the key lines are runs_parts') | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::reshapes_the_output_into_json_keeps_out_raw_and_shows_on_the_sheet` |
| runs_table Behaviour 8: slate-runs.test.ts:407 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_failing_reshape_fails_the_run_with_its_stderr_and_one_that_prints_no_json_says_so` |
| runs_table Behaviour 8: slate-runs.test.ts:414 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::reads_the_output_scrubbed_and_never_sees_a_secret` |
| runs_table Behaviour 3a: slate-runs.test.ts:422 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::a_tick_while_it_naps_is_held_saying_so_its_last_result_kept_and_nothing_launched` |
| runs_table Behaviour 3a: a tick on a napping box while it runs is a no-op | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::a_tick_while_it_naps_and_runs_is_a_no_op` |
| runs_table Behaviour 14: slate-runs.test.ts:437 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::an_always_timer_that_fails_five_starts_in_a_row_starts_once_every_five_minutes` |
| runs_table Behaviour 14: slate-runs.test.ts:474 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::tick_only_while_shown_by_default_always_with_always_never_under_10_s` |
| runs_table Behaviour 14: an unchanged timer keeps its period; a changed one fires at once | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::an_unchanged_timer_keeps_its_period_and_a_changed_one_fires_at_once` |
| runs_table Edge case, rule 8.18, pilot finding: a huge every never panics; the clock is asked the delay as is | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_huge_every_never_panics_and_asks_the_clock_for_its_whole_delay` |
| runs_table Behaviour 14: only a run with a timer counts a streak (slate-runs.ts:694-696) | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::failures_of_a_run_with_no_timer_never_back_off` |
| runs_table Behaviour 9: slate-runs.test.ts:497 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::is_named_as_the_thread_s_slate_run_while_it_runs_the_pid_its_own_and_released` |
| runs_table Behaviour 6, 8: slate-runs.test.ts:511 | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::on_the_thread_s_own_machine_carries_the_thread_s_mark_for_the_command_and_its_then` |
| runs_table Behaviour 6: SLATE_DIR comes from dir, and its failure fails the run with why (slate-runs.ts:645-650, 747-749) | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::slate_dir_is_the_slate_s_folder_and_files_not_written_fail_the_run` |
| runs_table Behaviour 2, pilot BLOCKER: a hold never wipes the kept result | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::a_hold_keeps_the_last_result_for_the_next_start` |
| runs_table Behaviour 1: a start with `last` seeds the count and the kept result once, from a record | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::a_first_start_seeds_its_count_and_kept_result_from_the_stored_record` |
| runs_table Behaviour 17, pilot BLOCKER: drop with keep still revokes approvals and forgets secrets | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::drop_with_keep_leaves_runs_running_but_revokes_approvals_and_forgets_secrets` |
| runs_table Behaviour 17: drop without keep kills every run and writes nothing | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::drop_without_keep_kills_every_run_and_writes_nothing` |
| runs_table Behaviour 18, rule 6.40: close kills every run of every thread and writes no record | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::close_kills_every_run_and_writes_no_record` |
| rule R1, pilot BLOCKER: on_record may re-enter the table, which holds no borrow while it calls out | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::on_record_may_call_back_into_the_table` |
| runs_table Behaviour 11, slates.test.ts:1210: deny beside a running instance drops only the new start | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::deny_beside_a_running_instance_drops_only_the_new_start` |
| invariant 2: one end per instance; a restarted run's first instance never writes its end | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::invariant_2_a_replaced_instance_never_writes_its_end` |
| runs_table Behaviour 2: the approvals deps backs Always when given (slate-runs.ts:615, 910) | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::always_is_kept_in_the_approvals_the_host_gives` |
