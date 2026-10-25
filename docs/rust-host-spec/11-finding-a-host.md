# 11 Finding a host, and the protocol version

How each client finds the host that serves a state file, what it checks before it trusts the answer, and the proposal for the version range the map's step 4 needs ("the app finds a host the way the CLI does today and checks a protocol version range in the first frame"). Every `file:line` is at `a3b870a6e`; facts about the GPUI app are at `origin/gpui/main` `ca56a2a1f`.

## Today: every client finds the host through two files

11.1. The state file MUST be chosen by the one order of rule 10.17 (`--state`, `$WSP_HOME/state.json`, a checkout's `./.wsp/state.json`, `~/.wsp/state.json`); the app uses `WSP_HOME` or `~/.wsp`, and a dev run from a checkout its `.wsp`, unless packaged (`apps/desktop/src/host-lifecycle.ts:113-148`).

11.2. Beside it, `host.lock` names the host (rule 2.2) and `host-token` holds its token (rule 2.1); a client dials `ws://<lock address, wildcard as 127.0.0.1>:<lock port>/ws` (`host-lock.ts:219-222`; `protocol/src/app-ports.ts:29-38`) and sends the token in its first frame (04). These two files and their formats are the whole discovery contract for this computer, and the Rust host MUST write them exactly (2.1, 2.2, 6.3).

Each client, as it stands:

| Client | Finds the host by | Checks before trusting it | Where |
|---|---|---|---|
| the command line | the aim order of rule 10.3: `--host` or `WSP_HOST`, the launch pair, the live lock here, the account's one host, else here with a start | the auth reply's `version` equals its own release, unless `anyRelease`; a pinned host key when off the loopback | `packages/host/src/verbs/client.ts:208-375`; `hosts.ts:296-356` |
| the Rust tool server | the same order, read afresh at each dial | the same equality (`release::refusal`) | `daemon/crates/wsp-mcp/src/aim.rs:98-227`; `release.rs:20-48` |
| the Electron app | the lock; with none, the account's host; else it writes the service unit (`wsp up --state <state>` through its shim) and waits | the page's boot line carries `tokenHash`, the sha256 of the token file, which it compares; the boot `version` against its own: an older host is replaced, a newer one attached to | `apps/desktop/src/host-lifecycle.ts:88-93,126,174,198-231,304-460` |
| the GPUI app | `wsp-mcp`'s dial (`HostClient::connect`, `gpui/main:apps/desktop-gpui/host-client/src/lib.rs:132-434`), with the aiming environment names stripped | `ReleaseRule::OwnOrNewer`: a host of its own release or newer | `gpui/main:.../src/lib.rs:47,250-255`; `.../tests/release_rule.rs` |
| the web page | the page it was served from carries `wsPath` | nothing | `apps/web/src/protocol/client.ts` |
| a thread | `WSP_HOST_URL` and `WSP_HOST_TOKEN` in its launch environment, and `WSP_HOST_KEY` where a place door exists | the key fingerprint, off the loopback | `protocol/src/env.ts:26-55`; `hosts.ts:313-354` |

11.3. There is no protocol version on the wire today. The host sends `version` (its release, `packages/host/src/version.ts:9-11`) and `road` in the auth reply (rule 4.11) and learns nothing about the client; clients compare releases, three different ways. Two releases misread each other quietly because zod drops unknown keys, which is why the command line insists on equality (`verbs/client.ts:328-330`).

11.4. The GPUI app also reads `state.db` directly, read only, to ask whether a `places` row exists before it starts a host (`gpui/main:apps/desktop-gpui/host-client/src/start.rs:659-661`). Under Proposal 3 the table stays, so this keeps working; any other layout breaks it (rule 2.4).

## Proposal 7: a protocol number in the first frame

> **Proposal, awaiting the owner's ruling** (ticket decision 7). Pick: one integer, the protocol number, beside the release. The auth request MAY carry `protocol: [min, max]`, the range the client speaks; the auth reply MUST carry `protocol: <n>`, the host's number, beside `version` and `road`. A host whose number is outside a client's range refuses the auth with kind `usage` and a sentence in two halves naming both and what to update; a client that sends no range is served as today. Discovery stays exactly as it is.

How the number moves: one constant in `wsp-wire`, starting at 1 with the Rust host's first release. A change that makes an older client misread the wire bumps it: an op removed or renamed, a field removed, renamed or changed in meaning, a refusal's kind changed. Adding an op, an optional field or an event does not. The parity tests check the constant against a list of the wire's shapes kept beside it, so a breaking change that forgets the bump fails a test.

Who checks what:

| Client | Sends | Checks |
|---|---|---|
| the command line and the tool server | nothing new | release equality, as today: after #2450 they ship inside the host's own binary, so they always match the host they started |
| the GPUI app | its range | the host's number in its range; the release rule `OwnOrNewer` goes, because a newer host that speaks the same protocol is fine and an older host that speaks it is too |
| the web page | nothing | nothing: the host serves the page it speaks to |
| an install alone on a server (#2451) | as the command line | as the command line |

Why a number and not the release: the app and the host ship separately once the host leaves the bundle (#2451), so release equality would refuse every patch release of either; release ordering (`OwnOrNewer`) says nothing about whether the wire changed. A number that moves only on a breaking change says exactly that, and sending the client's range lets the host refuse with a sentence a person reads rather than a quiet misread.

Why it touches the TS host: the comparison harness diffs the Rust host against goldens the TS host recorded, so a field only the Rust host sends would sit on the accepted list forever. The pick is to add `protocol: 1` to the TS host's auth reply (one field in the `released` object, serve.ts:395,854) before the goldens are frozen, so the oracle carries it. That is a change to the TS host during the port, which the map otherwise forbids, so it is the owner's ruling, not a builder's.

11.5. Until Proposal 7 is ruled, the Rust host MUST answer the auth frame exactly as rule 4.11 says, `version` and `road` and nothing more.
