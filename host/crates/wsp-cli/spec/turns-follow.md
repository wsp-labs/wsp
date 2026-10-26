# turns_help/follow: starting a turn, following it, waiting on threads, restarting the host

Port of `packages/host/src/verbs/turns-help.ts:520-863` at 65f6b89c7 (`begin`, `sent`, `redialUnanswered`,
`startDetached`, `follow`, `waitThrough`, the restart words and `restartHost`, `withSubResult`, `endedOf`, `history`,
`readThread`, `readLine`, `threadHead`, `headLine`, `firstEnded`). Rust: `src/verbs/turns_help/follow.rs`. Tests:
`tests/cli_c/turns_follow.rs` (167 recorded cases, generated from `tests/fixtures/cli_c/turns_follow.json`),
`turns_follow_rules.rs` (30 named tests), `drivers_turns_follow.rs`. Expected values were made by running the TS
(`record/turns_follow.probe.ts` with `TZ=UTC`), never by reading it. The TS tests that name these helpers:
`send-waits.test.ts` (6, carried by `turns_stream`), `verbs-exec-host.test.ts:105` (`firstEnded` of a gone thread, case
"a named thread the listing no longer holds"), `verbs-exec-host.test.ts:497,505` and `verbs.test.ts` (`hostRestartedLine`,
cases of the same name); the rest of those files run on a fake runtime and are reached through the verbs' tapes.

## Behaviour

- **A start** (`begin`, turns-help.ts:537). `client.events()` first (one `events.subscribe` per socket, never a second).
  Then `sessions.start` with the start's fields in the order given **without** `requestId`, then `startedBy`, `requestId`,
  `answerHeld: true`. `session.queued` and `session.steer` frames whose `requestId` is this start's are heard while the
  start is under way (`news.queued`, `news.steered`); a frame with another `requestId`, or none, is not. `session.starting`
  is heard until a `session.start` or `session.steer` frame for this request arrives, or (after the reply) the named turn's
  `session.end`; a `starting` that follows any of them is not heard. A reply that is not a `SessionStartResult`
  (missing or unknown `outcome`, no `session`) fails `other_version("sessions.start")`; a session with no `threadId` fails
  `the runtime stamped no thread on the session`. The turn is `{session, threadId, outcome}` plus `notice` where the reply
  carried one.
- **The request id** (`sent`, turns-help.ts:581). One v4 UUID per start call (`startDetached`, `follow`), minted before
  the first send and kept for every send of that start, so a start sent again after the host stopped is the same message
  to the host that comes back. It is never sent in the start's own fields (`requestId` is last of the four added).
- **A host that stopped before it answered** (`redialUnanswered`, :588). Only a socket the host let go as it stopped
  (close code 4001) is dialled again; any other failure (a refusal, a close with another code, no code) stands as it is.
  A stopped socket with no `redial` fails `NOT_DELIVERED_LINE`; with one, `redialed` is called and `redial()` answers the
  new socket; a refused dial is also `NOT_DELIVERED_LINE`. The loop goes on while sockets stop under the start (a third
  dial after a second stop), and ends with `NOT_DELIVERED_LINE` when the dial runs out.
- **`startDetached`** (:601): answers the turn as `begin` named it; only `queued` news is heard. **`follow`** (:624): the
  start gets `followed: true` after its own fields; held frames (`PushedFrames`) are taken before the start is sent, then
  every `session.*` frame (a string `type` starting `session.`) whose `turnId` is the reply's `turnId` goes to the turn:
  `session.start` with `afterCut == true` sets `turn.after_cut`; `session.done` sets `turn.result`; `session.end` with a
  `reason` sets `turn.reason`; `on.event(e, turn)` hears it after the turn is updated; the first done or end settles the
  follow and nothing after it is handed on. Frames of another turn id, and frames that are no session event, are left out.
  `on.started(turn)` is handed the live turn the moment it is named (before any event). Held frames are stopped on every road out.
- **A host that stopped under a named turn** (:660-680): socket closed with 4001 -> `on.redialed()`, `redial()`, then on
  the new socket `events()`, a fresh hold, and `sessions.history {workspaceId, threadId, whole: true}` read for a done or
  end of this turn (`turnId`), taken as events; if one settles the turn it is returned, else the follow goes on there. A
  socket that closed with another code, or any socket with no `redial`, fails `close_words()`. A dial that fails
  fails with its own message. A new socket's own close under the follow starts the loop again (three hosts in one follow).
  Also: after a redial for an *unanswered start* the transcript is read on the new socket although the start was only just
  accepted (`socket !== client`): pinned by the three cases "dialled again ..." (see unclear).
