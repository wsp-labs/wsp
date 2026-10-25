# images_records: the records area and the held images

TS: `images/records.ts` (245 lines) and `images/held.ts` (37). Rust: `src/images/records.rs` (`RecordsArea` over
`Rc<dyn RecordsCtx>`), `src/images/held.rs` (`HeldImages` over `Rc<dyn HeldCtx>`). TS tests: none call these files
directly; the whole-host cases in sealed-image, copy-on-press, places-remove-provider, runtime-create and
snapshot-storage read them through the runtime. Tests: `tests/images_records.rs`.

## Rules

R1. `view(r)` (records.ts:88-125) answers a `WorkspaceView` with keys in this order, each optional key absent when its
source is: `folder` (`ctx.module_folder(r)`), the keys of `ctx.home_of(r)` (`home`), `id`, `name`, `machineId`,
`phase`, `kind`, `golden`, `createdAt`, `project` (`ctx.ref_of(ctx.project_held(r.project))`), `worktree`, `from`,
`review`, `branchOfRecord`, `claudeSessionId`, `screen`, `queue`, `gone`, `theme`, `glyph`, `daemonNote`
(`ctx.daemon_note(r.id)`), `daemonRefusedAt`, `vaultedAt`, `vaultRefused`, `wakeRefused`, `diskGone`, `agents`
(`ctx.agents_of(r)`), `parentThreadId`, `rootThreadId`, `parentWorkspaceId`, `place`, `provider`
(`ctx.provider_of(r)`), `pauseMode`.
R2. `pauseMode` is `ctx.backend_for(r)`'s `capabilities.pause_mode`; absent where the backend has none or
`backend_for` fails (records.ts:80-87). One unreadable backend never fails the view.
R3. `put_look(r, key, value)` (records.ts:129-133): `None` (TS undefined) leaves the record, `Some(None)` (null)
removes the key, `Some(Some(v))` sets it. Keys are `theme` and `glyph` only.
R4. `persist(r)` (records.ts:135-142): bumps the live entry's `generation` by one first, where `ctx.live()` holds
`r.id`; then `store.put("workspaces", r.id, r)`; then `store.put("workspace-names", r.id, {workspaceId, name,
project})` in that key order. A failed first put fails the call and writes no name document; no entry, no bump.
R5. `shape_of(m)` (records.ts:144-147): `m.describe()`; a failure answers `None`.
R6. `size_built(shape, asked)` (records.ts:150-153): each of `cpu` and `memMb` is the shape's where it has one, else
the asked one.
R7. `read_memory(record, machine, sizes)` (records.ts:157-170) runs `MEM_READ` with `timeoutMs` `INLINE_EXEC_MS`
(20,000). Exit 0: `mem_mb_of(read_values(stdout))`; a stdout with no count answers false and warns nothing. Exit
non-zero: the reason `exit <code>` plus `: <stderr trimmed, its last 200 UTF-16 units>` where the trimmed stderr is
not empty. A rejected exec: its message. A reason warns `workspace <id>: memory not read on <machine id> (<reason>);
the row keeps <size_word(record.size)>` and answers false, record untouched.
R8. A count (records.ts:165-169) picks the first offered size with `|s.memMb - count| <= s.memMb / 16`; else, where
the largest offer is above 0, `min(count, largest)`; else the count. Only `size.memMb` changes; answers true.
R9. `forked_from(snapshot)` (records.ts:173): the names of live entries whose record's `golden` is the snapshot, in
live map order, whatever their phase.
R10. `copy_of(place, name)` reads `goldens/<place>/<name>`; `put_copy` writes it; `copy_recipe_of(place, name, v)`
reads `golden-recipes/<place>/<name>@v<v>`; `put_copy_recipe` writes it; `drop_copy_recipe` deletes that one key
(records.ts:179-190, less the bare fallback Proposal 3 drops). A stored value that does not parse reads as `None`.
R11. `record_of(name)` reads `images/<name>`; nothing stored answers `None` (the backfill, records.ts:229-242, is
dropped by Proposal 3).
R12. `recorded_images()` (records.ts:248-258): every stored manifest's versions' `snapshotId` then `templateId` where
set, manifests in store list order, then every live entry's `golden`; a set, first insertion kept.
R13. `image_of(snapshot)` (records.ts:261-277): the first manifest in store list order whose versions hold the
snapshot; answers `{golden: snapshot, version?}` with that version, or no `version`.
H1. `held.on(place)` (held.ts:21-25) awaits `ctx.ready()` first, then walks `store.keys("goldens")` in order: a key
whose place (`copy_key_parts`, a key with no place read as `ctx.wired()`) is not `place` is skipped, a missing value
skipped; every version of each copy becomes `HeldImage { id: snapshotId, template_id }`, copies in key order,
versions in manifest order.
H2. `held.drop(place, id)` (held.ts:26-34) does not wait on `ready`. For each copy at the place holding a version of
that snapshot: the copy is deleted where no version is left, else written back with the rest (every other key of
the manifest kept); then `ctx.drop_copy_recipe(place, name, version)` for each dropped version, in order. A copy
not holding it is not written.

## Restart

Nothing here is memory: every read goes to the store, so a new area over the same store answers the same
(`a_new_area_over_the_same_store_reads_every_copy_back`).

## After the port

- R4: a crash between the two puts leaves a record without its name document; the TS has no transaction there.
- The TS's `generation++` is the only signal a persist happened; nothing awaits it.

## Tests

| Rule | Test |
|---|---|
| R1 | `view_carries_every_present_fact_in_the_ts_key_order`, `view_leaves_out_every_absent_fact` |
| R2 | `view_leaves_out_pause_mode_when_the_backend_cannot_be_read`, `view_carries_the_backends_pause_mode` |
| R3 | `put_look_sets_clears_and_leaves` |
| R4 | `persist_bumps_the_generation_then_writes_record_and_name`, `persist_without_a_live_entry_writes_both`, `persist_that_fails_writes_no_name_document` |
| R5 | `shape_of_answers_none_when_describe_fails` |
| R6 | `size_built_takes_the_providers_word_else_the_ask` |
| R7 | `read_memory_warns_the_exit_and_the_stderr_tail`, `read_memory_warns_a_rejected_exec`, `read_memory_with_no_count_answers_false_quietly`, `read_memory_runs_mem_read_with_the_inline_timeout` |
| R8 | `read_memory_snaps_to_an_offer_within_a_sixteenth`, `read_memory_caps_at_the_largest_offer`, `read_memory_with_no_offers_keeps_the_count` |
| R9 | `forked_from_names_every_live_fork_of_the_snapshot_in_order` |
| R10 | `copies_and_recipes_are_filed_under_the_place`, `drop_copy_recipe_deletes_only_that_version`, `a_bare_key_is_never_read` |
| R11 | `record_of_reads_the_stored_record_and_nothing_else` |
| R12 | `recorded_images_holds_versions_templates_and_live_goldens` |
| R13 | `image_of_finds_the_version_in_the_first_manifest_holding_it`, `image_of_an_unknown_snapshot_has_no_version` |
| H1 | `held_on_waits_for_ready_and_lists_the_places_versions`, `held_on_skips_other_places` |
| H2 | `held_drop_writes_back_the_rest_and_drops_each_recipe`, `held_drop_deletes_a_copy_left_empty`, `held_drop_leaves_copies_without_the_snapshot_unwritten` |
| Restart | `a_new_area_over_the_same_store_reads_every_copy_back` |
