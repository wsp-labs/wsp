# slates_reads
Source: slates.ts:549-589 (`viewsFor`, `contextOf`, `scrub`, `sketched`), 592-685 (`boundPaths`, `problemsOf`,
`waitingOf`, `changedSince`, `asksOf`, `viewOf`), 1233-1279 (`read`, `catalog`), 1482-1488 (`resolve`) at 65f6b89c7.
Tests: slates.test.ts `a read carries each run's json`, `a read puts the paths it was asked for in its text`, `a
thread with no slate yet reads as an empty slate`, `a read asked for a run or a value without its $`, `a read
answers the JSX-like form by default`, `a read names each approval by the run`, `a timed run the person has not
allowed reads held`, `a plotted value that is not a number`.

## What it does
Everything the area answers without changing a record: how a path is resolved against the record and the host's
sources, the sketch with its standing problems and waiting runs, the sheet's asks, the view a window draws, and the
agent's `slate_read`, `slate_catalog` and a window's `resolve`.

## Behaviour
1. `views_for(r, extra)` (:549-553): the sources named by the document's JSON and the extra paths (`sources_named`),
   always with `time` first, viewed once through `view_sources`.
2. `context_of(r, views, row)` (:556-581): `now`, the row's `item` and `index` when given, and `resolve(path)`: a
   path without `$` reads the views; a `$` path that does not parse reads nothing; a derived name is evaluated once
   per context and cached, a name already being evaluated (a cycle) reads nothing, then the path's segments step
   into it; any other `$` path reads the values.
3. `sketched(r, views, check, asked, warnings)` (:585-588): the module's sketch with the context, the version, the
   warnings, and either `check: true` or the problems (`asked` then `problems_of`) and `waiting_of`; a text not
   starting `slate v<n>` gets `slate` replaced by `slate v<version>` at its start; the whole text scrubbed.
4. `bound_paths(doc)` (:592-609): the dependencies of every binding in every piece's props (and of every `${...}`
   in a lone `{format}`), of each piece's `when`, and of each derived expression, in that order, once each, less
   those starting `item` or `index`.
