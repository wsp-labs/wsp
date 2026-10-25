# Context trait table: wsp-machines against wsp-core's runtime context

Read at the lead's branch tip rust/2448-machines (8077d60d9), TS at 65f6b89c7 (`packages/runtime/src/context.ts`, `runtime.ts`, `machines/*.ts`, `places/*.ts`). Every area body in wsp-machines is still `todo!()`, so the table is built from the trait declarations, their doc comments and the TS area files, not from call sites in Rust.

Each row is one trait method. "Home" is exactly one of: `wsp-core <Area> (empty)` (the area trait whose TS interface in context.ts declares the member), `wsp-core item: <path>` (a real item exists; `wsp-engine item:` or `wsp-wire item:` where the item is in those crates), `opts: RuntimeOptions.<field>` (TS reads it off the options passed to createRuntime, and wsp-core has the field), or `gap: <what is missing>`.

Where a member sits in `RuntimeCore` and a real item also exists (store, clock, places, backend, bus, state fields), the row says item. A row that is a helper over a map the context holds says the area of that map and notes "helper over the map".

## Summary

628 methods in 40 traits. Table A is the 555 methods of the 35 context traits (calls the areas make out to the rest of the runtime or to what the host wired). Table B is 73 methods of 5 traits that are not calls out of an area: `PlaceDoor` (61, the door the places area implements), `PaneForwards` (3, implemented by pane-ports), `LinkSocket` (5) and `ForwardConn` (3), seams over a node `ws` socket and a `net.Socket`, and `BeforeDeploy` (1, a callback the door hands the installer). They are listed so the totals add up; drop them if the coordinator wants only the context traits.

Methods per trait, Table A:

| Trait | File | Methods |
|---|---|---|
| `BootRuntime` | machines/boot.rs | 98 |
| `LandingRoad` | machines/create.rs | 4 |
| `CreateRuntime` | machines/create.rs | 45 |
| `DaemonRuntime` | machines/daemon.rs | 16 |
| `KindsCtx` | machines/kinds.rs | 62 |
| `IdleDoor` | machines/machines.rs | 2 |
| `MachinesRuntime` | machines/machines.rs | 58 |
| `ReachCtx` | machines/reach.rs | 31 |
| `BackendFor` | machines/sub_life.rs | 1 |
| `SubLifeCtx` | machines/sub_life.rs | 31 |
| `SubQueueCtx` | machines/sub_queue.rs | 36 |
| `SubSpawnCtx` | machines/sub_spawn.rs | 62 |
| `ThreadEndsRuntime` | machines/thread_ends.rs | 8 |
| `ThreadEndsDoor` | machines/thread_ends.rs | 3 |
| `LinkRecords` | places/link.rs | 17 |
| `LinkViews` | places/link.rs | 11 |
| `ManageSetup` | places/manage.rs | 14 |
| `SetupRecords` | places/setup.rs | 3 |
| `PlaceInstaller` | places/types.rs | 1 |
| `PlaceDialler` | places/types.rs | 1 |
| `PlaceLogReader` | places/types.rs | 1 |
| `PlaceUpdater` | places/types.rs | 1 |
| `PlaceLeaver` | places/types.rs | 1 |
| `PlaceKeyForgetter` | places/types.rs | 1 |
| `PlaceSudo` | places/types.rs | 1 |
| `PlaceGithubToken` | places/types.rs | 1 |
| `PlaceUndoer` | places/types.rs | 1 |
| `PlaceRunOver` | places/types.rs | 1 |
| `PlaceBackHolder` | places/types.rs | 4 |
| `PlaceProvisioner` | places/types.rs | 9 |
| `PlaceRecording` | places/types.rs | 22 |
| `RecipeResolver` | places/types.rs | 2 |
| `LinkReads` | places/views.rs | 3 |
| `ViewsSetup` | places/views.rs | 3 |
| Total | | 555 |

Methods per trait, Table B:

| Trait | File | Methods |
|---|---|---|
| `PlaceDoor` | places/types.rs | 61 |
| `PaneForwards` | places/pane_ports.rs | 3 |
| `LinkSocket` | places/helpers.rs | 5 |
| `ForwardConn` | places/helpers.rs | 3 |
| `BeforeDeploy` | places/types.rs | 1 |
| Total | | 73 |

Supertraits are counted once, under the trait that declares them: `BackendFor` (1, supertrait of `SubLifeCtx`), `SetupRecords` (3, supertrait of `LinkRecords`), `ViewsSetup` (3, supertrait of `ManageSetup`). `IdleDoor` (2) and `LandingRoad` (4) are the sub-traits `MachinesRuntime::idle` and `CreateRuntime::project_landing` hand out.

Methods per home kind, Table A (555):

| Home kind | Methods |
|---|---|
| wsp-core area trait (empty) | 296 |
| wsp-core item (also wsp-engine items) | 106 |
| opts | 43 |
| gap, total | 110 |
| gap: TS free function, helper over several members, or no TS member | 37 |
| gap: `PlaceRecording`, assembled inline in runtime.ts, no context.ts member | 22 |
| gap: places-internal, a member of the door's own records, views or setup area, not of RuntimeContext | 51 |

Table B (73): 61 items (60 are `wsp-core item: wsp_core::types::pending::PlaceDoor`, an empty stand-in that the real trait in wsp-machines fills, and 1 is `wsp_core::reach::connect_daemon` for `LinkSocket::connect`) and 12 gaps (the seams, the callback, the interface the pane-ports area implements, and `PlaceDoor::off`).

Area homes in Table A (the `wsp-core <Area> (empty)` rows), by area:

| Area trait | Methods |
|---|---|
| RuntimeCore | 66 |
| ChannelsArea | 33 |
| RecordsArea | 26 |
| KindsArea | 25 |
| ThreadsArea | 20 |
| TranscriptsArea | 19 |
| MachinesArea | 18 |
| ReachArea | 15 |
| BootArea | 12 |
| RulesArea | 10 |
| AgentsArea | 9 |
| DaemonArea | 8 |
| BuildersArea | 7 |
| CreateArea | 6 |
| SubQueueArea | 4 |
| FoldersArea | 3 |
| ProjectsArea | 3 |
| SubLifeArea | 3 |
| SlatesArea | 2 |
| TurnsArea | 2 |
| UsageArea | 2 |
| ImageArea | 1 |
| SessionsArea | 1 |
| SubSpawnArea | 1 |

## Gaps worth the coordinator's attention

1. TS free functions the slice put on a context trait. None is a context.ts member. Where each one lives now:
   - `@wsp/protocol` `isBrokered` and `codexLoginUnreadRefusal` (`KindsCtx`): no Rust item in wsp-wire or wsp-words.
   - engine `golden.ts` `killUntilGone` (`CreateRuntime::kill_builder`) and `sweepLeftovers` (`ReachCtx`): no Rust item. `MachinesRuntime::gone_stop` is `ctx.gone.stop`, a RuntimeCore member whose `GoneWatch` type has no Rust item either.
   - `project-landing.ts` `projectLanding` (`CreateRuntime::project_landing`, with the 4 `LandingRoad` methods) and `cloneLines` (`KindsCtx::clone_lines`): wsp-core `project_landing.rs` is an empty module.
   - `projects/branch-of-record.ts` `brokeredStart` (in `CreateRuntime` and `SubSpawnCtx`), `namedOfRecord`, `landOnRecord`, `repoOfRecord`, `freeBranchName`: wsp-machines `src/projects/branch_of_record.rs` is empty.
   - `machines/brokered.ts` `codexBrokerOf`, `placeholdersOf`, `brokeredEnv`, `unbrokeredRefusal` (`KindsCtx`, and `unbrokeredRefusal` again in `SubSpawnCtx`): already ported and typed at `wsp_machines::machines::brokered`, but the traits declare `serde_json::Value` duplicates. These are not wsp-core items.
   - `machines/sub-life.ts` `subRowOf`, `diskKeptUntil` (`BootRuntime`): already ported at `wsp_machines::machines::sub_life::reads`, duplicated on the trait.
   - `usage.ts` `accountOnComputer` (`BootRuntime::account_key_on`), `os.hostname()` (`BootRuntime::hostname`), and the body of boot's own `imageHeadOrNone` (`BootRuntime::default_image_head`, which composes `RecordsArea.copyOf`, `BuildersArea.imagePlace` and `wsp_wire::views::golden_image::golden_head`).
2. Helpers that span several members, so no single area is their home: `MachinesRuntime::forget_workspace_maps` (11 members in RuntimeCore, DaemonArea, ReachArea and ChannelsArea), `SubLifeCtx::forget_marks` (ReachArea napRefusals and stopRefusals, DaemonArea unreached), `SubSpawnCtx::provider_backend` (`places.backend ?? placeDoor.backendOf`). Split them per area when the area traits are filled.
3. `spawn` is declared on `ReachCtx`, `SubLifeCtx`, `SubQueueCtx` and `SubSpawnCtx` for the TS `void promise`. It has no context.ts member and no wsp-core item. It wants one shared seam, not four copies.
4. No TS member at all: `KindsCtx::warn` (kinds.ts and brokered.ts have no `console.warn`), `SubQueueCtx::record_cell` (no mention in the spec or the oracle), `PlaceDoor::off` (TS `on` answers the unsubscribe; Table B). `KindsCtx::end_thread` and `end_owed` stand for the `threadEnds(ctx, placeDoor)` area object that kinds.ts builds itself, not a context member.
5. A real item exists but not in the shape the slice assumes:
   - `Bus::emit` (a concrete struct method, answers `Result<u64, HostError>`; the `EventBus` trait has no `emit`). The slice wants `Result<(), HostError>` and, in `ReachCtx`, `SubLifeCtx`, `SubQueueCtx` and `SubSpawnCtx`, passes a `serde_json::Value` where `Bus::emit` takes `EventUnion`. `EventBus::on` takes a typed listener where the slice passes `(&str, Fn(&Value))`.
   - `Clock`: `now()` answers `i64` and `schedule(f, ms: i64)` takes the function first; the slice uses `f64` and `schedule(ms, f)`.
   - `GoldenRecipe.on_exec` returns `()`; `MachinesRuntime::on_exec` needs `Result<(), String>`. `KillConfirm` is an empty stand-in struct.
   - `WorkspacesApi::resolve` answers a `WorkspaceView` (the slice wants the id). `SessionsApi::start` takes `StartOptions` and answers `Rc<dyn SessionHandle>`; the slice has three differently shaped calls (`sessions_start`, `start_turn`, `resume_turn`).
   - `DeviceDoor::list` answers `Vec<DeviceView>` and `revoke` answers `bool`; `agent_pid_of` answers `Option<i32>`; `LocalWiring::roots_path` is a `String`; `HereDaemon::version` answers `Result<u32>`; `Broker::status` and `codex_claims` are typed where the slice passes `Value`; `MachineExecOptions` is typed where the slice passes a `Map`; `SeedWiring::pack` takes a `SeedChoice` where the slice takes a file list; `folder_named` takes `(path, home, cwd)`; `machine_state_of` answers `MachineState` where the slice answers a `String`; `connect_daemon` takes `ReachOptions`.
   - `wsp_core::pipe::cut_off(&dyn PipeAreaContext, workspace_id, name)` against the slice's `cut_off(entry)` in `SubLifeCtx` and `SubQueueCtx`. wsp-core's `PipeAreaContext` already declares four calls the slice also declares (`daemon_token_of`, `forget_daemon_token`, `daemon_version`, `ready`).
