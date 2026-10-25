# thread_read
Source: the functions of `packages/protocol/src/thread-read.ts` (280 lines) at a3b870a6e; its zod shapes (`ThreadVoice`, `ThreadMessage`) and flat constant (`THREAD_SEED_CHARS`) are `wsp_wire::types::thread_read`, and the events it reads (`SessionEvent`, `SessionPermissionEvent`, `TurnResult`) are `wsp_wire::views::session_events`. Tests: `packages/protocol/test/thread-read.test.ts` (29 of 35 cases ported in `tests/thread_read.rs`). Skipped: :452-:473 (4 cases on `threadRowLines`, `fmtClock`, the two silences and `turnEndLine`: module `words::turn`, ported in `tests/turn.rs`), :520-:536 (2 cases on `worktreeOf` and `repoPathOf`: module `project_path`).

## What it does
One thread read off the transcript the host holds: the rows a reader sees (who spoke, when, and the words), the same rows as Markdown and as the seed a fresh session is handed, the result and final reply of the thread's latest turn, and the permission prompt the thread is stopped on.

## Behaviour
1. An event is the thread's only where its `threadId` equals the id asked for; an event with no `threadId` is no thread's (thread-read.ts:31). `open_ask` uses the same test (:275).
2. `thread_messages`, the rows each event makes (:77-104, :154-156): `session.start` closes the open agent row, clears the calls, the text and reply flags and the step and plan rows, then says its prompt as person where it has one; `session.steer` is person; `session.moved` is turn, `threadMovedLine(from, to, fresh === true)`; `session.behind` and `session.notice` are turn with their text; `thread.waited` is turn, `said` where present, else `waitedLine(ms)`; `session.earlier` keeps its `who`, a note read as turn; `session.compacted` is tool, `compactedLine(before, after)`. Any other event, and a delta of any other kind (a note, reasoning), makes no row.
3. Text deltas (:106-114): an empty piece makes nothing. A piece joins the open agent row, unless both carry a `messageId` and the two differ; a piece with no `messageId` joins whatever agent row is open. A new agent row takes the piece's `at` and `messageId`. Every other row closes the open agent row.
4. Tool calls (:115-136): a `tool_use` whose id this turn already holds appends its text to that call's input, takes a new `toolName` where one is given, and redraws the row with `toolActivityLine` over the whole input, the row keeping its first `at`. Any other `tool_use` is a new tool row, kept by its id where it has one, named "tool" where it has no name. A `tool_result` for a held id that is not `isError` turns the row to `toolDoneLine` where that answers a line; an error or an id not held leaves the present-tense line.
5. Turn ends (:71-76, :140-153): `session.done` marks the turn replied and ends it: where no text arrived and `result.text` is not empty, an agent row with that text, then a turn row `turnEndLine(result)`; the calls and the text flag are cleared, the replied flag is not. `session.end` with `unstarted` says its prompt as person where present, then ends a failed turn with `reason`, else NO_RESULT_LINE. Any other `session.end` ends such a failed turn only where the turn has not replied.
6. Plans and changes (:157-173): `steps` make one tool row per turn, `planStepsLine`, rewritten in place by later steps; plan `text` makes one agent row per turn, rewritten in place. `session.changes` makes a tool row, `turnChangesLine`, put just above the last row where that row is a turn row, else at the end, and closes the open agent row.
7. `thread_markdown(rows)` (:185-212): a person row is the block "**You** <clock>" ("**You**" where the clock is ""), then the text as a block of its own. A turn row is one block, each non-empty line as "*line*", empty lines kept empty. An agent or tool row opens with a "**Agent**" block where the last speaker was not the agent; agent text is a block as written; a tool row is "- <first line>" with each later line behind two spaces, appended to the block before where the row before is a tool row. Lines join with "\n", blocks with "\n\n".
8. `thread_seed(messages)` (:39-44): walks back from the newest row, keeping rows while the kept texts' lengths sum to at most THREAD_SEED_CHARS (100 000), and stops at the first that does not fit. With nothing left out it is `thread_markdown` of every row; else "*<n> earlier message|messages left out*\n\n" and the Markdown of the kept rows. Only the rows' texts count, not the Markdown around them.
9. The latest turn (:219-238), read newest first over the thread's events: the first `session.end` met is the end, and a second stops the walk; a `session.done` is taken where no end was met yet or its `turnId` is the end's; a `session.start` met before any end marks a newer turn running; a start with the end's `turnId` stops the walk. A done answers its own `at` and result; an end with no done of its turn answers failed with `reason`, else NO_RESULT_LINE, at the end's `at`; no end and no done answer None. `thread_result` is that result (:242-244).
10. `thread_reply_rows` (:251-260): empty with no latest turn. The body is `notifyBody(result, "whole")`, else `turnEndLine(result)`; the row is the agent's where the body equals `notifyReply(result, "whole")`, else the turn's, at the latest turn's `at`. Where a newer turn runs, a second turn row NEWER_TURN_LINE with no `at` follows.
11. `lead_ask(open)`: the first item, None for none (:265). `open_ask` keeps the thread's `session.permission` events by `askId` in the order first asked, deletes one on its `session.permission.closed`, and answers the oldest still open (:272-280).

## Edge cases
- Pieces with `messageId` None then "msg_a" make one row, "half a sentence" (test).
- A `session.end` with no reason reads "failed: turn ended without a result" (test).
- Events with no `threadId` are read by no thread; a thread of none of them is [] (test).
- `thread_seed` of rows under the cap equals `thread_markdown` of them (test).
- A newest row longer than the cap leaves every row out: the answer is the count line and "\n\n" with nothing after (read off :41-43).
- A tool row's empty later line becomes two spaces (read off :207).
- `thread_reply_rows` over a done with no end yet and no later start has no NEWER_TURN_LINE row (test).

