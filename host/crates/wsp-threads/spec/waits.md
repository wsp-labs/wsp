# waits: `threads/waits.ts` (108 lines)

`WaitsArea::new(ctx)` answers `note_waited`, `note_retry` and `agent_retried` (waits.ts:102-191). State, all memory:
`settling` (thread to the longest hold of a wait still settling), `said` (the last sentence each turn's row wrote),
`unsaid` (the retries the host saw that the agent has not said, by turn, oldest first). Nothing survives a restart.

## Constants

`WAIT_SETTLE_MS = 2_000` (waits.ts:94), `RETRY_SETTLE_MS = 2_000` (waits.ts:98), wsp-wire's `WAITED_ROW_MS = 5_000`.

## The row a write lands on (waits.ts:129-152)

- **W1** `latest_turn(thread)`: of the rows in `ctx.sessions()` whose `view.threadId` is the thread, the one with the
  greatest `startedAt`, a missing `startedAt` read as 0; on a tie the later row in map order wins (`>=`).
- **W2** No such row, or `ctx.closing()`: nothing is written.
- **W3** The row written is `thread.waited { workspaceId, sessionId, turnId, threadId, ms, said? }` through
  `ctx.record`, where `sessionId` is the row's `claudeSessionId`, else its turn id, and `ms` is
  `max(0, round(ms))` (JS `Math.round`: half rounds up; a negative reads 0). Key order as listed.
- **W4** A sentence (`said`) already written by the same turn is not written again; another sentence is, and becomes
  the turn's last. Keyed by the turn id (the TS keys the row object; see Unclear).

## `note_waited(thread, ms, text)` (waits.ts:154-172)

- **W5** With `text`: written at once, ms as given, the one leading `"wsp: "` cut (only at the start, only once). No
  threshold, no settling: a 0 ms sentence is a row.
- **W6** Without text: a hold of `WAITED_ROW_MS` or less writes nothing (`!(ms > 5000)`, so exactly 5000 and NaN
  write nothing) and arms nothing.
- **W7** The first hold over the threshold arms one `WAIT_SETTLE_MS` timer for the thread; each later hold before it
  fires raises the wait to the longest seen and arms nothing. At the fire the thread's wait is forgotten and one row
  is written with the longest ms, on the row latest at the fire (W1). A hold after the fire starts a new wait.
- **W8** `note_waited` never fails; it answers once the note is taken.

## `note_retry(thread, { attempt, why })` (waits.ts:175-189)

- **W9** Taken only where the thread's latest row is `running` and the host is not closing; otherwise nothing,
  and no timer.
- **W10** The note is appended to that turn's `unsaid` list and one `RETRY_SETTLE_MS` timer armed per note. At the
  fire, the note is written only if it is still in the list (the agent did not say it), the same row still reads
  `running`, and the host is not closing; it is then taken out of the list and written as
  `session.retry { workspaceId, sessionId, turnId, threadId, attempt, why }` with no `line` (the agent's own retries
  carry one, turns_forward F14). A turn whose list empties is forgotten.

## `agent_retried(turn, why)` (waits.ts:118-127)

- **W11** The agent said a retry: of that turn's unsaid notes the newest whose `why` equals the agent's is taken out;
  where none matches (or the agent named no why) the newest note is. A turn with no notes: nothing.

## Tests (`tests/waits.rs`)

| Rule | Test |
|---|---|
| W6, W7 | `one_row_for_a_14_minute_hold_and_none_for_2_seconds` (thread-waits.test.ts:39) |
| W7 | `one_row_of_the_longest_for_holds_together_and_a_new_row_for_a_later_wait` (thread-waits.test.ts:55) |
| W4, W5 | `a_sentence_is_written_once_per_turn_however_short_the_hold` (thread-waits.test.ts:70) |
| W2 | `nothing_for_a_thread_no_row_holds` (thread-waits.test.ts:78), `nothing_while_the_host_closes` |
| W6 | `exactly_the_threshold_writes_nothing` |
| W1 | `the_row_started_latest_takes_the_write_and_a_tie_goes_to_the_later_row` |
| W3 | `ms_is_rounded_and_never_negative`, `the_session_id_is_the_agents_else_the_turns` |
| W5 | `only_the_leading_wsp_prefix_is_cut` |
| W9 | `a_retry_on_a_thread_not_running_is_dropped` |
| W10, W11 | `a_retry_the_agent_says_in_the_window_is_no_row_and_one_it_never_says_is_written` (subagents.test.ts:341) |
| W10 | `a_retry_whose_turn_ended_before_the_fire_writes_nothing` |
| W11 | `the_agents_retry_takes_the_newest_note_of_its_reason_else_the_newest` |

## Unclear in the TS

- `said` is a `WeakMap` keyed by the row object. The start road's held row and `run_turn`'s row are two objects for
  one turn (sessions.ts:520 against turns.ts:650), so in the TS a sentence said before the handover is said again
  after it. The port keys by turn id, which says it once. No TS test meets the difference.
