# manifest

Source: `packages/collect/src/manifest.ts` (115) and `collect.ts` (42) at `a3b870a6e`. Tests: `manifest.test.ts` (7), `collect.test.ts` (5). Rust: `src/manifest.rs`, `src/collect.rs`; tests `tests/manifest.rs` (7), `tests/manifest_zod.rs` (7), `tests/collect.rs` (5), `tests/collect_order.rs` (1).

## What it does

The row every detector returns and the recipe file saves, its zod schema as a hand-written check that raises zod's issues in zod's order, and `collect()`, which runs each rung's detector in ladder order and parses the result.

## Behaviour

1. Shapes (manifest.ts:9-69): `Rung` words `identity shell toolchains tools agents logins` in that order; `RowDefault` `bring skip`; `Linux` `yes no unknown`; `LOGIN_CHOICES` is wsp-wire's. `Rung::as_str` gives the word.
2. Field rules (zod): `id` at least one UTF-16 unit; `bytes` a non-negative integer; `version`, `login`, `font` at least one unit when present; every item of `sources` and `keys` at least one unit; `pin` is wsp-wire's `ToolPin` with its own check (`tag`, `sha256` at least one unit; key order `tag, sha256, arch, latest`); `choice` a `LoginChoice`; `signIn` a `ServerSignIn`. Unknown keys and `arch` are dropped (zod strips).
3. `parse_entry` issue order (oracle `manifest.json`): first the shape pass over keys in schema order (`rung id label paths excludes volatile bytes default reason group required bring choice linux version pin consent detail secrets login font sources signIn keys`), one issue per failing key, array items by index; an object that is not an object at all is one issue at path `[]` (`Expected object, received null|array|...`). Messages are zod 3.25's: `Required`; `Invalid literal value, expected true` (`pin.latest`); issues inside `pin` carry the paths `pin.tag`, `pin.sha256`, `pin.arch`, `pin.latest`; `Expected <type>, received <type>`; `Expected integer, received float`; `Number must be greater than or equal to 0`; `String must contain at least 1 character(s)`; `Invalid enum value. Expected 'a' | 'b', received 'x'` for a string outside an enum and `Expected 'a' | 'b', received number` for a non-string.
4. Aborting: a missing key, a wrong type (enum, literal and nested object included: `pin: {latest: false}`, `pin: {arch: "arm64"}`) stops the row there: the refine of rule 5 does not run. A non-integer or negative number, a too-short string (`id`, `version`, an item of `sources` or `keys`, `pin.tag`, `pin.sha256`) and an unknown key do not stop it (zod's dirty state): the refine runs and adds its issues after. So `bytes: 1.5` with a bad id gives `bytes: Expected integer, received float` then the id issue.
5. The refine (manifest.ts:71-96), in this order: id not starting `<rung>/`; `choice` on a row that is neither `logins` nor `consent: true`; `linux`, `version` on a non-tools row; `login`, `font`, `sources` on a non-shell row; `required: true` with default `skip`. Each is one issue at its key with the TS words.
6. `safe_parse_manifest` (:99-107): not an object is `[]: Expected object, ...`; `entries` missing is `entries: Required`, `null` is `entries: Expected array, received null`, an object `entries: Expected array, received object`; each row's issues prefixed with `entries, i`. The duplicate-id refine runs only when no row aborted (rule 4): it walks rows in order and raises `entries.i.id: duplicate id <id>` for every row whose id was seen before, after every row issue.
7. `parse_manifest` (:111-115): Ok, or `HostError::new("invalid manifest: " + issues_line)` where each issue is `<path joined by .>: <message>` and issues are joined by `; ` (a path of `[]` gives `: <message>`). The parsed rows have `arch` `None`.
8. `collect` (collect.ts:34-42): for each rung in ladder order, `run_detector(rung, host, rows so far)`, then `on_rung(rung, count)`, then append; finally `parse_manifest(&to_value(entries))`. A detector's `Err` (agents rule 3, apt_tools rule 3a) is returned from `collect` as is: no `on_rung` for that rung or any later one, no later detector runs.
8a. `ManifestEntry::new(rung, id, label, paths, bytes)` is a Rust helper with no TS counterpart: `default: bring` and every optional field `None`.
9. `run_detector` (collect.ts:15-26): identity `detect_identity`; shell `detect_shell`, then `shell_sources` over those rows, then `detect_terminal_font` rows after them; toolchains `detect_toolchains`; tools `detect_tools`; agents `detect_agents` then `detect_mcp`; logins `detect_logins`.
10. Key order (rule 4.26): a row serializes in rule 3's key order, which is what zod rebuilds; a detector's row that carries `arch` writes it between `version` and `pin`.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/manifest.json` (`oracle/w2-manifest.oracle.ts`), replayed by `tests/manifest_zod.rs`
`w2_oracle::*` and `tests/collect.rs` `w2_oracle::*`.

- W1. The abort and dirty matrix of rule 4, 111 rows with their issues in order (`parse_entry`).
- W2. Numbers from JSON text: `1.0`, `1e3`, `-0` and `0.0` are the integers they write (`parse_entry` cases with
  `args_json`). `bytes` past 2^53 is an accepted difference: the TS rounds it, `u64` keeps it, and past `u64::MAX` the
  Rust cannot hold it; no real size reaches either.
- W3. `pin` key order and stripping, `pin` issues at `pin.*`, `Invalid literal value, expected true` (`parse_entry`).
- W4. The envelope: not an object, `entries` missing, `null` or an object, and the duplicate-id pass after every row
  issue (`safe_parse_manifest`, `parse_manifest`).
- W5. `null` on an optional key is a type error, never an absent key (`parse_entry`).
- W6. `collect` writes the TS manifest text, tells each rung's count in ladder order, and makes the same calls; a
  duplicate Brewfile line fails the parse after every rung was told (`collect`).

## Edge cases

- `{entries: []}` is a manifest and writes `{"entries":[]}` (`an_empty_list_of_entries_is_a_manifest_and_writes_back_as_one`).
- An empty laptop: every rung reports 0 and the manifest is `{entries: []}` (`an_empty_laptop_*`).
- Three rows with one id raise two duplicate issues, at 1 and 2 (oracle).

## Behaviour nobody wrote down

- Rule 8.3: "at least 1 character" counts UTF-16 units (`wsp_words::js::utf16::len`); only emptiness matters here.

## Decisions

1. wsp-wire's `check::parse` answers zod's first issue only; `parse_manifest` must give every issue (the recipe file's refusal names all of them), so this unit keeps its own walk over the `serde_json::Value`.

## After the port

- The duplicate check is skipped when any row aborted, so a recipe with a typo and a duplicate shows only the typo. Kept.

## Rules pinned

| Rule | Test |
|---|---|
| 1 | `lists_the_six_rungs_in_ladder_order_*`, `a_tools_row_may_say_whether_it_runs_on_linux` |
| 2, 3, 5 | `rejects_each_row_that_breaks_a_rule`, `every_row_refusal_is_zods_issues_in_zods_order_*`, `a_choice_sits_on_a_logins_row_*` |
| 4 | `a_type_error_in_a_row_stops_its_rung_checks_but_a_short_string_does_not` |
| 6, 7 | `parses_the_envelope_and_reports_the_failing_row`, `rejects_two_rows_with_the_same_id`, `every_envelope_refusal_*`, `the_duplicate_check_runs_only_when_no_row_was_refused_by_type` |
| 8, 9 | `composes_every_rung_in_ladder_order_*`, `reports_each_rung_s_row_count_*`, `matches_the_fixture_laptop_snapshot` |
| 10 | `a_parsed_row_writes_its_keys_in_the_schemas_order_*`, `a_detectors_row_keeps_arch_until_it_is_parsed`, `the_fixture_laptops_manifest_writes_exactly_the_json_the_ts_wrote` |
