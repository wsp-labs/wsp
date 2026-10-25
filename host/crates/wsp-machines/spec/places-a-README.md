# wsp-machines, places slice a: specs

The TS host is the oracle: `packages/runtime/src/places/` at `65f6b89c7` (the dead-code cut, #2441). Every
`file:line` below is at that commit and relative to that folder unless rooted. The slice is `setup.ts`, `link.ts`,
`helpers.ts`, `pane-ports.ts`, `provider-remove.ts`, `readings.ts` and `context.ts` (2,485 lines). The rest of
`places/` (`types`, `records`, `views`, `manage`, and `places.ts` itself) is slice b.

| Unit | Rust | TS | Spec | Tests |
|---|---|---|---|---|
| helpers | `src/places/helpers.rs` | helpers.ts (431) | places_helpers.md | `tests/places_helpers.rs` |
| pane_ports | `src/places/pane_ports.rs`, `listen_near` in helpers.rs | pane-ports.ts (134), helpers.ts:256-281 | places_pane_ports.md | `tests/places_pane_ports.rs` |
| readings | `src/places/readings.rs`, `src/places/context.rs` | readings.ts (94), context.ts (75) | places_readings.md | `tests/places_readings.rs` |
| provider_remove | `src/places/provider_remove.rs` | provider-remove.ts (117) | places_provider_remove.md | `tests/places_provider_remove.rs` |
| setup_area | `src/places/setup.rs` | setup.ts:112-245, 924-1093 | places_setup_area.md | `tests/places_setup_area.rs` |
| setup_run | `src/places/setup/run.rs` | setup.ts:79-111, 246-372, 746-765, 808-922 | places_setup_run.md | `tests/places_setup_run.rs` |
| setup_sign_ins | `src/places/setup/sign_ins.rs` | setup.ts:374-631 | places_setup_sign_ins.md | `tests/places_setup_sign_ins.rs` |
| setup_folders | `src/places/setup/folders.rs` | setup.ts:633-744, 767-806 | places_setup_folders.md | `tests/places_setup_folders.rs` |
| link_door | `src/places/link.rs` (join to load) | link.ts:44-315 | places_link_door.md | `tests/places_link_door.rs` |
| link_reads | `src/places/link.rs` (name_of to the end) | link.ts:316-541 | places_link_reads.md | `tests/places_link_reads.rs` |

The TS tests for this slice are whole-host tests (`createRuntime` plus `serveRuntime` over a websocket:
`test/places-setup.test.ts`, `places-link.test.ts`, `places-facts.test.ts`, `places-remove-provider.test.ts`,
`box-panels.test.ts`, `places-door.test.ts`). The Rust host has no runtime to stand up until wave 5, so each TS case
is ported at the unit's own seam, with fakes for the areas around it (`tests/common/places.rs`), and asserts the
same state the TS case does. Each spec's last section maps every rule to its test and names the TS case it came from.

## Rules every unit shares

- **One thread** (spec 03, rules 3.1 and 3.3). Door state is `Rc` with `RefCell` or `Cell`, never a lock; door
  traits are `#[async_trait(?Send)]`. No `RefCell` borrow is held across an `.await`, an `emit`, another area's
  method, an injected road or a callback. A test that re-enters (a watcher that calls back into the door, a road
  that answers synchronously) panics on a double borrow if this is broken.
- **Insertion order** (rule 3.11). Every TS `Map` or `Set` that is iterated is an `IndexMap` or `IndexSet`, and every
  removal is `shift_remove`. Named where it shows: the readers of a watch (`readings`), the held pane forwards
  (`pane_ports::list`), the waiting wakes (`setup_area`), the rows of a run (`setup_run`).
- **The clock** (rule 3.15). Every time read and every timer goes through `ctx.clock` (`now_ms()`, `schedule(ms, f)`
  answering a cancel). The TS reads `Date.now()` raw at setup.ts:144, :1079, :1084 and link.ts:204, and
  `setTimeout`/`setInterval` at setup.ts:153, :235, :1032, link.ts:184 and helpers.ts:58; the Rust reads the clock at
  each, so the tests drive time with the fake clock and never sleep. `unref` has no Rust meaning and is dropped.
- **JSON key order** is the TS object literal's (serde_json `preserve_order`). Where a test compares a record or a
  row, it compares keys in order: `landed_on` and `taken_report` pin orders the TS spread makes.
- **Errors.** Every failure is `wsp_words::exit::HostError`. A TS `Object.assign(new Error(m), { kind: "usage" })`
  is `HostError::with_kind(m, Kind::Usage)`; `{ kind: "conflict" }` is `Kind::Conflict`. The TS error classes of
  `types.ts` are slice b's constructors and predicates (`is_place_provisioning`, `is_place_absent` from the engine).
  A test compares `message` exactly, and `kind` where the TS stamps one.
- **Strings are UTF-16 where the TS counts.** Nothing in this slice cuts by length except `first_line_of` (a split
  on `\n`, no count), so no unit needs the `js` helpers.
- **What survives a host restart.** Only the place record in the store (`PLACES`), the pending adds (`PENDING`), the
  caps (`CAPS`) and the default mark (`DEFAULT_COLLECTION`). Every map in `PlaceDoorContext` and in each area
  (`live`, `kept`, `backends`, `forwards`, `asking`, the setup area's `waiting`, `closed_at`, `setting`, `syncing`,
  `signing_in`, `skippers`, `landers`, `again`, `sync_timers`, the readings' `watched`, the pane ports' `held` and
  `ended`, the link's `joining`) is memory only and starts empty. A restart is `load()` (link_door) reading the store
  back; the tests that pin it build a second door over the same store.
- **The oracle fixture.** `oracle/helpers.oracle.ts` runs the TS helpers and writes
  `tests/fixtures/oracle/helpers.json`; `places_helpers.rs` reads it. Run from the repository root:
  `pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1`.
  An implementer never edits a fixture or a test.

## Runtime invariants this slice holds (`packages/runtime/AGENTS.md` and `CLAUDE.md`)

- **6. A computer's record changes only in its turn, through `change()`, so no writer drops another's fields**
  (`test/places-setup.test.ts`). Every write in this slice is a `change(place_id, move)` on the records area, whose
  `move` starts from the record as it stands: setup's `write_setup` (setup.ts:193), the attach (link.ts:156),
  `forking_backend`'s facts (link.ts:445), `logins_again` (link.ts:362), `land_sign_in` (link.ts:69) and `load`
  (link.ts:300, :302). Named tests: `setup_area::write_setup_keeps_a_field_another_writer_set_meanwhile`,
  `link_door::attach_keeps_a_field_written_while_it_awaited`, `link_reads::backend_facts_land_on_the_record_as_it_stands`.
- **"One home for: a computer busy with a setup", `settingNow()`** (runtime CLAUDE.md, One home for). The start, the
  update and the fork gate read only `setting_now`. Named test: `setup_area::setting_now_is_the_one_busy_reading`.
- **Invariants 1 to 5** (turn logs, rows written once, origin off the token, gh as the person, transcripts) are not
  held by these files.
