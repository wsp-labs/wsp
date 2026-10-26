# sub_spawn_machine

Source: `packages/runtime/src/machines/sub-spawn.ts:32-74, 283-341, 414-621` (about 290 of 622 lines) at `65f6b89c7`.
Tests: `sub-spawn.test.ts:59-153, 419-479` (the fork's lines and the faults, 10), `sub-resume.test.ts:146-430` (the
send into a released sub-thread, 17), `sub-oom.test.ts` (6), `sub-start-events.test.ts:101-160` (3). Rust:
`src/machines/sub_spawn/machine.rs`.

## What it does

Makes the machine of a spawn the queue let through (`stand_up_queued`) and of a send into a released sub-thread
(`resume_sub`): one machine forked from the setup image's head, its daemon asked, the empty folder made, the picked
files landed through the provider's file road, and, on a resume, the kept agent session landed where it stood. A
machine that fails before the first turn in one of three ways a fresh machine cures is deleted and replaced once; a
second such failure fails the start naming both. Every step is written on the record's `sub.spawn` and told to the
thread's row as it goes. `sub_killed_why` words an agent the kernel killed for memory.

## Behaviour

1. `fault_of(e)` (:33-38): `GuestUnusable` gives `{kind: "unusable", box: machineId, why: detail}`; `EnvNotLanded` with
   exit 124 gives `{kind: "env", box}` (any other exit is no fault); `CreateCancelled` gives `{kind: "cancelled", box,
   why}` with an empty why read as `no reason given`; anything else is none.
2. `told(record)` (:47-49): `push_head` of the first thread record (in order) whose `workspaceId` is the record's;
   nothing when none (a first start has none yet). `step(record, move)` (:51-56): nothing without `sub.spawn`; else
   `move(spawn)` then `told`.
3. `logged(record, report)` (:58-67): a report that calls `report(stage, message, said)` first, then steps: the log
   gains `message` and keeps its last `SUB_SPAWN_LOG_LINES` (40); with `said.box` and a `spawn.machine`,
   `machine.id = box.id` and `box.state` is pushed when it differs from the last state.
4. `spawn_failed(record, e)` (:69-74): `failed = {step, said: refusal_said(message, fix), fix?}` where `step` is
   `files` when `spawn.files` exists without `ms`, else `machine`; `fix` only when the error carries a string fix.
5. `machine_for(record, at, on, seed, report, landed)` (:285-310) loops: `stand_up`; on an error, `drop_machine(at,
   record, fault.box)` always (even for a non-fault); a non-fault is re-thrown; a second fault refuses
   `sub_twice_refusal(first, fault, name, on)`; the first is remembered, its line `sub_replaced_line(fault, name, on)`
   is pushed on the answer's notices, stepped onto `spawn.replaced` (appended) with `spawn.files` removed, reported as
   stage `fork-requested` with `{notice: line}`, warned `workspace <id>: <line>`, and the `creates` row
   `workspace/<id>` is deleted so the next create is a new machine, not a replay.
6. `stand_up_queued(record, posted, waited)` (:314-341): the provider row whose id is the record's place id or its
   `provider`; `on` is its `provider_key_name`, else `sub.on`. The backend is `backend_for(record)`; the seed is read
   again now. One step: with `waited`, `spawn.startedAt = now`; `notSent` set to the seed's why or removed. The report
   is `logged` over `stage_reporter(id, name, now, {kind: "thread", threadId: lead.threadId, workspaceId:
   lead.workspaceId, rootThreadId: record.rootThreadId ?? lead.threadId})`. `forking` holds the name for the call. A
   failure that `is_cap_refusal` is re-thrown untouched (the queue waits again, nothing written); any other failure
   is `spawn_failed` and reported as stage `failed` with its message, then re-thrown.
