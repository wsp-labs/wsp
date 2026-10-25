# codex_conversations
Source: packages/adapter-codex/src/conversations.ts (215 lines) at a3b870a6e; the store assembled at adapter.ts:1331-1388.
Tests: packages/adapter-codex/test/conversations.test.ts (9 cases), plus placeholder-login.test.ts "titles, drafts, renames, probes and
conversation reads each run codex through it" (1). Rust: `src/codex/conversations.rs`; tests `tests/codex_conversations.rs`, run
against a fake app server (`tests/common/fake_app_server.rs`, a port of `test/fake-app-server.ts`, a node script) and the fixture
`test/fixtures/app-server-threads-0.162.1.json` copied byte for byte.

## What it does
Codex's own threads on a computer, for a person picking one up as a wsp thread, read through `codex app-server` there: `thread/list` for a
folder's threads, `thread/turns/list` and `thread/items/list` for the earlier messages. Whether another process writes a thread is learned
by resuming it the way a turn would (`-32600 ... already has an active writer`). Nothing reads Codex's sqlite or rollouts directly.

## Behaviour
1. Constants (conversations.ts:16-28): `WRITER_HELD = "already has an active writer"`, gone when the error matches
   `no rollout found|thread not loaded|invalid thread id`, `LETS_GO = "Codex lets go about a minute after its window closes."`, page sizes
   100 threads, 100 turns, 200 items, `LIST_PAGES = 10`. Request ids 1 initialize, 2 the ask, 3 the open.
2. Every script (conversations.ts:45) is `cd ~ && <ENV_FROM_INPUT>\n`, then for `list` alone
   `command -v <program word> > /dev/null 2>&1 || { echo wsp-codex-not-installed; exit 0; }\n`, then `app_server_script(codex_word(launch, broker), stages)`.
   The first stage is always `initialize` and `initialized`, waiting for one answer.
3. `list_command` (:54): `thread/list` with `{cwd: [..cwds], sortKey: "recency_at", limit: 100, cursor?}`.
4. `parse_list` (:91): a line that trims to `wsp-codex-not-installed` is a lasting store error `codex is not installed`. No answer to id 2
   is `codex's app server did not answer thread/list`; an error answer is its message. Rows are each `data[]` object with non-empty `id`
   and `cwd`; `next` is a non-empty `nextCursor`.
5. A row (:63): `title` is the non-empty `name`, else the `js_trim`med `preview` where the field is a non-empty string (so a blank one is `""`, CV1),
   else the id; `firstPrompt` that trimmed preview, `""` included; `branch` a non-empty `gitInfo.branch`; `lastAt` is the first finite of `recencyAt`, `updatedAt`, `createdAt`,
   else 0, times 1000; `origin` is `wsp` where `originator == "wsp"`, `script` where `source` is `exec` or `appServer`, else `terminal`;
   `path` (dropped before the row leaves the store) a non-empty `path`. Field order: id, title, firstPrompt, branch, cwd, lastAt, origin.
6. `sizes_command(paths)` and `parse_sizes(stdout, n)` (:108-118): one line per path; a line whose JS `Number()` of its trim is a safe integer
   is the size (`1e3`, `0x10`, `+5`, `12.0` all count, CV2), a blank, missing or other line is absent. `n` entries always.
7. The store's `list` (adapter.ts:1335): no folder asks nothing and answers `[]`. Pages of `thread/list` run until no `next` or 10 pages.
   Sizes are read in one more exec only where some row has a path; a failed size read is no sizes. Each row carries `bytes` where read.
8. `turns_command` (:122): with a cursor, one stage of `thread/turns/list`; without, an open stage first: `thread/resume {threadId}` when
   `probe`, else `thread/read {threadId, includeTurns: false}`, then the turns stage. Turns params: `threadId, limit: 100, sortDirection:
   "desc", itemsView: "summary", cursor?`. `items_command` (:130): `thread/items/list {threadId, limit: 200, sortDirection: "desc", cursor?}`.
9. `parse_opened` (:136): no answer to id 3 is `codex's app server did not answer opening the thread`; an error holding `WRITER_HELD` is
   `open`, one matching the gone pattern is `gone`, any other is its message; else `{cwd?, title?}`: cwd the thread's non-empty `cwd`,
   else the result's; title the non-empty `name`, else the trimmed non-empty `preview`.
10. `parse_turns` (:151): each turn with a non-empty id and the count of its `items` whose `type` is `userMessage` or `agentMessage`.
11. `parse_items` (:168): each `data[]` entry's `item` object; its line (`lineOf`, :184) is a person line of its `content[]` parts with
    `type: "text"` and a non-empty text, joined by `\n` and trimmed, none if empty; an agent line of a non-empty `text`; else a tool line
    from `toolCallOf` (codex_items) or none. `at` is the entry's finite `startedAtMs`, else `completedAtMs`; `turnId` its non-empty `turnId`.
