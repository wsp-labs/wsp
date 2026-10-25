# project_state_scripts: the python3 the machine runs

Source: `packages/engine/src/project-state/py.ts`, `listing.ts`, `merge.ts`, and the script half of `sqlite.ts`
(`sqliteFilter`, `portable`, `sqliteMergeStep`) at `a4033dc34`. Rust: `src/project_state/{py,listing,merge,sqlite}.rs`.
Tests: `tests/project_state_scripts.rs` (they run the scripts with python3 against fixture homes), hard tests in
`tests/project_state_hard.rs`. Oracle: `tests/fixtures/oracle/project-state-scripts.json`.

Nothing on the machine can run the engine, so these functions only build text. Every script is compared byte for byte
with the TS's in the oracle; the run tests then check what it does.

## Building blocks

- Y1. `py_data(v)` = `data(` + JSON string of base64(UTF-8(js_stringify(v))) + `)`. `js_stringify` is JS
  `JSON.stringify`: integer-like keys first in ascending order then insertion order, `1` for 1.0, `1e+21`, `-0` as `0`,
  ` ` and non-ASCII unescaped, control characters as `\uXXXX` except `\b\f\n\r\t`. Oracle `pyData`: 15 values.
- Y2. `PY_PREAMBLE` and `ROOTS_STEP` are the constants in the oracle, byte for byte.
- Y3. Indenting a step indents every line of it (split on `\n`) by four spaces; a filter's lines inside its `try:` by
  eight.

## listing.ts

- Y4. `list_script(agent, home, path, roots, steps)`: lines joined by `\n`, no trailing newline: preamble; `AGENT =
  <py_data>`; `HOME = <py_data>`; `PATH = os.path.normpath(<py_data(path)>)`; `ROOTS = [os.path.join(HOME, r) for r in
  <py_data(roots)>]`; `OUT = []`; `UNREAD = []`; say(); unread(); `try:`; the steps indented; `except Exception:`;
  `    OUT = []`; the roots step indented; the two print loops. Oracle `listScript`, `listScriptNoSteps`.
- Y5. `keyed_step(parent, key)` and `filter_step(scratch, home, shared, steps)`: oracle `keyedStep`, `filterStep`,
  `filterStepNoSteps`. `STORE` is `join(home, store)`, `COPY` is `store_copy(scratch, home, store)`.
- Y6. `store_copy(scratch, home, store)` = node posix `join`: segments joined by `/`, `.` dropped, `..` pops (never
  above the start of a relative path, where it is kept), repeated slashes collapsed, a trailing slash kept, all empty is
  `.`. Oracle `storeCopy`.
- Y7. `parse_state_listing(stdout)`: lines split on `\n`, empty lines skipped, a line opening `!unread ` decoded as
  base64 JSON of `{agent, store, why}`, others are paths; both lists keep order. Oracle `parseStateListing`.

## merge.ts

- Y8. `merge_script(from, to, steps)`: oracle `mergeScript`, `mergeScriptNoSteps`: preamble, moved(), out(), fail(),
  write(), `FROM`, `TO`, the counters, the steps unindented, the out line.
- Y9. `jsonl_cwd_step(files, holder)`: oracle `jsonlCwdStep` (holder `payload`), `jsonlCwdStepNoHolder` (`obj`).
- Y10. `parse_merge_output(stdout)`: the last line after trimming trailing whitespace of the whole text. Empty: `the
  merge script printed nothing`. Not JSON: `the merge script ended with: <line>`. An object with a string `waiting` is
  `Waiting` (counts ignored). Else numbers `merged` and `kept` (a string `"1"` is not a number) give `Merged` with `note`
  only where it is a string. Anything else, arrays and `null` included: `the merge script ended with: <line>`. Oracle
  `parseMergeOutput`, 10 cases.

## sqlite scripts

- Y11. `sqlite_filter(tables)`: oracle `sqliteFilter` (two tables) and `sqliteFilterNone`. TABLES carries only `table`
  and `where`, in that key order. `FILTER_BATCH` 500 rides the `fetchmany` line.
- Y12. `portable(row)`: the row as a JSON object in column order; a blob as `{"$blob": <standard base64>}`; integers
  as JSON integers, reals as JS numbers (1.0 is `1`), text as strings, null as null.
- Y13. `sqlite_merge_step(db, tables)`: oracle `sqliteMergeStep`, the rows typed in `sqliteMergeStepRows`. Each table
  on the wire is `table`, then `key`, `set` (Keyed) or `by`, `drop` (By), then `rows` as `portable`.

## What the scripts do on the machine (pinned by the run tests)

