# 04 The wire protocol

What a client and the host say to each other on the websocket: the transport, the door every socket passes, the three envelopes, events, and how JSON is written. Errors have their own chapter (05). The host's own links to daemons and joined computers are 09. Every `file:line` is at `a3b870a6e`; "serve.ts" means `packages/runtime/src/serve.ts`.

Every tape in this chapter was recorded from a fresh TS host by `checker/check.mjs` and is replayed by it; 00 says how to read one. The second pass (2026-10-25) re-checked every `file:line`, recorded the tapes from rule 4.30 on against a build of `a3b870a6e`, and added rules 4.30 to 4.45. They sit in the section they belong to, so their numbers run out of order; the earlier numbers did not move, because other chapters cite them. A behaviour with no tape says why.

## Transport

The host has no websocket port of its own. `serveRuntime` (serve.ts:382) makes a `ws` server in `noServer` mode (serve.ts:479-499) and the host attaches it to its HTTP servers (`packages/host/src/server.ts:641-650`):

| Listener | Address | Port | Where |
|---|---|---|---|
| The app server | `--listen`, default `127.0.0.1` | `--port`, default 4400; `0` asks the OS, and a verb that starts a host always passes `--port 0` | `server.ts:446,537,692`; `protocol/src/app-ports.ts:10,19`; `host-start.ts:66` |
| The door joined computers dial | `0.0.0.0` | the app port plus 20 (`PLACE_PORT_OFFSET`), bound when a computer is on record or a door opens; with `--port 0` the bound port is saved in `door-port` beside the state | `server.ts:537-600,750`; `app-ports.ts:13-15` |
| The door's loopback twin, off Linux only | `127.0.0.1` | the door's port | `server.ts:542-543,585-588` |

4.1. The websocket path MUST be exactly `/ws` (`WS_PATH`, `app-ports.ts:23`), matched on the URL's pathname. Any other path, `/ws/` included, MUST be answered with the raw bytes `HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n` and the socket destroyed (serve.ts:489-496).

4.2. An upgrade whose `Origin` header is present and non-empty MUST be refused with the raw bytes `HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n` unless the Origin's hostname equals the hostname of `http://` plus the `Host` header, ports ignored (`originAllows`, `server.ts:263-269`, handed to the socket server as its `originAllowed` option, `server.ts:644`; serve.ts:482-488). No Origin is allowed: it is a tool on this computer. The hostnames are compared as text, with no name lookup: recorded with curl against a host on `127.0.0.1:47391`, `Origin: http://127.0.0.1:9999` was let in (101) and `Origin: http://localhost` was refused (403).

