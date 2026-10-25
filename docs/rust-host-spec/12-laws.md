# 12 Laws: what checks the source, and what becomes a Rust lint

A law is a check that reads the source rather than running it. The TS repo has many; this chapter sorts them into three piles (survives as a tape or contract test, needs a Rust twin, goes with the TS code) and adds the lints the port itself needs. Every `file:line` is at `a3b870a6e`.

## What runs today

`pnpm check:laws` is `scripts/pre-review.sh --laws` (`package.json:12`), which runs four steps (`scripts/pre-review.sh:39-68,313-354`): the 28 test files of the `LAWS` array (228 cases) plus the three `test-files.sh` always adds (`memory.test.ts`, `stub-script.test.ts`, and `mcp-record.test.ts` under `--tools`); the size check (`scripts/file-size-check.mjs`: no tracked `.ts`, `.tsx`, `.mjs` or `.rs` outside test folders over 1,500 lines, and listed files may not grow, `:12-47`); the test budget (`scripts/test-budget.mjs`: per-package `it` and `test` counts and the daemon's `#[test]` count under a recorded ceiling, `:27-176`); and `cargo fmt --check` on `daemon/` when it changed. Of the 28, 17 read source text. Another 21 test files outside `LAWS` read source text and run only in the full suite or when a change reaches them (the map counted 12 files and 138 cases; the list read for this chapter finds more). `pre-review.sh` itself also checks commit messages (no em or en dash, no `wsp-map#`, no "Generated with", subject under 72 characters), runs the pre-commit hook's rules on a staged copy (no em dash in prose files, `.rs` included; no `wsp-NNN` labels in `.rs`, `.ts`, `.tsx`), and refuses an added `pkill` or `killall` pattern (`:135-192`).

## Proposal 8: the three piles

> **Proposal, awaiting the owner's ruling** (ticket decision 8). Pick: the sorting below. Laws about what crosses the wire or the command line survive unchanged as tapes and contract tests run against the Rust binary; laws about text or structure in any language get a Rust twin in one `cargo xtask laws` command; laws about TypeScript's own shape stay while the TS code they guard stays (the web app keeps `packages/protocol` and `packages/catalog`) and go with it otherwise.

**Pile 1: survives as a wire or contract test, pointed at the Rust binary.**

| Law | Holds | Becomes |
|---|---|---|
| `packages/host/test/parity.test.ts` (29 cases) | every verb has a tool, an output schema and a skill row; every `wsp` line in the skill parses | the Rust command line's contract suite (rule 10.15) |
| `packages/host/test/skill.test.ts` (29) | the skill and the instructions | unchanged: it reads `skills/wsp/SKILL.md` and the record JSON, not TS |
| `packages/host/test/contract.test.ts` (20) | every `--json` run, every exit class | run against the Rust binary |
| `packages/host/test/cloud.test.ts` (17) | with the cloud off nothing names Solari | run against the Rust binary's pages and tools |
| `packages/host/test/pages.test.ts` (15), `verb-words.test.ts` (1) | the help pages' words; every printed `wsp <word>` names a real command | over the Rust binary's output and `wsp-words` strings |
| `packages/host/test/mcp-record.test.ts` (2) | the tool server's record | its source moves to the Rust command line at cutover (rule 10.15) |
| `packages/protocol/test/daemon-contract.test.ts` (15) | the contract fixtures held by the schemas | `wsp-frames`'s own `tests/contract.rs` already does it in Rust |
| `apps/docs-next/test/generated.test.ts` (5) | the command-line doc pages match the verb list | generated from the Rust command line |
| `packages/protocol/test/refusal-shape.test.ts` (3) | every usage refusal has two halves | an xtask over `wsp-words`'s `usage_refusal` calls |

**Pile 2: needs a Rust twin, in `cargo xtask laws`.**

| TS law | Rule it holds | Rust twin |
|---|---|---|
| `packages/host/test/words.test.ts`, `boat-words.test.ts` | no retired word in text a person reads | the same word lists over the string literals of `wsp-words` and every crate's `///` docs |
| `apps/web/test/no-separator-dots.test.ts` | no middle dot or bullet in sources | extended to `.rs` |
| `packages/protocol/test/area-notes.test.ts` | area notes name only files and tests still in the tree | already reads `.rs`; each new crate gets an area note |
| `packages/protocol/test/test-hygiene.test.ts` | no fixed TCP port, no shared home, no wall-clock assertion, a limit on every process-starting test | the same over Rust tests: port 0, a temp home, the fake clock (3.15) |
| `packages/protocol/test/repo-names.test.ts`, `fixture-privacy.test.ts`, `law-list.test.ts` | language-agnostic `git` scans | unchanged |
| `packages/protocol/test/license.test.ts` | every package names AGPL-3.0-only | every crate's `Cargo.toml` names it too |
| `scripts/file-size-check.mjs`, `scripts/test-budget.mjs` | 1,500-line files, test counts | already count `.rs` and `#[test]`; the host's crates are added to the budget |
| `packages/protocol/test/format.test.ts` (its copy-rule cases) | one byte formatter, one copy of the image caps | one `fmt_bytes` in `wsp-words` |
| `packages/protocol/test/app-ports.test.ts` (one case) | no source outside its home spells port 4400 | over `.rs` |
| `packages/protocol/test/test-env.test.ts` | tests take the environment handed in, never read `WSP_` off the process | over Rust tests |
| `packages/host/test/harness-lines.test.ts` | every agent command line is built by `harnessLine()` | in `wsp-adapters`, one builder |
| `packages/host/test/verbs.test.ts` (its import case) | the verb table imports only protocol, catalog, collect and keys | `wsp-cli`'s `Cargo.toml` depends on `wsp-wire` and `wsp-words` alone (rule 1.6), checked by the xtask |
| `packages/protocol/test/slate/extension.test.ts`, `limits.test.ts` | each slate piece's rules in its own entry; limits read from one table | the same over `wsp-slate` |
| the commit-message, em-dash and kill-pattern checks of `pre-review.sh` | language-agnostic | unchanged |

**Pile 3: TypeScript's own shape; stays while the code does.** `packages/protocol/test/person-words.test.ts`, `ts-parse.test.ts`, `import-cycles.test.ts`, `adapter-port.test.ts`; `packages/host/test/entry-graph.test.ts`; `packages/wspx/test/release-notes.test.ts`; and every `apps/web/test` law (`plain-words`, `no-caps`, `design-literals`, `design-pieces`, `has-selectors`, `workspace-keyed-state`, `composer-pickers`, `keybindings`, `muted-ink`, `inner-html`, `project-mark`, `tokens`, `themes`, `font-stacks`, `design-skill`). The web app keeps its laws; the host's TS laws are deleted with the host's TS code at cutover.

## Lints the port adds

12.1. Every host crate MUST build with `cargo clippy -D warnings` and these denied, set once in the workspace's lint table:
- `clippy::await_holding_refcell_ref`, so no state borrow crosses an await (rule 3.3);
- `clippy::disallowed_methods`, listing: `str::trim`, `str::trim_start`, `str::trim_end` (use `js_trim`, rule 8.6); `f64::round` (use `js_round`, rule 8.12); `indexmap::IndexMap::remove` and `swap_remove` (use `shift_remove`, rule 3.11); `std::time::SystemTime::now`, `std::time::Instant::now` and `tokio::time::sleep` outside the clock module (rule 3.15); `uuid::Uuid::new_v4` and `rand::random` outside the ids module (rule 3.16); `std::process::Command::new` and `tokio::process::Command::new` outside the spawn helpers (06, 07);
- `clippy::todo` outside a unit still in its `packet` or `red` state, which the unit gate enforces (13.24).

12.2. `cargo xtask laws` MUST check that every host crate turns on serde_json's `preserve_order` (rule 4.26), and that no host crate depends on a crate in its own layer or above (rule 1.5).

12.3. `cargo xtask laws` MUST be one command with the shape of `pnpm check:laws`: every law runs even after one fails, each prints PASS or FAIL with its reason, and any FAIL exits 1.

12.4. Until cutover both run: `pnpm check:laws` for the TS tree and `cargo xtask laws` for the host's crates; `scripts/pre-review.sh` runs the second when a change touches the host's crates, as it runs `cargo fmt --check` for `daemon/` today.

12.5. A law that is retired MUST be deleted with the code it guards in the same change, and its row removed from `LAWS` (`law-list.test.ts` holds the list to the files).

12.6. A check that ran nothing MUST fail: a unit gate with zero tapes run for its ops, a parity run with an op family with no tapes, a law that matched no files. A green result over nothing is the failure the map's research found most often (tsrs's shortcut passed the whole suite while dropping errors).
