# wsp-core: core-rest units

The seventeen files of chapter 01's wsp-core row that no core slice wrote, at `65f6b89c7` (2,051 TS lines), in nine
units. Every expected value is the TS's own: `oracle/core_rest.oracle.ts` runs the files under the repository's vitest
and writes `tests/fixtures/core_rest/*.json`. Run it from the repository root with
`pnpm exec vitest run --config host/crates/wsp-core/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 host/crates/wsp-core/oracle/core_rest.oracle.ts`.
Shared test helpers: `tests/rest/mod.rs`.

| Unit | TS | Rust | Spec | Tests |
|---|---|---|---|---|
| host files | daemon-token 69, daemon-roots 18, host-id 44, github-cache 25 | daemon_token, daemon_roots, host_id, github_cache | host_files.md | host_files 26 |
| setup steps | setup-graph 72, recipe-sync 88 | setup_graph, recipe_sync | setup_steps.md | setup_graph 13, recipe_sync 12 |
| plan alerts | plan-alerts 77 | plan_alerts | plan_alerts.md | plan_alerts 14 |
| tunnels | tunnel 70, place-forward 57 | tunnel, place_forward | tunnels.md | tunnels 11 |
| sockets | sys-socket 57, sign-in-socket 97, forwards 29 | sys_socket, sign_in_socket, forwards | sockets.md | sockets 20 |
| harness catalog | harness-catalog 308 | harness_catalog | harness_catalog.md | harness_catalog 46 |
| agent setups | agent-setup 128, adapters 89, thread-defaults helpers | agent_setup, adapters, protocol_pending | agent_setups.md | agent_setups 10, adapters 11 |
| landing scripts | project-landing :106-352, :483-504 | project_landing (scripts) | landing_scripts.md | landing_scripts 50 |
| landing roads | project-landing :354-481, :506-639, project-sources 124 | project_landing (roads), project_sources | landing_roads.md | landing_roads 16, project_sources 10 |

Implementation order: the clock (core-b's) first, since plan alerts runs on `FakeClock`; then project sources before
the landing roads and scripts, which clone through them; the protocol stand-ins before agent setups and the catalog's
`not_run_refused`.

Runtime invariants of `packages/runtime/AGENTS.md`: none of the six is held by these files. The nearest are pinned as
rules: a stored agent setup writes its keys in the TS's order (agent_setups R4), the host id is written once and never
again (host_files 9), and plan alerts keep nothing over a restart and re-arm through `resume` (plan_alerts A6).