12. `newest_lines(items, total, last)` (:200): from the newest back, keep every tool line and up to `last` messages; stop at the first message
    past `last`; then drop tool lines at the front. `earlier` is `max(0, total - messages kept)`.
13. The store's `earlier` (adapter.ts:1353): the id is a slug (refused before anything runs). The first exec opens and reads the newest turn
    page; `gone`/`open` answer as is. Further turn pages are read only to count (not when `count: false`), up to 10 pages in all. Kept
    turns are the newest ones until their messages reach `last`. Item pages are read newest first, each item put at the front; reading
    stops at the first item of a turn not kept, or after 10 pages. The answer is `{cwd?, title?, lines, earlier}`, `earlier` 0 under `count: false`.
14. `store_marks(home)`: `letsGo` is `LETS_GO`; `stamp` runs `newest_day_stamp_command(home)`, which is
    `d=$(ls -d '<home>/sessions'/*/*/* 2>/dev/null | tail -n 1); <foldersStampCommand(['"$d"'])>`.
15. Every exec of the store but the sizes read carries the question environment: the turn's own env where brokered, else `build_env({base, home})`.

## Edge cases
- An id like `x' or '1'='1` is refused with `plain slug` and nothing runs.
- `thread/read` answering `thread not loaded: <id>` is `gone`; a resume refused for a second writer is `open`.

## Behaviour nobody wrote down
- The `str` here refuses `""`. Rule 8.6 for every trim.

## Decisions
1. The store is an inherent part of `CodexAdapter` (the port carries no `ConversationStore` yet; asked for in the report).

## After the port
- Nothing.

## Rules added for the port (each value printed by the TS at 65f6b89c7)

- **CV1.** A list row whose `preview` is `"   "` has title `""` and firstPrompt `""`; a `name` of `"  "` is the title as it is.
- **CV2.** `parse_sizes` of `1e3, 0x10, +5, 12.0, -5, Infinity, NaN, 1_0, "5 6", 9007199254740992, "", " 7 "` with 13 wanted is
  `1000, 16, 5, 12, -5, -, -, -, -, -, -, 7, -`.
- **CV3.** `parse_opened` reads a blank preview as title `""`; an empty `name` falls to the preview.
- After-port: a blank preview makes an empty title; `parse_sizes` reads hex and exponent text as sizes; two rows with one rollout
  path lose the size (the later absent one wins in the map); `earlier`'s 10 pages count the first.
- **CV4.** `newest_lines` over `[t0, p1, t1, a1, t2, p2, a2, t3]`, total 10: last 0 keeps `[]` with 10 earlier; 1 keeps `[a2, t3]`, 9;
  3 keeps `[a1, t2, p2, a2, t3]`, 7; 4 and 9 keep `[p1 .. t3]`, 6 (the tool line before the oldest message kept is dropped).

## Tests (tests/codex_conversations.rs, 10; TS 9 + 1)
- Rules 2-7: `the_threads_codex_kept_in_a_folder::come_off_thread_list_on_the_computers_own_app_server_newest_first_with_a_name_a`
  (row fields, origin, sizes), `...::pages_thread_list_by_its_cursor_and_leaves_out_the_size_of_a_rollout_that_is_not`,
  `...::asks_nothing_for_no_folder_at_all`.
- Rules 8-13: `a_codex_threads_earlier_messages::come_off_thread_turns_list_and_thread_items_list_each_at_the_time_codex_stamped`,
  `...::for_a_preview_read_one_page_of_turns_and_count_nothing_the_thread_never_taken`,
  `...::keep_the_newest_messages_and_count_the_rest_and_read_the_thread_without_taking_i`,
  `...::are_open_where_codex_refuses_the_resume_a_second_writer_and_gone_where_it_holds`,
  `...::refuse_an_id_that_is_not_a_plain_slug_before_anything_runs`.
- Rule 14: `...::say_a_person_may_try_again_once_codex_lets_go_and_how_soon`.
- Rules 2, 15 (each line runs codex through the brokered word): `every_other_codex_launch_on_a_brokered_machine::titles_drafts_renames_probes_and_conversation_reads_each_run_codex_through_it`.
- CV1 to CV3: `rules_added::*` in `codex_conversations.rs` (3 tests).
- CV4: `rules_added_newest::the_newest_lines_keep_every_tool_line_back_to_the_last_message_kept`.
