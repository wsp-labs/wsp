# builders_life: a builder's hold, grace, stages, create, settle and seal

TS: `images/builders.ts:80-189, 283-472`. Rust: `src/images/builders/life.rs`, methods of `BuildersArea`. TS tests:
runtime-builders (39), runtime-golden-import (35), lost-create, sealed-image, copy-on-press, golden-recipe. Tests:
`tests/builders_life.rs`.

## Rules

### Grace and the heartbeat

L1. `arm_grace(id, sealed_at)` (builders.ts:82-89) cancels that id's timer if any, then schedules one at
`max(0, GRACE_MS - (now - parse(sealed_at)))` (GRACE_MS 600,000) that removes itself from `grace_timers` and runs
`expire_grace()`, warning `grace sweep failed: <message>` on its failure. An unparsable time arms at 0.
L2. `in_window(sealed_at)` (builders.ts:91-94): true while `now - parse(sealed_at) < GRACE_MS`; exactly GRACE_MS is
out; an unparsable time is out.
L3. `expire_grace()` (builders.ts:99) runs one pass at a time: a second call while one runs gets the same answer and
starts nothing. Once a pass ends the next call starts a new one.
L4. A pass (builders.ts:100-118) awaits `ctx.refresh_builders()`, then takes each builder of a snapshot of the map in
order that has `sealed`, a life of `own` or `reusable`, and is out of its window. It asks `ctx.gone_stop(backend_at(
place), machine)`; a failure there (or in `backend_at`) adds `{id, message: "could not stop: <message>; stays
recorded, retried next sweep"}` to `failed` and leaves the record. A stop cancels and removes its grace timer, forgets
the builder (R-forget) and adds `{id, builder: true, reason: "grace", ageMs: now - parse(sealed.at)}` to `reaped`.
L5. `hold(b)` (builders.ts:141-147) writes nothing for a builder the map no longer holds as that same object. While the
host is not closed it stamps `heldBy {host: host id, pid: ctx.pid(), heartbeat: <clock now, ISO>}`; it always writes
`builders/<id>` with the record; while not closed it then arms the heartbeat. A closed host's write keeps the record's
old `heldBy`.
L6. The heartbeat (builders.ts:119-137): `arm` does nothing on a closed host or while a beat is armed; else it
schedules one beat at `HEARTBEAT_MS` (300,000) into `state.beat`. A beat clears `state.beat`, holds every builder
whose life is `own` in map order (a failed write warns `heartbeat for builder <id> not written: <message>` and the beat
goes on), and arms again only where it held at least one.
L7. `refuse_untouchable(entry)` (builders.ts:151-155), first match, no kind: `foreign`: `<id> wears another setup's
owner label (<owner label, "undefined" where none>); it is never sealed or reached from here`; `held`: `<id> is in use
by another wsp process (pid <heldBy.pid, "undefined" where none>); it is never sealed or reached from here`;
`building`: `<id> is still being prepared; it is never sealed or reached until its stages finish`.
L8. `forget_builder(id)` removes the map entry, then deletes `builders/<id>` (builders.ts:157-160).
L9. `forget_if_gone(entry, at)` (builders.ts:166-172) forgets only where `roads.read_gone(at, id)` answers `gone`; any
other answer or a failure keeps the record.

### Stages

L10. `stage_of(name, on, named)` (builders.ts:177-189) answers a listener. Each call builds the frame `{name, stage,
detail?, step?, left?}` (`left` only when not empty). With `on`, `ready`, `sealed` and `failed` remove
`stage_at[<on>/<name>]`, any other stage sets it to `{place: on, name, named, frame}`. Then it emits
`{...frame, type: "golden.stage", place?}`, `place` being `on` only when `named`.

### Create, settle, seal

