# builders_places: where an image is built, and how a builder is shown

TS: `images/builders.ts:57-78, 190-281`. Rust: `src/images/builders/places.rs`, methods of `BuildersArea` over
`Rc<dyn BuildersCtx>`. TS tests: whole-host cases in places-image, copy-on-press, sealed-image, places-fork and
runtime-builders. Tests: `tests/builders_places.rs`.

## Rules

P1. `builder_view(r, b)` (builders.ts:57-78) keys, in order: `id`, `name`, `kind`, `createdAt`, `placeId`
(`r.place`, else `ctx.wired()`), `placeName` (`place_name(place)`), `size`, `rateUsdPerHour` (the place's backend's
`pricing.rate_usd_per_hour(r.size)`, the backend looked up as `places.backend(place)` then `place_door.backend_of`;
absent with neither), `screen` (`{streamUrl}` where the record has one), `sealable` (`b.sealable`), `recipeHash`
(`r.import.recipeHash`), `recipe` (`r.import.recipe`), `foreignOwner` (life `foreign` only: the machine's
`wsp-owner` label, `""` where it has none), `heldBy` (life `held` and the record holds one), `building` (only `true`),
`sealed`.
P2. `backend_at(place)` (builders.ts:191-195): `places.backend(place)`, else the place door's `backend_of(place)`;
neither is a `HostError` of kind `missing` with `no_such_place_refusal(place, places.list() as provider_key_name)`.
P3. `place_at(word)` (builders.ts:200-221), in order: a provider row named `word` answers `{place: word, at}`. With no
place door: `cloud_named(places.list(), word)` or the word itself, through `backend_at`. With a door: `place_for(word)`;
its failure is rethrown with its message and kind `missing`. A `place_id` answers `{place: id, at:
door.forking_backend(id)}`. None: the word must name the wired provider (`cloud_named([wired], word)`) and that
provider must have a backend, else kind `conflict` with `place_builds_no_image_line(word)`.
P4. `forking_at(place_id)` (builders.ts:233-239): with an id, first awaits `ctx.place_door_of().forking_backend(id)`,
whose answer is dropped and whose refusal fails the call; then `ctx.backend_of_kind("cloud", id)`; a backend that
forks no machines answers `None`.
P5. `landing_backend(place_id)` (builders.ts:225-230): `forking_at`'s backend; with none, `build_places()` named by
`place_said`; none at all refuses with `opts.no_machines_line` or `NO_PROVIDER_LINE`, some with
`place_forks_nothing_pick_line(place_name(id or wired), names)`, both kind `conflict`.
P6. `place_name(place)` (builders.ts:242): the door's `name_of(place)` where it names one; else this computer's
hostname (`ctx.hostname()`) for `here`; else `provider_key_name(place)`.
P7. `image_place(name)` (builders.ts:245): `ctx.record_of(name)`'s `place`, else the wired provider.
P8. `place_kind_of(place)` (builders.ts:247): `provider` where `at_provider(place, places)`, else `computer`.
P9. `build_places()` (builders.ts:250-263): every provider row in `places.list()` order whose backend exists and forks,
as `{place: id, name: id, kind: provider}`; then, with a door, every row of `door.list(clock.now())` that is a joined
computer and whose `forking_backend` answers, as `{place: view.id, name: view.name, kind: computer}`. A computer whose
backend read fails is left out silently.
P10. `copy_recipe_or_throw()` answers `opts.copy_recipe`; none fails with `NO_COPY_RECIPE`, no kind
(builders.ts:267-271). `recipe_or_throw(named)` answers `named`, else `opts.golden_recipe`, else fails with `this
runtime has no golden recipe; the host wires one (setup + smoke) before the wizard can run`, no kind
(builders.ts:275-279).
P11. `builder_labels(extra)` (builders.ts:281): the extra labels in their order, then `wsp: "1"`, `wsp-builder: "1"`,
`wsp-owner: <state owner>`, `createdAt: <clock now, ISO>`; an extra label of one of those names keeps its place and
takes the new value (JS spread).

## After the port

- P6 reads `os.hostname()` at each call; the port reads `ctx.hostname()`.
- P11 reads the wall clock (`new Date()`); the port reads the context clock (rule 3.15).

## Tests

| Rule | Test |
|---|---|
| P1 | `builder_view_carries_every_fact_in_the_ts_key_order`, `builder_view_names_a_foreign_owner_and_a_holder_only_by_life`, `builder_view_has_no_rate_without_a_backend` |
| P2 | `backend_at_reads_the_row_then_the_door_else_refuses_missing` |
| P3 | `place_at_takes_a_provider_row_as_it_is`, `place_at_without_a_door_reads_the_cloud_word`, `place_at_asks_the_door_and_its_forking_backend`, `place_at_marks_a_door_refusal_missing`, `place_at_reads_the_wired_providers_name_when_the_door_names_no_place`, `place_at_refuses_a_word_naming_nothing_that_builds` |
| P4 | `forking_at_asks_the_computer_first_and_drops_a_backend_that_forks_nothing`, `forking_at_fails_with_the_computers_refusal` |
| P5 | `landing_backend_refuses_with_no_provider_line_when_nothing_runs`, `landing_backend_names_the_places_that_run`, `landing_backend_uses_the_hosts_own_no_machines_line` |
| P6 | `place_name_reads_the_door_then_hostname_then_provider_word` |
| P7 | `image_place_reads_the_record_else_wired` |
| P8 | `place_kind_of_tells_provider_from_computer` |
| P9 | `build_places_lists_forking_providers_then_joined_computers`, `build_places_skips_a_computer_whose_backend_fails` |
| P10 | `copy_recipe_or_throw_refuses_without_one`, `recipe_or_throw_prefers_the_named_recipe`, `recipe_or_throw_refuses_without_one` |
| P11 | `builder_labels_put_the_four_marks_after_the_extra_ones`, `builder_labels_keep_an_overridden_keys_place` |
