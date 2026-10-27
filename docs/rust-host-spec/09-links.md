# 09 Links: daemons, joined computers, the relay and the seal

The host's own connections outward: to a daemon, to a computer that joined it, to the relay, and the seal that protects the last two. On these links the host is the client of the daemon's wire, whose types already live in Rust (`wsp-frames`). Every `file:line` is at `a3b870a6e`; serve.ts is `packages/runtime/src/serve.ts`.

## Five ways the host reaches a daemon

| Road | When | First frame | Lifetime | Where |
|---|---|---|---|---|
| a place link | a joined computer dialled in; the handshake was the auth | none | as long as the computer's socket stands; one per place | `runtime/src/places/link.ts:137-226` |
| a held reach | this computer's own daemon at `http://127.0.0.1:<port>` (`packages/host/src/place-here.ts:119`); a machine's daemon through the provider's preview URL only for short probes, closed when they answer (`runtime/src/machines/reach.ts:207`, `daemonAnswer` and `helloVersion`; `host/src/local-daemon.ts:116,274`) | `{id, op: "auth", token}`, never the token in the URL | held and redialled while wanted | `runtime/src/reach.ts:63-265` |
| one dial per use | a workspace's daemon frames from the runtime (`withDaemon`) | `{op: "auth", token, id}` | one frame or one piece of work, then closed | `runtime/src/machines/kinds.ts:518`; `runtime/src/account/channels.ts:158` |
| the pipe pool | pipes to a workspace's daemon, from version 159 | auth with `pipe: true` | up to 4 idle (`PIPE_IDLE`), idle at most 600,000 ms, dialled at most 4 a second within 15,000 ms each, 64 at most (`PIPE_POOL_MAX`, `protocol/src/daemon-contract.ts:79`), with its own backoff | `runtime/src/pipe.ts` (backoff at `:148`) |
| a page's channel | one `daemon.open` from a window | `{op: "auth", token, id}` | one dial, no redial, closed with the socket that opened it | `runtime/src/daemon-channel.ts:72-170` |

9.1. A held reach MUST, after auth (sent only on a preview-URL dial), send the opening ritual `ports.watch`, `inbox.watch`, `inbox.rescan`, each awaited before the next (reach.ts:187-204), then ping every 10 s, the first ping 10 s after the ritual ends, and terminate a socket that answered no pong within one beat (reach.ts:56,117-135); a ping answered `ok: false` counts as no pong. It has no connect deadline and no ritual deadline, and the beat starts only after the ritual, so a daemon that takes the socket and never answers holds `ready` open. It redials with `linkBackoffMs`: `attempt` counts up from 1, `min(10000, 500 * 2^(attempt - 1)) + floor(random * 250)`, so 500, 1,000, 2,000, 4,000, 8,000, then 10,000 ms, each plus 0 to 249 (`protocol/src/wire/limits.ts:101-105`), and `attempt` goes back to 0 after a ritual completes. A 4401 close ends it for good (`reauth-needed`, no redial) unless its reason is the auth deadline's sentence; a reach riding a place socket never redials (status `dead`). Its request ids count from 1 per reach (reach.ts:72,99).

