# wsp-slates, slice a: the slate store and its runs

Source: `packages/runtime/src/slates.ts` (1,566), `slate-runs.ts` (987), `lazy-slates.ts` (64), `slate-read.ts` (37),
`slate-message.ts` (27), `slate-writes.ts` (25), `slate-refused.ts` (19), 2,725 lines at `65f6b89c7`. Tests:
`packages/runtime/test/slates.test.ts` (42 cases), `slate-runs.test.ts` (28), `lazy-slates.test.ts` (4). Every
`file:line` below is under `packages/runtime/src/` (or `test/`) at `65f6b89c7`.

## Units

| Unit | TS | Rust | Spec | Tests |
|---|---|---|---|---|
| helpers | slate-read.ts, slate-message.ts, slate-writes.ts, slate-refused.ts | `src/slate_{read,message,writes,refused}.rs` | `helpers.md` | `tests/helpers.rs` |
| lazy_slates | lazy-slates.ts | `src/lazy_slates.rs` | `lazy_slates.md` | `tests/lazy_slates.rs` |
| runs_parts | slate-runs.ts:17-242, 363-482, 985-987 | `src/slate_runs/parts.rs` | `runs_parts.md` | `tests/runs_parts.rs` |
| runs_secrets | slate-runs.ts:499-611 | `src/slate_runs/secrets.rs` | `runs_secrets.md` | `tests/runs_secrets.rs` |
| runs_road | slate-runs.ts:244-361, 491-497 | `src/slate_runs/road.rs` | `runs_road.md` | `tests/runs_road.rs` |
| runs_table | slate-runs.ts:613-983 | `src/slate_runs/table.rs` | `runs_table.md` | `tests/runs_table.rs` |
| slates_model | slates.ts:81-292, 686-758 | `src/slates/model.rs` | `slates_model.md` | `tests/slates_model.rs` |
| slates_access | slates.ts:294-555, 1095-1116, 1281-1288, 1450-1480, 1495-1563 | `src/slates/access.rs` | `slates_access.md` | `tests/slates_access.rs` |
| slates_reads | slates.ts:556-685, 1233-1279, 1482-1488 | `src/slates/reads.rs` | `slates_reads.md` | `tests/slates_reads.rs` |
| slates_batch | slates.ts:760-1015 | `src/slates/batch.rs` | `slates_batch.md` | `tests/slates_batch.rs` |
| slates_writes | slates.ts:1017-1231 | `src/slates/writes.rs` | `slates_writes.md` | `tests/slates_writes.rs` |
| slates_acts | slates.ts:1290-1493 | `src/slates/acts.rs` | `slates_acts.md` | `tests/slates_acts.rs` |

`createSlates` is one closure over shared state in the TS. In Rust it is one `SlatesArea` (`src/slates.rs`) holding
`Rc<SlatesInner>`, whose fields are the closure's `let`s, each in a `RefCell` or `Cell`; each unit is an
`impl SlatesInner` block in its own file, and `SlatesArea` implements the `Slates` trait by calling them. Likewise
`createSlateRuns` is `SlateRunsTable` over `Rc<RunsInner>` in `slate_runs/table.rs`.

## What this crate takes from crates that do not have it yet (`src/pending.rs`)

1. **`wsp-slate` has no Rust crate and no writer.** Proposal 1 puts `protocol/src/slate/` less `wire.ts` (5,755
   lines: parser, expressions, validator, batch, sketch, catalog) in `wsp-slate`, layer 2. `pending::slate` holds the
   names this slice calls, with the TS signatures, every body `todo!()`: the types of `slate/types.ts` that the
   stored record holds (`SlateDoc`, `SlateRunDecl`, `SlatePiece`, `SlateProblem`, ...) are ported in full, since the
   store reads them; the functions (`parse_slate`, `validate_slate`, `run_slate_batch`, `sketch_slate`, ...) are
   stubs. **Until `wsp-slate` is ported, every `slates_*` test that writes a slate stays red whatever this crate
   does.** The helpers, lazy_slates and runs units do not need it.
2. **`slate/wire.ts`'s answers and view are not in `wsp-wire`.** `wsp_wire::types::slate_wire` holds the three event
   shapes only. `pending::wire` ports `SlateView`, `SlateAsk`, `SlateApprovalView`, `SlateEmpty`, the four answers
   and the two constants, to move into `slate_wire` at the wire's drift pass.
3. **The other wsp-slates slice's files** (`slate-mcp.ts`, `slate-files.ts`, `slate-box.ts`, `slate-mcp-on.ts`,
   `slate-images.ts`, `slate-sources/`) are not pushed. `pending::siblings` names what `slates.ts` calls of them,
   by the TS names in Rust case; the join turns each into a `pub use` of the sibling's item.