L11. `recording_creates(b, name, imp, made, stop, place)` (builders.ts:285-324) answers a backend that is `b` for every
road but `create`. A create whose stop signal is already cancelled fails at once with `PrepareStoppedError` with no
builder id, and `b.create` is never asked. Otherwise the creating future is handed to `stop.began` before the provider
answers; it creates on `b`, passes the machine through `ctx.observed`, and records a placeholder: `{id, name, kind:
spec.kind, baseTemplate: spec.template or "", setupSha: "", createdAt, size, firstLife: true, building: true,
streamUrl?, import?, place}` where `createdAt` is the machine's `createdAt` label, else the spec's, else the clock;
`size` is the spec's cpu and memMb else **the runtime backend's** default size (not `b`'s); `import` is `{recipeHash,
recipe?, applied: [], smoke: "true"}` where `imp` is given. The placeholder (`sealable` true, life `own`) goes in the
map, `made` is told, and it is held (L5) before the create answers the machine.
L12. `settle_builder(name, builder, placeholder, place)` (builders.ts:327-350) writes a fresh record `{id, name, kind,
baseTemplate, setupSha, createdAt (the placeholder's, else the builder's), size, firstLife: true, streamUrl?, import?,
base?, parentSnapshotId?, retired?, place}` onto the placeholder (same object, its life untouched) or onto a new
entry (`sealable` true, life `own`), puts it in the map under the machine's id, holds it and answers it.
L13. `record_seal` (builders.ts:357-395), the image's own seal (no `copy`): the hash is `image_hash(<builder import's
recipeHash or "">, vault sha256)`; the stored `images/<name>` record is `{name, version, hash, recipeHash, recipe?
(the recipe's source), logins (or []), sealedAt (the version's createdAt), sealedFrom: host id, vault? {sha256, bytes:
tar length, paths, held, takenAt: version createdAt}, noSecrets? (true at a provider only), usedBytes?, place}`. The
vault blob is put before the record; a replaced record of another version has its vault blob deleted after. A copy's
seal writes no record and takes `copy.hash`. Where a hash stands the answered version and its manifest entry carry
`imageHash` (and `noSecrets: true` at a provider); a copy with no hash answers the result unchanged.
L14. `seal_entry(entry, keep, logins, copy)` (builders.ts:401-464): the recipe (`recipe_or_throw(entry.recipe)`)
fails before anything is read. The seal runs through `ctx.claiming("smoke/<id>", ..)` at the builder's place with:
`ctx.observing(b)`, the recipe's smoke, cpu, memMb, envs, labels `{...recipe labels, wsp, wsp-smoke, wsp-owner,
createdAt}`, the place's current copy as `manifest`, the stage listener `stage_of(name, place, copy given)`,
`killConfirm` and `snapshotRetryMs` where wired, `logins` where given, `vaultPaths` only on the image's own seal away
from a provider, `refuse` only at a provider (`NEVER_ON_BROKERED`, less `GH_LOGIN_PATHS` while the broker is not
ready), `keepBuilder: keep`, the name and `ctx.image_mark()`.
L15. After the seal: `record_seal`, then `ctx.put_copy(place, name, manifest)`, then the builder import's recipe
digest (where it has one) under the new version, then, for the image's own seal of `default`, the door's
`mark_default_if_none(place)` whose failure warns `the default place was not marked at the seal: <message>` and fails
nothing. A kept builder gets `sealed {at: clock ISO, version}`, `parentSnapshotId` (record and builder) of the new
snapshot, life `own`, a hold and its grace armed; any other seal forgets the builder.
L16. A failed seal (builders.ts:455-463): a `SnapshotFailed` whose builder is not `gone` fails as it is and keeps the
record; a `NotFirstLife` kills the machine until gone first (a failed kill fails with the kill's error); every other
failure, a failure after the seal included, runs `forget_if_gone` and fails with the error.
L17. `drop_image(v, at)` (builders.ts:469-472) drops the version's template (`roads.drop_template(at, templateId)`)
before deleting its snapshot; `at` defaults to the runtime backend.

## Restart

The map, the timers and the beat are memory. A record written by `hold` survives; a new host reads it back through
the boot area's `refresh_builders` (machines-a's slice), which is why every state change here is held before the call
answers.

## After the port

- L5 and L11 read `new Date()` and `process.pid`; the port reads the context clock and `ctx.pid()`.
- L16: a `NotFirstLife` whose kill fails loses the seal's own error.
- L11: the placeholder's size falls back to the runtime backend's default size even when it builds on another place.

## Tests

| Rule | Test |
|---|---|
| L1 | `arm_grace_schedules_the_rest_of_the_window_and_replaces_an_earlier_timer`, `a_grace_timer_that_fires_runs_the_sweep_and_drops_itself` |
| L2 | `in_window_is_open_until_exactly_the_grace` |
| L3 | `expire_grace_joins_a_pass_already_running` |
| L4 | `expire_grace_stops_and_forgets_builders_past_their_window`, `expire_grace_keeps_a_builder_it_could_not_stop`, `expire_grace_skips_unsealed_foreign_held_and_in_window_builders` |
| L5 | `hold_stamps_and_writes_the_record`, `hold_writes_nothing_for_a_forgotten_builder`, `hold_on_a_closed_host_writes_without_a_stamp_or_a_beat` |
| L6 | `the_heartbeat_holds_every_own_builder_and_rearms`, `a_failed_heartbeat_write_warns_and_the_beat_goes_on`, `arm_does_nothing_while_a_beat_is_armed` |
| L7 | `refuse_untouchable_names_foreign_held_and_building` |
| L8 | `forget_builder_drops_the_entry_and_the_record` |
| L9 | `forget_if_gone_forgets_only_on_gone` |
| L10 | `stage_of_sets_and_clears_the_stage_at_and_emits_the_frame`, `stage_of_names_the_place_only_for_a_copy` |
| L11 | `recording_creates_holds_a_placeholder_before_answering`, `recording_creates_refuses_after_a_stop_without_asking`, `recording_creates_hands_the_create_to_the_stop_before_it_lands`, `recording_creates_takes_the_created_at_label_of_the_machine` |
| L12 | `settle_builder_writes_the_finished_record_onto_the_placeholder`, `settle_builder_without_a_placeholder_makes_an_own_entry` |
| L13 | `an_own_seal_writes_the_record_and_its_vault`, `an_own_seal_drops_the_replaced_versions_vault`, `a_copys_seal_writes_no_record_and_stamps_the_copys_hash`, `a_seal_at_a_provider_marks_no_secrets` |
| L14 | `seal_entry_refuses_without_a_recipe_before_reading`, `seal_entry_asks_the_seal_with_the_ts_options`, `seal_entry_at_a_provider_refuses_paths_and_takes_no_vault` |
| L15 | `a_kept_seal_stamps_sealed_holds_and_arms_grace`, `an_unkept_seal_forgets_the_builder`, `a_default_seal_marks_the_default_place_and_a_failed_mark_only_warns` |
| L16 | `a_refused_snapshot_keeps_the_builder_record`, `a_not_first_life_builder_is_killed_then_forgotten_if_gone`, `a_failure_after_the_seal_still_asks_gone` |
| L17 | `drop_image_drops_the_template_before_the_snapshot` |
