# runs_parts
Source: slate-runs.ts:17-242 (types), 363-482 (records and key helpers), 484-489 (`asText`), 985-987 (`timeoutOf`) at
65f6b89c7. Tests: slate-runs.test.ts `stopAll quiet ... restart and rewind rewrite running records` (record half),
`then ... is part of the key`; oracle `oracle/helpers.out` (`key.*`, `stableJson`, `asText`, `timeoutOf`, `spent`).

## What it does
The shapes the runs table and the slate area pass between them, and the pure functions over them: the run record
and what a start keeps of the last one, what a restart and a rewind make of a running record, the start budget, the
approval key of a command, and how a value becomes a command's text.

## Behaviour
1. **The record is an ordered JSON object** (`RunRecord`, :20-37), `serde(transparent)` over `serde_json::Map`, so
   each road writes its keys in the TS's order (rule R4). Getters read `state`, `runs` (0 when absent), `why`,
   `startedAt`. Every literal below is the TS's order; a key in brackets is present only when the TS spreads it in.
   - held: `state, why, runs` (:711). Asleep tick: `[result fields], state, why, runs` (:889).
   - failed by `fail`: `state, why, exit: null, runs, endedAt` (:717).
   - running: `state, runs, startedAt` with no kept result; with one, `[result fields], state, runs, startedAt,
     refreshing: true` (`runningRecord`, :375-377).
   - finished: `state, [why], exit, err, [json], out, [lines], startedAt, endedAt, ms, runs, [cut: true]` (:791-809,
     819); `why` sits after `state` whether it came with the fields or from the timer's streak.
   - cancelled by `stop`: `state, why, exit: null, runs, [startedAt, ms], endedAt`, the two only when it was running
     and had a start time (:848-855).
   - ended from outside (`endedRecord`, :380-382): `state, why, exit: null, runs, [startedAt], endedAt`.
2. **Result fields** (`RESULT_FIELDS`, :366): `exit, out, err, json, lines, endedAt, ms, cut`, always in this order,
   whatever the record's own order; a key holding JSON `null` counts as present (`exit: null` is kept).
3. `last_result(record)` (:369-372): a running record answers its result fields only when `refreshing` is true; any
   other answers them only when it has `startedAt` and reads `done`, `failed` or `cancelled`; else none. A held,
   idle or never-started record keeps nothing.
4. `restarted_record(record, now)` (:385-387): a running record becomes `failed`, why `the host restarted while it
   ran`, its kept result dropped; any other record comes back unchanged. `rewound_record` (:390-392) the same with
   `cancelled` and `the thread was rewound`.
5. `spent(times, at)` (:462-465): keeps the stamps younger than 60,000 ms; 12 or more kept answers none (past the
   budget); else the kept stamps and `at` appended.
6. `stable_json(value)` (:467-474): arrays in order; objects with keys sorted by UTF-16 code units (JS `<`), each
   `JSON.stringify(key):value`; a leaf as `JSON.stringify`, absent as `null`. Numbers are written as JS writes them
   (rules 8.11 and 4.25): `20`, `2.5`, `1e+21`, never `20.0`.
