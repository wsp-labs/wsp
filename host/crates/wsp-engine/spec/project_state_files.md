# project_state_files: the resolver's file operations and the sqlite reads

Source: `packages/engine/src/project-state/resolver.ts` (267) and the runtime half of `project-state/sqlite.ts`
(`underPath` to `selectRows`, `projectRows`, `sharedTables`, `runUpdate`, `updateRows`) at `a4033dc34`. Rust:
`src/project_state/resolver.rs`, `src/project_state/sqlite.rs`. Tests: `tests/project_state_files.rs`, hard tests in
`tests/project_state_hard.rs`. Oracle: `tests/fixtures/oracle/project-state-scripts.json` (`paths`).

## Paths (resolver.ts:68-100)

- F1. `resolve_project_path(path)`: node `path.resolve` against the process cwd (absolute, `.` and `..` resolved,
  repeated and trailing slashes dropped, `/` stays `/`), then where that names an existing directory its real path
  (links resolved); otherwise the resolved spelling. A stat failure of any kind is the resolved spelling. Never fails.
- F2. `moved_path(path, from, to)`: `to + path[from.len()..]` where `under_project(path, from)` (path equal to from, or
  from + `/` a prefix of it), else `None`. `/a/bx` is not under `/a/b`.
- F3. `inside_folder(rel)`: false for `""`, an absolute path, `..`, and anything opening `../`; true otherwise
  (`..cache`, `a/..cache/b`, `a/../..` lexically: `a/../..` opens with `a`, so true, as the TS reads it).
- F4. `under_home(home, parts)`: `join(home, parts...)` (posix join, normalised) when `relative(home, at)` is inside
  per F3, else `None`. No parts is `None` (relative is `""`). Oracle `paths.underHome`.
- F5. `moved_or(value, from, to)`: a JSON string moved per F2 where under, any other value (null, a number) unchanged.

## Keyed directories (resolver.ts:103-192)

- F6. `recorded_cwd(dir)`: across `files_under(dir, ".jsonl")` in order, each file's lines split on `\n` (a last line
  with no newline read too), the first line that parses as a JSON object whose `cwd` is a string. Unparsable lines,
  arrays, a non-string cwd are passed over. `None` where none.
- F7. `keyed_directories(parent, key_of, from)`: nothing where parent does not exist. `own = key_of(from)`, `stem` the
  longest common prefix of `own` and `key_of(from + "/x")`. Entries of parent whose name starts with stem, sorted by
  name (UTF-16 order), that are directories (stat follows links): cwd is `recorded_cwd(dir)`, else `from` where the name
  is exactly `own`; kept where that cwd is under `from` (F2). Answer `{dir: join(parent, name), cwd}` in that order.
- F8. `move_keyed_directories`: for each keyed directory, `target = moved_path(cwd)`; the new directory is
  `join(parent, key_of(target))`; if it exists fail `Other("<moved> already exists")` (earlier renames stay); rename;
  push it to `files`, `changed += 1`; then each `.jsonl` under the moved directory gets `rewrite_cwd(f, from, to)` and,
  where it rewrote n > 0 lines, is pushed and `changed += n`.
- F9. `keyed_sessions`: the regular `.jsonl` files directly inside each keyed directory (not in subfolders), summed.
  `keyed_entries`: `files_under(dir, "")` of each keyed directory, concatenated in directory order.
- F10. `files_under(dir, ext)`: nothing where dir does not exist; every regular file (a symlink is not one) at any
  depth whose name ends with ext; full paths sorted by UTF-16 code unit order.

## rewrite_jsonl (resolver.ts:208)

- F11. Reads the file as UTF-8 text split on `\n`; each line (the last one too, even with no newline after it, and an
  empty last part is not a line) that parses as a JSON object and that `edit` answers true for is written back as its
  `JSON.stringify` (JS key order: integer-like keys first ascending, then insertion order; numbers in JS spelling);
  every other line, and every `\n`, as decoded: the file is decoded as UTF-8 *lossily* (node's stream with
  `encoding: "utf8"`), so an invalid byte is U+FFFD on every line of a rewritten file, edited or not (M2). A truncated
  last line passes through untouched. "Integer-like" means a canonical array index only (`0` to `4294967294`, no
  sign, no leading zero): `01`, `-1`, `4294967295`, `1.5` stay in place.
