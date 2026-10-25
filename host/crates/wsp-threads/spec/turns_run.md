# turns_run: `threads/turns.ts:17-290, 569-741`

`TurnsArea::run_turn(TurnStart)` opens one turn on a harness session and answers its `SessionHandle`. This unit is
the turn's state, its row, its handle, its ends (`end`, `settled`) and the prompt plumbing; the switch over adapter
events is turns_forward, which reads and writes the same `TurnState`.

## What `run_turn` is handed (turns.ts:19-67)

`TurnStart { entry, view: SharedView, written?, thread_id, turn_id, notify?, notify_by?, notify_road?, turn_token?,
scope_device_id?, outcome, opening, asked?: Rc<RefCell<TurnAsked>>, resume?, turn_live?, images_dir?, snapshot?,
reports_edits, waiting?: Rc<Cell<bool>>, keep?, open }`. `open(forward)` launches and answers the harness session
synchronously (it may call `forward` before it returns).

## State (turns.ts:69-170)

- **N1** `written` absent reads `{ lines: 0, subagents: 0, compactions: 0, retries: 0, started: false, changes:
  false, reply: None }`. Each counter starts at its written value and so does its replay count.
- **N2** `turn_live` is the row's (`t.turn_live`), else a fresh one; the same cell rides the row (N13) so the row and
  this turn read one reply.
- **N3** `write_held` is added to `ctx.held_at_close()` at the start and taken off at `turn_over`. It records
  `session.context { workspaceId, sessionId, turnId, threadId, context, window? }` (sessionId: the view's
  `claudeSessionId`, else its id) only where a call reported what the agent held and no result named it.

## Asks (turns.ts:173-283)

- **N4** `reads_open`: the lead ask of the open ones (`lead_ask`) sets `view.asking = asking_line(lead)` and
  `ctx.lead_asks()[thread] = lead`; none clears both. The waiting box reads `open > 0 || tasks running`. The first
  open starts `waiting_since = now` (kept if already set); the last close adds `now - waiting_since` to `waited` and
  clears it. `turn_live.waited` is always written; `turn_live.waiting_since` is written or taken off. Then
  `persist_sessions(workspace)`.
- **N5** An ask's options as clients read them: an option of effect `mode` with a mode is relabelled
  `permission_mode_option_label(catalog label of that mode, else the mode)`; others are copied.
- **N6** `close_ask(ask, outcome, option?)` does nothing for an ask not open; else closes it (N4) and records
  `session.permission.closed { ..., askId, outcome, optionId? }`. Ending a turn closes every open ask `cancelled`.
- **N7** `answer(ask, { optionId, reason })`: an ask not open, or no harness answer road: `gone`. No option picked by
  the id (`picked_options`, first): `no-option`. Outcome `denied` for a deny option, else `allowed`. The reason rides
  a deny alone, trimmed, and only when not blank. The harness is asked `{ optionId, outcome, denyMessage:
  denied_line(reason), reason? }`; `answered` answers `answered`, anything else `gone`.
- **N8** `waited_so_far = waited + (now - waiting_since if open)`; a reply carries `waitedMs` only where that is > 0.
- **N9** `after_told(result, held)`: with no line told yet, `held` sends nothing and an unheld end sends the result
  whole. A told line whose body differs from this end's whole body sends the end whole (unheld only). A held end
  whose text starts with the told words sends the rest, trimmed, where not empty. Otherwise nothing where the
  status matches the told one and the told line promised nothing, else the outcome alone (no `text`, no `error`).

## Opening the session (turns.ts:571-668)

- **N10** Before `open`: the waiting box is read again (a kept process may still say waiting), `idle.hold(ws)`. A
  throwing `open` releases the hold and fails the run. Then `arm_disk_watch()`. `finished` resolving releases the hold
  and reads the disk (a failure warned `"disk of <ws> not read after its turn: <message>"`); rejecting releases it.
- **N11** The row id: a view whose id is `""` or the turn id takes the session's `local_id`; any other id stays (a
  re-opened turn keeps the id a client holds). `row_id` is the id after that.
- **N12** A turn that resumes (`resume` set) and finds a row at the session's `local_id` takes over its
  `startedBy` (where set), `prompt` (where set) and `costUsd` (where set).
- **N13** One row per turn: where the turn id is not the row id, the turn-id key is removed (`shift_remove`), then
  the row is put at `row_id` with `{ view, turnId, calls, notify?, notifyBy?, notifyRoad?, turnToken?,
  scopeDeviceId?, handle, end, turnLive, run?, from?, asked?, snapshot? (only a commit already known), pid? }`.
- **N14** A session with a pid on a kind whose daemon is shared: `turn_groups[ws][pid] = thread`,
  `arm_roots_recheck()`. Then `port_roots_moved(ws)` and `persist_sessions(ws)`.
- **N15** A snapshot still being taken: once it answers a commit, the row (if still this turn and running) takes it
  and is persisted.
- **N16** The handle: `id` the row id, `view()` a copy, `interrupt()` marks the turn stopped and interrupts the
  session; `steer`, `answer`, `set_access` and `stop_task` only where the session has them; `set_access` moves
  `view.permissionMode` only on `set`.

## `end(reason, by_stop, unreached)` (turns.ts:634-647)

- **N17** Nothing once ended or once the view is not `running`. Else: by_stop marks stopped; every open ask closed
  `cancelled`; the turn ended (forward drops everything but `turn.unread` from here); the row's `snapshot` taken off
  where the row is this turn; `settle_cut({ view, turnId, notify?, notifyBy?, notifyRoad?, turnLive, asked? },
  reason, || reason, by_stop, unreached)`; `persist_sessions`; the session interrupted (its failure swallowed);
  `turn_over`.

