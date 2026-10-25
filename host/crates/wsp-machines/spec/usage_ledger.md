# usage_ledger: the ledger of what was used, and each account's last reading

TS: `usage.ts:41-109, 162-455` (`createUsageLedger`). Rust: `src/usage.rs` (types and constants) and
`src/usage/ledger.rs` (`UsageLedger` over `Rc<dyn Store>` and `Rc<dyn Clock>`). TS tests: usage-ledger.test.ts groups
"the ledger of what was used" (13), "work read from the logs" (2), "what each account may still use" (4), "an
account's banked resets" (4 of 6). Tests: `tests/usage_ledger.rs`.

## Rules

U1. Collections (usage.ts:42-50): day documents are blobs `usage-days/<YYYY-MM-DD>`; the index is the document
`usage-index/days` `{days: [...]}`; readings `limits/<account key>`; labels `usage-accounts/<key>` `{key, label}`.
`USAGE_RETENTION_DAYS` is 90; `retention_days` overrides it.
U2. One write at a time (usage.ts:211-216): `add`, `file_logs` and `limit` each run after every write asked before them
has settled; a failed write fails its own caller only, and the next write runs.
U3. A day (usage.ts:218-223): no blob reads as none; a blob that parses but is not a `UsageDay` reads as none; a blob
that is not JSON fails the call. The index (usage.ts:224-227): its `days` strings in order; anything else reads `[]`.
U4. The fold (usage.ts:230-270). The row key is the hour (`hour_of(at, zone)`), agent, account, computer, project,
model (`""` where none) and source. A new row is `{day, hour, agent, account, computer, project, model, source, turns
(entry turns, else 1), tokens (all five, absent ones 0), costReported?}`. Folding into a row: a cost on a row with none
first copies its tokens to `unreported`; no cost on a row with one adds the tokens to `unreported` (made zero first);
then turns, tokens and the cost add. A `wsp` entry's session is noted once on the day's `sessions`, appended.
U5. A write (usage.ts:273-282) puts the day's blob, then reads the index, deletes every day's blob before
`day_key_of(now - (keep - 1) days)`, and writes the index as the remaining days sorted. A day older than that is
written and then deleted in the same write.
U6. Account labels (usage.ts:284-289): one per key, the last label given for a key; written only where the stored label
differs.
U7. `add(entry)` (usage.ts:291-298): labels the account, then folds into the entry's day (`day_key_of(at, zone)`) and
writes it.
U8. `file_logs(entries, computers)` (usage.ts:300-329): every day in the index is read; every session a `wsp` turn was
filed under is skipped; every `log` row of a computer read is removed from every day; each entry not skipped and not
older than the retention is folded as `log` into its day; every day touched is written, in the order first touched. A
computer not named keeps its rows; a named computer with no entries loses them.
U9. `used(q)` (usage.ts:331-410) waits for the writes asked before it. Today starts at `day_start_of(now, zone)`; the
range is `RANGE_DAYS` days (day 1, week 7, month 30), each day's start stepped from noon; `since` is today for a day,
else the first day's start; `until` the start of tomorrow. Rows from logs are left out when `outside` is false. The
split value is the row's field, a model with its context window (`[1m]`) cut by `base_model`; a model row's key is
`<agent>:<model>`. Each line starts `{key, label, agent? (agent, account and model splits only), tokens (zeros),
priced: true}`; the tokens add; `turns` counts `wsp` rows only (absent where none); the rate table is read once, at
the first row counted, never for an empty range; a row the table prices adds `estimate` (`price_of`) and `saved`
(`cached * (input rate - cache read rate, else 0)`); `costReported` adds; the bare tokens (all of them with no
reported cost, else `unreported`) add their list price to `costList`, or, where unpriced and over zero input plus
output, set `priced: false`.
U10. The series (usage.ts:346-353, 387-393): 24 hourly steps for a day (`today + h hours`), one per day otherwise; a
step adds input plus output; each key's `points` line the same.
U11. The answer (usage.ts:396-409): rows sorted by input plus output descending, ties by key `localeCompare`; `lines`
only for rows over zero, in row order; `logs` (`{agents, computers}`, each the labels sorted by code unit) only where
a log row was counted. Keys: `range, split, rows, series, lines, since, until, logs?`. No `threads`, no `counts`.
U12. `limit(r)` (usage.ts:412-437): the held reading (one that does not parse is none). A reading with `at` not newer
than the held `readAt` answers `{before: held, after: held}` and writes nothing, no label either. Else `plan`, `status`
and `keyed` fall back to the held ones; `windows` are the reading's where it has some or it is keyed, else the held;
credits (U13) none where keyed; the label is written; the new reading `{key, agent, label, road, plan?, windows,
status?, keyed?, readAt (r.at, else now), computers (held then this one, once), credits?}` is stored and answered with
the one it replaced.
U13. Credits held (usage.ts:184-188): a reading without credits keeps the held; a reading in full replaces them with
`readAt` and `detailAt` now; a count alone keeps the held details while the count is the same (`readAt` now), else
keeps the count alone.
U14. `limits()` lists the readings that parse, in store order; `account_labels()` maps each stored `{key, label}` of
two strings; `day(d)` and `days()` read U3.

