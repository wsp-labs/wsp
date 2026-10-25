# sessions_acts: `threads/sessions.ts:992-1110, 1189-1262`

What a caller does to a thread's turn short of a stop: `steer`, `answer`, `access`, `rename`, `read`, `aside`, `run`.

## Shared

- **C0** Each waits for `ready`. A row the caller cannot reach answers as no row (`not-found`, or the refusal named
  per act). A workspace whose state refuses sends throws `send_refusal(state, gone, name)` (rename:
  `action_refusal(state, "rename", gone, name)`), no kind.

## `steer(row, { prompt, attachments })` (992-1010)

- **C1** No row: `not-found`. Not running or no handle: `not-running`. No steer road: `unsupported`.
- **C2** Files: `files_refusal`, else `steer_files_blocked(records, steers_images, harness)`; a refusal throws.
- **C3** A fresh steer id; the images inline. An outcome other than `accepted` is answered as it came, nothing kept.
  Accepted: `record_steer(row, id asked, ask, origin, steer id)`, `accepted`.

## `answer(row, { askId, optionId, reason })` (1012-1025, trap #1776)

- **C4** The row keyed by the id where it is running; else the running row whose agent session is that id; else the
  keyed row. None: `not-found`.
- **C5** No handle: `gone`; a handle with no answer road: `unsupported`; else the handle's answer.

## `access(id, mode)` (1027-1073)

- **C6** The row of that id, else the last row of that thread; else the thread's record. None: `not-found`.
- **C7** The mode checked with `start_picks(catalog, { permissionMode }, false)` where the harness has a table; its
  refusal is the caller's.
- **C8** No row: the record takes the mode, persisted, head pushed: `set`.
- **C9** The record takes the mode; the thread's latest row takes it where that row is not running; persisted and
  the head pushed. A latest row not running: `set`. Running: no running process here `not-running` (the TS `gone`
  maps there); a handle with no access road `unsupported`; the handle's `set` → `set`, `refused` →
  `unsupported`, anything else `not-running`.

## `rename(row, title)` (1075-1106)

- **C10** Trimmed blank: `EMPTY_TITLE_LINE`. No row: `not-found`. After `copy_blocked`. No rename road:
  `unsupported`. No agent session on the row: `no-session`.
- **C11** The write runs through `write_session(session, ...)` with a harness exec bounded by
  `SESSION_TITLE_TIMEOUT_MS`. `failed` answers `failed` with its error; `no-session` likewise.
- **C12** Renamed: every row of that workspace and session takes the title with `titleSource: person`; persisted,
  head pushed, `renamed`.

## `read(thread)` (1108-1110)

- **C13** `mark([thread], { readAt: now }, origin)`.

## `aside(row, question, ask_id)` (1189-1230)

- **C14** Blank question: `BLANK_ASIDE_LINE`. No row or not reached: `not_found_refusal("no session <id>")`. The
  send refusal (C0, without the name).
- **C15** The thread's latest row runs it. No aside road: `aside_unsupported_line(harness)`; no session:
  `ASIDE_NO_SESSION_LINE`. Text streamed as `aside.text { workspaceId, threadId, askId, text }` passed on the bus,
  only with an ask id. The ask carries the session, the question, the cut owed, and the latest row's cwd, model,
  effort, context window and fast (true only).
- **C16** An empty answer: `ASIDE_EMPTY_LINE`.
- **C17** A harness that loads no servers into an aside, on a workspace that is not brokered: asked bare. Else a
  launch is minted (`thread_launch(.., { aside: true })`), the wsp server served where the harness takes it, server
  values read where the aside loads them and the workspace is not brokered; the scoped device is revoked after,
  whichever way (warned `"the token of a side question on thread <word> was not taken away: <message>"`).

## `run(step)` (1232-1262)

- **C18** A caller with a thread scope: `RUN_PERSONS_LINE`. A thread with no row and no record, or not reached:
  `not_found_refusal("no thread <word>")`.
- **C19** An ending of that run already in the transcript (state not `running`) is answered as it stands, nothing
  recorded.
- **C20** Else `session.run { workspaceId, sessionId (latest session, else latest id, else the step's turn), turnId,
  threadId, runId, block, command, state, ptyId?, exitCode?, signal?, output? (run_output_tail) }` recorded and
  answered with `at: now`.

## Tests (`tests/sessions_acts.rs`)

| Rule | Test |
|---|---|
| C1-C3 | `a_steer_is_kept_only_when_accepted` (runtime-steer.test.ts), `steer_refuses_files_the_harness_cannot_take` (steer-images.test.ts), `steer_on_a_row_not_running_is_not_running` |
| C4, C5 | `answer_finds_the_running_row_by_the_agents_session_id` (permission-relay.test.ts), `answer_without_a_handle_is_gone` |
| C6-C9 | `access_on_a_trimmed_thread_moves_its_record` , `access_while_running_lands_on_the_record_and_the_row_follows` (thread-access.test.ts), `access_a_cli_refuses_is_unsupported` |
| C10-C12 | `rename_names_every_row_of_the_session` (session-title.test.ts), `rename_with_no_session_is_no_session`, `a_blank_rename_is_refused` |
| C13 | `read_marks_the_thread_read_now` (thread-marks.test.ts) |
| C14-C17 | `aside_asks_the_latest_session_with_what_it_ran_at` (aside.test.ts), `aside_streams_text_only_with_an_ask_id`, `an_empty_aside_answer_is_refused`, `an_aside_with_servers_revokes_its_token_after` |
| C18-C20 | `run_is_the_persons_alone`, `an_ended_run_is_answered_from_the_transcript` (session-run.test.ts), `a_run_step_is_recorded_with_its_tail` |
