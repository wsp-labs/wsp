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
- **One thread** (the coordinator's ruling on #2448, chapter 03). No engine trait has a `Send` or `Sync` bound and no
  future is required to be `Send`: `#[async_trait(?Send)]`, `LocalBoxFuture` and `LocalBoxStream`, and a callback type is
  a plain `Arc<dyn Fn(..)>`. A test that runs a machine's future in the background uses `LocalSet` and `spawn_local`.
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
  With one, the TS's own rules hold, with one difference: settle (box-backend.ts:760) throws the reason where it is an
  error, else `<what> of <id> was stopped by its caller`; `while_restoring` (:777) the reason, else `the call was stopped by
  its caller`. A Rust cancel carries words, not an `Error`, so both answer the reason as an `Error`'s message (the
  coordinator's ruling 4, comment 6405457065); the TS's two "stopped by its caller" sentences are for a reason that is
  not an `Error`, which no host caller passes. A link resume (link-backend.ts:190) the reason, else `Error(String(reason))`, the reason's words; a fetch
  rejects with the reason itself. `src/cancel.rs`'s `Cancel` is the type every method takes (as `&Cancel`): a clonable
  handle around a `CancellationToken` and the reason of the first cancel, set before the token fires. `cancel(None)` or
  `cancel(Some(reason))` cancels; `reason()` answers the reason or `"This operation was aborted"`; `given_reason()`
  answers `None` where none was passed, which is what the rules above test. `fetch::Abort` carries the caller's `Cancel`
  so a fetch it ended rejects with the reason. The tests that pass no reason assert `aborted()`.

- **Accepted differences** (the coordinator's ruling 5 on #2446, comment 6405457065). Three places where the Rust host
  departs from the TS on purpose, each a judgement call rather than a gap:
  1. *A value is kept to 1,000 levels deep* (`project_state_files.md`, M2a; the coordinator's ruling 2 on #2448). `JSON.parse`
     reads any depth (resolver.ts:219 for a transcript line, gemini.ts:35 for the registry, resolver.ts:122 for a cwd), and
     `JSON.stringify` throws `RangeError: Maximum call stack size exceeded` from about 3,650 levels. The Rust reader has
     no recursion and keeps 1,000 levels, the JSON.parse port's own cap, because serde_json clones a `Value` past about
     1,180 levels and drops it past about 6,500. A line between 1,000 and 3,650 levels deep that must change succeeds in
     node and fails here with that RangeError's words, and the file stays as it was; a registry that deep fails with
     "<file> is not JSON".
  2. *A sealed image's header line is read to 1,000 levels* (`vault_cipher.rs` `read_json`, same ruling 2).
     vault-cipher.ts:50's `JSON.parse` reads any depth. A header nested past 1,000 levels is refused here with "this file is
     not a sealed wsp image: its header is not one wsp wrote", where node reads it and then refuses it for not being a header
     wsp wrote; a header wsp wrote nests under 10 levels, so only a foreign file reaches the difference.
  3. *A cancelled link resume drives its frame for 80 ms* (`link_backend.md`, `resume`). The TS lets the request run on
     without end after the caller stops waiting. A Rust future that is dropped cannot, and `LinkMachine` holds no owned
     handle to spawn from, so a cancelled resume goes on driving its frame for 80 ms and then answers the cancel's
     words. The 80 ms is a choice, not the TS's.

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
