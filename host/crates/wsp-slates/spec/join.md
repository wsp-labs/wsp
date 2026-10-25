# The join of the two wsp-slates slices

Branch `rust/2448-slates-join`, cut from `rust/2448-base` at `43c13eea8`. Merged: `rust/2448-slates-a` (`91a186eff`, slate.ts, slate-runs.ts, lazy-slates.ts and the four helper files, 363 tests) and `rust/2448-slates-b` (`c04f2533b`, slate-mcp, slate-files, slate-images, slate-box, slate-mcp-on, slate-sources and `account/slates.ts`, 180 tests; it had merged slates-a's interface at `5695a4e73`). Both merges were clean. Nothing here is implemented: every `pub` body in `src/` is `todo!()`, and the only other bodies are the optional-member defaults of `SlatesDeps` and `SlateRunsDeps` (`None`, `Ok(())`) and `pending::null_kept` (three lines of serde plumbing).

## What the join settled

- **`pending::siblings` is gone.** `slates.rs` takes `BoxLedger`, `BoxPins`, `HashOn`, `ImageOn`, `SlateMcp`, `Away`, `MissingOnFn`, `McpServerSpec`, `ToolAsks` and `SlateSourceContext` from the files that define them, with slates-b's signatures (its comment 6403094459). `SlatesInner` holds `Rc<SlateMcp>`, `Rc<BoxLedger>` and a `BoxPins` struct, not traits. `Pin` is `slate_files::Pin`.
- **`ToolAsks` is wsp-core's** (`harness_catalog::ToolAsks`, "wsp-slates takes it from here"). `slate_mcp::ToolAsks` re-exports it; slates-b's own enum, with its serde kebab-case derive, is deleted. Nothing read it through serde.
- **`Views`** was defined in `slates.rs` and in `slate_sources.rs`; one definition now (`slate_sources::Views`, re-exported by `slates`).
- **`account/slates.rs` and `slates.rs` wrote five types twice.** One each now:

  | slates-b (`account/slates.rs`) | slates-a (`slates.rs`) | Kept |
  |---|---|---|
  | `SlateThread` | `SlateThreadFacts` | `SlateThreadFacts` (its `tool_asks` is `Option`, as the TS member is optional; the wiring answers `Some`) |
  | `DeliverStart` | `Delivery` | `Delivery` |
  | `Started` and `Delivered` (an enum) | `Delivered` (a struct) and `DeliveryOutcome` | the struct; `start_session` answers it |
  | `ThreadFacts` | none | wsp-core's `ThreadRecord` (it carries `workspace_id`, `harness`, `permission_mode`) |
  | `ServersTarget` | none | wsp-wire's `AgentsTarget` |
  | `HereProject` | none | wsp-wire's `AgentsProject` |

- **`SlatesWiring` implements `SlatesDeps`.** Decision 1 of `account_slates.md` says the join turns the wiring into `createSlates`'s deps. `SlatesAreaCtx` gained `store`, `clock`, `record` and `emit` (the four members the TS literal reads off the context, which the area's ctx trait had not listed) and lost `now` (`clock().now()`). `lazy_slates(Rc::new(wiring), Box::new(create_slates))` is the TS's `lazySlates({...})`. Signatures moved to meet `SlatesDeps`:
  - `wake` and `ready` answer `HostError`; `thread_of_token` answers `Result<String, HostError>` (the TS throws); `own_process` answers `Rc<dyn Fn()>`; `settled_now` answers `bool` and is not async, as `ctx.settledNow` is (`context.ts:522`), and `settled` answers `Option<bool>`.
  - `machine_of` is `Arc<dyn Machine>` on both sides (slates-a had `Rc`; the engine's `Machine` is shared by `Arc`, as slates-b's ledger and the entry hold it).
  - `mcp_server` answers `Option<Result<McpServerSpec, HostError>>` and `missing_on(thread)` answers the closure `Option<MissingOnFn>`, as the deps declare them; the tests call the closure.
  - `secrets_file` and `slates_dir` answer `Option<PathBuf>`.
  - `agents_reader_server` answers `HostError`; `start_session` answers `Result<Delivered, HostError>`.
