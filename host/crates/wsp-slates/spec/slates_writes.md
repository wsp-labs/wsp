# slates_writes
Source: slates.ts:464-468 (the write budget), 1017-1093 (`rehearse`, `writeDocument`, `undo`), 1118-1231 (`write`,
`state`) at 65f6b89c7. Tests: slates.test.ts cases on writing, patching, checking, rehearsing, refusing, the agent's
and the person's `state`, secrets, the paired computer, warnings and dead runs.

## What it does
`slate_write`: a whole document or a patch, checked or stored, the undo, and a check that rehearses values and a
press on a copy. `slate_state`: the live values a window or the agent sets, a secret the person types, and the runs
the agent may start. Each goes on the thread's chain.

## Behaviour
1. `write(p, caller)` (:1118-1167). Before the chain: the target (a write); `values` or `press` without `check`
   refuses usage `values and press rehearse a slate, which only a check does.` / `Add check, or set the live values
   with slate_state.`; both `text` and `document`, or neither without a rehearsal, refuses usage code A601 `A601
   step-arg: a slate write takes exactly one of text or document, and this one has <none|both>.` / `Send text
   alone.`; whole means a document, or text whose start (after JS whitespace) is `<slate`. On the chain, with the
   record or a fresh one:
   - neither given: no document refuses `Z802 no-slate: this thread has no slate to rehearse; send the <slate> with
     check`; else rehearse the live document.
   - whole: parse the text or validate the document; errors are the parser's then the MCP table's refusals
     (`refused_on`); any, or no document, refuses `invalid`. A rehearsal rehearses it over `values_for`; a check
     answers the sketch of it over fresh values (no sources read), version unchanged, its warnings, no problems;
     else `check_version`, the agent's write spends the budget (V751 below), and `write_document` with cause
     `write` and every piece id.
   - a patch: version 0 refuses `Z802 no-slate: this thread has no slate to patch; write a whole <slate> first`;
     parse errors refuse invalid; a lone `undo` op answers the current sketch for a check, else `check_version` then
     `undo`. Otherwise apply; errors (with the MCP refusals) refuse invalid; values are the record's for a clear, else
     `values_for` over the patched values; a rehearsal or a check answer as above over the patched document; else
     `check_version`, the budget, and `write_document` with cause `clear` or `write` and the ids the ops name, once
     each, in order.
