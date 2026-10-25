# daemon_channel
Source: packages/runtime/src/daemon-channel.ts (170 lines) at 65f6b89c7. Tests: packages/runtime/test/daemon-channel.test.ts (8 cases). Rust: `src/daemon_channel.rs`, tests `tests/daemon_channel.rs` against the fake daemon in `tests/support/fake_daemon.rs`.

## What it does
One dial of one daemon for one client of the host: dial, send the auth frame first, resolve on its reply. No redial, no subscribe ritual (that is `reach`). Replies match sends by id; every other frame, the hello included, goes to `on_event` in arrival order. Failures are told apart by kind: a door that answered the upgrade with a status (`refused`), a token the daemon refused (`reauth`), and a link that gave out with no answer (no kind).

## Behaviour
1. `open_daemon_channel(o)` dials `daemon_ws_url(o.url)` (reach.rs) (:75). The connect deadline is `o.connect_timeout_ms` or `CONNECT_TIMEOUT_MS = 15_000` (:61, :73), on the injected clock (`o.clock`, default the real clock, rule 3.15).
2. Ids count from 1 per channel; the auth frame takes id 1 (:77, :125). A sent frame is the caller's object with `id` set: the caller's keys in order, then `id` last; a caller's own `id` key keeps its place and takes the new value (:96 `{...frame, id}`). The auth frame is exactly `{"op":"auth","token":<token>,"id":1}`.
3. `send` on a socket that is not open fails at once with `Unanswered("daemon unreachable")` (:92), before taking an id.
4. On open the auth frame goes out (:124-135). Its reply with `ok === true` resolves the channel and stops the deadline. Any other reply rejects with a plain error, no kind: the reply's `error` as JS `String()` (a string as is), or `the machine refused the daemon link` when `error` is absent or null (:131).
5. Each text or binary frame is read as UTF-8 and parsed as JSON; one that does not parse is dropped silently (:137-144). A parsed value whose `id` is a JSON number equal to a pending id resolves that send with the whole object, `ok: false` included: a refusal comes back as the daemon wrote it, never as an error (:145-150). Everything else goes to `on_event` as parsed (:151), in arrival order, before or after the channel resolved; a non-object JSON value goes too.
6. A door that answers the upgrade with a non-101 status (:108-122): its body is read until it ends or errors, chunks appended while the text held is under `DOOR_BODY_CAP = 512` UTF-16 units, then cut to 512 units. The refusal is `DaemonDoorError { status, message }`, `kind() == "refused"`, `status` the HTTP status (0 when none). Message: the body's first line (`split("\n")[0]`, then `js_trim`); empty gives `the connection was refused with <status>`, else `the connection was refused with <status>: <line>` (:42-46). The refusal is remembered, so the close that follows rejects with it rather than as a dead link (:114, :164).
7. The deadline firing before settling (:84-89): settled, the socket terminated, reject with `Unanswered("daemon connect timed out after <ms>ms")`. The close that follows still settles `closed` and rejects pending sends.
8. On close (:156-168), in order: the deadline stops; every pending send fails with `Unanswered("connection lost")`; `closed` settles with `{code, reason}` (reason as UTF-8); then, only if the open never settled: the remembered door refusal; else `daemon_refused_token(code, reason)` gives `DaemonTokenError`, `kind() == "reauth"`, message `the machine refused this host's daemon token (4401 <reason>)` (:54); else `Unanswered("the link to the computer ended before it opened")` carrying `{code, reason}` as its cause, never in its words (:167).
9. `close()` closes with 1000 (:102). `closed` settles exactly once, for any end, `close()` included, and every await of it reads the same value (:63, :207 of the doc).
10. `is_daemon_unanswered(e)` is true exactly for the no-kind link failures of 3, 7 and 8 (:31-33); a door refusal, a token refusal and a refused auth reply are not.

## Edge cases
- A port nothing listens on: the dial fails, the close (1006) rejects `Unanswered("the link to the computer ended before it opened")`, no kind, not a door error (`a_port_nothing_listens_on_and_a_door_that_never_answers_both_reject_plainly...`).
- The daemon's auth deadline passing while this host stood still: the daemon closes 4401 with `DAEMON_AUTH_DEADLINE_PASSED`, which `daemon_refused_token` reads as no refusal, so it is `Unanswered`, and a second dial with the same token opens (`an_auth_deadline_that_passed_while_this_hosts_loop_stood_still...`).
- A send in flight when the socket is cut fails `connection lost`; a send after that fails `daemon unreachable` (`closed_settles_once_with_the_code_and_reason...`).
- A reply whose `id` is `1.0` matches pending id 1, as `JSON.parse` makes it the number 1; `"1"` (a string) does not and goes to `on_event`.

## Behaviour nobody wrote down
- Rule 3.15: the connect deadline is on the clock.
- Rule 3.3: `on_event` is called with no borrow of the channel's state held; it may call `send` or `close`.

## Decisions
1. `on_event` takes `serde_json::Value`, since the TS hands on whatever parsed.
2. Errors are one enum `ChannelError` with `kind()`; the TS classes map one to one: `Unanswered`, `Door`, `Token`, `Refused` (the plain error of Behaviour 4).
3. tokio-tungstenite folds a non-101 answer into `Error::Http(response)`; the implementer must still read the door's body up to the cap (Behaviour 6), not only what the handshake buffered.

## After the port
- A refused auth reply (Behaviour 4) leaves the socket open: nothing closes it, and the channel's caller never gets a handle to close it.

## Tests
| Rule | Test (`tests/daemon_channel.rs`) |
|---|---|
| 1, 2, 4, 5 | `dials_once_authenticates_and_round_trips_the_daemons_own_replies_with_the_hello_as_the_first_event`, `the_auth_frame_is_op_token_id_in_that_order_and_takes_id_one`, `a_callers_own_id_keeps_its_place_and_takes_the_channels_number` |
| 5 | `hands_a_refusal_back_as_the_daemon_wrote_it_with_the_typed_code_and_no_throw`, `a_frame_that_does_not_parse_is_dropped_and_the_next_one_still_lands` |
| 4 | `an_auth_reply_that_is_not_ok_rejects_plainly_with_the_daemons_words_or_the_default` |
| 8 | `a_token_the_daemon_refuses_rejects_as_a_token_error_carrying_its_4401_sentence`, `an_auth_deadline_that_passed_while_this_hosts_loop_stood_still_is_a_link_that_ended_never_a_refused_token` |
| 6 | `a_door_that_answers_the_upgrade_with_a_status_rejects_as_a_door_error_carrying_the_status_and_the_bodys_first_line`, `a_door_with_an_empty_body_says_only_its_status`, `a_door_body_past_the_cap_is_cut_at_512_units` |
| 7, 8, 10 | `a_port_nothing_listens_on_and_a_door_that_never_answers_both_reject_plainly_with_no_kind_to_read` |
| 3, 8, 9 | `closed_settles_once_with_the_code_and_reason_and_a_send_in_flight_when_the_socket_goes_rejects`, `a_close_the_caller_asked_for_settles_closed_too` |
