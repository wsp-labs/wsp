# 04 The wire protocol

What a client and the host say to each other on the websocket: the transport, the door every socket passes, the three envelopes, events, and how JSON is written. Errors have their own chapter (05). The host's own links to daemons and joined computers are 09. Every `file:line` is at `a3b870a6e`; "serve.ts" means `packages/runtime/src/serve.ts`.

Every tape in this chapter was recorded from a fresh TS host by `checker/check.mjs` and is replayed by it; 00 says how to read one.

## Transport

The host has no websocket port of its own. `serveRuntime` (serve.ts:382) makes a `ws` server in `noServer` mode (serve.ts:479-499) and the host attaches it to its HTTP servers (`packages/host/src/server.ts:641-650`):

| Listener | Address | Port | Where |
|---|---|---|---|
| The app server | `--listen`, default `127.0.0.1` | `--port`, default 4400; `0` asks the OS, and a verb that starts a host always passes `--port 0` | `server.ts:529,691-693`; `protocol/src/app-ports.ts:10,19`; `host-start.ts:66` |
| The door joined computers dial | `0.0.0.0` | the app port plus 20 (`PLACE_PORT_OFFSET`), bound when a computer is on record or a door opens; with `--port 0` the bound port is saved in `door-port` beside the state | `server.ts:537-600,750`; `app-ports.ts:13-15` |
| The door's loopback twin, off Linux only | `127.0.0.1` | the door's port | `server.ts:542-543,585-588` |

4.1. The websocket path MUST be exactly `/ws` (`WS_PATH`, `app-ports.ts:23`), matched on the URL's pathname. Any other path, `/ws/` included, MUST be answered with the raw bytes `HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n` and the socket destroyed (serve.ts:489-496).

4.2. An upgrade whose `Origin` header is present and non-empty MUST be refused with the raw bytes `HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n` unless the Origin's hostname equals the hostname of `http://` plus the `Host` header, ports ignored (`originAllowed`, `server.ts:263-269`; serve.ts:482-488). No Origin is allowed: it is a tool on this computer.

