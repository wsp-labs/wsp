# apt_tools
Source: `apt.ts` (94 lines) and `tools.ts` (263 lines) at a3b870a6e. Tests: `test/apt.test.ts` (7 cases),
`test/tools.test.ts` (22 cases, one `it.each` of 2 rows). Rust: `tests/detect_apt.rs`, `tests/detect_tools.rs`.

## What it does
Reads the tools rung: Homebrew's taps and formulae (ticked by the Linux bottle table), a Nix home-manager config, the
globals of npm, pnpm, bun, pipx, uv, cargo and apt, and Go binaries under `~/go/bin`. apt's reader keeps only the
packages a person chose: marked manual, not named in NOT_A_CHOICE, of priority `optional` or `extra`, shipping a file
directly under `/usr/bin` or `/bin`. Every command run below passes no timeout and no signal.

## Behaviour
1. `parse_dpkg_fields`: each `\n` line matching `^(\S+)\s+(\S+)\s*$` (so a trailing `\r` is fine, a third field drops
   the line) sets name to value; a repeat replaces the value in the first one's place (apt.ts:31-39).
2. `parse_apt_manual`: lines `js_trim`med, empties dropped, order and repeats kept (apt.ts:42-44).
3. `apt_packages` (apt.ts:70-81), in this order: run `apt-mark ["showmanual"]` (`None` reads as `""`); drop
   NOT_A_CHOICE; if nothing is left, return `[]` having run nothing else. Run `dpkg-query ["-Wf", "${Package} ${Priority}\n"]`
   (a real newline ends the second argument). List DPKG_INFO once: each name ending `.list`, less that suffix, cut at
   the first `:`, maps the package to `DPKG_INFO/<name>`; an empty package is dropped; a later file for the same package
   (later in `list`'s order, UTF-16 sorted, where `.` sorts before `:`, so `t:amd64.list` beats `t.list`) replaces
   the earlier one in place. Then for each manual name, in manual order, whose priority is in CHOSEN (a name
   dpkg-query did not list has priority `""` and is out), read its list file with `lines(file, None)` until a
   `js_trim`med line matches COMMAND_PATH; a name with no list file is out without a read. Each kept name is
   `Pkg { name, version: None, path: None }`; a repeated manual name is kept twice.
   3a. A list file's `lines` stream that yields `Err(e)` before a command line fails `apt_packages` at once with
   `HostError::new(e.to_string())`; no later list is read (the names are read one after another). A command line
   before the `Err` ends that file's read first, so nothing fails. `detect_tools` fails with the same error, and
   `collect` stops there: the rungs before it were reported through `on_rung`, the tools rung and later ones are not
   (coordinator's ruling on #2446; the TS iterator's throw rejects `Promise.all` and `collect`).
4. `apt_sizes(host, names)`: empty `names` returns an empty map and runs nothing. Otherwise one run of
   `dpkg-query ["-Wf", "${Package} ${Installed-Size}\n"]`; for each name in `names` order, the field read by
   JavaScript's `Number()` (absent is NaN); kept when finite and above 0, as `kib * 1024` (apt.ts:85-94).
5. `parse_brewfile`: each line `js_trim`med, matched by `^(tap|brew)\s+"([^"]+)"` (tools.ts:16-26).
6. `brewfile_here` runs `brew ["bundle","dump","--file=-","--formula","--taps","--no-restart"]` with env
   `{HOMEBREW_NO_AUTO_UPDATE: "1"}`; `None` parses as `""` (tools.ts:32-34).
7. `parse_npm_globals`: JSON whose `dependencies` is an object (not an array, not null); each entry but `npm` and
   `corepack`, in JavaScript key order, with `version` when it is a string (tools.ts:57-67). `parse_pipx_list` is the
   same over `venvs`, version at `metadata.main_package.package_version` (tools.ts:69-80). `parse_pnpm_globals` takes a
   JSON array, each element's `dependencies` object, `version` and `path` when strings; a non-object element is skipped
   (tools.ts:98-111).
8. `parse_uv_tool_list` and `parse_cargo_installs` are one parser: a line that is empty or starts with whitespace or
   `-` is skipped; else `^(\S+)\s+v(\S+?)(?::|\s|$)` gives name and version (`bat v0.24.0 (/src/bat):` is 0.24.0;
   tools.ts:83-95).
9. `parse_bun_globals`: lines matching `^[│├└─\s]+(@?[^@\s]+)@(\S+)$` (tools.ts:114-122).
10. `parse_go_version_m`: tab-split lines; column 1 `path` with a column 2 sets path, `mod` with a column 3 sets version,
    the last of each wins; `None` unless both are set (tools.ts:130-139).
11. `js_global_folders` runs `npm ["root","-g"]` and `pnpm NPM_LS` at once. Roots: `first_line` of npm's answer, then
    `/opt/homebrew/lib/node_modules`, `/usr/local/lib/node_modules`, then `expand(home, BUN_GLOBALS)`, `None` dropped,
    repeats dropped keeping the first. pnpm's listed packages with a `path` collect paths by name in order. The function
    gives `<root>/<name>` for each root, then that name's listed paths (tools.ts:181-187).
12. `package_commands` reads `<folder>/package.json` for every folder at once; for each in folder order: `bin` a string
    gives the name's last `/` segment, `bin` an object gives its keys in JavaScript order, anything else nothing;
    repeats dropped keeping the first (tools.ts:191-202).
13. `detect_tools` runs brew rows, nix rows, every GLOBALS manager and go rows at once and concatenates them in that
    order, GLOBALS in table order `npm, pnpm, bun, pipx, uv, cargo, apt`, whichever answers first (tools.ts:259-263).
14. Brew rows: `which("brew")` false gives `[]` and no dump. A tap is an item `tools/brew-tap/<name>`, group
    `Homebrew taps`, linux yes. A formula's linux is `yes` on a Linux host, else `linux_support(name, &LINUX_BOTTLES)`;
    id `tool_row_id("brew", name)`, group `Homebrew`; `no` is skip with reason `no Linux bottle`, `unknown` is skip
    with no reason, `yes` is bring (tools.ts:217-228).
15. Nix: when `~/.config/home-manager` or `~/.config/nixpkgs/home.nix` exists, one entry `tools/nix-home-manager`,
    label `Nix home-manager config`, linux yes, with `found` of both paths (tools.ts:244-247).
16. A manager whose `which(bin)` is false gives `[]` and runs nothing else. Otherwise its listing (or `npm`'s and apt's
    own reader); when `reads_row_road(id)` is false, a package `catalog_tool_for` knows is dropped. Each package is an
    item `tool_row_id(id, name)`, label `name<sep>version` (just the name with no version or no sep; apt has no sep),
    the manager's group, linux yes, `version` when the package has one (tools.ts:252-257).
17. npm (tools.ts:161-173): run `npm ["prefix","-g"]` first, then `npm NPM_LS`; then for `/opt/homebrew` and
    `/usr/local` in that order, skip the one equal to npm's own prefix (its `first_line`) or with no
    `<prefix>/lib/node_modules`, else run `npm NPM_LS + ["--prefix", prefix]`. A name seen before keeps the first one.
18. Go (tools.ts:230-242): `which("go")`; list `<home>/go/bin`; one `go ["version","-m","<home>/go/bin/<name>"]` per
    name in turn. No module: item `tools/go/<name>` label `<name> (no module info)`, skip, linux yes. Else an entry with
    `paths: ["<path>@<version>"]`, `bytes: 0`, linux yes, `version`.

## Edge cases
- `apt_sizes` field `"0"`, `"-5"`, `"abc"` or missing: left out. `"1e1"` is 10240, `"0x10"` is 16384 (JavaScript's
  `Number`), `"12.5"` is 12800.
- `parse_npm_globals("not json")`, `"null"`, `"[]"`, `{"dependencies": []}`: all `[]`.
- A global named `2` lists before `a` (JavaScript key order).
- `parse_go_version_m("garbage")` is `None`.

## Decisions
1. apt's `lines` stream yielding `Err` part way: ruled by the coordinator, rule 3a. Interface change: `apt_packages`
   and `detect_tools` return `Result<_, HostError>`. The live host's `lines` never yields `Err` (a read error ends
   its stream quietly, live_host rule 5), so only a fake host reaches this road.
