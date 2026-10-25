# image: the image record, its copies, the copy build and the fork's snapshot

TS: `images/image.ts` (272 lines). Rust: `src/images/image.rs`, `ImageArea` over `Rc<dyn ImageCtx>`. TS tests:
sealed-image (37), copy-on-press (22), places-image (12), places-fork (29). Tests: `tests/image.rs`.

## Rules

I1. Snapshot sizes (image.ts:37-41): no ids or no listing answers an empty map; a listing failure answers empty; a row
is kept only where its id was asked and it carries `restoredBytes`.
I2. `copies_of(name)` (image.ts:44-67): every `goldens` key whose name part is `name`, in key order, whose manifest has
a head; the place is the key's place (no place reads as the wired provider). Each copy is `{place, version, hash?
(head imageHash), snapshotId, templateId?, builtAt: head createdAt, sizeBytes?}`, the size asked of that place's
backend (`places.backend` then the door's `backend_of`; none asks nothing). Sorted: the wired provider first, the rest
by `localeCompare` of the place.
I3. `conflict(message)`: a `HostError` with kind `conflict` and no fix (image.ts:69).
I4. `get(name)` (image.ts:72-79): ready; `{image: record or null, copies}`, the name defaulting to `default`.
I5. `vault(name)` (image.ts:81-90): ready; refusals, each `conflict`, in order: no record `this host owns no image named
<name> yet`; no vault `<name> v<n> was sealed before its sign-ins were held, so there is nothing to export; cut the next
version to hold them`; no blob `the vault of <name> v<n> is not on this computer any more; cut the next version to take
it again`. Answers `{image, tar}`.
I6. `build(o)` refusals in order (image.ts:92-103), each `refusal(line, fix, conflict)`: a place that builds no images
(`place_builds_no_image_line(where)`, fix `copy_build_fix::NO_COPY`); no record (`this host owns no image named <name>
yet`, `NO_IMAGE`); a record with no recipe (`<name> v<n> was sealed before the image record kept the recipe it was
built from, so no other place can build it`, `NO_RECIPE`); unforced, a record whose copy would ask for sign-ins
(`copy_asks_sign_ins`; `<name> v<n> holds no sign-ins, so a copy at <where> would ask for every one of them again`,
`NO_VAULT`). `where` is `ctx.place_name(place)` of `ctx.place_at(o.place)`.
I7. Unforced, a place whose head is current (`copy_is_current(record, {hash: head imageHash})`) answers its standing
copy with `built: false` and builds nothing; no standing copy there fails `<where> holds <name> v<n> and no copy of it
was recorded there`, no kind (image.ts:104-113).
I8. Joining (image.ts:116-129): `o.starting` is called once the refusals passed, joined or not; a build in flight for
`<place>/<name>` answers that build's own future. A new build is in `copy_builds` until it settles. A failed build,
where the place's copy row stands and is not stopped, frames the stop (`ctx.frame_stopped(place, name, ..)`) with
`place_went_away_line(where)` when the place is away or the error is the engine's place-absent, else with the error;
then fails with the error.
I9. `keep_current(place, name)` (image.ts:132-158) never fails. It reads the copy row first; then ready; no record or a
place away returns; a place that builds no images returns; then builds until the copy it built is current against the
record as it stands after (`copy_is_current(record, built.copy)`) or the record is gone. A failure: where the row
changed since the start and now reads stopped, nothing more (the build said it); else where
`ctx.row_says_failure(place, e)`, it frames the stop with the error.
I10. The copy build (image.ts:163-206): at a provider the vault is never landed. A vault with no `held` list fails
`vault_unlisted_refusal(name, version)` (conflict) before the blob is read; a vault whose blob is gone fails as I5's
third; `roads.refuse_foreign_members("import", tar, held)` failing is a conflict with its message. Then the recipe
(`ctx.copy_recipe_or_throw()(record, {cloud: provider})`), then `ctx.golden_prepare({name, place, copy: true, recipe,
signal})`; a builder the map does not hold after fails `the builder <id> prepared at <where> left no record here;
nothing was sealed`. With a blob: stage `uploading-files` with `the image's sign-ins, <fmt_bytes(tar length)>`, then
`roads.import_image_vault(machine, tar)`; a failure kills (failure ignored), forgets if gone, stages `failed` with the
message and fails.
I11. Then (image.ts:197-205): the place's head before the seal is read; `ctx.seal_entry(entry, false, record.logins,
Some(record))`; at the image's own place with a head before, that head's digest is filed under the new version; the
copy now standing there is answered with `built: true`, none failing `<where> sealed <name> v<n> and no copy of it was
recorded there`.
I12. `copy_for_fork(golden, place, announce)` (image.ts:247-270): the place defaults to the wired provider. The image
whose head at its own place is this snapshot (`images` records in list order) decides; none, or the fork landing at
the image's own place, answers the snapshot asked. Without `announce`, a copy build in flight there is awaited and its
copy answered (its failure read as none). No record answers the snapshot. A current head there answers its snapshot.
Without `announce`, the snapshot asked. A place that builds no images answers the snapshot asked. Else a build runs
with `starting` calling `announce(place_name(place), at.pricing.rate_usd_per_hour(default size))` and its copy's
snapshot is answered.
I13. At a provider (image.ts:232-240) the answered snapshot is checked: no image records, no version known for the
snapshot (`ctx.image_of`), or a version marked `noSecrets` passes. Else the record it is a copy of (the one whose hash
the version carries; else the first whose own place lists the snapshot; else the only record) names the home (its
place, else wired; no record: the fork's place), and the fork is refused `image_held_sign_ins_line(place_name(home))`
with fix `image_rebuild_fix(home)`, kind `conflict`.

## After the port

- I8: `starting` runs even for a caller that only joins a build already announced.
- I2 asks each place's listing one at a time.

## Tests

| Rule | Test |
|---|---|
| I1 | `copies_carry_the_restored_size_only_where_the_listing_says_it`, `a_listing_that_fails_leaves_sizes_out` |
| I2 | `copies_list_each_places_head_wired_first_then_by_place`, `copies_skip_a_manifest_with_no_head` |
| I3 | `conflict_is_kind_conflict` |
| I4 | `get_answers_null_and_no_copies_for_an_unknown_image` |
| I5 | `vault_refuses_in_order_and_answers_the_blob` |
| I6 | `build_refuses_in_the_ts_order_with_each_fix`, `build_forced_passes_a_record_without_sign_ins` |
| I7 | `build_answers_a_current_copy_unbuilt`, `build_fails_when_a_current_head_has_no_recorded_copy` |
| I8 | `build_joins_one_in_flight_and_calls_starting_each_time`, `a_failed_build_frames_the_place_went_away`, `a_failed_build_frames_its_own_error_while_the_row_stands` |
| I9 | `keep_current_builds_until_current`, `keep_current_owes_nothing_for_an_away_place`, `keep_current_frames_a_refusal_before_any_build`, `keep_current_leaves_a_stop_the_build_framed` |
| I10 | `the_copy_build_lands_the_vault_before_the_seal`, `the_copy_build_lands_no_vault_at_a_provider`, `the_copy_build_refuses_an_unlisted_vault_before_reading_the_blob`, `the_copy_build_refuses_foreign_members`, `a_failed_vault_landing_kills_forgets_and_stages_failed` |
| I11 | `the_copy_at_the_images_own_place_keeps_the_planned_digest`, `the_copy_build_fails_with_no_recorded_copy_after_the_seal` |
| I12 | `copy_for_fork_answers_the_snapshot_at_the_images_own_place`, `copy_for_fork_waits_for_a_build_in_flight`, `copy_for_fork_takes_a_current_copy`, `copy_for_fork_builds_and_announces_with_the_rate` |
| I13 | `copy_for_fork_at_a_provider_refuses_a_version_holding_sign_ins`, `copy_for_fork_at_a_provider_passes_a_no_secrets_version` |
