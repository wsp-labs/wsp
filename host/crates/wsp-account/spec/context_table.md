# Context trait table: wsp-account against wsp-core's runtime context

Read at the join of the account and projects slices (rust/2448-account-join), TS at 65f6b89c7
(`packages/runtime/src/context.ts`, `runtime.ts`, `account/*.ts`, `projects/*.ts`, `usage-reset.ts`). Every area body in
wsp-account is still `todo!()`, so the table is built from the trait declarations, their doc comments and the TS area
files, not from call sites in Rust. The format is `wsp-machines/spec/context_table.md`'s.

Each row is one trait method. "Home" is exactly one of: `wsp-core <Area> (empty)` (the area trait whose TS interface in
context.ts declares the member; the `wsp-core` prefix is kept as in the machines table, and an area this crate fills
itself says so in the Note), `wsp-core item: <path>` (a real item exists; `wsp-engine`, `wsp-wire`, `wsp-words`,
`wsp-store`, `wsp-adapters`, `wsp-catalog` and `wsp-machines` items are named by their path), `opts:
RuntimeOptions.<field>` (TS reads it off the options passed to createRuntime, and wsp-core has the field), or `gap:
<what is missing>`.

Two slices wrote these traits, each on its own. The account slice's eight traits (260 methods) are one per area, with
`ChannelsCtx` shared by the three channels files. The projects slice's traits (331 methods) share one base half, `Base`
(`projects/workspaces/seam.rs`), and each unit adds its own. `WorkspacesCtx` is the four workspaces traits together and
has no method of its own.

The two halves were written by two readers and apply the Home rule at one point differently: where a body or a public
field already exists in wsp-machines (the machines area files), the projects half names the item
(`wsp-core item: wsp_machines::...`, 84 rows, listed in its gap 1) and the account half names the area trait that owns
the member. Read the two as the same home.

## Summary

591 methods in 22 traits.

Methods per trait:

| Trait | File | Methods |
|---|---|---|
| `BoxPanelsCtx` | account/box_panels.rs | 10 |
| `ChannelsCtx` | account/channels.rs | 63 |
| `PreferencesCtx` | account/preferences.rs | 35 |
| `ProcScopesCtx` | account/proc_scopes.rs | 11 |
| `PullRequestsCtx` | account/pull_requests.rs | 24 |
| `RulesCtx` | account/rules.rs | 25 |
| `StatusCtx` | account/status.rs | 40 |
| `UsageCtx` | account/usage.rs | 52 |
| `Base` | projects/workspaces/seam.rs | 25 |
| `FoldersCtx` | projects/folders.rs | 24 |
| `ProjectIconsCtx` | projects/images.rs | 9 |
| `ProjectsCtx` | projects/projects.rs | 35 |
| `LandingRoad` | projects/projects/add.rs | 10 |
| `ProjectSourceModule` | projects/projects/add.rs | 1 |
| `MoveRoads` | projects/projects/moves.rs | 14 |
| `StartFromCtx` | projects/start_from.rs | 20 |
| `CreateCtx` | projects/workspaces/create.rs | 43 |
| `GitCtx` | projects/workspaces/git.rs | 41 |
| `PhasesCtx` | projects/workspaces/phases.rs | 64 |
| `ReviewCtx` | projects/workspaces/review.rs | 30 |
| `UsageResetDeps` | usage_reset.rs | 10 |
| `Resets` | usage_reset.rs | 5 |
| Total | | 591 |

Methods per Home kind:

| Home kind | Account | Projects | Total |
|---|---|---|---|
| wsp-core area trait (empty) | 154 | 120 | 274 |
| wsp-core item (and the items of the other crates) | 67 | 165 | 232 |
| opts | 7 | 9 | 16 |
| gap | 32 | 37 | 69 |
| Total | 260 | 331 | 591 |

The detail of each home kind (which area traits, which crates) is in the two count blocks at the end of the file.

## Gaps worth the coordinator's attention

### From the account traits

Read at the worktree rust/2448-account-join, TS at packages/runtime/src. Rows are in ctx-table-account-rows.md. 260 methods in 8 traits: 154 area (empty), 67 item, 7 opts, 32 gap.

1. TS free functions put on a trait, with no Rust item. `UsageCtx`: `account_on_computer`, `account_of`, `account_rows`, `account_names`, `kept_computer_names`, `computer_name` (all usage.ts), `spend_reset` (usage-reset.ts, ported inside this crate at src/usage_reset.rs), `agent_label` and `model_label` (harness-catalog.ts). Others: `PreferencesCtx::setup_view` (setupView), `PreferencesCtx::access_table` (harnessCatalog). `account_on_computer` is also `BootRuntime::account_key_on` in the machines table: one fn, two traits.
2. `ChannelsCtx::brokered_env`, `terminal_sign_in`, `terminal_line` stand for machines/brokered.ts free functions that are already ported at `wsp_machines::machines::brokered`, in a crate outside the list, and not in the trait's shape: `terminal_sign_in` there takes `(own, read, token, now_ms)` and answers a Result, the slice takes `(harness, entry, token)`. `brokered_env` also needs `publicMarketplaces(ctx)` from projects/branch-of-record.ts (wsp-machines `branch_of_record.rs` is empty).
3. Empty module files that the rows lean on: wsp-core `harness_catalog.rs`, `adapters.rs` (the PLAN_RESETS and LOG_LIMITS tables), `agent_setup.rs` (the setups store and `keyOf`), `github_cache.rs`, `plan_alerts.rs`, and wsp-machines `usage.rs` (ledger, price table, burn, account rows) and `agents_read.rs`. The usage.rs header names wsp-machines' usage.rs and wsp-core's plan alerts as where those live; both are empty today.
4. Six `UsageCtx` rows (`limits`, `file_logs`, `file_limit`, `alerts_read`, `used`, `account_labels`) are not calls out to another area: usage.ts builds the ledger, alerts, burn and price table inside `usageArea` and hands them back as `UsageArea` members. Their Home is the area this crate fills, and `UsageLedger` has no Rust item. `file_limit` and `limits` are near names for two different ledger calls.
5. Node-only calls with no ctx member, 6 rows: `ChannelsCtx::host_pid`, `host_parent_pid`, `read_link`, `hostname`, `git_origin`, `PreferencesCtx::home_dir`. `hostname` is the same gap as `BootRuntime::hostname`. One host-process seam would cover all of them and the machines slice's one.
6. `spawn` (the TS `void promise`) is declared on six more traits: `BoxPanelsCtx`, `ChannelsCtx`, `ProcScopesCtx`, `PullRequestsCtx`, `StatusCtx`, `UsageCtx`. With the four in the machines table that is ten copies of one seam, still with no context.ts member and no wsp-core item.
7. Helpers that span several members, so no area is their home: `StatusCtx::confirm` (`observed`, `shapeOf`, `sizeBuilt` and the backend get), `UsageCtx::read_logs_on` and `sys_history` (`onThisComputer`, `withDaemon`, `overChannel`, `channelOver`, `entryOf`, `localRoad`, `placeDoor.channel`), `UsageCtx::login_home` (the local `planRoad`), `ChannelsCtx::box_panels_ctx` (`boxPanelsOpener` takes the whole RuntimeContext). Two more are filed under ChannelsArea but also read `KindsArea.localRoad`: `PreferencesCtx::local_channel`, `ProcScopesCtx::channel_here`. `PreferencesCtx::setup_write` is one call over `setups.set`, `configFolderOn` and `agentLabel`.
8. Real items in another shape than the slice assumes. `Bus::emit` takes `EventUnion` and answers `Result<u64, HostError>` (slice: `Value`, nothing), in `PreferencesCtx::emit` and `StatusCtx::emit`. `EventBus::on` takes a typed listener (slice: `Fn(&str)` or `Fn()`), in four rows. `Clock::schedule` takes `(f, ms)` (slice: `(ms, f)`) and has no unref. `StatusApi::spend` takes `Option<i64>` and answers a Result. `RuntimeState::preferences_held` is an `Option<serde_json::Value>` (slice: `Preferences`). `PreferencesApi::set` takes `PreferencesPatch` (slice: `Value`). `IdlePolicy::idle_at` answers `Option<i64>` (slice: `Option<f64>`). `lost_workspace` takes `ListedMachine` (slice: `ListedRow`). `is_local_workspace` takes a kind (slice: the entry). `WorkspacesApi::delete` also takes the caller.
9. `wsp_core::types::internal::pid_alive` counts EPERM as alive. The TS in `ChannelsCtx::pid_alive` (`appMain`) treats any throw from `process.kill(pid, 0)`, EPERM included, as not alive. A swap changes which app lock counts as live. `group_exists` and `agent_pid_of` take and answer `i32`, the slice uses `i64`.
10. `moduleOf` is `KindsArea` (empty), but `KindModule` is a real trait in wsp-core, and its methods take no kind (the module is already chosen). Eight rows go through it: `ChannelsCtx::home_dir`, `daemon_road`, `turn_road`, `ProcScopesCtx::shared_daemon`, `RulesCtx::relayed`, `turn_reach`, `wsp_mcp`, `StatusCtx::has_daemon`. `KindModule::turn_reach` answers `Option<Option<String>>` and `wsp_mcp` an `Option<McpServerSpec>`, where the slice answers an `Option` of a map and an `Option<Value>`. `KindModule::daemon_road` has no Result, the slice's has one.
11. Empty stand-ins that read like items: wsp-core `types::pending` holds `GitHubCache`, `Slates`, `ServerIcons` and `AgentsReader` as empty traits. `RuntimeOptions.server_icons` and `agents_reader` are typed with them. The slice declares its own `ServerIcons` (folder, forget) with the same name, so the join will meet two. `PullRequestsCtx::cache_get` and its two siblings are filed as RuntimeCore (empty) for that reason; the member is `opts.githubCache ?? memoryGitHubCache()` (runtime.ts:502).
12. The place door is read 24 times through `RuntimeCore.placeDoor` (22 rows say "via placeDoor", plus the two `has_door`). The real 61-method `PlaceDoor` is in wsp-machines, and wsp-core's is an empty stand-in. Two slice calls differ from the TS door: `box_exec` takes `&str` stdin where the door takes bytes, and `link` answers an identity handle (README A9) where the door answers a `DaemonReach`.
13. Time. `ChannelsCtx::schedule` (node `setTimeout` and `setInterval`), `UsageCtx::schedule` (the timer inside `bounded`) and `StatusCtx::wall_now` (`Date.now()`) do not go through `ctx.clock` in the TS. README A7 routes all three through the one Clock, so a fake clock now drives timers that a TS fake clock never drove. wsp-machines `places/helpers.rs` `bounded` already takes a Clock.
14. The account slice calls itself through the context: 28 rows point at an area trait that wsp-account fills (ChannelsArea 14, UsageArea 8, RulesArea 3, ProcScopesArea 2, PreferencesArea 1). The loop that matters is ChannelsArea and ProcScopesArea: `ChannelsCtx::proc_scopes_moved` and `BoxPanelsCtx::scopes_on` go to proc_scopes, while `ProcScopesCtx::turn_groups`, `thread_terminals` and `channel_here` go back to channels. The `turnGroups` map is built inside `channelsArea`; this crate's `ChannelsArea` holds it, with `daemon_notes` and `port_pids`, as public fields.
15. Near names that are easy to confuse. `project_held` is `projectsHeld.has` (RuntimeCore, a bool) in `PreferencesCtx` and `StatusCtx`, but `ctx.projectHeld` (ChannelsArea, throws on an unheld id) in `RulesCtx` and `PullRequestsCtx`, and `PreferencesCtx::project_of` is the throwing one. `ChannelsCtx::projects_held` is the whole map. `ChannelsCtx::settings_at` is `placeDoor.settingsAt`, not `ChannelsArea.settingsAt`. `RulesCtx::rows_on` is `AgentsArea.rowsOn`, not ThreadsArea. `UsageCtx::agent_label` is usage.ts's own helper over `harnessCatalog`, not `ctx.agentLabel`. `StatusCtx::held` is `RulesArea.held`, `PreferencesCtx::held` is `state.preferencesHeld`. `PreferencesCtx::store` is `Store::put` and `stored` is `Store::get`. `PullRequestsCtx::pauses` is `RulesArea.pauses`. `UsageCtx::name_of` is `placeDoor.nameOf`, `computer_name` is `usageComputerName`.
16. `UsageCtx::plan_agents` needs the PLAN_RESETS table and `log_limit` the LOG_LIMITS table, both in adapters.ts and both holding codex alone. wsp-adapters has the parts (`PlanResets`, `CodexPlanResets`, `codex::limits::limit_of`) but not the lookup by name, so `plan_read_command`, `plan_parse` and `log_limit` are items that cannot be reached until the table exists. `claude::limits::limit_of` exists too and is not in the TS table.
17. `wsp-catalog` is not in the crate list the brief names, but the rows use it: `catalog_agents` for `ChannelsCtx::agent_bins` and `PreferencesCtx::catalog_agents`, and `shared_on` and `login_home_in` under `UsageCtx::login_home`. The slice's `CatalogAgentKeys` keeps three of the entry's fields.
18. `ChannelsCtx::thread_launch` answers a narrower value than the TS member: `ctx.threadLaunch` also takes an `aside` option and answers `wsp` (an McpServerSpec); the slice's `ThreadLaunch` has neither. `StatusCtx::machine_reads` folds four optional machine members into one tuple; `Machine` in wsp-engine gives them default bodies, so Rust cannot tell a machine that has no `metrics` from one that has.

### From the projects traits

# Gaps worth the coordinator's attention: wsp-account projects traits against wsp-core's runtime context

Read at rust/2448-account-join (working tree, 5dba64017 plus uncommitted edits to seam.rs, folders.rs and add.rs), TS at packages/runtime/src. 331 methods in 14 traits; the rows are in `/root/ctx-table-projects-rows.md`.

