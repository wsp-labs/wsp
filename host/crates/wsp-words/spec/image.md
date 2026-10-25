# image
Source: `packages/protocol/src/words/image.ts` (466 lines) at a3b870a6e. Tests: `packages/protocol/test/format.test.ts` and `init-job.test.ts:305` (21 cases ported), plus `pinned` in `tests/image.rs` (every value printed by the TS under node).

## What it does
The image record and each place's copy as `wsp image` and Settings > Image read them; the line a copy build's frame says; project images; pins and the recipe's tools rows; the lineage marks; and every line a snapshot, a template, a seal and an update end on.

## Behaviour
1. `copy_asks_sign_ins(image)`: no vault and noSecrets not true (image.ts:16). `copy_is_current(image, copy)`: the copy's hash equals the image's; a copy with no hash is never current (image.ts:44-46). `copy_standing`: None when the image has no vault and is not noSecrets; else "current" or "stale" (image.ts:94-97).
2. `sealed_logins_held`: logins whose state is signed-in, copied or not-verified (image.ts:50-55).
3. `sealed_image_line(image)`: parts joined by two spaces: `<name> v<version>`, the hash, the held clause (noSecrets: "no sign-ins on it"; no vault: the no-vault line; else `<n sign-in(s)> held, <vault.paths path(s)>`), `fmt_bytes(usedBytes)` where set, `sealed on <sealedFrom>` (image.ts:58-62). `sealed_copy_line`: place, `v<version>`, size where set, standing where Some (image.ts:101-105). `sealed_built_line` adds `  already built from this image; nothing was built` when not built (image.ts:109-112).
4. `copy_building_line(stage)`: `copying your image: <lower_first(golden_stage_word(stage))>` (image.ts:66). `copy_stopped_line(reason)`: `lower_first("The build stopped")`, with `: <init_stopped_line(reason)>` when that is non-empty (image.ts:71-75). `copy_build_of(stage, detail)`: None for sealed; failed `{line: copy_stopped_line(detail), stopped: true}`; any other `{line: copy_building_line(stage), stopped: false}` (image.ts:81-84).
5. Project images: `sealed_project_line` joins snapshotId, `project <workspaceName>`, project names joined ", ", size, createdAt by two spaces (image.ts:116-119); `project_image_in_use_refusal` names the workspaces with `name_list` (a name with a comma quoted) (image.ts:127-128); `project_image_remove_notice` reads the first 10 units of createdAt (image.ts:131-132); `project_image_still_listed_line` says `js_round(grace/1000)` seconds (image.ts:140-141).
6. `logins_here_line(login, left)`: none left: `signed in here as <login>`; else `; <name_list(left)> stays|stay on this computer` (image.ts:157-158).
7. `golden_build_line(from, to, changes)`: changes with count > 0 as `<plural(count, noun)> <word>`; head `Builds version <to>` for from 0, else `... on top of version <from>`; `: ` and the changes joined ", " when any (image.ts:176-180).
8. `short_sum`: the first 12 units (image.ts:183-184). `pin_words`: the tag, `, installs latest` for latest true (image.ts:210-212). `pins_read_line(fixed, latest)`: `pinned: <listed_name(name) tag, ...>` and `installs latest on every place: <listed_name(name) tag[ words], ...>` joined by "; "; None when both are empty (image.ts:217-223). `sealed_pin_line` joins name, tag, `checksum <short_sum>`, latest words by two spaces (image.ts:227-231).
9. `missing_tool_row(t)`: name = id for None or "", mark = outcome else Failed (image.ts:299-301).
10. `provider_answer_line(a)`: `<status> <message> (request <id>)` or `(no request id from the provider, at <at>)` (image.ts:317-319).
11. `snapshot_refused_line(name, answer, disk)`: unknown disk names the reason; else the share `floor(used/size*100)` percent with `fmt_bytes_of`, said as the reason only when `disk_tone(used, size)` is danger (image.ts:326-332).
12. `snapshot_manager_line(name, check, dir, said, output = said)`: a pnpm 10 copy (`<.../.tools/pnpm/><v>` before an optional `_tmp_...` and a "/") names `<copy> and <copy>_tmp_*`; else a pnpm 11 store link `.../store/v<d>/links/@/pnpm/<v>`; else `make <check> answer there` (image.ts:337-350).
13. `snapshot_failed_line(attempts, answer, reading, err)`: a "gone" state says the provider no longer has the builder (404); Unread says it could not be read with err, else "no reason given"; any other state `while the builder read <state>` (image.ts:367-372).
14. `template_status_line`: "the image is saved" for ready, else `saving the image, the provider says <status>; asking again` (image.ts:376-378). `template_recorded_line`: the sharing clause only for a count > 0, "template carries" for 1 (image.ts:393-396).
15. `builder_stays_line` and the update lines write the rate with `to_fixed(rate, 2)` (image.ts:423-455).

## Edge cases
- A copy with no hash is stale against a vaulted image and has no standing against an image with neither vault nor noSecrets (`a_copy_stands_on_the_hash_alone_and_asks_sign_ins_only_without_a_vault`).
- A vault with 0 paths reads "0 sign-ins held, 0 paths" (`the_record_and_its_copies_in_one_line`).
- `copy_stopped_line(Some("a; a"))` says "a" once (`a_copy_build_reads_its_frame`).
- 95 of 100 bytes is danger and names the disk; 899 of 1000 says "89 percent" and that the disk is not the reason (`snapshot_refusals_name_the_disk_only_at_danger`).
- `upgrade_seal_failed_stays_line` with 2.005 writes "$2.00" (`to_fixed` on the double, rule 8.13).

## Behaviour nobody wrote down
- Rules 8.4, 8.12, 8.13: 12 and 10 units, `js_round` seconds, `to_fixed` dollars.
- Rule 8.7: the two pnpm regexes are ported by hand, both over ASCII paths.

## Tapes
`image.get`, `image.build`, `image.export`, `projectGoldens.*`, `golden.stage` events, `init.*` seal lines.

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
| 5 | `project_images` |
| 6, 7 | `golden_build_line::*`, `logins_forgets_and_build_lines` |
| 8, 9 | `a_pinned_release_that_moved::*`, `missing_tool_row_words::*`, `pins_and_tools_rows` |
| 10 to 13 | `provider_answer_line_snapshot_attempt_line_and_snapshot_failed_line::*`, `snapshot_refusals_name_the_disk_only_at_danger` |
| 14, 15 | `template_words::*`, `builder_stays_line_and_the_seal_s_and_the_update_s_last_lines::*`, `templates_builders_and_seals` |
