# Unit port: the shared adapter port's own behaviour

TS at a3b870a6e: `packages/protocol/src/adapter-port.ts:29-41, 336-376` (launch words, settles_within, end_run,
end_after_result), `packages/protocol/src/kept-run.ts` (keep_run, 175 lines) and `packages/protocol/src/lost-session.ts`
(start_after_loss, 53 lines). Rust: `src/port.rs`. Tests: `tests/port.rs` (hard cases; the TS has no unit tests of
its own for these: protocol/test/adapter-port.test.ts holds only source laws). Every adapter uses this unit, so it is
implemented first. Types and traits in port.rs are the contract for both adapter writers; their shapes are spelled in
the file and not repeated here.

## Launch words (adapter-port.ts:29-41)

- `program_word(bin, launch)`: `bin` where launch or its program is absent, else shell_quote(program).
- `agent_word(bin, launch)`: `sh -c '[ -z "${WSP_AGENT_PID_FILE:-}" ] || echo $$ > "$WSP_AGENT_PID_FILE"; unset WSP_AGENT_PID_FILE; exec "$0" "$@"' <program_word>`
  (the script shell-quoted as one word).
- `launch_words(launch)`: each arg shell-quoted; `[]` where none.
- `AdapterEvent::session_id()`: the `sessionId` of any variant.
- `system_now()`: ms since the epoch off the system clock.

## Ending a run (adapter-port.ts:336-376)

- `settles_within(turn, ms)`: true where `turn` settles (either way) before `ms` passes, false otherwise; the timer
  does not outlive the call.
- `end_run(stream, grace)`: teardown at once; where the stream has not exited within `grace`, kill; then await
  exited. Teardown is never skipped, kill happens at most once.
- `end_after_result(stream, wait, grace)`: where the stream exits within `wait`, nothing is signalled; else end_run.

## keep_run (kept-run.ts:111-175)

One reader for the process's whole life; lines go to the open turn's view, or to `between` while none is open.
- `turn()`: the first call answers `from: None`; later ones `stream.next_turn()`. A view still open is cut: its unread
  lines go to `between`, its reader ends. The new view's lines are the process's lines from now on. Where the process
  already ended, the view's lines end at once (with the stored failure) and its exited settles with the code.
- A view: run and pid of the stream; teardown and kill pass through; `write` answers Gone once the view's input
  closed, else passes through; write_after passes through where the stream has one; `signalled` only after the
  process ended; `exited` settles with 0 when the view is closed while the process lives, else with the process code.
- A view's `close_input`: once only. Released: closes the process's input. Not released: the view stops being the open
  one, its unread lines go to `between`, `stream.rest()` is called, its exited settles 0. The process keeps running.
- `release()`: the open view now takes the process with it. `up()`: not ended and not released.
- `close(wait, grace)`: released = true, process input closed, end_after_result(wait, grace).
- The process's end: every line read reaches the open view before its end is heard; a stream error is stored and the
  open view's reader ends with it.

## start_after_loss (lost-session.ts)

- The first launch runs with the resume and an on_event that swallows a `turn.done` whose `result.error` equals
  `lost` (marking it gone) and, once gone, the first launch's `session.end`; every other event passes through.
- `finished`: the first launch's result where not gone. Where gone: await the seed, launch again with no resume and
  prompt `lost_session_prompt(seed, prompt)`; every event of the second launch passes through, and after its first
  `session.start` one note delta `{sessionId: that start's id, kind: note, text: LOST_SESSION_NOTE}`. `now` answers
  the second launch from then on; `finished` is the second's result.
- A seed that fails rejects `finished` in the TS; here `finished` cannot fail, so it settles with a failed result
  carrying the seed's words (an after-port question: the TS leaves that promise rejected and the turn row open).

## Rules against tests

| Rule | Test |
|---|---|
| program word quoted only with a program; pid wrapper exact | `port::the_launch_words_are_the_ts_ones` |
| end_run kills only a run that outlives the grace | `port::end_run_kills_only_what_outlives_the_grace` |
| end_after_result leaves a run that goes on its own | `port::end_after_result_leaves_a_run_that_goes_on_its_own` |
| settles_within either way, and false on time | `port::settles_within_reads_a_rejection_as_settled` |
| kept run: lines between turns to `between`, views in turn | `port::a_kept_run_hands_each_turn_its_own_lines` |
| closing a view rests the process and answers writes Gone | `port::a_closed_view_rests_the_process_and_takes_no_write` |
| release hands the process's end to the open view | `port::a_released_view_ends_with_the_process` |
| a view opened after the end ends at once | `port::a_view_after_the_end_ends_at_once` |
| lost session: swallowed end, seeded relaunch, one note | `port::a_lost_session_starts_again_seeded_and_says_so` |
| a found session passes through untouched | `port::a_session_still_held_runs_as_one_launch` |
