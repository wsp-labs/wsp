# forks: a thread carried on from one of its turns

TS: `T/forks.ts` (240 lines) at `65f6b89c7`. Rust: `src/threads/forks.rs`, free functions over `&Rc<dyn ForksCtx>`
as the TS's are over `ctx`. No state of its own. Refusals carry the protocol's words and kinds; `conflict(h, f)` is
`refusal(h, f, conflict)`.

## Rules

F1. `fork_opened(ctx, ws, opened, origin)` (`:42-63`): a start with no `fork` answers itself and no plan. A fork beside
a `thread` or a `replaces` refuses `usageRefusal(FORK_BESIDE_LINE, FORK_BESIDE_FIX)`; beside an `outside`
conversation, `usageRefusal(FORK_RESUME_LINE, FORK_RESUME_FIX)`; both before the source is read. Then `read_fork`. A
named harness other than the source's refuses `forkAgentLine(<source label>, <named label>)`,
`forkAgentFix(<named label>)`, `usage`. The start runs on the source's harness, and takes the source's `model`,
`effort`, `contextWindow` where it names none, `fast` only when the source ran fast and it names none,
`permissionMode` where it names neither a mode nor an `access`, and the source's `cwd` where it names none and forks
on the source's own workspace. A field the start names is never replaced; a field it names keeps its key place, and
an added one goes after its keys.

F2. `read_fork(ctx, fork, origin)` (`:67-140`): awaits `ctx.ready()`. The workspace is the thread's latest row's, else
its record's; none, or `ctx.entry_of_row` refusing the caller, refuses `notFoundRefusal("no thread <threadWord>")`.
The harness is the record's, else the latest row's. The events are `ctx.open_thread(ws, thread)` less
`session.slate`; `earlier` are its `session.earlier` events; the turns are the events with a `turnId`, in first-seen
order. A turn is finished when it has a `session.end` or `session.done` and is neither the running turn
(`running_on`) nor the launching one (`launching_on`).

F3. The turn picked: a named `turnId` the thread never ran refuses `forkNoTurnLine(turnId)`, `FORK_NO_TURN_FIX`,
`not-found`; one not finished refuses `conflict(FORK_RUNNING_LINE, FORK_RUNNING_FIX)`. `at: 0` picks none. `at`
(default the count of finished turns) outside 1 to that count refuses `forkTurnsLine(n)`, `forkTurnsFix(n)`, `usage`;
a thread with no finished turn and no `at` so refuses with `n = 0`. Else the `at`-th finished turn.

F4. A picked turn of a sub-thread whose machine was deleted (`subTurnGone(record.sub, <the turn's first event with an
at>.at)`) refuses `SUB_TURN_GONE_LINE`, kind `conflict`.

