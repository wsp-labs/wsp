# The provision slice: what its nine units share

Port of `provision-files.ts`, `provision.ts`, `vault.ts`, `provision-mcp.ts` and `copier.ts` at a4033dc34 (2,566 TS
lines). Line numbers in every spec of this slice are `packages/engine/src/<file>:<line>` at that commit.

| Unit | Rust | TS lines | Spec |
|---|---|---|---|
| copier | `src/copier.rs` | copier.ts 1-189 | copier.md |
| vault_export | `src/vault.rs`, `src/vault/export.rs` | vault.ts 1-287 | vault_export.md |
| vault_import | `src/vault/import.rs` | vault.ts 289-539 | vault_import.md |
| files_land | `src/provision_files.rs`, `src/provision_files/land.rs` | provision-files.ts 1-301, 400-510 | files_land.md |
| files_ledger | `src/provision_files/ledger.rs` | provision-files.ts 303-398, 512-647, 798-909 | files_ledger.md |
| files_outside | `src/provision_files/outside.rs` | provision-files.ts 649-796 | files_outside.md |
| provision_mcp | `src/provision_mcp.rs` | provision-mcp.ts 1-378 | provision_mcp.md |
| provision_plan | `src/provision.rs`, `src/provision/plan.rs` | provision.ts 1-250 | provision_plan.md |
| provision_step | `src/provision/step.rs` | provision.ts 252-551 | provision_step.md |

## Rules every unit keeps

- **The TS is the oracle.** Describe it, do not improve it. Anything that looks wrong is an after-port note in the
  unit's spec, never a change.
- **Script text is contract.** Every function that returns a shell script returns the TS's text byte for byte:
  quoting by `wsp_wire::types::shell_quote::shell_quote` (`'` becomes `'\''`), the TS's `\\t`, `\\n`, `\\0` printf
  escapes written as the two characters backslash and letter, never the byte. `oracle/provision.oracle.ts` printed
  each one into `tests/fixtures/oracle/provision.json`; `tests/files_pins.rs` and `tests/provision_pins.rs` compare.
  Two of them are also held to the daemon's fixtures: `landed_files_script("{home}") + "\n"` is
  `daemon/fixtures/contract/landed-files.sh`, `outside_sweep_script("{root}") + "\n"` is `landed-outside.sh`.
- **Order is the TS's.** Arrays keep insertion order; `Map`s are `IndexMap`s in insertion order (a TS `Map.set` on
  an existing key keeps its first place). Rows come in the order the TS pushes them. Sorting, where the TS sorts,
  is by UTF-16 code unit (`Array.prototype.sort` with no comparator), which for these ASCII paths is byte order.
- **Lengths in characters are UTF-16 units** (`slice(0, 300)`, `slice(-500)`): cut on a UTF-16 boundary, keep whole
  characters. `Buffer.byteLength` is UTF-8 bytes.
- **Errors.** A TS `throw new Error(m)` is `EngineError::Other { message: m, code: None }`; `kind: "vaultTooLarge"`
  is `EngineError::VaultTooLarge { message, bytes }`; `kind: "exists"` is `EngineError::Exists { message }`. A TS
  `.catch(() => undefined)` swallows: the Rust drops the `Err` at that same call and nowhere else.
- **Timeouts are the TS's numbers**, passed as `ExecOptions.timeout_ms` (exec) or `RunOptions.deadline_ms` (run).
  `exec` for a short command, `run` for a long one, exactly where the TS uses each: tests record which.
- **Nothing here holds state in memory across calls.** What survives a host restart is on the far computer:
  `place_provision_paths(home).landed` (the list beside the job: one line per path or key wsp owns in the agents'
  homes) and `/opt/wsp/landed` (the list outside the home). The standing read, the landing and the close
  take a path's first line in `landed` (the servers and ownership reads take every line, files_ledger.md F3); every
  read of `/opt/wsp/landed` takes the last line; the tests that pin a second run reading the first run's list are named
  in files_land.md, files_ledger.md and files_outside.md.

## The golden seam

`golden*.ts` waits on the owner's ruling on #2441. What this slice calls of it is declared in `src/golden_seam.rs`
(the crate's one seam, below the machines writer's part) with the TS signature and `todo!()` bodies. A test that
reaches one of those bodies stays red until the golden port, whatever this slice's implementer does:

- provision_step: every `provision_step` test (installBase, installTools, diskUse, pathLine, agentSteps).
- provision_plan: `provision_plan_of`, `apt_need_steps`, `present_steps` (pathLine, viaRoad, agentSteps).
- provision_mcp: every `provision_mcp` and `headless_keys` test (readConfigsCmd, parseConfigs, landConfigs, mcpRows,
  absentCommands, withoutAbsent, scopeRowId, parseMcpId, rewriteString, mcpOpening).
- files_ledger: `machine_server_port`'s read and write, `unmerge_servers` (parseMcpId); files_land:
  `agent_state_file` (parseMcpId).

Each also reads wave 1's crates (`wsp-catalog`, `wsp-words`, `wsp-wire`), whose bodies are `todo!()` on their own
branches until wave 1 lands.

## Siblings this slice calls (other writers' modules)

`crate::exec_detached::{INLINE_EXEC_MS (20,000), GUEST_TMP ("/tmp"), OLD_APPEND_MARKS (".a[0-9a-f]*"), paged_reads,
markers_of}`, `crate::local_backend::memory_here`, `crate::child_exec::run_child`, `crate::land_bytes::{has_byte_road,
land_bytes}`, `crate::errors::{backoff_ms, classify, should_retry}`, `crate::machine_context::apply_machine_context`,
`crate::fetch::{Fetch, platform_fetch}`.
