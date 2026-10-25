# golden_upgrade: the update, the builder roads and the kill

TS: `images/golden.ts:292-459` (`upgrade`, `builderReach`, `builders`, `kill`). Rust: `src/images/golden/upgrade.rs`,
methods of `GoldenArea`. TS tests: runtime-golden-update (38), runtime-golden-import, runtime-builders, lost-create,
sealed-image, snapshot-storage, state-images. Tests: `tests/golden_upgrade.rs`.

Every road here awaits `ctx.ready()` first.

## Rules

U1. `upgrade(o)` reads, in order (golden.ts:293-302): the recipe (`ctx.recipe_or_throw(o.recipe)`), the image place,
its copy and the copy's head; no head fails `no golden named "<name>" to update`, no kind. The stage listener is
`ctx.stage_of(name, place)` (not named).
U2. Before any builder is looked at (golden.ts:308-310): `ctx.expire_grace()`, each failure staged as `creating` with
`an earlier kept builder <id>: <message>`, then `ctx.refresh_builders()`.
U3. A kept builder (golden.ts:311): the first in map order whose life is `own` or `reusable`, of the name, at the
place, sealed at the head's version and still in its window (`ctx.in_window`).
U4. With one (golden.ts:313-351): its grace timer is cancelled and removed, `sealed` removed, `building: true` set and
the record written before the first exec. Then `exec("true")` (a rejection reads as not alive). Not alive: killed until
gone (a failed kill fails the update), forgotten, and the update takes the fork road. Alive: stage `creating` with
`your builder from v<head version>, kept since the save`; `roads.apply_delta(machine, delta, {setup, previous_smoke:
head smoke cmd, previous_base: head base, previous_missing?, previous_left_behind?, on_stage})`; then the record's
import (the ledger), `setupSha` (`roads.next_setup_sha(head setupSha, setup, delta import)`), `parentSnapshotId` (the
head's snapshot) and `retired` (`delta.retiredOnImage`) are set, `building` removed, the same four put on the builder,
life `own`, held; stage `ready`. A failed apply kills (failure appended `; <message>`), forgets if gone, stages
`failed` with the detail and fails.
U5. The road is `builder` with a kept builder, else `fork` (golden.ts:352-379): the fork runs through
`ctx.claiming("builder/<name>", .., at, place)` (the bare name, not the place key) as `roads.upgrade_builder` on
`ctx.recording_creates(b, name, delta import, <made>, no stop, place)` with the head, the delta, the setup, cpu, memMb
and envs where set, the builder labels and the stage listener; a failure forgets the placeholder if gone and fails;
else `ctx.settle_builder`.
U6. Then the recipe is kept on the entry and `ctx.seal_entry(entry, true, o.logins)` (golden.ts:380-386).
U7. `keepPrevious: false` (golden.ts:389-413): where `ctx.forked_from(head snapshot)` names workspaces, warns `golden
<name> v<head> kept: <names joined ", "> still on it` and drops nothing. Else, on the fork road with the builder kept,
the builder goes first (timer cancelled, killed until gone, forgotten; a failed kill fails the update) and
`builderKept` becomes false; then `ctx.drop_image(head, at)`, the copy written back without the head's version, its
recipe dropped and `previousDropped` true. A failed drop warns `golden <name> v<head> kept: its snapshot was not
deleted (<message>)` and keeps the copy as the seal left it.
U8. Answers `{manifest, version, road, previousDropped, builderKept}` in that key order (`GoldenUpgradeResult`).
U9. `builder_reach(id)` (golden.ts:418-428): after `refresh_builders`, `no such builder: <id>` for an unknown id; then
`ctx.refuse_untouchable`; then `roads.refresh_preview_token(machine, DAEMON_PORT (7070), entry.reach)`, kept on the
entry; answers `{url, expiresAt, daemonToken?}` with `ctx.daemon_token_of(machine)`.
U10. `builders()` (golden.ts:430-434): after `refresh_builders`, every builder's view in map order.
U11. `kill(id)` with no such builder (golden.ts:444-454): `backend.get(id)` on the runtime backend, a missing machine
read as none and any other failure failing the call; only a machine whose `wsp-owner` label is this state's owner and
whose `wsp-builder` or `wsp-smoke` label is `"1"` is killed until gone; anything else fails `no such builder: <id>`.
U12. `kill(id)` of a builder (golden.ts:456-458): refused only for life `foreign` or `held` (a `building` record is
killed, as the sweep would); killed until gone on its place's backend (`ctx.backend_at`), then forgotten.

## After the port

- U5's claim key is `builder/<name>` while prepare's is `builder/<place>/<name>`, so an update and a copy's prepare of
  one name never exclude each other.
- U4: a kept builder whose exec failed and whose kill then fails leaves its record `building` with no `sealed`.

## Tests

| Rule | Test |
|---|---|
| U1 | `upgrade_refuses_without_a_head`, `upgrade_refuses_without_a_recipe_first` |
| U2 | `upgrade_sweeps_expired_builders_and_stages_each_failure` |
| U3 | `upgrade_takes_a_builder_kept_at_the_heads_version_in_its_window`, `upgrade_passes_over_a_builder_out_of_its_window` |
| U4 | `a_kept_builder_is_marked_building_before_its_first_exec`, `a_kept_builder_that_does_not_answer_is_killed_and_the_update_forks`, `a_kept_builder_takes_the_delta_and_carries_the_heads_snapshot`, `a_kept_builder_whose_delta_fails_is_killed_and_staged_failed` |
| U5 | `the_fork_road_claims_the_bare_builder_key_and_settles`, `the_fork_road_forgets_a_gone_placeholder_when_it_fails` |
| U6 | `the_update_seals_keeping_the_builder_with_the_logins` |
| U7 | `keep_previous_false_keeps_a_version_workspaces_stand_on`, `keep_previous_false_drops_the_head_and_its_recipe`, `keep_previous_false_on_the_fork_road_kills_the_kept_builder_first`, `a_failed_drop_warns_and_keeps_the_copy` |
| U8 | `upgrade_answers_in_the_ts_key_order` |
| U9 | `builder_reach_refreshes_the_preview_token_and_names_the_daemon_token`, `builder_reach_refuses_unknown_and_untouchable` |
| U10 | `builders_lists_every_view_in_map_order` |
| U11 | `kill_takes_an_unrecorded_machine_only_when_it_wears_this_owners_builder_label`, `kill_refuses_a_missing_machine_as_no_such_builder`, `kill_fails_with_the_providers_own_read_failure` |
| U12 | `kill_kills_and_forgets_a_recorded_builder_even_while_building`, `kill_refuses_a_foreign_or_held_builder` |
