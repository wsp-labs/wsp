# images

Source: `packages/runtime/src/projects/images.ts` (190 lines) at 65f6b89c7. Tests: `packages/runtime/test/project-icons.test.ts`
(6 cases: 2 on `keptProjectIcon`, ported whole; 4 drive a whole host, ported at the area's seam with a fake
`ProjectIconsCtx`; the ops table case belongs to the serve crate and is not ported).

## What it does

A project's image is a PNG kept under `<state folder>/project-icons/<sha256>.png`, never in preferences. The host checks
that what the window sent is the one PNG shape the window makes and keeps only its IHDR, IDAT and IEND chunks.
`projectIcons(dir)` is the file store (keep, read, holds, sweep); `projectIconsArea(ctx)` is the runtime's door, which
names the hash in `preferences.projectIcon` and deletes every file no project and no recipe names.

## Behaviour: `kept_project_icon(png)` (:52-82)

Each refusal is `usageRefusal(said, fix)` with `projectIconRefusal(why)`: said `The host did not take this image: <why>.`,
fix `PROJECT_ICON_AGAIN`, kind usage (:26-29). Checks in this order, the first that fails answers:

1. `len > 81920` (`PROJECT_ICON_MAX_BYTES`): `it is <len> bytes, over the 81920 an image may be`. Exactly 81920 passes.
2. Shorter than 8 bytes or not the PNG signature: `it is not a PNG`.
3. Chunks from byte 8 while `at < len` (:31-47): fewer than 12 bytes left, or `at + 12 + length > len`: `it ends inside
   a chunk`; CRC-32 of type and data not equal to the stored one: `its <type> chunk fails its checksum` (type read as
   latin1). Reading stops after the first `IEND`; bytes after it are ignored. The checksum is checked chunk by chunk, so a
   bad CRC before a missing IHDR is what is said.
4. First chunk not `IHDR` or its data not 13 bytes: `it does not open with its header`.
5. Last chunk read not `IEND`: `it has no end chunk`.
6. Width or height not 128: `it is <w> by <h> pixels, not 128 by 128`.
7. Depth not 8: `it has <depth> bits a channel, not 8`.
8. Colour type not 6 (4 channels) or 2 (3 channels): `its colour type is <colour>, not RGBA or RGB`.
9. Compression or filter byte not 0: `its header names a method PNG does not have`.
10. Interlace not 0: `it is interlaced`.
11. Any `acTL` chunk: `it is animated`.
12. `stride = 1 + 128 * channels`, `expected = 128 * stride` (65664 RGBA, 49280 RGB). The IDAT data concatenated in order
    is zlib-inflated with an output cap of `expected + 1`: more than `expected + 1` bytes out gives `its pixels inflate
    past the <expected> bytes 128 px holds` without inflating the rest; any other inflate failure (no IDAT, not zlib, a
    cut stream) gives `its pixels do not inflate` (:72-78).
13. Inflated length not `expected` (`expected + 1` included): `its pixels inflate to <n> bytes, not <expected>`.
14. Any row's first byte (index `row * stride`) over 4: `row <row> names a filter PNG does not have`, first such row.
15. Kept bytes: signature, the IHDR chunk whole, every IDAT chunk whole in order, the IEND chunk whole (:81).

## Behaviour: `project_icons(dir)` (:95-134)

- `keep(png)`: check as above, hash = lowercase hex SHA-256 of the kept bytes; make `dir()` recursively with mode 0700;
  write `.<hash>.<8 hex>.part` with mode 0600; rename it to `<hash>.png`; answer the hash (:106-114). The 8 hex come from `Ids`.
- `read(hashes)`: one entry per hash in the order asked; a hash that does not match `^[0-9a-f]{64}$` is null without a
  read; a file that cannot be read is null; else `data:image/png;base64,<standard base64 of the file>`, read now (:115-126).
  A hash asked twice is one key at its first place.
- `holds(hash)`: the listing of `dir()` holds `<hash>.png` (:127). A listing that fails is empty.
- `sweep(named)`: delete every `<64 hex>.png` whose hash is not in `named`, then every name ending `.part` (:128-132).
  Deletes are forced: a file already gone is no error. Names that are neither are left.

## Behaviour: the area (:137-190)

