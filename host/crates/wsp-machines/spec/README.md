# wsp-machines, machines-a slice: specs

The TS host is the oracle: `packages/runtime/src/machines/` at `65f6b89c7` (the dead-code cut, #2441). Every `file:line`
in these specs is at that commit. This slice is `boot.ts`, `machines.ts`, `create.ts`, `daemon.ts`, `thread-ends.ts`
and `brokered.ts` (2,901 lines), split into eleven units of at most about 400 lines.

| Unit | Rust | TS | Spec | Oracle fixture |
|---|---|---|---|---|
| brokered | `src/machines/brokered.rs` | brokered.ts (133) | brokered.md | `brokered.json` |
| thread_ends | `src/machines/thread_ends.rs` | thread-ends.ts (134) | thread_ends.md | `thread_ends.json` |
| daemon | `src/machines/daemon.rs` | daemon.ts (363) | daemon.md | `daemon.json` |
| create | `src/machines/create.rs` | create.ts (294) | create.md | `create.json` |
| machines_create | `src/machines/machines/create.rs` | machines.ts:15-362 | machines_create.md | `machines_creates.json` |
| machines_attach | `src/machines/machines/attach.rs` | machines.ts:270-301,364-587 | machines_attach.md | `machines_phases.json` (drop) |
| machines_phases | `src/machines/machines/phases.rs` | machines.ts:589-899 | machines_phases.md | `machines_phases.json` |
| boot_load | `src/machines/boot/load.rs` | boot.ts:16-326 | boot_load.md | `boot_builders.json`, `boot_hydrate.json`, `boot_reread.json` |
| boot_ready | `src/machines/boot/ready.rs` | boot.ts:328-621 | boot_ready.md | `boot_ready.json` |
| boot_entries | `src/machines/boot/entries.rs` | boot.ts:623-672,1014-1065 | boot_entries.md | `boot_rows.json` (entries) |
| boot_rows | `src/machines/boot/rows.rs` | boot.ts:673-1013 | boot_rows.md | `boot_rows.json` |

Rules every unit here shares:

- **Oracle scenes.** `oracle/*.oracle.ts` drive the real TS module through scenes with a recording fake of the
  runtime context and write every call it makes, in order, with its arguments, to `tests/fixtures/oracle/*.json`. The
  Rust scene tests build the same world from the fixture, run the same act through a fake of the unit's runtime trait
  that records calls in the same shape, and assert the log, the answer and the state after equal the fixture's. Run
  the oracles from the repository root:
  `pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1`.
  An implementer never edits a fixture. The area's calls are compared in strict order; the entries the act writes
  itself (`answer`, `paid`, `mark` and the rest of `MARKERS` in `tests/common/mod.rs`) are compared as their own list,
  since where the TS act resumes against a detached task's first steps is JavaScript's microtask order, which a Rust
  `spawn_local` does not reproduce (chapter 03, rule 3.12).
- **The runtime trait.** wsp-core's context handle is not written yet (`context.rs` is empty on
  `rust/2448-core-types`). Each area takes one trait of its own naming every call it makes on the rest of the runtime,
  in the TS names in Rust case (rule 1.3), with the shared maps handed out as `Rc<RefCell<IndexMap<..>>>`, borrowed
  inside a call and never across an await or a call out (rule 3.3). The wiring crate implements each trait over the real context; the tests implement it with recording fakes.
- **Clock and ids.** Every `Date.now()`, `new Date()` and timer goes through the injected clock (rule 3.15); every
  `randomUUID` and `randomBytes` through the `Ids` trait (rule 3.16). The test `Ids` answers `random_hex(n)` on its
  `k`-th call with byte `k` repeated `n` times, as the oracles mock `randomBytes`.
- **Pids.** Fixtures write `self` for this process, `alive` for a live other process, `dead` for 99,999,999.
- **Record key order.** wsp-core's `WorkspaceRecord` serializes in the view's key order, not the order each TS site
  writes it, so record comparisons are order-blind (`serde_json::Value` equality). Event and frame key order is
  compared exactly.
- **Errors.** A TS `throw Object.assign(new Error(m), {kind})` is `HostError` with that kind and `m` word for word.
- **What survives a restart:** the store rows each spec names; every map, join and timer in these areas is memory only.
