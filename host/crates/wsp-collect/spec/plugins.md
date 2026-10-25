# plugins
Source: `plugins.ts` (120 lines) at a3b870a6e. Tests: `test/plugins.test.ts` (8 cases, one nested describe, one
`it.each` of 2 rows). Rust: `tests/detect_plugins.rs`. The reading itself is each agent's `PluginShelf::read` in
wsp-catalog; this unit is the computer's side of it (the `PluginIo`), the row shape and the fan-out. The row tests run
on the real filesystem through `live_host::node_host()` and stay red until both it and the catalog's shelves exist.

## What it does
For every catalog agent with a plugin shelf, runs the shelf's read against a `PluginIo` over the host, turns each found
plugin into the report's `PluginRow` with home paths `~`-relative, and gathers every refusal.

## Behaviour
1. `detect_plugins(host, agents, projects)` (plugins.ts:279-294): agents are `agents` or `catalog_agents()`, filtered
   to those with `plugin_shelf`; projects map to `PluginProject { id, name, path }` (path as given, absolute). Every
   agent's read starts at once; results keep agent order. `plugins` is every agent's rows in agent order then the
   shelf's order; `refused` is every agent's refusals in agent order; `agents` holds `{agent: id, shelf, found}` each.
2. `plugin_io(host, agent)` (plugins.ts:253-264): `home` is the host's; `store` is `host.stores[agent]` and `seed` is
   `host.seeds[agent]`, each absent when unset. `peek(groups)` runs `sh ["-c", peek_line(groups)]` with
   `timeout_ms: 30000`; `run(line)` runs `bash ["-c", harness_line(agent, line, host.stores, None)]` with
   `timeout_ms: 30000`. It never edits: `can_edit` is false and `edit` is never called by a read.
3. `peek_line(groups)` (plugins.ts:229-235): for each group, `what` is `shell_quote(files joined by "\n")`, a space,
   `shell_quote(each dir as "<depth> <path>" joined by "\n")` (absent lists are empty). A group with roots adds
   `peek <what> <each root shell_quote'd, joined by " ">`; a group with `roots_from` adds
   `from <shell_quote(key)> <each file shell_quote'd, joined by " "> | while IFS= read -r r; do peek <what> "$r"; done`;
   a group with both adds both, roots first; one with neither adds nothing. The line is PEEK, the calls, then
   `printf '\036END\n'` (a literal backslash and digits), joined by `\n`.
4. `peek_records(said)` (plugins.ts:236-249): `None` when `said` is `None` or its `js_trim_end` does not end with
   `\u{1e}END`. Else split on `\u{1e}`, drop the first piece; a piece with no `\u{1f}` is skipped; its first unit is the
   kind, the text up to the first `\u{1f}` is the path, keyed by `join_path(path)`. `f`: the rest is the file's text
   (a later record for the same path replaces it). `d`: the rest split on `\u{1f}`, pieces `""` and `"\n"` dropped, is
   the folder's list. `r`: the root was looked at. Lookups go through `join_path` too, so `/a//b/` finds `/a/b`.
5. `plugin_row(home, agent, p)` (plugins.ts:267-276): `agent`, then the found plugin's fields less `skills`, with
   `project.path`, `path` and `set_in` passed through `tilde(home, _)`; every other field as found.
6. A read cannot fail in Rust (`PluginShelf::read` returns `PluginsFound`): the TS `.catch` that turned a thrown read
   into the refusal `plugins: <agent name>'s plugins were not read: <message>` has no Rust counterpart (plugins.ts:285).

## Edge cases
- `detect_plugins` with no agent that has a shelf: all three lists empty, nothing run.
- `peek_records(Some("\u{1e}END\n"))` is `Some` with nothing in it; `Some("")` is `None`.
- A file whose text holds `\u{1e}` is cut there (the next piece is read as a record).

## Behaviour nobody wrote down
- Key order (rule 4.26): the TS row object writes `project`, `path` and `setIn` after `brings`, since they are spread
  last; wsp-wire's `PluginRow` serializes in its schema order (project after scope, path and setIn after missing).
  The wire's order wins in Rust; if the parity harness compares key order inside `agents.report`, this is an accepted
  difference for wsp-wire to rule on, not this unit.

## Decisions
1. The TS `pluginIo` returns an object literal; Rust has `HostPluginIo { host, agent }` implementing the catalog's
   `PluginIo`, plus `peek_line` and `peek_records` split out of `peekOn` so they can be tested without a computer.

## After the port
- A peeked file holding the record separator is silently cut short.

## Tests
| Rule | Test |
|---|---|
| 1 | `keeps_claude_codes_rows_when_codexs_app_server_does_not_answer`, `detect_plugins_reads_nothing_for_agents_without_a_shelf` |
| 2 | `reads_claude_code_under_its_store_in_one_line_and_asks_codexs_app_server_under_the_stores_codex_home`, `asks_a_second_time_by_name_for_an_install_path_the_peek_could_not_read_off_the_index`, `plugin_io_runs_peek_under_sh_and_lines_under_bash_with_a_30_second_timeout`, the seed cases (`lists_each_seeded_plugin_as_installed_from_the_seed_after_turns` and the three after it) |
| 3 | `peek_line_quotes_every_group_and_ends_with_the_end_mark` |
| 4 | `peeks_a_folders_names_two_levels_down_and_a_files_text_and_nothing_of_what_is_not_there`, `peek_records_reads_records_by_joined_path_and_refuses_an_answer_cut_short` |
| 5 | `plugin_row_puts_home_paths_under_tilde_and_drops_skills` |