1. One chain, `inTurn`: every set, forget, wear and recipe sweep runs after the one before settles; the caller gets its
   own result or error, the chain's tail swallows it (:140-145, rule 3.9 chain, first form).
2. `sweep()` (:147-155): `named` = every value of `preferences.get().projectIcon`; then each recipe the shelf lists, each
   folder of its file in order, its `image` where set. The shelf absent reads as none. A shelf whose `list()` fails: the
   sweep returns and deletes nothing. The files are swept only where `ctx.opts.statePath` is set.
3. `named(projectId, hash)` (:156-160): `changePreferences` with `projectIcon` less `projectId`, then with
   `projectId: hash` appended last when the hash is not null. A re-set moves the project's key to the end.
4. `set(projectId, png)` (:164-172), in its turn: `ready()`; a project not in `projectsHeld`: usage
   `bareNoSuchProjectLine(projectId)` and nothing is written; png null: hash null; else keep the base64-decoded bytes
   (a refusal propagates, nothing named); `named`; `sweep`; answer `{image: hash}`.
5. `read(hashes)` (:173): no `statePath`: every hash null, in order; else the store's read. Not in the chain.
6. `forget(projectId)` (:174-182), in its turn: `changePreferences` dropping `projectLook[projectId]` and
   `projectIcon[projectId]`, keeping every other key in order; then `sweep`.
7. `wear(projectId, hash)` (:183-187), in its turn: nothing when the hash is absent, there is no `statePath`, or the
   store does not hold it; else `named(projectId, hash)`. No sweep.
8. A `recipes.changed` event queues a sweep in the chain; its failure is swallowed (:161).

## Edge cases

- The base64 the window sends is decoded as Node's `Buffer.from(s, "base64")` decodes it: both alphabets, characters
  outside them skipped, decoding stopped at the first `=`.
- `set(id, null)` on a project with no image writes the preferences unchanged in content and still sweeps.

## Behaviour nobody wrote down

- Rule 3.3: the chain is a stored future; no borrow of the area is held while a step awaits preferences or the shelf.
- Rule 8.3 does not apply: every count here is bytes.

## Decisions

1. Inflate is zlib (RFC 1950) with a hard output cap, as `inflateSync({maxOutputLength})`.
2. The `.part` suffix's random part comes from `Ids::bytes(4)` as hex.

## After the port

- `set` keeps the file before the record names it; a crash between leaves a file the next sweep deletes. Kept, and the
  TS says so (:130).
- `wear` names a hash without sweeping, so the old hash of that project stays on disk until the next sweep (:183-187).

## Rules against tests

| Rule | Tests |
|---|---|
| kept 1-15 | `takes_a_128_px_rgba_or_rgb_png_and_keeps_its_header_pixels_and_end_alone`, `refuses_each_file_that_breaks_a_rule_naming_the_rule_and_asking_again` (17 rows, every sentence whole), `the_checks_run_in_the_ts_order`, `inflate_cap_is_one_byte_past_the_rows`, `exactly_81920_bytes_passes_the_size_check`, `bytes_after_iend_are_ignored` |
| store | `keep_writes_the_hash_file_at_0600_under_a_0700_folder_and_leaves_no_part`, `read_answers_each_hash_in_order_from_disk_null_for_a_bad_hash_or_missing_file`, `holds_reads_the_listing_now`, `sweep_deletes_unnamed_hashes_and_every_part_and_leaves_other_names` |
| area 1 | `set_forget_and_wear_run_one_at_a_time_in_call_order`, `a_failed_set_does_not_stop_the_next` |
| area 2 | `sweep_keeps_a_file_a_recipe_folder_names`, `an_unreadable_recipe_shelf_deletes_nothing`, `no_state_path_sweeps_no_files` |
| area 3, 4 | `set_keeps_the_file_names_it_last_and_answers_its_hash`, `set_refuses_a_project_not_held_and_writes_nothing`, `a_refused_png_names_nothing_and_keeps_nothing`, `set_null_takes_the_image_off_and_deletes_the_file_nothing_names` |
| area 5 | `read_with_no_state_path_answers_null_for_each` |
| area 6 | `forget_drops_the_look_and_the_image_and_deletes_the_file` |
| area 7 | `wear_names_a_held_hash_and_ignores_one_not_held` |
| area 8 | `a_recipes_changed_event_sweeps_in_turn` |
| base64 | `set_decodes_base64_as_node_does` |
