# threads_owed: lines into threads, kept until a turn takes them, and the cut end

TS: `T/threads.ts:24-62, 586-955` at `65f6b89c7`. Rust: `src/threads/threads/owed.rs`, methods on `ThreadsArea`.
State (memory only): `held_lines: IndexMap<workspace, Vec<OwedLine>>`, `sending: IndexMap<line id, OwedLine>`,
`prompt_waits: IndexMap<request id, Rc<dyn Fn()>>`, `dark_tries: IndexMap<turn id, Rc<dyn Fn()>>`, `steers_out`
(threads_turn_end). `OwedLine = Rc<RefCell<Owed>>`; "carried" (`:685`) is `!ctx.closing() && sending[line.id]` being
the same `Rc` as this delivery's line. Constants: `OWED_RETRY_MS` 30,000, `OWED_RETRY_MAX_MS` 600,000, `OWED_FOR_MS`
3,600,000, collection `notify-owed`, `NOTIFY_ME` = `"me"`. `X` and `Y` below are `from` and `notify` cut to 8
characters.

## The stored line

O1. `Owed` (`:31`) is `{id, from, notify, text, by?, road?, startedBy?, toPerson?, since?, requestId?, tries?, next?,
images?, awaitsResult?}`; `toPerson` is `{workspaceId, sessionId, turnId, text}`; `images` is `{under, records}`.
`read_owed(raw)` (`:45-62`) answers `None` unless `id`, `from`, `notify` and `text` are strings; every other field
is kept only when it reads (`by` through `readScope`, `road` through `readRoad`, `startedBy` as a `SessionOrigin`,
`toPerson` with all four strings, `since`, `tries`, `next` as numbers, `requestId` and `awaitsResult` as strings,
`images` with a string `under` and a non-empty list of attachment records), and dropped silently otherwise. Its key
order varies with the line's history in the TS (each write spreads the line and adds keys last); the store row is this
host's own, so tests compare it as a JSON object, not as bytes.

## deliver(line, woke) (`:677-850`)

O2. Synchronously, in this order: `sent` is the line with `since = line.since ?? now` and `requestId = line.requestId
?? ids.uuid()`; `sending[line.id] = sent`; `deadline = since + OWED_FOR_MS`; when the line lacked `requestId` or
`since`, `sent` is put to `notify-owed` (a failure warns `the line of thread X into thread Y was not kept: <message>`)
and that put is what the task below waits on first.

O3. `parent = latest_on(notify)`. When it is not tellable (no row, or no `claudeSessionId`): warn `thread X ended, but
thread Y has no session to tell`, the line falls (O10), stop.

O4. Held for the wake (`:705-711`): when the parent's workspace is live and its phase reads anything but `running`
through `workspaceState` (pausing, napping, waking, queued, gone, released), `sent` is appended to
`held_lines[parent workspace]` and nothing else happens now. A workspace not live is not held.

O5. A line whose `road` is `paired` falls at once, since a paired device may not start a turn (`DEVICE_OPS` has no
`sessions.start`): warn `thread X ended, but its line was registered from a paired computer, which starts no turn on
thread Y`.

O6. The caller the start runs as is `road` (possibly none) when `by` is absent, else `{origin: road ?? here, by}`.

O7. Then one detached task (a failure anywhere in it removes `sending[line.id]`, warns `the line of thread X into
thread Y was not sent: <message>`, and leaves the row in the store):
1. await the put of O2;
2. with `awaitsResult`: `tail = ctx.sub_line(awaitsResult, since).await`; stop if no longer carried; a tail is added
   as `text + "\n" + tail`; `awaitsResult` is removed; the line is put again and awaited;
3. only when the line came in **with** a `requestId` (a try went before, here or on an earlier host): await
   `ctx.boot_work(parent workspace)` when one stands; stop if not carried; when a turn took the request (O13) the
   line is taken (O11) and stop. `out = carrying(notify, requestId)` (O14). With one: when `out` was launched under the
   request and holds a handle, settle that try (O8); when launched under it with no handle and no launch in flight,
   settle it as dark (O8, a computer that has not answered since the restart); otherwise wait for the try to be over:
   its handle's finish (over), else its launch (not over), else `OWED_RETRY_MS` on the clock (not over). Stop if not
   carried. When the message was steered and the turn is over: taken where `handle.tells_unread() == Some(true)`, else
   warn `thread X ended, and nothing says whether thread Y read its line` and fall. Otherwise `deliver(sent)` again
   (a fresh delivery: the old one stops being carried).
4. a line that failed before (`tries > 0`) and is not delivered by a wake: past its deadline, give up (O9) with `its
   last try failed`; with `next` still ahead, `deliver(sent)` again at `next` (only if still carried) and stop;
