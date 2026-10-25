# wsp-threads, slice a: sessions, turns, launch, waits, answered start, trimmed row, activity

TS at `65f6b89c7`, paths under `packages/runtime/src/`. 3,146 lines in 16 units. Every body in `src/` is `todo!()`;
every test in `tests/` is red until its unit is implemented. Read this page, then the unit's own page.

| Unit | TS | Rust | Spec | Tests |
|---|---|---|---|---|
| answered_start | threads/answered-start.ts (54) | `src/threads/answered_start.rs` | `answered_start.md` | `tests/answered_start.rs` |
| trimmed_row | threads/trimmed-row.ts (29) | `src/threads/trimmed_row.rs` | `trimmed_row.md` | `tests/trimmed_row.rs` |
| waits | threads/waits.ts (108) | `src/threads/waits.rs` | `waits.md` | `tests/waits.rs` |
| launch | threads/launch.ts (167) | `src/threads/launch.rs` | `launch.md` | `tests/launch.rs` |
| activity | activity.ts (424) | `src/activity.rs` | `activity.md` | `tests/activity.rs` |
| turns_run | turns.ts:17-290, 569-741 | `src/threads/turns/run.rs` | `turns_run.md` | `tests/turns_run.rs` |
| turns_forward | turns.ts:291-569 | `src/threads/turns/forward.rs` | `turns_forward.md` | `tests/turns_forward.rs` |
| turns_reopen | turns.ts:743-866 | `src/threads/turns/reopen.rs` | `turns_reopen.md` | `tests/turns_reopen.rs` |
| sessions_stop | sessions.ts:41-207, 977-990, 1482-1485 | `src/threads/sessions/stop.rs` | `sessions_stop.md` | `tests/sessions_stop.rs` |
| sessions_start | sessions.ts:255-475, 1486-1496 | `src/threads/sessions/start.rs` | `sessions_start.md` | `tests/sessions_start.rs` |
| sessions_launch | sessions.ts:476-895 | `src/threads/sessions/start_launch.rs` | `sessions_launch.md` | `tests/sessions_launch.rs` |
| sessions_reads | sessions.ts:901-975, 1175-1187 | `src/threads/sessions/reads.rs` | `sessions_reads.md` | `tests/sessions_reads.rs` |
| sessions_acts | sessions.ts:992-1110, 1189-1262 | `src/threads/sessions/acts.rs` | `sessions_acts.md` | `tests/sessions_acts.rs` |
| sessions_settle | sessions.ts:209-243, 1112-1173 | `src/threads/sessions/settle.rs` | `sessions_settle.md` | `tests/sessions_settle.rs` |
| sessions_rewind | sessions.ts:1264-1396 | `src/threads/sessions/rewind.rs` | `sessions_rewind.md` | `tests/sessions_rewind.rs` |
| sessions_delete | sessions.ts:1398-1480 | `src/threads/sessions/delete.rs` | `sessions_delete.md` | `tests/sessions_delete.rs` |

## How the areas reach the runtime

The TS areas read one `RuntimeContext` with about 120 members. Each Rust area takes a trait of its own naming every
call it makes (ruling 4 of 6401022701): `WaitsCtx`, `TrimmedCtx`, `LaunchCtx`, `ActivityCtx`, `TurnsCtx` and
`SessionsCtx`. The wiring crate implements each on wsp-core's context handle. The tests drive one `FakeHost`
(`tests/support/`) that implements all six over shared state and records every call in order.

**A workspace entry is a handle, `Entry = Rc<dyn EntryHandle>`** (`src/pending.rs`). The TS hands `LiveWorkspace`
objects around and reads `entry.record.*` at the moment of the read; `EntryHandle::record()` answers the record as it
stands now (a copy), so a read after an await sees a write made meanwhile, as the TS's shared object does. The two
writes these files make on an entry are methods: `set_session` (turns.ts:303) and `set_checked` (turns.ts:828).

**Rows share their view.** In the TS the start road's `view`, the row in `sessions` and the closures of `runTurn` are
one object; a turn writes its row through its own `view` even after the row's key moved (turns.ts:597-650), and a
resumed turn's new row replaces an old one under the same key (turns.ts:601-608). So `SessionEntry` holds
`view: SharedView = Rc<RefCell<SessionView>>`, and the same for `turn_live` and `asked`. wsp-core's `SessionEntry`
(`types/internal.rs`) holds them by value; `src/pending.rs` carries the shared shape until the wsp-core join takes it
(the same ruling as 6401022701 point 2 for `LiveWorkspace`). A borrow of a shared cell never lives across an await.

## Rules every unit keeps

- **R0.1 Maps keep JS order.** `sessions`, `threadRecords`, and every map these files make are `IndexMap`s. A JS
  `set` on a key already held keeps its place; `delete` then `set` puts it last. So a removal is `shift_remove`,
  never `swap_remove`. Iteration (`[...sessions.values()]`, `find`, `filter(...).at(-1)`) follows that order.
