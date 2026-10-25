# harness: the adapter context and the login environments

TS: `packages/runtime/src/types/harness.ts` (290 lines) at `65f6b89c7`. Rust: `src/types/harness.rs`. Tests:
`tests/harness_env.rs` (8), against `tests/fixtures/core_types/harness.json`.

## What is not here

`HarnessAdapter`, `HarnessSession`, `HarnessStartOptions`, `HarnessWarmOptions` and `KeptAgent` (harness.ts:103-285)
were ported with the adapters in `wsp-adapters/src/port.rs` and are re-exported, never defined twice. Their rules are
that crate's. They are `Send + Sync` and `Arc`, unlike the rest of wsp-core (chapter 03, rule 3.1): an adapter's
handles cross into the core as they are. A ruling on whether the adapter port moves to `Rc` is the map's, not this
unit's.

## HarnessAdapterContext (harness.ts:32-73)

A plain struct the runtime builds per session start with the workspace's machine at that moment (it changes on wake
and upgrade, harness.ts:287). `home` and `login_stands` are called with an agent id. `env` and `vault` are ordered
maps; `vault` is empty, not absent, where the host wired no vault. `brokered`, `codex_broker`, `project_key` and
`launch` are absent everywhere but where their comment says. No behaviour of its own, so no test.

`HarnessAdapterFactory` is `Arc<dyn Fn(HarnessAdapterContext) -> Arc<dyn HarnessAdapter>>`, one per agent id in
`RuntimeOptions::adapters`. `StartPicksAsked` (harness.ts:290) is a plain struct.

## The login environments (harness.ts:78-101)

Each is an ordered map (`Env`, an `IndexMap`), built fresh on each call, keys in the order of the TS spreads. A
later spread of a key already present overwrites its value in its first place, as an object spread does.

1. `machine_sandbox_env()`: `IS_SANDBOX=1`, `DISABLE_AUTOUPDATER=1`.
2. `guest_login_env()`: `HOME`, `USER` (engine `GUEST_USER_ENV`: `/root`, `root`), `PATH` = engine `TOOLS_PATH`,
   then the two sandbox keys.
3. `place_login_env()`: `HOME`, `USER`, `PATH` = wire `PLACE_WORKSPACE_PATH`, the two sandbox keys, then
   `install_env(install_homes(Some(TOOL_PREFIX)))` from the catalog, in its order (PNPM_HOME through GOBIN, 12 keys).
4. `login_env_on(place, npm_bin)`: the guest's map where `place` is None, the place's otherwise. **`Some("")` is a
   place** (the TS tests `place === undefined`). With `npm_bin`, the same map with `PATH` replaced in its own slot by
   `on_npm_bin(Some(npm_bin), PATH)`, which puts `/opt/wsp-agents/bin` and then the npm folder ahead of it.

Pinned by `machine_sandbox_env_is_the_sandbox_flag_then_the_autoupdater_flag`,
`guest_login_env_is_home_user_path_then_the_sandbox_flags`,
`place_login_env_puts_the_place_path_in_the_guest_path_slot_and_adds_the_tool_homes_last`, the four `login_env_on_*`
tests and `login_env_on_an_empty_place_name_reads_as_a_place_not_the_guest`. Each compares the ordered key-value pairs.

`GUEST_USER_ENV` has no Rust home yet (engine `golden-import.ts:817`); `types/pending.rs` holds it as
`guest_user_env()` until the engine's slice writes it.
