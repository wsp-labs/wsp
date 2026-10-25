# projects_add

Source: `packages/runtime/src/projects/projects.ts:108-341` at 65f6b89c7. Tests: the `projects.add`, `seedPlan` and
`projects.seed` cases of `packages/runtime/test/` (named in the table), ported at the area's seam with a fake `ProjectsCtx`.

## What it does

Recording a project: a folder on this computer, or a repo cloned into a folder here or recorded for a computer that
clones it (a box, a provider). The seed half (what of a folder of the person's travels to a computer that clones) is
read through the host's `SeedWiring`, from the menu git and a size pass give, with the choice the person remembered.

## Behaviour

1. `seed_wiring()` (:111-114): `opts.seed`, else no kind `NO_SEED_WIRING`.
2. `landing_kind(computer)` (:119-122): `kindOf(computer)` copies its folder: `mac`; runs in its folder: `box`; else
   `provider`.
3. `landing_deps(computer)` (:126-150): `placeId` = `landingPlace(computer).placeId`; `at` = `forkingAt(placeId)`,
   none on a failure; `joined` = `placeDoor.folderComputer(placeId)` where a place is named; deps: `land` (`landBytes`),
   `scratch` = `GUEST_TMP`, `computer: {machine, home}` where joined, `cloneEnv` = `{GITHUB_TOKEN: <vault's>}` only when
   the vault holds it, a place is named and `placeDoor.githubFromVault(placeId)` is true, else `{}`; `macStateHome` =
   `local.home("claude")` or `""`; `branchHere` = `branchAt`; `computerName` = `placeName(computer)`; `now` = clock.
4. `readable_computer(at, placeId)` (:156-158): no backend and a place named: `landingBackend(placeId)`, whose refusal
   (the computer's absent sentence) propagates.
5. `seed_plan(source, on?)` (:167-176): `ready()`; `folder` = `folderNamed(source)`; `read` = `seedWiring().plan(folder,
   homesHere())`; a computer row naming `on` (`namesPlace`) that is a provider marks `provider: true`; `remembered` =
   store `seed-choices`/`folder`; each file `ticked` = it travels (`seedTravels(plan, f)`) and (no remembered: its own
   `ticked`; else the remembered files hold its path); answers `{...plan, remembered: true?, files}`.
6. `seed(ref, o, origin)` (:180-197): `resolve(ref, origin)`; not this computer or not a folder source:
   `seedNotHereRefusal(name, nameOfComputer(computer, rows))`; `save` from a thread (a scope): `seedSaveRefusal(name,
   o)`; plan = `seed_plan(folder)` with `provider: true`; `prior` = the stored choice; `picks` = `seedPicks(plan, none ?
   [] : prior.files ?? [], keep ?? [], cut ?? [])`; with `save`: cuts with trailing slashes stripped, `held` =
   `heldBackPicks(plan, prior.files ?? [])` less the cuts, store `{files: [...picks.files, ...picks.missing, ...held],
   memory: prior.memory ?? false, commits: prior.commits ?? false, remember: true}`; answers `projectSeedOf(name, plan,
   picks, save)`.
7. `add(o, origin)` (:201-341), in order, each refusal before anything is written:
   1. `ready()`; `refuseRecording(source, origin)`; `rows` = `computerRows()`; `clones` = names of the rows other than
      here whose kind's project sources include `git`.
   2. `kind` = `sourceKind(source)`: `computer`: no kind `ADD_IS_A_COMPUTER_LINE`; a folder with `into`: no kind
      `INTO_TAKES_A_REPO_LINE`.
   3. `source` = `projectSourceOf(source, kind, folderNamed(source) for a folder)`.
   4. `computer` = the row naming `on`; with no `on`, here for a folder or an `into`, else none. None: no `on`: no kind
      `noComputerForSourceLine(source, clones)`; else no kind `noSuchPlaceRefusal(on, rows as tableName)`.
   5. `into` with a computer other than here: no kind `intoIsHereLine(nameOfComputer(here))`.
   6. `opts.cloudProjects === false` and a provider row: `usageRefusal(cloudProjectRefusal(tableName))`.
   7. The computer kind's sources lack the source's kind: a folder gives `folderOnCopyRefusal(name)`, else
      `noComputerForSourceLine`; no kind.
   8. A repo onto a computer that copies its folder: no `into`: no kind `cloneIntoNeeded(source)`; else
      `cloneHere(source, source record, into)` and `add` again with `source: <the folder>` and neither `into` nor `on`.
   9. A held project on the same computer with the same source (`sameSource`): kind conflict
      `sameSourceRefusal(held.name, computer name)`.
   10. `top` = `gitTopOf(path)` for a folder on a copying computer; `landing_deps`; `readable_computer`; `road` =
       `projectLanding(landingKind)`; `resolved` = `projectSource(kind).resolve(source, {seeding: road != mac,
       seedPlan: seed_plan(folder, computer), folderRemote: remoteHere})`.
   11. `seeding` = a seed resolved and road not mac; seeding with no `o.seed`: kind invalid `seedChoiceNeeded(path or
       source)`.
   12. `name` = `o.name ?? resolved.name`; `id` = `o.id ?? pr_<8 hex>`; `path` = `road.path`; `memoryKey` = the seed's
       memory key, else `claudeProjectKey(folder path, else path)`; `places` = `road.places`; the project, keys in
       order: `id, name, computer, source, path, remote, defaultBranch, memoryKey, memoryDir, base?, git: {top}?,
       createdAt (o.createdAt or clock ISO)`.
   13. `report(stage, message)` emits `{type: "project.add", projectId, computer, stage, message, elapsedMs}` and calls
       `o.report(message)`.
   14. `keep(landed)`: the landed fields less `notice` spread over the project; `rememberProject`; with
       `seed.remember` and a folder source, store `{...o.seed, files: unique(o.seed.files ++ heldBackPicks(seed plan,
       prior files))}` under the folder path (held back is empty with no seed plan); emit `{type: "project.added",
       project}`; answer it.
   15. `road.landsAtAdd({seeding})` false: `keep({})` and answer, nothing reported.
   16. Else report `planned` `addingProjectLine(name, source, seeding)`; pack the seed (`seedWiring().pack({plan,
       choice, homes})`) where seeding; a pack that left logins behind reports `seeding` `leftBehindLine(left)`;
       `road.land({project, source module, seed?: {tar, choice, plan}, report}, deps)`; `keep`; report `done`
       `addedProjectOn(recorded, computer name)`; notices = the landed notice, then the left-behind line; answers the
       record with `notice` joined by `; ` where any. A failure anywhere in 16 reports `failed` with its message and
       propagates.

## Edge cases

- A repo here with `into` records a folder project: the second add has no `on`, so its computer is here.
- A provider row named by `on` in `seed_plan` makes every secret-shaped file untravelling (`seedTravels`).
- `o.seed.remember` on a repo source stores nothing.

## Behaviour nobody wrote down

- Rule 3.16: `pr_<8 hex>` from `Ids`. Rule 3.15: `elapsedMs` and `createdAt` off the clock.
- Rule 3.11: `held` projects are compared in insertion order; the first same-source project is named.

## Decisions

1. `add`'s answer is `ProjectAdded {project, notice}` (wsp-core api.rs).

## After the port

- The `failed` report fires for a failure of `keep` after `land` succeeded, so a project recorded on the computer can
  read failed if the store write fails (:337-339). Kept.

## Rules against tests

| Rule | Tests |
|---|---|
| 1, 2, 3, 4 | `landing_kind_by_kind`, `landing_deps_gives_the_vault_token_only_where_github_came_from_the_vault`, `readable_computer_answers_an_absent_computers_sentence` |
| 5 | `seed_plan_ticks_from_the_remembered_choice`, `seed_plan_on_a_provider_marks_provider_and_unticks_secrets` |
| 6 | `seed_refuses_a_project_not_a_folder_here`, `invariant_3_a_thread_cannot_save_seed_picks`, `seed_save_keeps_held_back_files_unless_cut_by_name` |
| 7.2-7.9 | `add_refuses_in_the_ts_order` (one row per refusal, with nothing written and nothing emitted), `add_clones_a_repo_here_into_the_named_folder_and_records_the_folder` |
| 7.10-7.15 | `add_records_a_folder_here_with_its_git_top_and_no_landing`, `add_writes_the_project_keys_in_the_ts_order`, `add_needs_a_seed_choice_to_seed_a_computer_that_clones` |
| 7.16 | `add_lands_a_seed_reporting_each_stage_and_says_done_once`, `add_says_logins_left_behind_as_a_stage_and_a_notice`, `a_failed_landing_reports_failed_and_records_nothing`, `add_remembers_a_seed_choice_with_held_back_files` |
