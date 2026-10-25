# reach
Source: packages/runtime/src/reach.ts (265 lines) at 65f6b89c7. Tests: packages/runtime/test/reach.test.ts (14 cases). Rust: `src/reach.rs`, tests `tests/reach.rs` against `tests/support/fake_daemon.rs` and `tests/support/tcp_proxy.rs`.

## What it does
The host's held link to one daemon: dial with the token as the first frame, then the ritual (`ports.watch`, `inbox.watch`, `inbox.rescan`), then an app-level ping every beat. A cut socket redials with backoff and runs the ritual again, because events pushed during a gap are gone. A 4401 that refused the token is terminal. A socket handed over already open (a place's link) is never redialled: its close is the end. A sealed link seals every frame out and opens every frame in.

## Behaviour
1. `daemon_ws_url(url)` (:49-54): WHATWG parse; `https:` becomes `wss:`, `http:` becomes `ws:`, anything else is kept; the serialized URL comes back, so an empty path gains `/` and the query (the edge's `pt_token`) rides along. An unparsable URL is an error, as `new URL` throws.
2. `connect_daemon(o)` refuses synchronously (:64-65): neither `preview_url` nor `socket` gives `connectDaemon needs a previewUrl to dial or an open socket to take over`; `preview_url` without `token` gives `connectDaemon needs the daemon's token to dial a previewUrl`. Defaults: `heartbeat_ms` 10_000 (:56), `backoff_ms(attempt)` is `link_backoff_ms(attempt, random)` with the draw from the `Ids` seam (rule 3.16).
3. Status (:81-86) starts unset (`status()` is `None`) and every change calls `on_status` once; setting the status it already has does nothing. Values used: `connecting`, `live`, `reauth-needed`, `dead`.
4. `request(op, params)` (:96-105): no socket or one not open fails at once with `daemon unreachable`. Else the id is the next of a counter from 1 that is never reset across redials; the frame is `params`' keys in order, then `id`, then `op`; a key `id` or `op` already in `params` keeps its place and takes the reach's value, so a caller can never take another frame's reply (:102). Unsealed it goes as text; sealed as `seal.seal(text)`, a binary frame.
5. A reply (:137-157): the frame is opened (`open_frame(seal, raw)`) and parsed. A failure to open or parse: on a sealed link the socket is closed `1002` with `SEAL_REFUSAL` (the redial follows); unsealed it is dropped. A JSON-number `id` matching a pending request settles it: `ok === true` resolves with the whole object; anything else rejects with `refused(msg)` (:162-171): message `String(msg.error)`, or `daemon error` when absent or null; `kind` when it parses as a `MachineErrorKind`; `status` when a number; `code` when a string. Any other frame that parses as a `DaemonEvent` goes to `on_event`; one that does not is dropped.
6. The ritual (:187-204), on every socket: the auth request `auth` with `{token}` only when dialling (the frame is `{"token":..,"id":n,"op":"auth"}`), then `ports.watch`, `inbox.watch`, `inbox.rescan`, each awaited in turn. Any failure ends the ritual silently. After success, if the socket is still the current one and the reach not closed: `attempt = 0`; `connections += 1`; `stats.reconnects = connections - 1`; the heartbeat starts; status `live`; `ready` resolves.
7. Heartbeat (:112-135): every `heartbeat_ms` on the clock: if the last ping has no answer yet, the socket is terminated and nothing else this beat; else `awaiting = true`, `pings_sent += 1`, request `ping`; an `ok: true` answer sets `pongs_received += 1` and clears `awaiting`; a refused or failed ping leaves `awaiting` set, so the next beat cuts the socket. Stopping clears `awaiting`.
8. Dial (:206-227): closed does nothing; status `connecting`; the socket becomes current. Its close, if it is still current: heartbeat stopped, every pending request fails `connection lost`; closed reach stops there; `daemon_refused_token(code, reason)` sets `reauth-needed` and rejects `ready` with `daemon rejected token (4401 <reason>)` and never redials; any other close (a dial that failed included) goes to the reconnect.
9. Reconnect (:173-185): closed does nothing. A taken socket: status `dead`, `ready` rejects `the place's link closed`. A dialled one: status `connecting`, `attempt += 1`, a timer of `backoff_ms(attempt)` runs the dial. So a cut socket reads `connecting, live, connecting, live`, with no repeat.
10. Take (:232-245): the socket becomes current with no `connecting` status and no auth frame; the ritual starts at `ports.watch`. Its close (any code, 4401 included) after the heartbeat stop and the flush goes to the reconnect, which makes it `dead`.
11. `close()` (:255-263): closed; the retry timer cancelled; heartbeat stopped; every pending request fails `client closed`; the socket closed with 1000; status `dead`; `ready` rejects `client closed`.
12. `ready` settles once: a resolve or reject after it settled does nothing (:88-94). An unawaited rejection is never a crash.
13. `stats()` is a copy of `{pings_sent, pongs_received, reconnects}` (:254).

## Edge cases
- `daemon_ws_url("ws://127.0.0.1:7070")` is `ws://127.0.0.1:7070/`; `https://abc-7070.preview.getsolari.com/?pt_token=edge` is `wss://abc-7070.preview.getsolari.com/?pt_token=edge` with no `token` param added.
- The daemon closing 4401 with `DAEMON_AUTH_DEADLINE_PASSED` is not a refusal: the reach redials, goes live, and `reconnects` stays 0 because the first socket never finished its ritual (`dials_again_when_the_daemons_auth_deadline_passed...`).
- A daemon that is down: every dial fails and redials after `backoff_ms`, for as long as the reach is held; it goes live once the port answers (`keeps_retrying_while_the_daemon_is_unreachable...`).
- After `close()`, a late close event of the socket stops the heartbeat and flushes nothing and redials nothing.

## Behaviour nobody wrote down
- Rule 3.15: the heartbeat interval and the retry timer go through the clock (the TS uses raw `setInterval`, `setTimeout`); the interval is a self re-arming schedule.
- Rule 3.16: the backoff's jitter draw comes from the `Ids` seam.
- Rule 3.3: `on_event` and `on_status` run with no borrow of the reach's state held; either may call `request` or `close`.

## Decisions
1. A taken socket is `TakenSocket`, built from any `WebSocketStream` over a boxed stream; Node's `resume()` of a paused socket (:235) has no Rust counterpart, since nothing reads the stream before the reach does.
2. A refused request rejects with `ReachError { message, kind, status, code }`; link failures (`daemon unreachable`, `connection lost`, `client closed`) are `ReachError` with only a message.
3. `DaemonReach` is a trait (01's seam table, rule 1.3); `connect_daemon` answers `Rc<dyn DaemonReach>`.

## After the port
- A ritual step the daemon refuses with `ok: false` while the socket stays open ends the ritual silently: no heartbeat, no redial, status stays `connecting` and `ready` never settles (:193-196).
- A taken socket closed with 4401 reads `dead`, not `reauth-needed` (Behaviour 10).

## Tests
| Rule | Test (`tests/reach.rs`) |
|---|---|
| 1 | `turns_a_preview_url_into_a_wss_url_carrying_the_edge_token_and_never_ours`, `keeps_ws_urls_as_they_are`, `an_unparsable_preview_url_is_refused` |
| 7 | `heartbeats_at_the_configured_interval_and_gets_acks_back`, `a_ping_left_unanswered_for_a_beat_cuts_the_socket_and_redials` |
| 6, 9 | `survives_a_cut_socket_reconnects_resubscribes_and_rescans_the_inbox` |
| 3 | `reports_connecting_then_live_on_first_connect`, `cycles_connecting_live_across_a_cut_socket_without_duplicate_emissions` |
| 8 | `rejects_ready_and_reports_reauth_needed_on_a_bad_token`, `dials_again_when_the_daemons_auth_deadline_passed_while_this_hosts_loop_stood_still_and_goes_live`, `surfaces_a_post_establishment_4401_as_reauth_needed_and_stops_retrying` |
| 11, 12 | `reports_dead_once_the_client_closes`, `close_fails_every_pending_request_with_client_closed`, `a_close_before_the_first_connect_rejects_ready` |
| 9 | `keeps_retrying_while_the_daemon_is_unreachable_and_connects_once_it_is_back` |
| 10 | `takes_a_socket_somebody_else_opened_without_an_auth_frame_of_its_own_and_is_dead_when_that_socket_goes` |
| 4 | `stamps_its_own_id_and_op_over_any_the_callers_params_carry` |
| 5 | `a_refused_request_carries_the_daemons_kind_status_and_code`, `a_frame_that_is_no_daemon_event_is_dropped` |
| 2 | `refuses_to_be_built_with_neither_a_route_to_dial_nor_a_socket_to_take` |
| 4, 5 | `a_sealed_link_sends_binary_frames_and_cuts_on_a_frame_that_does_not_open` |