- **`Entry` is wsp-machines'** (`Rc<RefCell<LiveWorkspace>>`, as wsp-account holds it); `wsp-machines` is a new dependency. The ctx's generic `type Entry` and the eight accessors that only read a field of it (`entry_id`, `entry_name`, `entry_place`, `entry_project`, `entry_running`, `entry_machine`, `entry_checkout`, `entry_pr`) are gone: the wiring reads `record.id`, `record.name`, `record.place`, `record.project`, `record.phase`, `machine`, `checkout` and `pr`. `is_local` and `entry_folder_on_joined` are gone too: wsp-words' `is_local_workspace` and `folder_on_joined` answer them off `record.kind`. A machine on a box is a cloud record that names a place; a folder on a joined computer is kind `place`.
- **Test support is wsp-core's and wsp-store's.** `TestClock` (the wall clock) is `real_clock()`. `ManualClock` is a `FakeClock` plus the list of delays it was asked for (two tests pin the asked delay). `MemStore` is `wsp_store::store::memory_store()` plus a `list` a test can refuse and a count of `list` calls (the lazy-wrapper tests). Seeding a row goes through `put`. `tests/support/entry.rs` builds the real `LiveWorkspace` over a fake `Machine`, as wsp-account's `tests/common/entry.rs` does. `NoSources` implements the real `SlateSourceContext`.
- **Run records stay an ordered JSON map** (`RunRecord`, `serde(transparent)` over `serde_json::Map`): the TS writes a record's keys in a different order on each road, which is accepted. No struct, no parity line.
- **`pending/` is a folder.** `pending/slate.rs` holds every stand-in for `wsp-slate` in one file; `pending/wire.rs` holds `slate/wire.ts`'s view and answers; `pending/mod.rs` holds `code_of` and `null_kept`.

## Real item lacks what a slice needs (stand-in kept, slice test kept)

1. **`wsp-slate` has no crate** (`protocol/src/slate/` less `wire.ts`, 5,755 lines). `pending/slate.rs` holds the types the stored record holds, whole, and the 25 functions `slates.ts` calls, as stubs. The tests that wait on it are counted below.
2. **`wsp-wire` lacks `slate/wire.ts`'s view and answers**: `SlateView`, `SlateAsk`, `SlateCmdAsk`, `SlateApprovalView`, `SlateEmpty`, `SlateWriteAnswer`, `SlateStateAnswer`, `SlateReadAnswer`, `SlateEventAnswer`, `SlateEventOutcome`, `SlatesImageAnswer` (a `Value`), `SLATE_SEND_KEY`, `SLATE_HELD_APPROVAL`. `SlateAsk::key`, `run` and `to_json` are `todo!()`.
3. **`HostError` has no `code`.** The words fix round has not landed in the base. `pending::code_of` stands for `e.code` in 21 test lines; `ServerReadError` stands for the error `agentsRead.server` throws, which `wsp-machines`' `is_server_not_set_up` and `is_server_off` read by `code`.
4. **The slate sentences have no Rust home**: `slate_host_away`, `slate_server_not_set_up`, `slate_description_missing`, `slate_machine_on_box`, `slate_image_missing`, `slate_image_not_a_file`, `slate_image_too_big`, `slate_not_an_image`, `slate_address_refused`, `slate_looks_svg`, `slate_image_source` and `SLATE_IMAGE_MAX_BYTES` (`slate/problems.ts`, `slate/image.ts`). They belong to `wsp-slate`; slates-b's bodies name them only inside `todo!()`.
5. **`wsp-machines`' `ResolvedServer.spec` is the launch `McpServerSpec`** (`command`, `args`, from `views/harness.ts:161`), not slate-mcp.ts's `McpServerSpec` (`transport`, `cwd`, `env`, `secrets`, `open`, `post`, `signIn`), which is what the TS `agentsRead.server` answers. `ReadServer` (the slate spec and a `TargetLogin`) stays. The slate spec lives in this crate and `wsp-machines` cannot name it.
6. **`SETTLE_MS`** (`protocol/general-prefs.ts:38`) has no Rust item; `settle_ms` stays.
7. **`StdioRoad` has two shapes.** This crate's trait (`send`, `listen`, `closed`, `close`) is what `pipe_on` and `post_on` take; `wsp-account`'s `channels::daemon::StdioRoad` is a struct (`send`, `listen`, `channel`). Both stand for slate-mcp-on.ts:278. The runtime join adapts the struct to the trait.
8. **`at_provider`** needs `ctx.places` (a `PlaceBackends`, whose fake would be a whole `MachineBackend`), so the ctx keeps `at_provider(place)` and `entry_rate_usd_per_hour(entry)` (`backendFor(record).pricing`).
9. **`FakeClock`** has no list of asked delays, and **`MemoryStore`** can neither refuse `list` nor count calls: the two wrappers above.

