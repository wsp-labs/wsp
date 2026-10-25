# Context trait table: wsp-threads against wsp-core's runtime context

Read at the join of the two wave 4 threads slices (rust/2448-threads-join), TS at 65f6b89c7 (`packages/runtime/src/context.ts`, `runtime.ts`, `threads/*.ts`, `activity.ts`). Every area body in wsp-threads is still `todo!()`, so the table is built from the trait declarations, their doc comments and the TS area files, not from call sites in Rust. The format is `wsp-machines/spec/context_table.md`'s and `wsp-account/spec/context_table.md`'s.

Each row is one trait method. "Home" is exactly one of: `wsp-core <Area> (empty)` (the area trait whose TS interface in context.ts declares the member; the area trait is empty in wsp-core, and an area this crate fills itself says so in the Note), `wsp-core item: <path>` (a real item exists; items of `wsp-engine`, `wsp-wire`, `wsp-words`, `wsp-store`, `wsp-adapters`, `wsp-catalog`, `wsp-machines` and `wsp-account` are named by their path), `opts: RuntimeOptions.<field>` (TS reads it off the options passed to createRuntime, and wsp-core has the field), or `gap: <what is missing>`.

Two slices wrote these traits, each on its own, and each area keeps its own context trait (ruling 4 of #2448, comment 6401022701). Slice a's six traits (`WaitsCtx`, `TrimmedCtx`, `LaunchCtx`, `ActivityCtx`, `TurnsCtx`, `SessionsCtx`) and slice b's five (`ThreadsCtx`, `TranscriptsCtx`, `AgentsCtx`, `ConversationsCtx`, `ForksCtx`) are listed together. `SessionsCtx` has `LaunchCtx` and `TrimmedCtx` as supertraits, which are counted under the trait that declares them. The rows were written by three readers against the traits as they stand after the join (the shared items are the real ones: `Entry`, `Row`, `Sessions`, `ThreadRecords`, see `join.md`).

## Summary

425 methods in 11 traits.

Methods per trait:

| Trait | File | Methods |
|---|---|---|
| `SessionsCtx` | src/threads/sessions.rs | 148 |
| `TurnsCtx` | src/threads/turns.rs | 60 |
| `LaunchCtx` | src/threads/launch.rs | 33 |
| `WaitsCtx` | src/threads/waits.rs | 4 |
| `TrimmedCtx` | src/threads/trimmed_row.rs | 3 |
| `ActivityCtx` | src/activity.rs | 25 |
| `ThreadsCtx` | src/threads/threads.rs | 46 |
| `TranscriptsCtx` | src/threads/transcripts.rs | 19 |
| `AgentsCtx` | src/threads/agents.rs | 42 |
| `ConversationsCtx` | src/threads/conversations.rs | 21 |
| `ForksCtx` | src/threads/forks.rs | 24 |
| Total | | 425 |

Methods per Home kind:

| Home kind | Methods |
|---|---|
| wsp-core <Area> (empty) | 189 |
| wsp-core item | 208 |
| opts | 10 |
| gap | 18 |
| Total | 425 |

Area homes (the `wsp-core <Area> (empty)` rows), by area:

| Area trait | Methods |
|---|---|
| ThreadsArea | 66 |
| RuntimeCore | 47 |
| AgentsArea | 33 |
| TranscriptsArea | 27 |
| UsageArea | 5 |
| SlatesArea | 4 |
| WaitsArea | 2 |
| ActivityArea | 1 |
| TurnsArea | 1 |
| RecordsArea | 1 |
| SessionsArea | 1 |
| BuildersArea | 1 |

## Gaps worth the coordinator's attention

### From the sessions traits

1. TS free functions put on the trait (no ctx member, no wsp-core item). `fork_opened`, `fork_turn_of`, `fork_start`, `ready_fork`, `copy_images`, `pay_cut_owed`, `move_session` stand for forks.ts functions that are already ported in this crate (threads/forks.rs), where each takes a `ForksCtx` and not `SessionsCtx`. The seven trait methods are wrappers over them; the units can call the free functions with a `ForksCtx` and the seven go. Two shapes differ: `ready_fork` (the free function takes a refusal closure, the trait the harness name) and the `Option<Caller>` against `Option<&Caller>` of `fork_opened` and `fork_start`. `record_copies` is the same case folded into a TranscriptsArea member (forks.ts `copied_events` then `ctx.recordCopies`). `launch_area` stands for `launchArea(ctx)` (launch.ts), built once inside sessionsArea; it has no ctx member and is `LaunchArea::new(Rc<dyn LaunchCtx>)` here.
2. Helpers over several members. `project_computer_name` (BuildersArea.placeName and ChannelsArea.projectHeld, two areas, gap) and `git_restore` (FoldersArea.queued and ChannelsArea.withDaemon, gap, both real items). Folded calls that keep a single Home: `elsewhere_refusal` (adds `ChannelsArea::project_held`), `notify_end` (`notifyEnd` and `tellAs`, both ThreadsArea), `write_session` (`writeSession` plus the adapter's `renameSession` over `harness_exec`), `on_turn_settled` (the turn-id filter over `bus.on("*")`), `has_link`, `ping` and `on_place_absent` (each a TS line over `placeDoorOf().link` or `.on`). Helpers over one map the context holds: `live_entry`, `taken`, `cap_held`, `host_writes`, `kept_warm`, `kept_writes`, `booting`, `cancel_resume_timer`, `index_pos`, `index_workspaces`, `subagent_known` (`taken`, `index_pos`, `index_workspaces` and `subagent_known` read `transcriptIndex`, as `BootRuntime::index_pos` and `index_children` do).
3. Methods with no TS member: none by name. `launch_area` has no ctx member (item 1). `open_thread`'s doc line names `ctx.seedFor(...)`, which does not exist in TS; the member is `ctx.openThread` and the seed filter is the caller's.
4. Real item in another shape than the trait method. Result against a bare answer: `opens_in`, `of_threads_repository` (real `Result<bool>`), `checkout_of` (real `Result<String>`), `spawn_guard` (real `Result<Release>`, the trait drops the release), `emit` and `pass` (real `Result<u64>` and `Result<()>`), `push_head` (real `Result<()>`), `proven` (real answers the `WorkspaceView`), `drop_queued` (real a bare bool, the trait `Result<bool>`), `sub_done` (real async, trait sync). A typed argument against a loose one: `worktree_gone` (`WorktreeSettledWhy`), `away_for` (`WorkspaceLike` and `AcrossAct`), `acts_on` (`WorkspaceLike`), `elsewhere_refusal` (`&ProjectView`), `reaches_row` (`&RowAt`), `mark` (`Stamped`), `listed_rows` (`&[Row]`), `hold_queued` (`HoldAsk`, answers `StartedTurn`), `sub_delete` (`check: bool`, answers `SubDeleted`), `resume_sub` (prompt as `Option`), `row_gone` (thread id as `Option`), `workspace_place` (`&WorkspaceView`), `search_in` (`&TranscriptIndex`, `usize`), `aside` (an optional future of `AsideAnswer`). A different answer: `tree_records` (adds the workspace id to each tuple), `place_rows` (a present door), `placed_rows` (async), `on_turn_settled` and `on_place_absent` (an unlisten as `Box` and a `u64` id for `off`), `end_thread` and `end_owed` (sync, no Result, `Local` against `Joined`), `ping` (a `Frame` reply), `has_link` (a handle against a bool). `Bus::emit` is a concrete struct method, not on the `EventBus` trait.
5. Empty wsp-core files and traits the rows lean on. Every `wsp-core <Area> (empty)` row is an empty area trait in wsp-core's context.rs (RuntimeCore, ThreadsArea, AgentsArea, TranscriptsArea, TurnsArea, WaitsArea, ActivityArea, SlatesArea), so none of them has a Rust member yet; wsp-threads fills ThreadsArea, AgentsArea, TranscriptsArea, TurnsArea, WaitsArea and ActivityArea itself. SlatesArea belongs to wsp-slates, which is not in the workspace, and wsp-core's `Slates` and `ActivityDoor` (types/pending.rs) are empty traits. `moduleOf` (KindsArea) is empty but `KindModule` is a real trait, which `end_thread` and `end_owed` use.
6. Near names and duplicates to keep apart. `reap_kept` is declared on both `SessionsCtx` and its supertrait `LaunchCtx` with one signature. `rows_on` is `AgentsArea.rowsOn`, not ThreadsArea. `brokered` is `KindsArea.brokered` (the head-start listed it under ChannelsArea; `ChannelsArea::brokered` is a private helper over an entry). `cap_wait` is `ctx.capWait`, while `BootRuntime::cap_wait` is `ctx.capHeld`. `workspace_place` is `place-state.ts workspacePlace` (the record's `place`), not `ChannelsArea::place_id_of`. `refresh_title`'s second argument is TS `force`. `wake` and `delete_workspace` are `WorkspacesArea.workspaces` calls, so their Home is `WorkspacesApi`, whose bodies are the free functions `phases::wake` and `phases::delete` in wsp-account (those take a `PhasesCtx`, not a context). `sessions`, `records`, `live_entry` (as `live`), `index_pos` and `cancel_resume_timer` are also declared on wsp-machines' `BootRuntime` (`sessions` and `live` on `DaemonRuntime` too) as separate stand-ins for the same RuntimeCore members.

### From the turns traits

Read at rust/2448-threads-join. 125 methods in 5 traits: 62 area (empty), 55 item, 2 opts, 6 gap.

1. TS free functions put on a trait, with no Rust item. `TurnsCtx::mode_label` and `LaunchCtx::not_run_refused` stand for `harnessCatalog` and `notRunRefused` (harness-catalog.ts). wsp-core `harness_catalog.rs` is an empty file, so nothing below can be called. The doc comment on `LaunchCtx` already says the trait carries them for that reason.
2. Helpers over several members, so no single home. `LaunchCtx::catalog_on` is `harnessCatalog(...)` (free function, item 1) then `AgentsArea.catalogOn`; the second half is this crate's own area. `ActivityCtx::local_channel` is `KindsArea.localRoad`, `ChannelsArea.channelOver` and, where `guarded`, `ChannelsArea.killGuarded`; each part has a real item (wsp-machines `KindsArea::local_road`, wsp-account `ChannelsArea::channel_over` and `kill_guarded`), but the trait answers its own `DaemonAsk` wrapper over `overChannel`'s `ask`, not a channel.
3. Methods with no context member. `LaunchCtx::folder_exists` is node `existsSync`, and `ActivityCtx::host_pid` is `process.pid`; both are the host-process seam the account table already asks for (its `ChannelsCtx::host_pid`, `PhasesCtx::path_exists`). `ActivityCtx::clock` has a Home (`Clock`) but the TS never calls `ctx.clock` there: it reads `Date.now()` and `setTimeout` (activity.ts:149, 269), so a fake clock now drives timers a TS fake clock never drove. `TurnsCtx::harness_exec` is a method turns.ts never calls (`harnessExec` is called in sessions.ts, conversations.ts and agents.ts); the trait carries it with no use in the turns code. `LaunchCtx::adapter_warms` is `probe.warm === undefined`, a member of the harness adapter, not of the context.
4. Real item in another shape than the trait method. Entry types: wsp-machines' `KindsArea::exec_factory_for` and `thread_folder` take `&LiveWorkspace` where the traits take `&Entry` (`Rc<RefCell<LiveWorkspace>>`). Answers: `Bus::emit` answers `Result<u64, HostError>` and `Bus::pass` a `Result<(), HostError>` where `TurnsCtx::emit` and `pass` answer nothing; `ReachArea::read_checkout` answers `Option<Checkout>` (trait: `Result<(), HostError>`); `ReachArea::read_lead_of` is sync (trait: async `Result`); `ReachArea::read_disk` and `PullRequestsArea::settle_tree` answer `()` (trait: `Result`); `PullRequestsArea::read_pull_request` takes a `whole` argument and answers the seen state; `RulesArea::refusal_for` answers `Result<Option<String>, HostError>` over `Option<&WorkspaceLike>` where `ActivityCtx::refusal_for` answers `Option<HostError>` over a record; `BootArea::ready` answers a shared `Ready` handle (both `LaunchCtx::ready` and `ActivityCtx::ready`); `DeviceDoor::revoke` answers `Result<bool, HostError>` (`LaunchCtx` and `TurnsCtx::revoke_device` answer `Result<(), HostError>`); `UsageArea::usage_accounts` answers an `AccountResult`. Arguments: `RulesArea::drives` takes a `machine_id` the trait lacks; `BootArea::end_snooze_for` takes `(Option thread_id, Option root_thread_id)` where the trait takes the row; `BootArea::entry_of` takes an `EntryAsk` and `Option<&Caller>`; `PlaceDoor::name_of` answers a `String` where `ActivityCtx::place_name` answers an `Option`; `PlaceDoor::channel` is sync over an `Rc<dyn Fn(Map)>` listener and answers an `Rc<dyn DaemonChannel>`. Keys: `ChannelsArea::turn_groups` is keyed `i64`, the trait keys `u32`; `RuntimeState::preferences_held` is a `serde_json::Value`, `LaunchCtx::preferences_held` answers `Option<Preferences>`. Two `HarnessExec` shapes: wsp-engine's `run_env::harness_exec` answers an `Arc<dyn Fn>` type, the trait answers wsp-adapters' `HarnessExec` trait object (`TurnsCtx::harness_exec`).
5. Empty files and stand-ins the rows lean on. wsp-core `harness_catalog.rs` (items 1 and 2: `mode_label`, `not_run_refused`, `catalog_on`), `agent_setup.rs` (`launch_of`: `RuntimeCore.setups`), `plan_alerts.rs` (`alerts_read`), wsp-machines `usage.rs` (`ledger_add`, `ledger_limit`, `burn_add`, `alerts_read`; wsp-account declares its own `UsageEntry` and `alerts_read` on `UsageCtx`), and wsp-core `types::pending` `Slates` and `PlaceDoor` as empty traits (`slates_turn_ended`, `close_servers`; the place door rows go to wsp-machines' real `PlaceDoor` in `places/types.rs`, the member being `RuntimeCore.placeDoor`). `SlatesArea` is in wsp-core `context.rs` but not in the brief's list of area names; two rows use it, as the earlier tables do.
6. The five usage rows (`ledger_add`, `ledger_limit`, `alerts_read`, `burn_add`, `usage_accounts`) cross into wsp-account's `UsageArea`. Four of the five have no Rust item behind them, so the call has nowhere to land until the ledger is ported. `TurnsCtx::usage_account_of` and `usage_computer_of` are this crate's own (threads.ts:1149, 1158), but `usage_account_of` calls `accountOnComputer` of usage.ts, which wsp-account declares only as a method of its own `UsageCtx::account_on_computer`, with no Rust item behind it.
7. Both sides of 43 area rows are this crate's: ThreadsArea 22, AgentsArea 15, TranscriptsArea 5 and WaitsArea 1 (`ActivityCtx::latest_on`, `kept_pid`, `reap_kept_now` and `settles_thread` among them). The join must implement `TurnsCtx`, `LaunchCtx`, `WaitsCtx` and `ActivityCtx` over this crate's own areas. `TurnsCtx` alone reaches `ThreadsArea` through 13 methods (`keep_checkpoint`, `settle_cut`, `notify_end`, `send_back`, `hold_kept` and others).
8. Helpers over one map or member, kept as that member's row: `TurnsCtx::live_entry`, `ActivityCtx::live_entry` (one `live.get`), `LaunchCtx::kept_box_cap` (`keptBox(...)?.cap`), `ActivityCtx::kept_pid` (`keptAgents.get(id)?.pid`), `TrimmedCtx::thread_words` (two gets over `transcriptIndex`), `TurnsCtx::held_at_close` (a `Set` of closures against a `Vec` of `Rc`, so a delete must compare pointers), `TurnsCtx::pull_request_state` (a field of the workspace record, whose item is the `WorkspaceRecord::pr` field).
9. Same method name on more than one wsp-threads trait: 40 names touch these five traits (`clock` on 8 traits, `sessions` on 9, `closing` on 5, `latest_on` on 6, `adapter_for`, `revoke_device`, `harness_exec`, `ready`, `record`, `emit`, `place_name` and more). A join type that implements several of them needs the trait named at each call. Signatures differ for the following, beyond async against a boxed future: `TurnsCtx::notify_end` against `SessionsCtx::notify_end` (the latter takes `by` and `road` where the turn folds `tellAs` into the call), `TurnsCtx::refresh_title` (takes a `&Row`) against `SessionsCtx::refresh_title` (takes a `SessionView`), `ActivityCtx::place_name` (an `Option`) against `ThreadsCtx`, `AgentsCtx` and `ForksCtx::place_name` (a `String`), `ActivityCtx::place_rows` (a `Result`) against `SessionsCtx::place_rows` (an `Option`), `TurnsCtx::emit` (answers nothing) against `TranscriptsCtx::emit` (a `Result`), and `ActivityCtx::with_daemon` (a `Value` frame, a map answer) against `ThreadsCtx` and `ConversationsCtx::with_daemon` (an `AccountError`, a `Value` answer).

### From the threads traits

Read at the join branch rust/2448-threads-join (4c0474a82 plus its working tree), TS at 65f6b89c7. `Entry` below is wsp-machines' `Rc<RefCell<LiveWorkspace>>`; items that take `&LiveWorkspace` are noted in their rows.

1. TS free functions put on a trait, or folded with context members.
   - No Rust item: `TranscriptsCtx::rotate_daemon_token` (`rotateDaemonToken` and `daemonTokenPathOf`, daemon-token.ts; wsp-core `daemon_token.rs` is an empty file). `TranscriptsArea::new` also calls `assertTokenShape` and `daemonTokenFor` from that file, and no trait method names them. `AgentsCtx::process_home` is node's `homedir()`, the same gap as wsp-account's `PreferencesCtx::home_dir`.
   - A real item exists, folded with a context member: `AgentsCtx::at_provider` (`wsp_core::types::wiring::at_provider` with `ctx.places`), `AgentsCtx::computer_runs_in_folder` (`wsp_words::projects::runs_in_folder` over `KindsArea::kind_of`), `ConversationsCtx::harness_exec` (`wsp_engine::run_env::harness_exec`).
2. Helpers over several members, so no one area is their home.
   - `AgentsCtx::exec_factory_for` stands on `KindsArea.execFactoryFor`, `ChannelsArea.turnLimitOf` and `SubSpawnArea.subKilledWhy`. The trait's `TurnLaunch` carries only what the launch adds, and the join lays it over the place's turn limits.
   - `TranscriptsCtx::transcript_state` and `ConversationsCtx::transcript_state` are one handle over `transcriptIndex`, `pendingEvents`, `pendingBytes` and `unreadIndexes`. The TS also destructures `transcripts`, `transcriptBytes`, `indexFor` and `sizeOf` in the transcripts area; no method carries them (the blob road is not ported).
   - `ConversationsCtx::preferences` is two members (`state.preferencesHeld` and `preferences.get()`).
   - `ThreadsCtx::with_daemon` and `ConversationsCtx::with_daemon` are the one-frame form of `ChannelsArea::with_daemon`, which takes a work closure.
   - `ThreadsCtx::login_stands` and the six `AgentsCtx::kind_*` methods are `moduleOf(kind)` plus one `KindModule` method. `KindsArea::module_of` is a real body in wsp-machines, but wsp-core's `KindsArea` is empty.
   - `AgentsCtx::signs_in` folds two optional chains (`agentsReader?.signsIn?.`).
   - `ForksCtx::place_name` is `BuildersArea.placeName` (`placeDoor?.nameOf(place)`, else this computer's hostname, else the provider key name), not `PlaceDoor::name_of` alone.
   - Helpers over a shared map: `ThreadsCtx::entry`, `live_records`, `taken`, `boot_work`, `kept_writes`; `TranscriptsCtx::entry`, `is_live`; `AgentsCtx::entry`; `ConversationsCtx::live_project`; `ForksCtx::kept_writes`, `live_ids`, `running_turn`, `launching_turn`.
3. Methods with no ctx member (TS reads a node builtin direct): `ThreadsCtx::ids` (`randomUUID`), `TranscriptsCtx::ids` (`randomBytes`), `ConversationsCtx::clock` (`Date.now()`; `ctx.clock` is not read there), `AgentsCtx::process_home` (`homedir()`). The first three have a real home in `wsp_core::ids::Ids` and `wsp_core::clock::Clock`; `process_home` is one of the two `gap` rows.
4. A real item exists in another shape than the trait method.
   - Answer differs: `ThreadsCtx::revoke_device` (`DeviceDoor::revoke` answers `bool`), `checkout_of` and `project_held` (answer `Result`, in `ThreadsCtx`, `ForksCtx`, `ConversationsCtx`), `root_of` (`String`, not `Option<String>`), `AgentsCtx::place_agent_home`, `brokered_launch`, `push_head` (also `TranscriptsCtx::push_head`), `ForksCtx::entry_of_row` (answers `Option<Entry>`, so the not-found refusal at forks.ts:74 has nowhere to go), `ready` (a `Ready` handle, in `AgentsCtx` and `ForksCtx`), `ConversationsCtx::folders_of` (`Vec<Entry>`, not records), `ThreadsCtx::place_threads_at` and `place_mem_mb` (typed `u64` fields).
   - Arguments differ: `ThreadsCtx::interrupt` (extra `task`), `mark` (`Stamped`), `sub_line` (`f64`), `end_snooze_for` (two ids, not the row), `turn_snapshot_ms` (`u64`); `vault_on`, `know_broker` and `brokered_launch` take `&LiveWorkspace`; `AgentsCtx::place_exec` (`PlaceExecOpts`), `exec_factory_for` (`Option<MachineExecOptions>`); `ConversationsCtx::reaches_row` (`RowAt`), `entry_of` (`EntryAsk`), `folder_for` (`FolderAsk`); `ForksCtx::worktree_folder` (`Option` parent and checkpoint), `remove_worktree` (`RemoveAsk`), `git_here` (`&[&str]`, timeout, stdin).
   - Another type: `ThreadsCtx::on_place_move` (`PlaceDoor::on` answers a `u64` for `off(id)`; the real `PlaceEvent` enum has Joined, Changed, Removed and Absent but no Present variant, though TS `PlaceEvent` has `place.present` and the kept processes wait on it), `TranscriptsCtx::emit` (`Bus::emit` is a method of the concrete struct, not of `EventBus`), `TranscriptsCtx::rows` (`Store::transcripts` answers `Option<&dyn TranscriptRows>`), `ConversationsCtx::harness_exec` (wsp-engine's `HarnessExec` is an `Arc<dyn Fn>`, the trait answers wsp-adapters' `HarnessExec` trait), `AgentsCtx::kind_server_values` and `kind_ends_threads` (the item has no entry argument and never answers absent, and `end_thread` has no presence test that does not run the end).
5. Empty files and empty traits the rows lean on.
   - wsp-core `agent_setup.rs`: `AgentsCtx::setup_off`, `setup_config_dir`, `setup_launch` (home `RuntimeCore`, member `ctx.setups`).
   - wsp-core `daemon_token.rs`: `TranscriptsCtx::rotate_daemon_token`.
   - wsp-core `types/pending.rs` `AgentsReader` (an empty trait) and wsp-machines `agents_read.rs` (empty): `AgentsCtx::signs_in`.
   - wsp-machines `usage.rs` (empty): `ThreadsCtx::ledger_limits` (home `UsageArea`). wsp-account's `UsageCtx::limits` declares the same call untyped.
   - wsp-machines `images/builders.rs` (empty) and no `BuildersArea` body anywhere: `ForksCtx::place_name`.
6. Near names and loops worth knowing.
   - `project_held` in `ThreadsCtx`, `ConversationsCtx` and `ForksCtx` is `ctx.projectHeld` (ChannelsArea, throws on an unheld id), not `ctx.projectsHeld.has`. `ConversationsCtx::folders_of` is `CreateArea.foldersOf`, not the area's own `folders_of(entry)`. `rows_on` is `AgentsArea.rowsOn`, not `ThreadsArea`. `ThreadsCtx::place_name` is `PlaceDoor.nameOf`; `ForksCtx::place_name` is `BuildersArea.placeName`.
   - The five traits call each other's areas through the context: `ThreadsCtx` calls the transcripts area (`record`, `persist_sessions`, `keep_sent_images`, `open_thread`, `kept_writes`) and the sessions area (`stop_try`, `interrupt`, `start`); `AgentsCtx` calls the threads area (`write_session`) and the transcripts area (`persist_sessions`); `TranscriptsCtx` calls `AgentsArea::rows_on` and `ThreadsArea::latest_on`; `ConversationsCtx` calls both of those and `AgentsArea`'s label, off and default-pick methods; `ForksCtx` calls all three. The join needs the late-filled slots for these, as `RuntimeContext` has.

## Table A: context traits

## SessionsCtx (host/crates/wsp-threads/src/threads/sessions.rs, 148 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `SessionsCtx` | `clock` | `clock` (destructured from ctx, sessions.ts:42; RuntimeCore.clock) | wsp-core item: wsp_core::clock::Clock | `now()` answers i64 and `schedule(f, ms)` takes the function first; sessions.ts only calls `clock.now()` |
| `SessionsCtx` | `sessions` | `ctx.sessions` (RuntimeCore) | wsp-core RuntimeCore (empty) | handed out as `Rc<RefCell<IndexMap<String, Row>>>`; wsp-machines' `BootRuntime::sessions` is the same member |
| `SessionsCtx` | `records` | `ctx.threadRecords` (RuntimeCore) | wsp-core RuntimeCore (empty) | `ThreadRecords` map; `BootRuntime::thread_records` is the same member |
| `SessionsCtx` | `live_entry` | `live.get(workspaceId)` (sessions.ts:290,296,321; RuntimeCore.live) | wsp-core RuntimeCore (empty) | helper over the map |
| `SessionsCtx` | `latest_on` | `ctx.latestOn` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `rows_on` | `ctx.rowsOn` | wsp-core AgentsArea (empty) | this crate's own area; not ThreadsArea |
| `SessionsCtx` | `thread_runs` | `ctx.threadRuns` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `running_on` | `ctx.runningOn` | wsp-core ThreadsArea (empty) | this crate's own area; TS answers a `LiveSession` (a row with its handle), the trait a `Row` |
| `SessionsCtx` | `launching_on` | `ctx.launchingOn` | wsp-core ThreadsArea (empty) | this crate's own area; TS answers `{ turnId, launch }`, the trait a `Row` |
| `SessionsCtx` | `entry_of_row` | `ctx.entryOfRow(row, origin, talk, { beingMade })` | wsp-core item: wsp_machines::machines::boot::entries::BootArea::entry_of_row | same shape (`&RowAt`, origin, talk, being_made; `Result<Option<Entry>>`) |
| `SessionsCtx` | `entry_of_with` | `ctx.entryOf(id, origin, { now, act, thread, beingMade })` | wsp-core item: wsp_machines::machines::boot::entries::BootArea::entry_of | same shape (`EntryAsk`) |
| `SessionsCtx` | `record` | `ctx.record` | wsp-core TranscriptsArea (empty) | this crate's own area |
| `SessionsCtx` | `emit` | `bus.emit` | wsp-core item: wsp_core::types::events::Bus::emit | real answers `Result<u64, HostError>`, trait answers `()` |
| `SessionsCtx` | `pass` | `bus.pass` (sessions.ts:1204) | wsp-core item: wsp_core::types::events::Bus::pass | real answers `Result<(), HostError>`, trait answers `()` |
| `SessionsCtx` | `on_turn_settled` | `bus.on("*", e => session.start or session.end of this turnId)` (sessions.ts:537) | wsp-core item: wsp_core::types::events::EventBus::on | the filter on turn id and the two event types is inside the method; real `on(event_type, listener)` takes an `EventListener` that answers a Result and answers a `Box<dyn Fn()>`, the trait an `Rc<dyn Fn()>` |
| `SessionsCtx` | `persist_sessions` | `ctx.persistSessions` | wsp-core TranscriptsArea (empty) | this crate's own area |
| `SessionsCtx` | `push_head` | `ctx.pushHead` | wsp-core item: wsp_machines::machines::boot::rows::BootArea::push_head | real answers `Result<(), HostError>`, trait answers `()` |
| `SessionsCtx` | `agent_label` | `ctx.agentLabel` | wsp-core AgentsArea (empty) | this crate's own area |
| `SessionsCtx` | `waits` | `ctx.noteWaited`, `ctx.noteRetry` (sessions.ts:252-253) | wsp-core WaitsArea (empty) | this crate's own area; the trait hands out the whole `WaitsArea` where TS reads two members |
| `SessionsCtx` | `activity` | `ctx.activity.meet` (sessions.ts:1134,1420,1422) | wsp-core ActivityArea (empty) | this crate's own area (activity.ts `ActivityArea.activity`); wsp-core's `ActivityDoor` in types/pending.rs is an empty stand-in, the trait returns this crate's own |
| `SessionsCtx` | `place_away` | `ctx.placeAway` (RuntimeCore) | wsp-core RuntimeCore (empty) | |
| `SessionsCtx` | `has_link` | `ctx.placeDoorOf().link(place) !== undefined` (sessions.ts:74-76) | wsp-core item: wsp_machines::places::types::PlaceDoor::link | real answers `Option<Rc<dyn DaemonReach>>`, trait answers a bool; the door itself is `KindsArea.placeDoorOf` |
| `SessionsCtx` | `ping` | `link.request("ping")` (sessions.ts:91) | wsp-core item: wsp_core::reach::DaemonReach::request | real takes the op and a `Frame` and answers `Result<Frame, ReachError>`; the trait takes the place id and finds the link itself |
| `SessionsCtx` | `on_place_absent` | `ctx.placeDoorOf().on(e => e.type === "place.absent" && e.placeId === place)` (sessions.ts:86) | wsp-core item: wsp_machines::places::types::PlaceDoor::on | real `on(f)` answers a `u64` id for `off(id)`; the trait filters for one place and answers the off closure |
| `SessionsCtx` | `place_name_of` | `ctx.placeDoorOf().nameOf(place)` (sessions.ts:149,582) | wsp-core item: wsp_machines::places::types::PlaceDoor::name_of | same shape |
| `SessionsCtx` | `workspace_place` | `workspacePlace(record)` (@wsp/protocol place-state.ts:16, `view.place`) | wsp-core item: wsp_words::place_state::workspace_place | real takes a `&WorkspaceView`, trait takes `&WorkspaceRecord`; not `ChannelsArea::place_id_of`, which answers the here place for a local workspace |
| `SessionsCtx` | `cap_held` | `ctx.capHeld.get(turnId)` (ThreadsArea.capHeld) | wsp-core ThreadsArea (empty) | this crate's own area; helper over the map; `CapHeld` is this crate's cap.rs |
| `SessionsCtx` | `cap_stop` | `ctx.capStop` | wsp-core ThreadsArea (empty) | this crate's own area; TS answers a bool the caller drops |
| `SessionsCtx` | `cap_stopping` | `ctx.capStopping` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `drop_queued` | `ctx.dropQueued` | wsp-core item: wsp_machines::machines::sub_queue::SubQueueArea::drop_queued | real answers a bool, trait answers `Result<bool, HostError>` |
| `SessionsCtx` | `stop_under` | `ctx.stopUnder` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `try_stopped` | `ctx.tryStopped` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `line_try` | `ctx.lineTry` | wsp-core ThreadsArea (empty) | this crate's own area; TS takes `{ view, turnId, asked }`, the trait a `Row` |
| `SessionsCtx` | `end_thread` | `ctx.moduleOf(entry.record.kind).endThread` | wsp-core item: wsp_core::types::internal::KindModule::end_thread | `moduleOf` is KindsArea (empty) and picks the module; real answers `Option<Local<Option<String>>>` with no Result |
| `SessionsCtx` | `end_owed` | `ctx.moduleOf(entry.record.kind).endOwed` | wsp-core item: wsp_core::types::internal::KindModule::end_owed | real is sync and answers `Option<Local<Option<Local<()>>>>`; the trait is async with a Result and a `Joined` for the `paid` |
| `SessionsCtx` | `taken` | `transcriptIndex.get(workspaceId)?.taken.get(requestId)` (sessions.ts:261; RuntimeCore.transcriptIndex) | wsp-core RuntimeCore (empty) | helper over the map; `Taken` is real (wsp_core::types::internal::Taken) |
| `SessionsCtx` | `taken_turn` | `ctx.takenTurn` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `fork_opened` | `forkOpened(ctx, workspaceId, opened, origin)` (forks.ts) | gap: TS free function, no ctx member | ported in this crate at threads/forks.rs `fork_opened(&Rc<dyn ForksCtx>, ..)`, which takes `Option<Caller>` where the trait takes `Option<&Caller>` |
| `SessionsCtx` | `claim_warm` | `ctx.claimWarm` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `opens_in` | `ctx.opensIn` | wsp-core item: wsp_account::account::rules::RulesArea::opens_in | real answers `Result<bool, HostError>`, trait answers a bool |
| `SessionsCtx` | `of_threads_repository` | `ctx.ofThreadsRepository` | wsp-core item: wsp_account::account::rules::RulesArea::of_threads_repository | real answers `Result<bool, HostError>`, trait answers a bool |
| `SessionsCtx` | `elsewhere_refusal` | `ctx.elsewhereRefusal(origin, ctx.projectHeld(project), name)` (sessions.ts:306) | wsp-core item: wsp_account::account::rules::RulesArea::elsewhere_refusal | real takes `&ProjectView` and answers `Result<Option<HostError>, HostError>`; the trait takes the project id, so it folds in `ChannelsArea::project_held` (`Result<ProjectView>`) |
| `SessionsCtx` | `away_for` | `ctx.awayFor(record, origin, "start", workspaceId)` (sessions.ts:306) | wsp-core item: wsp_account::account::rules::RulesArea::away_for | real takes a `&WorkspaceLike` (built by `wsp_account::account::rules::like_of(record)`) and an `AcrossAct`, answers `Result<Option<HostError>, HostError>`; the trait takes `&WorkspaceRecord` and the act as `&str` |
| `SessionsCtx` | `acts_on` | `ctx.actsOn` | wsp-core item: wsp_account::account::rules::RulesArea::acts_on | real takes a `&WorkspaceLike`, trait takes `&WorkspaceRecord` |
| `SessionsCtx` | `project_computer_name` | `ctx.placeName(ctx.projectHeld(r.project).computer)` (sessions.ts:323) | gap: helper over BuildersArea.placeName and ChannelsArea.projectHeld | two areas; `ChannelsArea::project_held` is real (wsp_account::account::channels::daemon), `placeName` is BuildersArea (empty) |
| `SessionsCtx` | `worktree_gone` | `ctx.worktreeGone` | wsp-core item: wsp_account::projects::folders::tree::Folders::worktree_gone | real takes a `WorktreeSettledWhy`, trait takes `&str` |
| `SessionsCtx` | `computer_of` | `ctx.computerOf` | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::computer_of | same shape |
| `SessionsCtx` | `refuse_sub_send` | `ctx.refuseSubSend` | wsp-core item: wsp_machines::machines::sub_life::timers::SubLifeArea::refuse_sub_send | same shape |
| `SessionsCtx` | `proven` | `ctx.proven` | wsp-core item: wsp_machines::machines::machines::phases::MachinesArea::proven | real answers `Result<WorkspaceView, HostError>`, trait answers `()` |
| `SessionsCtx` | `copy_blocked` | `ctx.copyBlocked` | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::copy_blocked | same shape |
| `SessionsCtx` | `worktree_mounted` | `ctx.worktreeMounted` | wsp-core item: wsp_account::projects::folders::place::Folders::worktree_mounted | same shape |
| `SessionsCtx` | `kept_check` | `ctx.keptCheck` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `spawn_guard` | `ctx.spawnGuard` | wsp-core item: wsp_account::account::rules::RulesArea::spawn_guard | real takes a `SpawnAct` and answers `Result<Release, HostError>` (the TS release closure, which sessions.ts drops); the trait takes `&str` and answers `Result<(), HostError>` |
| `SessionsCtx` | `parent_of` | `ctx.parentOf` | wsp-core item: wsp_account::account::rules::RulesArea::parent_of | same shape |
| `SessionsCtx` | `root_of` | `ctx.rootOf` | wsp-core item: wsp_account::account::rules::RulesArea::root_of | same shape |
| `SessionsCtx` | `tree_of` | `ctx.treeOf` | wsp-core item: wsp_account::account::rules::RulesArea::tree_of | same shape (`TreeOf`) |
| `SessionsCtx` | `thread_of_token` | `ctx.threadOfToken` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `reaches_row` | `ctx.reachesRow(view, origin, talk)` | wsp-core item: wsp_machines::machines::boot::entries::BootArea::reaches_row | real takes `&RowAt`, trait takes `&SessionView` |
| `SessionsCtx` | `notify_reach` | `ctx.notifyReach` | wsp-core ThreadsArea (empty) | this crate's own area; TS answers a `Set`, the trait an `IndexSet` |
| `SessionsCtx` | `notify_on` | `ctx.notifyOn` | wsp-core ThreadsArea (empty) | this crate's own area; `NotifyOn` is this crate's threads/tree.rs |
| `SessionsCtx` | `resume_sub` | `ctx.resumeSub` | wsp-core item: wsp_machines::machines::sub_spawn::machine::SubSpawnArea::resume_sub | real takes the prompt as `Option<&str>`, trait as `&str` |
| `SessionsCtx` | `place_rows` | `ctx.placeDoor?.rows()` (cloudsTold, launch.ts:31, read at sessions.ts:421; RuntimeCore.placeDoor) | wsp-core item: wsp_machines::places::types::PlaceDoor::rows | real answers `PlaceResult<Vec<PlaceView>>` from a present door; the trait answers an `Option` (None where the context has no door) |
| `SessionsCtx` | `hold_queued` | `ctx.holdQueued(at, origin)` | wsp-core item: wsp_machines::machines::sub_queue::SubQueueArea::hold_queued | real takes one `HoldAsk` and answers a `StartedTurn`; the trait takes six loose arguments and answers `Rc<dyn SessionHandle>` |
| `SessionsCtx` | `launch_area` | `launchArea(ctx)` (launch.ts, built once in sessionsArea, sessions.ts:247) | gap: TS free function, no ctx member | ported in this crate at threads/launch.rs `LaunchArea::new(Rc<dyn LaunchCtx>)`; the area can build it from its own context |
| `SessionsCtx` | `fork_turn_of` | `forkTurnOf(plan, byCount)` (forks.ts) | gap: TS free function, no ctx member | ported in this crate at threads/forks.rs `fork_turn_of`, same shape |
| `SessionsCtx` | `sub_done` | `ctx.subDone` (`void`, sessions.ts:1494) | wsp-core item: wsp_machines::machines::sub_life::timers::SubLifeArea::sub_done | real is async and answers `()`, the trait is sync because TS detaches it |
| `SessionsCtx` | `kept_warm` | `ctx.keptAgents.get(claimed)?.warm !== undefined` (sessions.ts:1492; ThreadsArea.keptAgents) | wsp-core ThreadsArea (empty) | this crate's own area; helper over the map |
| `SessionsCtx` | `fork_start` | `forkStart(ctx, start, o, origin)` (forks.ts) | gap: TS free function, no ctx member | ported in this crate at threads/forks.rs `fork_start(&Rc<dyn ForksCtx>, ..)`, which takes `Option<Caller>` |
| `SessionsCtx` | `carried_title` | `ctx.carriedTitle` | wsp-core AgentsArea (empty) | this crate's own area; TS answers `Pick<SessionView, harnessTitle and titleSource>`, the trait a tuple |
| `SessionsCtx` | `cap_hold` | `ctx.capHold` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `cap_full` | `ctx.capFull` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `cap_wait` | `ctx.capWait` | wsp-core ThreadsArea (empty) | this crate's own area; TS takes `{ workspaceId, threadId, turnId, sessionId, requestId?, prompt }` and answers a Promise, the trait takes a `CapAt` and answers a `Joined`; not `BootRuntime::cap_wait`, which is `ctx.capHeld` |
| `SessionsCtx` | `cap_left` | `ctx.capLeft` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `cap_lend` | `ctx.capLend` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `first_run_here` | `ctx.firstRunHere` | wsp-core AgentsArea (empty) | this crate's own area |
| `SessionsCtx` | `limit_details_due` | `ctx.limitDetailsDue` | wsp-core ThreadsArea (empty) | this crate's own area (context.ts:798) |
| `SessionsCtx` | `host_writes` | `ctx.hostWrites.get(resume)` (sessions.ts:652; ThreadsArea.hostWrites) | wsp-core ThreadsArea (empty) | this crate's own area; helper over the map |
| `SessionsCtx` | `land_images` | `ctx.landImages(entry, adapter.attachments, folder, dir, images)` | wsp-core AgentsArea (empty) | this crate's own area; TS takes the harness's `AttachmentRoad`, the trait takes the whole adapter and reads that field itself |
| `SessionsCtx` | `land_files` | `ctx.landFiles` | wsp-core AgentsArea (empty) | this crate's own area |
| `SessionsCtx` | `ready_fork` | `readyFork(adapter, session, turn, refused)` (forks.ts) | gap: TS free function, no ctx member | ported in this crate at threads/forks.rs `ready_fork`, which takes `said: &dyn Fn(&str) -> HostError`; the trait takes the harness name and builds the refusal |
| `SessionsCtx` | `copy_images` | `copyImages(ctx, plan, at)` (forks.ts) | gap: TS free function, no ctx member | ported in this crate at threads/forks.rs `copy_images(&Rc<dyn ForksCtx>, ..)`, same shape |
| `SessionsCtx` | `pay_cut_owed` | `payCutOwed(ctx, adapter, at, resume)` (forks.ts) | gap: TS free function, no ctx member | ported in this crate at threads/forks.rs `pay_cut_owed(&Rc<dyn ForksCtx>, ..)`, same shape |
| `SessionsCtx` | `snapshot_of` | `ctx.snapshotOf` | wsp-core ThreadsArea (empty) | this crate's own area; TS answers a Promise, the trait a `Shared` future |
| `SessionsCtx` | `keep_steer` | `ctx.keepSteer(s, o, caller, steerId)` | wsp-core ThreadsArea (empty) | this crate's own area; the trait takes the row, `&StartOptions` and the origin |
| `SessionsCtx` | `steer_lost` | `ctx.steerLost` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `steer_answered` | `ctx.steerAnswered` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `cut_before` | `ctx.cutBefore` | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::cut_before | same shape |
| `SessionsCtx` | `take_kept` | `ctx.takeKept` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `record_copies` | `ctx.recordCopies(copiedEvents(plan, at))` (sessions.ts:777) | wsp-core TranscriptsArea (empty) | this crate's own area; the method also folds in forks.ts `copiedEvents` (ported in this crate at threads/forks.rs `copied_events`), the TS member takes the events |
| `SessionsCtx` | `run_turn` | `ctx.runTurn` | wsp-core TurnsArea (empty) | this crate's own area; the trait takes a `TurnStart` |
| `SessionsCtx` | `keep_sent_images` | `ctx.keepSentImages` | wsp-core TranscriptsArea (empty) | this crate's own area |
| `SessionsCtx` | `cancel_resume_timer` | `ctx.resumeTimers.get(threadId)?.()` (sessions.ts:836; RuntimeCore.resumeTimers) | wsp-core RuntimeCore (empty) | helper over the map (`BootRuntime::cancel_resume` is the same call) |
| `SessionsCtx` | `remember_target` | `ctx.rememberTarget` | wsp-core item: wsp_account::account::rules::RulesArea::remember_target | same shape |
| `SessionsCtx` | `notify_end` | `ctx.notifyEnd(s, notify, ctx.tellAs({ notifyBy, notifyRoad }), result)` (sessions.ts:863) | wsp-core ThreadsArea (empty) | this crate's own area; helper over two ThreadsArea members (`notifyEnd`, `tellAs`), the trait takes the `by` and `road` that `tellAs` would answer |
| `SessionsCtx` | `drop_images` | `ctx.dropImages` | wsp-core AgentsArea (empty) | this crate's own area |
| `SessionsCtx` | `drop_sent_images` | `ctx.dropSentImages` | wsp-core TranscriptsArea (empty) | this crate's own area; TS also takes the word "all" for the ids, the trait a slice |
| `SessionsCtx` | `end_kept` | `ctx.endKept` | wsp-core ThreadsArea (empty) | this crate's own area; TS takes an optional `{ now: true }` |
| `SessionsCtx` | `open_thread` | `ctx.openThread` (sessions.ts:820, its caller filters out this turn) | wsp-core TranscriptsArea (empty) | this crate's own area; the doc line names `ctx.seedFor(...)`, which is no TS member; the method answers every event of the thread |
| `SessionsCtx` | `tree_stands_on` | `ctx.treeStandsOn` | wsp-core item: wsp_machines::machines::boot::entries::BootArea::tree_stands_on | real takes the talk as `Option<TreeTalk>` (same as TS), trait takes it by value |
| `SessionsCtx` | `refuse_named` | `ctx.refuseNamed` | wsp-core item: wsp_account::account::rules::RulesArea::refuse_named | same shape |
| `SessionsCtx` | `title_rows` | `ctx.titleRows` | wsp-core AgentsArea (empty) | this crate's own area |
| `SessionsCtx` | `booting` | `ctx.bootWork.has(workspaceId)` (sessions.ts:915; RuntimeCore.bootWork) | wsp-core RuntimeCore (empty) | helper over the map |
| `SessionsCtx` | `refresh_title` | `ctx.refreshTitle(view, false)` (sessions.ts:915) | wsp-core AgentsArea (empty) | this crate's own area; the TS second argument is `force`, the trait names it `ended` |
| `SessionsCtx` | `tree_records` | `ctx.treeRecords` | wsp-core item: wsp_machines::machines::boot::entries::BootArea::tree_records | real answers `Vec<(thread id, workspace id, ThreadRecord)>` and takes `Option<TreeTalk>`; the trait answers `Vec<(String, ThreadRecord)>` and takes the talk by value |
| `SessionsCtx` | `listed_rows` | `ctx.listedRows(held)` (sessions.ts:924) | wsp-core item: wsp_machines::machines::boot::rows::BootArea::listed_rows | TS takes the held entries; real takes `&[Row]`, the trait `Vec<SessionView>` |
| `SessionsCtx` | `placed_rows` | `ctx.placedRows` | wsp-core item: wsp_machines::machines::boot::rows::BootArea::placed_rows | real is async, the trait is sync though TS answers a Promise |
| `SessionsCtx` | `drives_thread` | `ctx.drivesThread` | wsp-core ThreadsArea (empty) | this crate's own area; wsp-machines' `BootRuntime` declares it too |
| `SessionsCtx` | `open_transcript` | `ctx.openTranscript` | wsp-core TranscriptsArea (empty) | this crate's own area |
| `SessionsCtx` | `opened_words` | `ctx.openedWords` | wsp-core TranscriptsArea (empty) | this crate's own area |
| `SessionsCtx` | `index_pos` | `transcriptIndex.get(workspaceId)?.pos ?? 0` (sessions.ts:948; RuntimeCore.transcriptIndex) | wsp-core RuntimeCore (empty) | helper over the map (`BootRuntime::index_pos` is the same) |
| `SessionsCtx` | `read_page` | `ctx.transcriptReader.read(workspaceId, threadId, window)` (sessions.ts:950, and head at 963) | wsp-core TranscriptsArea (empty) | this crate's own area; the member's type is the `TranscriptReader` trait (wsp_store::transcript_reader), which this crate implements; its `read` answers `ThreadEvents` where the trait answers `HistoryPage` |
| `SessionsCtx` | `thread_facts` | `ctx.threadFacts` | wsp-core item: wsp_machines::machines::boot::rows::BootArea::thread_facts | same shape |
| `SessionsCtx` | `facts_after_read` | `ctx.factsAfterRead` | wsp-core item: wsp_machines::machines::boot::rows::BootArea::facts_after_read | same shape |
| `SessionsCtx` | `kept_writes` | `ctx.keptWrites.get(workspaceId)`, awaited (sessions.ts:969; TranscriptsArea.keptWrites) | wsp-core TranscriptsArea (empty) | this crate's own area; helper over the map |
| `SessionsCtx` | `store_get` | `store.get(ATTACHMENT_KEYS, workspaceId)` (sessions.ts:971; RuntimeCore.store) | wsp-core item: wsp_store::store::Store::get | same shape |
| `SessionsCtx` | `store_get_blob` | `store.getBlob(ATTACHMENTS, key)` (sessions.ts:972) | wsp-core item: wsp_store::store::Store::get_blob | same shape (the trait names the second argument `key`, real `id`) |
| `SessionsCtx` | `index_workspaces` | `for ([workspaceId, index] of transcriptIndex)` (sessions.ts:1181; RuntimeCore.transcriptIndex) | wsp-core RuntimeCore (empty) | helper over the map's keys |
| `SessionsCtx` | `search_in` | `searchIndex(workspaceId, index, words, drives, found)` (internal.ts:532) | wsp-core item: wsp_core::types::internal::search_index | real takes the `&TranscriptIndex` and answers a `usize`; the trait looks the index up itself and answers a `u32` |
| `SessionsCtx` | `record_steer` | `ctx.recordSteer(s, handleId, o, caller, steerId)` | wsp-core ThreadsArea (empty) | this crate's own area; the trait takes the row, its id and a `&SteerAsk` |
| `SessionsCtx` | `launch_adapter_for` | `ctx.launchAdapterFor(entry, named, turnEnv, waiting, servers)` | wsp-core AgentsArea (empty) | this crate's own area; the trait takes the harness and the server values only; `LaunchCtx::adapter_for` is the sync `adapterFor` |
| `SessionsCtx` | `write_session` | `ctx.writeSession(harnessSessionId, () => write(...))` (sessions.ts:1091) | wsp-core ThreadsArea (empty) | this crate's own area; the trait takes the adapter and the title and builds the write itself (`adapter.renameSession` over `wsp_engine::run_env::harness_exec(entry.machine, SESSION_TITLE_TIMEOUT_MS)`) |
| `SessionsCtx` | `brokered` | `ctx.brokered(entry.record)` (KindsArea.brokered, kinds.ts:248) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::brokered | same shape (`&WorkspaceRecord`, bool); not `ChannelsArea::brokered`, which is channels.ts's private helper over an entry |
| `SessionsCtx` | `server_values_for` | `ctx.serverValuesFor` | wsp-core AgentsArea (empty) | this crate's own area |
| `SessionsCtx` | `mark` | `ctx.mark` | wsp-core item: wsp_machines::machines::boot::rows::BootArea::mark | real takes a `Stamped` where the trait takes a `StampsOf` closure |
| `SessionsCtx` | `arm_resume` | `ctx.armResume` | wsp-core item: wsp_machines::machines::boot::rows::BootArea::arm_resume | same shape |
| `SessionsCtx` | `aside` | `adapter.aside(question)` (sessions.ts:1211, `bare.aside` or `adapter.aside`) | wsp-core item: wsp_adapters::port::HarnessAdapter::aside | real answers `Option<future of Result<AsideAnswer, String>>` (None where the harness has no aside); the trait takes the adapter, answers the text and does TS's blank-text check (`ASIDE_EMPTY_LINE`) |
| `SessionsCtx` | `subagent_known` | `[...transcriptIndex.values()].some(index => children has id)` (sessions.ts:217) | wsp-core RuntimeCore (empty) | helper over the map (`BootRuntime::index_children` reads the same children) |
| `SessionsCtx` | `settles_thread` | `ctx.settlesThread` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `settled_now` | `ctx.settledNow` | wsp-core item: wsp_machines::machines::boot::rows::BootArea::settled_now | same shape |
| `SessionsCtx` | `tree_under` | `ctx.treeUnder` | wsp-core ThreadsArea (empty) | this crate's own area |
| `SessionsCtx` | `restarts` | `ctx.restarts` | wsp-core ThreadsArea (empty) | this crate's own area; TS answers a `Map`, the trait an `IndexMap` |
| `SessionsCtx` | `reap_kept` | `ctx.reapKept` | wsp-core ThreadsArea (empty) | this crate's own area; also declared on `LaunchCtx` with the same signature, so a call on `C: SessionsCtx` names the trait |
| `SessionsCtx` | `checkout_of` | `ctx.checkoutOf` | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::checkout_of | real answers `Result<String, HostError>`, trait answers a `String` |
| `SessionsCtx` | `wake` | `ctx.workspaces.wake(id, origin)` (sessions.ts:1288; WorkspacesArea.workspaces) | wsp-core item: wsp_core::types::api::WorkspacesApi::wake | real answers `Result<WorkspaceView, HostError>`, trait answers `()`; the `phase === "running"` skip is the caller's; body is wsp_account::projects::workspaces::phases::wake |
| `SessionsCtx` | `git_restore` | `ctx.queued(id, () => ctx.withDaemon(entry, ask => ask({ op: "git.restore", cwd, checkpoint, scope })))` (sessions.ts:1289) | gap: helper over FoldersArea.queued and ChannelsArea.withDaemon | both are real (`Folders::queued`, `ChannelsArea::with_daemon`); the method builds the `git.restore` frame inline and answers an untyped map where the daemon's typed reply is `GitRestoreReply` |
| `SessionsCtx` | `flush_transcript` | `ctx.flushTranscript` | wsp-core TranscriptsArea (empty) | this crate's own area |
| `SessionsCtx` | `move_session` | `moveSession(ctx, threadId, from, to)` (forks.ts) | gap: TS free function, no ctx member | ported in this crate at threads/forks.rs `move_session(&Rc<dyn ForksCtx>, ..)`, same shape |
| `SessionsCtx` | `drop_from_transcript` | `ctx.dropFromTranscript(workspaceId, threadId, drops)` | wsp-core TranscriptsArea (empty) | this crate's own area; TS takes a predicate over events (rewind passes `e => cut.includes(e.turnId)`), the trait takes `Option<&[String]>` of turn ids |
| `SessionsCtx` | `slates_rewound` | `ctx.slates.rewound({ threadId, turnId, cut })` (sessions.ts:1393) | wsp-core SlatesArea (empty) | wsp-slates fills it and is not in the workspace; wsp-core's `Slates` (types/pending.rs) is an empty trait |
| `SessionsCtx` | `lead_of_sub` | `ctx.leadOfSub` | wsp-core item: wsp_machines::machines::sub_life::timers::SubLifeArea::lead_of_sub | same shape |
| `SessionsCtx` | `sub_delete` | `ctx.subDelete(entry, threadId, origin, o)` | wsp-core item: wsp_machines::machines::sub_life::timers::SubLifeArea::sub_delete | real takes `check: bool` and answers `SubDeleted`; the trait takes a `DeleteThreadAsk` and answers `ThreadDeleted` |
| `SessionsCtx` | `turn_runs` | `ctx.turnRuns` | wsp-core item: wsp_machines::machines::daemon::DaemonArea::turn_runs | same shape |
| `SessionsCtx` | `end_thread_shells` | `ctx.endThreadShells` | wsp-core item: wsp_account::account::channels::ptys::ChannelsArea::end_thread_shells | real answers `AccountResult<()>`, trait `Result<(), HostError>` |
| `SessionsCtx` | `delete_workspace` | `ctx.workspaces.delete(id, origin)` (sessions.ts:1428; WorkspacesArea.workspaces) | wsp-core item: wsp_core::types::api::WorkspacesApi::delete | same shape (`Result<(), HostError>`); body is wsp_account::projects::workspaces::phases::delete |
| `SessionsCtx` | `drop_checkpoints` | `ctx.dropCheckpoints` | wsp-core item: wsp_account::projects::folders::tree::Folders::drop_checkpoints | same shape |
| `SessionsCtx` | `drop_thread_files` | `ctx.dropThreadFiles` | wsp-core AgentsArea (empty) | this crate's own area; TS takes an `Iterable` of ids |
| `SessionsCtx` | `row_gone` | `ctx.rowGone(id, { workspaceId, threadId })` | wsp-core item: wsp_machines::machines::boot::rows::BootArea::row_gone | real takes the thread id as `Option<&str>`, the trait as `&str` |
| `SessionsCtx` | `slates_forget` | `ctx.slates.forget(threadId, { keep })` (sessions.ts:1449,1477) | wsp-core SlatesArea (empty) | wsp-slates fills it; `keep` is optional in TS (absent from the forget road at 1477) |

## TurnsCtx (src/threads/turns.rs, 60 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `TurnsCtx` | `clock` | clock = ctx.clock (turns.ts:18; clock.now() at 225, 490) | wsp-core item: wsp_core::clock::Clock | RuntimeCore.clock; the trait hands out the whole clock |
| `TurnsCtx` | `sessions` | ctx.sessions (turns.ts:18; rows read and written throughout) | wsp-core RuntimeCore (empty) | helper over the map: Sessions is the map itself, rows are typed `Row` (TS `SessionEntry`); wsp-machines' `MachinesRuntime::sessions` is the same map |
| `TurnsCtx` | `records` | threadRecords = ctx.threadRecords (turns.ts:18; get at 698) | wsp-core RuntimeCore (empty) | the map itself; Rust `ThreadRecord` carries `workspace_id` where TS types `ThreadRecord & { workspaceId }` |
| `TurnsCtx` | `live_entry` | live.get(workspaceId) (live = ctx.live, turns.ts:715, 777) | wsp-core RuntimeCore (empty) | helper over the map (one get); `Entry` is `Rc<RefCell<LiveWorkspace>>` |
| `TurnsCtx` | `latest_on` | ctx.latestOn(rootThreadId) (turns.ts:715) | wsp-core ThreadsArea (empty) | this crate's own area |
| `TurnsCtx` | `closing` | ctx.state.closing (turns.ts:95) | wsp-core item: wsp_core::context::RuntimeState::closing |  |
| `TurnsCtx` | `record` | ctx.record(event) (turns.ts:120, 252, 317 and others) | wsp-core TranscriptsArea (empty) | this crate's own area; `TranscriptsArea.record` |
| `TurnsCtx` | `emit` | bus.emit({ type: "thread.marked" \| "usage.account" ... }) (turns.ts:145, 480) | wsp-core item: wsp_core::types::events::Bus::emit | Bus::emit answers `Result<u64, HostError>` where the trait answers nothing |
| `TurnsCtx` | `pass` | bus.pass({ type: "session.context" ... }) (turns.ts:490) | wsp-core item: wsp_core::types::events::Bus::pass | Bus::pass answers `Result<(), HostError>` where the trait answers nothing |
| `TurnsCtx` | `persist_sessions` | ctx.persistSessions(workspaceId) (turns.ts:95, 144, 233 and others) | wsp-core TranscriptsArea (empty) | this crate's own area; `TranscriptsArea.persistSessions` |
| `TurnsCtx` | `persist_record` | ctx.persist(entry.record) (turns.ts:308) | wsp-core RecordsArea (empty) | `RecordsArea.persist`; no Rust item below |
| `TurnsCtx` | `held_at_close` | ctx.heldAtClose (turns.ts:122 add, 135 delete) | wsp-core RuntimeCore (empty) | TS holds a `Set<() => void>` and deletes by identity; the trait hands out an `Rc<RefCell<Vec<Rc<dyn Fn()>>>>`, so a delete must compare `Rc` pointers |
| `TurnsCtx` | `slates_turn_ended` | ctx.slates.turnEnded({ threadId, turnId }) (turns.ts:148) | wsp-core SlatesArea (empty) | `SlatesArea.slates`; wsp-core `types::pending::Slates` is an empty trait; the trait takes two ids where TS takes one object |
| `TurnsCtx` | `keep_checkpoint` | ctx.keepCheckpoint(entry, { sessionId, threadId, turnId, anchor?, kept? }) (turns.ts:149) | wsp-core ThreadsArea (empty) | this crate's own area; answers `Joined` (a shared future) where TS answers `Promise<void>` |
| `TurnsCtx` | `checkpoints_landing` | ctx.checkpointsLanding (turns.ts:150-152) | wsp-core ThreadsArea (empty) | this crate's own area; `Map<string, Promise<void>>` is `IndexMap<String, Joined>` |
| `TurnsCtx` | `read_turn_changes` | ctx.readTurnChanges(entry, { sessionId, turnId, threadId, cwd, from, startedAt, wrote? }) (turns.ts:88) | wsp-core ThreadsArea (empty) | this crate's own area; answers a `bool` inside a `Result` |
| `TurnsCtx` | `lead_asks` | ctx.leadAsks (turns.ts:219, 222, 702) | wsp-core ThreadsArea (empty) | this crate's own area; `Map<string, PermissionAsk>` |
| `TurnsCtx` | `mode_label` | harnessCatalog(view.harness)?.permissionModes.find(m => m.value === mode)?.label (turns.ts:239-241) | gap: TS free function `harnessCatalog` (harness-catalog.ts); wsp-core `harness_catalog.rs` is an empty file | the method is one lookup over the table `harnessCatalog` answers, with no context member |
| `TurnsCtx` | `settle_cut` | ctx.settleCut({ view, turnId, notify?, notifyBy?, notifyRoad?, turnLive, asked? }, reason, () => reason, byStop, unreached) (turns.ts:643) | wsp-core ThreadsArea (empty) | this crate's own area; the trait takes the row and drops the `cutLine` closure (it is always `() => reason`) |
| `TurnsCtx` | `line_try` | ctx.lineTry({ view, turnId, asked? }) (turns.ts:290) | wsp-core ThreadsArea (empty) | this crate's own area |
| `TurnsCtx` | `prompt_held` | ctx.promptHeld(requestId) (turns.ts:310) | wsp-core ThreadsArea (empty) | this crate's own area |
| `TurnsCtx` | `name_in_harness` | ctx.nameInHarness(view, title) (turns.ts:313) | wsp-core AgentsArea (empty) | this crate's own area |
| `TurnsCtx` | `source_of` | ctx.sourceOf(view) (turns.ts:345) | wsp-core AgentsArea (empty) | this crate's own area; `TitleSource` is a `String` option here |
| `TurnsCtx` | `hold_ask` | ctx.holdAsk(promise) (turns.ts:346) | wsp-core AgentsArea (empty) | this crate's own area; TS is generic and answers the promise, the trait takes a `()` future and answers nothing |
| `TurnsCtx` | `refresh_title` | ctx.refreshTitle(view, force) (turns.ts:347, 708) | wsp-core AgentsArea (empty) | this crate's own area; the trait takes the row |
| `TurnsCtx` | `make_title` | ctx.makeTitle(view) (turns.ts:348) | wsp-core AgentsArea (empty) | this crate's own area; the trait takes the row |
| `TurnsCtx` | `usage_account_of` | ctx.usageAccountOf(entry, view.harness, turnAccount) (turns.ts:410, 473, 487) | wsp-core ThreadsArea (empty) | this crate's own area (threads.ts:1158); TS answers `{ key, label: string, road: AccountRoad }`, `UsageAccount` has `label` and `road` as options and `road` as a `String`; its `named` argument is `{ id, label? }`, here a tuple |
| `TurnsCtx` | `usage_computer_of` | ctx.usageComputerOf(entry.record) (turns.ts:423) | wsp-core ThreadsArea (empty) | this crate's own area (threads.ts:1149) |
| `TurnsCtx` | `ledger_add` | ctx.ledger.add({ at, agent, account, accountLabel, computer, project, turns, model?, tokens?, costUsd?, session?, source }) (turns.ts:417) | wsp-core UsageArea (empty) | `UsageArea.ledger`; `UsageLedger` has no Rust item (wsp-machines `usage.rs` is empty); wsp-account's `UsageCtx` declares its own `UsageEntry` row for a log row, not this ledger row |
| `TurnsCtx` | `ledger_limit` | ctx.ledger.limit({ key, agent, label, road, computer, limit }) (turns.ts:474) | wsp-core UsageArea (empty) | `UsageArea.ledger`; answers the readings before and after as two bare `Value`s |
| `TurnsCtx` | `alerts_read` | ctx.alerts.read(before, after) (turns.ts:477) | wsp-core UsageArea (empty) | `UsageArea.alerts` (`PlanAlerts`); wsp-core `plan_alerts.rs` is an empty file; wsp-account's `UsageCtx::alerts_read` is the same call with the same `Value` arguments |
| `TurnsCtx` | `usage_accounts` | (await ctx.usageAccounts()).accounts (turns.ts:479) | wsp-core item: wsp_account::account::usage::UsageArea::usage_accounts | item answers `AccountResult<AccountsAnswer>` where the trait answers `Result<AccountsAnswer, HostError>`; `UsageArea` is wsp-account's own area, so the call crosses to a sibling crate's area |
| `TurnsCtx` | `burn_add` | ctx.burn.add({ account, threadId, tokens, replayed? }) (turns.ts:487) | wsp-core UsageArea (empty) | `UsageArea.burn`; no Rust item (wsp-machines `usage.rs` is empty); `replayed` is `(run, at)` here, `{ run, at }` in TS |
| `TurnsCtx` | `notify_end` | ctx.notifyEnd({ view, turnId, turnLive }, notify, ctx.tellAs(t), result, { open? }) (turns.ts:437, 526) | wsp-core ThreadsArea (empty) | this crate's own area; the trait folds `ctx.tellAs(row)` into the call and `open` into a `bool` |
| `TurnsCtx` | `agent_retried` | ctx.agentRetried(turnId, event.why) (turns.ts:505) | wsp-core WaitsArea (empty) | this crate's own area; `WaitsArea.agentRetried` |
| `TurnsCtx` | `end_snooze_for` | ctx.endSnoozeFor(view) (turns.ts:540, 695) | wsp-core item: wsp_machines::machines::boot::rows::BootArea::end_snooze_for | item takes `(Option<&str> thread_id, Option<&str> root_thread_id)` and `self: &Rc<Self>` where the trait takes the row |
| `TurnsCtx` | `idle_hold` | ctx.idle.hold(workspaceId) (turns.ts:573) | wsp-core item: wsp_core::idle::IdlePolicy::hold | `MachinesArea.idle` is the member; the trait method is one call on the policy |
| `TurnsCtx` | `idle_release` | ctx.idle.release(workspaceId) (turns.ts:578, 585, 588) | wsp-core item: wsp_core::idle::IdlePolicy::release | `MachinesArea.idle`; same as `idle_hold` |
| `TurnsCtx` | `arm_disk_watch` | ctx.armDiskWatch() (turns.ts:582) | wsp-core item: wsp_machines::machines::reach::disk::ReachArea::arm_disk_watch |  |
| `TurnsCtx` | `read_disk` | ctx.readDisk(entry) (turns.ts:586) | wsp-core item: wsp_machines::machines::reach::disk::ReachArea::read_disk | item answers `()` (best effort, TS catches and logs) where the trait answers a `Result` |
| `TurnsCtx` | `harness_exec` | harnessExec(entry.machine, ms) (@wsp/engine free function); turns.ts makes no such call | wsp-core item: wsp_engine::run_env::harness_exec | no TS member in turns.ts and no call in this crate's turns code; the real function takes a machine and answers wsp-engine's `HarnessExec` (an `Arc<dyn Fn>` over a command and variables) where the trait answers wsp-adapters' `HarnessExec` trait object; `ConversationsCtx` declares the same method |
| `TurnsCtx` | `shared_daemon` | ctx.moduleOf(entry.record.kind).sharedDaemon (turns.ts:651) | wsp-core item: wsp_core::types::internal::KindModule::shared_daemon | `KindsArea.moduleOf` picks the module (wsp-machines `KindsArea::module_of` answers a `Result`); the item takes no entry |
| `TurnsCtx` | `turn_groups` | ctx.turnGroups (turns.ts:652-654) | wsp-core item: wsp_account::account::channels::ChannelsArea::turn_groups | field of wsp-account's `ChannelsArea`, keyed `i64` where the trait keys `u32`; the TS map is built inside `channelsArea` |
| `TurnsCtx` | `arm_roots_recheck` | ctx.armRootsRecheck() (turns.ts:655) | wsp-core item: wsp_account::account::channels::ptys::ChannelsArea::arm_roots_recheck | item takes `self: &Rc<Self>` |
| `TurnsCtx` | `port_roots_moved` | ctx.portRootsMoved(workspaceId) (turns.ts:657, 693) | wsp-core item: wsp_account::account::channels::ptys::ChannelsArea::port_roots_moved |  |
| `TurnsCtx` | `hold_kept` | ctx.holdKept(threadId, { workspaceId, agent, launch, session, turnToken, pid?, scopeDeviceId?, waiting, ... }) (turns.ts:683) | wsp-core ThreadsArea (empty) | this crate's own area; TS answers `boolean`, the trait answers the hand back as `Option<KeptHand>` when the hold is refused, so the turn can close its process |
| `TurnsCtx` | `revoke_device` | deviceDoor.revoke(scopeDeviceId) (deviceDoor = ctx.deviceDoor, turns.ts:688) | wsp-core item: wsp_core::devices::DeviceDoor::revoke | item answers `Result<bool, HostError>`; `LaunchCtx` declares the same method |
| `TurnsCtx` | `read_checkout` | ctx.readCheckout(entry, true) (turns.ts:719) | wsp-core item: wsp_machines::machines::reach::daemon::ReachArea::read_checkout | item answers `Option<Checkout>` where the trait answers `Result<(), HostError>`; TS `readCheckout` answers `Promise<Checkout \| undefined>` |
| `TurnsCtx` | `ran_on_branch` | ctx.ranOnBranch(entry, threadId) (turns.ts:720) | wsp-core item: wsp_account::account::pull_requests::PullRequestsArea::ran_on_branch | item answers `AccountResult<()>` |
| `TurnsCtx` | `read_pull_request` | ctx.readPullRequest(entry, true) (turns.ts:721) | wsp-core item: wsp_account::account::pull_requests::PullRequestsArea::read_pull_request | item takes a third `whole` argument and answers `Option<PullRequestSeen>`; the trait takes `(entry, fresh)` and answers `Result<(), HostError>` |
| `TurnsCtx` | `read_lead_of` | ctx.readLeadOf(entry) (turns.ts:722) | wsp-core item: wsp_machines::machines::reach::daemon::ReachArea::read_lead_of | item is sync and answers `()` (TS `readLeadOf` answers void); the trait is async and answers a `Result` |
| `TurnsCtx` | `pull_request_state` | rootOn.record.pr.state (turns.ts:716, reads a field of the workspace record, no context member) | wsp-core item: wsp_core::types::wiring::WorkspaceRecord::pr | field read: `pr` is `Option<PullRequestRecord>`, whose `state` is a `PullRequestState`; the trait answers it as a `String` option |
| `TurnsCtx` | `settle_tree` | ctx.settleTree(rootOn) (turns.ts:716) | wsp-core item: wsp_account::account::pull_requests::PullRequestsArea::settle_tree | item answers `()` (no `Result`) and takes `self: &Rc<Self>` |
| `TurnsCtx` | `drop_images` | ctx.dropImages(entry, t.imagesDir) (turns.ts:726) | wsp-core AgentsArea (empty) | this crate's own area |
| `TurnsCtx` | `send_back` | ctx.sendBack({ view, turnId, turnLive }, unread, stopped) (turns.ts:732, 737) | wsp-core ThreadsArea (empty) | this crate's own area |
| `TurnsCtx` | `take_review` | ctx.takeReview(entry, threadId, text) (turns.ts:733) | wsp-core item: wsp_account::projects::start_from::StartFrom::take_review | same shape; TS home is `StartFromArea.takeReview`, filled by wsp-account |
| `TurnsCtx` | `is_held_away` | ctx.isHeldAway(entry.record.id) (turns.ts:752) | wsp-core item: wsp_machines::machines::boot::load::BootArea::is_held_away | same shape |
| `TurnsCtx` | `exec_sweep` | ctx.execFactoryFor(entry).sweep (turns.ts:753) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::exec_factory_for | the item answers `Result<Arc<dyn ExecStreamFactory>>` and `ExecStreamFactory::sweep` (wsp-adapters `port.rs`) answers an `Option` of a future; the trait folds the two into one call; item takes `&LiveWorkspace`, not `&Entry` |
| `TurnsCtx` | `adapter_for` | ctx.adapterFor(entry, view.harness, undefined, () => waiting.on).adapter (turns.ts:787) | wsp-core AgentsArea (empty) | this crate's own area; the trait takes an `AdapterAsk` where TS takes seven positional arguments; `LaunchCtx` declares the same method |
| `TurnsCtx` | `open_thread` | ctx.openThread(view.workspaceId, threadId) (turns.ts:801) | wsp-core TranscriptsArea (empty) | this crate's own area; `TranscriptsArea.openThread` |

## LaunchCtx (src/threads/launch.rs, 33 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `LaunchCtx` | `ids` | randomUUID() (launch.ts:85), randomBytes(16) (launch.ts:101), node:crypto; no ctx member | wsp-core item: wsp_core::ids::Ids | whole seam handed out, not one draw |
| `LaunchCtx` | `ready` | ctx.ready() (launch.ts:124) | wsp-core item: wsp_machines::machines::boot::ready::BootArea::ready | item answers a shared `Ready` handle over `Result<(), String>`, the trait a future of `Result<(), HostError>` |
| `LaunchCtx` | `entry_of` | ctx.entryOf(workspaceId, origin) (launch.ts:128) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::entry_of | item takes `(&Rc<Self>, id, Option<&Caller>, EntryAsk)` where the trait takes an owned `Option<Caller>` and no ask |
| `LaunchCtx` | `keeps_agents` | ctx.moduleOf(entry.record.kind).keepsAgents (launch.ts:130) | wsp-core item: wsp_core::types::internal::KindModule::keeps_agents | item takes no entry; `KindsArea.moduleOf` picks the module |
| `LaunchCtx` | `kept_box_cap` | ctx.keptBox(workspaceId)?.cap (launch.ts:130) | wsp-core ThreadsArea (empty) | this crate's own area; helper over `keptBox`, which answers `{ place, cap }`; the trait keeps only `cap` |
| `LaunchCtx` | `closing` | ctx.state.closing (launch.ts:130) | wsp-core item: wsp_core::context::RuntimeState::closing |  |
| `LaunchCtx` | `preferences_held` | ctx.state.preferencesHeld (launch.ts:131) | wsp-core item: wsp_core::context::RuntimeState::preferences_held | `RuntimeState` holds `Option<serde_json::Value>`, the trait answers `Option<Preferences>` |
| `LaunchCtx` | `preferences` | ctx.preferences.get() (launch.ts:131; PreferencesArea.preferences) | wsp-core item: wsp_core::types::api::PreferencesApi::get |  |
| `LaunchCtx` | `default_agent_of` | ctx.defaultAgentOf(prefs, entry) (launch.ts:132) | wsp-core AgentsArea (empty) | this crate's own area |
| `LaunchCtx` | `agent_off` | ctx.agentOff(entry, harness) (launch.ts:133) | wsp-core AgentsArea (empty) | this crate's own area |
| `LaunchCtx` | `adapter_for` | ctx.adapterFor(entry, harness, env, waiting, servers, threadId, asksUntilStopped?) (launch.ts:86, 105) | wsp-core AgentsArea (empty) | this crate's own area; the trait takes an `AdapterAsk` (`ask.named` is the harness) where TS takes seven positional arguments; `TurnsCtx` declares the same method |
| `LaunchCtx` | `adapter_warms` | probe.warm === undefined (launch.ts:87, a member of the adapter, no ctx member) | wsp-core item: wsp_adapters::port::HarnessAdapter::warm | the item answers `None` where there is no warm road and can only be asked by calling it with options; the trait asks a bare `bool` |
| `LaunchCtx` | `confine_setup` | ctx.confineSetup(entry, harness) (launch.ts:88) | wsp-core AgentsArea (empty) | this crate's own area |
| `LaunchCtx` | `catalog_on` | harnessCatalog(harness) === undefined ? undefined : ctx.catalogOn(table, entry, adapter) (launch.ts:48, 51) | gap: helper over the free function `harnessCatalog` (wsp-core `harness_catalog.rs` is empty) and `AgentsArea.catalogOn` | the `catalogOn` half is this crate's own `AgentsArea`; the table read is not a context member |
| `LaunchCtx` | `defaults_on` | ctx.defaultsOn(catalog, prefs, prefs.projectDefaults[project]) (launch.ts:51) | wsp-core AgentsArea (empty) | this crate's own area; the trait takes the project name where TS takes that project's overrides; answers a `DefaultsOn` |
| `LaunchCtx` | `named_mode` | ctx.namedMode(catalog, harness, o.access) (launch.ts:53) | wsp-core AgentsArea (empty) | this crate's own area; the trait answers an `Option` |
| `LaunchCtx` | `access_of` | ctx.accessOf(workspaceId, threadId, session) (launch.ts:59) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::access_of | same shape |
| `LaunchCtx` | `ran_on` | ctx.ranOn(workspaceId, threadId, session) (launch.ts:60) | wsp-core TranscriptsArea (empty) | this crate's own area; `TranscriptsArea.ranOn` |
| `LaunchCtx` | `resumed_fact` | ctx.resumedFact(workspaceId, session, "model") (launch.ts:67) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::resumed_fact | TS types `fact` as a key of `SESSION_FACTS`, the item and the trait take a `&str` |
| `LaunchCtx` | `not_run_refused` | notRunRefused(catalog, model) (harness-catalog.ts free function, launch.ts:69) | gap: TS free function `notRunRefused` (harness-catalog.ts); wsp-core `harness_catalog.rs` is an empty file | answers an error to throw or nothing |
| `LaunchCtx` | `thread_folder` | ctx.threadFolder(entry, asked) (launch.ts:91; KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::thread_folder | item takes `cwd: Option<&str>` and `&LiveWorkspace`; TS takes `{ cwd? }` |
| `LaunchCtx` | `folder_exists` | existsSync(folder) (node:fs, launch.ts:92); no ctx member | gap: node `fs.existsSync`; no context.ts member and no wsp-core item | same kind of node call as account's `PhasesCtx::path_exists` |
| `LaunchCtx` | `runs_in` | ctx.runsIn(entry, undefined, folder) (launch.ts:93; FoldersArea) | wsp-core item: wsp_account::projects::folders::tree::Folders::runs_in | same shape |
| `LaunchCtx` | `setup_place` | ctx.setupPlace(entry) (launch.ts:78) | wsp-core AgentsArea (empty) | this crate's own area |
| `LaunchCtx` | `launch_of` | setups.launchOf(place, harness) (setups = ctx.setups, launch.ts:79) | wsp-core RuntimeCore (empty) | `RuntimeCore.setups` (`AgentSetups`); wsp-core `agent_setup.rs` is an empty file, so `launchOf` has no item; wsp-account's `UsageCtx::launch_of` is the same call |
| `LaunchCtx` | `warm_on` | ctx.warmOn(workspaceId, harness) (launch.ts:95) | wsp-core ThreadsArea (empty) | this crate's own area; TS answers `[threadId, KeptProcess]`, the trait `(String, KeptLaunch)` |
| `LaunchCtx` | `rewarm` | ctx.rewarm(standing[0]) (launch.ts:97) | wsp-core ThreadsArea (empty) | this crate's own area |
| `LaunchCtx` | `reap_kept` | ctx.reapKept(standing[0]) (launch.ts:100) | wsp-core ThreadsArea (empty) | this crate's own area; `ActivityCtx::reap_kept_now` is the same member with `{ now: true }` |
| `LaunchCtx` | `thread_launch` | threadLaunch(entry, threadId, threadId) (threadLaunch = ctx.threadLaunch, launch.ts:104) | wsp-core RuntimeCore (empty) | `RuntimeCore.threadLaunch`; wsp-account's `ChannelsCtx::thread_launch` is the same member with no `aside` argument and no `wsp`; `ThreadLaunch` here keeps `scoped_device_id` where TS answers the whole paired device |
| `LaunchCtx` | `vault` | opts.vault?.() ?? {} (launch.ts:105) | opts: RuntimeOptions.vault | `RuntimeOptions.vault` is an `Option` of a closure over an `IndexMap`; the trait answers the map, empty where none |
| `LaunchCtx` | `launch_servers_for` | ctx.launchServersFor(entry, harness, adapter, cwd) (launch.ts:107) | wsp-core AgentsArea (empty) | this crate's own area |
| `LaunchCtx` | `hold_warm` | ctx.holdWarm(threadId, { workspaceId, agent, launch, turnToken, scopeDeviceId?, waiting, warm }) (launch.ts:115) | wsp-core ThreadsArea (empty) | this crate's own area |
| `LaunchCtx` | `revoke_device` | deviceDoor.revoke(scoped.deviceId) (deviceDoor = ctx.deviceDoor, launch.ts:118) | wsp-core item: wsp_core::devices::DeviceDoor::revoke | item answers `Result<bool, HostError>`; `TurnsCtx` declares the same method |

## WaitsCtx (src/threads/waits.rs, 4 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `WaitsCtx` | `clock` | clock = ctx.clock (waits.ts:20; clock.schedule at 81, 98) | wsp-core item: wsp_core::clock::Clock |  |
| `WaitsCtx` | `sessions` | sessions = ctx.sessions (waits.ts:20; values() at 48) | wsp-core RuntimeCore (empty) | helper over the map: the map itself, rows typed `Row` |
| `WaitsCtx` | `closing` | ctx.state.closing (waits.ts:55, 94, 100) | wsp-core item: wsp_core::context::RuntimeState::closing | read at the moment of each write |
| `WaitsCtx` | `record` | ctx.record(event) (waits.ts:60, 101) | wsp-core TranscriptsArea (empty) | this crate's own area; `TranscriptsArea.record` |

## TrimmedCtx (src/threads/trimmed_row.rs, 3 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `TrimmedCtx` | `started_as` | ctx.startedAs(r.workspaceId, threadId) (trimmed-row.ts:11) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::started_as | same shape |
| `TrimmedCtx` | `folder_of` | ctx.folderOf(r.workspaceId, resume) (trimmed-row.ts:12) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::folder_of | same shape (the trait names the second argument `session`) |
| `TrimmedCtx` | `thread_words` | ctx.transcriptIndex.get(workspaceId)?.words.get(threadId) (trimmed-row.ts:14) | wsp-core RuntimeCore (empty) | helper over the map: two gets over `RuntimeCore.transcriptIndex`; `ThreadWords` is a real type, `wsp_core::types::internal::ThreadWords` |

## ActivityCtx (src/activity.rs, 25 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `ActivityCtx` | `clock` | no ctx.clock call: `Date.now()` (activity.ts:269) and `setTimeout` (activity.ts:149, 269) | wsp-core item: wsp_core::clock::Clock | TS reads node time and timers here; README A7 routes them through the one Clock, so a fake clock now drives them |
| `ActivityCtx` | `ready` | ctx.ready() (activity.ts:287, 363) | wsp-core item: wsp_machines::machines::boot::ready::BootArea::ready | item answers a shared `Ready` handle over `Result<(), String>`, the trait a future of `Result<(), HostError>` |
| `ActivityCtx` | `has_door` | ctx.placeDoor !== undefined (activity.ts:102, 225) | wsp-core RuntimeCore (empty) | `RuntimeCore.placeDoor` is optional |
| `ActivityCtx` | `place_name` | ctx.placeDoor?.nameOf(place) (activity.ts:100, 224, 295, 300) | wsp-core item: wsp_machines::places::types::PlaceDoor::name_of | item answers a `String`, the trait an `Option` that folds the no-door case; the member is `RuntimeCore.placeDoor`, whose wsp-core type `types::pending::PlaceDoor` is an empty stand-in |
| `ActivityCtx` | `place_rows` | ctx.placeDoor.rows() (activity.ts:102) | wsp-core item: wsp_machines::places::types::PlaceDoor::rows | item answers `PlaceResult<Vec<PlaceView>>`, the trait a `Result<_, HostError>` |
| `ActivityCtx` | `joined` | ctx.placeDoor?.joined(id) === true (activity.ts:105, 217, 281) | wsp-core item: wsp_machines::places::types::PlaceDoor::joined | item answers a `bool`; the no-door case is `has_door` being false |
| `ActivityCtx` | `has_link` | door.link(place) !== undefined (activity.ts:225) | wsp-core item: wsp_machines::places::types::PlaceDoor::link | item answers `Option<Rc<dyn DaemonReach>>`; the trait asks only whether it is present |
| `ActivityCtx` | `has_local_daemon` | ctx.opts.local?.daemonRoad !== undefined (activity.ts:117, 318) | opts: RuntimeOptions.local | reads `LocalWiring.daemon_road.is_some()` (wsp-core `types/wiring.rs`) |
| `ActivityCtx` | `local_channel` | ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, onEvent), through ctx.killGuarded(..., true) where guarded (activity.ts:118, 320) | gap: helper over `ChannelsArea.channelOver` and `killGuarded` and `KindsArea.localRoad`; no single home | items exist for the parts: `wsp_account::account::channels::daemon::ChannelsArea::channel_over` (takes a `Value` listener, answers `AccountResult<Rc<dyn DaemonChannel>>`), `ptys::ChannelsArea::kill_guarded` and `wsp_machines::machines::kinds::roads::KindsArea::local_road`; the trait answers its own `DaemonAsk`, which wraps `overChannel`'s `ask` |
| `ActivityCtx` | `box_channel` | door.channel(place, onEvent) (activity.ts:227) | wsp-core item: wsp_machines::places::types::PlaceDoor::channel | item is sync and answers `Option<Rc<dyn DaemonChannel>>` over a listener of `Rc<dyn Fn(Map)>`; the trait answers an `Option` of a future of `Rc<dyn DaemonAsk>` |
| `ActivityCtx` | `sessions` | ctx.sessions.values() (activity.ts:173, 246) | wsp-core RuntimeCore (empty) | helper over the map: the map itself, rows typed `Row` |
| `ActivityCtx` | `records` | ctx.threadRecords (activity.ts:208, 215, 274, 280, 290) | wsp-core RuntimeCore (empty) | the map itself |
| `ActivityCtx` | `live_entry` | ctx.live.get(id) (activity.ts:174, 208, 215, 274, 280) | wsp-core RuntimeCore (empty) | helper over the map (one get) |
| `ActivityCtx` | `latest_on` | ctx.latestOn(threadId) (activity.ts:208, 215, 274, 280) | wsp-core ThreadsArea (empty) | this crate's own area |
| `ActivityCtx` | `thread_facts` | ctx.threadFacts(threadId) (activity.ts:160, 182, 231, 290) | wsp-core item: wsp_machines::machines::boot::rows::BootArea::thread_facts | same shape |
| `ActivityCtx` | `scopes_here` | ctx.scopesHere() (activity.ts:184) | wsp-core item: wsp_account::account::proc_scopes::ProcScopesArea::scopes_here | same shape |
| `ActivityCtx` | `kept_pid` | ctx.keptAgents.get(threadId)?.pid (activity.ts:186) | wsp-core ThreadsArea (empty) | this crate's own area; helper over the `keptAgents` map (one get, one field) |
| `ActivityCtx` | `reap_kept_now` | ctx.reapKept(threadId, { now: true }) (activity.ts:349) | wsp-core ThreadsArea (empty) | this crate's own area; `{ now: true }` is folded into the method name |
| `ActivityCtx` | `host_pid` | process.pid (activity.ts:200); no ctx member | gap: node `process.pid`; no context.ts member and no wsp-core item | same gap as account's `ChannelsCtx::host_pid` |
| `ActivityCtx` | `drives` | ctx.drives({ kind }, caller) (activity.ts:207, 209) | wsp-core item: wsp_account::account::rules::RulesArea::drives | item takes a third `machine_id: Option<&str>` (TS record `{ kind, machineId? }`); the trait takes `(kind, caller)` |
| `ActivityCtx` | `refusal_for` | ctx.refusalFor(record, caller) === undefined (activity.ts:209) | wsp-core item: wsp_account::account::rules::RulesArea::refusal_for | item takes `Option<&WorkspaceLike>` and answers `Result<Option<String>, HostError>`; the trait takes `&WorkspaceRecord` and answers `Option<HostError>` |
| `ActivityCtx` | `settles_thread` | ctx.settlesThread(threadId, caller) (activity.ts:210) | wsp-core ThreadsArea (empty) | this crate's own area |
| `ActivityCtx` | `end_thread` | ctx.moduleOf(entry.record.kind).endThread?.(entry, threadId, {}) (activity.ts:258) | wsp-core item: wsp_core::types::internal::KindModule::end_thread | exact shape; `KindsArea.moduleOf` picks the module |
| `ActivityCtx` | `with_daemon` | ctx.withDaemon(entry, ask => ask({ op: "pty.kill", ptyId })) (activity.ts:261) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::with_daemon | item takes a `Work<T>` closure over `ask` where the trait sends one `Value` frame and answers the reply map |
| `ActivityCtx` | `close_servers` | ctx.slates.closeServers(threadId) (activity.ts:263) | wsp-core SlatesArea (empty) | `SlatesArea.slates`; wsp-core `types::pending::Slates` is an empty trait |

## ThreadsCtx (src/threads/threads.rs, 46 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `ThreadsCtx` | `clock` | ctx.clock (RuntimeCore, destructured at threads.ts:71) | wsp-core item: wsp_core::clock::Clock |  |
| `ThreadsCtx` | `ids` | randomUUID() (node:crypto, threads.ts:680); no ctx member | wsp-core item: wsp_core::ids::Ids | whole seam handed out, not one draw |
| `ThreadsCtx` | `bus` | ctx.bus (`bus.on`, threads.ts:385, 469, 647, 893) | wsp-core item: wsp_core::types::events::EventBus | read side only: this area never calls `bus.emit` |
| `ThreadsCtx` | `store` | ctx.store (threads.ts:454, 518, 595, 599, 856, 926) | wsp-core item: wsp_store::store::Store |  |
| `ThreadsCtx` | `sessions` | ctx.sessions (RuntimeCore) | wsp-core RuntimeCore (empty) | the map handed out as Rc<RefCell<IndexMap<String, Row>>> |
| `ThreadsCtx` | `thread_records` | ctx.threadRecords (RuntimeCore) | wsp-core RuntimeCore (empty) | the map handed out as Rc<RefCell<IndexMap>> |
| `ThreadsCtx` | `closing` | ctx.state.closing (threads.ts:120, 225, 241, 685) | wsp-core item: wsp_core::context::RuntimeState::closing | read through StateCell::with in Rust; the trait answers the bool |
| `ThreadsCtx` | `entry` | live.get(ws) (ctx.live, RuntimeCore; threads.ts:133, 141, 706) | wsp-core RuntimeCore (empty) | helper over the map; answers the machines area's Entry |
| `ThreadsCtx` | `live_records` | [...live.values()].map(e => e.record) (threads.ts:417) | wsp-core RuntimeCore (empty) | helper over the map; every live record in live's order |
| `ThreadsCtx` | `taken` | ctx.transcriptIndex.get(ws)?.taken.get(request) (threads.ts:623, 632) | wsp-core RuntimeCore (empty) | helper over the map; `Taken` is wsp_core::types::internal::Taken |
| `ThreadsCtx` | `has_place_door` | ctx.placeDoor !== undefined (RuntimeCore.placeDoor; threads.ts:121, 164) | wsp-core RuntimeCore (empty) | presence of the Option; the door's methods below are items of the 61-method PlaceDoor in wsp-machines |
| `ThreadsCtx` | `place_linked` | door.link(place) !== undefined (ctx.placeDoor; threads.ts:122, 164) | wsp-core item: wsp_machines::places::types::PlaceDoor::link | through ctx.placeDoor (RuntimeCore, optional); the door answers Option<Rc<dyn DaemonReach>>, the trait only its presence |
| `ThreadsCtx` | `on_place_move` | door.on(listener) (threads.ts:123) | wsp-core item: wsp_machines::places::types::PlaceDoor::on | PlaceDoor::on takes Rc<dyn Fn(&PlaceEvent)> and answers a u64 that PlaceDoor::off(id) takes, the trait answers the remover itself; `PlaceMove` stands for the `place.present` and `place.removed` events, and the real `PlaceEvent` enum has Joined, Changed, Removed and Absent but no Present variant |
| `ThreadsCtx` | `place_mem_mb` | ctx.placeDoor?.folderComputer(place)?.shape.memMb (threads.ts:136) | wsp-core item: wsp_machines::places::types::PlaceDoor::folder_computer | answers a typed FolderComputer whose `mem_mb` is a u64; the trait answers Option<f64> |
| `ThreadsCtx` | `place_threads_at` | ctx.placeDoor?.threadsAt(placeId) (threads.ts:413) | wsp-core item: wsp_machines::places::types::PlaceDoor::threads_at | answers Option<u64>, the trait Option<i64> |
| `ThreadsCtx` | `place_name` | ctx.placeDoor!.nameOf(placeId) (threads.ts:431) | wsp-core item: wsp_machines::places::types::PlaceDoor::name_of | through ctx.placeDoor, not BuildersArea.placeName |
| `ThreadsCtx` | `place_id_of` | ctx.placeIdOf (ChannelsArea; threads.ts:393, 412) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::place_id_of |  |
| `ThreadsCtx` | `provider_of` | ctx.providerOf (ChannelsArea; threads.ts:417) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::provider_of |  |
| `ThreadsCtx` | `revoke_device` | deviceDoor.revoke(id) (RuntimeCore.deviceDoor; threads.ts:113, 289) | wsp-core item: wsp_core::devices::DeviceDoor::revoke | answers Result<bool, HostError>, the trait Result<(), HostError> |
| `ThreadsCtx` | `proc_scopes_moved` | ctx.procScopesMoved (ProcScopesArea; threads.ts:246, 270) | wsp-core item: wsp_account::account::proc_scopes::ProcScopesArea::proc_scopes_moved |  |
| `ThreadsCtx` | `root_of` | ctx.rootOf (RulesArea; threads.ts:349) | wsp-core item: wsp_account::account::rules::RulesArea::root_of | answers String, the trait Option<String> |
| `ThreadsCtx` | `interrupt` | ctx.sessionsApi.interrupt(row.id, caller) (SessionsArea.sessionsApi; threads.ts:551) | wsp-core item: wsp_core::types::api::SessionsApi::interrupt | this crate's own area (SessionsArea); SessionsApi::interrupt takes a third `task: Option<String>` |
| `ThreadsCtx` | `start` | ctx.sessionsApi.start(ws, options, caller) (SessionsArea.sessionsApi; threads.ts:834) | wsp-core item: wsp_core::types::api::SessionsApi::start | this crate's own area (SessionsArea); same shape |
| `ThreadsCtx` | `stop_try` | ctx.stopTry (SessionsArea; threads.ts:766) | wsp-core SessionsArea (empty) | this crate's own area |
| `ThreadsCtx` | `mark` | ctx.mark (BootArea; threads.ts:695) | wsp-core item: wsp_machines::machines::boot::rows::BootArea::mark | item takes &[String], a `Stamped` (Same(ThreadStamps) or Each(fn)) and Option<&Caller>; the trait passes the ThreadStamps itself |
| `ThreadsCtx` | `sub_line` | ctx.subLine (SubLifeArea; threads.ts:793) | wsp-core item: wsp_machines::machines::sub_life::timers::SubLifeArea::sub_line | `since` is an f64 there, an i64 here |
| `ThreadsCtx` | `boot_work` | ctx.bootWork.get(ws) (RuntimeCore; threads.ts:802) | wsp-core RuntimeCore (empty) | helper over the map; wsp-machines' BootRuntime::boot_work declares the whole map |
| `ThreadsCtx` | `kept_writes` | ctx.keptWrites.get(ws) (TranscriptsArea.keptWrites; threads.ts:853) | wsp-core TranscriptsArea (empty) | this crate's own area; helper over the map, which is the `kept_writes` field of wsp-threads' TranscriptsArea |
| `ThreadsCtx` | `keep_sent_images` | ctx.keepSentImages (TranscriptsArea; threads.ts:1043) | wsp-core TranscriptsArea (empty) | this crate's own area |
| `ThreadsCtx` | `record` | ctx.record (TranscriptsArea; threads.ts:483, 519, 606, 884, 918, 953, 954, 977, 1044, 1140) | wsp-core TranscriptsArea (empty) | this crate's own area |
| `ThreadsCtx` | `persist_sessions` | ctx.persistSessions (TranscriptsArea; threads.ts:1001, 1013) | wsp-core TranscriptsArea (empty) | this crate's own area |
| `ThreadsCtx` | `open_thread` | ctx.openThread (TranscriptsArea; threads.ts:986) | wsp-core TranscriptsArea (empty) | this crate's own area |
| `ThreadsCtx` | `idle_hold` | ctx.idle.hold(id) (MachinesArea.idle; threads.ts:456) | wsp-core item: wsp_core::idle::IdlePolicy::hold | reached through the machines area's `idle` handle |
| `ThreadsCtx` | `idle_release` | ctx.idle.release(id) (MachinesArea.idle; threads.ts:462) | wsp-core item: wsp_core::idle::IdlePolicy::release | reached through the machines area's `idle` handle |
| `ThreadsCtx` | `port_roots_moved` | ctx.portRootsMoved (ChannelsArea; threads.ts:942) | wsp-core item: wsp_account::account::channels::ptys::ChannelsArea::port_roots_moved |  |
| `ThreadsCtx` | `end_snooze_for` | ctx.endSnoozeFor(s.view) (BootArea; threads.ts:944) | wsp-core item: wsp_machines::machines::boot::rows::BootArea::end_snooze_for | item takes (Option<&str> thread id, Option<&str> root thread id); the trait takes the &SessionView |
| `ThreadsCtx` | `with_daemon` | ctx.withDaemon(entry, ask => ask(frame)) (ChannelsArea; threads.ts:969, 1071, 1087, 1125) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::with_daemon | item takes `Work<T>` (a boxed FnOnce over an `Ask`) and answers AccountResult<T>; the trait is the one-frame form every call here uses |
| `ThreadsCtx` | `checkout_of` | ctx.checkoutOf (ChannelsArea; threads.ts:969) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::checkout_of | answers Result<String, HostError> |
| `ThreadsCtx` | `project_held` | ctx.projectHeld (ChannelsArea; threads.ts:957) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::project_held | answers Result<ProjectView, HostError> (TS throws on an unheld id); not `ctx.projectsHeld.has` |
| `ThreadsCtx` | `reach_of` | ctx.reachOf (ReachArea; threads.ts:1068) | wsp-core item: wsp_machines::machines::reach::disk::ReachArea::reach_of | same shape; both answer wsp_wire's ReachState |
| `ThreadsCtx` | `turn_snapshot_ms` | opts.turnSnapshotMs (threads.ts:1061) | opts: RuntimeOptions.turn_snapshot_ms | Option<u64> there, Option<i64> here |
| `ThreadsCtx` | `vault` | opts.vault?.() (threads.ts:1152) | opts: RuntimeOptions.vault | an Option of a closure there; the trait answers its map, empty where none is wired |
| `ThreadsCtx` | `vault_on` | ctx.vaultOn (KindsArea; threads.ts:1163, 1173) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::vault_on | item takes &LiveWorkspace, the trait &Entry |
| `ThreadsCtx` | `login_stands` | ctx.moduleOf(kind).loginStands(entry, harness) (KindsArea.moduleOf; threads.ts:1163, 1173) | wsp-core item: wsp_core::types::internal::KindModule::login_stands | reached through KindsArea::module_of (wsp_machines::machines::kinds::KindsArea::module_of, answers Result<Rc<dyn KindModule>>); the trait takes the entry and folds the lookup |
| `ThreadsCtx` | `computer_of` | ctx.computerOf (ChannelsArea; threads.ts:1164, 1174) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::computer_of |  |
| `ThreadsCtx` | `ledger_limits` | ctx.ledger.limits() (UsageArea.ledger; threads.ts:1172) | wsp-core UsageArea (empty) | `UsageLedger` has no Rust item (wsp-machines usage.rs is an empty file); wsp-account's UsageCtx::limits declares the same call untyped (Vec<Value>), the trait answers typed AccountLimit |

## TranscriptsCtx (src/threads/transcripts.rs, 19 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `TranscriptsCtx` | `clock` | ctx.clock (destructured at transcripts.ts:19; `clock.now` 510, `clock.schedule` 712); `Date.now()` is also read direct at 539, 543, 677 | wsp-core item: wsp_core::clock::Clock | the TS's direct `Date.now()` calls go through this one clock here |
| `TranscriptsCtx` | `ids` | randomBytes(24) (node:crypto, transcripts.ts:528); no ctx member | wsp-core item: wsp_core::ids::Ids | whole seam handed out; the TS draws bytes only |
| `TranscriptsCtx` | `bus` | ctx.bus (`bus.on("workspace.cost")`, transcripts.ts:718) | wsp-core item: wsp_core::types::events::EventBus | read side |
| `TranscriptsCtx` | `emit` | bus.emit(event) (transcripts.ts:682) | wsp-core item: wsp_core::types::events::Bus::emit | a method of the concrete `Bus` struct, not of the EventBus trait; answers Result<u64, HostError>, the trait Result<(), HostError> |
| `TranscriptsCtx` | `store` | ctx.store (transcripts.ts:27-229, 653) | wsp-core item: wsp_store::store::Store |  |
| `TranscriptsCtx` | `rows` | ctx.rows = store.transcripts (RuntimeCore.rows; runtime.ts:950) | wsp-core item: wsp_store::store::Store::transcripts | answers Option<&dyn TranscriptRows> (the memory store holds none); the trait answers an Rc<dyn TranscriptRows>, always present on the Rust host, so the TS's `rows === undefined` blob road is not ported |
| `TranscriptsCtx` | `transcript_state` | ctx.transcriptIndex, ctx.pendingEvents, ctx.pendingBytes, ctx.unreadIndexes (RuntimeCore) | wsp-core RuntimeCore (empty) | one handle over four members, `TranscriptState` (this crate's own struct); the TS also destructures `transcripts`, `transcriptBytes`, `indexFor` and `sizeOf`, which have no method here (blob road) |
| `TranscriptsCtx` | `sessions` | ctx.sessions (RuntimeCore; transcripts.ts:591, 626) | wsp-core RuntimeCore (empty) | the map handed out as Rc<RefCell<IndexMap<String, Row>>> |
| `TranscriptsCtx` | `thread_records` | ctx.threadRecords (RuntimeCore; transcripts.ts:733) | wsp-core RuntimeCore (empty) | the map handed out as Rc<RefCell<IndexMap>> |
| `TranscriptsCtx` | `index_flushes` | ctx.indexFlushes (RuntimeCore; transcripts.ts:652, 655) | wsp-core RuntimeCore (empty) | the whole map as a StateCell |
| `TranscriptsCtx` | `entry` | live.get(ws) (ctx.live, RuntimeCore; transcripts.ts:733) | wsp-core RuntimeCore (empty) | helper over the map; answers the machines area's Entry |
| `TranscriptsCtx` | `is_live` | live.has(ws) (ctx.live, RuntimeCore; transcripts.ts:618) | wsp-core RuntimeCore (empty) | helper over the map |
| `TranscriptsCtx` | `closed` | ctx.state.closed (transcripts.ts:491) | wsp-core item: wsp_core::context::RuntimeState::closed | read through StateCell::with in Rust; the trait answers the bool |
| `TranscriptsCtx` | `push_head` | ctx.pushHead (BootArea; transcripts.ts:683) | wsp-core item: wsp_machines::machines::boot::rows::BootArea::push_head | answers Result<(), HostError>, the trait nothing |
| `TranscriptsCtx` | `rows_on` | ctx.rowsOn (AgentsArea; transcripts.ts:724) | wsp-core AgentsArea (empty) | this crate's own area |
| `TranscriptsCtx` | `latest_on` | ctx.latestOn (ThreadsArea; transcripts.ts:732) | wsp-core ThreadsArea (empty) | this crate's own area |
| `TranscriptsCtx` | `resumed_fact` | ctx.resumedFact (ChannelsArea; transcripts.ts:726, 727) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::resumed_fact | TS types `fact` as one of SESSION_FACTS; both take a &str |
| `TranscriptsCtx` | `daemon_token_seed` | opts.daemonToken (transcripts.ts:517) | opts: RuntimeOptions.daemon_token |  |
| `TranscriptsCtx` | `rotate_daemon_token` | rotateDaemonToken(machine, token, daemonTokenPathOf(machine, path)) (daemon-token.ts free functions; transcripts.ts:536) | gap: TS free functions rotateDaemonToken and daemonTokenPathOf (daemon-token.ts), folded into one call; wsp-core daemon_token.rs is an empty file | also `daemonTokenFor` and `assertTokenShape` (same file) have no Rust item; the area's `new` checks the seed with them |

## AgentsCtx (src/threads/agents.rs, 42 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `AgentsCtx` | `clock` | ctx.clock (destructured at agents.ts:27; `clock.now` 85, 96, 174, 188) | wsp-core item: wsp_core::clock::Clock |  |
| `AgentsCtx` | `store` | ctx.store (agents.ts:118, 148) | wsp-core item: wsp_store::store::Store |  |
| `AgentsCtx` | `bus` | ctx.bus (`ctx.bus.on`, agents.ts:577, 580, 583) | wsp-core item: wsp_core::types::events::EventBus | read side |
| `AgentsCtx` | `sessions` | ctx.sessions (RuntimeCore) | wsp-core RuntimeCore (empty) | the map handed out as Rc<RefCell<IndexMap<String, Row>>> |
| `AgentsCtx` | `thread_records` | ctx.threadRecords (RuntimeCore) | wsp-core RuntimeCore (empty) | the map handed out as Rc<RefCell<IndexMap>> |
| `AgentsCtx` | `entry` | live.get(ws) (ctx.live, RuntimeCore; agents.ts:169, 259, 286) | wsp-core RuntimeCore (empty) | helper over the map; answers the machines area's Entry |
| `AgentsCtx` | `closing` | ctx.state.closing (agents.ts:98, 295) | wsp-core item: wsp_core::context::RuntimeState::closing | read through StateCell::with in Rust; the trait answers the bool |
| `AgentsCtx` | `adapters` | ctx.adapters (RuntimeCore; destructured at agents.ts:27, read at 171) | opts: RuntimeOptions.adapters | ctx.adapters is opts.adapters unchanged (runtime.ts:492) |
| `AgentsCtx` | `local_home_dir` | local?.homeDir (RuntimeCore.local; agents.ts:339, 392) | wsp-core item: wsp_core::types::wiring::LocalWiring::home_dir | a String field; the trait answers its Option |
| `AgentsCtx` | `local_home` | local?.home(id) (RuntimeCore.local; agents.ts:383) | wsp-core item: wsp_core::types::wiring::LocalWiring::home | a closure field Rc<dyn Fn(&str) -> String>; the trait answers its Option |
| `AgentsCtx` | `process_home` | homedir() (node:os, agents.ts:339, 383, 392); no ctx member | gap: node:os homedir(); no context.ts member and no wsp-core item | LocalWiring::home_dir is a different value (`local?.homeDir`, the row above) |
| `AgentsCtx` | `state_path` | opts.statePath (agents.ts:340) | opts: RuntimeOptions.state_path |  |
| `AgentsCtx` | `setup_off` | setups.off(place, agent) (RuntimeCore.setups; agents.ts:331) | wsp-core RuntimeCore (empty) | wsp-core agent_setup.rs (the AgentSetups store) is an empty file |
| `AgentsCtx` | `setup_config_dir` | setups.get(place, agent)?.configDir (RuntimeCore.setups; agents.ts:356, 380, 508) | wsp-core RuntimeCore (empty) | wsp-core agent_setup.rs is an empty file |
| `AgentsCtx` | `setup_launch` | setups.launchOf(place, id) (RuntimeCore.setups; agents.ts:516, 528, 641) | wsp-core RuntimeCore (empty) | wsp-core agent_setup.rs is an empty file; the trait's SetupLaunch is the env and the launch program |
| `AgentsCtx` | `setup_refusal_moved` | ctx.setupRefusalMoved (BootArea; agents.ts:373) | wsp-core item: wsp_machines::machines::boot::rows::BootArea::setup_refusal_moved |  |
| `AgentsCtx` | `place_exec` | ctx.placeDoorOf().exec(place, script, { timeoutMs }) (KindsArea.placeDoorOf; agents.ts:343, 344) | wsp-core item: wsp_machines::places::types::PlaceDoor::exec | item takes a PlaceExecOpts and answers PlaceResult<ExecResult>; the trait takes a bare timeout_ms; KindsArea in wsp-machines has no `place_door_of` |
| `AgentsCtx` | `place_name` | door.nameOf(placeId) (ctx.placeDoorOf(); agents.ts:346) | wsp-core item: wsp_machines::places::types::PlaceDoor::name_of | KindsArea in wsp-machines has no `place_door_of` |
| `AgentsCtx` | `place_id_of` | ctx.placeIdOf (ChannelsArea; agents.ts:593) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::place_id_of |  |
| `AgentsCtx` | `at_provider` | atProvider(ctx.placeIdOf(record), ctx.places) (agents.ts:593) | wsp-core item: wsp_core::types::wiring::at_provider | a free function over (Option<&str>, &dyn PlaceBackends); the trait folds in `ctx.places` (RuntimeCore) and takes the place id alone |
| `AgentsCtx` | `place_agent_home` | ctx.placeAgentHome (KindsArea; agents.ts:528) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::place_agent_home | item takes Option<&str> place and answers Result<String, HostError> |
| `AgentsCtx` | `ready` | ctx.ready (BootArea; agents.ts:377) | wsp-core item: wsp_machines::machines::boot::ready::BootArea::ready | answers a `Ready` handle (a Shared future of Result<(), String>), the trait a future of Result<(), HostError> |
| `AgentsCtx` | `know_broker` | ctx.knowBroker (KindsArea; agents.ts:405, 696) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::know_broker | item takes &LiveWorkspace, the trait &Entry |
| `AgentsCtx` | `unreached_of` | ctx.unreachedOf (DaemonArea; agents.ts:171) | wsp-core item: wsp_machines::machines::daemon::DaemonArea::unreached_of |  |
| `AgentsCtx` | `persist_sessions` | ctx.persistSessions (TranscriptsArea; agents.ts:211, 311) | wsp-core TranscriptsArea (empty) | this crate's own area |
| `AgentsCtx` | `push_head` | ctx.pushHead (BootArea; agents.ts:215, 312) | wsp-core item: wsp_machines::machines::boot::rows::BootArea::push_head | answers Result<(), HostError>, the trait nothing |
| `AgentsCtx` | `write_session` | ctx.writeSession (ThreadsArea; agents.ts:266) | wsp-core ThreadsArea (empty) | this crate's own area; TS takes a thunk `() => Promise<SessionRenameWrite>`, the trait takes the started future |
| `AgentsCtx` | `kind_home` | ctx.moduleOf(kind).home(entry, id) (KindsArea.moduleOf; agents.ts:508) | wsp-core item: wsp_core::types::internal::KindModule::home | reached through KindsArea::module_of (wsp_machines::machines::kinds::KindsArea::module_of, answers Result<Rc<dyn KindModule>>); the trait takes the entry and folds the lookup |
| `AgentsCtx` | `kind_env` | ctx.moduleOf(kind).env(entry, harness, id => agentHome(entry, id)) (agents.ts:517) | wsp-core item: wsp_core::types::internal::KindModule::env | same shape; through module_of (see kind_home) |
| `AgentsCtx` | `kind_server_values` | ctx.moduleOf(kind).serverValues (agents.ts:563) | wsp-core item: wsp_core::types::internal::KindModule::server_values | item takes no entry and answers a ServerValuesRoad that is never absent; the trait takes the entry and answers an Option; through module_of (see kind_home) |
| `AgentsCtx` | `kind_ends_threads` | ctx.moduleOf(kind).endThread !== undefined (agents.ts:643) | wsp-core item: wsp_core::types::internal::KindModule::end_thread | the item takes (entry, thread_id, EndThread) and answers an Option of a future whose None means the kind has no end, so no presence test that does not run the end; the trait answers the presence as a bool; through module_of (see kind_home) |
| `AgentsCtx` | `kind_memory_key` | ctx.moduleOf(kind).memoryKey(entry, harness) (agents.ts:657) | wsp-core item: wsp_core::types::internal::KindModule::memory_key | same shape; through module_of (see kind_home) |
| `AgentsCtx` | `kind_login_stands` | ctx.moduleOf(kind).loginStands(entry, id) (agents.ts:664) | wsp-core item: wsp_core::types::internal::KindModule::login_stands | same shape; through module_of (see kind_home) |
| `AgentsCtx` | `computer_runs_in_folder` | runsInFolder(ctx.kindOf(computer)) (agents.ts:673, 679) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::kind_of | a helper over two items: KindsArea::kind_of answers a WorkspaceKind, and the @wsp/protocol function is wsp_words::projects::runs_in_folder(&WorkspaceKind) |
| `AgentsCtx` | `brokered_launch` | ctx.brokeredLaunch (KindsArea; agents.ts:638) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::brokered_launch | item takes &LiveWorkspace and answers Result<Option<BrokeredLaunch>, HostError>, the trait an Option |
| `AgentsCtx` | `vault` | opts.vault?.() (agents.ts:562) | opts: RuntimeOptions.vault | an Option of a closure there; the trait answers its map, empty where none is wired |
| `AgentsCtx` | `vault_on` | ctx.vaultOn (KindsArea; agents.ts:639) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::vault_on | item takes &LiveWorkspace, the trait &Entry |
| `AgentsCtx` | `exec_factory_for` | ctx.execFactoryFor(entry, limits, waiting) with ctx.turnLimitOf and ctx.subKilledWhy folded in (KindsArea; agents.ts:645-651) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::exec_factory_for | item takes Option<MachineExecOptions> and answers Result<Arc<dyn ExecStreamFactory>, HostError>; the trait takes a `TurnLaunch` that the join lays over ChannelsArea::turn_limit_of (answers a TurnWall) and the sub-thread kill reading ctx.subKilledWhy (SubSpawnArea): a helper over three members |
| `AgentsCtx` | `signs_in` | opts.agentsReader?.signsIn?.(addresses) (agents.ts:608) | opts: RuntimeOptions.agents_reader | `AgentsReader` is an empty trait in wsp-core types/pending.rs and wsp-machines agents_read.rs is an empty file, so `signsIn` has nothing to call; the trait folds both optional chains into an Option of a closure |
| `AgentsCtx` | `resolve_project` | ctx.projectsDoor.resolve (ProjectsArea; agents.ts:672) | wsp-core item: wsp_account::projects::projects::door::Projects::resolve | same shape; the interface is wsp_core::types::api::ProjectsApi::resolve |
| `AgentsCtx` | `list_projects` | ctx.projectsDoor.list (ProjectsArea; agents.ts:679) | wsp-core item: wsp_account::projects::projects::door::Projects::list | same shape; the interface is wsp_core::types::api::ProjectsApi::list |
| `AgentsCtx` | `project_folder` | ctx.projectFolder (FoldersArea; agents.ts:674) | wsp-core item: wsp_account::projects::folders::place::Folders::project_folder | same shape |

## ConversationsCtx (src/threads/conversations.rs, 21 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `ConversationsCtx` | `clock` | Date.now() (conversations.ts:148, 162); no ctx member read | wsp-core item: wsp_core::clock::Clock | the TS reads Date.now() direct; here it goes through the one clock |
| `ConversationsCtx` | `bus` | ctx.bus (`ctx.bus.on`, conversations.ts:79, 80) | wsp-core item: wsp_core::types::events::EventBus | read side |
| `ConversationsCtx` | `sessions` | ctx.sessions (RuntimeCore; conversations.ts:69, 86) | wsp-core RuntimeCore (empty) | the map handed out as Rc<RefCell<IndexMap<String, Row>>> |
| `ConversationsCtx` | `transcript_state` | ctx.transcriptIndex (RuntimeCore; conversations.ts:69, 85) | wsp-core RuntimeCore (empty) | this area reads only the index's `starts`; the handle is the same `TranscriptState` over four members the transcripts area takes |
| `ConversationsCtx` | `live_project` | ctx.live.get(ws)?.record.project (conversations.ts:76) | wsp-core RuntimeCore (empty) | helper over the map |
| `ConversationsCtx` | `with_daemon` | ctx.withDaemon(entry, ask => ask(frame)) (ChannelsArea; conversations.ts:92) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::with_daemon | item takes `Work<T>` and answers AccountResult<T>; the trait is the one-frame form |
| `ConversationsCtx` | `harness_exec` | harnessExec(entry.machine, STORE_EXEC_MS) (@wsp/engine free function; conversations.ts:91) | wsp-core item: wsp_engine::run_env::harness_exec | item takes the Arc<dyn Machine> and answers wsp-engine's `HarnessExec` (an Arc<dyn Fn>); the trait takes the entry and answers wsp-adapters' `HarnessExec` trait, a second shape of the same name |
| `ConversationsCtx` | `project_held` | ctx.projectHeld (ChannelsArea; conversations.ts:98) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::project_held | answers Result<ProjectView, HostError> |
| `ConversationsCtx` | `worktrees_of` | ctx.worktreesOf (FoldersArea; conversations.ts:100) | wsp-core item: wsp_account::projects::folders::tree::Folders::worktrees_of | same shape (Vec<ListedWorktree>) |
| `ConversationsCtx` | `folders_of` | ctx.foldersOf(project.id) (CreateArea; conversations.ts:101) | wsp-core item: wsp_machines::machines::create::CreateArea::folders_of | item answers Vec<Entry>, the trait Vec<WorkspaceRecord>; not the area's own `folders_of(entry)` (conversations.ts:97) |
| `ConversationsCtx` | `has_adapter` | ctx.adapters[harness] !== undefined (conversations.ts:108, 233) | opts: RuntimeOptions.adapters | presence of a key; ctx.adapters is opts.adapters unchanged (runtime.ts:492) |
| `ConversationsCtx` | `agent_off` | ctx.agentOff (AgentsArea; conversations.ts:108) | wsp-core AgentsArea (empty) | this crate's own area |
| `ConversationsCtx` | `conversation_store` | ctx.adapterFor(entry, harness).adapter.conversations (AgentsArea.adapterFor; conversations.ts:109, 233) | wsp-core item: wsp_adapters::port::HarnessAdapter::conversations | the adapter comes from AgentsArea.adapterFor (this crate's own area); the item answers Option<Arc<dyn ConversationStore>>, the same type |
| `ConversationsCtx` | `agent_label` | ctx.agentLabel (AgentsArea; conversations.ts:118, 133, 136, 171, 206, 215, 232) | wsp-core AgentsArea (empty) | this crate's own area |
| `ConversationsCtx` | `computer_of` | ctx.computerOf (ChannelsArea; conversations.ts:117, 133, 224, 241) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::computer_of |  |
| `ConversationsCtx` | `latest_on` | ctx.latestOn (ThreadsArea; conversations.ts:183) | wsp-core ThreadsArea (empty) | this crate's own area |
| `ConversationsCtx` | `reaches_row` | ctx.reachesRow (BootArea; conversations.ts:191) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::reaches_row | item takes a `RowAt` (thread id, workspace id) and Option<TreeTalk>; the trait passes the &SessionView |
| `ConversationsCtx` | `folder_for` | ctx.folderFor (FoldersArea; conversations.ts:169, 214, 266, 268) | wsp-core item: wsp_account::projects::folders::place::Folders::folder_for | item takes a `FolderAsk` (project, branch, cwd, picks, opens); the trait takes (project, cwd, origin) loosely |
| `ConversationsCtx` | `entry_of` | ctx.entryOf (BootArea; conversations.ts:229) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::entry_of | item takes Option<&Caller> and an `EntryAsk` |
| `ConversationsCtx` | `preferences` | ctx.state.preferencesHeld ?? await ctx.preferences.get() (conversations.ts:230) | wsp-core item: wsp_core::types::api::PreferencesApi::get | two members: RuntimeState::preferences_held is an Option<serde_json::Value> in wsp-core, and the real body is wsp_account::account::preferences::PreferencesArea::get (answers AccountResult<Preferences>) |
| `ConversationsCtx` | `default_agent_of` | ctx.defaultAgentOf (AgentsArea; conversations.ts:231) | wsp-core AgentsArea (empty) | this crate's own area |

## ForksCtx (src/threads/forks.rs, 24 methods)

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `ForksCtx` | `ready` | ctx.ready (BootArea; forks.ts:68) | wsp-core item: wsp_machines::machines::boot::ready::BootArea::ready | answers a `Ready` handle (a Shared future of Result<(), String>), the trait a future of Result<(), HostError> |
| `ForksCtx` | `store` | ctx.store (forks.ts:164, 172) | wsp-core item: wsp_store::store::Store |  |
| `ForksCtx` | `sessions` | ctx.sessions (RuntimeCore; forks.ts:192) | wsp-core RuntimeCore (empty) | the map handed out as Rc<RefCell<IndexMap<String, Row>>> |
| `ForksCtx` | `thread_records` | ctx.threadRecords (RuntimeCore; forks.ts:71, 232) | wsp-core RuntimeCore (empty) | the map handed out as Rc<RefCell<IndexMap>> |
| `ForksCtx` | `latest_on` | ctx.latestOn (ThreadsArea; forks.ts:70) | wsp-core ThreadsArea (empty) | this crate's own area |
| `ForksCtx` | `rows_on` | ctx.rowsOn (AgentsArea; forks.ts:114) | wsp-core AgentsArea (empty) | this crate's own area; not ThreadsArea |
| `ForksCtx` | `running_turn` | ctx.runningOn(thread)?.turnId (ThreadsArea; forks.ts:84) | wsp-core ThreadsArea (empty) | this crate's own area; helper over runningOn |
| `ForksCtx` | `launching_turn` | ctx.launchingOn(thread)?.turnId (ThreadsArea; forks.ts:84) | wsp-core ThreadsArea (empty) | this crate's own area; helper over launchingOn |
| `ForksCtx` | `entry_of_row` | ctx.entryOfRow({ threadId, workspaceId }, origin) (BootArea; forks.ts:73) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::entry_of_row | item takes a `RowAt`, Option<&Caller>, Option<TreeTalk> and `being_made`, and answers Result<Option<Entry>, HostError>; the trait answers an Entry, so the not-found refusal forks.ts:74 raises on None has no place in the call |
| `ForksCtx` | `open_thread` | ctx.openThread (TranscriptsArea; forks.ts:79) | wsp-core TranscriptsArea (empty) | this crate's own area |
| `ForksCtx` | `started_as` | ctx.startedAs (ChannelsArea; forks.ts:115) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::started_as |  |
| `ForksCtx` | `folder_of` | ctx.folderOf (ChannelsArea; forks.ts:116) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::folder_of |  |
| `ForksCtx` | `agent_label` | ctx.agentLabel (AgentsArea; forks.ts:48, 202) | wsp-core AgentsArea (empty) | this crate's own area |
| `ForksCtx` | `kept_writes` | ctx.keptWrites.get(ws) (TranscriptsArea.keptWrites; forks.ts:163) | wsp-core TranscriptsArea (empty) | this crate's own area; helper over the map |
| `ForksCtx` | `keep_sent_images` | ctx.keepSentImages (TranscriptsArea; forks.ts:176) | wsp-core TranscriptsArea (empty) | this crate's own area |
| `ForksCtx` | `persist_sessions` | ctx.persistSessions (TranscriptsArea; forks.ts:238) | wsp-core TranscriptsArea (empty) | this crate's own area |
| `ForksCtx` | `project_held` | ctx.projectHeld (ChannelsArea; forks.ts:204) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::project_held | answers Result<ProjectView, HostError> |
| `ForksCtx` | `place_name` | ctx.placeName(project.computer) (BuildersArea; images/builders.ts:242; forks.ts:205) | wsp-core BuildersArea (empty) | BuildersArea.placeName is `placeDoor?.nameOf(place) ?? (this computer's hostname or the provider key name)`, not PlaceDoor::name_of alone; no crate holds the builders area yet |
| `ForksCtx` | `git_here` | ctx.gitHere (KindsArea; forks.ts:209, 222) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::git_here | item takes args as &[&str], Option<u64> timeout and Option<&str> stdin, and answers AreaResult<ExecResult>; the trait takes a Vec<String> |
| `ForksCtx` | `live_ids` | new Set(ctx.live.keys()) (RuntimeCore; forks.ts:210) | wsp-core RuntimeCore (empty) | helper over the map |
| `ForksCtx` | `worktree_folder` | ctx.worktreeFolder(project, top, branch, from, root, ref) (FoldersArea; forks.ts:211) | wsp-core item: wsp_account::projects::folders::place::Folders::worktree_folder | item takes (project, top, branch, parent: Option<String>, made_for: Option<String>, checkpoint: Option<String>); the trait's `from` and `at_ref` are bare &str where the item's are Options |
| `ForksCtx` | `checkout_of` | ctx.checkoutOf (ChannelsArea; forks.ts:214) | wsp-core item: wsp_account::account::channels::daemon::ChannelsArea::checkout_of | answers Result<String, HostError> |
| `ForksCtx` | `take_setup_line` | ctx.takeSetupLine (FoldersArea; forks.ts:216) | wsp-core item: wsp_account::projects::folders::place::Folders::take_setup_line | same shape |
| `ForksCtx` | `remove_worktree` | ctx.removeWorktree(made, true) (FoldersArea; forks.ts:221) | wsp-core item: wsp_account::projects::folders::tree::Folders::remove_worktree | item takes a `RemoveAsk` (ending, check) where the trait takes only `force` |
