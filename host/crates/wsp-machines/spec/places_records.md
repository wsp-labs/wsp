# places/records: the place records, the providers beside them, and the turn every write takes

Port of `packages/runtime/src/places/records.ts` (374 lines) at `65f6b89c7`, into `src/places/records.rs`. Tests:
`tests/places_records.rs`. Every `file:line` is at `65f6b89c7`, `records.ts` unless named.

The area is `PlaceRecordsArea`, built once per door from the context (`PlaceRecordsArea::new(ctx)`), holding its own
state: `settings_held`, `limits_kept`, `limits_asking`, `awaiting`, `adds`, the mark chain and the record turns. It
implements places-a's `SetupRecords` and `LinkRecords` as one-line forwards. One thread (spec 03): state in
`RefCell`/`IndexMap`, no borrow held across an `.await` or a call out, removals by `shift_remove`.

## Reading records

- **R1 `records()`** (:42): `store.list(PLACES)`, keep what `is_place_record` holds (types.md T6), sort by `joinedAt`
  ascending. The sort is stable, so equal `joinedAt` keep the store's order (rowid order, spec 02 rule 2.5). The TS uses
  `localeCompare`; on the ISO strings this host writes it is a plain string compare. A document that is not a record is
  skipped, never an error.
- **R2 `record_of(id)`** (:79): `store.get(PLACES, id)` read through `is_place_record`; `None` for anything else.
- **R3 `settings_of(id)`** (:83): `PlaceSettings` parsed from `store.get(CAPS, id)`; an absent document or one that does
  not parse is `{}`. Read from the store at every call; `settings_held` (:88) is the memory copy `settingsAt` reads and
  `set`, `forget` and `load` write.

## Providers

- **R4 `wired_provider()`** (:46): `wiring.provider?.()?.id`.
- **R5 `provider_ids()`** (:50-56): with no wired provider, the table's `list()` less the table's own `wired` row (empty
  with no table). With one, the table's list when it includes the wired id, else `[wired]` alone. Order is the table's.
- **R6 `provider_backend(id)`** (:58): `opts.providers?.().backend(id)`.
- **R7 `provider_rate(id)`** (:61-65): for the wired id, `wiring.provider().rateUsdPerHour`, the table never read; for any
  other id, the backend's `pricing.rateUsdPerHour(pricing.defaultSize)`; `None` where no backend.
- **R8 `provider_sizes(id)`** (:69): the backend's `capabilities.sizes`, else empty.
- **R9 `image_facts(id, backend)`** (:72-78): `buildsImages: builds_images(capabilities)` only where a backend is given;
  `build: line` where `opts.copyBuild(id)` answers, and `buildStopped: true` only where it says stopped (never `false`).

## Rows

- **R10 `row_ids()`** (:90-94): this computer (`HERE_PLACE_ID`, computer), then each record in R1 order (computer), then
  each provider id (provider). Nothing asks any computer.
- **R11 `with_cap(row, ids)`** (:96-115): reads `settings_of(row.id)` now, then sets, in this order: `cap` =
  `place_cap_of(row, settings)` where defined; `capDefault` = `place_cap_of(row)` where defined; `settings` only where
  it has a key; `napMs` = `setting_for(None, settings.napMs, napDefault)` and `napDefault` = `opts.napMs ??
  NAP_AFTER_MS` only where `place_takes(row, "nap")`; `turnLimitMs` = `place_turn_limit(kind, settings)` and
  `turnLimitDefault` = `place_turn_limit(kind, {})` always (null is no limit); `spawn` = `agents_from(None,
  settings.spawn ?? {})` and `spawnDefault` = `AGENTS_ON` always; `running` = `recording.running_on(row.id, ids)`;
  `size`/`sizeDefault` (R13) where `place_takes(row, "size")`; `deleteAfterMs` = `settings.deleteAfterMs ??
  DELETE_AFTER_MS` where `place_takes(row, "delete-after")`. A `running_on` failure fails the call.
