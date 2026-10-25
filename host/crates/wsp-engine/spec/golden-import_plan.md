# import_plan (golden-import.ts:1009-1015, 1073-1234)

`tool_installs_for_rows(entries, table, custom, path, prefix)` is `toolInstallsFor`: a recipe's ticked tools rows as
the steps of the one tools loop, each waiting on what it needs, with the skips, the base rows and the Brewfile. Pure.
The order of `installs` is contract: the loop runs it as given and a step waits on one before it.
`tool_installs_for(ids, path, prefix)` is the provision tests' call: rows built as provision.test.ts:121 builds them.

## Walk (:1083-1234)

`viaBrew(f)` is `via_road({ brew, formula: f }, f, path, prefix)`, whose note is an error `<f>: <note>`. Manager order
is npm, pnpm, bun, uv, pipx, cargo, go. A manager's own step id is `tools/manager/<m>`.

1. `brew = brewfile_for(entries, table)`; `skipped` starts as its skips. `tool_rows`: ticked tools rows with no base
   row (read with an empty table). `rows_of(m)`: those under `tools/<m>/`.
2. `catalog`: the tool rows under `tools/catalog/` or that `catalog_tool_of` answers, each with `row_road(e, table)`;
   one that plans no road is skipped `not in the catalog`.
3. The toolchain: per `BREW_TOOLCHAIN` formula `f`, `tools/brew-toolchain/<f>`, label `Homebrew's <f>`, manager
   `brew`, viaBrew, check `brew_has_check([f])`, after the one before (the first after `tools/homebrew`). `last` is
   the last of them.
4. `custom_of(m)`: the custom rows whose `manager` is exactly the road name `m`.
5. Resolving a manager (once each): nothing for a manager the base carries. Else, first that holds: the Brewfile's
   formulae hold the manager's formula or its own name: it waits on `tools/brew/<that>`; a catalog row is the
   manager's own tool: it waits on that row, which is planned with the manager; an npm global of that name is
   ticked: it waits on `tools/npm/<m>`; the manager has a formula: its own step (label `m`, manager brew, viaBrew,
   `road_reads({ brew, formula }, m, prefix)`, after `last`, bin `m`) and the formula joins the shared list; the
   catalog has its tool: its own step by that road (label the entry's name, bin the entry's; a note is the error
   `<entry id>: <note>`). Else the error `<m> is neither in the base, nor in the catalog, nor a formula`.
