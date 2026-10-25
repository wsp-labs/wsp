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
  of a `WspError` is `EngineError::Provider(w)` with `w.message == m`. A cancelled `AbortSignal` is
  `errors::aborted()`.
- **Clock and draws.** Time and `Math.random()` come from the injected `RetryClock` (`now`, `sleep`, `random`), never
  the system clock, so a test's fake clock decides every wait.

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
