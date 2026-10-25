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
  → known.
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
