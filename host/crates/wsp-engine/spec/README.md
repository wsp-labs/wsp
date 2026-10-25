# wsp-engine, machines slice: specs

The TS host is the oracle: `packages/engine/src` at `a4033dc34` (the dead-code cut, #2441). Every `file:line` below is
at that commit. Rules every unit here shares:

- **Oracle fixtures.** `oracle/machines.oracle.ts` runs the TS modules and writes `tests/fixtures/oracle/*.json`; the
  `*_oracle.rs` tests compare against them. Run it from the repository root with
  `pnpm exec vitest run --config host/crates/wsp-engine/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1`.
  An implementer never edits a fixture.
- **Strings are UTF-16.** A TS `.length`, `.slice` or a regex over a string counts UTF-16 units. Where a cap or a cut
  counts characters (box names, snapshot names, daemon lines cut at 120), count UTF-16 units: wsp-words' `js` module
  has the helpers.
- **JSON key order** is the TS object literal's order, and serde_json here keeps insertion order. Where a Rust struct
  serializes a wire type in wsp-wire's schema order instead, the spec says so.
- **Numbers.** A TS `number` read off JSON is `f64` here unless the spec pins an integer. `Number(x)` on a string
  follows JavaScript: `""` and `"  "` are 0, `"1e3"` is 1000, `"0x10"` is 16, `"abc"` is NaN.
- **Nothing survives a host restart** in this slice except what the provider keeps (boxes, named snapshots, a create
  key for 24 hours) and what a guest keeps on its disk (`/etc/wsp/machine-context.json`). Every cache below
  (BoxBackend's limits, refusal log times, backstop timers and failures; a Workspace's routes and phase) is memory
  only and starts empty.
- **Errors.** Every failure is an `EngineError` variant (`src/errors.rs`); a TS `throw Object.assign(new Error(m), w)`
  of a `WspError` is `EngineError::Provider(w)` with `w.message == m`. A cancel: see the rule below.
- **Clock and draws.** Time and `Math.random()` come from the injected `RetryClock` (`now`, `sleep`, `random`), never
  the system clock, so a test's fake clock decides every wait.

- **A cancel and its words** (the coordinator's ruling on #2446, which replaces "every cancel is `aborted()`"). A
  cancel carries an optional reason. With none, every road answers `errors::aborted()` ("This operation was aborted").
  With one, the TS's own rules hold: settle (box-backend.ts:760) throws the reason where it is an error, else
  `<what> of <id> was stopped by its caller`; `while_restoring` (:777) the reason, else `the call was stopped by its
  caller`; a link resume (link-backend.ts:190) the reason, else `Error(String(reason))`, the reason's words; a fetch
  rejects with the reason itself. `tokio_util`'s `CancellationToken` holds no reason, so the type the signatures take
  must change (a token plus an optional reason, set before the cancel fires). The trait methods that take one,
  `Machine::resume` and `resume_at` among them, are shared with the rest and provision slices, so the type is left
  for the coordinator to place once across the crate; until then the tests here pass no reason and assert
  `aborted()`.

| Unit | Rust | TS | Spec |
|---|---|---|---|
| errors | `src/errors.rs`, `src/fetch.rs` | errors.ts (341) | errors.md |
| machine | `src/machine.rs`, `src/no_provider_backend.rs` | machine.ts (313), no-provider-backend.ts (78) | machine.md |
| machine_facts | `src/machine_facts.rs` | machine-facts.ts (118) | machine_facts.md |
| lifecycle | `src/lifecycle.rs` | lifecycle.ts (205) | lifecycle.md |
| context_probe | `src/machine_context/probe.rs`, `src/golden_seam.rs` (df_kb_cmd, parse_versions) | machine-context.ts:1-302 | context_probe.md |
| context_render | `src/machine_context/render.rs` | machine-context.ts:304-547 | context_render.md |
| box_plan | `src/box_backend/plan.rs` | box-backend.ts:23-345 | box_plan.md |
| box_request | `src/box_backend/request.rs` | box-backend.ts:347-786 | box_request.md |
| box_fleet | `src/box_backend/fleet.rs` | box-backend.ts:788-1044 | box_fleet.md |
| box_machine | `src/box_backend/machine.rs` | box-backend.ts:1046-1430 | box_machine.md |

## Wave 2: the pins and their oracles

Each spec now ends in a `## Pins (wave 2)` list: numbered rules read off the TS, each held by a named test. Four
oracle files beside the first two run the TS on the inputs those tests replay, each writing under
`tests/fixtures/scenes/`:

| Oracle | Fixture | Tests |
|---|---|---|
| `box_scenes.oracle.ts` | `box.json`: the Boat API scripted, every request, sleep and warn recorded | `box_machine_scenes.rs`, `box_fleet_scenes.rs` |
| `roads_scenes.oracle.ts` | `roads.json`: the held forward, the poll loop, link and place frames | `ssh_dial_scenes.rs`, `exec_run_scenes.rs`, `link_backend_scenes.rs`, `place_machine_scenes.rs` |
| `lifecycle_scenes.oracle.ts` | `lifecycle.json`: moves that fail half-way | `lifecycle_scenes.rs` |
| `unit_pins.oracle.ts` | `units.json`: edge inputs to pure readers | `<unit>_pins.rs` for errors, machine, machine_facts, context_probe, box_plan, box_request, exec_put, ssh_keys, ssh_machine, local_backend, fake_backend |

Run any of them as the first two are run, naming the file. Each is deterministic but where it says otherwise (the
fake backend's `createdAt`, which its test leaves out).
