# project_state_core: the registry and the walks over the catalog's agents

Source: `packages/engine/src/project-state/index.ts` (162) at `a4033dc34`. Rust: `src/project_state/mod.rs`. Tests:
`tests/project_state_core.rs`, `tests/project_state_files.rs` (moveProjectState), hard tests in
`tests/project_state_hard.rs`. Oracle: `project-state-scripts.json` (`resolvers`, `stateListings`, `unknownAgent`,
`agentHomes`).

- P1. `PROJECT_STATE_RESOLVERS`: claude, codex, gemini, hermes, opencode, pi in that order, each keyed by its own
  `agent()`. Each module's `states` are measured rows of its catalog entry; `carry` moves for claude and pi,
  transcript-only for the rest (oracle `resolvers`).
- P2. `project_state_key(agent, path)`: the module's `key(resolve_project_path(path))`; `None` for an agent without a
  module or without a key (only claude has one: `claude_project_key`).
- P3. `move_project_state(mv, agents)`: `from` and `to` resolved first (F1). For each agent in order: `from == to` is
  `Nothing` (checked before anything else, a module or not); no module is `TranscriptOnly` (its home never opened);
  a home not in the map or not on disk is `Nothing`; else the module's `move_state(home, from, to)`: an empty answer is
  `Nothing`, a non-empty one `Moved`, a failure `Failed(message)` and the walk goes on. The home is *not* put back: only the sqlite moves are transactional;
  claude and pi keep the folders renamed before the failing one (renamed and rewritten), codex keeps its committed
  index (and a second move then answers `Nothing`, so it does not resume), gemini keeps its rewritten registry and the
  markers written before the failing one (M1, M7). No partial `Moved` list is reported, only the failure.
  `None` agents is the catalog's ids.
- P4. `agent_homes(home_dir, vars)`: every catalog agent in catalog order (claude, codex, gemini, opencode, pi, hermes,
  crush, qwen, goose, amp, cursor today); the value of its `state_home_env` variable where set and non-empty (an empty
  value falls through), else `join(home_dir, state_home)`. `agent_home` falls back to `join(home_dir, ".<id>")`.
  `guest_agent_homes` uses the guest state home where the entry names one (claude: `/root/.claude-cfg`). Oracle
  `agentHomes`.
- P5. `state_listing(homes, path, scratch, ids)`: an id not in the catalog fails first with `unknown_agent_line(id,
  catalog ids)` (oracle `unknownAgent`). Agents in catalog order with a home in the map, a module, and (where `ids` is
  given) named in it. Steps: the module's `listing(home, path)` (none is empty); a module with a shared store wraps them
  in one `filter_step(scratch, home, shared, own)`; no steps is `[ROOTS_STEP]`. One `list_script(id, home, path, roots,
  steps)` each. No agent qualifies: `""`. Else `set -e` then `python3 -c <shell_quote(script)>` per agent, joined by
  `\n`. Oracle `stateListings`, byte for byte (an empty `ids` list and an id with no module give `""`).
- P6. `count_project_state(path, homes, agents)`: root resolved. Agents in order with a module and a home on disk;
  `sessions(home, root)`; 0 sessions is no row; else bytes is the sum of the sizes (stat, links followed) of
  `entries(home, root)`, row `{agent, name, sessions, bytes, carry}`. A failure of sessions, entries or a stat keeps the
  row as `{agent, name, sessions 0, bytes 0, carry, error: message}` and the count goes on.
- P7. `agents_on_machine(machine, ids)`: the catalog agents named by `ids` in catalog order; none asks nothing and
  answers empty. Else golden-tools' `missing_commands(machine, their bins, TOOLS_PATH, None)`; a failed read is `Other("could not see
  which agents are on the machine: <failed>")`; else every named agent whose bin is not missing.

Null and empty: no homes is no rows and no listing; an empty `ids` is the empty listing (not every agent). Ordering is
the catalog's, never the map's. Nothing survives a restart beyond what the modules wrote on disk.

## Pinned since (M-rules, values from the TS's run, tests/fixtures/oracle/strengthen_pstate.json)

- M1. A claude move whose second keyed folder collides fails with exactly `<parent>/<dashed target> already exists`,
  having already renamed and rewritten the first; the colliding folder and the old second folder are left as they are.
- `state_listing` hands its `path` to the scripts as given (never resolved, never normalised: python does
  `os.path.normpath` on the machine); `agent_homes` keeps an env value verbatim (`/opt//claude/`) and joins `home_dir`
  with node's `path.join` (`""` gives `.claude`).

## Rules and their tests

Files: `project_state_core.rs` (core), `project_state_files.rs` (files), `project_state_scripts.rs` (scripts),
`project_state_hard.rs` (hard).

| Rule | Tests |
|---|---|
| M1 | project_state_more.rs: `a_failed_claude_move_leaves_what_it_already_moved` |
| P1 | files `registers_each_module_under_its_own_catalog_agent_naming_only_measured_rows_of_that_entry`; hard `the_registry_matches_the_ts_modules` |
| P2 | hard `project_state_key_is_claudes_alone` |
| P3 | files `walks_every_catalog_agent_two_present_move_the_four_absent_report_nothing`, `reports_a_catalog_agent_without_a_module_as_transcript_only_and_never_touches_its_home`, `keys_on_the_real_path_a_symlinked_destination_resolves_before_the_dashed_key_is_built`, `does_nothing_when_the_machine_mirrors_the_old_path`, `reports_one_agents_failure_and_still_moves_the_others` |
| P4 | core `agent_homes_places_every_registered_agents_home_under_the_given_home_directory`, `one_harness_home_under_a_home_directory_is_one_rule_the_persons_variable_wins_else_the_catalogs`; hard `agent_homes_match_the_ts_oracle` |
| P5 | scripts `state_listing_runs_one_python3_per_agent_with_a_home_and_a_module_narrows_and_refuses_an_unknown_id`; hard `state_listing_matches_the_ts_byte_for_byte` |
| P6 | core `lists_each_agent_whose_home_holds_sessions_with_its_catalog_name_in_catalog_order`, `keys_on_the_real_path_and_skips_a_catalog_agent_with_no_module_whose_home_it_never_opens`, `an_agent_whose_store_cannot_be_read_keeps_its_row_with_the_reason_and_the_others_are_counted` |
| P7 | hard `agents_on_machine_reads_the_missing_lines` |
