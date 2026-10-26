# turns_target: where a run goes and the threads a line names

Port of `packages/host/src/verbs/turns-help.ts` lines 229-420 at 65f6b89c7 (`/root/cli-c-ts/turns-help.ts`).
Rust: `src/verbs/turns_help/target.rs`. Probe: `tests/fixtures/cli_c/record/turns_target.probe.ts` (views in `turns_views.ts`).
Recorded cases: `tests/fixtures/cli_c/turns_target.json`, one test each in `tests/cli_c/turns_target.rs`. Drivers:
`tests/cli_c/drivers_turns_target.rs`. Hand tests: `tests/cli_c/turns_target_rules.rs`.

## Scope

`FolderTarget`, `ForkTarget`, `RunTarget` (231-241), `runTarget` (247), `threadsInFolder` (296, private), `forkFor` (304),
`subFor` (320), `projectOfFolder` (341), `threadHere` (351), `subDeleting` (361), `refuseMachineThread` (370), `threadAt`
(381), `childOf` (396), `refuseNotAThread` (404, private), `sharingWith` (411), `threadDeleted` (417). They lean on
`workspaces`, `threads`, `workspaceOf`, `threadOf`, `projectOf`, `projectsOf`, `projectsHere` and `createFor`
(`workspaces_help`), which the scripted host answers for them.

Two changes to the TS shape, both because the TS reads a module constant (`CLOUD_ON`) a Rust test cannot flip in one process:
`run_target` and `child_of` take `cloud_on: bool`, and `thread_at_in` is `thread_at` with it (`thread_at` keeps the lead's
signature and reads `cloud_on()`). `refuse_not_a_thread` and `threads_in_folder` are `pub(crate)`.

## Behaviour

