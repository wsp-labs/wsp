# transcripts_rows: record, flush, caps, reads and held words

TS: `T/transcripts.ts:94-160, 257-282, 292-360, 382-506, 513, 546-588, 659-714` at `65f6b89c7`, the rows road only
(README R9). Rust: `src/threads/transcripts/rows.rs`, methods on `TranscriptsArea`. Shared state through
`ctx.transcript_state()`: a `StateCell<TranscriptState { index: IndexMap<ws, TranscriptIndex>, pending:
IndexMap<ws, Vec<SessionEvent>>, pending_bytes: IndexMap<ws, u64>, unread: IndexSet<ws> }>`. `index_for(ws)` answers
the held index, putting an empty one in first when none (`runtime.ts:959-963`). Rows: `ctx.rows()`, wsp-store's
`TranscriptRows`. Area state: `transcript_queue` (one chain per workspace, caller gets its own result, the stored
tail swallows; chapter 03), `flushes_queued: IndexMap<ws, Joined>`, `transcript_timers: IndexMap<ws, Cancel>`,
`warming: Cell<bool>`. Sizes are `event_bytes` (UTF-16 units of the JSON, rule 2.11). Constants: `TRANSCRIPT_CAP`
5,000, `TRANSCRIPT_BYTES` 4,194,304, `TOOL_RESULT_KEPT` 16,384, `PENDING_FLUSH_BYTES` 1,048,576,
`TRANSCRIPT_FLUSH_MS` 250, `WORDS_HELD` 2,097,152.

## Recording

T1. `record(e)` (`:677-684`), synchronous: `at` is `now`, except a `session.earlier` keeps its own `at` (now when it
has none); `pos = index_for(ws).pos + 1`. Both are set the way `{...e, at, pos}` sets them: a key the event already
has keeps its place, a new one goes after the event's own keys. Then `append`, then `ctx.emit` (the bus, which stamps
`seq` last), then, for a `session.start` or `session.end` with a thread, `ctx.push_head(thread)`.

T2. `record_copies(events)` (`:687-689`): each event, in order, gets `pos = index_for(ws).pos + 1` (its own `pos`
replaced in place) and is appended. No `at`, no emit, no push. **Invariant 2**: positions are dense per workspace and
never issued twice, since `append` folds each event into the index (moving `pos`) before the next is stamped.

T3. `append(e)` (`:690-714`): when the event's thread holds cold words on the index (`cold: true`), they are read
back first (`words_of`, T15) and set on `index_for(ws)`, before the event is pending. Then the kept form (T4) is
pushed on `pending[ws]`, its size added to `pending_bytes[ws]`, and `foldEvent(index_for(ws), kept)`. Then: a
`session.end`, or pending bytes strictly over 1 MiB, flushes now (detached); any other event but a `session.delta`
arms one 250 ms timer for the workspace when none stands (a later event does not move it); a delta alone arms none.

