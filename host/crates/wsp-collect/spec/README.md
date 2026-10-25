# wsp-collect specs, detect half

One page per unit, written from the TS at `a3b870a6e`. Paths are relative to `packages/collect/src/detect/` unless they
say otherwise. Each spec's last section maps its rules to the Rust tests that pin them.

| Unit | Rust files | TS lines | TS cases | Rust tests (ported + pins) |
|---|---|---|---|---|
| common | `src/detect/common.rs` | common.ts (95) | none of its own | `tests/detect_common.rs`: 0 + 9 |
| apt_tools | `src/detect/apt.rs`, `src/detect/tools.rs` | apt.ts (94), tools.ts (263) | apt 7, tools 22 | `tests/detect_apt.rs`: 7 + 4; `tests/detect_tools.rs`: 22 + 4 |
| commands | `src/detect/commands.rs` | commands.ts (185) | 10 | `tests/detect_commands.rs`: 10 + 4 |
| logins | `src/detect/logins.rs` | logins.ts (183) | 14 | `tests/detect_logins.rs`: 14 + 6 |
| terminal | `src/detect/terminal.rs` | terminal.ts (174) | 13 | `tests/detect_terminal.rs`: 13 + 1 |
| plugins | `src/detect/plugins.rs` | plugins.ts (120) | 8 | `tests/detect_plugins.rs`: 8 + 5 |
| skills | `src/detect/skills.rs` | skills.ts (286) | skills 14, skills-script 3 | `tests/detect_skills.rs`: 17 + 7 |
| mcp_args | `src/detect/mcp_args.rs` | mcp.ts:1-290 | 2 of mcp's 16 | `tests/detect_mcp_args.rs`: 2 + 7 |
| mcp | `src/detect/mcp.rs` | mcp.ts:292-464 | 14 of mcp's 16 | `tests/detect_mcp.rs`: 14 + 2 |
| shell_rc | `src/detect/shell_rc.rs` | shell-rc.ts:1-205, 525-563 | 5 of shell-rc's 37 | `tests/detect_shell_rc.rs`: 5 + 3 |
| shell_rc_strip | `src/detect/shell_rc_strip.rs` | shell-rc.ts:205-523 | 32 of shell-rc's 37 | `tests/detect_shell_rc_strip.rs`: 32 + 2 |
| **all** | | 2,427 | 144 | 198: 144 ported, 54 pins |

An `it.each` is one test looping over its rows, so a TS case is one `it(...)` or one `it.each`. The pins are tests
for spec rules no TS case reached; their expected values were printed by the TS module itself, run under
`node --experimental-strip-types` with its imports stubbed, never guessed.

Tests that stop in a sibling stub before they reach this half's code, and so go green only with that sibling:
`live_host::node_host` (23 of skills and plugins, real-filesystem cases), wsp-catalog's `catalog_agents`/`catalog`
(1 logins, 2 skills, 2 mcp, 1 plugins). Every row test also reads wsp-catalog's data through the unit itself.

Rules every unit follows, so no spec repeats them:

- **Text.** A JS string index, `slice`, `length` and `padEnd` count UTF-16 units; `trim()` is `wsp_words::js::js_trim`;
  `localeCompare` is `wsp_words::js::collate`; `Buffer.byteLength` is UTF-8 bytes (rule 8.2).
- **Regex.** Every regex source in a stub is JavaScript's. `\s` is JavaScript's whitespace set (it holds U+FEFF and
  U+00A0, not U+0085), `\d` and `\w` are ASCII, `[^]` is any character, lookahead and lookbehind exist, `.` stops at
  `\n`, `\r`, U+2028 and U+2029. Compile with fancy-regex after rewriting those, as wsp-catalog's `js_regex` does.
- **JSON.** `JSON.parse` that throws is `None`/empty, never a panic. A parsed object's keys come out in text order in
  serde_json with `preserve_order`, but JavaScript lists integer-like keys (`"0"`, `"12"`) first, ascending, then the
  rest in text order (rule 4.26). Every `Object.entries`/`Object.keys` over parsed JSON below follows JavaScript's order.
- **Rows.** A detector's rows are `ManifestEntry` values; their key order on the wire is manifest.rs's (the zod schema
  order, which `collect()` re-parses into). The TS detectors write other orders; nothing reads them before the parse.
- **Nothing here survives a restart.** No detector keeps state between calls; every answer is read fresh off the
  `Host`. No cache, file or timer is written by any unit in this half.
- **Concurrency.** `Promise.all` is `futures::future::join_all` (or `join!`) on one thread: all are started before any
  is awaited, and results keep the input order whatever finishes first.
