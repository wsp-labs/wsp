# workspaces_git

Source: `packages/runtime/src/projects/workspaces.ts:647-1053` at 65f6b89c7. Tests: `changes`, `bring-back`, `tree`,
`pull-request`, `folder-threads`, `exec` cases of `packages/runtime/test/` (named per test), ported at the area's seam
with a fake `WorkspacesCtx` whose daemon `ask` records each op.

## What it does

Everything that runs in or reads a workspace's checkout: commands and command streams, bringing work back as a branch
and a pull request, the folder doors, the Changes pane (checkout, discard, commit, a drafted message, viewed marks), the
pull request page and the acts on it, fixing a check or a stopped merge through the agent, merging, updating, and
merging a child into its lead.

## Behaviour

1. `exec(id, cmd, o, origin)` (:647-652): `entryOf(.., {act: "exec"})`; `copyBlocked`; `worktreeMounted`;
   `machine.exec(cmd, o)`.
2. `exec_stream(id, argv, cwd, origin)` (:654-691): as exec, then `launchAdapterFor(entry)`; `ranIn` =
   `threadFolder(entry, {cwd})`; the factory with idle and deadline unbounded runs `inFolder(ranIn, argv quoted and
   joined)` with the adapter's env; registered in `execs` as `{workspaceId, end(reason)}`; `end` settles the reason and
   kills the inner stream. The lines a reader gets: the inner lines until done; an end raised meanwhile fails the read
   with the reason; on finishing (any way) the exec is unregistered and the inner reader let go. `exited` = the inner
   exit or none once ended. `ranIn` on the answer where known.
3. `bring_back({workspaceId, title, body}, origin)` (:693-746): `spawnGuard("bring_back")`; `entryOf`; `copyBlocked`;
   `cwd` = checkout; `base` = the record's, else the project's; a pull request start off a fork not pushable: no kind
   `START_WORDS.forkNotPushable(owner)`; an issue start: `body` = `withCloses(body ?? "", number)`. In
   `queued(id)` and `withDaemon`: `git.push {cwd, base?}`; on the default branch: `defaultBranchRefused("bring back",
   name, git.status branch head)`; a child with no git credential: `keepTree(entry, {...tree, pushRefused: said})`;
   then the error propagates. A kept `pushRefused` is dropped after a push. A child of a lead thread
   (`parentThreadId`): `{branch, base, ahead, uncommitted, stat, note: childPushedLine(branch)}`. Else `git.pr {cwd,
   base?, title?, body?}`: a refusal for no host command line is `{note}`, any other `{refused}`, else `{pr}`; the
   answer `{branch, base, ahead, uncommitted, stat, note|refused|pr}`. Then a `pr`: `takePullRequest(entry, {...pr,
   readAt})`; `readLeadOf(entry)`.
4. `folder_for(o, origin)` (:748-752): `ready()`; folders' `folderFor`; `{workspace: view, cwd?, setup?}`.
   `take_setup_line(id, origin)`: `ready()`, the line for `entryOf(id).id`.
5. `folder({project}, origin)` (:759-765): `ready()`; resolve; not a folder kind: usage `notOnThisComputerLine`;
   `refuseRecording(name, origin)`; the project folder's view.
6. `worktree({project, branch}, origin)` (:767-778): resolve; not a folder kind: usage `notOnThisComputerLine`; a box:
   usage `placeBranchLine(placeName)`; no top: usage `noBranchesLine`; `worktreeFolder(project, top, branch, None,
   scope.rootThreadId)`; `{path: tree.path ?? top, branch, made: tree.made === true}`.
7. `worktree_remove({project, branch, force, check}, origin)` (:780-794): no top: usage `noBranchesLine`; `holding` =
   git's worktree on that branch; `entry` = the first folder record with a standing worktree at `holding`'s path, or
   by branch where git holds none; found: `refuseRelayed`; not made by wsp: kind invalid `notMadeWorktreeLine(branch)`;
   force from a thread: usage `WORKTREE_FORCE_LINE`; `removeWorktree(entry, force, {check}?)`.
8. `checkout(id, origin, fresh=false)` (:796-807): `readCheckout(entry, fresh)`; not fresh: `readPullRequest(entry,
   false)` and, where any live record is its child, `readTree(entry)`, both not awaited; `{checkout?}`.
9. `discard({workspaceId, path, check, threadId}, origin)` (:809-820): `entryOf(.., {thread}?)`; `copyBlocked`; check:
   `git.status`; neither `editsUnread` nor an entry for `path` other than `!!`: no kind `noChangeLine(path)`; `{path}`.
   Else `git.discard {cwd, path}` in `queued` through the daemon; `readCheckout(entry, true)`; the reply.
