# sessions_reads: `threads/sessions.ts:901-975, 1175-1187`

The reads of threads: `list`, `history`, `thread_history`, `page`, `head`, `attachment`, `search`. Each waits for
`ready` first and reads as absent what the caller may not reach.

## Rules

- **Q1** `list(ws?)`: a named workspace no thread of the caller's tree stands on is refused as any verb naming it
  (`refuse_named`). Rows the caller reaches for listing, in row order, of that workspace where named.
- **Q2** Titles: rows on workspaces still booting are not asked. A row with no `harnessTitle` blocks the listing on
  its title read; one with a title has its read go out unawaited.
- **Q3** Threads whose rows all fell off the cap are listed off their records (`tree_records(origin, list, ws)`),
  skipping any thread a row names; each one's end is the record's, else the transcript's result's status. Rows
  first (`listed_rows`, a row the caller does not reach in full `named_only`), then the trimmed rows; the whole
  through `placed_rows`.
- **Q4** `history(ws)`: a workspace no thread of the caller's tree stands on is read through `entry_of(..,
  being_made)` (its refusal is the caller's); the transcript's events of threads the caller drives, copies.
- **Q5** `thread_history`: needs `threadId` and no `before`/`limit`, else `usage_refusal("whole reads one thread's
  every event.", "Name the thread with threadId, and no before or limit.")`, refused before `ready`. A thread the
  caller does not drive: `[]`. Otherwise the thread's words are marked opened and every event answered.
- **Q6** `page`: a thread the caller does not drive answers `{ events: [], pos: index pos (0 with none), total: 0 }`.
  Otherwise opened words, and `transcript_reader.read(ws, thread, { before?, limit: asked else
  HISTORY_PAGE_EVENTS, bytes: HISTORY_PAGE_BYTES })`.
- **Q7** `head(thread, after_read)`: facts read after the read where asked, else now. No facts, or a row the caller
  cannot reach (being made counts): `not_found_refusal("no thread <word>")`. The events take `HEAD_BYTES` less the
  facts' JSON byte length (UTF-8) less 100, `limit` unbounded, strict, head shape.
- **Q8** `attachment(ws, thread, request, index)`: after the workspace's kept writes. A key that `attachment_key`
  does not make, a thread the caller does not drive, no kept image of that key, or no blob: `not_found_refusal("no
  image <index + 1> kept on that message")`. Answers `{ mediaType, bytes: base64 }`.
- **Q9** `search(query)`: the query trimmed and lowercased; empty answers `{ hits: [] }`. Workspaces the caller may
  read (a tree stands there, or `entry_of(.., now)` succeeds); hits newest first by `last`; `unsearched` only where
  above 0.

## Tests (`tests/sessions_reads.rs`)

| Rule | Test |
|---|---|
| Q1, Q3 | `a_listing_leaves_out_rows_the_caller_cannot_reach`, `a_thread_whose_rows_fell_off_is_listed_off_its_record` (thread-cap.test.ts) |
| Q2 | `a_row_with_no_title_waits_for_its_read_and_a_titled_one_does_not` (session-title.test.ts) |
| Q4 | `history_holds_only_threads_the_caller_drives` (runtime-history.test.ts) |
| Q5 | `whole_history_refuses_a_window` , `whole_history_of_a_thread_not_driven_is_empty` |
| Q6 | `a_page_of_a_thread_not_driven_is_empty_at_the_index_pos`, `a_page_reads_two_hundred_events_and_four_hundred_kib` |
| Q7 | `head_leaves_the_events_what_the_facts_leave` (thread-head.test.ts), `head_of_a_thread_not_reached_is_not_found` |
| Q8 | `an_attachment_answers_its_bytes_in_base64` (session-images.test.ts), `an_attachment_of_another_thread_is_not_found` |
| Q9 | `search_trims_and_lowercases_and_answers_newest_first` (session-search.test.ts), `a_blank_search_finds_nothing`, `unsearched_is_said_only_above_zero` |
