# wsp-account, the join of the account and projects slices

Two wave 4 slices wrote this crate's interface, specs and failing tests on separate branches (`rust/2448-account` at
`200b65f2b`, `rust/2448-projects` at `114bfe0a5`). This file says what joining them on the real `wsp-core`,
`wsp-machines`, `wsp-engine` and `wsp-words` changed and what is still open. Nothing here is implemented: every body in
`src/` is `todo!()`.

## One live entry

Both slices held their own copy of the live workspace. They now hold one: `wsp_machines::machines::machines::Entry`,
which is `Rc<RefCell<LiveWorkspace>>` over wsp-core's `LiveWorkspace`, its record shared by `Rc` as ruling 2 of #2448
asks. The account areas read it through `crate::account::core_types::Entry` and the projects areas through
`crate::projects::workspaces::seam::Entry`, which is the same re-export.

| Was | Now |
|---|---|
| account `LiveEntry` (`Cell` and `RefCell` fields), `Live` | `Entry` |
| account `EntryMachine` (`id`, `daemon_frame`, `state`, `exec`) | the engine's `Machine`; `offers().daemon_frame` and `daemon_frame(map)` say whether the machine serves its computer's frames |
| account `ExecAnswer` | `wsp_wire::wire::machine_link::ExecResult` |
| account `PrReading` | `Joined<Option<PullRequestSeen>>` |
| account `Cancel` | `wsp_core::clock::Cancel` |
| account `Frame` | `wsp_core::reach::Frame` |
| projects `LiveEntry` (no machine) | `LiveWorkspace` through `Entry` |
| projects `Live` (the map) | unchanged name, in `seam.rs` only: `Rc<RefCell<IndexMap<String, Entry>>>` |
| projects `FreeSlot` | an alias of the account's `Release`, one definition |
| account `ChannelThread`, projects `thread_workspaces()` pairs | `ThreadRecord`, which now carries `workspace_id` (below) |
| projects `WorktreeAsk` | `wsp_wire::views::workspace_more::WorktreeAsk` |
| projects `WorktreeRemoveAsk` | `wsp_engine::copier::WorktreeRemove` |
| projects `HereDaemon` (a struct holding a read version) | `wsp_core::types::wiring::HereDaemon`, the trait: the area awaits `version()` as the TS does |
| projects `ComputerRow` | `wsp_machines::machines::boot::ComputerRow` |
| projects `EntryAsk` | `wsp_machines::machines::boot::entries::EntryAsk` (its `act` is the `AcrossAct` enum) |
| account `TerminalSignIn` | `wsp_machines::machines::brokered::TerminalSignIn` (its `program` is optional) |
| both slices' clocks in the tests (`TestClock`, `ClockOf`, the clocks of `channels_daemon`, `channels_ptys`, `pull_requests`) | `wsp_core::clock::FakeClock` |
| account `FakeMachine`, `Machine` (channels_daemon), `SimMachine` (channels_copy), projects `FakeMachine` | one `FakeMachine` over the engine's `Machine`, `tests/common/entry.rs`, included by both test halves |
| both slices' `plain`, `kinded`, `refused`, `kind_word`, `kind_named` | one `tests/common/errors.rs`, included by both test halves |

A test builds an entry with `entry_over(record, machine)`, which makes `LiveWorkspace` the way `attach` does. The engine's
`Workspace::new` is `todo!()`, so every test that builds an entry stops there until it lands. `tests/common/entry.rs`
says so.

## Added to wsp-core

- `ThreadRecord.workspace_id` (`types/internal.rs`), first in the struct as every TS literal that makes one writes it
  (`sessions.ts:831`, `boot.ts:403`). The index record stores a thread under its workspace and holds none, so an empty
  one is left unwritten on the way out and reads empty on the way in.
- `WorkspaceRecord` (`types/wiring.rs`) fields reordered to the create literals' key order, see the next section. No
  field added or removed.

## Key order: what is pinned and the accepted differences

The TS writes a record's keys in the order of the literal that made it. One Rust struct has one order, so it follows the
create literals (`machines/create.ts:63`, `sub-spawn.ts:223`), and each path that differs is an accepted difference for
the parity list.

| Path | TS order | Difference from the struct |
|---|---|---|
| folder record, `folders.ts:37` | `id, name, kind, place?, machineId, phase, golden, createdAt, project, worktree?, parentWorkspaceId?, spec, size, firstLife, idleWindowMs` | `place`, `worktree` and `parentWorkspaceId` sit elsewhere, and `idleWindowMs` is last |
| adopted record, `status.ts:155` | `id, name, kind, project, place?, machineId, phase, golden, createdAt, spec, size, firstLife, screen?` | `project` and `place` sit before `machineId`, `screen` after `firstLife` |
| record loaded from the store, `boot.ts:302` | the stored order, then `screen`, `gone`, `nappedAt` where they are new | a record keeps the order it was stored in |
| sub-spawn record, `sub-spawn.ts:223` | `lead` before `branchOfRecord` | the struct has `branchOfRecord` first |
| worktree rewrite, `folders.ts:145` and `:262` | `path, made, branch?, madeFor?` (then `settled, gone`) | `wsp-wire`'s `WorktreeFolder` writes `path, branch, made, madeFor`, which is the order `folders.ts:84` and `:194` write; not edited here |

