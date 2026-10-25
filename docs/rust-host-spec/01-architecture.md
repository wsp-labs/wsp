# 01 Architecture and crate layout

What the TypeScript host is made of today, how it starts, where its seams are, and how the Rust host's crates are cut. Every `file:line` is at `a3b870a6e`; facts about the GPUI app are at `origin/gpui/main` `ca56a2a1f` and say so.

## The host as it stands

One Node process per state file, started by `wsp up` (directly, by a verb that needs one, or by the OS service manager the app writes). It holds the state, the keys and the runtime, and serves three kinds of client over one websocket path: the web app inside the Electron window, the GPUI app, and the command line and the MCP tool server, which are thin socket clients (`packages/host/src/verbs.ts:1-10`: "Nothing here reads a key or imports the runtime").

```
 clients                                    the host process (node)                        computers
 +---------------------+   ws /ws    +-------------------------------------------+
 | web app (Electron)  |------------>| app HTTP server :4400  (server.ts:529)    |
 | GPUI app            |------------>| ws upgrade on /ws     (serve.ts:382)      |
 | wsp verbs (CLI)     |------------>|   auth, ops, events                       |
 | wsp-daemon mcp      |------------>| place door :4420      (server.ts:539)     |<---- joined computers
 +---------------------+             |                                           |      (place link, sealed)
                                     | runtime: 35 areas on one context          |
                                     |   (runtime.ts:433-497)                    |
                                     | store: state.db (sqlite, WAL)             |
                                     | engine: backends local, link, ssh, Boat   |----> provider APIs (https)
                                     | adapters: claude, codex, cursor, opencode |
                                     | relay connector (relay-link.ts:658)       |----> relay (https address)
                                     +--------------------+----------------------+
                                                          | daemon link (ws, token)
                                                          v
                                     wsp-daemon (Rust) on this computer, on each box and machine:
                                     files, git, ptys, ports, processes, checkpoints
```

Packages and their import order, leaf first, counted over every `src` import line and agreeing with each `package.json`: `protocol`, `keys` and `own-file` import no wsp package; then `catalog`; then the four adapters, `collect` and `engine`; then `runtime`; then `host`; then `wspx`, the npm entry, which is one import of the host's bin (`packages/wspx/src/bin.ts:5`). The graph is acyclic. Only `packages/runtime/src/adapters.ts:9-12` imports the adapters, and only the host imports `collect`. The web app imports `@wsp/protocol` (341 import lines) and `@wsp/catalog` (52), so both stay a TypeScript contract for the React app after the port.

Inside the runtime and the host, folders are not a cut: every runtime folder sits in one import cycle, because `createRuntime` makes an empty context (`{} as RuntimeContext`) and fills it with `Object.assign(ctx, ...)` 35 times (`packages/runtime/src/runtime.ts:433-497`), and each area reads the others off that context at call time (`packages/runtime/src/context.ts`). At the file level, value imports are acyclic except one pair, `lazy-slates.ts` and `slates.ts`; the cycles are `import type` edges back to `context.ts` (43 of them) and `types/*`. The host has three file-level value cycles: `cli.ts` with `cli/*` and `server.ts`; `places/*` with `relay-link.ts` and `service.ts`; `verbs.ts` with `mcp-install.ts` and `agents-*`.

## How the host starts

From `packages/host/src/bin.ts` to a host that answers, in order. Every later chapter that names a start step points here.

