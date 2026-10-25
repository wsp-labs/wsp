# sockets
Source at 65f6b89c7: packages/runtime/src/sys-socket.ts (57), sign-in-socket.ts (97), forwards.ts (29). No TS test
of their own: serve.test.ts and the sign-in tests reach them through `serveRuntime`; those stay with wsp-serve. Rust:
`src/sys_socket.rs`, `src/sign_in_socket.rs`, `src/forwards.rs`; tests `tests/sockets.rs` (20), each handler driven
with a socket, a runtime and a send the test holds.

## Live readings (sys-socket.ts)
- **S1** (:22) An ask naming both or neither of `workspace_id` and `place_id` is refused with SYS_SUBSCRIBE_ONE_OF
  before anything is sent or watched; an unsubscribe too.
- **S2** (:24-29) Unsubscribe detaches the key's watch (or its pending mark) and removes it, then sends `{id, ok:
  true}`; nothing held still answers ok.
- **S3** (:30-33) A subscribe for a key already held, or pending, answers ok and watches nothing more.
- **S4** (:34-42) A place asks `watch_place_sys(place_id, l)` (no origin) and pushes `{type: "place.sys", placeId,
  sample}`; a workspace asks `watch_workspace_sys(workspace_id, l, origin)` and pushes `{type: "workspace.sys",
  workspaceId, sample}`. Keys are `place:<id>` and `workspace:<id>`, never shared.
- **S5** (:43-46) A watch that fails clears its pending mark (only if still its own) and passes the error on; nothing
  is sent.
- **S6** (:47-53) Where the socket closed or the key was unsubscribed while the first reading was in flight, the
  watch is detached as it lands, the key cleared, and the ask still answered ok. Otherwise the detach is held and ok
  sent.
- **S7** (:17-20) Making the handler registers the socket's leave at once; it detaches every held watch and clears
  them (a pending one is marked stopped).
- **S8** Answers write `{"id":..,"ok":true}` in that key order.

## Sign-ins (sign-in-socket.ts)
- **I0** (:11-14) The six ops; every other op is not one (`sign_in_op` answers `None`).
- **I1** (:32-46, :64-74) A start answers `{id, ok: true, signInId}` first, then every step the run pushed while
  starting, in order; a step pushed later goes straight out while the socket is open and is dropped after it closed.
  A step is sent as its frame `{type: "agents.signIn", ...the event}`.
- **I2** (:65-69) A start that answers a run this socket already holds runs the new following's `leave`, sends `{id,
  ok: true, signInId}` alone (steps queued meanwhile are never sent) and registers nothing.
- **I3** (:70-72) Otherwise the run is held and its `leave` registered on the socket; a socket closed by then runs
  `leave` at once and is sent nothing.
- **I4** (:58-62) `agents.signIn` asks `{agent, terminal: true}` only where terminal is true; `servers.signIn` asks
  `{agent, server: name, scope?, project?}`. Both pass the target and the origin.
- **I5** (:54-57) `agents.cloudSignIn` runs the host's cloud road with the agent, the emit and the origin, or is
  refused with CLOUD_SIGN_IN_UNSERVED where none is wired; the runtime's agents are not asked.
- **I6** (:76-85) `agents.signIns` follows every run not ended and not held (holding it, registering its leave) and
  answers `{id, ok: true, runs}` with every run, ended ones included, then any step queued meanwhile.
- **I7** (:87-94) A code or a stop for a run this socket does not hold sends `{id, ok: false, error:
  NO_SIGN_IN_REFUSAL, kind: "usage"}`. For a held run: the code is awaited then `{id, ok: true}`; the stop is called
  then `{id, ok: true}`.
- **I8** A start the runtime refuses passes the refusal on and holds nothing.
- **I9** A socket's leaves never end a run: `sign_in_stop` is called only by `agents.signInStop`.

## Forwards (forwards.ts)
- **W1** (:20) `list` is every source's list in the order given; `None` sources are dropped.
- **W2** (:21) `stop` asks the sources in order and stops at the first that answers true; false when none does.
- **W3** (:22-27) `on` registers the listener on every source; the answer runs each source's off in order.

## Decisions
1. The handlers read narrow traits, `SysWatch` and `SignInRuns`, with blanket impls over `Runtime` and `AgentsApi`
   (rulings 6401022701 rule 4), so their tests need no whole runtime.
2. A refusal the TS throws (S1, S5, I5, I8) is the handler's `Err`, which the door answers as it answers any op.
3. `SignInAsk` (types/pending.rs) gains `scope` and `project` for I4.
4. `ForwardEvent` is the union wsp-wire carries as two halves; it lives in `forwards.rs` until the wire adds it.

## Tests (`tests/sockets.rs`)
| Rule | Test |
|---|---|
| S1 | `a_sys_ask_naming_both_or_neither_daemon_is_refused_before_anything_is_sent_or_watched` |
| S2, S4, S8 | `a_workspace_subscribe_watches_once_with_the_origin_and_pushes_its_readings` |
| S3, S4 | `a_place_subscribe_twice_watches_once_and_a_workspace_of_the_same_id_is_its_own_key` |
| S2 | `unsubscribe_detaches_that_watch_alone_and_answers_ok_even_for_nothing_held` |
| S7 | `the_socket_closing_detaches_every_watch_it_holds` |
| S6 | `an_unsubscribe_while_the_watch_is_in_flight_detaches_it_as_it_lands`, `a_socket_closed_while_the_watch_is_in_flight_detaches_it_as_it_lands` |
| S5 | `a_watch_that_fails_passes_the_error_on_and_the_next_subscribe_watches_again` |
| I0 | `the_sign_in_ops_are_the_six_and_every_other_op_is_none` |
| I1 | `a_start_answers_with_its_run_before_the_steps_it_pushed_while_starting` |
| I1, I9 | `a_step_pushed_after_the_socket_closed_is_dropped` |
| I4 | `a_terminal_start_and_a_servers_start_ask_with_their_own_fields` |
| I2 | `a_start_joining_a_run_already_held_leaves_the_new_following_and_answers_the_id` |
| I3 | `a_socket_closed_before_the_start_answered_leaves_and_hears_nothing` |
| I5 | `a_cloud_sign_in_is_refused_unserved_and_otherwise_runs_the_hosts_own_road` |
| I8 | `a_refused_start_passes_the_refusal_on_and_holds_nothing` |
| I6 | `the_list_follows_the_live_runs_this_socket_does_not_hold_and_answers_them_all` |
| I7 | `a_code_and_a_stop_reach_a_held_run_and_are_refused_for_any_other` |
| W1, W2, W3 | `every_forward_source_as_one_lists_in_order_stops_at_the_first_holder_and_hears_each`, `a_stop_no_source_holds_is_false_after_asking_each_once` |

