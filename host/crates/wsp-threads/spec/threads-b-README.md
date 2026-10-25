# wsp-threads, slice b: threads, transcripts, agents, conversations, forks

Port of `packages/runtime/src/threads/{threads,transcripts,agents,conversations,forks}.ts` (3,150 lines) at
`65f6b89c7`. Every `file:line` below is at that commit; `T` is `packages/runtime/src/threads/`. The rest of the
crate (sessions, turns, launch, waits, answered-start, trimmed-row, activity) is slice a's.

## Units

| Unit | TS | Rust | Spec | Tests |
|---|---|---|---|---|
| threads_tree | threads.ts:64-97, 303-362, 523-585, 642-661, 1144-1179 | `src/threads/threads/tree.rs` | `threads_tree.md` | `tests/threads_tree.rs` |
| threads_kept | threads.ts:98-301 | `src/threads/threads/kept.rs` | `threads_kept.md` | `tests/threads_kept.rs` |
| threads_cap | threads.ts:364-521 | `src/threads/threads/cap.rs` | `threads_cap.md` | `tests/threads_cap.rs` |
| threads_owed | threads.ts:24-62, 586-955 | `src/threads/threads/owed.rs` | `threads_owed.md` | `tests/threads_owed.rs` |
| threads_turn_end | threads.ts:956-1142 | `src/threads/threads/turn_end.rs` | `threads_turn_end.md` | `tests/threads_turn_end.rs` |
| transcripts_rows | transcripts.ts:94-160, 257-282, 292-360, 382-506, 546-588, 659-714 | `src/threads/transcripts/rows.rs` | `transcripts_rows.md` | `tests/transcripts_rows.rs` |
| transcripts_kept | transcripts.ts:17-92, 513-545, 590-657, 716-735 | `src/threads/transcripts/kept.rs` | `transcripts_kept.md` | `tests/transcripts_kept.rs` |
| agents_lists | agents.ts:26-324, 682-703 | `src/threads/agents/lists.rs` | `agents_lists.md` | `tests/agents_lists.rs` |
| agents_launch | agents.ts:325-681 | `src/threads/agents/launch.rs` | `agents_launch.md` | `tests/agents_launch.rs` |
| conversations | conversations.ts (273) | `src/threads/conversations.rs` | `conversations.md` | `tests/conversations.rs` |
| forks | forks.ts (240) | `src/threads/forks.rs` | `forks.md` | `tests/forks.rs` |

`src/threads/threads.rs`, `transcripts.rs` and `agents.rs` hold the module list, the area struct, its context trait
and the re-exports; each unit file adds `impl` blocks on that struct.

## The red run

One `cargo test -j 3 --no-fail-fast` in a tree with the base's crates (`rust/2448-base` at `ffccfb720`) and without
`wsp-account` and `wsp-machines`, which these files do not use: 439 tests, 438 failing on `not yet implemented`,
1 green by design (`agents_lists::the_constants_are_the_ts_values`). 408 stop first in wsp-core's `StateCell::new`,
2 in wsp-store's `event_bytes`, the other 28 in this slice's bodies.

| Test file | Tests | TS cases ported (`// TS:` lines) |
|---|---|---|
| threads_tree.rs | 25 | spec tests, values read off the TS |
| threads_kept.rs | 30 | |
| threads_cap.rs | 18 | |
| threads_owed.rs | 33 | |
| threads_turn_end.rs | 20 | |
| threads_ported.rs | 77 | 91 |
| transcripts_rows.rs | 50 | 32 |
| transcripts_kept.rs | 26 | 24 |
| agents_lists.rs | 50 | 56 |
| agents_launch.rs | 37 | 38 |
| conversations.rs | 40 | 20 |
| forks.rs | 33 | 18 |

The TS tests of these files are whole-runtime tests: 555 cases touch them, 429 asserting their behaviour (`STRONG`),
24 testing only the blob road that Proposal 3 drops. Each ported test drives its area through a fake of its context
trait (`tests/support/`) and names its TS case in a `// TS:` line.

## Shared rules (every unit)

