# Scoped tool discovery (#2509)

Keep the host's existing thread permissions. A thread's tool server must advertise only tools with a permitted thread operation. Account and computer setup stays on the person's tools. A filtered computer listing must never be described as a connectivity inventory.

- [x] Reproduce scoped discovery advertising `agents`, despite `agents.read` being absent from `THREAD_OPS`.
- [x] Add an explicit thread tool allowlist in `packages/host/src/thread-tools.ts`, consume it in Node registration and record it for the Rust server. Apply Rust filtering to listing and calls, preserving guest and no-slate filters.
- [x] Update shared Computers CLI/MCP wording and skill guidance: listings are scoped; use projects to find eligible registered folders; `on` selects supported cloud providers, not an arbitrary connected computer. Agent choices already come from run's schema and server instructions.
- [x] Verify Node discovery, protocol permission coverage, regenerated record, Rust tools tests, binary parity and CLI/skill/contract suites. Build a new local Classic preview with the updated tool server.

Proposed host note: A thread's MCP discovery uses the thread tool allowlist, recorded for Rust; unavailable management tools are not advertised, and scoped computer listings do not prove a missing computer is disconnected.

## Validation

- Reproduced `agents` in scoped discovery before the change; the new regression failed before the fix.
- 196 focused host/CLI/skill tests passed, 22 existing gated skips.
- 263 built-binary CLI/MCP and guest replay tests passed; owner answer fixtures are unchanged, and 258 guest answers now refuse unavailable tools before host access.
- 152 Rust tests passed, including recorded answers replayed byte for byte. The replay harness keeps fake listeners open; run with `ulimit -n 4096` rather than the shell's default 256.
- Rust Clippy with warnings denied, host typecheck, whitespace and file-size checks passed.
- 555 law tests passed, 22 existing gated skips. Generated CLI, MCP and skill docs refreshed by their generator; the test budget records the three host and one Rust regression cases.
- Signed local Classic preview `0.3.9+boatfix.2506.3` built. Its packaged CLI lists 28 public thread tools; a direct call to `agents` fails without dialing a host. No live provider calls or restart of the person's host were performed.

The Boat fix has also been verified live by the person. This change does not implement isolated project transfer to a joined computer. The existing-project and provider routes are documented distinctly. Rich per-destination availability UI remains outside this focused discovery fix; ticket 2509 stays open for local acceptance and any remaining presentation work.
