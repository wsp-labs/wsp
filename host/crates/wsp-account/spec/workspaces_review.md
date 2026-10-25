# workspaces_review

Source: `packages/runtime/src/projects/workspaces.ts:1055-1247` at 65f6b89c7. Tests: `start-review`, `runtime-reach`,
`events-scope`, `local-workspace` cases of `packages/runtime/test/` (named per test), ported at the area's seam with a
fake `WorkspacesCtx`.

## What it does

The start and review doors built on start_from, the review draft and its post, the daemon and guest channels, port
reach and probe, and the synchronous filters the event stream runs per socket.

## Behaviour

1. `start({url, project, agent, model, effort, access}, origin)` (:1055-1067): `spawnGuard("start")`; `ready()`;
   `githubLinkOf(url)` none: kind invalid `START_WORDS.notALink(url)`; `projectByRepo(repo, project)`; `issueOf`; a
   pull request link: `pullRequestOf`; `fromOf`; `picksHold(project, {harness?, model?, effort?, access?})`;
   `workspaceFrom(.., "start", origin)`; `openWith(placed, {prompt: fromTaskPrompt(from, read), harness?, model?,
   effort?, access?}, origin)`.
2. `review({url, workspaceId, agent, model, effort}, origin)` (:1069-1102): `spawnGuard("review")`; `ready()`. A url:
   not a link: invalid `notALink`; not a pull request: invalid `START_WORDS.notAPullRequest`; `projectByRepo(repo,
   None)`. Else the workspace's kept `pr`, else its `from` unless an issue; none: invalid `notAPullRequest`; `repo` =
   `ownerRepoOf(remote) ?? remote`. `reviewer` = agent or `codex`; no read-only mode: invalid `START_WORDS.noReadOnly(
   reviewer, reviewers())`; `pullRequestOf`; `issueOf`; `git.prDiff` at home; `fromOf("review", ..)`; `picksHold`
   with `permissionMode: readOnly`; `workspaceFrom(.., "review")`; `openWith` with `reviewTaskPrompt(from, read,
   diff)`; the record's `reviewThread` = the opened thread; persist; the opened result.
3. `review_draft({workspaceId, summary, verdict, on}, origin)` (:1104-1119): a draft that reads and any of the three
   given: summary and verdict where given, each comment's `on` from the ticks by id; persist; emit
   `workspace.review`. Answers `{review}` where the record has one, else `{}`.
4. `review_post({workspaceId, threadId}, origin)` (:1121-1140): `spawnGuard("review_post")`; no draft that reads or no
   `from`: no kind `START_WORDS.noReviewYet(label)`; `ticked` = comments `on`, as `{id, path, line, side, body}`;
   `git.prReview {cwd: home, remote, number, headOid: draft.headOid, event: verdict, body: summary, comments}` on this
   computer; review `posted: {url, at, folded}`; persist; emit; drop the page cache; `readPullRequest(entry, true)` not
   awaited; `{url, number, comments: ticked - folded, folded: folded count}`.
5. `daemon_reach(id, origin)`: the kind's `daemonRoad(entry)`. `token_refused(id)`: forget `daemonTokens[machine id]`
   (no origin; `entryOf(id)`).
6. `daemon_channel(id, onEvent, origin)` (:1151-1162): not a shared daemon: `threadTagged(id, copyChannel(entry,
   onEvent, WORKSPACE_FRAMES))`. Shared: the workspace's pty ids (`portPids`, made once); events pass through, a
   `pty.exit` whose pty id was held removes it and calls `portRootsMoved(id)`; the channel wrapped
   `killGuarded(scopedProcs(scopesHere, rootedPorts(id, checkout, ptys, heldToOwner(id, threadTagged(id,
   copyChannel(.., heard, WORKSPACE_FRAMES))))), isLocalWorkspace(record))`.
7. `guest_channel(id, onEvent)`: `copyChannel(entryOf(id), onEvent, GUEST_ROAD_FRAMES)`.
8. `served_by_its_computer(id, origin)`: `servedByItsComputer(entry)` is some.
9. `watch_sys(id, fn, origin)` (:1172-1178): a kind whose metrics road is not `host`: no kind `<machineWord(kind)>
   reads its own load over its daemon link, not from this host`; no `local.sysSamples`: no kind `this host reads
   nothing of the computer it runs on`; else the samples' detach.
10. `port_reach(id, port, origin)` (:1180-1186): `copyBlocked`; the kind's own `portReach(entry, port)` or
    `ws.portReach(port)`; `{url, expiresAt}`.
11. `port_probe(id, port, origin)` (:1188-1204): `copyBlocked`; the kind's own reach with `{standing: true}`: fetch with
    no redirect and `PORT_PROBE_TIMEOUT_MS`; `{status, body: readBodyUpTo(res, PORT_PROBE_BODY_CAP)}`. Else
    `ws.portReach(port)`, the same fetch; a 401 then `ws.remintPortReach(port)`; `{status, body}`.
12. `origin_refusal(id, origin)`: `ready()`, `refusalFor(live record, origin)`.
13. `creating()` (:1211-1215): every create stage whose live record has no `lead` and that is not a shown create.
14. `seen_by(event, origin)` (:1217-1231), synchronous: the person: true. `askerOf(event)` some: its thread is the
    scope's, or its root thread is the scope's root. Else the event's workspace id; no live record: false; a string
    `threadId` on the event: `reachesRow({threadId, workspaceId}, origin)`; else `refusalFor(record, origin)` is none.
15. `shown_to(event, origin)` (:1233-1244): only a `workspace.status` with a `status.tree`, for a thread; children kept
    whose live record has no refusal for the caller; all kept: the event as given; none kept: `status` without `tree`;
    else `tree` with the kept children; the other keys in place.

## Behaviour nobody wrote down

- Invariant 4: review posts run on this computer as the person, pinned to the draft's head.
- Rule 3.4: `seen_by` and `shown_to` never await.

## After the port

- `review` asks `issueOf` and `prDiff` after `pullRequestOf` even though the pull request read holds the same facts
  (:1092-1094). Kept.

## Rules against tests

| Rule | Tests |
|---|---|
| 1 | `start_refuses_not_a_link`, `start_reads_checks_picks_then_places_and_opens`, `start_of_an_issue_reads_no_pr` |
| 2 | `review_by_url_refuses_an_issue_link`, `review_of_a_workspace_reads_its_kept_pr`, `review_refuses_an_agent_with_no_read_only_mode`, `review_records_the_review_thread` |
| 3, 4 | `review_draft_moves_ticks_and_emits_only_when_something_given`, `invariant_4_review_post_runs_on_this_computer_pinned_to_the_drafts_head`, `review_post_refuses_with_no_draft` |
| 5-9 | `token_refused_forgets_the_machines_token`, `daemon_channel_of_a_shared_daemon_tracks_ptys`, `watch_sys_refuses_a_kind_reading_its_own_load` |
| 10, 11 | `port_reach_prefers_the_kinds_own_road`, `port_probe_remints_after_a_401` |
| 12, 13 | `creating_leaves_out_sub_threads_and_shown_forks` |
| 14, 15 | `seen_by_shows_a_thread_its_tree_only`, `seen_by_reads_an_events_thread_row`, `shown_to_prunes_children_the_thread_may_not_see` |
