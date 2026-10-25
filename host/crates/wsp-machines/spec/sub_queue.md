# sub_queue

Source: `packages/runtime/src/machines/sub-queue.ts` (498 lines) at `65f6b89c7`. Tests:
`packages/runtime/test/sub-queue.test.ts` (19 cases), `sub-start-events.test.ts:101-150` (a failed first start, 2).
Rust: `src/machines/sub_queue.rs`; the area is `SubQueueArea`, built by `SubQueueArea::new(ctx)`.

## What it does

Every sub-thread spawn goes through this queue. Its record is stored `queued` with no machine, under its lead, until
its thread's first turn arrives (`hold_queued`), and past that while its cloud has no machine slot or no start left
this minute; its row reads Waiting with the count it waits on (`capped`). Spawns start first in, first out across the
host: only the head of the line moves, when a machine frees a slot, a setting moves, or a timed read of the cloud's
limits shows room. The held start is written to the `sub-starts` collection, so a restarted host puts it back in line
(`resume_queue`). A stop drops a held spawn with no start spent.

## State (memory only)

`held: ws -> Held` (:51), `last_wait: ws -> ThreadCapWait` (:53), `starting: ws -> {place_id, posted}` (:58),
`refused_until: ws -> ms` (:61), `queued_at: ws -> ms` (:64), the single pump in flight with its `again` flag
(:303-304) and one timer (:323). `IndexMap` with `shift_remove`. Durable: the record's `phase`, `queue {since, why}`,
and the `sub-starts` row (`SubStartRecord`).

## Behaviour

1. Constants (:21-25): `QUEUE_SLOT_READ_MS = 30_000`, `QUEUE_MINUTE_READ_MS = 10_000`, `QUEUE_START_WAIT_MS =
   30_000`. `KEPT` (:29), in this order: prompt, harness, model, effort, permissionMode, access, contextWindow, fast,
   startedBy, requestId, attempt, title, cwd, attachments.
2. `queued()` (:66-67): live entries with phase `queued` and a `queue`, sorted by `queue.since` with `collate`
   (`localeCompare`, rule 8.8), ties by id with `collate`.
3. `slots(place)` (:70-78) gives `{placeId, place, running, atOnce, why: "slot"}` in that key order: `place` is the
   place door's `name_of`, else `provider_key_name(place)`; `running` counts live records on the place that
   `holds_slot`, plus `starting` entries on the place whose id is not live; `atOnce` is the door's `machines_at`, else
   `CLOUD_CAP_DEFAULT.machines`.
4. `wait_on(place, limits)` (:82-92), first match: `running >= atOnce` gives the slots; limits with both
   `machinesAtOnce` and `activeBoxes` and `active >= atOnce` gives `{...slots, running: active, atOnce}`; a minute
   window with `used + count(starting on place, not posted) >= limit` gives `{...slots, running: used, atOnce: limit,
   why: "minute"}`; else none. Spreads keep `slots`' key order.
5. `queue_wait(place, limits)` (:96-101): `wait_on`, else the `last_wait` of the first `queued()` record on the place
   that has one, else none.
6. `claim_start(ws, place)` (:106-115) inserts `{place, posted: false}` in the same synchronous step; `posted()` sets
   `posted` when the entry still exists; `release()` removes it. A take-and-release helper (rule 3.9).
7. `cloud_full(place, limits)` (:119-122): `slots` with `running` and `atOnce` both the plan's `machinesAtOnce` where
   the read names one, else `slots` unchanged.
8. `enqueue(record, wait, refused)` (:127-141). `line` is the parsed `since` of every `queued()` record. `since` is
   `line[0] - 1` ms for a refused record with a non-empty line, else `max(now, line.last + 1)` (an empty line reads
   0), written as `toISOString()` (`YYYY-MM-DDTHH:MM:SS.sssZ`). A refused record gets `refused_until = now + 30_000`.
   Then `queued_at = now`, phase `queued`, `queue = {since, why: wait.why ?? "slot"}`, `machineId ""`, `last_wait` when
   a wait is given, `attach_queued`, persist, emit `{type: "workspace.created", workspace: view(record)}`, and a pump
   started, not awaited.
9. `attach_queued(record, wait)` (:143-148) attaches `absent_machine("", "sandbox", f)` whose every use fails with
   `cap_wait_line(wait)`, or `<name> is waiting for a machine` with no wait.
