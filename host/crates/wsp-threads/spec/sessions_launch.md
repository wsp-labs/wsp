# sessions_launch: `threads/sessions.ts:476-895`

From a `Prepared` start (sessions_start) to a running turn: the thread's row held before anything is asked of the
machine, the one-turn-at-a-time loop, the landing, the picks, and the handover to `run_turn`. The whole TS body is
one `try` with a `finally` that undoes whatever a start that never reached its machine left (Z1-Z6).

## The row (sessions.ts:476-523)

- **L1** The turn id is `as.turnId`, else a fresh UUID. The view: `{ id: turnId, workspaceId, harness, status:
  running, startedBy (asked, else person), threadId, parentThreadId?, rootThreadId?, attempt?, prompt:
  kept_opening(prompt), startedAt: now, harnessTitle and titleSource (a person's title; else an outside
  conversation's title as auto; else carried_title(thread)), claudeSessionId? (resume), contextWindow? }`.
- **L2** The lender: a caller's running thread waits on this one (a follow, or a start answered only at launch) and
  is not this thread: its running turn's id.
- **L3** `hold()`: once, and only while the thread runs no turn. The resume re-read off the thread's latest row;
  `cap_hold(record, turn, lender)`; the row put at the turn id `{ view, turnId, launch, replaces?, notify?,
  notifyBy?, notifyRoad? }`; `session.held { workspaceId, threadId, requestId? }` emitted. Held at once where it can.

## Saying it starts (sessions.ts:524-544)

- **L4** `session.starting { workspaceId, threadId, harness, installs? (only true), requestId? }` goes out at most
  once: at the launch of a new process, or `AGENT_STARTING_MS` after the start with none. Hushed by this turn's
  `session.start` or `session.end`, by a cap hold, by leaving, and never said for a queued send.

## The loop (sessions.ts:569-741)

- **L5** Before it: `launch_picks`; `picks_for(resume, or none for an outside)` (a refused pick fails here); the
  thread folder; a local workspace whose named cwd does not exist: `no_cwd_line(home_shortened(folder))`, usage;
  `limit_details_due`; an end owed by the kind (only where no stop end stands) holds the next turn on its computer
  (sessions_stop K1).
- **L6** No running turn, and a stop's end stands: `session.queued { workspaceId, threadId, harness, prompt,
  requestId?, waitsFor? }` once for a first wait, and once more when the wait turns to a computer that is away; the
  outcome becomes `queued`. Waiting on a computer: the send holds the row (L3), the row says `waitsFor`, and the send
  waits for the end or for the row holder's launch; a stop on it gives it up with `send_given_up_line(box)`. Then
  `refuse()` again and every check again.
- **L7** No running turn, another send's turn launching: wait for its launch (lending the cap slot where this send
  lends one); a steering harness stays `started`, others go `queued` with one `session.queued`.
- **L8** The checks of sessions_start S19 run again once after any wait.
- **L9** A host write in flight on the resumed session (`host_writes`) is waited out.
- **L10** The cap: stop marks pending are waited; a stop mark fails the start with its words; a full computer
  (unless the send wakes its lead) holds the turn: hushed, `cap_wait(...)`, the caller told held once through
  `on_held({ view with capped, turnId, wait })`, then wait and look again. Let through once: `cap_left(turn)`.
- **L11** The landing, once: images (`land_images`) and files (`land_files`) under folders keyed by thread, request
  and a fresh UUID minted per send; a fork readies its session and copies images; a send resuming pays a cut owed
  (`pay_cut_owed`); the folder's snapshot is started (awaited unless the harness takes its prompt late); launch
  servers read. Then `refuse()` and look again.
- **L12** A running turn: a steering harness whose turn has not answered and takes these files steers: the message
  is kept with the turn first (`keep_steer`), then written. A write that threw on a harness that does not tell
  unread messages: `steer_lost` and the start fails with the write's message. Answered (`steer_answered`): the
  handle with outcome `steered`. Otherwise the send queues behind the turn (`session.queued` once), waiting for the
  turn's finish or a stop's wake.

## The handover (sessions.ts:742-848)

- **L13** `cwd = runs_in(entry, ran_in, folder)`. `o.behind` and `o.setup` each record a `session.behind` row.
- **L14** A worktree gone, and the folder moved: `session.moved { from, to, branch?, fresh? }`; `fresh` (an agent that
  keys sessions to the folder, resuming) drops the resume and the view's session.
- **L15** The picks for the session resumed land on the view with the cwd and the session.
- **L16** The launch key, where the kind keeps agents and no fork: `{ fixed: launch_fixed(...), picks }`; a kept
  process is taken (`take_kept`) matched on what was asked; taking one revokes this send's device.
- **L17** A fork's copied history is recorded first; an outside conversation's earlier rows next.
- **L18** `run_turn` with the view, the token (the kept process's else the minted one), the device, the keep, the
  opening (prompt, requestId?, via?, afterCut?, afterLimit?, opensThread?, title?, attachments?), the asked
  (prompt handed with files, effort?, typed? where the files changed it, requestId?, owed?), images dir, snapshot,
  resume, waiting box, and `open` (the kept agent's `next`, else `adapter.start` with resume or outside copy, fork,
  cwd, picks, title, images, servers, launch servers, limit details, prompt after, keep, version, and a seed through
  `seed_after_loss` where resuming).
- **L19** Handed over: `session.starting` unless a kept process; sent images kept (`keep_sent_images`).
- **L20** The thread record is made at its first turn: `{ workspaceId, harness, parent?, root?, permissionMode?,
  replaces?, forkedFrom?, worked: thread_ran(its ended rows) }`; then `clouds` set where told; a `limitResume` taken
  off with its timer; a files folder added to `filesIn` once.
- **L21** The launch settles. A restart settles the thread it replaces (failure warned `"thread <word> was not settled
  after its restart: <message>"`) and pushes its head. A new session remembers the target (warned `"last target for
  <ws> not remembered: <message>"`). The handle answers with the start's notice where one.

