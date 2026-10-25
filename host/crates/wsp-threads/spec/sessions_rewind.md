# sessions_rewind: `threads/sessions.ts:1264-1396`

`rewind(thread, { turnId, files, undo })`: a thread back to one of its turns, its conversation, its files or both;
and the undo of the last files rewind.

## Refusals, all before anything moves (1265-1346)

- **W1** After `ready`. The workspace is the thread's first row's, else its record's; none, or not reached:
  `not_found_refusal("no thread <word>")`. A conflict is `HostError` of kind `conflict`.
- **W2** A row of the thread running: `REWIND_WORKING_LINE`. The thread's kept agent is reaped. A thread under it
  running (folded titles): `rewind_children_line(titles)`.
- **W3** Undo: no `rewound` on the record: `REWIND_NO_UNDO_LINE`; its turn's machine gone (`sub_turn_gone`):
  `SUB_TURN_GONE_LINE`; another thread of the workspace running: `rewind_beside_line(title)`. Then the machine is
  woken where not running, `git.restore` to `rewound.before` (queued on the workspace, through the daemon), the
  record loses `rewound`, the rows persisted, the transcript flushed, `thread.rewound` emitted. Answers
  `{ turns: 0, files: reply.files or 0 }`.
- **W4** The thread's turns in transcript order. A turn not among them: `not_found_refusal("no turn <id> on thread
  <word>")`; its machine gone: `SUB_TURN_GONE_LINE`; the latest turn: `REWIND_LATEST_LINE`; a turn copied from
  another thread: `REWIND_COPIED_LINE`.
- **W5** The kept checkpoint is the newest `session.checkpoint` of that turn. Files asked with no `ref`:
  `REWIND_NO_CHECKPOINT_LINE`.
- **W6** Files go back only where no other thread of the workspace has a row ending after the checkpoint (a running
  row counts as ending never): otherwise files are dropped and `kept: REWIND_SHARED_LINE` rides the answer.
- **W7** A harness that said it cannot cut (`kept` on a checkpoint of the thread) with no files: `REWIND_SHARED_LINE`
  where shared, else `rewind_kept_line(why, false)`.
- **W8** A harness with no revert but a fork copies; a copy with no anchor: `rewind_no_anchor_line(agent)`. Neither
  conversation nor files: `REWIND_SHARED_LINE` where shared, else `rewind_no_anchor_line(agent)`.

## The rewind (1350-1395)

- **W9** The machine woken; files first (`git.restore` to the checkpoint's ref); a refusal after that puts the
  files back to `before` (warned on failure) and fails.
- **W10** A reverting harness with a session: `revert({ session, cwd, beforeTurn })` where the first cut turn named
  an anchor, else `{ turns: each cut turn's { anchor?, result? } }`; its `kept` is remembered.
- **W11** A copying harness: `fork_session({ session, turn: { anchor } })`; a `resume` moves the thread's session.
- **W12** The record (made where none) loses `cutOwed` on a copy, gains `rewound: { before, at: now }` after files.
- **W13** A harness that kept the conversation: rows persisted, flushed, emitted, then
  `rewind_kept_line(why, files moved)` as a conflict.
- **W14** The conversation cut: the cut turns' events dropped from the transcript; the latest row's lines rewritten
  from the turn now last. `slates.rewound({ threadId, turnId, cut })`; persist, flush, emit. Answers `{ turns: cut
  count where cut else 0, files?, kept? }`.

## Tests (`tests/sessions_rewind.rs`)

Ports rewind.test.ts (26 cases) at the seam; the porter maps each case to the rule it pins.

| Rule | Test |
|---|---|
| W1, W2 | `a_running_thread_is_not_rewound`, `a_running_child_is_named` |
| W3 | `undo_puts_the_files_back_and_drops_the_mark`, `undo_with_nothing_to_undo_is_refused` |
| W4, W5 | `the_latest_turn_is_not_rewound`, `a_copied_turn_is_not_rewound`, `files_with_no_checkpoint_are_refused` |
| W6 | `files_another_thread_touched_stay_and_the_answer_says_so` |
| W7, W8 | `a_harness_that_keeps_the_conversation_and_no_files_is_refused`, `a_copy_with_no_anchor_is_refused` |
| W9 | `a_refused_revert_puts_the_files_back` |
| W10-W14 | `a_rewind_cuts_the_later_turns_and_their_rows`, `a_copying_harness_moves_the_session`, `a_rewind_to_turn_one_of_three_answers_two_turns` |
