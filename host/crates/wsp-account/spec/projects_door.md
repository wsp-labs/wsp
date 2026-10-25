# projects_door

Source: `packages/runtime/src/projects/projects.ts:343-481` at 65f6b89c7. Tests: the list, resolve, remove, defaults and
branch cases of `packages/runtime/test/` (named in the table), ported at the area's seam with a fake `ProjectsCtx`.

## What it does

The rest of the projects door: listing (a thread sees the projects it may start children on), resolving a word to a
project with the thread's own rules, what a project's computer holds (folder standing, seed files copied in, unsaved
work), removing a project with everything it made, and the start defaults the app shows per project.

## Behaviour

1. `list(origin)` (:343-348): `ready()`; every held project, for a thread only those `projectReached(origin, id)`;
   sorted by `createdAt` with `localeCompare` (`wsp_words::collate`), stable.
2. `computers()` (:350-353): `ready()`, `computerRows()`.
3. `resolve(ref, origin)` (:355-379): `ready()`; `found` = the first held project whose id is `ref`, else whose name is.
   For a thread: `mine` = `projectOfScope(scope)`; none: `notFoundRefusal(bareNoSuchProjectLine(ref))`; its own
   project by id or name answers first; no `found`: the same not-found; `found` reached: it; `elsewhereRefusal(origin,
   found, ref)` thrown where it answers one; of the thread's repository: `refusal(spawnFolderRefusal(threadId, ref),
   SPAWN_FOLDER_FIX, usage)`; else `refusal(spawnRepositoryRefusal(threadId, mine's name, ref),
   SPAWN_REPOSITORY_FIX, usage)`. For the person: no `found`: `notFoundRefusal(noSuchProjectLine(ref, every name in
   order))`.
4. `folder_stands(id)` (:383-387): `resolve(id)`; the landing road's `folderStands(project, deps)`, false where the road
   has none.
5. `seed_into(id, plan, files)` (:390-397): a road with no `seedInto`: no kind `<name> keeps no folder of its own on
   <computerName> to copy files into`; else pack `{plan, choice: {files, memory: false, commits: false}, homes}` and
   `road.seedInto(project, tar, deps)`.
6. `unsaved(project)` (:400-403): the road's `unsaved(project, deps)`, none where the road has none.
7. `remove(id, origin, {force?, check?, keepLook?})` (:405-437), in order: `ready()`; `spawnGuard("delete", origin)`
   (a thread's refusal); `resolve(id)` (no origin); `held` = live records of the project; `standing` = names of those
   that are not bare folders (`bareFolder(record, holdsThread(id))`); any: kind conflict
   `projectInUseRefusal(name, standing)`; `landing_deps`; `readable_computer`; `unsaved` = asked unless forced without
   check; unsaved and not forced: `usageRefusal(projectUnsavedRefusal(name, unsaved))`; `check`: answers `{said: "",
   unsaved?}`; each held record deleted in order (`workspaces.delete(id, origin)`); with a project top, a state path, a
   copier and a copying computer, `copier.worktreeForget({from: top, home, project: id})`, a failure warned `the frozen
   copies of <name>'s worktrees stayed: <why>`; `said` = `road.remove(project, deps)` (before the record goes, so a
   computer that cannot be reached keeps both); delete from `projectsHeld`; delete the store row `projects/<id>`; emit
   `{type: "project.removed", projectId}`; unless `keepLook`, `projectIcons.forget(id)`; answer `{said}`.
8. `defaults(origin)` (:449-473): `prefs`; `runs(id)` = an adapter is wired for it; `firstAgent` = the first catalog
   agent that runs, else `DEFAULT_AGENT`; the projects `list(origin)` gives; each computer's agent lists read once
   (`computerHome(computer, origin)` then `agentLists`), shared by every project on it; per project
   `resolveThreadDefaults({firstAgent, catalogOf, runs, prefs, project?: prefs.projectDefaults[id]})` where
   `catalogOf(id)` is the list's catalog for a running agent with `withCustomModels(.., prefs.agentDefaults[id].models)`;
   answered as a map in list order.
9. `branch(ref, origin)` (:476-480): `resolve(ref, origin)`, then the landing road's `branch(project, deps)`.
10. `icon` and `icons` forward to the project icons area (:445-446).

## Edge cases

- A project id and another project's name equal to `ref`: the id wins.
- Two projects with equal `createdAt` keep their insertion order in `list`.
- `remove` with `force` and `check` asks `unsaved` and answers it without removing.

## Behaviour nobody wrote down

- Invariant 3: `list` and `resolve` read the scope off `origin` only.
- Rule 3.11: `projectsHeld` is an `IndexMap`; `remove` uses `shift_remove`.

## Decisions

1. `remove`'s `keepLook` is an inherent method's argument (`remove_with`); the `ProjectsApi` signature carries force
   and check only.

## After the port

- `remove` deletes the held records one by one; a failure half way leaves the project with some of its records gone
  (:422). Kept.

## Rules against tests

| Rule | Tests |
|---|---|
| 1 | `invariant_3_projects_list_serves_a_thread_the_projects_it_may_start_on`, `list_sorts_by_created_at_and_keeps_ties_in_order` |
| 3 | `resolve_takes_an_id_before_a_name`, `resolve_for_the_person_names_every_project_when_none_matches`, `resolve_for_a_thread_answers_its_own_project_first`, `resolve_for_a_thread_refuses_in_the_ts_order` |
| 4, 5, 6 | `folder_stands_is_false_on_a_road_with_none`, `seed_into_refuses_a_road_with_no_folder`, `unsaved_asks_the_road` |
| 7 | `remove_counts_only_folders_a_thread_names_as_standing`, `remove_refuses_unsaved_work_unless_forced`, `remove_with_check_answers_unsaved_and_removes_nothing`, `remove_deletes_the_records_then_the_road_then_the_row_and_forgets_the_look`, `remove_with_keep_look_keeps_the_image`, `a_thread_cannot_remove_a_project` |
| 8 | `defaults_read_each_computers_agent_lists_once`, `defaults_fall_back_to_the_default_agent_when_none_runs` |
| 9 | `branch_asks_the_landing_road` |