## Behaviour nobody wrote down
- Rule 8.1: `thread_seed` counts each text's length in UTF-16 units (`js::utf16`).
- Rule 8.16: the clock of a person row in `thread_markdown` is `fmtClock` in the host's zone.
- `open_ask` leans on JavaScript's `Map`: setting an `askId` already open keeps its first place, and a closed then re-asked id goes to the end. An `IndexMap` matches with `insert` and `shift_remove`; `swap_remove` would reorder the asks.

## Tapes
`wsp thread read` and its `--last` (`readThread`, packages/host/src/verbs/turns-help.ts:765-771) and the `thread_read` tool; the rows `wsp thread head` prints; the result `wsp threads wait` ends with; the open prompt `wsp thread allow` and `wsp thread deny` answer; the app's Copy as Markdown (thread menu and palette); the seed a fresh session is handed where the session it resumed is gone (packages/runtime/src/threads/sessions.ts:820).

## Decisions
1. `thread_markdown` and `thread_seed` take no zone and read the host's zone (jiff) for `fmt_clock`, where `words::turn::thread_read_text` takes it as a closure.
2. `lead_ask` takes any `IntoIterator` for the TS `Iterable<T>`; `open_ask` answers an owned `SessionPermissionEvent`.
3. `inThread` and `latestTurn` are private helpers, as in the TS.

## After the port
- `session.changes` goes above the last row whenever that row's voice is turn, which a notice, a waited line, a moved line or a behind line also has, not only the turn's end.
- The replied flag is cleared only by `session.start`: a turn whose start a capped transcript lost, and which ended with no result, reads no failed row where the turn before it replied.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `a_thread_s_messages::reads_only_the_named_thread_s_events_and_none_of_a_row_the_runtime_stamped_no_th`, `the_prompt_a_thread_is_stopped_on::is_read_per_thread_an_ask_open_on_another_thread_is_not_this_one_s` |
| 2 | `a_thread_s_messages::prints_the_line_a_thread_is_told_where_its_worktree_went_and_where_it_was_left_b`, `a_thread_s_messages::prints_how_long_a_turn_waited_for_this_host_and_the_sentence_the_host_answered_w`, `a_thread_s_messages::leaves_a_harness_s_note_out_a_read_is_the_words_of_a_thread_and_the_note_is_the`, `a_thread_s_messages::says_a_compaction_as_a_tool_row_with_the_figures_the_agent_gave`, `a_thread_s_messages::puts_a_message_steered_into_a_running_turn_where_the_turn_saw_it_as_a_person_s` |
| 3 | `a_thread_s_messages::folds_the_transcript_into_who_spoke_when_and_the_text_with_the_tool_call_one_row`, `a_thread_s_messages::reads_the_harness_s_two_messages_as_two_rows_the_reply_given_while_a_background`, `a_thread_s_messages::keeps_the_pieces_of_one_message_in_one_row_and_text_a_harness_named_no_message_f`, `the_printout_a_reader_sees::is_who_and_the_clock_on_one_line_and_the_text_under_it_a_blank_line_between_rows` |
| 4 | `a_thread_s_messages::keeps_a_call_whose_input_arrives_in_pieces_as_one_row_and_draws_its_line_from_th`, `a_thread_s_messages::holds_a_call_in_the_present_until_its_result_lands_so_a_read_of_a_write_nobody_h`, `a_thread_s_messages::gives_a_call_the_harness_named_no_id_for_a_row_of_its_own_and_never_runs_a_new_t` |
| 5 | `a_thread_s_messages::reads_a_turn_that_never_started_as_a_turn_of_its_own_with_the_words_it_was_sent`, `a_thread_s_messages::takes_the_reply_from_the_result_where_no_text_arrived_which_is_a_transcript_capp`, `a_thread_s_messages::says_why_a_turn_did_not_complete_on_its_turn_row_and_says_so_too_for_a_turn_the` |
| 6 | `a_thread_s_messages::says_the_turn_s_step_list_as_one_tool_row_the_count_then_a_task_line_per_step_re`, `a_thread_s_messages::says_what_a_turn_changed_as_one_tool_row_above_the_turn_s_end_however_late_the_c`, `a_thread_s_messages::says_which_changes_were_the_thread_s_own_in_a_folder_other_threads_worked_in_or` |
| 7 | `the_printout_a_reader_sees::copies_as_markdown_who_spoke_as_a_bold_header_the_reply_as_written_tool_rows_as` |
| 8 | `the_thread_a_new_session_is_handed::*` |
| 9 | `a_thread_s_latest_turn_and_its_final_reply::is_the_done_s_result_and_the_reply_is_the_whole_final_message_the_thread_s_finis`, `a_thread_s_latest_turn_and_its_final_reply::is_failed_with_the_runtime_s_reason_where_the_runtime_ended_the_turn_and_the_rep`, `a_thread_s_latest_turn_and_its_final_reply::is_nothing_when_the_transcript_holds_no_turn_of_the_thread_which_a_caller_answer`, `a_thread_s_latest_turn_and_its_final_reply::takes_the_newest_turn_not_an_older_turn_s_done` |
| 10 | `a_thread_s_latest_turn_and_its_final_reply::never_hands_a_footer_or_an_error_back_under_the_agent_s_name_a_completed_turn_th`, `a_thread_s_latest_turn_and_its_final_reply::says_under_the_reply_when_the_thread_has_started_another_turn_since_so_a_stale_r`, `a_thread_s_latest_turn_and_its_final_reply::is_the_done_s_result_and_the_reply_is_the_whole_final_message_the_thread_s_finis` |
| 11 | `the_prompt_a_thread_is_stopped_on::*` |