7. `stand_up(record, on, seed, report, landed)` (:416-472), in order: step `spawn.machine = {since: now, states: []}`;
   report `fork-requested` `starting_line(name, on)`; `ctx.fork(record, on_machine, None, report, false)` where
   `on_machine(m)` sets `machineId = m.id`, `phase = running`, attaches the machine with `creating = true`, and calls
   `landed()`. Then with the live entry: where the machine has a preview route or a daemon-answers road,
   `ping_daemon(entry)` and report `daemon-answering` with `Daemon answered.` or `Daemon did not answer.` and
   `{notice: fault}`, then start `sync_daemon(entry)` without waiting. `install -d -m 0755 '<folder>'` (no timeout);
   non-zero fails `<folder> was not made on <machine> (exit <n>): <stderr.trim() || stdout.trim()>`. Step
   `machine.ms = now - since`. With a tar: step `files = {since: now, paths, bytes}`; a machine without `put_bytes`
   fails `<on> takes no file onto <machine>, so the picked files cannot land`; report `project-cloned`
   `sub_copying_line(seed.files)`; put the tar at `/tmp/wsp-seed-<record id>.tgz`; `tar -xzf '<at>' -C '<folder>' &&
   rm -f '<at>'`; non-zero fails `the picked files did not land in <folder> (exit <n>): <tail>`; step `files.ms`.
   Step `readyAt = now` and `asked` removed. Persist. `creating` removed. Report `ready` `CREATE_READY`. A record with
   no `releasedAt` emits `workspace.created` with its view; a resumed one emits `{type: "workspace.woken",
   workspaceId, machineId}` and `emit_status(entry, "reachable")`.
8. `drop_machine(at, record, box)` (:476-494): a live entry whose machine is not an absent stand-in is unforked (a
   failure that is not `is_missing` warns `workspace <id>: <machine> was not deleted: <message>`) and removed from
   `live`; a fault's `box` other than that machine is fetched and killed at the backend (same warning rule); then
   `machineId = ""`.
9. `resume_sub(asked, thread, harness, prompt)` (:501-516): a resume in flight for the record is joined (rule 3.9's
   join): the caller waits for it and answers `{entry: <live entry now>, notices: []}`. A record not `released`
   answers `{entry: asked, notices: []}`. Otherwise one resume runs and is held for joiners until it settles.
