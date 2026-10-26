# client-table

Source: `packages/host/src/verbs/client.ts` lines 736-1102 (367 lines) at 65f6b89c7. Tests: `project-flag.test.ts` (7, `optionalValues`), `refusals.test.ts` and `contract.test.ts` cases that end in `failed`, `mcp-record.test.ts`'s use of `releaseRefusal`, `absolutePath`, `otherVersion`. Rust: `src/verbs/client.rs`, "The verb table's types and the shared helpers"; tests `tests/client_helpers.rs` and the `lookup`/`parse` parts of `tests/verbs_oracle.rs`, from `tests/fixtures/client.json` and `lookup.json`.

## What it does
The types every verb module is written against (`Verb` in four shapes, `CliVerb`, `Tool`, `FlagTable`, `VerbDeps`, `VerbContext`, `CallToolResult`), and the helpers they share: the shared flags lifted off a line, the optional-value rewrite, flag readers, place lookup, the path and access refusals, the release refusal, and the one failure printer for each door.

## Behaviour
1. `take_common` (933-959): walk the line; `--` moves itself and the rest to `rest` and stops; a word is common when it is `--host`, `--state`, `--json`, `--help` (name read before any `=`) or `-h` exactly; a string flag without `=` takes the next word, unless there is none, then it stays in `rest` where typed; anything else stays in `rest` in order. `--hostx` and `--json=1` stay or move by name: `--json=1` moves (the parse refuses it), `--hostx` stays.
2. `optional_values` (838-851): for each flag row with `value_with`, every bare `--<flag>` before the first `--` becomes `--<flag>=` when the `value_with` flag (as `--name` or `--name=...`) is not on the line, or no word follows, or the next word opens with `-`.
3. `flag` (971) reads a string value only; `flag_list` (972) a list only, else empty; `flag_set` is `=== true`.
4. `COMMON` (919-924): host, state (string), json, help (boolean, `-h`), in that order. `PICK_FLAGS` model, effort, access; `SEND_FLAGS` model, effort; both tables string rows in that order.
5. `place_named` (978-983): one `places.list`; the first row `names_place(row, word)` takes; else usage `no_such_place_refusal(word, table names)` with `Run wsp computers.`
6. `absolute_path(named, path)` (987-990): refused unless it opens with `/`: `<named>, absolute, and got "<JSON.stringify(path)>".` with `Give a path that opens with /, since whoever reads it works in a folder this line cannot see.`; `absolute_folder` uses `--cwd is a path on the machine` and passes None.
7. `access_word_of` (1000-1004): ask, auto-edit, full, plan; else usage `<access_words_line(given)>.` with the fixed fix.
8. `under(path, folder)` (1007): equal, or a prefix with one `/` between (a folder ending in `/` is used as is).
9. `release_refusal` (1035-1041): `said = this line runs wsp <mine> and <host> runs wsp <theirs>:`; mine older (by `compare_versions`) is `update_here(release_update_line(argv, theirs), theirs)`; else here (state, or a loopback address) is `release_here_fix(mine, road)`, elsewhere `update_there(mine)`. Kind usage, message `said + " " + fix`.
10. `failed(io, json, e, prefix)` (1076-1080): one `io.error` line, `json_line(verb_failure(e))` under --json, else `say_once(prefix, message)`; nothing on stdout; answers the class's exit code. `say_once` drops the prefix only where the message opens with the command name and a space, so `wsp exec: x` behind `wsp exec: ` prints twice (pinned).
11. `tool_failure(e, usage)` (1085-1088): with a usage, a message `validator_refusal` reads becomes `usage_refusal(said, "usage: <usage>")` first; the answer is `{content:[{type:"text",text:error}], structuredContent: verb_failure, isError:true}` in that key order.
12. `host_schema_refusal(e, usage)` (1099-1102): Some only where `validator_refusal(message)` reads the message.
13. `json_asked` (1091-1094): `--json` exactly, among the words before the first `--`.
14. `pick` (1069-1071): the given fields in order, absent dropped, `null` kept. `printable` (1063) drops C0 but tab and newline, DEL and C1; `cell` (1066) turns newline and tab to spaces then `without_control_chars`.
15. `opened_thread_line` (1056): `thread <id>` without an `opened`, else `opened(id, folder)`; `opened_thread_said` adds two spaces and `THREAD_PREFIX_WORD`.
16. `thread_id_of` (1047): `threadId ?? id`. `tool_name` (914): spaces to `_`. `has_tool` (909): Cli, ToolOnly and CommandTool entries.

## Edge cases
- `take_common`: a shared string flag at the line's end stays (`threads --host` reaches the parse, which refuses it by name). A string flag takes the next word whatever it is, `--` included: `exec t --state -- ls` lifts `--state --` and leaves `exec t ls`, so the command loses its separator (pinned in `lookup.json`; see After the port). `-hx` stays whole.
- `optional_values` leaves words after `--` alone.

## Behaviour nobody wrote down
- Rule 10.1: the shared flags are lifted before the verb's words are matched; any other flag before them stops the match.
- Rule 10.6: a failure is one stderr line; `left` never reaches the failure object.

## Decisions
1. `Flags` is an `IndexMap` in parse order; `FlagValue` replaces node's `string | boolean | string[]`.
2. The verb table's entries hold fn pointers (`RunFn`, `ToolCallFn`) so a table is plain data the modules build.
3. `Tool.input` is a list of `ToolInput` (name, kind, optional, describe), the zod shape as far as the command line and the parity rows read it; `tools/list`'s JSON Schema stays `wsp-mcp`'s recorded one.
4. `release_refusal` takes `argv` where the TS reads `process.argv`.

## After the port
- `take_common` lets `--state` swallow a following `--`, so `wsp exec t --state -- ls` reads `ls` as a positional and `--` as the state path.
- `failed`'s doc says a sentence that names itself prints alone, but `say_once` needs a space after the name, so `wsp exec: ...` prints the prefix twice.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 13 | `verbs_oracle::take_common_splits_the_shared_flags_off_the_line`, `json_asked_reads_json_before_the_first_double_dash` |
| 2 | `verbs_oracle::optional_values_gives_a_bare_value_with_flag_its_equals_sign` |
| 3 | `client_helpers::flag_reads_a_string_value_alone_and_flag_list_a_list_alone` |
| 4 | `common_is_host_state_json_and_help`, `pick_options_and_send_options_are_string_flags` |
| 5 | `io_roads::folders::an_unknown_computer_is_refused...`, `client_rows_reads::an_unknown_computer_is_refused...` |
| 6 | `absolute_path_refuses_*`, `absolute_folder_*`, `run_verb_answers_the_exit_code...` (the relative --cwd rows) |
| 7 | `access_word_of_takes_one_of_the_four_words_and_refuses_the_rest` |
| 8 | `under_is_the_folder_or_a_path_inside_it` |
| 9 | `release_refusal_names_both_releases...`, `release_refusal_of_a_newer_host_reads_this_processs_road...`, `release_words_*`, `release_here_fix_*`, `client_dial::the_release::*` |
| 10 | `failed_prints_one_line_on_stderr...`, `failed_says_the_prefix_once...`, `run_verb::a_failure_is_one_stderr_line...` |
| 11, 12 | `tool_failure_is_the_text_and_the_object_marked_as_an_error`, `host_schema_refusal_*` |
| 14 | `pick_drops_only_the_absent_fields...`, `printable_*`, `cell_*` |
| 15, 16 | `opened_thread_line_and_said_*`, `tool_name_joins_*`, `the_entry_accessors_read_the_entrys_own_fields` |
