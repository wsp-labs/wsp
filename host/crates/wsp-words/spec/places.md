# places
Source: `packages/protocol/src/words/places.ts` (642 lines) at a3b870a6e. Tests: `packages/protocol/test/format.test.ts`, `hosts-menu.test.ts`, `workspace-theme.test.ts`, `absent-computer.test.ts`, `cost-history.test.ts` (37 cases ported; 1 ignored as constants alone; `placeAddSheetWord` and `opacityCap` cases skipped, not in this unit), plus `pinned` in `tests/places.rs` (every value printed by the TS under node).

## What it does
The words of the computers table and a computer's detail rows, a provider's remove and forget, the import and export dialogs and their progress bar, the terminal config readout, the hosts menu and the hosts table, a workspace theme's CSS variables, and a cloud's plan line.

## Behaviour
1. `import_progress(events, ws)`: None for no events or at a `failed` event. Per event: planned and consented say "Starting"; packing says its message less one trailing "."; uploading says `Uploading[ sessions][,] <fmt_bytes(total)>` (" sessions" once a landing has passed; the comma only then) and raises the fraction to bytes/total where both are set and total > 0, never lowering it; landing counts a landing, says `Landing on <ws>`, fraction 1; done says "Done", fraction 1 (places.ts:56-90).
2. `export_progress(events, here)`: the same fold; each packing counts, the pass is "the folder" for the first packing and "sessions" after; downloading says `Downloading <pass>[, <fmt_bytes(total)>]` with the same fraction rule; landing `Landing` or `Landing on <here>` for a non-empty here (places.ts:96-124).
3. `terminal_config_lines(c)`: `[NO_TERMINAL_CONFIG_LINE]` for no files; else `Read <files, joined ", ">` then `key = value` for each set row in this order: font-family (", " joined, only non-empty), font-size, theme, background, foreground (`#rrggbb` lower-case hex), palette (`<n> of 16 colors` counting non-null, only n > 0), selection-background, cursor-color, cursor-style, cursor-style-blink, window-padding-x (`l` or `l,r`), window-padding-y (`t` or `t,b`), background-opacity, background-blur; numbers via `number_string` (places.ts:135-156).
4. `look_word`: first UTF-16 unit upper-cased plus the rest (places.ts:160-162). `fmt_percent`: `js_round(share*100)%` (places.ts:177-179). `fmt_rgb`: `rgb(r g b)` or `rgb(r g b / <fmt_percent(alpha)>)` (places.ts:602-605).
5. `shell_version_notice(shell, host, label)`: no shell: `this app is older than <named>, which is <host>: get the new app`, update true; equal by `compare_versions`: None; older: `this app is <shell>, <named> is <host>: get the new app`, true; newer: `...: run the app's own host`, false. named = "the host" or `the host <label>` (places.ts:197-204).
6. `offline_for(ms)`: minutes = max(0, floor(ms/60000)); `<m> min` under 60, `<h> h` under 24 h, else `<d> d` (places.ts:212-217).
7. `place_workspaces_parts(view, count, month)`: count is `number_string(count)`; a build note where `view.build` is set; for kind "provider" with a month, `<fmt_cost> this month`; else no note. `place_workspaces_cell` joins with ", " (places.ts:225-240).
8. `spend_meter_word(today, cap)`: `<fmt_cost(today)>/<$cap if integral else fmt_cost(cap)>` (places.ts:247-249). `place_spend_line`, `places_spend_foot` per places.ts:254-263.
9. `agent_version_word`: the first `\d+\.\d+\.\d+` in the text, else the text trimmed (places.ts:286-288). `strict_version(raw)`: `js_trim`, one leading lower-case "v" off, then the whole text matches semver.org's pattern and is <= 64 units, else None (places.ts:291-299).
10. `agents_cell(place)`: per agent id in order, the id, its version word where one is recorded, its sign-in word where one is recorded, joined by " "; agents joined by ", "; "" for none (places.ts:304-312).
11. `codex_key_refused_line(env, reason, login)`: said = `js_trim(utf16::cut(js_trim(reason), 80))`; `: <said>` only when non-empty (places.ts:323-326).
12. Provider remove and forget lines follow places.ts:394-498 word for word: tasks counted with "its task" / "its N tasks"; `provider_key_still_line` joins places with " and in " and readers with " or ", ending "there", "of both" or "of each"; `provider_kept_line` names the first 5 (`PROVIDER_LEFT_NAMED`) as `id (said)` and counts the rest; image lists join 1 or 2 names with " and ", 3 or more as `a, b and c`, each with ` (<fmt_bytes>)` where sized.
13. `join_roads(token, urls, relay)`: one road per url in order, then the relay last with the relay note; keys url, line, note (places.ts:502-505). `hosts_menu_items`: this computer first with id `switch:`, checked when current is None; each host `switch:<alias>`, checked when it is current; group "hosts", enabled (places.ts:514-519). `host_menu_action(id)`: None unless it starts with `switch:`; Some(None) for an empty alias (places.ts:523-527).
14. `host_beat_word(away)`: None (null) "not yet"; <= 120000 ms `up <fmt_duration(max(0, away))>`; else `away <offline_for(away)>` (places.ts:562-566). `hosts_table`: the header row, then per row host, address (else "not up yet" when awayMs is absent or null, "waits on its next start" otherwise), state ("" when absent), device, connector, key, "default" only for true (places.ts:569-582).
15. `fmt_theme_vars(theme, dark)`: alpha = `effective_opacity`; one dot `linear-gradient(c, c)`, two the two linear gradients at 160deg and 340deg with the second colour first, three or more the two radial gradients and the linear one of the first three; grain `number_string(grain)`; tint `fmt_rgb(theme_ink)` (places.ts:607-618).
16. `exit_line(code)`: None for null, else `exit <code>` (places.ts:628). `cloud_plan_line`: trial `Trial: <n machine(s)> at once[, <day> starts a day], <sizes joined " and "> sizes`; paid `<$N plan | Plan>: <n machines> at once[, <m> starts a minute, <h> an hour, <d> a day]` with N from a key `box_<digits>` (places.ts:632-639).

