# threads_cap: a computer's threads at once

TS: `T/threads.ts:364-521` at `65f6b89c7`. Rust: `src/threads/threads/cap.rs`, methods on `ThreadsArea`. State:
`cap_held: RefCell<IndexMap<turn id, CapHeld>>`, `cap_order: Cell<u64>` from 0, `borrowed: RefCell<IndexMap<turn,
lender turn>>`, and `held_writes`, one chain for the whole host (chapter 03, kind "chain", caller gets the swallowed
promise). Every function here is synchronous: `cap_wait` does all its work in the call and answers a future that only
settles (rule 3.4: a start let through is counted before anything else looks).

## Rules

C1. `cap_hold(record, turn, lender)` (`:392-394`): when `turn` is not held, holds it as `{placeId:
ctx.place_id_of(record), lenders: [lender] or [], order: cap_order++}`. A turn already held is left as it is, its
lenders included.

C2. `cap_lend(turn, lender)` (`:398-403`): nothing for a turn not held or a lender it already has; else appends the
lender and wakes the turn's wait if it waits.

C3. `cap_full(record, turn)` (`:411-434`): `None` when the record has no place id, the place door answers no
`threads_at(place)` (a cloud, a host with no door), or `turn` is not held. Else, with `atOnce` that number:
- the place rows are `[here]` plus `[place]` when the place is not `here`, both of kind `computer`;
- `standing` is every live record with `provider: ctx.provider_of(record)`;
- `others` is `foldThreads` over every row whose turn is not `turn` and not held; `here` is the threads of `others`
  that `runningOn(place, rows, standing, [thread]) > 0`; `running_here` is the turn ids of the running rows of those
  threads;
- `lent` is every `borrowed` pair whose turn and lender both run here;
- the lender is the first of the turn's lenders that runs here and lends to no turn in `lent`. With one: `borrowed[turn]
  = lender`, the turn's `through = {why: lent, atOnce}` when it waits, and `None` answers;
- `ahead` counts the other held turns on the same place id with a smaller `order`. When `here.len() - lent.len() +
  ahead >= atOnce` the answer is `ThreadCapWait {placeId, place: ctx.place_name(placeId), running: here.len(),
  atOnce}` with no `why`. Else, when the turn waits, `through = {why: raised when atOnce > its wait's atOnce else freed,
  atOnce}`, and `None` answers.

C4. `cap_wait(at, wait)` (`:442-485`), in this order in the call:
1. the turn's hold, or a new one `{placeId: wait.placeId, lenders: [], order: cap_order++}`, put back;
2. a turn already stopped answers a future that fails with `HostError::new(<the stop's reason>)`, and nothing below
   runs;
3. `said` is the wait the turn last said, `moved` is true when there is none or its `running`, `atOnce` or `place`
   differ from `wait`'s;
4. on the turn's first wait only, a `held-starts` record keyed by the turn id goes on the chain: `{workspaceId,
   threadId, turnId, sessionId, harness, place: wait.place, prompt: at.prompt, notify?, notifyBy?, notifyRoad?}` in
   that key order, `harness` and the notify fields read off the row `sessions[turn id]` (`""` and absent when there is
   no such row);
5. `ctx.idle_hold(workspace)`;
6. a `*` listener on the bus wakes the wait on `session.end`, `place.changed` or `workspace.deleted`;
7. the turn's `waiting` becomes `{wait, at: {workspaceId, threadId, sessionId}, wake, stop}`;
8. when `moved`, `ctx.record(session.capped)` with keys `type, workspaceId, sessionId, turnId, threadId, placeId,
   place, running, atOnce, why?, requestId?, prompt?`, `prompt` (the send's whole text) only on the turn's first wait.

The wait settles once: `wake` resolves it, `stop(reason)` fails it with `HostError::new(reason)`. Either way, once,
the bus listener is removed and `ctx.idle_release(workspace)` runs. A wake or stop after it settled does nothing. The
turn's `waiting` stays set after it settles, so its next `cap_wait` writes no second record and says the wait again
only when it moved.

C5. `cap_stop(turn, reason)` (`:489-495`): `false` for a turn not held. Else marks it stopped with the reason, stops a
wait it is in, and answers `true`; a later `cap_wait` fails at once (C4.2).

C6. `cap_stopping(turn)` (`:499-509`): for a held turn, adds an unsettled promise to its `stops` and answers a release
that removes and settles it; for a turn not held, a release that does nothing.

C7. `cap_left(turn)` (`:514-521`): nothing for a turn not held. Else removes it; when it had waited, the `held-starts`
row's delete goes on the chain; when it had waited and a look let it through, `ctx.record(session.uncapped)` with keys
`type, workspaceId, threadId, sessionId, turnId, why, atOnce`; then every turn still held that waits is woken, in
map order.

C8. A failed `held-starts` write or delete warns `a held start was not written down: <message>` and the chain goes
on; the caller never sees it. Writes land in call order.

C9. On every `session.end` that names a turn, the bus listener registered at construction (`:385-389`) removes that
turn's `borrowed` entry and every entry whose lender is that turn.

C10. Restart: `cap_held`, `borrowed` and `cap_order` are memory only. Only `held-starts` rows survive; boot fails each
(rule 2.37, slice a and machines).

## Tests (`tests/threads_cap.rs`)

| Rule | Test |
|---|---|
| C1 | `cap_hold_holds_once_and_keeps_its_first_lenders` |
| C2 | `cap_lend_adds_a_lender_once_and_wakes_the_wait` |
| C3 | `cap_full_is_none_on_a_cloud_or_with_no_hold`, `cap_full_counts_running_threads_and_the_turns_ahead`, `cap_full_lets_a_turn_through_in_its_lenders_slot`, `cap_full_lends_one_slot_to_one_turn_at_a_time`, `cap_full_says_raised_or_freed_when_it_lets_a_waiting_turn_through` |
| C4 | `cap_wait_writes_the_held_start_once_and_says_the_wait`, `cap_wait_says_the_wait_again_only_when_it_moved`, `cap_wait_holds_the_machine_awake_until_it_settles`, `cap_wait_wakes_on_an_end_a_place_change_or_a_delete` |
| C5 | `cap_stop_ends_a_wait_with_the_reason`, `a_stop_before_the_wait_fails_it_at_once_and_writes_nothing` |
| C6 | `cap_stopping_holds_a_promise_until_released` |
| C7 | `cap_left_deletes_the_held_start_says_why_and_wakes_the_rest`, `cap_left_of_a_turn_that_never_waited_writes_nothing` |
| C8 | `a_failed_held_start_write_warns_and_the_chain_goes_on` |
| C9 | `an_end_frees_its_borrowed_slots_both_ways` |