| # | Step | Where |
|---|---|---|
| 1 | `runBin(argv)`; node's sqlite warning silenced; 2 s per address on connect | `bin.ts:6-8`, `entry.ts:23-46` |
| 2 | `cli()` finds the `up` command by the longest matching words | `cli.ts:59-173`, `cli/commands.ts:444-450` |
| 3 | `up`: refuse if a lock is live (`refuseIfServed`), refuse if the state file is a service's, pick ports | `cli/commands.ts:116-139`, `host-lock.ts:249` |
| 4 | Read the login shell's environment into the process, then provider keys by layers: the environment, `.env` beside the state, `.env` in the folder | `cli/service.ts:47-51`, `login-path.ts:146`, `cli/keys.ts:29-76` |
| 5 | Open the state before anything reads it: build `state.db` if absent, WAL, the shape check, import an older `state.json`, start the checkpoint worker (02) | `cli/service.ts:53`, `runtime/src/sqlite-store.ts:141-330` |
| 6 | Place wiring: the host's place key `place-host-key.json` beside the state | `places.ts:32-37` |
| 7 | `makeRuntime`: provider slot, the thread `wsp` shim, the local backend with a lazy local daemon, broker, recipes, agents reader, then `createRuntime` | `cli/wiring.ts:286-384`, `runtime/src/runtime.ts:433-497` |
| 8 | Take the lock `host.lock` beside the state (`{pid, port, address, startedBy, startedAt}`); drop `WSP_STARTED_BY` from the environment | `cli/serve.ts:269-274`, `host-lock.ts:132,259-283` |
| 9 | Read the relay record, admitted devices, the restart road; analytics; init jobs | `cli/serve.ts:278-308` |
| 10 | Mint the host token (24 random bytes, base64url) into `host-token`, mode 0600, before any bind | `cli/serve.ts:311-313`, `host-lock.ts:137`, `own-file/src/index.ts:27` |
| 11 | `startHost`: guest door, callback relay, the app HTTP server, the place door at the app port plus 20, the websocket server on `/ws` in `noServer` mode on both | `server.ts:438-545,641` |
| 12 | Bind the app server (default port 4400, address 127.0.0.1) | `server.ts:693-707`, `protocol/src/app-ports.ts:10` |
| 13 | Start reads at the provider (orphan sweep, builder log, storage line), waited on up to 2 s | `server.ts:717-744,168` |
| 14 | Timers: orphan sweep every 10 min; release check 30 s after start, then every 6 h | `server.ts:745-746`, `release.ts:20-21` |
| 15 | Rewrite the lock with the bound port; refresh MCP configs and skill copies that name this state | `cli/serve.ts:356-364`, `mcp-install.ts:176,260` |
| 16 | Relay tunnel and heartbeat if linked and not `--no-relay`; the recipe watcher (5 s debounce) | `cli/serve.ts:373-387`, `cli/wiring.ts:189-219` |
| 17 | Signal handlers (06), and on Linux a 5 s check that the host's own program file is still there | `cli/commands.ts:145-147`, `cli/serve.ts:108-156` |
| 18 | Handlers for uncaught errors that log and keep serving, installed only once serving | `cli/serve.ts:164-178` |

The local daemon is not started here: it spawns on the first dial (`cli/wiring.ts:114-128`, `local-daemon.ts`). Close runs the reverse (`server.ts:775-795`, `cli/serve.ts:388-398`): recipe watcher, usage, relay, back door, broker, timers, callback relay, the socket server, the HTTP servers, `rt.close()`, analytics, and the lock last.

Rules:

1.1. The Rust host MUST keep this order where a client can see it: the token file exists before the port is bound (step 10 before 12), the lock names the bound port only after the bind (step 15), and the state is open before the socket answers. A client that reads the lock and dials MUST never find a socket that refuses a valid token.

1.2. The Rust host MUST NOT start the local daemon at start; it starts on the first dial, as step 7 wires it (06 has the spawn).

## Seams that already exist

Each is an interface with more than one implementation or a fake the tests inject. They are the Rust host's first traits, because the tests that will be ported already lean on them.

