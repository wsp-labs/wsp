# trimmed_row: `threads/trimmed-row.ts` (29 lines)

`trimmed_row(ctx, thread_id, record, ended)` builds the row a thread is listed by once every row of its own fell off
the index cap (trimmed-row.ts:9-29). Pure over three context reads.

## Rules

- **T1** `ended` defaults to `record.ended`; the row's `status` is `ended`, else `completed`. Never `running`: the cap
  never drops a running row.
- **T2** `id` and `threadId` are both the thread's id; `workspaceId` and `harness` are the record's.
- **T3** `resume = ctx.started_as(workspace_id, thread_id)`. Where present, `claudeSessionId: resume`, and
  `cwd = ctx.folder_of(workspace_id, resume)` where that answers one. With no resume, `folder_of` is never asked and
  there is no `cwd`.
- **T4** `words = ctx.thread_words(workspace_id, thread_id)` (the transcript index's `words.get(threadId)`):
  `prompt: words.first` where present; `startedAt` and `endedAt` both `words.last` only where `words.last > 0`. No
  index, or no words for the thread: neither key.
- **T5** `parentThreadId` present: the row says `startedBy: agent` and `parentThreadId`. Absent: no `startedBy`.
- **T6** `rootThreadId`, `permissionMode`, `forkedFrom` copied where the record has them.
- **T7** Key order of the serialized row is the TS literal's: `id, threadId, workspaceId, harness, status, prompt,
  startedAt, endedAt, startedBy, parentThreadId, rootThreadId, claudeSessionId, cwd, permissionMode, forkedFrom`.
  Since `SessionView` serializes in struct order, the tests compare the serde value, not text (as places-b does).

## Tests (`tests/trimmed_row.rs`)

| Rule | Test |
|---|---|
| T1, T2 | `a_record_with_no_end_reads_completed_under_the_threads_id`, `the_end_passed_wins_over_the_records` , `the_records_own_end_is_the_default` |
| T3 | `a_thread_with_a_session_names_it_and_its_folder`, `no_session_asks_no_folder`, `a_session_with_no_folder_has_no_cwd` |
| T4 | `the_first_words_title_it_and_the_last_place_it_in_time`, `last_zero_places_nothing`, `no_words_no_prompt_and_no_times` |
| T5, T6 | `a_child_reads_started_by_its_agent_under_its_parent`, `the_records_root_access_and_fork_ride_the_row` |

TS cases: session-row.test.ts and thread-cap.test.ts list a thread whose rows fell off the cap ("listed off its
record"); sessions_reads covers the listing.
