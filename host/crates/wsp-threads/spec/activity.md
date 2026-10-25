# activity: `activity.ts` (424 lines)

`activity_door(ctx)` answers `read`, `end` and `meet` (activity.ts:97-424): this computer's processes and a box's,
by the thread that started them, and the End of one here by the pid, start and identity it was listed with. Each
read opens its own channel, takes one snapshot and closes it. No state of its own; nothing survives a restart.
The view math (`activity_of`, `end_plan`, `end_heads`, `processes_left`, `processes_left_refusal`) is wsp-wire's and
wsp-words' and is not this unit's; the tests compare the views they build whole.

## Constants

`BOX_END_POLL_MS = 200`, `SNAPSHOT_WAIT_MS = 10_000`, `NO_DAEMON_HERE = "this host runs no daemon on this computer,
so it reads no processes"` (activity.ts:63-68).

## Which computer (activity.ts:99-113)

- **AC1** This computer's name is the door's name for `HERE_PLACE_ID`, else `THIS_COMPUTER`. No computer named,
  `HERE_PLACE_ID`, `THIS_COMPUTER`, or this computer's name in any case: this computer.
- **AC2** Otherwise the door's rows (no door: none), matched by id or by name in any case. No match:
  `usage_refusal(no_such_place_refusal(computer, every row's name), "Name one wsp computers lists.")`. A row this
  host has not joined: `usage_refusal(not_read_line(name), NOT_READ_FIX)`. Else that box, by its row's name.
- **AC3** An End names this computer only: a box is `usage_refusal(box_end_line(name), BOX_END_FIX)`, signalling
  nothing.

## One snapshot (activity.ts:116-156)

- **AC4** This computer's channel: none where the host runs no daemon here (`NO_DAEMON_HERE`, no kind).
- **AC5** Unless `ports: false`, `ports.watch` is asked first; its `ports` rows with a numeric `pid` and `port` are
  grouped by pid, each list sorted ascending; a non-array answer is no ports.
- **AC6** `once`: `proc.list` is asked; its rows are the snapshot. A refusal whose message is exactly
  `unknown_op_line("proc.list")` falls back to the watch; any other failure is the read's.
- **AC7** `watch: false` and no list: fails `unknown_op_line("proc.list")`.
- **AC8** Otherwise `proc.watch` is asked and the first `proc.snapshot` event that parses is the snapshot; none
  within `SNAPSHOT_WAIT_MS` fails `"<THIS_COMPUTER>'s daemon sent no processes in 10 s"`. Events of other types and
  snapshots that do not parse are ignored.
- **AC9** The channel is closed once the work ends, whichever way (`over_channel`).

## This computer's scopes (activity.ts:168-196)

- **AC10** From the rows of local workspaces with a thread: the running threads; for each running row with a `run`,
  `agent_pid_of(run)` where it reads one; each thread's latest workspace by `startedAt` (missing reads 0, `>=`).
- **AC11** One scope per `ctx.scopes_here()` entry that is not `warm`: `{ threadId, title, workspaceId, roots,
  kept?, agents?, bin? }`, `title` the thread's facts' title else `thread_word(id)`, `roots` the unique union of its
  agents then its groups in that order, `workspaceId` the entry's own else the latest row's else `""`, `kept` the
  kept agent's pid only while the thread runs no turn, `bin` the catalog agent's bin for the thread's harness.
- **AC12** Then every other thread a local row names, with `roots: []`.
- **AC13** Running threads first, then newest started first; equal keys keep their order.
- **AC14** A read of this computer: `{ computer, at, procs, daemon, host: ctx.host_pid(), scopes, agentBins: every
  catalog agent's bin, ports }`.

## Who reaches what (activity.ts:206-218, 273-282)

- **AC15** No thread: only a caller with no thread scope, on a computer it drives (`drives({kind}, caller)`, kind
  `local` here, `place` on a box).
- **AC16** A thread: its workspace record (by its record, else its latest row) must pass `refusal_for` (no record:
  `drives({kind})`), and the caller must settle the thread (`settles_thread`).
- **AC17** A thread's box: its workspace is not local, names a place, and that place is joined. `elsewhere`: its
  record exists and is not local. Unjoined box: the record's kind keeps folders on a joined computer
  (`folder_on_joined`), names a place, and the place is not joined.

## A box (activity.ts:222-271)

- **AC18** A box with no door or no link: `"<name> is away"`. Its read is `once` with `watch: false` over the
  door's channel; a box with no one-shot list is `usage_refusal(box_behind_activity_line(name),
  BOX_BEHIND_ACTIVITY_FIX)`. Scopes are the threads asked, each `{ threadId, title, workspaceId, roots: [],
  cgroup: thread_cgroup(id), bin? }`; `host: 0`; `cgroups` only where the snapshot has them.
- **AC19** A quiet box read (for settle and delete) answers none on any failure and warns
  `"<name>'s processes were not read for a settle or a delete: <message>"`.
- **AC20** A box's threads, unnamed: every thread a row puts on it, newest first by its latest `startedAt`.
- **AC21** A box thread's end: the kind's `end_thread(entry, thread, {})`; then `pty.kill` for each pty of a listed
  process (same pid, start within `SAME_START_MS`), one at a time, a failure warned `"shell <pty> of thread <word>
  was not ended: <message>"`; then `slates.close_servers(thread)`; then quiet reads every `BOX_END_POLL_MS` until no
  listed process stands (a failed read counts every one standing) or `END_GRACE_MS` passed. Answers `{ pid: first
  process's, ended: every listed process, forced: [], left }`.