9.2. A page's channel MUST bound its connect at 15 s (`CONNECT_TIMEOUT_MS`, `daemon-channel.ts:61`), hand every frame that is not a reply to `onEvent`, and refuse a forwarded frame whose op is `auth` (`DaemonFrame`, `protocol/src/wire/devices.ts:201`). `daemon.open` takes exactly one of `workspaceId` and `placeId`, else the plain error `daemon.open opens a channel to one daemon: name workspaceId or placeId, not both` (no kind, 05); its reply `{id, ok: true, channel}` goes first, with daemon frames that came early queued behind it (serve.ts:1425-1485); `daemon.send {channel, frame}` answers `{id, ok: true, reply: <the daemon's reply untouched>}`.

9.3. The daemon's door mirrors the host's (rule 4.10) with two differences a client reads: its auth deadline (`AUTH_DEADLINE_MS` 5,000, with at most `PRE_AUTH_MAX_BYTES` 4,096 before auth, `daemon-contract.ts:116,118`) closes with 4401 and the reason `no auth frame arrived in time` (`:204`), not 4408, and `daemonRefusedToken` reads a 4401 with that reason as no refusal of the token (`:213`); a token rotated under an open socket closes it with `DAEMON_TOKEN_ROTATED` (`protocol/src/daemon-contract.ts:201-213`). Daemon tokens are lowercase hex, per machine: in production 24 random bytes as hex, minted once per machine per host process and never stored (`machineTokens`, `runtime/src/threads/transcripts.ts:515-527`), and `HMAC-SHA256(seed, machineId)` only when a seed is given, which only tests do (`runtime/src/daemon-token.ts:19-29`); re-read from the token file at every auth frame so the host rotates them live (`daemon/CLAUDE.md` invariant 8; `rotateDaemonToken`, `daemon-token.ts:59-69`). `DaemonAuthRequest` is `{id, op: "auth", token, port?, pipe?}` (`protocol/src/wire/daemon.ts:75-82`).

## The daemon's version

`DAEMON_VERSION` is the length of `DAEMON_CONTENTS`, 167 at this commit, one sha of the daemon's sources per version, cut at landing and never on a branch (`protocol/src/wire/daemon-version.ts:716`; `daemon/crates/wsp-frames/src/numbers.rs:5`). A daemon says its version once, in `daemon.hello {root, version}` right after the auth reply (`protocol/src/wire/daemon-events.ts:14`); there is no version op.

9.4. A daemon behind the host MUST still link. `placeDaemonBehind(place)` answers nothing when the version is unknown or not behind, else the word `daemon <n>, host <DAEMON_VERSION>` (`daemon-version.ts:766-769`): it is a row word and an update trigger (`runtime/src/places/views.ts:280-283,382-396`), not a refusal. Hard refusals exist only per op, by minimum version: `FS_FOLDERS` 73, `NAMED_WATCH` 136, `LEAVE_ASKS` 141, `LEAVE_TAKES` 145, `FORK_BRANCH` 150, `SYS_UNWATCH` 152, `STDIO` 154, `HTTP_POST` 155, `PIPE` 159 (`daemon-version.ts:773-815`), and each gate does what its site does rather than one refusal: `FS_FOLDERS` refuses with `placeBehindLine` (serve.ts:437), `STDIO` with `slateServerBehindLine` (`account/channels.ts:237`), `PIPE` falls back to the plain ask with no refusal (`channels.ts:198`), and `SYS_UNWATCH` only decides whether the daemon is told to stop (`places/readings.ts:35-38`); each unit's spec pins its own gates. A machine whose hello is behind gets its daemon replaced on wake (`runtime/src/machines/daemon.ts:221`). A daemon or command line one version behind still answers: gate a new op or flag on its version and read a missing op as refused (runtime trap, #1484, #1613).

9.5. The Rust host MUST reuse `wsp-frames` for every daemon frame and MUST keep reading the contract fixtures in `daemon/fixtures/contract` (216 frame files for 108 ops, 42 event files, 52 reply files) as the wire's truth. A frame that fails its schema is dropped whole, so a field the host makes required that an older daemon does not send drops every such frame (protocol trap, #1720: `seq` on `ProcSnapshot` blanked the Processes pane); a new field stays optional.

## The place link

A joined computer dials the host, never the other way: `ws://<host>:<door port>/ws` on a LAN door, or `wss://<relay host>/ws` through the relay, with TLS against roots baked into the daemon (`daemon/crates/wsp-daemon/src/link.rs:70-90`).

9.6. The handshake MUST be: `place.join` or `place.auth` in the clear; the host answers with its public key, its nonce, its signature and its ephemeral key, and sets the seal; `place.prove`, the first sealed frame, carries the computer's signature over the expected transcript, its `PlaceReport`, and on a join the code and window name; the sealed reply `{id, ok: true, device?}`; then the socket is handed to the place door (`handedOver`: `ws.pause()`, the client listener off, serve.ts:692-716,735-763). A place that sends no ephemeral at a join gets `PLACE_UNSEALED_JOIN_REFUSAL`, at an auth `placeBehindLine` (`places/link.ts:90,112`). A refused `place.auth` for an unknown place carries the host's signature over `"wsp place refusal v1\n<placeId>\n<nonce>\n<sentence>\n"` (`protocol/src/wire/place-link.ts:176-178`).