- F12. Writes to `<file>.wsp-move`. With no line rewritten, the temp file is removed and the original is not touched
  (mtime kept); answers 0. Else the temp file takes the original's mode and is renamed over it; answers the count.
- F13. Any failure (the file missing, an edit failing, a write failing) leaves no `<file>.wsp-move` and the original as
  it was. A missing file fails with `Other { message: "ENOENT: no such file or directory, open '<file>'", code:
  Some("ENOENT") }`. An edit's own failure is passed through as it was.
- F14. `rewrite_cwd(file, from, to, at)`: the holder is `at(line)` (the line itself when `None`); a holder that is not
  an object or a cwd that is not a string is no change; a cwd under `from` becomes `moved_path`.

## sqlite (sqlite.ts)

- Q1. `under_path(c)` = `(<c> = $from or substr(<c>, 1, length($from) + 1) = $from || '/')`; `moved_column(c)` =
  `iif(<under_path(c)>, $to || substr(<c>, length($from) + 1), <c>)`. Oracle `underPath`, `movedColumn`.
- Q2. A database file that does not exist is never opened or created: `count_rows` 0, `select_rows` `None`,
  `project_rows` empty, `update_rows` `None`, `select_then_update` `None`.
- Q3. Reads open read-only. `count_rows` answers the `n` column as a number. Values read as node:sqlite does: integer,
  real, text, blob, null, columns in select order.
- Q4. `update_rows`: one transaction (`begin` ... `commit`) for all updates; answers each update's changed-row count in
  order. A failing statement fails the call and nothing of the transaction is kept.
- Q5. `project_rows(db, t, from)`: `select * from <table> where <where> order by 1, 2` with `$from`.
- Q6. `shared_tables`: per table in order, its project rows each passed through `move(row, from, to)`; `None` where
  every table has no row. The table keeps its mode for the merge step.

## Null, empty, restart

- An empty file has no lines (0 rewritten, untouched). A home without the parent is nothing. Nothing is held between
  calls; what survives a restart is only what was written to disk.

## Pinned since (M-rules, values from the TS's run, tests/fixtures/oracle/strengthen_pstate.json)

- M2. A rewritten line is `JSON.parse` then `JSON.stringify`, so the port needs a JS-faithful reader and writer, not
  serde's defaults: integers past 2^53 round to the f64 (`9007199254740993` is `...992`, `12345678901234567890` is
  `12345678901234567000`); `1e400` is `null` and the line still counts; a lone surrogate escape (`"\ud83d"`) parses and
  is written back as that escape; `-0` is `0`, `1E2` is `100`, `1.50` is `1.5`, `0.0000001` is `1e-7`, `1e21` is
  `1e+21`; a duplicate key keeps its first place and its last value; index keys move first, nested objects too; a
  rewritten line loses its `\r` and its outer spaces, an untouched line keeps its own; a line opening with a BOM is
  not JSON (n is 0, the file untouched).
- M2a. `JSON.parse` reads any depth (5 million levels) and `JSON.stringify` throws `RangeError: Maximum call stack size
  exceeded` from about 3,650. A line is read without recursion; what lies below 3,500 levels is checked for syntax and
  left out (the 3,500 is an accepted difference, README.md: a line between 3,500 and 3,650 levels that must change
  succeeds in node and fails here). A line left out that `edit` answers true for fails the rewrite with that RangeError's words and leaves the
  file; one `edit` leaves alone is passed over as it is. A string holds a lone surrogate as an escape char and the
  private-use char that stands for it, a real escape char twice, so U+10F800 to U+10FFFF written raw survive a rewrite.