4.3. The upgrade reads the query parameter `ticket` (serve.ts:512), the `cf-connecting-ip` and `cf-ray` headers (either one means the request came through the relay's connector, `server.ts:219-221`), the peer address with `::ffff:` stripped (`app-ports.ts:50`), and which listener accepted it. It reads no Authorization header, no subprotocol and no cookie; the token never rides the URL (serve.ts:1-7).

4.4. A socket is on the **own road** when it came to the app server, not through the connector, from a loopback peer (`ownRoad`, `server.ts:240-242`). Only an own-road socket may authenticate with a thread's token (serve.ts:836).

4.5. The host sets no ping timer and no idle timeout on a client socket; `ws` answers a client's ping by itself (`autoPong`). No permessage-deflate. The largest frame is `ws`'s default 100 MiB, past which `ws` closes 1009 (serve.ts:478-480; `ws@8.21.3` `websocket-server.js:74`).

4.6. A frame that is wrong at the websocket level (unmasked, invalid UTF-8 in a text frame, a bad opcode) MUST end the socket with no reply, after one warning line `socket from <peer> dropped on a wire fault: <message>` (serve.ts:508-511; `packages/runtime/test/serve.test.ts:136-152`).

The same ports serve plain HTTP (`server.ts:498-524`): `GET /` is the web app's `index.html` with its one `window.__WSP__ = {...}` boot line swapped for the host's (`server.ts:204-212,485-496`: `wsPath`, `paired`, `version`, and on the loopback page only `tokenHash`, the sha256 hex of the host token, and `statePath`); `GET <file>` is a file of the web bundle; any other GET with no extension and no trailing slash is the same page; anything else is `404 {"error":"no route: <METHOD> <path>"}`; a throw is `500 {"error": message}`. There are no JSON routes at this commit: `bearerOf` (`server.ts:273`) has no caller.

4.7. The Rust host MUST serve the same routes with the same boot line. A host started with no built web app refuses to start with `web app not built: <path> is missing (pnpm --filter @wsp/web... build)` (`server.ts:206`); a page with no boot line refuses with `<path> has no window.__WSP__ boot line to replace` (`server.ts:210`). The checker's hosts serve a one-line stub page for this reason.

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

4.10. A socket that sends no first frame within 5 s (`AUTH_DEADLINE_MS`, `protocol/src/daemon-contract.ts:118`) MUST be closed with 4408 `no auth frame arrived in time` (`AUTH_DEADLINE_CLOSE`, `protocol/src/wire/limits.ts:112`; daemon-contract.ts:204). It is 4408, not 4401, so a client redials (`apps/web/src/protocol/client.ts:414-415`). A socket that sends more than 4096 bytes before passing (`PRE_AUTH_MAX_BYTES`, daemon-contract.ts:116), counted on the raw stream after the upgrade request so a half-sent frame counts, MUST be closed with 4401 `too many bytes before the auth frame` and no reply (serve.ts:588-593; `serve.test.ts:175-182`). Both limits come off once the socket is through the door, never apply to a ticket socket, and still apply after `seal.open` (serve.ts:604-607).

4.11. The auth reply MUST carry `version`, the host's release (`packages/host/src/version.ts:9-11`), and `road`, how this host was started: one of `app`, `up`, `verb`, `service`, `init` (`packages/host/src/restart.ts:15-21`). A second `auth` on an authed socket answers the same again (serve.ts:888-890). There is no hello frame, no protocol version and no capability exchange; 11 has the proposal for one.

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

## Envelopes

**Request.** One JSON object per websocket message: `{id, op, ...params, origin?}` (`RuntimeRequest`, `protocol/src/wire/runtime.ts:1025`). `id` is a string or a number (`reqId`, `protocol/src/wire/helpers.ts:83`). `op` selects one schema of the `RuntimeOp` discriminated union (`runtime.ts:51-1011`), 211 ops counting the 14 slate ops (`runtime.ts:23-42`) and the five handshake ops of `protocol/src/wire/place-link.ts`. The op list with each op's params and handler is `RUNTIME_OPS` (`runtime.ts:1029`); the harness's per-op coverage counts against it.

**Success reply.** `{id, ok:true, ...payload}`, the payload's keys flat at the top level, never under `result`; some ops answer the bare `{id, ok:true}` (serve.ts:1405-1406,1535-1537). No `type` key.

**Failure reply.** `{id, ok:false, error, kind?, fix?, errors?, warnings?, left?}`, built at the one catch around the op switch (serve.ts:1932-1942). 05 has every field's rule.

**Event.** `{type, ...fields, seq?}`: no `id`, no `ok`. `EventUnion` is a discriminated union on `type` of 71 members, each with an optional positive `seq` (`protocol/src/wire/events-union.ts:18-96`). Clients tell an event from a reply by `type` present and `ok` absent (`apps/web/src/protocol/client.ts:462`) or by an id they are waiting on (`packages/host/src/verbs/client.ts:246-252`; `wsp-mcp/src/client.rs:440-449`).

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

4.14. A params object's unknown keys MUST be dropped silently, except under the schemas declared strict, which refuse them with an `unrecognized_keys` issue: `PreferencesPatch` (`protocol/src/views/preferences.ts:185-203`) and `AgentsTarget` (`protocol/src/agents-report.ts:32`), and any other `.strict()` schema the interface writer finds in the op's schema.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"projects.list","extra":1}}
{"out":{"id":2,"ok":true,"projects":[]}}
{"in":{"id":3,"op":"preferences.set","patch":{"theme":"dark","extra":1}}}
{"out":{"id":3,"ok":false,"error":"[\n  {\n    \"code\": \"unrecognized_keys\",\n    \"keys\": [\n      \"extra\"\n    ],\n    \"path\": [\n      \"patch\"\n    ],\n    \"message\": \"Unrecognized key(s) in object: 'extra'\"\n  }\n]"}}
```

4.15. `origin` on a frame (`"here" | "relayed" | "paired"`, `protocol/src/views/workspace.ts:270`) MUST be honored only on a socket nothing stamped: the host-token socket. A thread's socket, a paired device's socket and a ticket socket are stamped at the door, and the stamp wins over the frame (serve.ts:858-864). Absent reads as `here`.

4.16. Frames of one socket MUST be handled concurrently: each message starts its own handler (serve.ts:636-638), a slow op does not hold the next, and replies may come back in another order than the requests. 03 says how the Rust host keeps this without sharing state between handlers.

## Who may send which op

4.17. A thread-scoped socket MAY send only the 50 ops of `THREAD_OPS` (`runtime.ts:1056-1132`); a paired device's socket only the 87 of `DEVICE_OPS` (`runtime.ts:1147-1242`). Both checks run by op name before the schema, so a refused op is refused whatever it carries, with no `kind`: `<op> is not a thread's to ask for; this request came in on the token of <thread>, and a thread's token starts work only under its own root and reads only that tree` (`runtime.ts:1247-1249`; serve.ts:664-667) and `<op> is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on` (`protocol/src/words/computer.ts:83-85`; serve.ts:672-676). A `relayed` ticket socket is not held to `DEVICE_OPS` (serve.ts:672).

4.18. Ops only the owner's own road may ask (places, account, devices, recipes, editor, ssh, relay, host restart) refuse any other socket with `kind: "ticket"` and the family's `*_TICKET_REFUSAL` sentence (serve.ts:872-876 and the sites 05 lists).

## Events

A socket receives no event until it sends `events.subscribe {after?, stream?, creates?}` (`runtime.ts:196-208`). The handler (serve.ts:1178-1197) does, in one synchronous step: read the ring since `after`; attach the socket's listener to the bus and to the doors that push outside it (forwards, init, doctor, release); send the reply `{id, ok:true, seq, stream, gap?}`; send each replayed event; and with `creates: true`, the last `workspace.creating` frame of each create in flight.

4.19. The event ring MUST be one per host process, 5,000 events across every workspace (`EVENT_RING_CAP`, `runtime/src/types/events.ts:21`). `seq` counts from 1 per process and `stream` is a random UUID minted per process (`events.ts:62-63`). Only `emit` takes a new `seq` (`{...event, seq: ++head}`, `events.ts:81-87`); `pass` sends to listeners with the current head and puts nothing in the ring (`events.ts:91-96`), and some events never touch the bus and carry no `seq` at all (`init.job`, `doctor.line`, `release.changed`, `exec.*`, `daemon.*`, `workspace.sys`, `place.sys`, `app.procs*`, sign-in steps; serve.ts:1186-1192, `sys-socket.ts:41-42`).

4.20. `since(after, stream)` (`events.ts:74-80`) MUST answer: with no `after`, no replay and `gap` absent; otherwise a gap, with nothing replayed and `gap: true`, when `stream` is given and is not this process's, when `after` is past the head, or when `after` is older than the ring's oldest minus one; else every event after `after`, oldest first. Replayed events come after the reply. The cases are pinned in `packages/runtime/test/serve-replay.test.ts:83-243`.

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

## How JSON is written

The TS host writes every frame with `JSON.stringify`. Clients parse it, and the harness compares it structurally, but some of `JSON.stringify`'s habits are behaviour a person or a client sees.

4.25. Numbers MUST be written as JavaScript writes them (`Number.prototype.toString`): an integral value with no `.0`, `1e+21` for 10^21, `1e-7` for 10^-7, and `-0` as `0`. serde_json writes `3.0`, `1e21` and `-0.0` for an `f64`. Every integral field is an `i64` or `u64` in Rust; every other number goes through one JavaScript-style formatter in `wsp-wire` (the `ryu-js` crate implements the same algorithm).

4.26. Key order MUST be JavaScript's everywhere the host writes JSON: every object's keys in the order the TS code wrote them, a parsed object's keys in the order they were read, except that keys reading as array indexes (`"2"`, `"10"`) come first in ascending numeric order (checked on node 22: `{"10":1,"b":2,"2":3}` writes as `{"2":3,"10":1,"b":2}`). serde_json without its `preserve_order` feature sorts the keys of every `Value` map, so it is on for every host crate, set once in the workspace's dependency line; a struct declares its fields in the order the TS code writes them; a `Record` is an `IndexMap`; and one helper in `wsp-wire` moves index-like keys first where a map's keys can be numbers (ports, counts). The pilot found an object `env` value reordered by a port without `preserve_order`, and no TS test covers it. Key order is visible in:
- the agent config files the host edits (`~/.claude.json`, the opencode and gemini settings; `packages/catalog/src/mcp.ts:721-727`): the edit is made in place on the text, but the entry it inserts is `JSON.stringify` of an object wsp built, an `env` value among them, and the fallback writes the person's whole file back with `JSON.stringify(root, null, 2)` (`mcp.ts:812`), where sorting would reorder every key the person wrote;
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
