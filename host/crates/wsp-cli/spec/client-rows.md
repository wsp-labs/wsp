# client-rows

Source: `packages/host/src/verbs/client.ts` lines 453-735 (283 lines) at 65f6b89c7. Tests: `places.test.ts` "the table wsp places prints" (14), `recipes.test.ts` (2 of 38 touch these lines), `usage-table.test.ts` (5). Rust: `src/verbs/client.rs`, "Output and the rows the listings print"; tests `tests/client_rows.rs`, `tests/client_rows_reads.rs` (oracle rows from `tests/fixtures/client.json`, ported cases, whole lines the TS printed).

## What it does
How every verb prints (`Out`), the C1-escaping writer for the tool server's stdout, the rows `wsp places`, `wsp computers`, `wsp recipes` and `wsp usage` print, the two reads `wsp usage` and `wsp computers` make, the computer settings write, the flag words for `--nap`, `--turn-limit`, `--delete-after`, `--spend`, and the one table renderer.

## Behaviour
1. `Out::emit` (460-465): under --json `io.log(json_line(value))`, nothing else; without, `io.log(line)` only when a line is given. `stream` (466-468) goes to `io.stream` only without --json. `Out::quiet()` drops both.
2. `C1Escaped` (474-479): every chunk through `escape_c1`, a UTF-8 character split across writes decoded whole first; `flush` writes what the decoder holds.
3. `place_lines` (483-504): empty list is the one sentence; header `PLACE KIND CORES MEMORY DISK FREE COPIES PRESENT FORKS LAST SEEN DEFAULT BEHIND IMAGE`; memory `fmt_bytes(memMb * 1024 * 1024)`; a provider row shows `fmt_price(rate ?? 0)` where the others show yes/no, and no LAST SEEN; FORKS `running of running+room`.
4. `computer_lines` (518-554): 18 columns; the STARTS column (index 11) is dropped from every row, header included, unless some row carries `starts`; a pending add with no row of its own, or whose place is not listed, is a row `name ?? address`, `pending`, then its word and sentence in STATE and TOOLS; a computer with a pending add reads that add's step words in TOOLS; THREADS or MACHINES `running/atOnce` by `place_room(p).noun`; SPEND only where the kind has a limit and the spend was read.
5. `follow_line` (557-560): none or `NO_RECIPE` keeps what it has; else follows it.
6. `recipe_lines` (563-566): empty is the one sentence; SAVED is the first 10 characters of `savedAt` or "".
7. `recipe_shown_lines` (569-590): name and summary; one row per kind of `RECIPE_KINDS` with rows, kind padded to 8; configs rows skip undefined entries; a row's fields skip undefined and empty arrays, arrays joined with ", "; `keys` only when `copyKeys === true`; `followed by` no computer or the list; `hash`.
8. `one_of` (600-604): None passes; a word outside refuses `<--name[ for on]> takes one of a, b, and got "<JSON.stringify(word)>".` with `Name one of those.`
9. `read_usage` / `read_computers` (608-611, 653-656): both requests go out together, before either answers (rule 4.16); answers carried as the host wrote them; `pending` empty when absent.
10. `account_named` (614-622): exact key first; then label or address case-insensitively; one match wins; none is kind `not-found` `no_such_account_line(word)`; several is usage `<word> names N accounts: <list_words(keys)>.` with `Name one by its key.`
11. `usage_table_lines` (625-650): accounts table or "No agent is signed in on any computer."; a blank line; the used table or `USAGE_WORDS.noUse + "."`; split "model" adds an AGENT column first.
12. `set_computer` (664-669): a word is looked up with one `places.list`; one `places.set` `{placeId, ...set, reset (only non-empty), ...also given}`; answers the row and the name it was asked by.
13. `computer_set_lines` (673-680): rename first (only when the name changed), login (trimmed), recipe, then the settings line where settled or nothing else was named.
14. `nap_asked` (688-693): "off" is 0; `/^\d+$/`, 1 to 180 minutes; else refused naming `--nap[ for on]` and `JSON.stringify(word)`. `turn_limit_asked` (696-701): 1 to 24 hours. `delete_after_asked` (704-708): 0 to 1,440, no "off". `dollars_asked` (711-715): JS `Number(word)`, finite, not below 0, a blank word refused; so "1e1", "0x10", " 5", "+5" are read as JS reads them.
15. `table` (731-734): widths per column across all rows, measured in UTF-16 units (`cell.length`); cells padded with spaces to their column's width except a row's last cell; joined by two spaces; `trimEnd` (white space, the JS set) on each line. Rows may be ragged.

## Edge cases
- Every number word row, the limits and each refusal's fix: `client.json` `asked` (24 words per flag, plus 4 for dollars).
- Emoji counts as 2 units, `é` as 1, `日本` as 2 (`table_pads_by_utf16_units...`).

## Behaviour nobody wrote down
- Rule 8.3/Proposal 5: widths are UTF-16 units, not chars or bytes.
- Rule 4.16: the two reads of usage and computers are sent at once.

## Decisions
1. `ComputerRef` carries the word or the row `setComputer` takes as `string | PlaceView`.

## After the port
- `places.test.ts` "says beside the engine how a workspace's copy..." looks for an ENGINE column the header no longer has, so its order check is always true (found by the test porter; the Rust test checks the real order).

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `formatter_without_json_*`, `formatter_with_json_*`, `formatter_prints_the_whole_run_as_the_ts_did` |
| 2 | none (no TS test; the tool server's stdio is `wsp-mcp`'s); reviewer reads 474-479 |
| 3, 4 | `rows::*` (14 ported) and `whole_lines_as_the_ts_printed_them::place_lines_*`, `computer_lines_*` |
| 5-7 | `builders_file::*`, `recipe_lines_*`, `recipe_shown_lines_*`, `recipe_saved_line`, `recipe_removed_line` tests |
| 8 | `one_of_matches_what_the_ts_answered_for_every_row` |
| 9 | `asks_for_what_was_used_as_the_usage_page_does...` (order and fields), `client_rows_reads::the_computers_read_asks_the_list_and_the_spend_together...` |
| 10 | `client_rows_reads::an_account_is_named_by_its_key_first...`, `a_word_no_account_answers_is_not_found...` |
| 12 | `client_rows_reads::one_settings_write_names_the_computer_by_its_id...`, `a_row_already_read_is_not_listed_again...`, `an_unknown_computer_is_refused_before_anything_is_written` |
| 13 | not pinned here; `computers set` (cli-b) prints it; the reviewer reads it against 673-680 |
| 11 | `used_by_*`, `says_each_account_s_banked_resets...`, `names_the_agent_beside_each_model...`, `usage_table_lines` whole lines |
| 14 | `nap_asked_*`, `turn_limit_asked_*`, `delete_after_asked_*`, `dollars_asked_*`, `the_limits_the_refusals_name_*` |
| 15 | `table_matches_the_lines_the_ts_printed_for_every_set_of_rows`, `table_pads_by_utf16_units_and_never_pads_the_last_cell` |
