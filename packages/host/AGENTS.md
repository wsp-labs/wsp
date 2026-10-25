# Host

The `wsp` command, its MCP tool server and the host process. Paths are under packages/host/ unless rooted at the repo.

## How it works

A verb is a socket client of the running host (`dialHost()`); only the host holds keys and the runtime, one host per state file.
Each host start rewrites the MCP configs and skill copies naming its state, so a shape already written there must be moved (`refreshServers()`, `refreshSkills()`).
The daemon binary carries a Rust tool server that answers byte for byte like `src/mcp.ts`, held by `test/mcp-record.test.ts`.
A box's agent is root there and can go around anything its daemon enforces, so a limit belongs on the host too.

## Invariants

1. On add, a box's key is held to the pin before a sudo password goes out or anything runs as root; on remove and update `placeSudoReader()` checks the record's key only when sudo asks a password, else known_hosts does: `test/places.test.ts`.
2. Under a folder a recipe ships or digests, a link is followed only to a file: `test/places-add.test.ts`, `test/recipes.test.ts`.
3. No secret lands from a config row: a name like a secret, a token-shaped value or a URL with a login is cut: `test/recipes.test.ts`.
4. The host caps guest sessions per workspace itself: `test/guest.test.ts`.
5. The binary's tool server line always carries `--state`, with `--host` too; old node lines naming this state move at start: `test/mcp-switch.test.ts`.
6. A served host writes into the wsp home it was handed, never the person's: `test/listen.test.ts`.
7. With the cloud off no page, tool or skill line names a cloud: `test/cloud.test.ts`.
8. A host whose program is gone stops on Linux and keeps serving on a Mac; an update in place is not gone: `test/stop-signals.test.ts`, `test/host-lock.test.ts`.

## Traps

- A recursive fs.watch does not follow links: watch a skill at its real path, a file through its folder by name, never the home (#1481, #1482).
- A road that changes what a computer follows emits recipes.changed, or the watch set never moves (#1481).
- A leave or undo that deletes on a box treats a missing, partial or overwritten record as "keep everything"; write records whole, merge a repeat add (#1729).
- `test/memory.test.ts` runs the built dists: rebuild them on the base for a before reading; every bus event sits in a 5000-event ring (#1694).
- Cloud-only words go inside `<!-- cloud -->` spans; run `test/cloud.test.ts` too, outside the parity check (#1700).
- A tool's words, input or answer move the Rust record: rerun `test/mcp-record.test.ts`, run the `WSP_WRITE_RECORD=1` line it prints, read the diff line by line, build words from their functions (#1297, #1585, #1623).
- A test gated on an env variable skips quietly in CI: name its file in a CI step, its variable in `GATES` (`vitest.env.ts`) (#1297).
- Each node process wsp ships calls `connectPatiently()` first, the bin via `runBin()`: node's 250 ms per address drops a slow first one (#2250, #2259).
- The broker reads a machine's workspace with `heldOrCreating()`: get refuses one mid-create, and the create's clone passes the broker (#2266).
- A fake server's sockets and a child's stdin it writes carry an error listener: a child that answers before reading stdin (git http-backend on a GET) exits first, an unhandled EPIPE (#2280, #2267).

## One home for

| Rule | File | Function |
|---|---|---|
| files under a folder a recipe ships, links followed only to files | `src/folder-files.ts` | `folderFiles()` |
| a value shaped like a token | `src/token-shapes.ts` | `tokenShaped()` |
| a name shaped like a secret | `packages/collect/src/detect/shell-rc.ts` | `isSecretName()` |
| the program a process runs is gone | `src/host-lock.ts` | `programGone()` |
| df on an owned computer: the install folder's volume, never the home or /root | `packages/engine/src/golden-tools.ts` | `prefixVolume()` |
| a shell command bounded by killing its group (macOS has no timeout) | `packages/engine/src/machine-context.ts` | `boundedCommand()` |
| a tools probe with its own drain bound | `src/server-tools.ts` | `stdioScript()` |
| the line an agent's config starts the tools with | `src/mcp-install.ts` | `mcpServerSpec()` |
| a file a workspace may write: no link, no wait, a cap | `src/place-report.ts` | `readPlainHere()` |
| where prepare builds an image's builder | `packages/runtime/src/images/golden.ts` | `prepareAt()` |
| an agent config read on a machine; a `ScopeUnread` is there, unread | `packages/engine/src/golden-mcp.ts` | `readConfigsCmd()` |
