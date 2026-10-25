# tunnels
Source at 65f6b89c7: packages/runtime/src/tunnel.ts (70), place-forward.ts (57). Tests: place-forward.test.ts (4);
tunnel.ts has none of its own (packages/host's relay.ts is its other caller). Rust: `src/tunnel.rs`,
`src/place_forward.rs`; tests `tests/tunnels.rs` (11), over real loopback sockets on the owner thread. Callers:
wsp-machines' places door (`HeldForward = Shared<.. Rc<PlaceForward> ..>`, spec places_manage_add.md RD2) and the
host's relay (wsp-serve).

## The tunnel (tunnel.ts)
- **T1** (:33-36) `plumb_tunnel` registers the connection under its id in `conns` and calls `touched` once, before
  anything is asked.
- **T2** (:37-51) Nothing is read off the connection until `tunnel.open {tunnelId, port}` answers. Then `opened` is
  called once, and every chunk read is sent as `tunnel.write {tunnelId, data: <base64>}`, in read order, calling
  `touched` per chunk. Bytes read before the open answered go after it, in order.
- **T3** (:53) An open the link refuses hands the connection to `refused` where given (the caller answers it, as the
  relay ends it with an HTTP 502), else destroys it. Nothing is written over the link for it.
- **T4** (:49) A `tunnel.write` the link fails destroys the connection.
- **T5** (:40-43) The connection closing, for any reason (the browser, a destroy, a refusal answered), takes its id
  out of `conns` and sends `tunnel.close {tunnelId}` once; that op's failure is ignored. Socket errors are swallowed.
- **T6** (:59-70) `tunnel_frame`: an event that is neither `tunnel.data` nor `tunnel.end`, or names an id not in
  `conns`, answers false and touches nothing. `tunnel.end` half-closes the connection's write side (it stays in
  `conns` until it closes) and does not call `touched`. `tunnel.data` calls `touched`, then writes the decoded bytes.
- Frames to the link carry keys in the TS's order: `{tunnelId, port}`, `{tunnelId, data}`, `{tunnelId}`.

## The place forward (place-forward.ts)
- **F1** (:25-41) Listens on LOOPBACK (127.0.0.1) port 0. A listener that cannot bind fails with the OS's words; one
  that bound no port fails with NO_FREE_PORT (`this computer offered no free port for the road to a place`).
- **F2** (:29-31) Each accepted connection is a tunnel `pl<n>`, n counting from 1 per forward, to `daemon_port` over
  `reach.request`.
- **F3** (:47-53) `close`: the first call stops the listener (a dial after it is refused) and destroys every
  connection riding it; a second call does nothing.
- **F4** (:46) `event` hands a frame to `tunnel_frame` and ignores the answer.
- The listener holds no host open (TS `unref`): nothing here keeps the runtime alive.

## Decisions
1. `TunnelConn` is node's `Socket` write side as a cheap handle: `write`, `end(last)` and `destroy`, safe to call after
   the connection went. Its fields are the implementer's; a writer task per connection is the expected shape.
2. Everything spawns on the current `LocalSet` (rule 3.1): `plumb_tunnel` returns at once.
3. `TunnelLink` answers `HostError`; the forward wraps `DaemonReach::request`'s `ReachError` into one.

## Tests (`tests/tunnels.rs`)
| Rule | Test |
|---|---|
| F1, F2, T2, T6 | `opens_one_tunnel_on_the_link_per_connection_on_the_port_that_computer_said_its_daemon_bound` |
| T5, T6 | `ends_the_connection_when_the_far_side_ends_its_tunnel_and_closes_the_tunnel_when_the_connection_goes` |
| T3 | `drops_a_connection_the_far_side_would_not_open_a_tunnel_for_rather_than_holding_it_on_nothing`, `a_refused_tunnel_is_answered_by_the_callers_own_words_and_still_closed_on_the_link` |
| F3 | `stops_answering_once_it_is_closed_so_a_link_that_went_takes_its_port_with_it`, `a_forward_numbers_its_tunnels_and_close_drops_every_connection_once` |
| F2 | `a_forward_numbers_its_tunnels_and_close_drops_every_connection_once` |
| F4 | `a_forward_ignores_a_frame_for_a_tunnel_it_does_not_hold` |
| T1, T2, T5 | `bytes_read_before_the_open_answers_wait_and_go_after_it_in_order` |
| T4 | `a_write_the_link_fails_drops_the_connection` |
| T1, T2, T6 | `touched_hears_the_start_and_every_chunk_and_opened_hears_the_open_once` |
| T6 | `tunnel_frame_answers_whether_the_frame_was_one_of_these_connections` |
