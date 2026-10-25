# helpers
Source: slate-read.ts (37), slate-message.ts (27), slate-writes.ts (25), slate-refused.ts (19) at 65f6b89c7. Tests: none
direct in TS; the slates.test.ts cases that reach them (`a read carries each run's json`, `a refused write names
every error`, `a read asked for a run or a value without its $`). Oracle: `oracle/helpers.out`, the TS run under node.

## What it does
Four pure leaves of the slate area. `slate_read` cuts a run's record for a read and names a path that can only read
null. `slate_message` makes a carried value safe for a send's `slate:` line and finds and cuts its longest string.
`slate_writes` is the agent's write budget per thread. `slate_refused` builds the one-line refusal of a write the
parser or validator refused.

## Behaviour
1. `is_run_record(v)`: an object (not null, not an array) whose `state` is a string and `runs` a number (read:6-7).
2. `READ_OUTPUT_CHARS` = 4,000 UTF-16 units (read:10).
3. `run_shown(rec, name, named)` (read:14-28): not a record answers `rec`, or `null` when absent. `named`: only the keys
   `state, why, exit, ms, runs, stale, refreshing, endedAt`, in the record's own key order. Otherwise every key in
   order, except `out` is left out when the record has the key `json` at all (a `json: null` counts); a string
   `out` or `err` over 4,000 units becomes its first 4,000 units, then `… (N characters; values ["$name.field"]
   reads it whole)` (U+2026, a space, N the whole length); `lines` keeps its last 50; anything else is kept as is
   (a non-string `out` too).
4. `misread(doc, path)` (read:31-37): a path not matching `^[A-Za-z_]\w*(\.\w+|\[\d+\])*$` answers none (so any
   `$`-path, `len(...)`); its root is the text before the first `.` or `[`. In this order: a run of the document
   gives `X401` with message `<path> is not a source: $<root> is a run, read as $<path>; its record is also under
   runs.$<root>` and fix `$<path>`; a value or a derived value gives `<path> is not a source: $<root> is a value,
   read as $<path>` with the same fix; a root that is no `SLATE_SOURCES` name gives `<root> is not a source and not
   this slate's; a slate's own names start with $` with no fix; else none. With no document only the last check
   runs. The problem is `slate_problem("X401", ...)`: keys `code, name ("path-unknown"), message, fix`.
5. `defanged(value)` (message:6-11): in every string, at each line start (multiline), optional whitespace, `slate`,
   optional whitespace, `:` that is followed by optional whitespace and `{` becomes the leading whitespace, `slate`
   and U+FF1A `：`; the whitespace between `slate` and the colon goes. Strings inside arrays and object values are
   walked; keys and non-strings are not touched.
6. `longest(value, at)` (message:15-19): a string answers its own path and its length in UTF-16 units; an array or
   object answers the longest of its children in order, the first one on a tie; a number, bool, null, an empty
   array or object answers none.
7. `cut_at(value, at, keep)` (message:21-27): at the end of the path a string becomes its first `keep` units then
   ` (cut)`; anything else is unchanged. In an array only the element whose index equals the head is walked; in an
   object the key keeps its place.
8. `write_budget(now)` (writes:2-25): per thread a bucket `{tokens: 20, at, hour: []}` made at the first spend.
   `spend` refills `tokens = min(20, tokens + (at - was) / 1000 * 5)`, moves `at`, drops hour stamps 3,600,000 ms
   old or older, keeps the bucket, then: tokens below 1 answers `ceil((1 - tokens) / 5)` seconds; else 600 stamps in
   the hour answers `ceil((3,600,000 - (at - oldest)) / 1000)`; either way nothing is taken. Otherwise one token is
   taken, `at` stamped, and none is answered. `delete(thread)` answers whether a bucket was there.
9. `invalid(errors, warnings)` (refused:8-19): a `HostError` with kind `invalid`, `errors` the first 20 as JSON,
   `warnings` all, no fix, message `slate refused: N error[s]: ` then the one error said alone, or each of the first
   20 as `i) <said>` joined by a space, then ` And M more.` past 20. `said` is `[line L][, piece[.prop]]: ` (only
   the parts present, joined by `, `, the colon only when either is) then `code name message` then
   `. Did you mean <fix>?` when a fix is given.

