# terminal
Source: `terminal.ts` (174 lines) at a3b870a6e. Tests: `test/terminal.test.ts` (13 cases, four `it.each`). Rust:
`tests/detect_terminal.rs`. Calls `crate::ghostty_config::{GHOSTTY_BUILT_IN_FONT, parse_ghostty_directives,
read_ghostty_config}` (the other half of collect); `read_ghostty_config(host, TerminalScheme::Dark)` stands for the
TS call with its default scheme, and its `files` and `font_family` fields are what terminal reads.

## What it does
One `shell/terminal-font` row naming the font family the person's terminal draws with, so the app's terminal pane can
default to it. Five terminals are known, in this order: Ghostty, iTerm2, WezTerm, kitty, Alacritty.

## Behaviour
1. `usable(family)` (terminal.ts:14-17): `js_trim`, strip one leading `"` or `'` and one trailing `"` or `'`
   independently, `js_trim` again; `None` for no input or when the lowercased result is `monospace`, `mono` or `""`.
2. `primary_face(families)`: walk in order; an unusable family empties the list so far, a usable one is appended; the
   answer is the list's first (terminal.ts:25-33). `ghostty_font(text)` is that over the `font-family` directives'
   values (terminal.ts:20-22).
3. `iterm_font(xml)` (terminal.ts:37-45): the default bookmark guid from
   `<key>Default Bookmark Guid</key>\s*<string>([^<]*)</string>`; the bookmark starts at the first
   `<key>Guid</key>\s*<string>{escape_reg_exp(guid)}</string>` and runs to the next `<key>Guid</key>` searched from one
   unit past that start; no guid or no such bookmark reads the whole text. Its first
   `<key>Normal Font</key>\s*<string>([^<]*)</string>`, less a trailing `\s+[\d.]+` (anchored at the end before any trim, so `Monaco 12 ` with a trailing space keeps its size,
   and `usable` then trims it to `Monaco 12`), through `usable`. The default bookmark with no Normal Font is `None`: no other bookmark
   is tried.
4. `wezterm_font(lua)` (terminal.ts:48-52): drop `--[[ ... ]]` blocks (lazy, across lines) then `--` to end of line;
   the first match of `\bfont(?:_with_fallback)?\s*\(?\s*\{?\s*(?:\{\s*)?(?:family\s*=\s*)?(["'])([^"']+)\1`, group 2
   through `usable`, else GHOSTTY_BUILT_IN_FONT. Never `None`.
5. `kitty_font(conf)` (terminal.ts:55-61): the first match of `^\s*font_family\s+(.+?)\s*$` (multiline, JS `\s`, so a bare
   `font_family` line matches across the newline and takes the next non-blank line as its value; only that first
   match is used, a later `font_family` is never tried even when the first is unusable); in its value a
   `\bfamily\s*=\s*(?:"([^"]*)"|(\S+))` gives the family, else the whole value; through `usable`.
6. `alacritty_font(text)` (terminal.ts:64-72): the first of three TOML forms that matches (`[font.normal]` table,
   `normal = { family = ... }`, `normal.family = ...`), else the YAML form; through `usable`. Patterns at
   terminal.ts:66-70, multiline. A TOML form that matches gives its family through `usable` and the answer stops
   there: an unusable family is `None` and the YAML form is not tried. None of the patterns is anchored to a real
   table header, and the YAML `family:` with nothing after it takes the next line (JS `\s` crosses the newline).
7. Configured fonts (terminal.ts:98-150): Ghostty: `read_ghostty_config`; no files is `None`, else
   `primary_face(font_family)` or the built-in. iTerm2: only on darwin and when
   `~/Library/Preferences/com.googlecode.iterm2.plist` exists, run `defaults ["export","com.googlecode.iterm2","-"]`
   and read its XML; the plist is never read as a file; a `defaults` answer of `None` or `""` gives `None` (no font). WezTerm: the first readable of `~/.wezterm.lua`,
   `~/.config/wezterm/wezterm.lua`. kitty: `~/.config/kitty/kitty.conf`. Alacritty: the first readable of
   `~/.config/alacritty/alacritty.toml`, `~/.alacritty.toml`, `~/.config/alacritty/alacritty.yml`, `~/.alacritty.yml`.
   "Readable" is `read_text` answering, each path through `expand`: an empty file answers `""`, so it is readable,
   stops the search over that terminal's paths, and is parsed (giving no font). WezTerm's configured font is never
   `None` once a file is readable (`wezterm_font` falls back to the built-in). Ghostty's `read_ghostty_config` now
   answers an ordered JSON map (ghostty_config rule 8a): read `files` and `fontFamily` from it.
