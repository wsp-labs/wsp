# image
Source: `packages/protocol/src/words/image.ts` (268 lines) at the cut, 383708c78 (466 lines at a3b870a6e). Tests: `packages/protocol/test/format.test.ts` and `setup.test.ts` (the builder's sign-in file case, once `init-job.test.ts`'s) (15 cases ported), plus `pinned` in `tests/image.rs` (every value printed by the TS under node).

## What it does
The image record and each place's copy as `wsp image` and Settings > Image read them; the line a copy build's frame says; a pinned release's moved lines; the lineage marks and the missing tools' rows; and every line a snapshot attempt, a template and a seal end on.

## Behaviour
1. `copy_asks_sign_ins(image)`: no vault and noSecrets not true (image.ts:16). `copy_is_current(image, copy)`: the copy's hash equals the image's; a copy with no hash is never current (image.ts:38-40). `copy_standing`: None when the image has no vault and is not noSecrets; else "current" or "stale" (image.ts:88-91).
2. `sealed_logins_held`: logins whose state is signed-in, copied or not-verified (image.ts:44-49).
3. `sealed_image_line(image)`: parts joined by two spaces: `<name> v<version>`, the hash, the held clause (noSecrets: "no sign-ins on it"; no vault: the no-vault line; else `<n sign-in(s)> held, <vault.paths path(s)>`), `fmt_bytes(usedBytes)` where set, `sealed on <sealedFrom>` (image.ts:52-56). `sealed_copy_line`: place, `v<version>`, size where set, standing where Some (image.ts:95-99). `sealed_built_line` adds `  already built from this image; nothing was built` when not built (image.ts:103-106).
4. `copy_building_line(stage)`: `copying your image: <lower_first(golden_stage_word(stage))>` (image.ts:60). `copy_stopped_line(reason)`: `lower_first("The build stopped")`, with `: <init_stopped_line(reason)>` when that is non-empty (image.ts:65-69). `copy_build_of(stage, detail)`: None for sealed; failed `{line: copy_stopped_line(detail), stopped: true}`; any other `{line: copy_building_line(stage), stopped: false}` (image.ts:75-78).
5. `short_sum`: the first 12 units (image.ts:122-123).
6. `missing_tool_row(t)`: name = id for None or "", mark = outcome else Failed (image.ts:177-179).
7. `provider_answer_line(a)`: `<status> <message> (request <id>)` or `(no request id from the provider, at <at>)` (image.ts:195-197).
8. `snapshot_failed_line(attempts, answer, reading, err)`: a "gone" state says the provider no longer has the builder (404); Unread says it could not be read with err, else "no reason given"; any other state `while the builder read <state>` (image.ts:210-215).
9. `template_status_line`: "the image is saved" for ready, else `saving the image, the provider says <status>; asking again` (image.ts:219-221). `template_recorded_line`: the sharing clause only for a count > 0, "template carries" for 1 (image.ts:236-239).
10. `image_rebuild_fix(place)` is `Seal your image again for <place>.`; `NO_SEALED_IMAGE` and `NO_IMAGE_YET` are both `no image yet` (image.ts:20, 31, 33).

## Edge cases
- A copy with no hash is stale against a vaulted image and has no standing against an image with neither vault nor noSecrets (`a_copy_stands_on_the_hash_alone_and_asks_sign_ins_only_without_a_vault`).
- A vault with 0 paths reads "0 sign-ins held, 0 paths" (`the_record_and_its_copies_in_one_line`).
- `copy_stopped_line(Some("a; a"))` says "a" once (`a_copy_build_reads_its_frame`).

## Behaviour nobody wrote down
- Rule 8.4: 12 units.

## Tapes
`image.get`, `image.build`, `golden.stage` events.

## Decisions
1. `BuilderReading` is `State(word)` or `Unread`; the state words are MachineState's ("gone" among them).
2. `copy_build_of` takes the frame's stage and detail; `sealed_built_line` takes the copy and the built flag.
3. `LINEAGE_MARKS` and `REPO_STATE_WORDS` are enums with `words()` and `pane()`.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `a_copy_stands_on_the_hash_alone_and_asks_sign_ins_only_without_a_vault` |
| 3 | `the_record_and_its_copies_in_one_line` |
| 4 | `a_copy_build_reads_its_frame` |
| 5, 6 | `missing_tool_row_words::*`, `pins_and_tools_rows` |
| 7, 8 | `provider_answer_line_snapshot_attempt_line_and_snapshot_failed_line::*`, `snapshot_refusals_name_the_disk_only_at_danger` (its provider answer and failed lines) |
| 9 | `template_words::*`, `templates_builders_and_seals` |
| 10 | `a_copy_stands_on_the_hash_alone_and_asks_sign_ins_only_without_a_vault` |