- **R0.2 One clock.** Every `Date.now()`, `setTimeout` and `clock.now()` of these files reads the context's
  `Clock` (rule 3.15). The TS reads `Date.now()` in eight places (sessions.ts:495, 873, 1261; turns.ts:85, 554, 694,
  814; activity.ts:269) and a raw `setTimeout` in three (sessions.ts:87, 536; activity.ts:149, 269); the port reads
  the clock in all of them, and the tests drive a `FakeClock` and never sleep. Listed as an after-port ticket.
- **R0.3 One source of ids.** `randomUUID()` and `randomBytes(16).toString("hex")` come from `wsp_core::ids::Ids`
  (rule 3.16), in the order the TS draws them. A test that names an id scripts the draws.
- **R0.4 Warn lines.** `console.warn(x)` is `wsp_core::console::warn(x)`; tests read them with `console::capture()`
  and compare whole lines.
- **R0.5 Detached work.** A TS `void p.catch(...)` is a `spawn_local` the caller does not await. Tests run on a
  current-thread runtime inside a `LocalSet` and let detached work run with `settle().await` (yields until the
  runtime is idle), never with a sleep.
- **R0.6 Refusals.** A TS `throw new Error(line)` is `HostError` with no kind; `Object.assign(new Error(l), { kind })`
  and `refusal(line, fix, kind)` carry the kind; `usageRefusal`, `notFoundRefusal` are wsp-words' builders. Tests
  compare `message`, `fix` and `kind` whole, computing the expected sentence with the wsp-words function after the
  call under test.
- **R0.7 Absent and empty.** A TS optional field left out is `None`; `""` is `Some("")`. A spread
  `...(x !== undefined ? { x } : {})` writes no key, so serialized output has no `"x": null`.

## Runtime invariants (packages/runtime/AGENTS.md) these files hold

| Invariant | Where | Tests |
|---|---|---|
| 1. A restarted host re-opens each running turn by its handle and reads its log from byte 0, keeping a reply that landed meanwhile | turns.ts:769-864 (`reattach`) | `turns_reopen::invariant_1_reopens_the_run_by_its_handle_from_its_first_line`, `invariant_1_keeps_a_reply_that_landed_before_the_restart` |
| 2. A re-opened turn writes no row twice | turns.ts:70, 103-110, 359-362, 449-452, 494-504 | `turns_forward::invariant_2_a_reopened_turn_writes_no_delta_twice`, `invariant_2_no_subagent_compaction_or_retry_row_twice`, `invariant_2_a_written_reply_is_not_written_again`, `invariant_2_a_written_start_row_is_not_written_again` |
| 3. Origin stamped off the token | sessions.ts:374, 386, 408, 411, 504 (scope read off `origin`, never the message) | `sessions_start::invariant_3_the_tree_and_notify_by_come_from_the_callers_token` |
| 5. A transcript that does not parse is set aside and later turns are still written | turns.ts:799-804 (a transcript that does not read leaves the turn as it was) | `turns_reopen::invariant_5_a_transcript_that_does_not_read_leaves_the_turn_running_and_unreached` |

Invariants 4 and 6 are not held by these files. Traps of the AGENTS.md held here: #1776 (`answer()` looks up both
ids: `sessions_acts::answer_finds_the_running_row_by_the_agents_session_id`), #1646 (`end()` hands `settleCut` a
fresh literal: `turns_run::end_hands_settle_cut_the_turns_own_row_fields`), #1620/#1665 (a held prompt: the seed and
`awaitsPrompt` kept on the row: `turns_forward::a_start_before_the_prompt_marks_the_row_awaiting_it`).

## What survives a host restart

Only what reaches the store through `persistSessions` (the rows and `threadRecords`) and the transcript through
`record`. Everything else these files hold is memory and dies with the host: `ending`, `waitsFor`, `behindTurn`,
`heldSends`, `claimedBy`, `lastSettleAt` (sessions.ts), `warming` (launch.ts), `settling`, `said`, `unsaid`
(waits.ts), every closure of `runTurn`. A restarted host learns of a running turn only through `reattach`, of a stop's
owed end only through `kind.endOwed` (sessions.ts:580).

## After-port tickets (TS behaviour kept)

1. R0.2: eight `Date.now()` reads and three raw timers beside an injected clock.
2. `waits.ts:132`: `latestTurn` takes the last row of equal `startedAt` (`>=`), so of two rows started in one ms the
   later in map order wins; a row with no `startedAt` reads 0.
3. `sessions.ts:1136`: `lastSettleAt` lives in memory, so two settles in one ms across a restart can share a stamp,
   and a restore of one takes back the other's threads.
4. `sessions.ts:1308`: an undo answers `files: Number(back.files ?? 0)`; a daemon answer whose `files` is not a
   number gives `NaN`, which the Rust `u64` cannot hold (reads 0).
5. `activity.ts:269`: a box end polls on wall time with a raw timer, so a host on a fake clock waits out
   `END_GRACE_MS` in real time.
6. `turns.ts:597`: a row keyed by its turn id while launching changes id under a client at the handover; safe only
   because every client keys a thread by its threadId (the TS comment says so).
7. `sessions.ts:275`, `:310`: two refusals use `new Error("no thread ... in this folder")` with no kind and the
   whole id, where every other absence is `notFoundRefusal` with `threadWord`.
8. `launch.ts:283`: `existsSync` on this computer's disk for a local warm, a blocking read on the host's thread.