- **`firstEnded`** (:799). `events()`, then the listing (`threads()`, folded `sessions.list`), in that order, so an end
  between them is a held frame. A named thread the listing lacks is over: answered through `endedOf(.., {status: failed})`
  (checked first, before any over thread). Else the first of `named` (in the order named) whose listing row is not
  `running`: answered through `endedOf` with its status. Else the first `session.done` or `session.end` pushed for a named
  thread id (a frame with no `threadId`, a `threadId` not named, or a non-session frame is ignored): a done answers
  `{threadId, result}` as pushed (no transcript read); an end is answered through `endedOf` with the workspace *the end
  names* and `{status: failed}` plus `error: reason` where the end carried one. `endedOf` is the transcript's own reading
  (`thread_result(history)`), else the status given. A deadline (`timeout_ms`, 0 allowed) passing first is `TimedOutMs(ms)`
  (the given number, not elapsed). The socket going first fails `close_words()` (4001 says the turn goes on). No deadline and
  nothing to hear waits for ever. An empty `named` waits too.
- **`waitThrough`** (:685): `deps.client()`, `firstEnded`, then `withSubResult` (a named thread that is a sub-thread: one
  `sessions.head {threadId: row.id, afterRead: true}`, its `facts.sub.result` added as `sub` where there is one; a thread with
  no `sub` asks nothing). A 4001 stop: `redialed()`, then `hostBack(deps, min(host_wait_ms or SERVICE_WAIT_MS, left))`,
  the deadline kept across the dial (`left = max(0, until - now)`). A deadline that has passed when the dial finally fails is
  `TimedOutMs(timeout)`; any other refusal stands; a close with another code is not dialled again.
- **`restartHost`** (:718): `host.restart` on `deps.client()`, then waits for that socket to close (any code) or
  `SERVICE_WAIT_MS` (20 s, wall clock; the one slow test) -> `host_did_not_stop_line(20_000)`; then `host_back(deps)` and
  the ids (`thread_id_of`) of the listing's `running` threads in the listing's order. A refusal of `host.restart` is the road's
  own words (kind kept) and nothing is dialled; a socket that closed before the answer fails `close_words()`.
- **`readThread`** (:765): `sessions.history {workspaceId, threadId, whole}` first, then `sessions.read {threadId: thread.id}`
  (the stamp moves only after a transcript was read), then the messages (`thread_messages`, or with `last` `thread_reply_rows`).
  `threadId` is `thread_id_of` (`threadId` else `id`). `readLine` (:774): no messages is `no_reply_line` (with `last`) or
  `no_messages_line`, else `thread_read_text`; Rust takes the person's UTC offset as `offset_seconds_at` (the tests pass UTC).
- **`threadHead`** (:778): `sessions.head {threadId: thread.id}`; only `facts`, `events`, `pos`, `total` kept, in that order.
  `headLine` (:785): see the doc comment; `the newest <events.len()> of <total> events` counts the head's events, not messages.
- **Words**: `host_did_not_stop_line(ms)` uses `fmt_duration` (0 ms reads `1ms`; 1000 ms `1.0s`; 20000 `20s`; 61000 `1m 1s`;
  3600000 `60m`). `host_restarted_line` takes the first 8 characters of each id, `1 thread is`, `<n> threads are`, `no thread is
  running on it`.

## Limits and caps