9.7. The place door MUST keep one socket per place: a second link cuts the first (`cut`, `places/views.ts:220-228`). `cut` closes the place's reach first, whose `close()` is `ws.close(1000)` on the same socket (`reach.ts:260`), so the later `socket.close(1000, reason)` is ignored and the first link closes with 1000 and an empty reason, never the `replaced by a newer link` its code names (`REPLACED`, `places/helpers.ts:236`; checked by a reader with `ws`: `close(1000)` then `close(1000, "x")` reaches the client as `1000 ""`); `removed from this host` and `this host is stopping` go out the same way. It records `report`, `lastSeenAt` and `road.from`, writes `seen` every 60 s (`SEEN_EVERY_MS`, `helpers.ts:232`) through `change()`, and emits `place.absent` on close (`places/link.ts:211`) and `place.present` on attach (`:213`), in the attach at `:137-226`. From then on the host is the requester on that socket: sealed binary frames `{id, op, ...}` for `machine.*` (19 ops, `protocol/src/wire/machine-link.ts:204-242`) and the daemon's own ops, and the place answers or pushes events.

9.8. The computer's side of the link (already Rust) cuts a link quiet for 30 s and redials with backoff `2 s << (attempt - 1)` up to 30 s, jittered 0.8 to 1.2 times; a refused link waits 10 minutes; each address and each handshake frame has 10 s (`CONNECT_MS`), a link up 60 s counts as settled (`SETTLED`), and a close waits 2 s (`CLOSE_WAIT`) (`daemon/crates/wsp-daemon/src/link.rs:34-50`). The host's ping every 10 s is what keeps a healthy link from going quiet.

9.9. Machine link replies MUST keep their shapes: `{id, ok: true, ...}` or `{id, ok: false, error, code?, kind?, status?}` with `kind` one of `concurrency plan missing conflict snapshotUnavailable transient auth unknown absent` (`machine-link.ts:155,531-544`); `putBytes` parts are base64 of at most 4 MiB of raw bytes each (`MACHINE_PUT_PART_BYTES`, about 5.33 MiB of base64, `machine-link.ts:201`) in `seq` order (`seq >= 0`) under one `uploadId` matching `/^[a-z0-9]{1,32}$/`, and the part marked `last` lands the file (`machine-link.ts:196-242`); a `machine.exec` `cmd` is at most 16 KiB.

## The seal

After a handshake that proves ed25519 keys, every frame both ways travels inside AES-256-GCM under keys agreed in the same handshake (`packages/keys/src/index.ts:1-169`). The daemon's twin is `daemon/crates/wsp-seal`; the vectors are `daemon/fixtures/place-link-seal.json` and `place-link-signing.json`.

9.10. The seal MUST be byte-compatible with `wsp-seal` and the fixtures: ed25519 keys as SPKI DER in base64 (44 bytes), raw 64-byte signatures in base64, X25519 ephemeral keys raw 32 bytes, 32-byte nonces; fingerprint `SHA256:` plus unpadded base64 of the sha256 of the DER (`keyFingerprint`, `keys/src/index.ts:23-25`); two keys from HKDF-SHA256 with the shared secret as input, the place id in UTF-8 as salt (`client` for a native client), length 32, info `wsp place link host to place` and `wsp place link place to host` (`:67-88`); AES-256-GCM with a 12-byte nonce of four zero bytes and an 8-byte big-endian counter, no AAD, ciphertext then the 16-byte tag (`:107-133`); one counter per direction per socket from 0, moving at every frame sealed or opened. The transcript a side signs is the UTF-8 of `"wsp place link v2\n<role>\n<placeId>\n<challenge>\n<answer>\n<challenger ephemeral>\n<answerer ephemeral>\n"` (`place-link.ts:169-178`).

9.11. A sealed frame MUST be one websocket binary message; plain frames are text. With no seal, a binary frame is read as text (`String(raw)`), so a binary frame holding JSON passes; with a seal, a text frame is a seal failure. Every seal failure (a bad tag, a short frame, a counter out of step) is one sentence, `a frame on this link did not open under the key both ends agreed` (`keys/src/index.ts:75,146-150`). On the client websocket a frame that fails to open is answered `{id: null, ok: false, error: "invalid json"}` and closes the socket only before auth; the host's own daemon reach and the node client close with 1002 instead (serve.ts:646-650; reach.ts:144).