4. **`HostError` has no `code` yet** (ruling 6402177374 point 3). `pending::code_of(&HostError)` stands for
   `e.code`; tests read the code through it.

## Rules every unit holds

- **R1 One thread.** Chapter 03: everything is `Rc` and `RefCell`, futures are not `Send`. No borrow is held across
  an `.await` or while calling out (rule 3.3): `deps.record`, `deps.emit`, `deps.deliver`, the runs' `on_record`,
  `on_line`, `on_timer`, a road's `kill`, a store write. `on_record` in particular re-enters the slates area
  (`moved`, slates.ts:332-335), which queues a batch; the runs table must not hold its own borrow when it calls it.
- **R2 The per-thread chain.** `serial(threadId, run)` (slates.ts:298-302) is rule 03's "chain" form where the
  caller gets its own result: the next task starts when the previous one settles, success or failure; the stored
  tail swallows the failure; the caller sees its own result or error. Two calls on one thread never interleave
  their awaits; calls on two threads do.
- **R3 Time.** Every `deps.now()` and `Date.now()` read and every `setInterval`, `setTimeout` goes through
  `wsp_core::clock::Clock` (rule 3.15). The runs table's `setInterval` becomes a self re-arming `schedule`; its
  delay is clamped as node clamps it (rule 8.18): above 2,147,483,647 ms it fires after 1 ms.
- **R4 Order.** JS `Map` and object key order is insertion order (rule 3.11): `IndexMap` with `shift_remove`, never
  `swap_remove`. Every JSON the area writes or sends keeps the TS's key order (ruling 6402177374 point 1); the
  run records are ordered maps for that reason (`runs_parts.md`). A JS object puts keys that read as array
  indexes first in numeric order (rule 4.26); a record or a values map built with such keys keeps that rule.
- **R4b Whitespace.** `trim`, `trimStart` and `\s` use JavaScript's whitespace set (rule 8.6, `wsp_words::js_trim`).
- **R5 Text.** Lengths and cuts are UTF-16 units where the TS counts `.length` (rule 8.3): `READ_OUTPUT_CHARS`,
  `MESSAGE_CHARS`, the label's 80, `longest`, `cutAt`; a cut may split a surrogate pair as the TS does (8.5).
  `slateBytes` counts UTF-8 bytes (8.2).
- **R6 Processes.** A slate run and a `then` lead their own process group (rule 6.24) and are killed by `SIGKILL`
  to the group, every error ignored (6.36). Tests that kill groups run each in its own process group (#2448 How 4):
  `tests/runs_road.rs` and the process cases of `tests/runs_table.rs` call `support::own_group()` first.
- **R7 What survives a restart** (rules 2.16, 2.17, 6.40): the `slates` collection's records and
  `slates.secrets.json` (secrets marked `keep`). Lost: secrets not kept, "Run once" approvals, every run's live
  state, queues, timers, the write, press, reaction and event buckets, request ids, holds and windows. At start a
  record whose run read `running` becomes `failed` with `the host restarted while it ran` in one batch, its `done`
  reactions fire once (runtime invariant: nothing restarts by itself), held runs are held again, timers armed.
- **R8 Close** (rule 6.40): `close()` stops every run with no reason (SIGKILL to the group, no record written),
  kills reshapes, clears timers, closes the MCP servers and the ledger. A record left `running` in the store is the
  next start's to fail.

## Runtime invariants (`packages/runtime/AGENTS.md`) these files hold

- **I2 "no row written twice"**: the slate's instance is "a run's end fires its `done` once". A completion of an
  instance nobody waits for any more is dropped by its generation (slate-runs.ts:776, 817); a restart fails a
  running record once and a second start finds nothing running (slates.test.ts:212-216). Tests:
  `runs_table::invariant_2_*`, `slates_access::invariant_2_*`.
- **I3 "origin stamped off the token"**: a slate op's thread is read off the caller's token before any argument
  (`targetOf`, slates.ts:485-500); a thread writes only its own slate; a sub-thread has none. Tests:
  `slates_access::invariant_3_*`.
- Invariants 1 (re-open from byte 0), 4 and 5 (a transcript that does not parse) are not held by these files.

## How the tests reach the area

`tests/support/mod.rs` is one `FakeHost`: an in-memory `Store`, a `FakeClock` or the real clock, threads with their
facts (folder, workspace, root, session), `record` and `emit` captured in order, `deliver` capturing prompts and
answering `started`, `under`, `thread_of_token`, an empty source context, `run_env` with `PATH` and `HOME`, a temp
`slates_dir` and `secrets_file`. `turn_ended` and `rewound` are called by the tests where the TS test ran a turn or a
rewind through the runtime: the area sees the same calls the runtime makes. Cases that test the runtime and not the
slate (a start into a missing folder, the model a press's turn starts on) stay with `wsp-threads` and are listed in
`slates_writes.md`.
