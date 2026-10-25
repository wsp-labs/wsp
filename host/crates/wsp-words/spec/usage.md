# usage
Source: the functions of `packages/protocol/src/usage.ts` (464 lines) at a3b870a6e; its zod shapes (`LimitWindow`, `AccountRow`, `UsedRow`, `UsedAnswer`, ...) and flat constants (`USAGE_WORDS`, `RANGE_DAYS`, `PRICES_URL`, `LIMIT_RESUME_PROMPT`, ...) are `wsp_wire::types::usage`. Tests: `packages/protocol/test/usage.test.ts` (17 cases ported; 1 ignored as constants alone), plus `pinned` in `tests/usage.rs` (every value printed by the TS under node).

## What it does
What a plan's limits and a range's use read as: who an account is, the sentences a reset ends in, the list price of a turn's tokens off LiteLLM's price file, the day, hour and clock words a row carries in a named zone, and the words of a turn a usage limit stopped.

## Behaviour
1. `limit_kind_of_minutes(m)`: session up to 300, week up to 10 080, else month (usage.ts:49-53).
2. `reset_who`: the label alone for the person's own account, else "<label> on <computer>" (:137). `reset_words::*`, `reset_question` (a reset, the one reset, one of the N resets), and the reset lines (:140-167) are fixed sentences.
3. `used_ask(range, split)`: `{ range, split, outside: true }` (:179). `fresh_in`: `max(0, input - cached - (cacheWrite or 0))` (:268). `list_words`: "", "a", "a and b", "a, b and c" (:271). `logs_line` (:274). `used_headline`: "No use" words for zero tokens, else tokens, the price (with "at list price" where any came off the table), the cached part, and the unpriced sentence (:279-290).
4. `account_words`, in order: keyed, a non-empty plan (first UTF-16 unit upper-cased, behind the brand where one is named), the address it signed in as, your sign-in, the computer it is signed in on, the agent alone (:293-302).
5. `parse_rate_table(document)`: {} for anything not an object; per entry, skip `sample_spec` and entries without a finite non-negative input and output price; keep cacheRead, cacheWrite and provider where the file names them, keys in that order (:322-339).
6. `base_model`: one trailing `[...]` off (:342). `rate_of`: the model as named, else the part after the last "/" less a context suffix, else that less an 8-digit date (:346-349). `price_of`: None without a rate; fresh input, cached at cacheRead (else input), written at cacheWrite (else input), output at output (:353-360).
7. `day_key_of`, `hour_of` (`Number(hour) % 24`), `resets_word` ("reset" at or past the time, minutes under an hour with at least 1, whole hours under a day, else the weekday), `credits_word`, `clock_word` (HH:MM on `now`'s day, else "Ddd HH:MM"), `as_of_word` (:363-408), each in the zone named, else the host's.
8. `used_price` (:413-417), `window_cell` (`js_round(usedPercent)%`, then the resets word) (:420-423), `account_state` (:426-428), `limit_words::*` (:436-455), `held_until`: the latest reset among windows at 100% or more that reset after now (:458-462).

## Edge cases
- `base_model("x[a][b]")` is "x[a]"; `base_model("a[1m]b")` is unchanged; `base_model("[1m]")` is "".
- `rate_of("claude-sonnet-4-5-2025", ..)` is None: only an 8-digit date comes off.
- `resets_word(at, now)` with at 5 ms before now is "reset"; 30 s ahead is "resets in 1 min" (pinned).
- `held_until` skips a window at 100% with no reset time, and one whose reset is now.

## Behaviour nobody wrote down
- Rules 8.14 and 8.16: day keys, hours, weekdays and clock times come from `Intl.DateTimeFormat` in fixed forms (en-CA, en-GB h23, en-US short weekday) in the zone named, else the host's; the Rust writes each form by hand and reads the zone with jiff.
- Rule 8.12: `window_cell` rounds with `js_round`; `resets_word` minutes with `js_round`, hours and days with `floor`.

## Tapes
`usage.accounts`, `usage.used`, `usage.reset` answers; the Usage page and `wsp usage` rows; the limit strip.

## Decisions
1. RESET_WORDS and LIMIT_WORDS are modules: fixed words are constants, sentences functions.
2. The TS object parameters become arguments or structs: `reset_who(label, own, computer)`, `fresh_in(input, cached, cache_write)`, `credits_word(count, next_expires_at, now)`, `AccountWho`, `PricedTokens`; answers are `UsedAsk` and `UsedPrice`; a RateTable is an `IndexMap<String, Rate>` in the file's order; `parse_rate_table` takes a `serde_json::Value`.
3. A `timeZone?: string` is `Option<&str>`, an IANA name.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `pinned::lists_logs_and_the_used_ask` |
| 2 | `pinned::an_account_and_its_reset_sentences`, `pinned::the_reset_and_limit_sentences` |
| 3 | `what_a_range_used_said_once::*`, `pinned::lists_logs_and_the_used_ask` |
| 4 | `an_account_in_plain_words::*`, `pinned::an_account_and_its_reset_sentences` |
| 5 | `the_rate_table::*` |
| 6 | `a_turns_list_price::*`, `pinned::a_model_is_priced_off_its_base_name` |
| 7 | `the_day_a_row_is_filed_under::*`, `a_limits_windows_and_words::*`, `pinned::clock_words_read_the_zone_named` |
| 8 | `a_used_rows_price::*`, `pinned::clock_words_read_the_zone_named`, `pinned::the_reset_and_limit_sentences` |
