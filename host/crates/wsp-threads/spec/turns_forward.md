# turns_forward: `threads/turns.ts:291-569`

`TurnState::forward(event)`: every `AdapterEvent` a turn's harness session emits, turned into rows, row facts,
ledger entries and line deliveries. Shares `TurnState` with turns_run. The replay counters are what invariant 2
rests on: a re-opened run is read from its first line again (invariant 1), so every row kind with a count skips
as many as were already written.

## Rules

- **F1** `turn.unread` is kept (its ids) whenever it comes, after the end too. Every other event is dropped once the
  turn was ended here (turns_run N17).
- **F2** `session.start`: the view takes `claudeSessionId`, `cwd` and `model` (each where given) and the entry's
  record the session id (`set_session`); with `asked`: `prompted: false` before the start row was written sets
  `awaitsPrompt`, any other `prompted` clears it (trap #1620). Then `persist(record)` and `persist_sessions`. A
  prompted start with a request id: `prompt_held(request)`. A title given at the start, before the start row: 
  `name_in_harness(view, title)`.
- **F3** One start row per turn: once written (or `written.started`), later starts write nothing more. The row:
  `session.start { workspaceId, sessionId, turnId, threadId, prompt, requestId?, via?, afterCut? (only true),
  afterLimit?, opensThread? (only true), startedBy, attachments?, model?, cwd?, permissionMode?, effort?, agent,
  tools?, harness? }` in that order, `model` and `cwd` from the event, `permissionMode` and `effort` from the view.
  A view whose title source is `seed` then holds one title ask: `refresh_title(view, false)` then
  `make_title(view)`, a failure warned `no_made_title_log_line(thread, workspace, message)`.
- **F4** `turn.delta`: a `tool_use` with an id and a name is an open call (`calls`), its changed files added to
  `wrote` where the harness reports edits; a `tool_result` with an id closes it. This bookkeeping runs on replayed
  deltas too. A replayed delta (while the replay count is above 0) is counted down and writes nothing; otherwise
  `session.delta { ..., line: ++lines, kind, text, messageId?, toolName?, toolUseId?, isError?, parentToolUseId?,
  cwd?, exitCode?, durationMs?, bytes?, patch? }`.
- **F5** `turn.done`: a result naming `tokens.context` marks the held context as named. A reply already written
  (`written.reply`) is skipped once, leaving `turn_live.reply` as written (or the recorded reply where none): a
  reply never overwrites one already given.
- **F6** Otherwise: `turn_live.reply = status`; the result gains `waitedMs` (turns_run N8); `write_lines(view,
  turn_lines(result))`; `view.refusal` and `view.limit` where the result has them; `view.costUsd += costUsd`.
- **F7** A result with tokens or a cost files ledger rows: `uses` is the result's `models` where non-empty, else
  one use `{ model: result.model ?? view.model, tokens, costUsd }`; the turn counts once, on the first use of the
  result's model (index 0 where none matches). Each row `{ at: now, agent, account, accountLabel, computer,
  project, turns, model?, tokens?, costUsd?, session?, source: "wsp" }`, `session` the view's session else the
  event's; a failure warned `"the use of turn <turn> was not filed: <message>"`.
- **F8** Then `persist_sessions`; the line to the turn's notify targets is `after_told(result, held)` (N9), sent with
  `notify_end({ view, turnId, turnLive }, notify, tell_as(turn), told)` only where there are targets, something to
  tell, and the turn is not a line's try (`line_try`); then `session.done { ..., result }`; then the changes read.
- **F9** `turn.anchor`: kept for the checkpoint (`anchor`, `kept`). No row.
- **F10** `subagent`: replay counted like deltas; `session.subagent { ..., line: ++subagents, task, state,
  parentToolUseId?, title?, summary?, depth?, model?, asked?, usage? }`.
- **F11** `limit`: the turn's account becomes the limit's own where it names one. `ledger.limit({ key, agent, label,
  road, computer, limit })`, then `alerts.read(before, after)`, then the account's Usage row (by key) is pushed as
  `usage.account { key, row }` where found; a failure warned `"the limits of <key> were not kept: <message>"`.
- **F12** `turn.usage`: `burn.add({ account, threadId, tokens, replayed? })`, `replayed: { run: turnId, at }` only on
  a re-opened turn whose event carries `at`. A `context` sets what the agent holds and passes (never records)
  `session.context { ..., at: now, context, window? }`.
- **F13** `turn.compacted`: replay counted; `session.compacted { ..., line, before?, after? }`.
- **F14** `turn.retry`: replay counted first (a replayed retry tells waits nothing); then `agent_retried(turn,
  why)`, then `session.retry { ..., line, attempt, of?, why? }`.
- **F15** `turn.plan`: `session.plan { ..., steps?, text? }`, every time (no count).
- **F16** `turn.tasks`: tasks running is `running > 0` and the waiting box is read again. With `replied`: the reply
  (with waited time); a `completed` reply with work running gains `still_running_line(running)` after a blank line
  (an empty text gives the line alone); `told_as = { status, body: notify_body(reply, "whole"), said: text or "",
  promised? (only where the line was added) }`. A re-opened turn's told count is skipped first; otherwise, with
  targets, `notify_end(..., replied, { open: true } where promised)` and `told += 1`. Then `persist_sessions`.
- **F17** `permission.ask`: options relabelled (N5), the ask opened (N4), `session.permission { ..., ...ask }`,
  then `end_snooze_for(view)`. `permission.close`: `close_ask` (N6).
- **F18** `session.end`: the stream's failure kept on `turn_live`; every open ask closed `cancelled`; the view
  takes the held reply's status (before the row is recorded); `endedAt = now`; `session.end { ..., exitCode,
  sawResult, promptless? (only where the row awaits its prompt) }`; `turn_over`; the changes read.

## Tests (`tests/turns_forward.rs`)

| Rule | Test |
|---|---|
| F1 | `unread_ids_are_kept_after_the_end` , `events_after_an_end_here_write_nothing` |
| F2 | `the_start_names_the_session_on_the_view_and_the_record`, `a_start_before_the_prompt_marks_the_row_awaiting_it` (trap #1620), `a_title_given_at_the_start_is_written_into_the_harness_once` |
| F3 | `one_start_row_however_often_the_harness_announces`, `invariant_2_a_written_start_row_is_not_written_again`, `a_seed_titled_thread_asks_for_a_title_once` |
| F4 | `deltas_are_numbered_from_one`, `invariant_2_a_reopened_turn_writes_no_delta_twice` (subagents.test.ts), `open_calls_follow_tool_use_and_result_even_while_replaying` |
| F5 | `invariant_2_a_written_reply_is_not_written_again` |
| F6 | `the_reply_writes_its_lines_refusal_limit_and_adds_its_cost` (turn-costs.test.ts) |
| F7 | `a_turn_counts_once_on_its_own_models_ledger_row` (usage-ledger.test.ts), `no_tokens_and_no_cost_files_nothing` |
| F8 | `the_end_is_told_to_its_targets_once_unless_a_lines_try` |
| F10, F13, F14 | `invariant_2_no_subagent_compaction_or_retry_row_twice` (subagents.test.ts:280-340) |
| F11 | `a_limit_is_kept_and_pushes_its_usage_row` (usage-limit-resume.test.ts) |
| F12 | `usage_is_burned_and_the_context_passes_unrecorded`, `only_a_reopened_turn_marks_its_usage_replayed` |
| F14 | `the_agents_retry_tells_waits_and_is_numbered` |
| F15 | `a_plan_is_written_every_time` (turn-plan.test.ts) |
| F16 | `a_reply_over_running_work_is_told_once_and_says_it_is_still_open` (codex-held-turn.test.ts), `a_reopened_turn_does_not_tell_a_told_reply_again` |
| F17 | `an_ask_is_written_with_named_options_and_ends_a_snooze` (permission-relay.test.ts) |
| F18 | `the_exit_closes_open_asks_and_settles_the_row_before_its_end_row`, `promptless_rides_the_end_of_a_turn_awaiting_its_prompt` |
