# golden_storage: snapshot storage, orphans, retention, rollback and promote

TS: `images/golden.ts:66-72, 461-611`. Rust: `src/images/golden/storage.rs`, methods of `GoldenArea`. TS tests:
snapshot-storage (9), snapshot-owners (10), sealed-image, golden-templates (9), runtime-builders. Tests:
`tests/golden_storage.rs`.

## Rules

S1. The account read (golden.ts:68): where the runtime backend bills named snapshots by count
(`billed_by_count`), listings over the whole account pass `Some([])` (named rows only); else `None`.
S2. A storage read (golden.ts:69-72): a backend without `snapshot_listing` answers `None`; else
`snapshot_storage(<listing of the ids>, <backend pricing>, {hostId: ctx.image_mark(), recorded:
ctx.recorded_images(), now: clock})`. `storage()` (golden.ts:461-464) awaits ready and reads the runtime backend
with the account ids.
S3. `orphans()` (golden.ts:466-486): ready; no listing answers `None`. The owner read (`recorded_images` and the clock)
is taken before the listing. Snapshots and the backend's templates (none where `templates_of` has none) are each
split by owner; answers `{snapshots: <orphan snapshots>, templates: <orphan templates>, freedBytes: <their sizes>,
savesUsdPerMonth: monthly(total) - monthly(total - freed), others: {snapshots, templates: <foreign>}}`, the monthly
figure `snapshot_monthly_usd(bytes, pricing, named count)` with the named count less the orphans' named count.
S4. `delete_orphans()` (golden.ts:488-519): `None` where `orphans()` is. Templates are deleted first, each failure a
`failed` row `{id, name?, message}`; then snapshots, where a missing snapshot counts as deleted and any other failure
is a `failed` row. One failure stops nothing.
S5. `retention_places(name)` (golden.ts:521-533): ready; the image place first; then the wired provider where the
image place is another and a copy is filed there; then every other place a copy of the name is filed at that still
has a backend (`places.backend` or the door's `backend_of`), sorted by UTF-16 code units (plain `sort`).
S6. `retention(name, asked)` (golden.ts:535-547): ready; the asked place or the image place; no listing or no copy
there answers `None`; else `retention_plan(manifest, <listing of the manifest's snapshot ids>, forked_from, pricing)`
with `place`, `placeName` and `copy: place != image place` added.
S7. `prune(name, asked)` (golden.ts:549-574): no plan answers `{dropped: [], failed: []}` with no `storage`. Each
version to drop: `ctx.drop_image(v, at)`; a failure that is not missing is a `failed {version, message}` and the
version stays; otherwise the copy is re-read and written back without it, its recipe dropped, and it is `dropped`.
`storage` is the storage read over the kept versions' ids, a failure there leaving it out.
S8. `rollback(version, name)` (golden.ts:576-589) does not wait for ready: no wired copy fails `no golden named
"<name>"` kind `missing`; `roads.rollback`'s failure fails with its message, kind `missing`; else the rolled-back
manifest is written as the wired copy and answered.
S9. `promote(name)` (golden.ts:591-611): ready; no templates on the runtime backend answers `None`. Each wired
version with no template, in order: `roads.promote_version(templates, snapshot, golden_name(image mark, name,
version))`; the copy re-read (gone fails `golden <name> was dropped while its versions were being promoted`), the
version's `templateId` set and written back; the row `{golden, version, templateId, sharing?}`. Any failure, the name
included, is the row `{golden, version, error: <message>}` and the loop goes on. No copy answers `[]`.

## Tests

| Rule | Test |
|---|---|
| S1 | `storage_reads_named_rows_alone_where_snapshots_bill_by_count` |
| S2 | `storage_is_none_without_a_listing`, `storage_reads_the_owner_mark_recorded_images_and_the_clock` |
| S3 | `orphans_split_snapshots_and_templates_and_price_what_deleting_saves`, `orphans_is_none_without_a_listing` |
| S4 | `delete_orphans_deletes_templates_then_snapshots_and_counts_a_missing_one_deleted`, `delete_orphans_reports_each_failure_and_goes_on` |
| S5 | `retention_places_lists_own_then_wired_then_sorted_others_with_a_backend` |
| S6 | `retention_plans_the_asked_place_and_says_whether_it_is_a_copy`, `retention_is_none_without_a_listing_or_a_copy` |
| S7 | `prune_drops_each_planned_version_and_its_recipe`, `prune_counts_a_missing_snapshot_as_dropped`, `prune_keeps_a_version_whose_delete_failed`, `prune_without_a_plan_drops_nothing`, `prune_leaves_out_storage_a_failed_listing_did_not_read` |
| S8 | `rollback_refuses_missing_and_writes_the_rolled_back_wired_copy`, `rollback_marks_the_engines_refusal_missing` |
| S9 | `promote_templates_each_version_without_one`, `promote_writes_a_failed_row_and_goes_on`, `promote_is_none_without_templates`, `promote_fails_a_version_whose_golden_was_dropped_meanwhile` |
