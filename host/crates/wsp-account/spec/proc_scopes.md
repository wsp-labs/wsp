# proc_scopes: the threads named to a daemon (`account/proc-scopes.ts`, 210 lines)

The host tells a computer's daemon which thread owns which processes (`proc.scopes`), so the daemon stamps an owner
on every `proc.watch` row. On this computer a scope is a thread's agent pids, its turns' process groups, what the host
started for it, its terminals; on a joined computer it is the thread's cgroup. TS tests: `test/proc-scopes.test.ts`,
8 cases, all ported (`tests/proc_scopes.rs`), the two "shell a thread's pane opened" cases at the seam with the
channels area (they need `channels_ptys`' `rootedPorts`, `heldToOwner` and `threadTagged`, and are ported there as
`proc_scopes_terminal_*` in `tests/channels_ptys.rs`).

## Interface

`ProcScopesArea::new(ctx: Rc<dyn ProcScopesCtx>)` with `own_process(thread, pid, kind) -> Release`,
`proc_scopes_moved()`, `scopes_here() -> Vec<ProcScope>`, `scopes_on(place) -> Vec<ProcScope>`,
`scoped_procs(scopes, channel) -> Rc<dyn DaemonChannel>`.

`ProcScopesCtx`: `turn_groups()` (workspace → group pid → thread, map order), `group_exists(pid)` (the
`local_exec::group_exists` probe; a fake in tests, no real process group), `sessions()` (pid, status, thread,
workspace, `startedAt` per row, map order), `kept_agents()` (thread → workspace, pid, warm claimed), `latest_on(thread)`
(workspace, cwd), `thread_record_workspace(thread)`, `thread_records()`, `thread_terminals()`, `live_record(id)`
(kind, place), `checkout_of(record)`, `shared_daemon(kind)`, `has_local_road()`, `channel_here().await` (the
`channelOver(localRoad(), THIS_COMPUTER, () => {})` dial), `spawn(fut)`.

## Rules

- P1 `scopesHere` (92-145) builds one draft per thread, in first-seen order over: each turn group that still exists
  (`group_exists`; a gone group names nothing), each running session row with a pid (`agents`), each kept agent with
  a pid or an unclaimed warm one (`warm`), each owned pid (its workspace off `latestOn` then the thread record), each
  terminal of a local workspace (`kind: "terminal"`). Then each workspace of a kind with a shared daemon names its
  newest session's thread (by `startedAt`, missing 0; a later equal start does not replace) as a draft that is not
  live. A draft's workspace is the first one given; `live` once any source is live.
- P2 Order and caps (131-144): live drafts first, stable otherwise; at most `PROC_SCOPES_MAX` = 256 scopes; `agents`,
  `groups` and `pids` each cut to `PROC_SCOPE_PIDS_MAX` = 64, in insertion order (a pid is in `agents` once).
- P3 A scope's keys in this order, each only when present: `thread`, `workspace`, `agents`, `groups`, `folder`,
  `warm: true`, `pids`. `folder` is the thread's latest cwd, else the workspace's checkout; nothing where the workspace
  is not held. Test `p1_p3_scopes_here_carry_agents_groups_pids_and_folders` (the TS case at test:72),
  `p2_scopes_here_caps_scopes_and_pids`, `p2_live_threads_go_first`.
- P4 A warm agent is named with `warm: true` until a send claims it, then not at all (test:113). Test
  `p4_a_warm_agent_is_named_until_claimed`.
- P5 `ownProcess` (74-82) names the pid at once and moves the scopes; its release takes it away and moves them only
  while the pid is still that thread's (a pid since given to another thread is left). Test
  `p5_a_release_takes_only_its_own_pid`.
- P6 `scopesOn(place)` (149-172): every thread of every workspace whose record's `place` is that place, by session
  rows then thread records (at 0) then terminals (at 0), the newest start per thread kept; newest first, at most 256;
  each `{ thread, workspace, folder, cgroup: threadCgroup(thread), pids? }`, `pids` the thread's terminals there cut to
  64. Test `p6_scopes_on_a_box_newest_first` (test:153), `p6_terminals_on_a_box`.
