# serve_door: the door every socket passes, and the refusal log

serve.ts:349-367, 444-449, 493-848 at `65f6b89c7`. Rust: `src/serve/door.rs` (`Socket::on_text`, `Socket::opened`,
`Socket::bytes_before_auth`, `refused_line`, `who_is`). Tests: `tests/serve_door.rs`.

A socket arrives with what its upgrade said (`Upgrade`: the path and query, the peer, the host's `own_road` reading)
and a `FrameSink`. Until it is let in it may send exactly the frames below; once let in, every frame goes to the op
units through `answer_frame` (serve_places, serve_workspaces, serve_ops).

## Opening

- D1. `serve_runtime` refuses an empty `auth_token` with `serveRuntime refuses to start without an auth token`
  before it listens (serve.ts:385).
- D2. **A ticket in the query** (`?ticket=T`, serve.ts:516-527): the ticket is taken out of the table whether it
  is good or not (single use, spent even when expired). Unknown, a purpose `TICKET_ORIGIN` does not answer, or
  `now() > expiresAt` closes the socket `4401 "unauthorized"` and reads no frame. At exactly `expiresAt` it is
  still good (the compare is `>`). A good ticket lets the socket in at once: `stamped = TICKET_ORIGIN[purpose]`
  (`connect` is `here`, `relay` is `relayed`), `paired_road = ticket.paired`, `me = host`, no deadline, no byte cap.
- D3. **The deadline** (serve.ts:586-593): a socket not let in at open gets `auth_deadline_ms` (default
  `AUTH_DEADLINE_MS` 5,000; `ServeOptions.auth_deadline_ms` shortens it in tests). Passing it takes both limits off
  and closes `AUTH_DEADLINE_CLOSE` 4408 with `DAEMON_AUTH_DEADLINE_PASSED` ("no auth frame arrived in time"). The
  timer holds no process open (unref).
- D4. **The byte cap** (serve.ts:579-585, 600-603): the raw bytes the socket delivers after its upgrade, frame
  headers and masks included, counted as they arrive and not as frames assemble, so an unfinished frame counts.
  More than `PRE_AUTH_MAX_BYTES` (4,096; exactly 4,096 passes) takes both limits off and closes 4401 with
  `DAEMON_PRE_AUTH_BYTES_EXCEEDED` ("too many bytes before the auth frame"); the socket is answered nothing.
- D5. `through_door()` takes both limits off (serve.ts:596-599). It runs at: an auth frame that passed, a
  `pair.redeem` that paid, a `device.auth` that admitted, a `place.join` or `place.auth` answered (before its reply
  is sent), the deadline, the cap, and the socket's close. **Not** at `seal.open`: a socket that agreed a key is
  still held to both.

## Every frame, in this order (serve.ts:628-675)

- D6. A socket handed over to a place link reads nothing more (`handed_over`, serve.ts:629, 547).
- D7. The text is opened under the seal when one stands (`open_frame`, wsp-store `keys`), then parsed as JSON.
  Either failing sends `{"id":null,"ok":false,"error":"invalid json"}`; a socket not let in is then closed 4401
  `unauthorized`. A plaintext frame after the seal began is a failure to open (serve.ts:635-641).
- D8. JSON that is not an object (`null`, `7`, `"a string"`, `true`, `[]`) sends
  `{"id":null,"ok":false,"error":REQUEST_NOT_AN_OBJECT}`; closed 4401 if not let in, kept if let in, and the
  next request is answered (serve.ts:645-649).
- D9. **A thread's own token** (`by` set): an op that is not a string or not in `THREAD_OPS` is refused by name
  before its shape is read: `{"id":<asked id>,"ok":false,"error":thread_op_refusal(op or "that frame",
  by.threadId, rt.workspaces().thread_place(by))}`, no kind, socket kept. `<asked id>` is the frame's own `id` as
  sent, any JSON value, `null` when absent (serve.ts:654-659).
- D10. **A paired computer** (`paired_road && stamped != relayed`): an op outside `DEVICE_OPS` is refused the same
  way with `device_held_refusal(op or "that frame")` (serve.ts:664-668).
- D11. The schema (`parse_runtime_request`, wsp-wire): a refusal sends `{"id":<id or null>,"ok":false,
  "error":<zod's ZodError.message, SchemaError::issue_text>}` and logs the issues line (`issues_line`) as the
  sentence (D21); a socket not let in is closed 4401 (serve.ts:669-674). Unknown keys are dropped, so a spread of
  the rest of a request carries only the fields its op declares.

## Before the socket is let in (serve.ts:677-848)

`refuse(error, signed?)` sends `{"id":..,"ok":false,"error":..,"kind":"auth",...signed}` then closes 4401
`unauthorized` (serve.ts:678-681).

- D12. While a place's prove is awaited, only `place.prove` is taken; any other op is `refuse("unauthorized")`.
  The prove clears `proving` first, asks `place_door.prove(place_id, msg, expect, from, now)`; an error becomes
  its message. A refusal refuses. Otherwise: `handed_over = true`, the socket stops being read, the reply
  `{"id":..,"ok":true,"device"?}` is sent, then `place_door.attach(place_id, socket, report, from, now, seal)`
  (serve.ts:684-708). Nothing the socket sends after the reply reaches this door.
- D13. `deciding`: a second frame while the first is decided is `refuse("unauthorized")` (serve.ts:709-710).
- D14. `seal.open`: a seal already agreed refuses `unauthorized`; a runtime with no place door refuses
  `SEAL_UNSERVED`; `answer_challenge(SEAL_CLIENT, nonce, ephemeral)` answering nothing refuses `unauthorized`.
  Otherwise `deciding` clears, the reply `{"id","ok":true,"nonce","hostPublicKey","signature","ephemeral"}` goes
  **in the clear**, and only then the seal and `seal_expect` are set (serve.ts:711-726).
- D15. `place.join` / `place.auth`: a seal already agreed refuses `unauthorized`. The door's `join(msg, from, now)`
  or `auth(msg, now)`; an error refuses with its message (no place door: `NO_PLACE_DOOR`); nothing answered
  refuses `PLACE_CODE_REFUSAL`; a refusal refuses with its sentence and its signed fields. Otherwise `proving =
  { place_id, expect }` (the join's `reply.placeId`, the auth's `msg.placeId`), `deciding` clears, `through_door()`,
  the reply `{"id","ok":true,...reply,"notice"?}` in the clear, then the seal is set (serve.ts:727-755).
- D16. `place.prove` with nothing to prove refuses `unauthorized` (serve.ts:756).
- D17. `pair.redeem`: no device door, a store that failed, or a code not held all refuse `PAIR_CODE_REFUSAL`, one
  sentence for every reason. Paid: let in, `through_door()`, `bind(device)`, reply
  `{"id","ok":true,"deviceId","deviceToken"}`, with no release fields (serve.ts:757-769).
- D18. `device.auth`, four checks in order (serve.ts:770-823): no `admitted` door, no signer, or no device door
  refuses `DEVICE_ACCOUNT_UNSERVED`; no `seal_expect` (no `seal.open` on this socket) refuses `unauthorized`; the
  listing's row for `key_fingerprint(publicKey)`, asked once more through `admitted.refresh()` (its failure
  swallowed) when the first read lacks it, else `DEVICE_AUTH_REFUSAL`; one admission whose signer key (the
  signer's own, or a device admitted through the account with that fingerprint, read off `devices.list()`) verifies
  `device_admission_transcript(fingerprint, by, issuedAt)`, else `DEVICE_AUTH_REFUSAL` (an error reading is the
  same); the socket's own `seal_expect` signed by `publicKey`, else `DEVICE_AUTH_REFUSAL`; `devices.refuses(fp)`
  true or failing refuses `DEVICE_REVOKED_REFUSAL`; `admit_account(name, via, now)` failing refuses
  `DEVICE_AUTH_REFUSAL`. Admitted: let in, `through_door()`, `bind`, reply
  `{"id","ok":true,"deviceId","deviceToken",...released}`. `via` is `{kind:"account", relayDeviceId: row.id,
  fingerprint, publicKey, admittedBy}` in that order.
- D19. Anything but `auth` refuses `unauthorized` (serve.ts:824). `auth`: `who_is(token)` (`safe_equal` with the
  host token is the host; else the device door's `match`; no door or no match is nobody; an error is nobody).
  Nobody refuses `unauthorized`. A scoped device token on a road the host does not call its own
  (`own_road(upgrade)` false) refuses `SCOPED_TOKEN_ROAD_REFUSAL` before `bind` and before `seen`. Otherwise let in,
  `through_door()`; a device is bound, its thread (`thread_of`) sets `stamped = road` and `by`, and
  `devices.seen(id, now)` runs without being awaited, its failure swallowed; the host's token sets `me = host`.
  Reply `{"id","ok":true,...released}` (`version`, then `road`, each only where given) (serve.ts:825-847).
- D25. `bind(device)` (serve.ts:608-614): `paired_road = scope is None && here != true`; `me = device`; the socket
  is held under the device id so `revoke_device` cuts it (`4401 unauthorized`), and let go at close.

## The refusal log (serve.ts:349-367, 616-626)

- D20. Every frame sent with `ok: false` logs one line through `opts.log`, after the send:
  `refused <op> kind=<kind>: <sentence>`. `<op>` is the frame's op when it is a string in `RUNTIME_OPS`, else
  `frame` (a key sent as the op never names the line). `<kind>` is the payload's string `kind`, else `none`.
- D21. `<sentence>` is the schema's issues line where the schema refused (D11), else the payload's `error`; its
  first line (up to the first `\n`), with every secret value of the request blanked (`redacted` over the distinct
  `request_secrets(frame)`), then cut by `marked_cut` at 400 UTF-16 units with ` (cut N characters)`. Blanked
  before the cut.
- D22. More than `SCANNED_SECRETS` (256) distinct secret values: the line is
  `refused <op> kind=<kind>: (sentence withheld, <n> secret values)` and nothing is scanned.
- D23. A secret value shorter than 4 characters once trimmed is not blanked (`redacted`); a padded one is blanked
  both padded and trimmed.
- D24. The line never carries the token of a refused `auth` frame.

## Origin (invariant 3, serve.ts:505-538, 850-856)

- D30. `road = "paired"` when `paired_road && stamped != relayed`; else `stamped`; else the frame's own `origin`.
- D31. `origin = { origin: road, by }` when the socket is a thread's and a road stands, else `road` alone.
- D32. A thread token's road is the one its mint recorded (`here` beside the host, `relayed` on a machine, and
  `relayed` for a scoped record with no road), whatever `origin` its frames send.
- D33. A paired computer's frames read `paired` even when they send `origin: "here"`; a relay ticket minted by a
  paired computer stays `relayed`.
- D34. `own_road()` is `stamped is None`: a ticketed socket and a thread's socket are never the person's own road.

## Close (serve.ts:564-573)

- D35. A socket's close runs every detach in push order once, clears them, and lets go of its device hold. Its
  daemon channels are closed by the detach pushed at open (serve.ts:564-567).

## Rules against tests (`tests/serve_door.rs`)

| Rule | Tests | TS case |
|---|---|---|
| D1 | `an_empty_auth_token_refuses_to_serve` | serve.ts:385 |
| D2 | `a_ticket_lets_one_socket_in_once`, `a_ticket_past_its_expiry_closes_4401`, `a_ticket_at_its_expiry_still_opens`, `a_relay_ticket_stamps_relayed_and_a_connect_ticket_here` | serve.test.ts:311, 332 |
| D3 | `a_silent_socket_closes_4408_with_the_daemons_sentence`, `a_socket_let_in_is_held_to_no_deadline` | serve.test.ts:169, 184 |
| D4 | `more_bytes_than_the_cap_close_4401_and_answer_nothing`, `exactly_the_cap_passes`, `a_socket_through_the_door_takes_any_size` | serve.test.ts:175, 184 |
| D5 | `seal_open_leaves_both_limits_standing` | serve.ts:711-726 |
| D7 | `invalid_json_before_auth_closes_and_after_auth_stays` | serve.ts:635-641 |
| D8 | `a_bare_value_before_auth_is_refused_under_a_null_id_and_closes`, `bare_values_on_the_owners_socket_are_refused_and_it_stays` | serve.test.ts:89, 102 |
| D9 | `a_threads_token_is_refused_an_op_outside_its_list_by_name` | spawn-guard.test.ts (door cases) |
| D10 | `a_paired_device_is_refused_an_op_outside_its_list`, `a_paired_device_on_a_relay_ticket_reads_the_relays_gate` | pairing.test.ts |
| D11 | `a_malformed_op_on_an_authed_socket_is_refused_and_it_stays`, `a_schema_refusal_before_auth_closes` | serve.test.ts:74, 67 |
| D12 | `a_place_that_proved_is_handed_over_and_reads_nothing_more`, `a_prove_refused_closes_4401` | places-link.test.ts |
| D13 | `a_second_frame_while_the_first_is_decided_closes` | serve.ts:709 |
| D14 | `seal_open_answers_in_the_clear_then_seals_every_frame`, `a_second_seal_open_closes_4401`, `seal_open_after_auth_is_refused`, `seal_open_with_no_place_door_says_seal_unserved` | serve.test.ts:216, 233, 265 |
| D15 | `a_place_handshake_on_a_sealed_socket_closes`, `a_join_code_not_held_says_the_code_refusal` | serve.test.ts:247 |
| D16 | `a_prove_with_nothing_to_prove_closes` | serve.ts:756 |
| D17 | `a_redeem_with_no_device_door_reads_as_a_code_not_held`, `a_paid_redeem_lets_in_and_binds_without_release_fields` | pairing.test.ts |
| D18 | `device_auth_refuses_each_check_in_order`, `device_auth_admits_and_names_the_release` | pairing.test.ts |
| D19 | `a_wrong_token_closes_4401`, `the_first_op_not_auth_closes_4401`, `auth_names_the_release_and_road_only_where_given`, `a_scoped_token_off_the_hosts_road_is_refused_before_seen` | serve.test.ts:37, 54, 67 |
| D20-D24 | `a_refusal_is_logged_once_as_op_kind_and_first_line`, `a_refused_auth_never_logs_its_token`, `the_sentence_is_cut_at_400_and_says_so`, `a_schema_refusal_logs_its_issues_line`, `a_padded_key_is_blanked`, `an_undeclared_op_logs_as_frame`, `short_secret_values_stay_readable`, `more_than_256_secrets_withhold_the_sentence`, `a_deeply_nested_secret_is_refused_and_logged`, `a_ticketed_socket_refusals_carry_kind_ticket` | serve-refusal.test.ts:61-164 |
| D25 | `a_revoked_device_loses_every_socket_it_held` | pairing.test.ts |
| D30-D34 | `invariant_3_a_threads_frames_are_stamped_with_its_mint_road`, `invariant_3_a_paired_device_reads_paired_whatever_it_sends`, `invariant_3_a_relay_ticket_minted_by_a_paired_device_stays_relayed`, `invariant_3_an_unstamped_socket_carries_its_clients_word` | spawn-guard.test.ts, pairing.test.ts |
| D35 | `a_close_runs_every_detach_once_in_order` | serve.ts:569-573 |

Ported from the TS cases only in part: the pairing, spawn-guard and places-link files drive a whole host; the door
half of each case is ported here with a fake device door and a fake place door, and the area half is the area's.
