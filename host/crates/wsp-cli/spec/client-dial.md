# client-dial

Source: `packages/host/src/verbs/client.ts` lines 1-452 (452 lines) at 65f6b89c7. Tests: `dial-envelope.test.ts` (1), `host-release.test.ts` (8), parts of `verbs-exec-host.test.ts` (5 of 14), `cold-start.test.ts` (3, host side, not ported). Rust: `src/verbs/client.rs`, "The socket"; tests `tests/client_dial.rs`.

## What it does
One websocket to the host per command line. The first frame carries the token (`auth`), or the seal's `seal.open` first when the aim pins a host key; then request and reply by id. A refusal of the token's frame is classed as auth by its `kind` or by a 4401 close within 500 ms. The auth answer's `version` must be this wsp's unless the line dials any release. A close fails every waiting request; 4001 says the turn goes on. Beside the socket: the wait that redials a host that stopped under it, the race of work against the socket, and the hold on pushed frames.

## Behaviour
1. `host_address` (158-167): a url aim is `ws_url_of(url)` and its token or ""; an alias is the record's url and device token; here is the lock's `dial_address` (no address or a wildcard is 127.0.0.1) and `authority(address, port)`, with the token file beside the state trimmed. No lock: plain error `no wsp host is serving <s>; start one with wsp up --state <s>` (no kind, exit 1). Lock but no token file: auth refusal `the host's token file is missing: <dir>/host-token`.
2. `dial_once` order (207-215): a here aim calls `held_or_started(state, start, say)` first (say defaults to stderr); a url aim with no token and no admit is refused `address_not_paired_line(url)` + `READ_THE_HOSTS` (usage) before any socket.
3. One deadline (`opts.deadline_ms` or `dial_window_ms(aim)`, 5,000 here or loopback, else 15,000) covers the open, the seal and the first answer (273-275, 361-374). Past it: `no_answer_within(where, ms)` = `the host at <where> did not answer: nothing came back within <ms> ms`, kind `unreachable`. A failed open is `no_answer_refusal(where, <reason>)`, kind `unreachable` (336-339). `where` is the url's authority for here (`127.0.0.1:7777`) and the aim's address as given for any other aim (272).
4. Frames (259-265): `{...params, id, op}` serialized as JS does: the params' keys in order, an `id` or `op` already among them overwritten where it stands, else appended `id` then `op`. Ids count from 1 for every frame the socket sends, the auth and seal frames included.
5. Replies (235-252): a frame whose numeric `id` matches a waiting request settles it; every other frame (no id, a string id, an id nobody waits on) goes to every `on_frame` sink in arrival order. A frame that does not open under the seal, or comes clear after it, closes the socket with 1002 and `SEAL_REFUSAL`.
6. A reply with `ok !== true` (266) fails with `error` if a string else `<op> failed`, `kind` only if a string, `errors` and `warnings` only if arrays; `left` never.
7. A close (254-257) fails every waiting request with a plain error (no kind): `HOST_STOPPING_LINE` for code 4001, else `HOST_CLOSED_LINE`. `close_words` reads the same. `stopped_under` (393-395) is true only when the socket is already gone with 4001; it never waits.
8. The token's frame refused (310-326): no kind, wait for a close up to 500 ms; a 4401 then means auth, else the refusal stands as it came. A kind other than auth stands. Auth on an admit is `device_auth_old_host_line(where)` when the kind was missing, else the host's own sentence; on an alias, `device_refused_line(alias)`; else the host's sentence, kind auth.
9. Release (330-335): skipped when `any_release`, when `version` is not a string, or when it equals `VERSION`. Else `release_refusal(VERSION, theirs, host, road)` where host is `State(state)` for here, else `Where{aim_name(aim), is_loopback(hostname of aim_address)}`, road the answer's `road` when a string.
10. Seal (286-304, 344-345): `seal.open {nonce, ephemeral}`; any answer that does not parse as `SealOpenReply`, a fingerprint other than the pinned one, or a bad signature is one auth refusal `pair_key_refusal(where)`, and no token frame is sent.
11. `dial_host` (172-198): alias with empty `device_token` and a `host_key`: admit at once with this computer's device key (`read_device_key_pair(home)`), name `hostname()`; on an auth refusal of the token's frame and not already admitting, dial once more admitting. A dial that proved the key and was answered a device token rewrites `hosts/<alias>.json` with the new id and token, every other field kept.
12. `events` (378) sends `events.subscribe` once per socket; later calls await the same answer. `close` sends a close with no code (the host sees 1005); `terminate` drops it.
13. `host_again` (404-414): try with `max(1, left)`; on failure, past the window the last failure stands, else sleep `min(POLL_MS=200, max(0, left))`. `host_back` (418-420) is it over `deps.client(Some(Again{within_ms}))` with `host_wait_ms` or `SERVICE_WAIT_MS` (20,000).
14. `until_settled` (423-430): the work's answer, or `close_words()` as a plain error when the socket goes first.
15. `PushedFrames` (435-451): holds every non-reply frame from creation; `follow` replays the held ones through `pick` in order, then passes live ones; `stop` removes the sink and drops what is still held, from inside a sink too.

## Edge cases
- `--host ""` never reaches here: `aimed_host` refuses it (rule 10.22). A record with empty device token and no host key dials with "" and is refused by the host.
- A request sent after the close is never failed in the TS (`ws` drops it). See After the port.

## Behaviour nobody wrote down
- Rule 10.19: one socket per line; ids count every frame; the close carries no code.
- Rule 10.20: only a wait redials, every 200 ms for 20 s, starting nothing.
- Rule 10.26: the lock, token and records are read afresh at each dial.

## Decisions
1. Time is `tokio::time` (so a paused clock drives `host_again`); the TS reads `Date.now()`.
2. `HostClient::gone()` is the non-waiting read `stoppedUnder` makes with `Promise.race([closed, resolve()])`.
3. A request sent after the socket is gone fails at once with `close_words()` in Rust rather than hanging (the TS hangs); no tape can see the difference, and a hang is not a behaviour to port. Flagged for the reviewer.

## After the port
- A request sent after a close waits forever (rule 10.20's note); every caller races `closed` to cover it.
- `CLOSE_GRACE_MS` waits the full 500 ms on every kindless refusal from a current host, which always stamps a kind.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `the_address::*` (5) |
| 2 | `a_url_aim_with_no_token_is_refused_before_any_socket_opens` |
| 3 | `the_deadline::*` (3) |
| 4 | `a_request_sends_its_params_first...`, `stamps_its_own_id_and_op...` |
| 5 | `replies_are_matched_by_id...`, `every_frame_that_answers_no_waiting_request...` |
| 6 | `a_refused_reply_carries_its_error...never_left`, `a_refusal_with_no_error_string...` |
| 7 | `the_socket_going::*` (3) |
| 8 | `the_token_refused::*` (4) |
| 9 | `the_release::*` (3) |
| 10 | `a_pinned_host_key_opens_the_seal_first...` |
| 11 | not pinned by a test here (needs a sealed host and a device key); the reviewer reads it against 172-198 |
| 12 | `events_subscribes_at_most_once_per_socket`, `close_ends_the_socket_with_no_close_code_of_its_own`, `the_token_rides_the_first_frame...` |
| 13 | `waiting::host_again_*`, `a_try_is_never_handed_less_than_one_millisecond` |
| 14 | `until_settled_answers_the_work_or_the_close_words...` |
| 15 | `pushed_frames::*` (2) |
