# tool_sizes: what a recipe costs on the builder's disk, and the shared brew step

Source at `a4033dc34`: `packages/engine/src/tool-sizes.ts` (180), `brew-shared.ts` (85), and the golden-import items
they read (`isTap`, `formulaOf`, `catalogToolOf`, `CATALOG_PREFIX`, `MACOS_ONLY_FORMULAE`, the brew table types) and
golden-tools' `TOOLS_DISK_FLOOR`, carried in `src/golden_seam.rs` until #2441 rules. Rust: `src/tool_sizes.rs`,
`src/brew_shared.rs`. Tests: `tests/tool_sizes.rs`, hard tests in `tests/tool_sizes_hard.rs`. Oracle:
`tool-sizes.json`, `brew-shared.json`.

## Constants (tool-sizes.ts:17-32)

- Z1. `BUILDER_DISK_GB` 20; `BUILDER_FREE_BYTES` 17570 MiB; `DISK_ROOM_BYTES` = that less `TOOLS_DISK_FLOOR` 2 GiB;
  `BREW_TOOLCHAIN_BYTES` 1024 MiB. Assumed sizes: go 500, uv 100, npm 50, agent 350, other 100 MiB.

## assumed_size (tool-sizes.ts:43)

- Z2. An `agents` rung row is 350 MiB "an agent" whatever its id; else by id prefix `tool_row_prefix(m)`: go "a go
  install", uv "a uv tool", npm, pnpm or bun "an npm global"; anything else "an install". `tools/gox` is an install
  (the prefix is `tools/go/`). Oracle `assumed`.

## source_of (tool-sizes.ts:62)

- Z3. URL matched by `^https://github\.com/([^/]+)/([^/]+?)(?:\.git)?(?:/(.*))?$` (lazy repo, so `.git` is cut); repo
  `owner/name`; rest `releases/download/<tag>/...` gives the tag, `archive/[refs/tags/]<tag>.tar.gz|.zip` gives it,
  an empty rest takes the tag beside it (an empty string tag is a tag; null or absent is none); anything else none.
  Oracle `sourceOf`: 12 cases (`/tree/main` none, `r.git/` with tag `t` is `o/r`).

## parse_brew_info (tool-sizes.ts:74)

- Z4. Not an object with a `formulae` array: no rows (null, an array, `formulae` an object). Each formula that is an
  object: `name` must be a string (else skipped); `fullName` the `full_name` string else name. `installed[0]` where it
  is an object; runtime deps the `full_name` strings of its `runtime_dependencies` objects (entries without one or not
  objects skipped). deps = the `dependencies` strings then the runtime ones, each once, first-seen order. macosOnly: in
  `MACOS_ONLY_FORMULAE` or a requirement object named `macos`. Source from `urls.stable.url` and `urls.stable.tag`
  (absent tag reads null). Row keys in order name, fullName, deps, macosOnly, version (where installed[0] has a string
  one), source (where found); `bytes` is never set here. Oracle `parseBrewInfo`.

## parse_du and brew_table (tool-sizes.ts:99-117)

- Z5. Each line trimmed and matched `^(\d+)\s+(.+)$`; the path's trailing slashes cut; key the last segment (or
  `key_of(path)`); value kB * 1024. A later line of the same key overwrites the value and keeps the first one's place.
  Oracle `parseDu`, `parseDuKeyed` (a 102400 then a 7 for ffmpeg reads 7168).
- Z6. `brew_table(json, du)`: each parsed formula by full name, with `bytes` = du by its short `name` where present.

## tool_size and agent_size (tool-sizes.ts:129-180)

- Z7. A tap row (`tools/brew-tap/`) is `None`. The catalog's size first: a `tools/catalog/<pkg>` row is
  `catalog_entry(pkg)`; else `catalog_tool_of(row)`; else, where the id's second segment is a road, the catalog tool of
  that road for the package; its `size_bytes` gives `{bytes, Measured, deps 0}`. Else the row's formula (`tools/brew/`)
  where its bytes are known (catalog brew tool's size, else the Mac table's bytes): the closure walks deps breadth
  first from the formula, skipping glibc and gcc and any formula already seen, and not descending into a formula the
  catalog carries for brew; bytes = sum over every member (unknown ones 0); `{bytes, Mac, deps: members - 1}`.
- Z8. `agent_size`: `catalog_entry(id after its first /)`'s size, `None` where it has none.

## brew-shared.ts

- Z9. `brew_shared_deps(formulae)` and `brew_shared_check(formulae)`: byte for byte the oracle's, four lists (two
  formulae, one, none, quoted names). Each formula is `shell_quote`d in the `deps --for-each` line; the toolchain grep
  excludes `-e glibc -e gcc`; the whole is passed through `as_linuxbrew_script`. `UNLISTED_AWK` is the TS
  `String.raw` text byte for byte.

## Null, empty, restart

An empty formula list still writes the step (oracle). An empty table sizes nothing by the Mac road. Nothing is kept.

## Rules and their tests

Files: `tool_sizes.rs` (ported), `tool_sizes_hard.rs` (hard).

| Rule | Tests |
|---|---|
| Z1 | ported `a_formulas_number_is_its_dependency_closure_read_from_this_macs_homebrew` (reads the constants beside the closure) |
| Z2 | ported `a_row_nothing_measured_counts_at_the_default_for_its_kind_and_says_which_kind`; hard `assumed_sizes_match_the_ts_oracle` |
| Z3 | ported `names_the_github_repository_and_tag_behind_a_formulas_url`; hard `source_of_matches_the_ts_oracle` |
| Z4, Z6 | ported `reads_name_dependencies_size_and_source_per_formula_a_du_line_without_a_formula_is_dropped`; hard `parse_brew_info_matches_the_ts_oracle` |
| Z5 | hard `parse_du_matches_the_ts_oracle` |
| Z7 | ported `a_formulas_number_is_its_dependency_closure_read_from_this_macs_homebrew`, `a_formula_the_catalog_carries_takes_the_catalogs_measured_closure_once_whatever_this_macs_table_says`, `a_formula_nothing_measured_or_read_has_no_size_taps_are_nothing_a_managers_global_takes_the_catalogs_number`, `homebrews_own_toolchain_is_left_out_of_a_closure_it_is_the_groups_own_line` |
| Z8 | ported `measured_install_sizes_by_agent_an_agent_nobody_measured_has_none` |
| Z9 | ported `a_failed_shared_check_names_the_formula_that_is_missing_and_the_recipes_tools_that_need_it_never_its_shell`, `a_failed_shared_install_names_the_formula_it_left_behind_and_the_last_line_brew_printed_for_it`; hard `brew_shared_scripts_match_the_ts_byte_for_byte` |