2. `apt_sizes` returns `f64` bytes, since `Number()` can give a fraction.

## After the port
- A Linux laptop's formula is `yes` whatever the table says, so a formula with no Linux bottle is ticked there.

## Wave 2 rules

Each rule's cases are in `tests/fixtures/oracle/w2/apt_tools.json` (written by `oracle/w2-apt_tools.oracle.ts` from the
TS itself) under the key named, and replayed by `tests/detect_tools.rs` `w2_oracle::*`.

- W1. `parse_dpkg_fields` uses JavaScript's `\s`/`\S`: U+0085 is not a separator, U+00A0, U+FEFF and U+2028 are; a
  line with leading space, or a third field, is dropped; a repeated name keeps the last value (`parse_dpkg_fields`).
- W2. `parse_apt_manual` trims with `js_trim` (U+FEFF and U+00A0 go) and keeps repeats (`parse_apt_manual`).
- W3. COMMAND_PATH is tested on each `js_trim`med line: `/usr/bin/x` and `/bin/x` count (a space inside the name
  too), `/usr/bin/`, `/usr/bin/a/b`, `/usr/sbin/x`, `/usr/local/bin/x` do not; a last line with no LF counts. CHOSEN is
  case-sensitive (`Optional` is out) and a priority line with three fields is dropped, so its package is out
  (`apt_packages`: "command path table and the priority filter").
