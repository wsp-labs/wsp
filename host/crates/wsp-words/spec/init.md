# init
Source: `packages/protocol/src/words/init.ts` (598 lines) at a3b870a6e. Tests: `packages/protocol/test/init-job.test.ts` and `format.test.ts:1467` (17 cases ported, 3 in part; 3 ignored as constants alone; 8 schema cases skipped as wire's), plus `pinned` in `tests/init.rs` (every value printed by the TS under node).

## What it does
Every word of wsp init's job: the Image card's steps, the sign-in answers and outcomes, each build stage and row state, the lines a snapshot and a stopped build end on, what the job waits on the person for, the one sidebar line, the build's stage list with the sign-ins folded into one row and its count, the cost and setup lines, and the agent road's brief.

## Behaviour
1. The word tables are copied by script from the live TS objects: `cloud_setup_words`, `host_sign_ins`, `host_sign_in_commands`, `init_row_states` (init.ts:12-121, 197-224). Functions inside them (`keys::refused`, `refused_saved`, `unchecked`, `host_sign_ins::as`, `missing`) build their sentence from `provider_key_name`.
2. `init_phase_word` per init.ts:166-193; `login_state_word` per init.ts:153-160; `init_sign_in_word(login, platform)`: signed-in "done", copied `copied from <this_computer(platform)>`, skipped "skipped", deferred the deferred word (init.ts:229-236); `init_sign_in_outcome` is `{state, login}` (init.ts:241).
3. `sign_in_answer_label`/`short` per init.ts:137-144; `sign_in_choices(platform)` in `LOGIN_CHOICES` order: copy, machine, later, key, skip, token (init.ts:147-150).
4. `golden_stage_word(stage)`: the table's phrase; None for failed (init.ts:244-257). `init_stage_row_id`: `stage/<stage wire word>` (init.ts:264). `init_row_timed(row)`: the row id is the id of snapshotting or promoting (init.ts:267).
5. `init_elapsed_line(ms)`: under 60000 `<max(0, floor(ms/1000))>s`, else `fmt_duration(ms)` (init.ts:270).
6. `init_sign_in_line(row)`: None unless the state is "waiting for you"; "waiting for the code" when the row has a code or finishes by code; else "the page is open on this computer" (init.ts:299-302).
7. `init_row_unrun(state)`: skipped, not made, stopped. `init_row_failed(row)`: state failed or login not-signed-in. `init_row_over(row)`: any login set, or a state in done, failed, stopped, forked, imported, key set, MCP added, skipped, not made, gone (init.ts:322-338).
8. `init_stopped_line(error)`: split on ";", trimmed, empties dropped, duplicates dropped keeping first order; none left gives the error unchanged; every part offline (`^fetch failed$` or containing ENOTFOUND, EAI_AGAIN, ECONNREFUSED, ENETDOWN, ENETUNREACH, EHOSTUNREACH as a word) gives NETWORK_LOST_LINE; else the parts joined by "; " (init.ts:385-389). `init_failed_line` appends MACHINE_GONE_LINE or STOP_LEFT_MACHINE_LINE after a space (init.ts:394-398).
9. `init_job_over`: done, failed, cancelled; `init_job_building`: building, signing-in, sealing, finishing; `init_agent_step(job)`: the agent road and phase agent, or over with no screens (init.ts:401-410).
10. The open sign-in is the first row of kind sign-in in state "waiting for you". `init_need_what`: `sign in to <its label>` or None; `init_sign_in_waited_on`: its label (init.ts:422-434). `init_sweeping(rows)`: a machine row in state "machine still running, retrying" (init.ts:457). `init_progress_line(job)`: the open sign-in's line, else the sweep line, else the phase word with ` <done>/<total>` while building and total > 0 (init.ts:462-468).
11. `title_with_need(title, needed)`: one leading "• " taken off, then put back when needed (init.ts:450-453).
12. `init_build_rows(rows)`: agent rows dropped; every non-sign-in row kept in order; the first sign-in row replaced by `{id: "stage/sign-ins", kind: "stage", label: "Signing in on the computer", state[, login]}`, later sign-ins dropped; `sign_ins` is every sign-in row. The folded state: open if any is open; running if any runs; while not all are over, waiting when all wait else running; the first failed row's state and login; skipped when all are skipped; else done (init.ts:475-505).
13. `init_stage_count(rows)`: of the rows of kind stage, those over, not failed and not unrun, of all of them (init.ts:514-517).
14. `provider_said_line(status, said)`: the status alone for "", else `<status> <said>` (init.ts:528-530). `init_cost_line`: `A <fmt_size> workspace costs about $<to_fixed(rate, 2)> an hour while it runs and naps when idle` (init.ts:554-556). `init_setup_lines(setup)` per init.ts:560-575: keys in the record's order named by `provider_key_words(provider).key_name` else the word, agents with ` (MCP added)` where configured, the cost line when priced, then the job's lines or the no-setup line.

## Edge cases
- `init_stopped_line(" ; ; ")` is the error unchanged; `"a; b; a"` is "a; b"; `"fetch failedx"` is not offline; one offline part beside another reason keeps both (`a_stopped_build_says_one_reason_once`).
- `init_elapsed_line(-5)` is "0s"; 61500 is "1m 2s" (`stage_rows_clocks_and_snapshot_lines`).
- Two leading marks lose one: `title_with_need("• • T", false)` is "• T".
- All sign-ins skipped fold to skipped; one waiting and one key set fold to running (`the_sign_ins_fold_into_one_stage_and_the_count_reads_the_fold`).
- A sign-in that ran out is counted not done in the stage count.

## Behaviour nobody wrote down
- Rule 8.13: the cost line's dollars are `to_fixed(rate, 2)`, not `fmt_cost`.
- Rule 4.26: the folded row's keys are id, kind, label, state, login.

## Tapes
`init.get`, `init.start`, `init.answer`, `init.build` and the `init.job` event: every row word and progress line.

## Decisions
1. `Record<LoginState, ...>` tables are functions over the enum; `GOLDEN_STAGE_TIMED` is a two-element array.
2. `disk_unsettled_line` takes the two kB counts as numbers; `init_failed_line`'s machine is the `MachineEnd` enum.
3. `provider_key_name` and `provider_key_words` are declared in `pending_wire::types::place_word` until the `place_word` unit of this crate is written (the views writer leaves `PROVIDER_KEY_WORDS` to this crate).

## After the port
- `init_stopped_line` of an error made only of separators returns the separators.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `the_words_the_clients_print_for_the_job::*` (3 ignored: constants alone) |
| 2, 3, 4 | `one_word_per_phase_in_lower_case_...`, `the_sign_in_answers_offer_six_words_...`, `every_phase_login_choice_and_stage_has_one_word` |
| 5, 6 | `stage_rows_clocks_and_snapshot_lines`, `the_sync_s_stage_line_...` |
| 7 | `a_row_s_state_words_have_one_table_...`, `a_rows_state_says_whether_it_ran_failed_or_ended` |
| 8 | `a_stopped_build_s_sentence_is_this_computer_s_own_word_...`, `a_stopped_build_says_one_reason_once` |
| 9 | `one_predicate_says_the_job_is_over_...`, `the_jobs_phase_and_step` |
| 10, 11 | `the_collapsed_row_s_line_...`, `only_a_wait_the_person_is_not_looking_at_is_a_need_...`, `the_need_the_title_mark_and_the_sidebar_line` |
| 12, 13 | `the_sign_ins_fold_into_one_stage_and_the_count_reads_the_fold` |
| 14 | `wsp_setup_prints_the_keys_as_held_or_not_...`, `keys_cost_and_the_agent_road` |
