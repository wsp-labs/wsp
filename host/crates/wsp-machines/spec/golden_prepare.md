# golden_prepare: building, preparing and sealing the image

TS: `images/golden.ts:64-290` (`build`, `get`, `buildPlace`, `prepareAt`, `prepare`, `seal`, `recipe`). Rust:
`src/images/golden/prepare.rs`, methods of `GoldenArea` over `Rc<dyn GoldenCtx>`. TS tests: runtime-builders,
runtime-golden-import, runtime-create, lost-create, sealed-image, copy-on-press, places-image, golden-recipe,
golden-templates. Tests: `tests/golden_prepare.rs`.

Every road here awaits `ctx.ready()` first.

## Rules

G1. `build(o)` (golden.ts:80-97): the name defaults to `default`; the wired copy is read first; the build runs through
`ctx.claiming("golden/<name>", ..)` as `roads.build_golden` with the request's options, the claimed backend, the
name, `ctx.image_mark()` as host id, labels `{...request labels, wsp: "1", wsp-owner: <owner>, createdAt: <clock
ISO>}` and the prior manifest where one stands; the answer's manifest is written as the wired copy; answers
`{manifest, version}`.
G2. `get(name)` (golden.ts:98-102): the copy at `ctx.image_place(name)`.
G3. `build_place(word)` (golden.ts:104-131): where `ctx.record_of("default")` stands the target is its image place,
else the word. A target is resolved by `ctx.place_at` and must fork: a place that forks no machines fails with
`PlaceForksNowhere` (`<place name> forks no machines, so your image cannot be built there`, kind `conflict`), with no
fallback. With no target: the door's `default_place()` (none read as no mark); no mark tries the wired provider
(`ctx.backend_at(wired)`), a mark tries `door.forking_backend(id)`. Only a `PlaceForksNowhere` there falls through; any
other failure fails the call. Then `ctx.build_places()`: none refuses `NO_BUILD_PLACE_LINE`, more than one refuses
`build_place_ask_line(<each place_said>)`, both kind `conflict`; one answers it. An answer is `{place, name:
place_name(place), kind: place_kind_of(place), backend}`.
G4. `prepare_at(o)` (golden.ts:133-137, 75-78): for an image that has a record (and is not a copy) the record's place,
whatever was asked; else the asked word through `place_at`; no word is the wired provider through `backend_at`.
Answers `{place, name: place_name(place)}`.
G5. `prepare(o)` refusals, in order (golden.ts:139-155): the recipe (`ctx.recipe_or_throw(o.recipe)`), then the place
(G4, `copy` honoured), then the join key `<place>/<name>`: a prepare in flight there with the same import hash (both
absent counts as the same) answers that prepare's own future; with another hash it fails `a builder named <name> is
still being prepared for a different recipe; wait for it to finish, then run again`, no kind.
G6. The run (golden.ts:156-268) goes through `ctx.claiming("builder/<place>/<name>", .., at, place)` and is in
`preparing` until it settles, then removed. It awaits `ctx.refresh_builders()` and looks for a builder to attach to:
only with an import, the first builder in map order whose life is `own` or `reusable`, not `building`, not `sealed`,
of the same name, at the same place, with the same import hash. A recipe with no import never attaches.
G7. Attach (golden.ts:207-235): stages `creating` and `deploying-daemon` with `ALREADY_APPLIED`; then
`roads.apply_golden_import(machine, {import, setup, ledger: the record's import, on_stage})`; then `exec("true")`,
exit non-zero failing `the builder answered exit <n> to a no-op; it is not serving`; a stop that came meanwhile fails
with the stop's error. Then the record's import becomes the applied ledger, life `own`, the recipe is kept, and it is
held (`ctx.hold`). A failure (not after a stop) kills the machine until gone (a kill failure appended as `; <message>`
to the detail), forgets it if gone, stages `failed` with the detail and fails with the original error. Success stages
`ready` and answers `ctx.builder_view`.
G8. Fresh (golden.ts:236-258): `roads.prepare_builder` on `ctx.recording_creates(b, name, import, <made: mine>,
{signal, began: <creating>}, place)` with the recipe less `deployDaemon`, `smoke` and `import`, plus `kind`,
`deployDaemon`, `import` where given, labels `ctx.builder_labels(recipe labels)` and the stage listener. A failure
forgets the placeholder if gone (not after a stop) and fails. A stop after the builder landed fails with the stop's
error. Else `ctx.settle_builder`, the recipe kept on the entry, answers `ctx.builder_view`.
G9. The stop (golden.ts:172-206): a cancelled signal, before or during the run, starts the stop once. The stop waits
for a create in flight first (its failure ignored). No machine yet: `PrepareStopped` with no id. An attached builder:
its `heldBy` removed, life `reusable`, the record written (a failed write warns `hold on builder <id> not released; it
ages out in 15 minutes`), `PrepareStopped {id, kept}`. A made builder: killed until gone (a failed kill answers
`PrepareStopped {id, left: <message>}` and keeps the record), then forgotten (a failed forget warns `record of builder
<id> not dropped; the machine is gone and the next load drops it`), `PrepareStopped {id}`. Once a stop has begun its
answer is the run's answer, whatever the work does after.
G10. `seal(id, o)` (golden.ts:273-281): after `refresh_builders`, an unknown id fails `no such builder: <id>`, no kind;
`ctx.refuse_untouchable`; then `ctx.seal_entry(entry, keep, o.logins)` where keep is `o.keepBuilder` not false and the
record's import carries a recipe.
G11. `recipe(name)` (golden.ts:283-290): the digest filed under the image place's copy head; no copy answers `None`.

## After the port

- G1 labels read the wall clock; the port reads the context clock.
- G5: two prepares with no import share one future even when their recipes differ in everything else.

## Tests

| Rule | Test |
|---|---|
| G1 | `build_claims_the_golden_key_and_files_the_wired_copy`, `build_hands_the_prior_manifest_and_the_owner_labels` |
| G2 | `get_reads_the_copy_at_the_image_place` |
| G3 | `build_place_goes_to_the_records_place_whatever_was_asked`, `build_place_refuses_a_target_that_forks_nothing`, `build_place_takes_the_marked_place`, `build_place_falls_through_a_wired_provider_that_forks_nothing`, `build_place_keeps_a_marked_places_own_failure`, `build_place_refuses_none_and_asks_among_many` |
| G4 | `prepare_at_names_the_records_place_for_an_image_with_a_record`, `prepare_at_takes_the_word_or_the_wired_provider` |
| G5 | `prepare_refuses_without_a_recipe_before_any_place_is_read`, `prepare_joins_one_in_flight_with_the_same_hash`, `prepare_refuses_one_in_flight_with_another_hash` |
| G6 | `prepare_claims_the_builder_key_per_place`, `prepare_attaches_to_a_reusable_builder_with_the_same_import`, `prepare_never_attaches_without_an_import`, `prepare_leaves_preparing_once_settled` |
| G7 | `attach_stages_already_applied_and_holds_the_builder`, `attach_kills_and_fails_a_builder_that_does_not_answer`, `attach_appends_a_failed_kill_to_the_failed_stage` |
| G8 | `fresh_prepares_on_the_recording_backend_and_settles`, `fresh_forgets_a_gone_placeholder_when_the_prepare_fails` |
| G9 | `a_stop_before_any_machine_answers_stopped_with_no_id`, `a_stop_waits_for_the_create_in_flight_then_kills_it`, `a_stop_on_an_attached_builder_releases_it_reusable`, `a_stop_whose_kill_fails_says_what_was_left`, `a_stop_answers_whatever_the_work_does_after` |
| G10 | `seal_refuses_an_unknown_builder`, `seal_refuses_an_untouchable_builder`, `seal_keeps_only_a_builder_from_a_recipe` |
| G11 | `recipe_reads_the_digest_of_the_copy_head` |
