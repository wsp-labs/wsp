# base
Source: `packages/protocol/src/words/base.ts` (65 lines) and `words/boat.ts` (53 lines) at a3b870a6e. Tests: `packages/protocol/test/format.test.ts` (8 cases on these names), `format-cases.test.ts` (the plural rows of `daemon/fixtures/contract/formats.json`), plus `pinned` in `tests/base.rs`.

## What it does
The smallest words every other unit builds on: a count with its noun, a name list, a line cut at a word inside a room of UTF-16 units, a text's last line, and the one test for a boolean mark on an error. Boat's plan line from its `/limits` answer.

## Behaviour
1. `plural(n, noun)`: `${n} ${noun}` plus `s` unless n is exactly 1; n written by `js::number_string` (base.ts:5).
2. `lower_first`: the first UTF-16 unit lowered (`charAt(0).toLowerCase()`), the rest as written; "" stays "" (base.ts:10).
3. `listed_name`: a name holding "," is written as `JSON.stringify(name)` (quotes, `\"`, `\\`, `\t`, `\n`, `\uXXXX` for other C0); else as is. `name_list` joins with ", " (base.ts:14-21).
4. `cut_line(text, room)`: text whose length is <= room comes back whole. Else head = room - 1 (the ellipsis is one unit); the words of `words_within(text, head)`, else the first `head` units with a split high surrogate dropped and `[\s,;:]+$` trimmed; then "…" (base.ts:33-47).
5. `words_within(line, room)`: the first room + 1 units; the last " " in them at index > 0 marks the cut; the part before it with `[\s,;:]+$` trimmed; None when there is no such space (base.ts:51-55). Only U+0020 is a boundary; the trim uses JS whitespace (rule 8.6).
6. `last_line`: split on `\r?\n`, drop lines that are empty after `js_trim`, take the last, collapse JS whitespace runs to one space, `js_trim` it; None when no line is left (base.ts:59-63).
7. `marked(e, mark)`: true exactly when `e.marks` holds `mark` (base.ts:65).
8. `boat_added_line(limits, size)`: no limits: `Boat added; <size words>.`; a trial: `Boat added: trial (<held>); small and default sizes.` with held = hours (whole, else `toFixed(1)`) + ` h`, then `N machine(s) at once`, then the three start windows; the brackets only when held is non-empty; paid: `Boat added: <$N plan | a paid plan>, <machines>, <starts>; <size words>.` Size words: `<type> <fmt_cpus> <fmt_mem_gb> at <fmt_rate_dollars>/h until you pick another size` (boat.ts:31-42).
9. `boat_forks_no_first_line(flags)`: flags joined with " and ", `does` for exactly one flag else `do` (boat.ts:45).

## Edge cases
- `plural(1.5, "file")` is "1.5 files"; `plural(-1, ..)` "-1 files"; 1e21 writes "1e+21" (`plural_writes_the_count_...`).
- `cut_line("abcd", 3)` is "ab…"; a single word longer than the room is cut mid-word; a pair at the edge is dropped whole (`cut_line_counts_utf16_units_...`).
- `words_within(" one", 5)` is None: a space at index 0 is no boundary.
- `last_line("x\n\u{85}")` is "\u{85}": U+0085 is not whitespace to JS; `"x\n\u{feff}"` gives "x".
- Boat hours 2.25 read "2.3 h" (`toFixed` ties away from zero); `boat_forks_no_first_line([])` is "so  do nothing here." (two spaces).

## Behaviour nobody wrote down
- Rule 8.4: `room` counts UTF-16 units; rule 8.5: the cut never leaves half a pair. Rule 8.6: `\s` and `trim` are JavaScript's.
- Rule 8.11: every number in a sentence goes through `js::number_string`.

## Tapes
None of these ops alone; their words reach most tapes.

## Decisions
1. Counts are `f64`, as the TS `number`: a caller holding a `usize` passes `n as f64`.
2. `BoatLimitsRead.starts` is all three windows or none, as the TS `Record` type says.

## After the port
- `boatForksNoFirstLine([])` prints two spaces; no caller passes an empty list today.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `is_the_one_rule_for_a_count_and_its_noun_...`, `s_s_reads_j`, `plural_writes_the_count_as_javascript_does_...` |
| 2, 3 | `listed_name_and_name_list::*`, `plural_writes_the_count_as_javascript_does_...` |
| 4, 5 | `cut_line::*`, `cut_line_counts_utf16_units_cuts_at_a_word_and_never_splits_a_pair` |
| 6 | `last_line::*`, `last_line_skips_blank_lines_by_javascripts_whitespace_and_collapses_runs` |
| 7 | `marked_reads_a_mark_the_error_carries_and_no_other` |
| 8, 9 | `boat_added_line_says_only_what_the_account_answered` |
