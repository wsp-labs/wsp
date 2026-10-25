# project_state_agents: each agent's module

Source: `packages/engine/src/project-state/{claude,codex,gemini,hermes,opencode,pi}.ts` (372) at `a4033dc34`. Rust:
`src/project_state/<agent>.rs`. Tests: `tests/project_state_agents.rs` (fixtures as the agents left them after one
headless run, compared byte for byte, sqlite by rows), the merge run tests in `tests/project_state_scripts.rs`. Oracle:
`project-state-scripts.json` (`resolvers`: listing, shared filter, key, hasMerge per module).

## claude (claude.ts)

- A1. Key `claude_project_key`; python `key_py()` as the oracle's listing carries it. Roots `["projects"]`.
- A2. move: `move_keyed_directories(<home>/projects, key, from, to)`; nothing changed is `[]`. Else `session
  transcripts` with its files and count, then `auto memory` `{files: [claude_memory_dir(home, key(to))], changed 1}`
  only where that folder exists after the move.
- A3. sessions and entries: `keyed_sessions` / `keyed_entries` over `<home>/projects`; listing one `keyed_step`.

## pi (pi.ts)

- A4. Key: `--` + path less one leading `/`, every `/`, `\` and `:` a `-`, + `--` (oracle `piKeys`: `/` is `----`,
  `/a/b/` is `--a-b---`). Roots `["sessions"]`; move gives `sessions` or `[]`; listing one keyed step.

## codex (codex.ts)

- A5. Index `state_5.sqlite`; threads of the project are `under_path("cwd")`. `rollout_under(home, recorded)`: the tail
  after the last `/sessions/` under `<home>/sessions` by `under_home`; no such segment or a climbing tail is `None`.
- A6. move: in one transaction on the index, the project's `rollout_path`s ordered by rollout_path, then `update
  threads set cwd = moved_column(cwd) where under_path(cwd)`. No index: `[]`. The recorded rollouts resolved under this
  home; present files kept once each in order, the rest counted as skipped. Threads changed > 0: `thread index {files:
  [index], changed, skipped only where > 0}`. Each present rollout gets `rewrite_cwd(f, from, to, at = line.payload)`;
  any rewritten: `rollout transcript {files, changed}`.
- A7. sessions: `select count(*) as n from threads where under_path(cwd)`; entries: the present rollouts of the index.
- A8. shared `{store: state_5.sqlite, filter: sqlite_filter([threads])}`; listing as the oracle carries it.
- A9. merge: the project's thread rows (Q5) whose rollout is present under this home; none is `None`. Rows moved: cwd
  by `moved_or`, rollout_path `rollout_under(guest_home, recorded)` or null. Script: `merge_script(from, to,
  [jsonl_cwd_step(each row's rollout_path, "payload"), sqlite_merge_step(<guest_home>/state_5.sqlite, [{threads, key
  [id], set [cwd, rollout_path], rows}])])`.

## gemini (gemini.ts)

- A10. Registry `projects.json` `{projects: {path: slug}}`; slug dirs `tmp` (`project temp dir`), `history` (`shell
  history`). A slug that climbs out of the home (`under_home`) is passed over everywhere.
- A11. move: no registry is `[]`. Every key under `from` with a safe slug is renamed to `moved_path`; a target already
  in the registry fails `<target> already exists in <file>` before anything is written. None renamed: `[]`. Else the
  registry is rewritten in place (key order kept, the renamed key in its old place) as `JSON.stringify(registry, null, 2)
  + "\n"` through config-here's `write_config_here(file, home, config_sum(read bytes), text)`; `project registry {files:
  [file], changed: renamed}`. Then per slug dir, each renamed slug's `.project_root` whose trimmed-end text equals the
  old path is rewritten (the first occurrence of the old path replaced, the rest kept) the same way; any rewritten:
  `{state, files, changed: files}`.
- A12. sessions: `.jsonl` files under `<home>/tmp/<slug>/chats` for each slug registered at or under the path, registry
  order. entries: every file under each slug's `tmp` and `history` dirs, all sorted.
- A13. shared: the registry filtered to keys under PATH (the oracle's filter lines). listing: each string slug of the
  copy, its tmp and history dirs said.
- A14. merge: `[old, target, slug]` for every registry key under `from`; none is `None`. The script (its whole text
  pinned since by `the_gemini_merge_script_is_the_ts_text`): creates HOME, reads or starts the machine's registry, fails `slug <s> already belongs to <other> in <REG>`
  before any write, keeps a slug the machine already maps to the target (`kept`, with a note where it is another slug),
  adds the rest (`merged`), rewrites a landed `.project_root` whose stripped text is the old path, writes the registry
  only where something was added.

## hermes (hermes.ts) and opencode (opencode.ts)

- A15. hermes: `state.db`; sessions at the path are `under_path(cwd) or under_path(git_repo_root)`. move: one update of
  both columns; changed > 0 is `sessions {files: [db], changed}`. entries `[]`. Tables: sessions keyed `[id]`, set `[cwd,
  git_repo_root]`, moved by `moved_or` on both (null stays null); messages `session_id in (<ids>)` by `session_id`, drop
  `[id]`, unmoved. merge: `shared_tables` none is `None`, else one `sqlite_merge_step(<guest>/state.db, tables)`.
- A16. opencode: `opencode.db`; three updates in one transaction: project (worktree moved, `sandboxes = '[]'`), project
  directories, sessions; each with changed > 0 is a row in that order. entries `[]`. Tables: project `[id]` set
  `[worktree, sandboxes]` (sandboxes `"[]"`), project_directory `[project_id, directory]` set none, session `[id]` set
  `[directory]`, message and part `[id]` set none, unmoved. merge as hermes.

## Null, empty, restart

A home without the module's files is `[]`, 0 sessions, no entries, no merge. Nothing is kept in memory; the moves
write in place, so what survives a restart is the moved files.

## Pinned since (M-rules, values from the TS's run, tests/fixtures/oracle/strengthen_pstate.json)

- M6. gemini writes the whole registry back as `JSON.stringify(registry, null, 2) + "\n"`: compact input comes out
  pretty, `1.0` is `1`, nested index keys reorder, an empty `{}` stays compact, other top-level keys keep their place.
- M7. The registry is written first, then the markers (states outer, folders inner), with no rollback: a marker with a
  second hard link fails with the hard-link sentence while the registry and the markers before it are rewritten and
  the later ones are not reached.
- M8. A registry with no `projects` fails the move `Cannot convert undefined or null to object` (while counts read it
  as 0); a non-string slug fails with node's `The "path" argument must be of type string. Received type number (5)`;
  a target held by `null` counts as taken (`<target> already exists in <file>`); an unsafe slug (`../x`) is skipped
  before the check; `projects: []` is `Nothing`. JSON parse failures carry V8's words, which the port cannot match:
  those messages are not compared.
- M9. The gemini merge script is the TS's text byte for byte (KEYS carries every key under `from`, an unsafe slug
  included); a slug registered twice is counted twice (sessions 2, bytes 22 for one 8-byte marker and one 3-byte chat).
- M10. `rollout_under` checks containment against the home, not `<home>/sessions`: `sessions/../state_5.sqlite` is
  `<home>/state_5.sqlite`, a path ending `/sessions/` is `<home>/sessions`. Codex's move is index first, rollouts
  after: a failing rollout leaves the index moved. hermes's merge fails `no such table: messages` when the source
  lacks it. A marker's trailing-space test is JS `trimEnd` (strips U+FEFF, keeps U+0085); not pinned.

## Rules and their tests

Files: `project_state_agents.rs` (agents), `project_state_core.rs` (core), `project_state_scripts.rs` (scripts),
`project_state_hard.rs` (hard).

| Rule | Tests |
|---|---|
| M6 | project_state_more.rs: `gemini_writes_the_whole_registry_back_as_js_stringify_does` |
| M7 | project_state_more.rs: `a_gemini_marker_that_cannot_be_written_leaves_the_registry_moved` |
| M8 | project_state_more.rs: `gemini_registry_shapes_answer_as_the_ts_does` |
| M9 | project_state_more.rs: `the_gemini_merge_script_is_the_ts_text`, `a_slug_registered_twice_is_counted_twice` |
| M10 | project_state_more.rs: `rollouts_index_keys_listing_lines_and_an_empty_note` |
| A1 to A3 | agents: the four `claude_` tests; core `a_claude_code_directory_holding_memory_alone_is_zero_sessions` |
| A4 | agents: the two `pi_` tests; hard `pi_keys_match_the_ts_oracle` |
| A5 to A9 | agents: the four `codex_` tests; scripts: the three `codex_` merge tests and `codex_the_copy_of_the_index_holds_the_projects_threads_alone_and_the_store_on_the_machine_is_not_touched` |
| A10 to A14 | agents: the four `gemini_` tests; scripts: the four `gemini_` merge tests and `gemini_the_copy_of_the_registry_holds_the_projects_keys_alone_in_the_shape_the_agent_writes` |
| A15, A16 | agents `hermes_updates_cwd_and_git_repo_root_on_rows_at_or_under_the_path_leaving_sibling_alone`, `opencode_updates_worktree_project_directory_and_session_directory_at_or_under_path_and_clears_sandboxes`; scripts: the hermes and opencode listing and merge tests |
| listings, filters, keys | hard `each_modules_listing_and_filter_match_the_ts`, `the_registry_matches_the_ts_modules` |
