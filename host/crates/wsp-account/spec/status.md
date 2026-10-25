# status: the status rows and lost machines (`account/status.ts`, 182 lines)

Two things: the rows the status tracker (wsp-core's `status`) polls, built from the live records, and `adoptLost`,
which the sweep calls with a provider's listing to record again a machine whose record was lost, or to report and
later delete one nothing here reaches. TS tests: `test/status.test.ts` (the adopt, unrecorded and row cases),
`test/lost-create.test.ts`, `test/sweep-places.test.ts`, `test/gone.test.ts`. Case list in `tests/status.rs`' header.

`StatusCtx`: `held()`, `view(record)`, `rate_usd_per_hour(record)`, `row_reason(entry)`, `idle_at(id)`,
`away_line(record)`, `unreached_of(entry)`, `reach_of(entry)`, `has_daemon(entry)`, the machine's optional reads,
`polled_reach_set/delete`, `boot_work_has(id)`, `revive_daemon`, `offer_daemon_again`, `read_version_again`,
`adopt_gone(entry, reason).await`, store `get`/`put`/`list`/`delete`, `now()` (the clock) and `wall_now()` (the TS
`Date.now()`/`new Date()`, the same fake in tests: README A7), `owner()`, `live(id)`, `project_held(id)`,
`name_refusal(name)`, `places_wired()`, `golden_head().await`, `observed(machine)`, `shape_of(machine).await`,
`size_built(shape, default)`, `attach(record, machine)`, `persist(record).await`, `sync_daemon(entry)`,
`emit(event)`, `emit_status(entry, reach, reason).await`, `gone_stop(backend, machine).await`, `warn(line)`.

## Rules

**The rows** (13-43)

- S1 Each held entry (not creating, live map order) after `ready()`: the view, then `size`, `rateUsdPerHour`,
  `generation`, then each only when present, in this order: `reason` (`rowReason`), `wakeAsk`, `checkout`, `pr`,
  `idleAt` (only for a `running` record with an idle mark), `away`, `unreached`, `daemonReach` (where the kind has a
  daemon), `daemonAnswers`, `daemonAnswersPromptMs`, then `providerState`, `metrics?`, `facts?` (left out while the
  reach reads `unreachable`), `exec`. Test `s1_row_keys_and_order`, `s1_facts_skipped_when_unreachable`.
- S2 `onPolled` (49-61): a status of a machine no longer under its record is skipped. A `running` one records the
  polled reach, any other phase deletes it. A workspace with boot work out stops there; else the daemon is revived,
  offered again and its version read again, in that order. Test `s2_on_polled_order`.
- S3 `onGone(id, machineId, reason)` (62-66): only where the record still holds that machine, `adoptGone`; a failure
  warns `"<id>'s gone reading was not confirmed: <message>"`.

**adoptLost(listing, known, failed, at)** (81-180)

- S4 The dropped watch (86-90): `dropped` rows younger than `DROPPED_WATCH_MS` (by `clock.now() - Date.parse(at)`)
  are watched; older ones are deleted from the store.
- S5 Skipped rows, in order: in `known`; dropped and `running`; not a lost workspace of this owner
  (`lostWorkspace(row, owner, now)`). A stamped id (`WORKSPACE_LABEL`) not live but stored in `workspaces` → known,
  failed `"not recorded: <id> is stored here but was not read at start, so its <state> machine is left as it is until
  the host starts again"`. A live holder skips a running row, and any row while the holder is creating, waking,
  deleting or napping. Test `s5_skips_table`.
- S6 Unreached (105-132): a stamped id with no kept name (`workspace-names`) for a row whose holder is not live, or a
  holder whose record names another machine: known and unreached. The first sighting stores `{ machineId, place,
  seenAt: ISO(now) }` in `unrecorded`. Before `seenAt + UNRECORDED_DELETE_MS` (a day) it is failed with `"not recorded:
  <whose>, so nothing here reaches this <state> machine of yours; deleted at <ISO>, a day after this host first saw
  it"`, `whose` being `"no workspace of this host is <id>"`, `"<id> holds no machine"` (record machine `""`) or `"<id>
  runs on <machineId>"`. After it: the machine is stopped (`gone.stop(backend, backend.get(id))`; a missing machine
  counts as stopped; any other failure → failed `"could not delete: <message>; no record of this host's reaches it,
  retried next sweep"` and kept), the `unrecorded` row deleted, and reaped `{ id, labels, builder: false, reason:
  "unrecorded", ageMs? }` (`ageMs` from `CREATED_AT_LABEL` where it parses). **Restart**: the `unrecorded` row is the
  only memory of the first sighting, so a host restarted within the day deletes at the same time. Tests
  `s6_unreached_named_with_its_deletion_time`, `s6_deleted_a_day_after_first_seen`, `s6_first_sighting_survives_a_restart`,
  `s6_a_failed_delete_is_retried`.
- S7 No project (133-139): no stamp, no kept project or one not held → known, failed `"not recorded: this host holds no
  project for it, and a workspace is one project's copy; it is a machine of yours still <state>"`.
- S8 The confirming read (140-153): `backend.get(id)` observed; missing → skipped (not known); another failure →
  known, failed `"not recorded: <message>; retried next sweep"`; a state (seen, else asked) neither running nor paused
  → known. A failing state ask is outside the TS `try` (149): `adoptLost` fails whole with its message
  (`ReadFailure::StateFailed`), rows after it are not read, and nothing of the sweep's `unrecorded` pruning runs.
  Test `s8_a_failed_state_read_fails_the_adopt`.
- S9 The record (154-169), keys in this order: `id` stamp, `name` (kept name where `nameRefusal` passes, else the
  machine id), `kind: "cloud"`, `project`, `place` (only when not the wired place), `machineId`, `phase` (`napping` for
  paused, else `running`), `golden` (label, else the golden head's snapshot id, else `""`), `createdAt` (label where it
  parses, else now ISO), `spec: { labels }`, `size` (`sizeBuilt(shapeOf(machine), backend default)`), `firstLife:
  false`, `screen?`. Then attach, persist, sync the daemon (running only, not awaited), emit `workspace.created`, emit
  the status (`reachOf(entry)` or `napping`, reason `RECORD_RESTORED`), adopted `{ id, workspaceId, name, phase }`.
  Test `s9_adopted_record_whole`.
- S10 After the listing (178): every `unrecorded` row of this place not unreached this sweep is deleted; rows of other
  places stay. Test `s10_unrecorded_rows_the_listing_dropped_go`.

## Null and empty inputs

An empty listing adopts nothing, reaps nothing, and still prunes `dropped` and this place's `unrecorded` rows.

## Tests

`tests/status.rs`. `oracle/status.oracle.ts` writes `tests/fixtures/oracle/status.json`: `RECORD_RESTORED`, the two
limits and the collection and label names, read off the TS the area imports. Every failed line and every record is
written out whole in the test, from the TS tests it is ported from. The fake context's clock is one `Cell<i64>` that
`Lab::advance` moves; a restart is `Lab::restarted`, a second area and context over the same store and clock.
`lost_workspace` on the fake is `lostWorkspace` (packages/engine/src/orphans.ts:87) written out, since the Rust engine's
is not built yet.

A row's TS functions (`daemonReach`, `daemonAnswers`, `providerState`, `metrics`, `facts`, `exec`) read `null` in the
Rust row map: the key's presence and place is what the area decides (S1).

| Rule | Tests |
|---|---|
| S1 | `s1_row_keys_and_order`, `s1_facts_skipped_when_unreachable` |
| S2 | `s2_on_polled_order` |
| S3 | `s3_on_gone_adopts_only_the_machine_the_record_still_holds`, `s3_a_gone_reading_that_fails_is_warned_whole` |
| S4 | `s4_dropped_rows_older_than_the_watch_are_deleted` |
| S5 | `s5_skips_table` |
| S6 | `s6_unreached_named_with_its_deletion_time`, `s6_deleted_a_day_after_first_seen`, `s6_first_sighting_survives_a_restart`, `s6_a_failed_delete_is_retried`, `s6_a_reaped_machine_carries_its_age_where_its_label_parses` |
| S7 | `s7_no_project_is_reported_and_left` |
| S8 | `s8_the_confirming_read` |
| S9 | `s9_adopted_record_whole`, `s9_a_paused_machine_at_another_place_hydrates_napping_there`, `s9_the_golden_falls_back_to_empty_and_the_name_to_the_machine` |
| S10 | `s10_unrecorded_rows_the_listing_dropped_go` |
| empty listing | `an_empty_listing_adopts_nothing_and_still_prunes` |

Ported TS cases:

| TS case | Test |
|---|---|
| `test/status.test.ts:100` | `ported_the_row_carries_the_record_size_and_the_rate_the_context_prices` |
| `test/status.test.ts:477`, `test/local-workspace.test.ts:1358` | `ported_a_row_nobody_bills_carries_a_zero_rate` |
| `test/local-workspace.test.ts:1366` | `ported_facts_ride_the_row_of_a_machine_that_has_them_alone` |
| `test/status.test.ts:986` | `ported_a_rename_leaves_the_idle_mark_where_it_was` |
| `test/sweep-places.test.ts:40`, `:102` | `s6_unreached_named_with_its_deletion_time` |
| `test/sweep-places.test.ts:54`, `:71`, `:235` | `s6_deleted_a_day_after_first_seen` |
| `test/sweep-places.test.ts:80` | `ported_a_machine_the_listing_stops_carrying_is_forgotten` |
| `test/sweep-places.test.ts:91` | `s7_no_project_is_reported_and_left` |
| `test/sweep-places.test.ts:131` | `ported_a_lost_record_at_a_place_is_recorded_again_there` |
| `test/sweep-places.test.ts:153` | `ported_a_record_the_start_left_as_it_was_is_never_adopted_over_the_stored_one` |
| `test/lost-create.test.ts:134` | `ported_the_sweep_adopts_the_box_when_its_name_was_kept_and_no_record_holds_the_workspace` |
| `test/reconcile.test.ts:44`, `:63` | `s9_adopted_record_whole`, `s9_a_paused_machine_at_another_place_hydrates_napping_there` |
| `test/reconcile.test.ts:71`, `:89` | `s7_no_project_is_reported_and_left`, `ported_a_machine_it_cannot_name_is_reported_off_the_listing_sweep_after_sweep` |
| `test/reconcile.test.ts:105` | `ported_a_machine_inside_its_create_minute_and_one_the_provider_no_longer_knows_are_left` |
| `test/reconcile.test.ts:123` | `ported_a_body_a_rebuild_replaced_is_not_recorded_twice` |
| `test/reconcile.test.ts:139` | `s9_the_golden_falls_back_to_empty_and_the_name_to_the_machine` |
| `test/reconcile.test.ts:150` | `s8_the_confirming_read`, `ported_a_confirming_read_the_provider_refuses_is_tried_again_next_sweep` |
| `test/reconcile.test.ts:175`, `:200`, `:510` | `ported_a_deleted_workspaces_machine_that_comes_back` |
| `test/reconcile.test.ts:220` | `ported_a_deleted_machine_is_forgotten_a_week_on_by_the_runtimes_clock` |
| `test/reconcile.test.ts:446` | `ported_the_name_a_person_gave_outranks_the_forks` |
| `host/test/host.test.ts:621` | `ported_a_box_first_seen_a_day_ago_is_deleted_and_a_young_one_named` |
| `test/sub-resume.test.ts:654` | `s6_unreached_named_with_its_deletion_time` (the `holds no machine` row) |

Not ported: `test/reconcile.test.ts:241` and `test/sweep-places.test.ts:185, :206` (weak: the engine's `lostWorkspace`
and its known set decide them), `test/pull-request.test.ts:215, :231` (weak: the pull request area's own rows),
`test/status.test.ts:420` (weak: the tracker's accrual).
