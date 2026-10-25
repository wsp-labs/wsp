# internal-index: the transcript index, search and what a turn wrote

TS: `packages/runtime/src/types/internal.ts:229-554` (about 330 lines) at `65f6b89c7`. Rust: `src/types/internal.rs`.
Tests: `tests/internal_index.rs` (27), against `tests/fixtures/core_types/internal.json`. Runtime invariants 2 and 5
of `packages/runtime/AGENTS.md` live here.

## Text is counted in UTF-16 units

Every length, size, room, cut and offset here is JavaScript's: UTF-16 code units (`str.encode_utf16().count()`), not
bytes and not chars. An emoji is 2. `ThreadWords.size`, `room`, `held_of`, `newest_of`, `first_of`'s 200 and
`snippet_around`'s `at`, `length`, 40 and 80 all count units. Pinned by `held_of_counts_utf16_units`,
`newest_of_keeps_the_newest_lines_that_fit` and the emoji snippets.

**The one forced divergence.** Where a cut falls inside a surrogate pair the TS keeps the lone half (the oracle wrote
`"\ud83d"`); a Rust `String` cannot hold it. The port drops the half: `first_of` cuts at the last whole character at
or before 200 units, and `snippet_around` starts at the first whole character at or after its start and ends at the
last whole one before its end. Pinned by `the_first_line_drops_a_surrogate_pair_the_cut_splits` and
`snippet_around_drops_a_pair_its_window_splits`. Filed as an after-port ticket for the TS.

## The index (internal.ts:261-299)

`TranscriptIndex`: `words`, `starts`, `cut`, `facts`, `taken`, `children` (thread to task to `Child`), `trimmed`,
`pos`. Every map iterates in insertion order and a removal is `shift_remove` (chapter 03, rule 3.11): `reindex_threads`
and `forget_child` depend on it. `empty_index()` is all empty and `pos` 0.

## fold_event (internal.ts:376-403), in this order

1. `pos`: where the event has one greater than the index's, it becomes the index's. Never moves back
   (`the_index_position_never_moves_back`).
2. A copied row (`copied_from_of(e)` is Some): only its subagent row (`fold_child`), its end's `end_children`, and its
   words (where it has a thread). No start, no request, no cut, no facts. Then stop.
3. A `session.start` or `session.steer` with `requestId`, `threadId` and `turnId` all present: `taken[requestId] =
   {sessionId, threadId, turnId, outcome}` (started or steered); a later one under the same id replaces it in place.
4. A `session.end` with `promptless: true`: every `taken` entry of that turn whose outcome is started goes; a steered
   one stays (`a_promptless_end_frees_the_requests_its_turn_started`).
5. Then exactly one of: a `session.start`: `starts[threadId] = sessionId` where it has a thread; `facts[sessionId]`
   gains each of cwd, permissionMode, model, effort the start names (made `{}` where none, kept otherwise). A
   `session.end` with a thread and not `unstarted: true`: `cut[threadId] = exitCode is null && !sawResult`, then
   `end_children(thread, turnId, at)`. A `session.subagent`: `fold_child`.
6. Where the event has a thread: `fold_words`.

Pinned whole by `a_chat_folds_into_the_index_the_ts_writes` (17 events: replies in pieces, a subagent's text, a tool
row, a steer, a cut end, earlier rows, a copied pair, a row with no thread, an unstarted end).

## fold_words (internal.ts:407-433)

The thread's words, made `{lines: [], open: None, last: 0, size: 0}` on first use. `last = max(last, at or 0)` for
every event, words or not. A cold thread takes nothing more. Then:

- a `session.start` with a prompt, or any `session.steer`: push the prompt; `open = None`.
- a `session.earlier` whose `who` is person or agent: push its text; `open = None`. Tool rows are not words.
- a `session.delta` of kind text with no `parentToolUseId`: the message key is `<turnId or sessionId>:<messageId or "">`.
  Equal to `open`: append the text to the last line and add its length to size. Else push it. Then `open` = the key.
- nothing else touches the lines or `open`; a tool delta between two pieces of one message does not break it.

Push adds the line, adds its length to `size`, and sets `first = first_of(line)` only where `first` is unset.
`open` is written in the index's JSON while it names a message.

## Children (internal.ts:305-371)

`fold_child`: no thread, nothing. A `running` row sets the child: `id = task`, `title` = the row's, else the held one's,
else the task; `state` running; each of parentToolUseId, depth, model, asked, usage the row's, else the held one's,
present only where one is; `startedAt` the held one's, else the row's `at`, else 0; `turnId` and `startRow` (= `at`)
where the row has them. Any other state moves a held child only: state, `endedAt` = `at` or its `startedAt`, done sets
`lastLine` = `listed_last_line(summary)` where that gives one, failed with a summary sets `failure =
listed_failure(summary)`, and a row's usage replaces the held one. An end with no held child is dropped.

