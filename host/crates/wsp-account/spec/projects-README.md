# wsp-account, the projects slice

Source at `65f6b89c7`: `packages/runtime/src/projects/workspaces.ts` (1,247 lines), `projects.ts` (576), `folders.ts` (472),
`start-from.ts` (262), `images.ts` (190) and `usage-reset.ts` (183). `projects/branch-of-record.ts` is `wsp-machines`'.
`folders.ts:359-447` (`moveOldCopies`, `moveOldCopy`, `rescueTips`, `refPart`, `COPIES_KEPT_FILE`) does not port:
chapter 02 drops `moveOldCopies` and `copies-kept.txt` under Proposal 3.

## Units

| Unit | Rust | TS | Spec | Tests |
|---|---|---|---|---|
| usage_reset | `src/usage_reset.rs` | usage-reset.ts:1-183 | `usage_reset.md` | `tests/usage_reset.rs` |
| images | `src/projects/images.rs` | images.ts:1-190 | `images.md` | `tests/project_icons.rs` |
| start_from | `src/projects/start_from.rs` | start-from.ts:1-262 | `start_from.md` | `tests/start_from.rs` |
| folders_place | `src/projects/folders/place.rs` | folders.ts:16-200 | `folders_place.md` | `tests/folders_place.rs` |
| folders_tree | `src/projects/folders/tree.rs` | folders.ts:201-358, 448-472 | `folders_tree.md` | `tests/folders_tree.rs` |
| projects_add | `src/projects/projects/add.rs` | projects.ts:108-341 | `projects_add.md` | `tests/projects_add.rs` |
| projects_door | `src/projects/projects/door.rs` | projects.ts:343-481 | `projects_door.md` | `tests/projects_door.rs` |
| projects_move | `src/projects/projects/moves.rs` | projects.ts:23-106, 482-574 | `projects_move.md` | `tests/projects_move.rs` |
| workspaces_create | `src/projects/workspaces/create.rs` | workspaces.ts:66-288 | `workspaces_create.md` | `tests/workspaces_create.rs` |
| workspaces_phases | `src/projects/workspaces/phases.rs` | workspaces.ts:22-65, 289-645 | `workspaces_phases.md` | `tests/workspaces_phases.rs` |
| workspaces_git | `src/projects/workspaces/git.rs` | workspaces.ts:647-1053 | `workspaces_git.md` | `tests/workspaces_git.rs` |
| workspaces_review | `src/projects/workspaces/review.rs` | workspaces.ts:1055-1247 | `workspaces_review.md` | `tests/workspaces_review.rs` |

## How each area reaches the runtime

