# sub_life_timers

Source: `packages/runtime/src/machines/sub-life.ts:156-597` (442 of 597 lines) at `65f6b89c7`. Tests:
`packages/runtime/test/sub-lifecycle.test.ts` (30 cases), `sub-results.test.ts` (the line and head cases, 6),
`sub-resume.test.ts:100-145` (the keep, 3), `sub-disk-keep.test.ts` (4, the held line's day). Rust:
`src/machines/sub_life/timers.rs`; the area is `SubLifeArea`, built by `SubLifeArea::new(ctx)`.

## What it does

A sub-thread's machine from its done to its delete. At each turn's end (`session.end`) the host reads off the machine
what was pushed (`sub_result_command`), keeps the agent's session file as a blob, then arms a delete at
`now + deleteAfterMs` when everything is pushed, or holds the machine over work not pushed, persists, tells the row,
and naps the machine. A timer per record fires the delete (`expire`) at `sub.deleteAt`; the record then stays,
released, with its thread, transcript, result and session. A turn that starts takes the machine back (no delete, no
hold). Done and delete of one record never cross: both run through one per-record chain (`serial`).

## State (memory only; nothing here survives a restart except what the record holds)

- `timers: id -> cancel` (:158), `working: id -> chained promise` (:160), `read_waits: id -> set of {since, go}`
  (:163), `done_started: id -> ms` (:168). `IndexMap`s, `shift_remove` (rule 3.11).
- What survives: `sub.deleteAt`, `sub.held`, `sub.result`, `sub.session`, the `sub-sessions` blob. `arm_subs()` at boot
  (:544-553) re-arms from them.

## Behaviour

1. `serial(id, work)` (:170-178) is rule 3's chain of the second form: the work starts after the record's previous one
   settles, fulfilled or rejected; its failure is logged `sub-thread machine <id>: <message>` and swallowed, so the
   returned future never fails; the map entry is removed when it settles if it is still the tail. `sub_done`,
   `sub_expire`, the timer, the `session.end` listener and `arm_subs` all go through it.
2. `thread_of(workspace_id)` (:181-185): the first thread record whose `workspaceId` matches, in insertion order; else
   the first session row with that `workspaceId` and a `threadId`; else none.
3. `busy(thread)` (:194) is `ctx.thread_runs(thread) || ctx.launching_on(thread).is_some()`.
4. `delay_ms(entry)` (:199): `settings_at(place_id_of(record)).deleteAfterMs`, else `DELETE_AFTER_MS` (1,800,000). Read
   at each done, never cached.
5. `read_result(entry)` (:201-218). `readAt` is `now` as an ISO string, taken first. `failed(why)` keeps the record's
   last `pushedSha` (or `null`) and `commits` (or 0) with `unpushed: 0, changed: 0, readFailed: why`. A phase other
   than `running` is `failed("its machine was <phase>")` with no exec. Exec has `INLINE_EXEC_MS`; a non-zero exit is
   `failed("exit <n>: <tail>")`, the tail `(stderr.trim() || stdout.trim())` cut to its last 200 UTF-16 units; no
   counts is `failed("it answered no counts: <stdout.trim() last 200>")`; an exec error is `failed(<message>)`. A
   `norepo` read on a record whose last result had a `pushedSha` answers the read with `norepo` dropped and the old
   `pushedSha` and `commits` put back in their places.
6. `push_refused_in(ws, thread)` (:222-234) reads the transcript with `limit 400, bytes 2_000_000`, maps every
   `session.delta` `tool_result` with a `toolUseId` by that id (a later one replaces), then walks events newest first
   to the newest `tool_use` whose `isGitPush(text)`; that push failed when its result exists and is `isError` or has a
   non-zero `exitCode` (absent reads 0), giving `pushRefusedOf(result.text)`; a push that went through, or one with no
   result, gives none, and so does no push at all. Only the newest push is read.
7. `keep_session(entry, thread)` (:238-273). Nothing (no store write, no exec) when `latestOn(thread)` has no
   `claudeSessionId`, the phase is not `running`, the adapter names no `sessionFile`, or the machine offers no
   `read_bytes`. Else, at `/tmp/wsp-session-<thread>.gz`: the pack command (`INLINE_EXEC_MS`); exit 0 with `nofile` in
   stdout deletes the blob and writes `{agent, id, kept: null, why: "no session file for <id> under <folder>"}`; a size
   line `size <n>` and a path line `path <p>` (multiline regex) are required with exit 0, else an error `the pack exited
   <n>: <tail 200>`; `size > SUB_SESSION_MAX_BYTES` deletes the blob and writes `{agent, id, kept: null, bytes,
   why: "the agent's session was too large to keep (<fmt_bytes>)"}`; otherwise `read_bytes(at, size)` is put as the
   blob and `{agent, id, kept: now, bytes, path}` written. Any error deletes the blob (its own failure ignored),
   writes `{agent, id, kept: null, why: <message>}` and warns. Always, last, `rm -f '<at>'` is exec'd, its failure
   ignored. The session is written on `sub.session` in memory only; the caller persists.
8. `arm(entry)` (:275-292) cancels the record's timer first; with `deleteAt` set and a phase other than `released`,
   schedules `serial(id, expire)` after `max(0, deleteAt - now)` and removes the timer entry when it fires.
9. `release(entry, {lost?, cloudGone?})` (:295-328). Cancels the timer. An already `released` record only gains
   `cloudGone` (persist and tell) when asked and not set; nothing else. Otherwise in this order: `endSessions(id,
   GONE_REASON)`; phase `released`, `machineId ""`, `gone` removed, `releasedAt = now`, `deleteAt` and `held` removed,
   `lost` and `cloudGone` set when given; `entry.lateRead()` called; the id leaves `napRefusals`, `stopRefusals`,
   `unreached`; `attach(record, deadMachine(""))`; persist; `idle.forget(id)`; tell the row; emit
   `{type: "workspace.released", workspaceId}`; `emitStatus(attached, "gone")`.
10. `done(id)` (:330-338) takes `startedAt = now` first, runs the steps, and always (even on failure) sets
    `done_started[id] = startedAt` and wakes every read wait with `since <= startedAt`.
11. The done steps (:340-381), in order, each a return point:
    a. no live entry, no `sub`, or phase `released`: nothing.
    b. no thread, or `busy(thread)`: nothing.
    c. phase not `running`, a result held, and the thread's latest turn started at or before that result's `readAt`
       (absent start reads 0): nothing, the old read stands.
    d. `read_result`; then `push_refused_in` only when the read is not all pushed (its failure reads none), added as
       `pushRefused`; then `keep_session`.
    e. `busy(thread)` again, or the live entry is no longer this entry (a release re-attached it): nothing is written.
    f. `sub.result = result`; all pushed sets `deleteAt = now + delay_ms` and removes `held`; otherwise `held =
       {files: changed, commits: unpushed, at: now}` and removes `deleteAt`. Persist, `push_head(thread)`, `arm`, then
       wake read waits with `since <= now` (the lead's line goes before the nap).
    g. phase not `running`: stop. Else `nap_with(id, DONE_NAP)`. A nap that fails, while `deleteAt` is set, the entry
       is still live and still `running`, warns and calls the lifecycle's `backstop(machine, deleteAt + 600_000,
       exact: true)` where the lifecycle has one, its failure warned. A failed nap with no `deleteAt` does nothing.
12. `expire(id)` (:385-429): nothing without a live entry, `deleteAt`, or with phase `released`. A busy thread removes
    `deleteAt`, persists, stops. Else it awaits the entry's `napping` then `waking` futures (failures ignored). Still
    `running`: read again; the result is written; not all pushed sets `held {files, commits, at: now}`, removes
    `deleteAt`, persists, tells, stops; all pushed keeps the session (when a thread exists). Then, if `deleteAt` was
    removed meanwhile or the thread is busy, stop. `cutOff(ctx, entry)` (the broker's revoke) runs before `unfork`.
    An `unfork` failure that is not `isMissing` warns `sub_delete_refused_line(cloud, name, said, 10)` with `said =
    failureWords(e)`, sets `entry.deleteSaid = {phase, line}` (the same line), `deleteAt = now + 600_000`, persists,
    tells, re-arms, stops. A missing machine counts as deleted. Then `deleteSaid` is removed and `release(entry)`.
13. `reclaim(ws)` (:432-443) on `session.start`, `session.held`, `session.queued`: only for a live sub record holding a
    `deleteAt` or a `held`; cancels the timer, removes both, calls the broker's `lift(ws)` where wired, and persists
    without awaiting (a failure warns `sub-thread machine <id>: <message>`). No row push.
14. `session.end` (:445-449): for a live sub record, a done is queued on `serial` from a task spawned after the current
    one (TS `setImmediate`), so a send queued behind the turn takes the thread first and the done sees it busy.
15. `workspace.deleted` (:453-458): cancels the timer and forgets `done_started`; read waits stay (they time out).
16. `sub_unpushed(entry)` (:462-472): none without `sub`, or phase `released` or `queued`, or no thread. A turn on the
    thread (latest `startedAt`) with no result, or started after the result's `readAt`, gives `{files: 0, commits: 0,
    unread: true}`. Else none without `held`; else `{files, commits, readFailed?}` (from the result).
17. `forgotten(entry)` (:476-487): cancels the timer, removes `deleteAt`, sets `cloudGone`, `lateRead()`, attaches
    `deadMachine(<the record's machineId>)` (the id kept), persists, `idle.forget`. The phase is kept. No row push here.
18. `sub_read(ws, since)` (:491-516): `read()` is the live record's result when its `readAt` parses to `>= since`.
    No live sub record answers none. When there is no such read, the phase is not `released`, and `done_started[ws]`
    (absent is minus infinity) is `< since`, it waits on a read wait `{since}` or `SUB_LINE_WAIT_MS` (300,000 ms),
    whichever is first; going cancels the timer and removes the wait, and the set when empty. Then answers `read()`.
19. `sub_line(ws, since)` (:520-537): `sub_read`, then none when the record is gone, no result, no thread, or busy.
    `kept` is computed only for a held record not released: `disk_kept_until(ctx, record, held.at)`. The line is
    `subResultLine({threadId, lead: record.lead.threadId ?? threadId, result, base: sub.base.branch, deleteInMs?,
    kept?})`, `deleteInMs = max(0, deleteAt - now)` only with a `deleteAt` and a phase other than `released`, `kept =
    {on: cloudSaid(sub.on), until}`.
20. `arm_subs()` (:544-553): over live entries in order, skipping no sub, `released`, and `cloudGone`; a `deleteAt`
    arms (a past one fires at once); otherwise with no `held`, a done is queued on `serial` (a done cut by a restart).
21. `sub_lost(entry, reason)` releases with `{lost: reason}` and answers `Settled`. `release_subs_on(place, {forgot})`
    walks a snapshot of live entries whose `placeIdOf` is the place: `forgot` and not released is `forgotten`, every
    other is `release(entry, {cloudGone: true})`.
22. `sub_delete(asked, thread, origin, {check})` (:566-583): awaits the record's chain tail first (the done in flight).
    The entry is the live one, else `asked`. Not `queued` and busy refuses `THREAD_WORKING_LINE` with kind `conflict`.
    `held = sub_unpushed(entry)`; a thread's token (`scope_of(origin)` some) with a held answer refuses
    `subHeldDeleteRefusal(thread, held, branch)` (class usage). `check` answers `{workspaceId, threads: 1, on,
    question}`, the title `foldThreads(rows of that thread)[0].title`, else the record's name; nothing deleted.
    Otherwise `workspaces.delete(id)` and `{workspaceId, threads: 1, machine: "deleted", on}`. `on = cloudSaid(sub.on)`.
23. `refuse_sub_send` refuses `subCloudGoneRefusal(cloud)` for `cloudGone`. `refuse_sub_wake` refuses the same, then
    `subReleasedRefusal(thread_of(id) ?? name, cloud)` for a released sub record. `lead_of_sub(ws, origin)`: true only
    when the live record's `lead.threadId` equals the caller's scope thread.

## Edge cases

- A read that fails holds `{files: 0, commits: 0}` and arms nothing (sub-lifecycle.test.ts:139).
- `norepo` with no pushed sha is all pushed and arms the delete (:131).
- A pack answering `size 67108864` is kept; `67108865` is not (rule 7, `>`).
- `sub_read` with `since` exactly the result's `readAt` answers it (`>=`).
- `expire` with `deleteAt` removed by a reclaim during the read: no unfork (:581-605).

## Behaviour nobody wrote down

- Rule 3.9: `serial` is the swallowed-chain form; `sub_delete` awaits the tail, which never rejects.
- Rule 3.3: no borrow of the record across `nap_with`, `unfork`, `emit`, a store write, or `keep_session`'s execs; the
  record is re-read through the context after each.
- Rule 3.4: steps 11e and 12's re-checks sit after their awaits exactly; nothing added between f's writes.
- Rule 3.27: the chain is not awaited at close; timers die with the area.
- Invariant (no row written twice): `pushHead` once per done that writes (step f), never on a done that returns early.

## Decisions

1. The area is `Rc<SubLifeArea>` over `Rc<dyn SubLifeCtx>`; bus listeners are registered in `new`, holding a `Weak`.
2. `setImmediate` (rule 14) is `ctx.spawn_local` of a task that first yields once (`tokio::task::yield_now`).

## After the port

- `workspace.deleted` leaves the record's read waits standing until their 5-minute timer (sub-life.ts:453-458).
- A failed nap with no `deleteAt` (a held machine) moves no backstop, so a held machine whose nap failed runs until
  the idle nap tries again (:373-374).

## Tests (tests/sub_life_timers.rs)

| Rule | Test (TS case) |
|---|---|
| 5, 7, 11 | `reads_the_result_off_local_refs_keeps_the_session_arms_the_delete_and_naps` (lifecycle :63) |
| 4, 11f | `arms_the_delete_at_the_delay_the_person_set_read_at_the_done` (:87) |
| 11f | `holds_the_machine_over_work_not_pushed_arms_no_delete_and_naps_all_the_same` (:95) |
| 11f | `tells_the_row_once_the_done_read_holds_the_machine` (:105) |
| 6 | `names_why_the_push_was_refused_off_the_agents_own_push` (:111) |
| 6 | `names_no_cause_where_the_last_push_went_through_or_all_is_pushed` (:119) |
| 5 | `arms_the_delete_for_a_sub_thread_that_never_fetched_the_code` (:131) |
| 5 | `treats_a_read_that_fails_as_work_not_pushed` (:139) |
| 11g | `leaves_a_machine_whose_nap_was_refused_running_with_the_backstop_at_delete_plus_10_minutes` (:147) |
| 12, 9 | `deletes_the_napped_box_with_no_wake_and_keeps_the_record_released` (:174) |
| 9 | `tells_the_row_at_the_release` (:206) |
| 12 | `cuts_the_machine_off_before_the_delete_keeps_it_off_where_refused_and_lifts_it_for_a_turn` (:218) |
| 13 | `is_cancelled_by_a_turn_that_starts_and_armed_again_at_its_done` (:247) |
| 12 | `turns_into_a_hold_where_the_machine_still_runs_and_its_read_finds_work` (:265) |
| 20 | `arm_subs_fires_a_delete_at_passed_while_the_host_was_down` (:294) |
| 11c | `keeps_the_dones_word_when_a_start_a_restart_failed_ends_on_the_napped_machine` (:309) |
| 20 | `arm_subs_arms_a_delete_at_still_ahead_and_fires_it_then` (:324) |
| 21, 9 | `a_lost_box_reads_released_with_lost_and_no_gone` (:341) |
| 22 | `delete_takes_the_record_through_workspaces_delete_and_answers_deleted` (:355) |
| 22 | `refuses_a_leads_token_over_work_not_pushed_and_lets_the_person` (:373) |
| 22 | `lets_a_leads_token_delete_one_whose_work_is_all_pushed` (:388) |
| 21, 9 | `removing_the_cloud_releases_every_sub_with_cloud_gone` (:397) |
| 22 | `delete_check_answers_the_question_and_deletes_nothing` (:428) |
| 16, 22 | `refuses_a_leads_token_at_once_the_turn_not_read_yet` (:466) |
| 1, 22 | `waits_for_the_done_in_flight_then_refuses_over_the_work_it_found` (:474) |
| 20 | `arm_subs_runs_a_done_a_restart_cut_short` (:488) |
| 17, 21 | `forgetting_the_cloud_keeps_the_phase_marks_cloud_gone_and_arms_nothing` (:525) |
| 12 | `waits_for_the_wake_and_reads_the_machine_again_before_it_deletes` (:559) |
| 12 | `keeps_the_machine_of_a_turn_started_while_the_machine_is_read` (:581) |
| 7 | `keeps_none_past_64_mb_and_drops_the_copy_it_held` (resume :115) |
| 7 | `a_keep_that_fails_after_a_good_one_drops_the_earlier_copy` (resume :131) |
| 7 | `keeps_the_next_dones_session_in_place_of_the_first` (resume :101) |
| 19 | `line_ends_with_the_branch_pushed_and_the_delete_in_a_span` (results :52) |
| 19 | `line_ends_with_the_work_not_pushed_and_the_machine_kept` (results :62) |
| 19 | `line_names_the_day_the_cloud_may_delete_a_held_disk` (results :70) |
| 18 | `sub_read_waits_for_the_done_after_the_turn` (results :99) |
| 18 | `sub_read_gives_up_after_five_minutes` (new, from :511) |
| 18 | `sub_read_with_since_equal_to_read_at_answers_at_once` (new) |
| 1 | `serial_runs_a_done_and_a_delete_one_after_the_other_and_swallows_a_failure` (new, from :170) |
| 14 | `session_end_runs_the_done_after_a_send_queued_behind_takes_the_thread` (new, from :448) |
| 23 | `refuse_sub_wake_names_released_and_cloud_gone` (lifecycle :174, :397) |
| 15 | `workspace_deleted_cancels_the_timer` (new) |
