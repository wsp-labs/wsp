# threads_turn_end: checkpoints, taken turns, steers, and what a turn changed

TS: `T/threads.ts:956-1142` at `65f6b89c7`. Rust: `src/threads/threads/turn_end.rs`, methods on `ThreadsArea`. State:
`steers_out: RefCell<IndexMap<steer id, SteerOut>>` (`SteerOut { ended: Option<(TurnRow, bool)> }`),
`checkpoints_landing: RefCell<IndexMap<thread, Joined>>` (written by slice a's turns, read here by no one; it is the
area's member). `TurnRow` is the snapshot of a row these functions read: `{view, turn_id, turn_live}`. A function that
changes a row's `turnLive.steered` changes the entry in `ctx.sessions()` keyed by the row's `view.id`.

## Rules

E1. `not_a_repo(r)` (`:957`): the kind runs in a folder (`runsInFolder`) and `ctx.project_held(r.project).git` is
absent.

E2. `keep_checkpoint(entry, turn)` (`:963-978`): unless `not_a_repo`, asks the workspace's daemon `{op:
git.checkpoint, cwd: ctx.checkout_of(record), thread, turn, scope: record.id}` in that key order. A string `ref` in
the answer is kept; `based` is true only for `based: true`. A failure warns `noCheckpointLogLine(thread, record id,
message)` and goes on with no ref. With no ref, no `anchor` and no `kept`, nothing is recorded. Else
`record({type: session.checkpoint, workspaceId, sessionId, turnId, threadId, ref?, based?: true only with a ref,
anchor?, kept?})`.

E3. `taken_turn(ws, taken)` (`:982-991`): `None` when `sessions` has no row `taken.sessionId`. When that row holds a
handle and its turn is `taken.turnId`, that handle with `outcome` replaced by `taken.outcome`. Else, after
`ctx.open_thread(ws, taken.threadId)`: a handle with `id = taken.sessionId`, `workspaceId`, `turnId`, `outcome`, a
`finished` already settled to the first `session.done`'s `result` for that turn, else `{failed, error: <the first
session.end's reason, else RESTARTED_REASON>}`; `view()` reads the row from `sessions` at each call, falling back to
the row as it was; `interrupt` does nothing.

E4. `keep_on_turn(s, o, caller, steer_id)` (`:995-1002`): nothing (and no persist) when the row has no `turnLive`.
Else `turnLive.steered[steer_id] = {prompt, startedBy: o.startedBy ?? person, by? (scope of caller), road? (road of
caller), requestId?, attachments? (records, only when any)}` in that key order, an existing id keeping its place, then
`ctx.persist_sessions(ws)`, whose future it answers.

E5. `keep_steer` (`:1004-1007`): `steers_out[steer_id] = {}` before `keep_on_turn`.

E6. `drop_steer(s, id)` (`:1009-1014`): nothing when the row did not keep it; else removes it (the others keep their
order) and persists detached. `steer_lost(s, id)` (`:1017-1020`): forgets the steer's write, then `drop_steer`.

E7. `steer_answered(s, handle_id, o, caller, id, landed)` (`:1025-1038`): takes `steers_out[id]` away and reads its
`ended`. Not ended and landed: `record_steer`, answers true. Ended by a stop: `send_back(ended row, [id], true)` (the
person is told), true. Every other case (not landed; or ended without a stop, landed or not): `drop_steer`, false (the
start road sends it as the thread's next turn).

E8. `record_steer(s, handle_id, o, caller, id)` (`:1039-1058`): keeps it on the turn (E4, detached) when the row did
not already; when the row has a thread and the send carried attachments, `ctx.keep_sent_images(ws, thread,
requestId, attachments)` runs detached before the row is written; then `record({type: session.steer, workspaceId,
sessionId: claudeSessionId ?? handle_id, turnId, threadId?, prompt, attachments? (records), requestId?, via?,
waiting: true only while the row's view is asking})`.

E9. `snapshot_of(entry, cwd)` (`:1067-1082`): `None` for `not_a_repo` or a reach other than `reachable`. Asks `{op:
git.snapshot, cwd}` and answers `String(answer.commit)` (an answer with no `commit` answers the text `"undefined"`),
racing a clock timer of `turnSnapshotMs` (option, default 3,000). A refusal with code `not-a-git-repo` answers `None`
silently; any other failure warns `no snapshot of <cwd> on <record name>: <message>` and answers `None`. The timer
winning warns `no snapshot of <cwd> on <record name> inside <ms>ms; the turn runs without one and its reply lists no
changes` and answers `None`. The timer is cancelled either way.

E10. `top_of(entry, cwd)` (`:1086-1093`): asks `{op: git.worktrees, cwd}`; a reply that parses answers
`worktreeOf(cwd, <paths of the worktrees not prunable>)`; a failure or a reply that does not parse answers `None`.

E11. `shared_folder(entry, thread, cwd, top, from, to)` (`:1097-1106`): the cwds of this workspace's rows of any
other thread (a row with no thread counts as another) whose `startedAt` (absent 0) is at or before `to` and whose
`endedAt` (absent `to`) is at or after `from`. True when they hold `cwd` itself. With none, false without reading the
top. Else the top (`top()`, read once) and each distinct folder's `top_of` in order; true at the first equal top.
**Trap #1664**: two subfolders of one checkout share.

E12. `own_in(cwd, top, wrote)` (`:1110-1114`): empty `wrote` is an empty set without reading the top; no top is
`None`; else every path `repoPathOf(path, cwd, top)` answers.

E13. `read_turn_changes(entry, turn)` (`:1120-1142`): no snapshot (E9) answers false. Asks `{op: git.turn, cwd, from,
to}`; a failure warns `what <turnId> changed in <cwd> was not read: <message>`. A reply that does not parse answers
false. Files are `{path, kind, additions, deletions}`. No files and nothing moved answers true and records nothing.
Else `shared = shared_folder(.., startedAt, now)`; where shared and the harness reported `wrote`, the turn's files are
those in `own_in`, the rest `others`; `record({type: session.changes, workspaceId, sessionId, turnId, threadId, from,
to, files, others?, moved, shared?: true})`; true.

## Tests (`tests/threads_turn_end.rs`)

| Rule | Test |
|---|---|
| E1, E2 | `keep_checkpoint_records_the_ref_based_and_anchor`, `keep_checkpoint_of_a_folder_with_no_repo_asks_nothing`, `keep_checkpoint_warns_and_records_the_anchor_alone_when_the_daemon_fails`, `keep_checkpoint_with_nothing_to_keep_records_nothing` |
| E3 | `taken_turn_hands_back_the_running_handle_with_the_taken_outcome`, `taken_turn_of_an_ended_turn_reads_its_result_off_the_transcript`, `taken_turn_of_a_row_gone_is_none` |
| E4, E5 | `keep_steer_keeps_the_message_on_the_turn_and_persists` |
| E6 | `drop_steer_removes_only_that_message` |
| E7 | `steer_answered_records_a_landed_steer`, `steer_answered_tells_the_person_when_a_stop_ended_the_turn`, `steer_answered_drops_a_steer_the_turn_ended_on` |
| E8 | `record_steer_keeps_images_before_the_row_and_says_waiting` |
| E9 | `snapshot_of_answers_the_commit_or_nothing_inside_its_deadline`, `snapshot_of_is_silent_for_a_folder_git_holds_no_repo_in` |
| E10, E11 | `shared_folder_compares_checkout_tops_not_cwds`, `shared_folder_ignores_rows_outside_the_turns_time` |
| E12, E13 | `read_turn_changes_records_files_moved_and_shared`, `read_turn_changes_splits_own_files_from_others`, `read_turn_changes_of_nothing_changed_records_nothing` |
