# wsp-core: core-types units

The port of `packages/runtime/src/types/` at `65f6b89c7` (3,455 TS lines), split along its seams into eight units.
Each spec is written from the TS code; the TS has no tests of these files of its own (its tests import them for their
constants), so every expected value here was read off the TS by running it: `oracle/core_types.oracle.ts`, under
the repository's vitest, writes `tests/fixtures/core_types/*.json`. Run it from the repository root with
`pnpm exec vitest run --config host/crates/wsp-core/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1`.

| Unit | TS lines | Rust | Spec | Tests |
|---|---|---|---|---|
| events | events.ts, 98 | src/types/events.rs | events.md | tests/events.rs, 22 |
| harness | harness.ts, 290 | src/types/harness.rs | harness.md | tests/harness_env.rs, 8 |
| wiring-records | wiring.ts 1-509, 532-798, 979-1060 | src/types/wiring.rs | wiring-records.md | tests/wiring_records.rs, 17 |
| wiring-helpers | wiring.ts 510-531, 799-1060 | src/types/wiring.rs | wiring-helpers.md | tests/wiring_helpers.rs, 17 |
| internal-records | internal.ts 44-228, 556-863, 1068-1259 | src/types/internal.rs | internal-records.md | tests/internal_records.rs, 106 |
| internal-index | internal.ts 229-554 | src/types/internal.rs | internal-index.md | tests/internal_index.rs, 27 |
| internal-machines | internal.ts 694-738, 864-1066 | src/types/internal.rs | internal-machines.md | tests/internal_machines.rs, 12 |
| api | api.ts, 748 | src/types/api.rs | api.md | none: no behaviour |

`src/types/pending.rs` holds stand-ins for names whose Rust home is another slice's; see wiring-records.md.

Runtime invariants of `packages/runtime/AGENTS.md` held here: 2 (`invariant_2_turn_written_reads_the_newest_stamp_of_each_kind`)
and 5, the index half (`invariant_5_an_index_that_does_not_read_is_none`).

Green before any implementation, by design: the `constants_*` tests and the `record_*` round trips that serde derives
already satisfy. Every other test fails on a `todo!()`.
