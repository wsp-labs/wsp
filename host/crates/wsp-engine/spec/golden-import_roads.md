# import_roads (golden-import.ts:20-52, 641-1081)

What a recipe row installs by and what a step reads off its road. Pure: no machine, no clock, nothing kept. The
catalog's road modules (`road_module(road)`, wsp-catalog) do the road-specific work; this unit composes them.

## The row (:20-52, 758-799)

`RecipeEntry` reads a recipe file's row with serde (camelCase; absent fields absent). A row is ticked when `bring` is
`Some(true)`, nothing else. `package_of(id)` is the id's segments after the second (`tools/npm/@a/b` is `@a/b`).
`BrewTable` keeps the caller's order; `brew.get(name)` is by full name.

## Reads off a road (:689-724)

- `asks_version(road)`: `version_of(road)` where `fixes_version(road)`, else `None`.
- `road_reads(road, bin, prefix)`: `present` = the module's `present(road, bin)`; `bins` = `bins(road,
  install_homes(prefix))`, `None` when empty; `check` never. `road_step_reads` adds `check` = the module's `check(road,
  bin)`.
- `pin_read_of(road, bin, prefix)`: `read` = the module's `installed(road, bin, install_homes(prefix))`, `fixed` =
  `fixes_version(road)`, `words` = the module's words.

## Rows and their roads (:765-996)

- `base_row_for(e, brew)`: tools rows only; `base_entry_for(package_of(e))`, none is `None`; version is the row's,
  else the brew table's for that package; name the entry's.
- `is_tap`: id under `tools/brew-tap/`. `formula_of`: the id after `tools/brew/`.
- A formula is macOS-only when `MACOS_ONLY_FORMULAE` names it or its table entry says `macosOnly`.
- The release road of a row (releaseFor): only a formula with a `/` in it (a tap's), `linux == "unknown"`, a table
  entry with a `source`, not macOS-only: `{ id, name: entry.name, source, pin: row's pin }`. Its road is `{ road:
  release, repo, version: tag, pin, go: "github.com/<repo>@<tag>" }`, bin the entry's name.
- `brewfile_for(entries, brew)`: ticked tools rows in recipe order. A base row goes to `base`; a tap to `taps`; a
  formula that `catalog_tool_of` answers is left out of the Brewfile entirely; else `linux == "no"` is skipped `no
  Linux bottle`, unknown and macOS-only `macOS only`, a release road to `roads`, unknown `no Linux bottle known`, any
  other (no `linux` at all included) to `formulae`. `text`: `tap "<t>"` lines then `brew "<f>"` lines, joined and
  ending `\n`; empty when there are none. Unticked and non-tools rows add nothing.
- `catalog_tool_of(e)`: under `tools/catalog/`, the catalog entry by package when it is a tool; under `tools/brew/`,
  the manager tool the formula stands for (managerTool: `catalog_tool_for(formula)`, not on the brew road, and the
  tool of one of the managers npm, pnpm, bun, uv, pipx, cargo, go); else `None`.
- `manager_formula(m)`: the manager's catalog road's formula when that road is brew; `pipx` for pipx, which has no
  catalog row; else `None`.
- `row_road(e, brew)`:
  1. A catalog tool: `at = at_catalog_pin(<the road fixed at the row's version where the row has one and the module
     has `at`, else the catalog road>, row version)`. Notes, in order: `by an unmeasured road` when the entry's
     evidence road is unmeasured; then `at.note`, else (row version set, module without `at`, and the version is not
     the road's own) `pinned_note(version, road version)` or `<version> asked, installed <words> at its current
     version` where the road has none. The road is `at.road` with the row's pin over it; bin the entry's; `after` is
     `install_after(entry)`; `note` the notes joined `; `; `pinned` and `below` (its road also takes the row's pin)
     off `at`.
  2. Else a tap formula's release road, as above.
  3. Else an id under `tools/catalog/` is `None`.
  4. Else the first road in `ROADS` whose `tools/<road>/` the id starts with, read by that road's own module
     (`row_road(manager, { name: package, version, paths, label })`): `None` where it reads none (apt); then
     `at_catalog_pin(that road, row version)`; bin the module's `bin(road)`.

## Lines (:803-1071)

- `path_line(path, prefix)`: `export PATH=<path>` then ` NAME=value` for every entry of `install_env(install_homes(
  prefix))` in its order, all space-joined, nothing quoted (an empty path is `export PATH=`).
- `brew_housekeeping(path)`: `path_line(path, None)`, newline, `as_linuxbrew("autoremove")`; the same with
  `as_linuxbrew("cleanup -s --prune=all")`.
- `custom_prelude(path, prefix)`: `path_line`, `APT_ENV`, `export <BREW_ENV>`, newline-joined.
- `custom_installs_for(custom, after, path, prefix)`, in the given order: id `tools/custom/<id>`, label the name,
  manager `script`, cmd `custom_prelude` then each install line, newline-joined; the row's road where its `manager`
  is a road whose module reads rows (`row_road(manager, { name, paths: [], label: name })`) gives `present`, `bins` and
  `check` through `road_step_reads(road, name, prefix)`; `check` is that road check, else the row's own; `shown` is
  `shown_of(install)`; `after` is what `after(row)` answers. A manager that names no road reads like no manager.
- `shown_of(lines)`: joined `; `.
- `via_road(road, bin, path, prefix)`: the module's `install(road, bin, install_homes(prefix))`; a `RoadNote` is
  `ViaRoad::Note`; else `cmd` = `path_line(path, prefix)`, newline, the line; `shown` = the module's `shown(road, bin)`,
  else `shown_of(line.split("\n"))`.
- `apt_index_step(id, cmd)`: label `apt index`, manager `apt`, shown `apt-get update`.
- `brew_tap_check(tap)`: `<as_linuxbrew("tap")> 2>/dev/null | grep -qx <shell_quote(tap)>`.

## Rules against tests

| Rule | Tests (`golden_import_roads_hard.rs`, `golden_import_plan.rs`) |
|---|---|
| path line, prefix, empty path, quotes left alone; the other lines | `the_path_line_and_the_lines_beside_it_are_the_ts_byte_for_byte` |
| 13 roads and 11 catalog tools, with and without a prefix: script, shown, reads, pin read, asks | `every_road_reads_and_scripts_as_the_ts_with_and_without_a_prefix`, `what_a_step_shows_is_the_managers_own_line_the_brew_line_without_su_where_a_release_comes_from`, `the_version_a_step_asks_for_is_the_rows_own_where_its_road_installs_at_one`, `carries_its_roads_own_presence_read_and_the_directories_that_road_links_into` |
| 37 rows: base row, road, catalog tool (unticked, pinned, tap, macOS-only, unknown, catalog ids, agents) | `every_row_has_the_ts_base_row_road_and_catalog_tool`, `a_catalog_rows_version_is_the_installs_where_the_road_pins_one_else_the_note_says_what_it_installed`, `the_catalogs_pin_wins_over_an_older_version_on_any_road_a_row_names_the_tool_by`, `a_row_asking_a_newer_version_than_the_catalog_pins_keeps_its_own_the_pin_is_a_floor` |
| Brewfile: taps then formulae, skips, roads, base, empty | `the_brewfile_lists_taps_then_formulae_and_notes_what_does_not_go`, `brewfile_for_lists_ticked_taps_and_formulae_with_a_linux_bottle_unknown_ones_noted`, `brewfile_for_is_empty_when_nothing_homebrew_is_ticked` |
| manager formulae | `every_manager_comes_by_the_ts_formula_or_none` |
| rows outside the catalog: lines, manager reads, waits, unknown manager | `rows_outside_the_catalog_install_by_their_own_lines_with_their_manager_reads_and_wait_on_what_is_named` |
| tap release roads, their pin and go fallback | `a_tap_formula_on_the_road_names_its_binary_too`, `a_pin_for_the_same_tag_is_checked_one_for_another_tag_is_not`, `the_go_fallback_lands_under_the_rows_command_a_module_suffix_v_n_is_not_the_name` |