- **R12 `with_limits(row)`** (:122-138): synchronous. For a provider row whose backend offers `placeLimits` (`offers().place_limits`, the engine's word for the TS method being there) and no read in
  flight for that id, start one read in the background (a spawned local task) and mark it asking; the read's answer is
  kept for that id (a failure deletes the kept answer, so the fields go rather than stay stale), and the asking mark is
  cleared when it settles. The row returned carries the kept answer as it stands now (`plan`, then `starts` and
  `creditHours` where present), or is the row unchanged: the first listing of a cloud never waits on its read.
- **R13 `size_facts(id, settings)`** (:141-145): `size` = the person's `settings.size`, else the backend's
  `pricing.defaultSize`; `sizeDefault` = that default. Each only where defined.

## The update wait

- **R14 `until_daemon_version(id, from, wait_ms)`** (:148-158): deadline = clock now + `wait_ms`. Loop: read the record's
  `report.daemonVersion` (`from` where the record is gone); answer it when `>= DAEMON_VERSION` or the clock has passed
  the deadline (`>=`); else wait `UPDATE_POLL_MS` (500) and read again. The wait is a clock timer (spec 03 rule 3.15),
  so a test drives it by advancing the clock. A version that moved but is still behind keeps waiting.

## Adds and installs in flight

- **R15 `awaiting`** (:163): the installs waiting on a computer to dial in, keyed by the code each handed it:
  `{place_id?, login?, back?, woken?}`. `join` (views) writes `place_id`; `add` (manage) writes the rest.
- **R16 `put_add(add_id, next)`** (:167-173): nothing where no job has that id. Else replace the job in place (insertion
  order kept), then take the finished jobs (state not `running`) in insertion order and delete all but the last
  `ADDS_KEPT` (20) of them. A running job is never trimmed.
- **R17 `with_road(record, install)`** (:177-189): the record unchanged where the install has no login. Else `road` =
  the record's road with `ssh` replaced and `keyPath`, `hostKey`, `back` replaced only where the install has each, so a
  field the install lacks keeps the record's value.
- **R18 `road_of_install(id)`** (:244-247): the first `awaiting` entry, in insertion order, whose `place_id` is `id`.
- **R19 `login_of(record)`** (:194-197): `None` where `ssh_road_of(record.road)` is `None` (no road, no `ssh`, or `ssh`
  empty). Else `{ssh, key_path, host_key}` off the record's road.

## Logins and the place file

- **R20 `root_over(login, password, verb, name)`** (:201-202): `None` with no `sudoOver` road. Else the password where the
  road answers `"taken"`, `None` for any other answer; a throw (a changed host key among them) passes to the caller.
- **R21 `login_answers(login)`** (:209-217): `true` with no `dial` road. Else the dial bounded by `dial_wait_ms` (the
  bound's sentence `ssh <login.ssh> was not answered in <s>s`); `true` where it resolves, `false` on any failure.
- **R22 `reached_over(record, login, password)`** (:223-240), checks in this order:
  1. no `runOver` road: unread `this host holds no ssh road to read it over`;
  2. no `HOME` in `record.report.login`: unread `<name> never said where its home is`;
  3. `runOver(login, held_place_script(home), dial_wait_ms, password)`, bounded by `dial_wait_ms` with the what
     `ssh <login.ssh>`. A throw is unread with `refusal_parts(e).said`, plus `refused: true` only for a login refusal
     (types.md T9);
  4. exit `CHILD_TIMED_OUT`: unread `the read timed out after <round(dial_wait_ms/1000)>s`;
  5. any other non-zero exit: unread `last_line(stderr)`, else `the read exited <code>`;
  6. `held_place_file(stdout)` naming this record's id and this host's public key: same;
  7. else not same, with `other` = the name of the first record in R1 order, not this one, whose id the file names
     with this host's key; no `other` where none does.
- **R23 `hold_back(record)`** (:251-265): nothing without a back holder, a login (R19), `record.road.back` and a `HOME`.
  Else `back.hold(login, back, {home, computer: {id, name}}, moved)`, not awaited, its failure swallowed. `moved(to)`
  walks a snapshot of the `kept` ids and, through `change`, sets `road.back = to` on every record whose login's `ssh` is
  this login's and which has a road; records on other logins are not written. Its failures are swallowed.

## Writes

- **R24 `keep(record)`** (:271-276): `held = with_road(record, road_of_install(record.id))`; put `held` in `ctx.kept`
  first, then `store.put(PLACES, id, held)`; answer `held`. Every record write goes through here.
- **R25 `in_record_turn(id, write)`** (:336-345): one chain per place id. A write starts after the previous one on that
  id settles, its caller gets its own result or failure, and the chain carries on past a failure. The id's entry is
  removed once its last write settles and no later write joined. Writes on two ids do not wait on each other.
- **R26 `change(id, move)`** (:349-354), runtime invariant 6: inside the record turn, read `record_of(id)` fresh; `None`
  (no write) where the record is gone; `None` (no write) where `move` answers `None`; else `keep(next)` and answer what
  `keep` wrote (with an in-flight install's road, R24). `move` returning the record unchanged still writes.
- **R27 `write_seen(id, at)`** (:330-332): `change` setting `lastSeenAt` to `at` as an ISO string.
- **R28 `login_listed(id, agent)`** (:359-365): `change` appending `shared_login_file(agent)` to `report.logins` and
  sorting by UTF-16 code units. No write where the agent shares no login file, where `report.logins` is absent (an old
  daemon), or where the file is already listed.

## The default mark

- **R29 `default_id()`** (:277-280): the `placeId` of `store.get(DEFAULT_COLLECTION, DEFAULT_ID)` where it is a string.
- **R30 `in_turn(write)`** (:282-287): one chain for the whole door, the mark writes and `set`'s settings writes (manage)
  and `forget`'s mark check (views) on it. The caller gets its own result or failure; the chain swallows it.
- **R31 `mark_default(id)`** (:288): in turn, put `{placeId: id}`.
- **R32 `mark_held()`** (:290-294): the mark where it names `HERE_PLACE_ID`, a provider id (R5) or a record (R1); else
  `None`, the mark itself left in the store.

## The handshake

- **R33 `challenge(subject, their_nonce, their_ephemeral)`** (:303-321): `None` without their ephemeral, and `None` where
  the shared secret cannot be made from it (a malformed key). Else a fresh nonce of `PLACE_LINK_NONCE_BYTES` random bytes
  in base64, a fresh ephemeral, `signature` = `sign_place_bytes(host private key, place_link_transcript("host", subject,
  their_nonce, nonce, {challenger: theirs, answerer: mine}))`, `expect` = `place_link_transcript("place", subject, nonce,
  their_nonce, {challenger: mine, answerer: theirs})`, `seal` = `make_seal(seal_keys(secret, subject), host)`,
  `hostPublicKey` = the wiring's. Two calls never share a nonce or an ephemeral.
- **R34 `signed_refusal(id, nonce, sentence)`** (:325-328): the host public key and its signature over
  `place_refusal_transcript(id, nonce, sentence)`, which `verify_place_bytes` accepts with that key.

## What survives a restart

Only the store: `places`, `caps` and `place-default`. `adds`, `awaiting`, `settings_held`, the limits answers and both
chains are memory and start empty.

## Tests

| Rule | Test |
|---|---|
| R1 | `records_sorted_by_joined_at_skipping_non_records`, `records_tie_keeps_store_order` |
| R2, R3 | `record_of_reads_only_records`, `settings_of_reads_empty_for_absent_or_unparsed` |
| R5 | `provider_ids_wired_and_table`, `provider_ids_no_wired_drops_table_wired`, `provider_ids_wired_not_in_table` |
| R7, R8 | `provider_rate_wired_reads_wiring`, `provider_rate_other_reads_backend_default_size`, `provider_sizes_empty_without_backend` |
| R9 | `image_facts_build_stopped_only_when_stopped` |
| R10 | `row_ids_here_records_providers_in_order` |
| R11 | `with_cap_computer_row_fields`, `with_cap_provider_row_fields`, `with_cap_settings_only_when_set` |
| R12 | `with_limits_first_call_unchanged_then_kept`, `with_limits_failed_read_drops_kept`, `with_limits_one_read_in_flight` |
| R13 | `size_facts_person_over_default` |
| R14 | `until_daemon_version_answers_once_current`, `until_daemon_version_gives_up_at_deadline`, `until_daemon_version_record_gone_reads_from` |
| R16 | `put_add_trims_finished_to_twenty_keeping_running`, `put_add_unknown_id_is_nothing` |
| R17, R24 | `keep_writes_install_road_onto_record`, `with_road_keeps_fields_install_lacks` |
| R19 | `login_of_empty_ssh_is_none` |
| R20, R21 | `root_over_taken_answers_password`, `login_answers_false_on_dial_failure_or_bound` |
| R22 | `reached_over_order_of_checks`, `reached_over_names_other_record`, `reached_over_refused_marks_login_refusal`, `reached_over_timed_out_and_exit_lines` |
| R23 | `hold_back_moved_writes_back_on_same_login_records_only` |
| R25, R26 | `change_one_at_a_time_per_place_no_field_lost` (invariant 6), `change_gone_record_writes_nothing`, `change_move_none_writes_nothing`, `change_chain_survives_a_failure` |
| R28 | `login_listed_appends_sorted_once`, `login_listed_old_daemon_writes_nothing` |
| R29-R32 | `mark_held_drops_gone_row`, `mark_writes_take_turns` |
| R33, R34 | `challenge_none_without_ephemeral`, `challenge_signature_verifies_and_expect_is_place_transcript`, `signed_refusal_verifies` |
| restart | `restart_keeps_records_caps_mark_only` |
