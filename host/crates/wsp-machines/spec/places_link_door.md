# places_link_door: the join, the prove, the attach, a channel and the load

Source: `link.ts:44-315`. Module `places::link` (`LinkDoor`, methods `answer_challenge` to `load`). Shared rules:
`places-a-README.md`. TS cases: `test/places.test.ts` (joining :142-325, dialling back :326-453, the proved socket
:454-512), `places-link.test.ts:193, :245`, `places-facts.test.ts:15, :48, :78`, `places-door.test.ts:32`,
`places-setup.test.ts:1265-1290, :2318, :2438`, at this unit's seam: a fake `LinkRecords` and `LinkViews`
(`tests/common/places.rs`), a real `PlaceSetupArea` over the fake records, a fake socket and reach.

## What it reads off the other areas

`LinkRecords` (link.ts:49-52, slice b): `records()` (sorted by joinedAt), `wired_provider()`, `provider_ids()`,
`provider_backend(id)`, `record_of`, `settings_of`, `settings_held` (a shared map), `awaiting` (a shared map of
waits with an optional `woken(place)`), `hold_back(record)`, `default_id()`, `in_turn(f)`, `mark_default(id)`,
`mark_held()`, `challenge(subject, nonce, ephemeral) -> Option<PlaceChallenge>`, `signed_refusal(place, nonce,
sentence)`, `write_seen(place, at)`, `change`, `with_cap(row, ids)`, `row_ids()`, `login_listed`.
`LinkViews` (link.ts:55, slice b): `pending_records()`, `put_pending(p)`, `flooring` (a shared set),
`floor_once(p, place)`, `taken_back(p)`, `backend_from(place, facts)`, `tunnelled(place, event) -> bool`, `cut(place,
reason)`, `joining` (a shared map of `{ public_key, at }`), `joined(place, pending, req, expect, from, at)`,
`joined_row(record, marked)`. And the context's `door()` for `forking_backend`, `backend_of`, `exec`, `forward`.

## Join (:82)

1. A key `reads_as_ed25519` refuses is `Err(PLACE_BAD_KEY_REFUSAL)` before anything else: nothing held, no id drawn.
2. `id = "p_" + 16 lowercase hex` (8 random bytes, the `Ids` seam).
3. `challenge(id, nonce, ephemeral)`; none is `Err(PLACE_UNSEALED_JOIN_REFUSAL)` and nothing is held.
4. Every `joining` entry older than `JOIN_PROVE_MS` (`at - held.at > 120_000`, strictly) is dropped, then `joining[id]
   = { public_key, at }`. Nothing is written to the store and no code is spent.
5. `{ reply: { placeId, hostPublicKey, nonce, signature, ephemeral, hostName: wiring.host_name() }, expect, seal }`.

## Auth (:102)

No record: `{ refusal: PLACE_UNKNOWN_REFUSAL, signed: signed_refusal(place, nonce, that sentence) }`. No challenge
(a daemon older than the seal): `Err(place_behind_line(name, place_daemon_behind(report) or ""))`. Else `{ reply: {
nonce, hostPublicKey, signature, ephemeral }, expect, seal }`.

`host_key()`: `key_fingerprint(wiring.host_key.public_key)`.

## Prove (:120)

A place in `joining`: the views' `joined(...)` answers. Else no record, or a signature over `expect` that does not
verify against the record's key: `{ refusal: PLACE_KEY_REFUSAL }`. Else `taken_report(req.report)`: its refusal as
`{ refusal }`, else `{ report }`. Nothing is written here.

## Attach (:137)

1. `cut(place, REPLACED)` (an older link goes first).
2. No record: the socket closed `1000, "this host no longer holds that place"`, nothing else.
3. One `change`: the report taken, `lastSeenAt` and `reportedAt` = ISO(at), `road.from = from` (the rest of the road
   kept), `dialled` removed; where the daemon version moved, `backendFacts` removed. The record's name never comes
   off a report. A `change` that answers none (removed meanwhile): the socket closed as in 2.
4. A new daemon version: `backends[place]` and `asking[place]` dropped.
5. The reach over the socket (`connect_daemon` with the seal). Each event: a tunnel's bytes are the views' and go
   nowhere else; else to the place's open forward, the readings, every channel reader of the place (in order), and
   `opts.on_daemon_event(place, e)`.
6. A timer every `seen_every_ms` writes the last seen (its failure ignored). `live[place] = { socket, reach, seen }`.
   `readings.linked(place)`. `woken(place, true)`: a frame held over the gap goes again on this link before anything
   else this attach does.
7. `forking_backend(place)` behind the attach: a `PlaceProvisioning` refusal is silent; any other failure is one
   warning `"{name} did not say what it forks with: {message}"`.
8. On the socket's close, where it is still the live one: `live` loses it, `closed_at[place] = now`, the timer stops,
   the forward closes, the reach closes, the last seen is written, `place.absent { placeId }` emitted. A close of a
   socket already replaced does nothing.
