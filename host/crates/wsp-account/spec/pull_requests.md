# pull_requests: the git host read for a workspace (`account/pull-requests.ts`, 300 lines)

The one home of "a git host read for a workspace" (`readHost()`), and the pull request kept on a workspace: read,
cached, polled, dropped when a folder leaves its branch, and settled into the tree once merged or closed. Holds
runtime invariant 4. TS tests: `test/pull-request.test.ts` (35), `test/settle-tree.test.ts`, `test/box-settle.test.ts`,
`test/bring-back.test.ts` (update). Case list in `tests/pull_requests.rs`' header.

`PullRequestsCtx`: `now()`, `schedule(ms, f) -> Cancel`, the GitHub cache (`get`, `set`, `delete` by key: `{ body,
tag?, fetchedAt }`), `live(id)`, `sessions()` (views), `on_this_computer(work).await`, `with_daemon(entry, work).await`,
`checkout_of(record)`, `entry_of(id, origin).await`, `project_held(id)`, `status_watched()`, `persist(record).await`,
`status_now(entry).await`, `read_checkout(entry, force).await`, `slates_watch_pr(id)`, `mark(threads, { readAt,
settledAt }).await`, `pauses(record)`, `nap_with(id).await`, `read_lead_of(child)`, `queued(id, work).await`,
`spawn(fut)`, `warn(line)`.

## Rules

- Q1 `readHostOn` (25-32): this computer first (`onThisComputer`, the frame built for its home), answering `here: true`.
  Only a `no-host-cli` refusal falls through, and only for a copy that reads the host itself (not local, phase
  `running`): then the copy's own daemon with the frame built for its checkout, `here: false`. Any other failure, or a
  stopped or local copy, rethrows. `readHost` answers the read alone. Test `q1_read_host_falls_back_to_a_running_copy`.
- Q2 `heldOff(road, read)` (37-47): a road (`"here"` or the workspace id) that met `DaemonRefusal` code
  `rate-limited` answers every read for `RATE_LIMIT_HOLD_MS` with that same refusal and runs nothing; the hold ends
  at `now >= until`. Other refusals hold nothing. Tests `q2_rate_limit_holds_the_road`, `q2_the_hold_ends_on_time`.
- Q3 `pullRequestOn(id, origin)` (50-55): the entry by `entryOf`; no `record.pr.number` → plain
  `noPullRequestRefusal(name)`; else `{ entry, remote: project.remote, number }`. `pageKey` is `page:<remote>#<n>`,
  `prKey` is `pr:<remote>#<n>`.
- Q4 `readPage(entry, remote, n, fresh)` (61-70): unless fresh, a cached page younger than `PR_PAGE_HOLD_MS` answers
  as held. Else `git.prView` `{ op, cwd, remote, number }` through Q1, cached `{ body: { page, here }, fetchedAt }`,
  and a timer of `PR_PAGE_HOLD_MS` deletes it only if the entry is still the one it set. Test `q4_page_held_then_dropped`.
- Q5 **Invariant 4.** `postAsPerson(remote, n, frame, parse)` (74-82) deletes the held page first, then asks only this
  computer; a `no-host-cli` refusal becomes plain `pullRequestPostLine(hostOfRemote(remote))` and the copy is never
  asked. `hostOfRemote` (85): the git host's first ssh host, else the remote's host, else the remote. Tests
  `invariant_4_post_as_person_never_takes_the_copy_road`, `invariant_4_post_as_person_drops_the_held_page`.
- Q6 `prPollMs` (88): `PR_POLL_MS` while a window watches, `PR_POLL_IDLE_MS` otherwise.
- Q7 `readPullRequest(entry, force, whole = false)` (99-152), in order:
  1. a folder that left its branch (Q9) drops the pull request (Q10) and reads again;
  2. a kept record pr not `open` answers the entry's pr, no read;
  3. unforced, a held read younger than its hold answers held: the hold is `prPollMs()` for a fact, `CHECKOUT_TTL_MS`
     for a `why`; a fact whose armed timer's interval is no longer the one in force re-arms from its `readAt`;
  4. unforced, no kept pr and `prNoneAt` equal to the checkout's head: answers held, no read;
  5. a read in flight is joined unless `whole`;
  6. the read: a project with remote `""` answers the entry's pr; with no kept pr the branch comes off the checkout
     (read when not held); no branch, the base (`record.base ?? project.base`) or a `(`-branch → answers the entry's pr;
     `seen` is the cached tag for the kept number, unless `whole`, the cached body does not parse, or a check is pending;
     the frame `{ op: "git.prRead", cwd, remote, number | branch, seen? }`;
  7. `unchanged` with a parsed cached body → take the cached body with `readAt: at`; else cache a pr that came with
     `seen` (`{ body: pr, tag: seen, fetchedAt: at }`); `prNoneAt` = the checkout head when no pr and none kept, else
     cleared; take the pr (or nothing);
  8. a `no-host-cli` failure takes `{ why, readAt: at }`: `pullRequestStoppedLine(host, name)` for a copy not local and
     not running, else `pullRequestUnreadLine(host)`; any other failure keeps the last fact and re-arms the poll;
  9. the in-flight mark clears however it ends.
  Tests `q7_read_pull_request_table` (one test per step, 9), `q7_a_read_in_flight_is_joined`, `q7_whole_reads_alone`.