7. `run_key(decl)` (the table's `key`, :662-668): the first 32 hex characters of the sha256 of the JSON array
   `[kind, cmd, [[name, stable_json(expr)] for env sorted by name], [stable_json(a) for args], stdin ? stable_json :
   null, on ?? "thread", cwd ?? "", stream === true, every ?? null, always === true, timeout ?? 60, once === true,
   ...then?, ...stable_json(files)?, ...stable_json(scripts)?]`. `confirm` is not in it. `stdin: null` (a literal
   null) differs from no stdin.
8. `as_text(value)` (:484-489): null is empty; a string itself; a number or bool as JS's `String`; else
   `JSON.stringify` in the value's own key order.
9. `map_strings(value, fn)` (:477-482): every string, in arrays and object values, through `fn`; keys untouched.
10. `timeout_of(decl)` (:985-987): `min(max(timeout ?? 60, 1), 600)` seconds, fractions kept (`2.5`).
11. Constants: `NO_FOLDER`, `HELD_APPROVAL` (= `SLATE_HELD_APPROVAL`, `needs your approval`), `HELD_BUDGET`
    (`started 12 times in a minute; press to run it again`), `HELD_PRESSED` (`pressed 12 times in a minute; it can
    start again in a minute`), held busy `4 runs are already running`, held asleep `the box was asleep, so this tick
    did not wake it; press to run it now`, `BACKED_OFF`, `SECRET_DOTS` (`••••`), `STARTS_PER_MINUTE` 12, running at
    once 4, lines kept 500, timer floor 10 s, backoff after 5 failures for 300,000 ms, close grace 1,000 ms, default
    timeout 60 s, env name `^[A-Za-z_][A-Za-z0-9_]*$` (:363-415).

## Edge cases
- `restarted_record({state:"running", runs:2, startedAt:1}, 9)` is exactly
  `{"state":"failed","why":"the host restarted while it ran","exit":null,"runs":2,"startedAt":1,"endedAt":9}`;
  a done record is the same record (test `stopAll quiet ...`).
- `spent` of 12 stamps at 0..11 answers none at 59,999 and keeps 1..11 plus 60,000 at 60,000 (oracle).
- Keys pinned against the TS (oracle): `{kind:"cmd", cmd:"true"}` is `6144e0b9c343b38f3a9a13eb194e63bf`;
  timeout 60 and `on: "thread"` give the same key as none; `confirm` does not move it; the full declaration of the
  oracle is `4fd3f52da753cc4eadb19e8406b302e1`.
- `as_text`: `[1,"a"]` is `[1,"a"]`, `{b:1,a:2}` keeps `b` first, `1e21` is `1e+21`, `-0` is `0`.
- `timeout_of`: 0 and -5 give 1, 0.5 gives 1, 601 gives 600.

## Behaviour nobody wrote down
- Rule 3.11: the env names are sorted for the key, the record's keys are not.
- Rule 8.8: the sort compares UTF-16 units, which differs from Rust's `str` order only past U+FFFF.

## Decisions
1. `RunRecord` and `RunResult` are ordered JSON maps, not structs: every road's order differs (rule R4), and the
   slate area stores and pushes them as JSON. The alternative, a struct with accepted-difference lines for eight
   roads, is named in the slice's report.
2. `CmdRunDecl` holds its expressions as `serde_json::Value` and keeps `stdin: null` apart from no stdin.
3. `RunInput` is an enum `Value(Value)` / `Secret(String)`.

## After the port
- None in these lines.

## Tests

50 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| runs_parts Edge case: slate-runs.test.ts:377 (the record half, restartedRecord and rewoundRecord) | `runs_parts::process_lifetime::stopall_quiet_drops_completions_and_writes_nothing_restart_and_rewind_rewrite_ru` |
| runs_parts Behaviour 7: slate-runs.test.ts:395 (the key lines: then is part of the key) | `runs_parts::process_lifetime::then::reshapes_the_output_into_json_keeps_out_raw_is_part_of_the_key_and_shows_on_the` |
| runs_parts Behaviour 1: the six states name themselves, and parse back | `runs_parts::run_state_names_and_parses_the_six_states` |
| runs_parts Behaviour 1: who started a run names itself | `runs_parts::run_by_names_each_starter` |
| runs_parts Behaviour 1: the getters read state, runs (0 when absent), why and startedAt | `runs_parts::a_record_reads_its_state_runs_why_and_start` |
| runs_parts Behaviour 1: a record comes from a stored object whose state is a string and runs a number | `runs_parts::a_record_is_read_from_a_stored_object_and_written_back_in_its_order` |
| runs_parts Behaviour 3: a running record keeps its result only when refreshing is true, in RESULT_FIELDS order | `runs_parts::last_result_of_a_refreshing_running_record_is_its_result_fields_in_order` |
| runs_parts Behaviour 3: a running record that is not refreshing keeps nothing | `runs_parts::last_result_of_a_running_record_that_is_not_refreshing_is_none` |
| runs_parts Behaviour 3: done, failed and cancelled keep their result when they have startedAt | `runs_parts::last_result_of_a_finished_record_with_a_start_is_its_result_fields` |
| runs_parts Behaviour 3: a finished record that never started keeps nothing | `runs_parts::last_result_of_a_finished_record_without_a_start_is_none` |
| runs_parts Behaviour 3: a held, idle or never-started record keeps nothing, whatever else it holds | `runs_parts::last_result_of_a_held_or_idle_record_is_none` |
| runs_parts Behaviour 3: a finished record with no result field answers an empty result, not none | `runs_parts::last_result_of_a_finished_record_with_no_result_field_is_empty` |
| runs_parts Behaviour 2: a key holding null counts as present | `runs_parts::last_result_keeps_a_null_exit_and_a_null_json` |
| runs_parts Behaviour 2: the result fields and their order | `runs_parts::the_result_fields_are_these_eight_in_this_order` |
| runs_parts Behaviour 1: running without a kept result is state, runs, startedAt | `runs_parts::running_record_without_a_result_is_state_runs_started_at` |
| runs_parts Behaviour 1: running with a kept result is the result, then state, runs, startedAt, refreshing | `runs_parts::running_record_with_a_result_keeps_it_then_state_runs_started_at_refreshing` |
| runs_parts Behaviour 1: an empty kept result still marks the record refreshing | `runs_parts::running_record_with_an_empty_result_is_still_refreshing` |
| runs_parts Behaviour 1: ended from outside is state, why, exit null, runs, startedAt, endedAt | `runs_parts::ended_record_drops_the_kept_result_and_keeps_the_start` |
| runs_parts Behaviour 1: a record with no startedAt ends without one | `runs_parts::ended_record_without_a_start_has_no_started_at` |
| runs_parts Behaviour 4: a running record drops its kept result when the host restarted | `runs_parts::restarted_record_drops_the_kept_result_of_a_refreshing_record` |
| runs_parts Behaviour 4: any other record comes back unchanged | `runs_parts::restarted_record_leaves_every_other_state_as_it_is` |
| runs_parts Behaviour 4: a rewind makes a running record cancelled, saying so | `runs_parts::rewound_record_makes_a_running_record_cancelled` |
| runs_parts Behaviour 4: a rewind leaves every other record as it is | `runs_parts::rewound_record_leaves_every_other_state_as_it_is` |
| runs_parts Edge case: oracle spent | `runs_parts::spent_keeps_the_stamps_under_a_minute_and_refuses_past_twelve` |
| runs_parts Behaviour 5: under twelve the stamp is appended, and the twelfth stamp fills the budget | `runs_parts::spent_appends_the_stamp_until_twelve_are_kept` |
| runs_parts Behaviour 5: a stamp exactly 60,000 ms old has aged out | `runs_parts::spent_drops_a_stamp_that_is_exactly_a_minute_old` |
| runs_parts Behaviour 6: oracle stableJson | `runs_parts::stable_json_sorts_object_keys_and_keeps_array_order` |
| runs_parts Behaviour 6: oracle stableJson.undefined | `runs_parts::stable_json_of_nothing_is_null` |
| runs_parts Behaviour 6: numbers are written as JS writes them | `runs_parts::stable_json_writes_numbers_as_js_does` |
| runs_parts Behaviour 6: strings and keys are written as JSON.stringify writes them | `runs_parts::stable_json_escapes_strings_and_keys` |
| runs_parts Behaviour 6 / Rule 8.8: keys sort by UTF-16 units, so a character past U+FFFF sorts before U+FF5E | `runs_parts::stable_json_sorts_keys_by_utf16_units` |
| runs_parts Edge case: oracle key.plain | `runs_parts::run_key_of_a_plain_command_is_the_oracles_hash` |
| runs_parts Edge case: oracle key.env | `runs_parts::run_key_with_an_env_binding_is_the_oracles_hash` |
| runs_parts Edge case: oracle key.full | `runs_parts::run_key_of_the_full_declaration_is_the_oracles_hash` |
| runs_parts Edge case: oracle key.timeout60_same_as_absent | `runs_parts::run_key_of_timeout_60_is_the_key_of_no_timeout` |
| runs_parts Edge case: oracle key.on_thread_same_as_absent | `runs_parts::run_key_of_on_thread_is_the_key_of_no_on` |
| runs_parts Edge case: oracle key.confirm_not_in_key | `runs_parts::run_key_does_not_move_with_confirm` |
| runs_parts Behaviour 7: a literal null stdin differs from no stdin | `runs_parts::run_key_keeps_a_null_stdin_apart_from_none` |
| runs_parts Behaviour 7 / Rule 3.11: the env names are sorted for the key, the expressions are not | `runs_parts::run_key_sorts_env_names_and_not_args` |
| runs_parts Behaviour 7: every part the key lists moves it | `runs_parts::run_key_moves_with_each_part_it_lists` |
| runs_parts Behaviour 7: stream, always and once count only when true | `runs_parts::run_key_counts_stream_always_and_once_only_when_true` |
| runs_parts Edge case: oracle asText | `runs_parts::as_text_writes_each_kind_of_value` |
| runs_parts Behaviour 8: a bool, a whole number and an empty string | `runs_parts::as_text_writes_false_whole_numbers_and_the_empty_string` |
| runs_parts Behaviour 9: every string through the function, keys untouched | `runs_parts::map_strings_passes_every_string_through_and_leaves_keys_and_the_rest` |
| runs_parts Edge case: oracle timeoutOf | `runs_parts::timeout_of_clamps_to_one_and_600_and_keeps_fractions` |
| runs_parts Behaviour 11: env name `^[A-Za-z_][A-Za-z0-9_]*$` | `runs_parts::is_env_name_follows_the_pattern` |
| runs_parts slate-runs.ts:458: the sentence a failed files write gives | `runs_parts::files_unwritten_names_the_cause` |
| runs_parts Behaviour 11: the constants | `runs_parts::the_constants_have_the_ts_text_and_values` |
| runs_table Behaviour 12: slate-runs.test.ts:377 (the stop_all half; the records half is runs_parts') | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::stop_all_quiet_drops_completions_and_writes_nothing` |
| runs_table Behaviour 8: slate-runs.test.ts:395 (the run half; the key lines are runs_parts') | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::reshapes_the_output_into_json_keeps_out_raw_and_shows_on_the_sheet` |
