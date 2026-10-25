# views_session
Source: `foldThreads` (:380-444) of `packages/protocol/src/views/session.ts` (455 lines) at a3b870a6e, with its private `threadCost` (:452-455). The rest of the file is wsp-wire's: the shapes (`SessionView`, `ThreadView`, ...), `threadKeyOf` :368 and `threadRan` :374 (wsp-wire's spec/views-small.md); `needsYouCount` :448 is web-only and ported by neither crate (wsp-wire's spec/shapes.md, Decision 2). Tests: `packages/protocol/test/protocol.test.ts`, the 16 cases of :1530-1867 that call foldThreads: 15 ported in `tests/protocol_words.rs`, `mod thread_provenance`; :1545 (a thread reads its attempt off its opening row alone) is not ported. :1789 and :1798 reach the fold only through needsYouCount and stay out, and every `ThreadView.parse` assert is wsp-wire's. No pinned module.

## What it does
Folds the session index, one row per turn, into the threads the sidebar and `wsp threads` list: who opened each thread and where comes off its first turn, its state and marks off its latest, and its title, turn count, whether it ever ran and what it cost off all of them.

## Behaviour
1. Grouping (:381-388): rows group by `thread_key_of` (the thread id, else the row's own id); threads come in the order of each one's first row, turns in row order. `first` is a thread's first row, `latest` its last.
2. From the first row (:396-400, :438-440): `id` is the key; `threadId` where the first row has one; `workspaceId` and `harness`; `startedBy`, "person" where the first row has none; `parentThreadId`, `rootThreadId` and `attempt` where set.
3. From the latest row (:401-433): `status`, `sessionId` (the latest's id), and each of claudeSessionId, startedAt, endedAt, cwd, asking, waitingOn, setupRefusal, sub, diskKeptUntil, diskGone, capped, limit, resumeAt, lastLine, failure, pid, agentPid, readAt, settledAt, pinnedAt, order, foldedAt, replaces, replacedBy, snoozedUntil, wokeAt, section, rewoundAt, forkedFrom and permissionMode where the latest has it; an earlier row's value never stands in. `fast` only where the latest's is true; `subagents` only where the latest's list is not empty (:434-435).
4. The title (:394): the latest row's harnessTitle through `title_line`; else the first row's prompt through `opening_title`; else the first row's claudeSessionId; else the first row's id.
5. `turns` is the thread's row count; `ran` is `thread_ran` over its rows: a turn running, or one with a harness session id and no refusal (:436-437).
6. `costUsd` (:441, :452-455): the rows' figures that are set, added from 0 in row order; left out where no row carries one, so a thread whose figures are all 0 says 0.

## Edge cases
- A row with no thread id is a thread of one under its own id, with no `threadId` key (test :1530, row s4).
- A row with no thread id whose own id equals another row's thread id joins that thread; where it comes first, the thread has no `threadId` key (read off :368 and :397).
- Costs 0.75 then 0.39 sum to 1.1400000000000001 (test :1559 checks within 5e-11).
- A pid on an earlier turn and none on the latest: no pid (test :1597). A prompt raised on an earlier turn: no `asking` (test :1613).
- `fast: false` on the latest leaves the key out, the same as no `fast`.
- harnessTitle "  Building  the server \nand its tests" titles "Building the server" (test :1854).

## Behaviour nobody wrote down
- Rules 8.6 and 8.7: `title_line` splits on `\r?\n`, keeps the first line with anything but whitespace, folds `\s+` to one space and trims, all by JavaScript's whitespace.
- Rules 8.4 and 8.5: `opening_title` takes the first sentence (`^.*?[.!?](?=\s|$)`) and cuts it with `cut_line` to 48 UTF-16 units, the ellipsis included, never half a surrogate pair.
- The cost is f64 addition from 0.0 in row order, so the Rust sum carries the same bits as the TS one.
- The groups are an insertion-ordered map: a thread's place is its first row's.

## Tapes
`wsp threads` and the `threads` tool (host/src/verbs/workspaces-help.ts:134, :180); the runtime's count of what runs on a computer (runtime/src/runtime.ts:692), a workspace's thread titles (:945), the tree a thread reads (runtime/src/threads/sessions.ts:1278); the app's sidebar (apps/web/src/adapt/workspaces.ts:27).

## Decisions
1. `fold_threads(&[SessionView]) -> Vec<ThreadView>`, the TS arguments as they are; a key the TS leaves out is None, which serde leaves out.
2. ThreadView is wsp-wire's struct, keys in the zod shape's order (wsp-wire's spec/shapes.md, Decision 1). The TS literal writes turns, ran, the opener's provenance and costUsd last; the keys and values are the same, their order in raw JSON is not.
3. `threadCost` has no function of its own: `fold_threads` sums inline, `costUsd` a JsNumber.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `thread_provenance::foldthreads_groups_turns_by_threadid_titles_by_the_opening_turn_reads_state_row` |
| 2 | `thread_provenance::foldthreads_groups_turns_by_threadid_titles_by_the_opening_turn_reads_state_row`, `thread_provenance::a_thread_always_says_who_opened_it_the_fold_reads_a_row_from_before_provenance_a`; parentThreadId, rootThreadId and attempt are pinned by no test (:1545 is not ported) |
| 3 | `thread_provenance::foldthreads_carries_the_access_and_the_fast_mode_the_latest_turn_ran_at_so_a_lis`, `thread_provenance::foldthreads_carries_the_latest_turn_s_process_on_this_computer_so_the_pane_heads`, `thread_provenance::foldthreads_carries_the_latest_turn_s_open_prompt_so_the_word_every_row_reads_co`, `thread_provenance::foldthreads_carries_the_latest_turn_s_last_line_its_failure_and_the_fold_mark_so`, `thread_provenance::foldthreads_carries_the_latest_turn_s_folder_so_every_director_shows_where_the_t`, `thread_provenance::a_turn_that_ended_and_that_no_window_has_shown_since_reads_done_until_a_read_sta`, `thread_provenance::a_snooze_that_passed_reads_done_until_a_read_at_or_after_its_end_and_the_thread` |
| 4 | `thread_provenance::foldthreads_titles_a_thread_by_its_opening_prompt_s_first_line_so_the_cli_s_tabl`, `thread_provenance::foldthreads_titles_a_thread_with_no_harness_title_by_its_opening_turn_s_first_se`, `thread_provenance::foldthreads_titles_a_thread_by_what_the_harness_calls_the_session_the_next_send`, `thread_provenance::foldthreads_keeps_the_opening_turn_s_words_while_the_harness_has_no_title_of_its`; the claudeSessionId and id fallbacks are pinned by no test |
| 5 | `thread_provenance::a_thread_reads_as_run_once_a_turn_of_it_did_work_announcing_a_session_is_not_eno`, `thread_provenance::foldthreads_groups_turns_by_threadid_titles_by_the_opening_turn_reads_state_row` |
| 6 | `thread_provenance::foldthreads_adds_up_what_a_thread_s_turns_cost_and_says_nothing_where_no_turn_of` |
