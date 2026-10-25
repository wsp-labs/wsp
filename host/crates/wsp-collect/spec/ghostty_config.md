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
   8a. The answer is a `serde_json::Map` (preserve_order) in the TS object's key order, per the coordinator's ruling on #2446: `files`, `fontFamily`, `palette`, then each applied key in the order it was first set (the theme file's directives apply first, so its keys lead), `theme` last. `cursor-style-blink =` deletes its key (`shift_remove`, as JS `delete`), so a later `cursor-style-blink = false` puts it at the end. Keys are camelCase (`fontSize`, `selectionBackground`, `cursorColor`, `cursorStyle`, `cursorStyleBlink`, `windowPaddingX` as `{left, right}`, `windowPaddingY` as `{top, bottom}`, `backgroundOpacity`, `backgroundBlur`), so `serde_json::from_value::<TerminalConfig>` reads it.
   8b. Numbers are written as JavaScript writes them: an integral value is a JSON integer (`12`, not `12.0`), a fraction keeps its digits (`0.5`); colours are `{r, g, b}` integers.

## Edge cases

- A light-only theme read in the dark is no theme: no `theme` key, no theme file read (oracle).
- A theme that no folder holds keeps its name and reads no file (`resolves_a_theme_against_*`).
- `palette = 16=#...` and `palette = 3=#zzzzzz` change nothing (oracle).

## Decisions

1. Key order: ruled by the coordinator on #2446. The answer keeps the TS key order as an ordered map (rule 8a), not wsp-wire's `TerminalConfig` struct, whose field order is fixed. Interface change: `read_ghostty_config` returns `serde_json::Map<String, Value>`.
2. The theme folder is the XDG one only; Application Support's themes folder is not searched, as the TS.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/ghostty_config.json` (`oracle/w2-ghostty_config.oracle.ts`, every value printed by the TS),
replayed by `tests/ghostty_config.rs` `w2_oracle::*` (the config compared as JSON text).

Corrections to the rules above, found against the TS (each overrides the rule it names):

- C1. Rule 3: the include path is unquoted again in both cases (`unquote(optional ? value.slice(1) : value)`), so `config-file = ""b""` loads `b`.
- C2. Rule 6 (palette): N is read by `Number()`: an empty N is 0 (`palette = =#112233` sets slot 0); `1.0`, `1e1`, `+3`, `08`, `-0` are integers; U+00A0 around N is trimmed.
- C3. Rule 6 (blur): the word is lower-cased first (`TRUE`, `MACOS-GLASS-CLEAR` work), and the integer test is on the `Number()` value (`0x10`, `1e1`, `20.0` accepted).
- C4. Rule 7: JavaScript `Number()` takes a sign only on decimal text (`-0x10` and `+0x10` are refused), accepts `.5`, `5.`, `012`, `+12`, `1E1`, `0X1F`, `0B11`, `0O7`, and refuses `1_000`, `12px`, `1 2`, `1,5` and non-ASCII digits.
- C5. Rule 8b: number text is `JSON.stringify`'s: at or above 1e21 or below 1e-6 an exponent with a sign (`1e+21`, `1e-7`), a value above 2^53 rounded first, `-0` written `0`. A blur past `i64` is still a number.
- C6. Rule 1: it is the trimmed value after `=` that must start and end with `"`, and the regex's `.` stops at `\r`, U+2028 and U+2029, so a value holding one is not unquoted.

- W1. Directives, themes and config paths (`parse_ghostty_directives`, `theme_for`, `ghostty_config_paths`).
- W2. 219 whole configs, the answer as JSON text with its key order and number text, and every read in order: the tables for font size, padding, opacity, blur, palette, cursor and colour keys (`read_ghostty_config`).

## Rules pinned

| Rule | Test |
|---|---|
| 1 | `reads_key_value_lines_in_order_*`, `every_text_of_the_oracle_gives_the_directives_the_ts_gave` |
| 2 | `loads_config_ghostty_then_config_under_xdg_*` |
| 3 | `follows_config_file_includes_*`, oracle configs 3 and 4 |
| 4, 5 | `resolves_a_theme_against_the_config_s_themes_folder_*`, `every_theme_value_of_the_oracle_*`, oracle configs 5 to 8 |
| 6, 7 | `takes_every_key_the_pane_honours_*`, `an_empty_font_family_resets_the_list_*`, oracle configs 1, 2, 9 |
| 8 | `with_no_config_file_answers_the_empty_object_*` |
| 8a, 8b | `key_order::*` (3, text compared), `every_config_of_the_oracle_reads_as_the_ts_read_it` (now text compared) |
