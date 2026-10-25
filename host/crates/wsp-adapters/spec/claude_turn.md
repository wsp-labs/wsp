# Unit claude-turn: one turn's reader and what a caller does to it

TS at a3b870a6e: `packages/adapter-claude/src/adapter.ts:643-1270` (`follow` and the session it returns, about 630
lines). Rust: `src/claude/turn.rs`. Tests: `tests/claude_turn.rs` and `tests/claude_turn_results.rs` (adapter.test.ts,
131), `tests/claude_control.rs` (adapter-permissions and steer-hold, 40), `tests/claude_lines.rs` (tool results),
`tests/claude_restart.rs` (hard cases). Both claude-turn and claude-adapter must be in place for these tests to run:
they drive a turn through `ClaudeAdapter::start`, so the two units are implemented in one packet, turn first.

## Concurrency

The reader runs on a task of its own from the moment `follow` is called (the TS starts the async reader at once; tests
read events before awaiting `finished`). State lives behind one lock. Never call `on_event`, `stream.write` (await),
`end_run` or `end_after_result` while holding it: callers answer prompts and steer from inside `on_event`, and a stream
may answer a write only after a later line. Events reach `on_event` in the order the TS emits them; a write the TS
fires and forgets (`void stream.write(...)`) is started in that order and not awaited by the reader.

## State per turn (adapter.ts:664-772), all reset only by a new follow

sessionId (local id until init), sawInit (true at the start for an attach), sawResult, exited, interruptRequested,
turnResult, emptyResult, stderr tail (last 5 non-JSON non-blank lines, trimmed, stderr after the result included),
harness and shell cwd, backgroundTasks count, running task ids, heldReply and heldAt (ms, deps.now), heldTold, task
names, Steers (from `stream.taken()` and `o.steered`), PlanBook, HandbackBook, heldContext, rejected windows, compacted,
initModel, saved (`o.saved`), totals, drawn message ids, spent, calls by message id, callsCounted, finishedAfter lines,
the settle timer, woken, refusalCause, pending asks, asked requests, askedSeq, launchedBy, subagents, askedBy,
modelSaid, SpendBook, skipsPrompts, anchor, anchored.

## Per line, in this order (adapter.ts:871-1090)

