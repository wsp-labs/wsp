# sessions_start: `threads/sessions.ts:255-475, 1486-1496`

The checks a start makes before it holds the thread's row: one sent again, the thread it lands in, who may send,
the agent it runs on, the notify targets, and the launch's token and adapter. `prepare_start` answers either a
start that is already answered (a retry, a queued hold) or a `Prepared` that sessions_launch carries to the launch.
Every refusal here comes before anything is held, except K16-K19's after the mint, which hand the token back.

## Rules

- **S1** After `ready`; `prefs = preferences_held() ?? preferences()`.
- **S2** A start sent again: a request id the workspace's index has taken, on a thread the caller may send to,
  answers `taken_turn(ws, taken)` where that answers, starting nothing.
- **S3** `fork_opened` rewrites the options and may hand a fork plan.
- **S4** The thread named: its latest row's workspace, else its record's, else (where the transcript names a session
  for it) this workspace. Another workspace: `"no thread <id> in this folder"` (no kind).
- **S5** An outside conversation with a thread named: `refusal(RESUME_A_THREAD_LINE, RESUME_A_THREAD_FIX, usage)`.
- **S6** `resume`: the latest row's session, else the transcript's, else an outside conversation's id where it is no
  copy.
- **S7** A warm agent is claimed only for a person's opening send that names its harness and nothing that changes
  the launch (no thread, no fork plan, no outside, no thread scope, no replaces, title, servers or asks-until-stopped).
- **S8** The thread id: the named thread, else `as.threadId`, else the claimed warm agent's, else the workspace's
  `opens_thread` where no record and no row hold it, else a fresh UUID.
- **S9** It opens a thread where no record and no row name it.
- **S10** A sub-machine (its record names a lead) opening a second thread: `sub_machine_taken_refusal(name, the
  thread it runs)`.
- **S11** Opening a thread on a workspace of the thread's repository the caller may not open in: a folder kind is
  refused by `elsewhere_refusal`, a machine by `away_for(.., "start", ..)`, where those answer.
- **S12** The entry: an opening reads `entry_of(ws, origin unless opens_in)`; a send reads `entry_of_row({thread,
  ws}, origin, "send")`; none: `"no thread <named> in this folder"`.
- **S13** `replaces`: only on an opening (else `refusal(RESTART_OPENS_LINE, RESTART_OPENS_FIX, usage)`), then
  `replaceable` (sessions_settle E6).
- **S14** A send from a thread on another computer into its tree carries words alone: attachments refused
  `send_files_across_line(from, to, thread)`, a follow `wait_across_line(...)`, both usage.
- **S15** A worktree removed by hand reads gone (`worktree_gone(entry, "removed", starting)`).
- **S16** The thread keeps its agent: a send naming another harness fails `thread_runs_on_line(carried, asked)`;
  a send drops its `permissionMode` and `access`. An outside conversation sent into a thread: S5's refusal.
- **S17** The harness: the thread's, else the outside's, else the asked one, else `default_agent_of(prefs, entry)`.
  An agent turned off: `agent_off_line(label, computer)`, usage. Then `confine_setup`.
- **S18** `refuse()`: a deleted workspace `deleted_before_start_line(name)`; `refuse_sub_send`; a released
  sub-machine passes; else `send_refusal(state, gone, name)` where it answers. Read after an unchecked running
  machine is proven.
- **S19** A thread not running: `copy_blocked`, `worktree_mounted`, `kept_check(thread)`. A blank title:
  `EMPTY_TITLE_LINE`. `spawn_guard("thread_new" or "send", origin)`.
- **S20** The tree: an opening by a thread hangs under that thread's root (`tree_of(scope)`); a person's restart of a
  child stands where the replaced thread stood; a send reads the thread's own parent and root.
- **S21** Notify targets: `me` from a turn's token is the caller's thread; duplicates collapse, first kept. Each
  other target must be a thread the caller reaches (`"no thread <t> to notify"`), not this thread (`"a thread cannot
  notify itself"`), and not one whose chain already reaches this thread (`"thread <t8> already notifies this
  thread; a cycle would run forever"`). Targets absent: the registered ones, with their `by` and road; named: the
  caller's scope and road (invariant 3).
- **S22** A released sub-thread is resumed (`resume_sub`): the entry may move, the prompt gains the preamble, the
  notices join as the start's notice.
- **S23** Clouds told (launch L3-L6) for a folder workspace that is no sub-machine.
- **S24** A workspace with a queue: `hold_queued(...)` answers the start.
- **S25** After the mint (`thread_launch`): a harness with no adapter, a fork the agent cannot make
  (`forks_not_line`, usage; or `fork_refused_line(agent, ASIDE_NO_SESSION_LINE)`, conflict), files or servers the
  harness refuses (`files_blocked`, `mcp_servers_blocked`): each revokes the scoped device and fails.
- **S26** The wrapper: any failure of a start reaps the warm agent it claimed (if still warm) and arms a sub-machine's
  done (`sub_done`), then fails.

## Tests (`tests/sessions_start.rs`)

| Rule | Test |
|---|---|
| S2 | `a_start_sent_again_answers_the_turn_it_opened_and_starts_nothing` (serve-replay.test.ts) |
| S4, S12 | `a_thread_of_another_workspace_is_no_thread_in_this_folder` |
| S5, S16 | `an_outside_conversation_opens_a_thread_only` (conversations.test.ts) |
| S7, S8, S26 | `an_opening_send_claims_the_warm_agent` (thread-warm-agent.test.ts:82), `a_refused_start_reaps_the_agent_it_claimed` |
| S10 | `a_sub_machine_runs_one_thread` (sub-spawn.test.ts) |
| S13 | `a_restart_only_opens`, `a_restart_of_a_busy_thread_is_refused` (replaces.test.ts) |
| S14 | `a_send_across_computers_carries_no_files_and_no_follow` (box-lead.test.ts) |
| S16, S17 | `a_thread_keeps_its_agent`, `a_send_drops_its_access`, `an_agent_turned_off_is_refused` |
| S18 | `a_deleted_workspace_refuses_the_start`, `a_napped_workspace_refuses_in_its_words` |
| S19 | `a_blank_title_is_refused` |
| S20, S21 | `invariant_3_the_tree_and_notify_by_come_from_the_callers_token` (spawn-guard.test.ts, runtime-notify.test.ts), `notify_refuses_itself_a_stranger_and_a_cycle`, `notify_me_is_the_callers_thread` |
| S24 | `a_queued_workspace_holds_the_start` (sub-queue.test.ts) |
| S25 | `a_refusal_after_the_mint_revokes_the_device` |
