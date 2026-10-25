# boot_ready
Source: `packages/runtime/src/machines/boot.ts:328-621` (about 290 of 1,071 lines) at `65f6b89c7`. Tests: integration
cases in `runtime-builders.test.ts`, `thread-marks.test.ts`, `session-opening.test.ts`, `runtime-index.test.ts`,
`runtime-comeback.test.ts`, `thread-cap.test.ts`, `spawn-guard.test.ts`, `channels.test.ts`, `folder-threads.test.ts`
(under "Rules against tests"), and 5 scenes in `oracle/boot_ready.oracle.ts` written to
`tests/fixtures/oracle/boot_ready.json`. Rust: `src/machines/boot/ready.rs`. Chapter 02 "What a start rebuilds"
(rules 2.17, 2.18, 2.36, 2.37) is the same list; this spec pins the calls in order.

## What it does
`ready()`: the host's start. It reads the store back into memory in a fixed order, re-opens every turn the store left
running, fails the starts a thread cap held, takes back device tokens whose turn is over, and admits the builders, all
once per process.

## Behaviour
1. `ready()` is a join: the first call starts the one future, every call answers it (scene "a fresh store").
2. Owner (:331): `owner/id` with a non-empty string `id` is read back; anything else mints `h_` + `Ids::random_hex(4)`
   and writes `{id}`. Reads stamp (:337): a number `at` in `reads/since` is read back; anything else is `now`, written.
3. `migrate_copies()`; `place_door.load()` (when wired); every `projects` row into `projects_held` in store order.
4. With local wiring, each held project with no `git`, a `folder` source and a computer whose kind copies a folder
   gets `git_top_of(path)`; a top found is remembered as `{...project, git: {top}}`; a failure is skipped. All run
   together (scene "projects are held ..."). Proposal 3 drops this backfill; the port keeps the call for parity until
   02 rules otherwise (see Decisions).
5. `setups.load()`. Then every `workspaces` row hydrated in store order under one load `{until: now + 1500, late}`;
   each answered entry is remembered for the daemon sync.
6. `transcripts` rows moved (`move_transcript`) and deleted after; a failure warns `<why>, so it stays in the state file
   for the next boot to move`. `move_blobs(rows)` when rows are wired. Then `load_index(id)` for every workspace key
   whose index is not held. (Proposal 3 drops both moves; the calls stay behind the store answering none.)
7. The `sessions` documents, in store order, only for live workspaces:
   a. `viewed` marks with string values are kept per workspace (others dropped).
   b. Each thread with a string `harness` becomes a thread record `{workspaceId, harness, permissionMode?,
      parentThreadId?, rootThreadId?, readAt?, settledAt?, settleNamedAt?, pinnedAt?, order?, foldedAt?, replaces?,
      snoozedUntil?, section?, cutOwed?, forkedFrom?, limitResume?, rewound?, worked?, ended?}` in that order, each
      only when its type reads right (`section`, `forkedFrom`, `ended` by their schemas; `cutOwed` from `resumeAt` or
      `cutOwed`; `limitResume` needs both fields; `rewound` both). `filesIn` and `clouds` are not read back (02's
      after-port note). A `snoozedUntil` arms `wake_at`; a `limitResume` arms `resume_on_reset`.
   c. A document with no `sessions` array warns `sessions document for <ws> has no rows array, read as empty`.
   d. Each row: `notify` a string reads `[notify]`; `notifyBy`, `notifyRoad`, `asked`, `steered` by their readers;
      a prompt longer than `OPENING_KEPT` (600 UTF-16 units) is cut by `kept_opening` and the workspace marked for one
      more write; `turnLive` only when one of `reply`, `told` (number), `steered`, `waited` (number), `waitingSince`
      (number) is there; `from` only a safe non-negative integer; `toldLast` is dropped. Each row gets an `end` that,
      while it runs, settles it cut with its reason and persists its workspace. Running rows are kept to re-open;
      others with a `snapshot` to read their card.
