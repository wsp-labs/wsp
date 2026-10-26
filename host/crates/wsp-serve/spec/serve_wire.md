# serve_wire: the listener, the upgrades, the socket's transport and the server's own calls

serve.ts:135-347, 375-492, 1899-1934 at `65f6b89c7`. Rust: `src/serve.rs` (`serve_runtime`, `ServeOptions`,
`RuntimeServer`, the host door traits) and `src/serve/socket.rs` (`FrameSink`), `src/serve/listener.rs` and `src/serve/wsframe.rs` (the websocket as the `ws`
library speaks it). Tests: `tests/serve_wire.rs` and `tests/review_door.rs`, the tests in this crate that open a real
websocket (tokio-tungstenite and raw sockets on loopback).

## Options and doors

- W1. `ServeOptions` (serve.ts:139-220) keeps the TS's fields, `Option` where the TS has `?`. Defaults:
  `host` `LOOPBACK` (127.0.0.1), `ticket_ttl_ms` 300,000, `pair_ttl_ms` `PAIR_CODE_TTL_MS` (600,000),
  `auth_deadline_ms` `AUTH_DEADLINE_MS` (5,000), `now` the wall clock in ms, `released` none (serve.ts:386-389).
- W2. `attach` (several HTTP servers in the TS) is the host's to wire: the server answers
  `RuntimeServer::upgrade(head, io)` for each upgrade any of the host's servers took. With no `port` and no
  attach, the runtime listens nowhere and `port` is 0.
- W3. `origin_allowed`, `own_road` and `peer_of` read an upgrade's `UpgradeHead` (path and query, headers, peer).
  Absent: every origin allowed, every road the host's own, the peer the socket's own address through
  `peer_address` (serve.ts:465-469, 496).

## The listener (serve.ts:470-492, 1901-1911)