6. Two definitions of the same names. wsp-core `types::pending::PlaceDoor` is an empty trait while the 61-method `PlaceDoor` is in wsp-machines `places/types.rs`; `CreateRuntime::place_door` returns the empty one. wsp-core `PlaceWiring` is an empty trait (`RuntimeOptions.place_links`) while wsp-machines has a `PlaceWiring` struct that carries the 12 wired roads. Both should be one item.
7. The place door is read through two paths. `RuntimeCore.placeDoor` backs 23 methods: 12 on `KindsCtx` (`has_place_door` and eleven flat `door_*` methods), 3 on `BootRuntime`, 3 on `ThreadEndsDoor`, 2 on `CreateRuntime` (`place_door`, `mark_used`), 2 on `SubQueueCtx` and 1 on `SubSpawnCtx`. `KindsArea.placeDoorOf` backs 3 more (`BootRuntime::place_for`, `CreateRuntime::place_door_name`, `MachinesRuntime::place_name_of`). These should be one `PlaceDoor` handle, not one flattened method each.
8. Places: 51 Table A methods (`SetupRecords`, `LinkRecords`, `LinkViews`, `LinkReads`, `ViewsSetup`, `ManageSetup`) are what the door's own areas read off each other through `PlaceDoorContext` (places/context.ts). They are not RuntimeContext members and need no wsp-core slot. The 22 `PlaceRecording` methods are one object runtime.ts builds inline (runtime.ts:594-770, `PlaceDoorOptions.recording`) out of ctx members; the member each one stands on is named in its row. The 12 wired roads (`PlaceInstaller` ... `PlaceProvisioner`, `PlaceBackHolder`) come in as `opts.placeLinks`, and the two `RecipeResolver` methods as `opts.recipes`.
9. The shared maps (`live`, `sessions`, `thread_records`, `builders`, `projects_held`, `boot_work`, `rereading`, `viewed_marks`, `unreached`, `nap_refusals`, `stop_refusals`) are handed out as `Rc<RefCell<IndexMap<..>>>`. RuntimeCore and the area traits in wsp-core are empty, so nothing yet says which of them owns each map in that shape.
10. Two TS members share a near name and are easy to confuse: `MachinesRuntime::project_held` is `ctx.projectsHeld.has` (RuntimeCore) while `CreateRuntime`, `KindsCtx`, `ReachCtx` and `SubSpawnCtx` `project_held` are `ctx.projectHeld` (ChannelsArea); `BootRuntime::cap_wait` is `ctx.capHeld` and `setup_refusal` is `ctx.setupRefusals`, not `ctx.capWait` and `ctx.setupRefusal`; `PlaceRecording::running_on` is not `ThreadsArea.runningOn`.

