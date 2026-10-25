# start_from

Source: `packages/runtime/src/projects/start-from.ts` (262 lines) at 65f6b89c7. Tests: the start and review cases of
`packages/runtime/test/` that drive `workspaces.start` and `workspaces.review` through a whole host (named per test in
the table below); ported at the area's seam with a fake `StartFromCtx`.

## What it does

The pieces `wsp start <url>` and `wsp review` are made of (the doors themselves are workspaces_review's): which project
a GitHub repository names, the issue and pull request read on this computer, where the work came from, the folder or
workspace the thread runs in (a pull request's head fetched and caught up where it may be), the picks checked before
anything is made, the thread opened detached, and a reviewer's reply read into a review draft.

## Behaviour

1. `project_by_repo(repo, named)` (:20-25): over `projectsHeld` in order; with no name, `projectForRepo(held, repo,
   HERE_PLACE_ID)`; with one, the first project holding the repo whose id or name is `named`. None: kind invalid,
   `START_WORDS.noProjectForRepo(repo)`.
2. `issue_of(remote, n)` asks `git.issueRead {cwd: home, remote, number}` on this computer and answers `.issue` (:28-29).
   `pull_request_of` asks `git.prRead`; a reply with no `pr`: no kind, `#<n> is not a pull request` (:32-36).
3. `from_of(kind, repo, read, fact)` (:39-48): `{kind, repo, number, url, title}` from the read, then with a fact
   `base` and `head: {branch, oid: headOid, fork?}` in that key order; `fork` only where the fact has one.
4. `workspace_from(project, from, fact, kind, origin)` (:53-91):
   - `kindOf(project.computer)` copies its folder: `folder_from` (rule 6).
   - runs in its folder (a joined box): with a fact, usage `placeBranchLine(placeName(computer))`; else the project
     folder's record (`projectFolder`).
   - else (a fork): `name` = `takenNameAfter(startName(kind, number, title), names of every live record)`;
     `landingDeps(computer)`; `brokered` = `brokeredStart(landingKind(computer, at) == "provider")`; `head` =
     `pullRequestOfRecord(remote, fact.number, project.name, placeName(placeId ?? places.wired), kind == "start")`
     only with a fact and brokered; `as` = `{threadId, turnId}` from two UUIDs (thread first) only when brokered.
   - `workspaces.create({project: id, name, branchOfRecord?: head, opensThread?: as.threadId}, origin)`; the entry
     not live after: no kind, `<name> was made and then not found`.
   - On the entry's record: `from`; with a fact `base`, `pr: {number, url, state, base}`, and the entry's `pr` =
     `{...fact, readAt: clock.now()}`; then `persist`.
   - With a fact and no brokered head: `git.prCheckout {cwd: checkoutOf(record), number}` through the entry's daemon;
     a refusal deletes the workspace (`workspaces.delete(id, origin)`; a failed delete only warns `<name> was not taken
     back after its checkout failed: <why>`) and the refusal propagates.
   - Answers `{entry, as?}`.
5. `folder_from(project, from, fact)` (:96-116): no fact: the project folder. No `git.top`: usage
   `noBranchesLine(name)`. `branch` = `fact.branch`, or `<fork.owner>/<branch>` for a fork. `holds` = a worktree of
   `top` already on `branch` (`worktreesOf`); not held: `fetch_head` first. `worktreeFolder(project, top, branch)`;
   `behind` = `catch_up` only where it was held; `setup` = `takeSetupLine(entry id)`. A record with no worktree (the
   project folder holds the branch) answers `{entry, behind?, setup?}` with nothing written; else the record takes
   `from`, `base`, `pr`, the entry's `pr` with `readAt`, and is persisted.
6. `fetch_head` (:151-163): no fork: write the daemon roots of the project folder, then `git.fetchBranch {cwd:
   project.path, remote, branch: fact.branch, into: branch}`; on a refusal, when the remote still holds
   `refs/heads/<fact.branch>` the refusal propagates, else fall through. Then (a fork, or a branch gone at the remote)
   `git fetch --quiet --no-tags <remote> pull/<n>/head:refs/heads/<branch>` in `top` with `CLONE_MS`; a non-zero exit:
   no kind, `gitSaid(result)`.
7. `remote_holds(top, remote, ref)`: `git ls-remote --exit-code <remote> <ref>` in `top`, `CLONE_MS`; only exit 2 is no.
8. `catch_up(entry, project, top, fact)` (:170-190), answering a line or nothing, in order: `path` = the record's
   worktree path, else `top`; `byName` = no fork and the remote holds the branch; fetch `refs/heads/<branch>` (by name)
   or `pull/<n>/head` into FETCH_HEAD (`CLONE_MS`); a failed fetch: `PR_BEHIND_WORDS.unread(n, gitSaid)`; `head` =
   `rev-parse FETCH_HEAD` in top, `at` = `rev-parse HEAD` in path (both trimmed); equal, or head an ancestor of at:
   nothing; at not an ancestor of head: `diverged(n, path)`; `status --porcelain` not blank: `changed(n, path)`; no
   worktree record: `folder(n, path)`; worktree not made by wsp: `notMade(n, path)`; not by name: `byHead(n, path,
   remote)`; else `git.update {cwd, base: fact.branch}` through the entry's daemon after its roots are written: nothing,
   or on a refusal `unread(n, its message)`.
9. `picks_hold(project, o)` (:120-140): nothing for a computer that does not copy its folder (`kindForComputer`).
   Else the project folder; `prefs` = `state.preferencesHeld` or `preferences.get()`; `harness` = `o.harness` or
   `defaultAgentOf(prefs, home)`; no catalog table for it: nothing. `launchAdapterFor(home, harness)`; `resolved` =
   `defaultsOn(catalogOn(table, home, adapter), prefs, prefs.projectDefaults[project id])`; `o.access` set and
   `accessWordRefusal` answers a refusal: it is thrown; `named` = `o.permissionMode`, else `namedMode(catalog,
   harness, access)` where access is set; `model`, `effort` from `o` else `resolved.open`; `startPicks(catalog,
   {model?, effort?, permissionMode?, fast?: true}, true)`; a throw becomes `notRunRefused(catalog, model)` or else
   `pickRefusal(e, catalog)`.
10. `open_with(placed, o, origin)` (:193-211): `sessionsApi.start(entry id, {behind?, setup?, as?, prompt, harness?,
    model?, effort?, permissionMode?, access?}, origin)` in that key order; answers `{workspace: view(record),
    threadId: view.threadId ?? view.id, sessionId: view.id}`.
11. `read_only_of(agent)`: the catalog's `readOnlyMode` for that harness. `reviewers()`: every harness with one, Codex
    first, the rest in catalog order (:214-219).
12. `take_review(entry, threadId, text)` (:225-257): nothing unless `record.from.kind == "review"`; nothing when
    `reviewThread` is set and is not `threadId`. `read` = `reviewFromReply(text)`; `at` = clock.
    - Not read: a prior draft that reads (`isReviewRead`) is left as it was; else `reasked` = the prior is a note with
      `reasked: true`; the record's review = `{note: read.why, at, reasked: true}`; persist; emit
      `{type: "workspace.review", workspaceId}`; and only when not reasked before, `sendDetached(id, {prompt:
      reviewReaskPrompt(why), thread: threadId}, None)` not awaited, its failure warned `the reviewer of <name> was not
      asked again: <why>`.
    - Read: the diff via `git.prDiff {cwd: home, remote, number: from.number}` on this computer, `None` on any
      failure; each comment n (from 1) is `{id: "c<n>", ...comment, on: true, inSummary: true?}`, `inSummary` only
      where a diff was read and `lineInDiff` answers false; `headOid` = the entry's held pull request fact's
      `headOid`, else `from.head.oid`, else `""`; review = `{verdict, summary, comments, headOid, threadId, at}`;
      persist; emit `workspace.review`.

## Edge cases

- Two live records named alike: `takenNameAfter` appends; names come from every live record, gone ones included.
- `catch_up` on the project folder itself (no worktree record) still fetches and compares, then answers `folder`.
- A non-brokered start makes no UUIDs: `as` is absent from both the create and the session start.

## Behaviour nobody wrote down

- Rule 3.4: `entry.record` is written after `create` returns and before `persist` with no await between the writes.
- Rule 3.16: the two UUIDs are drawn thread id first.

## Decisions

1. `take_review`'s detached send is spawned on the host thread (`spawn_local`); the area does not wait on it.

## After the port

- `takeReview` reads `entry.pr` for the head, which may be a newer fact than the one the review was asked against
  (:253). Kept.

## Rules against tests

| Rule | Tests |
|---|---|
| 1 | `project_by_repo_prefers_this_computers_project`, `project_by_repo_with_a_name_takes_that_project_only`, `project_by_repo_with_no_project_is_invalid` |
| 2 | `pull_request_of_refuses_an_issue_number`, `issue_of_asks_git_issue_read_at_home` |
| 3 | `from_of_writes_its_keys_in_the_ts_order_with_and_without_a_fork` |
| 4 | `workspace_from_on_a_provider_brokered_picks_the_thread_first_and_names_the_branch_of_record`, `workspace_from_puts_from_and_pr_on_the_record_before_persist`, `a_refused_checkout_takes_the_workspace_back_and_answers_its_own_refusal`, `a_refused_checkout_whose_delete_fails_warns_and_still_refuses`, `a_joined_box_refuses_a_pull_request_and_runs_an_issue_in_the_folder`, `a_made_workspace_not_found_after_is_refused` |
| 5, 6, 7 | `folder_from_fetches_a_branch_no_worktree_holds_then_records_the_pr_on_the_worktree`, `folder_from_fetches_a_forks_head_by_pull_ref_into_owner_slash_branch`, `fetch_head_falls_back_to_the_pull_ref_when_the_branch_is_gone_at_the_remote`, `fetch_head_keeps_the_daemon_refusal_when_the_remote_holds_the_branch`, `folder_from_on_the_project_folders_own_branch_writes_nothing`, `folder_from_with_no_git_top_refuses_no_branches` |
| 8 | `catch_up_answers_each_line_in_order` (one row per line: unread, nothing, diverged, changed, folder, not made, by head, updated, update refused) |
| 9 | `picks_hold_refuses_an_access_word_the_agent_lacks`, `picks_hold_turns_a_start_picks_throw_into_the_pick_refusal`, `picks_hold_asks_nothing_off_a_computer_that_does_not_copy` |
| 10 | `open_with_starts_the_session_with_its_keys_in_order_and_answers_the_thread` |
| 11 | `reviewers_put_codex_first` |
| 12 | `take_review_reads_the_draft_marks_lines_outside_the_diff_and_emits`, `take_review_asks_once_more_then_keeps_the_note`, `take_review_leaves_a_read_draft_on_a_reply_with_no_block`, `take_review_ignores_another_thread_and_a_start` |