- W4. A list file name loses one `.list` (`x.list.list` is package `x.list`), is cut at the first `:` (`x:a:b.list`
  is `x`), `:amd64.list` gives an empty package and is dropped, `.md5sums` and `.LIST` are not lists; a folder named
  `w.list` is listed like a file (`apt_packages`: "package name from the list file name").
- W5. NOT_A_CHOICE is exact and case-sensitive, tested after the trim; survivors keep manual order.
- W6. dpkg-query answering `None` or `""`: no list is read; apt-mark answering `None`, `""` or blank lines: nothing
  else runs (calls exactly `["run apt-mark showmanual"]`). No list folder: the two runs and no reads. A repeated
  manual name with no command line is read once per entry.
- W7. The `lines` failure of rule 3a, three ways: before any match (error, later packages unread), after the match
  (never seen), in the second package's file (error after the first package was read).
- W8. `apt_sizes` reads with JavaScript's `Number()` (`+5`, `.5`, `5.`, `0b11`, `0B11`, `0o17`, `0XFF`, `00012`, `1e3`,
  `1e-1`, `0.0001` all parse; `1e400`, `1_0`, `-0`, `0x`, `1,5`, `NaN`, `-Infinity` are dropped); answers in `names`
  order, a missing name skipped, a repeated name once at its first place, a repeated dpkg line's last value; one run
  even for `[""]`, none for `[]` (`apt_sizes`).
- W9. `parse_brewfile`: `js_trim` per line (a leading U+FEFF goes), double quotes only, an empty name skipped, `brewx`,
  `Brew`, `cask`, comments and a name split over two lines skipped, repeats kept (`parse_brewfile`).
- W10. The JSON listings: an empty-string version is a version; a non-string version, a non-object value or `null`
  gives a name with no version; only `npm` and `corepack` are dropped; a duplicate JSON key keeps its first place and
  its last value; any top level of the wrong type is `[]` (`parse_npm_globals`, `parse_pnpm_globals`,
  `parse_pipx_list`). pnpm reads `dependencies` only, keeps an empty `path`, drops a non-string one, and never dedupes
  across global dirs.
- W11. uv and cargo share one line parser: the version stops at the first `:` or whitespace (JavaScript's, so `\r`
  and U+00A0 end it), `y v` has no version, a tab separates, an indented or `-` line is skipped, a last line with no LF
  counts (`parse_uv_tool_list`, `parse_cargo_installs`). bun's line must match whole: a `\r\n` line, a trailing space,
  `g@`, `@h`, `@@i@1` and a line with no tree prefix are dropped; `f@1@2` is `f` at `1@2` (`parse_bun_globals`).
- W12. `parse_go_version_m`: columns split on tab; the first column must be present (a line with no leading tab is
  not read); empty columns count as set; a `mod` line with three columns is not read; the last `path` and the last
  `mod` win in either order; `\r` is kept (`parse_go_version_m`).
- W13. `js_global_folders` runs `npm root -g` and `pnpm ls -g --depth=0 --json` whatever is on PATH, in that order;
  npm's root is the first line `js_trim`med (a blank first line is no root); the npm root comes first, then the three
  fixed roots, skipping a root equal to one already listed; pnpm's paths follow, never deduped; a pnpm `path` of `""`
  is kept, a non-string dropped (`js_global_folders`).