6. Resolve, in manager order, each manager with rows, catalog rows on its road, or custom rows. Then for each catalog
   row whose dependency (its own `after`, else its road module's `after`) is the tool of a manager, resolve that one.
7. Homebrew: when the Brewfile has taps or formulae, or there are manager formulae, catalog rows on the brew road
   (their module's `names`), or custom brew rows: `tools/homebrew` (label `Homebrew`, manager brew, cmd
   `path_line` + the bootstrap script, shown `git clone github.com/Homebrew/brew at <HOMEBREW_TAG>`, bin `brew`, present
   `BREW_SHIM_PRESENT`); the toolchain; each tap (`tools/brew-tap/<t>`, label `t`, `as_linuxbrew("tap <t>")`, shown
   `brew tap <t>`, after `last`, check `brew_tap_check(t)`); with two or more of Brewfile, manager and catalog
   formulae, `tools/brew-shared` (label `shared Homebrew dependencies`, cmd `brew_shared_deps(all)`, shown `brew install
   the dependencies <all joined ", "> share`, after `last`, check `brew_shared_check(all)`); each Brewfile formula
   (`tools/brew/<f>`, label `f`, viaBrew, `road_step_reads`, after `last`, pin `pin_read_of`).
8. What a row waits on (afterDep): `apt-index` adds `tools/apt-index` (`apt_index_step`, cmd `path_line` + `APT_UPDATE`)
   once, where it is first needed, and waits on it; `homebrew` waits on `last`; a catalog tool that is a manager's
   toolchain resolves that manager and waits on what brings it; anything else brings the row's own road's manager.
   Bringing a manager (once): push its own step, waiting on what its own road's module needs, then plan its catalog
   row if it is one; answer what it waits on (nothing for a manager with no entry).
9. Planning a row: `via_road(road, bin or package)`; a note skips the row with it. The step: id, the row's label,
   manager the road's name, the step's cmd and shown (a `below` replaces cmd with `path_line` + `below_line(...)` and
   adds `below: { node, asks }`), `road_reads(road, bin)`, `after`, `bin` when the plan named one, `note`,
   `asks_version(road)`, `pin: pin_read_of(road, bin)`.
10. Then per manager in order with rows, catalog rows not planned as a manager's toolchain, or custom rows: bring the
    manager, plan its own rows (recipe order) and its catalog rows. Then the Brewfile's release roads. Then the
    catalog rows on every other road (brew, apt, release, vendor, script) not planned as a manager's. Last, the custom
    rows (`custom_installs_for`), each after what its manager road waits on (afterRoad).
11. Answer `{ installs, skipped, base: brew.base, brewfile: brew.text }`.

Null and empty: no ticked tools rows is no step and an empty Brewfile; an agent or shell row adds nothing; an apt row
plans nothing (apt reads no row); a tap alone brings Homebrew and its toolchain.

## Rules against tests

Every recipe is compared step by step, every field, plus skips, base rows and the Brewfile
(`golden_import_plan_hard.rs`), beside the ported cases (`golden_import_plan.rs`).

| Rule | Tests |
|---|---|
| Homebrew first, taps, shared, formulae, managers after their step | `homebrew_first_then_taps_formulae_and_each_manager_after_its_own_install`, `tool_installs_for_homebrew_first_then_taps_formulae_then_each_manager_after_its_own_install` |
| the shared step's count rule | `one_formula_has_no_shared_step`, `two_formulae_get_one_shared_step_between_the_taps_and_the_first_formula`, `eight_formulae_share_one_step`, `tool_installs_for_one_formula_has_nothing_to_share_two_get_one_between_the_taps_and_the_first` |
| base rows, versions from the table | `rows_the_floor_covers_are_the_bases_with_the_version_read_from_the_brew_table`, `tool_installs_for_rows_the_base_floor_covers_install_nothing_and_are_listed_as_the_bases` |
| managers: a formula, rustup, a Mac formula, bun on npm, pipx | `a_manager_needed_only_for_its_rows_brings_homebrew_when_a_formula_is_its_road`, `cargos_rows_bring_rustup_and_wait_on_it`, `a_macs_rust_formula_is_the_catalogs_rust_row_and_cargo_rows_wait_on_it`, `a_macs_rustup_formula_is_the_catalogs_rust_row`, `buns_rows_wait_on_an_npm_install_of_bun` |
| the node step once, the recipe's node row, a Mac's node | `the_node_step_runs_once_ahead_of_every_row_on_node`, `the_recipes_own_node_row_is_the_step`, `a_macs_node_formula_is_the_step`, `the_node_step_runs_once_ahead_of_every_row_whose_road_runs_on_it_whichever_road_asked` |
| catalog rows on every road, an unknown id skipped | `catalog_rows_on_every_road_wait_on_what_their_road_needs_and_an_unknown_one_is_skipped`, `a_catalog_go_row_beside_go_rows_is_the_managers_step` |
| tap release roads, unknown and no-bottle skips | `tap_formulae_with_a_known_release_go_by_it_with_their_pin_and_the_rest_are_skipped` |
| custom rows last, after their manager; alone they bring it | `rows_outside_the_catalog_come_last_each_after_its_manager`, `rows_outside_the_catalog_alone_bring_their_managers` |
| empty, unticked, agent and apt rows | `an_empty_recipe_plans_nothing_and_writes_an_empty_brewfile`, `unticked_rows_plan_nothing`, `agent_and_apt_rows_plan_nothing_and_a_tap_alone_brings_homebrew` |
| prefix and probe PATH in every line | `ruff_and_tokei_under_a_prefix_name_the_prefix_homes`, `gh_and_turbo_on_the_probe_path` |
| the provision wrapper | `provisions_own_rows_plan_as_the_ts_with_the_id_as_the_label`, `the_wrapper_provision_calls_is_the_plan_of_provisions_own_rows` |
| the Homebrew bootstrap script and shim | `installs_the_shim_as_the_brew_on_path_roots_own_file_and_links_homebrews_brew`, `costs_a_line_and_not_the_step_when_the_upstream_branch_will_not_fetch_the_shim_still_lands`, `leaves_the_checkout_the_origin_remote_a_plain_clone_has_so_brew_update_finds_a_branch`, `never_pipes_a_download_into_a_shell` |