1. A held reply with no background task and nothing busy (woken or a steer working): re-arm the settle timer.
2. Not JSON (parse_line None): push the trimmed line to the stderr tail where non-blank (keep 5); next line.
3. `cost-state`: saved = saved_use_of(line, local_id) (None for another session's or an unreadable line); next.
4. Steers::read true (command_lifecycle): where steers.working, clear the settle timer. Where a reply is held, no
   background task, nothing open and not woken: deliver it (interrupted, with calls after, where a stop was asked;
   else held_with_finished), held flag = heldTold. Next line.
5. A `result` that is not a drained notice: totals = `{costUsd, models_of(modelUsage)}` where total_cost_usd is
   numeric, else None; spent += usage_tokens(usage) where usage is an object; callsCounted = calls.len().
6. A lead's assistant line with a string uuid: anchor = uuid.
7. subagent_prompts → askedBy. note_line(book).
8. A child line (assistant, not an API error, with parent_tool_use_id): the running subagent launched by that call
   gets note_child_call; the first such line naming `message.model` of a running subagent whose model was not said
   emits `subagent{task, state: running, parentToolUseId, model}` once.
9. control_line: ask → under skipsPrompts and not the question tool, write control_allow_line and stop; else map the
   ask's agent id through launchedBy to parentToolUseId, hold it in pending and emit `permission.ask`. cancel → where
   pending, close it `cancelled`. unknown → write control_error_line. answer → settle the asked request with its error
   or `{}`. Next line.
10. note_progress: a task_progress line of a running subagent emits `subagent{running, parentToolUseId?, usage}`.
11. subagent_launch: record launchedBy; where subagent_started, mark running and emit `subagent{task, running,
    parentToolUseId?, title?, depth?, asked: subagent_asked(prompt)?}`, removing that askedBy entry. Next line.
12. background_tasks_of: names recorded, count and ids replaced, emit `turn.tasks{running: count}`; held reply with 0
    tasks and not busy → arm settle; count > 0 → clear settle. Next line.
13. interim_end → next line. task_ended for a running subagent, or a known one with a later end: state set, emit
    `subagent{task, state, parentToolUseId?, summary?, usage: subagent_spent_at_end?}`.
14. Every end_answer delta is emitted. task_finished_of: where a reply is held, push task_finished_line(name or id,
    status, now - heldAt); next line. An ended task (step 13) → next line.
15. No held reply, no result seen and a drained notice → next line.
16. compacted_to: heldContext = after, compacted = true, emit `turn.compacted{before?, after}` (the line still goes on).
17. A call line (assistant, not an API error) with message usage, id and model: calls[id] = {model, tokens: input =
    input_tokens(usage), output = max(seen, output_tokens), cached, cacheWrite, reasoning 0}. First line of each
    message id: emit `turn.usage{tokens: held_tokens(usage), context: the same where it is the lead's own, at:
    Date.parse(timestamp) where it parses}`.
18. A lead's assistant usage (not API error) sets heldContext = held_tokens. rate_limit_event → note_rejected.
19. api_error_cause not NotOne: refusalCause = it; next line.
20. normalize_event with the refusal (`{road: deps.signInRefusal where cause is sign-in, cause}`), each event:
    session.start sets sessionId, initModel (keeps the old where none), sawInit, opens steer, harness and shell cwd,
    skipsPrompts where harness.permissionMode is bypassPermissions, and under a held reply woken = true and clears the
    settle timer. A tool_use delta with both cwds gets `cwd` where shell_cwd_after moves it. turn.done: after a result
    already went, drop it; woken = false; under a held reply a result with num_turns 0 is dropped; result =
    spanned(with_context(own_use(result, saved, fresh, spent), heldContext, initModel)). Then: tasks > 0 and a stop
    asked → write stop_task_line(`wsp-stop-task-<++seq>`, id) for each running task and deliver; a stop asked under a
    held reply and the result's text trims empty → deliver it with the held text; tasks > 0, or a steer open with no
    stop → hold it (heldAt = now, finishedAfter cleared, heldTold = tasks > 0; told → emit `turn.tasks{running,
    replied: result}`, else arm settle where not busy); else deliver. Every other event is emitted as is.

## deliver(reply, sessionId, held) (adapter.ts:804)

result = with_limit(reply, rejected); clear settle; heldReply = None; sawResult = true. Release the keeper where the
status is neither completed nor interrupted, or it answered nothing. queuedLeft = steers.settle(); release on it.
close_input; settle every asked request `gone`; end the run (end_run(grace) where queuedLeft, else
end_after_result(result_exit_ms, grace)) without awaiting. Answered nothing and not compacted: keep as emptyResult and
emit nothing. Else turnResult = result; turn.anchor once (where an anchor was seen); tell_unread; emit
`turn.done{sessionId, result, held: true where held}`.

Held reply helpers: `withCallsAfter` adds the calls printed after the last result as cost-less model entries (tokens
summed, context kept, totals marked unpriced); `heldWithFinished` adds `"\n\n"` + the finished lines (+
background_tasks_line(running) where running > 0 and completed) to the text; `spanned` sets durationMs = held
duration + max(now - heldAt, own duration). The settle timer fires after result_exit_ms and delivers the held reply
(with finished lines) where no task runs.

## The end (adapter.ts:1092-1136)

A stream error ends the loop; its message becomes the error, its failure rides session.end. Clear settle; await
exited; exited = true; close every pending ask `cancelled`; settle asked `gone`. exitLine = the stream error or
harness_exit_line("claude", code, env PATH, {reached: sawInit, signal}). Held reply left: interrupted (cut, timed to
now), woken → failed with exitLine, else held_with_finished(reply, tasks); anchor, unread, turn.done with held where
told and not woken. No result at all: emptyResult → failed with no_output_error(empty, tail); a stop asked →
`{interrupted, ...drew_use}`; else `{failed, error: exitLine, ...drew_use}`; anchor, unread, turn.done. Always last:
`session.end{exitCode, sawResult, failure?}`. `finished` settles with turnResult after session.end.

## The caller's side (adapter.ts:1141-1267)

- running = sawInit and no result, no exit, no stop asked.
- `answer(ask, o)`: unknown ask or exited → Gone. Remove it before the write (a racing second answer is Gone). A
  write that throws puts it back (unless exited) and rethrows; Gone from the write → Gone; else emit
  `permission.close{askId, outcome, optionId}` and Answered.
- `steer(prompt, id, images)`: before init, wait for init, the end, or steer_join_ms; not running → NotRunning; add
  the id; write user_message_line(prompt, sessionId, images, id); written and still running → Accepted; else forget
  and NotRunning.
- `set_access(mode)`: not running → Gone. Request id `wsp-set-mode-<++seq>`; write set_mode_line; not written → Gone.
  bypassPermissions: skipsPrompts = true, answer each pending non-question ask with allow (outcome allowed), Set
  without waiting. Else wait the CLI's answer (gone → Gone, error → Refused, else Set); on Set answer each pending ask
  that offered that mode with its mode option.
- `stop_task(task)`: not running, or a known subagent not running → NotRunning. Request `wsp-stop-task-<++seq>`,
  timer stop_task_wait_ms; a failed write → NotRunning; gone → NotRunning; silence → Refused("it did not answer within
  <fmt_duration(wait)>"); no error → Accepted; error containing STOP_TASK_UNSUPPORTED → Unsupported; else Refused.
- `interrupt()`: live = running; stop asked; open steer. A held reply with no task, nothing busy and a CLI that
  cannot cancel its queue: deliver it interrupted and wait settles_within(finished, grace). Else on a kept process
  that is up and live: write interrupt_line(`wsp-interrupt-<++seq>`, cancels_queued); written and finished settles
  within grace → done. Else release the keeper and end_run(grace).
- `kept()`: only with a keeper that is up and a result seen: next turns run `follow` again on the keeper's next view
  with `fresh: false`, `saved` = this turn's totals, and the user line written at once (or once `after` settles, by
  write_after where the road holds lines); close(now) = keeper.close(now ? 0 : result_exit_ms, grace).

## What survives a host restart

An attach re-reads the run from byte `from` (or 0): the CLI's own lines are the record. `announced: true` makes the
turn take a steer at once. Steers start from the run's input read back (`taken`, only ids after the last uuid-less user
line) plus the ids the row kept (`steered`), so a message whose write was lost is still told unread. heldAt and timing
use this host's clock: a replay takes a moment, and spanned takes the CLI's own figure as the floor. Nothing else is
carried: no saved totals (fresh false, saved None, so the cost of an attached resume is left out).

## Rules against tests

| Rule | Test |
|---|---|
| events in order init / delta / done / end over the recorded turn | `claude_turn::normalizes_the_stream_into_session_start_delta_done_end` (ported) |
| held reply over background tasks, delivered once, woken agent, stop | `claude_turn_results` "a reply given while the agent's background work runs" (ported, 14) |
| permission asks, answers, races, bypass mid-turn, mode options | `claude_control` adapter-permissions (ported, 26) |
| steer before init, after result, gone, unread on stop | `claude_turn` steer cases and `claude_control` steer-hold (ported) |
| cost off saved totals, split by model | `claude_turn_results` "what a turn cost" (ported) |
| exit words: signal, 127, never reached, transport line | `claude_turn_results` result classification (ported) |
| an attach takes a steer at once and tells a kept steer unread | `claude_restart::an_attached_turn_tells_a_kept_steer_unread` |
| an attach replays the log without launching and keeps the run handle | `claude_turn::attaches_to_a_run_an_earlier_process_launched` (ported) |
| on_event may answer a prompt from inside the callback | `claude_restart::a_prompt_answered_from_inside_on_event_closes_once` |
| a stream that throws ends the turn with its words and carries the failure | `claude_restart::a_transport_failure_rides_session_end` |
