# project_seed
Source: the functions of `packages/protocol/src/project-seed.ts` (233 lines) at a3b870a6e; its flat constants (`SEED_DIR`, `SEED_MEMORY_DIR`, `SEED_PATCH`) are `wsp_wire::types::project_seed`, and the shapes it reads (`SeedPlan`, `SeedFile`, `SeedChoice`, `ProjectSeed`) are `wsp_wire::views::workspace`. Tests: `packages/protocol/test/project-seed.test.ts` (20 of 22 cases ported in `tests/project_seed.rs`; the 2 of `describe("the key a project's memory sits under")`, :33-43, test `claudeProjectKey` and `claudeMemoryDir` of `project_path` and are skipped). No `pinned` module.

## What it does
The rules every client reads a seed menu by: which rows of a folder's git-ignored files start ticked, what a person's keeps, cuts and flags do to that, and which rows never leave this computer. It also writes every line a seed says: the menu's rows and the consent lines under it, a project's saved picks, the seeding stage's lines, and the refusals of a pick off the menu.

## Behaviour
1. `seed_travels(plan, file)`: false for kind never; false for kind secret when `plan.provider` is exactly true; else true (project-seed.ts:22-23). An absent or false provider lets a secret travel.
2. `held_back_picks(plan, remembered)`: the remembered paths, in their order and with any repeats, that name a menu row of kind secret that does not travel on this plan (:27-28).
3. `default_seed_choice(plan)`: files are the menu rows ticked and travelling, in menu order; memory is `plan.memory` not null; commits is `plan.unpushed` not null; no `remember` (:35-41).
4. `seed_choice_from(plan, keep, cut, flags)`: files are `seed_picks(plan, default files, keep, cut).files`, its error passed up; memory and commits are the default's unless the flag is exactly `Some(false)` (a `Some(true)` adds nothing the plan lacks); `remember` is `Some(true)` only when that flag is `Some(true)`, else None (:45-58).
5. `seed_picks(plan, start, keep, cut)`, in order (:80-106): trailing "/" runs come off each keep and cut path, never off a start path; missing is every start path no menu row names; any keep or cut equal to ".git" or starting ".git/" refuses with `never_git_line`; then each keep, in order, refuses with `not_in_the_menu_line` when no row names it, `never_travels_line` (the row's name, else None) when its kind is never, `stays_here_line` when it does not travel; then each cut refuses with `not_in_the_menu_line` unless a row names it or it is missing. Files are the menu rows, in menu order, that travel, sit in start or keep, and are not cut; missing less the cuts. Every refusal is an Invalid error, its message the line alone.
6. The refusal lines are fixed sentences (:62-77): `not_in_the_menu_line` lists the menu's paths in plan order joined ", ", or "nothing" for an empty menu; `never_travels_line` says "a login" without a row name, "<row>, a login" with one.
7. `project_seed_of` (:109-118): one row per menu row in menu order, `why` from `seed_row_words(file, plan)`, picked where `picks.files` names it; picked paths in menu order (a pick no row names is dropped), their bytes summed, missing copied. `project_seed_lines` (:122-133): rows sorted picked first, then bytes descending, then `js::collate` of the path; no rows reads as no table at all, else a header `["", "PATH", "SIZE", "WHY"]` and cells "x" or " ", the path with "/" for a folder, `fmt_bytes`, why. Foot: the missing line first where any are missing, then "<n> picked." (with ", <bytes>" before the full stop when n is not 0) and the saved or not-saved sentence.
8. `seed_save_refusal(project, o)` (:137-142): a usage refusal whose fix is "Hand them the line " and `shell_line` of `wsp projects seed <project>`, then `--none` when `o.none` is `Some(true)`, each keep as `--keep <p>`, each cut as `--cut <p>`, then `--yes`. `seed_not_here_refusal(project, computer)` (:145-146): a usage refusal. Both messages are `refusal_line(happened, fix)`.
9. `seed_row_words(file, plan)` (:149-156): secret is the row's name, else "values", when it travels, else "stays on this computer"; config is the row's name, else "config"; rebuilt, data and never have fixed words; any other kind is "not in the catalogue". Without a plan a secret travels.
10. `seed_bytes(plan, choice)`: the bytes of the menu rows the choice names, each row once, a named path no row has adds nothing (:161-163). `seeding_line` (:168-174): "0 files" with no bytes, else `plural(n, "file")` and the bytes in brackets; the memory part where the choice takes memory and the plan has some; the commits part where the choice takes commits and the plan has unpushed commits; then ", from <source>.". `seed_commits_landed_line`: 0 is "no commits to land", else "<n> commit(s) landed" (:179).
11. `seed_summary_lines(plan, choice)` (:183-191), one line each in order: "no files travel" or the count, bytes and ticked paths in menu order; the memory line on the same terms as rule 10; the commits line with the first 7 UTF-16 units of `unpushed.base`; the uncommitted line only when `uncommitted` is over 0.
12. `seed_menu_rows(plan, choice)` (:197-206): the header always; the files sorted bytes descending then `js::collate` of the path, each `[x]` or `[ ]`, path ("/" after a folder), `fmt_bytes`, `seed_row_words(file, plan)`; then a memory row `<key>/memory` where the plan has memory, ticked off `choice.memory`; then a `<branch> as a patch` row with an empty size where the plan has unpushed commits, ticked off `choice.commits`.
13. `seed_consent_lines(plan, computer, add_line)` (:210-216): "nothing was sent. ", the uncommitted clause only when over 0, "<computer> clones <remote, else "the repo"> itself."; then the two lines with `add_line("--yes")` and `add_line("--yes --keep <path> --cut <path> --no-memory --no-commits")`, the labels padded with three and four spaces. `left_behind_line`, `seed_commits_lost_line`, `no_remote_line` are fixed sentences (:221-233).

## Edge cases
- `seed_choice_from(plan, ["node_modules/"], [], ..)` ticks node_modules: the trailing slash comes off a keep (test).
- `.git` kept and `.git/config` cut both refuse with `never_git_line` of that path, exit class usage (test); ".gitignore" passes the check (:94).
- `seed_picks(plan, [".env.local", "old.env"], [], ["old.env"])` takes the cut of a missing start path and drops it from missing; `seed_picks(plan, ["old.env"], ["old.env"], ["old.env"])` still refuses, since keeps are checked before cuts (test).
- On a provider plan a ticked or started secret drops out of files silently and is not missing (test); keeping it by name refuses with `stays_here_line`, class usage (test).
- `seed_choice_from` with no flags has `remember` None (test).
- `project_seed_lines` with nothing picked: every tick cell is " " and the foot is "0 picked. Nothing was saved; add --yes to save these picks." (test).
- `seeding_line` of a memory-only seed: "Seeding 0 files, Claude Code memory (1 file, 69 B), from /Users/dev/spoo-landing." (test).
- `seed_summary_lines` cuts the base "9f1c2e4aa11b..." to "9f1c2e4"; a bare plan with nothing ticked reads `["no files travel"]` (test).
- `seed_menu_rows` on a provider plan holds `["[ ]", ".env", "120 B", "stays on this computer"]` (test).
- `seed_commits_landed_line(0.0)` is "no commits to land"; 1 is "1 commit landed" (test).

## Behaviour nobody wrote down
- Rule 8.8: both sorts break a bytes tie with `localeCompare`, so `js::collate`; bytes compare as numbers first.
- Rules 8.4 and 8.5: `base.slice(0, 7)` cuts 7 UTF-16 units; a sha is ASCII, so no pair is split in practice.
- Rule 8.7: `/\/+$/` has an ASCII class; it trims trailing "/" alone.
- Rule 8.11: every count in a sentence goes through `plural` or `String(n)`, so `js::number_string`.
- `row` absent and `remote` null read alike as "no name": the fallback words above. Only `provider: true` holds a secret back.
- An Invalid error (rule 5) carries no fix and its message is the line alone; a usage refusal (rule 8) joins a fix with `refusal_line`. Both exit as usage.

## Tapes
`wsp projects seed` and the `projects_seed` tool (`project_seed_lines`, `seed_save_refusal`, `seed_not_here_refusal`); the `wsp add` seed menu and consent lines; the add's seeding stage (`seeding_line`, `seed_commits_landed_line`, `seed_commits_lost_line`, `left_behind_line`); the add's refusal of a folder with no remote; the sub-thread's picked-elsewhere words (sub-thread.ts:343).

## Decisions
1. `seed_travels` and `seed_row_words` take a whole `&SeedPlan` and `&SeedFile` where the TS takes `Pick`s; the TS default plan `{}` of `seed_row_words` is `None`.
2. Throws are `Result<_, HostError>` (`seed_picks`, `seed_choice_from`); the two refusals answer a `HostError`.
3. Structural TS objects become structs: `SeedFlags` (the TS default `{}` is `SeedFlags::default()`), `SeedPicks` for `{ files, missing }` (also `project_seed_of`'s picks), `SeedLines` for `{ head, rows, foot }`, `SeedAsked` for `{ keep?, cut?, none? }`.
4. `seed_bytes` answers `f64`; `seed_commits_landed_line` and `seed_commits_lost_line` take the count as `f64`, the TS number.
5. `seed_consent_lines` takes `add_line` as `&dyn Fn(&str) -> String`; string lists are `&[impl AsRef<str>]`.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `a_file_of_values_or_a_key::*`, `what_a_seed_ticks_before_anybody_touches_the_menu::never_ticks_a_login_whatever_a_remembered_choice_says_about_it` |
| 2 | no test pins it |
| 3 | `what_a_seed_ticks_before_anybody_touches_the_menu::*`, `a_file_of_values_or_a_key::is_ticked_for_a_box_as_configuration_is_and_named_by_its_row` |
| 4 | `what_a_person_s_own_words_make_of_the_menu::*` |
| 5 | `a_project_s_seed_picks::start_where_they_are_told_and_keep_a_start_path_the_menu_no_longer_shows_as_miss`, `what_a_person_s_own_words_make_of_the_menu::*`, `a_file_of_values_or_a_key::drops_out_of_saved_picks_for_a_provider_place_and_stays_in_them_for_a_box`, `a_file_of_values_or_a_key::never_goes_to_a_provider_place_unticked_whatever_was_remembered_shown_as_staying` |
| 6 | `what_a_person_s_own_words_make_of_the_menu::refuses_a_cut_that_names_no_row_so_a_typo_never_leaves_the_path_it_meant_ticked`, `what_a_person_s_own_words_make_of_the_menu::refuses_to_carry_a_login_by_name_even_when_it_is_the_path_they_kept` (in part: `never_travels_line` by its words alone) |
| 7 | `a_project_s_seed_picks::draw_the_tick_column_one_wide_whether_anything_is_picked_or_not_and_name_the_mis` |
| 8 | no test pins it |
| 9 | `the_words_a_row_ends_in::*`, `a_file_of_values_or_a_key::is_ticked_for_a_box_as_configuration_is_and_named_by_its_row`, `a_file_of_values_or_a_key::never_goes_to_a_provider_place_unticked_whatever_was_remembered_shown_as_staying` |
| 10 | `what_the_seeding_stage_says_as_the_files_go_and_once_they_have_landed::*`, `what_the_person_is_told_would_travel::is_the_ticked_files_the_memory_the_patch_and_what_stays_here` |
| 11 | `what_the_person_is_told_would_travel::*` |
| 12 | `a_file_of_values_or_a_key::never_goes_to_a_provider_place_unticked_whatever_was_remembered_shown_as_staying` (one row alone) |
| 13 | no test pins it |
