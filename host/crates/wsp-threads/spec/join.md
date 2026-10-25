# wsp-threads, the join of the two threads slices

Two wave 4 slices wrote this crate's interface, specs and failing tests on separate branches (`rust/2448-threads-a` at
`7cb19b93f`, `rust/2448-threads-b` at `152035714`). This file says what joining them on the real `wsp-core`,
`wsp-machines`, `wsp-account`, `wsp-engine`, `wsp-adapters`, `wsp-wire` and `wsp-words` changed and what is still open.
Nothing here is implemented: every body in `src/` is `todo!()`, bar the 27 one-line forwarders of `impl SessionsApi for
SessionsArea` that slice a wrote (`sessions.rs`), which only name the unit method that answers each verb.

## The merge

| What | Settled how |
|---|---|
| `Cargo.toml`, `src/lib.rs` | slice a's, as the brief says; `wsp-account` and `wsp-machines` added to the dependencies; `pub mod pending` dropped with the file |
| `tests/support/mod.rs` (the only textual conflict, add/add) | both kept: slice a's is `tests/support_a/`, slice b's is `tests/support_b/`; each test file names its own with `#[path = "support_a/mod.rs"] mod support;` |
| `src/threads/{agents,conversations,forks,threads,transcripts}.rs` | slice b's real files over slice a's one-line skeleton of the same path; `src/threads/{turns,sessions,launch,...}.rs` are slice a's |
| specs | no overlap: slice a's 16 and slice b's 11 specs, and both READMEs (`threads-a-README.md`, `threads-b-README.md`), are kept; this file and `context_table.md` are new |
| tests | all kept: 621 in slice a's 16 files, 439 in slice b's 12 files (438 red and the one constant check), counts per file checked against each branch |

## One definition per item

Where both slices declared the same item, the slice that ports the TS file defining it owns it and the other imports it.

| Item | Owner | Gone |
|---|---|---|
| `Entry`, `Row` | `wsp_machines::machines::machines`, re-exported from `threads::threads` | a's `EntryHandle`/`Entry`, b's `Entry { record, machine }` |
| `Sessions`, `ThreadRecords`, `Waiting` | `threads::threads` (aliases over the real items) | a's `Rows`/`Records`, b's `StateCell<..>` maps |
| `CheckpointAsk`, `TurnChangesAsk`, `CapHeld` (now `Clone`), `CapAt`, `NotifyOn`, `KeptHand` | `threads::threads::*` (b) | a's `CheckpointAsk`, `ChangesAsk`, `CapHeld`, `CapWaitAsk`, `Registered`, `KeptHold`, `WarmHold`, `TakenKept` |
| `ForkPlan`, `StartFn` | `threads::forks` (b) | a's own `ForkPlan` (it carried a raw JSON copy), `StartFn`, `Readied` |
| `AdapterAsk`, `BuiltAdapter`, `LaunchServers`, `DefaultsOn`, `LandedImages` | `threads::agents::launch` (b) | a's `AdapterAsk`, `BuiltAdapter`, `LaunchServers`, `Resolved`, `Landed` |

`hold_kept` answers `Option<KeptHand>` in both: nothing where the runtime kept the process, the hand back where it refused
it (slice a's finding; slice b's `bool` is `is_none()`). b's `launch_adapter_for` and `adapter_for` answer a
`BuiltAdapter` where they answered a tuple.

## The row model

The TS shares one row object between `ctx.sessions`, the start road and the turn's closures. Slice a shared the row's
view, live state and asked separately (`SharedView`, `SharedLive`, `SharedAsked`); wsp-core's `SessionEntry` is plain and
wsp-machines already holds it as `Row = Rc<RefCell<SessionEntry>>` in `Rc<RefCell<IndexMap<String, Row>>>`. The join
takes that: the row is shared as a whole, `TurnStart` and `TurnState` carry `row: Row` in place of `view`, `turn_live`,
`asked` and `calls`, and both slices' `TurnRow` and a's `ReopenRow` are `Row`. A borrow of the map or of a row is never
held across an await or a call out. `ThreadRecord` carries `workspace_id` (ruling 2), so `HeldThread` and the thread and
workspace pairs are gone. wsp-machines still declares `ThreadOn` for its own `thread_records()` (gap 8 below).

## Stand-ins replaced by the real item

