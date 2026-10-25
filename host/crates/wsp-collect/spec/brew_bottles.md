# brew_bottles

Source: `packages/collect/src/brew-bottles.ts` (73 lines) and the generated `src/data/linux-bottles.ts` (8,598 lines) at `a3b870a6e`. Tests: `brew-bottles.test.ts` (5 cases). Rust: `src/brew_bottles.rs`, `src/data/linux_bottles.rs`, `data/linux-bottles.json`, `data/linux_bottles.py`; tests `tests/brew_bottles.rs` (5 ported, 3 on the embedded table).

## What it does

Which Homebrew formulae have a Linux bottle, from a dated snapshot shipped with the crate, so the tools rung can grey out Mac-only formulae with no network call.

## Behaviour

1. `classify_formulae` (:26-40): not an array gives two empty lists; each item that is an object with a string `name` goes to `linux` when `bottle.stable.files` (each step an object) has a key `all`, `x86_64_linux` or `arm64_linux`, else to `no_linux`; other items are skipped. Both lists sorted by UTF-16 units.
2. `linux_support` (:64-73): `yes` for a name in `linux`, `no` for one in `no_linux`, else `unknown`. Exact, case-sensitive match.
3. `render_snapshot`: `serde_json::to_string_pretty(snapshot)` plus `\n`; keys `fetched, source, linux, noLinux`; empty lists render `[]`.
4. `LINUX_BOTTLES`: `LINUX_BOTTLES_JSON` parsed once, on first use.
5. The data file is not hand-written: `python3 host/crates/wsp-collect/data/linux_bottles.py` reads the TS table and writes `data/linux-bottles.json` in rule 3's form. A refresh from Homebrew goes through the TS script (`pnpm --filter @wsp/collect run bottles`) and then this one until the TS is gone.
6. Pin: the file's sha256 is `ae9c156d...41e721`, and `render_snapshot(&LINUX_BOTTLES)` gives the file back byte for byte.
7. Agreement: the embedded lists equal the TS table's, name for name in order: 8,428 with a Linux bottle, 158 without, fetched 2026-09-02.

## Edge cases

- `classify_formulae("nope")` and `[{nope:1}, null, 4]` give empty lists.
- `oven-sh/bun/bun` (a tap) is unknown.

## Decisions

1. The TS renders a TS source file; the crate's generated file is JSON, so `render_snapshot` renders that. A TS-side refresh and the Python script stay the only writers.
2. The TS caches a `Set` per snapshot object (`WeakMap`); the Rust may search the lists. Only the time differs.

## Rules pinned

| Rule | Test |
|---|---|
| 1 | `splits_formulae_by_whether_any_bottle_runs_on_linux_sorted_by_name`, `ignores_anything_that_is_not_a_formula_list` |
| 2 | `a_name_maps_to_its_linux_support`, `the_checked_in_snapshot_is_dated_disjoint_*` |
| 3 | `renders_a_snapshot_as_*`, `an_empty_snapshot_renders_its_lists_as_empty_arrays` |
| 4, 6 | `rendering_the_embedded_snapshot_gives_back_the_embedded_file_byte_for_byte` |
| 7 | `the_embedded_table_holds_every_name_of_the_ts_table_in_its_order` |