4.3. The upgrade reads the query parameter `ticket` (serve.ts:512), the `cf-connecting-ip` and `cf-ray` headers (either one means the request came through the relay's connector, `server.ts:219-221`), the peer address with `::ffff:` stripped (`app-ports.ts:50`), and which listener accepted it. It reads no Authorization header, no subprotocol and no cookie; the token never rides the URL (serve.ts:1-7).

4.4. A socket is on the **own road** when it came to the app server, not through the connector, from a loopback peer (`ownRoad`, `server.ts:240-242`). Only an own-road socket may authenticate with a thread's token (serve.ts:836).

4.5. The host sets no ping timer and no idle timeout on a client socket; `ws` answers a client's ping by itself (`autoPong`). No permessage-deflate. The largest frame is `ws`'s default 100 MiB, past which `ws` closes 1009 (serve.ts:478-480; `ws@8.21.3` `websocket-server.js:74`).

4.6. A frame that is wrong at the websocket level (unmasked, invalid UTF-8 in a text frame, a bad opcode) MUST end the socket with no reply, after one warning line `socket from <peer> dropped on a wire fault: <message>` (serve.ts:508-511; `packages/runtime/test/serve.test.ts:136-152`).

The same ports serve plain HTTP (`server.ts:498-524`): `GET /` is the web app's `index.html` with its one `window.__WSP__ = {...}` boot line swapped for the host's (`server.ts:204-212,485-496`: `wsPath`, `paired`, `version`, and on the loopback page only `tokenHash`, the sha256 hex of the host token, and `statePath`); `GET <file>` is a file of the web bundle; any other GET with no extension and no trailing slash is the same page; anything else is `404 {"error":"no route: <METHOD> <path>"}`; a throw is `500 {"error": message}`. There are no JSON routes at this commit: `bearerOf` (`server.ts:273`) has no caller.

4.7. The Rust host MUST serve the same routes with the same boot line. A host started with no built web app refuses to start with `web app not built: <path> is missing (pnpm --filter @wsp/web... build)` (`server.ts:206`); a page with no boot line refuses with `<path> has no window.__WSP__ boot line to replace` (`server.ts:210`). The checker's hosts serve a one-line stub page for this reason.

4.42. The boot line MUST be `<script>window.__WSP__ = <json>;</script>` (`server.ts:211`), the JSON written by `JSON.stringify` with `<`, `>`, `&`, U+2028 and U+2029 escaped as `\uXXXX` (`server.ts:200-202`), keys in this order: `tokenHash` (loopback page only), `wsPath`, `paired`, `version`, `terminalFont` (when set), `statePath` (loopback page with a state path only), `productUsageOff` (when set) (`server.ts:487-495`). The page counts as loopback when the listener is the app server, the request did not come through the connector, and the hostname asked for is a loopback name (`server.ts:529,539`); the door's listener never serves it. A 404 or 500 is `content-type: application/json` with the one `error` key; `POST /` is a 404 like any other route. Recorded with curl from the throwaway host (no tape: the checker speaks websocket only):

```text
GET /      200 text/html; charset=utf-8
<!doctype html><script>window.__WSP__ = {"tokenHash":"38b1aeba...5d55e","wsPath":"/ws","paired":true,"version":"0.3.8","statePath":"/tmp/specb-host-mN0k/state.json","productUsageOff":"build"};</script>
GET /x.js  404 application/json  {"error":"no route: GET /x.js"}
POST /     404 application/json  {"error":"no route: POST /"}
GET /a/    404 application/json  {"error":"no route: GET /a/"}
upgrade /ws/      400 Bad Request
upgrade /wsx      400 Bad Request
upgrade /ws?x=1   101 Switching Protocols
```

## The door

A socket is unauthenticated until its first frame passes, or until it redeemed a ticket at the upgrade (serve.ts:524-535). The frames a door accepts, in serve.ts:636-856:

| First frame | What it proves | Reply on success |
|---|---|---|
| `auth {token}` | the host token (constant-time compare, `runtime/src/devices.ts:23-27`); a device token (its sha256 compared against every stored device hash, `devices.ts:245-254`); a thread-scoped token, a device record with `scope: {kind:"thread", threadId, workspaceId, rootThreadId}`, own road only | `{id, ok:true, version, road}` |
| `pair.redeem {code, name}` | a pairing code: 8 characters of `0123456789ABCDEFGHJKMNPQRSTVWXYZ`, matched exactly, never normalized by the host (`protocol/src/wire/devices.ts:66,78,83`); codes live 10 minutes (`devices.ts:62`) | `{id, ok:true, deviceId, deviceToken}` with no `version` |
| `seal.open {nonce, ephemeral}` then one of the above inside the seal | the host proves its key to a client that pinned it (09) | `{id, ok:true, nonce, hostPublicKey, signature, ephemeral}` |
| `device.auth {publicKey, name, signature}` after `seal.open` | an account computer, by four checks in order (serve.ts:778-831) | `{id, ok:true, deviceId, deviceToken, version?, road?}` |
| `place.join` or `place.auth`, then `place.prove` | a joined computer; the socket is then handed to the place link (09) | |
| `?ticket=` on the upgrade | a one-use ticket of 24 random bytes, 300 s by default, spent even when expired (serve.ts:396,524-535); the socket is authed with the host's rights but stamped, so it is not on the own road | none: no frame is needed |

4.8. Before auth, the host MUST answer exactly as these tapes show and then close the socket with 4401 `unauthorized`: a non-JSON frame gets `invalid json`; a JSON value that is not an object gets the sentence `a request is one JSON object, not a bare value or an array` (`protocol/src/words/computer.ts:199-210`); a frame that fails its schema gets the schema refusal of 05; any other op, or a wrong token, gets `{id, ok:false, error:"unauthorized", kind:"auth"}` (serve.ts:643-689).

```tape
{"in":{"id":1,"op":"projects.list"}}
{"out":{"id":1,"ok":false,"error":"unauthorized","kind":"auth"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

```tape
{"in":{"token":"wrong","id":1,"op":"auth"}}
{"out":{"id":1,"ok":false,"error":"unauthorized","kind":"auth"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

```tape
{"in":"not json"}
{"out":{"id":null,"ok":false,"error":"invalid json"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

```tape
{"in":5}
{"out":{"id":null,"ok":false,"error":"a request is one JSON object, not a bare value or an array"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

4.9. Before auth a client MUST wait for each reply before its next frame: a frame that arrives while the door is still deciding the last one is refused like a wrong op (serve.ts:717-718). `seal.open` is the exception, so the token frame may follow it (serve.ts:729).

4.10. A socket that sends no first frame within 5 s (`AUTH_DEADLINE_MS`, `protocol/src/daemon-contract.ts:118`) MUST be closed with 4408 `no auth frame arrived in time` (`AUTH_DEADLINE_CLOSE`, `protocol/src/wire/limits.ts:112`; daemon-contract.ts:204). It is 4408, not 4401, so a client redials (`apps/web/src/protocol/client.ts:414-415`). A socket that sends more than 4096 bytes before passing (`PRE_AUTH_MAX_BYTES`, daemon-contract.ts:116), counted on the raw stream after the upgrade request so a half-sent frame counts (bytes that arrived in the same read as the upgrade request, `ws`'s `head` buffer, are not counted, since the counter is a `data` listener on the raw socket added after the upgrade, serve.ts:609), MUST be closed with 4401 `too many bytes before the auth frame` and no reply (serve.ts:588-593; `serve.test.ts:175-182`). Both limits come off once the socket is through the door, never apply to a ticket socket, and still apply after `seal.open` (serve.ts:604-607).

4.11. The auth reply MUST carry `version`, the host's release (`packages/host/src/version.ts:9-11`), and `road`, how this host was started: one of `app`, `up`, `verb`, `service`, `init` (`packages/host/src/restart.ts:15-21`). A second `auth` on an authed socket answers the same again without reading its token, a wrong or empty one included, and changes nothing about the socket (serve.ts:888-890; tape under rule 11.6). There is no hello frame, no protocol version and no capability exchange; 11 has the proposal for one.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"token":"<host-token>","id":2,"op":"auth"}}
{"out":{"id":2,"ok":true,"version":"0.3.8","road":"up"}}
```

4.12. After auth, a non-JSON or non-object frame gets the same reply as before auth and the socket stays open; so does a schema failure (`serve.test.ts:74-83`).

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":"not json"}
{"out":{"id":null,"ok":false,"error":"invalid json"}}
{"in":{"id":2,"op":"projects.list"}}
{"out":{"id":2,"ok":true,"projects":[]}}
```

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":"5"}
{"out":{"id":null,"ok":false,"error":"a request is one JSON object, not a bare value or an array"}}
{"in":"[1,2]"}
{"out":{"id":null,"ok":false,"error":"a request is one JSON object, not a bare value or an array"}}
{"in":"null"}
{"out":{"id":null,"ok":false,"error":"a request is one JSON object, not a bare value or an array"}}
```

4.30. Each frame MUST pass these steps in this order (serve.ts:636-864), and the first that refuses answers:
1. a socket already handed to the place link reads nothing more (serve.ts:637);
2. `JSON.parse` of the text (a sealed socket's frame is opened first, and a frame that does not open reads as invalid JSON): on failure `{id:null, ok:false, error:"invalid json"}`, and close 4401 if not authed (serve.ts:645-648);
3. the frame is an object, not `null` and not an array: else the non-object sentence, and close 4401 if not authed (serve.ts:653-655);
4. on a thread's socket, `op` is a string in `THREAD_OPS`; on a paired device's socket, in `DEVICE_OPS` (serve.ts:664-676), each refusal echoing the raw `id` (rule 4.31) with no kind and no close;
5. the schema, `RuntimeRequest.safeParse` (serve.ts:677-682): on failure the schema refusal of 05, echoing the raw `id`, and close 4401 if not authed;
6. if not authed, the door (serve.ts:685-856), in this order: a socket between `place.join` or `place.auth` and its prove takes only `place.prove`; a frame while the last is still being decided is refused (rule 4.9); then `seal.open`, `place.join` and `place.auth`, `place.prove` (refused here), `pair.redeem`, `device.auth`, and last `auth`; any other op is `unauthorized`;
7. the road is stamped (rule 4.15, serve.ts:863-864), then the op's handler runs inside the one catch (05).

The schema runs before the door, so before auth a frame that names no op the union holds gets the full schema refusal and then the close, and an `auth` with no `token` gets a schema refusal, not `unauthorized`. An empty token passes the schema and fails the compare:

```tape
{"in":{"id":1,"op":"auth"}}
{"out":{"id":1,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"string\",\n    \"received\": \"undefined\",\n    \"path\": [\n      \"token\"\n    ],\n    \"message\": \"Required\"\n  }\n]"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

```tape
{"in":{"token":"","id":1,"op":"auth"}}
{"out":{"id":1,"ok":false,"error":"unauthorized","kind":"auth"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

4.32. `pair.redeem` with a code the host does not hold MUST answer `that pairing code is not one this host is waiting for; run wsp host pair on the host for a fresh one` (`PAIR_CODE_REFUSAL`). Before auth it carries kind `auth` and the socket is closed 4401; after auth the op switch answers the same sentence with no kind and the socket stays open, since a socket through the door that redeems is asking for a second identity (serve.ts:906-909). Every way a redeem fails reads as this one sentence (a store error included, serve.ts:766-770). A code is spent by any lookup that finds it, expired or not, and an unknown code costs nothing: no count cap, no attempt limit, no rate limit (`spendCode`, `runtime/src/devices.ts:203-208`). `pair.redeem` takes `code` of at most 64 characters and `name` of at most 200 (`runtime.ts:60`).

```tape
{"in":{"id":1,"op":"pair.redeem","code":"AAAAAAAA","name":"x"}}
{"out":{"id":1,"ok":false,"error":"that pairing code is not one this host is waiting for; run wsp host pair on the host for a fresh one","kind":"auth"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"pair.redeem","code":"AAAAAAAA","name":"x"}}
{"out":{"id":2,"ok":false,"error":"that pairing code is not one this host is waiting for; run wsp host pair on the host for a fresh one"}}
{"in":{"id":3,"op":"projects.list"}}
{"out":{"id":3,"ok":true,"projects":[]}}
```

4.33. A frame is read as text whatever its opcode: the handler takes `String(raw)` of the bytes `ws` delivers and never reads `isBinary` (serve.ts:636; `packages/keys/src/index.ts:146-150`). So an empty frame, text or binary, is `invalid json`; a binary frame carrying JSON is handled exactly like a text frame; invalid UTF-8 in a binary frame is decoded with U+FFFD and fails as `invalid json`, while invalid UTF-8 in a text frame is a wire fault (rule 4.6). Unsealed replies are text frames; a sealed socket's replies are binary (serve.ts:627). The checker sends text only, so the binary cases have no tape; the empty text frame does:

```tape
{"in":""}
{"out":{"id":null,"ok":false,"error":"invalid json"}}
{"close":{"code":4401,"reason":"unauthorized"}}
```

4.34. These are every limit on a client socket, and the Rust host MUST add no other:

| Limit | Value | Applies | Where |
|---|---|---|---|
| bytes before the door passes | 4096, counted on the raw socket | until through the door; never on a ticket socket | rule 4.10; serve.ts:588-593 |
| first frame deadline | 5,000 ms, an unref'd timer | until through the door; `seal.open` does not clear it; never on a ticket socket | serve.ts:594-604 |
| largest frame | 100 MiB, close 1009 | always | `ws` `maxPayload` |
| fragments of one message | 16,384, close 1009 | always | `ws` `maxFragments` default |
| close handshake wait | 30 s | always, `ws`'s own | `ws` `closeTimeout` default |
| the event ring | 5,000 events | per process | rule 4.19 |

There is no cap on sockets, on subscriptions per socket (rule 4.22), on daemon channels or exec streams per socket, or on tickets and pairing codes outstanding; there is no backpressure (no read of `bufferedAmount` anywhere in serve.ts), so a slow reader makes the host buffer without bound; and there is no per-socket queue except the replies held behind `daemon.open` and the sign-in ops (serve.ts:1428; `sign-in-socket.ts:31`), which are unbounded. After-port note: tickets nobody redeems stay in the map for the life of the process, since entries leave only on redemption (serve.ts:525-526, 1174). Once through the door, a frame past 4096 bytes is fine:

```tape
{"in":{"token":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx","id":1,"op":"auth"}}
{"close":{"code":4401,"reason":"too many bytes before the auth frame"}}
```

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"projects.list","pad":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"}}
{"out":{"id":2,"ok":true,"projects":[]}}
```

4.35. `ticket.issue {purpose: "connect" | "relay"}` (`runtime.ts:53`; `wire/devices.ts:12`) MUST answer `{id, ok:true, ticket, expiresAt}`: `ticket` is 24 random bytes in base64url, 32 characters, and `expiresAt` is `now + 300000` in epoch ms (serve.ts:1167-1177). There is no TTL parameter; `ticketTtlMs` is a test option the host never passes (serve.ts:396). A socket the door stamped (a thread's token, a ticket socket, a relayed socket) is refused with `RELAY_TICKET_REFUSAL` and kind `ticket`. `connect` stamps the redeeming socket `here`, `relay` stamps it `relayed` (`TICKET_ORIGIN`, `wire/devices.ts:19`), and a ticket a paired device minted holds its socket to `DEVICE_OPS`. At the upgrade, `?ticket=` is taken out of the map before it is checked, so it is spent even when it fails; it passes while `now <= expiresAt`. A ticket that fails still gets the websocket upgrade (101) and is then closed 4401 `unauthorized` with no frame. A ticket socket is authed with no frame, has no first-frame deadline and no byte cap, counts as the host for `me` but is stamped, so it is off the own road. No tape: the ticket is random and the checker dials one fixed URL. Recorded from the throwaway host with a node client:

```text
{"id":2,"op":"ticket.issue","purpose":"connect"}  ->  {"id":2,"ok":true,"ticket":"<32 chars>","expiresAt":1792923880696}
{"id":2,"op":"ticket.issue","purpose":"x"}        ->  schema refusal, invalid_enum_value on ["purpose"]
ws /ws?ticket=<that ticket>, no auth frame:
  {"id":1,"op":"projects.list"}  ->  {"id":1,"ok":true,"projects":[]}
  {"id":2,"op":"pair.issue"}     ->  {"id":2,"ok":false,"error":"only a socket holding this host's own token may mint a pairing code; run wsp host pair on the computer the host runs on"}
  {"id":3,"op":"devices.list"}   ->  {"id":3,"ok":false,"error":"a socket let in on a ticket cannot see or change the devices paired with this host; run wsp host devices on the computer the host runs on","kind":"ticket"}
ws /ws?ticket=<the same ticket again>  ->  101, then close 4401 "unauthorized"
ws /ws?ticket=nope                     ->  101, then close 4401 "unauthorized"
```

4.36. `pair.issue {here?: true}` (`runtime.ts:57`) MUST answer `{id, ok:true, code, expiresAt, hostKey}` on a host-token socket on the own road, `hostKey` the `SHA256:` fingerprint of the host's place key, absent when no place door is wired; any other socket gets `PAIR_ISSUE_REFUSAL` with no kind (serve.ts:891-905). Issuing purges expired codes and nothing else does (`devices.ts:222-226`; the comment there says every restart clears them too, and the code does not). Codes live in `state.db` (collection `pairings`), so an unspent code survives a restart (rule 4.41). No tape: the code, the time and the key differ on every host. Recorded: `{"id":3,"ok":true,"code":"Z54BDATP","expiresAt":1792924017028,"hostKey":"SHA256:MpfXim+8PHckXjUp09h7ex529yQ4a+uvON39BCtniZw"}`.

## Envelopes

**Request.** One JSON object per websocket message: `{id, op, ...params, origin?}` (`RuntimeRequest`, `protocol/src/wire/runtime.ts:1025`). `id` is a string or a number (`reqId`, `protocol/src/wire/helpers.ts:83`). `op` selects one schema of the `RuntimeOp` discriminated union (`runtime.ts:51-1020`), 211 ops counting the 14 slate ops (`slateOps`, `runtime.ts:30-49`) and the five handshake ops of `protocol/src/wire/place-link.ts`. The op list with each op's params and handler is `RUNTIME_OPS` (`runtime.ts:1029`); the harness's per-op coverage counts against it.

**Success reply.** `{id, ok:true, ...payload}`, the payload's keys flat at the top level, never under `result`; some ops answer the bare `{id, ok:true}` (serve.ts:1405-1406,1535-1537). No `type` key.

**Failure reply.** `{id, ok:false, error, kind?, fix?, errors?, warnings?, left?}`, built at the one catch around the op switch (serve.ts:1932-1942). 05 has every field's rule.

**Event.** `{type, ...fields, seq?}`: no `id`, no `ok`. `EventUnion` is a discriminated union on `type` of 77 members, each with an optional positive `seq` (`protocol/src/wire/events-union.ts:18-96`). Clients tell an event from a reply by `type` present and `ok` absent (`apps/web/src/protocol/client.ts:462`) or by an id they are waiting on (`packages/host/src/verbs/client.ts:246-252`; `wsp-mcp/src/client.rs:440-449`).

4.13. The reply MUST echo the request's `id` with its JSON type: a string stays a string, a fraction stays a fraction. A request with no `id` fails its schema and is answered with `id: null`.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"op":"projects.list"}}
{"out":{"id":null,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_union\",\n    \"unionErrors\": [\n      {\n        \"issues\": [\n          {\n            \"code\": \"invalid_type\",\n            \"expected\": \"string\",\n            \"received\": \"undefined\",\n            \"path\": [\n              \"id\"\n            ],\n            \"message\": \"Required\"\n          }\n        ],\n        \"name\": \"ZodError\"\n      },\n      {\n        \"issues\": [\n          {\n            \"code\": \"invalid_type\",\n            \"expected\": \"number\",\n            \"received\": \"undefined\",\n            \"path\": [\n              \"id\"\n            ],\n            \"message\": \"Required\"\n          }\n        ],\n        \"name\": \"ZodError\"\n      }\n    ],\n    \"path\": [\n      \"id\"\n    ],\n    \"message\": \"Invalid input\"\n  }\n]"}}
{"in":{"id":"abc","op":"projects.list"}}
{"out":{"id":"abc","ok":true,"projects":[]}}
{"in":{"id":3.5,"op":"projects.list"}}
{"out":{"id":3.5,"ok":true,"projects":[]}}
```

4.31. The `id` MUST be read as the schema reads it and written back as JavaScript writes it. The schema takes any string, `""` included, or any JSON number (`reqId = z.union([z.string(), z.number()])`, `protocol/src/wire/helpers.ts:83`); a number is an IEEE double, so the Rust host parses a numeric id as `f64` and writes it with rule 4.25's formatter: `-0` comes back `0`, `12345678901234567890` comes back `12345678901234567000`, and `1e999` parses as Infinity and comes back `null` on an `ok: true` reply. Where the reply is written before the schema passes (the thread and paired doors, the schema refusal), the raw `id` is echoed whatever its JSON type, `true` or an object included, and a missing or `null` id is `null` (`parsed.id ?? null`, serve.ts:663,679). A refusal written by the door after the schema passed uses the parsed id (serve.ts:687).

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":"","op":"projects.list"}}
{"out":{"id":"","ok":true,"projects":[]}}
{"in":{"id":-1,"op":"projects.list"}}
{"out":{"id":-1,"ok":true,"projects":[]}}
{"in":{"id":0,"op":"projects.list"}}
{"out":{"id":0,"ok":true,"projects":[]}}
{"in":{"id":1e+21,"op":"projects.list"}}
{"out":{"id":1e+21,"ok":true,"projects":[]}}
{"in":"{\"id\":-0,\"op\":\"projects.list\"}"}
{"out":{"id":0,"ok":true,"projects":[]}}
{"in":"{\"id\":12345678901234567890,\"op\":\"projects.list\"}"}
{"out":{"id":12345678901234567000,"ok":true,"projects":[]}}
{"in":"{\"id\":1e999,\"op\":\"projects.list\"}"}
{"out":{"id":null,"ok":true,"projects":[]}}
{"in":{"id":true,"op":"projects.list"}}
{"out":{"id":true,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_union\",\n    \"unionErrors\": [\n      {\n        \"issues\": [\n          {\n            \"code\": \"invalid_type\",\n            \"expected\": \"string\",\n            \"received\": \"boolean\",\n            \"path\": [\n              \"id\"\n            ],\n            \"message\": \"Expected string, received boolean\"\n          }\n        ],\n        \"name\": \"ZodError\"\n      },\n      {\n        \"issues\": [\n          {\n            \"code\": \"invalid_type\",\n            \"expected\": \"number\",\n            \"received\": \"boolean\",\n            \"path\": [\n              \"id\"\n            ],\n            \"message\": \"Expected number, received boolean\"\n          }\n        ],\n        \"name\": \"ZodError\"\n      }\n    ],\n    \"path\": [\n      \"id\"\n    ],\n    \"message\": \"Invalid input\"\n  }\n]"}}
```

`origin` is part of the envelope's schema (`RuntimeRequest`, `runtime.ts:1025`), so a value outside the three words fails the schema, while an unknown key in the `auth` frame is dropped like any other (rule 4.14):

```tape
{"in":{"id":1,"op":"auth","token":"<host-token>","extra":{"a":1}}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"projects.list","origin":"relayed"}}
{"out":{"id":2,"ok":true,"projects":[]}}
{"in":{"id":3,"op":"projects.list","origin":"elsewhere"}}
{"out":{"id":3,"ok":false,"error":"[\n  {\n    \"received\": \"elsewhere\",\n    \"code\": \"invalid_enum_value\",\n    \"options\": [\n      \"here\",\n      \"relayed\",\n      \"paired\"\n    ],\n    \"path\": [\n      \"origin\"\n    ],\n    \"message\": \"Invalid enum value. Expected 'here' | 'relayed' | 'paired', received 'elsewhere'\"\n  }\n]"}}
```

4.37. Reply objects MUST carry their keys in the order the TS host writes them (rule 4.26):

| Reply | Keys, in order | Where |
|---|---|---|
| auth, and a second auth | `id, ok, version, road` (`road` absent when the host was started with none) | serve.ts:854,889; `released` built at `packages/host/src/server.ts:642` |
| door refusal | `id, ok, error, kind`, then `hostPublicKey, signature` on a place refusal only | serve.ts:687 |
| failure from the catch | `id, ok, error, kind, fix, errors, warnings, left`, each after `error` present only as 05 says | serve.ts:1934-1940; `exit.ts:44-47` |
| `events.subscribe` | `id, ok, seq, stream, gap` | serve.ts:1193 |
| `seal.open` | `id, ok, nonce, hostPublicKey, signature, ephemeral` | serve.ts:730 |
| `pair.redeem` | `id, ok, deviceId, deviceToken` | serve.ts:775 |
| `ticket.issue` | `id, ok, ticket, expiresAt` | serve.ts:1175 |
| `pair.issue` | `id, ok, code, expiresAt, hostKey` | serve.ts:903 |
| success from a helper family | `id, ok`, then the payload's keys as the helper returns them | serve.ts:878-883 |

4.14. A params object's unknown keys MUST be dropped silently, except under the schemas declared strict, which refuse them with an `unrecognized_keys` issue: `PreferencesPatch` (`protocol/src/views/preferences.ts:185-203`) and `AgentsTarget` (`protocol/src/agents-report.ts:32`), and any other `.strict()` schema the interface writer finds in the op's schema.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"projects.list","extra":1}}
{"out":{"id":2,"ok":true,"projects":[]}}
{"in":{"id":3,"op":"preferences.set","patch":{"theme":"dark","extra":1}}}
{"out":{"id":3,"ok":false,"error":"[\n  {\n    \"code\": \"unrecognized_keys\",\n    \"keys\": [\n      \"extra\"\n    ],\n    \"path\": [\n      \"patch\"\n    ],\n    \"message\": \"Unrecognized key(s) in object: 'extra'\"\n  }\n]"}}
```

4.15. `origin` on a frame (`"here" | "relayed" | "paired"`, `protocol/src/views/workspace.ts:270`) MUST be honored only on a socket nothing stamped: the host-token socket, and a device socket whose device record is `here: true` (the browser `wsp init` lets in, `bind` at serve.ts:618 leaves its `pairedRoad` false). A thread's socket, a paired device's socket and a ticket socket are stamped at the door, and the stamp wins over the frame (serve.ts:858-864). Absent reads as `here`.

4.16. Frames of one socket MUST be handled concurrently: each message starts its own handler (serve.ts:636-638), a slow op does not hold the next, and replies may come back in another order than the requests. 03 says how the Rust host keeps this without sharing state between handlers.

The checker waits for each reply before the next frame, so no tape shows two requests in flight. The command line does send them: `wsp usage --json`, recorded through a logging proxy between the verb and the throwaway host, sent two requests before either answered, and the second came back first:

```text
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"fresh":true,"id":2,"op":"usage.accounts"}}
{"in":{"range":"day","split":"agent","outside":true,"id":3,"op":"usage.used"}}
{"out":{"id":3,"ok":true,"used":{"range":"day","split":"agent","rows":[],"series":[...]}}}
{"out":{"id":2,"ok":true,"accounts":[]}}
```

## Who may send which op

4.17. A thread-scoped socket MAY send only the 50 ops of `THREAD_OPS` (`runtime.ts:1056-1132`); a paired device's socket only the 87 of `DEVICE_OPS` (`runtime.ts:1147-1242`). Both checks run by op name before the schema, so a refused op is refused whatever it carries, with no `kind`: `<op> is not a thread's to ask for; this request came in on the token of <thread>, and a thread's token starts work only under its own root and reads only that tree` (`runtime.ts:1247-1249`; serve.ts:664-667) and `<op> is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on` (`protocol/src/words/computer.ts:83-85`; serve.ts:672-676). A `relayed` ticket socket is not held to `DEVICE_OPS` (serve.ts:672).

4.18. Ops only the owner's own road may ask (places, account, devices, recipes, editor, ssh, relay, host restart) refuse any other socket with `kind: "ticket"` and the family's `*_TICKET_REFUSAL` sentence (serve.ts:872-876 and the sites 05 lists).

## Events

A socket receives no event until it sends `events.subscribe {after?, stream?, creates?}` (`runtime.ts:196-208`). The handler (serve.ts:1178-1197) does, in one synchronous step: read the ring since `after`; attach the socket's listener to the bus and to the doors that push outside it (forwards, init, doctor, release); send the reply `{id, ok:true, seq, stream, gap?}`; send each replayed event; and with `creates: true`, the last `workspace.creating` frame of each create in flight.

4.19. The event ring MUST be one per host process, 5,000 events across every workspace (`EVENT_RING_CAP`, `runtime/src/types/events.ts:21`). `seq` counts from 1 per process and `stream` is a random UUID minted per process (`events.ts:62-63`). Only `emit` takes a new `seq` (`{...event, seq: ++head}`, `events.ts:81-87`); `pass` sends to listeners with the current head and puts nothing in the ring (`events.ts:91-96`), and some events never touch the bus and carry no `seq` at all (`init.job`, `doctor.line`, `release.changed`, `exec.*`, `daemon.*`, `forward.open` and `forward.close` (`forwards.on(pass)`, serve.ts:1190), `workspace.sys`, `place.sys`, `app.procs*`, sign-in steps; serve.ts:1186-1192, `sys-socket.ts:41-42`).

4.20. `since(after, stream)` (`events.ts:74-80`) MUST answer: with no `after`, no replay and `gap` absent; otherwise a gap, with nothing replayed and `gap: true`, when `stream` is given and is not this process's, when `after` is past the head, or when `after` is older than the ring's oldest minus one; else every event after `after`, oldest first. Replayed events come after the reply. The cases are pinned in `packages/runtime/test/serve-replay.test.ts:83-243`.

4.38. `events.subscribe`'s params MUST be validated as the schema does (`runtime.ts:196-208`): `after` is an integer at least 0 (a fraction or a negative fails the schema), `stream` is any string with no UUID check, and `creates` a boolean. With `stream` alone and no `after`, there is no replay and no gap, whatever the stream says; with both, a stream that is not this process's is a gap:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"events.subscribe","after":-1}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"code\": \"too_small\",\n    \"minimum\": 0,\n    \"type\": \"number\",\n    \"inclusive\": true,\n    \"exact\": false,\n    \"message\": \"Number must be greater than or equal to 0\",\n    \"path\": [\n      \"after\"\n    ]\n  }\n]"}}
{"in":{"id":3,"op":"events.subscribe","after":1.5}}
{"out":{"id":3,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"integer\",\n    \"received\": \"float\",\n    \"message\": \"Expected integer, received float\",\n    \"path\": [\n      \"after\"\n    ]\n  }\n]"}}
{"in":{"id":4,"op":"events.subscribe","stream":"not-a-uuid"}}
{"out":{"id":4,"ok":true,"seq":0,"stream":"739c454e-f606-450e-b829-b36fa04e35b7"}}
{"in":{"id":5,"op":"events.subscribe","after":0,"stream":"not-a-uuid"}}
{"out":{"id":5,"ok":true,"seq":0,"stream":"739c454e-f606-450e-b829-b36fa04e35b7","gap":true}}
```

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"preferences.set","patch":{"theme":"dark"}}}
{"out":{"id":2,"ok":true,"preferences":{"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}}}
{"in":{"id":3,"op":"events.subscribe","after":0}}
{"out":{"id":3,"ok":true,"seq":1,"stream":"4f89d220-cddd-4bdc-9765-59ac4988edf7"}}
{"out":{"type":"preferences.changed","preferences":{"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false},"seq":1}}
```

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"preferences.set","patch":{"theme":"dark"}}}
{"out":{"id":2,"ok":true,"preferences":{"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}}}
{"in":{"id":3,"op":"events.subscribe","after":5}}
{"out":{"id":3,"ok":true,"seq":1,"stream":"9adcafc9-0c86-4853-a239-f54f71d89d9e","gap":true}}
```

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"events.subscribe","after":0,"stream":"00000000-0000-0000-0000-000000000000"}}
{"out":{"id":2,"ok":true,"seq":0,"stream":"558133ed-20a3-434c-8fb7-7e281c6fb08a","gap":true}}
```

4.21. An event an op caused MUST reach a subscribed socket **before** that op's reply, because the handler awaits its work and sends the reply after (pinned for `sessions.steer` at `serve.test.ts:580` and the doctor at `serve.test.ts:1340`). The exceptions, where the reply goes first: `events.subscribe` itself; `daemon.open`, whose early daemon frames are queued behind the reply (serve.ts:1429-1469); the sign-in ops (`heldBehindAnswer`, `runtime/src/sign-in-socket.ts:30-45`); `workspaces.exec`, whose `exec.output` and `exec.exit` follow (serve.ts:1688-1699); `host.restart`; `devices.revoke`, whose reply sits between the record and cutting the device's sockets (serve.ts:1162-1165); and `sessions.start`, which answers once the turn has started, so its `session.start` events may come first.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":0,"stream":"eab35c4f-d419-45b8-8d06-66a647f0e6a7"}}
{"in":{"id":3,"op":"preferences.set","patch":{"theme":"dark"}}}
{"out":{"type":"preferences.changed","preferences":{"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false},"seq":1}}
{"out":{"id":3,"ok":true,"preferences":{"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}}}
{"in":{"id":4,"op":"preferences.set","patch":{"theme":"light"}}}
{"out":{"type":"preferences.changed","preferences":{"theme":"light","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false},"seq":2}}
{"out":{"id":4,"ok":true,"preferences":{"theme":"light","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}}}
{"in":{"id":5,"op":"events.subscribe","after":1}}
{"out":{"id":5,"ok":true,"seq":2,"stream":"eab35c4f-d419-45b8-8d06-66a647f0e6a7"}}
{"out":{"type":"preferences.changed","preferences":{"theme":"light","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false},"seq":2}}
```

4.22. A second `events.subscribe` on one socket MUST attach a second listener, so every later event reaches that socket twice; the host has no guard and the clients guard themselves (`verbs/client.ts:379`, `wsp-mcp/src/client.rs:305-306`, `apps/web/src/protocol/client.ts:363`). This is a wrong-looking behaviour kept for parity and listed in 14 as an after-port ticket.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":0,"stream":"6696e0fa-7ff8-41ce-b6d8-a50c62fd6496"}}
{"in":{"id":3,"op":"events.subscribe"}}
{"out":{"id":3,"ok":true,"seq":0,"stream":"6696e0fa-7ff8-41ce-b6d8-a50c62fd6496"}}
{"in":{"id":4,"op":"preferences.set","patch":{"sendWith":"mod-enter"}}}
{"out":{"type":"preferences.changed","preferences":{"theme":"system","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"mod-enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false},"seq":1}}
{"out":{"type":"preferences.changed","preferences":{"theme":"system","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"mod-enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false},"seq":1}}
{"out":{"id":4,"ok":true,"preferences":{"theme":"system","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"mod-enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}}}
```

4.23. Each event MUST pass the socket's filter before it is sent (serve.ts:1185-1187): `seenBy` (`runtime/src/projects/workspaces.ts:1269-1283`) lets a socket with no thread scope see everything, and a thread-scoped socket only an event whose `askedBy` names its thread or root, or that names a workspace in its tree; an event naming no workspace and no asker (`preferences.changed`, `place.*`, `usage.*`, `agents.changed`, `release.changed`, `host.notice`, `init.*`, `doctor.line`) never reaches a thread socket (`serve.test.ts:1409-1415`). `costMoved` (`runtime/src/status.ts:320-332`) sends a `workspace.cost` only when its figures moved since the last one this subscription sent for that workspace. `shownTo` (`workspaces.ts:1285-1297`) cuts a `workspace.status` event's `status.tree.children` to what a thread socket may drive, dropping `tree` when none remain.

4.24. A closed socket MUST release everything it held: its bus listeners, status watch, daemon channels, sys watches, slate holds and exec streams (serve.ts:565-581).

4.39. Every reply MUST go through one send that drops the frame silently when the socket is not open (`if (ws.readyState !== ws.OPEN) return`, serve.ts:624-628). A dropped send writes nothing and, on a sealed socket, does not move the seal's counter. A refusal's log line (05, rule 5.7) is still written for a dropped send, because the log runs after the send whatever it did (serve.ts:629-634).

4.40. A client that closes while its op runs MUST NOT stop the op: handlers are never cancelled, they run to the end and write their state, and only their reply is dropped (rule 4.39). The close runs the socket's detaches (serve.ts:577-581): `workspaces.exec` tears its stream down through one (serve.ts:1681-1684), and `daemon.open` checks the socket after its dial and closes the channel it just opened (serve.ts:1450-1453). The checker cannot close mid-op, so this has no tape; `serve.test.ts` holds the detaches.

4.41. The host's stop MUST run in this order (`packages/host/src/server.ts:774-795`; serve.ts:1966-1979): the back door, the broker, the reap timer, the release reader and the relay close; then the socket server: the upgrade listener comes off, every client socket and place socket gets close 4001 `stopping` at once, a 1,000 ms timer terminates whatever has not finished its close, and the server waits until it holds no clients; then the HTTP servers' connections are closed; then `rt.close()`; then the stand-in guests. `ws.close` sets the socket `CLOSING` at once, so from the first moment of a stop every in-flight reply is dropped (rule 4.39) while its handler goes on until `rt.close()`. What a client sees across a restart:

| Thing | Across a restart | Where it lives |
|---|---|---|
| `seq`, `stream`, the event ring | gone: `seq` starts at 1 again under a new `stream`, so a resubscribe with the old pair is a gap | memory, `eventBus()` per `createRuntime` (`runtime.ts:505`); `serve-replay.test.ts:186,234` |
| the host token | new on every start, so a socket or a client holding the old one is refused | the `host-token` file (11, rule 11.6) |
| tickets | gone | memory (serve.ts:398) |
| pairing codes, devices, revoked keys | kept | `state.db` collections `pairings`, `devices`, `revoked-keys` (`runtime/src/devices.ts:11-19`) |
| subscriptions, daemon channels, exec streams, slate holds | gone with the socket | memory |
| the port | kept when named; a new one with `--port 0`, except that a verb host restarting itself respawns on its own port | 11, rule 11.7 |

A tape runs on one host, so none spans a restart.

## How JSON is written

The TS host writes every frame with `JSON.stringify`. Clients parse it, and the harness compares it structurally, but some of `JSON.stringify`'s habits are behaviour a person or a client sees.

4.25. Numbers MUST be written as JavaScript writes them (`Number.prototype.toString`): an integral value with no `.0`, `1e+21` for 10^21, `1e-7` for 10^-7, and `-0` as `0`. serde_json writes `3.0`, `1e21` and `-0.0` for an `f64`. Every integral field is an `i64` or `u64` in Rust; every other number goes through one JavaScript-style formatter in `wsp-wire` (the `ryu-js` crate implements the same algorithm).

4.26. Key order MUST be JavaScript's everywhere the host writes JSON: every object's keys in the order the TS code wrote them, a parsed object's keys in the order they were read, except that keys reading as array indexes (`"2"`, `"10"`) come first in ascending numeric order (checked on node 22: `{"10":1,"b":2,"2":3}` writes as `{"2":3,"10":1,"b":2}`). serde_json without its `preserve_order` feature sorts the keys of every `Value` map, so it is on for every host crate, set once in the workspace's dependency line; a struct declares its fields in the order the TS code writes them; a `Record` is an `IndexMap`; and one helper in `wsp-wire` moves index-like keys first where a map's keys can be numbers (ports, counts). The pilot found an object `env` value reordered by a port without `preserve_order`, and no TS test covers it. Key order is visible in:
- the agent config files the host edits (`~/.claude.json`, the opencode and gemini settings): the edit is made in place on the text (`editJson`, `packages/catalog/src/mcp.ts:723-727`), but the entry it inserts is `JSON.stringify` of an object wsp built (`rendered` and its callers, `mcp.ts:592,648,677`), an `env` value among them, and the fallback writes the person's whole file back with `JSON.stringify(root, null, 2)` (`mcp.ts:812`), where sorting would reorder every key the person wrote;
- every file of 02's table written as JSON (`host.lock`, whose order rule 2.2 fixes, `relay.json`, `hosts/*.json`, `place-host-key.json`) and every `collections.json` row in `state.db`, which the harness diffs after a run;
- the command line's `--json` output and the MCP tools' answers, which the host's reply objects flow into and which `packages/host/test/mcp-record.test.ts` and `mcp-binary.test.ts` hold byte for byte (10);
- a `Record` the app draws in its key order (`Object.entries` of an `env`, a map of ports).

4.27. A field the TS code leaves `undefined` MUST be absent, never `null`; a field the TS code sets to `null` MUST be `null`. Optional fields are `Option` with `skip_serializing_if` unless the TS code writes `null` for them.

4.28. Escapes MUST be `JSON.stringify`'s: `\"`, `\\`, `\b`, `\f`, `\n`, `\r`, `\t`, other controls below U+0020 as `\u00xx` in lower case, and nothing else (non-ASCII, `/`, U+2028 and U+2029 raw). serde_json escapes the same set. A lone surrogate is the one difference (08, rule 8.5). The command line's `--json` output also escapes DEL and the C1 controls (`escapeC1`, `protocol/src/exit.ts:51`); the websocket does not.

## Close codes

| Code | Reason | When | Where |
|---|---|---|---|
| 4401 | `unauthorized` | any refusal at the door, a bad or spent ticket, a revoked device's sockets | serve.ts:529,620,688 |
| 4401 | `too many bytes before the auth frame` | 4.10 | serve.ts:592 |
| 4408 | `no auth frame arrived in time` | 4.10 | serve.ts:598 |
| 4001 | `stopping` | the host closes every client and place socket as it stops, with 1 s grace before terminating (`STOP_GRACE_MS`) | serve.ts:338,1966-1979; `limits.ts:109` |
| 1000 | `this host no longer holds that place`, or the replaced-link reason | the place link (09) | `runtime/src/places/link.ts`, `places/helpers.ts:236` |
| 1009 | | a frame past 100 MiB, by `ws` | `ws/lib/receiver.js` |

4.29. The Rust host MUST close with these codes and reasons. Clients act on them: 4001 tells a verb the turn goes on (`verbs/client.ts:202,254`), 4408 makes the web client redial.