- W4. With `port` set, the runtime binds `host:port` itself (port 0 picks one) and answers every path there; an
  upgrade `origin_allowed` refuses is answered `401` before any frame (the ws library's refusal; the client reads
  an unexpected server response, with ws's body `Unauthorized`). `RuntimeServer.port` is the bound port; with no
  `port`, `opts.port` or 0. The head is read here as ws's own HTTP server reads it: a request that is no upgrade is
  answered `426 Upgrade Required`; a method other than GET is refused 405 `Invalid HTTP method`, and a missing or
  wrong Upgrade header, key, version or subprotocol list 400 with ws's sentence (W11); the bytes the client sent
  after the head are its first frames.
- W5. An attached upgrade `origin_allowed` refuses is answered `HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n`
  and the socket ends; a path whose pathname is not `WS_PATH` (`/ws`; the query is ignored) is answered
  `HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n`; then ws's `handleUpgrade` checks (W11). The method is the
  host's to refuse, since `UpgradeHead` carries none. Both write after an error listener is on, so a peer that resets
  cannot end the process (serve.ts:473-490).
- W6. A frame wrong at the wire (an unmasked client frame, a bad opcode, invalid UTF-8) warns one line through
  wsp-core's `console::warn`: `socket from <peer> dropped on a wire fault: <the ws library's sentence>`. The socket is
  closed with the fault's own code (1002, 1007 for invalid UTF-8, 1009 past the ceiling) and an empty reason, then
  cut without waiting for the peer's answer. The host goes on answering the next socket (serve.ts:500-503). The
  sentences are ws's (`Invalid WebSocket frame: MASK must be set`, `invalid opcode 3`, `invalid status code 1005`,
  `Max payload size exceeded`).

## The websocket (ws 8.21.3, `src/serve/wsframe.rs`)

- W11. The upgrade is checked in ws's order (`handleUpgrade`): a method other than GET 405 `Invalid HTTP method`; an
  Upgrade header that is not `websocket` 400 `Invalid Upgrade header`; a key that is not 22 base64 characters and
  `==` 400 `Missing or invalid Sec-WebSocket-Key header`; a version other than 13 or 8 400 `Missing or invalid
  Sec-WebSocket-Version header` (with `Sec-WebSocket-Version: 13, 8`); a subprotocol list that is not distinct tokens
  400 `Invalid Sec-WebSocket-Protocol header`. Each refusal carries its sentence as the body. The 101 names the first
  subprotocol offered.
- W12. The largest frame, and the largest message of fragments, is 100 MiB (ws's `maxPayload`), checked when a frame's
  length is read: past it the socket is closed 1009 `Max payload size exceeded`. A frame of 16 MiB is read.
- W13. Frames are read as ws's receiver reads them: RSV bits, opcodes, FIN on control frames, control payloads of at
  most 125 bytes, the mask, close codes (1000-1014 but 1004-1006, and 3000-4999), UTF-8 of text and of a close reason.
  A message of more than 16,384 fragments is closed 1008 `Too many message fragments` (ws's `maxFragments`). `tests/ws_corpus.rs`
  replays 3,480 recorded cases of ws 8.21.3's own receiver (`tests/fixtures/ws_receiver/`, remade by `gen.mjs`) and
  compares each event, payload, close code and sentence to the byte.
  The peer's close is echoed with its code and reason (a close with no code, with none); a receiver that has read a
  close or a fault reads nothing more. A ping is answered with a pong while the socket is open.
- W14. Every message of one read reaches the door in order before the door counts the bytes of that read toward the
  4,096 of the pre-auth cap (serve.ts:579-603), and each frame's handler runs as far as its first wait before the next
  frame is read, as ws emits its messages synchronously. So a second frame that arrives with an `auth` frame finds
  the door deciding (`unauthorized`, 4401), and a reply a handler gives without waiting goes out before the close
  that follows it in the same read.
- W15. A socket that redeemed a ticket is through the door at once: neither the 4,096 bytes nor the auth deadline
  apply to it (serve.ts:526, 594-599).
- W16. After the host's own close frame (a deadline, a refusal, a stop) a peer that never answers is cut 30 s later
  (ws's `closeTimeout`). A stop cuts a client that stopped reading even while frames are queued for it.

## The server's own calls (serve.ts:1913-1933)

- W7. `authorize(token)`: `None` or `""` names nobody; otherwise `who_is` (serve_door.md, D19), so the JSON routes
  and the socket read one token store.
- W8. `revoke_device(id)`: the device door's `revoke(id)` (no door throws `NO_DEVICE_DOOR`), then every socket
  that device holds is closed `4401 "unauthorized"`; answers what the door answered. The op `devices.revoke` sends
  its reply between the record and the cut (serve_places.md, P27).
- W9. `close()`: stops taking upgrades from the attached servers, closes every open socket with
  `HOST_STOPPING_CLOSE` 4001 and the reason `stopping`, cuts any socket still open after `STOP_GRACE_MS` (1,000),
  and resolves once the listeners are closed. A client reading the close sees 4001, never an abnormal close.
- W10. A send to a socket that is not open is dropped (serve.ts:617). A sealed socket's text is sealed before it
  is written (serve.ts:619).

## Rules against tests (`tests/serve_wire.rs`)

Generated from the rule each test names in its first comment; 11 tests, and the review's (`tests/review_door.rs`,
`tests/review_panics.rs`) for W6 and W11-W16.

| Rule | Tests | TS case |
|---|---|---|
| W1 | `the_defaults_are_the_protocols_numbers` | serve.ts:327-331, 386-389 |
| W2 | `a_runtime_given_port_zero_binds_one_and_names_it`, `a_runtime_with_no_port_and_no_attach_names_port_zero` | serve.test.ts:37; serve.ts:1911 |
| W4 | `a_runtime_given_port_zero_binds_one_and_names_it`, `the_own_port_answers_any_path`, `an_upgrade_the_hosts_reading_refuses_is_closed_on_its_own_port_and_an_attached_one` | serve.test.ts:287; serve.test.ts:37; serve.ts:470 (no path option on the runtime's own server) |
| W5 | `an_upgrade_the_hosts_reading_refuses_is_closed_on_its_own_port_and_an_attached_one`, `an_attached_upgrade_on_another_path_is_answered_400` | serve.test.ts:287; serve.ts:481-488 |
| W6 | `an_unmasked_frame_is_said_in_one_line_and_the_next_socket_is_answered` | serve.test.ts:136 |
| W7 | `authorize_names_nobody_for_an_empty_token_and_the_host_for_its_own` | serve.ts:1917 |
| W8 | `revoke_device_cuts_every_socket_that_device_holds` | pairing.test.ts |
| W9 | `a_stop_closes_every_client_with_4001`, `a_client_that_never_answers_the_close_is_cut_after_the_grace` | serve.test.ts:44; serve.ts:1923-1926 |

W3 (the upgrade readings' defaults) rides every listener test with none given; W10 is pinned in serve_door.rs, `seal_open_answers_in_the_clear_then_seals_every_frame`.