## Threads items

`wsp-threads` is not in the base (`rust/2448-threads-join` is not pushed), so nothing here names its items. What stays with it, as `SlatesDeps` and `SlatesAreaCtx` members and the tests the slices did not port: `deliver` and `start_session` (the session start), `settled_now`, `running_turn`, `latest_on`, `thread_record`, `thread_of_token`, `own_process`, `tree_under`, `on_queued`. Not ported, kept for it (`slates_writes.md` Decisions): the start into a missing folder, and the model, effort and window halves of the press and notify cases.

## Type paths that do not join yet (the runtime join)

- wsp-core's `types::pending::Slates` and `context::SlatesArea` stay empty handle traits. This crate depends on wsp-core, so wsp-core cannot name `slates::Slates`; the runtime join holds `Rc<dyn slates::Slates>`.
- `WorkspaceRecord.project` and `.size` are required in the TS as in Rust, so the wiring's `project !== undefined` and `size === undefined` branches never run. `entry_rate_usd_per_hour` keeps its `Option` as the spec writes it. After the port.

## Counts

One run, `cargo test --no-fail-fast -- --test-threads=3`, through `cargo-slot.sh`: **543 tests, 0 passed, 543 failed, every first panic `not yet implemented`.** No crate test is a constant check, as in wsp-account.

| Slice | Tests | This crate | wsp-core clock | wsp-store `memory_store` | Engine (`Workspace::new`) | wsp-slate stand-in |
|---|---|---|---|---|---|---|
| slates-a | 363 | 122 | 110 | 131 | 0 | 0 |
| slates-b | 180 | 127 | 42 | 0 | 11 | 0 |

Per target (first panic): helpers 52 (43 this crate, 9 clock), lazy_slates 20 (store), runs_parts 48 (this), runs_road 18 (3 this, 15 clock), runs_secrets 41 (10 this, 31 clock), runs_table 46 (clock), slates_model 28 (16 this, 1 clock, 11 store), slates_access 22 (21 store, 1 clock), slates_reads 24 (2 this, 22 store), slates_batch 13 (store), slates_writes 27 (26 store, 1 clock), slates_acts 24 (18 store, 6 clock); slate_files 25 (this), slate_images 29 and slate_images_alloc 1 (this), slate_mcp_client 30 (23 this, 7 clock), slate_mcp_runs 33 (clock), slate_mcp_on 16 (this), slate_box 11 (9 this, 2 clock), slate_sources 16 (this), account_slates 19 (8 this, 11 engine).

The tests that stop first at wsp-core or wsp-store reach this crate's and wsp-slate's `todo!()` next. **No test stops first in `pending/slate.rs`, and these wait on it** (read from the units' TS imports):
- 138 slate-area tests (`slates_model` 28, `slates_access` 22, `slates_reads` 24, `slates_batch` 13, `slates_writes` 27, `slates_acts` 24): every one goes through `create_slates`, which calls the parser, validator, batch and sketch.
- 9 helper tests (`misread_*`): `is_slate_source` and `slate_problem`.
- About 24 slates-b tests say a slate sentence from item 4 above: images 16, MCP client 5, MCP runs 2, account 1. Counted by reading each test's expected text.

Not waiting on it: the runs units (225 less the 9), lazy_slates, files, boxes, sources, `slate_mcp_on`.

## After-port (found while joining)

1. `slates.ts`'s `settled` reads `deps.settled?.(id) === true`; the Rust default `None` reads as not settled. Same result, noted so a port does not make it `Some(false)`.
2. `StdioRoad` is written twice (above). One shape wanted before the runtime join.
3. The TS wiring's `project !== undefined` and `size === undefined` checks are dead code.