9.12. A native client that pins a host key sends `seal.open {nonce, ephemeral}` first; the host answers `{nonce, hostPublicKey, signature, ephemeral}` and the client checks the fingerprint and the signature before it sends any secret (`packages/host/src/verbs/client.ts:287-305,345-346`). One key per socket: a second `seal.open`, or a `place.*` after one, is refused; a host with no place key refuses with `this host holds no key to prove itself with; the host that serves the app wires one` and closes (serve.ts:719-739; `place-link.ts:211`). The web page and loopback clients do not seal: the loopback is the trust boundary.

## The relay

9.13. A host linked to an account (`relay.json`) MUST: ask the relay over HTTPS for a tunnel (`POST <relayUrl>/hosts/<hostId>/tunnel {port}` with `Authorization: Bearer <relay token>`, `packages/host/src/relay-link.ts:680`); run the pinned `cloudflared` as 07 says; beat `POST <relayUrl>/hosts/<hostId>/heartbeat {hostname?, version, hostKey?}` every 60 s (`HOST_BEAT_MS`, `protocol/src/words/places.ts:531`; a `setInterval`, `relay-link.ts:767-769`, the first beat sent from `learn()`, no deadline on the call, a forced refresh at most every 10 s, `BEAT_FLOOR_MS`), where `version` is the pinned cloudflared's version, `CLOUDFLARED.version`, not wsp's (`relay-link.ts:706`); and verify the account's computers and their signed admissions in the beat's answer itself (`relay-link.ts:592-632,701-725`). The default relay is `https://relay.usewsp.com` (`protocol/src/wire/devices.ts:108`); sign-in is a device-code flow, `POST /link/start` then `POST /link/poll` (`relay-link.ts:170-178`).

9.14. A request through the relay lands on the host's own loopback app port as an ordinary request carrying `cf-connecting-ip` or `cf-ray`; the host MUST read either header as "not the own road" (rule 4.4) and MUST NOT put the token in the page it serves there (`packages/host/src/server.ts:219-252`, the host's HTTP server, not the runtime's `serve.ts`). The relay's operator ends TLS, which is why a client off the loopback pins the host key and seals (9.12).

## Guests

