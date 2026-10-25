# wsp-catalog: units, specs and tests

The port of `packages/catalog` at a3b870a6e (7,688 src lines, 321 TS tests), wave 1 of map #2440, ticket #2445. Every
body in `src/` is `todo!()`; each unit's spec is here and its tests are under `tests/`.

## Units

| Unit | Rust files | TS source (lines) | Spec |
|---|---|---|---|
| jsonc | jsonc, jsonc_tree, js_regex, mcp_bearer, mcp_travelled | jsonc.ts, mcp-bearer.ts, mcp-travelled.ts, the jsonc-parser reads (86 plus the parser) | jsonc.md |
| json_edit | json_edit | mcp.ts:458-768 (310) | json_edit.md |
| mcp_refs | mcp, mcp_refs | mcp.ts:15-456 and the format tables (about 470) | mcp_refs.md |
| mcp_json | mcp_json | mcp.ts:470-1255 less the referrer (about 420) | mcp_json.md |
| mcp_json_refer | mcp_json_refer | mcp.ts:906-1087 (180) | mcp_json_refer.md |
| mcp_toml | mcp_toml | mcp.ts:1257-1594, 1737-1757 (360) | mcp_toml.md |
| mcp_toml_refer | mcp_toml_refer, mcp_codex | mcp.ts:1596-1735, mcp-codex.ts (223) | mcp_toml_refer.md |
| mcp_modules | mcp_check, mcp_oauth, mcp_login, mcp_config | mcp-check, mcp-oauth, mcp-login, mcp-config (212) | mcp_modules.md |
| mcp_launch | mcp_launch, config_write | mcp-launch.ts, config-write.ts (447) | mcp_launch.md |
| roads | roads, linux_casks, release_pins, mac_paths, base_image | roads, linux-casks, release-pins, mac-paths, base-image (635, mostly data) | roads.md |
| road_modules | road_modules, road_release | road-modules.ts (684) | road_modules.md |
| signin | signin, signin_rows | signin.ts (594) | signin.md |
| catalog | catalog, catalog_data, bwrap_apparmor | catalog.ts, agents/*.ts, bwrap-apparmor.ts (about 990, half data) | catalog.md |
| skills_context | skills, context | skills.ts, context.ts (400) | skills_context.md |
| hooks | hooks | hooks.ts, codex-hooks.ts (266) | hooks.md |
| plugins | plugins | plugins.ts, plugins-claude.ts (417) | plugins.md |
| plugins_codex | plugins_codex | plugins-codex.ts, codex-app-server.ts (241) | plugins_codex.md |
| seed | seed, first_run, git_hosts | seed, first-run, git-hosts, thread-agents, ecosystems (484) | seed.md |
| marks | marks | marks/*.ts (about 300, mostly svg data) | marks.md |

`pending.rs` stands in for the wsp-wire and wsp-words items the catalog reads (shell quoting, refusal sentences,
version order, the protocol's enums and constants). The merge deletes it and points each use at the real item.

## Tests

397 tests: the 321 TS tests ported one for one (tests/<ts file name>.rs, one `#[ignore]` where the TS skips without
cargo), and 76 hard tests held to fixtures the TS catalog writes (tests/oracle_*.rs, and plugins.rs's
`the_whole_read_is_the_ts_read`). Every one fails on a `todo!()` until its unit is implemented.

The fixtures under `tests/fixtures/` (catalog, roads, road_modules, signin, marks, seed, context, text, plugins) are
written by the oracle scripts under `oracle/` from the TS catalog itself. Regenerate them from the repository root
with:

```
pnpm exec vitest run --config host/crates/wsp-catalog/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
```

The data rows (`catalog_data`, the sign-in rows, the casks, the marks, the seed rows) are copied from these fixtures
by a script, never typed by hand: the research rule "script the copying".

## Running

```
/root/cargo-slot.sh cargo test -j 3 --no-fail-fast   # in host/crates/wsp-catalog, CARGO_TARGET_DIR under /tmp
```