- W14. `package_commands`: `bin` a string (even `""`) gives the last `/` segment of the asked name (`""` for `@s/`);
  `bin` an object gives its keys in JavaScript order (integer-like first), `""` kept; anything else gives nothing; the
  package.json `name` is never read; repeats across folders keep their first place; no folders, no reads
  (`package_commands`).
- W15. `detect_tools` rows: repeated Brewfile lines are repeated rows in the Brewfile's order; an empty-string version
  gives the label `a@` (or `e ` for pipx) and `version: ""`; Go rows are in group `Go binaries`, an empty or failed
  `go version -m` gives `<name> (no module info)` skip, else the label is the name and `paths` is `["<path line>@<version>"]`
  (the `path` line, not the `mod` name); nix paths list only the paths found; apt rows have no version and their
  catalog filter drops what the catalog carries while npm's row of the same name stays; apt rows come on darwin too
  when apt-mark is on PATH; a failed apt read is the rung's error (`detect_tools`).
- W16. No run in this unit carries a timeout or a signal; brew's run alone carries env `{HOMEBREW_NO_AUTO_UPDATE: "1"}`.

## Tests
| Rule | Test |
|---|---|
| 1, 2 | `reads_a_package_and_its_field_from_dpkg_query_dropping_a_line_that_names_no_value`, `reads_the_manual_list_as_names_blank_lines_dropped`, `a_repeated_dpkg_line_replaces_the_value_in_place` |
| 3 | `keeps_the_packages_a_person_chose_and_drops_the_distros_own`, `reads_one_directory_and_two_dpkg_query_runs_never_a_dpkg_l_per_package`, `says_nothing_when_apt_mark_answers_nothing`, `apt_mark_naming_only_unchosen_packages_runs_no_dpkg_query`, `a_later_list_file_for_a_package_replaces_the_earlier` |
| 3a | `a_failing_list_read::*` (4) |
| 4 | `reads_each_packages_size_from_dpkgs_own_record_in_bytes`, `asks_dpkg_nothing_when_no_package_needs_a_size`, `a_size_is_read_as_javascripts_number_reads_it` |
| 5, 6 | `parses_a_brewfile_into_taps_and_formulae_casks_mas_apps_and_vscode_lines_have_no_linux_build`, `asks_homebrew_for_its_taps_and_formulae_alone_with_its_auto_update_off` |
| 7, 8, 9, 10 | `parses_npm_globals_dropping_npm_and_corepack`, `parses_pnpm_globals_one_project_object_per_global_dir`, `parses_bun_globals_a_header_line_then_a_tree_of_name_at_version`, `parses_pipx_uv_tool_and_cargo_listings`, `reads_module_path_and_version_out_of_go_version_m`, `json_listings_keep_javascripts_key_order`, `a_listing_that_is_not_the_right_json_shape_is_empty` |
| 11, 12 | `js_global_folders_lists_each_root_then_pnpms_own_paths`, `package_commands_reads_bin_off_every_folder_once_each` |
| 13 | `asks_every_manager_at_once_and_its_rows_keep_their_order_whichever_answers_first`, `global_installs_become_one_row_per_package_under_their_managers_group` |
| 14 | `homebrew_rows_taps_and_formulae_marked_by_the_linux_bottle_snapshot`, `default_ticks_no_linux_bottle_locks_the_row_off_with_its_reason`, `a_formula_already_installed_on_a_linux_laptop_runs_on_linux`, `no_brew_means_no_dump_is_attempted` |
| 15 | `nix_home_manager_config_is_brought_wholesale` |
| 16 | `what_apt_has_that_a_person_chose_is_one_row_per_package`, `files_no_apt_row_for_a_tool_the_catalog_carries`, `a_computer_with_no_apt_is_asked_nothing_about_it`, `bun_with_an_empty_global_dir_lists_nothing` |
| 17 | `npm_globals_under_a_node_prefix_npm_no_longer_uses_are_listed_too`, `the_prefix_npm_itself_uses_is_listed_once` |
| 18 | `a_go_binary_is_labelled_by_name_the_module_path_is_its_first_detail_line`, `a_go_binary_without_module_info_is_offered_unticked` |
