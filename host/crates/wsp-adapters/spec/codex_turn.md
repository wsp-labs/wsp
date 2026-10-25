# codex_turn
Source: packages/adapter-codex/src/adapter.ts:513-1227 (`follow` and the session it answers), background.ts (24) at a3b870a6e.
Tests: adapter.test.ts, every describe but the revert/fork, side question, plan-limit `creditsOf`, kept-server and lost-session ones
(those are codex_adapter's): 72 cases. Rust: `src/codex/turn.rs`, `src/codex/children.rs`, `src/codex/background.rs`; tests
`tests/codex_turn.rs` over the fake stream in `tests/common/codex_wire.rs` (a port of the test's `wire`/`server`/`launcher`), fixtures
`app-server-turn.jsonl`, `app-server-subagent.jsonl`, `no-login-app-server.jsonl`, `auto-compaction.jsonl` byte for byte.
This unit is about 640 TS lines, past the 400 target: `follow` is one closure over one state, and its split is two files, one gate.

## What it does
Reads one Codex turn off `codex app-server`'s stdout and answers the server on its stdin. One state per turn: the thread id (the local
id until the server names it), the turn id, the subagents, the pending approvals, the requests awaiting an answer, the usage seen, the
rate limits, the last failure words. It emits `AdapterEvent`s in order, ends the turn at the lead's `turn/completed` once no subagent
still runs, and answers steer, approval, stop-one-subagent, interrupt and kept.

## Behaviour
1. Start (:552). The reader is spawned at once (TS runs it eagerly); `finished` settles with the turn result. A session that is neither a
   side question nor a side run joins the adapter's registry under its local id (:1225).
2. Announce (:642), once: `session.start {sessionId, model?, cwd?, prompted?}`. model is the start's model, else the server's; cwd the
   start's, else the server's; `prompted: false` where the run hands its prompt over later (a `turn_line` or `owed_turn` was given) and no
   turn id is known yet. Notes said before the announce (`configWarning`/`warning`) are held and emitted right after it, in order.
3. Lines (:1080): a line `read_message` (codex_rpc) cannot read is stderr: trimmed, kept if non-empty, last 5 only. Each message goes to
   its handler by kind. Every notification but `error` clears the reconnect stall (rule 14).
4. Foreign (:632): a notification whose `threadId` is a string other than the announced thread's goes to the subagent handler (rule 12),
   except `serverRequest/resolved`. Before the announce nothing is foreign.
5. Responses (:898). `wsp-thread`: `legacy` turns on where not yet announced and `thread.historyMode == "legacy"`; announce with the thread
   id, `model` (answer's, else thread's), `cwd` (answer's, else thread's); then a side run gets the answer; else a turn line (where the
   thread was not already open) is handed over (rule 6); else an owed turn is written where the re-opened stream's `taken` holds no line
   starting a turn (`is_turn_start`). With a side run, `wsp-turns` and `wsp-revert` answers go to it. `wsp-account`: a sign-in of type
   `apiKey` emits `limit {windows: [], keyed: true}`; email (as label) and `planType` (as plan) merge into the account; then the limit is
   emitted if a reading exists. `wsp-rate-limits`: a string `accountId` sets the account id; the reading becomes `snapshot_of(answer)`;
   the limit is emitted with `credits_of(answer)`. Any other string id settles its awaiting request as answered.
6. Hand-over (:884): the turn line goes once `prompt_after` (or nothing) settles, unless the turn was stopped or the process exited by
   then (then nothing is sent). Where a `prompt_after` was given and the stream has `write_after`, the line is parked there with that
   gate; else it is written when due.
7. Errors (:938): an awaited id settles with the error. Else `wsp-revert`: fail `codex would not cut the thread: <m>`; `wsp-turns`:
   `codex could not list the thread's turns: <m>`; `wsp-thread`: `conversationWrittenElsewhereLine("Codex", LETS_GO)` where the message
   holds `already has an active writer`, else `codex could not open the thread: <m>`; `wsp-turn` or `wsp-initialize`:
   `codex could not start the turn: <m>`. These failures carry no `durationMs`. Other ids are ignored.
8. Notifications (:951): `thread/started` announces (legacy rule as 5). `turn/started`: the first one that names an id opens steering;
   where the run handed its prompt over later and is announced, it emits `session.start` again without `prompted`; then
   `turn.anchor {anchor: turn id, kept: CODEX_LEGACY_HISTORY where legacy}`, once. `item/started|completed`: commands start/end in the
   turn's set and the background set; a file change's `changes` are kept by item id; a completed `agentMessage` is the last text; a
   `contextCompaction` opens a compaction (`before` = `held_of` of the latest usage's last call, where usage was seen), marks it ended on
   completion, and says it where `after` is known; subagent signs (rule 12); then `item_deltas`. `thread/tokenUsage/updated` with a
   `last` object: `turn.usage {tokens: input+output of last, context: held_of(last), window?, at: emittedAtMs?}`; usage becomes
   `{before: kept or total_before(total, last), total, last, window?}`; an open compaction with no `after` takes `held_of(last)` and is said
   if ended. `account/rateLimits/updated`: skipped when `rateLimits.limitId` is a string other than `codex`; else every non-null field of
   it overwrites the last reading (absent and null keep the old value), and the limit is emitted. `turn/plan/updated`: `turn.plan` with
   each object of `plan` as `{text: step or "", state: step_state(status)}`. `configWarning`/`warning`: the string `summary`, else
   `message`, as a note. `serverRequest/resolved`: an open approval under `String(requestId)` closes `cancelled`. `error`: last error is
   the message (or ""); `codexErrorInfo == usageLimitExceeded` without `willRetry: true` marks the turn usage-limited; the failure words of
   `"<message> <additionalDetails or ''>"` replace the last words where they name any; a message starting `Reconnecting...` arms the stall;
   with `willRetry: true`, `retry_of` emits a `turn.retry` where it gives one. `turn/completed` whose turn id matches (or either is
   unknown): a `usageLimitExceeded` error marks the limit; the lead's end is `{status (default "failed"), error?}`; any compaction is said;
   the lead settles (rule 10). Other methods are ignored.
9. Compaction is said once: `turn.compacted {before?, after?}`, then cleared.
10. The lead's reply (:785) goes once the lead ended, no child is running and the process has not exited. Its result: `interrupted` alone;
    a non-completed status fails with its error (default `codex reported a failed turn`) through failure words (`refusal` where they
    claim a cause, `durationMs` set); completed is `{status, durationMs, text?, tokens?, model?}`, tokens being `turn_tokens_of(usage)`
    with child tokens added (codex_items 10).
11. Finish (:749), once: a failed turn that was usage-limited gets `limit {resetsAt?}` from `stopped_until(limit_of(reading, account).windows,
    now)`. A non-completed, non-interrupted turn releases the keeper. Otherwise, on a keeper that is up with thread options, the background
    is held for this thread where it has running commands; with none, `thread/unsubscribe` is written. Then `turn.done`, every open
    approval closed `cancelled`, every awaiting request settled gone, stdin closed, and `end_after_result(result_exit_ms, grace)` started.
12. Subagents (children.rs, :664-841). A child is learned (`learn`) from a `spawnAgent` collab call not `failed` (each receiver; parent the
    call id, prompt and model where strings), a `subAgentActivity` `started` (parent the item id; name the last path segment, depth
    `depth_of(path)`), or its own `turn/started`. A new child is `running`, its parent its own id until a sign names one, its title the first
    line of the prompt trimmed, else the name; it emits `subagent` and the task count. A known child takes what a sign adds (a parent where it
    had its own id, a prompt and title where none, model, depth, title) and is said again only where something was added and it runs. A
    child ends `done` at a completed `closeAgent` call or `subAgentActivity completed`, `stopped` at `interrupted`, or by its own
    `turn/completed` (status through `child_end`, ignored when its turn id differs). An end clears its turn id, says it, says the count and
    settles the lead. A `subagent` event carries title, depth, model and `asked` (`subagentAsked(prompt)`) while running, summary and usage
    once ended. The count (`turn.tasks`) is the running children with a turn id, emitted only when it changes. A child's
    `thread/tokenUsage/updated` emits `turn.usage {tokens, at?}` with no context, adds every breakdown field to the child tokens, and where
    `child_usage_of(total)` gives usage, keeps it and emits `subagent {running, parentToolUseId, usage}` while running. A child's completed
    `agentMessage` is its summary. Its own items count commands and changes as the lead's do; its plan and texts are never the lead's.
13. Requests (:849): a method that is not one of the two approvals is answered `refuse_request_line`. A side question or side run answers
    every approval `decline`. Else an ask: `askId = String(id)`, toolName from `APPROVALS`, toolUseId the `itemId`, parentToolUseId (for a
    foreign thread) its child's parent or the thread id, input the JSON of `{changes}` (from the kept changes) or `{command:
    shell_script_of(command), cwd?}`, detail the non-empty `reason`, options Allow then Deny. It is kept pending and `permission.ask` emitted.
14. Stall (:621): the first `Reconnecting...` error after progress arms a timer of `reconnect_stall_ms` (90 s); at it the words become
    `codexReconnectLine(now - stalled_at)` and the run is ended (`end_run`). Progress clears it.
15. Aside wall (:1061): a side question arms `aside_wall_ms` (3 min) at start; at it the words become `asideWallLine(ms)` and the run is
    ended. Cleared when the process exits. A turn has no wall.
16. Exit (:1107): after the lines end (a thrown stream error is kept with its message), await the exit code; mark exited; clear the stall;
    close open approvals `cancelled`; settle awaiting gone; end every running child (`stopped` under a stop, else `failed`); a lead end
    seen but not finished emits its `turn.done` now; with no result: `interrupted` under a stop, else the last words, else the stream's
    error message, else `codex exited with code <code> before its turn ended[: <last error, else the stderr tail joined by \n>]`, through the
    limit rule; then `session.end {exitCode, sawResult, failure?}`.
17. `steer(prompt)` (:1154): before a turn id, wait up to `steer_join_ms` (10 s) for steering to open or the turn to end; not running
    (no turn id, lead ended, finished, exited or stopped) is `not-running`. The request id is `wsp-steer-<n>` from one counter shared with
    interrupts. A write that throws reads `unanswered` while still running (the answer decides), `gone` otherwise (`not-running`). Accepted
    only where the server's answer is a result.
18. `answer(askId, a)` (:1169): no such pending ask, or exited, is `gone`. The ask leaves the pending map before the write; a write that
    throws while not exited puts it back and rethrows; `gone` from the write is `gone`. Written: `permission.close {outcome, optionId}`,
    and a deny with a reason steers that reason into the turn. Allow writes `accept`, anything else `decline`.
19. `stop_task(task)` (:1186): not running (exited, finished, unknown, not running, no turn id) is `not-running`; else interrupt that
    child's turn: gone is `not-running`, a result `accepted`, an error `refused` with its words.
20. `interrupt()` (:1193): nothing after exit. Marks stopped, opens steering. With no running lead turn and no running child with a turn,
    ends the run (`end_run`). Else, with commands running, releases the keeper; interrupts each running child first, then the lead, each
    `wsp-interrupt-<n>`; then `end_after_result(grace, grace)`.
21. `kept()` (:1210): only with a keeper that is up and a finished turn: `{next, close, exited, working, sessionFile: rollout_in}`; close
    waits `result_exit_ms` (0 with `now`); working is whether the background has commands.
22. Background (background.ts): between turns a completed `commandExecution` leaves the set; when it empties and a thread is held,
    `thread/unsubscribe` is written for it (its failure ignored) and the hold cleared.
23. The synchronous half of each TS method runs when the Rust method is called, before its future is returned: `answer` takes the ask off
    the map and starts the write at call time, so an `answer` made from inside `on_event` lands before the reader's next line, as in TS.
    Every write nobody awaits (`void stream.write`) is still driven to completion.

## Edge cases
- A `thread/started` of another thread after the announce changes nothing (legacy read off this thread's own announce only).
- An interrupt's answer is not the end; `turn/completed` with `interrupted` is.
- A child's `turn/completed` never ends the lead; the lead's never ends a running child's hold.

## Behaviour nobody wrote down
- Rule 3.3: no callback (`on_event`) is called while the turn's state is borrowed; collect, release, then emit, in order.
- Rule 4.25: `askId` from a numeric id is the JS number text.

## Decisions
1. `answer` returns `Result<AnswerOutcome, ExecFailure>`: the TS rejects with the write's error and the port's `HarnessSession::answer`
   cannot; asked of the port's owner in the report.
2. Time is `deps.now` where given, else the system clock; timers are tokio's.

## After the port
- Nothing found.

## Tests (tests/codex_turn.rs 42, codex_turn_more.rs 41, codex_turn_rules.rs 7 written for this port; TS 84 of adapter.test.ts)
- Rules 1, 21: `codex_turn_rules.rs` `a_turn_joins_the_registry_under_its_local_id_and_a_side_question_never_does`,
  `a_kept_servers_next_turn_resumes_the_thread_with_the_first_turns_options_then_sta`; `codex_turn.rs` `resumes_a_thread_by_the_id_the_row_holds_the_registry_key_is_that_id_and_the_see`.
- Rules 2, 8 (announce, notes): `reads_a_recorded_turn_session_start_unprompted_as_the_thread_opens_and_again_onc`;
  `codex_turn_rules.rs` `notes_said_before_the_thread_is_announced_follow_its_session_start_in_order`.
- Rule 3: `a_process_that_dies_mid_turn_fails_under_the_thread_s_id_with_its_last_stderr_li`. Rule 4 and 12: `codex_turn_more.rs`
  `a_subagent_s_frames_on_its_lead_s_stream::*` (12), `a_recorded_codex_turn_whose_lead_spawned_an_agent_and_ended_first::*`.
- Rules 5, 7: `a_server_that_turns_the_thread_down_fails_the_turn_in_its_words_and_sends_no_tur`; the account and limit cases in
  `codex_turn_more.rs` `a_codex_account_s_plan_limits_on_the_app_server::*` (8); attach and owed turn: `a_turn_a_later_host_attaches_to::*`.
- Rule 6: `takes_its_prompt_late_the_server_starts_and_opens_the_thread_at_once_and_the_tur`, `interrupt::a_stop_before_the_prompt_was_handed_over_sends_no_turn_then_or_once_the_prompt_i`.
- Rule 8 (items, usage, plan, compaction, retries, errors): `a_codex_turn_s_tokens_and_plan_on_the_app_server::*` (4),
  `codex_adapter_over_codex_app_server::*` item cases, `a_model_call_codex_tries_again::*`, `a_codex_thread_s_own_compaction::*` (5),
  `what_a_rewind_needs_of_a_codex_turn::*` (3 anchor cases), `a_codex_turn_that_does_not_complete::*`.
- Rules 9-11: `what_ends_a_codex_turn::only_turn_completed_the_thread_going_idle_and_its_last_message_leave_the_turn_op`,
  `a_turn_the_plan_s_usage_limit_stopped::*` (4), `codex_turn_rules.rs` `a_failed_turn_releases_its_keeper_so_the_next_turn_launches_again`,
  `the_process_a_finished_turn_leaves::*` (2).
- Rule 13: `an_approval_codex_asks_for::*` (6). Rule 14: `a_provider_that_never_answers_the_server_retries_forever_so_the_adapter_ends_the`,
  `a_retry_run_that_recovers_into_the_turn_is_not_cut`. Rule 15: `codex_adapter.rs` `ends_a_fork_that_answers_nothing_for_the_whole_wall_with_its_tree_and_says_so`,
  `a_turn_is_held_to_no_wall_of_the_side_questions`.
- Rule 16: `a_transport_that_ends_the_turn_itself_makes_its_message_the_turn_s_error`, `a_process_that_goes_with_a_child_still_running_ends_the_child_too_stopped_under`.
- Rule 17: `a_message_sent_while_a_codex_turn_runs::*` (3), `what_ends_a_codex_turn::a_steer_*` (3), `codex_turn_rules.rs` `steers_and_interrupts_are_numbered_off_one_counter`.
- Rules 18, 23: `an_answer_whose_write_failed_leaves_the_request_open_so_answering_again_lands_it`, the recorded turn answered from `on_event`.
- Rule 19: `stops_one_child_alone_by_its_thread_id_the_lead_running_on_and_its_interrupted_e`.
- Rule 20: `interrupt::*` (5), `codex_turn_rules.rs` `an_interrupt_with_a_command_running_takes_the_kept_server_down_with_the_turn`.
- Rule 22: `codex_turn_rules.rs` `a_kept_turn_that_leaves_a_command_running_holds_the_thread_and_lets_it_go_once_t`.
