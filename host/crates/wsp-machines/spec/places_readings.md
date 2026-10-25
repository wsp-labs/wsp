# places_readings: the door's shared state and one computer's readings watch

Source: `context.ts` (75) and `readings.ts` (94). Modules `places::context`, `places::readings`. Shared rules:
`places-a-README.md`. TS cases: `test/box-panels.test.ts:693-860` (eight), at this unit's seam: the page's
subscribe is `watch`, the pane's and the channel's holds are `watch` too (the channel's own handling of `sys.watch`
and `sys.unwatch` is `link_door`'s, link.ts:247-280).

## `PlaceDoorContext` (context.ts:13, :37)

`place_door_context(opts) -> Rc<PlaceDoorContext>`, holding, all memory only and empty at birth:
`opts`, `store`, `devices`, `wiring`, `recording` (from `opts`); `clock` (`opts.clock`, else the real one: the TS
`opts.now` and `opts.schedule` are the one clock here, rule 3.15); `seen_every_ms` (`opts.seen_every_ms` else
60,000), `dial_wait_ms` (else 20,000), `frame_wait_ms` (else 300,000), `relink_wait_ms` (else 120,000); `live:
IndexMap<place, Live>`, `kept: IndexMap<place, PlaceRecord>`, `backends: IndexMap<place, Arc<dyn
MachineBackend>>`, `forwards: IndexMap<key, Forward>`, `asking: IndexMap<place, shared future>`, `watchers`; and
`door`, set once after every area is built (a `OnceCell<Weak<dyn PlaceDoor>>`). `door()` before it is set panics: an
area reads it inside a call, never while it is built.

- `sign_ins_here(place, agents?)` (:57): `None` for a place not in `kept`; else `sign_ins_of(record.report,
  opts.vault() or {}, record.applied.rows or [], agents)`. It reads `kept` and the vault at the call, never a copy
  taken at birth: a record `kept` moves after the context was made reads the new rows.
- `emit(e)` (:68): every watcher in registration order, iterated live (rule 3.7): one added during the emit is
  reached, one removed before its turn is skipped. `watch(f) -> unsubscribe`.

## `place_readings(ctx)` (readings.ts:106)

State: `watched: IndexMap<place, Watched { readers: IndexSet<reader id>, asked: Option<{ on: link, answer: shared
future of the reply }> }>` and `still_sending`: the links whose daemon could not be told to stop, held by link
identity (`Rc::ptr_eq`) and never keeping a dropped link alive (the TS `WeakSet`).

- `unwatches(place)`: the kept record's `report.daemon_version >= SYS_UNWATCH_DAEMON_VERSION`; false with no record.
- **`ask(w, on)`** (:115): a link in `still_sending` is answered `{ ok: true }` with no frame sent. Else one
  `sys.watch` request on `on`; a rejection becomes `{ ok: false, error: message }`; once the reply is in, a reply
  that is not `ok: true` clears `w.asked` where it is still this ask (a refusal is never inherited). `w.asked = { on,
  answer }` is set before the frame's reply.
- **`watch(place, f) -> Result<release>`** (:133):
  1. A place not in `kept`: `Err(no_such_place_refusal(place, kept names in order))`, nothing held.
  2. `f` joins `watched[place]` (made where absent).
  3. No live link: the release, at once, and no frame (held while away).
  4. Else the answer of `w.asked` where it was asked on this same link, or a fresh `ask`: one `sys.watch` per link
     however many readers.
  5. A reply not `ok: true` while that link is still the live one: `f` released and `Err(error string, else "the
     computer would not send its readings")`. One lost with its link (the live link changed or went while it was in
     flight): `f` stays held and the release is answered.
- **`release()`** (:138): nothing if `f` was already released, if other readers remain, or if `watched[place]` is
  no longer this `Watched`. Else `watched[place]` is removed; and where the watch was asked on the link still live:
  `sys.unwatch` sent and its failure ignored when `unwatches`, else that link joins `still_sending`. A watch asked on
  a link since replaced sends nothing.
- **`linked(place)`** (:157): with readers and a live link the watch was not asked on, `ask` on it.
- **`heard(place, event)`** (:164): a `sys.sample` only, to each reader of a snapshot taken before the first call (a
  reader released during the delivery is still called this once). Every other event type is dropped.

## Pins

| Rule | Test | TS case |
|---|---|---|
| defaults of the four waits; the context's maps start empty | `the_context_takes_the_ts_defaults` | |
| sign_ins_here reads kept and the vault at the call; None for an unknown place | `sign_ins_here_reads_the_record_as_it_stands` | places-setup:748 |
| emit iterates live | `emit_reaches_a_watcher_added_during_it_and_skips_one_removed` | |
| door read before set panics | `door_is_unset_until_the_door_is_built` | |
| one watch per link, each sample to every reader once | `readers_ride_one_watch_and_each_sample_reaches_each_once` | box-panels:703 |
| unwatch only with the last reader; a reader's own release lets go only its hold | `the_last_reader_leaving_sends_one_unwatch` | box-panels:718 |
| held while away, asked on the link that replaces it | `readers_are_held_while_away_and_asked_on_the_next_link` | box-panels:748 |
| a daemon one behind the host but at or past the unwatch version takes unwatch | `a_daemon_at_the_unwatch_version_is_sent_unwatch` | box-panels:784 |
| a daemon before unwatch: never asked twice on one link | `a_daemon_before_unwatch_is_never_asked_twice` | box-panels:792 |
| a subscribe in flight when the link dropped stays held | `a_watch_in_flight_when_the_link_drops_stays_held` | box-panels:817 |
| a refusal is said and not kept; the next reader asks again | `a_refusal_is_said_and_the_next_reader_asks_again` | box-panels:833 |
| an unknown place is refused by name | `an_unknown_place_is_refused_with_the_places_there_are` | box-panels:856 |
| a refusal while away is no refusal; the refused reader is released | `a_refused_reader_is_released_and_leaves_no_watch` | |
| a second release does nothing | `a_second_release_does_nothing` | |
| heard drops other events; snapshot delivery | `heard_passes_samples_alone_to_a_snapshot_of_readers` | |
| a release on a link that was replaced sends nothing | `a_release_after_the_link_was_replaced_sends_nothing` | |
| no record version means still sending | `a_record_with_no_daemon_version_keeps_the_watch_running` | |