## Restart

Everything is in the store: a new ledger over the same store answers the same rows, readings and labels
(`a_new_ledger_over_the_same_store_reads_every_day_back`). The write chain is memory.

## After the port

- U3: a day blob that is not JSON fails every `used` over that day.
- U5: a turn older than the retention is written and deleted in one write, and every `add` rewrites the index.
- U4: the stored row's key order follows the fold's literal; `day()` reads it back through `UsageDay`'s order.

## Tests

| Rule | Test |
|---|---|
| U1 | `files_a_turn_under_its_day_blob_and_lists_the_day_in_the_index` |
| U2 | `two_turns_at_once_both_land`, `a_failed_write_fails_its_caller_and_the_next_runs` |
| U3 | `a_day_that_is_not_a_usage_day_reads_as_none`, `a_day_that_is_not_json_fails_the_read`, `an_index_that_is_not_a_list_reads_empty` |
| U4 | `files_a_turn_under_its_day_and_hour_and_two_turns_of_one_key_are_one_row`, `keeps_bare_tokens_apart_only_once_the_row_holds_a_reported_cost`, `notes_a_wsp_turns_session_once` |
| U5 | `keeps_ninety_days_of_documents_the_oldest_dropped_as_a_new_day_is_filed`, `a_turn_older_than_retention_leaves_no_day` |
| U6 | `keeps_the_label_each_account_was_filed_under` |
| U7 | (U4, U5) |
| U8 | `skips_a_session_a_wsp_turn_was_filed_under_and_a_second_read_replaces_the_first`, `replaces_each_computers_rows_only_when_that_computer_was_read` |
| U9 | `reads_the_rate_table_only_for_a_range_with_a_row_once`, `folds_a_day_a_week_and_a_month_over_their_own_days`, `splits_by_account`, `files_one_model_under_each_agent_and_names_the_agent_on_three_splits`, `keeps_the_harnesss_cost_prices_the_bare_tokens_and_says_what_it_could_not_price`, `returns_the_token_mix_wsps_turns_and_the_list_price`, `counts_log_rows_with_wsps_and_says_whose_logs` |
| U10 | `draws_the_range_as_a_series`, `draws_each_split_values_line_on_the_series_steps` |
| U11 | `draws_no_line_for_a_value_that_used_nothing`, `names_no_threads_and_counts_none`, `ties_sort_by_key` |
| U12 | `keeps_an_accounts_last_reading_and_every_computer`, `answers_each_reading_with_the_one_it_replaced`, `keeps_a_reading_taken_earlier_only_over_an_older_one`, `keeps_the_plan_a_later_reading_leaves_out` |
| U13 | `keeps_a_full_readings_resets_and_a_same_count_reading_keeps_them`, `drops_details_a_count_no_longer_matches`, `leaves_resets_alone_for_a_reading_without_any`, `holds_none_for_a_keyed_account` |
| U14 | `limits_skip_a_reading_that_does_not_parse` |
| Restart | `a_new_ledger_over_the_same_store_reads_every_day_back` |
