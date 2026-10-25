# daemon
Source: packages/protocol/src/wire/daemon.ts (972 lines) at a3b870a6e, less the process rows (proc_frames). About 700 of its lines are the 85-op `DaemonRequest` union and the reply shapes; the Rust re-exports the 33 shapes the TS holds equal to wsp-frames' generated types. Tests: protocol.test.ts (9 cases), broker-contract.test.ts (2), callback-relay.test.ts (2), pty-mode.test.ts (2), daemon-ws.test.ts (1); 4 more here; and tests/contract_fixtures.rs (4) reads every daemon contract fixture. Rust: `src/wire/daemon.rs`; tests `tests/daemon.rs` (20), `tests/contract_fixtures.rs` (4).

## What it does
What the host sends a daemon and how it reads the daemon's answers: the auth frame, the 85 ops of a daemon socket (ptys, ports, files, git, processes, tunnels, stdio, http, guests, the place's leave and update), their caps, and the reply shapes. The host writes these frames more than it reads them; it reads them where a page's frame passes through (`daemon.send`) and in tests, and it reads every reply.

## Behaviour
1. `parse_daemon_request(frame)` (daemon.ts:516-959), as zod's discriminated union: a non-object is `invalid_type` at []; an absent or unknown op is ONE `invalid_union_discriminator` at ["op"], `detail.options` = `DAEMON_REQUEST_OPS` (85 words, `pty.create` first, `place.update` last, the TS order), `message` = `Invalid discriminator value. Expected 'a' | 'b' | ...`; a known op then reads its fields in the TS object's order, `id` first: a missing id is `invalid_union` at ["id"], message `Invalid input` (zod's union of string and number). Unknown keys are dropped.
2. The caps, each an issue at its field: `exec.cmd` and `stdio.open.cmd` at most `EXEC_BODY_MAX` 16384, `stdio.open.cmd` at least 1; `exec.timeoutMs` and `http.post.timeoutMs` positive integers at most `EXEC_TIMEOUT_MAX_MS` 600000; `http.post.url` passes `is_http_url` (custom "http or https URL"), `headers` each value at most 8192 and at most 64 keys (custom "at most 64 headers" at ["headers"]), `body` at most `EXEC_OUTPUT_MAX` 2097152; `ports.watch.cgroups` at most 16 cgroup paths (helpers_limits 9) and `watch` a watch name (helpers_limits 10); `proc.watch.cgroups` the same 16; `proc.scopes.scopes` at most `PROC_SCOPES_MAX` 256, each scope's `thread` 1..=64, `workspace` <=64, `agents`/`groups`/`pids` at most `PROC_SCOPE_PIDS_MAX` 64 non-negative integers, `folder` <=4096, `cgroup` <=512; `proc.kill.pid` a positive integer, `identity` <=32, `protect`/`apps`/`agents` at most `KILL_PIDS_MAX` 256 non-negative integers, `agentBins` at most 64 names of <=255; `container.stop.container` exactly `^[0-9a-f]{64}$`; `usage.logs.stores` at most `USAGE_STORES_MAX` 32; `transcripts.list`/`.read` `root` 1..=4096, `dirs` at most 64 of <=255, `cwds` at most 64 of <=4096, `session` 1..=64, `last` and `tail` positive; `guest.open` `token`/`turnToken` <=512, `argv` at most 256 words, `cwd` <=4096; `git.prReply.body` <=65536; `ssh.start.authorizedKey` 1..=1024; the six ops' `token` (git.push, git.pr, git.prCheckout, git.update, git.startOn, git.mergeIn) <=512; `place.update` `uploadId` 1..=32 of `[a-z0-9]`, `seq` >= 0, `sha256` exactly `^[0-9a-f]{64}$`.
3. `DaemonAuthRequest` (:76-82): `op` the literal `auth`, `token` a string, `port` 1..=65535, `pipe` a boolean, both optional.
4. `GitDiffReply.moved` defaults to `[]` (:301): a reply without it reads as none moved and writes `"moved": []`. `GitDiffReply.base` is a string or null, written as null.
5. `GitStatusReply.stashes` is a positive integer where present; `GitStatusEntry.origPath` optional.
6. `FsReadReply` (`content`, `size`, `truncated`), `PtyListEntry` (`reply`, `thread` optional), `ProcInspectReply.cwd` nullable, `ProcKillReply` all three lists optional.
7. `GuestCliMessage` is an untagged union: `{stream: out|err, text}` or `{exit: integer}`; `GuestOpenReply.session`.
8. `DaemonExecReply` needs `exitCode` (integer), `stdout`, `stderr`, `truncated`. The constants `EXEC_OUTPUT_MAX` 2097152, `EXEC_TIMEOUT_DEFAULT_MS` 20000, `EXEC_TIMEOUT_MAX_MS` 600000, `EXEC_DEADLINE_EXIT` 124, `DETACHED_HEAD` `(detached)`, `PROC_KINDS` (11 words, agent first, other last) are copied.
9. Every reply shape re-exported from wsp_frames (FsListReply, FsHashReply, GitCommitReply and the rest on the `pub use` line) reads every bound while deserializing; its Check is `Ok(())`.
10. `parse_daemon_response` is proc_frames 8.

11. Each op's rules run in the key's own read (`src/wire/daemon_checks.rs`), in the TS object's key order, so the first issue is the first key of the schema that fails, by type or by rule, and `issues_of` lists every issue zod lists: a list answers its length rule and then each item, a key with several checks (`uploadId`) answers each that fails. `guest.send` and `guest.reply` `message` and the `guest.message` event's are `z.unknown()`: absent stays absent, `null` stays `null`.
12. Written order: a `DaemonRequest` writes `op`, the call's keys, then `id`, as the daemon channel builds a frame (`{...frame, id}`); `DaemonExecRequest` read or written alone is zod's own object, `id`, `op: "exec"`, then its keys. `DaemonFrame` and a channel event's `event` write integer-like keys before the known key and never a `__proto__` key.
13. URLs are read as node's `new URL` reads them (`src/wire/node_host.rs`): the `url` crate's IDNA tables differ from node's, so a host holding a code point node refuses is refused, U+1E9E maps to `ss`, and a host the crate refuses but node takes is rebuilt in punycode when its odd characters are in the generated tables. A host with any other character the crate refuses stays refused, which is the safe side; against node, no host of 253,732 tried and no single code point in six label positions was taken here and refused by node.

## Edge cases
- A fraction id (`2.5`) is an id and is written back as `2.5`.
- 64 headers pass, 65 refuse; a header value of 8193 units refuses at ["headers", "<name>"].
- `{"id": 1}` (no op) is the discriminator issue, never a missing-field issue.

## Behaviour nobody wrote down
- Rule 4.26: `pty.create.env`, `stdio.open.env`, `http.post.headers` are IndexMaps in the order sent; a page's env order reaches the daemon unchanged.
- Rule 8.3: every string cap counts UTF-16 units.
- Chapter 05, Proposal 9: the first issue's code and path are what a client and the harness read; the unknown-op issue is the version hint.

## Tapes
`daemon.open`/`daemon.send` tapes carry daemon frames through the host (chapter 04); the contract fixtures (daemon/fixtures/contract/frames, 216 files, 108 names) are the accept and reject cases, run in tests/contract_fixtures.rs. Held out: none counted here.

## Decisions
1. The 33 shapes the TS holds `Same` as wsp-frames' generated types are re-exported, not ported (`protocol/src/generated` is not ported).
2. `DaemonRequest` is the wire's own enum, generated from the zod union, not wsp_frames' `DaemonRequest`: the daemon's type is its writer's and differs in leniency (the host must read and refuse exactly as zod does).
3. `DAEMON_REQUEST_OPS` is a const list copied from the union, in order, so the discriminator issue can name them without walking the enum.

## After the port
- A missing id answers an `invalid_union` whose nested `unionErrors` zod writes in full; Proposal 9 compares only code and path, so the nesting is not reproduced.
