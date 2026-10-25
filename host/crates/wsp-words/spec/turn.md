# turn
Source: `packages/protocol/src/words/turn.ts` (380 lines) at a3b870a6e. Tests: `packages/protocol/test/format.test.ts` and `thread-read.test.ts` (22 + 4 cases ported, 1 skipped: `storedTitleSource` needs `attachedFilesPrompt`, a later unit), plus `pinned` in `tests/turn.rs` (every value printed by the TS under node, `TZ=UTC`).

## What it does
What a turn's end says: the notify line a lead and a wait read, the chat footer's parts, a read's rows, the refusal a harness answered with. A thread's title from its opening turn and from a harness's answer, and the opening a row keeps.

## Behaviour
1. `notify_reply(r, len)`: text = `r.text` or ""; Tail is `last_line(text)`; Whole is `js_trim(text)`, None when that is "" (turn.ts:17-20). `notify_body`: a completed turn says the reply, else the error; any other status says the error, else the reply (turn.ts:25-28).
2. `listed_last_line(t)`: `last_line(t or "")`, then the list marker `^(?:[-*+]|\d{1,9}[.)])\s+(?=\S)` removed, then `cut_line(_, 200)` (turn.ts:50-56). `listed_failure(why)`: the first line of `why` (split on `\r?\n`) that is not blank by `js_trim`, whitespace collapsed and trimmed; NO_RESULT_LINE when none; cut to 200 (turn.ts:59-62). `subagent_asked`: `cut_line(js_trim(p), 280)` (turn.ts:65).
3. `turn_lines(r, reason)`: `lastLine` where `listed_last_line(r.text)` is Some; `failure` only when status is failed, from `r.error`, else `reason` (turn.ts:69-72). Keys in that order.
4. `notify_line(id, r, len)`: `thread <first 8 units of id> finished (<status>[, <fmt_duration(worked)>][, <fmt_cost(cost)>])[: <notify_body>]` (turn.ts:81-86).
5. `turn_worked_ms`: None without a duration; waited = min(waited if > 0 else 0, duration); worked = duration - waited (turn.ts:124-128). `turn_settled_parts`: `Worked for <worked>`; `waited on you <waited>` only when waited > worked; the cost, as `<cost>[ word] this turn` when the opened figure is > 0, else `<cost>[ word]`; then `<opened> in threads it opened` (turn.ts:134-145). `turn_facts_parts`: model label, else model, unless ""; then `<fmt_tokens(in)> in`, `<fmt_tokens(out)> out` (turn.ts:149-155).
6. `turn_settled_line` joins status and the settled parts with two spaces; `turn_end_line` adds the facts parts and `: <error>` for a turn that did not complete (turn.ts:160-214).
7. `refused_turn(r, refusal)`: said = `js_trim(r.text ?? r.error ?? "")`; sentence = said and the road joined by "; ", empty parts dropped; result `{status: failed, durationMs?, costUsd?, tokens?, model?, error? (only when the sentence is non-empty), refusal?}` in that key order; text, waitedMs, models, limit and unreached are dropped (turn.ts:221-235).
8. `fmt_clock(at, zone)`: "" for None; else HH:MM:SS of `at` plus the zone's offset, 24-hour (turn.ts:239-241). `thread_row_lines`: `who[ clock]`, then the text split on "\n"; `thread_read_text` joins rows with a blank line (turn.ts:245-253).
9. `title_line`: the first line not blank by `js_trim`, whitespace collapsed, trimmed; "" when none (turn.ts:278-281). `opening_title`: the shortest prefix of that line ending in `.`, `!` or `?` followed by whitespace or the end, else the line; `cut_line(_, 48)` (turn.ts:289-292).
10. `kept_opening`: the text when <= 600 units; else the first 600, or 599 where unit 599 is a high surrogate (turn.ts:310-314). `opening_cut`: length >= 599 (turn.ts:317).
11. `stored_title_source(stored, opening)`: title = stored collapsed and trimmed; Seed when it starts with the lost-session head; Person when opening is None; Seed when title is non-empty and (`<words> Attached files:` starts with title, or title starts with it, or opening is cut and title starts with words); else Person (turn.ts:326-333).
12. `title_prompt(opening, reply)`: the three instruction lines, "", "The opening turn:", excerpt; with a reply "", "The reply:", excerpt; joined by "\n". excerpt = `js_trim`, cut to 600 units plus "…" when longer (turn.ts:341-363).
13. `generated_title(answer)`: None for blank or any `\r`/`\n` after `js_trim`; a wrapping quote pair (`"'“‘` opening, `"'”’` closing, any mix) taken off and trimmed; trailing `.`s off and trimmed; None when empty; the title when <= 40 units, else `words_within(title, 40)` (turn.ts:373-380).

## Edge cases
- `notify_reply(blank text, Whole)` is None; `notify_body` of a completed turn with blank text and an error says the error (`notify_reply_and_body_read_null_empty_and_blank_text_as_the_ts_does`).
- A 10-digit number is no list marker (`\d{1,9}`); `-no space` keeps its dash.
- An id starting with an emoji: its 8 units hold the emoji whole.
- waited -3 reads 0; waited over the duration reads worked 0 (`worked_waited_and_spend_parts_keep_their_order`).
- `kept_opening` of 598 a's and an emoji keeps 600 units, of 599 a's keeps 599 (`titles_cut_and_kept_openings_never_split_a_pair`).
- `generated_title("\"\"")`, `"..."` and a single 52-letter word are None; `"'single'."` keeps its quotes since the stop sits outside them.

## Behaviour nobody wrote down
- Rules 8.4, 8.5: 8, 200, 280, 48, 40 and 600 count UTF-16 units; no cut leaves half a pair.
- Rule 8.6: every trim and `\s` is JavaScript's.
- `ownCopy` (turn.ts:47) exists for V8's string views and has no Rust meaning: a `String` is already its own copy.

## Tapes
`sessions.read`, `sessions.head`, the notify frames of `sessions.start` with `--notify`: every line of a thread's end.

## Decisions
1. `fmt_clock` takes the zone as a closure answering its UTC offset in seconds at the instant, since `toTimeString` reads the host's zone (rule 8.16); tests pass UTC.
2. `turn_worked_ms` and `turn_settled_parts` take `TurnClocks`, `turn_facts_parts` `TurnFacts`, both with `From<&TurnResult>`, where the TS takes a structural subset.
3. `turn_changes_line` takes `others` as a count: the TS reads only its length.

## After the port
- `notify_line` of a failed turn with no error and no text ends after the brackets, with no reason.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `notify_line::*`, `notify_reply_and_body_read_null_empty_and_blank_text_as_the_ts_does` |
| 2, 3 | `listings_cut_at_their_caps_and_take_a_list_marker_off` |
| 4 | `notify_line::*`, `the_notify_line_names_eight_units_of_the_thread` |
| 5, 6 | `a_turn_s_activity_in_one_line_each::*`, `worked_waited_and_spend_parts_keep_their_order` |
| compaction, retry, plan, changes | `compaction_retry_plan_and_changes_lines` |
| 7 | `refused_turn::*`, `a_refused_turn_keeps_only_the_fields_it_had` |
| 8 | `the_printout_a_reader_sees::*` |
| 9, 10 | `title_line::*`, `opening_title::*`, `titles_cut_and_kept_openings_never_split_a_pair` |
| 11 | `a_stored_title_is_the_seed_or_the_persons` |
| 12, 13 | `title_prompt::*`, `generated_title::*`, `the_title_question_and_its_answer` |
