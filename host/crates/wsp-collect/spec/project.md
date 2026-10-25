# project

Source: `packages/collect/src/project/index.ts` (87), `reader.ts` (42), `readers/marker.ts` (38), `package-json.ts` (52), `toolchain-pins.ts` (52), `workflows.ts` (64) at `a3b870a6e`. Tests: `project.test.ts` (13 cases). Rust: `src/project/`; tests `tests/project.rs` (13), `tests/oracle_cases.rs` `project::*` (1: 7 scans against the TS).

## What it does

What one project folder's own files say it needs: four readers turn manifests into words, and `scan_project` names each word against the catalog once, as a row the catalog carries or a candidate for a custom row.

## Behaviour

1. `PROJECT_READERS` order: package-json, toolchain-pins, marker, workflows (index.ts:16). The first finding to land on a row writes its `why`; later ones for the same id are dropped. Candidates likewise by word.
2. `paths_of` (:39-45): `name/` is `list(<dir>/name)` mapped to `name/<child>`; an entry with `/*/` lists the folder before it and puts each child in the star's place; anything else is itself.
3. `files_for` (:49-63): per entry, per path, in order: a presence reader takes a path whose `stat` is a file, with empty text, and never reads it; a text reader takes a path `read_text` answers.
4. `scan_project` (:69-87): `dir` is the folder without trailing slashes. Each finding: with `installs`, `catalog_tool_for_dependency(road, name)`; else `catalog_tool_for(name)`. A hit records `{id, name: catalog name, why}` under the tool id. A miss with `catalog_only` is dropped; else a candidate `{id: word, name: label, why}`.
5. Marker (marker.ts:7-38): files in table order; a path in the table gives `{name, label, why: "<path> needs <label>"}`.
6. package.json (package-json.ts:30-52), files `package.json`, `apps/*/package.json`, `packages/*/package.json`: JSON that is not an object or array gives nothing. A non-empty string `packageManager`: cut at the first `+`, name before the first `@`; an empty name gives nothing; why `packageManager <spec>`. Each string `engines` entry: catalog-only, why `engines.<key> <range>`. Each string script: every `command_names` name, catalog-only, why `<path> scripts run <name>`. Each string dependency then devDependency: catalog-only, `installs: npm`, why `<path> depends on <name> <range>`. Non-string values are skipped.
7. Toolchain pins (toolchain-pins.ts:14-52), files `.tool-versions`, `mise.toml`, `.mise.toml`: `.tool-versions` lines with `#...` cut, trimmed, split on whitespace: name and optional first version. mise: lines with `#...` cut and trimmed; a line starting `[` sets `in_tools` to `line == "[tools]"`; in tools, `key = value` with the key's surrounding quotes removed and the first quoted string after `=` as version. Why `<path> names <name>[ <version>]`.
8. Workflows (workflows.ts:9-64), files `.github/workflows/`: each line matching the setup regex (:14) gives the setup word, or, for a word ending `-action`, the owner (or `<x>/action-setup`'s `x`); why `<path> sets up <name>`. Then every run block's `command_names`, catalog-only, why `<path> runs <name>`. A run block is the inline text after `run:`, or for an empty, `|`, `>`, `|-` or `>-` rest the following lines (each JS-trimmed) up to the first non-blank line indented no deeper than the `run:` line's leading spaces, joined with `\n`.

## Edge cases

- A folder that is not there, or holds nothing a reader asks for: `{dir, rows: [], candidates: []}`; a trailing slash is cut from `dir`.
- `package.json` of `[1,2]` gives nothing; `packageManager: "@yarnpkg/cli@4"` gives nothing (empty name) (oracle).
- A `[tools.extra]` table is not `[tools]` (oracle).

## Behaviour nobody wrote down

- Rule 3.11 / 4.26: rows and candidates keep first-found order; the oracle test compares them as arrays.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/project.json` (`oracle/w2-project.oracle.ts`, every value printed by the TS),
replayed by `tests/project.rs` `w2_oracle::*`.

Corrections to the rules above, found against the TS (each overrides the rule it names):

- C1. Rule 6: any JSON object or array parses (`typeof value === "object" && value !== null`); `engines`, `scripts`, `dependencies` and `devDependencies` are read with `Object.entries` only when they are objects themselves; a root array has none of the keys.
- C2. Rule 7: the comment cut is `replace(/#.*$/, "")` with JavaScript's `.`: on a CRLF line the `.*` stops before `\r` and `$` cannot match there, so the comment is not cut; the split is on JavaScript's `\s`.
- C3. Rule 7 (mise): the version is the first match of `/["']([^"']+)["']/` (the two quotes need not match); the key loses one leading and one trailing quote independently (`/^["']|["']$/g`).
- C4. Rule 8: the indent compared is the whitespace before the optional `- ` of `- run:` (regex group 1), so for `  - run: |` the limit is 2; the `RUN` regex ends in `(.*)$`, so on a CRLF file `run: cmd` and `run: |` never match (only `run:` followed by `\r` alone does).
- C5. Rule 3: an empty file answers `""` and is handed to its text reader.

- W1. `scan_project` over 44 folders: rows and candidates in first-found order, every read in order (`scan_project`, with calls).
- W2. Each reader over its edge files: package.json shapes, `.tool-versions` and `mise.toml` pins with comments, CRLF and quotes, markers, workflow `uses:` and `run:` blocks (`package_json_read`, `toolchain_pins_read`, `marker_read`, `workflows_read`).

## Rules pinned

| Rule | Test |
|---|---|
| 1, 4 | `a_gemfile_and_a_composer_json_tick_ruby_and_php_and_the_first_file_*`, `an_engines_key_the_catalog_has_no_row_for_*`, oracle |
| 2, 3 | `a_workspace_package_s_own_package_json_counts_*`, `a_marker_is_read_for_its_presence_alone_*` |
| 5 | `a_repo_with_a_compose_file_*` |
| 6 | `package_json_the_manager_it_pins_*`, `package_json_dependencies_*`, `corepack_s_integrity_suffix_*`, oracle scans 1 to 4 |
| 7 | `the_toolchain_pins_name_a_tool_and_its_version_*`, oracle scan 5 |
| 8 | `a_workflow_s_setup_actions_*`, `an_action_named_after_its_owner_*`, oracle scan 6 |
| empty | `a_folder_with_nothing_to_read_*`, `every_reader_is_registered_once_*` (passes on the constant table) |