| Seam | TS interface | Implementations | Test fakes |
|---|---|---|---|
| Store | `Store` (`runtime/src/store.ts:9`); `TranscriptRows` (`sqlite-transcripts.ts:35`) | `memoryStore` (`store.ts:76`), `jsonFileStore` (`:155`), `sqliteStore` (`sqlite-store.ts:278`) | `memoryStore` in 20+ files, wrapped to inject faults (`runtime/test/reconcile.test.ts:533`) |
| Harness adapter | `HarnessAdapter` (`runtime/src/types/harness.ts:196`), vocabulary in `protocol/src/adapter-port.ts` (`ExecStream` `:260`) | the four `create*Adapter` (`adapter-claude/src/adapter.ts:627`, `adapter-codex/src/adapter.ts:513`, `adapter-cursor/src/adapter.ts:111`, `adapter-opencode/src/adapter.ts:124`), wired by `HARNESS_ADAPTERS` (`runtime/src/adapters.ts:57`) | scripted agents in `host/test/verbs-fixture.ts:89-329`; `runtime/test/fed-runs.ts` |
| Machine backend | `MachineBackend` (`engine/src/machine.ts:241`), `Machine` (`:92`) | `LocalBackend`, `BoxBackend` (Boat; in the code `box` means Boat, `engine/src/box-backend.ts:1-8`), `SolariBackend`, `LinkBackend` (a joined computer), `placeFolderBackend`, `NoProviderBackend`, `FakeBackend`; `SshMachine` implements `Machine` alone | `runtime/test/stub-backend.ts` (493 lines), `host/test/stub-backend.ts` (301 lines) |
| Daemon channel | `DaemonChannel` (`runtime/src/daemon-channel.ts:21`), `DaemonReach` (`runtime/src/reach.ts:39`) | `openDaemonChannel` (`:72`), `connectDaemon` (`reach.ts:63`) | `host/test/fake-procs.ts`, `host/test/fake-pty-link.ts` |
| Clock | `Clock` (`runtime/src/clock.ts:4`: `now`, `schedule(fn, ms, {unref})`) | `realClock` (`:16`) | `fakeClock()` (`runtime/test/fake-clock.ts:7`) in 48 test files |
| Processes | `ExecStreamFactory` (`protocol/src/adapter-port.ts:300`); `HostStarter` (`host/src/host-start.ts:16`); `ServiceManager` (`host/src/service.ts:85`) | `localExecStream` (`runtime/src/local-exec.ts:437`), `machineExecStream` (`runtime/src/machine-exec.ts:300`) | `host/test/fake-procs.ts` |
| Copy road | `Copier` (`engine/src/copier.ts:15`) | `verbCopier` (`:62`, shells out to the daemon binary) | `fakeCopier` (`:135`, in src) |
| Collect's computer | `Host`, `HostFs`, `HostExec` (`collect/src/host.ts:15-43`) | `nodeHost()` (`collect/src/live-host.ts:258`) | `fakeHost()` (`collect/test/fake-host.ts:27`) |

What is not a seam: the filesystem outside `collect` (`node:fs` in 23 runtime, 67 host and 12 engine files), child processes outside the factories above (`node:child_process` in 28 host files), and HTTP, which each reader takes as its own `fetch` option (`engine/src/box-backend.ts:517`, `host/src/release.ts:115` and a dozen more). 45 `Date.now()` and 38 `setTimeout(` sites in `runtime/src` sit outside the clock. Every injected port is named in `RuntimeOptions` (`runtime/src/types/wiring.ts:670-795`).

1.3. Every seam in the table above MUST be a Rust trait in the crate Proposal 1 gives it, with the TS method names in Rust case, so the TS tests that inject a fake port one for one (13.11).

1.4. A port MUST route time through the clock trait wherever the TS code uses the clock, and MAY route the TS's bare `Date.now()` sites through it too; it MUST NOT add a seam the TS code does not have for any other reason (the TS host is the oracle, and a new seam is a redesign).

## The Rust side today

Seven crates in one workspace under `daemon/` (`daemon/Cargo.toml:1-4`, resolver 2, edition 2021, toolchain 1.97.1 for `x86_64-unknown-linux-musl`):

| Crate | Lines | Holds | Matters to the host because |
|---|---|---|---|
| `wsp-frames` | 6,817 | every daemon request, reply and event as serde types; writes the 200 TS types in `packages/protocol/src/generated` (`daemon/scripts/ts-types.sh`) | the host speaks these frames to every daemon; reused as is |
| `wsp-seal` | 232 | ed25519 proofs, key fingerprints, the place link's seal | the place link and `seal.open` (09) |
| `wsp-runtime` | 17,802 | the workspace manager behind machine ops (youki containers, copy road) | a name the host's crates must not reuse |
| `wsp-daemon` | 35,596 | the daemon's door and op switch | linked into the one binary (#2450) |
| `wsp-guest` | 486 | the `wsp` a process runs inside a machine | |
| `wsp-mcp` | 15,959 | the wsp tools as an MCP server on stdio; already dials the host like the command line (`wsp-mcp/src/client.rs:1-60`, `aim.rs:179-191`) | the Rust host serves it unchanged; its dial is the GPUI app's dial too |
| `wsp-daemon-bin` | 970 | the `wsp-daemon` binary | |

