# folders_place

Source: `packages/runtime/src/projects/folders.ts:16-200` at 65f6b89c7. Tests: the folder, worktree and `folderFor` cases
of `packages/runtime/test/` (named in the table), ported at the area's seam with a fake `FoldersCtx`.

## What it does

Where a thread on this computer, or on a box the person joined, runs: the project folder's record (made at its first
thread), the record of a worktree holding a branch (made by the daemon binary's copier under the host's folder, or one
git already has), and `folderFor`, which reads the start's words (project, branch, cwd) against the asking thread.
A worktree this call made runs its module installs and the project's after-worktree command once, and the line saying
what ran is kept for the first start that names it.

## Behaviour

1. `record_folder(project, worktree?, parent?)` (:28-62), in order: `kind` = `kindOf(computer)`; `lands` =
   `workspaceLands(computer, None)`, `place` its place when it lands at a place; `moduleOf(kind).admitFolder({kind,
   name, place?})` where the module has one; `mine` = `backendOfKind(kind, place)`; `machine` = `mine.get(place ??
   LOCAL_MACHINE_ID)`; `id` = `ws_<8 hex>`; the record, keys in this order: `id`, `name` (`takenNameAfter(project.name`,
   or `<name>@<branch>` for a worktree with a branch, every live record's name)), `kind`, `place?`, `machineId`,
   `phase: "running"`, `golden: ""`, `createdAt` (clock, ISO), `project`, `worktree?`, `parentWorkspaceId?`, `spec: {}`,
   `size: mine.pricing.defaultSize`, `firstLife: false`, `idleWindowMs: null`. Then `attach(record, machine)`,
   `persist`, `writeDaemonRoots(live entry)`, `arm_sweep()` when `worktree.made` is true, emit `{type:
   "workspace.created", workspace: view(record)}`, answer the live entry.
2. `project_folder(project)` (:64-68): the first record of `foldersOf(project.id)` with no worktree; else
   `oneFolder("<id>\0", record_folder(project))`, so two first threads at once make one record (join, rule 3.9).
3. `worktree_folder(project, top, branch, parent?, madeFor?, checkpoint?)` (:72-90), joined under `"<id>\0<branch>"`:
   no `local.copier`: kind invalid, `NO_COPIER_HERE`; a `local.hereDaemon` whose `version()` is below `DAEMON_VERSION`:
   no kind, `hereDaemonBehindLine(version, DAEMON_VERSION, fix)`, before the copier runs; `copier.worktree({from: top,
   home: stateFolder(), project: id, branch, modules: CARRY_MODULES, checkpoint?})`; a path equal to `top`: the project
   folder; else `worktree_record_at(project, {path, branch, made, madeFor?}, parent)`; when `made.fresh`,
   `settle_worktree` and a line it answers is kept in the setup lines under the record's id.
4. `CARRY_MODULES` = the catalog's ecosystem modules as `{id, lockfiles, carry, never, installed?}` (:17).
5. `take_setup_line(id)`: answers and deletes the kept line (:94-98). Memory only.
6. `settle_worktree(project, made)` (:104-126): `own` = `preferences.projectDefaults[id].afterWorktree`; commands =
   each module the report marks `rebuild`, mapped to every ecosystem module of its id as `{command: rebuild, folder}`,
   then `{own, "."}`; none: nothing. On the local machine, each in order: `at` = `made.path`, or `made.path/folder`;
   `shown` = `homeShortened(at, home)`; emit `host.notice` `worktreeCommandRunningLine(command, shown)`; exec
   `cd <shellLine([at])> && <command>` with `timeoutMs` 180000 (`WORKTREE_COMMAND_MS`) and an empty stdin; exit 0:
   `worktreeStepWords(command, folder)`; else `end` = `{stoppedAfterMin: 3}` for `CHILD_TIMED_OUT`, else `{exitCode,
   said: lastLineOf(stderr.trim() || stdout.trim())}`; warn `<command> in <at> exited <code>: <stderr or stdout,
   trimmed>`; emit `host.notice` `worktreeCommandFailedLine(command, shown, end)`; the step words with `end`. Every
   command runs whatever the one before did. Answers `worktreeSetupLine(steps)`.
7. `worktree_mounted(entry)` (:130-136): only for a worktree wsp made, not gone, with a project top, a copier and its
   path on disk: `copier.worktreeMount({from: top, home, path})`; a failure warns `the worktree at <path> was not
   mounted again: <why>` and the call still answers.
8. `worktree_record_at(project, tree, parent?)` (:140-149): the folder record of the project whose worktree path is
   `tree.path`: written again as `{path, made: was.made || tree.made, branch? (tree's), madeFor? (the record's own)}`,
   persisted, `arm_sweep()`, answered. None: `record_folder(project, tree, parent)`.
9. `folder_for(o, origin)` (:154-158): `place_for`, then with `opens` the setup line taken for that record.
10. `place_for(o, origin)` (:159-200), in order:
    - `scope` = `scopeOf(origin)`; `asking` = the live entry of the scope's workspace.
    - No project named and an asking thread on a kind that does not run in its folder: usage
      `refusalLine(guestNamesWorkspaceLine, GUEST_NAMES_FIX)`.
    - `own` = the asking record's project where that record has a `place`; a named project that is neither its id nor
      name and names (by id or name) a project on another computer: `elsewhereRefusal(origin, that, word)` thrown where
      it answers one.
    - `project` = `projectsDoor.resolve(o.project, origin)` when named, else the asking record's project; none: usage
      `NAME_A_PROJECT_LINE`.
    - A kind that does not run in its folder: usage `notOnThisComputerLine(name)`.
    - `picks` given: `picksHold(project, picks)`.
    - `beside` = the asking entry where its kind runs in its folder and its project is this one.
    - Both branch and cwd: usage `BRANCH_OR_CWD_LINE`.
    - A joined box (runs in folder, does not copy): a branch is usage `placeBranchLine(placeName(computer))`; `cwd` =
      `posix.normalize(o.cwd)`; not absolute or not under `project.path`: usage `cwdOutsideLine(cwd, name)`; answers
      `beside` when it has no worktree, else the project folder, with `cwd`.
    - This computer, with a cwd: `folderNamed(cwd)`; under `project.path`: `beside` (no worktree) or the project folder,
      with `cwd`; else the worktree of `top` holding it; none (or no top): usage `cwdOutsideLine(homeShortened(cwd),
      name)`; the one at `top`: the project folder; else `worktree_record_at(project, {path, branch?, made: false,
      madeFor?: scope.rootThreadId})`.
    - No branch: `beside` or the project folder. No top: usage `noBranchesLine`. `branchAt(checkout of beside, or
      project.path)` equal to the branch: `beside` or the project folder. Else `worktree_folder(project, top, branch,
      beside id, scope.rootThreadId)`.
11. `under(path, folder)` (:455): equal, or starts with `folder` plus `/` (no second slash when it ends in one).

## Edge cases

- A record for a worktree made with plain `git worktree add` is `made: false` and carries the asking tree's root thread.
- `cwd` on this computer is `folderNamed` (home and links resolved, rule of wiring.ts), on a box only normalized.
- A setup line is kept only when `fresh`; a record written again keeps no line.

## Behaviour nobody wrote down

- Rule 3.9, join: `oneFolder` keys are the project id with `\0` and the branch; two starts of one branch make one record.
- Rule 3.4: the record's name reads every live name with no await before `attach`.

## Decisions

1. `oneFolder` is a context method taking a boxed future, as the TS takes a thunk.

## After the port

- `worktree_record_at` writes the tree anew from four fields, so a record written again loses `settled`, `gone`, `kept`
  and `removeFailed` (:145): a worktree made again is swept as a new one. Kept; pinned by
  `worktree_record_at_keeps_made_for_of_the_record_it_writes_again`.

## Rules against tests

| Rule | Tests |
|---|---|
| 1 | `record_folder_writes_the_record_in_the_ts_key_order`, `record_folder_writes_the_daemon_roots_before_it_emits_created`, `record_folder_names_a_worktree_project_at_branch_and_takes_a_free_name`, `record_folder_arms_the_sweep_only_for_a_made_worktree` |
| 2 | `project_folder_answers_the_held_record`, `two_first_threads_make_one_project_folder` |
| 3, 4, 5 | `worktree_folder_refuses_with_no_copier`, `worktree_folder_refuses_a_daemon_behind_before_the_copier_runs`, `worktree_folder_at_the_top_is_the_project_folder`, `worktree_folder_keeps_the_setup_line_of_a_fresh_worktree_for_one_start`, `two_starts_of_one_branch_make_one_worktree` |
| 6 | `settle_worktree_runs_installs_then_the_after_worktree_command_and_says_each`, `settle_worktree_says_a_timeout_in_minutes_and_a_failure_with_its_last_line`, `settle_worktree_with_nothing_to_run_answers_nothing` |
| 7 | `worktree_mounted_mounts_only_a_made_standing_worktree`, `a_failed_mount_warns_and_answers` |
| 8 | `worktree_record_at_keeps_made_for_of_the_record_it_writes_again`, `worktree_record_at_makes_a_child_of_the_asking_folder` |
| 10 | `place_for_refuses_a_guest_naming_no_project`, `place_for_refuses_a_box_thread_naming_another_computers_project`, `place_for_refuses_branch_and_cwd_together`, `place_for_on_a_box_refuses_a_branch_and_a_cwd_outside`, `place_for_runs_beside_the_asking_thread`, `place_for_with_a_cwd_in_a_plain_git_worktree_records_it_for_the_tree`, `place_for_on_the_folders_own_branch_runs_in_the_folder`, `place_for_a_branch_makes_a_worktree_child_of_the_asking_folder`, `folder_for_takes_the_setup_line_only_when_it_opens` |
| 11 | `under_matches_the_folder_and_below_and_not_a_sibling_prefix` |