5. the start: images read back (O15) when the line carries some; `ctx.start(parent workspace, {prompt: text,
   attachments (only when any read back), harness: parent row's harness, thread: notify, startedBy: line.startedBy ??
   agent, requestId, owed: true, asksUntilStopped: true while now < deadline, wakesLead: true when line.startedBy is
   absent}, caller)`. An outcome `steered` is taken; any other is settled (O8). A failure: held for the wake when O4
   holds now; a silent computer (`computer_silent`) tries again (O9); anything else warns `thread X ended, but its line
   did not reach thread Y: <message>` and falls.

O8. Settling a try (`settleTry`, `:745-789`): `prompt_waits[requestId]` takes the line when called while carried;
when a turn took the request already, the line is taken (and the settle goes on). A dark try registers
`dark_tries[turn]`, which cuts it short as `interrupted`. A timer at `max(deadline - now, 0)` is set when time is left
or the try is dark: at it, nothing if not carried or the request was taken; else it marks the hour, and a dark try is
cut short `failed` with `nothing answered for the turn carrying it since the host restarted`, any other is stopped by
`ctx.stop_try(handle id)` (a failure warns `the try of the line of thread X into thread Y was not stopped:
<message>`). The try's end is the first of: the cut, the handle's finish, and the row's own `session.end` (O16) where
the row `handle id` still holds that turn; a failed finish reads `{failed, error: <message>}`. Then the timer is
cancelled, and: stop if not carried; taken when a turn took the request; `said` is the end's error or `the turn ended
<status> before its agent started`; at the hour give up with `said`; `interrupted` warns `thread X ended, but the turn
carrying its line into thread Y was stopped` and falls; held for the wake when O4 holds; a silent failure of the
turn's stream (`failure_of`) tries again; anything else warns as O7.5 and falls. Both maps lose their entries at the
end whatever happened.

O9. Trying again (`:728-739`): `left = deadline - now`; at or below 0 give up. Else `tries += 1`, `wait = min(30000 *
2^(tries-1), 600000, left)`, `next = now + wait`, the line is put (detached), warn `thread X ended, but its line did
not reach thread Y yet, and goes again in <wait/1000 as JavaScript prints it> s: <said>` (`12.345`, `30`, `600`),
and `deliver(sent)` at `wait` if still carried. Giving up warns `thread X ended, but its line did not reach thread Y
within 60 minutes: <said>` and falls.

O10. Falling (`:686-689`): the line leaves `sending` and the store (a failed delete warns `the line of thread X into
thread Y stays owed: <message>`), then the person is told (O12).

O11. Taken (`:692-696`): nothing when the line is no longer this delivery's; else it leaves `sending` and the store,
and when `startedBy` is absent (a child's finished line) `ctx.mark([from], {readAt: now}, None)` runs detached, a
failure warning `thread X was not marked read: <message>`.

O12. Telling the person (`tellPerson`, `:603-612`): nothing without `toPerson`. Else `record({type: session.notify,
workspaceId, sessionId, turnId, text, threadId: from, notify: me})` in that key order, then every other line in
`sending` whose `toPerson` has the same `turnId` and `text` loses its `toPerson` and is put again (detached), so one
end is told once.

O13. `took_request(ws, request)` (`:622-626`): the index's `taken[request]`; none is false; `steered` is true; a start
is true unless a row of that turn still `awaitsPrompt`. `line_try(s)` (`:629-633`): false unless the row's `asked` is
`owed` with a request id; then true when the index's taken turn for it is not this turn or the row still awaits its
prompt.

O14. `carrying(thread, request)` (`:655-656`): the first running row of the thread whose `asked.requestId` is the
request or whose `turnLive.steered` holds a message under it.

O15. Images read back (`keptImages`, `:852-860`): after `ctx.kept_writes(ws)` settles, each record by index: key
`attachmentKey(thread, under, index)`, its `attachments` blob; one with no key or no blob is left out; each kept one
is `{mediaType, bytes: base64, name?}`, in record order.

O16. `row_ended(s)` (`:645-652`): the first `session.end` for the row's turn answers `{status: interrupted when the
row reads interrupted then, else failed, error: reason?}`.

## Who delivers

O17. `send_back(s, ids, stopped)` (`:870-890`): nothing for a row with no thread. For each id: one whose write is
still out (`steers_out`) is marked ended with the row and `stopped` and left to that road; one the row did not keep
is skipped. Else `toPerson = {workspaceId, sessionId: claudeSessionId ?? row id, turnId, text: unreadLine(prompt)}`;
stopped records `session.notify` as O12 with the thread as `threadId`; else `deliver({id: <turn>:unread:<id>, from:
thread, notify: thread, text: prompt, startedBy, by?, road?, toPerson, images? {under: requestId, records:
attachments} when both are set})`.

O18. On `workspace.woken` and `workspace.upgraded` (listeners at construction, `:892-899`) every line held for that
workspace is delivered with `woke`, in the order held, and the hold is emptied.

O19. `notify_end(s, notify, named, result, open)` (`:905-923`): nothing for a row with no thread. A target is
reachable when it is `me` or its `latest_on` row is tellable; with every target reachable the targets are `notify` as
given, else the reachable ones with `me` added last when absent. `toPerson.text` is `notifyLine(thread, result,
tail)`. For each target in order: `record({type: session.notify, workspaceId, sessionId: claudeSessionId ?? row id,
turnId, threadId, notify: target, text})`, `text` being the tail for `me` and the whole report for a thread; then for
a thread target `deliver({id: <turn>:<told or 0>:<target>, from: thread, notify: target, text, by?, road? (named),
toPerson only when me is not among the targets, awaitsResult: <workspace> when the row's workspace record has a sub
block and open is not set})`.

O20. `deliver_owed()` (`:925-930`): every `notify-owed` row in store order that reads (O1) and is not in `sending` is
delivered. A store that fails to list fails the call.

## settle_cut(row, reason, cut_line, stopped, unreached) (`:936-955`)

O21. Changes the row in `sessions` it is named by (trap #1646), in order: `reply = turnLive.reply`; `snapshot`
removed; `endedAt = now`; status is `reply`, else `interrupted` when stopped, else `failed`; with no reply the row's
lines are written as `turnLines({status}, reason)`; `ctx.port_roots_moved(ws)`; `view.endedAt` set; a `failed` row
calls `ctx.end_snooze_for(view)`; `view.asking` removed and the thread's `leadAsks` entry dropped. `cut = {status:
interrupted when stopped else failed, error: cut_line(endedAt), unreached: true only when stopped and unreached}`.
With no reply, a `notify` list, and not a line's try: `notify_end(row, notify, tell_as(row), cut)`. With no reply and
stopped: `record({type: session.done, workspaceId, sessionId, turnId, threadId, result: cut})`. Always:
`record({type: session.end, workspaceId, sessionId, turnId, threadId, exitCode: null, sawResult: reply set or
stopped, reason, promptless: true only when asked.awaitsPrompt})`. `sessionId` is `claudeSessionId ?? row id`.
No borrow of `sessions` is held across a record.

## Restart

O22. A line survives a restart in `notify-owed` with `since`, `requestId`, `tries` and `next`: the next host's
`deliver_owed` keeps its spacing and its hour (O7.4), asks the same request (O7.3), and waits on the boot's re-open
before reading the try.

## Tests (`tests/threads_owed.rs`)

| Rule | Test |
|---|---|
| O1 | `read_owed_needs_the_four_strings_and_drops_a_field_that_does_not_read` |
| O2, O11 | `deliver_writes_since_and_request_before_the_first_try_and_takes_the_line_once_started_on` |
| O3 | `deliver_to_a_thread_with_no_session_falls_to_the_person` |
| O4, O18 | `a_line_into_a_napping_workspace_is_held_and_goes_at_the_wake`, `a_held_line_goes_at_a_rebuild_too` |
| O5 | `a_line_registered_from_a_paired_device_falls` |
| O6 | `the_start_runs_as_the_thread_that_named_the_target_and_its_road` |
| O7.2 | `a_sub_threads_line_waits_for_its_done_and_carries_its_tail` |
| O7.3 | `a_line_with_a_request_asks_the_turn_already_carrying_it`, `a_steered_line_whose_turn_ended_unread_and_cannot_say_falls` |
| O7.4, O22 | `a_restarted_line_keeps_its_spacing_and_its_hour` |
| O7.5 | `a_steered_start_takes_the_line`, `a_refused_start_falls_with_its_words` |
| O8 | `a_try_stopped_at_the_hour_gives_up`, `a_dark_try_is_settled_at_the_hour_without_its_turn`, `a_try_the_person_stopped_falls` , `a_try_that_failed_silent_goes_again` |
| O9 | `retries_double_from_thirty_seconds_to_ten_minutes_within_the_hour`, `the_retry_wait_prints_seconds_as_javascript_does` |
| O10, O12 | `a_fallen_line_tells_the_person_once_per_end` |
| O13 | `took_request_reads_the_index_and_a_prompt_not_yet_held`, `line_try_is_the_lines_own_turn_until_its_agent_holds_it` |
| O15 | `images_go_back_with_the_line_and_a_missing_one_is_left_out` |
| O17 | `send_back_redelivers_an_unread_steer_as_its_starter`, `send_back_of_a_stopped_turn_tells_the_person`, `send_back_leaves_a_steer_whose_write_is_out_to_that_road` |
| O19 | `notify_end_records_a_line_per_target_and_delivers_the_whole_report`, `notify_end_adds_me_when_a_target_has_gone`, `notify_end_names_the_agents_session_else_the_row` |
| O20 | `deliver_owed_sends_every_readable_line_not_already_carried` |
| O21 | `settle_cut_changes_the_row_it_is_handed`, `settle_cut_of_a_replied_turn_keeps_the_reply_and_sends_no_line`, `settle_cut_of_a_stopped_turn_records_its_done_before_its_end` |