- Q8 `takePullRequest(entry, seen)` (203-220): the entry's pr becomes `seen`. A fact whose state, number or branch
  differs from the record's writes the record's pr `{ number, url, state, base, branch, mergedAt|closedAt?, checks? }`
  (key order as written; `mergedAt`/`closedAt` = `readAt`; `checks` kept only once settled) and persists; a new number
  drops `prSent`. Then the status is pushed, the poll re-armed, and a settled record pr settles the tree (Q12). Test
  `q8_take_writes_the_record_only_when_it_moved`, `q8_a_new_number_drops_what_was_sent`.
- Q9 `leftBranch` (158-163): only a shared folder (runs in folder, no worktree) with a kept pr and a checkout on a real
  branch: the kept pr's branch differs, or (for a pr kept before branches were) the checkout is on the base
  (`record.base ?? project.base ?? project.defaultBranch`). Test `q9_left_branch_table`.
- Q10 `dropPullRequest` (171-180): cancels the poll, clears the entry's pr, `prNoneAt`, the record's `pr` and `prSent`,
  persists, pushes the status.
- Q11 `ranOnBranch(entry, thread)` (185-197): only a shared folder on a real branch. On its base: removes
  `branchThreads` (persisting only when there was one). Else adds the thread to `branchThreads` for that branch
  (starting a new list when the branch changed), persisting only when it moved. Test `q11_ran_on_branch_table`.
- Q12 `pollPullRequest(entry, from = now)` (224-241): cancels any armed poll; arms one only for an open fact on an entry
  still live under its id. The interval is `SLATE_PR_POLL_MS` when a check is pending and a slate watches the
  workspace's pr, else Q6; `prPollMs` is recorded; the timer fires at `max(0, from + ms - now)` and forces a read.
  Test `q12_poll_intervals`.
- Q13 `settleTree(entry)` (256-273): nothing for a workspace a thread opened (`parentThreadId`). The roots are the
  workspace's session rows with no `rootThreadId`, keyed `threadKeyOf`; in a shared folder only the threads listed in
  `branchThreads` for the pr's branch. No roots → nothing. The tree is the roots and every row whose `rootThreadId` is a
  root; any `running` → nothing (its turn's end settles it). Else `mark(unique keys, { readAt: now, settledAt: now })`
  (a failure warns `"the tree of <name> was not settled: <message>"`), and a pausing kind still `running` is napped (a
  failure warns `"<name> was not napped once its pull request settled: <message>"`). Nothing is deleted. Tests
  `q13_settle_marks_the_tree_and_naps`, `q13_a_running_turn_waits`, `q13_a_folder_settles_only_its_branch_threads`.
- Q14 `mergeSettings(remote)` (277-285): cached per remote for `REPO_SETTINGS_MS` = 3,600,000 ms; read through
  `heldOff("here")` on this computer, `{ op: "git.repoRead", cwd: home, remote }`. Test `q14_merge_settings_held_an_hour`.
- Q15 `updateCopy(entry)` (289-295): `git.update` at the checkout with `base` (record, then project) through the
  workspace's queue and its daemon; then the checkout read (forced, awaited) and the pr read (forced, not awaited).
  Test `q15_update_copy_reads_after`.
- Q16 `keepTree(child, kept)` (244-249): an empty object removes `tree`, else sets it; persists; reads the lead's rows.

## What survives a restart

The record's `pr`, `prSent`, `branchThreads` and `tree` (persisted). The rate-limit holds, the repository settings,
the page and pr cache entries in memory, the poll timers and `prNoneAt` do not: a restarted host reads afresh.
