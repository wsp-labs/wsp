# folders_tree

Source: `packages/runtime/src/projects/folders.ts:201-358, 448-472` at 65f6b89c7 (`:359-447` does not port, Proposal 3).
Tests: the worktree removal, sweep and git queue cases of `packages/runtime/test/` (named in the table), ported at the
area's seam with a fake `FoldersCtx` and a fake clock.

## What it does

The host's own git writes on one folder run one at a time. A worktree wsp made is removed only when no turn runs there
and nothing uncommitted would go, its record stays (gone) while threads name it, and one sweep every ten minutes
settles worktrees whose pull request merged or closed or whose branch is gone at the remote, removes one settled six
hours, and stops trying after a week.

## Behaviour

1. `queued(id, work)` (:203-212): `run` = (the key's tail, or resolved) with its failure dropped, then `work`; the
   caller gets `run`'s own result or error; the key's tail is `run` with its failure dropped; when the tail settles and
   is still the key's, the key is removed. Chain, first form (rule 3.9). Memory only, one map for the host.
2. `remove_worktree(entry, force, {ending?, check?})` (:216-232), in `queued(entry id)`: `tree` = the record's
   worktree; no tree, not made by wsp, or no project top: no kind, `notMadeWorktreeLine(tree.branch ?? "")`; no copier:
   kind invalid, `NO_COPIER_HERE`; not ending and `turnRuns(id)`: kind conflict, `WORKTREE_BUSY_LINE`; the path on
   disk and not forced: `refuse_changed(path)`; `check`: answer here; the path on disk: `copier.worktreeRemove({from:
   top, home, path, force})`; delete the setup line; not ending: `worktree_gone(entry, "removed")`.
3. `refuse_changed(path)` (:234-239): `git status --porcelain` in path; a non-zero exit: no kind, `stderr.trim() ||
   stdout.trim()`; lines not blank after trim counted; more than 0: kind conflict, `worktreeChangedLine(n)`.
4. `drop_checkpoints(entry, threadId)` (:242-248): await the thread's `checkpointsLanding` (its failure dropped); a
   record that is not a repo: nothing; else `git.checkpointDrop {cwd: checkoutOf, scope: entry id, thread}` through the
   daemon; a refusal warns `the checkpoints of thread <threadWord(id)> were not dropped: <why>` and answers.
5. `holds_thread(id)` (:250-251): any session row whose view names the workspace, or any thread record that does.
6. `worktree_gone(entry, why, {starting?})` (:254-264): no worktree: nothing. Not starting and no thread holds it:
   `drop(id)` and answer. Else the tree becomes `{path, made, branch?, madeFor?, settled: (its settled, else {at: now,
   why}), gone: true}` in that key order (`kept` and `removeFailed` go), and is persisted.
7. `runs_in(entry, ranIn, folder)` (:267-271): `ranIn` absent: `folder`; a gone worktree whose path holds `ranIn`:
   `folder`; else `ranIn`.
8. Constants (:274-276): `SWEEP_MS` 600000, `SETTLED_STANDS_MS` 21600000, `KEPT_FOR_MS` 604800000.
9. `arm_sweep()` (:280-296): nothing when `state.sweepTimer` or `state.sweeping` is set, when `state.sweepStopped`, or
   when no live record has a worktree made by wsp and not gone. Else schedule `SWEEP_MS` on the clock; when it fires the
   timer slot is cleared, `sweeping` = the sweep, whose failure warns `the sweep of worktrees stopped: <why>`, and when
   it settles `sweeping` is cleared and `arm_sweep()` runs again.
10. `sweep_worktrees()` (:298-304): over a snapshot of `live`, each record with a made, standing worktree that is still
    the live entry for its id, one at a time; a failure of one warns `the worktree at <path> was not swept: <why>`.
11. `sweep_one(entry)` (:308-332), in order: path not on disk: `worktree_gone(entry, "removed")`. `settled` = the
    tree's, else `settled_now`; none, or `kept` is `KEPT_ABANDONED_LINE`: answer. `age` = now - settled.at; under
    six hours: answer. `keep(said)`: `next` = the tree as it stands now with `said` over it and, at a week or more,
    `kept: KEPT_ABANDONED_LINE`; unchanged by its JSON text: nothing; else write it, persist, `statusNow(entry)`.
    A running turn: `keep({kept: KEPT_RUNNING_LINE})`. `git status --porcelain` exit 0 with n changed lines:
    `keep({kept: keptChangedLine(n)})`. Else `remove_worktree(entry, false)`; its failure: `keep({removeFailed:
    lastLineOf(message)})`.
12. `settled_now(entry)` (:336-349): the project's remote empty: none. `readPullRequest(entry, true)`; the record's
    `pr` merged: `{at: mergedAt ?? now, why: "merged"}`; closed: `{at: closedAt ?? now, why: "closed"}`; else, with a
    branch, `branch_gone_at_remote(path, branch)`: `{at: now, why: "deleted"}`. Found: written on the tree, persisted,
    `statusNow(entry)`, answered.
13. `branch_gone_at_remote(path, branch)` (:351-358): `git config --get branch.<b>.remote` and `.merge` (trimmed); either
    empty, or remote `.`: false. Else on the local machine `GIT_TERMINAL_PROMPT=0 <shellLine(["git", "-C", path,
    "ls-remote", "--exit-code", "--heads", remote, merge])>` with `INLINE_EXEC_MS`; true only on exit 2.
14. `git_said(res)` (:453): the first stderr line starting `error: ` or `fatal: `, trimmed; else the last line of
    `stderr.trim() || stdout.trim()` (`""` when both are empty).
15. `worktrees_of(top)` (:458-467): `git worktree list --porcelain`; non-zero: empty. Each `worktree <path>` line starts
    one; a `branch refs/heads/<b>` line sets the last one's branch. Read each time, never kept.

## Edge cases

- `keep` compares `JSON.stringify` of the trees, so the same fields in another key order count as a change.
- A sweep that finds the timer set by a concurrent `arm_sweep` arms nothing more (one timer at most).
- `remove_worktree` with `check` refuses as it would, then answers before removing anything.

## Behaviour nobody wrote down

- Rule 3.9: the git queue is a chain of the first form; one failed write never blocks the next.
- Rule 3.17: the worktree sweep is 10 minutes while a made worktree stands; the clock drives it (rule 3.15).
- Rule 3.27: `rt.close()` does not await the git queue; `sweepStopped` is the close's.

## Decisions

1. The tree comparison serializes both trees with serde_json in struct order; a test pins that a write with no change
   persists nothing.

## After the port

- `settledNow` stamps `deleted` off one `ls-remote`; a remote that answers 2 for a moment settles the tree (:343). Kept.
- The sweep re-reads every pull request each ten minutes with `force` (:339).

## Rules against tests

| Rule | Tests |
|---|---|
| 1 | `queued_runs_one_folders_writes_in_call_order_and_others_alongside`, `a_failed_write_does_not_block_the_next_and_its_caller_sees_it`, `the_queue_forgets_a_key_once_its_tail_settles` |
| 2, 3 | `remove_refuses_a_worktree_wsp_did_not_make`, `remove_refuses_while_a_turn_runs_unless_ending`, `remove_refuses_over_uncommitted_files_naming_how_many`, `remove_with_check_removes_nothing`, `remove_takes_the_tree_and_marks_the_record_gone`, `remove_of_a_tree_gone_from_disk_skips_the_copier` |
| 4 | `drop_checkpoints_waits_on_the_landing_and_warns_a_refusal` |
| 5, 6 | `worktree_gone_drops_a_record_no_thread_names`, `worktree_gone_keeps_a_record_a_thread_names_as_gone_with_its_first_settled`, `worktree_gone_while_starting_keeps_the_record` |
| 7 | `runs_in_moves_a_gone_trees_threads_to_the_folder` |
| 8, 9 | `arm_sweep_holds_no_timer_without_a_made_worktree`, `arm_sweep_arms_one_timer_and_re_arms_after_the_sweep`, `a_stopped_sweep_arms_nothing` |
| 10, 11 | `sweep_one_marks_a_missing_path_removed`, `sweep_one_leaves_a_tree_settled_under_six_hours`, `sweep_one_keeps_a_tree_a_turn_runs_in`, `sweep_one_keeps_a_tree_with_changes`, `sweep_one_removes_a_settled_clean_tree`, `sweep_one_marks_a_failed_removal`, `sweep_one_abandons_after_a_week_and_then_leaves_it`, `sweep_one_writes_nothing_when_the_tree_did_not_change`, `one_failed_tree_does_not_stop_the_sweep` |
| 12, 13 | `settled_now_stamps_merged_and_closed_from_the_pr`, `settled_now_stamps_deleted_when_the_remote_says_no_branch`, `branch_gone_at_remote_reads_exit_2_alone`, `a_branch_with_no_upstream_is_never_gone`, `settled_now_asks_nothing_with_no_remote` |
| 14, 15 | `git_said_takes_the_first_error_line_else_the_last_line`, `worktrees_of_parses_porcelain_and_reads_none_on_failure` |
