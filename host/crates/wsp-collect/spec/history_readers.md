# history_readers

Source: `packages/collect/src/history/reader.ts` (86), `claude.ts` (49), `codex.ts` (43), `hermes.ts` (49) at `a3b870a6e`. Tests: `history.test.ts` `describe` Claude Code reader (3), Codex reader (1), Hermes reader (3), weighing by project (3). Rust: `src/history/{reader,claude,codex,hermes}.rs`; tests `tests/history_readers.rs` (10), `tests/history_hard.rs` `reader_helpers::*` (4) and `hermes::*` (2).

## What it does

One reader per session store format the catalog names. A reader lists its store's files with a stamp each, and turns one file into tool calls: a shell line, or any other tool by name. Nothing an agent saw is kept; the caller reduces the calls to counts (history_store).

## Behaviour

1. `stamp_of` (reader.ts:46-49): `stat`; not a file (a dir, missing) is `None`; else `{path, stamp: "<mtimeMs>:<bytes>"}` with the mtime written as JavaScript's `String(n)` (`wsp_words::js::number_string`): `1000`, `1500.25`.
2. `jsonl_files` (:53-61): over `fs.walk(root)` in walk order, a path ending `.jsonl` for which `session(path)` answers, stamped; a path that does not stamp is skipped.
3. `try_json`: `serde_json::from_str`, `None` on error. `is_record`: an object. `stem` (:82-86): the text after the last `/`, cut before its last `.` when that dot is past index 0.
4. `time_of` (:77-80): only a JSON string (a number, null, absent, an object or an array is `None`); V8's `Date.parse` ISO grammar: `YYYY`, `YYYY-MM`, `YYYY-MM-DD` (UTC midnight), optionally `T` (either case) and `HH:mm`, `HH:mm:ss` or `HH:mm:ss.f+` (the fraction truncated to ms, any number of digits), then `Z`/`z`, `±HH:mm` or `±HHmm`, or nothing (local time); `±YYYYYY` expanded years. Range checks are V8's, not the calendar's: month 1 to 12, day 1 to 31 whatever the month (`2026-02-30` rolls to March 2), hour 0 to 24 with `24:00:00` the next midnight, minute and second 0 to 59, offset hour 0 to 23; the result must be within ±8.64e15 ms. `-00:00` and `+14:00` are accepted. Padding, a trailing newline or junk, a space before `Z`, `,5` fractions and a bare number are `None`. Exact values: rule 4's table in `reader_helpers::time_of_*` and the w2 fixture. V8's legacy fallback forms (`2026/09/06`, `Sep 6, 2026`, `2026-09-06 10:01:00`, `12`) parse in the TS too; they are an accepted difference (see Decisions 2): `None` in Rust.
5. `claude_session(rel)` (claude.ts:12-18): split on `/`; no second part is `None`; second part `_cleared_sessions` gives `stem(third)` or `None`; a second part ending `.jsonl` gives its stem; any other second part (a session's folder) is itself.
6. `claude_call` (:20-25): `Bash` with a string `input.command` is a shell call; anything else (a non-object input included) is `Other{name}`. `folder` and `at` are carried as given.
7. Claude reader: files are `jsonl_files(root, f -> claude_session(f[root.len+1..]))`; holder is the first path segment under root (the project folder). Every `.jsonl` at any depth under any folder of the root is a transcript of the session `claude_session` names (`-p/memory/notes.jsonl` is session `memory`, `-p/s1.jsonl/x.jsonl` is `s1`, `-q/.jsonl` is `.jsonl`). `read`: no session for the path gives nothing; `fs.lines(file, Some("\"tool_use\""))`; a line is parsed only when it holds both `"tool_use"` and `"assistant"`; it must be an object with `type == "assistant"` and an object `message` whose `content` is an array; `folder` is a string `cwd` else none (`""` is a folder `""`); `at = time_of(timestamp)`; each content block that is an object with `type == "tool_use"` and a string `name` yields `claude_call(session, name, input, folder, at)`. A line that does not parse is skipped; an `Err` from `lines` is passed on as the stream's `Err` (the TS iterator throwing), converted to `HostError` with the io error's text.
8. `codex_call` (codex.ts:10-19): names `exec_command shell container.exec local_shell` with `cmd ?? command` (only a missing or null `cmd` falls through: `{cmd: 5, command: "ls"}` is Other, `{cmd: "", command: "pwd"}` a shell call with line `""`) a string are a shell call with that line; an array of strings is joined with one space; else `Other{name}`.
9. Codex reader: files `jsonl_files(root, stem)` (every jsonl); holder is the file itself; session is `stem(file)`. Reading every line (no `holding`): until a folder is known, a line holding the text `"session_meta"` (no `type` check) that parses to an object whose `payload.cwd` is a string sets it, `""` included, and once set no later line changes it; then a line holding `"function_call"` that is an object with `type == "response_item"`, an object `payload` with `payload.type == "function_call"` and a string `payload.name` yields `codex_call(session, name, args, folder, time_of(timestamp))`, where `args` is `try_json(arguments)` when it is a string, else `arguments` itself.
10. Hermes (hermes.ts:32-48): files is `stamp_of(root)` as a one-item list, else empty; holder the file. `read` runs `sqlite3 -readonly -json <file> <QUERY_WITH_CWD>` and, only when that answers `None`, the same with `QUERY`; both `None` is one `Err("<file> could not be read with sqlite3")`. A JS-trimmed empty answer is no rows; an answer whose JSON is not an array is `Err("<file>: sqlite3 returned no JSON")`. Each row that is an object with a string `session_id` yields `hermes_calls(session_id, tool_calls, cwd if a string)` (`""` is kept for both `session_id` and `cwd`; `null` cwd is none). The plain `QUERY` runs only when the first run answers `None`: a first answer of `[]` or of JSON that is not an array does not fall back.
11. `hermes_calls` (:21-30): `tool_calls` parsed when a string; not an array gives `[]`; each item with an object `function` and a string `function.name` yields `hermes_call(session, name, args, folder)`, `args` parsed when a string. `hermes_call`: `terminal` with a string `command` is a shell call, else `Other`; hermes calls never carry `at`.

## Edge cases

- `claude_session("-p/.jsonl")` is `.jsonl`; `-p/_cleared_sessions/x/y.jsonl` is `x` (oracle `reader.json`).
- `stem("/a/b/")` is empty; `stem("/a/b/.hidden")` is `.hidden`.
- A transcript line that is not JSON in the middle of a file is skipped and reading goes on (`counts_a_session_once_across_its_sub_agent_transcripts_*`).
- Hermes with no database: no files, so nothing is run (`reads_nothing_when_there_is_no_database_*`).

## Behaviour nobody wrote down

- Rule 4.25: the stamp's mtime text must be JavaScript's number text, so a cache written by the TS host stays valid for the Rust host and the other way round.

## Decisions

1. Date-time without an offset: local time, as JavaScript. The agents always write `Z`; the oracle ran at UTC, so the hard test skips that one value.
2. `Date.parse`'s legacy fallback (anything outside the ISO grammar of rule 4 that V8 still reads) is not ported: the agents write only `...Z` with milliseconds, and V8's legacy parser has no written grammar. A cache entry is a count, never a date, so nothing persisted depends on it. Flagged for the coordinator.

## After the port

- Codex only takes the folder from a `session_meta` line seen before a call; calls before it have no folder. Kept.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/history_readers.json` (`oracle/w2-history_readers.oracle.ts`), replayed by
`tests/history_hard.rs` `w2_oracle::readers_pure_functions_answer_what_the_ts_answered` and
`w2_oracle::readers_list_stamp_and_read_files_as_the_ts_did`.

- W1. `time_of`: rule 4's grammar, 65 values (`time_of`); the cases named `[local time, ...]` are checked only at UTC.
- W2. `stamp_of` writes the mtime as `String(n)` for odd values (`1500.25`, `0.1`, `1e21`, negative); a folder or a
  missing path is `None` (`stamp_of`). Node's `mtimeMs` carries sub-millisecond digits; a Rust stamp computed another
  way may differ in the last digit, so a cache the TS wrote may be read again once by the Rust host (never wrongly
  trusted).
- W3. Claude files: every `.jsonl` at any depth under a project folder, keyed by `claude_session`; a `.jsonl` at the
  root itself has no session and is not listed (`claude_reader_files`). `claude_session` rows (`claude_session`).
- W4. Claude read: the eighteen-line file of every shape; a path with no session opens nothing (`claude_reader_read`).
  `claude_call` keeps `""` folder and `0` time (`claude_call`).
- W5. Codex: `cmd ?? command`, an array joined with one space, an object `cmd` is Other (`codex_call`); every
  `.jsonl` (lower case only) is a rollout, `archived/` included (`codex_reader_files`); the folder is set by the
  first `session_meta` line whose `payload.cwd` is a string, `""` included, a later one never changes it
  (`codex_reader_read`).
- W6. Hermes: the fallback runs only when the first answer is `None`; `[]` and non-array JSON do not fall back;
  rows keep `""` session ids and cwds (`hermes_calls`, `hermes_reader_read`, `hermes_reader_files`).
- W7. `holder` per reader (`holder`).

## Rules pinned

| Rule | Test |
|---|---|
| 1 | `reader_helpers::a_stamp_is_the_mtime_as_javascript_writes_it_then_the_size` |
| 3 | `reader_helpers::stem_cuts_the_folder_and_the_last_extension_but_keeps_a_leading_dot` |
| 4 | `reader_helpers::time_of_reads_what_date_parse_reads_*` |
| 5 | `keys_a_transcript_to_its_top_level_session_*`, `reader_helpers::claude_session_keys_every_shape_*` |
| 6, 7 | `counts_a_session_once_across_its_sub_agent_transcripts_*`, `counts_nothing_from_a_session_whose_store_never_recorded_a_folder` |
| 8, 9 | `reads_exec_command_and_the_older_shell_array_*`, `takes_a_codex_sessions_folder_from_the_session_meta_*` |
| 10, 11 | `asks_sqlite3_read_only_*`, `falls_back_to_the_columns_alone_*`, `reads_nothing_when_there_is_no_database_*`, `hermes::*` |