T4. `kept_event(e)` (`:663-671`): only a `session.delta` changes. Its text is clipped when longer than 16,384
UTF-16 units and it is a `tool_result`, or it has a `parentToolUseId` and is `text` or `thinking`. A `tool_result`
with a `patch` runs `keptPatch(patch, 16384)`. Nothing clipped and no `patchCut` answers the event as it is.
Otherwise `{...e, ...text, ...patch}`: `text` cut to its first 16,384 units (a cut inside a surrogate pair drops the
lone half, as wsp-core's ruling for `firstOf`), `bytes` added only on a clipped `tool_result` as the event's own
`bytes` else the UTF-8 length of the whole text; `patch` replaced in place, `patchCut: true` added last.

## Flushing

T5. `flush_transcript(ws)` (`:557-588`): cancels the workspace's timer; a flush already queued and not begun is the
answer (asks coalesce). Else a turn on the workspace's queue that first forgets the queued mark, then runs
`settle_rows(ws)` only when the workspace has pending events. Its failure is swallowed (no log): the pending events
stay for the next trigger. `cancel_flush(ws)` cancels and forgets the timer.

T6. `settle_rows(ws, cut)` (`:401-446`), in one `rows.atomically`: append the pending events (less those `cut`
drops); for a cut, remove the thread's kept rows it drops, add the thread to reindex when it dropped anything, and
remove from `index.facts` the session of every dropped `session.start` that is not a copy; for each thread the pending
events name (`""` for none), when `rows.total` is over a cap, run `drop_oldest_of(rows.sizes(..), size)` (T8) and
collect what goes; for what goes, mark each thread trimmed (T9), read the dropped starts back (`rows.at`) to release
their images after, remove the rows, add the threads to reindex; reindex with `reindexThreads(index, threads, t =>
rows.thread(ws, t))`, or, while the workspace is unread, rebuild the whole index from `rows.all` keeping `pos` at
least as issued and `trimmed` as it was, and clear the unread mark; then `rows.put_index(indexBytes(index, None,
false))` unless the workspace is still unread. After the commit: release the dropped starts' images
(the area's `drop_kept_keys`, only when there are keys), clear `pending` and `pending_bytes`, set the index when the
workspace still holds one, `balance_words`. A failure inside the transaction rolls it back and leaves pending as it
was.

T7. `drop_from_transcript(ws, thread, drops)` (`:385-396`): one turn of the queue running `settle_rows` with the cut
`thread == e.threadId && drops(e)`, `drops` defaulting to every event.

## Caps

T8. `drop_oldest_of(events, bytes, weigh, gone)` (`:102-143`): nothing changes when the whole list is at most 5,000
events and `bytes` at most 4 MiB. The byte bound applies only when `bytes` is over 4 MiB. Per thread (`""` for none)
count events and, when byte bound, weigh them; a thread is over when strictly more than 5,000 events or strictly more
than 4 MiB. For each thread over, in first-seen order: drop its events before its newest `session.start` oldest
first, then the oldest of that newest turn, never its newest event, stopping once it is under. A thread with no start
drops oldest first. Dropped events leave the list in place (order kept), each handed to `gone` in list order, and the
answer is `bytes` less what went. `drop_oldest(events, bytes)` (`:98-100`) weighs by `event_bytes` and defaults
`bytes` to their sum.

T9. `mark_trimmed` (`:152-154`) adds the event's thread to `index.trimmed`; an event with no thread is never marked.

## Reads

T10. `open_transcript(ws)` (`:301-333`): `rows.all(ws)` then the pending events, trimmed by `drop_oldest` against
`all.size + pending_bytes`. Deletes nothing and releases no image.

T11. `open_thread(ws, thread)` (`:336-343`): `rows.thread(ws, thread)` then that thread's pending events, trimmed
against `thread.size + <their sizes>`.

T12. `read(ws, thread, o)` (the `TranscriptReader`, `:346-360`): the thread's pending events newest first, then
`rows.newest(ws, thread, o.before)`, through `pick_newest(.., o)`; `total = rows.count + pending of the thread`; `pos`
the held index's (0 when none); `trimmed` only when the index marks the thread. A pending event at or past `before`
is skipped by `pick_newest`.

T13. `load_index(ws)` (`:257-282`): a kept index row that reads (`indexRead`) is held; one that does not read, or is
missing, is rebuilt from `rows.all` and written with `put_index`, unless the workspace has no rows, which holds no
index at all. Any failure of the rows warns `<transcriptUnreadLine(ws, why)>, so search and a send leave it out until
it reads`, holds an empty index whose `pos` is the newest row's (when that read answered), and marks the workspace
unread. **Invariant 5**: an unread workspace's flushes still append rows and skip `put_index` until a reindex
rebuilds the whole index (T6).

## Held words

T14. `balance_words()` (`:476-483`): every thread's words across every workspace ranked by `max(last, opened)`
newest first (a stable sort: ties keep index then words order). Walking them, `held` sums the shares: a thread whose
turn runs and whose words are not cold takes its whole `size`; any other takes `min(size, WORDS_HELD - held)` (never
below 0). A thread holding more than its share (`heldOf`) is cut to `newestOf(w, share)`. Then warming is asked for.

T15. Warming (`:484-505`): at most one pending `clock.schedule(.., 0)`; at it, unless the host closed, the first cold
thread whose `room` (0 when absent) is under its share is read back (`words_of`, cut to its share) and warming is
asked for again; with none, warming stops. `words_of(ws, thread, had)` reads `rows.word_events(ws, thread)` then every
pending event of the workspace, through `threadWordsOf(.., thread)`, with `last = max(read last, had.last)` and
`had.opened` kept.

T16. `opened_words(ws, thread)` (`:507-512`): nothing when the index holds no words for it; else `opened = now`, and
a cold thread balances.

## Tests (`tests/transcripts_rows.rs`)

| Rule | Test |
|---|---|
| T1 | `record_stamps_at_and_pos_after_the_events_keys_then_emits_and_pushes_the_head`, `record_keeps_an_earlier_rows_own_time`, `record_keeps_the_place_of_a_pos_the_event_carried` |
| T2 | `invariant_2_pos_is_never_issued_twice_across_record_and_copies`, `record_copies_emits_nothing` |
| T3 | `append_flushes_at_an_end_and_past_a_mebibyte`, `append_arms_one_timer_per_workspace_for_a_non_delta`, `a_delta_alone_arms_no_timer`, `append_warms_cold_words_before_the_event_is_pending` |
| T4 | `kept_event_clips_a_long_tool_result_with_its_bytes`, `kept_event_clips_a_subagents_text_without_bytes`, `kept_event_cuts_a_patch_and_marks_it_last`, `kept_event_leaves_a_short_delta_alone` |
| T5 | `flush_coalesces_a_burst_into_one_write`, `a_failed_flush_keeps_its_pending_events` |
| T6 | `settle_rows_appends_trims_reindexes_and_writes_the_index_in_one_transaction`, `settle_rows_releases_a_dropped_starts_images_after_the_commit` |
| T7 | `drop_from_transcript_takes_one_threads_events_and_its_sessions_facts` |
| T8, T9 | `drop_oldest_keeps_a_transcript_under_both_caps_whole`, `drop_oldest_drops_outside_the_newest_turn_first`, `drop_oldest_never_drops_a_threads_newest_event`, `drop_oldest_caps_each_thread_alone`, `drop_oldest_applies_bytes_only_when_the_whole_is_over`, `an_event_with_no_thread_is_thread_empty_and_never_marked` |
| T10, T11 | `open_transcript_trims_in_memory_and_deletes_no_row`, `open_thread_reads_one_threads_rows_and_pending` |
| T12 | `read_pages_pending_then_rows_newest_first_with_total_and_pos` |
| T13 | `load_index_holds_a_kept_index_and_rebuilds_one_that_does_not_read`, `invariant_5_an_unread_index_keeps_writing_rows_and_skips_the_index_until_reindexed` |
| T14, T15 | `balance_words_holds_the_newest_threads_inside_the_budget`, `a_running_thread_holds_all_its_words`, `warming_reads_back_one_thread_a_tick` |
| T16 | `opened_words_ranks_a_thread_as_just_spoken` |