Every tokio runtime in them is current-thread (`daemon/crates/wsp-daemon-bin/src/main.rs:31-35`, `wsp-mcp/src/lib.rs:66`). The lock has tokio 1.53.1, serde_json 1.0.151, tokio-tungstenite 0.30.0, rustls 0.23.44, ring 0.17.14, nix 0.31.3, and no rusqlite, axum or hyper. On `gpui/main`, `apps/desktop-gpui/host-client` (a separate workspace, toolchain 1.99.1) depends on `wsp-mcp` by path and opens the host's `state.db` read-only with rusqlite 0.41 to ask whether a `places` row exists (`gpui/main:apps/desktop-gpui/host-client/src/start.rs:659-661`), so the table names are a cross-process contract today (02, rule 2.4).

## Proposal 1: the crates

> **Proposal, awaiting the owner's ruling** (ticket decision 1). Pick: the fifteen crates below in a new workspace `host/` beside `daemon/`, with path dependencies on `wsp-frames`, `wsp-seal` and `wsp-mcp`. The ticket's list, with three changes: `wsp-runtime` is renamed (the daemon already has one) and split in four along the runtime's areas, `wsp-host` is split in two, and the slate module gets its own crate.

| Layer | Crate | Ported from | TS lines | Depends on |
|---|---|---|---|---|
| 0 | `wsp-wire` | `protocol/src/wire/`, `views/`, the type-only files of `protocol/src/` | about 14,000 | `wsp-frames` |
| 0 | `wsp-store` | `runtime/src/store.ts`, `sqlite-store.ts`, `sqlite-transcripts.ts`, `transcript-reader.ts`, `own-file`, `keys` | 1,248 | |
| 1 | `wsp-words` | `protocol/src/words/`, `format.ts`, `exit.ts` and the sentence files of `protocol/src/` | about 9,000 | `wsp-wire` |
| 2 | `wsp-slate` | `protocol/src/slate/` (parser, expressions, validator, sketch) | 6,061 | `wsp-wire`, `wsp-words` |
| 2 | `wsp-catalog` | `catalog` | 7,688 | `wsp-wire`, `wsp-words` |
| 2 | `wsp-collect` | `collect`, its 8,598-line bottle table written by script | 5,458 | `wsp-catalog` |
| 2 | `wsp-engine` | `engine` | 16,559, less the cut | `wsp-catalog`, `wsp-seal` |
| 2 | `wsp-adapters` | the four adapters and `protocol/src/adapter-port.ts` | 7,890 | `wsp-catalog` |
| 3 | `wsp-core` | the runtime's level 0 and 1 files: `context.ts`, `types/`, `clock.ts`, `status.ts`, `places.ts`, `daemon-channel.ts`, `reach.ts`, `local-exec.ts`, `machine-exec.ts`, `pipe.ts` and the rest of `runtime/src/*.ts` not named below; the state owner (03) | about 12,000 | layer 0 to 2 |
| 4 | `wsp-threads` | `runtime/src/threads/`, `activity.ts`, `agents-read.ts`, conversations | about 8,000 | `wsp-core` |
| 4 | `wsp-machines` | `runtime/src/machines/`, `places/`, `images/` | 12,428 | `wsp-core` |
| 4 | `wsp-account` | `runtime/src/account/`, `projects/`, `usage.ts`, `usage-reset.ts` | about 7,000 | `wsp-core` |
| 4 | `wsp-slates` | `runtime/src/slates.ts`, `slate-*.ts`, `slate-sources/` | 4,998 | `wsp-core`, `wsp-slate` |
| 5 | `wsp-serve` | `runtime/src/serve.ts`, `serve-*.ts`, `runtime.ts`: the op switch and the assembly of the areas | 3,677 | layer 4 |
| 5 | `wsp-cli` | `host/src/verbs/`, `cli/`, `verbs.ts`, `cli.ts`, `skill.ts`, `guest-cli.ts`, the kept `init-layout` toolkit | about 13,800 | `wsp-wire`, `wsp-words` only: a verb is a socket client |
| 6 | `wsp-host` | the serving half of `host/src`: `server*.ts`, `broker/`, `relay*.ts`, `connector.ts`, `pairing.ts`, `host-*.ts`, `service.ts`, `restart.ts`, `local-daemon.ts`, guests, analytics, release | 10,729 | `wsp-serve` |
| 6 | `wsp-setup` | the roads to computers, agents and sign-ins: `place*.ts`, `places/`, `doctor*.ts`, `providers*.ts`, `agents*.ts`, `skills*`, `servers*`, `recipe*`, `mcp-install.ts`, `chatgpt/`, `anthropic/`, `codexlogin/`, `gitroad/`, `ghroad/`, `packguard/` | 17,880 | `wsp-serve`, `wsp-collect` |
| 7 | the `wsp` binary | `entry.ts`, `bin.ts` | small | everything, plus the daemon crates at #2450 |

