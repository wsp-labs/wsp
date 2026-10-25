# roads (with linux_casks, release_pins, mac_paths, base_image)

Source: packages/catalog/src/roads.ts (416), linux-casks.ts (130), release-pins.ts (51), mac-paths.ts (16),
base-image.ts (22) at a3b870a6e. Tests: the road cases of catalog.test.ts. Rust: tests/oracle_roads.rs
(constants_and_scripts, 9), tests/catalog.rs.

## What it does

The install roads' shape, the pin rules every road reads, and the fixed installers, pins and tables the script,
vendor and release roads carry. Almost all of it is data: every constant and every installer text is held to the TS
text byte for byte through tests/fixtures/roads.json.

## Behaviour

1. `InstallRoad` is the TS union with `road` as its tag; optional fields absent rather than null (roads.ts:65-88).
2. `pin_state_of(version, pin)`: none without a pin; same where there is no version or it equals the pin's tag; else
   moved (roads.ts:19). `standing_pin` is the pin while same; `unpinned` drops the `pin` field only;
   `version_of` is the road's own version, else the standing pin's tag (roads.ts:97).
3. `pin_check_line` (roads.ts:39): `[ "$sum" = '<sha>' ] || { echo "Error: <pin_mismatch_line(what, tag,
   short_sum(sha), ${sum:0:12})>" >&2; exit 1; }`.
4. `is_road` answers for the twelve words exactly; `ROADS` is their order (roads.ts:91-93).
5. `rewritten_rel` (roads.ts:120): the first rewrite whose prefix starts the path moves it; a prefix ending `/` also
   moves the bare folder, to `to` less its trailing slash.
6. Every installer text is the TS array joined by `\n`, with the pinned versions and sums interpolated:
   CLAUDE_INSTALL, GOLDEN_SETUP, UV_INSTALL, DOCKER_INSTALL, PYTHON_INSTALL, RUSTUP_INSTALL, FD_INSTALL,
   YARN_INSTALL, HERMES_INSTALL, PLAYWRIGHT_INSTALL, SWIFT_INSTALL, OP_INSTALL, and `node_install_script(floor,
   release)` (roads.ts:153-416).
7. The casks (linux-casks.ts): gcloud, kubectl, cursor-agent, each a version, two sums, an install script built from
   the shared prelude and arch line, and an uninstall line.
8. `pinned_release(repo, go)`: the table's tag and assets, `go` only where given; "the release pins table names no
   <repo>" otherwise (release-pins.ts:373). The table keeps its order.
9. `MAC_ONLY`, `MAC_BIN_DIRS` and `BASE_IMAGE_COMMANDS` are the TS lists in order.

## Edge cases

- `rewritten_rel(".claude")` is `.claude-cfg`; `".claude.jsonx"` moves too (a plain prefix match); `""` stays.
- `node_release(18)` is none: only majors 20 and 22 are pinned.

## Behaviour nobody wrote down

- Rule 8.19: every constant keeps its unit (MIB in bytes).
- The cut (#2441, "Version pins") may remove the pin rules here (rules 2 and 3); their tests are the four
  `pin` cases of oracle_roads and catalog.rs's pin tests, which go with them.

## Tapes

None directly; the install lines reach a computer through places.setup and image.build (#2443).

## Decisions

1. Literal constants are copied into the stub; every text the TS builds is a `LazyLock` built the same way, never
   pasted, so a bump in one place reaches every text.
2. `NODE_RELEASES` is an array of (major, release) pairs for the TS object keyed by major.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1, 6, 7, 9 | oracle_roads: constants_and_scripts::every_constant_and_pinned_installer_is_the_ts_text_byte_for_byte, ::the_casks_and_the_release_pins_are_the_ts_tables_in_order, ::the_mac_paths_and_the_base_image_commands_are_the_ts_lists, ::the_node_install_script_keeps_a_node_between_the_floor_and_its_own_major; catalog: catalog::the_default_agent_is_the_vendor_s_own_binary_at_a_pinned_version_checked_against, ::runs_cursor_s_agent_from_its_vendor_s_linux_download_at_a_pinned_version_checked, ::cursor_s_install_unpacks_the_checked_download_under_opt_and_links_both_its_names |
| 2 | oracle_roads: constants_and_scripts::a_pin_stands_while_the_version_is_its_tag_or_none_and_goes_once_it_moved, modules::every_sample_road_installs_takes_off_reads_back_and_pins_as_the_ts_module_does; catalog: catalog::one_rule_says_which_pin_stands_and_one_line_checks_it_the_sum_the_tag_and_both_s |
| 3 | oracle_roads: constants_and_scripts::the_pin_check_go_binary_formula_name_and_linuxbrew_lines_are_the_ts_text |
| 4 | oracle_roads: constants_and_scripts::a_word_names_a_road_only_where_it_is_one_of_the_twelve |
| 5 | oracle_roads: constants_and_scripts::a_home_relative_path_lands_where_the_guest_rewrites_put_it |
| 8 | oracle_roads: constants_and_scripts::a_pinned_release_carries_the_tag_assets_and_go_and_refuses_a_repo_the_table_lacks; catalog: catalog::names_the_artifact_and_the_sum_before_anything_runs_every_release_row_a_pinned_t |
