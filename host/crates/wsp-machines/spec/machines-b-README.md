# wsp-machines, machines-b slice: specs

The TS host is the oracle: `packages/runtime/src/machines/` at `65f6b89c7` (the dead-code cut, #2441). Every
`file:line` in these specs is at that commit. Five TS files, 2,764 lines, nine units.

| Unit | Rust | TS | Spec |
|---|---|---|---|
| kinds_roads | `src/machines/kinds/roads.rs` | kinds.ts:26-316, 506-563 | kinds_roads.md |
| kinds_table | `src/machines/kinds/table.rs` | kinds.ts:317-505 | kinds_table.md |
| sub_spawn_checks | `src/machines/sub_spawn/checks.rs` | sub-spawn.ts:25-30, 76-281, 343-412 | sub_spawn_checks.md |
| sub_spawn_machine | `src/machines/sub_spawn/machine.rs` | sub-spawn.ts:32-74, 283-341, 414-621 | sub_spawn_machine.md |
| sub_life_reads | `src/machines/sub_life/reads.rs` | sub-life.ts:1-154 | sub_life_reads.md |
| sub_life_timers | `src/machines/sub_life/timers.rs` | sub-life.ts:156-597 | sub_life_timers.md |
| sub_queue | `src/machines/sub_queue.rs` | sub-queue.ts | sub_queue.md |
| reach_disk | `src/machines/reach/disk.rs` | reach.ts:1-197, 374-379, 476-482 | reach_disk.md |
| reach_daemon | `src/machines/reach/daemon.rs` | reach.ts:199-372, 381-474 | reach_daemon.md |

Rules every unit here shares:

- **The context is a trait per area.** The TS areas read one shared `RuntimeContext`; here each area declares what it
  reads as a trait (`KindsCtx`, `SubSpawnCtx`, `SubLifeCtx`, `SubQueueCtx`, `ReachCtx`) in its module file, and the
  assembled context implements them later. The tests drive `tests/support/` fakes of those traits. A trait method
  never hands out a borrow: state is read and written through `with_*` closures (rule 3.3).
- **One thread.** Areas are `Rc`, not `Send`; bus listeners hold a `Weak` to their area; no lock guards host state
  (rule 3.1). A TS `void p` is a local task spawned through the context; a TS `setImmediate` is a spawned task that
  yields once first.
- **Order.** Every TS `Map`/`Set` is an `IndexMap`/`IndexSet` and every removal `shift_remove` (rule 3.11).
- **Words.** Every sentence comes from wsp-words or wsp-wire by its TS name; no sentence is retyped. `trim()` is
  `js_trim`, a `.slice(-200)` is `wsp_words::js::utf16::tail(s, 200)`, `Math.round` is `js_round`.
- **JSON.** Records and rows are compared as JSON text against the TS key order (rule 4.26).
- **Oracle.** `oracle/machines_b_reads.oracle.ts` runs the TS on the inputs the fixture tests replay; run it from the
  repository root with `pnpm exec vitest run --config host/crates/wsp-machines/oracle/machines_b.vitest.config.ts
  --minWorkers=1 --maxWorkers=1`. An implementer never edits a fixture.
- **Time and ids** come from the context's clock and ids (rules 3.15, 3.16), never the system.
- **What survives a restart:** only the stored records (`workspaces`, `sub-starts`, `creates`, the `sub-sessions`
  blobs). Every map in these areas is memory and starts empty; `arm_subs` and `resume_queue` rebuild from records.
- **The core stand-ins.** `src/machines/core_types.rs` holds what wsp-core will own (#2448, comment 6400338016):
  `WorkspaceRecord`, `LiveWorkspace`, `SessionEntry`, `SubStartRecord`, `KindModule` and `AreaError`. `AreaError`'s
  methods are `todo!()`; whichever unit's implementer lands first fills them from their doc lines. When wsp-core has
  the types, the file becomes `pub use` lines.
