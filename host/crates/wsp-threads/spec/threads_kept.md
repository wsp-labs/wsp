# threads_kept: agent processes kept between turns and launched ahead

TS: `T/threads.ts:98-301` at `65f6b89c7`. Rust: `src/threads/threads/kept.rs`, methods on `ThreadsArea`. State:
`kept_agents: RefCell<IndexMap<thread id, Rc<RefCell<KeptProcess>>>>` (wsp-core's `KeptProcess`) and
`host_writes: RefCell<IndexMap<harness session id, Joined>>`. "Still kept" means `Rc::ptr_eq` with the map's value
for that thread (`keptAgents.get(t) === kept`). Constants (protocol `wire/limits.ts:45-70`): `AGENT_KEEP_MS` 30 min,
`AGENTS_KEPT` 6, `KEPT_AGENT_MB` 400, `KEPT_MEMORY_SHARE` 0.25, `AGENT_WARM_MS` 3 min, `AGENTS_WARM` 2,
`INLINE_EXEC_MS` 20,000.

## Rules

K1. `kept_box(ws)` (`:132-137`): `None` when `ws` is not live or `workspacePlace(record)` is `None` (a workspace on
this computer or at a provider). Else `{place, cap: agentsKeptOn(memMb)}`, `memMb` read off
`ctx.folder_computer(place).shape.memMb`, 0 when the door or the computer is unknown. `agentsKeptOn(m) =
min(6, max(0, floor(m * 0.25 / 400)))`: 4,096 MB keeps 2, 8,192 keeps 5, 1,599 keeps 0, 16,384 keeps 6.

K2. `reap_kept(t, now)` (`:105-110`): nothing for a thread with no kept process; else removes it from the map, then
`end_kept`.

K3. `end_kept(t, kept, now)` (`:111-128`), in order: the keep's timer is cancelled; where `scopeDeviceId` is set,
`ctx.revoke_device(id)` runs detached and a failure warns `the token of thread <threadWord> was not taken away:
<message>`; then the close. The close is `kept.agent.close(now)` run detached, a failure warning `the kept agent of
thread <threadWord> did not end: <message>`. It runs at once when the host is closing, the workspace stands on no
joined computer, the host has no place door, or the door holds a link to that computer. Otherwise (a joined computer
that is away) it waits on the door's events: the first `place.present` for that place closes it; the first
`place.removed` for it drops the close for good; events of other places or types are ignored; the listener is removed
on the first of the two. The thread's own end (every process the thread left there) is never called here.

K4. `box_file_size(ws, at)` (`:140-151`): `None` when `ws` is not live. Runs on its machine, with
`timeoutMs: 20000`, exactly `for f in <q(folder)><"/*" repeated depth times>/<q(name)>; do [ -f "$f" ] && { wc -c <
"$f"; exit 0; }; done; echo -1` (`q` is `shellQuote`). Answers `parseInt(stdout.trim(), 10)` when the exit code is 0
and that is a safe integer (`"-1"` answers -1, `"12 bytes"` answers 12), `None` for any other exit, for text that
does not start with a number, and when the exec fails.

K5. `write_session(sid, write)` (`:177-199`): runs `write()`. When it answers `written`, each kept process on session
`sid` is stamped again: one whose `boxFile` and `agent.session_file()` are both set gets a fresh `box_file_size` read
as its `boxFile`, and the write's answer waits for those reads; any other gets `stampSessionFile(sessionFile,
kept.file.path)` as its `file` where that answers a stamp, leaving `file` alone where it answers `None`. Any answer
other than `written` stamps nothing. The caller gets the write's own answer or failure. `host_writes[sid]` holds a
barrier: it settles once the prior holder and this write (failure swallowed) have both settled, and is removed then
only if no later write replaced it.

K6. `kept_check(t)` (`:160-169`): nothing when the thread has no kept process or it stands on no joined computer.
When the door holds no link to that computer, `reap_kept(t)` and stop. When the process holds a session, await
`host_writes[session]` (if any). Then stop if the process is no longer the kept one, has no `boxFile` or no session
file, or is `working()`. Else await the stored `boxFile` and a fresh `box_file_size` together; reap only when both are
`Some`, they differ, and the process is still the kept one.

K7. `take_kept(t, launch, session, whole)` (`:206-218`), synchronous: `None` for no kept process. `asked` is
`launch` for a process with no `warm`, else `whole`. It matches when `asked` is set, `launchesAs(kept.launch, asked)`,
and for a warm one also `launchesAs(asked, kept.launch)`. A process that does not match, holds another `session`
(`None` and `Some` differ), or is not working and whose session file no longer stands as stamped
(`!sameSessionFile(sessionFile, file)`), is reaped and `None` answers. Else it leaves the map, its timer is cancelled,
and it is answered with its device and agent untouched.

K8. `hold_kept(t, o)` (`:224-235`): when the host is closing or `t` has no thread record, `o.agent.close(false)` runs
detached with its failure dropped silently, and `false` answers (the caller keeps the token). Else any kept process of
`t` is reaped first; a workspace on a joined computer gets `boxFile = box_file_size(..)` (only where the agent names a
session file) and no `file`; any other gets `file = stampSessionFile(sessionFile, None)`; `usedAt = now`; `keep` for
`AGENT_KEEP_MS`; `true` answers.

K9. `hold_warm(t, o)` (`:240-252`): when closing, `end_kept` at once (device revoked, agent closed). Else `keep` for
`AGENT_WARM_MS`, `ctx.proc_scopes_moved()`, then while more than `AGENTS_WARM` processes are warm and unclaimed
across every workspace, the one with the oldest `usedAt` is reaped (on a tie the first in map order). A warm process
counts against its computer's kept cap too (K12).

K10. `warm_on(ws, harness)` (`:254-255`): the first kept process in map order on `ws` that is warm for `harness` and
unclaimed, with its thread id.

K11. `rewarm(t)` (`:257-263`): nothing for no kept process; else cancels its timer, `usedAt = now`, and arms it for
`AGENT_WARM_MS` again. It does not check that the process is warm.

K12. `claim_warm(ws, harness)` (`:266-272`): `warm_on`; none answers `None`; else marks it claimed, calls
`ctx.proc_scopes_moved()` and answers its thread id. A claimed process is never answered again.

K13. The keep's timer (`arm`, `:274-280`): at `ms` it does nothing when the process is no longer the kept one; re-arms
for the same `ms` while `working()`; else reaps it.

K14. `keep(t, kept, ms)` (`:281-301`), synchronous: arms the timer, then puts it in the map (a thread already holding
one keeps its map position and the old one is dropped without a close: only `hold_warm` can reach that). When the
agent's `exited()` settles while it is still the kept one: it leaves the map, its timer is cancelled and its device is
revoked with the failure dropped silently; nothing is closed. Then the cap of the computer it stands on: every kept
process whose `kept_box(..).place` equals this one's (`None` equals `None`, so this computer's and every provider
workspace's processes share one count) counts. While the count is over `cap` (`AGENTS_KEPT` where there is no box),
the idle one (not `working()`) with the oldest `usedAt` is reaped; when every one is working the count stays over.

## Tests (`tests/threads_kept.rs`)

| Rule | Test |
|---|---|
| K1 | `kept_box_caps_a_joined_computer_off_its_memory`, `kept_box_is_none_here_and_at_a_provider` |
| K2, K3 | `reap_kept_cancels_revokes_and_closes_now`, `end_kept_waits_for_an_away_computer_and_closes_on_present`, `end_kept_never_closes_after_the_computer_is_removed`, `end_kept_closes_at_once_while_closing`, `end_kept_warns_when_the_revoke_or_close_fails` |
| K4 | `box_file_size_sends_the_one_shell_line_and_reads_its_answer`, `box_file_size_is_none_on_a_failed_or_unparsable_answer` |
| K5 | `write_session_restamps_every_process_on_the_session_once_written`, `write_session_stamps_nothing_on_a_failed_write`, `host_writes_holds_until_every_write_settled` |
| K6 | `kept_check_reaps_when_the_link_is_gone`, `kept_check_reaps_when_the_file_there_changed`, `kept_check_keeps_a_working_process_and_an_unread_size` |
| K7 | `take_kept_answers_a_matching_process_untouched`, `take_kept_reaps_on_another_launch_session_or_file`, `take_kept_matches_a_warm_process_both_ways` |
| K8 | `hold_kept_refuses_while_closing_or_with_no_record`, `hold_kept_replaces_the_threads_older_process`, `hold_kept_reads_the_file_size_on_a_joined_computer` |
| K9, K10, K12 | `hold_warm_keeps_two_unclaimed_and_reaps_the_oldest`, `claim_warm_takes_one_once`, `hold_warm_ends_at_once_while_closing` |
| K11 | `rewarm_starts_the_window_over` |
| K13 | `the_keep_ends_an_idle_process_at_its_window_and_re_arms_a_working_one` |
| K14 | `a_process_that_exits_leaves_the_keep_with_its_device`, `the_cap_reaps_the_idle_one_kept_longest`, `the_cap_counts_this_computer_and_providers_together`, `the_cap_stays_over_when_every_process_works` |