1. How a Home was chosen where wsp-machines now has a body. The machines table marked these as `wsp-core <Area> (empty)` because its bodies were `todo!()`. Here 84 rows say `wsp-core item: wsp_machines::...` because a concrete `pub fn` or `pub` field exists (BootArea, KindsArea, MachinesArea, ReachArea, DaemonArea, CreateArea, SubLifeArea, SubQueueArea, SubSpawnArea) or, for 3 of them, the real `PlaceDoor` trait. A trait declaration in another context trait did not count. If the coordinator wants the machines-table style, those 84 flip to the area trait named in the TS member column.
2. `WorkspacesCtx` (workspaces.rs:38) is `CreateCtx + PhasesCtx + GitCtx + ReviewCtx` and declares no method of its own, so it has no rows. `ProjectsCtx: Base + MoveRoads` and the other area traits extend `Base`, so supertrait methods are counted once, under the trait that declares them.
3. TS free functions put on a trait, `projectLanding` and `projectSource`: `ProjectsCtx::landing_road` and `project_source`, and every method of `LandingRoad` (10) and `ProjectSourceModule` (1), are members of the objects those functions answer. wsp-core `project_landing.rs` and `project_sources.rs` are empty modules, so all 13 rows are gaps.
4. TS free functions, branch-of-record.ts: `StartFromCtx::brokered_start` (`brokeredStart`) and `pull_request_of_record`. wsp-machines `src/projects/branch_of_record.rs` is empty, and wsp-machines declares its own `brokered_start` on `CreateRuntime` and `SubSpawnCtx`.
5. TS free functions, harness-catalog.ts: `harnessCatalog` (`UsageResetDeps::agent_name`, `StartFromCtx::resolved_picks`), `HARNESS_CATALOGS` (`StartFromCtx::read_only_modes`), `smallestModel` (`GitCtx::drafter`) and `notRunRefused` (`StartFromCtx::start_picks_refusal`). wsp-core `harness_catalog.rs` is empty. `start_picks` itself is real (`wsp_wire::views::harness::start_picks`).
6. TS free functions with no Rust item: `accessWordRefusal` (`StartFromCtx::access_word_refusal`) and `resolveThreadDefaults` (`ProjectsCtx::thread_defaults`, only the `ThreadDefaults` types exist in wsp-wire), `readGone` from @wsp/engine (`PhasesCtx::read_gone`), and the `PLAN_RESETS` table (`UsageResetDeps::resets`; wsp-core `adapters.rs` is empty, only the codex row `CodexPlanResets` exists).
7. Node builtins made into methods, with no ctx member: `Base::home_dir` (`os.homedir()`, folders.ts:113), `MoveRoads::temp_dir` (`os.tmpdir()`, projects.ts:529; its doc line describes actionRefusal instead), `PhasesCtx::path_exists` (`fs.existsSync`) and `ReviewCtx::probe` (global `fetch`; `readBodyUpTo` is `wsp_core::types::internal::read_body_up_to`). They want one host seam, as `Ids` is for randomness.
8. Near-name: `LocalWiring::home_dir` (opts.local.homeDir) is what branch-of-record.ts:104 reads, not the `os.homedir()` that `Base::home_dir` stands for.
9. Helpers that span several members, so no single area is their home (all gaps): `MoveRoads::provider_row` (BootArea.computerRows, ChannelsArea.placeIdOf, `isProviderPlace`), `CreateCtx::shown_view` (ChannelsArea.refOf, RulesArea.treeOf), `GitCtx::exec_stream_of` (AgentsArea, KindsArea, RuntimeCore.execs), `ReviewCtx::shared_channel` (ChannelsArea and ProcScopesArea), `StartFromCtx::resolved_picks`, `GitCtx::drafter`, and `drop_page_cache` on both `GitCtx` and `ReviewCtx` (RuntimeCore.githubCache plus PullRequestsArea.pageKey; the wsp-core `GitHubCache` is an empty stand-in).
10. Helpers over members of one area were kept as that area's row with a note: `ProjectsCtx::agent_lists`, `PhasesCtx::took_the_resume`, `ReviewCtx::copy_channel` and `guest_road_channel`, and the map helpers (`forget_polled_reach`, `forget_failed_create`, `forking_add`, `forking_delete`, `boot_work`, `port_pids`, `forget_daemon_token`).
11. `UsageResetDeps`: `places`, `road`, `run` and `file` are closures account/usage.ts builds inline (usage.ts:208-262) over placeDoor, local, setups, ledger and alerts. They are not context members, so their home is the usage area's own `UsageCtx`, not wsp-core. `limits` and `row` stand on `UsageArea.ledger` and `usageAccounts`, both built in the same file.
12. `Resets` is `wsp_adapters::port::PlanResets` with two shape changes: `spend_command` answers `String` there and `Result<String, String>` here (the TS throws on a bad credit id), and `too_old` answers `&str` there. `UsageResetDeps::resets` hands out `Rc<dyn Resets>`, so the wiring has to adapt the adapter's table.
13. Same function, two items. `ProjectsApi::resolve` is `ctx.projectsDoor.resolve` (projects.ts:475) and `AgentsApi::homes_here` is `ctx.homesHere` (runtime.ts:1056); the rows name the api item for both, though the TS member sits in ProjectsArea and AgentsArea. `resolve_project` is declared on `FoldersCtx`, `CreateCtx` and `GitCtx`.
14. Near-names easy to confuse: `WorkspacesApi::take_setup_line`, `folder_for` and `thread_place` take an origin, the ctx members FoldersArea.takeSetupLine and folderFor and RulesArea.placeOfThread do not; `ProjectsApi::branch` takes a reference, `LandingRoad::branch` a project; wsp-machines `SubQueueArea::queued()` lists held spawns, while `GitCtx::queued(id, work)` is FoldersArea's write queue; `PipeAreaContext::forget_daemon_token` is `ReviewCtx::forget_daemon_token`.
15. `ProjectIconsCtx::project_held` is `ctx.projectsHeld.has(id)` (a bool), while `Base::project_held` is `ctx.projectHeld(id)` (a Result that refuses). `ProjectIconsCtx` also repeats `ready`, `ids` and `preferences` from `Base` because it does not extend it, and `UsageResetDeps` repeats `store` and `uuid`.
16. Two traits named `LandingRoad` for one TS object. wsp-machines (machines/create.rs) has 4 methods (`kind` as a String, `refusal`, `memory_dir`, `workspace_binds`) over a `serde_json::Value` deps; wsp-account (add.rs) has 10 over typed `LandingDeps` and `LandingKind`. The account one lacks `refusal` and `workspaceBinds`, which the TS object has. They should be one trait in wsp-core `project_landing.rs`.
17. Method names declared on more than one trait: 39 names. Among the four traits `WorkspacesCtx` joins there are 14 (`resolve_project`, `refuse_recording`, `project_folder`, `refuse_relayed`, `backend_capabilities`, `name_given`, `name_refusal`, `unfork`, `talks_to_tree_on`, `refusal_for`, `label_of`, `drop_page_cache`, `served_by_its_computer`, `shared_daemon`), so a call on the join type or on `C: WorkspacesCtx` needs the trait named. Two of the 14 also differ in signature and cannot share one body: `refuse_relayed` (`CreateCtx` takes kind, name, project; `GitCtx` takes a record) and `backend_capabilities` (`CreateCtx` takes kind and place; `PhasesCtx` takes the entry). Across areas, `read_pull_request`, `worktree_folder`, `default_agent_of`, `send_detached` and `machine_exec` differ the same way.
18. 90 of the 120 area rows point at areas this crate fills itself (RulesArea 26, ChannelsArea 18, FoldersArea 17, PullRequestsArea 11, StartFromArea 10, ProjectsArea 2, ProjectIconsArea 3, UsageArea 2, PreferencesArea 1), so both sides of those seams are wsp-account's. The other 30 are owned elsewhere: RuntimeCore 8, AgentsArea 6, RecordsArea 5, BuildersArea 4, TranscriptsArea 4, ThreadsArea 3.
19. Shapes against real items, events and sessions: `Bus::emit` takes `EventUnion` and answers `Result<u64>`, `Base::emit` takes a `serde_json::Value` and answers `Result<()>`; `EventBus::on` takes a typed listener and answers an `Unlisten` (`ProjectIconsCtx::on_recipes_changed`); `SessionsApi::start` takes `StartOptions` and answers `Rc<dyn SessionHandle>`, `StartFromCtx::start_session` takes a Value and answers `OpenedSession`.
20. Shapes against real items, wsp-machines `KindsArea`: `state_folder` answers `Result<String>` (`ProjectIconsCtx` wants a `PathBuf`), `remote_here` answers (remote, default branch) where `ProjectsCtx` wants the remote, `git_here` takes `&[&str]` where `GitAsk` is a struct, `project_of` takes `f64` and answers `WorkspaceProject` where `MoveRoads` answers `Option<Value>`, `keeps_images` and `pause_keeps_disk` take a backend where `PhasesCtx` takes the entry.
21. Shapes against real items, other wsp-machines areas: `BootArea::ready` answers a `Ready` handle; `ReachArea` `emit_status` and `reach_of` use `ReachState` where `PhasesCtx` uses `&str` and `String`; `SubLifeArea::sub_lost` answers `GoneOutcome`; `DaemonArea::deploy_daemon_on` takes only the entry; `send_detached` and `to_first_thread` answer `wsp_core::context::DetachedSend` where `GitCtx` has its own `SentTo`; `CreateArea` holds its maps as `RefCell<IndexMap<String, ShownCreate>>` fields (`ShownCreate` is `WorkspaceView`) where `CreateCtx` wants `Rc<RefCell<..>>`; `MachinesArea::attach` takes `Rc<RefCell<WorkspaceRecord>>`.
22. Shapes against real items, wsp-core: `RuntimeState::preferences_held` is `Option<serde_json::Value>` while `StartFromCtx::preferences_held` answers `Option<Preferences>`; `FoldersCtx::sweep_state` groups three `RuntimeState` fields (`sweep_timer`, `sweeping`, `sweep_stopped`) into one `Rc<RefCell<SweepState>>`, but `RuntimeState` is reached only through `StateCell::with`; `pipe::cut_off` takes `(&dyn PipeAreaContext, workspace_id, name)` where `PhasesCtx::cut_off` takes the entry.
23. Two methods take a `Caller` where the TS member takes a `ThreadScope`: `ProjectsCtx::project_of_scope` and `CreateCtx::project_of_scope` (RulesArea.projectOfScope). `Base::spawn_guard` and `PhasesCtx::refuse_cannot` take the act and the capability as `&str` where the TS types them (`SpawnAct`, a key of `Capabilities`).
24. Through the place door: `ProjectsCtx::folder_computer` and `github_from_vault` and `CreateCtx::find_places` read `ctx.placeDoor` (RuntimeCore, optional). Their item is the 61-method `PlaceDoor` trait in wsp-machines `places/types.rs`, while wsp-core `types::pending::PlaceDoor` is still an empty stand-in (machines table items 6 and 7). `folder_computer` answers `FolderComputer` there and `JoinedComputer` here; `find` answers `PlaceRecord` rows and `find_places` wants names.
25. One item sits outside the crates the brief named: `ProjectsCtx::first_agent` stands on `CATALOG_AGENTS` and `DEFAULT_AGENT`, which are `wsp_catalog::catalog::catalog_agents` and `default_agent`, over `opts.adapters`.
26. Stale doc lines in the traits: `MoveRoads::temp_dir` (see 7), and `StartFromCtx::landing_place` says it also reports whether a backend could be read, but answers only `Option<String>`.

## Table A: context traits

