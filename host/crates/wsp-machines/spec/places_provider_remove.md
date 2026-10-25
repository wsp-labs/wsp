# places_provider_remove: taking a provider back out

Source: `provider-remove.ts` (117). Module `places::provider_remove`. Shared rules: `places-a-README.md`. TS cases:
`test/places-remove-provider.test.ts` (16), at this unit's seam: the whole-host `remove("box")` becomes a call of
`remove_provider` with the engine's fake backend (`wsp_engine::fake_backend`) standing for the stub, `known` for the
ids the host's records hold there, `forget_key` and `images` for the host's roads. The `--forget` case
(places-remove-provider:242) asks nothing of the provider and never reaches this function: it is slice b's
(`manage.ts`), and is not ported here.

## Constants and words

`KILLS_AT_ONCE = 4`, `RELISTS = 3`. Words from wsp-words: `provider_key_name(place)` (`box` reads `Boat`),
`PROVIDER_LEFT_WORDS` (`running(state)`, `unread(said)`, `undeleted(said)`, `late`, `listed(grace_ms)`),
`provider_kept_line`, `provider_images_kept_line`, `provider_unlisted_line`, `provider_key_refused`, `refusal`.
From the engine: `OWNER_LABEL`, `sight_machine`, `kill_until_gone`, `snapshot_until_gone`, `drop_template`,
`is_missing`, `MachineAliveError`, `KillConfirm`.

## `remove_provider(o) -> Result<PlaceRemoved, HostError>` (:152)

`o = { place_id, backend, owner, known: ids, forget_key: Option<road>, confirm: Option<KillConfirm>, images:
Option<{ held(), drop(id) }> }`. `name = provider_key_name(place_id)`.

1. **The marked listing** (:163): `backend.list({ OWNER_LABEL: owner })`, then only rows whose own label equals
   `owner` (a backend may ignore the labels it is handed; another host's machine on the account is never touched).
   A listing refused with kind `auth` is `refusal(said, fix, Auth)` of `provider_key_refused(name, message)`: the
   whole call fails with it, nothing deleted, the key not forgotten. Any other listing failure on the first read
   passes through as it came.
2. **The ids** (:207): the marked ids, then `known`, deduplicated, first place kept.
3. **Passes**: while ids remain. On pass 4 (the fifth listing found more), every remaining id is left with
   `late` and the passes stop. Each pass marks its ids handled, deletes them, then lists again: a relist that fails
   stops the passes where anything is left already, else answers at once `{ removed: false, swept: [], deleted,
   note: provider_unlisted_line(name, deleted, message) }`. The next ids are the relist's not yet handled. So the
   listing is read at most five times and deletes run in at most four passes.
4. **Deleting** (:174): at most 4 at once, each worker taking the next id off one queue in order.
   - `sight_machine(id)`: a failure leaves the id with `unread(message)`; gone (12 reads in a row, the engine's
     rule, so one 404 from a copy that never held it is not gone) is skipped and not counted.
   - Else `kill_until_gone` with the machine's own `kill` wrapped so a kill failure that is not missing marks the
     delete as failed. Success counts one in `deleted`. A `MachineAliveError` leaves `running(state)`; else a marked
     delete failure leaves `undeleted(message)`; else (a read after the delete failed) `unread(message)`.
5. Anything left: `{ removed: false, swept: [], deleted, note: provider_kept_line(name, deleted, left) }` (five named,
   the rest counted, by the words' own rule); the images are not touched and the key stays.
6. **Images** (:233), only once every machine went: for each held image in order, `drop_template(template_id)`,
   then `snapshot_until_gone(id, confirm)`. `listed` leaves `listed(grace_ms)` and keeps its record; `deleted` or
   `missing` drops its record (`images.drop(id)`) and counts one; any failure leaves `undeleted(message)` and keeps
   the record. No `images` road is no images.
7. Images left: `{ removed: false, swept: [], deleted, snapshots?, note: provider_images_kept_line(name, deleted,
   left) }`, the key kept. `snapshots` is present only when at least one went.
8. Else `still = forget_key(place_id)` where wired, and `{ removed: true, swept: [], deleted, snapshots?, note? }`
   with `note = still` where it answered one. The key is forgotten only here, after every delete read back gone.

`PlaceRemoved` is the wire struct (`removed`, `took`, `swept`, `note`, `deleted`, `snapshots`); this function never
sets `took`, always sets `deleted`, and sets `snapshots` and `note` only as above.

## Pins

| Rule | Test | TS case |
|---|---|---|
| deletes by the owner mark, another host's machine stays, key forgotten after | `deletes_every_marked_machine_then_forgets_the_key` | :97 |
| one that will not go is named and the key stays; a second remove finishes | `names_a_machine_that_stays_and_keeps_the_key` | :115 |
| a known id the listing missed is deleted; one already gone is not counted | `deletes_a_known_id_the_listing_missed` | :137 |
| nothing there: removed, deleted 0 | `a_provider_with_nothing_of_ours_forgets_the_key` | :153 |
| one 404 is not gone | `one_404_from_a_copy_is_not_gone` | :159 |
| a machine made meanwhile is deleted before the key goes | `deletes_a_machine_made_after_the_first_listing` | :171 |
| five listings, four passes, then late | `names_a_machine_still_showing_after_the_last_listing` | :189 |
| unread words, five named and the rest counted | `says_a_machine_that_could_not_be_read_names_five_and_counts_the_rest` | :209 |
| auth refusal in two halves, key kept | `refuses_in_two_halves_when_the_key_is_refused` | :228 |
| unread after the delete; undeleted where the delete failed | `tells_an_unread_machine_from_an_undeleted_one` | :259 |
| a relist that failed after deletes | `says_a_failed_relist_with_how_many_went` | :278 |
| still note from forget_key | `answers_where_the_key_still_is` | :292 |
| relist failure with nothing deleted drops the count | `a_failed_relist_with_nothing_deleted_drops_the_count` | :300 |
| images deleted before the key, records dropped, another provider's image untouched | `deletes_each_image_snapshot_before_the_key` | :315 |
| an image that will not go keeps the key and its record | `names_a_snapshot_that_stays_and_keeps_its_record` | :336 |
| a still-listed snapshot | `a_snapshot_still_listed_after_its_window_is_named` | |
| never more than four deletes at once | `never_runs_more_than_four_deletes_at_once` | |
| a relist failure after something is left answers the kept line | `a_relist_failure_after_a_left_machine_answers_the_kept_line` | |
| took never set, deleted always set | `the_answer_sets_deleted_and_never_took` | |
| a first listing that fails (not auth) passes through | `a_first_listing_that_fails_passes_through` | |
| a snapshot the provider lost is dropped and counted | `a_snapshot_the_provider_lost_is_dropped_and_counted` | |