## `read` (activity.ts:286-313, 358-360)

- **AC22** After `ready`. Each thread asked (`thread` or `threadIds`) with no record and no facts, or not reached:
  `not_found_refusal("no thread <word>")`.
- **AC23** A named thread on an unjoined box: `usage_refusal(not_joined_activity_line(thread, name),
  not_joined_activity_fix(name))`; on a cloud machine: `usage_refusal(cloud_activity_line(thread),
  CLOUD_ACTIVITY_FIX)`.
- **AC24** A named thread with no computer named reads its box; a computer named that is not the thread's:
  `not_found_refusal("no thread <word> on <name>")`.
- **AC25** The threads kept are those asked that are on the computer read, each reached; with none asked, every
  reached thread. `wsp` and `unowned` are emptied unless nothing was asked and the caller reaches threadless rows.

## `end` (activity.ts:337-365)

- **AC26** After `ready`, on this computer only (AC3), off a fresh read (not `once`). The plan is made on that read
  before any signal (`end_plan` with AC16 for each thread). A kept agent's plan reaps the kept agent now
  (`reap_kept(thread, now)`) and answers `{ pid, ended: that thread's agent rows, forced: [], left: [], freed: thread }`,
  signalling nothing.
- **AC27** Otherwise one `proc.kill` of the plan's first target: `{ op, pid, signal: "TERM", mode: "tree",
  startedAt, identity? }` in that key order, over a channel guarded by `kill_guarded(.., true)`. A refusal coded
  `not-found` reads as nothing signalled; any other is the End's. A reply with no `signalled` names the pid alone
  (a daemon before kill modes). The answer's `ended` is the plan's rows the daemon signalled, each the read's row
  for it (or one built off the snapshot entry); `forced` and `left` deduplicated.

## `meet` (activity.ts:367-421)

- **AC28** `keep`: nothing read, `[]`.
- **AC29** The threads not elsewhere are read here once (`once`); each box thread must be reached (else
  `not_found_refusal`) and each box is read quietly once for all its threads. Box threads with processes left carry
  `on: <box name>`.
- **AC30** Nothing left: `[]`. Every head's plan is made before anything is asked or signalled; a head whose plan
  meets a thread the caller does not reach is the person's (`theirs`); a kept agent's head fails
  `"<pid> is the agent thread <word> keeps, which a settle frees itself"`.
- **AC31** `ask`: `processes_left_refusal(verb, left, theirs)`. `stop` with a head of the person's: the first such
  refusal, before any signal.
- **AC32** `stop`: every local head killed in one go (all `proc.kill`s sent at once), then each box thread ended in
  turn (AC21). Answers the ends in that order.

## Tests (`tests/activity.rs`)

Ported at the seam: the TS drives the command line over a fake daemon (host/test/activity.test.ts, 24 cases) and the
runtime over a fake box (box-activity.test.ts, 7). Here `FakeHost`'s daemon channel answers the same ops from the
same process table and records every frame.

| Rule | Test |
|---|---|
| AC10-AC14, AC25 | `lists_a_threads_tree_then_wsps_own_and_leaves_out_what_nobody_started` (activity:48), `lists_the_newest_thread_first` (:93), `a_relayed_caller_reads_nothing_outside_a_thread` (:374) |
| AC5 | `ports_are_grouped_by_pid_and_sorted` |
| AC6-AC8 | `a_daemon_with_no_list_is_watched_instead` (:363), `no_snapshot_in_ten_seconds_fails_the_read`, `a_box_with_no_list_is_refused_in_the_usage_class` (box-activity:71) |
| AC9 | `the_channel_is_closed_after_a_failed_read` |
| AC1-AC3 | `a_computer_this_host_does_not_hold_is_refused` (:395), `an_end_on_a_box_signals_nothing` (box-activity:64), `this_computers_name_in_any_case_is_here` |
| AC4 | `a_host_with_no_daemon_here_reads_nothing` |
| AC26 | `end_frees_a_kept_agent_and_signals_nothing` , `end_refuses_the_agent_and_sends_nothing` (:106), `end_refuses_a_pid_that_names_another_process_now` (:115), `end_refuses_a_process_wsp_did_not_start` (:123) |
| AC27 | `end_takes_the_process_and_its_tree_in_one_kill` (:133), `end_signals_through_the_guard` (:148), `end_names_what_the_daemon_forced` (:159), `end_signals_nothing_when_the_pid_holds_another_process` (:173, :186), `end_on_a_daemon_before_kill_modes_names_the_pid_alone` (:215) |
| AC28-AC32 | `settle_stop_ends_every_tree_in_one_go` (:225), `settle_ask_refuses_naming_what_outlives_the_agent` (:240), `delete_ask_refuses_and_keep_reads_nothing` (:264), `a_shell_the_threads_pane_opened_is_asked_about_and_stopped` (:287, :301, :315), `a_thread_beside_it_keeps_its_own_processes` (:324), `a_thread_settling_itself_hears_which_processes_only_the_person_ends` (:197) |
| AC18, AC20 | `lists_a_boxs_threads_by_their_cgroups` (box-activity:35), `totals_a_box_thread_by_its_cgroup` (:47), `reads_a_thread_alone_on_its_box` (:56) |
| AC23 | `names_the_unjoined_box_a_thread_runs_on` (:85), `says_cloud_machine_for_a_provider_this_host_does_not_fork_on` (:96) |
| AC19, AC21 | `a_box_end_kills_each_listed_shell_and_waits_out_the_grace`, `a_quiet_box_read_that_fails_is_warned_and_holds_nothing` |