5. `problems_of(r, views)` (:613-620): the record's problems; `Z804 rewound-before: the thread was rewound to before
   this slate existed; write one with slate_write` when the document is null and `empty` is `rewound-before`; then
   for each bound path without `$` that resolves to nothing, `R900 data-missing: <path> has no value yet`.
6. `waiting_of(r)` (:623-627): `$name` of each declared run whose record reads held with no why or the approval why.
7. `changed_since(r, run, key, pin)` (:631-641): over the record's other allowed approvals of the same run: unread
   pinned files answer `<paths> could not be read on <computer>, so it asks again`; else the files whose hash
   differs from the approval's, in the order of the approval's then the new ones, answer `<paths> changed since you
   allowed it, so it asks again`; else none.
8. `asks_of(r)` (:643-667): each run the runs table holds for approval, as `{key, run, kind:"cmd", cmd, env, args,
   [stdin], computer (or "this computer"), folder, timeoutS, [confirm], [then], [files], [noAlways: true], why}`;
   `noAlways` when the pin left anything unpinned; `why` the pin's refusal, else `changed_since`, else the record's
   why, else `needs your approval`. Then the MCP table's held asks.
9. `view_of(r)` (:669-684): `{threadId, workspaceId, version, revision, document, values, [empty (or "none") when
   no document], comments, approvals (with `send: {state:"allowed", at}` added when sends are allowed), asks,
   problems, shownOnce, canUndo (a previous exists), updatedAt}`.
10. `read(p, caller)` (:1233-1261): the target (not a write); no record reads as a fresh one, never a refusal. Paths:
    `*` with a document means every bound path, else the asked list less `*`. Views include those paths. Misreads
    of each path; the sketch (or only `slate v<n>` with `sketch: false`) carries them. The text is the sketch, then
    `read:` and `  <path> = <JSON of the scrubbed value>` per path (cut at 4,000 units with `... (N characters)`),
    then a blank line and the printed document unless `text: false` or no document. The answer: `version, text,
    [document] (only with document: true), values (each path), state (each value not a run, as $name), derived
    (each, as $name), runs (each, `run_shown`, a summary when paths were named), problems (misreads then problems_of),
    waiting, comments, approvals` where each approval is keyed `$<run>` when a declared run carries it now, else its
    key, and reads its state. Every string scrubbed.
11. `catalog(p, caller)` (:1263-1279): a sub-thread refuses Z803; the module's catalog text; when a name was given
    and the text says `<name> is not in the catalog`, and the call names a thread (a refused target answers the
    text), the MCP table's catalog of that server, or the text plus `\n<name> as an MCP server of this thread's
    agent: <message>` when that fails.
12. `resolve(p)` (:1482-1488): no record answers each path null; else each path evaluated, scrubbed.

## Edge cases
- A thread with no slate reads `{version: 0, text: "slate v0, empty: write one with slate_write", problems: []}`.
- `values: ["$ticks", "$tick.state"], text: false` gives `read:\n  $ticks = 0\n  $tick.state = "held"` and no
  `<slate`.
- `values: ["tick", "ticks", "spot.json", "time.now", "$ticks", "len(git.changed)"]` gives exactly three X401s
  (run, value, no source) and the text carries `X401 slate: tick is not a source`.
- A run with `json` reads without `out`; a 9,001-unit out is cut with the `… (9001 characters; values
  ["$raw.out"] reads it whole)` tail; a read naming paths gives run summaries without `json`.
- A read of an approval made by "Always" reads `{$tick: "allowed"}`.
- `document: true` adds the JSON document; by default the text carries the printed `<slate title=...>`.

## Behaviour nobody wrote down
- Rule 8.21: two cut forms (U+2026 in `run_shown`, `...` here). Rule R5: 4,000 units.
- Every answer string passes through the thread's scrub (secrets never leave).

## Decisions
1. The source views are `IndexMap<String, Option<Value>>` (a source with no view is `None`).

## After the port
- None.

## Tests

24 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| slates_reads Behaviour 4: every binding, every when and every derived expression, once each, less item and index | `slates_reads::bound_paths_lists_bindings_whens_and_derived_once_each_less_item_and_index` |
| slates_reads Behaviour 6: the runs held with no why or the approval why | `slates_reads::waiting_of_names_each_declared_run_held_with_no_why_or_the_approval_why` |
| slates_reads Edge case: slates.test.ts:400 | `slates_reads::a_read_carries_each_run_s_json_without_its_out_twice_cuts_long_output_and_onl` |
| slates_reads Edge case: slates.test.ts:506 | `slates_reads::a_read_puts_the_paths_it_was_asked_for_in_its_text_not_only_its_structured_an` |
| slates_reads Edge case: slates.test.ts:530 | `slates_reads::a_thread_with_no_slate_yet_reads_as_an_empty_slate_not_a_refusal` |
| slates_reads Edge case: slates.test.ts:543 | `slates_reads::a_read_asked_for_a_run_or_a_value_without_its_dollar_says_where_it_is_and_one` |
| slates_reads Edge case: slates.test.ts:555 | `slates_reads::a_read_answers_the_jsx_like_form_by_default_and_the_json_document_only_when_a` |
| slates_reads Edge case: slates.test.ts:393 | `slates_reads::a_read_names_each_approval_by_the_run_it_is_for_not_by_its_key` |
| slates_reads Edge case: slates.test.ts:373 | `slates_reads::a_timed_run_the_person_has_not_allowed_reads_held_in_a_read_and_is_no_problem` |
| slates_reads Edge case: slates.test.ts:826 | `slates_reads::a_plotted_value_that_is_not_a_number_reads_r905_once_a_run_fills_it` |
| slates_reads Behaviour 10: `*` means every bound path, and a path asked for is read in the answer | `slates_reads::a_star_reads_every_path_the_document_binds_and_nothing_else` |
| slates_reads Behaviour 10: the sketch alone, then the paths, then a blank line and the printed document | `slates_reads::a_read_s_text_is_the_sketch_then_the_paths_then_a_blank_line_and_the_document` |
| slates_reads Behaviour 10: state, derived and runs are answered by name with their dollar | `slates_reads::a_read_answers_values_derived_values_and_runs_each_by_its_dollar_name` |
| slates_reads Behaviour 10: a path's line is cut at 4,000 units, the structured answer is whole | `slates_reads::a_path_s_line_in_the_text_is_cut_at_4000_units_and_its_value_is_whole` |
| slates_reads Behaviour 5: a bound path that resolves to nothing is R900 | `slates_reads::a_bound_source_path_with_no_value_reads_r900_data_missing` |
| slates_reads Behaviour 10: every answer string goes through the thread's scrub | `slates_reads::a_secret_a_run_printed_comes_back_scrubbed_from_a_read_and_a_resolve` |
| slates_reads Behaviour 12: no record answers every path null, else each path evaluated | `slates_reads::a_resolve_answers_null_for_a_thread_with_no_slate_and_each_path_evaluated_for_one_with` |
| slates_reads Behaviour 11: a sub-thread has no slate, so no catalog either | `slates_reads::a_sub_thread_s_catalog_refuses_z803` |
| slates_reads Behaviour 11: a name the catalog lacks is looked up as an MCP server of the thread's agent | `slates_reads::a_catalog_name_that_is_not_in_the_catalog_asks_the_threads_mcp_servers_and_says_why_it_fails` |
| slates_reads Behaviour 9: the view's keys in the TS's order, empty only when there is no document | `slates_reads::the_window_s_view_keeps_the_keys_of_the_ts_in_order_and_adds_empty_only_with_no_document` |
| slates_reads Behaviour 9: a thread's permission to message the agent reads as a send approval, canUndo follows previous | `slates_reads::a_view_adds_the_send_approval_once_the_person_allowed_it_and_can_undo_follows_previous` |
| slates_reads Behaviour 9: shown marks the slate shown once, and a thread with no slate stays without one | `slates_reads::shown_marks_the_slate_shown_once_and_makes_none_for_a_thread_without` |
| slates_reads Behaviour 8: a held command's ask, keys in the TS's order, the computer's own name | `slates_reads::a_held_command_asks_with_its_env_args_computer_folder_timeout_and_why_in_order` |
| slates_reads Behaviour 7: a script edited since the person said Always names itself in the ask | `slates_reads::an_edited_script_names_itself_in_the_ask_that_asks_again` |
