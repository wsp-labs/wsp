# 11 Finding a host, and the protocol version

How each client finds the host that serves a state file, what it checks before it trusts the answer, and the proposal for the version range the map's step 4 needs ("the app finds a host the way the CLI does today and checks a protocol version range in the first frame"). Every `file:line` is at `a3b870a6e`; facts about the GPUI app are at `origin/gpui/main` `ca56a2a1f`.

The second pass (2026-10-25) re-checked every `file:line`, recorded the files and a restart from a throwaway TS host built at `a3b870a6e`, and added rules 11.6 to 11.12. Discovery is files and HTTP, not frames, so most of it is recorded as `text` blocks; the frames a client sends once it has found the host are 04's tapes.

## Today: every client finds the host through two files

11.1. The state file MUST be chosen by the one order of rule 10.17 (`--state`, `$WSP_HOME/state.json`, a checkout's `./.wsp/state.json`, `~/.wsp/state.json`); the app uses `WSP_HOME` or `~/.wsp`, and a dev run from a checkout its `.wsp`, unless packaged (`statePathIn` and `homeOf`, `apps/desktop/src/host-lifecycle.ts:113-116,148-151`).

11.2. Beside it, `host.lock` names the host (rule 2.2) and `host-token` holds its token (rule 2.1); a client dials `ws://<lock address, wildcard as 127.0.0.1>:<lock port>/ws` (`dialAddress`, `host-lock.ts:219-222`, with `isWildcard`, `protocol/src/app-ports.ts:69-71`; the URL by `wsUrlOf`, `app-ports.ts:29-34`) and sends the token in its first frame (04). These two files and their formats are the whole discovery contract for this computer, and the Rust host MUST write them exactly (2.1, 2.2, 6.3).

Each client, as it stands:

| Client | Finds the host by | Checks before trusting it | Where |
|---|---|---|---|
| the command line | the aim order of rule 10.3: `--host` or `WSP_HOST`, the launch pair, the live lock here, the account's one host, else here with a start | the auth reply's `version` equals its own release, unless `anyRelease`; a pinned host key when off the loopback | `packages/host/src/verbs/client.ts:208-375`; `hosts.ts:296-353` |
| the Rust tool server | the same order, read afresh at each dial | the same equality (`release::refusal`) | `daemon/crates/wsp-mcp/src/aim.rs:98-227`; `release.rs:20-48` |
| the Electron app | the lock; with none, the account's host; else it writes the service unit (`wsp up --state <state>` through its shim) and waits | the page's boot line carries `tokenHash`, the sha256 of the token file, which it compares; the boot `version` against its own: an older host is replaced, a newer one attached to | `apps/desktop/src/host-lifecycle.ts:88-93,126,174,198-231,304-460` |
| the GPUI app | `wsp-mcp`'s dial (`HostClient::connect`, `gpui/main:apps/desktop-gpui/host-client/src/lib.rs:132-434`), with the aiming environment names stripped | `ReleaseRule::OwnOrNewer`: a host of its own release or newer | `gpui/main:.../src/lib.rs:431-438` (the rule set at 437); the rule's code at `gpui/main:daemon/crates/wsp-mcp/src/release.rs:27,38`; `.../tests/release_rule.rs` |
| the web page | the page it was served from carries `wsPath` in its boot line (rule 4.42) | nothing | `runtimeUrl`, `apps/web/src/protocol/pairing.ts:21-24` |
| a thread | `WSP_HOST_URL` and `WSP_HOST_TOKEN` in its launch environment, and `WSP_HOST_KEY` where a place door exists | the key fingerprint, off the loopback | `protocol/src/env.ts:37,41,55`; `hosts.ts:313-353` |

11.3. There is no protocol version on the wire today. The host sends `version` (its release, `packages/host/src/version.ts:9-11`) and `road` in the auth reply (rule 4.11) and learns nothing about the client; clients compare releases, three different ways. Two releases misread each other quietly because zod drops unknown keys, which is why the command line insists on equality (`sameRelease`, `verbs/client.ts:331-336`, under the comment at 328-330).

11.4. The GPUI app also reads `state.db` directly, read only, to ask whether any computer is on record before it starts a host: `select exists (select 1 from collections where collection = 'places')`, so `places` is a collection name in the `collections` table, not a table (`gpui/main:apps/desktop-gpui/host-client/src/start.rs:659-661`). Under Proposal 3 the table stays, so this keeps working; any other layout breaks it (rule 2.4).

## Proposal 7: a protocol number in the first frame

> **Proposal, accepted by the owner (26 Oct 2026, #2442)** (ticket decision 7). Pick: one integer, the protocol number, beside the release. The auth request MAY carry `protocol: [min, max]`, the range the client speaks; the auth reply MUST carry `protocol: <n>`, the host's number, beside `version` and `road`. A host whose number is outside a client's range refuses the auth with kind `usage` and a sentence in two halves naming both and what to update; a client that sends no range is served as today. Discovery stays exactly as it is.

How the number moves: one constant in `wsp-wire`, starting at 1 with the Rust host's first release. A change that makes an older client misread the wire bumps it: an op removed or renamed, a field removed, renamed or changed in meaning, a refusal's kind changed. Adding an op, an optional field or an event does not. The parity tests check the constant against a list of the wire's shapes kept beside it, so a breaking change that forgets the bump fails a test.

Who checks what:

| Client | Sends | Checks |
|---|---|---|
| the command line and the tool server | nothing new | release equality, as today: after #2450 they ship inside the host's own binary, so they always match the host they started |
| the GPUI app | its range | the host's number in its range; the release rule `OwnOrNewer` goes, because a newer host that speaks the same protocol is fine and an older host that speaks it is too |
| the web page | nothing | nothing: the host serves the page it speaks to |
| an install alone on a server (#2451) | as the command line | as the command line |

Why a number and not the release: the app and the host ship separately once the host leaves the bundle (#2451), so release equality would refuse every patch release of either; release ordering (`OwnOrNewer`) says nothing about whether the wire changed. A number that moves only on a breaking change says exactly that, and sending the client's range lets the host refuse with a sentence a person reads rather than a quiet misread.

Why it touches the TS host: the comparison harness diffs the Rust host against goldens the TS host recorded, so a field only the Rust host sends would sit on the accepted list forever. The pick is to add `protocol: 1` to the TS host's auth reply (one field in the `released` object, built at `packages/host/src/server.ts:642` and spread into the reply at `packages/runtime/src/serve.ts:395,854`) before the goldens are frozen, so the oracle carries it. That is a change to the TS host during the port, which the map otherwise forbids, so it is the owner's ruling, not a builder's.

11.5. Until Proposal 7 is ruled, the Rust host MUST answer the auth frame exactly as rule 4.11 says, `version` and `road` and nothing more.

## What a client can rely on, pinned

11.6. `host-token` MUST be 24 random bytes in base64url, 32 characters with no padding and no newline, written by `writeOwn` (a 0600 temp file, then a rename) after the lock is taken and before any port is bound, minted afresh on every start, and never removed: a stopped host leaves its last token behind (`cli/serve.ts:311-313`; `own-file/src/index.ts:27-41`). A reader trims it (`hostTokenFor`, `host-lock.ts:143-149`); a missing file is the auth refusal `the host's token file is missing: <path>` (`verbs/client.ts:152-153,165-166`; the same in `wsp-mcp/src/aim.rs:185-188`); an empty file is sent as the empty token and refused at the door (04, the empty-token tape under rule 4.30). A token is checked once per socket: after the first `auth` passes, a later `auth` answers ok whatever token it carries (rule 4.11):

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"token":"wrong","id":2,"op":"auth"}}
{"out":{"id":2,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"token":"","id":3,"op":"auth"}}
{"out":{"id":3,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":4,"op":"projects.list"}}
{"out":{"id":4,"ok":true,"projects":[]}}
```

11.7. `host.lock` MUST be `JSON.stringify` of `{pid, port, address, startedBy?, startedAt}` in that order, with no newline: `port` a number, `address` the bound address as a string (the type marks it optional, the host always writes it), `startedBy` one of `verb`, `up`, `service` and absent when nothing named the road, `startedAt` an ISO time (`takeLock`, `host-lock.ts:259-270`; `cli/serve.ts:271`). It is written twice: before the bind with the asked port, through `host.lock.<pid>` linked into place so only one start of many wins; and after the bind with the bound port, by renaming `host.lock.<pid>` over it (`cli/serve.ts:356`; `host-lock.ts:356-360`). It is removed only on close, and only by the pid it names (`releaseLock`, `host-lock.ts:350-352`). Recorded from the throwaway host, 105 bytes, last byte `}`; after `SIGTERM` the lock was gone and `host-token` stayed; after a restart on the same `--port`:

```text
{"pid":456256,"port":47391,"address":"127.0.0.1","startedBy":"up","startedAt":"2026-10-25T10:17:34.744Z"}
(stopped: home host-token host.out place-host-key.json release.json state.db state.db-shm state.db-wal thread-bin web)
{"pid":521071,"port":47391,"address":"127.0.0.1","startedBy":"up","startedAt":"2026-10-25T10:28:31.989Z"}
token: new
```

Across a restart the port is kept when it was named; `wsp up` with no `--port` takes 4400 and steps over a held default (`app-ports.ts:158`; `cli/commands.ts:135-144`); a verb-started host runs `up --port 0` and gets whatever the OS gives, except that a verb host restarting itself respawns on its own port (`host-start.ts:66,83`; `restart.ts:63-79`); the service's manager reruns its own line (`restart.ts:58-60`). `door-port` holds `<port>\n`, written once when `--port 0` and none exists, reused at the next start, and not rewritten when a held port moves the door elsewhere (`server.ts:224-231,538,574-594`).

11.8. A lock MUST be read as live exactly as `servingHost` reads it (`host-lock.ts:100,225-228`): the pid answers `kill(pid, 0)` (EPERM counts as alive) and the program behind `/proc/<pid>/exe` still exists (`runsVanished`, `host-lock.ts:62-97`). There is no port check in that read; a starter's wait adds a GET of the page with a 2,000 ms limit (`httpProbe`, `service.ts:535-550`). A missing, unreadable, empty or non-JSON lock, and one missing `pid`, `port` or `startedAt` or with a wrong type, reads as no lock; `address` and `startedBy` are not checked and extra keys are accepted (`host-lock.ts:45-56,102-123`). A reader deletes no lock; only a starter's `takeLock` replaces one that is not live, through its marker files, a marker with no pid counting as a crash's after 10,000 ms (`host-lock.ts:285-306`). The readers differ today: the Electron app also wants the pid to be one it owns (`host-lifecycle.ts:129`), and `wsp-mcp` checks only `kill(pid, 0)` (`aim.rs:166-175`, rule 10.16).

11.9. The dial address MUST be the lock's `address`, except that exactly `0.0.0.0` or `::` dials `127.0.0.1` and an absent address is loopback (`isWildcard`, `app-ports.ts:69-71`; `dialAddress`, `host-lock.ts:219-222`); `[::]` and long forms of the zero address are not mapped. An address holding `:` and not starting with `[` is bracketed (`authority`, `app-ports.ts:112-115`), and the URL is `ws://<authority>:<port>/ws` (`wsUrlOf`, `app-ports.ts:29-34`, which maps `https` to `wss` and drops trailing slashes before adding `/ws`).

11.10. The account step of the aim MUST count the records under `<home>/hosts/` whose `via.kind` is `account` with a string `via.hostId`, less the one naming this computer's own relay `hostId` (`accountHosts`, `hosts.ts:122-131,177-201`): exactly one is dialled; none falls through to a host here, started if need be; two or more is a usage refusal naming them (`NAME_ONE_HOST`, `severalAccountHostsLine`, `hosts.ts:206-211,324`). The read is offline and synchronous. The Electron app does not refuse on several: it logs the line and opens the host here, and dials an account host with an 8,000 ms limit (`host-lifecycle.ts:165,178-184`).

11.11. The Electron app's checks MUST keep working against the Rust host, so the page it reads is held to rule 4.42: it GETs `/` with a 2,000 ms limit; a page with the boot line and no `version` is refused as an older wsp; on a loopback lock only, no page is waited on, no boot line is refused, and a missing or wrong `tokenHash` (sha256 hex of the token file, compared in constant time) is refused; a host bound beyond loopback is not digest-checked, since its page carries no digest (`lockedHost`, `host-lifecycle.ts:53-60,88-91,126-144`). A host of the app's release, or one naming none, is attached to; a newer one is attached to; an older one is replaced, and the replacement must then be exactly the app's release (`host-lifecycle.ts:304-327`).

11.12. Every wait in finding and starting a host MUST keep the TS value:

| Wait | Value | Where |
|---|---|---|
| the dial: socket open plus first answer, here or loopback | 5,000 ms (`NEAR_WINDOW_MS`) | `hosts.ts:251,260-266`; `verbs/client.ts:216,273-276` |
| the dial, any other address or one that cannot be read | 15,000 ms (`FAR_WINDOW_MS`) | `hosts.ts:256,262-263` |
| the close after a refusal with no kind | 500 ms (`CLOSE_GRACE_MS`) | `verbs/client.ts:122,315` |
| polling a lock or a redial | 200 ms (`POLL_MS`) | `host-lock.ts:38` |
| a started host's lock and page | 20,000 ms (`SERVICE_WAIT_MS`), then `no host answered for <state> within 20s` (rule 6.6) and the last 20 non-empty lines of the last 64 KiB of `host.log`; a child that exits first fails at once | `host-lock.ts:43`; `host-start.ts:103-111`; `service.ts:558-574,590,620-635` |
| a take-over marker with no pid | 10,000 ms (`MARKER_MS`) | `host-lock.ts:285` |
| a service manager command | 15,000 ms; a stop 100,000 ms | `service.ts:411,417` |
| the Electron app's service start | fails at 20 s with no process or after 2 exits, at 120 s with nothing moving; asks the manager every 1,000 ms | `host-lifecycle.ts:498-553` |
| the Electron app replacing or restarting a host | 20,000 ms for the old pid to go | `host-lifecycle.ts:441,457,471` |
