# js
Source: no TS file of its own; the JavaScript built-ins the TS host calls (`String.prototype.length`, `slice`, `indexOf`, `lastIndexOf`, `padStart`, `padEnd`, `trim`, `Array.prototype.sort`, `Math.round`, `Number.prototype.toFixed`, `String(n)`, `toLocaleString`), as spec chapter 08 (branch `docs/2442-rust-host-spec`, `08-text-numbers-time.md`) names them. Tests: `tests/js.rs` (19 cases), every expected value printed by node 22.23.4.

## What it does
The one home of the rules where JavaScript and Rust compute the same expression differently. Every other unit of this crate counts, cuts, trims, rounds and writes numbers through it (rules 8.1, 8.3, 8.6, 8.11 to 8.14).

## Behaviour
1. `utf16::len` is `s.length`: UTF-16 code units, a char above U+FFFF counts 2 (rule 8.1).
2. `utf16::slice(s, start, end)` is `s.slice(start, end)`: negative indexes count from the end, indexes clamp to `[0, len]`, `start >= end` gives "".
3. `utf16::cut(s, n)` is `slice(0, n)`, `utf16::tail(s, n)` is `slice(-n)` for n > 0 and "" for n = 0.
4. `utf16::index_of`, `last_index_of`, `pad_start`, `pad_end` count units as their JS namesakes; an empty needle is found at `min(from, len)` (`indexOf`) or `len` (`lastIndexOf`); an empty fill leaves the string as it is.
5. `utf16::cmp` orders as a bare `.sort()`: by code units, so U+E000..U+FFFF sort after a char above U+FFFF (rule 8.8).
6. `is_js_whitespace` is ECMA-262 WhiteSpace plus LineTerminator: U+0009..U+000D, U+0020, U+00A0, U+1680, U+2000..U+200A, U+2028, U+2029, U+202F, U+205F, U+3000, U+FEFF. `js_trim*` and `js_collapse_whitespace` (`replace(/\s+/g, " ")`) use it (rule 8.6).
7. `js_round` is `Math.round`: `floor(x + 0.5)` with the exact-half care JS takes (`0.49999999999999994` rounds to 0), ties toward +infinity, a result in (-0.5, 0] is -0 (rule 8.12).
8. `to_fixed(x, d)` is `toFixed`: decided on the exact binary value, an exact tie away from zero; `|x| >= 1e21` gives `number_string(x)`; a negative value that rounds to zero keeps its sign ("-0.00"), negative zero itself does not ("0.00") (rule 8.13).
9. `number_string` is `String(x)`: shortest round-trip digits, plain notation for 1e-6 <= |x| < 1e21, else `d.ddde+N` / `d.ddde-N`; "-0" is "0" (rule 8.11, 4.25). Where two spellings of the shortest length are equally near `x` (a 16 or 17 digit number whose exact value ends in a 5 one place past them), the even last digit is written, as V8 writes it: `String(100000000000000.125)` is `100000000000000.12`, and `.375` is `.38` (Rust's `{:e}` takes the upper one).
10. `locale_number(x, grouping, min, max)` is `toLocaleString` with `minimumFractionDigits`/`maximumFractionDigits`: round the shortest decimal of `x` (the digits `number_string` would give) to `max` places half away from zero, drop trailing zeros down to `min`, group the integer part (en-US threes; en-IN last three then twos), lead with "-" for a negative value even when it rounds to zero (rule 8.14).

11. `collate(a, b)` is `a.localeCompare(b)` with the root locale (ICU4X's collator), only its sign read (rule 8.8).
12. `serialize_f64` / `serialize_opt_f64` are the `serialize_with` every serialized `f64` field uses, so an integral value is written `1`, not `1.0` (rule 4.25), and `serde_json::Value` equality matches a `json!` literal. A finite value that is not a whole number up to 2^53 is written as `number_string` writes it (`0.000003`, not serde_json's `3e-6`; `1e+21`, not `1e21`).
13. `to_number` reads a `0x`, `0o` or `0b` integer exactly and rounds it once, to the nearest double with ties to even, however many digits it has; folding digit by digit in an `f64` rounds at every step past 2^53. `ports_asked` reads its port through it.
14. `js_key_order(entries)` is the order `Object.keys` lists: the keys that read as array indexes (`0`, `7`, `10`, never `01` or `-1`) first, in ascending number, then every other key as added. A map read out of JSON, or written back with `recipe_canon`, goes through it wherever the TS iterates the object (`parse_rate_table`, a tool call's fields, a question's answers).
15. `is_object_prototype_key` and `inherited_text` name the twelve keys `Object.prototype` gives every plain object and what `String(table[key])` reads for one the table holds no row of: `function Object() { [native code] }` for `constructor`, `function <key>() { [native code] }` for the rest and `[object Object]` for `__proto__`. The tables written as object literals in the TS (`IMAGE_TYPES`, `PROVIDER_KEY_WORDS`, `SHELL_STARTUP_FILE`, `REQUEST_WORDS`) are looked up through it.
16. `to_string_radix_16` is `n.toString(16)` for a whole number: exact hex of the double, `-` before a negative one, no padding (`hex_color` pads a channel to two).

## Edge cases
- A cut, tail, slice or pad that splits a surrogate pair drops the lone half, so the result can be one unit shorter than asked (rule 8.5; `a_cut_inside_a_surrogate_pair_drops_the_lone_half`, `pad_start_and_pad_end_fill_to_the_width_in_units`).
- NaN: `js_round` NaN, `to_fixed` "NaN", `number_string` "NaN". Infinity stays infinite.
- Empty string: every utf16 function answers as JS does on "" (`slice_follows_javascript_index_rules`).

## Behaviour nobody wrote down
- Rule 8.5 is the port's one deliberate text difference: where JS would emit a lone surrogate (`\ude00` once JSON-written), Rust emits nothing. Accepted-difference class `lone-surrogate`.

## Tapes
None directly; every tape with a cut sentence or a printed number passes through here.

## Decisions
0. This unit moves to `wsp-wire` at the merge, as `wsp_wire::text` with the same item names (agreed with the wire and views writers on #2445: views and wire code need `utf16`, `collate` and the rest, and `wsp-words` depends on `wsp-wire`); `wsp-words` then re-exports it as `js`.
1. `number_string` lives here until `wsp-wire` pushes the formatter rule 4.25 names; at the merge one re-exports the other.
2. `ryu-js` is not a dependency: the stub leaves the algorithm to the implementer, held by the tests' printed values.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 8.1 | `length_counts_a_char_above_the_bmp_as_two_units` |
| 2, 3 | `slice_follows_javascript_index_rules`, `cut_and_tail_take_the_first_and_last_units` |
| 4 | `index_of_and_last_index_of_count_units`, `pad_start_and_pad_end_fill_to_the_width_in_units` |
| 5, 8.8 | `sort_order_is_utf16_units_not_bytes` |
| 6, 8.6 | `trim_uses_javascripts_whitespace_set`, `whitespace_set_matches_ecma_262_exactly`, `collapse_turns_each_run_of_whitespace_into_one_space_and_trims_nothing` |
| 7, 8.12 | `math_round_takes_a_half_toward_positive_infinity` |
| 8, 8.13 | `to_fixed_rounds_on_the_exact_double_with_ties_away_from_zero` |
| 9, 8.11 | `number_string_writes_numbers_as_javascript_does` |
| 10, 8.14 | `en_us_with_two_digits_rounds_the_shortest_decimal_half_up`, `en_us_with_no_options_keeps_up_to_three_digits_and_drops_trailing_zeros`, `en_in_groups_the_last_three_digits_then_pairs` |
| 8.5 | `a_cut_inside_a_surrogate_pair_drops_the_lone_half` |
| 11 | `collate_orders_as_locale_compare_with_the_root_locale` |
| 12 | `serialized_numbers_are_written_as_javascript_writes_them` |

## The date and the JSON reader (added by the fix round after the review)
- `js_date` is `Date`: a time value is a whole number of milliseconds within 8.64e15 either way (`time_value`; NaN, anything past it and an infinity are Invalid Date, and a fraction is cut toward zero first, so `-0.5` is 0). `fields(t, offset)` reads a time value on a clock `offset` seconds ahead of UTC with integer arithmetic, so it holds any year a `Date` holds (to 275760); `iso_year` writes `0000` to `9999` and else a sign and six digits (`+010000`, `-000001`), `iso_string` is `toISOString`, `intl_year` is the year of its era `year: "numeric"` writes (year 0 is 1, year -1 is 2, no padding).
- `DateZone::named` takes a zone as `Intl` does: an IANA name in any case, or an offset (`+05:30`, `+0530`, `+05`); `+5:30` is no zone. `offset_seconds(t)` asks jiff, and for a moment jiff cannot hold it asks the same moment moved by whole 400-year cycles into 2000 to 2399, which has the same calendar and the same weekdays.
- Where the TS throws RangeError for an instant no `Date` holds and the Rust function has no way to say so, it answers `Invalid Date` (the usage clock words, `iso_seconds`, the kept day of a sub-thread), `NaN` (`hour_of`, `month_start`, `day_start`) or `Invalid ` with its space (`fmt_clock`, the head of `toTimeString()` of an Invalid Date). `end_plan` answers the RangeError, which has a `Result`.
- `date_parse` is `Date.parse`, a port of V8's reader (node 22): the ISO format first (a date alone is UTC, a date and a time with no zone is local), then the legacy reader (the space where the `T` goes, a zone-less date as local time, RFC 2822, a month by name, `10/25/2026`), tokens and composers as V8 has them. `tests/date_parse.rs` holds 120 texts and what node answered for each; a text whose answer depends on the zone is run only where this computer's zone is UTC.
- `v8_json` is `JSON.parse`: V8's messages (a line ends at `\n`, `\r\n` or a lone `\r`; a backslash before a character above U+00FF is an unexpected token), read off an explicit stack so a reply nested a million deep answers as node answers it. A container nested past `KEPT_DEPTH` (128) is read and checked but kept as an empty one of its kind, since no shape the host reads holds anything that deep and a `Value` that deep overflows the stack when it is dropped. A number past the double range, which V8 reads as Infinity, is held as a half with its sign by `parse_json` (zod's `.int()` calls it a float) and as null by `parse_json_lax` (what `JSON.stringify` writes for it). `parse_json_whole` is the host socket's reader: it keeps `WHOLE_DEPTH` (506) levels, the cap of 500 and the 6 a frame wraps a value in, and refuses a deeper frame as invalid json where serve.ts:637 (`JSON.parse`) reads any depth, so a frame past 506 that node answers (a deep key the schema strips) is refused here, after-port (ruling 2 of fix round 3 of #2448). The tool-call readers (`tool_input`, `shell_command_of`, `threads_followed`, `validator_refusal`) take `parse_json_lax`.