10. The held row (:151-170). View keys in order: `id` (turn id), `workspaceId`, `harness` (opts' or `""`), `status:
    "running"`, `startedBy` (opts' or `"person"`), `threadId`, the tree's keys, `prompt: kept_opening(opts.prompt)`,
    `startedAt: Date.parse(at)`, `harnessTitle` and `titleSource: "person"` with a title, `capped` with a wait. The
    entry is `{view, turnId, launch: gone, queued: true, notify?, notifyBy?, notifyRoad?}`, set into `sessions` by turn
    id; then emit `{type: "session.held", workspaceId, threadId}`; with a wait, `record` a `session.capped` event
    `{type, workspaceId, sessionId: turnId, turnId, threadId, ...wait}`.
11. `hold_queued(at, origin)` (:183-218): refuses `sub_waiting_refusal(name)` when a start is held for the record. The
    kept opts are the `KEPT` keys present in `at.o`, in `KEPT` order (`onHeld` is never kept). The record is
    `{workspaceId, threadId, turnId: <new uuid>, at: <now ISO>, opts, tree, notify?, by?: scope_of(origin), road?:
    road_of(origin)}`. The row is shown at once (with the last wait if any). The store write runs in the background:
    on success the start is `written`, the last wait (if any) is told through `onHeld({view, turnId, wait})` once (the
    `tell` removes itself), and a pump starts; on failure the spawn is dropped as failed with `the held start was not
    written down: <message>`. The future answers the handle `sessions.start` gives once let through, or fails.
12. `forget(ws)` (:220-225) removes the record from `held`, `last_wait`, `refused_until`, `queued_at`.
13. `tell_failed` (:228-230): only with `notify`; `notify_end({view, turnId}, notify, tell_as({notifyBy?,
    notifyRoad?}), {status: "failed", error: reason})`.
14. `drop(h, reason, failed_spawn)` (:234-245): nothing when `h` is no longer the held entry. Then `forget`, tell the
    targets only for a failed spawn with a row, remove the row, `left()`, fail the caller with `reason`, delete the
    `sub-starts` row, then `ctx.drop(ws)` (the record goes).
15. `fail_first(h, record, e)` (:250-276): `why = failure_words(e)`; `forget`; phase `released`, `machineId ""`; with a
    prompt, `sub.opening = prompt` and `sub.spawn.asked = prompt` where a spawn exists; `attach(dead_machine(""))`;
    persist; on the row: `status "failed"`, `failure: listed_failure(why)`, `endedAt: now`, `capped` and `launch`
    removed, targets told, `persist_sessions(ws)`; then `push_head(thread)`, `rows_sent()` awaited (the failed row is
    out before the refusal, rule 3.12), `left()`, delete the `sub-starts` row, fail the caller with `e` itself.
16. `drop_queued(turn)` (:280-285): true after stopping the held start of that turn with `SUB_DROPPED_LINE`, else
    false. `stop(h, reason)` (:287-290): a record whose `queue` is gone (its machine is being made) only marks
    `h.stopped = reason`; otherwise `drop(h, reason, false)`.
17. `delete_queued(ws)` (:294-298): `None` with no held start; otherwise a future that stops it with
    `deleted_before_start_line(name)` and then waits for `gone` (the start left the queue).
18. `pump()` (:305-321): one pump at a time; a call while one runs sets `again` and answers the running one. The pump
    loops `pump_once` while it answers true, and again while `again` was set meanwhile. A failure warns `the queue of
    held spawns did not move: <failure_words>` and is swallowed.
19. `arm(ms)` (:324-330) replaces the one timer; firing clears it and pumps.
20. `pump_once` (:332-384), in order:
    a. A record with no held start whose `queued_at` (else its parsed `since`) plus 30,000 ms has passed is dropped
       (warn `workspace <id>: its first turn never came, so its queued spawn is dropped`, `forget`, `ctx.drop`), one
       per call, answering true.
    b. No held head: cancel the timer, arm at the earliest due of the startless records when any, answer false.
    c. A head with `refused_until` in the future arms for the rest and answers false; else `refused_until` is removed.
    d. The limits are read (`backend_for(record).limits()` where offered, a failure reads none). If the head is no
       longer the live entry or its held start changed during the read, answer true.
    e. A wait: for every queued record on the place, in line order, with a held start whose last wait differs in
       `running`, `atOnce` or `why`: set its last wait, tell it when written, set the row's `capped`, write
       `queue.why` (persist) when it changed, and `record` a `session.capped`. Then arm 10,000 ms for `minute`, else
       30,000, and answer false.
    f. Room: let the head through without awaiting it, and answer true.
21. `let_through(entry, h)` (:388-449): `waited` is whether a last wait was held; `queue` and the last wait are removed;
    a start is claimed; the row's `capped` removed and `session.held` emitted (when the row exists); persist;
    `stand_up_queued(record, claim.posted, waited)`; release the claim. If the held start changed, return. A stop
    that came meanwhile cuts the broker off (`cut_off`), unforks the standing machine (warn `workspace <id>: its
    machine was not deleted after the stop: <why>`), drops the start, returns. Else `forget`, remove the row, `left()`,
    delete the `sub-starts` row, and start the turn with `sessions_api.start(ws, {...opts, notify?, as: {threadId,
    turnId}}, origin)`, where origin is `{origin: road ?? "here", by}` with a `by`, else the road alone.
    On a failure: release the claim; a stop drops; a cap refusal (`is_cap_refusal`) of the still-held, unstopped start
    re-enqueues it refused with `cloud_full` (limits read again) and sets the row's `capped`; any other failure warns
    `workspace <id>: the held spawn did not start: <why>` and is `fail_first` while still held, otherwise (the turn
    start itself failed) the targets are told and the caller failed. A pump starts at the end either way. The caller
    hears the handle last.
22. `resume_queue()` (:454-474): every `sub-starts` row whose record is not live and `queued` is deleted; every live
    `queued` record with no row is dropped; one with no `queue` gets `{since: row.at, why: "slot"}` and is persisted;
    its last wait is `wait_on(...)` else `slots(place)`; its start is held again with no caller; then a pump.
23. Bus `"*"` (:478-495): `workspace.deleted` forgets `queued_at` always. With held starts: the deleted record's own
    start is forgotten, left, its caller failed with `deleted_before_start_line(name)`, its row deleted from the
    store without waiting; every held start whose record's `lead.workspaceId` is the deleted one is dropped with `its
    lead's folder was deleted`. `workspace.napped`, `.deleted`, `.gone`, `.released` and `place.changed` pump, only
    while starts are held.

## Edge cases

- Two spawns in one millisecond keep their order: the second's `since` is the first's plus 1 ms (rule 8).
- A refused create goes before every queued record, 1 ms ahead of the head (sub-queue.test.ts:399).
- A startless record alone is still dropped: rule 20b arms the timer for it (:504).
- `hold_queued` twice for one record refuses the second, the first unchanged.
- A cap refusal after a stop drops; it does not re-enqueue (:424-427).

## Behaviour nobody wrote down

- Rule 3.4: `claim_start` inserts in the same step as the read of the count; `pump_once` re-checks identity after its
  one await (20d); `let_through` re-checks `held` after `stand_up_queued`.
- Rule 3.3: no borrow across `persist`, `stand_up_queued`, `sessions_api.start`, `emit`, `record`, `notify_end`.
- Rule 3.11: `queued()` orders by `since` string then id, not by map order.
- Restart: only the record and the `sub-starts` row survive; callers' futures do not (resumed starts have none).

## Decisions

1. `hold_queued` returns `LocalBoxFuture<'static, Result<SessionHandle, HostError>>` built over a oneshot pair.
2. `onHeld` is `Option<Rc<dyn Fn(HeldTold)>>` on the start options the context hands in.

## After the port

- The minute count adds this host's unposted starts to the cloud's own count, which can count one start twice
  until the create is posted (sub-queue.ts:54-57, written down in the TS).
- A `workspace.deleted` with no held starts does not pump, so a startless record's drop waits for its timer.

## Tests (tests/sub_queue.rs)

| Rule | Test (TS case) |
|---|---|
| 8, 10, 11, 20, 21 | `is_held_as_queued_answered_capped_then_starts_by_itself_first_in_first_out` (:175) |
| 10 | `lists_its_waiting_row_with_the_head_of_a_long_brief` (:218) |
| 4, 20e | `holds_a_spawn_with_no_start_left_in_the_minute_and_starts_it_when_a_read_shows_one` (:233) |
| 15, 21 | `a_held_spawn_whose_start_failed_fails_its_row_and_tells_its_targets` (:254) |
| 16, 14 | `drops_a_held_spawn_whose_lead_is_stopped_no_start_spent` (:275) |
| 23 | `a_held_spawns_record_deleted_goes_with_its_start` (:300) |
| 17 | `delete_queued_drops_the_held_start_and_answers_once_gone` (:316) |
| 22 | `survives_a_restart_queued_record_and_start_come_back_waiting` (:333) |
| 22 | `a_restart_while_a_let_through_spawns_machine_was_made_puts_it_back_in_line` (:357) |
| 3 | `slots_count_a_starting_spawn_and_not_a_released_machine` (:375) |
| 7, 8, 21 | `a_cap_refusal_puts_the_spawn_back_at_the_head_and_asks_again_a_read_later` (:399) |
| 6, 4 | `two_spawns_asked_together_under_one_machine_one_starts_the_other_queues` (:445) |
| 11 | `answers_held_a_start_that_met_the_wait_in_the_queue` (:456) |
| 4, 6 | `two_spawns_on_the_minutes_last_start_one_machine_the_other_held_for_the_minute` (:470) |
| 20a | `passes_over_a_startless_record_and_drops_it_after_the_wait` (:484) |
| 20b | `a_startless_record_alone_is_still_dropped_after_the_wait` (:504) |
| 16, 21 | `a_delete_while_a_let_through_machine_is_made_drops_it_as_it_lands` (:519) |
| 5, 20e | `a_spawn_behind_the_head_shows_the_heads_wait_and_every_row_takes_its_count` (:549) |
| 15 | `a_failed_first_start_stays_released_with_its_opening_kept` (start-events :101) |
| 2 | `queued_orders_by_since_then_id` (new) |
| 18 | `a_pump_asked_while_one_runs_runs_once_more_not_twice_at_once` (new) |
| 9 | `attach_queued_answers_the_wait_line_on_every_use` (new) |
| 12, 22 | `resume_queue_drops_rows_whose_record_is_not_queued` (new) |