- M3. `files_under` follows a linked folder (Node's recursive `readdirSync` does), spelling paths through the link, and
  sorts whole paths by UTF-16 unit. On a regular file it fails `ENOTDIR: not a directory, scandir '<file>'`.
- M4. A dangling link among a keyed parent's candidates fails the whole call, `ENOENT: no such file or directory, stat
  '<path>'` (an unguarded `statSync`); a plain file there is skipped. Two paths with one key collide with `<moved>
  already exists`, nothing renamed. A keyed folder that is a link is renamed as the link, the rewrite going through it.
- M5. `rewrite_cwd` on a link to a file replaces the link with a regular file and leaves the target as it was (the
  write is by rename); on a folder it fails `EISDIR: illegal operation on a directory, read` and leaves no temp file; a
  stale `<file>.wsp-move` is overwritten and gone after. Node's io errors are rendered as node renders them,
  `<CODE>: <text>, <syscall> '<path>'`.
- Q (sqlite): opened with no busy timeout (node:sqlite's 0, rusqlite's default is 5 s): a lock fails at once as
  `database is locked`. An integer past 2^53 is the error `Value is too large to be represented as a JavaScript
  number: <n>` (node:sqlite without big ints). `shared_tables` keeps a table with no rows as `rows: []` and is `None`
  only when every table is empty; a table missing from the source fails `no such table: <t>`. `select_then_update`
  reads a NULL `rollout_path` as the string `"null"` (`String(...)`), which then counts as skipped.

## Rules and their tests

Files: `project_state_files.rs` (files), `project_state_agents.rs` (agents), `project_state_core.rs` (core),
`project_state_hard.rs` (hard).

| Rule | Tests |
|---|---|
| M2 | project_state_more.rs: `a_rewritten_line_is_js_parse_then_js_stringify` |
| M3 | project_state_more.rs: `files_under_follows_a_linked_folder_and_sorts_whole_paths` |
| Q busy timeout, 2^53 | project_state_more.rs: `sqlite_never_waits_on_a_lock_and_refuses_an_integer_past_2_53` |
| F1, F2 | files `a_path_is_under_the_project_at_the_root_or_inside_never_a_sibling_trailing_slash_is_not_part_of_key`, `keys_on_the_real_path_a_symlinked_destination_resolves_before_the_dashed_key_is_built`; hard `resolve_project_path_resolves_dots_and_links` |
| F3, F4 | files `a_name_an_archive_carried_stays_inside_the_home_and_a_name_starting_with_two_dots_is_a_folder`; hard `paths_match_the_ts_oracle` |
| F6, F7, F9 | agents `claude_renames_dashed_directories_rewrites_cwd_on_every_line_and_leaves_sibling_alone`, `claude_moves_a_directory_with_no_transcript_only_when_its_key_is_the_projects_own`, `pi_renames_double_dashed_directories_of_root_and_sub_folder_and_rewrites_header_cwd`; core `counts_every_agents_sessions_at_the_path_or_under_it_never_a_sibling_another_project_or_a_subagent`, `names_the_files_holding_the_projects_state_alone_never_a_shared_index_or_another_projects` |
| F8 | agents `claude_refuses_to_move_onto_a_directory_that_already_exists_rather_than_merge_into_it` |
| F10 | hard `files_under_skips_links_and_sorts_by_utf16` |
| F11, F12 | hard `rewrite_jsonl_reorders_integer_keys_on_a_rewritten_line_only`, `a_file_with_nothing_to_rewrite_is_not_touched`; agents `claude_renames_dashed_directories_rewrites_cwd_on_every_line_and_leaves_sibling_alone` (the truncated tail line) |
| F13 | files `a_rewrite_that_fails_leaves_the_file_as_it_was_and_no_wsp_move_beside_it` |
| Q1 to Q6 | agents `codex_updates_threads_cwd_and_rollouts_for_root_and_sub_folder_leaving_rollout_path_and_sibling`, `hermes_updates_cwd_and_git_repo_root_on_rows_at_or_under_the_path_leaving_sibling_alone`, `opencode_updates_worktree_project_directory_and_session_directory_at_or_under_path_and_clears_sandboxes`; hard `update_rows_rolls_back_every_update_when_one_fails`, `a_missing_database_is_never_created`, `sqlite_filter_and_merge_step_match_the_ts_byte_for_byte` |
