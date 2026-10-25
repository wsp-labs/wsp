# turns_reopen: `threads/turns.ts:743-866`

`sweep_runs(entry)` and `reattach(row, again)`: what a restarted host does with the runs a machine still holds.

## `sweep_runs(entry)` (turns.ts:748-766)

- **P1** Nothing unless the workspace is `running`, it is not held by a stand-in (`is_held_away`), and its exec
  factory has a `sweep`.
- **P2** The sweep is handed every running row's `run`, every workspace's (the workspaces of one computer share its
  run folder), in row order.
- **P3** A sweep that fails is warned `"the runs on <ws> were left as they are: <message>"` and ends nothing more; a
  sweep that ended runs warns `swept_runs_log_line(ws, swept)`; none ended, no line.

## `reattach(row, again)` (turns.ts:774-864)

- **P4** `cannot`, with no line, where the row has no `run`, no thread, no live entry, or a workspace not `running`.
- **P5** An adapter that cannot be built: `cannot`, warned `"thread <first 8> on <ws> cannot be re-opened:
  <message>"`. An adapter with no `attach`: `cannot`, no line.
- **P6** The thread's transcript is read before the run is opened (invariant 5); a transcript that does not read:
  `unreached`, warned `"thread <first 8> on <ws> was left running: <message>"` unless `again`. The run is not
  opened.
- **P7** `attach({ run, sessionId (the view's session else its id), startedAt (the view's else now), model?, cwd?,
  prompt? and effort? (from asked), from?, steered? (the turn's steered ids, in order), onEvent })`. A failure:
  `unreached` with the same line rule. `gone`: `gone`, the row left as it is.
- **P8** Events the session emits before its turn is registered are held and handed to the turn's forward in
  order once it is, before any later event.
- **P9** Opened: the entry is marked checked (no proof needed). `run_turn` with `written = turn_written(transcript,
  turn)`, the row's notify, notifyBy, notifyRoad, token, device, turnLive and asked, the snapshot only where the row
  has one and the view a cwd, `outcome: started`, a fresh waiting box, and an opening of the asked words typed (else
  the prompt handed, else the view's prompt, else `""`) with the asked request id. A `run_turn` that fails:
  `cannot` with the line of P5. Else `attached`.

## Tests (`tests/turns_reopen.rs`)

| Rule | Test |
|---|---|
| P1-P3 | `the_sweep_names_every_running_rows_run_on_the_computer`, `a_napped_or_away_workspace_is_not_swept`, `a_failed_sweep_is_warned_and_ends_nothing` |
| P4, P5 | `a_row_with_no_run_cannot_be_reopened_silently`, `an_adapter_that_cannot_be_built_says_why` |
| P6 | `invariant_5_a_transcript_that_does_not_read_leaves_the_turn_running_and_unreached`, `again_says_nothing` |
| P7 | `the_attach_carries_what_the_row_kept`, `a_run_the_machine_no_longer_holds_is_gone`, `a_machine_that_answers_nothing_leaves_the_turn_unreached` |
| P8 | `events_before_the_row_exists_are_handed_over_in_order` |
| P9 | `invariant_1_reopens_the_run_by_its_handle_from_its_first_line`, `invariant_1_keeps_a_reply_that_landed_before_the_restart` (local-exec.test.ts, subagents.test.ts), `the_opening_is_what_was_typed_for_the_turn` |