## `turn_over` (turns.ts:132-154)

- **N18** Once per turn. Takes `write_held` off the close list and runs it. Every thread record of this workspace
  with `rewound` loses it; where any did, `persist_sessions` and `thread.marked { workspaceId, threadIds }` in
  record order. `slates.turn_ended({ threadId, turnId })` unawaited, a failure warned `"the slate of thread <word>
  was not kept at the end of turn <turn>: <message>"`. `keep_checkpoint(entry, { sessionId, threadId, turnId,
  anchor?, kept? })` set as the thread's landing checkpoint, cleared when it settles if still the thread's.

## Changes (turns.ts:80-97)

- **N19** `read_changes(session)` runs once, only with a snapshot: awaits the snapshot's commit; with one, asks
  `read_turn_changes(entry, { sessionId, turnId, threadId, cwd, from, startedAt (view's, else now), wrote? })`. When it
  answers, and the end was already written and the row is still this turn with a snapshot, the row's snapshot is
  taken off and persisted, unless the read failed while the host closes (the commit stays for the next host).

## `settled(status)` (turns.ts:671-727), on `finished`

- **N20** The row's `turnToken` is taken off where it is this turn's; its snapshot where it is this turn and no read
  is out.
- **N21** A turn launched to keep (`keep`) whose session hands a kept agent keeps it where the turn was not ended
  here, has a token and a session id, and `hold_kept(thread, { workspaceId, agent, launch, session, turnToken, pid?,
  scopeDeviceId?, waiting })` takes it; otherwise the agent is closed (failure swallowed).
- **N22** Unless kept, the scoped device is revoked (warned `"the token of thread <word> was not taken away:
  <message>"` on failure).
- **N23** Not ended here: the view takes `status`; with no reply, `write_lines(view, turn_lines({ status }))`.
  `port_roots_moved`, `endedAt` set where unset, `end_snooze_for(view)` on `failed`, the thread record's
  `permissionMode` copied onto the view, `calls` cleared, the thread's lead ask dropped, `persist_sessions`, the end
  marked written.
- **N24** Not ended here: `refresh_title(view, true)`, then in order `read_checkout(entry, true)`,
  `ran_on_branch(entry, thread)`, `read_pull_request(entry, true)`, `read_lead_of(entry)`, a failure warned
  `"the branch of <name> was not read at a turn's end: <message>"`, then: the root's workspace (the entry, or the
  live entry of the root's latest row) with a pull request not `open` is settled again (`settle_tree`).
- **N25** Images landed for the turn are dropped (`drop_images(entry, dir)`), however it ended.
- **N26** `finished` ok: `settled(result.status)`, `send_back({ view, turnId, turnLive }, unread, stopped)`, and a
  `completed` turn not ended here asks `take_review(entry, thread, text or "")` (warned `"the review in <name> was not
  read: <message>"`). `finished` failed: `settled(failed)`, `send_back`.

## Tests (`tests/turns_run.rs`)

| Rule | Test |
|---|---|
| N1, N13 | `a_new_turn_holds_one_row_under_the_sessions_id_and_drops_its_turn_key` |
| N11 | `a_reopened_row_keeps_the_id_a_client_holds` |
| N12 | `a_resumed_turn_takes_over_its_rows_opener_prompt_and_cost` (session-row.test.ts) |
| N10 | `a_launch_that_throws_releases_the_idle_hold_and_fails`, `the_hold_is_released_when_the_turn_finishes` |
| N4, N6, N8 | `an_open_ask_marks_the_row_asking_and_counts_the_time_on_the_person` (permission-relay.test.ts) |
| N5 | `a_mode_option_reads_the_catalogs_label` |
| N7 | `answer_allows_denies_and_carries_a_reason_only_on_a_deny`, `answer_on_a_closed_ask_is_gone`, `answer_naming_no_option_is_no_option` |
| N9 | `after_told_sends_nothing_twice_and_the_rest_of_a_held_end` (codex-held-turn.test.ts) |
| N14 | `a_shared_daemon_turn_names_its_group` |
| N15, N19, N20 | `the_snapshot_rides_the_row_until_the_changes_are_read` (turn-changes.test.ts) |
| N16 | `the_handle_has_only_the_roads_the_session_has`, `set_access_moves_the_row_only_when_set` |
| N17 | `end_closes_open_asks_settles_the_cut_and_interrupts_once`, `end_after_the_turn_settled_does_nothing`, `end_hands_settle_cut_the_turns_own_row_fields` (trap #1646) |
| N18 | `the_turns_end_drops_every_undo_on_its_workspace_and_says_so`, `the_turns_end_keeps_a_checkpoint_and_writes_what_the_agent_held` |
| N21, N22 | `a_kept_agent_keeps_its_token_and_device`, `an_agent_not_kept_is_closed_and_its_device_revoked` (thread-keeps-process.test.ts) |
| N23 | `a_process_that_exits_with_no_result_reads_its_status_words`, `a_pick_landed_on_the_record_rides_the_ended_row` |
| N24 | `a_turns_own_end_reads_the_branch_then_the_pull_request_then_the_lead`, `a_turn_ended_here_reads_no_title_and_no_branch` |
| N26 | `a_completed_turn_takes_its_review`, `a_failed_finish_settles_failed` |