8. Built-ins: Ghostty and WezTerm draw with GHOSTTY_BUILT_IN_FONT; iTerm2, kitty and Alacritty have none.
   Ghostty is installed when `/Applications/Ghostty.app` exists on darwin, or `which("ghostty")` on linux; no other
   terminal has an installed check.
9. `detect_terminal_font` (terminal.ts:159-174): own terminals are those whose name pattern (`/ghostty/i`, `/iterm/i`,
   `/wezterm/i`, `/kitty/i`, `/alacritty/i`) matches `host.terminal` (every matching row is own; `None` or `""` matches
   none); for each own terminal in table order, its configured font or its built-in wins; an own terminal with
   neither is passed over, and an own terminal is never asked again among the others. Then each other terminal's configured font in table order. Then the first
   other terminal with a built-in whose installed check is true. Else `[]`.
10. The row: rung shell, id TERMINAL_FONT_ID, label `terminal font: <font> (<terminal name>)`, `paths: []`, `bytes: 0`,
    default bring, `font` (terminal.ts:154). The names are `Ghostty`, `iTerm2`, `WezTerm`, `kitty`, `Alacritty`.

## Edge cases
- `iterm_font("<plist><dict></dict></plist>")` is `None`; `kitty_font("font_family monospace\n")` is `None`.
- `wezterm_font("-- config.font = wezterm.font(\"Old\")\n")` is the built-in.
- With `terminal` set to an unknown program, every terminal is "other".

## Decisions
1. The `Terminal` table is a private list in table order; its `program` patterns are case-insensitive.

## After the port
- `usable` strips a leading and a trailing quote independently, so `"Fira Code'` reads as `Fira Code`.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/terminal.json` (`oracle/w2-terminal.oracle.ts`), replayed by `tests/detect_terminal.rs`
`w2_oracle::*`.

- W1. kitty: rule 5's first match across lines, `family=` anywhere in the value wins, no comment stripping, CRLF
  (`kitty_font`).
- W2. WezTerm: comment stripping is the two regexes only (`--[=[` long brackets are not comments; `--` inside a
  quoted name cuts the line), the regex sees a family only right after the call; lookalikes give the built-in
  (`wezterm_font`).
- W3. Alacritty: rule 6's forms, unanchored headers, `[^[]*?` stopping at the next table, YAML's next-line value, a
  matched unusable TOML form stopping the search (`alacritty_font`).
- W4. iTerm2: rule 3 with the default bookmark that has no font (`None`), a literal guid, no default key (the whole
  text), sizes and names (`iterm_font`).
- W5. Ghostty's text parse (`ghostty_font`).
- W6. `detect_terminal_font`: rule 9's own-terminal walk, an own terminal with nothing to say passed over, an empty
  file readable, a readable WezTerm file never `None`, Alacritty stopping at its first readable file, Ghostty's reset
  list giving the built-in, iTerm2's empty `defaults`, the per-terminal read order (`detect_terminal_font`).

## Tests
| Rule | Test |
|---|---|
| 1, 2 | `ghostty` (it.each, 3 rows), `usable_strips_one_quote_at_each_end_and_drops_generic_names` |
| 3 | `iterm2_the_default_bookmarks_normal_font_without_its_size_else_the_first` |
| 4 | `wezterm` (it.each, 10 rows) |
| 5 | `kitty` (it.each, 5 rows) |
| 6 | `alacritty` (it.each, 7 rows) |
| 7, 8 | `ghostty_with_no_font_family_or_with_no_config_at_all_when_it_is_the_running`, `ghostty_on_a_linux_computer_with_no_config_draws_with_the_same_built_in_font`, `reads_ghosttys_macos_config_location_and_prefers_the_one_under_config`, `iterm2s_plist_is_read_through_defaults_never_as_a_file_and_only_on_a_mac`, `wezterm_kitty_and_alacritty_configs_each_name_the_row_after_their_terminal` |
| 9, 10 | `with_no_running_terminal_known_a_config_that_names_a_font_beats_an_installed`, `the_terminal_the_collector_runs_in_wins_over_the_others_configs`, `a_terminal_whose_config_names_no_usable_font_is_passed_over_for_the_next_one` |
