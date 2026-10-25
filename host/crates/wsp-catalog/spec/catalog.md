# catalog (with catalog_data, bwrap_apparmor)

Source: packages/catalog/src/catalog.ts (578), agents/*.ts (about 380 over twelve files), bwrap-apparmor.ts (33) at
a3b870a6e. Tests: catalog.test.ts (49). Rust: tests/catalog.rs (49), tests/oracle_catalog.rs (16: every row field for
field, and every reader on the oracle's inputs).

## What it does

The table of every agent wsp ships and every tool agents reach for, and the readers every other crate asks it
through: lookups by id, by package, by road and by a recipe row's id; the floor; pins; sign-in rows; install lines.

## Behaviour

1. `catalog()` is the eleven agents (claude, codex, gemini, opencode, pi, hermes, crush, qwen, goose, amp, cursor),
   then the tool rows in catalog.ts:239-312's order. The rows are data: `catalog_data` is generated from
   tests/fixtures/catalog.json by a script, never typed; `oracle_catalog::every_row_is_the_ts_row_in_the_ts_order`
   holds every field.
2. `catalog_agents`, `catalog_tools`, `default_agent` (the first agent), `mcp_agents` (catalog order),
   `mcp_agent_ids` (joined by ", "), `base_floor` (floor tools in catalog order), `floor_bytes(compiler)` (the floor's
   measured bytes, build-essential only with the compiler).
3. `guest_env` (catalog.ts:325): the state home variable at the guest state home where both are set, then the plugin
   seed's variable at its folder.
4. `harness_line` (catalog.ts:351): `cd '<folder>' 2>/dev/null; ` where a folder is named, then `export
   <VAR>='<store>'; ` where the agent has a state-home variable and `stores` names one for it, then the line.
5. Lookups: `catalog_tool_for(pkg)` by id, command, the road's names, covers, then a brought command (first tool in
   order); `catalog_tool_for_dependency` only by `depends[road]`; `catalog_tool_by_road`; `catalog_id_of_row`
   (an agents row its agent, a `tools/` row the tool its package names, else nothing); `base_entry_for` only a floor
   tool.
6. `at_catalog_pin(road, asked)` (catalog.ts:423): a road with no package, or no tool pinning that package on that
   road, or an ask at or above the pin (`compare_versions`), stays; no ask takes the pin; an older ask takes the pin
   with `pinned_note`, and where the tool names `pinNode` also the "or <asked> where the computer's Node is older than
   <node>, which <package> <version> needs" note and the road `below`. `below_line` wraps both install lines in `if
   <node_at_least>; then ... else ... fi`.
7. `node_at_least(floor)`: the awk line of catalog.ts:416 with the major and minor read as `Number` reads them
   (`NaN` for non-numbers, written `NaN`).
8. `base_note` (catalog.ts:455): "<name> is part of the base", or both majors where the computer runs another,
   cutting its version to as many parts as the pin names, `v` stripped, worded by `this_computer(platform)`.
9. `login_rows` (catalog.ts:492): per entry with a login, its row under `login_id_of`, then a keys row where it has
   key files; per entry without one, a row only where it mints a token or carries a note. `login_sign_in(row)` reads
   `logins/<id>` or a bare id, nothing for a nested path. `shared_on`, `sign_in_ways_of` (token, key where a key
   variable, machine where a login there), `shared_agents_on`, `setup_need_rows` and `apt_need_rows` (ids under
   `agents/<agent>/`).
10. `install_line` is the road module's install with no homes; a note is an error "<id>: <note>". `install_shown`
    is the module's shown words where it does not paste, else its shown line or the install line, and the module's
    words where that line spans lines.
11. `BWRAP_APPARMOR` is the setup row of bwrap-apparmor.ts:33, its three scripts joined as the TS joins them.

## Edge cases

- `agent_mark("crush")` and `agent_mark("gh")` are none; `agent_name("nothing")` is "nothing".
- `catalog_tool_for("")` and `catalog_tool_for("claude")` are none.

## Behaviour nobody wrote down

- Rule 4.26: every list is catalog order; `guest_env` and stores maps keep insertion order.
- The cut (#2441) may remove pin fields; the table's fixture is regenerated from the TS after it lands.

## Tapes

agents.read, recipes.*, places.setup, image.build (#2443).

## Decisions

1. Modules a TS row holds as objects (`mcp.format`, `context`, `hooks`, `plugins`, `pluginShelf`, `bundledSkills`,
   checks, logins, OAuth store, `trusts`) are enums serialized by id; `catalog_data` holds data only.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1 | oracle_catalog: every_row_is_the_ts_row_in_the_ts_order; catalog: catalog::gives_every_entry_its_own_id, ::keeps_one_module_per_agent_under_src_agents_each_registered_once_the_agents_firs, ::ships_the_six_agents_whose_project_state_a_move_can_follow_each_with_a_measured, ::carries_the_tier_1_rows_*, ::carries_the_tier_2_and_tier_3_rows_*, ::names_the_evidence_behind_every_default_*, ::gives_every_row_a_size_in_bytes_*, ::carries_no_token_looking_value |
| 2 | oracle_catalog: the_derived_lists_keep_catalog_order; catalog: catalog::the_default_agent_is_the_first_entry_and_it_is_an_agent_with_a_context_module, ::seeds_every_golden_with_the_base_floor_the_entries_flagged_for_it_in_catalog_ord |
| 3 | oracle_catalog: each_agents_env_setup_rows_ways_shared_login_name_mark_and_switch_are_the_ts_ones; catalog: catalog::a_golden_s_env_for_an_agent_points_it_at_its_guest_state_home_and_its_image_s_pl |
| 4 | oracle_catalog: a_harness_line_cds_into_the_folder_and_points_at_the_store_as_the_ts_does |
| 5 | oracle_catalog: a_package_name_finds_the_ts_tool_by_every_road_it_is_known_by, a_recipe_rows_catalog_id_and_a_login_rows_sign_in_are_the_ts_ones; catalog: catalog::finds_the_tool_a_package_name_stands_for_by_id_command_road_name_cover_or_brough, ::one_rule_names_the_id_a_recipe_row_is_known_by_on_every_computer_the_catalog_id, ::names_the_base_row_a_recipe_s_tools_row_stands_for_by_id_command_road_argument_o |
| 6, 7 | oracle_catalog: a_road_at_the_catalogs_pin_is_the_ts_answer_for_every_ask, the_below_line_installs_the_pin_on_a_new_node_and_the_ask_on_an_old_one, the_node_floor_test_and_the_pinned_note_are_the_ts_text; catalog: catalog::one_rule_says_the_version_a_road_installs_at_* |
| 8 | oracle_catalog: each_tools_install_after_and_base_note_are_the_ts_ones; catalog: catalog::a_covered_row_s_note_names_the_computer_that_was_read_and_both_majors_when_it_ru |
| 9 | oracle_catalog: the_login_rows_are_the_ts_rows_keys_rows_beside_their_logins; catalog: catalog::files_one_sign_in_row_per_login_id_and_a_keys_row_beside_a_login_whose_key_files |
| 10 | oracle_catalog: each_rows_install_line_shown_words_smoke_and_bytes_are_the_ts_ones; catalog: catalog::gives_every_entry_one_install_line_from_its_road_s_module_*, ::sends_every_entry_down_a_known_road_with_its_argument |
| 11 | oracle_catalog: the_size_methods_say_how_each_was_read_and_the_bwrap_row_is_the_ts_row |