10. `resume_once` (:518-582), in order: the provider row `names_place(row, sub.on)` with a backend, else refuses
    `sub_cloud_gone_refusal(cloud_said(sub.on))`; `on`, `place = record.place ?? row.id`; `git_host_of(sub.repo)` none
    refuses `sub_other_host_refusal`; `image_at` (with `brokered(record)`); `place_guard`; `agent_on`; `seed_of`;
    limits; `starts_left`; `size` is the sold size with the record's cpu and memMb, else `{name:
    sub_size_word(size), ...record.size}`. Then: `golden = head.snapshotId`, phase `running`, `firstLife = true`,
    the broker's `lift(id)`, `sub.lost` and `record.diskGone` removed; `tried = sub.opening ?? prompt`; `sub.spawn =
    {startedAt: now, asked?: tried, notSent?: seed.why, log: []}`; `told`. Report is `logged` over `stage_reporter(id,
    name, now, None)`; `forking` holds the name. `machine_for(..., landed = no-op)`. On failure: `spawn_failed`,
    report `failed`, phase `released`, `machineId ""`, `attach(dead_machine(""))`, persist, `rows_sent()` awaited
    (the failed row goes out before the refusal, rule 3.12), re-thrown.
    On success: notices gain `sub_not_sent_line(why, project.name)` and `sub_stayed_line(stayed)` as in the spawn. A
    record with `sub.opening` removes it, persists, and answers `{entry, notices}` plus `preamble: opening` unless the
    send's prompt is the opening itself. Otherwise `land_session(entry, thread)`, then `{entry, preamble:
    sub_resume_preamble({on, size, folder, sent: seed.paths, setupGit, repo: sub.repo, base: sub.base, branch:
    sub.branch, pushed: sub.result.pushedSha is set, deleteAfterMinutes, brokered?: true with a branch of record}),
    notices}`.
11. `land_session(entry, thread)` (:587-601): nothing unless the kept session has `kept` set and a `path`, the adapter
    names a `sessionFile(id)`, the `sub-sessions` blob exists, the machine offers `put_bytes`, and the path starts with
    `<file.folder>/`. Then put the blob at `/tmp/wsp-session-<thread>.gz` and run `sub_session_land_command(file,
    path, at)` with `INLINE_EXEC_MS`; non-zero is an error `exit <n>: <tail 200>`; any error is warned `the kept session
    of sub-thread <thread> did not land on <machine>, so its turn goes on from the transcript: <message>` and
    swallowed.
12. `sub_killed_why(entry, started_at)` (:606-619): `journalctl -k --since @<floor(started_at / 1000)> --no-pager |
    grep -ci 'out of memory'` with `INLINE_EXEC_MS`; an exec error warns `the kernel log of <name> was not read after
    its agent was killed: <message>` and answers none; `parseInt(stdout) > 0` (leading digits; NaN is not) or none.
    Then the sold sizes (a limits failure reads none), the record's size by cpu and memMb else `{name:
    sub_size_word, ...}`, `bigger = sub_next_size(size, sold)`, the provider row (a rows failure reads none), and
    `sub_oom_line(name, provider_key_name(row.name) ?? sub.on, size, bigger.map(sub_size_word))`.

## Edge cases

- A replayed fault's box is killed at the backend because the provider left it standing (rule 8; sub-spawn.test.ts:421
  with `replayed`).
- A non-fault error still drops the machine before re-throwing (rule 5).
- Two sends into one released record make one machine; the second waits and opens with no preamble of its own
  (sub-resume.test.ts:147).
- A session path outside the adapter's folder is not landed (rule 11).
- `parseInt("2\n")` is 2; `"0\n"` keeps the agent's own exit line (sub-oom :74).

## Behaviour nobody wrote down

- Rule 3.3: no borrow across `fork`, `ping_daemon`, `persist`, `emit`, `put_bytes`, an exec or a report; `step` borrows
  the record only inside its closure and calls `push_head` after the borrow ends.
- Rule 3.9: `resume_sub` is a join keyed by record id; the map entry is removed when the work settles either way.
- Restart: `sub.spawn` and `sub.opening` survive in the record; the `creates` row is deleted between attempts.

## Decisions

1. `StageReport` is `Rc<dyn Fn(CreateStage, &str, Option<&StageSaid>)>`; `logged` returns a new one wrapping it.

## After the port

- `stand_up`'s `install -d` and `tar` execs carry no timeout (sub-spawn.ts:440, 453): a machine that hangs holds the
  start until the backend's own limit.
- A sub-thread dropped between `bind` and the fork's return leaves its box standing, as the TS does: `stand_up` fails the
  start and `drop_machine` is called with no fault box, so it finds no live entry to kill (sub-spawn.ts:476-494, :433).

## Tests (tests/sub_spawn_machine.rs)

| Rule | Test (TS case) |
|---|---|
| 7 | `forks_the_image_head_lands_the_picks_by_the_file_road_and_names_only_paths_in_commands` (spawn :71, :462) |
| 3, 7 | `writes_each_step_on_the_record_as_it_goes` (spawn :129) |
| 1, 5, 8 | `is_deleted_and_replaced_once_for_each_fault` (spawn :421, the five rows) |
| 5 | `fails_on_a_second_failure_naming_both_and_leaves_neither_standing` (spawn :442) |
| 5 | `names_both_faults_when_they_differ` (spawn :452) |
| 9 | `a_send_while_the_new_machine_is_made_waits_for_it` (resume :147) |
| 10, 11 | `makes_a_new_machine_from_the_head_lands_picks_and_session_and_resumes_under_the_preamble` (resume :230) |
| 10 | `lifts_the_broker_before_the_new_machine` (resume :279) |
| 10 | `names_a_stayed_pick_in_the_notice_of_the_send` (resume :301) |
| 10 | `a_retried_failed_first_start_names_a_stayed_pick` (resume :308) |
| 11 | `lands_codexs_rollout_at_its_dated_path` (resume :321) |
| 5, 11 | `replaces_a_broken_box_once_and_lands_the_session_on_the_second_alone` (resume :333) |
| 10 | `puts_the_failed_starts_row_out_before_the_refusal` (resume :347) |
| 4, 10 | `fails_after_a_second_broken_box_and_stays_released_with_asked_and_failed` (resume :364) |
| 10 | `asks_gh_here_again_before_a_new_machine_while_brokered` (resume :386) |
| 10 | `refuses_an_agent_the_broker_adds_no_key_for_before_a_new_machine` (resume :399) |
| 11 | `with_no_session_kept_lands_none` (resume :412) |
| 10 | `a_sub_that_pushed_nothing_starts_again_from_the_leads_commit` (resume :423) |
| 12 | `oom_reads_the_kernel_log_once_and_says_the_oom_line` (oom :38, :47) |
| 12 | `the_kernel_counting_no_oom_keeps_the_agents_own_line` (oom :74) |
| 12 | `names_the_next_size_up_and_none_on_the_largest` (oom :87) |
| 10 | `a_failed_first_start_retried_opens_with_its_preamble_and_task` (start-events :101, :131) |
| 6 | `a_cap_refusal_from_the_queued_start_is_rethrown_untouched` (new, from :333) |
| 6 | `stand_up_queued_restarts_the_clock_after_a_wait` (new) |
