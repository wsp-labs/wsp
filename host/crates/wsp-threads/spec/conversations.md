# conversations: the agents' own conversations outside wsp

TS: `T/conversations.ts` (273 lines) at `65f6b89c7`. Rust: `src/threads/conversations.rs`, `ConversationsArea` on
`Rc<dyn ConversationsCtx>`. State: `kept: RefCell<IndexMap<"<project>\0<harness>", Kept {at, cwds, stamp, read:
Shared<Result<StoreRead, HostError>>}>>`, memory only. Constants: `STORE_EXEC_MS` 30,000, `KEPT_LIST_MS` 60,000,
`EARLIER_MESSAGES` 200, `PREVIEW_MESSAGES` 4, `PREVIEW_TAIL_BYTES` 262,144. Every refusal is `refusal(happened, fix,
kind)` with the protocol's words and kind.

## Rules

V1. Forgetting (`:75-80`): on every `session.done` and `session.end`, the kept reads of that workspace's project (keys
starting `<project>\0`) go. A workspace not live forgets nothing.

V2. `wsp_sessions()` (`:83-88`): agent session id to thread: each held transcript index's `starts` (thread to
session) in index order, then every row's `claudeSessionId -> threadId` (rows with both), a row overwriting.

V3. The store road (`:90-93`): `exec` is `harnessExec(machine, 30000)`; `ask` sends a daemon frame through
`ctx.with_daemon(entry, ..)`.

V4. `folders_of(entry)` (`:97-103`): the project's path, then, on a kind that copies a folder and with a git top, every
worktree `ctx.worktrees_of(top)` lists, then every folder record of the project whose worktree is not gone; each path
once, first place kept.

V5. `stores_on(entry, agent)` (`:106-111`): catalog agents in order, only `agent` when named, with an adapter, not
turned off on the entry's place, whose adapter reads conversations.

V6. `unread_of(e, entry, harness)` (`:114-119`): an error whose message starts with `unknownOpLine("")` (a daemon from
before the op) is `{agent, said: conversationsUnreadLine(label, computer), fix: conversationsUpdateFix(computer)}`;
any other is `None`.

V7. `read_store` (`:125-142`): the list and the live ids run together (`live` is none for a store with no live
read). A failed list: a daemon too old answers no rows with that held line; else warn `the <label> conversations on
<computer> were not read: <why>`; a lasting store error (`isLastingStoreError`) answers no rows and no line; any other
answers no rows, `{agent, said: conversationsFailedLine(label, why), fix: CONVERSATIONS_AGAIN_FIX, why}` and is
passing. A failed live read fails the read in the TS; the Rust store's `live` cannot fail, so it answers none.

V8. `kept_read` (`:147-166`): first every kept read 60 s or older (clock) goes, any project's. The stamp is the
store's `stamp(cwds, road)`, `""` for a store with none or a failed stamp. A kept read under the same folders (joined
with `\0`) and stamp answers it; when the store reads live ids and one of the kept rows that is not a wsp thread was
live, the live ids are read again, kept on the entry (if it is still that one) and answered. Else a fresh read is kept
`{at: now, cwds, stamp}` and dropped again once it turns out passing or failed (only while it is still the kept one).

V9. `list(o, origin)` (`:168-199`): the folder for `o.project` (or the default) through `ctx.folder_for`. A named agent
with no store refuses `noConversationsLine(label)`, `NO_CONVERSATIONS_FIX`, `usage`. For each store, at once: its held
line goes on `held` as each read settles; each row: a script's conversation that is no wsp thread is left out; else
`{agent, ...row, origin: wsp for a wsp thread else the row's, live: not a wsp thread and live now, thread: the wsp
thread only where the caller reaches its latest row (ctx.reaches_row), letsGo: the store's when it has one}`. Rows
sorted by `lastAt` newest first, then `id` (`localeCompare`). Answers `{rows, held}`.

V10. `earlier_rows(lines, left, label)` (`:203-209`, the harness's label passed in): when `left > 0`, first a `note`
row `earlierLine(left, label)`
at the first line's `at`; then each line, a tool line as `toolDoneLine(tool, text) ?? toolActivityLine(tool, text)`
with `who: tool`, any other as `{who, text}`; `at` only where the line has one.

V11. `read(o, origin)` (`:213-226`): no store for the agent refuses as V9. Asks `earlier(id, {cwds, last: 4 or 200,
probe: false, tail: 262144 and count: false on a preview})`; a daemon too old refuses with its held line, kind
`usage`; any other failure goes through. An answer that is not a conversation refuses `conversationGoneLine(label, id,
computer)`, `CONVERSATION_GONE_FIX`, `not-found`. Answers `{cwd?, title: title ?? id, rows: earlier_rows(lines, 0 on a
preview else earlier, agent)}`.

V12. `opening(ws, o, origin)` (`:228-262`): `ctx.entry_of(ws, origin)`; the harness is `o.harness`, else
`default_agent_of(preferences)` (the held preferences, else read); no conversations store refuses as V9. A
conversation a wsp thread ran refuses `conversationIsThreadLine(id, thread)`, `conversationIsThreadFix(thread)`,
`usage`. The read is `earlier(id, {cwds, last: 200, probe: not copy})`; `gone` refuses as V11. `open` reads the title
again with `{last: 0, probe: false}`. Open (the read said so, or, not copying, the live ids hold it) refuses
`conversationOpenLine(title, computer)`, `conversationOpenFix(store.letsGo)`, `CONVERSATION_OPEN_KIND`. A read that is
not a conversation refuses as gone. Answers `{harness, id, copy, project, cwd?, title, earlier}` in that key order.

V13. `placed(start, resume, origin)` (`:264-270`): a start naming a thread, a branch or a cwd refuses
`RESUME_WHERE_LINE`, `RESUME_WHERE_FIX`, `usage`. The base workspace is the start's, else the project's folder; then
`opening`; then the folder for the opening's project and cwd. Answers `{workspaceId, cwd?, outside}`.

## Tests (`tests/conversations.rs`)

| Rule | Test |
|---|---|
| V1, V8 | `a_kept_list_answers_a_second_visit_and_goes_when_a_thread_ends`, `a_kept_list_goes_after_a_minute`, `a_new_stamp_reads_the_store_again`, `a_kept_list_reads_live_ids_again_when_one_of_the_persons_rows_was_open` |
| V2 | `wsp_sessions_reads_the_index_starts_then_the_rows` |
| V4 | `folders_of_lists_the_project_its_worktrees_and_held_folders_once` |
| V5, V9 | `list_marks_wsp_threads_and_leaves_out_scripts`, `list_refuses_a_named_agent_with_no_store`, `list_hides_a_thread_the_caller_does_not_reach`, `list_sorts_newest_first_then_by_id` |
| V6, V7 | `list_says_the_update_a_daemon_too_old_needs`, `a_passing_failure_is_said_and_not_kept`, `a_lasting_store_error_lists_nothing_and_says_nothing` |
| V10, V11 | `read_answers_the_newest_rows_with_a_note_for_the_rest`, `read_of_a_preview_asks_the_tail_and_counts_nothing`, `read_of_a_conversation_gone_refuses_not_found` |
| V12 | `opening_refuses_a_conversation_a_thread_ran`, `opening_refuses_an_open_conversation_with_its_title`, `opening_answers_the_earlier_rows` |
| V13 | `placed_refuses_a_thread_branch_or_cwd_and_places_the_opening` |