10. `commit({workspaceId, message, paths, threadId}, origin)` (:822-839): `spawnGuard("commit")`; `entryOf(.., {act:
    "commit", thread?})`; `copyBlocked`; in `queued` through the daemon: `named` = `paths`, else `git.diff {cwd, scope:
    "head"}` files' paths; none named from the diff: no kind `cleanCheckoutLine(labelOf(entry, threadId))`;
    `git.commit {cwd, message, paths}`; `readCheckout(entry, true)`; the reply.
11. `commit_draft({workspaceId, paths, threadId}, origin)` (:841-879): `spawnGuard("commit")`; `paths` empty: `{message:
    null, note: DRAFT_NOTES.nothing}` before anything; `entryOf(.., {act: "commit"})`; `copyBlocked`; the thread's rows
    by `startedAt`; a thread named with no row: `notFoundRefusal(threadElsewhereLine(threadId))`; `head` = the first
    row's prompt; `opening` = the thread's first `session.start` whose prompt starts with the head, read only when the
    head was cut (`openingCut`), else the head; `named` = the newest row's harness, else `defaultAgentOf(prefs,
    entry)`; no adapter or the agent off: `noAgent`; the adapter without `draftFor`: `noAgent`; `git.diff {cwd, scope:
    "head", paths?}`; `model` = `smallestModel(catalogOn(..))` where a table exists; `promptFile` =
    `/tmp/wsp-draft-<12 hex>.txt`; `putFiles(machine, [{path, text: draftPrompt(cutDiff(patches joined), opening)}],
    {before: ["umask 077"]})`; a non-zero exit: `noAnswer`; `draftFor({promptFile, model?}, harnessExec(machine,
    TITLE_MAKE_TIMEOUT_MS))`; `commitMessage(answer)`; none: `noAnswer`; else `{message}`; once the file is put (the `finally` opens after the put's check), `rm -f <file>` on the
    machine, failure ignored.
12. `viewed({workspaceId, path, blob}, origin)` (:881-891): marks copied; no path: `{viewed}`; a string blob sets,
    anything else deletes; stored in `viewedMarks`; `persistSessions(id)`; emit `{type: "workspace.viewed",
    workspaceId, viewed}`; `{viewed}`.
13. Pull request page (:893-935): `view`: `pullRequestOn`; `readPage(entry, remote, n, fresh)`; `mergeSettings` (failure
    ignored); `{...page, merge?, sent: prSent ?? [], postsAsYou: here}`. `diff`: `readHost(.., git.prDiff {cwd, remote,
    number, maxBytes: GIT_DIFF_CAP_BYTES})`. `reply`: a blank body (trim): kind invalid `REPLY_EMPTY_LINE`; then
    `postAsPerson(remote, n, git.prReply {cwd, remote, number, replyTo?, threadId?, body})`. `resolve`, `react`: through
    `postAsPerson`. `send`: a thread named with no row: `notFoundRefusal(threadElsewhereLine)`; `copyBlocked`; the page
    (not fresh); items unique by kind and id, first kept; `pullRequestSendPrompt(page, asked, n)`; to the first thread
    or `sendDetached` to the named one; `prSent` = earlier entries not re-sent, then each asked `{kind, id, at}`;
    persist; `{outcome, threadId, agent, capped?, sent}`.
14. `fix({workspaceId, check, child, threadId, childThreadId}, origin)` (:937-979): `spawnGuard("fix")`; both check and
    child: kind invalid `FIX_CHECK_OR_CHILD`; `entryOf(.., {act: "fix", thread?})`; a thread with no row: not found;
    `copyBlocked`. A child: `childOf`; `leadBranch` = the lead's checkout branch, record base, project base, default
    branch; `childBranch` = the child's checkout branch, worktree branch, `""`; `remote` = the child's checkout where
    the project's remote has no host, else `origin`; `mergeChildPrompt`. Neither: `updateCopy(entry)`; merged:
    `{outcome: "updated", base}`; else `conflictsPrompt({base, branch: checkout branch ?? pr branch ?? base, files})`.
    A check: `readPullRequest(entry, true, true)`; no fact: no kind `noPullRequestRefusal(label)`; no such check:
    `noSuchCheckRefusal(check, names)`; not failed: `checkNotFailedRefusal(check, state)`; the run's log through
    `readHost(git.runLog)`, none on failure; `checkFailedPrompt`. Sent as in 13; `{outcome, threadId, check?, child?,
    base, agent, capped?}`.
15. `merge({workspaceId, method, whenChecksPass, head, threadId}, origin)` (:981-1006): `spawnGuard("merge")`; the held
    fact, read only where none is held (`readPullRequest(entry, false)`); none: `notOpenRefusal(number, state)` for a
    kept pr not open, else `noPullRequestRefusal`; not open: `notOpenRefusal`; `mergeSettings(remote)`; `by` = method
    or the default; not allowed: `mergeMethodRefusal(by, methods)`; auto asked and off: `AUTO_MERGE_OFF_LINE`;
    `git.prMerge {cwd: home, remote, number, method, auto, headOid: head ?? fact.headOid}` on this computer; drop the
    page cache; `readPullRequest(entry, true)`; `{number, method, merged, autoArmed}`.
16. `update({workspaceId, threadId}, origin)`: `spawnGuard("update")`; `entryOf(.., {act: "update", thread?})`;
    `copyBlocked`; `updateCopy(entry)`.
17. `merge_in({workspaceId, child, threadId, childThreadId}, origin)` (:1015-1053): `spawnGuard("merge_in")`; `lead`;
    a thread on another workspace: no kind `mergeIntoOwnRefusal(threadId)`; `childOf`; running turns on the lead other
    than the asking thread's: no kind `leadBusyRefusal(label, thread ids)`; `copyBlocked(lead)`; `branch` = the
    sub-thread's branch, else the child's checkout (fresh), else its worktree branch, else `""`; empty or detached:
    `childOnNoBranchRefusal(label)`; a remote with no host: both local, `from` = the child's checkout, else
    `noRemoteForTreeLine(project)`; `git.mergeIn {cwd, branch, from?}` in `queued(lead)`; `kept` = the child's tree:
    merged drops `conflicts` and, with commits and an oid, sets `merged: {oid, at, head?}`; not merged sets
    `conflicts`; `readCheckout(lead, true)`; `keepTree(child, kept)`; `{lead, child, branch, merged, commits,
    conflicts, unpushed?}` (`unpushed` from the sub-thread's result where above 0).

## Behaviour nobody wrote down

- Trap #1383: every door that acts in a copy calls `copyBlocked` first (exec, exec_stream, bring_back, discard, commit,
  commit_draft, send, fix, update, merge_in); checkout, view and merge do not.
- Invariant 4: pull request posts go through `postAsPerson`; merge sends the held head.
- Rule 3.9: host git writes go through `queued(id)`.

## After the port

- `commitDraft` writes the prompt to a fixed `/tmp` path pattern on the machine (:865); kept.

## Rules against tests

| Rule | Tests |
|---|---|
| 1, 2 | `exec_mounts_the_worktree_after_the_copy_block`, `exec_stream_ends_the_reader_with_the_reason_and_unregisters` |
| 3 | `bring_back_pushes_and_opens_a_pr_in_the_queue`, `bring_back_of_a_lead_threads_child_opens_nothing`, `bring_back_refuses_an_unpushable_fork`, `bring_back_closes_the_issue_it_came_from`, `bring_back_on_the_default_branch_says_so`, `bring_back_keeps_a_push_refused_on_the_childs_tree`, `bring_back_answers_a_pr_refusal_beside_the_push` |
| 4-7 | `folder_refuses_off_this_computer`, `worktree_answers_path_branch_made`, `worktree_remove_refuses_one_wsp_did_not_make_and_a_threads_force` |
| 8, 9 | `checkout_reads_the_pr_and_tree_only_when_not_fresh`, `discard_check_refuses_a_path_with_no_change`, `discard_queues_and_rereads` |
| 10, 11 | `commit_names_every_changed_file_and_refuses_a_clean_checkout`, `commit_draft_with_no_paths_asks_nothing`, `commit_draft_drafts_from_the_whole_opening_and_removes_the_file`, `commit_draft_says_no_agent_and_no_answer` |
| 12 | `viewed_sets_and_clears_a_mark_and_emits` |
| 13 | `pull_request_view_carries_sent_and_posts_as_you`, `pull_request_reply_refuses_a_blank_body`, `pull_request_send_keeps_one_of_each_item_and_writes_sent` |
| 14 | `fix_refuses_check_and_child_together`, `fix_a_failed_check_sends_its_log`, `fix_conflicts_sends_the_files_or_says_updated`, `fix_a_child_sends_the_merge_prompt` |
| 15 | `invariant_4_merge_sends_the_held_head_not_a_fresh_read`, `merge_refuses_a_closed_pr_a_method_and_auto_off` |
| 16, 17 | `update_reads_through_copy_blocked`, `merge_in_refuses_while_the_lead_runs_a_turn_but_not_the_asking_threads_own`, `merge_in_from_a_child_folder_with_no_remote`, `merge_in_keeps_merged_or_conflicts_on_the_childs_tree` |
