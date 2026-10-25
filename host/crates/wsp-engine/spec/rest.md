# wsp-engine, rest slice: specs

Every file of `packages/engine/src` at `a4033dc34` (the dead-code cut, #2441) outside the machines, roads and
provision slices and outside `golden*` and `image-*`: 3,310 TS lines in 11 units. The shared rules of `README.md`
(oracle fixtures, UTF-16 strings, JSON key order, JS numbers, errors, the injected clock) hold here too, with these:

- **Oracle.** `oracle/rest.oracle.ts` writes `tests/fixtures/oracle/{project-state-scripts,solari,brew-shared,login,
  commands,tool-sizes,small,vault}.json`; the `*_hard.rs` tests compare against them. Run it with the README's line
  and `rest.oracle` at the end.
- **Scripts are text.** Every python3 and shell line a unit builds is compared byte for byte with the TS's; the run
  tests then execute it (python3, sh, sqlite through rusqlite) and check what it did.
- **Golden items** a unit reads (`BUILDER_IDLE_MS`, `templatesOf`, `missingCommands`, the brew table types, the tool
  row helpers) live at the end of `src/golden_seam.rs`, so the golden port replaces one file.
- **Wall clock.** Where the TS reads `Date.now()` as a default argument, the Rust takes `now` (preview tokens,
  previews, describe ages); the Solari backend's waits read its `RetryClock`.
- **Restart.** Nothing here keeps state across a host restart except what it wrote: project-state moves rewrite
  agent stores in place, config writes land a file. The provider slot and the Solari backend are memory only.

| Unit | Rust | TS | Spec | Tests |
|---|---|---|---|---|
| solari_call | `src/solari_backend.rs` (backend) | solari-backend.ts:1-380 | solari_call.md | solari_call, key_check, solari_hard |
| solari_machine | `src/solari_backend.rs` (`SolariMachine`) | solari-backend.ts:382-538 | solari_machine.md | solari_machine, solari_hard |
| project_state_core | `src/project_state/mod.rs` | project-state/index.ts (162) | project_state_core.md | project_state_core, project_state_files, project_state_hard |
| project_state_files | `resolver.rs`, `sqlite.rs` (reads) | resolver.ts (267), sqlite.ts (part) | project_state_files.md | project_state_files, project_state_hard |
| project_state_scripts | `py.rs`, `listing.rs`, `merge.rs`, `sqlite.rs` (scripts) | py.ts, listing.ts, merge.ts, sqlite.ts (part) | project_state_scripts.md | project_state_scripts, project_state_hard |
| project_state_agents | `claude.rs` ... `pi.rs` | the six agent modules (372) | project_state_agents.md | project_state_agents, project_state_scripts |
| child_exec | `child_exec.rs`, `spawn_fed.rs`, `config_here.rs`, `run_env.rs` | 246 | child_exec.md | child_exec, child_exec_hard |
| sweeps | `orphans.rs`, `snapshot_storage.rs`, `snapshot_names.rs`, `labels.rs` | 405 | sweeps.md | sweeps, sweeps_hard |
| tool_sizes | `tool_sizes.rs`, `brew_shared.rs`, golden seam items | 265 | tool_sizes.md | tool_sizes, tool_sizes_hard |
| machine_reads | `target_line.rs`, `daemon_targets.rs`, `land_bytes.rs`, `disk_sync.rs`, `preview.rs` | 296 | machine_reads.md | machine_reads, machine_reads_hard |
| small | `key_check.rs`, `key_fingerprint.rs`, `dotfiles_presets.rs`, `drop_template.rs`, `provider_slot.rs`, `project_install.rs`, `vault_cipher.rs` | 290 | small.md | small, key_check, small_hard |

Shared test fakes: `tests/rest_common/mod.rs` (a closure machine, a scripted fetch, clocks, a warn sink), taken with
`#[path = "rest_common/mod.rs"] mod common;`. Project-state fixtures: `tests/ps_fixtures/mod.rs`.