9.15. A process inside a machine reaches the host through its daemon's guest road: the daemon carries a session up its link as `guest.opened`, `guest.message`, `guest.closed` events, and the host answers with `guest.reply` and `guest.close` ops (`protocol/src/wire/daemon-events.ts:68-92`; `packages/host/src/guest.ts:62-93`). The host's guest door turns an opened session into either a tool session (the daemon binary's `mcp --scoped --guest`, 10) or a command line run in the host process, each dialling the host's own loopback socket with the thread's launch pair, and caps the guest sessions per workspace itself (host invariant 4; scope, kind and loopback checks at `guest.ts:107-118`): at 64 open sessions on one workspace (`GUEST_SESSIONS_PER_WORKSPACE_CAP`, `protocol/src/daemon-contract.ts:190`, checked at `guest.ts:157`) the next is answered `guest.close {session, error}` with `this folder already holds as many guest sessions as it may; end one and run it again` (`GUEST_WORKSPACE_FULL`, `daemon-contract.ts:313`). A guest message is at most 4 MiB (`GUEST_MESSAGE_CAP_BYTES`) and a session no one watches ends after 600,000 ms (`GUEST_UNWATCHED_MS`). A box's agent is root there and can go around anything its daemon enforces, so every limit on a guest lives on the host too.

## Inputs, stops, order and restart

9.16. `daemon.open` MUST answer these inputs as the TS does (serve.ts:1425-1470; the first tape below): neither id, and both ids, give the same plain error `daemon.open opens a channel to one daemon: name workspaceId or placeId, not both`, with no kind and the socket kept open (`:1439,1448`); an empty string counts as given; an unknown `workspaceId` is `no such workspace: <id>`, kind `not-found` (`machines/boot.ts:628-636`), and from a thread's own token `no such workspace` with no id; an unknown `placeId`, or a known one that is not linked, is `<name> is not answering; it connects on its own when it is on` with no kind, the name falling back to the id (`places/link.ts:317`), so the two cannot be told apart; a `placeId` from a socket off the own road is `PLACES_TICKET_REFUSAL`, kind `ticket`, before any dial. A dial that fails answers the channel's own failure: a door status other than 101 is `the connection was refused with <status>[: <first line>]`, kind `refused`; a 4401 close is `the machine refused this host's daemon token (4401 <reason>)`, kind `reauth`; 15 s with no socket is `daemon connect timed out after 15000ms`. The channel id is 6 random bytes as hex, and the reply `{id, ok: true, channel}` goes before every queued `daemon.event`, flushed in arrival order; a page socket closed during the dial gets no reply and the channel is closed. The channel the page gets is wrapped by road, each wrapper able to refuse or rewrite a frame, their rules in their units' specs: a workspace whose kind shares its daemon gets `threadTagged`, `heldToOwner`, `rootedPorts`, `scopedProcs` and `killGuarded`, any other workspace `threadTagged` alone (`runtime/src/projects/workspaces.ts:1203-1214`), and this computer's own channel `heldToOwner`, `scopedProcs` and `killGuarded` (`runtime.ts:1054`).

9.17. `daemon.send` and `daemon.close` MUST answer an unknown channel with `no such daemon channel on this socket`, no kind (serve.ts:1473,1480); a missing `channel` or `frame`, a frame with no `op`, or a frame whose `op` is `auth` are 05's schema refusal (its text is Proposal 9) with the issue's path (`["frame","op"]` and `the host authenticates the channel` for `auth`, the second tape). A daemon's own refusal is not the host's: `daemon.send` answers `{id, ok: true, reply: {id, ok: false, ...}}`. A link that drops under a frame answers `daemon unreachable` or `connection lost`, no kind. `daemon.close` deletes the channel before it closes it, so no `daemon.closed` follows; a daemon that closes first sends the page `{type: "daemon.closed", channel, code, reason}`; a page socket that closes ends each of its channels with `close(1000)` and no reason, and the page is not told.

9.18. The frames before auth MUST be answered as the TS does (serve.ts:692-763; the handshake tapes below), each refusal `{id, ok: false, error, kind: "auth"}` followed by a close with 4401 `unauthorized`: a `place.join` whose key is not ed25519 is `PLACE_BAD_KEY_REFUSAL` (`places/link.ts:85`), read before anything else; a `place.join` with no ephemeral is `PLACE_UNSEALED_JOIN_REFUSAL` (`:90`); a `place.auth` for a place this host does not hold is `PLACE_UNKNOWN_REFUSAL` with `hostPublicKey` and `signature` over the refusal transcript added (`:106-107`); a `place.auth` for a held place with no ephemeral is `placeBehindLine(name, "daemon <n>, host <DAEMON_VERSION>")`; a `place.prove` as a first frame, a second `seal.open`, a `place.*` after a `seal.open`, a `seal.open` whose ephemeral the challenge cannot use, and any second frame sent while the first is still being decided are `unauthorized` (`:717,723`); a `seal.open` on a host with no place door is `SEAL_UNSERVED`. A pre-auth frame that fails its schema is the schema refusal with no kind, then the same close. After auth, `seal.open` is `unauthorized` with no kind and no close, and `place.auth` is `PLACE_UNKNOWN_REFUSAL`'s text with no kind and no close (serve.ts:914-925). A join's pending entry lives 120 s (`JOIN_PROVE_MS`, `places/helpers.ts:91`) and is dropped only when a later join runs (`link.ts:93`).

9.19. A host stop MUST close the links in this order (`packages/host/src/cli/serve.ts:390-397`, then `packages/host/src/server.ts:774-795`): first the relay link, before the server's own close: the beat cleared, cloudflared sent `SIGTERM` and waited for with no bound, `connector.pid` removed, and nothing sent to the relay; then the back door, the broker, the reap timer, the release watch and the callback relay (`startCallbackRelay`, `server.ts:471,781`); then every websocket client, place links included, gets `close(4001, "stopping")` and is terminated after 1,000 ms (serve.ts:1970); then the HTTP servers; then `rt.close()`, which cuts every place with 1000 and an empty reason (9.7). Held reaches have no close of their own at a stop; their owners close them or the process exit does.

9.20. What survives a restart MUST be what the TS stores: place records, loaded as absent, with nothing redialled (a computer dials in, 9.8); `relay.json` beside the state; `device-key.json` and `relay-client.json` in the wsp home (`host/src/account.ts:81,99,115`); `place-host-key.json` as `{publicKey, privateKeyPem}`, written once, and remade without a word when missing or malformed, which breaks every joined computer's pin (`places/this-computer.ts:30-45`; after the port). Daemon tokens are not stored (9.3): a new host mints each machine's anew and rewrites its token file on the first reach (`rotateDaemonToken`), so a token from before the restart opens nothing.

9.21. At start the host MUST stop a cloudflared a crash left behind: `stopRecordedConnector` reads `connector.pid` (`{pid, startedAt}`), skips it when `startedAt` is before the computer's last boot, else sends `SIGTERM` and polls every 50 ms for up to 5 s, then gives up with no `SIGKILL` (`packages/host/src/connector.ts:267-282`).

## Tapes

Recorded from a throwaway TS host at `a3b870a6e`. The channel ops on a fresh host, where no workspace, place or channel exists:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up","protocol":1}}
{"in":{"id":2,"op":"daemon.open"}}
{"out":{"id":2,"ok":false,"error":"daemon.open opens a channel to one daemon: name workspaceId or placeId, not both"}}
{"in":{"id":3,"op":"daemon.open","workspaceId":"ws_00000000","placeId":"p_0000000000000000"}}
{"out":{"id":3,"ok":false,"error":"daemon.open opens a channel to one daemon: name workspaceId or placeId, not both"}}
{"in":{"id":4,"op":"daemon.open","workspaceId":"ws_00000000"}}
{"out":{"id":4,"ok":false,"error":"no such workspace: ws_00000000","kind":"not-found"}}
{"in":{"id":5,"op":"daemon.open","placeId":"p_0000000000000000"}}
{"out":{"id":5,"ok":false,"error":"p_0000000000000000 is not answering; it connects on its own when it is on"}}
{"in":{"id":6,"op":"daemon.send","channel":"000000000000","frame":{"id":1,"op":"ping"}}}
{"out":{"id":6,"ok":false,"error":"no such daemon channel on this socket"}}
{"in":{"id":7,"op":"daemon.close","channel":"000000000000"}}
{"out":{"id":7,"ok":false,"error":"no such daemon channel on this socket"}}
```

A forwarded frame may not authenticate; this is the schema refusal (05) with the path the host's own check names:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up","protocol":1}}
{"in":{"id":2,"op":"daemon.send","channel":"000000000000","frame":{"id":1,"op":"auth","token":"x"}}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"code\": \"custom\",\n    \"message\": \"the host authenticates the channel\",\n    \"path\": [\n      \"frame\",\n      \"op\"\n    ]\n  }\n]"}}
```

A join whose key is not ed25519, refused before anything else, and the socket closed:

```tape
{"in":{"id":1,"op":"place.join","publicKey":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=","nonce":"BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=","ephemeral":"CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk="}}
{"out":{"id":1,"ok":false,"error":"that join sent a key this host cannot verify a signature against; a place's key is ed25519, as SPKI DER in base64","kind":"auth"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

A join from a wsp older than the seal, with no ephemeral key:

```tape
{"in":{"id":1,"op":"place.join","publicKey":"MCowBQYDK2VwAyEA3l3/HPzp3b739S9ViIC9LAai8wOgLPYqQBSk5yUN4jc=","nonce":"BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="}}
{"out":{"id":1,"ok":false,"error":"that computer's wsp is older than this host and seals no link; update wsp there and join again","kind":"auth"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

Why no tape for the rest: an accepted `seal.open`, `place.join` or `place.auth` answers with the host's fresh nonce, its key, its ephemeral key and a signature, and the refusal of an unknown `place.auth` carries a signature over the place's nonce by a key minted per host, none of which the checker normalizes; every frame after them is sealed, and the checker speaks only plain frames. The held reach, the place link's traffic, the relay and the guest road need a daemon, a joined computer or the relay. Each is a test in its unit's packet, against the fixtures of rule 9.10 for the seal.

## After the port

The `replaced by a newer link`, `removed from this host` and `this host is stopping` reasons that never reach the place (9.7); a held reach with no connect or ritual deadline (9.1); `place-host-key.json` remade without a word (9.20); the relay heartbeat's `version` carrying cloudflared's version (9.13); `stopRecordedConnector` giving up with no `SIGKILL` (9.21).