`SERVICE_WAIT_MS` 20 000 ms (restart's stop wait); the host-back window is `deps.host_wait_ms` (tests give 200 ms) or
20 000 ms; dials poll every `POLL_MS` (200 ms, client.ts `hostAgain`); an id is cut to 8 characters in the restart line.

## Null and empty inputs

Empty transcript (`events: []`), empty listing, empty `named`, a start with no optional fields, a reply with no `notice`, a
thread with only an `id`, a deadline of 0, no deadline, a done with no text, an end with no reason, a `session.start` with
`afterCut: false` (not a cut), a frame with no `type`, with no `turnId`, with a non-string `type`.

## Order

`events.subscribe` -> `sessions.start`; `events.subscribe` -> `sessions.list` -> (`sessions.history` | nothing) ->
(`sessions.head` for a sub-thread); `host.restart` -> (socket closes) -> dial -> `sessions.list`; `sessions.history` ->
`sessions.read`. Callbacks: `queued`/`starting`/`steered` before the reply, then `started`, then each `event`, `redialed`
at the moment of the dial.

## Invariants held

Rule 10.19 (a graceful close leaves the code 1005): no close is sent by these helpers. Rule 10.8 is not reached. Host
invariants 1-8 of `packages/host/AGENTS.md` do not reach this file (it sends ops to a host and holds no keys, links, config
rows or guest caps); the exit contract's class rule is held by `turns_stream` (the failure's class is the error's own stamp).

## What the TS leaves unclear

- After a redial for a start the old host never answered, `follow` reads the new host's transcript (`socket !== client`)
  although the start has only just landed there; a host that refuses that read fails the follow (case "dialled again, and
  the transcript read is refused").
- `restartHost` waits the full 20 s when the host answered and did not close; the wait is wall clock and not injectable.
- `firstEnded` for an empty `named` waits for ever (no deadline) or until the deadline.

## Rule -> test

| Rule | Recorded cases (helper / case) | Named test (`turns_follow_rules.rs`) |
|---|---|---|
| frames go out in order, requestId last of four | startDetached "the fields go out in order..." ; follow "the start's followed mark goes last..." | `the_start_goes_out_with_its_fields_then_started_by_request_id_and_answer_held`, `a_follow_adds_its_followed_mark_after_the_starts_own_fields` |
| a v4 id, the same one resent, a new one per start | startDetached "the host stops under the start, dialled again" | `a_start_carries_a_v4_request_id_and_goes_again_with_the_same_one`, `two_starts_mint_two_request_ids` |
| one subscription per socket | follow "the host stops under the turn twice" | `events_are_subscribed_once_before_the_start_on_each_socket` |
| only a stopped host is dialled again | startDetached "...another code...", "the host refuses then is not asked again" | `a_socket_the_host_did_not_stop_is_not_dialled_again`, `a_start_the_host_refused_is_never_sent_again` |
| no road back is not delivered | startDetached "no way back", "the dial is refused", "then no second dial is left" | `no_road_back_is_a_message_not_delivered_never_a_turn_that_goes_on` |
| a named turn's stop says the turn goes on | follow "the host stops under the turn, no way back" | `a_host_that_stops_under_a_named_turn_says_the_turn_goes_on_where_nothing_can_dial_again`, `the_dial_that_fails_after_the_turn_is_named_fails_with_its_own_words` |
| transcript read on the new host | follow "...transcript holds its done / its end / another turn's done / pushes the done / has nothing" | `a_follow_on_a_new_host_reads_the_whole_transcript_of_its_thread`, `an_end_already_in_the_transcript_ends_the_follow_with_its_reason` |
| held frames, turn-id filter, first end settles, afterCut | follow "a turn that completes", "...the end after it is not handed on", "another turn's done...", "a start of another turn...", "a frame that is no session event...", "a done after an end..." | (whole-case compare: frames, said lines, turn) |
| queued / steered / starting news by request id | startDetached and follow "queued...", "a wait with another request id...", "a starting after the session started...", "...after the message steered...", "...after the turn's own end..." | (whole-case compare) |
| subscribe before list; gone before over | firstEnded all 30 cases | `first_ended_subscribes_before_it_lists`, `a_thread_the_listing_lost_is_answered_failed_off_its_transcript_alone`, `a_gone_thread_is_answered_before_one_that_is_over`, `an_end_reads_the_transcript_of_the_workspace_the_end_names` |
| deadline answers the number given; none waits | firstEnded "the deadline passes first", "a deadline of zero", "no deadline...", "no thread named..." | `a_deadline_that_passes_answers_the_deadline_never_an_error`, `no_deadline_waits_for_ever` |
| sub-thread result after the read | waitThrough "a sub-thread's end ..." (2) and "...is not named a sub-thread..." | `a_sub_thread_end_asks_for_its_machine_after_the_read`, `a_thread_that_is_no_sub_thread_asks_nothing_more` |
| dial again through a wait, deadline kept | waitThrough "the host stops under the wait", "...twice", "...dial fails once", "...never comes back..." (3), "...with another code" | `wait_through_says_redialed_once_for_each_stop`, `a_deadline_that_passes_while_the_host_is_coming_back_is_the_deadlines_answer`, `a_socket_closed_with_another_code_is_not_dialled_again_by_a_wait` |
| restart: close, dial, list; order of ids; 20 s | restartHost all 10 | `restart_dials_again_only_after_the_host_closed_and_lists_nothing_else`, `restart_names_the_running_threads_in_the_listings_order`, `a_restart_the_road_refuses_stops_nothing_and_dials_nothing`, `a_host_that_took_the_restart_and_kept_serving_is_told_after_twenty_seconds` |
| read: transcript, then stamp | readThread all 10, readLine 5 | `a_read_reads_the_transcript_before_it_moves_the_stamp`, `a_read_reads_one_threads_whole_transcript` |
| head keeps four parts; head line | threadHead 4, headLine 3 | `a_head_keeps_the_hosts_four_parts_and_nothing_else` |
| words: durations, ids cut to 8 | hostDidNotStopLine 7, hostRestartedLine 5 | (whole-case compare) |
