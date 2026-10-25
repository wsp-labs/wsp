# ghostty_config

Source: `packages/collect/src/ghostty-config.ts` (243 lines) at `a3b870a6e`. Tests: `ghostty-config.test.ts` (8 cases). Rust: `src/ghostty_config.rs`; tests `tests/ghostty_config.rs` (8), `tests/oracle_cases.rs` `ghostty::*` (3: 11 texts, 22 theme values, 9 whole configs against the TS).

## What it does

Reads the person's Ghostty config the way Ghostty does and answers the keys the app's terminal pane honours, as wsp-wire's `TerminalConfig`. The host serves it to the pane (`packages/host/src/server.ts:655`) and to `wsp` (`verbs/threads.ts:658`).

## Behaviour

1. `parse_ghostty_directives` (:30-42): split on `\n`; drop one leading U+FEFF; JS-trim; skip empty and `#`-led lines and lines without `=`; key is the JS-trimmed text before the first `=`, skipped when empty; value is the JS-trimmed rest with one enclosing pair of `"` removed (`/^"(.*)"$/`: the line must start and end with `"`). Order and repeats kept.
2. `ghostty_config_paths` (:50-55): `<xdg>/ghostty/config.ghostty`, `<xdg>/ghostty/config`, and on darwin the same two names under `~/Library/Application Support/com.mitchellh.ghostty`, expanded; `<xdg>` is `xdg_config_home` or `<home>/.config`.
3. Loading (:89-108, 228): each path in order, then its includes depth first. A path is normalised lexically (`.` and empty parts dropped, `..` pops) before the seen check; a seen path or one `read_text` cannot read is skipped (an unreadable file is not marked seen). Read files are appended to `files`. A file's includes are its `config-file` values in order; an empty value clears the ones collected so far in that file. A value starting `?` is optional (the rest, unquoted again); an empty path is ignored; `~/` expands; a relative path joins the including file's folder (`/` for a file at the root).
4. Theme (:229-240): the value of the last `theme` directive across all loaded files, through `theme_for`. A non-empty theme is looked up at `<xdg>/ghostty/themes/<name>`, then `/Applications/Ghostty.app/Contents/Resources/ghostty/themes/<name>` on darwin or `/usr/local/share/ghostty/themes/<name>`, `/usr/share/ghostty/themes/<name>` on linux; an absolute name is looked up as is only. The first readable file is appended to `files` and its directives other than `theme` and `config-file` are applied first; then every config directive is applied in load order.
5. `theme_for` (:214-221): split on `,`; each part with `:` past index 0 maps its JS-trimmed left to its JS-trimmed right (a later side wins); the scheme's side, else the whole JS-trimmed value when no part had a side, else `""`.
6. `apply` (:151-211), a value that does not parse leaving the key as it was: `font-family` empty clears the list, else appends; `font-size` a finite number over 0; `background`, `foreground`, `selection-background`, `cursor-color` a hex colour; `palette` `N=COLOR` with N an integer 0 to 15 written as JavaScript's `Number` reads it (decimal, `0x`, `0o`, `0b`, surrounding space) and a hex colour; `cursor-style` one of `block bar underline block_hollow`; `cursor-style-blink` `true`, `false`, or empty which unsets it; `window-padding-x`/`-y` one or two non-negative finite numbers split on `,` (one sets both; three parts, an empty part or a negative is refused); `background-opacity` any finite number, clamped to 0 to 1; `background-blur` and `background-blur-radius` `false` is 0, `true` or `macos-glass-*` is 20, else a non-negative integer.
7. `finite` (:117-120): JavaScript's `Number(value.trim())` when the trimmed value is not empty and the result is finite: `1e1` is 10, ` 12 ` is 12, `0x10` is 16, `Infinity` and `abc` are refused. `hex_color` (:110-115): `#?` and exactly six hex digits after a JS trim.
8. Answer (:242): `files`, `font_family`, `palette` (16 slots, `None` unset), the applied keys, and `theme` when non-empty; with no file, `{files: [], fontFamily: [], palette: 16 × null}`.

## Edge cases

- A light-only theme read in the dark is no theme: no `theme` key, no theme file read (oracle).
- A theme that no folder holds keeps its name and reads no file (`resolves_a_theme_against_*`).
- `palette = 16=#...` and `palette = 3=#zzzzzz` change nothing (oracle).

## Decisions

1. Key order: the TS answer's keys run `files, fontFamily, palette`, then each key in the order it was first set, `theme` last; wsp-wire's `TerminalConfig` fixes the schema's order. The hard tests compare as a map; a tape comparing this reply byte for byte would differ. Flagged for the coordinator.
2. The theme folder is the XDG one only; Application Support's themes folder is not searched, as the TS.

## Rules pinned

| Rule | Test |
|---|---|
| 1 | `reads_key_value_lines_in_order_*`, `every_text_of_the_oracle_gives_the_directives_the_ts_gave` |
| 2 | `loads_config_ghostty_then_config_under_xdg_*` |
| 3 | `follows_config_file_includes_*`, oracle configs 3 and 4 |
| 4, 5 | `resolves_a_theme_against_the_config_s_themes_folder_*`, `every_theme_value_of_the_oracle_*`, oracle configs 5 to 8 |
| 6, 7 | `takes_every_key_the_pane_honours_*`, `an_empty_font_family_resets_the_list_*`, oracle configs 1, 2, 9 |
| 8 | `with_no_config_file_answers_the_empty_object_*` |