- The listing prints each path once, only where it exists; a store it cannot read prints nothing else of that home,
  removes a half-written copy and prints the `!unread` line with `str(e)` or the exception's class name. A copy left by
  an earlier trip is removed first. A store with no row for the project is not named. A virtual table's shadow tables
  are skipped where their create fails. The filter streams 500 rows at a time.
- The merge waits (`{"waiting": "no <DB> on the machine"}`, `... no table <t> in <DB>`) without creating anything; a
  keyed table inserts or updates only the `set` columns that differ (`is not excluded.`), so a second run changes
  nothing; a by-table inserts a group only where the machine holds no row of that owner; one transaction per store; a
  sqlite error fails with `merge into <DB> failed: <e>` and leaves the store as it was.

## Restart

Scripts are built fresh on every call; nothing is kept.

## Pinned since (M-rules)

- M10 (here). `py_data` moves only canonical array-index keys (`{"b":1,"01":2,"-1":3,"4294967295":4,"4294967294":5,
  "0":6}` is `{"0":6,"4294967294":5,"b":1,"01":2,"-1":3,"4294967295":4}`); `js_stringify` keeps text order, so py_data
  reorders. `parse_state_listing` splits on `\n` alone (a `\r` stays on the path) and `!unread` without its space is
  a path. `parse_merge_output` keeps an empty `note` as `Some("")`. Only the gemini module's merge text was pinned
  before by `hasMerge`; it is now pinned whole (agents M9); the other modules' merge data rides their run tests.

## Rules and their tests

Files: `project_state_scripts.rs` (scripts), `project_state_hard.rs` (hard).

| Rule | Tests |
|---|---|
| M10 | project_state_more.rs: `rollouts_index_keys_listing_lines_and_an_empty_note` |
| Y1, Y2 | hard `py_data_writes_the_ts_text_for_every_oracle_value`; scripts `every_listing_and_the_merge_open_with_the_one_preamble_so_the_under_rule_is_defined_once` |
| Y4 to Y6 | hard `list_and_filter_steps_match_the_ts_byte_for_byte`, `store_copy_joins_as_node_posix_join`; scripts `each_module_names_the_paths_under_its_home_it_reads_and_the_store_it_keeps_is_one_of_them`, `brings_down_the_projects_own_entries_and_a_filtered_copy_of_each_shared_store_never_another_projects`, `a_second_trip_through_the_same_scratch_root_writes_the_copy_again_rather_than_into_the_one_there`, `names_nothing_for_a_path_no_agent_ran_in_a_store_not_made_and_a_home_that_is_not_there`, `a_source_with_a_trailing_slash_or_a_dot_segment_as_the_wire_carries_it_names_the_same_paths` |
| Y7 | hard `parse_state_listing_matches_the_ts`; scripts `a_store_on_the_machine_it_cannot_read_brings_nothing_from_that_home_and_names_the_store_and_why` |
| Y8, Y9 | hard `merge_script_and_cwd_step_match_the_ts_byte_for_byte`; scripts `codex_a_rollout_line_that_is_not_utf8_passes_through_byte_for_byte_while_the_cwd_lines_are_rewritten` |
| Y10 | hard `parse_merge_output_answers_as_the_ts_for_every_oracle_line`; scripts `the_scripts_result_is_its_last_line_anything_else_is_an_error_naming_what_came_back` |
| Y11 to Y13 | hard `sqlite_filter_and_merge_step_match_the_ts_byte_for_byte`, `portable_writes_blobs_as_base64_and_whole_reals_as_js_numbers`; scripts `codex_the_copy_of_the_index_holds_the_projects_threads_alone_and_the_store_on_the_machine_is_not_touched`, `hermes_the_copy_of_the_store_holds_the_projects_sessions_and_their_messages_alone`, `opencode_the_copy_of_the_store_holds_the_projects_rows_in_every_table_its_messages_and_parts_alone`, `a_store_whose_schema_holds_a_virtual_table_is_filtered_all_the_same`, `the_filter_streams_the_projects_slice_into_the_copy_peak_memory_stays_a_fraction_of_the_rows_it_writes` |
| merge behaviour | scripts `codex_merge_lands_thread_rows_at_machine_paths_fixes_cwd_keeps_machine_rows_and_a_second_run_changes_nothing`, `codex_a_thread_the_machine_already_lists_keeps_its_own_columns_and_gets_only_cwd_and_rollout_path`, `a_machine_without_the_store_waits_and_gets_none_a_store_missing_a_column_fails_and_is_left_alone`, the two hermes, the opencode and the four gemini merge tests |
