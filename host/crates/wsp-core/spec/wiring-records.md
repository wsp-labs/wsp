# wiring-records: what a host wires in, and the records of a workspace

TS: `packages/runtime/src/types/wiring.ts:1-509, 532-798, 979-1060` (about 790 lines, almost all types) at
`65f6b89c7`. Rust: `src/types/wiring.rs`. Tests: `tests/wiring_records.rs` (17).

This unit is shapes. Its behaviour is the areas' that read the shapes; what it pins is how each stored shape reads
and writes, and which fields live only in memory.

## Serde rules for every stored shape

1. Keys are camelCase and written in the struct's order, which is the TS interface's order. An absent option is left
   out, never written as null, except where a rule below says null.
2. Every number a record stores is a `JsNumber` (or an integer type where the TS value is always a whole count), so a
   whole number writes as `18`, never `18.0`.
3. `WorkspaceRecord` is `Omit<WorkspaceView, "project">` with `project` the project's id: the view's keys in the
   view's order, then `spec`, `idleWindowMs`, `size`, `firstLife` and the record's own optional keys in the TS order
   (wiring.ts:318-402). A record stored before `kind` existed has none and reads `cloud`
   (`workspace_record_with_no_kind_reads_as_cloud`, `workspace_record_with_a_kind_keeps_it`).
4. `idleWindowMs` has three states (wiring.ts:126, 325): absent is the runtime's default, `null` is auto-nap off,
   a number is the window. It reads and writes each back as it came: `Option<Option<JsNumber>>` with wsp-wire's
   `double_option`. Pinned by the three `workspace_record_*_idle_window_*` tests.
5. `CreatedWorkspace` is the view flattened, then `notice` and `sub` (`created_workspace_*`).
6. `GoldenPromotion` is `{golden, version}` then either `{templateId, sharing?}` or `{error}`, untagged
   (wiring.ts:987); pinned by the three `golden_promotion_*` tests with their exact key order.
7. `SubRecord.cloudGone` is `true` where present; written only where set.

## What lives in memory only

`LiveWorkspace` (wiring.ts:416-477) is never stored: a restart rebuilds it from the record and loses every join
(`waking`, `napping`, `adopting`, `deleting`, `checkout_reading`, `tree_reading`, `pr_reading`), every timer cancel
(`late_read`, `pr_poll`), `wake_said`, `delete_said` ("held here, not on the record, so a restart forgets it",
wiring.ts:451-453), `wake_ask`, `resize_to`, `nap_wait_ms`, `unchecked`, `creating` and `generation`. The checkout,
tree and pull request it carries are refreshed from reads; the record keeps its own `checkout`, `tree` and `pr` for
after a restart. A join is a `Shared` future so a second caller awaits the same answer (wiring.ts:427-437).

`record` is `Rc<RefCell<WorkspaceRecord>>`. The TS `attach` makes a new entry that shares the same record object, and
sub-life relies on it: a done that finds `live.get(id) !== entry`, or the record released, knows the workspace was
released while it ran. Every entry `attach` makes for a workspace clones one `Rc`; two entries are the same workspace
when `Rc::ptr_eq` holds on their records. The borrow is held between awaits only (rule 3.3).

`generation` moves with every write of the record (wiring.ts:425-426); the status poll drops a row built under an
older one. The areas own those writes; this unit only declares the field.

## The wiring traits

`ProjectBundler`, `HostFolders`, `HostTerminalConfig`, `HostEditor`, `HostSsh`, `InitDoor`, `RecipeShelf`,
`ProjectLander`, `HereDaemon`, `SeedWiring`, `Broker`, `PlaceBackends`, `SessionHandle`, `OriginStatusApi` and
`UsageDoor` are the TS interfaces one method each, `#[async_trait(?Send)]` where the TS returns a promise. Optional TS
methods are default methods answering None (`Broker::codex_models`, `status`, `revoke`; `HereDaemon::held`).
`Broker::pipe` and `lift` return nothing, so `has_pipe`/`has_lift` say whether the TS had them. A TS function-valued
option is an `Rc<dyn Fn>` field (`LocalWiring::exec_stream`, `env`, `daemon_road`; `RuntimeOptions::vault`).

`RuntimeOptions` (wiring.ts:647-787) holds every option as `Option`, absent where the host wired none. Its defaults
(`places` as one row named "default", `clock` the real one, `providerReadMs` 30 000, `goneConfirmMs` 5 000, the
wake's `lateReadMs` 180 000 and `vaultCapBytes` 200 MiB, `noMachinesLine` NO_PROVIDER_LINE, `cloudProjects` true,
the broker never ready) are applied in `createRuntime` (wsp-serve), not here.

## Names this unit holds for others

`types/pending.rs` holds stand-ins for names whose home is not ported: the clock, the daemon channel, the machine
exec options, the status tracker, the devices door, the GitHub cache and the store (wsp-core's and wsp-store's
other slices), the engine's `BuildGoldenOptions`, `Builder`, `CacheRule`, `Copier`, `GoldenDelta`, `GoldenImport`,
`ImportLedger`, `KillConfirm`, `Sighting` and `GUEST_USER_ENV`, and the area types rule 1.5 moves down from the
crates above (`PlaceDoor`, `PlaceWiring`, `Slates`, `ActivityDoor`, the agents readers and acts, `SignInAsk`,
`SkillAsk`). Each becomes a `pub use` of its home when that lands; nothing else changes.
