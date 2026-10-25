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
2. Announce (:642), once: `session.start {sessionId, model?, cwd?, prompted?}`. model is the one `follow` was handed (an attach's row or
   a kept thread's), else the server's (answer's, else thread's): a launched start hands `follow` its cwd alone (:1283), so its own model
   rides the thread line and is never read back (CT10); cwd the start's, else the server's; `prompted: false` where the run hands its prompt over later (a `turn_line` or `owed_turn` was given) and no
   turn id is known yet. Notes said before the announce (`configWarning`/`warning`) are held and emitted right after it, in order.
3. Lines (:1080): a line `read_message` (codex_rpc) cannot read is stderr: trimmed, kept if non-empty, last 5 only (CT1). Each message goes to
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
8. Notifications (:951): `thread/started` announces (legacy rule as 5). `turn/started`: one with no id changes nothing; one that names an id sets the turn id, a later one replacing it with no new anchor
   (CT12); the first one that names an id opens steering;
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
    any other status, or none, fails with its error (default `codex reported a failed turn`, CT6) through failure words, where words
    said earlier by an `error` notification beat the turn's own error (CT11) (`refusal` where they claim a cause, `durationMs` set); completed is `{status, durationMs, text?, tokens?, model?}`, tokens being `turn_tokens_of(usage)`
    with child tokens added (codex_items 10).
11. Finish (:749), once: a failed turn that was usage-limited gets `limit {resetsAt?}` from `stopped_until(limit_of(reading, account).windows,
    now)`. A non-completed, non-interrupted turn releases the keeper. Otherwise, on a keeper that is up with thread options, the background
    is held for this thread where it has running commands; with none, `thread/unsubscribe` is written. Then `turn.done`, every open
    approval closed `cancelled`, every awaiting request settled gone, stdin closed, and `end_after_result(result_exit_ms, grace)` started.
12. Subagents (children.rs, :664-841). A child is learned (`learn`) from a `spawnAgent` collab call not `failed` (each receiver; parent the
    call id, prompt and model where strings), a `subAgentActivity` `started` (parent the item id; name the last path segment, depth
    `depth_of(path)`), or its own `turn/started`. A new child is `running`, its parent its own id until a sign names one, its title the first
    line of the prompt trimmed, else the name; it emits `subagent` and the task count. A child that ended and prints its own `turn/started` again runs again: its summary cleared, said again, counted. A known child takes what a sign adds (a parent where it
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
    shell_script_of(command), cwd?}`, detail the non-empty `reason`, options Allow then Deny. It is kept pending and `permission.ask` emitted; a second request under the same id replaces the first and is asked again.
14. Stall (:621): the first `Reconnecting...` error after progress arms a timer of `reconnect_stall_ms` (90 s); at it the words become
    `codexReconnectLine(now - stalled_at)` and the run is ended (`end_run`). Progress clears it.
15. Aside wall (:1061): a side question arms `aside_wall_ms` (3 min) at start; at it the words become `asideWallLine(ms)` and the run is
    ended. Cleared when the process exits. A turn has no wall.
16. Exit (:1107, CT1 to CT4, CT8): after the lines end (a thrown stream error is kept with its message), await the exit code; mark exited; clear the stall;
    close open approvals `cancelled`; settle awaiting gone; end every running child (`stopped` under a stop, else `failed`); a lead end
    seen but not finished emits its `turn.done` now; with no result: `interrupted` under a stop, else the last words, else the stream's
    error message, else `codex exited with code <code> before its turn ended[: <last error, else the stderr tail joined by \n>]`, through the
    limit rule (none of these carries `durationMs`; a signal reads `code null`); then `session.end {exitCode, sawResult, failure?}`.
17. `steer(prompt)` (:1154): before a turn id, wait up to `steer_join_ms` (10 s) for steering to open or the turn to end; not running
    (no turn id, lead ended, finished, exited or stopped) is `not-running` and writes nothing. The request id is `wsp-steer-<n>` from one counter shared with
    interrupts. A write that throws reads `unanswered` while still running (the answer decides), `gone` otherwise (`not-running`). Accepted
    only where the server's answer is a result.
18. `answer(askId, a)` (:1169): no such pending ask, or exited, is `gone`. The ask leaves the pending map before the write; a write that
    throws while not exited puts it back and rethrows; `gone` from the write is `gone`. Written: `permission.close {outcome, optionId}` as the caller named them, whatever decision went (CT13),
    and a deny with a reason steers that reason into the turn. Allow writes `accept`, anything else `decline`.
19. `stop_task(task)` (:1186): not running (exited, finished, unknown, not running, no turn id) is `not-running`; else interrupt that
    child's turn: gone is `not-running`, a result `accepted`, an error `refused` with its words.
20. `interrupt()` (:1193): nothing after exit. A second call finds no live lead, so it ends the run at once. Marks stopped, opens steering. With no running lead turn and no running child with a turn,
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
- A prompt arriving on a known child sets its title unconditionally, so a spawn whose prompt starts with a blank line wipes a title
  an activity item gave (adapter.ts:713).
- `before` sticks to the first usage even when that line has no `total`, so the turn's tokens read as the last call alone.
- After an `account` answer the `limit` it emits carries no credits, though a rates answer had them.
- Two approvals under one id: the first is lost. The registry is never pruned. The stall timer is not cleared at finish.
- `APPROVALS` is a plain object, so a request whose method is `constructor` or `toString` finds an inherited member and is asked as an
  approval with a function for its tool name; the port reads it as unknown and refuses it.

## Rules added for the port (numbered; each value printed by the TS at 65f6b89c7, adapter code unchanged since a3b870a6e)

- **CT1.** The stderr tail (:1080-1085) takes every line `read_message` refuses, so a JSON array or an object with neither a
  method nor an id counts as stderr. Each is JS-trimmed; an empty result is dropped before the count; the last 5 are kept
  and joined by `\n`. Lines `"  first  ", "", "   ", "two", {"neither":true}, "three", [1,2], "four", "five"` then exit 3
  read `codex exited with code 3 before its turn ended: {"neither":true}\nthree\n[1,2]\nfour\nfive`.
- **CT2.** The exit words' reason is `lastError ?? tail`: an `error` notification with no message sets `lastError` to `""`,
  which is not absent, so the tail is never read and the words end in `: ` (`...before its turn ended: `). A process with
  no code (a signal) reads `code null`, and `session.end` carries `exitCode: null`.
- **CT3.** On the exit road the words beat the stream's message: a 401 said in an `error` notification, then a stream that
  throws, ends `{failed, error: <not signed in line>, refusal: sign-in}`, and `session.end` carries the thrown failure. The
  exit road's failed result never carries `durationMs`; the lead's own failed end (`failed()`, :770) always does.
- **CT4.** The exit road's order (:1108-1125): each open ask closes `cancelled` with no option id, in the order raised;
  every awaited request settles gone (a steer waiting on its answer reads `not-running`); each running child ends
  `failed` (`stopped` under a stop) followed by `turn.tasks`; `turn.done`; `session.end` last. After it `answer` is Gone,
  `steer` and `stop_task` not running, `interrupt` returns at once, and nothing more is written.
- **CT5.** The finish road's order is the other way round (:763-767): `turn.done` first, then each open ask closes
  `cancelled`, then awaited requests settle gone, then stdin closes. A finished, failed or interrupted turn stays in the
  registry under its local id; nothing ever removes it, and a later start under the same key replaces it. A new
  thread's local id is a fresh uuid, never the id the server names.
- **CT6.** A lead `turn/completed` with no `status`, or one that is not `completed` or `interrupted`, fails as
  `{failed, durationMs, error: "codex reported a failed turn"}`. A `turn/completed` naming another turn id, when both ids
  are known, is ignored, so the turn ends only by the exit (`codex exited with code 0 before its turn ended`).
- **CT7.** A refusal before the thread is announced (`wsp-thread` or `wsp-initialize` error) says only `turn.done` and
  `session.end`, both under the local id, and writes no `turn/start`. A refused `turn/start` comes after the announce, so
  `session.start` with `prompted: false` is said once before them.
- **CT8.** A lead that ended while a child runs holds its end and closes nothing. When the process then goes, the child
  ends `failed`, the held end becomes the result (completed, `durationMs` set, its text and model) said by `turn.done`,
  and `session.end` reads `sawResult: true`. This road neither releases the keeper nor writes `thread/unsubscribe`.
- **CT10.** A launched start's `model` rides the thread line only; `session.start` and the result name the server's model
  (`gpt-5.6-sol` where the start asked `my-model`). The cwd is the start's.
- **CT11.** Failure words said earlier beat a completed turn's own error: an error `Missing environment variable: <FOO_KEY in backticks>`, then
  `turn/completed` failed with `other boom`, ends `{failed, durationMs, error: <missing env line for FOO_KEY>}`.
- **CT12.** A second `turn/started` with another id replaces the turn id with no new anchor: a steer then names `expectedTurnId: tt2`,
  the first turn's `turn/completed` is ignored, and the anchors said are `[tt1]`.
- **CT13.** `answer` writes `accept` only for `PERMISSION_ALLOW`; the close carries the caller's outcome and option id as given
  (`allowed`/`whatever` wrote `decline`), and a deny's reason is steered (`wsp-steer-<n>`) after its decision line.
- **CT9.** `TurnResult` key order per road, as `JSON.stringify` printed it. wsp-wire's struct writes one order (status,
  durationMs, waitedMs, costUsd, tokens, model, models, text, error, unreached, refusal, limit), so this is the accepted
  difference ruled for claude (#2447, coordinator rulings, item 1): listed here, pinned by no test in this crate.

  | Road | TS key order |
  |---|---|
  | completed | status, durationMs, text, tokens{input, output, cached, cacheWrite, reasoning, context, window}, model |
  | completed, no usage, no text, no model | status, durationMs |
  | interrupted (server's end, or a stop) | status |
  | lead failed | status, durationMs, error, refusal? |
  | lead failed, usage limit | status, durationMs, error, limit (`{}` where no window names a reset ahead) |
  | exit road, words or died | status, error, refusal? |
  | exit road, usage limit | status, error, limit |
  | refused request (`onError`, :941-947) | status, error |
- **CT14.** The reconnect stall (`reconnect_stall_ms`) arms at the first `Reconnecting...` error while none is armed and is cleared by
  any notification but `error` (a foreign one or `serverRequest/resolved` included), and by the exit; a request, a response or a stderr
  line does not clear it. At it the words are `codexReconnectLine(elapsed)` (`stopped after 0m 00s of Codex reconnecting to its model
  provider with no answer` under a second), the run is ended (`teardown`, then `kill` past the grace), and the exit road's result
  carries no `durationMs`. The open ask closes `cancelled` before `turn.done`.
- **CT15.** In `finish` the keeper's `thread/unsubscribe` is written before `turn.done` is said, and an open compaction is said before
  `turn.done` (`before: 900`, no `after` where no usage followed it).
- **CT16.** A stop while `prompt_after` is pending: no turn id, so the run ends at once; no `turn/start` is ever written; events
  `session.start` (unprompted), `turn.done {interrupted}`, `session.end`.

## Recorded sessions
The record is the fixtures, codex-cli's own output: `app-server-turn.jsonl` (0.155.1, a signed-in turn that asked for a command, was
allowed and took a steer), `app-server-subagent.jsonl` (0.155.1, a lead that spawned an agent and ended first), `auto-compaction.jsonl`
(0.155.1, a compaction the server ran at a turn's start), `no-login-app-server.jsonl` (0.155.1, an empty CODEX_HOME). Replayed by:
- app-server-turn: `codex_turn.rs` `reads_a_recorded_turn_session_start_unprompted_as_the_thread_opens_and_again_onc`,
  `launches_the_app_server_in_the_folder_under_codex_home_seeding_initialize_and_th`, `resumes_a_thread_by_the_id_the_row_holds_the_registry_key_is_that_id_and_the_see`,
  `takes_its_prompt_late_the_server_starts_and_opens_the_thread_at_once_and_the_tur`, `reads_the_plan_the_server_updates_as_the_turn_s_steps_the_step_under_way_as_work`,
  `images_ride_the_turn_as_local_paths_and_one_with_no_path_on_the_machine_is_refus`, `a_turn_s_launch_turns_off_each_server_it_is_told_to_leave_out_and_says_it_leaves`,
  `a_turn_on_a_machine_whose_credentials_the_host_keeps_writes_its_placeholder_logi`; `codex_turn_more.rs` `re_opens_the_run_with_its_channel_replays_the_log_and_writes_nothing_in_answer_t`,
  `a_server_that_exits_on_eof_is_left_alone`, `an_interrupt_after_the_process_is_gone_sends_nothing_and_ends_nothing`,
  `a_stop_before_the_prompt_was_handed_over_sends_no_turn_then_or_once_the_prompt_i`.
- app-server-subagent: `holds_the_lead_s_reply_until_the_child_s_own_turn_completes_and_says_the_child_s`.
- auto-compaction: `says_a_compaction_the_server_ran_by_itself_at_a_turn_s_start_with_what_the_threa`.
- no-login-app-server: `a_turn_with_no_sign_in_as_codex_cli_0_155_1_printed_it_is_the_sign_in_line_with`.
- `app-server-threads-0.162.1.json` (0.162.1's thread list) is replayed by codex_conversations, not a turn.
No recording yet of: a turn on 0.162.1 or later (turns are 0.155.1), a lost session (`no rollout found`), a reconnect run and its
stall, an interrupt mid-command, a file-change approval, a rate-limit update or a usage-limited turn (rate-limit lines were stripped
from the recordings), MCP and web-search items, a 401 under a key, `thread/turns/list` pages, `thread/revert` and `thread/fork`, a
side question, a kept server's second turn, and an attach replay with `from` above 0. Every case for these is hand-written.

## Tests (tests/codex_turn.rs 42, codex_turn_more.rs 41, codex_turn_rules.rs 7 written for this port; TS 84 of adapter.test.ts)
- Rules 1, 21: `codex_turn_rules.rs` `a_turn_joins_the_registry_under_its_local_id_and_a_side_question_never_does`,
  `a_kept_servers_next_turn_resumes_the_thread_with_the_first_turns_options_then_st`; `codex_turn.rs` `resumes_a_thread_by_the_id_the_row_holds_the_registry_key_is_that_id_and_the_see`.
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
- CT1 to CT8: `codex_turn_edges.rs` `the_stderr_tail_keeps_the_last_five_non_blank_trimmed_lines_unreadable_json_among_them` (CT1),
  `an_error_with_no_message_leaves_an_empty_reason_and_a_signal_reads_code_null` (CT2),
  `failure_words_said_before_the_stream_throws_beat_the_stream_s_message_which_rides_session_end` (CT3),
  `a_process_that_goes_mid_turn_cancels_its_asks_then_fails_its_children_then_ends_the_turn` (CT4),
  `a_turn_that_completes_under_an_open_ask_says_turn_done_first_then_cancels_the_ask` (CT5),
  `a_lead_end_with_no_status_or_an_unknown_one_fails_in_the_default_words_and_another_turn_s_end_is_ignored` (CT6),
  `a_thread_or_handshake_the_server_refuses_ends_the_turn_under_its_local_id_with_no_session_start` (CT7),
  `a_lead_that_ended_while_its_child_ran_is_completed_when_the_process_goes_and_its_stdin_never_closed` (CT8). CT9: no test.
- CT10 to CT13: `codex_turn_edges.rs` `a_launched_start_s_model_rides_the_thread_line_but_the_turn_names_the_model_the_server_answered` (CT10),
  `failure_words_said_earlier_beat_the_completed_turn_s_own_error` (CT11),
  `a_later_turn_started_takes_the_turn_id_with_no_new_anchor_and_the_old_turn_s_end_is_ignored` (CT12),
  `an_answer_s_close_carries_the_caller_s_outcome_and_option_and_a_deny_s_reason_is_steered_after_the_decision` (CT13).
- CT14 to CT16: `codex_turn_edges.rs` `the_reconnect_stall_is_cleared_by_any_notification_but_an_error_and_never_by_a_request_a_response_or_stderr`,
  `a_kept_turn_writes_its_unsubscribe_and_says_its_compaction_before_turn_done`, `a_stop_before_the_prompt_is_due_sends_no_turn_and_ends_interrupted`.
