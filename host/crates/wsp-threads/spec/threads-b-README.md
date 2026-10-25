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