2. The agent's budget (:464-468): a refusal reads `V751 write-rate: wait <s> s; a slate that rewrites itself
   constantly is a bug`, kind conflict. The person's writes spend none.
3. `write_document(r, next, values, by, cause, pieces, warnings)` (:1052-1080): values over 262,144 bytes refuse
   invalid `S500 values-too-big: the values are <n> bytes; a slate holds at most 262144`; declarations dropped; a
   record past version 0 keeps the one-step `previous`; document and values set, `empty` `cleared` for a clear;
   version and revision + 1; held; timers armed; with a document, held asks re-held when their key moved, and each
   timed run reading idle that would be held is started now `by: timer` (so it reads held and asks); saved;
   announced; `watch_pr` when the document names `pr.checks`; every value pushed; answers `{version, text (sketch),
   warnings, problems, waiting}`.
4. `undo(r, by)` (:1082-1093): no `previous` refuses conflict `V752 nothing-to-undo: there is no earlier slate to go
   back to; the undo keeps one step`; else the current becomes `previous`, the old one is put back (`become`),
   version and revision + 1, saved, announced `undo`, all pushed; answers the sketch with no warnings or problems.
5. `rehearse(r, doc, values, p, warnings)` (:1021-1050): each value path (a bare name is `$name`); a path into a run
   seeds a copy of that run's record as `{state:"done", runs:1, ...was, [state "done", endedAt now when it had none]}`
   and sets the field there; the rest are the batch's writes. A press of a piece the document lacks refuses invalid
   `D203 piece-missing: there is no piece <p> to press` with the nearest id as its fix. The batch runs over the copy
   as the person with the press event; the answer's text is the sketch after it, then a line each `would start
   $<run> (<why>)`, `would cancel $<run>`, `would <do> <JSON text>`, `would <do> in the window`; the version is the
   record's; nothing stored, started or sent.
6. `state(p, caller)` (:1169-1231): the target (a write); a paired computer naming `start` refuses usage `a paired
   computer writes a slate's values and starts no run.` / `Press it on the computer the slate runs on.` On the
   chain: `need_record`; no document refuses `Z802 no-slate: this slate is empty; write one with slate_write`;
   `check_version`; neither values nor starts refuses usage `slate state sets values or starts runs, and this one
   names neither.` / `Give values, start, or both.` Each start (a leading `$` dropped): undeclared refuses invalid
   `K702 run-name: $<run> is not a run` with `$<nearest>`; approved always starts (`agent` or `person`); held says
   `$<run>: it already waits for the person to allow it on the slate, and starts once they do`; a `confirm` says
   `$<run>: it has confirm, so it asks the person every start and only a press or a <when> starts it`; else
   `$<run>: you start only a run the person allowed "Always in this thread"; a press, a <when> or every= starts it
   and the slate asks them`. Each value: not a value path refuses S501 `<written> is not a value path; a value path
   reads $name, with .field and [index] below it`; a derived or a run refuses A607 `$<n> is a <derived value|run>
   and cannot be written` with fix `write the value it reads`; undeclared refuses S501 `$<n> is not declared` with
   the nearest; a secret takes only the person's typed string at its root (S520 `$<n> is a secret: only the person
   types it, into its input` / `$<n> takes the typed text`), empty clears it, and the batch gets the handle. The
   agent spends the write budget; a call with values and no starts releases the shared start budget. One batch for
   the values (no reactions from a paired computer), `announce(state)` for the agent's, one batch for the starts;
   sends delivered without waiting (a failure logs `a slate send in thread <word> was not delivered: <message>`).
   Answers `{version, text (sketch, then "not started:" and each indented), problems, waiting, [notStarted]}`.

## Edge cases
- A whole write as the agent: version 1, text starting `slate v1`; a timed run not allowed reads held, `waiting:
  ["$tick"]`, no problem, and the text names it as waiting.
- Twice the same declaration keeps its timer and approval (no ask); a changed `every`, `always`, `timeout` or
  `once` asks again with a new key.
- A rehearsal never moves version, revision, values or asks; `check` with `values: {"$market.json": {open: true},
  "$market.exit": 0}` sketches `Open` and stores nothing.
- A refused write lists every error on one numbered line; one error is said alone.
- A file naming a secret is refused invalid with an S520 error.
- A write at the version read passes while a run moves values (versions move only with documents).

## Behaviour nobody wrote down
- Rule R2: everything after the target runs on the chain; the refusals before it never queue.

## Decisions
1. Not ported here, kept with `wsp-threads`: `a start into a folder that is not there is refused before the agent
   is launched` and the model, effort and window a press's or a notify's turn starts on (both test the session
   start). The slate half of the press (`outcome: started`, the prompt) is in `slates_acts`.

## After the port
- None.

## Tests

27 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| slates_writes Behaviour 1, 3 / Edge case: slates.test.ts:316 | `slates_writes::changing_how_often_a_command_runs_or_letting_it_run_hidden_asks_again` |
| slates_writes Edge case: slates.test.ts:373 | `slates_writes::a_timed_run_the_person_has_not_allowed_reads_held_from_the_write_and_the_writ` |
| slates_writes Behaviour 5 / Edge case: slates.test.ts:498 | `slates_writes::a_check_previews_what_the_panel_shows_for_a_run_s_result_setting_run_json_on` |
| slates_writes Edge case: slates.test.ts:514 | `slates_writes::a_run_that_nothing_can_start_warns_in_the_write_s_text_and_the_slate_is_stil` |
| slates_writes Behaviour 3 / Edge case: slates.test.ts:522 | `slates_writes::a_write_s_warnings_are_in_its_text_with_their_fixes_not_only_in_the_structure` |
| slates_writes Edge case: slates.test.ts:536 | `slates_writes::a_refused_write_names_every_error_on_its_one_line_numbered_not_the_first_alon` |
| slates_writes Edge case: slates.test.ts:1158 | `slates_writes::refuses_a_file_that_names_a_secret` |
| slates_writes Edge case: slates.test.ts:1010 | `slates_writes::slate_read_shows_each_file_s_text_in_the_printed_slate_and_its_size_in_the_sk` |
| slates_writes Behaviour 6 / Edge case: slates.test.ts:965 | `slates_writes::the_agent_starts_a_run_the_person_said_always_to_and_an_unapproved_or_ask_eve` |
| slates_writes Behaviour 6 / Edge case: slates.test.ts:723 | `slates_writes::a_paired_computer_writes_values_and_starts_nothing_no_reaction_fires_off_its` |
| slates_writes Edge case: slates.test.ts:826 | `slates_writes::a_plotted_value_that_is_not_a_number_is_a_problem_in_a_read_once_a_run_fills_` |
| slates_writes Behaviour 1, 5, 6 / Edge case: slates.test.ts:588 | `slates_writes::a_check_rehearses_values_and_a_press_on_a_copy_a_bare_name_writes_name_and_a_` |
| slates_writes Behaviour 1 / Edge case: values and press rehearse only under a check | `slates_writes::values_or_a_press_without_check_refuse_with_the_fix_to_add_it` |
| slates_writes Behaviour 1: exactly one of text or document | `slates_writes::a_write_of_neither_text_nor_document_or_of_both_is_refused_a601` |
| slates_writes Behaviour 1: a rehearsal with no document to rehearse | `slates_writes::a_rehearsal_of_the_live_slate_with_no_slate_refuses_z802` |
| slates_writes Behaviour 1: a patch needs a slate to patch | `slates_writes::a_patch_with_no_slate_refuses_z802_and_asks_for_a_whole_slate` |
| slates_writes Behaviour 1: a check stores nothing and leaves the version | `slates_writes::a_check_of_a_whole_slate_answers_its_sketch_at_the_version_read_and_stores_no` |
| slates_writes Behaviour 1 / Edge case: a write at a stale version | `slates_writes::a_write_at_a_version_that_is_not_the_slate_s_refuses_v750` |
| slates_writes Behaviour 2: the agent's budget, the person's none | `slates_writes::the_agent_s_twenty_first_write_at_once_refuses_v751_and_the_person_s_never` |
| slates_writes Behaviour 3: the record, the announcement and the pushes of a write | `slates_writes::a_write_announces_every_piece_id_and_pushes_every_value` |
| slates_writes Behaviour 3: pr.checks makes the host watch the pull request | `slates_writes::a_document_that_names_pr_checks_makes_the_host_watch_the_pull_request` |
| slates_writes Behaviour 3, 4: a patch keeps one step back, undo puts it back and keeps one step too | `slates_writes::a_patch_keeps_the_slate_before_it_and_undo_puts_it_back` |
| slates_writes Behaviour 1, 3: a clear keeps the values and reads cleared | `slates_writes::a_clear_empties_the_slate_keeps_its_values_and_reads_cleared` |
| slates_writes Behaviour 6: a state with no slate, a stale version, and names that are not values | `slates_writes::a_state_with_no_slate_or_at_a_version_behind_is_refused` |
| slates_writes Behaviour 6: a path that is not a value, a derived value, a run, an undeclared name | `slates_writes::a_state_names_that_are_not_value_paths_refuse_with_the_nearest_name` |
| slates_writes Behaviour 6: a secret takes only the person's typed text at its root | `slates_writes::a_secret_takes_only_the_person_s_typed_string_and_an_empty_one_clears_it` |
| slates_writes Behaviour 6: the agent's value write is announced, and a person's is not | `slates_writes::the_agent_s_state_is_announced_as_a_state_and_the_person_s_is_not` |
