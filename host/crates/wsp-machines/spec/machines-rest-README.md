# wsp-machines, machines-rest slice: specs

The TS host is the oracle: `packages/runtime/src/` at `65f6b89c7` (the dead-code cut, #2441). Every `file:line` in
these specs is at that commit. Eight TS files, 3,092 lines, twelve units.

| Unit | Rust | TS | Spec | Tests |
|---|---|---|---|---|
| images_records | `src/images/records.rs`, `src/images/held.rs` | images/records.ts, images/held.ts | images_records.md | `tests/images_records.rs` |
| builders_places | `src/images/builders/places.rs` | builders.ts:57-78, 190-281 | builders_places.md | `tests/builders_places.rs` |
| builders_life | `src/images/builders/life.rs` | builders.ts:80-189, 283-472 | builders_life.md | `tests/builders_life.rs` |
| golden_prepare | `src/images/golden/prepare.rs` | golden.ts:64-290 | golden_prepare.md | `tests/golden_prepare.rs` |
| golden_upgrade | `src/images/golden/upgrade.rs` | golden.ts:292-459 | golden_upgrade.md | `tests/golden_upgrade.rs` |
| golden_storage | `src/images/golden/storage.rs` | golden.ts:461-611 | golden_storage.md | `tests/golden_storage.rs` |
| image | `src/images/image.rs` | images/image.ts | image.md | `tests/image.rs` |
| agents_read_targets | `src/agents_read/targets.rs` | agents-read.ts:1-311, 351-485, 597-647 | agents_read_targets.md | `tests/agents_read_targets.rs` |
| agents_read_sign_ins | `src/agents_read/sign_ins.rs` | agents-read.ts:312-350, 426-439, 486-605 | agents_read_sign_ins.md | `tests/agents_read_sign_ins.rs` |
| usage_ledger | `src/usage/ledger.rs` | usage.ts:41-109, 162-455 | usage_ledger.md | `tests/usage_ledger.rs` |
| usage_accounts | `src/usage/accounts.rs` | usage.ts:111-160, 181-201, 457-618 | usage_accounts.md | `tests/usage_accounts.rs` |
| branch_of_record | `src/projects/branch_of_record.rs` | projects/branch-of-record.ts | branch_of_record.md | `tests/branch_of_record.rs` |

## Rules every unit here shares

- **One context trait per area** (ruling 4 of #2448 comment 6401022701). `RecordsCtx`, `HeldCtx`, `BuildersCtx`,
  `GoldenCtx`, `ImageCtx`, `BranchCtx` name every `ctx.<member>` the TS area reads, in Rust case. The assembled
  context implements them at the join; tests drive `tests/support/images.rs` and the other fakes. A member another
  area of this slice owns (the golden area calls `ctx.sealEntry`, a builders member) is a context call, not a direct
  call, so each unit is tested alone.
- **The engine's image roads are one trait.** The engine's `golden.ts` and `image-vault.ts` (`buildGolden`,
  `prepareBuilder`, `sealGolden`, `upgradeBuilder`, `applyGoldenImport`, `applyDelta`, `killUntilGone`, `readGone`,
  `dropTemplate`, `promoteVersion`, `templatesOf`, `rollback`, `nextSetupSha`, `refreshPreviewToken`,
  `importImageVault`, `refuseForeignMembers`, `imageHash`, `recipeHash`) have no Rust port yet (`wsp-engine`'s
  `golden_seam.rs` says so). They are `images::GoldenRoads`, reached as `ctx.roads()`. When the engine ports them the
  join implements `GoldenRoads` over the engine's functions. The pure helpers the engine and wire already have
  (`golden_head`, `snapshot_storage`, `retention_plan`, `split_by_owner`, `golden_name`, `billed_by_count`,
  `snapshot_monthly_usd`, `named_count`, `mem_mb_of`, `read_values`, `forks_no_machines`, `builds_images`) are called
  directly.
- **Builders are shared and changed in place.** TS `LiveBuilder` objects are mutated by whoever holds them
  (`same.record.import = ...`, `entry.life = "own"`) and compared by identity (`builders.get(id) !== b`,
  builders.ts:143). Here a builder is `BuilderRef = Rc<RefCell<LiveBuilder>>`, the map `Builders` holds those, and
  identity is `Rc::ptr_eq`. No borrow is held across an await (rule 3.3).
- **Joins are shared futures.** `preparing`, `copyBuilds`, `starting` and `expiring` hold
  `Shared<LocalBoxFuture<'static, Result<T, ImageError>>>` (or `AgentsError`); a second caller awaits the same one, and
  the first caller's removal runs in the future's own `finally`, keyed so a later run is never removed by an older
  one's end.
- **Errors keep their kind.** `ImageError` is `Host(HostError)`, `Engine(EngineError)`, `Stopped(PrepareStoppedError)`
  or `SnapshotFailed { message, builder_state }` (engine golden.ts's `SnapshotFailedError`). Every TS
  `Object.assign(new Error(m), { kind })` is a `HostError` with that kind; a bare `new Error(m)` is `HostError::new(m)`.
  `message()` is `e instanceof Error ? e.message : String(e)`.
- **Words.** Every sentence comes from wsp-words or wsp-wire by its TS name. The sentences written inline in the TS
  (`this host owns no image named ${key} yet`, the builder refusals) are spelled in the interface's doc lines and in
  the tests verbatim.
- **Time.** Every `clock.now()` is the context clock. `new Date().toISOString()` in a label or a held stamp is the
  context clock as an ISO string with milliseconds (rule 3.15; the TS reads the wall clock there, after-port note in
  each spec). `process.pid` is `ctx.pid()`.
- **Order.** Every TS `Map` and `Set` is an `IndexMap` or `IndexSet`; removal is `shift_remove`. `localeCompare` is
  node's ICU root collation (`Intl.Collator()` default), the same need core-types' `find_session_file` has; a test
  pins each place it matters.
- **JSON key order is the TS's** (ruling 1 of #2448 comment 6402177374). A view, a stored record and a report write
  their keys in the TS literal's order, absent keys left out.
- **What survives a restart:** only the store: `goldens`, `golden-recipes`, `images`, the `image-vaults` blobs,
  `builders`, `workspaces`, `workspace-names`, `usage-index`, the `usage-days` and `prices` blobs, `limits`,
  `usage-accounts`, `usage-computers`. Everything else (the builder map, grace timers, the heartbeat, joins, the agents
  reports, sign-ins, the burn, `publicReadAt`, `marketplacesRead`) is memory and starts empty.
- **Proposal 3 drops** `migrateCopies` (records.ts:202-221), the bare-name fallback of `copyOf`, `copyRecipeOf` and
  `dropCopyRecipe` (records.ts:178-190) and the image record backfilled from a copy's head (records.ts:229-242). The
  Rust host never writes a bare key, so none is read.

## Runtime invariants (`packages/runtime/AGENTS.md`)

Invariants 1, 2, 4, 5 and 6 are not held by these files. Invariant 3 (origin stamped off the token) is held in one
place: `branch_of_record`'s op token is scoped to the workspace's own thread, never a caller's word
(`invariant_3_an_op_token_is_scoped_to_the_workspaces_thread_not_the_asker`). Traps held here: #2255, a builder
listed before its create returns, through `claiming` (`golden_prepare::prepare_claims_the_builder_key_per_place`).

## Implementation order

`images_records`, `builders_places`, `builders_life`, then the three golden units and `image` in any order;
`usage_ledger` and `usage_accounts` stand alone; `agents_read_targets` before `agents_read_sign_ins`;
`branch_of_record` stands alone.
