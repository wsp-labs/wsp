# road_modules (with road_release)

Source: packages/catalog/src/road-modules.ts (684) at a3b870a6e, split: road_modules.rs for the homes, Homebrew, the
package managers, go, the module table and the steps; road_release.rs for the release road's two scripts
(road-modules.ts:436-563). Tests: road-modules.test.ts (9) and the road cases of catalog.test.ts. Rust:
tests/road_modules.rs (9), tests/oracle_roads.rs (modules, 7: every module on 28 sample roads, own homes and wsp's
prefix), tests/catalog.rs.

## What it does

Each install road as a module: the line that installs, the line that takes off, the folders it writes and links
into, the names a row may carry, what it runs on, the version read back, and how it reads to a person. Where each
manager installs is one answer, `install_homes`: the machine's home with no prefix, a folder of wsp's own under
`/opt/wsp` for a computer somebody owns.

## Behaviour

1. `install_homes(None)`: pnpm at PNPM_HOME with its knob; bun, uv, pipx, cargo, go at their own folders under
   `/root`, no knobs. `install_homes(Some(p))`: each under `<p>/<name>`, commands in /usr/local/bin, with the knobs of
   road-modules.ts:164-169; pnpm and cargo carry `links` (their own bin folder) since they take no knob for it.
2. `home_bins`: a folder that is a manager's own bin under no prefix moves, under a prefix, to that manager's links
   and bin, both answering; every other folder stands (road-modules.ts:196-209).
3. Each module's words, roots, bins, after, present, check, shown, pastes, from_row, install, uninstall, env, names,
   bin, at and installed are road-modules.ts:312-607 exactly; the 28 sample roads in the oracle fixture cover every
   branch (a core and a tap formula, each package manager with and without version and pin, go with and without a
   module, the release road with full, half and no assets, with and without tag, repo, pin and arch, vendor, apt,
   empty apt, and three script rows).
4. A manager with `links` runs its own lines with `export PATH=<links>:$PATH; ` ahead and ends the install by
   linking every executable of `links` into `bin`; its uninstall removes that one link (road-modules.ts:214-225).
5. `go_binary`: the last path element of the module less `@version`, or the one before it where the last is `v2`+
   (`^v[1-9]\d*$`, more than one part).
6. The go `from_row` reads `path@version` from the row's first path, else from the label's `(...)`; an `@` at the start
   or the end reads as no module (road-modules.ts:401-406).
7. The release road: no repo gives the note "no GitHub release to install from"; no version (and no standing pin)
   "names no release tag; name the version"; assets give `asset_install`, else `release_install` with the standing
   pin. The two scripts are road-modules.ts:481-543 byte for byte: a road with both arches recorded renders only the
   asset path; otherwise the if/else with the go fall-through where `go` is given (`<go>@<tag>` unless it names one).
8. `install_env`: every module's knobs in ROADS order, later ones overwriting earlier keys in place.
9. `row_road(manager, row)` and `reads_row_road(manager)`: only managers whose module has `from_row` (brew, npm, pnpm,
   bun, uv, pipx, cargo, go).
10. `road_step`: the limit, retry and environment lines of road-modules.ts:666-679; `CURL_NET` is the one curl.

## Edge cases

- apt's installed line reads the first package, `''` for an empty list.
- `fixes_version` is true where the module has `at` or the road carries a version.

## Behaviour nobody wrote down

- Rule 4.26: `install_env` and every module's `env` keep the knob order of road-modules.ts:164-169.

## Tapes

places.setup and the recipe and doctor reads (#2443) read these lines.

## Decisions

1. `RoadModule` is one struct naming its road; each TS optional member is a method answering `None` where the TS
   module lacks it. `Result<String, RoadNote>` stands for `string | { note }`.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1, 2, 4 | road_modules: where_a_jobs_managers_install::* (5); oracle_roads: modules::the_homes_for_no_prefix_and_for_wsps_own_prefix_are_the_ts_homes, ::a_rows_own_bin_folders_move_with_their_manager_under_a_prefix |
| 3 | oracle_roads: modules::every_sample_road_installs_takes_off_reads_back_and_pins_as_the_ts_module_does, ::every_modules_words_roots_paste_after_and_knobs_are_the_ts_ones; road_modules: a_roads_own_presence_read::*, a_roads_own_check::*, the_directories_a_road_links_its_commands_into::* (2); catalog: catalog::has_one_module_per_road_with_its_words_and_each_module_writes_the_install_and_it, ::each_module_says_the_one_line_a_person_reads_for_an_install_whose_script_is_not, ::installs_every_road_inside_a_tree_a_workspace_on_a_computer_somebody_owns_can_se |
| 5, 6, 9 | oracle_roads: modules::a_managers_row_reads_the_road_the_ts_reader_reads, constants_and_scripts::the_pin_check_go_binary_formula_name_and_linuxbrew_lines_are_the_ts_text; catalog: catalog::says_which_managers_the_build_reads_a_row_s_road_off_every_package_manager_a_col |
| 7 | oracle_roads: modules::the_release_scripts_are_the_ts_text_for_every_shape_of_assets_and_pin; catalog: catalog::a_release_pin_s_sum_is_checked_only_against_the_file_of_the_arch_it_was_recorded |
| 8 | oracle_roads: modules::the_homes_for_no_prefix_and_for_wsps_own_prefix_are_the_ts_homes |
| 10 | oracle_roads: modules::every_road_steps_limit_retry_and_lines_are_the_ts_ones; catalog: catalog::gives_every_road_a_step_how_long_it_may_run_whether_a_run_that_hit_the_limit_is, ::the_curl_every_road_script_types_goes_through_one_function_that_clocks_the_conne, ::no_road_line_spells_curl_s_flags_itself_the_function_is_their_one_home |