8. Each recut workspace is persisted (not awaited). The late loads start. Each unread card waits for its workspace's
   late load, then: no thread or no cwd, or the turn already written, or `read_turn_changes(...)`, then the snapshot
   is dropped and the workspace persisted when it read or the host is not closing; a failure warns `what <turn>
   changed was not read: <why>`. `reread_held(id, "record load")` for every live id, not awaited.
9. Per live workspace, as its `boot_work` (removed when done; a failure warns `the turns and runs on <id> were not
   settled: <why>`): wait its late load; a made worktree is remounted; `reopen` its running rows; `sweep_runs`; then
   `sync_daemon` when its entry was answered by the load (or the late load answered).
10. `reopen(rows, again)`: clears `unanswered`, asks `reattach(row, again)` for all together; `cannot` settles cut with
    `RESTARTED_REASON` and `restart_cut_line(endedAt - (startedAt ?? endedAt))`; `gone` with `RUN_GONE_LINE` both
    ways; each workspace with an answer other than `unreached` is persisted; `unreached` rows are marked and asked again
    after `LINK_RETRY_WINDOW_MS` (60,000), only rows still the same entry, still running, on a `running` workspace (the
    rest wait another window; a closing host stops). Threads cut here lose their scoped devices whose thread does not
    run, except those a shell holds.
11. `resume_queue()`. Then each `held-starts` row: delete it; skip one whose workspace is not live; with `notify`,
    `notify_end` as failed with `cap_restarted_line(place)` told as its scope and road; `record` the `session.end` of
    02 rule 2.37 (`claudeSessionId` on the view only when the session id differs from the turn id).
12. Every scoped device whose thread does not run is revoked, except those a shell holds; then `revoke_asides()`.
13. `arm_subs()`; `deliver_owed()` (not awaited, a failure warns). The builders listed and each `admit_once`d together
    (a failure warns `builder <id> was not read, so the next look at the builders reads it again: <why>`), waited no
    later than the load's deadline.
14. `copies_moving = move_old_copies()` (not awaited); `arm_sweep()`; `slates.ready()` (not awaited).

## Edge cases
- A held start whose session id equals its turn id records no `claudeSessionId` (scene: `turn-h2`).
- A sessions document of a workspace not live is skipped, not deleted.
- A turn cut at restart revokes its device in rule 10 and again in rule 12 (two revokes of one id, scene log).

## Behaviour nobody wrote down
- Runtime invariant 1 (AGENTS.md): a restarted host re-opens each running turn by its handle; only the machine's
  answer ends it. Pinned by `reopen` above and `a_machine_that_answers_nothing_leaves_the_turn_running`.
- Rule 3.4: rows are put into `sessions` before any re-open starts; ends are told once every workspace's rows are in.
- Rule 2.17: what is memory only (held starts, waits) is failed or dropped here, never re-read.

## Decisions
1. Proposal 3 drops `migrateCopies`, the `git` backfill, transcript and blob moves and `moveOldCopies`. The Rust keeps
   their call sites as trait calls so the order of the rest is the TS order; the wiring answers them with nothing.

## After the port
- A turn cut at restart has its scoped device revoked twice.

## Rules against tests
| Rule | Tests (`tests/boot_ready_scenes.rs` replays a scene by name; `tests/boot_ready.rs`) |
|---|---|
| 1, 2, 14 | scenes `a_fresh_store_mints_*`, `a_kept_owner_and_reads_stamp_*` |
| 3, 4 | scenes `projects_are_held_*`, `no_local_wiring_reads_no_repo` |
| 5-13 | scene `the_sessions_documents_fill_the_rows_and_the_thread_records_and_running_rows_are_re_opened` |
| invariant 1 | `a_machine_that_answers_nothing_leaves_the_turn_running_and_is_asked_again_a_window_later`, `a_run_the_machine_no_longer_holds_ends_on_the_gone_line_once` |
| 2.37 | `every_held_start_fails_once_with_the_restart_named_and_no_row` |
| 12 | `a_shell_held_token_outlives_the_restart_revocation` |