## Edge cases
- `run_shown(undefined, ..)` is `null`; `{state:"x", runs:"1"}` is not a record and comes back whole (oracle).
- An `out` of exactly 4,000 units is not cut; 4,001 is (oracle `runShown.cut4000`, `runShown.cut`).
- `cut_at("abcdef", [], 0)` is ` (cut)`; `cut_at(5, [], 3)` is `5` (oracle).
- `longest({a:"xx", b:"yy", c:["zzz","q"]})` is `c[0]`, 3 (oracle).
- Budget at one instant: 20 spends pass, the 21st and 22nd answer 1; 100 ms later still 1 (half a token); 200 ms
  later a whole token is back and it passes (oracle `budget.*`). One spend a second: the 601st answers 3,000, and at
  3,600 s the oldest has aged out and it passes.
- `invalid([], [])` reads `slate refused: 0 errors: ` with the trailing space (a write whose parse gave no document
  and no error calls it so).

## Behaviour nobody wrote down
- Rule 8.21: `READ_OUTPUT_CHARS`' cut writes U+2026 here and three ASCII dots at slates.ts:1246; both kept.
- Rule 8.7: the three regexes are ported by hand; none relies on a surrogate half.

## Decisions
1. `defanged`, `longest` and `cut_at` take and answer `serde_json::Value`; object order is kept (`preserve_order`).
2. A path segment is `PathSeg::Key(String)` or `PathSeg::Index(usize)`.

## After the port
- `cut_at` on an object whose key is missing writes the key as `undefined`, which JSON drops; only `longest`'s own
  paths reach it, so it never happens.

## Tests