Line counts are `wc -l` over the named files at `a3b870a6e`, before the cut (#2441) removes about 9,456 lines, most of it from `host/src/init*.ts` (8,764 lines, of which the kept terminal toolkit is 2,057). The large-module pilot's Rust came to 2.1 to 2.4 times the TS source (#2440, comment 6396255198), not the 1.3 the map's summary assumed, so at about 2.25 the crates land between 3,000 and 40,000 Rust lines and five pass the ticket's 20,000: `wsp-wire` (about 31,000), `wsp-engine` (about 32,000 after the cut), `wsp-cli` (about 31,000), `wsp-machines` (about 28,000) and `wsp-setup` (about 40,000). Each has a second cut named now, taken when its port passes 25,000 Rust lines: `wsp-wire` into `wsp-wire` (`wire/`) and `wsp-views` (`views/`); `wsp-engine` into `wsp-engine` and `wsp-clouds` (the Boat and Solari backends, 1,968 TS lines plus their helpers); `wsp-cli` into `wsp-cli` and `wsp-help` (the `*-help.ts` pages, 2,702 lines); `wsp-machines` into `wsp-machines` and `wsp-places` (`places/`, 4,975 lines); `wsp-setup` into `wsp-computers` (`place*.ts`, `places/`, `doctor*.ts`) and `wsp-agents` (agents, skills, servers, recipes and the sign-in roads). That makes up to 20 crates, each under about 25,000 Rust lines.

Why this cut:

- **The area cycle is broken by the owner, not by folders.** The runtime's folders all import each other through the shared context. In Rust the context becomes a trait in `wsp-core` (the handle to the state owner, 03), each area crate implements its handlers against that trait, and only `wsp-serve` sees every area. An edit in `wsp-threads` rebuilds `wsp-threads`, `wsp-serve` and the crates above it, never `wsp-machines`.
- **The command line sits beside the runtime, not above it.** `verbs.ts` imports nothing of the runtime (`verbs.ts:1-10`), so `wsp-cli` depends on the wire and the words alone, and an edit to a verb's help never recompiles the runtime.
- **Leaves stay leaves.** `wsp-wire` and `wsp-words` change rarely once ported; everything depends on them, so a change there rebuilds the world, which is the right price for a wire change.
- **`wsp-runtime` is taken.** The daemon's workspace manager is `daemon/crates/wsp-runtime`, and the one binary (#2450) links both, so the host's core is `wsp-core`.
- **The slate module is two crates.** The pure module (parser, expressions, validator) is used by the batch on the host and has no state; the slate store and runs hold state and processes. The web app keeps its TS copy of the pure module for drawing (`apps/web` imports `@wsp/protocol/slate` 46 times), so the Rust and TS evaluators must agree; the slate spec v2's checker fixtures become the shared test for both.

Why a workspace of its own: the daemon's workspace builds a static musl binary with static libseccomp (`daemon/.cargo/config.toml:5-6`) and its CI job is 14 minutes; the host crates should not move that build while the port runs, and the daemon crates they need are three path dependencies. Joining the workspaces is #2450's job, when one binary needs one lock file.

Rules for the layout once ruled:

1.5. A crate MUST NOT depend on a crate in its own layer or above. Two layer-4 crates that need each other go through a trait in `wsp-core`.

1.6. `wsp-cli` MUST NOT depend on any crate of layers 2 to 6. A verb learns everything from the host over the socket, as today.

1.7. A new crate, a move of a module between crates, or a dependency that breaks 1.5 is an owner ruling on the map, never a builder's call.

## What the app and the command line keep in TypeScript

The web app keeps `packages/protocol` (types, words, the slate module for drawing) and `packages/catalog` as TypeScript, because it imports them. Whether those become generated from the Rust wire is the map's open question ("Whether the web app keeps a TS protocol package of its own or reads types generated from the Rust wire"); this spec does not settle it. Until it does, a wire change touches the Rust type and the TS zod schema in one change, and the comparison harness holds them together.
