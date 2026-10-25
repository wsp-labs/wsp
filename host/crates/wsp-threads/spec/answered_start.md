# answered_start: `threads/answered-start.ts` (54 lines)

Two free functions the socket's `sessions.start` and `sessions.warm` call (serve.ts). No state, no clock.

## `answered_start(answer_held, start, answer)` (answered-start.ts:11-21)

- **A1** `start` is called once, handed `Some(on_held)` when `answer_held` is true, else `None`.
- **A2** `on_held(HeldStart { view, turn_id, .. })` answers at once with `{ session: view, outcome: held, turnId }`
  and marks the start answered. No `notice` key on a held answer. A second `on_held` answers a second time (the TS
  has no guard; the start road calls it at most once, sessions.ts:676).
- **A3** When `start` resolves to a handle and nothing was answered: one answer
  `{ session: handle.view(), outcome: handle.outcome(), turnId: handle.turn_id(), notice? }`, `notice` present only
  where the handle carries one. `view()` is read at that moment, after the start resolved.
- **A4** When `start` resolves after a held answer: no second answer (the start went on unheard).
- **A5** When `start` fails before any answer: the failure is the function's own (`Err` with the same `HostError`),
  and nothing is answered.
- **A6** When `start` fails after a held answer: `Ok(())`, nothing more is answered; the failure reaches followers
  as the turn's end, which is the start road's business, not this function's.

## `start_at(workspaces, msg, origin, opens)` (answered-start.ts:25-54)

- **S1** `msg.workspace_id` present: answers `{ workspaceId, cwd?, setup? }` with `cwd` the message's own. When
  `opens` is true it reads `take_setup_line(workspace_id, origin)` first and `setup` is that line; when false,
  `take_setup_line` is never called and there is no `setup`. `folder_for` is never called.
- **S2** No `workspace_id`: one `folder_for(ask, origin)` call. `ask` carries `project`, `branch`, `cwd` only where
  the message has them, `picks` only where at least one pick is present, and `opens: true` only where `opens`.
- **S3** The picks are `harness`, `model`, `effort`, `access`, `permissionMode` where present, and `fast: true` only
  where `msg.fast == Some(true)`; `fast: Some(false)` writes no pick, and alone makes no `picks` at all.
- **S4** The answer is `{ workspaceId: found.workspace.id, cwd?, setup? }` from what `folder_for` found.
- **S5** A failure of either call is the function's own, unchanged.

## Tests (`tests/answered_start.rs`)

| Rule | Test |
|---|---|
| A1, A3 | `answers_once_with_the_handle_when_the_turn_starts`, `hands_no_on_held_when_the_caller_did_not_ask` |
| A3 | `carries_the_handles_notice_only_where_it_has_one`, `reads_the_view_after_the_start_resolved` |
| A2, A4 | `answers_held_at_once_and_not_again_when_the_turn_starts` |
| A5 | `a_failure_before_any_answer_is_the_callers` |
| A6 | `a_failure_after_a_held_answer_is_swallowed` |
| S1 | `a_named_workspace_answers_its_own_cwd_and_reads_no_folder`, `a_named_workspace_that_opens_takes_its_setup_line` |
| S2, S3 | `a_project_start_asks_the_folder_with_only_the_keys_named`, `fast_false_alone_names_no_picks`, `every_pick_named_rides_the_ask` |
| S4 | `answers_the_folder_found_with_its_cwd_and_setup` |
| S5 | `a_folder_refusal_is_the_callers` |

No TS test calls these functions directly; serve.test.ts and session-held.test.ts drive them over the socket. The
held answer's shape is pinned by session-held.test.ts ("answers held at once").