Each TS area takes the whole `RuntimeContext`. Under ruling 4 of #2448 (comment 6401022701) each Rust area declares the
calls it makes as a trait of its own, and the wsp-core join implements them on the context handle:
`UsageResetDeps` (the TS deps object, usage-reset.ts:53), `ProjectIconsCtx`, `StartFromCtx`, `FoldersCtx`, `ProjectsCtx`
and `WorkspacesCtx`. A method keeps its TS name in snake case. The shared maps come out of the trait as
`Rc<RefCell<IndexMap<..>>>`, as machines a does (`live`, `projects_held`, `sessions`, `thread_records`), and a live
entry is `Entry = Rc<RefCell<LiveWorkspace>>`, machines a's alias (`wsp-machines/src/machines/machines.rs:38`). By
ruling 2 the join makes every entry of one workspace share one record; until then the record is read and written
through the entry. Every type this slice takes from a crate that is not on `rust/2448-base` yet is named in
`src/projects/workspaces/seam.rs` (`crate::projects::workspaces::seam`; `projects.rs` is the skeleton owner's), so the swap to the joined crates is one file.

The areas themselves are structs over `Rc<dyn ...Ctx>`: `Workspaces` implements `wsp_core::types::api::WorkspacesApi`,
`Projects` implements `ProjectsApi`; the area handle traits of `wsp_core::context` (`FoldersArea`, `StartFromArea`,
`WorkspacesArea`, `ProjectsArea`, `ProjectIconsArea`) stay empty and the structs carry the methods the other areas call.

## Rules every unit holds

- **One thread.** Rule 3.1: no lock; a `RefCell` borrow is never held across an `.await` or a call out (an `emit`, a
  store write, a context method, a callback; rule 3.3). A test re-enters from a context method to catch a held borrow.
- **Await points.** Rule 3.4: an `.await` where the TS awaits and none where it does not between a read and a write of
  shared state. The two stretches the spec chapter names in this slice: `workspaces.create` takes the spawn slot with no
  await between the count and the take (workspaces.ts:139-157), and the two-deletes gap of rule 3.10 (workspaces.ts:575-578
  against :569, the `refuseChanged` await before `entry.deleting` is set) stays unguarded.
- **Time** comes from the context clock (rule 3.15), ids from `Ids` (rule 3.16): `ws_<8 hex>`, `pr_<8 hex>`, UUIDs, the
  draft prompt file's 12 hex and the icon part file's 8 hex.
- **Errors** are `HostError` with the TS kind: `Object.assign(new Error(s), {kind})` is `HostError::with_kind`, a bare
  `new Error(s)` is `HostError::new` (no kind), and a refusal built by `refusal()`/`usageRefusal()` carries `fix`.
- **Order.** Maps iterate in insertion order (rule 3.11): `live`, `projectsHeld`, `sessions`, `threadRecords`, the git
  write queue and the setup lines are `IndexMap`s and every removal is `shift_remove`.
- **Console lines.** `console.warn` is `wsp_core::console::warn` with the TS words; tests read them where the TS test
  does.

## Runtime invariants these files hold

`packages/runtime/AGENTS.md` invariants 1, 2, 5 and 6 are not held here. What is:

- **Invariant 3** (origin stamped off the token): every door reads the caller's scope off `origin` through `scope_of`,
  never off a field of the ask. Named tests: `invariant_3_a_threads_create_takes_its_parent_from_its_scope_not_the_ask`
  (workspaces_create), `invariant_3_projects_list_serves_a_thread_the_projects_it_may_start_on` (projects_door),
  `invariant_3_a_thread_cannot_save_seed_picks` (projects_add).
- **Invariant 4** (nothing posts as the person through a copy's gh; a merge sends the head the person was shown):
  `invariant_4_merge_sends_the_held_head_not_a_fresh_read` and
  `invariant_4_review_post_runs_on_this_computer_pinned_to_the_drafts_head` (workspaces_git, workspaces_review).
- **Traps**: `copyBlocked()` before acting in a copy (#1383), pinned per door in workspaces_git; `mergeIn()` refuses
  while the lead runs a turn except the asking thread's own (#1476):
  `merge_in_refuses_while_the_lead_runs_a_turn_but_not_the_asking_threads_own`; every worktree record is made by
  `worktreeRecordAt()`, which keeps `madeFor` (#1607): `worktree_record_at_keeps_made_for_of_the_record_it_writes_again`;
  `ls-remote --exit-code` exit 2 alone is gone (#1607): `branch_gone_at_remote_reads_exit_2_alone`; the roots file before
  a turn's snapshot (#1607, #1645): `record_folder_writes_the_daemon_roots_before_it_emits_created`; a bare folder record
  counts with `bareFolder()` (#1634): `remove_counts_only_folders_a_thread_names_as_standing`.

## What survives a restart

Stored, and so read back by a new host: the workspace record (`persist`), `workspace-names`, `creates` pending rows,
the projects map (`PROJECTS`), `seed-choices`, `usage-reset-pending` and the project icon files. Memory only, lost on
restart: `running` resets (a second press after a restart starts its own spend, reusing the stored key), the git write
queue, the setup lines a worktree left for its first start, the icon write chain, `failedCreates`, `shownCreates`,
`viewedMarks` beyond `persistSessions`, and the sweep timer (re-armed by `armSweep()` at start, boot.ts).

## Cross-slice names

`brokeredStart` and `pullRequestOfRecord` (branch-of-record.ts) are `wsp-machines`'; start_from reaches them through
`StartFromCtx`. `cutOff` (pipe.ts) is `wsp-core`'s (`wsp_core::pipe::cut_off`), reached through `WorkspacesCtx`.
