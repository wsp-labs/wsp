# common
Source: `packages/collect/src/detect/common.ts` (95 lines) at a3b870a6e. Tests: none of its own in TS; every detector
test reaches it. Rust: `tests/detect_common.rs` (written for this port).

## What it does
The row builders every detector uses: which candidate paths exist and their total size, a row from a spec, a list item
with no path, dropping absent rows, and the first line of a command's output.

## Behaviour
1. `found(host, paths)` stats each path in input order through `expand(home, p)` (common.ts:35-44); a path whose stat is
   `None` is skipped; `paths` keeps the input's spelling (`~/x`, not the expanded form); `bytes` is the sum of each
   stat's `bytes` (a dir's is its whole tree, host.ts:8). Duplicates in the input are stat'ed and counted twice.
2. `exists` is `stat(expand(home, path)).is_some()` (common.ts:46-48).
3. `row(host, spec)` is `None` when no candidate exists and `always` is false; otherwise `entry` of the spec with the
   found paths and bytes (common.ts:51-55). With `always` and nothing found the row has `paths: []`, `bytes: 0`.
4. `entry(spec)` (common.ts:63-81): `default` is the spec's or `bring`; `group`, `reason`, `required`, `linux`,
   `version`, `arch`, `detail` are set exactly when the spec sets them (`required: Some(false)` stays `Some(false)`);
   `volatile` is the spec's volatile list filtered to entries that are in `paths`, in the volatile list's order, and
   absent when that leaves none (common.ts:64,78). Every other optional field of the row is absent.
5. `item` is `entry` with `paths: []` and `bytes: 0`, so it never carries `volatile` (common.ts:84-86).
6. `present` drops the `None`s and keeps order (common.ts:88-90).
7. `first_line(out)` (common.ts:92-95): `None` for `None`; else the text before the first `\n`, `js_trim`med; `None`
   when that is empty. Only the first line counts: `"\nv1"` is `None`, not `"v1"`. `\r` is trimmed (`"v1\r\n"` is `v1`).

## Edge cases
- `found(host, &[])` is `Found { paths: [], bytes: 0 }` and stats nothing.
- `entry` with `volatile: Some(vec![])` or with no volatile path among `paths` leaves `volatile` absent.
- `first_line(Some(""))`, `Some("   ")` and `Some("\u{feff}")` are `None` (js_trim strips U+FEFF).

## Decisions
1. TS `RowSpec`/`EntrySpec` spread their optional fields at the top level; Rust groups them in `RowOptions`. Same fields.
2. `Detector` is a plain fn pointer returning a boxed local future, since a detector holds `&Host` across awaits.

## After the port
- `entry` writes `arch`, which the zod schema strips on parse (manifest.ts has no `arch`): a detector's `arch` never
  reaches the recipe.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/common.json` (`oracle/w2-common.oracle.ts`, every value printed by the TS),
replayed by `tests/detect_common.rs` `w2_oracle::*`.

Corrections to the rules above, found against the TS (each overrides the rule it names):

- C1. Rules 1 and 2 lean on `expand` (small rule 2): `~/` expands to `<home>/`, which a stat does not find as the home folder; `~user/x` is not expanded.

- W1. `found`, `exists`, `row` with every call, `entry`, `item`, `present`, `first_line` (each key of the fixture).

## Tests
| Rule | Test |
|---|---|
| 1 | `found_keeps_input_order_and_spelling_and_sums_bytes`, `found_of_nothing_stats_nothing` |
| 2 | `exists_expands_a_tilde_path` |
| 3 | `row_is_none_when_nothing_exists_unless_always` |
| 4 | `entry_defaults_to_bring_and_leaves_unset_fields_absent`, `entry_keeps_only_volatile_paths_that_are_on_the_row` |
| 5 | `item_has_no_path_no_bytes_and_no_volatile` |
| 6 | `present_drops_absent_rows_in_order` |
| 7 | `first_line_reads_only_the_first_line_trimmed_as_javascript_trims` |