## Edge cases
- An upload after a landing whose bar is already full keeps fraction 1 and says "sessions" (`trips_fold_their_events_into_one_line_and_a_bar_that_never_drops`).
- A download with total 0 says ", 0 B" and leaves the fraction where it was.
- `strict_version("1.2.3\n")` is "1.2.3" (trim); "V1.2.3", "01.2.3", "1.2.3-01" are None; 64 units pass, 65 do not (`versions_are_strict_semver_under_64_units`).
- `spend_meter_word(10, 0)` is "$10.00/$0"; `fmt_percent(-0.005)` is "0%" (round toward +infinity).
- `host_menu_action("switch:switch:")` names the alias "switch:".

## Behaviour nobody wrote down
- Rules 8.4, 8.11, 8.12: the 64 and 80 caps count units; numbers via `number_string`; rounding via `js_round`.
- Rule 4.26: `join_roads`, `hosts_menu_items` and `place_workspaces_parts` key order is the TS literal's.

## Tapes
`places.list` rows, `places.holds`, `places.remove`; the desktop's hosts menu.

## Decisions
1. `place_spend_line` takes the month and rate as two numbers; `host_menu_action` answers `Option<Option<String>>` for undefined, `{alias: null}` and `{alias}`.
2. `KeyStill` is an enum of the two TS shapes.
3. `fmt_theme_vars` reads `dot_colour`, `effective_opacity` and `theme_ink`, declared in `wsp_wire::types::workspace_look` until the `workspace_look` unit is written.
4. Nested TS constant objects (`PLACE_INSTALL`, `PLACES_WORDS`, `PROVIDER_LEFT_WORDS`, `HOST_WORDS`) are modules of constants and functions, values copied by script.

## After the port
- `spend_meter_word(10, 0)` reads "$10.00/$0", a cap of nothing written as whole dollars.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `the_import_dialog_s_words::*`, `the_export_dialog_s_words::*`, `trips_fold_their_events_into_one_line_and_a_bar_that_never_drops` |
| 3, 4 | `terminal_config_lines::*`, `terminal_config_and_look_words` |
| 5 | `a_desktop_shell_and_the_host_that_served_it_its_page::*`, `the_shell_notice_reads_semver_order` |
| 6, 7, 8, 10 | `what_the_computers_table_says_about_a_computer::*`, `the_computers_table_cells` |
| 9, 11 | `versions_are_strict_semver_under_64_units` |
| 12 | `a_providers_remove_and_forget_say_what_went_and_what_stays` |
| 13, 14 | `hosts_menu_items::*`, `the_one_listing_of_the_hosts_a_computer_can_reach::*`, `hosts_menu_and_table` |
| 15 | `the_custom_properties::*` |
| 16 | `a_clouds_plan_line` |