- `runTarget` (247): with a ref, the ref is read first, before any token: `projects.resolve {ref}`, else the first project of
  `projects.list` (or of the thread's own workspaces if the list is refused) whose id or name is the ref. A project on this
  computer, or one whose `workspaces.landing` kind runs in its folder (a refused or kindless landing does not), is a folder
  target (`here.project` is `id`, `name`, `path` with the path from a second `projects.resolve {ref: id}`, "" if that is
  refused; then `branch`, `cwd`). Any other project: a branch is refused; with the cloud off, or with no machine of that name
  (`workspaces.list`), the answer is `fork: project` (the whole object the host sent); a machine of that name goes on as a
  word. With no project: cloud off reads `projects.resolve` once more and answers its refusal unless it is `not-found`,
  then `no_project_line`; cloud on answers `workspaceOf`, a branch refused after it.
  With no ref: a turn token (`WSP_TURN`) or host token (`WSP_HOST_TOKEN`) that is not empty means beside the thread asking
  (`here` holds only branch and cwd, nothing is asked, even for a guest); else a guest is refused (`elsewhere`, nothing
  asked); else `project_of_folder` over `projects.list` (a refused list is no project): none is `no_thread_target_line`; one
  gives the whole project, branch, cwd, and the caller's folder as `cwd` when it is outside the project's folder (a worktree)
  and neither branch nor cwd was named. `opened(id, folder?)` says `thread <id>` with no folder, else `thread_opened_line`
  with the folder shortened against HOME for a folder on this computer and not otherwise.
- `forkFor` (304): `workspaces.list`, then `createFor` with the name taken off the task (first line, five words, forty
  characters, numbered where taken). `say` hears each stage of that name (message trimmed at its end) and the created line.
- `subFor` (320): the name is the title (not cut) or off the task, numbered; `workspaces.list`, `events.subscribe`,
  `workspaces.create {project or "", name, sub}`; stages are said whole, from the subscription on until `stop` (also after
  the answer); the notice is said a line at a time; `sub` is read as `SubStarted`.
- `projectOfFolder` (341), `threadHere` (351), `subDeleting` (361), `refuseMachineThread` (370), `threadAt` (381), `childOf`
  (396), `sharingWith` (411), `threadDeleted` (417): as the doc comments in `target.rs` say.

## Limits and caps

The name of a machine is forty characters at most and five words (cases at 39 and 40, six words, a taken name numbered past a
gap: 2, 3, then 4 when 2 and 3 and 5 are taken, past 3 taken names). The numbers start at 2.

## Null and empty inputs

No ref and no folder, an empty ref, no projects, a refused list of projects, no thread, a thread with no `sub`, a ref naming no
thread or two, a notice that is empty or ends in a newline, a title that is empty (a name of ""), a landing with no kind, a
`where` with empty branch and folder (kept, named). Each is a recorded case.

## Order

`runTarget` with a ref: `projects.resolve`, `[projects.list, [workspaces.list]]`, then `workspaces.landing`, `projects.resolve`
(the path), `[workspaces.list]`, `workspaces.resolve`; with none, no frame (token, guest) or `projects.list`. `forkFor`:
`workspaces.list`, `workspaces.landing`, `events.subscribe`, `workspaces.create`. `subFor`: `workspaces.list`,
`events.subscribe`, `workspaces.create`. `threadAt`: `sessions.list`, `workspaces.get`; a ref naming no thread then reads
`projects.list` (with a fallback to `workspaces.list` when refused) and with the cloud `workspaces.list` again. The project
wording wins over the machine's. `sub` of a create: `on`, `size`, `from`, `branch`, `harness`. `sessions.delete`:
`threadId`, `processes`, `check`.

## Invariants held

- Host invariant 7: with the cloud off `run_target`, `thread_at` and `child_of` never ask for a machine (`workspaces.resolve`),
  never answer one and never name one (`<word> is a machine`, `no workspace`). Cases come in pairs, cloud off and on.
- Guest rule: `run_target` for a guest with no ref and no thread token is refused before any frame, and before the caller's
  folder is looked at; a ref or a token comes first, as the TS orders them.
- A machine is made last: `run_target` never sends `workspaces.create`; `fork_for` and `sub_for` send it after every read.

## What the TS leaves unclear

- `runTarget` line 290 names a boolean `inside` that is true when the caller is outside the project's folder (a worktree).
- A project found in the list by name takes the first of two of one name, though the host's own `projects.resolve` may
  prefer the thread's; the list is only read when `projects.resolve` refused.
- With the projects not listable the thread's own workspaces name the project; the whole project is then not known and
  `here.project.path` is "" unless the second `projects.resolve` answers.
- `forkFor` trims the end of each stage it says; `subFor` says it whole (a trailing newline included).
- `refuseMachineThread` refuses with a plain error stamped `usage`, so it carries no fix; a missing workspace record is let by.
- `projectOfFolder` reads the disk for a worktree: a `.git` with no `gitdir:` line, or one pointing outside `<main>/.git/worktrees/`,
  is no repository; an unreadable `.git` file throws in the TS (not recorded).
- A walk up from a folder with no `.git` goes to `/`; a `.git` at `/` or `/tmp` would be found (no case relies on its absence).

## Rule -> test

| Rule | Tests |
|---|---|
| run_target: ref branches, cloud off and on | `run_target__*` (each in two settings) |
| run_target: token, guest, caller's folder, worktree | `run_target__no_project_*`, `a_thread_token_runs_beside_the_thread_and_asks_nothing` |
| invariant 7, cloud off | `cloud_off_never_names_a_cloud`, `cloud_on_names_a_machine` |
| guest rule, refused before any read | `elsewhere_run_target_refuses_before_any_read` |
| the ref is read before the token | `a_ref_is_read_before_the_token` |
| a machine is made last | `a_machine_is_made_after_every_read` |
| names off the task, numbering | `a_machine_is_named_off_the_task_and_a_taken_name_is_numbered`, `fork_for__*`, `sub_for__*` |
| `sub` field order, stop | `sub_for_writes_the_sub_in_one_order`, `sub_for_stops_listening_when_stopped` |
| project_of_folder, deepest, worktree, disk | `project_of_folder__*`, `project_of_folder_takes_the_deepest_and_then_the_first` |
| thread_here, sub_deleting, refuse_machine_thread | `thread_here__*`, `sub_deleting__*`, `refuse_machine_thread__*`, `a_machine_thread_is_refused_as_a_usage_error_with_no_fix` |
| thread_at, child_of, refuse_not_a_thread | `thread_at__*`, `child_of__*` (project, machine, both, cloud off and on) |
| sharing_with, thread_deleted | `sharing_with__*`, `thread_deleted__*`, `thread_deleted_sends_the_thread_then_what_was_asked` |
| signature kept | `thread_at_keeps_the_signature_the_other_units_call` |
| ported TS tests | `ported_ts_tests_are_recorded_cases` (run-beside 4, verbs-turns intents) |
