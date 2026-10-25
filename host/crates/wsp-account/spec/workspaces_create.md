# workspaces_create

Source: `packages/runtime/src/projects/workspaces.ts:66-288` at 65f6b89c7. Tests: the create, landing, list and resolve
cases of `packages/runtime/test/` (`spawn-guard`, `create-*`, `workspace-resolve`, `folder-threads`, `local-workspace`;
named per test), ported at the area's seam with a fake `WorkspacesCtx`.

## What it does

The workspaces door's create and lookup half: where a project's workspaces land, making a workspace (the project
folder's record on this computer or a joined box, a fork everywhere else, a sub-thread through `spawnSub`), and reading
one or all of them back with the caller's rules.

## Behaviour

1. `landing(o, origin)` (:67-82): `ready()`; a thread asking `sized`: `actRefusal(scope, "size")`; `resolve(project,
   origin)`; a kind that runs in its folder: `{place?, name: placeName(place ?? HERE), capabilities:
   backendOfKind(kind, place).capabilities, kind}` with `place` from `workspaceLands(computer)`; else `placeId` =
   `landingPlace(computer).placeId`, `at` = `landingBackend(placeId)`, `{place?: placeId, name: placeName(placeId ??
   places.wired), capabilities: at.capabilities, kind}`. Key order as written.
2. `create(opts, origin)` (:84-214), in order:
   1. `ready()`; `opts.sub` set: `spawnSub(opts, origin)` and nothing more.
   2. A thread naming `golden`: kind usage `spawnGoldenRefusal(threadId, placeOfThread(scope))`; naming `cpu` or
      `memMb`: `actRefusal(scope, "size")`; naming `parent`: `entryOf(parent, origin)` (its refusal propagates).
   3. `bornOf` = the person's `opts.parent`, or for a thread `opts.parent ?? scope.workspaceId`; `o` = opts with `name`
      = `nameGiven(opts.name)` and `parent` = `bornOf` where set. `project` = `projectsDoor.resolve(o.project, origin)`;
      `kind` = `kindOf(computer)`.
   4. `opts.parent` (the caller's own word) on a folder kind: `refusal(folderForkRefusal(name), folderForkFix(name),
      usage)`.
   5. A folder kind: `refuseRecording(o.name, origin)`; fork words (`--from` for golden, `--size` for cpu or memMb)
      present: kind invalid `copyTakesNone(name, words, placeName(computer)?)`, the place name only on a box; a thread
      naming `agents`: `actRefusal(scope, "agents")`; `folder` = `projectFolder(project)`; with `agents`, the record's
      `agents` = `agentsFrom(spawnAt(place ?? HERE), agents)` and persisted; answers the folder record's view.
   6. A fork: `refuseRelayed({kind, name, project: id}, origin)`; `parent` = `live[o.parent]`; a parent named and not
      live or still `creating`: kind invalid `noParentWorkspaceLine(parent)`; a parent of another project not the same
      repository (`sameRepository(parent's project, project, origin)`): kind invalid `parentProjectRefusal(parent name,
      its project name, project name)`.
   7. `placeGuard(landingPlace(computer).placeId ?? places.wired)` (awaited).
   8. With no await from here to the try: `freePlace` = `spawnGuard("fork", origin)`; a thread naming `agents`:
      `freePlace()` then `actRefusal(scope, "agents")`; `nameRefusal(name)` answering a sentence: `freePlace()` then
      kind conflict with it; `forking.add(name)`; `supersedeFailed(name)`; `id` = `ws_<8 hex>`; `began` = clock;
      `report` = `stageReporter(id, name, began, scope)`.
   9. Try: `start` = `leadStart(parent, name, "fork" when the caller named a parent else "run")` where there is a
      parent; a thread's fork first `showCreate({id, name, machineId: "", phase: "queued", kind, golden: o.golden ??
      "", createdAt: ISO(began), project: refOf(project), ...treeOf(scope), parentWorkspaceId?})`; `made` =
      `createStaged(o, project, id, report, scope, freePlace, start.base)`; no `start`: answer `made`. Else the child's
      record: a brokered branch of record gives `childStartedLine(name, branch)`, else `startChildOn(child, base, parent
      name)`, only when `start.onLeads` and a base; `lines` = `start.lines` then that; none: `made`; else `made` with
      `notice` = `[made.notice, ...lines]` less absent, joined by newlines.
   10. Catch: a live entry for `id`: `unfork(entry)`; gone (ok, or `isMissing`): removed from `live`; else kept: its
       `creating` cleared, persisted (a failure warned `workspace <id> not stored: <why>`), warned `workspace <id> failed
       to create and its machine <machine id> would not stop; the record stays for wsp delete`. The pending create row
       `creates/workspace/<id>` deleted unless it holds a `request`. `shown` = `shownCreates[id]`, deleted. Not kept:
       `failed` = `failedView(id, name, kind, golden ?? "", began, project, said)` where none was shown, else the shown
       view with `phase: "gone"`, `gone: said`; `failedCreates[id] = failed`; a shown one emits `workspace.created`
       with it. `report("failed", said)`; a kept one emits `workspace.created` with its view. The error propagates.
   11. Finally: `shownCreates.delete(id)`, `forking.delete(name)`, `freePlace()`.
3. `get(id, origin, threadId?)` (:216-218): the view of `entryOf(id, origin, {thread}?)`.
4. `held_or_creating(id)` (:220-225): `ready()`; await `bootWork[id]`; the live record's view or none.
5. `thread_place(scope)` = `placeOfThread(scope)` (:227).
6. `list(origin)` (:229-235): `ready()`; `listedFor(origin)` views; a thread gets those alone; the person gets them then
   every shown create and every failed create that has a `parentThreadId`, that `drives(view, origin)` and is not
   already listed by id.
7. `resolve(ref, origin)` (:237-287): `ready()`; `rows` = `held()` for the person, `listedFor(origin)` for a thread;
   `tree` (a thread only) = a held record whose id is `ref` and `talksToTreeOn(ref, origin)`; `exact` = a row with id
   `ref`, else `tree`, else a row with name `ref`; `started` = rows whose id starts with `ref`, only with no exact and
   `ref` at least `ID_PREFIX_MIN` units; more than one: no kind `idPrefixRefusal(ref, ids)`; `entry` = exact or the one
   started.
   - None, for a thread: `mine` = `projectOfScope`; `theirs` = a held record by id, else by name; theirs, mine, and a
     refusal for theirs (`refusalFor`): not of the thread's repository: `refusal(spawnRepositoryWorkspaceRefusal(threadId,
     mine name, ref), FIX, usage)`; `away` = `elsewhereRefusal(origin, project, ref)` on a folder kind, else
     `awayFor(record, origin, "work", ref)`, thrown where it answers; a folder kind not reached: `refusal(
     spawnFolderRefusal(threadId, ref), SPAWN_FOLDER_FIX, usage)`; else `refusal(spawnReachRefusal(threadId, ref),
     spawnReachFix(project name, !folder), usage)`. Otherwise `projectsDoor.resolve(ref, origin)` whose error is
     rethrown unless its kind is `not-found`; then `notFoundRefusal(noWorkspaceRefusal(ref))`.
   - None, for the person: a failed create by id or name answers its view; `placeDoor.find(ref)` naming places:
     `notFoundRefusal(refusalLine(placeNotAWorkspaceLine(first name), placeNotAWorkspaceFix(first name)))`; else
     `notFoundRefusal(noWorkspaceRefusal(ref))`.
   - Found as `tree`: the view parsed as `WorkspaceTalked` (the talked fields alone). Else `refuseRelayed(record,
     origin)` and the view.

## Edge cases

- A thread that names no parent creates a child of its own workspace (`bornOf`), but rule 2.4 reads `opts.parent`, so
  it is never refused as a folder fork.
- `ref` shorter than `ID_PREFIX_MIN` never matches by prefix.
- A failed create's name stays free: `forking` is cleared in `finally`.

## Behaviour nobody wrote down

- Rule 3.4: steps 8 run with no await (the spawn slot is counted and taken in one step; workspaces.ts:139-157).
- Rule 3.16: `ws_<8 hex>`.
- Invariant 3: the parent of a thread's create is its scope's workspace when the ask names none.

## Decisions

1. `freePlace` is a `Rc<dyn Fn()>` the guard hands back; calling it twice is the guard's to make harmless (the TS calls
   it in a refusal and again in `finally` only on the try path).

## After the port

- A failed create whose `unfork` throws a non-missing error keeps the record but still reports `failed` (:182-206). Kept.

## Rules against tests

| Rule | Tests |
|---|---|
| 1 | `landing_on_a_folder_kind_names_the_place_and_its_backend`, `landing_of_a_fork_names_the_landing_place`, `a_thread_cannot_ask_a_sized_landing` |
| 2.1-2.5 | `create_with_sub_goes_to_spawn_sub`, `a_thread_cannot_name_an_image_or_a_size`, `invariant_3_a_threads_create_takes_its_parent_from_its_scope_not_the_ask`, `a_parent_on_a_folder_kind_is_refused`, `create_on_a_folder_kind_answers_the_folder_record_and_refuses_fork_words`, `create_on_a_folder_kind_sets_agents` |
| 2.6-2.8 | `create_refuses_a_parent_not_held_or_still_creating`, `create_refuses_a_parent_of_another_repository`, `create_takes_the_spawn_slot_and_frees_it_on_a_name_taken`, `create_frees_the_slot_on_a_threads_agents` |
| 2.9 | `create_answers_the_staged_workspace`, `create_of_a_threads_fork_shows_it_queued_first`, `create_of_a_child_says_the_lead_lines_in_its_notice` |
| 2.10, 2.11 | `a_failed_create_takes_the_machine_back_and_keeps_a_failed_view`, `a_failed_create_whose_machine_will_not_stop_keeps_the_record`, `a_failed_create_keeps_a_pending_row_holding_a_request`, `a_failed_create_frees_the_name_and_the_slot` |
| 3-6 | `get_reads_through_entry_of`, `held_or_creating_waits_on_boot_work`, `list_gives_the_person_shown_and_failed_forks_once` |
| 7 | `resolve_takes_id_then_tree_then_name_then_a_prefix`, `resolve_refuses_a_prefix_that_starts_two`, `resolve_for_a_thread_refuses_with_the_rule_that_keeps_it_out`, `resolve_for_the_person_answers_a_failed_create`, `resolve_of_a_place_name_says_a_place_is_no_workspace`, `resolve_of_a_tree_workspace_answers_the_talked_fields` |
