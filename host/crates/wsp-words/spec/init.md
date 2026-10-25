# init
Source: `packages/protocol/src/words/init.ts` (244 lines) at the cut, 383708c78 (598 lines at a3b870a6e; the cut took the job's phase, row, screen and progress words out with `init-job.ts`). Tests: `packages/protocol/test/setup.test.ts` and `format.test.ts:1467` (the disk sync lines), plus `pinned` in `tests/init.rs` (every value printed by the TS under node).

## What it does
The words of the cloud setup that stay: the Image card's steps, the sign-in answers and outcomes, each build stage, the lines a snapshot and a stopped build end on, the keys' lines, the cost line and the setup lines `wsp setup` prints.

## Behaviour
1. The word table is copied by script from the live TS object: `cloud_setup_words` (init.ts:12-102). Functions inside it (`keys::refused`, `refused_saved`, `unchecked`) build their sentence from `provider_key_name`.
2. `login_state_word` per init.ts:131-139; `sign_in_answer_label`/`short` per init.ts:114-129, one answer for each of `LOGIN_CHOICES` in order: copy, machine, later, key, skip, token.
3. `golden_stage_word(stage)`: the table's phrase; None for failed (init.ts:141-155).
4. `disk_unsettled_line`, `disk_sync_failed_line`, `DISK_SYNC_LINE`, `snapshot_stage_line`, `snapshot_progress_line` and `SAVING_IMAGE_LINE` (init.ts:157-177).
5. `init_stopped_line(error)`: split on ";", trimmed, empties dropped, duplicates dropped keeping first order; none left gives the error unchanged; every part offline (`^fetch failed$` or containing ENOTFOUND, EAI_AGAIN, ECONNREFUSED, ENETDOWN, ENETUNREACH, EHOSTUNREACH as a word) gives `This computer lost its network`, a private constant at the cut; else the parts joined by "; " (init.ts:184-195). `machine_left_line(reason)`: init.ts:181.
6. `title_with_need(title, needed)`: one leading "• " taken off, then put back when needed (init.ts:199-209). `NEEDS_YOU` and `NEEDS_YOU_MARK`.
7. `provider_said_line(status, said)`: the status alone for "", else `<status> <said>` (init.ts:213). `key_refused_line`, `saved_key_refused_line` and `key_unchecked_line` name the provider by `provider_key_name`.
8. `init_cost_line`: `A <fmt_size> workspace costs about $<to_fixed(rate, 2)> an hour while it runs and naps when idle` (init.ts:234). `init_setup_lines(setup)` (init.ts:239): the keys in the record's order, each `<key name>: saved|not set` where the key name is `provider_key_words(provider).key_name` else the word, then the cost line when the setup is priced. The agents here and the job's lines left with the cut.

## Edge cases
- `init_stopped_line(" ; ; ")` is the error unchanged; `"a; b; a"` is "a; b"; `"fetch failedx"` is not offline; one offline part beside another reason keeps both (`a_stopped_build_says_one_reason_once`).
- Two leading marks lose one: `title_with_need("• • T", false)` is "• T".

## Behaviour nobody wrote down
- Rule 8.13: the cost line's dollars are `to_fixed(rate, 2)`, not `fmt_cost`.

## Tapes
`init.get` and `init.keys`: the keys' lines and the cost line.

## Decisions
1. `Record<LoginState, ...>` tables are functions over the enum.
2. `disk_unsettled_line` takes the two kB counts as numbers.
3. `provider_key_name` and `provider_key_words` are this crate's `place_word` unit.
4. wsp-wire's `InitSetup` still carries the `agents` and `job` fields the cut dropped; `init_setup_lines` reads neither, and the fixtures that build an `InitSetup` carry them empty until the wire's drift pass.
5. Items the cut removed are gone with their tests; no crate on `rust/2448-base` called one of them (checked by name over `host/crates`, 25 Oct).

## After the port
- `init_stopped_line` of an error made only of separators returns the separators.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `the_words_the_clients_print_for_the_job::*` (constants alone) |
| 2, 3 | `the_sign_in_answers_offer_six_words_...`, `every_phase_login_choice_and_stage_has_one_word` |
| 4 | `the_sync_s_stage_line_...`, `stage_rows_clocks_and_snapshot_lines` |
| 5 | `a_stopped_build_s_sentence_is_this_computer_s_own_word_...`, `a_stopped_build_says_one_reason_once` |
| 6 | `the_need_mark_is_carried_once_...`, `the_need_the_title_mark_and_the_sidebar_line` |
| 7, 8 | `wsp_setup_prints_the_keys_as_held_or_not_and_the_price_and_no_key`, `keys_cost_and_the_agent_road` |
