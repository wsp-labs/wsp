# thread_state
Source: `packages/protocol/src/thread-state.ts` (118 lines) and the functions of `packages/protocol/src/wsp-tools.ts` (93 lines; its constants, `SLATE_BRIEF` among them, are `wsp_wire::types::wsp_tools`) at a3b870a6e. Tests: no TS tests of their own; `tests/thread_state.rs` (4 cases, 185 rows, every value printed by the TS under node).

## What it does
The one vocabulary for a thread's state, read off its row: unseen ends, unread, needs you, the state word; when a read thread settles. And which wsp tool calls start or follow threads, and whether a launch carries the slate.

## Behaviour
1. `thread_unseen_at(t)`: None while running; else the latest of `endedAt` and `wokeAt` that is set and later than `readAt` (any, when readAt is unset); None when neither qualifies (thread-state.ts:16-20).
2. `thread_unread(t)`: completed or interrupted, with an unseen moment (:22-24). Failed is never "unread".
3. `thread_needs_you(t)`: asking or waitingOn set; or a sub-thread row held and not running; or an unseen moment, where an interrupted thread's `endedAt` does not count (only a later `wokeAt`) (:26-30).
4. `thread_state(t)`: Waiting when asking or waitingOn; else Done when unread; else the status (:32-35). Words: Needs you, Working, Idle (completed and interrupted), Failed, Done (:41-48). Subagent words: Working, Done, Failed, Stopped.
5. `waiting_line(t)`: the asking text; else `<asking_line(waitingOn.prompt)> needs an answer`; else None (:37-40). `needs_you_line(ask)`: `needs you: <asking_line(ask)>` (lower-cased Waiting word) (:59-61).
6. `thread_seen(f)`: no end, or a read at or after the end (:70). `settle_moment(f, settle_ms, held)`: None while asking or working; a hand settle at or after the latest of start and end answers `{ms: settledAt, byHand: true}` whatever held and the window; then None when held, no window, failed, no start or end, or unseen; else `{ms: max(last, readAt ?? last) + settle_ms, byHand: false}` (:75-82). `thread_settled`: a moment that is by hand or not after now (:71-74).
7. `spawns_thread(name)`: a wsp server tool named run or fork (wsp-tools.ts:16-19). `launch_has_slate(servers)`: the wsp server is listed and its noSlate is not true (:23-26).
8. `threads_followed(name, input)`: only wsp's send, run and threads_wait; input must parse as a JSON object (null and arrays refused by `typeof`, an array is an object in JS: `[1]` parses and has no field, so it follows nothing); `detach: true` (exactly true) follows nothing; run follows the thread it opens; send and threads_wait follow the non-empty strings of their field (one value or an array), None when none (:62-81).

## Edge cases
- A running thread is never unseen, even with an end and a wake.
- An interrupted thread that ended unseen needs nobody, but a later wake makes it need the person.
- A hand settle earlier than the latest end is ignored.
- `threads_followed` of `{"thread": "t1", "detach": "yes"}` follows t1 (detach must be true); `{"threads": ["a", "", 3, "b"]}` follows a and b.

## Behaviour nobody wrote down
- Rule 4.26: a followed list keeps the call's order.

## Tapes
`status.list` thread rows (state words), `sessions.settle`, the notify and wait lines that follow threads.

## Decisions
1. `ThreadState` is SessionStatus's four words plus waiting and done, serialized as the TS strings.
2. `threads_followed` takes the call's tool name and input text; `Followed` is Named or Opened.
3. `launch_has_slate` takes wsp-wire's `McpServerSpec` map, whose `noSlate` is `true` or absent; the TS test's `{noSlate: false}` is outside that type and not ported.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 to 5 | `thread_states::a_threads_state_reads_asking_waiting_and_unseen_ends` |
| 6 | `thread_states::a_read_thread_settles_after_the_window_unless_held_failed_or_unseen` |
| 7 | `wsp_tool_calls::a_spawn_is_wsps_run_or_fork_and_a_slate_rides_unless_turned_off` |
| 8 | `wsp_tool_calls::a_call_follows_the_threads_it_names_or_opens_unless_detached` |