## Table A: context traits

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `BootRuntime` | `store` | ctx.store | wsp-core item: wsp_store::store::Store |  |
| `BootRuntime` | `clock` | ctx.clock | wsp-core item: wsp_core::clock::Clock |  |
| `BootRuntime` | `ids` | randomBytes (node:crypto, boot.ts:334); no ctx member | wsp-core item: wsp_core::ids::Ids::bytes |  |
| `BootRuntime` | `places` | ctx.places | wsp-core item: wsp_core::types::wiring::PlaceBackends |  |
| `BootRuntime` | `emit` | bus.emit | wsp-core item: wsp_core::types::events::Bus::emit |  |
| `BootRuntime` | `pass` | bus.pass | wsp-core item: wsp_core::types::events::Bus::pass |  |
| `BootRuntime` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `BootRuntime` | `owner` | ctx.state.owner | wsp-core item: wsp_core::context::RuntimeState::owner |  |
| `BootRuntime` | `set_owner` | ctx.state.owner = ... | wsp-core item: wsp_core::context::RuntimeState::owner |  |
| `BootRuntime` | `reads_since` | ctx.state.readsSince | wsp-core item: wsp_core::context::RuntimeState::reads_since |  |
| `BootRuntime` | `set_reads_since` | ctx.state.readsSince = ... | wsp-core item: wsp_core::context::RuntimeState::reads_since |  |
| `BootRuntime` | `closed` | ctx.state.closed | wsp-core item: wsp_core::context::RuntimeState::closed |  |
| `BootRuntime` | `closing` | ctx.state.closing | wsp-core item: wsp_core::context::RuntimeState::closing |  |
| `BootRuntime` | `hostname` | os.hostname() (node:os, boot.ts:1039) | gap: node:os hostname(); no context.ts member and no wsp-core item |  |
| `BootRuntime` | `local_wired` | ctx.local !== undefined (RuntimeCore.local) | wsp-core item: wsp_core::types::wiring::LocalWiring | presence of the Option |
| `BootRuntime` | `live` | ctx.live | wsp-core RuntimeCore (empty) |  |
| `BootRuntime` | `sessions` | ctx.sessions | wsp-core RuntimeCore (empty) |  |
| `BootRuntime` | `thread_records` | ctx.threadRecords | wsp-core RuntimeCore (empty) |  |
| `BootRuntime` | `builders` | ctx.builders | wsp-core RuntimeCore (empty) |  |
| `BootRuntime` | `projects_held` | ctx.projectsHeld | wsp-core RuntimeCore (empty) |  |
| `BootRuntime` | `boot_work` | ctx.bootWork | wsp-core RuntimeCore (empty) |  |
| `BootRuntime` | `rereading` | ctx.rereading (MachinesArea) | wsp-core MachinesArea (empty) |  |
| `BootRuntime` | `viewed_marks` | ctx.viewedMarks (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `BootRuntime` | `has_transcript_index` | ctx.transcriptIndex.has (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over the map |
| `BootRuntime` | `index_pos` | ctx.transcriptIndex.get(ws)?.pos (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over the map |
| `BootRuntime` | `index_children` | ctx.transcriptIndex children (boot.ts:697,704) (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over the map |
| `BootRuntime` | `observed` | ctx.observed (MachinesArea) | wsp-core MachinesArea (empty) |  |
| `BootRuntime` | `attach` | ctx.attach | wsp-core MachinesArea (empty) |  |
| `BootRuntime` | `mark_gone` | ctx.markGone | wsp-core MachinesArea (empty) |  |
| `BootRuntime` | `idle_touch` | ctx.idle.touch | wsp-core item: wsp_core::idle::IdlePolicy::touch |  |
| `BootRuntime` | `sync_daemon` | ctx.syncDaemon | wsp-core DaemonArea (empty) |  |
| `BootRuntime` | `backend_for` | ctx.backendFor | wsp-core KindsArea (empty) |  |
| `BootRuntime` | `module_of` | ctx.moduleOf | wsp-core KindsArea (empty) |  |
| `BootRuntime` | `persist` | ctx.persist | wsp-core RecordsArea (empty) |  |
| `BootRuntime` | `shape_of` | ctx.shapeOf | wsp-core RecordsArea (empty) |  |
| `BootRuntime` | `size_built` | ctx.sizeBuilt | wsp-core RecordsArea (empty) |  |
| `BootRuntime` | `arm_grace` | ctx.armGrace (BuildersArea) | wsp-core BuildersArea (empty) |  |
| `BootRuntime` | `door_backend_of` | placeDoor?.backendOf(place) (boot.ts:46,84) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `BootRuntime` | `image_kind` | ctx.imageOf(golden).version?.kind | wsp-core RecordsArea (empty) | derived from imageOf |
| `BootRuntime` | `attach_queued` | ctx.attachQueued (SubQueueArea) | wsp-core SubQueueArea (empty) |  |
| `BootRuntime` | `migrate_copies` | ctx.migrateCopies (RecordsArea) | wsp-core RecordsArea (empty) |  |
| `BootRuntime` | `place_door_load` | placeDoor?.load() (boot.ts:346) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `BootRuntime` | `git_top_of` | ctx.gitTopOf (KindsArea) | wsp-core KindsArea (empty) |  |
| `BootRuntime` | `remember_project` | ctx.rememberProject (RulesArea) | wsp-core RulesArea (empty) |  |
| `BootRuntime` | `setups_load` | ctx.setups.load (RuntimeCore.setups) | wsp-core RuntimeCore (empty) |  |
| `BootRuntime` | `move_transcript` | ctx.moveTranscript (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `BootRuntime` | `move_blobs` | ctx.moveBlobs | wsp-core TranscriptsArea (empty) |  |
| `BootRuntime` | `load_index` | ctx.loadIndex | wsp-core TranscriptsArea (empty) |  |
| `BootRuntime` | `wake_at` | ctx.wakeAt (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `BootRuntime` | `resume_on_reset` | ctx.resumeOnReset (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `BootRuntime` | `settle_cut` | ctx.settleCut (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `persist_sessions` | ctx.persistSessions (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `BootRuntime` | `turn_changes_written` | turnWritten(await ctx.openThread(ws, thread), turn).changes | wsp-core TranscriptsArea (empty) | openThread; turnWritten is wsp_core::types::internal::turn_written |
| `BootRuntime` | `read_turn_changes` | ctx.readTurnChanges (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `reattach` | ctx.reattach (TurnsArea) | wsp-core TurnsArea (empty) |  |
| `BootRuntime` | `devices` | deviceDoor.list() (RuntimeCore.deviceDoor) | wsp-core item: wsp_core::devices::DeviceDoor::list | DeviceDoor::list answers Vec<DeviceView>, slice wants Vec<ScopedDevice> |
| `BootRuntime` | `revoke_device` | deviceDoor.revoke(id) | wsp-core item: wsp_core::devices::DeviceDoor::revoke | DeviceDoor::revoke answers bool, slice wants () |
| `BootRuntime` | `revoke_asides` | deviceDoor.revokeAsides() | wsp-core item: wsp_core::devices::DeviceDoor::revoke_asides |  |
| `BootRuntime` | `shell_held_devices` | ctx.shellHeldDevices (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `BootRuntime` | `thread_runs` | ctx.threadRuns (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `worktree_mounted` | ctx.worktreeMounted (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `BootRuntime` | `sweep_runs` | ctx.sweepRuns (TurnsArea) | wsp-core TurnsArea (empty) |  |
| `BootRuntime` | `resume_queue` | ctx.resumeQueue (SubQueueArea) | wsp-core SubQueueArea (empty) |  |
| `BootRuntime` | `notify_end` | ctx.notifyEnd (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `record` | ctx.record (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `BootRuntime` | `arm_subs` | ctx.armSubs (SubLifeArea) | wsp-core SubLifeArea (empty) |  |
| `BootRuntime` | `deliver_owed` | ctx.deliverOwed (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `move_old_copies` | ctx.moveOldCopies (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `BootRuntime` | `arm_sweep` | ctx.armSweep (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `BootRuntime` | `slates_ready` | ctx.slates.ready() (SlatesArea.slates) | wsp-core SlatesArea (empty) | wsp-core pending.rs Slates is an empty trait |
| `BootRuntime` | `drives_thread` | ctx.drivesThread (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `acts_on` | ctx.actsOn (RulesArea) | wsp-core RulesArea (empty) |  |
| `BootRuntime` | `talks_to_its_tree` | ctx.talksToItsTree (RulesArea) | wsp-core RulesArea (empty) |  |
| `BootRuntime` | `refuse_relayed` | ctx.refuseRelayed (RulesArea) | wsp-core RulesArea (empty) |  |
| `BootRuntime` | `resolve_workspace` | ctx.workspaces.resolve(ref, origin) (WorkspacesArea.workspaces) | wsp-core item: wsp_core::types::api::WorkspacesApi::resolve | WorkspacesApi::resolve answers a WorkspaceView, slice wants the id |
| `BootRuntime` | `place_rows` | placeDoor.rows() (boot.ts:1039) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `BootRuntime` | `default_image_head` | goldenHead(await ctx.copyOf(await ctx.imagePlace("default"), "default"))?.snapshotId (the body of BootArea.imageHeadOrNone, boot.ts:1051) | gap: no context member; it is the body of BootArea's own imageHeadOrNone over RecordsArea.copyOf, BuildersArea.imagePlace and wsp_wire::views::golden_image::golden_head |  |
| `BootRuntime` | `place_for` | ctx.placeDoorOf().placeFor(place) (boot.ts:1064) | wsp-core KindsArea (empty) | placeDoorOf; placeFor is a PlaceDoor method |
| `BootRuntime` | `restarts` | ctx.restarts (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `stopped_behind` | ctx.stoppedBehind (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `cap_wait` | ctx.capHeld.get(turnId)?.waiting?.wait (ThreadsArea.capHeld) | wsp-core ThreadsArea (empty) | helper over the map; not ctx.capWait |
| `BootRuntime` | `agent_pid_of` | agentPidOf(run) (local-exec.ts free fn) | wsp-core item: wsp_core::local_exec::reap::agent_pid_of | item answers Option<i32>, slice Option<u32> |
| `BootRuntime` | `setup_refusal` | ctx.setupRefusals.get(keyOf(place, harness)) (AgentsArea.setupRefusals) | wsp-core AgentsArea (empty) | helper over the map; not ctx.setupRefusal |
| `BootRuntime` | `setup_place` | ctx.setupPlace (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `BootRuntime` | `latest_on` | ctx.latestOn (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `rows_on` | ctx.rowsOn (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `BootRuntime` | `sub_row_of` | subRowOf(record) (sub-life.ts free fn) | gap: TS free function subRowOf (sub-life.ts); already ported at wsp_machines::machines::sub_life::reads::sub_row_of, not a wsp-core item |  |
| `BootRuntime` | `disk_kept_until` | diskKeptUntil(ctx, record) (sub-life.ts free fn) | gap: TS free function diskKeptUntil (sub-life.ts); already ported at wsp_machines::machines::sub_life::reads::disk_kept_until, not a wsp-core item |  |
| `BootRuntime` | `place_id_of` | ctx.placeIdOf (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `BootRuntime` | `sub_read` | ctx.subRead (SubLifeArea) | wsp-core SubLifeArea (empty) |  |
| `BootRuntime` | `title_rows` | ctx.titleRows (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `BootRuntime` | `refresh_title` | ctx.refreshTitle (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `BootRuntime` | `reap_kept` | ctx.reapKept (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `BootRuntime` | `cancel_resume` | ctx.resumeTimers.get(thread)?.() (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over the map |
| `BootRuntime` | `lead_asks` | ctx.leadAsks.has(threadId) (ThreadsArea) | wsp-core ThreadsArea (empty) | helper over the map |
| `BootRuntime` | `ledger_limits` | ctx.ledger.limits() (UsageArea.ledger) | wsp-core UsageArea (empty) |  |
| `BootRuntime` | `account_key_on` | accountOnComputer({...}).key (usage.ts free fn) over ctx.vaultedFor, ctx.usageComputerOf, ctx.computerOf, ctx.vaultOn | gap: TS free function accountOnComputer (usage.ts), composed with members of ThreadsArea, ChannelsArea and KindsArea |  |
| `BootRuntime` | `resume_turn` | ctx.sessionsApi.start(ws, {prompt, thread, afterLimit}) (SessionsArea.sessionsApi) | wsp-core item: wsp_core::types::api::SessionsApi::start | SessionsApi::start takes StartOptions and answers Rc<dyn SessionHandle> |
| `LandingRoad` | `kind` | ProjectLanding.kind (project-landing.ts:71) | gap: member of the object projectLanding(kind) answers (project-landing.ts); wsp-core project_landing.rs is an empty module |  |
| `LandingRoad` | `refusal` | ProjectLanding.refusal | gap: member of the object projectLanding(kind) answers (project-landing.ts) |  |
| `LandingRoad` | `memory_dir` | ProjectLanding.places({...}).memoryDir | gap: member of the object projectLanding(kind) answers (project-landing.ts) |  |
| `LandingRoad` | `workspace_binds` | ProjectLanding.workspaceBinds | gap: member of the object projectLanding(kind) answers (project-landing.ts) |  |
| `CreateRuntime` | `clock` | ctx.clock (RuntimeCore) | wsp-core item: wsp_core::clock::Clock |  |
| `CreateRuntime` | `ids` | randomUUID (node:crypto, create.ts:62); no ctx member | wsp-core item: wsp_core::ids::Ids::uuid |  |
| `CreateRuntime` | `emit` | bus.emit | wsp-core item: wsp_core::types::events::Bus::emit |  |
| `CreateRuntime` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `CreateRuntime` | `live` | ctx.live | wsp-core RuntimeCore (empty) |  |
| `CreateRuntime` | `wired` | places.wired | wsp-core item: wsp_core::types::wiring::PlaceBackends::wired |  |
| `CreateRuntime` | `place_door` | ctx.placeDoor (RuntimeCore) | wsp-core RuntimeCore (empty) | wsp-core pending.rs PlaceDoor is an empty handle; the 61-method PlaceDoor trait is in wsp-machines places/types.rs |
| `CreateRuntime` | `landing_place` | ctx.landingPlace (BootArea) | wsp-core BootArea (empty) |  |
| `CreateRuntime` | `place_refuses` | ctx.placeRefuses | wsp-core ChannelsArea (empty) |  |
| `CreateRuntime` | `landing_backend` | ctx.landingBackend (BuildersArea) | wsp-core BuildersArea (empty) |  |
| `CreateRuntime` | `landing_kind` | ctx.landingKind (ProjectsArea) | wsp-core ProjectsArea (empty) |  |
| `CreateRuntime` | `landing_deps` | ctx.landingDeps (ProjectsArea) | wsp-core ProjectsArea (empty) | slice keeps only .deps of {deps, at, placeId} |
| `CreateRuntime` | `project_landing` | projectLanding(kind) (project-landing.ts free fn) | gap: TS free function projectLanding (project-landing.ts); wsp-core project_landing.rs is an empty module |  |
| `CreateRuntime` | `remember_project` | ctx.rememberProject (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateRuntime` | `project_held` | ctx.projectHeld | wsp-core ChannelsArea (empty) |  |
| `CreateRuntime` | `keeps_images` | ctx.keepsImages | wsp-core KindsArea (empty) |  |
| `CreateRuntime` | `copy_for_fork` | ctx.copyForFork (ImageArea) | wsp-core ImageArea (empty) |  |
| `CreateRuntime` | `image_head` | ctx.imageHead (BootArea) | wsp-core BootArea (empty) |  |
| `CreateRuntime` | `image_size` | ctx.imageOf(golden).version?.size | wsp-core RecordsArea (empty) | derived from imageOf |
| `CreateRuntime` | `brokered_start` | brokeredStart(ctx, provider) (projects/branch-of-record.ts free fn) | gap: TS free function brokeredStart (projects/branch-of-record.ts); wsp-machines src/projects/branch_of_record.rs is empty |  |
| `CreateRuntime` | `named_of_record` | namedOfRecord(ctx, ...) (projects/branch-of-record.ts free fn) | gap: TS free function namedOfRecord (projects/branch-of-record.ts) |  |
| `CreateRuntime` | `place_name` | ctx.placeName (BuildersArea) | wsp-core BuildersArea (empty) |  |
| `CreateRuntime` | `place_door_name` | ctx.placeDoorOf().nameOf(place) | wsp-core KindsArea (empty) | placeDoorOf; duplicates place_name above |
| `CreateRuntime` | `spawn_at` | ctx.spawnAt (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `CreateRuntime` | `tree_of` | ctx.treeOf (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateRuntime` | `attach` | ctx.attach | wsp-core MachinesArea (empty) |  |
| `CreateRuntime` | `fork` | ctx.fork | wsp-core MachinesArea (empty) |  |
| `CreateRuntime` | `refresh_builders` | ctx.refreshBuilders (BootArea) | wsp-core BootArea (empty) |  |
| `CreateRuntime` | `builders` | ctx.builders (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `CreateRuntime` | `cancel_grace` | ctx.graceTimers.get(id)?.() (BuildersArea) | wsp-core BuildersArea (empty) | helper over the map |
| `CreateRuntime` | `kill_builder` | killUntilGone(ctx.backendAt(...), machine, opts.killConfirm) (engine golden.ts free fn) | gap: TS free function killUntilGone (engine golden.ts, no Rust item); reads ctx.backendAt (BuildersArea) and opts.killConfirm |  |
| `CreateRuntime` | `forget_builder` | ctx.forgetBuilder | wsp-core BuildersArea (empty) |  |
| `CreateRuntime` | `holds_slot` | ctx.holdsSlot (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateRuntime` | `served_by_its_computer` | ctx.servedByItsComputer (ChannelsArea) | wsp-core ChannelsArea (empty) | slice answers Option<String>, TS answers a function or undefined |
| `CreateRuntime` | `daemon_answers_ms` | ctx.lifecycleOf(entry).budgets.daemonAnswersMs | wsp-core KindsArea (empty) | lifecycleOf |
| `CreateRuntime` | `daemon_reach` | entry.ws.daemonReach() (LiveWorkspace.ws) | wsp-engine item: wsp_engine::lifecycle::Workspace::daemon_reach | reached through LiveWorkspace.ws, not ctx |
| `CreateRuntime` | `ping_daemon` | ctx.pingDaemon | wsp-core ReachArea (empty) |  |
| `CreateRuntime` | `sync_daemon` | ctx.syncDaemon | wsp-core DaemonArea (empty) |  |
| `CreateRuntime` | `read_memory` | ctx.readMemory | wsp-core RecordsArea (empty) |  |
| `CreateRuntime` | `module_of` | ctx.moduleOf | wsp-core KindsArea (empty) |  |
| `CreateRuntime` | `write_daemon_roots` | ctx.writeDaemonRoots | wsp-core DaemonArea (empty) |  |
| `CreateRuntime` | `persist` | ctx.persist | wsp-core RecordsArea (empty) |  |
| `CreateRuntime` | `mark_used` | placeDoor?.markUsed(placeId) (create.ts:197) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `CreateRuntime` | `view` | ctx.view | wsp-core RecordsArea (empty) |  |
| `CreateRuntime` | `ref_of` | ctx.refOf (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `DaemonRuntime` | `clock` | ctx.clock | wsp-core item: wsp_core::clock::Clock |  |
| `DaemonRuntime` | `ids` | randomUUID (node:crypto, daemon.ts:19); no ctx member | wsp-core item: wsp_core::ids::Ids::uuid |  |
| `DaemonRuntime` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `DaemonRuntime` | `daemon_hello_timeout_ms` | ctx.daemonHelloTimeoutMs (RuntimeCore; opts.daemonHelloTimeoutMs) | wsp-core RuntimeCore (empty) |  |
| `DaemonRuntime` | `live` | ctx.live | wsp-core RuntimeCore (empty) |  |
| `DaemonRuntime` | `sessions` | ctx.sessions | wsp-core RuntimeCore (empty) |  |
| `DaemonRuntime` | `daemon_notes` | ctx.daemonNotes (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `DaemonRuntime` | `closed` | ctx.state.closed | wsp-core item: wsp_core::context::RuntimeState::closed |  |
| `DaemonRuntime` | `sessions_start` | ctx.sessionsApi.start (SessionsArea.sessionsApi) | wsp-core item: wsp_core::types::api::SessionsApi::start | SessionsApi::start takes StartOptions and answers Rc<dyn SessionHandle>; slice takes DetachedStart and answers Started |
| `DaemonRuntime` | `push_status` | ctx.pushStatus (ReachArea) | wsp-core ReachArea (empty) |  |
| `DaemonRuntime` | `persist` | ctx.persist | wsp-core RecordsArea (empty) |  |
| `DaemonRuntime` | `served_by_its_computer` | ctx.servedByItsComputer | wsp-core ChannelsArea (empty) |  |
| `DaemonRuntime` | `module_of` | ctx.moduleOf | wsp-core KindsArea (empty) |  |
| `DaemonRuntime` | `project_path` | ctx.projectHeld(id).path | wsp-core ChannelsArea (empty) | projectHeld |
| `DaemonRuntime` | `checkout_of` | ctx.checkoutOf | wsp-core ChannelsArea (empty) |  |
| `DaemonRuntime` | `backend_for` | ctx.backendFor | wsp-core KindsArea (empty) |  |
| `KindsCtx` | `now` | clock.now() (kinds.ts:32) | wsp-core item: wsp_core::clock::Clock::now | slice answers f64, Clock::now answers i64 |
| `KindsCtx` | `state_path` | opts.statePath | opts: RuntimeOptions.state_path |  |
| `KindsCtx` | `local_machine` | moduleOf("local").backend(...).get(LOCAL_MACHINE_ID) over local.backend (RuntimeCore.local) | wsp-core item: wsp_core::types::wiring::LocalWiring::backend |  |
| `KindsCtx` | `own_backend` | ctx.backend (RuntimeCore) | wsp-engine item: wsp_engine::machine::MachineBackend |  |
| `KindsCtx` | `released_backend` | RELEASED_BACKEND (types/internal.ts) | wsp-core item: wsp_core::types::internal::released_backend |  |
| `KindsCtx` | `place_name` | ctx.placeName (BuildersArea) | wsp-core BuildersArea (empty) |  |
| `KindsCtx` | `wired_place` | places.wired | wsp-core item: wsp_core::types::wiring::PlaceBackends::wired |  |
| `KindsCtx` | `project_held` | ctx.projectHeld (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `KindsCtx` | `checkout_of` | ctx.checkoutOf (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `KindsCtx` | `computer_of` | ctx.computerOf (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `KindsCtx` | `served_by_its_computer` | ctx.servedByItsComputer (ChannelsArea) | wsp-core ChannelsArea (empty) | slice answers bool, TS answers a function or undefined |
| `KindsCtx` | `has_place_door` | ctx.placeDoor !== undefined (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `KindsCtx` | `door_backend_of` | placeDoor?.backendOf (kinds.ts:238,328) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `door_name_of` | placeDoor.nameOf (kinds.ts:329,446,498) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `door_folder_computer` | placeDoor.folderComputer (kinds.ts:228,480) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `door_joined` | placeDoor?.joined (kinds.ts:229,508) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `door_logins_of` | placeDoor?.backendOf(place)?.logins (kinds.ts:238) | wsp-core RuntimeCore (empty) | via placeDoor; derived from backendOf |
| `KindsCtx` | `door_vault_at` | placeDoor?.vaultAt (kinds.ts:244) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `door_sign_ins_at` | placeDoor?.signInsAt (kinds.ts:297) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `door_github_from_vault` | placeDoor?.githubFromVault (kinds.ts:314) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `door_exec` | placeDoor.exec (kinds.ts:497) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `door_forward` | placeDoor.forward (kinds.ts:463) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `door_report_daemon_version` | placeDoor.reportOf(place)?.daemonVersion (kinds.ts:490) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `KindsCtx` | `vault` | opts.vault?.() (kinds.ts:244) | opts: RuntimeOptions.vault |  |
| `KindsCtx` | `broker_ready` | opts.broker?.ready() === true | opts: RuntimeOptions.broker -> Broker::ready |  |
| `KindsCtx` | `has_broker_status` | opts.broker?.status !== undefined | opts: RuntimeOptions.broker -> Broker::status |  |
| `KindsCtx` | `broker_status` | opts.broker.status() | opts: RuntimeOptions.broker -> Broker::status | Broker::status answers BrokerStatus, slice answers serde_json::Value |
| `KindsCtx` | `broker_codex_claims` | opts.broker.codexClaims() | opts: RuntimeOptions.broker -> Broker::codex_claims | Broker::codex_claims answers Option<CodexClaims>, slice answers serde_json::Value |
| `KindsCtx` | `is_brokered` | isBrokered(record, provider) (@wsp/protocol free fn) | gap: TS free function isBrokered (@wsp/protocol); no Rust item found in wsp-wire or wsp-words |  |
| `KindsCtx` | `codex_broker_of` | codexBrokerOf(status, claims, models) (machines/brokered.ts free fn) | gap: TS free function codexBrokerOf (brokered.ts); already ported, typed, at wsp_machines::machines::brokered::codex_broker_of |  |
| `KindsCtx` | `unbrokered_refusal` | unbrokeredRefusal(harness) (machines/brokered.ts free fn) | gap: TS free function unbrokeredRefusal (brokered.ts); already ported at wsp_machines::machines::brokered::unbrokered_refusal |  |
| `KindsCtx` | `codex_login_unread_refusal` | codexLoginUnreadRefusal() (@wsp/protocol free fn) | gap: TS free function codexLoginUnreadRefusal (@wsp/protocol); no Rust item found in wsp-wire or wsp-words |  |
| `KindsCtx` | `placeholders_of` | placeholdersOf(status, token) (machines/brokered.ts free fn) | gap: TS free function placeholdersOf (brokered.ts); already ported, typed, at wsp_machines::machines::brokered::placeholders_of |  |
| `KindsCtx` | `brokered_env` | brokeredEnv(token, branchOfRecord, publicMarketplaces(ctx)) (machines/brokered.ts free fn) | gap: TS free function brokeredEnv (brokered.ts); already ported, typed, at wsp_machines::machines::brokered::brokered_env |  |
| `KindsCtx` | `land_on_record` | landOnRecord(ctx, entry, project, lines) (projects/branch-of-record.ts free fn) | gap: TS free function landOnRecord (projects/branch-of-record.ts); wsp-machines src/projects/branch_of_record.rs is empty |  |
| `KindsCtx` | `clone_lines` | cloneLines({...}) (project-landing.ts free fn) | gap: TS free function cloneLines (project-landing.ts); wsp-core project_landing.rs is an empty module |  |
| `KindsCtx` | `daemon_token_of` | ctx.daemonTokenOf (TranscriptsArea) | wsp-core TranscriptsArea (empty) | wsp-core PipeAreaContext::daemon_token_of is the same call |
| `KindsCtx` | `forget_daemon_token` | ctx.daemonTokens.delete(machineId) (kinds.ts:211) | wsp-core TranscriptsArea (empty) | wsp-core PipeAreaContext::forget_daemon_token is the same call |
| `KindsCtx` | `has_deploy` | opts.goldenRecipe?.deployDaemon !== undefined | opts: RuntimeOptions.golden_recipe -> GoldenRecipe.deploy_daemon |  |
| `KindsCtx` | `deploy_daemon` | opts.goldenRecipe.deployDaemon(machine, token) | opts: RuntimeOptions.golden_recipe -> GoldenRecipe.deploy_daemon | DeployDaemon answers Option<String>, slice answers () |
| `KindsCtx` | `local_daemon_road` | local.daemonRoad (RuntimeCore.local) | wsp-core item: wsp_core::types::wiring::LocalWiring::daemon_road |  |
| `KindsCtx` | `machine_exec_opts` | opts.machineExec | opts: RuntimeOptions.machine_exec | wsp-core has the typed MachineExecOptions, slice passes a serde_json::Map |
| `KindsCtx` | `copy_import` | ctx.copyImport (ProjectsArea) | wsp-core ProjectsArea (empty) |  |
| `KindsCtx` | `hello_version` | ctx.helloVersion (ReachArea) | wsp-core ReachArea (empty) |  |
| `KindsCtx` | `has_local` | local !== undefined (RuntimeCore.local) | wsp-core item: wsp_core::types::wiring::LocalWiring | presence of the Option |
| `KindsCtx` | `local_backend` | local.backend | wsp-core item: wsp_core::types::wiring::LocalWiring::backend |  |
| `KindsCtx` | `local_home` | local.home(id) | wsp-core item: wsp_core::types::wiring::LocalWiring::home |  |
| `KindsCtx` | `local_home_dir` | local.homeDir | wsp-core item: wsp_core::types::wiring::LocalWiring::home_dir |  |
| `KindsCtx` | `local_env` | local.env() | wsp-core item: wsp_core::types::wiring::LocalWiring::env |  |
| `KindsCtx` | `local_roots_path` | local.rootsPath | wsp-core item: wsp_core::types::wiring::LocalWiring::roots_path | LocalWiring::roots_path is a String, slice answers Option<String> |
| `KindsCtx` | `has_local_daemon_road` | local.daemonRoad !== undefined | wsp-core item: wsp_core::types::wiring::LocalWiring::daemon_road |  |
| `KindsCtx` | `has_local_restart` | local.restartDaemon !== undefined | wsp-core item: wsp_core::types::wiring::LocalWiring::restart_daemon |  |
| `KindsCtx` | `here_daemon_version` | local.hereDaemon?.version() | wsp-core item: wsp_core::types::wiring::HereDaemon::version | HereDaemon::version answers Result<u32>, slice answers Option<Result<Option<f64>>> |
| `KindsCtx` | `local_scratch` | local.backend.folder (kinds.ts:412) | wsp-core item: wsp_core::types::wiring::LocalWiring::folder |  |
| `KindsCtx` | `agents_wsp_mcp` | opts.agents?.wspMcp | opts: RuntimeOptions.agents -> AgentsReach.wsp_mcp |  |
| `KindsCtx` | `agents_here_url` | opts.agents?.here?.url | opts: RuntimeOptions.agents -> AgentsReach.here_url |  |
| `KindsCtx` | `door_login_user` | placeOf(record).machine.loginOf().user (kinds.ts:445) | wsp-engine item: wsp_engine::place_machine::PlaceFolderMachine::login_of | reached through the machine, not ctx |
| `KindsCtx` | `end_thread` | threadEnds(ctx, placeDoor).end (kinds.ts:31,468) | gap: the thread-ends area object kinds builds itself (machines/thread-ends.ts threadEnds); no context.ts member |  |
| `KindsCtx` | `end_owed` | threadEnds(ctx, placeDoor).owed (kinds.ts:469) | gap: the thread-ends area object kinds builds itself (machines/thread-ends.ts threadEnds); no context.ts member |  |
| `KindsCtx` | `now_ms` | clock.now() | wsp-core item: wsp_core::clock::Clock::now | duplicate of now above; slice answers f64 |
| `KindsCtx` | `sleep` | setTimeout (kinds.ts:260, the broker read race) | wsp-core item: wsp_core::clock::Clock::schedule | TS uses a raw setTimeout, no ctx.sleeps |
| `KindsCtx` | `warn` | none: kinds.ts and brokered.ts hold no console.warn | gap: no TS call site found in kinds.ts or brokered.ts |  |
| `IdleDoor` | `touch` | ctx.idle.touch | wsp-core item: wsp_core::idle::IdlePolicy::touch | IdlePolicy already implements IdleDoor |
| `IdleDoor` | `forget` | ctx.idle.forget | wsp-core item: wsp_core::idle::IdlePolicy::forget | IdlePolicy already implements IdleDoor |
| `MachinesRuntime` | `store` | ctx.store (RuntimeCore) | wsp-core item: wsp_store::store::Store |  |
| `MachinesRuntime` | `clock` | ctx.clock (RuntimeCore) | wsp-core item: wsp_core::clock::Clock |  |
| `MachinesRuntime` | `ids` | randomBytes (node:crypto, machines.ts:73); no ctx member | wsp-core item: wsp_core::ids::Ids::bytes |  |
| `MachinesRuntime` | `backend` | ctx.backend (RuntimeCore; opts.backend) | wsp-engine item: wsp_engine::machine::MachineBackend |  |
| `MachinesRuntime` | `places` | ctx.places (RuntimeCore) | wsp-core item: wsp_core::types::wiring::PlaceBackends |  |
| `MachinesRuntime` | `emit` | bus.emit (RuntimeCore.bus) | wsp-core item: wsp_core::types::events::Bus::emit |  |
| `MachinesRuntime` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `MachinesRuntime` | `owner` | ctx.state.owner | wsp-core item: wsp_core::context::RuntimeState::owner |  |
| `MachinesRuntime` | `kill_confirm` | opts.killConfirm | opts: RuntimeOptions.kill_confirm | KillConfirm is an empty stand-in struct in wsp-core pending.rs |
| `MachinesRuntime` | `on_exec` | opts.goldenRecipe?.onExec | opts: RuntimeOptions.golden_recipe -> GoldenRecipe.on_exec | GoldenRecipe.on_exec is Fn(GoldenExec) returning (), the slice wants Result<(), String> |
| `MachinesRuntime` | `idle` | ctx.idle (MachinesArea) | wsp-core item: wsp_core::idle::IdlePolicy | IdleDoor below is the two-call view of it |
| `MachinesRuntime` | `live` | ctx.live (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `MachinesRuntime` | `sessions` | ctx.sessions (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `MachinesRuntime` | `thread_records` | ctx.threadRecords (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `MachinesRuntime` | `unreached` | ctx.unreached | wsp-core DaemonArea (empty) |  |
| `MachinesRuntime` | `nap_refusals` | ctx.napRefusals | wsp-core ReachArea (empty) |  |
| `MachinesRuntime` | `stop_refusals` | ctx.stopRefusals | wsp-core ReachArea (empty) |  |
| `MachinesRuntime` | `project_held` | ctx.projectsHeld.has(id) (RuntimeCore) | wsp-core RuntimeCore (empty) | not ctx.projectHeld |
| `MachinesRuntime` | `kept_agents_on` | ctx.keptAgents, filtered by workspace | wsp-core ThreadsArea (empty) | helper over the map |
| `MachinesRuntime` | `end_execs` | ctx.execs, loop at machines.ts:522 (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over the set |
| `MachinesRuntime` | `entry_of` | ctx.entryOf | wsp-core BootArea (empty) |  |
| `MachinesRuntime` | `view` | ctx.view | wsp-core RecordsArea (empty) |  |
| `MachinesRuntime` | `persist` | ctx.persist | wsp-core RecordsArea (empty) |  |
| `MachinesRuntime` | `emit_status` | ctx.emitStatus | wsp-core ReachArea (empty) |  |
| `MachinesRuntime` | `reach_of` | ctx.reachOf | wsp-core ReachArea (empty) |  |
| `MachinesRuntime` | `nap_refused_of` | ctx.napRefusedOf | wsp-core ReachArea (empty) |  |
| `MachinesRuntime` | `nap_refused_reason` | ctx.napRefusedReason | wsp-core ReachArea (empty) |  |
| `MachinesRuntime` | `stop_refused_reason` | ctx.stopRefusedReason | wsp-core ReachArea (empty) |  |
| `MachinesRuntime` | `says_waking` | ctx.saysWaking | wsp-core ReachArea (empty) |  |
| `MachinesRuntime` | `ping_daemon` | ctx.pingDaemon | wsp-core ReachArea (empty) |  |
| `MachinesRuntime` | `sync_daemon` | ctx.syncDaemon | wsp-core DaemonArea (empty) |  |
| `MachinesRuntime` | `write_daemon_roots` | ctx.writeDaemonRoots | wsp-core DaemonArea (empty) |  |
| `MachinesRuntime` | `sub_lost` | ctx.subLost | wsp-core SubLifeArea (empty) |  |
| `MachinesRuntime` | `backend_for` | ctx.backendFor | wsp-core KindsArea (empty) |  |
| `MachinesRuntime` | `module_of` | ctx.moduleOf | wsp-core KindsArea (empty) |  |
| `MachinesRuntime` | `keeps_images` | ctx.keepsImages | wsp-core KindsArea (empty) |  |
| `MachinesRuntime` | `names_workspace` | ctx.namesWorkspace | wsp-core KindsArea (empty) |  |
| `MachinesRuntime` | `pause_keeps_disk` | ctx.pauseKeepsDisk | wsp-core KindsArea (empty) |  |
| `MachinesRuntime` | `image_of` | ctx.imageOf | wsp-core RecordsArea (empty) |  |
| `MachinesRuntime` | `recorded_images` | ctx.recordedImages | wsp-core RecordsArea (empty) |  |
| `MachinesRuntime` | `place_id_of` | ctx.placeIdOf | wsp-core ChannelsArea (empty) |  |
| `MachinesRuntime` | `at_provider` | atProvider(placeId, ctx.places) (types/wiring.ts free fn) | wsp-core item: wsp_core::types::wiring::at_provider | free function put on the trait |
| `MachinesRuntime` | `place_name_of` | ctx.placeDoorOf().nameOf(place) | wsp-core KindsArea (empty) | placeDoorOf; nameOf is a PlaceDoor method |
| `MachinesRuntime` | `place_nap_ms` | ctx.settingsAt(place)?.napMs | wsp-core ChannelsArea (empty) |  |
| `MachinesRuntime` | `shape_of` | ctx.shapeOf | wsp-core RecordsArea (empty) |  |
| `MachinesRuntime` | `size_built` | ctx.sizeBuilt | wsp-core RecordsArea (empty) |  |
| `MachinesRuntime` | `read_memory` | ctx.readMemory | wsp-core RecordsArea (empty) |  |
| `MachinesRuntime` | `vault_export` | ctx.vaultExport (RuntimeCore; built at runtime.ts:529 over engine exportPaths) | wsp-core RuntimeCore (empty) | wsp_engine::vault::export_paths exists but not the paths and caches wrapper |
| `MachinesRuntime` | `gone_stop` | gone.stop(at, machine) (ctx.gone, RuntimeCore) | wsp-core RuntimeCore (empty) | GoneWatch and killUntilGone (engine golden.ts) have no Rust item yet |
| `MachinesRuntime` | `forget_workspace_maps` | revivedAt, unreadAt, unreached, polledReach, napRefusals, transcripts, transcriptBytes, pendingEvents, pendingBytes, transcriptIndex, daemonNotes (machines.ts:540-550) | gap: one helper over 11 members of 4 areas (RuntimeCore 5, DaemonArea 3, ReachArea 2, ChannelsArea 1); split per area |  |
| `MachinesRuntime` | `reap_kept` | ctx.reapKept | wsp-core ThreadsArea (empty) |  |
| `MachinesRuntime` | `forget_slate` | ctx.slates.forget(threadId) (SlatesArea.slates) | wsp-core SlatesArea (empty) | wsp-core pending.rs Slates is an empty trait |
| `MachinesRuntime` | `forget_viewed` | ctx.viewedMarks.delete(id) | wsp-core TranscriptsArea (empty) |  |
| `MachinesRuntime` | `cancel_flush` | ctx.cancelFlush | wsp-core TranscriptsArea (empty) |  |
| `MachinesRuntime` | `settle_flushes` | ctx.transcriptQueue (machines.ts:565) then indexFlushes (RuntimeCore) | wsp-core TranscriptsArea (empty) | also RuntimeCore.indexFlushes |
| `MachinesRuntime` | `clear_rows` | rows?.clear(id), unreadIndexes.delete(id) (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `MachinesRuntime` | `drop_sent_images` | ctx.dropSentImages | wsp-core TranscriptsArea (empty) |  |
| `MachinesRuntime` | `forget_kept_writes` | ctx.keptWrites.delete(id) | wsp-core TranscriptsArea (empty) |  |
| `ReachCtx` | `live` | ctx.live.get(id) (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `ReachCtx` | `live_all` | ctx.live.values() (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `ReachCtx` | `session_entries` | ctx.sessions.values() (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `ReachCtx` | `backend_for` | ctx.backendFor | wsp-core KindsArea (empty) |  |
| `ReachCtx` | `pause_keeps_disk` | ctx.pauseKeepsDisk | wsp-core KindsArea (empty) |  |
| `ReachCtx` | `sweep_leftovers` | sweepLeftovers(machine) (engine free fn) | gap: TS free function sweepLeftovers (@wsp/engine); no Rust item found |  |
| `ReachCtx` | `closing` | ctx.state.closing | wsp-core item: wsp_core::context::RuntimeState::closing |  |
| `ReachCtx` | `provider_of` | ctx.providerOf (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `ReachCtx` | `row_reason` | ctx.rowReason (UsageArea) | wsp-core UsageArea (empty) |  |
| `ReachCtx` | `unreached_of` | ctx.unreachedOf (DaemonArea) | wsp-core DaemonArea (empty) |  |
| `ReachCtx` | `kind_has_daemon` | ctx.moduleOf(kind).hasDaemon(entry) | wsp-core KindsArea (empty) | moduleOf; KindModule::has_daemon exists in wsp-core |
| `ReachCtx` | `view` | ctx.view | wsp-core RecordsArea (empty) |  |
| `ReachCtx` | `machine_state_of` | machineStateOf(phase) (status.ts free fn) | wsp-core item: wsp_core::status::machine_state_of | item answers MachineState, slice answers String |
| `ReachCtx` | `idle_at` | ctx.idle.idleAt (MachinesArea.idle) | wsp-core item: wsp_core::idle::IdlePolicy::idle_at |  |
| `ReachCtx` | `daemon_answers_ms` | ctx.lifecycleOf(entry).budgets.daemonAnswersMs | wsp-core KindsArea (empty) | lifecycleOf |
| `ReachCtx` | `daemon_hello_timeout_ms` | ctx.daemonHelloTimeoutMs (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `ReachCtx` | `daemon_token_of` | ctx.daemonTokenOf (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `ReachCtx` | `connect_daemon` | connectDaemon({...}) (reach.ts free fn) | wsp-core item: wsp_core::reach::connect_daemon | item takes ReachOptions and answers Result<Rc<dyn DaemonReach>, ReachError> |
| `ReachCtx` | `project_held` | ctx.projectHeld | wsp-core ChannelsArea (empty) |  |
| `ReachCtx` | `with_daemon` | ctx.withDaemon (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `ReachCtx` | `on_this_computer` | ctx.onThisComputer (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `ReachCtx` | `checkout_of` | ctx.checkoutOf | wsp-core ChannelsArea (empty) |  |
| `ReachCtx` | `served_by_its_computer` | ctx.servedByItsComputer | wsp-core ChannelsArea (empty) | slice answers bool |
| `ReachCtx` | `persist` | ctx.persist | wsp-core RecordsArea (empty) |  |
| `ReachCtx` | `emit` | bus.emit | wsp-core item: wsp_core::types::events::Bus::emit | slice passes a serde_json::Value, Bus::emit takes EventUnion |
| `ReachCtx` | `record` | ctx.record (TranscriptsArea) | wsp-core TranscriptsArea (empty) | slice passes a serde_json::Value |
| `ReachCtx` | `now` | clock.now() | wsp-core item: wsp_core::clock::Clock::now | slice answers f64 |
| `ReachCtx` | `schedule` | clock.schedule(fn, ms, {unref}) | wsp-core item: wsp_core::clock::Clock::schedule | slice takes (ms, f), Clock::schedule takes (f, ms) |
| `ReachCtx` | `sleep` | setTimeout (reach.ts:158) and clock.schedule (reach.ts:243) | wsp-core item: wsp_core::clock::Clock::schedule |  |
| `ReachCtx` | `spawn` | void <promise> (reach.ts:148); no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `ReachCtx` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `BackendFor` | `backend_for` | ctx.backendFor | wsp-core KindsArea (empty) | supertrait of SubLifeCtx, declared here once |
| `SubLifeCtx` | `live` | ctx.live.get(id) | wsp-core RuntimeCore (empty) |  |
| `SubLifeCtx` | `live_all` | ctx.live.values() | wsp-core RuntimeCore (empty) |  |
| `SubLifeCtx` | `thread_records` | ctx.threadRecords entries | wsp-core RuntimeCore (empty) |  |
| `SubLifeCtx` | `session_views` | ctx.sessions.values() views | wsp-core RuntimeCore (empty) |  |
| `SubLifeCtx` | `latest_on` | ctx.latestOn (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `SubLifeCtx` | `thread_runs` | ctx.threadRuns (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `SubLifeCtx` | `launching_on` | ctx.launchingOn (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `SubLifeCtx` | `push_head` | ctx.pushHead (BootArea) | wsp-core BootArea (empty) |  |
| `SubLifeCtx` | `place_id_of` | ctx.placeIdOf | wsp-core ChannelsArea (empty) |  |
| `SubLifeCtx` | `delete_after_ms` | ctx.settingsAt(place)?.deleteAfterMs | wsp-core ChannelsArea (empty) | settingsAt |
| `SubLifeCtx` | `session_file` | ctx.adapterFor(entry, harness).adapter.sessionFile?.(id) | wsp-core AgentsArea (empty) | adapterFor |
| `SubLifeCtx` | `read_transcript` | ctx.transcriptReader.read (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `SubLifeCtx` | `put_blob` | ctx.store.putBlob (SUB_SESSIONS) | wsp-core item: wsp_store::store::Store |  |
| `SubLifeCtx` | `delete_blob` | ctx.store.deleteBlob (SUB_SESSIONS) | wsp-core item: wsp_store::store::Store |  |
| `SubLifeCtx` | `persist` | ctx.persist | wsp-core RecordsArea (empty) |  |
| `SubLifeCtx` | `nap_with` | ctx.napWith (MachinesArea) | wsp-core MachinesArea (empty) |  |
| `SubLifeCtx` | `end_sessions` | ctx.endSessions (MachinesArea) | wsp-core MachinesArea (empty) |  |
| `SubLifeCtx` | `attach_dead` | ctx.attach(record, deadMachine(id)) (MachinesArea.attach) | wsp-core MachinesArea (empty) | dead_machine is wsp_core::types::internal::dead_machine |
| `SubLifeCtx` | `forget_marks` | ctx.napRefusals, ctx.stopRefusals, ctx.unreached delete (sub-life.ts:319-321) | gap: one helper over members of two areas (ReachArea napRefusals and stopRefusals, DaemonArea unreached); split per area |  |
| `SubLifeCtx` | `idle_forget` | ctx.idle.forget | wsp-core item: wsp_core::idle::IdlePolicy::forget |  |
| `SubLifeCtx` | `emit_status` | ctx.emitStatus (ReachArea) | wsp-core ReachArea (empty) |  |
| `SubLifeCtx` | `emit` | bus.emit | wsp-core item: wsp_core::types::events::Bus::emit | slice passes a serde_json::Value |
| `SubLifeCtx` | `unfork` | ctx.unfork (MachinesArea) | wsp-core MachinesArea (empty) |  |
| `SubLifeCtx` | `cut_off` | cutOff(ctx, entry) (pipe.ts free fn) | wsp-core item: wsp_core::pipe::cut_off | item takes (&dyn PipeAreaContext, workspace_id, name), slice takes an entry |
| `SubLifeCtx` | `broker_lift` | ctx.opts.broker?.lift?.(id) (sub-life.ts:441) | opts: RuntimeOptions.broker -> Broker::lift |  |
| `SubLifeCtx` | `delete_workspace` | ctx.workspaces.delete(id) (WorkspacesArea.workspaces) | wsp-core item: wsp_core::types::api::WorkspacesApi::delete |  |
| `SubLifeCtx` | `now` | clock.now() | wsp-core item: wsp_core::clock::Clock::now | slice answers f64 |
| `SubLifeCtx` | `schedule` | clock.schedule(fn, ms, {unref}) | wsp-core item: wsp_core::clock::Clock::schedule | slice takes (ms, f), Clock::schedule takes (f, ms) |
| `SubLifeCtx` | `spawn` | void <promise>; no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `SubLifeCtx` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `SubLifeCtx` | `on_event` | bus.on(type, fn) (sub-life.ts:445-453) | wsp-core item: wsp_core::types::events::EventBus::on | slice takes (&str, Fn(&Value)), EventBus::on takes a typed listener |
| `SubQueueCtx` | `live` | ctx.live.get(id) | wsp-core RuntimeCore (empty) |  |
| `SubQueueCtx` | `live_all` | ctx.live.values() | wsp-core RuntimeCore (empty) |  |
| `SubQueueCtx` | `session` | ctx.sessions.get(turnId) (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `SubQueueCtx` | `set_session` | ctx.sessions.set (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `SubQueueCtx` | `update_session` | ctx.sessions row changed in place (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `SubQueueCtx` | `remove_session` | ctx.sessions.delete (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `SubQueueCtx` | `place_id_of` | ctx.placeIdOf | wsp-core ChannelsArea (empty) |  |
| `SubQueueCtx` | `holds_slot` | ctx.holdsSlot (RulesArea) | wsp-core RulesArea (empty) |  |
| `SubQueueCtx` | `place_name_of` | ctx.placeDoor?.nameOf (sub-queue.ts:72) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `SubQueueCtx` | `machines_at` | ctx.placeDoor?.machinesAt (sub-queue.ts:76) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `SubQueueCtx` | `limits_of` | ctx.backendFor(record).limits (sub-queue.ts:360) | wsp-core KindsArea (empty) | backendFor; MachineBackend::limits exists in wsp-engine |
| `SubQueueCtx` | `view` | ctx.view | wsp-core RecordsArea (empty) | slice answers a serde_json::Value |
| `SubQueueCtx` | `persist` | ctx.persist | wsp-core RecordsArea (empty) |  |
| `SubQueueCtx` | `persist_sessions` | ctx.persistSessions (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `SubQueueCtx` | `emit` | bus.emit | wsp-core item: wsp_core::types::events::Bus::emit | slice passes a serde_json::Value |
| `SubQueueCtx` | `record` | ctx.record (TranscriptsArea) | wsp-core TranscriptsArea (empty) | slice passes a serde_json::Value |
| `SubQueueCtx` | `attach_absent` | ctx.attach(record, absentMachine("", "sandbox", ...)) (MachinesArea.attach) | wsp-core MachinesArea (empty) | absent_machine is wsp_core::types::internal::absent_machine |
| `SubQueueCtx` | `attach_dead` | ctx.attach(record, deadMachine("")) (MachinesArea.attach) | wsp-core MachinesArea (empty) | dead_machine is wsp_core::types::internal::dead_machine |
| `SubQueueCtx` | `record_cell` | none found in sub-queue.ts or its spec | gap: no TS member and no mention in the spec or the oracle |  |
| `SubQueueCtx` | `drop_workspace` | ctx.drop(id) (MachinesArea) | wsp-core MachinesArea (empty) | named drop in TS |
| `SubQueueCtx` | `notify_failed` | ctx.notifyEnd(...) with ctx.tellAs(...) (ThreadsArea, sub-queue.ts:229) | wsp-core ThreadsArea (empty) | notifyEnd; tellAs is also ThreadsArea |
| `SubQueueCtx` | `push_head` | ctx.pushHead (BootArea) | wsp-core BootArea (empty) |  |
| `SubQueueCtx` | `rows_sent` | ctx.rowsSent (BootArea) | wsp-core BootArea (empty) |  |
| `SubQueueCtx` | `put_start` | ctx.store.put(SUB_STARTS, ...) | wsp-core item: wsp_store::store::Store |  |
| `SubQueueCtx` | `delete_start` | ctx.store.delete(SUB_STARTS, ...) | wsp-core item: wsp_store::store::Store |  |
| `SubQueueCtx` | `list_starts` | ctx.store.list(SUB_STARTS) | wsp-core item: wsp_store::store::Store |  |
| `SubQueueCtx` | `stand_up_queued` | ctx.standUpQueued (SubSpawnArea) | wsp-core SubSpawnArea (empty) |  |
| `SubQueueCtx` | `start_turn` | ctx.sessionsApi.start (SessionsArea.sessionsApi) | wsp-core item: wsp_core::types::api::SessionsApi::start | SessionsApi::start takes StartOptions and answers Rc<dyn SessionHandle>; slice takes a Map and answers StartedTurn |
| `SubQueueCtx` | `cut_off` | cutOff(ctx, standing) (pipe.ts free fn, sub-queue.ts:409) | wsp-core item: wsp_core::pipe::cut_off | item takes (&dyn PipeAreaContext, workspace_id, name), slice takes an entry |
| `SubQueueCtx` | `unfork` | ctx.unfork (MachinesArea) | wsp-core MachinesArea (empty) |  |
| `SubQueueCtx` | `now` | clock.now() | wsp-core item: wsp_core::clock::Clock::now | slice answers f64 |
| `SubQueueCtx` | `uuid` | randomUUID (node:crypto, sub-queue.ts:193); no ctx member | wsp-core item: wsp_core::ids::Ids::uuid |  |
| `SubQueueCtx` | `schedule` | clock.schedule | wsp-core item: wsp_core::clock::Clock::schedule | slice takes (ms, f), Clock::schedule takes (f, ms) |
| `SubQueueCtx` | `spawn` | void <promise>; no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `SubQueueCtx` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `SubQueueCtx` | `on_event` | bus.on("*", fn) (sub-queue.ts:478) | wsp-core item: wsp_core::types::events::EventBus::on | slice takes (&str, Fn(&Value)), EventBus::on takes a typed listener |
| `SubSpawnCtx` | `live` | ctx.live.get(id) | wsp-core RuntimeCore (empty) |  |
| `SubSpawnCtx` | `remove_live` | ctx.live.delete(id) (sub-spawn.ts:483) | wsp-core RuntimeCore (empty) |  |
| `SubSpawnCtx` | `is_absent_machine` | isAbsentMachine(entry.machine) (types/internal.ts) | wsp-core item: wsp_core::types::internal::is_absent_machine |  |
| `SubSpawnCtx` | `thread_records` | ctx.threadRecords | wsp-core RuntimeCore (empty) |  |
| `SubSpawnCtx` | `push_head` | ctx.pushHead (BootArea) | wsp-core BootArea (empty) |  |
| `SubSpawnCtx` | `local_machine` | ctx.moduleOf("local").backend(...).get(LOCAL_MACHINE_ID) (sub-spawn.ts:95,355) | wsp-core KindsArea (empty) | moduleOf |
| `SubSpawnCtx` | `provider_backend` | places.backend(id) ?? placeDoor?.backendOf(id) (sub-spawn.ts:77 providerAt) | gap: local helper providerAt (sub-spawn.ts:77) over two members: PlaceBackends::backend (wsp-core item) and RuntimeCore.placeDoor.backendOf |  |
| `SubSpawnCtx` | `computer_rows` | ctx.computerRows (BootArea) | wsp-core BootArea (empty) |  |
| `SubSpawnCtx` | `landing_place` | ctx.landingPlace (BootArea) | wsp-core BootArea (empty) |  |
| `SubSpawnCtx` | `place_settings` | placeDoor?.settingsAt(place) (sub-spawn.ts:87) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method); slice answers a serde_json::Map |
| `SubSpawnCtx` | `delete_after_ms` | ctx.settingsAt(place)?.deleteAfterMs | wsp-core ChannelsArea (empty) | settingsAt |
| `SubSpawnCtx` | `place_name` | ctx.placeName (BuildersArea) | wsp-core BuildersArea (empty) |  |
| `SubSpawnCtx` | `place_id_of` | ctx.placeIdOf | wsp-core ChannelsArea (empty) |  |
| `SubSpawnCtx` | `backend_for` | ctx.backendFor | wsp-core KindsArea (empty) |  |
| `SubSpawnCtx` | `name_given` | ctx.nameGiven (CreateArea) | wsp-core CreateArea (empty) |  |
| `SubSpawnCtx` | `spawn_guard` | ctx.spawnGuard("fork", origin) (RulesArea) | wsp-core RulesArea (empty) |  |
| `SubSpawnCtx` | `project_held` | ctx.projectHeld | wsp-core ChannelsArea (empty) |  |
| `SubSpawnCtx` | `copy_blocked` | ctx.copyBlocked (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `SubSpawnCtx` | `daemon_frame` | ctx.withDaemon(entry, ask => ask(frame)) (ChannelsArea) | wsp-core ChannelsArea (empty) | withDaemon |
| `SubSpawnCtx` | `checkout_of` | ctx.checkoutOf | wsp-core ChannelsArea (empty) |  |
| `SubSpawnCtx` | `brokered` | ctx.brokered({kind, place}) (KindsArea) | wsp-core KindsArea (empty) |  |
| `SubSpawnCtx` | `brokered_start` | brokeredStart(ctx, true) (projects/branch-of-record.ts free fn) | gap: TS free function brokeredStart (projects/branch-of-record.ts); wsp-machines src/projects/branch_of_record.rs is empty |  |
| `SubSpawnCtx` | `repo_of_record` | repoOfRecord(ctx, remote, name, on) (projects/branch-of-record.ts free fn) | gap: TS free function repoOfRecord (projects/branch-of-record.ts) |  |
| `SubSpawnCtx` | `free_branch_name` | freeBranchName(ctx, wanted, heads) (projects/branch-of-record.ts free fn) | gap: TS free function freeBranchName (projects/branch-of-record.ts) |  |
| `SubSpawnCtx` | `copy_of` | ctx.copyOf (RecordsArea) | wsp-core RecordsArea (empty) |  |
| `SubSpawnCtx` | `record_of` | ctx.recordOf (RecordsArea) | wsp-core RecordsArea (empty) |  |
| `SubSpawnCtx` | `place_guard` | ctx.placeGuard (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `SubSpawnCtx` | `default_agent_of` | ctx.defaultAgentOf(prefs, entry) (AgentsArea) | wsp-core AgentsArea (empty) | slice takes only the lead entry and reads preferences itself (PreferencesArea.preferences) |
| `SubSpawnCtx` | `unbrokered_refusal` | unbrokeredRefusal(harness) (machines/brokered.ts free fn) | gap: TS free function unbrokeredRefusal (brokered.ts); already ported at wsp_machines::machines::brokered::unbrokered_refusal |  |
| `SubSpawnCtx` | `name_refusal` | ctx.nameRefusal (CreateArea) | wsp-core CreateArea (empty) |  |
| `SubSpawnCtx` | `tree_of` | ctx.treeOf (RulesArea) | wsp-core RulesArea (empty) |  |
| `SubSpawnCtx` | `forking_add` | ctx.forking.add(name) (CreateArea) | wsp-core CreateArea (empty) |  |
| `SubSpawnCtx` | `forking_remove` | ctx.forking.delete(name) (CreateArea) | wsp-core CreateArea (empty) |  |
| `SubSpawnCtx` | `supersede_failed` | ctx.supersedeFailed (CreateArea) | wsp-core CreateArea (empty) |  |
| `SubSpawnCtx` | `enqueue` | ctx.enqueue (SubQueueArea) | wsp-core SubQueueArea (empty) |  |
| `SubSpawnCtx` | `queue_wait` | ctx.queueWait (SubQueueArea) | wsp-core SubQueueArea (empty) |  |
| `SubSpawnCtx` | `view` | ctx.view | wsp-core RecordsArea (empty) | slice answers a serde_json::Value |
| `SubSpawnCtx` | `stage_reporter` | ctx.stageReporter (CreateArea) | wsp-core CreateArea (empty) |  |
| `SubSpawnCtx` | `fork` | ctx.fork(record, onMachine, undefined, report, false) (MachinesArea) | wsp-core MachinesArea (empty) |  |
| `SubSpawnCtx` | `attach` | ctx.attach | wsp-core MachinesArea (empty) |  |
| `SubSpawnCtx` | `attach_dead` | ctx.attach(record, deadMachine("")) (MachinesArea.attach) | wsp-core MachinesArea (empty) | dead_machine is wsp_core::types::internal::dead_machine |
| `SubSpawnCtx` | `ping_daemon` | ctx.pingDaemon | wsp-core ReachArea (empty) |  |
| `SubSpawnCtx` | `sync_daemon` | ctx.syncDaemon | wsp-core DaemonArea (empty) |  |
| `SubSpawnCtx` | `persist` | ctx.persist | wsp-core RecordsArea (empty) |  |
| `SubSpawnCtx` | `emit_status_reachable` | ctx.emitStatus(entry, "reachable") (ReachArea) | wsp-core ReachArea (empty) | emitStatus |
| `SubSpawnCtx` | `unfork` | ctx.unfork | wsp-core MachinesArea (empty) |  |
| `SubSpawnCtx` | `broker_lift` | ctx.opts.broker?.lift?.(id) (sub-spawn.ts:540) | opts: RuntimeOptions.broker -> Broker::lift |  |
| `SubSpawnCtx` | `rows_sent` | ctx.rowsSent | wsp-core BootArea (empty) |  |
| `SubSpawnCtx` | `session_file` | ctx.adapterFor(entry, kept.agent).adapter.sessionFile?.(id) (sub-spawn.ts:590) | wsp-core AgentsArea (empty) | adapterFor |
| `SubSpawnCtx` | `get_seed_choice` | ctx.store.get(SEED_CHOICES, folder) (sub-spawn.ts:396) | wsp-core item: wsp_store::store::Store |  |
| `SubSpawnCtx` | `get_blob` | ctx.store.getBlob (sub-spawn.ts:591) | wsp-core item: wsp_store::store::Store |  |
| `SubSpawnCtx` | `delete_create` | ctx.store.delete(CREATES, key) (sub-spawn.ts:307) | wsp-core item: wsp_store::store::Store |  |
| `SubSpawnCtx` | `seeds` | ctx.opts.seed !== undefined (sub-spawn.ts:397) | opts: RuntimeOptions.seed |  |
| `SubSpawnCtx` | `homes_here` | ctx.homesHere (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `SubSpawnCtx` | `seed_plan` | ctx.opts.seed.plan (sub-spawn.ts:400) | opts: RuntimeOptions.seed -> SeedWiring::plan |  |
| `SubSpawnCtx` | `seed_pack` | ctx.opts.seed.pack (sub-spawn.ts:406) | opts: RuntimeOptions.seed -> SeedWiring::pack | SeedWiring::pack takes (plan, SeedChoice, homes), slice takes (plan, files, homes) |
| `SubSpawnCtx` | `folder_named` | folderNamed(path) (types/wiring.ts free fn) | wsp-core item: wsp_core::types::wiring::folder_named | item takes (path, home, cwd), slice takes (path) |
| `SubSpawnCtx` | `emit` | bus.emit | wsp-core item: wsp_core::types::events::Bus::emit | slice passes a serde_json::Value |
| `SubSpawnCtx` | `now` | clock.now() | wsp-core item: wsp_core::clock::Clock::now | slice answers f64 |
| `SubSpawnCtx` | `random_hex` | randomBytes(n).toString("hex") (node:crypto, sub-spawn.ts:222) | wsp-core item: wsp_core::ids::Ids::bytes | Ids::bytes |
| `SubSpawnCtx` | `spawn` | void <promise>; no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `SubSpawnCtx` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `ThreadEndsRuntime` | `store_get` | ctx.store.get (thread-ends.ts:34) | wsp-core item: wsp_store::store::Store |  |
| `ThreadEndsRuntime` | `store_put` | ctx.store.put | wsp-core item: wsp_store::store::Store |  |
| `ThreadEndsRuntime` | `store_delete` | ctx.store.delete | wsp-core item: wsp_store::store::Store |  |
| `ThreadEndsRuntime` | `ready` | ctx.ready (BootArea) | wsp-core BootArea (empty) | wsp-core PipeAreaContext::ready is the same call |
| `ThreadEndsRuntime` | `running_on` | ctx.runningOn (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `ThreadEndsRuntime` | `launching_on` | ctx.launchingOn (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `ThreadEndsRuntime` | `stop_holds` | ctx.stopHolds (SessionsArea) | wsp-core SessionsArea (empty) |  |
| `ThreadEndsRuntime` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `ThreadEndsDoor` | `exec` | door.exec (thread-ends.ts:46; door = ctx.placeDoor) | wsp-core RuntimeCore (empty) | PlaceDoor method, via placeDoor |
| `ThreadEndsDoor` | `link` | door.link(place) !== undefined | wsp-core RuntimeCore (empty) | PlaceDoor method, via placeDoor |
| `ThreadEndsDoor` | `name_of` | door.nameOf | wsp-core RuntimeCore (empty) | PlaceDoor method, via placeDoor |
| `LinkRecords` | `records` | placeRecords().records | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `wired_provider` | placeRecords().wiredProvider | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `provider_ids` | placeRecords().providerIds | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `provider_backend` | placeRecords().providerBackend | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `settings_of` | placeRecords().settingsOf | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `settings_held` | placeRecords().settingsHeld | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `awaiting_for` | placeRecords().awaiting (the waits, filtered by place) | gap: places-internal, member of the door's records area (places/records.ts) | helper over the map |
| `LinkRecords` | `hold_back` | placeRecords().holdBack | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `default_id` | placeRecords().defaultId | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `in_turn` | placeRecords().inTurn | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `mark_default` | placeRecords().markDefault | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `mark_held` | placeRecords().markHeld | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `challenge` | placeRecords().challenge | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `signed_refusal` | placeRecords().signedRefusal | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `write_seen` | placeRecords().writeSeen | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `with_cap` | placeRecords().withCap | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkRecords` | `row_ids` | placeRecords().rowIds | gap: places-internal, member of the door's records area (places/records.ts) |  |
| `LinkViews` | `pending_records` | placeViews().pendingRecords (places/views.ts) | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `put_pending` | placeViews().putPending | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `flooring` | placeViews().flooring | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `floor_once` | placeViews().floorOnce | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `taken_back` | placeViews().takenBack | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `backend_from` | placeViews().backendFrom | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `tunnelled` | placeViews().tunnelled | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `cut` | placeViews().cut | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `joining` | placeViews().joining | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `joined` | placeViews().joined | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `LinkViews` | `joined_row` | placeViews().joinedRow | gap: places-internal, member of the door's views area (places/views.ts) |  |
| `ManageSetup` | `waiting_ids` | placeSetup().waiting keys (places/setup.ts) | gap: places-internal, member of the door's setup area (places/setup.ts) | helper over the map |
| `ManageSetup` | `is_setting` | placeSetup().setting.has | gap: places-internal, member of the door's setup area (places/setup.ts) | helper over the set |
| `ManageSetup` | `is_syncing` | placeSetup().syncing.has | gap: places-internal, member of the door's setup area (places/setup.ts) | helper over the set |
| `ManageSetup` | `skipper` | placeSetup().skippers.get | gap: places-internal, member of the door's setup area (places/setup.ts) | helper over the map |
| `ManageSetup` | `setting_now` | placeSetup().settingNow | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `ManageSetup` | `recipes_moved` | placeSetup().recipesMoved | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `ManageSetup` | `sync_frame` | placeSetup().syncFrame | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `ManageSetup` | `setup_frame` | placeSetup().setupFrame | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `ManageSetup` | `start_setup` | placeSetup().startSetup | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `ManageSetup` | `started_or_said` | placeSetup().startedOrSaid | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `ManageSetup` | `clear_sync_timers` | placeSetup().syncTimers (cancel and clear) | gap: places-internal, member of the door's setup area (places/setup.ts) | helper over the map |
| `ManageSetup` | `sync_of` | placeSetup().syncOf | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `ManageSetup` | `mark_sync` | placeSetup().markSync | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `ManageSetup` | `sync_soon` | placeSetup().syncSoon | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `SetupRecords` | `record_of` | placeRecords().recordOf (places/records.ts) | gap: places-internal, member of the door's records area (places/records.ts), reached through PlaceDoorContext (places/context.ts) |  |
| `SetupRecords` | `change` | placeRecords().change (places/records.ts) | gap: places-internal, member of the door's records area (places/records.ts), reached through PlaceDoorContext (places/context.ts) |  |
| `SetupRecords` | `login_listed` | placeRecords().loginListed (places/records.ts) | gap: places-internal, member of the door's records area (places/records.ts), reached through PlaceDoorContext (places/context.ts) |  |
| `PlaceInstaller` | `install` | wiring.install (PlaceWiring, types.ts:168) | opts: RuntimeOptions.place_links -> PlaceWiring.install | wsp-core PlaceWiring is an empty trait; the real one is a struct in wsp-machines places/types.rs |
| `PlaceDialler` | `dial` | wiring.dial (PlaceWiring) | opts: RuntimeOptions.place_links -> PlaceWiring.dial |  |
| `PlaceLogReader` | `log` | wiring.log (PlaceWiring) | opts: RuntimeOptions.place_links -> PlaceWiring.log |  |
| `PlaceUpdater` | `update` | wiring.update (PlaceWiring) | opts: RuntimeOptions.place_links -> PlaceWiring.update |  |
| `PlaceLeaver` | `leave` | wiring.leave (PlaceWiring) | opts: RuntimeOptions.place_links -> PlaceWiring.leave |  |
| `PlaceKeyForgetter` | `forget_key` | wiring.forgetKey (PlaceWiring) | opts: RuntimeOptions.place_links -> PlaceWiring.forget_key |  |
| `PlaceSudo` | `sudo_over` | wiring.sudoOver (PlaceWiring) | opts: RuntimeOptions.place_links -> PlaceWiring.sudo_over |  |
| `PlaceGithubToken` | `github_token` | wiring.githubToken (PlaceWiring) | opts: RuntimeOptions.place_links -> PlaceWiring.github_token |  |
| `PlaceUndoer` | `undo` | wiring.undo (PlaceWiring) | opts: RuntimeOptions.place_links -> PlaceWiring.undo |  |
| `PlaceRunOver` | `run_over` | wiring.runOver (PlaceWiring) | opts: RuntimeOptions.place_links -> PlaceWiring.run_over |  |
| `PlaceBackHolder` | `hold` | wiring.back.hold (PlaceBackHolder) | opts: RuntimeOptions.place_links -> PlaceWiring.back |  |
| `PlaceBackHolder` | `release` | wiring.back.release | opts: RuntimeOptions.place_links -> PlaceWiring.back |  |
| `PlaceBackHolder` | `door` | wiring.back.door | opts: RuntimeOptions.place_links -> PlaceWiring.back |  |
| `PlaceBackHolder` | `close` | wiring.back.close | opts: RuntimeOptions.place_links -> PlaceWiring.back |  |
| `PlaceProvisioner` | `setup` | wiring.provision.setup (PlaceProvisioner) | opts: RuntimeOptions.place_links -> PlaceWiring.provision |  |
| `PlaceProvisioner` | `floor` | wiring.provision.floor | opts: RuntimeOptions.place_links -> PlaceWiring.provision |  |
| `PlaceProvisioner` | `step` | wiring.provision.step | opts: RuntimeOptions.place_links -> PlaceWiring.provision |  |
| `PlaceProvisioner` | `has_undo` | wiring.provision.undo !== undefined | opts: RuntimeOptions.place_links -> PlaceWiring.provision | stands for an optional TS member |
| `PlaceProvisioner` | `undo` | wiring.provision.undo? | opts: RuntimeOptions.place_links -> PlaceWiring.provision |  |
| `PlaceProvisioner` | `has_project_servers` | wiring.provision.projectServers !== undefined | opts: RuntimeOptions.place_links -> PlaceWiring.provision | stands for an optional TS member |
| `PlaceProvisioner` | `project_servers` | wiring.provision.projectServers? | opts: RuntimeOptions.place_links -> PlaceWiring.provision |  |
| `PlaceProvisioner` | `has_estimate` | wiring.provision.estimate !== undefined | opts: RuntimeOptions.place_links -> PlaceWiring.provision | stands for an optional TS member |
| `PlaceProvisioner` | `estimate` | wiring.provision.estimate? | opts: RuntimeOptions.place_links -> PlaceWiring.provision |  |
| `PlaceRecording` | `projects_on` | recording.projectsOn, runtime.ts:649 over ctx.projectsHeld (RuntimeCore) | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member |  |
| `PlaceRecording` | `holds_on` | recording.holdsOn, runtime.ts:629 over standingOn, ctx.workspaces.wake and exec, ctx.checkoutOf, ctx.projectsDoor.unsaved | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member |  |
| `PlaceRecording` | `drop_on` | recording.dropOn, runtime.ts:653 over ctx.workspaces.delete, ctx.projectsDoor.remove | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member |  |
| `PlaceRecording` | `forget_on` | recording.forgetOn, runtime.ts:671 over ctx.endSessions, ctx.drop, forgetProject | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member |  |
| `PlaceRecording` | `running_on` | recording.runningOn, runtime.ts:680 over ctx.live, ctx.providerOf, ctx.capHeld, runningOnPlace | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | name matches ThreadsArea.runningOn but is a different call |
| `PlaceRecording` | `machines_on` | recording.machinesOn, runtime.ts:686 over ctx.live, ctx.placeIdOf | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `forget_records_on` | recording.forgetRecordsOn, runtime.ts:690 over ctx.endSessions, ctx.drop, ctx.releaseSubsOn | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `sub_holds_on` | recording.subHoldsOn, runtime.ts:698 over ctx.live, ctx.subUnpushed, ctx.threadRecords | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `release_records_on` | recording.releaseRecordsOn, runtime.ts:706 over ctx.releaseSubsOn | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `has_images` | recording.imagesOn and dropImageOn both defined | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | stands for two optional TS members |
| `PlaceRecording` | `images_on` | recording.imagesOn, runtime.ts:710 over heldImages(ctx).on | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `drop_image_on` | recording.dropImageOn, runtime.ts:711 over heldImages(ctx).drop | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `spent_today` | recording.spentToday, runtime.ts:712 over ctx.status.spend (StatusArea) | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `sign_in` | recording.signIn, runtime.ts:717 over followSignIn (runtime.ts free fn) | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `mint_here` | recording.mintHere, runtime.ts:719 over followSignIn (runtime.ts free fn) | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `sign_in_line` | recording.signInLine, runtime.ts:713 over ctx.agentsRead.signInLine (PreferencesArea) | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `stores_on` | recording.storesOn, runtime.ts:714 over ctx.placeStores (AgentsArea) | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `login_here` | recording.loginHere, runtime.ts:715 over loginHere(ctx.homesHere, agent) | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `add_folder` | recording.addFolder, runtime.ts:725 over ctx.projectsDoor, ctx.projectsHeld, ctx.preferences, ctx.projectIcons | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `folder_remote` | recording.folderRemote, runtime.ts:754 over ctx.remoteHere (KindsArea) | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member | optional in TS |
| `PlaceRecording` | `remove_folder` | recording.removeFolder, runtime.ts:756 over ctx.projectsDoor.list, ctx.live, forgetProject | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member |  |
| `PlaceRecording` | `folder_look` | recording.folderLook, runtime.ts:763 (folderLook at :583) over ctx.preferences.set, ctx.projectIcons | gap: assembled inline in runtime.ts (PlaceDoorOptions.recording); no context.ts member |  |
| `RecipeResolver` | `resolve` | PlaceDoorOptions.recipes()?.resolve (types.ts:503,514; runtime.ts passes opts.recipes) | opts: RuntimeOptions.recipes -> RecipeShelf::resolve | RecipeShelf::resolve answers wsp-core ResolvedRecipe, slice answers its own Resolved |
| `RecipeResolver` | `resolve_file` | PlaceDoorOptions.recipes()?.resolveFile (runtime.ts passes opts.recipes) | opts: RuntimeOptions.recipes -> RecipeShelf::resolve_file | RecipeShelf::resolve_file answers wsp-core ResolvedRecipe, slice answers its own Resolved |
| `LinkReads` | `backend_of` | ctx.door.backendOf (PlaceDoor link half, views.ts:126,178) | gap: places-internal, member of the door's own link half (PlaceDoor.backendOf), read through ctx.door |  |
| `LinkReads` | `forking_backend` | ctx.door.forkingBackend (PlaceDoor link half) | gap: places-internal, member of the door's own link half (PlaceDoor.forkingBackend), read through ctx.door |  |
| `LinkReads` | `folder_computer` | ctx.door.folderComputer (PlaceDoor link half) | gap: places-internal, member of the door's own link half (PlaceDoor.folderComputer), read through ctx.door |  |
| `ViewsSetup` | `woken` | placeSetup().woken (places/setup.ts) | gap: places-internal, member of the door's setup area (places/setup.ts) |  |
| `ViewsSetup` | `forget_closed_at` | placeSetup().closedAt.delete (places/setup.ts) | gap: places-internal, member of the door's setup area (places/setup.ts) | helper over the map |
| `ViewsSetup` | `link_to` | placeSetup().linkTo (places/setup.ts) | gap: places-internal, member of the door's setup area (places/setup.ts) |  |

## Table B: traits the area implements, seams and callbacks

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `LinkSocket` | `is_open` | socket.readyState === OPEN (ws WebSocket passed to attach, link.ts) | gap: seam over the node ws WebSocket handed in as an argument; not a context member |  |
| `LinkSocket` | `close` | socket.close(code, reason) | gap: seam over the node ws WebSocket handed in as an argument; not a context member |  |
| `LinkSocket` | `on_close` | socket.once("close", fn) | gap: seam over the node ws WebSocket handed in as an argument; not a context member |  |
| `LinkSocket` | `off_close` | socket.off("close", fn) | gap: seam over the node ws WebSocket handed in as an argument; not a context member |  |
| `LinkSocket` | `connect` | connectDaemon({...}) over the socket (link.ts:170) | wsp-core item: wsp_core::reach::connect_daemon | item takes ReachOptions and answers Result<Rc<dyn DaemonReach>, ReachError> |
| `ForwardConn` | `write` | conn.write (net.Socket, helpers.ts forward) | gap: seam over a node net.Socket; not a context member |  |
| `ForwardConn` | `end` | conn.end | gap: seam over a node net.Socket; not a context member |  |
| `ForwardConn` | `destroy` | conn.destroy | gap: seam over a node net.Socket; not a context member |  |
| `PaneForwards` | `list` | PaneForwards.list (pane-ports.ts:21; door.paneForwards) | gap: interface the pane-ports area implements (PanePorts); not a call out of the area, no wsp-core item |  |
| `PaneForwards` | `stop` | PaneForwards.stop (pane-ports.ts:21) | gap: interface the pane-ports area implements (PanePorts); not a call out of the area, no wsp-core item |  |
| `PaneForwards` | `on` | PaneForwards.on (pane-ports.ts:21) | gap: interface the pane-ports area implements (PanePorts); not a call out of the area, no wsp-core item |  |
| `BeforeDeploy` | `before_deploy` | PlaceInstallRequest.beforeDeploy (types.ts:358) | gap: callback the door hands the installer in a request; not a runtime member |  |
| `PlaceDoor` | `answer_challenge` | PlaceDoor.answerChallenge (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `join` | PlaceDoor.join (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `auth` | PlaceDoor.auth (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `host_key` | PlaceDoor.hostKey (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `prove` | PlaceDoor.prove (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `attach` | PlaceDoor.attach (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `link` | PlaceDoor.link (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `channel` | PlaceDoor.channel (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `watch_sys` | PlaceDoor.watchSys (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `load` | PlaceDoor.load (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `backend_of` | PlaceDoor.backendOf (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `folder_computer` | PlaceDoor.folderComputer (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `joined` | PlaceDoor.joined (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `forking_backend` | PlaceDoor.forkingBackend (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `name_of` | PlaceDoor.nameOf (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `settings_at` | PlaceDoor.settingsAt (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `turn_limit_at` | PlaceDoor.turnLimitAt (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `threads_at` | PlaceDoor.threadsAt (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `machines_at` | PlaceDoor.machinesAt (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `sign_ins_at` | PlaceDoor.signInsAt (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `github_from_vault` | PlaceDoor.githubFromVault (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `vault_at` | PlaceDoor.vaultAt (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `login_landed` | PlaceDoor.loginLanded (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `logins_again` | PlaceDoor.loginsAgain (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `key_landed` | PlaceDoor.keyLanded (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `gh_there` | PlaceDoor.ghThere (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `offer_of` | PlaceDoor.offerOf (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `forward` | PlaceDoor.forward (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `pane_forwards` | PlaceDoor.paneForwards (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `place_for` | PlaceDoor.placeFor (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `default_place` | PlaceDoor.defaultPlace (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `mark_used` | PlaceDoor.markUsed (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `mark_default_if_none` | PlaceDoor.markDefaultIfNone (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `add` | PlaceDoor.add (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `set_up` | PlaceDoor.setUp (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `pending` | PlaceDoor.pending (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `choose` | PlaceDoor.choose (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `dial` | PlaceDoor.dial (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `set` | PlaceDoor.set (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `road` | PlaceDoor.road (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `exec` | PlaceDoor.exec (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `report_of` | PlaceDoor.reportOf (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `home_of` | PlaceDoor.homeOf (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `list` | PlaceDoor.list (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `rows` | PlaceDoor.rows (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `adds` | PlaceDoor.adds (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `update` | PlaceDoor.update (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `holds` | PlaceDoor.holds (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `remove` | PlaceDoor.remove (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `find` | PlaceDoor.find (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `follow` | PlaceDoor.follow (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `recipe_changed` | PlaceDoor.recipeChanged (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `followers` | PlaceDoor.followers (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `skip` | PlaceDoor.skip (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `setup_log` | PlaceDoor.setupLog (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `estimate` | PlaceDoor.estimate (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `unfollow` | PlaceDoor.unfollow (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `picks_of` | PlaceDoor.picksOf (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `on` | PlaceDoor.on (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
| `PlaceDoor` | `off` | no TS member (TS on() answers the unsubscribe function) | gap: the slice splits PlaceDoor.on into on and off; no TS member named off |  |
| `PlaceDoor` | `close` | PlaceDoor.close (places/types.ts:531) | wsp-core item: wsp_core::types::pending::PlaceDoor (empty stand-in) |  |