| Trait | Method | TS member | Home | Note |
|---|---|---|---|---|
| `BoxPanelsCtx` | `on_workspace_deleted` | bus.on("workspace.deleted", fn) (box-panels.ts:63) | wsp-core item: wsp_core::types::events::EventBus::on | slice takes Fn(&str) of the id, EventBus::on takes a typed listener |
| `BoxPanelsCtx` | `session_rows` | ctx.sessions.values() (box-panels.ts:70) | wsp-core RuntimeCore (empty) | helper over the map; slice keeps three fields as StartedRow |
| `BoxPanelsCtx` | `thread_records` | ctx.threadRecords (box-panels.ts:74) | wsp-core RuntimeCore (empty) | helper over the map (thread id, workspace id) |
| `BoxPanelsCtx` | `link` | placeDoor.link(placeId) (box-panels.ts:83) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method); slice answers an identity handle Option<Rc<()>> (README A9) |
| `BoxPanelsCtx` | `name_of` | placeDoor.nameOf(placeId) (box-panels.ts:109) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `BoxPanelsCtx` | `daemon_version` | placeDoor.reportOf(placeId)?.daemonVersion (box-panels.ts:109) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `BoxPanelsCtx` | `watch_sys` | placeDoor.watchSys(placeId, fn) (box-panels.ts:207) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method); the release is a Promise of a function in TS |
| `BoxPanelsCtx` | `scopes_on` | ctx.scopesOn (ProcScopesArea) | wsp-core ProcScopesArea (empty) | filled by this crate's proc_scopes.rs |
| `BoxPanelsCtx` | `thread_env` | ctx.threadEnv (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `BoxPanelsCtx` | `spawn` | void <promise> (box-panels.ts:173-194); no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `ChannelsCtx` | `index_start` | ctx.transcriptIndex.get(ws)?.starts.get(thread) (channels.ts:33) | wsp-core RuntimeCore (empty) | helper over the map |
| `ChannelsCtx` | `index_cut` | ctx.transcriptIndex.get(ws)?.cut.get(thread) (channels.ts:37) | wsp-core RuntimeCore (empty) | helper over the map |
| `ChannelsCtx` | `index_fact` | ctx.transcriptIndex.get(ws)?.facts.get(resume)?.[fact] (channels.ts:50) | wsp-core RuntimeCore (empty) | helper over the map |
| `ChannelsCtx` | `session_rows` | ctx.sessions.values() (channels.ts:45,364) | wsp-core RuntimeCore (empty) | helper over the map; slice's ChannelRow keeps the fields SESSION_FACTS names |
| `ChannelsCtx` | `thread_record` | ctx.threadRecords.get(id) (channels.ts:62) | wsp-core RuntimeCore (empty) | helper over the map; the answer type ThreadRecord is a real wsp-core item |
| `ChannelsCtx` | `thread_ids` | ctx.threadRecords keys (channels.ts:183) | wsp-core RuntimeCore (empty) | helper over the map |
| `ChannelsCtx` | `latest_harness` | ctx.latestOn(thread)?.harness (ThreadsArea, channels.ts:686) | wsp-core ThreadsArea (empty) |  |
| `ChannelsCtx` | `home_dir` | ctx.moduleOf(kind).homeDir(record) (channels.ts:71) | wsp-core KindsArea (empty) | moduleOf; KindModule::home_dir exists in wsp-core |
| `ChannelsCtx` | `daemon_road` | ctx.moduleOf(kind).daemonRoad(entry) (channels.ts:140) | wsp-core KindsArea (empty) | moduleOf; KindModule::daemon_road exists in wsp-core and answers DaemonReachView without a Result |
| `ChannelsCtx` | `turn_road` | ctx.moduleOf(kind).turnRoad (channels.ts:200) | wsp-core KindsArea (empty) | moduleOf; KindModule::turn_road() takes no kind and answers ScopedRoad, slice takes the kind and answers an Option |
| `ChannelsCtx` | `has_door` | ctx.placeDoor !== undefined (channels.ts:235,256,273) | wsp-core RuntimeCore (empty) |  |
| `ChannelsCtx` | `offer_of` | placeDoor?.offerOf(placeId) (channels.ts:81) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `ChannelsCtx` | `wired` | places.wired (channels.ts:81) | wsp-core item: wsp_core::types::wiring::PlaceBackends::wired |  |
| `ChannelsCtx` | `settings_at` | placeDoor?.settingsAt(placeId) (channels.ts:86) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method); not ChannelsArea.settingsAt, which wraps it for an optional id |
| `ChannelsCtx` | `turn_limit_at` | placeDoor?.turnLimitAt(placeId) (channels.ts:91) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method); TS answers number or null or undefined, slice Option<Option<i64>> |
| `ChannelsCtx` | `name_of` | placeDoor?.nameOf(place) (channels.ts:125) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method); the TS falls back to the id where no door |
| `ChannelsCtx` | `joined` | placeDoor?.joined(place) (channels.ts:177) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `ChannelsCtx` | `report_of` | placeDoor.reportOf(placeId) (channels.ts:236,274,597) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `ChannelsCtx` | `rows` | placeDoor.rows() (channels.ts:257) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `ChannelsCtx` | `link_channel` | placeDoor.channel(placeId, onEvent) (channels.ts:238,606) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method); named `channel` in TS |
| `ChannelsCtx` | `spend_today` | ctx.status.spend(rows, now) (channels.ts:261) | wsp-core item: wsp_core::status::StatusApi::spend | StatusApi::spend takes (places, Option<i64>) and answers Result<Vec<PlaceSpend>, String>, slice answers (id, Option<f64>) pairs |
| `ChannelsCtx` | `projects_held` | ctx.projectsHeld (channels.ts:103) | wsp-core RuntimeCore (empty) | the whole map, as a copy; not ctx.projectHeld |
| `ChannelsCtx` | `open_channel` | ctx.openChannel (KindsArea, channels.ts:148) | wsp-core item: wsp_core::daemon_channel::open_daemon_channel | KindsArea.openChannel is opts.daemonChannel ?? openDaemonChannel (runtime.ts:515); the override is RuntimeOptions.daemon_channel |
| `ChannelsCtx` | `local_has_daemon` | local?.daemonRoad !== undefined (RuntimeCore.local, channels.ts:248) | wsp-core item: wsp_core::types::wiring::LocalWiring::daemon_road | presence of the Option |
| `ChannelsCtx` | `local_home_dir` | local.homeDir (channels.ts:249) | wsp-core item: wsp_core::types::wiring::LocalWiring::home_dir |  |
| `ChannelsCtx` | `local_road` | ctx.localRoad() (KindsArea, channels.ts:250) | wsp-core KindsArea (empty) |  |
| `ChannelsCtx` | `adopt_pause` | ctx.adoptPause (MachinesArea, channels.ts:142) | wsp-core MachinesArea (empty) |  |
| `ChannelsCtx` | `broker_ready` | ctx.opts.broker?.ready() === true (channels.ts:173) | opts: RuntimeOptions.broker -> Broker::ready |  |
| `ChannelsCtx` | `root_of` | ctx.rootOf (RulesArea, channels.ts:199) | wsp-core RulesArea (empty) | filled by this crate's rules.rs |
| `ChannelsCtx` | `device_door` | ctx.deviceDoor (RuntimeCore, channels.ts:200,204,746) | wsp-core item: wsp_core::devices::DeviceDoor | used for mint and revoke |
| `ChannelsCtx` | `host_pid` | process.pid (channels.ts:369); no ctx member | gap: node process.pid; no context.ts member and no wsp-core item |  |
| `ChannelsCtx` | `host_parent_pid` | process.ppid (channels.ts:369); no ctx member | gap: node process.ppid; no context.ts member and no wsp-core item |  |
| `ChannelsCtx` | `has_state_path` | ctx.opts.statePath !== undefined (channels.ts:341) | opts: RuntimeOptions.state_path |  |
| `ChannelsCtx` | `state_folder` | ctx.stateFolder() (KindsArea, channels.ts:343) | wsp-core KindsArea (empty) |  |
| `ChannelsCtx` | `read_link` | readlinkSync(path) (node:fs, channels.ts:343); no ctx member | gap: node:fs readlinkSync; no context.ts member and no wsp-core item |  |
| `ChannelsCtx` | `hostname` | hostname() (node:os, channels.ts:344); no ctx member | gap: node:os hostname(); no context.ts member and no wsp-core item | same gap as BootRuntime::hostname |
| `ChannelsCtx` | `pid_alive` | process.kill(pid, 0) (channels.ts:346); no ctx member | wsp-core item: wsp_core::types::internal::pid_alive | pid_alive counts EPERM as alive, the TS catch here counts it as not alive; takes i32, slice i64 |
| `ChannelsCtx` | `agent_bins` | CATALOG_AGENTS.map(a => a.bin) (@wsp/catalog, channels.ts:362) | wsp-catalog item: wsp_catalog::catalog::catalog_agents | AgentEntry::bin; wsp-catalog is outside the crate list the header names |
| `ChannelsCtx` | `agent_pid_of` | agentPidOf(run) (local-exec.ts free fn, channels.ts:366) | wsp-core item: wsp_core::local_exec::agent_pid_of | answers Option<i32>, slice Option<i64> |
| `ChannelsCtx` | `group_exists` | groupExists(pid) (local-exec.ts free fn, channels.ts:511) | wsp-core item: wsp_core::local_exec::group_exists | takes i32, slice i64 |
| `ChannelsCtx` | `proc_scopes_moved` | ctx.procScopesMoved (ProcScopesArea, channels.ts:524) | wsp-core ProcScopesArea (empty) | filled by this crate's proc_scopes.rs |
| `ChannelsCtx` | `roots_recheck` | ctx.state.rootsRecheck (channels.ts:496) | wsp-core item: wsp_core::context::RuntimeState::roots_recheck | state sits behind StateCell::with; slice hands out Rc<RefCell<Option<Cancel>>> |
| `ChannelsCtx` | `box_panels_ctx` | boxPanelsOpener(ctx, ...) (box-panels.ts free fn, channels.ts:518) | gap: no member; TS hands the whole RuntimeContext to boxPanelsOpener |  |
| `ChannelsCtx` | `store_get` | ctx.store.get (channels.ts:385,722) | wsp-core item: wsp_store::store::Store::get |  |
| `ChannelsCtx` | `store_put` | ctx.store.put (channels.ts:417,736) | wsp-core item: wsp_store::store::Store::put |  |
| `ChannelsCtx` | `store_delete` | ctx.store.delete (channels.ts:416,735) | wsp-core item: wsp_store::store::Store::delete |  |
| `ChannelsCtx` | `store_keys` | ctx.store.keys (channels.ts:775) | wsp-core item: wsp_store::store::Store::keys |  |
| `ChannelsCtx` | `on_workspace_deleted` | ctx.bus.on("workspace.deleted", fn) (channels.ts:429,778) | wsp-core item: wsp_core::types::events::EventBus::on | slice takes Fn(&str) of the id, EventBus::on takes a typed listener |
| `ChannelsCtx` | `live_has` | ctx.live.has(id) (channels.ts:775,822) | wsp-core RuntimeCore (empty) | helper over the map |
| `ChannelsCtx` | `thread_launch` | ctx.threadLaunch (RuntimeCore, channels.ts:675) | wsp-core RuntimeCore (empty) | TS also takes an aside option and answers wsp (an McpServerSpec); slice's ThreadLaunch drops both |
| `ChannelsCtx` | `brokered_env` | brokeredEnv(token, branchOfRecord, publicMarketplaces(ctx)) (machines/brokered.ts free fn, channels.ts:678) | gap: TS free function brokeredEnv, and publicMarketplaces(ctx) (projects/branch-of-record.ts); brokeredEnv is ported at wsp_machines::machines::brokered::brokered_env, outside the crate list | ported fn takes (token, repo, direct), slice takes (token, entry) |
| `ChannelsCtx` | `terminal_sign_in` | terminalSignIn(harness, launchToken => ctx.brokeredLaunch(...), token) (machines/brokered.ts free fn, channels.ts:688) | gap: TS free function terminalSignIn over ctx.brokeredLaunch (KindsArea); ported at wsp_machines::machines::brokered::terminal_sign_in, outside the crate list | ported fn takes (own, read, token, now_ms) and answers a Result |
| `ChannelsCtx` | `terminal_line` | terminalLine(harness, program) (machines/brokered.ts free fn, channels.ts:690) | gap: TS free function terminalLine; ported at wsp_machines::machines::brokered::terminal_line, outside the crate list | ported fn takes (own, Option<program>) |
| `ChannelsCtx` | `thread_folder` | ctx.threadFolder (KindsArea, channels.ts:812) | wsp-core KindsArea (empty) |  |
| `ChannelsCtx` | `thread_env` | ctx.threadEnv (AgentsArea, channels.ts:815) | wsp-core AgentsArea (empty) |  |
| `ChannelsCtx` | `persist` | ctx.persist (RecordsArea, channels.ts:888) | wsp-core RecordsArea (empty) |  |
| `ChannelsCtx` | `read_checkout_soon` | void ctx.readCheckout(entry, true) (ReachArea, channels.ts:877,889) | wsp-core ReachArea (empty) | TS ignores the promise; the method answers nothing |
| `ChannelsCtx` | `delete_workspace` | ctx.workspaces.delete(id) (WorkspacesArea, channels.ts:893) | wsp-core item: wsp_core::types::api::WorkspacesApi::delete | WorkspacesApi::delete also takes the caller |
| `ChannelsCtx` | `git_origin` | spawnSync("git", ["-C", path, "remote", "get-url", "origin"]) (node:child_process, channels.ts:914); no ctx member | gap: node child_process spawnSync; no context.ts member and no wsp-core item |  |
| `ChannelsCtx` | `now` | clock.now() (channels.ts:261) | wsp-core item: wsp_core::clock::Clock::now |  |
| `ChannelsCtx` | `schedule` | setTimeout, setInterval (channels.ts:194,497,711); no ctx.clock call | wsp-core item: wsp_core::clock::Clock::schedule | TS uses node timers here, not ctx.clock (README A7 routes them through the clock); Clock::schedule takes (f, ms), slice takes (ms, f) |
| `ChannelsCtx` | `spawn` | void <promise> (channels.ts:196,390,541,799,877,889); no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `ChannelsCtx` | `warn` | console.warn (channels.ts:204,419,476,707,738,893) | wsp-core item: wsp_core::console::warn |  |
| `PreferencesCtx` | `env` | opts.env ?? process.env (preferences.ts:20) | opts: RuntimeOptions.env | falls back to process.env; RuntimeOptions.env is an IndexMap of Option values, slice answers plain strings |
| `PreferencesCtx` | `server_icons` | opts.serverIcons | opts: RuntimeOptions.server_icons | wsp-core's ServerIcons (types/pending.rs) is an empty stand-in; the slice declares its own ServerIcons with folder and forget |
| `PreferencesCtx` | `home_dir` | homedir() (node:os); no ctx member | gap: node:os homedir(); no context.ts member and no wsp-core item |  |
| `PreferencesCtx` | `adapter_ids` | Object.keys(adapters) | opts: RuntimeOptions.adapters | ctx.adapters (RuntimeCore) is opts.adapters unchanged (runtime.ts:492) |
| `PreferencesCtx` | `access_table` | harnessCatalog(id) ?? { label: ctx.agentLabel(id), permissionModes: [] } | gap: TS free function harnessCatalog (harness-catalog.ts); wsp-core harness_catalog.rs is empty | falls back to AgentsArea.agentLabel; answer carries the table as a serde_json::Value |
| `PreferencesCtx` | `project_held` | projectsHeld.has(projectId) | wsp-core RuntimeCore (empty) | helper over the map; not ctx.projectHeld |
| `PreferencesCtx` | `project_of` | ctx.projectHeld(entry.record.project) (ChannelsArea) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs; slice answers (id, name) or the refusal |
| `PreferencesCtx` | `default_agent_of` | ctx.defaultAgentOf(prefs, undefined) (AgentsArea) | wsp-core AgentsArea (empty) | slice passes no entry |
| `PreferencesCtx` | `session_thread_keys` | threadKeyOf(row.view) over ctx.sessions.values() | wsp-core RuntimeCore (empty) | helper over the map; threadKeyOf is wsp_wire::views::session::thread_key_of |
| `PreferencesCtx` | `thread_record_held` | ctx.threadRecords.has(lead) | wsp-core RuntimeCore (empty) | helper over the map |
| `PreferencesCtx` | `ready` | ctx.ready (BootArea) | wsp-core BootArea (empty) | wsp-core PipeAreaContext::ready is the same call |
| `PreferencesCtx` | `stored` | store.get(PREFERENCES, PREFERENCES_ID) | wsp-core item: wsp_store::store::Store::get |  |
| `PreferencesCtx` | `store` | store.put(PREFERENCES, PREFERENCES_ID, next) | wsp-core item: wsp_store::store::Store::put | the method is named `store`, the call is Store::put |
| `PreferencesCtx` | `held` | ctx.state.preferencesHeld | wsp-core item: wsp_core::context::RuntimeState::preferences_held | RuntimeState holds an Option<serde_json::Value>, slice reads Preferences |
| `PreferencesCtx` | `set_held` | ctx.state.preferencesHeld = next | wsp-core item: wsp_core::context::RuntimeState::preferences_held | same field as `held` |
| `PreferencesCtx` | `emit` | bus.emit({ type: "preferences.changed" ... }) | wsp-core item: wsp_core::types::events::Bus::emit | slice passes a serde_json::Value, Bus::emit takes EventUnion and answers Result<u64, HostError> |
| `PreferencesCtx` | `on_workspace_deleted` | bus.on("workspace.deleted", fn) | wsp-core item: wsp_core::types::events::EventBus::on | slice takes Fn(&str) of the id, EventBus::on takes a typed listener |
| `PreferencesCtx` | `entry_of` | ctx.entryOf (BootArea) | wsp-core BootArea (empty) |  |
| `PreferencesCtx` | `catalog_agents` | CATALOG_AGENTS (@wsp/catalog) | wsp-catalog item: wsp_catalog::catalog::catalog_agents | slice keeps three fields as CatalogAgentKeys; wsp-catalog is outside the crate list the header names |
| `PreferencesCtx` | `thread_env` | ctx.threadEnv(entry, id) (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `PreferencesCtx` | `checkout_of` | ctx.checkoutOf (ChannelsArea) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs |
| `PreferencesCtx` | `is_local` | isLocalWorkspace(entry.record) (@wsp/protocol free fn) | wsp-words item: wsp_words::workspace_state::is_local_workspace | takes Option<&WorkspaceKind>, slice takes the entry |
| `PreferencesCtx` | `at_provider` | atProvider(ctx.placeIdOf(record), ctx.places) | wsp-core item: wsp_core::types::wiring::at_provider | free fn takes (Option<&str>, &dyn PlaceBackends); slice also folds in ChannelsArea.placeIdOf |
| `PreferencesCtx` | `pause_mode` | ctx.view(entry.record).pauseMode (RecordsArea) | wsp-core RecordsArea (empty) | a field read off the view |
| `PreferencesCtx` | `projects_on` | [...projectsHeld.values()].filter(p => p.computer === placeId) | wsp-core RuntimeCore (empty) | helper over the map; the TS awaits ctx.ready() first |
| `PreferencesCtx` | `workspace_channel` | ctx.workspaces.daemonChannel(id, onEvent, origin) (WorkspacesArea) | wsp-core item: wsp_core::types::api::WorkspacesApi::daemon_channel |  |
| `PreferencesCtx` | `local_channel` | ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, onEvent) | wsp-core ChannelsArea (empty) | helper over ChannelsArea.channelOver and KindsArea.localRoad; filled by this crate's channels.rs |
| `PreferencesCtx` | `place_channel` | placeDoor?.channel(placeId, onEvent) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `PreferencesCtx` | `place_name` | placeDoor?.nameOf(placeId) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `PreferencesCtx` | `setup_write` | setups.set(placeId, name, change, opts) (RuntimeCore.setups) | wsp-core RuntimeCore (empty) | one call over setups.set, ctx.configFolderOn and ctx.agentLabel (both AgentsArea); wsp-core agent_setup.rs is empty |
| `PreferencesCtx` | `setup_view` | setupView(setups.get(placeId, name)) (@wsp/protocol free fn) | gap: TS free function setupView, no Rust fn (wsp_wire::types::thread_defaults::AgentSetupView is the type only); its input setups.get is RuntimeCore.setups, wsp-core agent_setup.rs is empty | answers a serde_json::Value |
| `PreferencesCtx` | `drop_setup_refusal` | ctx.setupRefusals.delete(keyOf(placeId, name)) (AgentsArea) | wsp-core AgentsArea (empty) | keyOf is a free fn of the setups file, not ported (wsp-core agent_setup.rs is empty) |
| `PreferencesCtx` | `setup_refusal_moved` | ctx.setupRefusalMoved (BootArea) | wsp-core BootArea (empty) |  |
| `PreferencesCtx` | `relayed` | backend.capabilities.callbackRelay (RuntimeCore.backend) | wsp-engine item: wsp_engine::machine::MachineBackend::capabilities | the field is Capabilities::callback_relay in wsp-wire |
| `PreferencesCtx` | `forget_read` | agentsRead.forget(id) (PreferencesArea.agentsRead) | wsp-core PreferencesArea (empty) | agentsRead is built in preferences.ts from agentsReads (agents-read.ts); wsp-machines agents_read.rs is empty |
| `ProcScopesCtx` | `turn_groups` | ctx.turnGroups (ChannelsArea) | wsp-core ChannelsArea (empty) | the map is built inside channelsArea (channels.ts:491); this crate's ChannelsArea struct holds it as `turn_groups` |
| `ProcScopesCtx` | `group_exists` | groupExists(pid) (local-exec.ts free fn) | wsp-core item: wsp_core::local_exec::group_exists | takes i32, slice i64 |
| `ProcScopesCtx` | `sessions` | ctx.sessions.values() | wsp-core RuntimeCore (empty) | helper over the map; slice keeps the fields as ScopeRow |
| `ProcScopesCtx` | `kept_agents` | ctx.keptAgents (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `ProcScopesCtx` | `latest_on` | ctx.latestOn (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `ProcScopesCtx` | `thread_records` | ctx.threadRecords | wsp-core RuntimeCore (empty) | helper over the map (thread, workspace) |
| `ProcScopesCtx` | `thread_terminals` | ctx.threadTerminals (ChannelsArea) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs |
| `ProcScopesCtx` | `live_record` | ctx.live.get(id)?.record | wsp-core RuntimeCore (empty) | helper over the map |
| `ProcScopesCtx` | `shared_daemon` | ctx.moduleOf(kind).sharedDaemon | wsp-core KindsArea (empty) | moduleOf; KindModule::shared_daemon() exists in wsp-core and takes no kind |
| `ProcScopesCtx` | `channel_here` | ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, () => {}) | wsp-core ChannelsArea (empty) | helper over ChannelsArea.channelOver and KindsArea.localRoad; filled by this crate's channels.rs |
| `ProcScopesCtx` | `spawn` | void <promise> (proc-scopes.ts:41,53,66); no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `PullRequestsCtx` | `now` | clock.now() | wsp-core item: wsp_core::clock::Clock::now |  |
| `PullRequestsCtx` | `schedule` | clock.schedule(fn, ms, { unref }) (pull-requests.ts:68,233) | wsp-core item: wsp_core::clock::Clock::schedule | slice takes (ms, f), Clock::schedule takes (f, ms); Clock has no unref option |
| `PullRequestsCtx` | `cache_get` | githubCache.get(key) (RuntimeCore.githubCache) | wsp-core RuntimeCore (empty) | githubCache is opts.githubCache ?? memoryGitHubCache() (runtime.ts:502); wsp-core's GitHubCache (types/pending.rs) is an empty stand-in and github_cache.rs is empty |
| `PullRequestsCtx` | `cache_set` | githubCache.set(key, entry) | wsp-core RuntimeCore (empty) | same member as `cache_get` |
| `PullRequestsCtx` | `cache_delete` | githubCache.delete(key) | wsp-core RuntimeCore (empty) | same member as `cache_get` |
| `PullRequestsCtx` | `live` | live.get(id) (pull-requests.ts:228) | wsp-core RuntimeCore (empty) | helper over the map; the TS compares entry identity |
| `PullRequestsCtx` | `session_views` | sessions.values() views (pull-requests.ts:258) | wsp-core RuntimeCore (empty) | helper over the map |
| `PullRequestsCtx` | `on_this_computer` | ctx.onThisComputer (ChannelsArea) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs |
| `PullRequestsCtx` | `with_daemon` | ctx.withDaemon (ChannelsArea) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs |
| `PullRequestsCtx` | `checkout_of` | ctx.checkoutOf (ChannelsArea) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs |
| `PullRequestsCtx` | `entry_of` | ctx.entryOf (BootArea) | wsp-core BootArea (empty) |  |
| `PullRequestsCtx` | `project_held` | ctx.projectHeld (ChannelsArea) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs; throws for an unheld id |
| `PullRequestsCtx` | `status_watched` | ctx.status.watched() (StatusArea) | wsp-core item: wsp_core::status::StatusApi::watched |  |
| `PullRequestsCtx` | `persist` | ctx.persist (RecordsArea) | wsp-core RecordsArea (empty) |  |
| `PullRequestsCtx` | `status_now` | ctx.statusNow (ReachArea) | wsp-core ReachArea (empty) |  |
| `PullRequestsCtx` | `read_checkout` | ctx.readCheckout (ReachArea) | wsp-core ReachArea (empty) |  |
| `PullRequestsCtx` | `slates_watch_pr` | ctx.slates.watchesPr(id) (SlatesArea.slates) | wsp-core SlatesArea (empty) | wsp-core pending.rs Slates is an empty trait |
| `PullRequestsCtx` | `mark_settled` | ctx.mark(threads, { readAt, settledAt }, undefined) (BootArea) | wsp-core BootArea (empty) | slice fixes the stamp and the origin |
| `PullRequestsCtx` | `pauses` | ctx.pauses (RulesArea) | wsp-core RulesArea (empty) | filled by this crate's rules.rs |
| `PullRequestsCtx` | `nap_with` | ctx.napWith (MachinesArea) | wsp-core MachinesArea (empty) | TS takes a reason and answers a WorkspaceView; slice answers nothing |
| `PullRequestsCtx` | `read_lead_of` | ctx.readLeadOf (ReachArea) | wsp-core ReachArea (empty) |  |
| `PullRequestsCtx` | `queued` | ctx.queued(id, work) (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `PullRequestsCtx` | `spawn` | void <promise> (pull-requests.ts:236,271,293); no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `PullRequestsCtx` | `warn` | console.warn (pull-requests.ts:268,271) | wsp-core item: wsp_core::console::warn |  |
| `RulesCtx` | `no_machines_line` | opts.noMachinesLine (rules.ts:33) | opts: RuntimeOptions.no_machines_line |  |
| `RulesCtx` | `store_put_project` | store.put(PROJECTS, project.id, project) (rules.ts:17) | wsp-core item: wsp_store::store::Store::put | the same TS function also sets projectsHeld; the map write goes through the `projects_held` handle |
| `RulesCtx` | `door_name_of` | placeDoor?.nameOf(placeId) (rules.ts:244) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `RulesCtx` | `landing` | ctx.landing (RuntimeCore.landing) | wsp-core RuntimeCore (empty) | shared with the create area |
| `RulesCtx` | `live` | ctx.live values (rules.ts:212,312) | wsp-core RuntimeCore (empty) | helper over the map |
| `RulesCtx` | `live_get` | ctx.live.get(id) (rules.ts:76,97) | wsp-core RuntimeCore (empty) | helper over the map |
| `RulesCtx` | `projects_held` | ctx.projectsHeld (rules.ts:16,142) | wsp-core RuntimeCore (empty) | the map shared with every reader |
| `RulesCtx` | `sessions_size` | ctx.sessions.size (rules.ts:253) | wsp-core RuntimeCore (empty) | helper over the map |
| `RulesCtx` | `thread_records_size` | ctx.threadRecords.size (rules.ts:253) | wsp-core RuntimeCore (empty) | helper over the map |
| `RulesCtx` | `thread_record` | ctx.threadRecords.get(threadId) (rules.ts:232,249,262) | wsp-core RuntimeCore (empty) | helper over the map |
| `RulesCtx` | `rows_on` | ctx.rowsOn (AgentsArea, rules.ts:232,249,262) | wsp-core AgentsArea (empty) | not ThreadsArea; slice answers TreeFacts |
| `RulesCtx` | `target_workspace` | (await ctx.preferences.get()).target?.workspace (rules.ts:21) | wsp-core item: wsp_core::types::api::PreferencesApi::get | PreferencesArea.preferences; helper reading one field |
| `RulesCtx` | `set_preferences` | ctx.preferences.set(patch) (rules.ts:24) | wsp-core item: wsp_core::types::api::PreferencesApi::set | slice passes the patch as a serde_json::Value, PreferencesApi::set takes PreferencesPatch |
| `RulesCtx` | `capability` | ctx.backendFor(record).capabilities[can] === true (rules.ts:44) | wsp-core KindsArea (empty) | backendFor; Capabilities is a real item in wsp-wire (wsp_wire::wire::capabilities), `can` is a TS key string |
| `RulesCtx` | `pause_mode_declared` | ctx.backendFor(record).capabilities.pauseMode !== undefined (rules.ts:48) | wsp-core KindsArea (empty) | backendFor; Capabilities in wsp-wire |
| `RulesCtx` | `forks_no_machines` | forksNoMachines(ctx.backendFor(record).capabilities) (rules.ts:33) | wsp-core KindsArea (empty) | backendFor; forksNoMachines is wsp_wire::wire::capabilities::forks_no_machines |
| `RulesCtx` | `relayed` | ctx.moduleOf(kind).relayed(machineId) (rules.ts:61) | wsp-core KindsArea (empty) | moduleOf; KindModule::relayed(machine_id) exists in wsp-core and takes no kind |
| `RulesCtx` | `turn_reach` | ctx.moduleOf(kind).turnReach(entry) (rules.ts:266) | wsp-core KindsArea (empty) | moduleOf; KindModule::turn_reach answers Option<Option<String>>, slice answers an Option of a map |
| `RulesCtx` | `wsp_mcp` | ctx.moduleOf(kind).wspMcp(entry) (rules.ts:268) | wsp-core KindsArea (empty) | moduleOf; KindModule::wsp_mcp answers Option<McpServerSpec>, slice answers an Option<Value> |
| `RulesCtx` | `same_repository` | ctx.sameRepository (ChannelsArea, rules.ts:83) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs |
| `RulesCtx` | `project_held` | ctx.projectHeld (ChannelsArea, rules.ts:83) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs; throws for an unheld id |
| `RulesCtx` | `kind_of` | ctx.kindOf (KindsArea, rules.ts:95) | wsp-core KindsArea (empty) |  |
| `RulesCtx` | `place_name` | ctx.placeName (BuildersArea, rules.ts:132,159,275) | wsp-core BuildersArea (empty) |  |
| `RulesCtx` | `agents_held` | ctx.agentsHeld (ChannelsArea, rules.ts:237) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs |
| `RulesCtx` | `place_id_of` | ctx.placeIdOf (ChannelsArea, rules.ts:243) | wsp-core ChannelsArea (empty) | filled by this crate's channels.rs |
| `StatusCtx` | `ready` | ctx.ready (BootArea, status.ts:13) | wsp-core BootArea (empty) | wsp-core PipeAreaContext::ready is the same call |
| `StatusCtx` | `held` | ctx.held() (RulesArea) | wsp-core RulesArea (empty) | filled by this crate's rules.rs; not PreferencesCtx::held |
| `StatusCtx` | `view` | ctx.view(record) (RecordsArea) | wsp-core RecordsArea (empty) | TS answers a WorkspaceView, slice answers a serde_json::Map |
| `StatusCtx` | `rate_usd_per_hour` | ctx.backendFor(record).pricing.rateUsdPerHour(size) | wsp-core KindsArea (empty) | backendFor; MachineBackend::pricing exists in wsp-engine |
| `StatusCtx` | `row_reason` | ctx.rowReason (UsageArea) | wsp-core UsageArea (empty) | filled by this crate's usage.rs |
| `StatusCtx` | `idle_at` | ctx.idle.idleAt(id) (MachinesArea.idle) | wsp-core item: wsp_core::idle::IdlePolicy::idle_at | answers Option<i64>, slice Option<f64>; the TS calls it only while the phase is running |
| `StatusCtx` | `away_line` | ctx.awayLine (UsageArea) | wsp-core UsageArea (empty) | filled by this crate's usage.rs |
| `StatusCtx` | `unreached_of` | ctx.unreachedOf (DaemonArea) | wsp-core DaemonArea (empty) | TS answers a string, slice an Option<Value> |
| `StatusCtx` | `reach_of` | ctx.reachOf (ReachArea) | wsp-core ReachArea (empty) | TS answers ReachState, slice a String |
| `StatusCtx` | `has_daemon` | ctx.moduleOf(kind).hasDaemon(entry) | wsp-core KindsArea (empty) | moduleOf; KindModule::has_daemon exists in wsp-core |
| `StatusCtx` | `machine_reads` | e.machine.daemonAnswers, daemonAnswersPromptMs, metrics, facts (members of the live machine, not ctx) | wsp-engine item: wsp_engine::machine::Machine::daemon_answers | one method folds four optional members into a tuple; Machine also has daemon_answers_prompt_ms, metrics, facts, as default bodies, so Rust cannot read TS's `!== undefined` presence |
| `StatusCtx` | `polled_reach_set` | ctx.polledReach.set(id, { machineId, reach }) (ReachArea) | wsp-core ReachArea (empty) | helper over the map |
| `StatusCtx` | `polled_reach_delete` | ctx.polledReach.delete(id) (ReachArea) | wsp-core ReachArea (empty) | helper over the map |
| `StatusCtx` | `boot_work_has` | ctx.bootWork.has(id) (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over the map |
| `StatusCtx` | `revive_daemon` | ctx.reviveDaemon (DaemonArea) | wsp-core DaemonArea (empty) |  |
| `StatusCtx` | `offer_daemon_again` | ctx.offerDaemonAgain (DaemonArea) | wsp-core DaemonArea (empty) |  |
| `StatusCtx` | `read_version_again` | ctx.readVersionAgain (DaemonArea) | wsp-core DaemonArea (empty) |  |
| `StatusCtx` | `adopt_gone` | ctx.adoptGone (MachinesArea) | wsp-core MachinesArea (empty) |  |
| `StatusCtx` | `live` | live.get(id) | wsp-core RuntimeCore (empty) | helper over the map |
| `StatusCtx` | `store_get` | store.get(collection, id) | wsp-core item: wsp_store::store::Store::get |  |
| `StatusCtx` | `store_put` | store.put(collection, id, value) | wsp-core item: wsp_store::store::Store::put |  |
| `StatusCtx` | `store_list` | store.list(collection) | wsp-core item: wsp_store::store::Store::list |  |
| `StatusCtx` | `store_delete` | store.delete(collection, id) | wsp-core item: wsp_store::store::Store::delete |  |
| `StatusCtx` | `now` | clock.now() | wsp-core item: wsp_core::clock::Clock::now |  |
| `StatusCtx` | `wall_now` | Date.now(), new Date() | wsp-core item: wsp_core::clock::Clock::now | TS reads the wall clock beside clock.now(); README A7 makes it the one clock, so two methods read one Clock::now |
| `StatusCtx` | `owner` | ctx.state.owner | wsp-core item: wsp_core::context::RuntimeState::owner |  |
| `StatusCtx` | `lost_workspace` | lostWorkspace(row, owner, now) (@wsp/engine free fn) | wsp-engine item: wsp_engine::orphans::lost_workspace | takes ListedMachine and i64, slice takes ListedRow |
| `StatusCtx` | `project_held` | projectsHeld.has(id) | wsp-core RuntimeCore (empty) | helper over the map; not ctx.projectHeld |
| `StatusCtx` | `name_refusal` | ctx.nameRefusal (CreateArea) | wsp-core CreateArea (empty) |  |
| `StatusCtx` | `places_wired` | ctx.places.wired | wsp-core item: wsp_core::types::wiring::PlaceBackends::wired |  |
| `StatusCtx` | `golden_head` | goldenHead(await ctx.golden.get())?.snapshotId (GoldenArea.golden) | wsp-core item: wsp_core::types::api::GoldenApi::get | goldenHead is wsp_wire::views::golden_image::golden_head; GoldenApi::get takes an optional name |
| `StatusCtx` | `confirm` | ctx.observed(await at.backend.get(id)), machine.state(), ctx.shapeOf, ctx.sizeBuilt (status.ts adoptLost) | gap: helper over several members: MachinesArea.observed, RecordsArea.shapeOf and sizeBuilt, and the `at` backend adoptLost is handed |  |
| `StatusCtx` | `stop_unrecorded` | ctx.gone.stop(at.backend, await at.backend.get(id)) (RuntimeCore.gone) | wsp-core RuntimeCore (empty) | GoneWatch has no Rust item; the TS isMissing check stays in the area |
| `StatusCtx` | `attach` | ctx.attach (MachinesArea) | wsp-core MachinesArea (empty) | takes the machine `confirm` read |
| `StatusCtx` | `persist` | ctx.persist (RecordsArea) | wsp-core RecordsArea (empty) |  |
| `StatusCtx` | `sync_daemon` | ctx.syncDaemon (DaemonArea) | wsp-core DaemonArea (empty) | TS ignores the promise |
| `StatusCtx` | `emit` | bus.emit | wsp-core item: wsp_core::types::events::Bus::emit | slice passes a serde_json::Value, Bus::emit takes EventUnion and answers Result<u64, HostError> |
| `StatusCtx` | `emit_status` | ctx.emitStatus (ReachArea) | wsp-core ReachArea (empty) |  |
| `StatusCtx` | `spawn` | void <promise> (status.ts: syncDaemon, adoptGone); no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `StatusCtx` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |
| `UsageCtx` | `now` | clock.now() | wsp-core item: wsp_core::clock::Clock::now |  |
| `UsageCtx` | `schedule` | setTimeout inside bounded(work, ms, what) (places/helpers.ts:58) | wsp-core item: wsp_core::clock::Clock::schedule | TS bounded uses a node timer, not ctx.clock; wsp-machines places/helpers.rs bounded already takes a Clock; slice takes (ms, f), Clock::schedule takes (f, ms) |
| `UsageCtx` | `usage_logs_on` | (await ctx.preferences.get()).usageLogs (PreferencesArea) | wsp-core item: wsp_core::types::api::PreferencesApi::get | helper reading one field |
| `UsageCtx` | `places` | placeDoor?.list(clock.now()) ?? [] | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method); empty list where no door |
| `UsageCtx` | `has_door` | ctx.placeDoor !== undefined | wsp-core RuntimeCore (empty) |  |
| `UsageCtx` | `place_away` | ctx.placeAway (RuntimeCore) | wsp-core RuntimeCore (empty) | TS function member of RuntimeCore |
| `UsageCtx` | `name_of` | placeDoor.nameOf(id) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `UsageCtx` | `linked` | placeDoor?.link(id) !== undefined | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `UsageCtx` | `signed_in_at` | placeDoor.signInsAt(id)?.[name] === "signed-in" | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `UsageCtx` | `place_for` | placeDoor.placeFor(word) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `UsageCtx` | `box_login_path` | placeDoor.reportOf(id)?.login["PATH"] | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method) |
| `UsageCtx` | `box_exec` | placeDoor.exec(id, script, { timeoutMs, stdin }) | wsp-core RuntimeCore (empty) | via placeDoor (PlaceDoor method); TS stdin is bytes (envInput), slice takes &str |
| `UsageCtx` | `local_wired` | local !== undefined (RuntimeCore.local) | wsp-core item: wsp_core::types::wiring::LocalWiring | presence of the Option |
| `UsageCtx` | `local_has_daemon` | local.daemonRoad !== undefined | wsp-core item: wsp_core::types::wiring::LocalWiring::daemon_road | presence of the Option |
| `UsageCtx` | `local_env` | local.env() | wsp-core item: wsp_core::types::wiring::LocalWiring::env |  |
| `UsageCtx` | `local_home` | local.home(id) | wsp-core item: wsp_core::types::wiring::LocalWiring::home |  |
| `UsageCtx` | `local_exec_lines` | local.execStream({ idleMs, deadlineMs })(script, { env }) | wsp-core item: wsp_core::types::wiring::LocalWiring::exec_stream | exec_stream takes (Option<MachineExecOptions>, Option<TurnWaiting>) and answers a factory; slice drains lines and the exit in one call |
| `UsageCtx` | `read_logs_on` | readOn(id) in readLogs: ctx.onThisComputer, or placeDoor.channel + ctx.overChannel, then UsageLogsReply.parse (usage.ts) | gap: helper over ChannelsArea.onThisComputer and overChannel, placeDoor.channel and a reply parse; no single member |  |
| `UsageCtx` | `sys_history` | usage.readings body: ctx.withDaemon + ctx.entryOf, or ctx.overChannel + ctx.channelOver + ctx.localRoad, or placeDoor.channel, then SysHistoryReply.parse (usage.ts) | gap: helper over ChannelsArea withDaemon, overChannel and channelOver, BootArea.entryOf, KindsArea.localRoad and placeDoor.channel; no single member |  |
| `UsageCtx` | `ready` | ctx.ready (BootArea) | wsp-core BootArea (empty) | wsp-core PipeAreaContext::ready is the same call |
| `UsageCtx` | `wsp_sessions` | sessions.values() claudeSessionId | wsp-core RuntimeCore (empty) | helper over the map |
| `UsageCtx` | `projects_on` | [...projectsHeld.values()].filter(p => p.computer === id) | wsp-core RuntimeCore (empty) | helper over the map |
| `UsageCtx` | `project_name` | projectsHeld.get(value)?.name | wsp-core RuntimeCore (empty) | helper over the map |
| `UsageCtx` | `vaulted_for` | ctx.vaultedFor (ThreadsArea) | wsp-core ThreadsArea (empty) | TS takes optional loginStands and vault; slice takes the name alone and answers a Value |
| `UsageCtx` | `launch_of` | setups.launchOf(place, name) (RuntimeCore.setups) | wsp-core RuntimeCore (empty) | wsp-core agent_setup.rs is empty; answers env, config dir and program as a tuple |
| `UsageCtx` | `login_home` | planRoad in usage.ts: placeDoor.list(...).logins + sharedOn(name) + loginHomeIn(logins, shared) | gap: local helper planRoad (usage.ts) over placeDoor.list and setups.launchOf; its catalog parts are real items at wsp_catalog::catalog::shared_on and wsp_catalog::signin::login_home_in | slice answers Option<String>, TS throws resetNoLoginsLine where none |
| `UsageCtx` | `read_sign_ins_here` | opts.agentsReader?.read({ kind: "here" }, { latest: false }) | opts: RuntimeOptions.agents_reader | wsp-core's AgentsReader (types/pending.rs) is an empty stand-in; slice reduces the report to (id, state) pairs |
| `UsageCtx` | `on_agents_changed` | bus.on("agents.changed", fn) | wsp-core item: wsp_core::types::events::EventBus::on | slice takes Fn(), EventBus::on takes a typed listener |
| `UsageCtx` | `limits` | ledger.limits() (UsageArea.ledger) | wsp-core UsageArea (empty) | UsageLedger has no Rust item; wsp-machines usage.rs is empty |
| `UsageCtx` | `file_logs` | ledger.fileLogs(entries, read) (UsageArea.ledger) | wsp-core UsageArea (empty) | UsageLedger has no Rust item |
| `UsageCtx` | `file_limit` | ledger.limit(reading) (UsageArea.ledger) | wsp-core UsageArea (empty) | UsageLedger has no Rust item; near name of `limits` |
| `UsageCtx` | `alerts_read` | alerts.read(before, after) (UsageArea.alerts) | wsp-core UsageArea (empty) | wsp-core plan_alerts.rs is empty |
| `UsageCtx` | `used` | ledger.used({ range, split, label, outside }) (UsageArea.ledger) | wsp-core UsageArea (empty) | UsageLedger has no Rust item |
| `UsageCtx` | `account_labels` | ledger.accountLabels() (UsageArea.ledger) | wsp-core UsageArea (empty) | UsageLedger has no Rust item |
| `UsageCtx` | `account_on_computer` | accountOnComputer({...}) (usage.ts free fn) | gap: TS free function accountOnComputer (usage.ts); wsp-machines usage.rs is empty | also listed as BootRuntime::account_key_on in the machines table |
| `UsageCtx` | `account_of` | accountOf({...}) (usage.ts free fn) | gap: TS free function accountOf (usage.ts); wsp-machines usage.rs is empty |  |
| `UsageCtx` | `agent_label` | harnessCatalog(name)?.label ?? name (usage.ts local agentLabel) | gap: TS free function harnessCatalog (harness-catalog.ts); wsp-core harness_catalog.rs is empty | usage.ts's own local helper, not ctx.agentLabel (AgentsArea) |
| `UsageCtx` | `model_label` | modelLabel(model) (harness-catalog.ts free fn) | gap: TS free function modelLabel (harness-catalog.ts); wsp-core harness_catalog.rs is empty |  |
| `UsageCtx` | `log_limit` | LOG_LIMITS[name]?.(snapshot) (adapters.ts const) | wsp-adapters item: wsp_adapters::codex::limits::limit_of | the LOG_LIMITS table is adapters.ts (wsp-core adapters.rs is empty) and holds codex alone; limit_of takes (snapshot, account, credits) |
| `UsageCtx` | `plan_agents` | Object.keys(PLAN_RESETS) (adapters.ts const) | gap: PLAN_RESETS table (adapters.ts); wsp-core adapters.rs is empty | the table holds codex alone |
| `UsageCtx` | `plan_read_command` | PLAN_RESETS[name].readCommand(road) | wsp-adapters item: wsp_adapters::port::PlanResets::read_command | found through the PLAN_RESETS table (gap); CodexPlanResets is the implementor; slice's PlanRoad against ResetRoad |
| `UsageCtx` | `plan_parse` | PLAN_RESETS[name].parseRead(out) | wsp-adapters item: wsp_adapters::port::PlanResets::parse_read | found through the PLAN_RESETS table (gap); answers ResetReading, slice answers an Option<HarnessLimit> |
| `UsageCtx` | `account_rows` | accountRows({...}) (usage.ts free fn) | gap: TS free function accountRows (usage.ts); wsp-machines usage.rs is empty | the TS call also passes plan brands, vaulted, prints-limits and burn, which the context holds |
| `UsageCtx` | `account_names` | accountNames({...}) (usage.ts free fn) | gap: TS free function accountNames (usage.ts); wsp-machines usage.rs is empty |  |
| `UsageCtx` | `kept_computer_names` | keptComputerNames(store, places) (usage.ts free fn) | gap: TS free function keptComputerNames (usage.ts), reads ctx.store; wsp-machines usage.rs is empty |  |
| `UsageCtx` | `computer_name` | usageComputerName(places, id, kept) (usage.ts free fn) | gap: TS free function usageComputerName (usage.ts); wsp-machines usage.rs is empty | near name of `name_of` (placeDoor.nameOf) |
| `UsageCtx` | `spend_reset` | usageResets({...}) (usage-reset.ts free fn built in usageArea) | gap: TS free function usageResets (usage-reset.ts); ported inside this crate at src/usage_reset.rs, not in a wsp-core crate | the TS usage.reset wrapper reads placeDoor.placeFor first |
| `UsageCtx` | `nap_refused_reason` | ctx.napRefusedReason (ReachArea) | wsp-core ReachArea (empty) |  |
| `UsageCtx` | `stop_refused_reason` | ctx.stopRefusedReason (ReachArea) | wsp-core ReachArea (empty) |  |
| `UsageCtx` | `disk_reason` | ctx.diskReason (ReachArea) | wsp-core ReachArea (empty) |  |
| `UsageCtx` | `spawn` | void <promise> (usage.ts: alerts.resume); no member | gap: detached task (TS `void p`); no context.ts member and no wsp-core item |  |
| `UsageCtx` | `warn` | console.warn | wsp-core item: wsp_core::console::warn |  |

## Counts

260 methods in 8 traits.
| `Base` | `ready` | ctx.ready (BootArea) | wsp-core item: wsp_machines::machines::boot::ready::BootArea::ready | BootArea::ready answers a Ready handle, not a future of Result |
| `Base` | `clock` | ctx.clock (RuntimeCore) | wsp-core item: wsp_core::clock::Clock |  |
| `Base` | `ids` | randomUUID, randomBytes (node:crypto); no ctx member | wsp-core item: wsp_core::ids::Ids | whole seam handed out, not one draw |
| `Base` | `store` | ctx.store (RuntimeCore) | wsp-core item: wsp_store::store::Store | Base reaches it through the wsp_core::types::pending::Store re-export |
| `Base` | `live` | ctx.live (RuntimeCore) | wsp-core RuntimeCore (empty) | map of LiveWorkspace, here a map of Entry |
| `Base` | `projects_held` | ctx.projectsHeld (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `Base` | `project_held` | ctx.projectHeld (ChannelsArea) | wsp-core ChannelsArea (empty) | TS throws on a project not held, here a Result |
| `Base` | `session_views` | ctx.sessions.values() (RuntimeCore.sessions) | wsp-core RuntimeCore (empty) | helper over the map |
| `Base` | `thread_records` | ctx.threadRecords (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over the map, a Vec of (id, record) here |
| `Base` | `persist` | ctx.persist (RecordsArea) | wsp-core RecordsArea (empty) |  |
| `Base` | `view` | ctx.view (RecordsArea) | wsp-core RecordsArea (empty) |  |
| `Base` | `emit` | bus.emit (RuntimeCore.bus) | wsp-core item: wsp_core::types::events::Bus::emit | Bus::emit takes EventUnion and answers Result<u64>; here a serde_json::Value and Result<()> |
| `Base` | `entry_of` | ctx.entryOf (BootArea) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::entry_of | takes Option<&Caller> |
| `Base` | `spawn_guard` | ctx.spawnGuard (RulesArea) | wsp-core RulesArea (empty) | act is a &str here, SpawnAct in TS |
| `Base` | `kind_of` | ctx.kindOf (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::kind_of |  |
| `Base` | `place_name` | ctx.placeName (BuildersArea) | wsp-core BuildersArea (empty) | not wsp_words::place_name::place_name, which takes a PlaceView |
| `Base` | `copy_blocked` | ctx.copyBlocked (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `Base` | `with_daemon` | ctx.withDaemon (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `Base` | `on_this_computer` | ctx.onThisComputer (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `Base` | `git_here` | ctx.gitHere (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::git_here | takes &[&str] args and stdin as Option<&str> |
| `Base` | `checkout_of` | ctx.checkoutOf (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `Base` | `preferences` | ctx.preferences.get() (PreferencesArea.preferences) | wsp-core item: wsp_core::types::api::PreferencesApi::get |  |
| `Base` | `write_daemon_roots` | ctx.writeDaemonRoots (DaemonArea) | wsp-core item: wsp_machines::machines::daemon::DaemonArea::write_daemon_roots |  |
| `Base` | `home_dir` | os.homedir() (node:os, folders.ts:113,191); no ctx member | gap: node:os homedir(); no ctx member and no wsp-core item | LocalWiring::home_dir is a different value (branch-of-record.ts:104) |
| `Base` | `machine_id` | entry.machine.id (LiveWorkspace.machine) | wsp-core item: wsp_engine::machine::Machine::id | reached through the entry, not ctx |
| `FoldersCtx` | `state_folder` | ctx.stateFolder (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::state_folder | KindsArea::state_folder answers Result<String> |
| `FoldersCtx` | `has_state_path` | ctx.opts.statePath !== undefined | opts: RuntimeOptions.state_path | presence of the Option |
| `FoldersCtx` | `has_copier` | local?.copier !== undefined (RuntimeCore.local) | wsp-core item: wsp_core::types::wiring::LocalWiring::copier | presence of the Option |
| `FoldersCtx` | `copier_worktree` | local.copier.worktree | wsp-core item: wsp_engine::copier::Copier::worktree | reached through LocalWiring.copier |
| `FoldersCtx` | `copier_worktree_mount` | local.copier.worktreeMount | wsp-core item: wsp_engine::copier::Copier::worktree_mount | Copier takes a WorktreeAt |
| `FoldersCtx` | `copier_worktree_remove` | local.copier.worktreeRemove | wsp-core item: wsp_engine::copier::Copier::worktree_remove | Copier answers WorktreeRemoval |
| `FoldersCtx` | `here_daemon` | local.hereDaemon (RuntimeCore.local) | wsp-core item: wsp_core::types::wiring::LocalWiring::here_daemon |  |
| `FoldersCtx` | `admit_folder` | ctx.moduleOf(kind).admitFolder | wsp-core item: wsp_core::types::internal::KindModule::admit_folder | reached through ctx.moduleOf (KindsArea, empty); KindModule takes a FolderAdmit |
| `FoldersCtx` | `folder_machine` | ctx.backendOfKind(kind, place).get(...) and .pricing.defaultSize | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::backend_of_kind | get and pricing are read off the MachineBackend |
| `FoldersCtx` | `attach` | ctx.attach (MachinesArea) | wsp-core item: wsp_machines::machines::machines::attach::MachinesArea::attach | MachinesArea::attach takes the record as Rc<RefCell<WorkspaceRecord>> |
| `FoldersCtx` | `folders_of` | ctx.foldersOf (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::folders_of |  |
| `FoldersCtx` | `one_folder` | ctx.oneFolder (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::one_folder |  |
| `FoldersCtx` | `resolve_project` | ctx.projectsDoor.resolve (ProjectsArea.projectsDoor) | wsp-core item: wsp_core::types::api::ProjectsApi::resolve | projectsDoor is its own member; rt.projects.resolve is the same function (projects.ts:475) |
| `FoldersCtx` | `elsewhere_refusal` | ctx.elsewhereRefusal (RulesArea) | wsp-core RulesArea (empty) |  |
| `FoldersCtx` | `picks_hold` | ctx.picksHold (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `FoldersCtx` | `branch_at` | ctx.branchAt (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::branch_at |  |
| `FoldersCtx` | `turn_runs` | ctx.turnRuns (DaemonArea) | wsp-core item: wsp_machines::machines::daemon::DaemonArea::turn_runs |  |
| `FoldersCtx` | `drop_workspace` | ctx.drop (MachinesArea) | wsp-core item: wsp_machines::machines::machines::attach::MachinesArea::drop | TS name is drop, here drop_workspace |
| `FoldersCtx` | `checkpoints_landed` | ctx.checkpointsLanding.get(threadId) (ThreadsArea) | wsp-core ThreadsArea (empty) | helper over the map |
| `FoldersCtx` | `not_a_repo` | ctx.notARepo (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `FoldersCtx` | `read_pull_request` | ctx.readPullRequest (PullRequestsArea) | wsp-core PullRequestsArea (empty) |  |
| `FoldersCtx` | `status_now` | ctx.statusNow (ReachArea) | wsp-core item: wsp_machines::machines::reach::disk::ReachArea::status_now |  |
| `FoldersCtx` | `local_exec` | ctx.backendOfKind("local").get(LOCAL_MACHINE_ID) then machine.exec (folders.ts:109,115,355) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::backend_of_kind | helper: the exec itself is Machine::exec |
| `FoldersCtx` | `sweep_state` | ctx.state.sweepTimer, .sweeping, .sweepStopped (RuntimeState) | wsp-core item: wsp_core::context::RuntimeState::sweep_timer | three RuntimeState fields (sweep_timer, sweeping, sweep_stopped) grouped as SweepState here |
| `ProjectIconsCtx` | `state_folder` | ctx.stateFolder (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::state_folder | PathBuf here, Result<String> there |
| `ProjectIconsCtx` | `has_state_path` | ctx.opts.statePath !== undefined | opts: RuntimeOptions.state_path | presence of the Option |
| `ProjectIconsCtx` | `ready` | ctx.ready (BootArea) | wsp-core item: wsp_machines::machines::boot::ready::BootArea::ready |  |
| `ProjectIconsCtx` | `project_held` | ctx.projectsHeld.has(id) (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over the map; not ctx.projectHeld |
| `ProjectIconsCtx` | `preferences` | ctx.preferences.get() (PreferencesArea.preferences) | wsp-core item: wsp_core::types::api::PreferencesApi::get |  |
| `ProjectIconsCtx` | `change_preferences` | ctx.changePreferences (PreferencesArea) | wsp-core PreferencesArea (empty) |  |
| `ProjectIconsCtx` | `recipes` | ctx.opts.recipes | opts: RuntimeOptions.recipes |  |
| `ProjectIconsCtx` | `on_recipes_changed` | ctx.bus.on("recipes.changed", f) | wsp-core item: wsp_core::types::events::EventBus::on | EventBus::on takes a typed EventListener and answers an Unlisten |
| `ProjectIconsCtx` | `ids` | randomBytes (node:crypto, images.ts:110); no ctx member | wsp-core item: wsp_core::ids::Ids::bytes |  |
| `ProjectsCtx` | `state_folder` | ctx.stateFolder (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::state_folder |  |
| `ProjectsCtx` | `has_state_path` | ctx.opts.statePath !== undefined | opts: RuntimeOptions.state_path | presence of the Option |
| `ProjectsCtx` | `seed_wiring` | ctx.opts.seed | opts: RuntimeOptions.seed |  |
| `ProjectsCtx` | `cloud_projects` | ctx.opts.cloudProjects | opts: RuntimeOptions.cloud_projects |  |
| `ProjectsCtx` | `vault_github_token` | opts.vault?.()[GITHUB_TOKEN_ENV] | opts: RuntimeOptions.vault | helper: one key off the vault's map |
| `ProjectsCtx` | `local_claude_home` | local?.home("claude") (RuntimeCore.local) | wsp-core item: wsp_core::types::wiring::LocalWiring::home |  |
| `ProjectsCtx` | `homes_here` | ctx.homesHere (AgentsArea) | wsp-core item: wsp_core::types::api::AgentsApi::homes_here | same function: runtime.ts:1056 hands ctx.homesHere to the runtime's api object |
| `ProjectsCtx` | `refuse_recording` | ctx.refuseRecording (RulesArea) | wsp-core RulesArea (empty) |  |
| `ProjectsCtx` | `computer_rows` | ctx.computerRows (BootArea) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::computer_rows |  |
| `ProjectsCtx` | `name_of_computer` | ctx.nameOfComputer (BootArea) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::name_of_computer | BootArea::name_of_computer has no self |
| `ProjectsCtx` | `clone_here` | ctx.cloneHere (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::clone_here |  |
| `ProjectsCtx` | `git_top_of` | ctx.gitTopOf (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::git_top_of |  |
| `ProjectsCtx` | `remote_here` | ctx.remoteHere (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::remote_here | KindsArea::remote_here answers (remote, default branch); here only the remote |
| `ProjectsCtx` | `landing_place` | ctx.landingPlace(computer).placeId (BootArea) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::landing_place |  |
| `ProjectsCtx` | `forking_at` | ctx.forkingAt (BuildersArea) | wsp-core BuildersArea (empty) | TS answers undefined where no backend, here a Result |
| `ProjectsCtx` | `landing_backend` | ctx.landingBackend (BuildersArea) | wsp-core BuildersArea (empty) |  |
| `ProjectsCtx` | `folder_computer` | placeDoor?.folderComputer (RuntimeCore.placeDoor) | wsp-core item: wsp_machines::places::types::PlaceDoor::folder_computer | via placeDoor; PlaceDoor answers FolderComputer, here JoinedComputer |
| `ProjectsCtx` | `github_from_vault` | placeDoor?.githubFromVault (RuntimeCore.placeDoor) | wsp-core item: wsp_machines::places::types::PlaceDoor::github_from_vault | via placeDoor |
| `ProjectsCtx` | `landing_road` | projectLanding(kind) (project-landing.ts free fn) | gap: TS free function projectLanding (project-landing.ts); wsp-core project_landing.rs is an empty module | answers the LandingRoad below |
| `ProjectsCtx` | `project_source` | projectSource(kind) (project-sources.ts free fn) | gap: TS free function projectSource (project-sources.ts); wsp-core project_sources.rs is an empty module | answers the ProjectSourceModule below |
| `ProjectsCtx` | `remember_project` | ctx.rememberProject (RulesArea) | wsp-core RulesArea (empty) |  |
| `ProjectsCtx` | `project_reached` | ctx.projectReached (RulesArea) | wsp-core RulesArea (empty) |  |
| `ProjectsCtx` | `project_of_scope` | ctx.projectOfScope (RulesArea) | wsp-core RulesArea (empty) | TS takes a ThreadScope, here the Caller |
| `ProjectsCtx` | `elsewhere_refusal` | ctx.elsewhereRefusal (RulesArea) | wsp-core RulesArea (empty) |  |
| `ProjectsCtx` | `of_threads_repository` | ctx.ofThreadsRepository (RulesArea) | wsp-core RulesArea (empty) |  |
| `ProjectsCtx` | `holds_thread` | ctx.holdsThread (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `ProjectsCtx` | `delete_workspace` | ctx.workspaces.delete (WorkspacesArea.workspaces) | wsp-core item: wsp_core::types::api::WorkspacesApi::delete |  |
| `ProjectsCtx` | `copier_worktree_forget` | local.copier.worktreeForget | wsp-core item: wsp_engine::copier::Copier::worktree_forget | reached through LocalWiring.copier; Copier takes a WorktreeProject |
| `ProjectsCtx` | `forget_icon` | ctx.projectIcons.forget (ProjectIconsArea) | wsp-core ProjectIconsArea (empty) |  |
| `ProjectsCtx` | `set_icon` | ctx.projectIcons.set (ProjectIconsArea) | wsp-core ProjectIconsArea (empty) |  |
| `ProjectsCtx` | `read_icons` | ctx.projectIcons.read (ProjectIconsArea) | wsp-core ProjectIconsArea (empty) |  |
| `ProjectsCtx` | `runs_agent` | adapters[id] !== undefined (RuntimeCore.adapters) | opts: RuntimeOptions.adapters | helper: presence of a key |
| `ProjectsCtx` | `agent_lists` | ctx.computerHome(computer, origin) then ctx.agentLists (AgentsArea) | wsp-core AgentsArea (empty) | helper over two members of the area |
| `ProjectsCtx` | `thread_defaults` | resolveThreadDefaults (@wsp/protocol free fn) | gap: TS free function resolveThreadDefaults (@wsp/protocol); no Rust item in wsp-wire or wsp-words | wsp_wire::types::thread_defaults has only the ThreadDefaults types; also reads withCustomModels and prefs |
| `ProjectsCtx` | `first_agent` | CATALOG_AGENTS.find(a => runs(a.id))?.id ?? DEFAULT_AGENT.id (projects.ts:452) | wsp-core item: wsp_catalog::catalog::catalog_agents | helper over opts.adapters and wsp_catalog::catalog::default_agent; wsp-catalog is outside the crates the brief named |
| `LandingRoad` | `kind` | ProjectLanding.kind (project-landing.ts:71) | gap: member of the object projectLanding(kind) answers (project-landing.ts); wsp-core project_landing.rs is an empty module |  |
| `LandingRoad` | `path` | ProjectLanding.path | gap: member of the object projectLanding(kind) answers (project-landing.ts) |  |
| `LandingRoad` | `memory_dir` | ProjectLanding.places({...}).memoryDir | gap: member of the object projectLanding(kind) answers (project-landing.ts) | TS member is places, which answers ProjectPlaces; only memoryDir is read |
| `LandingRoad` | `lands_at_add` | ProjectLanding.landsAtAdd | gap: member of the object projectLanding(kind) answers (project-landing.ts) |  |
| `LandingRoad` | `land` | ProjectLanding.land | gap: member of the object projectLanding(kind) answers (project-landing.ts) | near-name: ProjectLander::land (wsp_core::types::wiring) is this computer's export side, a different call |
| `LandingRoad` | `remove` | ProjectLanding.remove | gap: member of the object projectLanding(kind) answers (project-landing.ts) |  |
| `LandingRoad` | `folder_stands` | ProjectLanding.folderStands? | gap: member of the object projectLanding(kind) answers (project-landing.ts) | optional in TS, Option here |
| `LandingRoad` | `seed_into` | ProjectLanding.seedInto? | gap: member of the object projectLanding(kind) answers (project-landing.ts) | optional in TS, Option here |
| `LandingRoad` | `unsaved` | ProjectLanding.unsaved? | gap: member of the object projectLanding(kind) answers (project-landing.ts) | optional in TS, Option here |
| `LandingRoad` | `branch` | ProjectLanding.branch | gap: member of the object projectLanding(kind) answers (project-landing.ts) | near-name: ProjectsApi::branch takes a project reference |
| `ProjectSourceModule` | `resolve` | ProjectSourceModule.resolve (project-sources.ts:35) | gap: member of the module projectSource(kind) answers (project-sources.ts); wsp-core project_sources.rs is an empty module | near-name: ProjectsApi::resolve resolves a project reference |
| `MoveRoads` | `agents_on_machine` | agentsOnMachine (@wsp/engine free fn) | wsp-core item: wsp_engine::project_state::agents_on_machine | item takes a &dyn Machine and answers a HashSet |
| `MoveRoads` | `guest_agent_homes` | guestAgentHomes (@wsp/engine free fn) | wsp-core item: wsp_engine::project_state::guest_agent_homes |  |
| `MoveRoads` | `scratch` | ctx.moduleOf(kind).scratch | wsp-core item: wsp_core::types::internal::KindModule::scratch | reached through ctx.moduleOf (KindsArea, empty) |
| `MoveRoads` | `land_bundle` | landBundle (@wsp/engine free fn) | wsp-core item: wsp_engine::vault::import::land_bundle | item takes LandOptions, here replace and LandHooks |
| `MoveRoads` | `import_into` | importInto (@wsp/engine free fn) | wsp-core item: wsp_engine::vault::import::import_into | item takes VaultOptions |
| `MoveRoads` | `merge_on_machine` | mergeOnMachine (free fn) | wsp-core item: wsp_core::types::wiring::merge_on_machine |  |
| `MoveRoads` | `project_of` | ctx.projectOf (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::project_of | KindsArea::project_of answers WorkspaceProject and takes size f64; here Option<Value> and bytes u64 |
| `MoveRoads` | `provider_row` | ctx.computerRows() row of ctx.placeIdOf(record) or record.provider, then isProviderPlace (projects.ts:32) | gap: helper over BootArea.computerRows and ChannelsArea.placeIdOf, with the TS free function isProviderPlace |  |
| `MoveRoads` | `export_folder` | exportFolder (@wsp/engine free fn) | wsp-core item: wsp_engine::vault::export::export_folder | item takes CacheRule and VaultOptions |
| `MoveRoads` | `export_paths_into` | exportPathsInto (@wsp/engine free fn) | wsp-core item: wsp_engine::vault::export::export_paths_into | item takes ArchiveGroup and VaultOptions |
| `MoveRoads` | `machine_run` | entry.machine.run (LiveWorkspace.machine) | wsp-core item: wsp_engine::machine::Machine::run | reached through the entry, not ctx |
| `MoveRoads` | `machine_exec` | entry.machine.exec (LiveWorkspace.machine) | wsp-core item: wsp_engine::machine::Machine::exec | reached through the entry, not ctx |
| `MoveRoads` | `kind_import` | ctx.moduleOf(kind).import | wsp-core item: wsp_core::types::internal::KindModule::import | reached through ctx.moduleOf (KindsArea, empty) |
| `MoveRoads` | `temp_dir` | tmpdir() (node:os, projects.ts:529); no ctx member | gap: node:os tmpdir(); no ctx member and no wsp-core item | the doc line above it describes actionRefusal and copyBlocked, not this method |
| `StartFromCtx` | `places_wired` | ctx.places.wired (RuntimeCore.places) | wsp-core item: wsp_core::types::wiring::PlaceBackends::wired |  |
| `StartFromCtx` | `landing_place` | ctx.landingDeps(computer).placeId (ProjectsArea) | wsp-core ProjectsArea (empty) | landingDeps answers {deps, at, placeId}; only placeId is kept |
| `StartFromCtx` | `landing_kind` | ctx.landingKind(computer, at) (ProjectsArea) | wsp-core ProjectsArea (empty) | TS takes the backend too, here the computer alone |
| `StartFromCtx` | `brokered_start` | brokeredStart(ctx, provider) (projects/branch-of-record.ts free fn) | gap: TS free function brokeredStart (projects/branch-of-record.ts); wsp-machines src/projects/branch_of_record.rs is empty | wsp-machines declares its own brokered_start on CreateRuntime and SubSpawnCtx |
| `StartFromCtx` | `pull_request_of_record` | pullRequestOfRecord(ctx, ...) (projects/branch-of-record.ts free fn) | gap: TS free function pullRequestOfRecord (projects/branch-of-record.ts); wsp-machines src/projects/branch_of_record.rs is empty |  |
| `StartFromCtx` | `create_workspace` | ctx.workspaces.create (WorkspacesArea.workspaces) | wsp-core item: wsp_core::types::api::WorkspacesApi::create |  |
| `StartFromCtx` | `delete_workspace` | ctx.workspaces.delete (WorkspacesArea.workspaces) | wsp-core item: wsp_core::types::api::WorkspacesApi::delete |  |
| `StartFromCtx` | `project_folder` | ctx.projectFolder (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `StartFromCtx` | `worktree_folder` | ctx.worktreeFolder (FoldersArea) | wsp-core FoldersArea (empty) | TS takes also parent, madeFor and checkpoint; the call passes three |
| `StartFromCtx` | `worktrees_of` | ctx.worktreesOf (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `StartFromCtx` | `take_setup_line` | ctx.takeSetupLine (FoldersArea) | wsp-core FoldersArea (empty) | near-name: WorkspacesApi::take_setup_line takes an origin |
| `StartFromCtx` | `preferences_held` | ctx.state.preferencesHeld (RuntimeState) | wsp-core item: wsp_core::context::RuntimeState::preferences_held | RuntimeState holds it as Option<serde_json::Value>, here Option<Preferences> |
| `StartFromCtx` | `default_agent_of` | ctx.defaultAgentOf (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `StartFromCtx` | `resolved_picks` | harnessCatalog(harness), ctx.launchAdapterFor, ctx.catalogOn, ctx.defaultsOn (AgentsArea) | gap: helper over three AgentsArea members and the TS free function harnessCatalog (harness-catalog.ts; wsp-core harness_catalog.rs is empty) |  |
| `StartFromCtx` | `access_word_refusal` | accessWordRefusal (@wsp/protocol free fn) | gap: TS free function accessWordRefusal (@wsp/protocol); no Rust item in wsp-wire or wsp-words |  |
| `StartFromCtx` | `named_mode` | ctx.namedMode (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `StartFromCtx` | `start_picks_refusal` | startPicks(catalog, picks, true) then notRunRefused ?? pickRefusal (free fns) | wsp-core item: wsp_wire::views::harness::start_picks | item answers Result<StartPicks, PickRefusal>; notRunRefused (harness-catalog.ts) has no Rust item |
| `StartFromCtx` | `start_session` | ctx.sessionsApi.start (SessionsArea.sessionsApi) | wsp-core item: wsp_core::types::api::SessionsApi::start | SessionsApi::start takes StartOptions and answers Rc<dyn SessionHandle>; here a Value and OpenedSession |
| `StartFromCtx` | `read_only_modes` | HARNESS_CATALOGS (harness-catalog.ts const) | gap: TS const HARNESS_CATALOGS (harness-catalog.ts); wsp-core harness_catalog.rs is empty |  |
| `StartFromCtx` | `send_detached` | ctx.sendDetached (DaemonArea) | wsp-core item: wsp_machines::machines::daemon::DaemonArea::send_detached | DaemonArea answers wsp_core::context::DetachedSend, here () |
| `CreateCtx` | `places_wired` | places.wired (RuntimeCore.places) | wsp-core item: wsp_core::types::wiring::PlaceBackends::wired |  |
| `CreateCtx` | `resolve_project` | ctx.projectsDoor.resolve (ProjectsArea.projectsDoor) | wsp-core item: wsp_core::types::api::ProjectsApi::resolve | same function as rt.projects.resolve |
| `CreateCtx` | `landing_place` | ctx.landingPlace (BootArea) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::landing_place |  |
| `CreateCtx` | `landing_capabilities` | ctx.landingBackend(placeId).capabilities (BuildersArea) | wsp-core BuildersArea (empty) | derived: landingBackend, then the backend's capabilities |
| `CreateCtx` | `backend_capabilities` | ctx.backendOfKind(kind, place).capabilities (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::backend_of_kind | derived: capabilities read off the MachineBackend |
| `CreateCtx` | `act_refusal` | ctx.actRefusal (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateCtx` | `spawn_sub` | ctx.spawnSub (SubSpawnArea) | wsp-core item: wsp_machines::machines::sub_spawn::checks::SubSpawnArea::spawn_sub | SubSpawnArea takes SpawnAsk and answers SpawnAnswer |
| `CreateCtx` | `place_of_thread` | ctx.placeOfThread (RulesArea) | wsp-core RulesArea (empty) | near-name: WorkspacesApi::thread_place |
| `CreateCtx` | `name_given` | ctx.nameGiven (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::name_given | no self |
| `CreateCtx` | `refuse_recording` | ctx.refuseRecording (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateCtx` | `refuse_relayed` | ctx.refuseRelayed (RulesArea) | wsp-core RulesArea (empty) | TS takes a record or a {kind, name, project} literal |
| `CreateCtx` | `project_folder` | ctx.projectFolder (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `CreateCtx` | `agents_from` | agentsFrom(ctx.spawnAt(place), ...) (workspaces.ts:121) | wsp-core item: wsp_wire::views::workspace::agents_from | free function; the held half is ctx.spawnAt (ChannelsArea, empty) |
| `CreateCtx` | `same_repository` | ctx.sameRepository (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `CreateCtx` | `place_guard` | ctx.placeGuard (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `CreateCtx` | `name_refusal` | ctx.nameRefusal (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::name_refusal |  |
| `CreateCtx` | `forking_add` | ctx.forking.add (CreateArea.forking) | wsp-core item: wsp_machines::machines::create::CreateArea::forking | helper over the set |
| `CreateCtx` | `forking_delete` | ctx.forking.delete (CreateArea.forking) | wsp-core item: wsp_machines::machines::create::CreateArea::forking | helper over the set |
| `CreateCtx` | `supersede_failed` | ctx.supersedeFailed (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::supersede_failed |  |
| `CreateCtx` | `stage_reporter` | ctx.stageReporter (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::stage_reporter |  |
| `CreateCtx` | `lead_start` | ctx.leadStart (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `CreateCtx` | `show_create` | ctx.showCreate (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::show_create |  |
| `CreateCtx` | `shown_view` | ctx.refOf(project) and ctx.treeOf(scope) as the fields of a ShownCreate (workspaces.ts:169) | gap: helper over ChannelsArea.refOf and RulesArea.treeOf | builds the view showCreate takes |
| `CreateCtx` | `create_staged` | ctx.createStaged (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::create_staged | CreateArea takes landed as a callback and child_base as &str; here a FreeSlot |
| `CreateCtx` | `child_started_line` | childStartedLine (@wsp/protocol free fn) | wsp-core item: wsp_words::tree::child_started_line |  |
| `CreateCtx` | `start_child_on` | ctx.startChildOn (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `CreateCtx` | `unfork` | ctx.unfork (MachinesArea) | wsp-core item: wsp_machines::machines::machines::create::MachinesArea::unfork | answers Result<(), HostError> there, EngineError here |
| `CreateCtx` | `shown_creates` | ctx.shownCreates (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::shown_creates | a RefCell<IndexMap> field there, Rc<RefCell<IndexMap>> here |
| `CreateCtx` | `failed_creates` | ctx.failedCreates (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::failed_creates | a RefCell<IndexMap> field there, Rc<RefCell<IndexMap>> here |
| `CreateCtx` | `failed_view` | ctx.failedView (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::failed_view |  |
| `CreateCtx` | `boot_work` | ctx.bootWork.get(id) (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over the map, awaited |
| `CreateCtx` | `listed_for` | ctx.listedFor (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateCtx` | `held` | ctx.held (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateCtx` | `drives` | ctx.drives (RulesArea) | wsp-core RulesArea (empty) | TS takes {kind, machineId}, here a WorkspaceView |
| `CreateCtx` | `talks_to_tree_on` | ctx.talksToTreeOn (BootArea) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::talks_to_tree_on |  |
| `CreateCtx` | `project_of_scope` | ctx.projectOfScope (RulesArea) | wsp-core RulesArea (empty) | TS takes a ThreadScope, here the Caller |
| `CreateCtx` | `refusal_for` | ctx.refusalFor (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateCtx` | `of_threads_repository` | ctx.ofThreadsRepository (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateCtx` | `elsewhere_refusal` | ctx.elsewhereRefusal (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateCtx` | `away_for` | ctx.awayFor (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateCtx` | `project_reached` | ctx.projectReached (RulesArea) | wsp-core RulesArea (empty) |  |
| `CreateCtx` | `find_places` | placeDoor?.find(word) (RuntimeCore.placeDoor) | wsp-core item: wsp_machines::places::types::PlaceDoor::find | via placeDoor; PlaceDoor answers PlaceRecord rows, here their names |
| `CreateCtx` | `create_stages` | ctx.createStages (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::create_stages | a RefCell<IndexMap> field there, a Vec here |
| `GitCtx` | `worktree_mounted` | ctx.worktreeMounted (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `GitCtx` | `machine_exec` | entry.machine.exec (LiveWorkspace.machine) | wsp-core item: wsp_engine::machine::Machine::exec | reached through the entry, not ctx |
| `GitCtx` | `exec_stream_of` | ctx.launchAdapterFor (AgentsArea), ctx.execFactoryFor (KindsArea), execs (RuntimeCore) (workspaces.ts:654-673) | gap: helper over members of three areas (AgentsArea, KindsArea, RuntimeCore.execs) |  |
| `GitCtx` | `queued` | ctx.queued (FoldersArea) | wsp-core FoldersArea (empty) | near-name: wsp-machines SubQueueArea::queued() lists held spawns, a different thing |
| `GitCtx` | `keep_tree` | ctx.keepTree (PullRequestsArea) | wsp-core PullRequestsArea (empty) |  |
| `GitCtx` | `take_pull_request` | ctx.takePullRequest (PullRequestsArea) | wsp-core PullRequestsArea (empty) |  |
| `GitCtx` | `read_lead_of` | ctx.readLeadOf (ReachArea) | wsp-core item: wsp_machines::machines::reach::daemon::ReachArea::read_lead_of |  |
| `GitCtx` | `folder_for` | ctx.folderFor (FoldersArea) | wsp-core FoldersArea (empty) | near-name: WorkspacesApi::folder_for takes an origin and answers FolderFor |
| `GitCtx` | `take_setup_line` | ctx.takeSetupLine (FoldersArea) | wsp-core FoldersArea (empty) | near-name: WorkspacesApi::take_setup_line takes an origin |
| `GitCtx` | `resolve_project` | ctx.projectsDoor.resolve (ProjectsArea.projectsDoor) | wsp-core item: wsp_core::types::api::ProjectsApi::resolve | same function as rt.projects.resolve |
| `GitCtx` | `refuse_recording` | ctx.refuseRecording (RulesArea) | wsp-core RulesArea (empty) |  |
| `GitCtx` | `refuse_relayed` | ctx.refuseRelayed (RulesArea) | wsp-core RulesArea (empty) |  |
| `GitCtx` | `project_folder` | ctx.projectFolder (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `GitCtx` | `worktree_folder` | ctx.worktreeFolder (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `GitCtx` | `worktrees_of` | ctx.worktreesOf (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `GitCtx` | `folders_of` | ctx.foldersOf (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::folders_of |  |
| `GitCtx` | `remove_worktree` | ctx.removeWorktree (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `GitCtx` | `read_checkout` | ctx.readCheckout (ReachArea) | wsp-core item: wsp_machines::machines::reach::daemon::ReachArea::read_checkout | ReachArea answers a bare Option<Checkout>, here a Result of it |
| `GitCtx` | `read_pull_request` | ctx.readPullRequest (PullRequestsArea) | wsp-core PullRequestsArea (empty) |  |
| `GitCtx` | `read_tree` | ctx.readTree (ReachArea) | wsp-core item: wsp_machines::machines::reach::daemon::ReachArea::read_tree | ReachArea answers Option<TreeFact>, here () |
| `GitCtx` | `label_of` | labelOf(entry, threadId) (types/internal.ts free fn) | wsp-core item: wsp_core::types::internal::label_of | item takes the record name and thread id |
| `GitCtx` | `open_thread` | ctx.openThread (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `GitCtx` | `default_agent_of` | ctx.defaultAgentOf (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `GitCtx` | `agent_off` | ctx.agentOff (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `GitCtx` | `has_adapter` | adapters[named] !== undefined (RuntimeCore.adapters) | opts: RuntimeOptions.adapters | helper: presence of a key |
| `GitCtx` | `drafter` | ctx.launchAdapterFor, ctx.catalogOn (AgentsArea) and smallestModel (harness-catalog.ts free fn) | gap: helper over two AgentsArea members and the TS free function smallestModel (harness-catalog.ts; wsp-core harness_catalog.rs is empty) |  |
| `GitCtx` | `put_files` | putFiles (@wsp/engine free fn) | wsp-core item: wsp_engine::exec_detached::put::put_files | item takes GuestWrite rows and PutFilesOptions |
| `GitCtx` | `draft_for` | adapter.draftFor (HarnessAdapter, optional) | wsp-core item: wsp_adapters::port::HarnessAdapter::draft_for | optional in TS, a provided method answering Option there; the adapter is picked by ctx.launchAdapterFor (AgentsArea) |
| `GitCtx` | `viewed_marks` | ctx.viewedMarks (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `GitCtx` | `persist_sessions` | ctx.persistSessions (TranscriptsArea) | wsp-core TranscriptsArea (empty) |  |
| `GitCtx` | `pull_request_on` | ctx.pullRequestOn (PullRequestsArea) | wsp-core PullRequestsArea (empty) |  |
| `GitCtx` | `read_page` | ctx.readPage (PullRequestsArea) | wsp-core PullRequestsArea (empty) |  |
| `GitCtx` | `merge_settings` | ctx.mergeSettings (PullRequestsArea) | wsp-core PullRequestsArea (empty) |  |
| `GitCtx` | `read_host` | ctx.readHost (PullRequestsArea) | wsp-core PullRequestsArea (empty) | TS takes a frame builder and a parse; here a Value ask |
| `GitCtx` | `post_as_person` | ctx.postAsPerson (PullRequestsArea) | wsp-core PullRequestsArea (empty) | TS takes a frame builder and a parse; here a Value ask |
| `GitCtx` | `to_first_thread` | ctx.toFirstThread (DaemonArea) | wsp-core item: wsp_machines::machines::daemon::DaemonArea::to_first_thread | DaemonArea answers wsp_core::context::DetachedSend, here the SentTo struct |
| `GitCtx` | `send_detached` | ctx.sendDetached (DaemonArea) | wsp-core item: wsp_machines::machines::daemon::DaemonArea::send_detached | DaemonArea answers wsp_core::context::DetachedSend, here the SentTo struct |
| `GitCtx` | `child_of` | ctx.childOf (BootArea) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::child_of |  |
| `GitCtx` | `update_copy` | ctx.updateCopy (PullRequestsArea) | wsp-core PullRequestsArea (empty) |  |
| `GitCtx` | `drop_page_cache` | githubCache.delete(ctx.pageKey(remote, number)) (workspaces.ts:1003,1137) | gap: helper over RuntimeCore.githubCache (wsp-core item is an empty GitHubCache stand-in) and ctx.pageKey (PullRequestsArea) |  |
| `GitCtx` | `is_local_workspace` | isLocalWorkspace(record) (@wsp/protocol free fn) | wsp-core item: wsp_words::workspace_state::is_local_workspace | item takes Option<&WorkspaceKind> |
| `PhasesCtx` | `unfork` | ctx.unfork (MachinesArea) | wsp-core item: wsp_machines::machines::machines::create::MachinesArea::unfork | answers Result<(), HostError> there, EngineError here |
| `PhasesCtx` | `sub_lost` | ctx.subLost (SubLifeArea) | wsp-core item: wsp_machines::machines::sub_life::timers::SubLifeArea::sub_lost | SubLifeArea answers GoneOutcome, here () |
| `PhasesCtx` | `tell_threads` | ctx.tellThreads (ReachArea) | wsp-core item: wsp_machines::machines::reach::disk::ReachArea::tell_threads | ReachArea takes a line function, here a fixed line |
| `PhasesCtx` | `emit_status` | ctx.emitStatus (ReachArea) | wsp-core item: wsp_machines::machines::reach::disk::ReachArea::emit_status | ReachArea takes ReachState, here a &str |
| `PhasesCtx` | `follow_machine` | ctx.followMachine (MachinesArea) | wsp-core item: wsp_machines::machines::machines::attach::MachinesArea::follow_machine |  |
| `PhasesCtx` | `reach_of` | ctx.reachOf (ReachArea) | wsp-core item: wsp_machines::machines::reach::disk::ReachArea::reach_of | ReachArea answers ReachState, here a String |
| `PhasesCtx` | `nap_with` | ctx.napWith (MachinesArea) | wsp-core item: wsp_machines::machines::machines::phases::MachinesArea::nap_with | MachinesArea takes an optional reason |
| `PhasesCtx` | `refuse_pauseless` | ctx.refusePauseless (RulesArea) | wsp-core RulesArea (empty) |  |
| `PhasesCtx` | `talks_to_tree_on` | ctx.talksToTreeOn (BootArea) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::talks_to_tree_on |  |
| `PhasesCtx` | `refuse_sub_wake` | ctx.refuseSubWake (SubLifeArea) | wsp-core item: wsp_machines::machines::sub_life::timers::SubLifeArea::refuse_sub_wake |  |
| `PhasesCtx` | `recover_gone` | ctx.recoverGone (MachinesArea) | wsp-core item: wsp_machines::machines::machines::phases::MachinesArea::recover_gone |  |
| `PhasesCtx` | `machine_state` | entry.machine.state() (LiveWorkspace.machine) | wsp-core item: wsp_engine::machine::Machine::state | reached through the entry, not ctx |
| `PhasesCtx` | `provider_said` | providerSaid(e) (status.ts free fn) | wsp-core item: wsp_core::status::provider_said |  |
| `PhasesCtx` | `settle_gone` | settled(ctx.settleGone(entry, goneWords(...))) (MachinesArea) | wsp-core item: wsp_machines::machines::machines::phases::MachinesArea::settle_gone | helper: MachinesArea answers GoneOutcome and the area calls settled and goneWords itself |
| `PhasesCtx` | `adopt_pause` | ctx.adoptPause (MachinesArea) | wsp-core item: wsp_machines::machines::machines::phases::MachinesArea::adopt_pause |  |
| `PhasesCtx` | `runs_under_napping` | ctx.runsUnderNapping (MachinesArea) | wsp-core item: wsp_machines::machines::machines::phases::MachinesArea::runs_under_napping |  |
| `PhasesCtx` | `adopt_running` | ctx.adoptRunning (MachinesArea) | wsp-core item: wsp_machines::machines::machines::phases::MachinesArea::adopt_running |  |
| `PhasesCtx` | `proven` | ctx.proven (MachinesArea) | wsp-core item: wsp_machines::machines::machines::phases::MachinesArea::proven |  |
| `PhasesCtx` | `resume_asks` | ctx.lifecycleOf(entry).budgets.resumeAsks (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::lifecycle_of | derived: budgets.resumeAsks read off the Lifecycle |
| `PhasesCtx` | `sleeps` | ctx.sleeps (RuntimeCore) | wsp-core RuntimeCore (empty) |  |
| `PhasesCtx` | `took_the_resume` | tookTheResume(await readsState(machine)) (RuntimeCore) | wsp-core RuntimeCore (empty) | helper over two members, readsState then tookTheResume |
| `PhasesCtx` | `ws_wake` | entry.ws.wake (LiveWorkspace.ws) | wsp-core item: wsp_engine::lifecycle::Workspace::wake | reached through the entry, not ctx |
| `PhasesCtx` | `ws_rebuild` | entry.ws.rebuild (LiveWorkspace.ws) | wsp-core item: wsp_engine::lifecycle::Workspace::rebuild | reached through the entry, not ctx |
| `PhasesCtx` | `ws_upgrade` | entry.ws.upgrade (LiveWorkspace.ws) | wsp-core item: wsp_engine::lifecycle::Workspace::upgrade | reached through the entry; Workspace::upgrade takes a SpecOverride |
| `PhasesCtx` | `ws_current_phase` | entry.ws.currentPhase (LiveWorkspace.ws) | wsp-core item: wsp_engine::lifecycle::Workspace::current_phase | reached through the entry, not ctx |
| `PhasesCtx` | `ws_note_paused` | entry.ws.notePaused (LiveWorkspace.ws) | wsp-core item: wsp_engine::lifecycle::Workspace::note_paused | reached through the entry, not ctx |
| `PhasesCtx` | `arm_late_read` | ctx.armLateRead (MachinesArea) | wsp-core item: wsp_machines::machines::machines::phases::MachinesArea::arm_late_read |  |
| `PhasesCtx` | `can_resume_at` | entry.machine.resumeAt !== undefined (LiveWorkspace.machine) | wsp-core item: wsp_engine::machine::Machine::resume_at | presence of the optional member; a provided method here |
| `PhasesCtx` | `provider_of` | ctx.providerOf (ChannelsArea) | wsp-core ChannelsArea (empty) | TS takes the record, here the entry |
| `PhasesCtx` | `backend_capabilities` | ctx.backendFor(record).limits?.() then .capabilities (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::backend_for | derived: limits and capabilities read off the MachineBackend |
| `PhasesCtx` | `thread_runs` | ctx.threadRuns (ThreadsArea) | wsp-core ThreadsArea (empty) |  |
| `PhasesCtx` | `shape_of` | ctx.shapeOf (RecordsArea) | wsp-core RecordsArea (empty) | TS takes the machine, here the entry |
| `PhasesCtx` | `size_built` | ctx.sizeBuilt (RecordsArea) | wsp-core RecordsArea (empty) |  |
| `PhasesCtx` | `status_now` | ctx.statusNow (ReachArea) | wsp-core item: wsp_machines::machines::reach::disk::ReachArea::status_now |  |
| `PhasesCtx` | `refuse_cannot` | ctx.refuseCannot (RulesArea) | wsp-core RulesArea (empty) | cap is a &str here, a key of Capabilities in TS |
| `PhasesCtx` | `keeps_images` | ctx.keepsImages (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::keeps_images | KindsArea takes the backend, here the entry |
| `PhasesCtx` | `pause_keeps_disk` | ctx.pauseKeepsDisk (KindsArea) | wsp-core item: wsp_machines::machines::kinds::roads::KindsArea::pause_keeps_disk | KindsArea takes the backend, here the entry |
| `PhasesCtx` | `name_given` | ctx.nameGiven (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::name_given | no self |
| `PhasesCtx` | `name_refusal` | ctx.nameRefusal (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::name_refusal |  |
| `PhasesCtx` | `put_look` | ctx.putLook (RecordsArea) | wsp-core RecordsArea (empty) |  |
| `PhasesCtx` | `served_by_its_computer` | ctx.servedByItsComputer (ChannelsArea) | wsp-core ChannelsArea (empty) | TS answers a function or undefined, here Option<String> |
| `PhasesCtx` | `computer_of` | ctx.computerOf (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `PhasesCtx` | `has_deploy_daemon` | ctx.moduleOf(kind).deployDaemon !== undefined | wsp-core item: wsp_core::types::internal::KindModule::deploy_daemon | presence of the optional member; reached through ctx.moduleOf (KindsArea, empty) |
| `PhasesCtx` | `deploy_daemon_on` | ctx.deployDaemonOn (DaemonArea) | wsp-core item: wsp_machines::machines::daemon::DaemonArea::deploy_daemon_on | TS passes the kind's deploy function in; DaemonArea takes the entry alone |
| `PhasesCtx` | `delete_queued` | ctx.deleteQueued (SubQueueArea) | wsp-core item: wsp_machines::machines::sub_queue::SubQueueArea::delete_queued |  |
| `PhasesCtx` | `refuse_changed` | ctx.refuseChanged (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `PhasesCtx` | `path_exists` | existsSync (node:fs, workspaces.ts:576) | gap: node:fs existsSync(); no ctx member and no wsp-core item |  |
| `PhasesCtx` | `cut_off` | cutOff(ctx, entry) (pipe.ts free fn) | wsp-core item: wsp_core::pipe::area::cut_off | item takes (&dyn PipeAreaContext, workspace_id, name), here the entry |
| `PhasesCtx` | `end_sessions` | ctx.endSessions (MachinesArea) | wsp-core item: wsp_machines::machines::machines::attach::MachinesArea::end_sessions |  |
| `PhasesCtx` | `drop_checkpoints` | ctx.dropCheckpoints (FoldersArea) | wsp-core FoldersArea (empty) |  |
| `PhasesCtx` | `remove_worktree_ending` | ctx.removeWorktree(entry, false, {ending: true}) (FoldersArea) | wsp-core FoldersArea (empty) | helper: a fixed call of removeWorktree |
| `PhasesCtx` | `drop_thread_files` | ctx.dropThreadFiles (AgentsArea) | wsp-core AgentsArea (empty) |  |
| `PhasesCtx` | `driven` | kindWords(kind).driven (@wsp/protocol free fn) | wsp-core item: wsp_words::workspace_state::kind_words | the driven field of WorkspaceKindWords |
| `PhasesCtx` | `machine_kill` | entry.machine.kill() (LiveWorkspace.machine) | wsp-core item: wsp_engine::machine::Machine::kill | reached through the entry, not ctx |
| `PhasesCtx` | `drop_workspace` | ctx.drop (MachinesArea) | wsp-core item: wsp_machines::machines::machines::attach::MachinesArea::drop | TS name is drop, here drop_workspace |
| `PhasesCtx` | `read_gone` | readGone(ctx.backendFor(record), machine.id) (@wsp/engine free fn) | gap: TS free function readGone (@wsp/engine); no Rust item in wsp-engine or wsp-core | reads ctx.backendFor (KindsArea) as well |
| `PhasesCtx` | `idle_touch` | ctx.idle.touch (MachinesArea.idle) | wsp-core item: wsp_core::idle::IdlePolicy::touch |  |
| `PhasesCtx` | `has_restart_daemon` | ctx.moduleOf(kind).restartDaemon !== undefined | wsp-core item: wsp_core::types::internal::KindModule::restart_daemon | presence of the optional member; reached through ctx.moduleOf (KindsArea, empty) |
| `PhasesCtx` | `restart_daemon_of` | ctx.moduleOf(kind).restartDaemon(entry) | wsp-core item: wsp_core::types::internal::KindModule::restart_daemon | reached through ctx.moduleOf (KindsArea, empty) |
| `PhasesCtx` | `shared_daemon` | ctx.moduleOf(kind).sharedDaemon | wsp-core item: wsp_core::types::internal::KindModule::shared_daemon | reached through ctx.moduleOf (KindsArea, empty) |
| `PhasesCtx` | `shared_daemon_replaced` | ctx.sharedDaemonReplaced (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `PhasesCtx` | `forget_polled_reach` | ctx.polledReach.delete(id) (ReachArea) | wsp-core item: wsp_machines::machines::reach::ReachArea::polled_reach | helper over the map |
| `PhasesCtx` | `push_status` | ctx.pushStatus (ReachArea) | wsp-core item: wsp_machines::machines::reach::disk::ReachArea::push_status |  |
| `PhasesCtx` | `forget_failed_create` | ctx.failedCreates.delete(id) (CreateArea) | wsp-core item: wsp_machines::machines::create::CreateArea::failed_creates | helper over the map |
| `ReviewCtx` | `project_by_repo` | ctx.projectByRepo (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `ReviewCtx` | `issue_of` | ctx.issueOf (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `ReviewCtx` | `pull_request_of` | ctx.pullRequestOf (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `ReviewCtx` | `from_of` | ctx.fromOf (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `ReviewCtx` | `picks_hold` | ctx.picksHold (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `ReviewCtx` | `workspace_from` | ctx.workspaceFrom (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `ReviewCtx` | `open_with` | ctx.openWith (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `ReviewCtx` | `read_only_of` | ctx.readOnlyOf (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `ReviewCtx` | `reviewers` | ctx.reviewers (StartFromArea) | wsp-core StartFromArea (empty) |  |
| `ReviewCtx` | `drop_page_cache` | githubCache.delete(ctx.pageKey(remote, number)) (workspaces.ts:1003,1137) | gap: helper over RuntimeCore.githubCache (wsp-core item is an empty GitHubCache stand-in) and ctx.pageKey (PullRequestsArea) | declared on GitCtx too |
| `ReviewCtx` | `reread_pull_request` | void ctx.readPullRequest(entry, true) (PullRequestsArea) | wsp-core PullRequestsArea (empty) | helper: started, not awaited |
| `ReviewCtx` | `label_of` | labelOf(entry, threadId) (types/internal.ts free fn) | wsp-core item: wsp_core::types::internal::label_of | item takes the record name and thread id; declared on GitCtx too |
| `ReviewCtx` | `daemon_road` | ctx.moduleOf(kind).daemonRoad | wsp-core item: wsp_core::types::internal::KindModule::daemon_road | reached through ctx.moduleOf (KindsArea, empty) |
| `ReviewCtx` | `forget_daemon_token` | ctx.daemonTokens.delete(machine.id) (TranscriptsArea) | wsp-core TranscriptsArea (empty) | helper over the map; wsp-core PipeAreaContext::forget_daemon_token is the same call |
| `ReviewCtx` | `shared_daemon` | ctx.moduleOf(kind).sharedDaemon | wsp-core item: wsp_core::types::internal::KindModule::shared_daemon | reached through ctx.moduleOf (KindsArea, empty); declared on PhasesCtx too |
| `ReviewCtx` | `copy_channel` | ctx.threadTagged(id, ctx.copyChannel(entry, onEvent, ctx.WORKSPACE_FRAMES)) (ChannelsArea) | wsp-core ChannelsArea (empty) | helper over three members of the area |
| `ReviewCtx` | `shared_channel` | ctx.killGuarded, ctx.scopedProcs, ctx.rootedPorts, ctx.heldToOwner, ctx.threadTagged, ctx.copyChannel (workspaces.ts:1153-1161) | gap: helper over members of two areas (ChannelsArea and ProcScopesArea) | ProcScopesArea.scopedProcs and scopesHere |
| `ReviewCtx` | `port_pids` | ctx.portPids (ChannelsArea) | wsp-core ChannelsArea (empty) | helper over the map, made once per workspace |
| `ReviewCtx` | `port_roots_moved` | ctx.portRootsMoved (ChannelsArea) | wsp-core ChannelsArea (empty) |  |
| `ReviewCtx` | `guest_road_channel` | ctx.copyChannel(entry, onEvent, ctx.GUEST_ROAD_FRAMES) (ChannelsArea) | wsp-core ChannelsArea (empty) | helper over two members of the area |
| `ReviewCtx` | `served_by_its_computer` | ctx.servedByItsComputer (ChannelsArea) | wsp-core ChannelsArea (empty) | TS answers a function or undefined, here Option<String> |
| `ReviewCtx` | `metrics_on_host` | readingRoad(kind, "metrics") === "host" (@wsp/protocol free fn) | wsp-core item: wsp_words::workspace_state::reading_road | item takes a WorkspaceKind and a KindReading |
| `ReviewCtx` | `machine_word` | machineWord(kind) (@wsp/protocol free fn) | wsp-core item: wsp_words::workspace_state::machine_word |  |
| `ReviewCtx` | `sys_samples` | local?.sysSamples (RuntimeCore.local) | wsp-core item: wsp_core::types::wiring::LocalWiring::sys_samples | answers an Option of the sampler |
| `ReviewCtx` | `kind_port_reach` | ctx.moduleOf(kind).portReach | wsp-core item: wsp_core::types::internal::KindModule::port_reach | reached through ctx.moduleOf (KindsArea, empty) |
| `ReviewCtx` | `ws_port_reach` | entry.ws.portReach (LiveWorkspace.ws) | wsp-core item: wsp_engine::lifecycle::Workspace::port_reach | reached through the entry, not ctx |
| `ReviewCtx` | `ws_remint_port_reach` | entry.ws.remintPortReach (LiveWorkspace.ws) | wsp-core item: wsp_engine::lifecycle::Workspace::remint_port_reach | reached through the entry, not ctx |
| `ReviewCtx` | `probe` | fetch(url, {redirect: "manual", signal}) (node global) and readBodyUpTo (types/internal.ts) | gap: node global fetch(); no wsp-core item | readBodyUpTo is wsp_core::types::internal::read_body_up_to |
| `ReviewCtx` | `refusal_for` | ctx.refusalFor (RulesArea) | wsp-core RulesArea (empty) |  |
| `ReviewCtx` | `reaches_row` | ctx.reachesRow (BootArea) | wsp-core item: wsp_machines::machines::boot::entries::BootArea::reaches_row | BootArea takes a RowAt; here thread and workspace ids |
| `UsageResetDeps` | `store` | ctx.store passed as deps.store (account/usage.ts:293) | wsp-core item: wsp_store::store::Store |  |
| `UsageResetDeps` | `limits` | ledger.limits() (UsageArea.ledger) as deps.limits | wsp-core UsageArea (empty) | ledger is a member of UsageArea, built in account/usage.ts |
| `UsageResetDeps` | `agent_name` | harnessCatalog(...)?.label as deps.agentName | gap: TS free function harnessCatalog (harness-catalog.ts); wsp-core harness_catalog.rs is empty |  |
| `UsageResetDeps` | `resets` | PLAN_RESETS lookup (adapters.ts const) as deps.resets | gap: TS const PLAN_RESETS (adapters.ts); wsp-core adapters.rs is empty | its codex row is wsp_adapters::codex::reset::CodexPlanResets |
| `UsageResetDeps` | `places` | planPlaces closure (account/usage.ts:228) as deps.places | gap: local closure over placeDoor.list, placeDoor.link, placeDoor.signInsAt, local and clock; not a context member | answers a PlaceOf function |
| `UsageResetDeps` | `road` | planRoad closure (account/usage.ts:241) as deps.road | gap: local closure over setups.launchOf, local.home, placeDoor.list, sharedOn, loginHomeIn and harnessCatalog; not a context member |  |
| `UsageResetDeps` | `run` | resetRun closure (account/usage.ts:208) as deps.run | gap: local closure over placeDoor.exec, placeDoor.reportOf, local.execStream and setups.launchOf; not a context member |  |
| `UsageResetDeps` | `file` | filePlanLimit closure (account/usage.ts:258) as deps.file | gap: local closure over accountOf, ledger.limit and alerts.read; not a context member |  |
| `UsageResetDeps` | `row` | usageAccounts() then find by key (UsageArea.usageAccounts) as deps.row | wsp-core UsageArea (empty) | helper: finds one row of the answer |
| `UsageResetDeps` | `uuid` | randomUUID (node:crypto) as deps.uuid | wsp-core item: wsp_core::ids::Ids::uuid | no ctx member |
| `Resets` | `read_command` | PlanResets.readCommand (adapter-port.ts:577) | wsp-core item: wsp_adapters::port::PlanResets::read_command |  |
| `Resets` | `parse_read` | PlanResets.parseRead | wsp-core item: wsp_adapters::port::PlanResets::parse_read |  |
| `Resets` | `spend_command` | PlanResets.spendCommand | wsp-core item: wsp_adapters::port::PlanResets::spend_command | item answers String; here Result<String, String> because the TS throws on a credit id the adapter will not take |
| `Resets` | `parse_spend` | PlanResets.parseSpend | wsp-core item: wsp_adapters::port::PlanResets::parse_spend | item answers ResetSpend |
| `Resets` | `too_old` | PlanResets.tooOld | wsp-core item: wsp_adapters::port::PlanResets::too_old | item answers &str, here String |

## Counts

331 methods in 14 traits.

## Count blocks

### Account traits

Methods per trait:

| Trait | File | Methods |
|---|---|---|
| `BoxPanelsCtx` | account/box_panels.rs | 10 |
| `ChannelsCtx` | account/channels.rs | 63 |
| `PreferencesCtx` | account/preferences.rs | 35 |
| `ProcScopesCtx` | account/proc_scopes.rs | 11 |
| `PullRequestsCtx` | account/pull_requests.rs | 24 |
| `RulesCtx` | account/rules.rs | 25 |
| `StatusCtx` | account/status.rs | 40 |
| `UsageCtx` | account/usage.rs | 52 |
| Total | | 260 |

Methods per Home kind:

| Home kind | Methods |
|---|---|
| wsp-core area trait (empty) | 154 |
| wsp-core item (also other crates' items) | 67 |
| opts | 7 |
| gap | 32 |
| Total | 260 |

The 67 item rows by crate: wsp-core 58, wsp-engine 3 (`MachineBackend::capabilities`, `Machine::daemon_answers`, `orphans::lost_workspace`), wsp-adapters 3 (`PlanResets::read_command`, `PlanResets::parse_read`, `codex::limits::limit_of`), wsp-catalog 2 (`catalog_agents`, twice), wsp-words 1 (`is_local_workspace`). Of the wsp-core 58: 11 are `wsp_store::store::Store` calls, which the machines table also writes as `wsp-core item:`.

The 7 opts rows: `RuntimeOptions.broker` (1), `state_path` (1), `env` (1), `server_icons` (1), `adapters` (1), `no_machines_line` (1), `agents_reader` (1).

The 32 gap rows by kind: detached task `spawn` 6; node-only call with no ctx member 6 (`host_pid`, `host_parent_pid`, `read_link`, `hostname`, `git_origin`, `home_dir`); TS free function 14; table in adapters.ts with no Rust item 1 (`plan_agents`); helper over several members, or no member at all 5 (`box_panels_ctx`, `confirm`, `read_logs_on`, `sys_history`, `login_home`).

Area homes (the `wsp-core <Area> (empty)` rows), by area, 154 in all:

| Area trait | Methods |
|---|---|
| RuntimeCore | 64 |
| KindsArea | 16 |
| ChannelsArea | 14 |
| ReachArea | 11 |
| UsageArea | 8 |
| BootArea | 7 |
| AgentsArea | 6 |
| DaemonArea | 5 |
| RecordsArea | 5 |
| MachinesArea | 4 |
| ThreadsArea | 4 |
| RulesArea | 3 |
| ProcScopesArea | 2 |
| BuildersArea | 1 |
| CreateArea | 1 |
| FoldersArea | 1 |
| PreferencesArea | 1 |
| SlatesArea | 1 |

28 of those 154 rows point at an area trait that wsp-account itself fills (ChannelsArea 14, UsageArea 8, RulesArea 3, ProcScopesArea 2, PreferencesArea 1), so the account slice calls itself through the context.

### Projects traits

Methods per trait:

| Trait | File | Methods |
|---|---|---|
| `Base` | projects/workspaces/seam.rs | 25 |
| `FoldersCtx` | projects/folders.rs | 24 |
| `ProjectIconsCtx` | projects/images.rs | 9 |
| `ProjectsCtx` | projects/projects.rs | 35 |
| `LandingRoad` | projects/projects/add.rs | 10 |
| `ProjectSourceModule` | projects/projects/add.rs | 1 |
| `MoveRoads` | projects/projects/moves.rs | 14 |
| `StartFromCtx` | projects/start_from.rs | 20 |
| `CreateCtx` | projects/workspaces/create.rs | 43 |
| `GitCtx` | projects/workspaces/git.rs | 41 |
| `PhasesCtx` | projects/workspaces/phases.rs | 64 |
| `ReviewCtx` | projects/workspaces/review.rs | 30 |
| `UsageResetDeps` | usage_reset.rs | 10 |
| `Resets` | usage_reset.rs | 5 |
| Total | | 331 |

Methods per Home kind:

| Home kind | Methods |
|---|---|
| wsp-core area trait (empty) | 120 |
| wsp-core item (also wsp-engine, wsp-wire, wsp-words, wsp-store, wsp-adapters, wsp-machines items) | 165 |
| opts | 9 |
| gap, total | 37 |
| gap: helper over several members, or a closure built in account/usage.ts | 12 |
| gap: members of the LandingRoad object and the ProjectSourceModule (project-landing.ts, project-sources.ts) | 11 |
| gap: TS free function or const, no Rust item | 10 |
| gap: node builtin, no ctx member | 4 |
| Total | 331 |

Of the 165 item rows, 84 are real bodies or fields in wsp-machines (by type: CreateArea 18, KindsArea 17, MachinesArea 15, BootArea 11, ReachArea 10, DaemonArea 6, PlaceDoor 3, SubLifeArea 2, SubSpawnArea 1, SubQueueArea 1); the other 81 are in wsp-core, wsp-engine, wsp-wire, wsp-words, wsp-store, wsp-adapters and wsp-catalog.

Area homes (the `wsp-core <Area> (empty)` rows), by area:

| Area trait | Methods |
|---|---|
| RulesArea | 26 |
| ChannelsArea | 18 |
| FoldersArea | 17 |
| PullRequestsArea | 11 |
| StartFromArea | 10 |
| RuntimeCore | 8 |
| AgentsArea | 6 |
| RecordsArea | 5 |
| BuildersArea | 4 |
| TranscriptsArea | 4 |
| ProjectIconsArea | 3 |
| ThreadsArea | 3 |
| ProjectsArea | 2 |
| UsageArea | 2 |
| PreferencesArea | 1 |
| Total | 120 |