| Was (this crate) | Now |
|---|---|
| a's `Local`, `Joined`, b's `Answer` | `wsp_core::types::wiring::Local`, `wsp_core::context::Joined` |
| both `AskFailure` | `wsp_account::account::core_types::AccountError` (`Host`, or `Refusal(DaemonRefusal)` with the daemon's code) |
| a's `Forward` | `wsp_adapters::port::OnEvent` (the port is one-thread now: its futures are `LocalBoxFuture` and the fakes follow) |
| a's `Readied`, b's `ForkCut` | `wsp_adapters::port::{ForkedSession, ForkTurn}` |
| `BrokeredLaunch` (b) | `wsp_machines::machines::kinds::BrokeredLaunch` |
| `ServerValuesRoad` (b, with an invented `None`) | `wsp_core::types::internal::ServerValuesRoad`; `kind_server_values` answers an `Option` |
| `ProjectSeen`, `HeldProject` | `wsp_wire::views::workspace::ProjectView` |
| `Tree`, `ResumedSub` | `wsp_account::account::rules::TreeOf`, `wsp_machines::machines::sub_spawn::machine::Resumed` |
| `SessionWrite`, `ReadWindow` | `wsp_adapters::port::SessionRenameWrite`, `wsp_store::transcript_reader::ThreadRead` |
| `Through`, `ReadMark`, `EventSink`, `Handle` | `SessionUncappedEventWhy`, `ThreadStamps`, `wsp_core::types::api::DaemonEventListener`, `Rc<dyn SessionHandle>` |
| `FolderAt`, `worktrees_of`'s strings | `wsp_account::projects::folders::place::FolderPlaced`, `ListedWorktree` |
| `interrupt`'s outcome word, `reach_of`'s word | `SessionInterruptResult`, `ReachState` |
| `emit` and `pass` on `SessionsCtx`, `TurnsCtx` (bare JSON) | `wsp_wire::wire::events_union::EventUnion` |
| `usage_accounts`' pairs | `wsp_wire::types::usage::AccountsAnswer` |
| `entry_of_row`'s two ids, `entry_of_with`'s flags | `wsp_machines::machines::boot::entries::{RowAt, EntryAsk}` |
| `entry.harness_exec(ms)` | `TurnsCtx::harness_exec(entry, ms)`, as `ConversationsCtx` had it |
| `vault_server_values() -> ServerValues` | `LaunchCtx::vault() -> Env`; the area reads `wsp_catalog::signin_rows::server_values_of` |
| the 35 constants (`CATALOG_TTL_MS`, `TRANSCRIPT_CAP`, `AGENTS_KEPT`, `OWED_RETRY_MS`, `INLINE_EXEC_MS`, ...) and `agents_kept_on` | `pub use` of the real items at the same paths, with the real integer types (tests cast) |
| the test doubles of a clock | `wsp_core::clock::FakeClock` (b's `TestClock` is a thin wrapper over it, same name) |

## Open: the real item lacks what a slice needs, or is a narrower view (the slice's test is kept)

| Where | What is missing or why it stays |
|---|---|
| `launch.rs` `ThreadLaunch` | wsp-account's `ThreadLaunch` has `scoped: Option<PairedDevice>` and `env`, and no `wsp` (the TS `threadLaunch` answers `scoped?, env, wsp?`); the slice keeps `scoped_device_id` |
| `launch.rs` `SessionHeld` | wsp-adapters' `HarnessAdapter` has no `session_held` (ruling 5 of 6402177374, the adapters' round); only the Codex adapter has an inherent one |
| `threads/threads.rs` `PlaceMove` | wsp-machines' `PlaceEvent` has Joined, Changed, Removed and Absent and no `Present`, which the kept processes wait on (`place.present`) |
| `turns.rs` `UsageAccount`, `LedgerUse`, `LimitAsk` | wsp-account holds `AccountKey`, `UsageEntry` and `LimitReading` pending wsp-machines' empty `usage.rs`; they lack `turns`, and carry `label` and `road` as plain strings; `ledger_limit` and `alerts_read` take untyped values as wsp-account's own `UsageCtx` does |
| `turns.rs` `Sweep`, `agents.rs` `TurnLaunch` | `ExecStreamFactory::sweep` fails with `ExecFailure` and `MachineExecOptions` carries `killed_why` as a closure; the slice's are the narrower reads |
| `tree.rs` `SILENT_MARKS`, `computer_silent` | `ExecFailureKind::silent` reads an `ExecFailure`; nothing real reads a `HostError` by its marks that way, and `HostError.marks` carries two of the six |
| `turn_end.rs` `TurnChangesAsk` | wsp-machines' `CardAsk` is a narrower copy of the same ask (no `wrote`); one shape has to be chosen, here the wider |
| `sessions.rs` `StampsOf` | `Stamped::Same` is missing: the slice has the per-thread closure case only |
| `sessions.rs` `git_restore` | answers an untyped map; wsp-frames has a typed `GitRestoreReply` |
| `activity.rs` `DaemonAsk`, `ActivityDoor` | the TS reads a channel through `overChannel`'s `ask`, which throws the refusal; the real `DaemonChannel` answers the raw frame and wsp-account's `over_channel` does the ask. wsp-core's `types::pending::ActivityDoor` is an empty trait, so the three-method trait stays here |
| `answered_start.rs` `StartAtWorkspaces` | the TS is `Pick<Runtime["workspaces"], "folderFor" | "takeSetupLine">`; `WorkspacesApi` has 50 methods, so the two-method view stays (same signatures) |
| `LaunchServers`, `KeptHand`, `SteerSent` | picks of `HarnessStartOptions`, `KeptProcess` and `StartOptions` (the TS `Pick`/`Omit` types); the real structs are wider |
| `threads/threads/owed.rs` `Owed` | a different thing from wsp-machines' `Owed` (the thread-ends debt record) |
| `TestBus`, `CountingIds`, `FakeStore` (support_b) | wsp-core has no bus double, its `SeqIds` lives in its own tests, and `MemStore` neither logs writes nor fails them |
| `HarnessAdapter` | no `account_models` (b's fake keeps it as an inherent method), no presence reads for `warm`, `rename_session`, `aside`, `session_held` (ruling 5) |
| `StateCell` | wsp-core's own handle is not what the joined crates use for `sessions` and `thread_records` (they use `Rc<RefCell<IndexMap<..>>>`), so b's maps changed; the transcript state and the index flushes stay `StateCell` |
| `ThreadOn` (wsp-machines) | still pairs `workspace_id` with a flattened `ThreadRecord`; the crate's `thread_records()` is `ThreadRecords` |
| `FakeClock` | `due()` and an absolute set are missing, as the account join found; b's tests did not need them |

## Key order

`SessionIndexRecord` (wsp-core `types/internal.rs`) now writes `workspaceId, sessions, threads, viewed?`, the order
`persistSessions` writes (`transcripts.ts:651`), where it wrote `viewed` second. wsp-core's test
`record_session_index_record_writes_its_keys_in_the_order_persist_sessions_does` pins it. No other record changed: the
rest of slice b's records (`Owed`, the held start `workspaceId, threadId, turnId, sessionId, harness, place, prompt,
notify?`, the person row) were already in the TS order and are pinned by its tests. An event's own key order (`at` and
`pos` after the event's keys, `copiedFrom` last) is wsp-wire's to keep; these tests compare fields.

## Tests whose setup or claim changed

Every test is kept and every count equals its branch's. These changed their form:

- `turns_run`: `send_back_carries_the_unread_ids_and_whether_the_turn_was_stopped_and_only_the_turns_own_row` can no longer say the old `TurnRow` held only three fields; it checks the row handed over is the turn's own (`Rc::ptr_eq`).
  `end_hands_settle_cut_the_turns_own_row_fields` says the row handed to `settle_cut` is the turn's row.
- `turns_run`: the usage row test (`a_limit_is_kept_and_pushes_its_usage_row`) answers a typed `AccountsAnswer`; `AccountRow` has no `used` field, so its expected row carries `plan` where it carried `used`.
- `sessions_start`, `sessions_launch`: `ForkPlan` has no `PartialEq` (`same_plan` compares field by field) and has no `raw` (never asserted); its source needs a live entry.
  The check that the held agent's waiting box is the start's own box reads the flag through the `TurnWaiting` closure.
- `sessions_acts`: `rename_names_every_row_of_the_session` recorded the exec bound through its own entry type; a `TurnsCtx::harness_exec` hook records it now (the assertion on it still passes with nothing recorded, as it did).
- `sessions_reads`: `ThreadRead` carries a function pointer and is not `PartialEq`; the test converts it to a comparable window.
- `threads_ported`: the `turn_row` helper copies the row where the old `TurnRow` was a snapshot.
- `conversations`: `AccountError::Host` carries no code, so a test that read a code off a channel failure would not see it (none does).

## Where the tests stop

Every one of the 1,060 tests is red on `todo!()` except `agents_lists::the_constants_are_the_ts_values`. One run
(`cargo test --no-fail-fast -- --test-threads=3`), by where the first panic is:

| Slice | Tests | wsp-core `FakeClock::new` | wsp-core console capture | engine `Workspace::new` | wsp-store `event_bytes` | This crate's `todo!()` | Anywhere else |
|---|---|---|---|---|---|---|---|
| a | 621 | 588 | 14 | 1 | 0 | 18 | 0 |
| b | 439 | 379 | 0 | 34 | 2 | 23 | 0 (1 green constant check) |

Because 967 of the 1,060 stop at the clock, nothing past it is proven by this run beyond compiling; the build and the
one-panic-site check are all the join can show until `FakeClock::new` lands.