What this crate writes as its own structs is pinned by tests: the project keys (`add_writes_the_project_keys_in_the_ts_order`),
the status rows (`s1_row_keys_and_order`), the unrecorded rows, the scope records (P3), the pull request record
(`Q8`), the usage reset's pending key (`usage_reset.md` rule 17), every daemon frame and every oracle trace compared as
a JSON string.

## Open: the real item lacks what a slice needs (the slice's test is kept)

| Where | What is missing | What stays |
|---|---|---|
| `account/usage.rs` `UsageEntry`, `LimitReading` | wsp-machines `src/usage.rs` is an empty file (no slice wrote it yet) | the two structs, with the fields the area files |
| `projects/projects/add.rs` `LandingRoad`, `LandingKind`, `LandingDeps`, `LandAsk`, `Landed`, `AddReport`, `ProjectSourceModule`, `SourceResolved`, `ResolveAsk` | wsp-core `project_landing.rs` and `project_sources.rs` are empty; wsp-machines' `LandingRoad` is the four-method sub-trait the create takes, not the whole road. `wsp_engine`'s `Landed` is a different item (provision files). | the traits and structs, as `projectLanding` and `projectSource` write them |
| `account/preferences.rs` `ServerIcons` | wsp-core `types::pending::ServerIcons` is an empty trait; the area reads `folder()` and `forget()` | the trait with its two methods |
| `account/rules.rs` `TreeFacts` | a session row (`SessionView`) has 53 fields and no rule reads more than its workspace and its two parents, which the TS reads as `rowsOn(id)[0]` | `TreeFacts` for rows only; a thread's record is `ThreadRecord` |
| `usage_reset.rs` `Resets` | `wsp_adapters::port::PlanResets::spend_command` answers a `String` and cannot refuse (ruling 5, item 5, is the adapters' round) | `Resets`, whose `spend_command` answers a `Result` |
| `HostError` `code` and `status` | wsp-words has neither yet (ruling 3, the words fix round); `bringBack` tells `no-host-cli`, `no-git-credential` and `on-default-branch` apart by the daemon refusal's code | the tests carry the code as `Kind::Other(code)`, and `AccountError::Refusal(DaemonRefusal)` holds one |
| `EngineError::MachineAlive` | not in the engine yet (ruling 3, the engine halves) | `delete` tests encode it as `Other { code: "machineAlive" }` |
| `MachineState` | the daemon frames' enum has `Starting, Running, Paused, Gone` and no `stopped`; `channels_daemon` case D12 ("something else") reads `Starting` | the case, on `Starting` |
| `FakeClock` (`wsp-core/src/clock.rs`) | no list of due moments (`due()`) and no absolute set; `advance` runs every due timer with no yield between them | `due()` is `assert_due`, over `pending()` and `due_for(at)`; the two tests that yield after each timer (`usage.rs`, `channels_copy.rs`) keep their own clock |
| `TestIds`, `MemStore` (`tests/support/base.rs`) | wsp-core's `SeqIds` and `MemStore` live under its own `tests/support`, not in `src`, so no other crate can use them | the two fakes |
| `warn` | the account areas call `ctx.warn(line)` (A6) and the projects areas call `wsp_core::console::warn` and read `console::capture()`; two conventions for one item | both kept, each slice's tests as written |
| `spawn`, `schedule`, `now` | every account context trait declares them; wsp-core's `Clock` has `schedule(f, ms)` with the function first and no `spawn` | the traits keep `schedule(ms, f)`; the join implements them on the context handle |

## Where the tests stop

Every one of the 797 tests is red on `todo!()`. There are no constant checks in this crate. One run
(`cargo test --no-fail-fast -- --test-threads=3`), by where the first panic is:

| Slice | Tests | This crate's `todo!()` | `FakeClock::new` (wsp-core) | `Workspace::new` (engine) | Anywhere else |
|---|---|---|---|---|---|
| account | 347 | 125 | 107 | 115 | 0 |
| projects | 450 | 52 | 398 | 0 | 0 |

The projects tests build their fake base first, which makes a `FakeClock`, so 398 of them stop there before they reach an
entry or a unit. The same run with the slice's own clock (before the clock was swapped for wsp-core's) stopped 276 tests
at `Workspace::new` (115 account, 161 projects), 2 at wsp-core's console capture, and the other 519 in this crate.