52 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| helpers Behaviour 1: slate-read.ts:6-7 | `helpers::is_run_record_wants_an_object_with_a_string_state_and_a_number_runs` |
| helpers Behaviour 2: READ_OUTPUT_CHARS counts UTF-16 units, 4,000 of them | `helpers::read_output_chars_is_4000_utf16_units` |
| helpers Behaviour 3 / Edge case: oracle runShown.named | `helpers::run_shown_named_keeps_only_the_summary_keys_in_the_records_order` |
| helpers Behaviour 3: the key list names refreshing, in the record's own order | `helpers::run_shown_named_keeps_refreshing_and_follows_the_records_key_order` |
| helpers Behaviour 3 / Edge case: oracle runShown.json_drops_out | `helpers::run_shown_leaves_out_the_out_when_the_record_has_json` |
| helpers Behaviour 3: a json of null counts as the key being there | `helpers::run_shown_counts_a_json_of_null_as_json` |
| helpers Behaviour 3: without json the out stays, every key in order | `helpers::run_shown_keeps_every_key_in_order_when_there_is_no_json` |
| helpers Behaviour 3 / Edge case: oracle runShown.cut | `helpers::run_shown_cuts_an_out_of_4001_and_says_where_the_rest_is` |
| helpers Edge case: oracle runShown.cut4000 | `helpers::run_shown_does_not_cut_an_out_of_exactly_4000` |
| helpers Behaviour 3: an err is cut the same way, named by its own field | `helpers::run_shown_cuts_a_long_err_naming_the_err_field` |
| helpers Behaviour 3 / Edge case: oracle runShown.lines | `helpers::run_shown_keeps_the_last_50_lines` |
| helpers Behaviour 3: anything else is kept as is, a non-string out and a non-array lines too | `helpers::run_shown_keeps_a_non_string_out_and_non_array_lines_as_they_are` |
| helpers Edge case: oracle runShown.notrecord, runShown.runsString | `helpers::run_shown_answers_a_non_record_whole` |
| helpers Edge case: oracle runShown.undefined | `helpers::run_shown_of_nothing_is_null` |
| helpers Behaviour 4: a run of the document | `helpers::misread_names_a_run_read_without_its_dollar` |
| helpers Behaviour 4: the root is the text before the first `.` or `[` | `helpers::misread_cuts_the_root_at_a_bracket_too` |
| helpers Behaviour 4: a value of the document | `helpers::misread_names_a_value_read_without_its_dollar` |
| helpers Behaviour 4: a derived value reads as a value | `helpers::misread_names_a_derived_value_as_a_value` |
| helpers Behaviour 4: a run is checked before a value | `helpers::misread_checks_a_run_before_a_value` |
| helpers Behaviour 4: a root that is no source and no name of the slate | `helpers::misread_refuses_a_root_that_is_no_source_and_not_the_slates` |
| helpers Behaviour 4: a source of the host reads fine | `helpers::misread_answers_none_for_a_source` |
| helpers Behaviour 4: a path the pattern does not match answers none | `helpers::misread_answers_none_for_a_path_the_pattern_does_not_match` |
| helpers Behaviour 4: with no document only the last check runs | `helpers::misread_with_no_document_checks_only_the_source_name` |
| helpers Behaviour 5 / Edge case: oracle defanged.line | `helpers::defanged_turns_a_slate_line_colon_into_a_fullwidth_one` |
| helpers Behaviour 5: oracle defanged.nested | `helpers::defanged_walks_arrays_and_object_values` |
| helpers Behaviour 5: keys and non-strings are not touched | `helpers::defanged_leaves_keys_and_non_strings_alone` |
| helpers Behaviour 6 / Edge case: oracle longest.tie | `helpers::longest_takes_the_first_on_a_tie` |
| helpers Behaviour 6: oracle longest.none | `helpers::longest_of_no_string_is_none` |
| helpers Behaviour 6: oracle longest.string | `helpers::longest_of_a_string_is_its_own_path_and_length` |
| helpers Behaviour 6: lengths are UTF-16 units | `helpers::longest_counts_utf16_units` |
| helpers Behaviour 7: oracle cutAt.obj | `helpers::cut_at_cuts_the_string_at_the_path_and_keeps_the_rest` |
| helpers Behaviour 7: oracle cutAt.root | `helpers::cut_at_the_root_cuts_a_string` |
| helpers Edge case: oracle cutAt.keep0 | `helpers::cut_at_keep_zero_is_just_the_marker` |
| helpers Edge case: oracle cutAt.nonstring | `helpers::cut_at_a_non_string_is_unchanged` |
| helpers Behaviour 7: in an array only the element at the head is walked | `helpers::cut_at_walks_only_the_element_at_the_head_of_an_array` |
| helpers Behaviour 7: in an object the key keeps its place | `helpers::cut_at_keeps_an_objects_key_order` |
| helpers Behaviour 7: keep counts UTF-16 units | `helpers::cut_at_keep_counts_utf16_units` |
| helpers Behaviour 8 / Edge case: oracle budget.burst22 | `helpers::a_burst_of_20_passes_and_the_21st_and_22nd_wait_one_second` |
| helpers Edge case: oracle budget.after100ms | `helpers::half_a_token_after_100_ms_still_waits_one_second` |
| helpers Edge case: oracle budget.after200ms | `helpers::a_whole_token_is_back_after_200_ms` |
| helpers Edge case: oracle budget.10s | `helpers::ten_seconds_later_three_writes_pass` |
| helpers Behaviour 8: the bucket refills no further than the burst | `helpers::the_bucket_refills_no_further_than_20` |
| helpers Behaviour 8: a refused spend takes nothing | `helpers::a_refused_spend_takes_nothing` |
| helpers Behaviour 8: every thread has its own bucket | `helpers::each_thread_has_its_own_bucket` |
| helpers Behaviour 8: oracle budget.hour.600th_and_601st, budget.hour.at600s, budget.hour.at3600s | `helpers::the_601st_write_of_an_hour_waits_and_the_oldest_ages_out_at_3600_seconds` |
| helpers Behaviour 8: delete answers whether a bucket was there | `helpers::delete_answers_whether_a_bucket_was_there_and_the_thread_starts_full_again` |
| helpers Behaviour 9 / Edge case: oracle invalid.one | `helpers::invalid_with_one_error_says_it_alone_with_its_place_and_fix` |
| helpers Behaviour 9 / Edge case: oracle invalid.two | `helpers::invalid_with_two_errors_numbers_them_and_keeps_the_warnings` |
| helpers Edge case: oracle invalid.zero | `helpers::invalid_with_no_errors_keeps_the_trailing_space` |
| helpers Behaviour 9 / Edge case: oracle invalid.22 | `helpers::invalid_lists_the_first_20_errors_and_counts_the_rest` |
| helpers Behaviour 9: exactly 20 errors are all listed and nothing is added | `helpers::invalid_with_exactly_20_errors_adds_no_more_line` |
| helpers Behaviour 9: said is `[line L][, piece[.prop]]: ` with only the parts present | `helpers::invalid_said_joins_only_the_parts_that_are_present` |