9. `place.present { placeId, from }`; every `awaiting` wait for this place woken (none taken off the list).
10. The record read again: a setup running and not in `setting` resumes (`started_or_said(place, its addId)`); else a
    record following a recipe (not `NO_RECIPE`) whose setup is done syncs at once (`sync_soon(place, 0)`); then each
    pending add for this place at step `floor` not in `flooring` floors (`floor_once`).

`link(place)`: the live reach. `watch_sys(place, f)`: the readings' `watch`.

## Channel (:232)

No live link: `None`. Else the reader joins `channels[place]`; `closed` settles once with `{ code, reason }`:
- `send(frame)`: `op` taken out, the rest the params. The live link no longer the one the channel opened on:
  `Err(PlaceAbsent(absent_computer(name or id, null).sentence))`. `sys.watch`: the channel's one hold on the shared
  watch (a second `sys.watch` reuses it), `{ id: null, ok: true }`, or `{ id: null, ok: false, error }` with the hold
  dropped. `sys.unwatch`: the channel's hold let go, `{ id: null, ok: true }`, nothing sent up. Else the frame on the
  link and its answer.
- `close()`: the reader off, the channel's close listener off the socket, the hold let go, `closed` with `{ 1000,
  "closed here" }`. The socket closing does the same with its own code and reason. A place with no readers left has
  no `channels` entry.

## Load (:291)

1. `settings_held[place] = settings_of(place)` for every key of `CAPS`.
2. For every record in order: `kept` takes it; facts and no backend: `backend_from`; `hold_back(record)`. A setup that
   ended with a wait standing: every wait `expired`. A sync left `running`: `behind`.
3. Each pending add at step `wsp` with no place and no failure: where a record's `road.ssh` is the add's login, the add
   goes to step `floor` with that place; else it is taken back (not awaited).

## Pins

| Rule | Test | TS case |
|---|---|---|
| a bad key refused before anything; nothing held | `a_join_with_a_key_this_host_cannot_verify_holds_nothing` | |
| join id shape, reply fields, host name, nothing written | `a_join_answers_the_challenge_and_writes_nothing` | places-door:88 |
| no challenge: unsealed refusal | `a_join_this_host_cannot_seal_is_refused` | |
| joining entries older than two minutes dropped, at two minutes kept | `a_join_forgets_joins_whose_prove_never_came` | |
| auth: unknown id signed refusal; behind sentence; reply | `auth_refuses_an_unknown_id_with_a_signed_sentence`, `auth_refuses_a_daemon_older_than_the_seal_in_its_rows_words`, `auth_answers_a_known_place` | places:348, :384, :327 |
| prove: pending goes to joined; bad signature; bad home; taken report | `prove_refuses_a_wrong_signature_and_writes_nothing`, `prove_refuses_a_report_with_no_plain_home`, `prove_hands_a_pending_join_to_the_views`, `prove_answers_the_taken_report` | places:365, :247, :290 |
| attach: replaced first; no record closes; record moved keeps its name | `attach_cuts_the_older_link_first`, `attach_to_a_place_no_longer_held_closes_the_socket`, `attach_keeps_the_joined_name_whatever_the_report_says` | places:469, :260 |
| attach keeps from, drops dialled, drops facts on a new daemon | `attach_keeps_where_the_link_came_from_and_drops_the_last_dial`, `attach_on_a_new_daemon_asks_the_facts_again` | places-link:193, :245, places-facts:15 |
| attach keeps a field written while it awaited | `attach_keeps_a_field_written_while_it_awaited` | invariant 6 |
| events routed: tunnel bytes to the views only, channels, readings, the hook | `an_attached_link_routes_each_event_to_its_readers` | places-facts:78 |
| seen timer, close marks absent and moves last seen; a replaced socket's close does nothing | `the_link_closing_marks_the_place_absent`, `a_replaced_sockets_close_does_nothing` | places:504, places-door:32 |
| a held frame goes on the new link before the attach asks anything | `a_held_frame_goes_again_before_the_attach_asks_anything` | |
| facts read behind the attach; a busy computer quiet, a failure one warning | `the_facts_are_read_behind_the_attach` | places-facts:48 |
| a running setup resumes at the link; a following computer syncs | `a_link_resumes_a_running_setup`, `a_link_catches_a_following_computer_up` | places-setup:1290, :2318 |
| a pending add at floor floors once | `a_link_floors_a_pending_add_once` | |
| events present and absent; awaiting woken | `attach_says_present_and_wakes_what_waits_on_it` | places-door:98 |
| channel: none on no link; frames up and back; closed by the socket | `a_channel_rides_the_link_it_opened_on` | places-facts:78 |
| channel: sys.watch is one hold; sys.unwatch never goes up | `a_channels_watch_is_a_hold_on_the_shared_watch` | box-panels:768 |
| channel on a replaced link refuses | `a_channel_on_a_link_that_went_refuses_its_frames` | |
| load: kept, backends, waits expired, syncs behind | `load_reads_every_record_back`, `load_expires_a_wait_an_ended_setup_left` | places-setup:1376, :2438 |
| load: a pending add whose join landed goes to floor, the rest taken back | `load_finishes_an_add_the_host_stopped_in` | places-setup:1272 |