R1. **One thread, no locks** (chapter 03, rules 3.1 to 3.3). Each area is a struct behind an `Rc`, built once from
`Rc<dyn <Area>Ctx>`. Its private state (the TS closures' maps) sits in `RefCell`s borrowed only between awaits, never
across an `.await`, an `emit`, a store call, a callback or another area's method. The runtime's shared maps
(`sessions`, `threadRecords`, `live`, `transcriptIndex`, `pendingEvents`, ...) are reached through the context trait
as `StateCell`s (wsp-core `context.rs`), whose `with` takes a closure.

R2. **Each area names what it reads as its own trait** (ruling 4 of comment 6401022701): `ThreadsCtx`,
`TranscriptsCtx`, `AgentsCtx`, `ConversationsCtx`, `ForksCtx`. One method per `ctx.<x>` the TS area calls, named in
Rust case. The wsp-threads join implements each on wsp-core's context handle. Tests drive fakes.

R3. **Time and ids go through the seams** (rules 3.15, 3.16): every `Date.now()`, `clock.now()`, `setTimeout`,
`setImmediate` and `clock.schedule` reads `ctx.clock()`; every `randomUUID()` and `randomBytes` reads `ctx.ids()`.
The TS sites that bypass the clock (`threads.ts:939, 1070, 1137`; `transcripts.ts:535, 542, 679`;
`conversations.ts:148, 162`) move onto it. `setImmediate` (`transcripts.ts:489`) is `clock.schedule(f, 0)`.

R4. **Await points are the TS's** (rule 3.4). A synchronous TS stretch has no `.await` in Rust. Named stretches:
`capFull` (whole), `capHold`, `capLeft`, `takeKept`, `holdKept`, `keep`, `record` and `append`, `settleRows` inside
`atomically`, `persistSessions` up to the chain, `copiedEvents`, `moveSession`.

R5. **Map order is insertion order** (rule 3.11): `IndexMap`/`IndexSet`, every removal `shift_remove`.

R6. **Warnings** go through `wsp_core::console::warn`, word for word as the TS writes them; tests read them with
`console::capture()`. Ids in owed-line warnings are cut with `.slice(0, 8)` (UTF-16 units; ids are ASCII).

R7. **Fire and forget.** A TS `void p.catch(...)` is a `spawn_local` of the future with the same catch; a TS
`void p` with no catch is a spawned future whose error is dropped. A failure there never reaches the caller.

R8. **JSON** keeps key order (`serde_json` `preserve_order`). A record the store keeps is written with the key order
the TS object literal gives, which each unit's spec names where it matters.

R9. **Proposal 3.** The blob road of `transcripts.ts` does not port: `readTranscript`, `writeIndex`,
`moveTranscript`, `holdTranscript`, `openInQueue`, `writeTranscript`, `moveBlobs`, the `transcripts` held map and
the blob branch of `loadIndex`, `openTranscript`, `openThread`, `transcriptReader.read`, `dropFromTranscript` and
`flushTranscript`. The Rust host always has `TranscriptRows`.

R10. **What the TS leaves to a later await.** Where the TS starts a promise and awaits it later (`Promise.all`,
`Promise.race`), the Rust starts the same work at the same point (`spawn_local` or a joined future polled at once),
so side effects land in the TS order.

## Runtime invariants this slice holds (`packages/runtime/AGENTS.md`)

| Invariant | Where | Named test |
|---|---|---|
| 2, a re-opened turn writes no row twice | `record` stamps `pos` off the index, `recordCopies` writes none twice | `transcripts_rows::invariant_2_pos_is_never_issued_twice_across_record_and_copies` |
| 3, origin stamped off the token | `threadOfToken`, `drivesThread` | `threads_tree::invariant_3_a_token_no_running_row_or_kept_process_holds_is_refused` |
| 5, a transcript that does not parse is set aside and later turns are written | `loadRowsIndex`, `settleRows` | `transcripts_rows::invariant_5_an_unread_index_keeps_writing_rows_and_skips_the_index_until_reindexed` |
| Trap: trim with `dropOldest()` (#1520, #1666, #1722) | `dropOldest` | `transcripts_rows::drop_oldest_*` |
| Trap: `end()` hands `settleCut()` a fresh literal (#1646) | `settleCut` changes the row it is handed | `threads_owed::settle_cut_changes_the_row_it_is_handed` |
| Trap: rows carry the agent's session id (#1776) | `settleCut`, `notifyEnd` read `claudeSessionId ?? id` | `threads_owed::notify_end_names_the_agents_session_else_the_row` |
| Trap: `sharedFolder()` compares checkout tops (#1664) | `sharedFolder` | `threads_turn_end::shared_folder_compares_checkout_tops_not_cwds` |

Invariants 1, 4 and 6 are not held by these files.

## What survives a host restart (chapter 02, rule 2.17)

Durable: `notify-owed` lines (each with `since`, `requestId`, `tries`, `next`), `held-starts`, the `sessions`
document, transcript rows and the index row, `attachment-keys` and `attachments` blobs, `agent-lists`. Memory only,
lost at restart: kept and warm agents, `capHeld`, `borrowed`, `leadAsks`, `heldLines` (re-held from the store by
`deliverOwed` and the wake), `sending`, `steersOut`, catalog and title caches, `titlesAsked`, the conversations
list cache, machine daemon tokens, `cloudServers`, `setupRefusals`, `accrued`, `viewedMarks` (re-read from the
sessions document by boot, which is slice a's and machines'), `unreadIndexes`, word warmth.

## Cross-crate findings

- `SessionIndexRecord` in wsp-core (`types/internal.rs`) declares `viewed` before `sessions`; `persistSessions`
  writes `{workspaceId, sessions, threads, viewed?}` (`transcripts.ts:651`, rule 2.36). `transcripts_kept` pins the
  TS order on the JSON it puts; the wsp-core type needs its field order fixed.
- `ThreadRecord` in wsp-core has no `workspaceId`; the runtime's map holds `ThreadRecord & { workspaceId }`
  (`context.ts:213`). This slice keeps it as `HeldThread { workspace_id, record }` in `threads.rs` until wsp-core
  carries one.
- `KeptProcess` in wsp-core: `box_file` is `Option<u64>`, but `boxFileSize` answers -1 where the agent wrote no file
  (`threads.ts:139`), so it needs `i64`; `cancel` is a `Box<dyn FnOnce()>`, but the TS calls a keep's cancel more than
  once (`rewarm` then `reapKept`, `threads.ts:260, 112`), so it needs wsp-core's `Cancel`.
- The adapters' `HarnessAdapter` answers an ability only by being called (`rename_session`, `aside`, `fork_session`,
  `revert`, `probe_catalog`, `session_title`, `title_for` return `None` when absent). `known` (`agents.ts:60-76`)
  and the title and rename roads test presence before building any argument. The trait needs presence answers, and it
  has no `accountModels` at all (`agents.ts:81`).
- `KeptAgent::close` cannot fail in Rust, so the TS warning for a close that failed (`threads.ts:115`) has no road.
- `computer_silent` reads marks on `HostError` (`SILENT_MARKS`); the places that turn an engine, deadline, launch or
  daemon channel error into a `HostError` must stamp them.
- Events are typed (`SessionEvent`), so an event's key order is wsp-wire's; these tests compare fields, not key
  order (rule 3.24 belongs to wsp-wire's fix round).
- Not ported by anyone yet, and named by these units' bodies: `wsp_core::adapters` (`secretsOf`, `PLAN_RESETS`),
  `wsp_core::harness_catalog` (`harnessCatalog`, `HARNESS_CATALOGS`, `catalogFromProbe`, `smallestModel`),
  `wsp_core::agent_setup` (`keyOf`, `realFolderHere`, `realFolderScript`), `wsp_core::daemon_token`
  (`daemonTokenFor`, `assertTokenShape`), each an empty file in wsp-core; and usage.ts's `accountOf`,
  `accountOnComputer`, `resetDetailsDue` in wsp-machines.
- The crate's `Cargo.toml` names `wsp-account` and `wsp-machines`; these five files use neither (each area reads the
  others through its own trait), so slice b builds in a tree where they are absent.

## What the TS leaves unclear, and the choice taken

- `persistSessions` sets `worked: true` only on a record that reads `worked: false`; a record with no `worked` stays
  without it (`transcripts.ts:623`). Pinned as written.
- `settleCut` changes the row through `sessions` by its id (trap #1646); the restart load hands it a row that is in
  `sessions` too, so the Rust takes a row id.
- `capWait` reads the harness off `sessions.get(turnId)`, which holds only while a start's held row is keyed by its
  turn id (`threads.ts:448`); pinned with rows keyed so.
- A line's warnings cut ids with `.slice(0, 8)` and thread words with `threadWord`, the same cut; ids are ASCII.
- `deliver`'s detached task is one `spawn_local` started in the call; its first await is the store put of O2, so a
  synchronous fall (O3 to O5) lands before the call returns, as in the TS.

## After-port tickets (TS behaviour kept, not changed)

1. `snapshotOf` answers `String(answer.commit)`, so a daemon answer with no `commit` becomes the snapshot `"undefined"`
   and the turn's range is asked `to: "undefined"` (`threads.ts:1071`).
2. `rowEnded`'s `session.end` listener is removed only when that turn ends; a try settled another way leaves it
   registered for the host's life (`threads.ts:645-652`).
3. `keep` on a thread that already holds a process replaces it without closing it or revoking its device; only
   `holdWarm` can reach that (`threads.ts:281-283`).
4. The kept cap counts this computer's and every provider workspace's processes together, since neither stands on a
   joined computer (`threads.ts:295`).
5. A retried line that reaches its hour gives up with `its last try failed`, not the words of the failure before it
   (`threads.ts:824`).
6. `settleRows` changes the held index in place inside the transaction (facts dropped, threads reindexed); a
   transaction that fails rolls the rows back and leaves those changes in memory (`transcripts.ts:404-438`).
7. `conversations.list` collects each store's held line as its read settles, so the order of `held` follows which
   store answered first (`conversations.ts:180`).
8. A store whose live read fails fails the whole list, while a failed list read is caught (`conversations.ts:139`).
9. `dropThreadFiles` reads no exit code, so a removal that failed reads as done (`agents.ts:488`).
