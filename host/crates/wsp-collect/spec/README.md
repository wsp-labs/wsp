# wsp-collect specs, detect half

One page per unit, written from the TS at `a3b870a6e`. Paths are relative to `packages/collect/src/detect/` unless they
say otherwise. Each spec's last section maps its rules to the Rust tests that pin them.

| Unit | Rust files | TS lines | TS cases | Rust tests |
|---|---|---|---|---|
| common | `src/detect/common.rs` | common.ts (95) | none of its own | `tests/detect_common.rs` |
| apt_tools | `src/detect/apt.rs`, `src/detect/tools.rs` | apt.ts (94), tools.ts (263) | apt 7, tools 23 | `tests/detect_apt.rs`, `tests/detect_tools.rs` |
| commands | `src/detect/commands.rs` | commands.ts (185) | 10 | `tests/detect_commands.rs` |
| logins | `src/detect/logins.rs` | logins.ts (183) | 14 rows of cases | `tests/detect_logins.rs` |
| terminal | `src/detect/terminal.rs` | terminal.ts (174) | 13 rows of cases | `tests/detect_terminal.rs` |
| plugins | `src/detect/plugins.rs` | plugins.ts (120) | 8 | `tests/detect_plugins.rs` |
| skills | `src/detect/skills.rs` | skills.ts (286) | skills 14, skills-script 3 | `tests/detect_skills.rs` |
| mcp_args | `src/detect/mcp_args.rs` | mcp.ts:1-290 | part of mcp's 16 | `tests/detect_mcp_args.rs` |
| mcp | `src/detect/mcp.rs` | mcp.ts:292-464 | part of mcp's 16 | `tests/detect_mcp.rs` |
| shell_rc | `src/detect/shell_rc.rs` | shell-rc.ts:1-205, 525-563 | part of shell-rc's 37 | `tests/detect_shell_rc.rs` |
| shell_rc_strip | `src/detect/shell_rc_strip.rs` | shell-rc.ts:205-523 | part of shell-rc's 37 | `tests/detect_shell_rc_strip.rs` |

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