## A start that never reached its machine (sessions.ts:849-894)

- **Z1** Hushed; `cap_left(turn)`; the scoped device revoked (warned `"the token of thread <word> was not taken
  away: <message>"`).
- **Z2** Held, told held, failed, with targets, and not owed: the targets hear `notify_end(..., { status: failed,
  error })`.
- **Z3** Held: the row is removed. Failed and the thread not running: `session.end { sessionId: view.id, exitCode:
  null, sawResult: false, reason, at: now }` emitted on the bus alone (no transcript row); a sender told held whose
  start failed: the thread's latest row takes `failure: listed_failure(reason)` and is persisted.
- **Z4** The launch settles after the row is gone.
- **Z5** Landed images dropped; a fork's copy dropped (warned) and its sent images dropped.
- **Z6** A kept process taken for it is ended (`end_kept`).

## Tests (`tests/sessions_launch.rs`)

| Rule | Test |
|---|---|
| L1, L3 | `the_row_is_held_before_the_machine_is_asked` (session-held.test.ts), `a_send_into_a_running_thread_holds_no_second_row` |
| L4 | `starting_is_said_once_after_four_seconds_and_not_once_started` (session-opening.test.ts) |
| L6 | `a_send_behind_a_stops_owed_end_waits_and_says_the_computer` (owed-box.test.ts) |
| L7, L12 | `a_second_send_queues_behind_the_running_turn` (waiting-behind.test.ts), `a_steering_harness_steers_into_the_running_turn` (runtime-steer.test.ts), `a_lost_steer_falls_to_the_person` (steer-lines.test.ts) |
| L10 | `a_full_computer_holds_the_turn_and_tells_the_caller_once` (thread-cap.test.ts) |
| L11 | `images_land_in_a_folder_of_their_own_per_send` (session-images.test.ts) |
| L14 | `a_gone_worktree_moves_the_turn_and_says_so` (folder-threads.test.ts) |
| L16 | `a_turn_runs_on_the_threads_kept_process_at_the_same_launch` (thread-keeps-process.test.ts) |
| L18 | `the_launch_carries_picks_servers_and_seed` (session-mcp.test.ts, lost-session-line.test.ts) |
| L20 | `the_record_is_made_at_the_first_turn_only`, `files_folder_is_kept_once` |
| L21 | `a_restart_settles_the_thread_it_replaces` (replaces.test.ts) |
| Z1-Z6 | `a_start_that_fails_before_its_launch_leaves_no_row_and_ends_the_wait` (session-held.test.ts), `a_held_start_that_fails_tells_its_targets`, `a_failed_start_hands_its_token_back` |
