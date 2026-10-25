# How cli-c's fixtures were made

The expected values in `../<unit>.json` come from running the TS helpers of `turns-help.ts`, `workspaces-help.ts` and
`agents-help.ts` at 65f6b89c7 (a read-only checkout with node_modules at `/root/wsp-vm/wt-2441-cut`), never from reading
them. Each helper is called with a case's arguments against a scripted host, and every frame it sends (as text, key order
included), every line it says or prints, what it returns and what it throws are written down.

```sh
cd /root/wsp-vm/wt-2441-cut   # illustrative
TZ=UTC P=<unit>.probe.ts node_modules/.bin/vitest run --config <this folder>/vitest.config.mjs   # one unit's cases
python3 gentests.py <unit>...     # tests/cli_c/<unit>.rs, one test per case
python3 show.py <unit> [helper]   # one line per case: what the TS did
```

- `lib.ts`: the recorder. `Script` is the host a case scripts: `replies` (an op's answer), `pushed` (frames heard while the op
  is under way), `after` (heard once its reply is back), `closes` (the socket goes during the op under that close code),
  `closesAfter` (it goes once the reply is back), `redials` (what each later dial answers). `<request id>` in a pushed frame
  is the request id the op was sent with; the id is written `<request id>` wherever it shows in what the TS printed. The text
  `<cwd>` in a case's arguments, script and files is the case's own temp folder.
- `<unit>.probe.ts`: a unit's cases and the `runs` that call the TS. `turns_views.ts` builds valid views for two probes.
- `../../cli_c/common.rs` is the Rust side of the same host (`ScriptedHost`, `Deps`, `Sinks`) and `holds`, which compares
  frames, lines, value, error and a hang; `run` keeps what a case left for the hand-written rules tests.
- Replies are built from the real answers recorded for the MCP tools (`daemon/crates/wsp-mcp/tests/answers/<tool>.json`) where
  one exists. A case the TS answers with a ZodError is never recorded (zod's text cannot be matched).
- The paths at the top of `lib.ts` and `vitest.config.mjs` name the checkout and this worktree.
