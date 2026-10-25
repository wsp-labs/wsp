# events: the event bus

TS: `packages/runtime/src/types/events.ts` (98 lines) at `65f6b89c7`. Rust: `src/types/events.rs`. Tests:
`tests/events.rs` (22). Chapter 03 rules 3.6, 3.7, 3.8, 3.20 to 3.22 and 3.28 are this unit's; this page adds what
they leave out. Every expected value in the tests is the TS bus's own answer (`tests/fixtures/core_types/events.json`,
written by `oracle/core_types.oracle.ts`).

## State (rule 3.20)

`BusState` holds exactly: `listeners` (event type, or `"*"`, to the listeners in the order added), `ring` (at most
`EVENT_RING_CAP` = 5000 events), `stream` (the id the bus was made with) and `head` (0 until the first emit).
Nothing else, and nothing of it survives a restart (rule 3.28: a new process mints a new stream and starts at 0).

- `event_bus(stream)` takes the stream id from its caller, which mints it once through the ids seam (rule 3.16)
  where the TS calls `randomUUID()` (events.ts:62). Pinned by `a_new_bus_starts_over_under_its_own_stream`.
- One thread owns the bus (rule 3.1): `Rc<Bus>`, `RefCell<BusState>`. A listener may call `emit`, `pass`, `on` and
  the unlisten of any listener while it runs, so no borrow of the state may be held while a listener is called
  (rule 3.3). A double borrow is a panic the nested test catches.

## on and unlisten (events.ts:65-73)

1. `on(type, l)` appends `l` to that type's list, making the list on first use. **The TS list is a `Set`**: adding
   the same listener again under the same type is a no-op, so it hears each event once; compare listeners with
   `Rc::ptr_eq`. Pinned by `the_same_listener_added_twice_hears_once`.
2. The unlisten removes that one listener from that type's list; calling it again does nothing, and it never touches
   another listener. Pinned by `unlisten_twice_deletes_only_its_own_listener`.
3. A list that empties stays in the map (`an_emptied_listener_set_stays_in_the_map`). No cap on listeners.

## emit (events.ts:81-88, rules 3.6, 3.7)

In this order, with no await anywhere:

1. `head += 1`; the stamped event is the event with `seq = head` (`with_seq`). Its JSON writes `seq` last, as the
   wire's `Sequenced<E>` does (`a_ringed_event_writes_seq_last`). A seq the event already carried is overwritten.
2. Push the stamped event on the ring; if the ring now holds more than 5000, drop from the oldest end until it holds
   5000. The ring holds `head - 4999 ..= head` (`the_ring_keeps_newest_five_thousand`).
3. Deliver the stamped event to the type's listeners in the order added, then to the `"*"` listeners in the order
   added (`delivery_runs_typed_listeners_before_any_and_pass_takes_the_head`). Each listener gets `&EventUnion`, the
   same value the ring holds.
4. Delivery reads the list live, as JavaScript `Set` iteration does: a listener removed before its turn is skipped,
   one added during delivery is reached in the same delivery. A `Vec<Option<_>>` walked by index with `None` left
   where a listener was removed does this; a snapshot of the list does not (rule 3.7). A nested `emit` from a
   listener runs to completion at once, with a larger seq, before the outer delivery goes on, so a later listener of
   the outer event hears the inner one first. Pinned by `a_nested_emit_is_delivered_depth_first_over_a_live_listener_set`,
   whose order is the TS's: `typed1:w1:1, typed1:w9:2, typedB:w9:2, any1:w9:2, added:w1:1, any1:w1:1`.
5. The first listener that answers `Err` stops delivery; `emit` returns that error. The event is already ringed and
   the head already moved (`a_failing_listener_stops_delivery_and_the_event_stays_ringed`). `emit` answers the seq it
   stamped. With no listener the event still takes a seq and a ring slot (`emit_with_no_listener_still_takes_a_seq`).

## pass (events.ts:91-96, rule 3.21)

Delivers exactly as `emit` does, listener order, live list and errors included, and touches neither the ring nor the
head. The event delivered is `with_seq(event, head)` when `head > 0`; when `head == 0` it is the event as given, a seq
it already carried untouched (`pass_before_any_emit_leaves_the_events_own_seq`). A pass between emits carries the
head's seq, so it shares a number with the event before it (`pass_never_moves_the_head`).

## since (events.ts:74-80, rule 3.22)

With `oldest = head - ring.len() + 1` and `foreign = stream is Some(s) and s != this stream` (the empty string is
foreign):

| `after` | Answer | Test |
|---|---|---|
| None | no events, no gap; the stream is not read | `since_with_no_cursor_never_reads_the_stream` |
| Some, and foreign | gap, no events | `since_an_empty_stream_with_a_cursor_is_foreign`, `since_reads_its_own_stream_and_refuses_another` |
| greater than head | gap | `since_a_cursor_past_a_fresh_head_is_a_gap`, `since_one_past_the_head_is_a_gap` |
| less than `oldest - 1` | gap; after a wrap, `after: 0` is a gap | `the_ring_keeps_the_newest_five_thousand` |
| `oldest - 1 ..= head` | the ring's events after `after`, oldest first, no gap; `after == head` is none | `emit_rings_each_event_with_its_seq_and_pass_rings_nothing`, `since_at_the_head_replays_nothing_and_is_no_gap` |

Every answer carries this bus's stream and head. On an empty bus `oldest - 1` is 0, so `after: 0` is no gap and
`after: 1` is a gap (`since_with_no_cursor_on_a_fresh_bus_answers_head_zero_and_no_gap`). The order of the checks
does not change any answer. `after` is a `u64`: the wire schema refuses a negative or fractional cursor before
`since` is called (chapter 03's tape), so `since` never sees one.

## Helpers

`event_type` answers the variant's `type` string, `event_seq` its seq, `with_seq` the event with seq set and every
other key as it was (`event_type_seq_and_with_seq_read_and_set_the_keys`). `EventUnion` has no such accessors in
wsp-wire today; write them here with one match, or a serde round trip, as long as the key order holds.

## Constants (events.ts:21-57)

`EVENT_RING_CAP` 5000, `SLATE_PR_POLL_MS` 30 000, `CATALOG_TTL_MS` 300 000, `CATALOG_PROBE_TIMEOUT_MS` 25 000,
`FIRST_RUN_READ_MS` 3 000, `AGENT_VERSION_READ_MS` 3 000, `SESSION_TITLE_TTL_MS` 10 000, `CHECKOUT_TTL_MS` 10 000,
`PR_PAGE_HOLD_MS` 60 000, `RATE_LIMIT_HOLD_MS` 60 000, `SESSION_TITLE_TIMEOUT_MS` 15 000,
`SESSION_TITLE_REFRESH_MAX` 20, `TITLE_MAKE_TIMEOUT_MS` 25 000. Pinned by `constants_match_the_ts`. Only the ring cap
is read in this file; the rest are the areas' and are pinned here because they live here.