`end_children(thread, turn, at)`: every running child whose `turnId` equals `turn` (None equals None) becomes stopped,
`endedAt = at or startedAt`, and its usage loses `durationMs`. `forget_child(e)`: only a running subagent row with a
thread, and only where it is the held child's newest start row (`startRow == at`); the thread's map goes when it
empties. Pinned by `subagents_fold_into_each_threads_children`, `forget_child_takes_a_child_only_by_its_newest_start_row`,
`end_children_stops_only_that_turns_running_children`.

## Words held and let go (internal.ts:436-452)

`thread_words_of(events, thread)` folds that thread's events alone (empty words for none). `cold_words(w)`: no lines,
no open, `last`, `size`, `first` and `opened` kept, `cold: true`. `newest_of(w, room)`: warm words with `size <= room`
come back as they are; otherwise the newest lines whose lengths sum to at most `room` (stopping at the first that does
not fit, newest first), with `cold: true` and `room` set, the rest of `cold_words` kept. `held_of`: `size` for warm
words, the kept lines' lengths for cold ones.

## Bytes and reading back (internal.ts:510-526)

`index_bytes(index, of, lines)` is the JSON object `of` (left out where None), `words`, `starts`, `cut`, `facts`,
`taken`, `children`, `trimmed`, `pos`, each map an array of `[key, value]` pairs in map order, `children` an array of
`[thread, [[task, child], ...]]`. With `lines` false each thread's words are `cold_words`; the index itself is not
changed. `index_read` answers None where the bytes do not parse as that shape or lack `taken`, `children` or `pos`.
Words without `size` gain the sum of their line lengths, and without `first` gain `first_of` of their first line where
they have one. A missing `trimmed` is empty. Tests compare the JSON as values (key order inside a value is not the
point here: the index file is this host's own and a Rust host never reads a TS one), arrays by order.

**Invariant 5** (its index half): an index that does not read is None, never a half index, so boot makes it again from
its transcript: `invariant_5_an_index_that_does_not_read_is_none`. Also `index_read_gives_back_the_index_written`,
`index_read_fills_the_size_and_first_line_of_old_words`, `index_read_takes_a_missing_trimmed_list_as_empty`,
`index_bytes_without_lines_writes_every_thread_cold`, `index_bytes_leaves_out_an_absent_mark`.

## reindex_threads (internal.ts:462-466)

For the threads named: drop their `words`, `starts`, `cut` and `children` entries, drop every `taken` entry of theirs,
then fold each thread's events again, so their entries come back at the end of each map. `facts` and `pos` stay.
Pinned by `reindex_threads_refolds_only_the_threads_named`.

## Search (internal.ts:528-548)

`search_index(ws, index, words, drives, found)`: `words` is already lowercased. For each thread in map order that
`drives` lets through: a cold thread with no lines and `size > 0` counts as unsearched; then the first line whose
lowercase holds `words` gives one hit `{workspaceId, threadId, snippet}` with the thread's `last`. Answers the
unsearched count. Lowercasing is JavaScript's `toLowerCase`, and the offset of the hit is read in the lowercased line
and used on the original. `snippet_around(text, at, length)`: `from = max(0, at - 40)`, `to = min(len, at + length +
80)`, the slice with every run of whitespace as one space and trimmed, `…` before where `from > 0` and after where `to <
len`. Pinned by `search_finds_one_hit_a_thread_and_counts_cold_threads` and `snippet_around_matches_the_ts`.

## turn_written: invariant 2 (internal.ts:469-503)

Each count is read off the newest row of that turn and kind, by its `line` stamp, never by how many rows survive:
`lines` the newest `session.delta`'s line, `subagents` the newest `session.subagent`'s, `compactions` the newest
`session.compacted`'s, `retries` the newest `session.retry` **that has a line** (a retry the host saw itself has none),
each 0 where absent. `reply` is the newest `session.done`'s status. `started` is true where any count is above 0, a
reply is present or the turn has a `session.start`. `changes` is true where the turn has a `session.changes`.

The TS reads the newest delta even when it has no line, so a turn whose newest delta is unlined reads `lines: 0`
(fixture `u1`). Pinned as it is by `invariant_2_turn_written_reads_the_newest_stamp_of_each_kind` and filed as an
after-port ticket. Also `turn_written_counts_by_stamp_after_the_head_was_evicted`,
`turn_written_reads_reply_card_and_start`.

## Rows (internal.ts:336-348)

`named_only(view)`: the view without `lastLine` and `failure`, its subagents without `asked`, `lastLine` and
`failure`; a view with no subagents gets none. `write_lines(view, last_line, failure)`: each set where Some and
removed where None. Pinned by `named_only_leaves_out_what_turns_said`, `write_lines_sets_and_clears_each_line`.

## Caps

`TRANSCRIPT_CAP` 5000 events and `TRANSCRIPT_BYTES` 4 MiB per thread, `TOOL_RESULT_KEPT` 16 384 units,
`TRANSCRIPTS_HELD` 4, `PENDING_FLUSH_BYTES` 1 MiB, `WORDS_HELD` 2 MiB of units, `SESSION_INDEX_CAP` 200 rows,
`TRANSCRIPT_FLUSH_MS` 250, `MAX_TIMER_MS` 2^31 - 1. The transcripts area enforces them; they are pinned here by the
`constants_*` tests in `tests/internal_records.rs`.