- P7 `scopedProcs` (176-207): a `proc.watch` sends `{ id: null, op: "proc.scopes", scopes }` first (always, even when
  unchanged since the last watch), then the watch. While watching, a move sends the scopes again only when their JSON
  changed. `proc.unwatch` stops it. A refused `proc.scopes` is ignored. Its push leaves at the channel's close or when
  `closed` settles. Test `p7_named_before_a_watch_and_as_they_move` (test:89).
- P8 The standing channel here (36-72): while any scope here has `agents`, `groups` or `pids`, one channel to this
  computer's daemon is held and told the scopes whenever their JSON moves (`{ id: null, op: "proc.scopes", scopes }`).
  When none has, the held channel is told the scopes once more (a list whose scopes carry none of those, often `[]`)
  and closed. A dial that fails, or a channel that closes, lets the next move dial again. A host with no daemon road
  here never dials (the dial fails and is dropped). Tests `p8_this_computers_daemon_is_told_while_a_root_stands`
  (test:125), `p8_never_dialled_without_a_daemon_here` (test:146), `p8_a_failed_dial_dials_again`.

## Null and empty inputs

No sessions, groups, agents or terminals: `scopesHere()` is `[]` and nothing is dialled. `scopesOn` of a place
holding no workspace is `[]`.

## What survives a restart

Nothing: `owned`, the pushes and the standing channel are memory; a restarted host names its threads afresh.

## Tests

`tests/proc_scopes.rs` (14 tests) and, for the two cases that need the channels area, two tests in
`tests/channels_ptys.rs`. The six TS cases that need one area keep the literals of `test/proc-scopes.test.ts` (real
`sleep` leaders become fixed pids behind `group_exists`). What that file leaves open is
`tests/fixtures/oracle/proc_scopes.json`, written by `oracle/proc_scopes.oracle.ts`: each world beside what the TS
answered.

| Rule | Tests |
|---|---|
| P1, P3 | `p1_p3_scopes_here_carry_agents_groups_pids_and_folders`, `p1_sources_in_first_seen_order_and_the_newest_thread_of_each_folder`, `p3_a_workspace_this_host_does_not_hold_has_no_folder` |
| P2 | `p2_scopes_here_caps_scopes_and_pids`, `p2_live_threads_go_first` |
| P4 | `p4_a_warm_agent_is_named_until_claimed` |
| P5 | `p5_a_release_takes_only_its_own_pid` |
| P6 | `p6_scopes_on_a_box_newest_first`, `p6_terminals_on_a_box` |
| P7 | `p7_named_before_a_watch_and_as_they_move`, `p7_a_refused_scopes_frame_is_ignored_and_the_push_leaves_at_close_and_at_the_end` |
| P8 | `p8_this_computers_daemon_is_told_while_a_root_stands`, `p8_never_dialled_without_a_daemon_here`, `p8_a_failed_dial_dials_again` |

| TS case (`packages/runtime/test/proc-scopes.test.ts`) | Test |
|---|---|
| :72 | `p1_p3_scopes_here_carry_agents_groups_pids_and_folders` |
| :90 | `p7_named_before_a_watch_and_as_they_move` |
| :113 | `p4_a_warm_agent_is_named_until_claimed` |
| :124 | `p8_this_computers_daemon_is_told_while_a_root_stands` |
| :147 | `p8_never_dialled_without_a_daemon_here` |
| :156 | `p6_scopes_on_a_box_newest_first` |
| :195 | `proc_scopes_terminal_follows_its_thread_and_a_watching_channel_hears_each_move` (channels_ptys.rs) |
| :211 | `proc_scopes_terminal_on_a_box_is_named_to_the_boxs_daemon_never_this_computers` (channels_ptys.rs) |

Fields the implementation needs that `ProcScopesArea` does not hold yet: `standing` (the standing channel's shared dial
future, a token telling it from a later one, and the JSON last told), which `proc_scopes_moved` and its close handler
read and reset.
