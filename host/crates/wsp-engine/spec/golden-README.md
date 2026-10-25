# The golden seam (golden-base.ts, golden-tools.ts, golden-import.ts, golden-mcp.ts at a4033dc34)

What provisioning runs on, whatever #2441 rules about the image machinery: the base floor, the tools loop, the tools
and agents plans, and the MCP round. Eight units, one spec each (`spec/golden-<unit>.md`); the Rust lives in
`src/golden_seam.rs` (shared types) and `src/golden_seam/<unit>.rs`, every stub with its TS source as a comment above
it. Out of scope, and not ported: snapshotting, sealing, image records, `planFiles`, `recipeDigest`, the shell plan,
`toolUninstall`, `agentUninstall`, `agentOwning`, `applyPlugins`, `buildPluginSeed`, `usedBytes`, `sweepLeftovers`.

| Unit | TS | Lines |
|---|---|---|
| tools_disk | golden-tools.ts:42-235, 278-285 | ~200 |
| tools_loop | golden-tools.ts:237-345, 374-490 | ~230 |
| base | golden-base.ts (all) | 192 |
| import_roads | golden-import.ts:20-52, 641-1081 | ~420 |
| import_plan | golden-import.ts:1009-1015, 1073-1234 | ~170 |
| import_agents | golden-import.ts:1348-1403, 1422-1534 | ~170 |
| mcp_files | golden-mcp.ts:1-335 | 335 |
| mcp_rows | golden-mcp.ts:337-566 | 230 |

## The witness

`oracle/golden.oracle.ts` runs the TS functions and writes `tests/fixtures/oracle/golden_<unit>.json` (run line in
its header). Every async function runs against a scripted machine: a list of rules `{ has, kind?, seq }`; the first
rule whose `has` is in the call's text (and whose `kind`, when set, is the call's `exec`, `run` or `put`) answers with
its next answer, the last again once they run out; no rule is exit 0 with nothing. An answer is `{ exit, stdout,
stderr, lines, throws }`: `lines` go to the run's `on_line` before it resolves, `throws` rejects with those words.
Every call is recorded `{ kind, text, timeoutMs?, deadlineMs?, unlogged? }`. `tests/golden_common/mod.rs` reads the
same rules, so each `*_hard.rs` test replays the TS's calls and compares every one, its text byte for byte, its
timeout, and its place in the order. `ms` on a tool row is the loop's clock: fixtures carry only that it was set.
`randomBytes` was pinned to `ab` bytes; a test pins a landing's twelve hex digits the same way before comparing.

## Rules all eight share

- **Script text is contract.** Every shell line here is compared byte for byte with the TS's. Nothing is reflowed,
  re-quoted or re-ordered. `shell_quote` is `wsp_wire::types::shell_quote::shell_quote`, the TS's `shellQuote`.
- **Lengths are UTF-16 units** where the TS cuts a string (`slice(0, 160)` in `reasonOf` and in df's junk line).
  A cut that lands inside a surrogate pair writes U+FFFD for the half (coordinator ruling on #2446).
- **JavaScript reads.** `trim`/`trimEnd` are JavaScript's (`wsp_catalog::pending::js_trim` and its sibling), not
  Rust's `char::is_whitespace`; `split(/\s+/)` splits on JavaScript `\s`; `Number(x)` reads hex (`0x10`), exponents
  (`1e3`), `""` as 0 and junk as NaN; `split("\n")` keeps a trailing empty piece.
- **Order.** Lists keep the TS's order everywhere; a `Set` the TS builds is an `IndexSet` here. JSON fixtures compare
  as maps, so key order inside a serialized value is not tested, except where a spec says it is.
- **Errors.** Where the TS throws, the Rust function answers `Err(EngineError::Other { message: <the TS words>, code:
  None })`, or, for a rejected machine call, the machine's own error unchanged. Where the TS catches (`.catch(refused)`,
  `.catch(() => undefined)`), the Rust function never fails. Each signature says which.
- **What survives a host restart: nothing in this seam.** Every function is pure or talks to one machine for one
  call. What it leaves is on the machine (`/etc/profile.d/wsp-golden.sh`, the agents' config files, the installs);
  the next run reads it back fresh (the version read, `present`, the config read).
- **Timeouts.** Every inline exec is bounded at `INLINE_EXEC_MS` (20,000), every guarded run at
  `guard_deadline_ms(limit)` = `(limit + 60) * 1000`; the MCP read runs at `READ_MS` (120,000), unlogged.

## What these units call in other slices

`exec_detached::{INLINE_EXEC_MS, paged_reads, markers_of}` (roads), `land_bytes::land_bytes` (rest),
`dotfiles_presets::{PRELUDE, apt}` (rest; empty on this branch, filled on `rust/2446-engine-rest`), wsp-catalog's
`road_module`, `install_homes`, `install_env`, `road_step`, `base_floor`, `install_after`, `smoke_of`,
`at_catalog_pin`, `below_line`, `catalog_tool_for`, `catalog_entry`, `catalog_id_of_row`, `config_landing`,
`config_write_line`, `config_refusal`, `config_sum`, `under_base`, the McpFormat editors, and wsp-words'
`fmt_bytes`, `name_list`, `listed_name`, `timed_out_line`, `step_retry_line`, `config_unread_line`. Most are wave-1
stubs today; a test that reaches one first panics there, which is still a red test of this seam.