F5. The plan: `source {threadId, workspaceId, entry, harness, title, session?, cwd?}` (title `foldThreads(rows)[0].title`,
else the record's `forkedFrom.title`, else the thread word; session the latest row's agent session, else
`ctx.started_as(ws, thread)`; cwd the latest row's, else `ctx.folder_of(ws, session)`); `turn {turnId, anchor?, ref?,
after}` with the picked turn's last checkpoint's `anchor` and `ref` and, for each later turn, `{anchor?, result?}`
off its last checkpoint and its last done; `events`: the `earlier` events (only when a turn is picked), then every
event of the turns up to and including the picked one, in transcript order; `picks {model?, effort?, contextWindow?,
fast? (only true), permissionMode?}` off the latest row (permission mode the record's first); `forkedFrom {threadId,
turnId?, title}`.

F6. `fork_turn_of(plan, by_count)` (`:143-149`): no turn answers `None`; a turn with an anchor answers it; else a
harness that finds no turn by count refuses `conflict(forkNoAnchorLine(<label>), FORK_NO_ANCHOR_FIX)`; else `{after}`.

F7. `copied_events(plan, to)` (`:153-158`): each event less `pos`, its `workspaceId` and `threadId` the fork's (in
place), and `copiedFrom` its own where it was already a copy, else the source thread (added last).

F8. `copy_images(ctx, plan, to)` (`:162-178`): after `ctx.kept_writes(source ws)` settles, the source thread's entries
in `attachment-keys`; none copies nothing. For each start or steer event carrying an image, every attachment keeps its
index: an image whose key the source holds and whose blob reads is `{name: name or "", mediaType, bytes: base64}`; any
other is `{name: name or "", mediaType: "text/plain", bytes: ""}`; then `ctx.keep_sent_images(to ws, to thread,
requestId, images)`, awaited, event by event.

F9. `ready_fork(adapter, session, turn, said)` (`:181-187`): the adapter's `fork_session`; its failure becomes
`said(<message>)`.

F10. `move_session(ctx, thread, from, to)` (`:191-193`): every row of the thread on agent session `from` names `to`.

F11. `fork_start(ctx, start, o, origin)` (`:199-226`): reads the fork; refuses a harness other than the source's as
F1. With no branch: `start(source ws, o)`. With one: a kind that copies no folder refuses
`usageRefusal(placeBranchLine(<computer name>), FORK_NO_CHECKPOINT_FIX)`; no git top refuses
`usageRefusal(noBranchesLine(project name), FORK_NO_CHECKPOINT_FIX)`; no checkpoint ref, or `git rev-parse --verify
-q <ref>^{commit}` at the top exiting non-zero, refuses `FORK_NO_CHECKPOINT_LINE`, `FORK_NO_CHECKPOINT_FIX`,
`conflict`. Else the worktree `ctx.worktree_folder(project, top, branch, source record id, caller's root thread,
ref)`; the cwd is the worktree path joined with the source cwd relative to the source checkout when the source cwd
is under `<checkout>/`, else the worktree path; the setup line `ctx.take_setup_line(made id)`; `start(made id, {...o,
cwd?, setup?})`. When that start fails and the worktree is new to this call and was made by it, it is removed
(`ctx.remove_worktree(made, true)`, a failure warning `the worktree a refused fork made at <path> was not removed:
<message>`) and its branch deleted (`git branch -D <branch>`, failure dropped), and the start's error goes on.

F12. `pay_cut_owed(ctx, adapter, at, resume)` (`:231-240`): no `cutOwed` on the thread's record, or an adapter that
forks no session, answers `resume` and changes nothing. Else the fork through `{anchor: cutOwed}`; a failure warns
`thread <threadWord> resumes without its owed cut: <message>`. A copy moves the thread's rows onto it (F10). Either
way `cutOwed` is removed from the record and the sessions are persisted (detached); the copy's session answers, else
`resume`.

## Tests (`tests/forks.rs`)

| Rule | Test |
|---|---|
| F1 | `fork_opened_refuses_a_fork_beside_a_thread_or_a_conversation`, `fork_opened_runs_on_the_sources_agent_and_picks`, `fork_opened_keeps_what_the_start_named`, `fork_opened_refuses_another_agent` |
| F2, F3 | `read_fork_picks_the_latest_finished_turn_by_default`, `read_fork_refuses_a_running_turn_and_a_turn_never_run`, `read_fork_refuses_an_at_out_of_range`, `read_fork_at_zero_carries_no_turn_and_no_earlier_rows`, `read_fork_refuses_a_thread_the_caller_does_not_reach` |
| F4 | `read_fork_refuses_a_turn_of_a_deleted_sub_machine` |
| F5 | `read_fork_plan_carries_the_turns_events_anchor_ref_and_the_turns_after` |
| F6 | `fork_turn_of_refuses_a_turn_with_no_anchor_on_a_harness_that_does_not_count` |
| F7 | `copied_events_drop_pos_and_mark_the_source` |
| F8 | `copy_images_keeps_each_image_at_its_index_under_the_fork` |
| F9, F12 | `pay_cut_owed_moves_the_thread_onto_the_copy_and_clears_the_debt`, `pay_cut_owed_warns_and_resumes_uncut_when_the_copy_fails` |
| F10 | `move_session_renames_only_that_threads_rows_on_that_session` |
| F11 | `fork_start_without_a_branch_starts_on_the_source`, `fork_start_refuses_a_branch_with_no_checkpoint`, `fork_start_runs_in_the_matching_subfolder_of_the_worktree`, `fork_start_takes_its_worktree_and_branch_away_when_the_start_refuses` |
