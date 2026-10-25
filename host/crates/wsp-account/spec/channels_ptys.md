# channels_ptys: whose shell is whose, the kill guard and the port roots (`account/channels.ts:280-580`)

The wrappers a client's channel into a daemon goes through: frame lists, the pty owners on a daemon several owners
dial, the kill guard, the thread tags of a workspace's shells (kept in the store), and the port roots of a local
workspace. TS tests: `test/channels.test.ts` (owners, tags, kill guard, roots), `test/proc-scopes.test.ts:176-217`
(a tagged shell named to its thread), `test/deleted-workspace-state.test.ts` (tags dropped with the workspace),
`test/serve-app-procs.test.ts` (the kill guard's lists).

## Rules

- Y1 The lists (282-299), exact and in order: `GUEST_EVENTS` = guest.opened, guest.message, guest.closed,
  tunnel.data, tunnel.end; `PTY_EVENTS` = pty.data, pty.exit, pty.mode; `COMPUTER_WATCHES` = ports.watch, sys.watch;
  `COMPUTER_PROCS` = proc.watch, proc.unwatch, proc.inspect, proc.kill, container.stop; `WORKSPACE_FRAMES` = the 22 of
  295; `GUEST_ROAD_FRAMES` = `WORKSPACE_FRAMES` then guest.watch, guest.reply, guest.close, ssh.start, tunnel.open,
  tunnel.write, tunnel.close. Test `y1_frame_lists`.
- Y2 `heldToOwner(owner, channel)` (312-337): one set of ptys per owner, shared by every channel of that owner. A
  frame naming a `ptyId` the owner did not open is answered `{ id, ok: false, error: "no such pty: <id>" }` without
  reaching the daemon. A created pty joins the set, a killed one leaves. A `pty.list` answers only the owner's rows,
  and drops from the set only the ids held when the list was asked that the daemon no longer lists. A refused reply
  passes whole and changes nothing. `sharedDaemonReplaced` (483-485) empties every owner's set. Tests
  `y2_an_owner_sees_and_drives_only_its_ptys`, `y2_a_list_drops_only_what_it_was_asked_over`,
  `y2_a_replaced_daemon_forgets_every_owner`.
- Y3 `killGuarded(channel, here)` (358-373): only `proc.kill` changes. Its `protect`, `apps` and `agents` keys are
  dropped whatever the page sent; `agentBins` (every catalog agent's `bin`, catalog order) is added last. On this
  computer the frame also carries, before `agentBins`: `protect: [host pid, host parent pid]`, `apps: appMain()`,
  `agents`: for every running session row with a pid, the pid then the agent pid its run recorded
  (`agentPidOf(run)`) where there is one. Key order: the frame's own keys less the three, then `protect`, `apps`,
  `agents`, `agentBins`. Test `y3_kill_guard_here_and_elsewhere`.
- Y4 `appMain` (340-351): no state path → `[]`; else the lock link `<stateFolder>/<APP_DATA_FOLDER>/<APP_LOCK_FILE>`
  read as `<hostname>-<pid>`; another host's name, an unreadable link or a pid that does not answer signal 0 → `[]`.
  Through the context: `read_link`, `hostname`, `pid_alive`. Test `y4_app_main_table`.
- Y5 Tags (378-436, 440-470): per workspace, pty id → thread, read once from the store's `pty-threads` document
  (a non-object, a failed read or a non-string value reads as no tag). A `pty.create` naming a string `thread` tags the
  pty the daemon answered; `pty.kill` drops its tag; `pty.list` drops every tag the daemon no longer lists and names
  each listed row's `thread` (appended last on that row). `pty.thread` is answered here, never by the daemon: it needs
  a string `ptyId` and `thread` (else `{ id, ok: false, error: "pty.thread names a ptyId and a thread" }`), retags,
  keeps, and moves the workspace's port roots. Tests `y5_a_create_tags_its_thread`, `y5_pty_thread_hands_a_shell_over`,
  `y5_a_list_names_and_drops`.
- Y6 Keeping tags (406-428): writes are one at a time per workspace, each writing the tags as they stand when it runs
  (`put` of the object, or `delete` when empty); a failed write warns `"whose shells are whose in <id> was not kept
  for the next host: <message>"` and the chain goes on. Test `y6_tag_writes_run_in_turn_last_wins`,
  `y6_a_failed_tag_write_warns_and_the_next_runs`.
- Y7 A `workspace.deleted` (429-436) sets the workspace's tags to empty, writes (which deletes the document), and once
  that last write settles forgets the workspace's tags and writes. A reply landing after the delete writes nothing to
  the store. Test `y7_deleted_workspace_tags_go_with_it` (deleted-workspace-state.test.ts), and **restart**:
  `y7_tags_survive_a_restart` (a second area over the same store reads the tags the first kept).
- Y8 `threadTerminals()` (398-404): for every pty a local workspace's channels opened (`portPids`), the thread its tag
  names; an untagged or `""`-tagged pty is left out, and only tags already read count. Test
  `y8_thread_terminals_from_tags_read`.
- Y9 `endThreadShells(entry, thread)` (472-481): every pty tagged to that thread is killed through `withDaemon` (a
  failure warns `"shell <id> of a deleted thread was not ended: <message>"`), its shell token ended (copy's
  `shellsEnded`), its tag dropped and kept. Test `y9_end_thread_shells`.
- Y10 Port roots (491-525): `turnGroups` holds, per workspace, each group a turn led and its thread.
  `portRootsOf(workspace)` is every group that still exists (a gone group is dropped from the map, and a workspace with
  none left is removed) plus every pty pid its channels opened, sorted ascending, once each. `portRootsMoved` reads
  them, calls every watcher of the workspace, then `procScopesMoved`. `armRootsRecheck` (495-504) sets one interval of
  `PORT_ROOTS_RECHECK_MS` = 5,000 ms (on `ctx.state`, so only one stands) that moves every workspace with groups and
  stops itself once none has. Tests `y10_port_roots_sorted_and_pruned`, `y10_recheck_runs_every_5s_and_stops`.
- Y11 `rootedPorts(workspace, folder, ptys, channel)` (529-578): a `ports.watch` forgets what it told, then sends the
  frame with `roots` (always the fresh roots: with nothing told, `fresh()` never answers unchanged, so the `?? []` is
  dead) and `folder` set after the frame's own keys (a key the frame had keeps its slot). While watching, a push sends `{ id:
  null, op: "ports.watch", roots, folder }` only when the roots' joined string changed. A created pty with a numeric
  `pid` joins `ptys` and moves the roots; a `pty.list` drops every pty not listed standing (`exited !== true`) and
  moves the roots when any went. The push leaves at close or when `closed` settles. Tests
  `y11_rooted_ports_watch_and_push`, `y11_a_list_drops_exited_ptys`.

## What survives a restart

The `pty-threads` documents (Y5 to Y7). The owners, port pids, turn groups and watchers are memory.

## Tests

`tests/channels_ptys.rs` (26 tests). Expected values are `tests/fixtures/oracle/channels_ptys.json`, written by
`oracle/channels_ptys.oracle.ts` (the TS area over a fake daemon and context; process ids, the host name and the
host's pids are fixed in the oracle so the fixture does not move). A spawned write or push is read only after
`settle()`; the TS's awaits give each tag write the turn before the next send, so the tests settle between sends.
JavaScript objects list integer keys ascending, so `turnGroups` is compared in that order.

| Rule | Tests |
|---|---|
| Y1 | `y1_frame_lists` |
| Y2 | `y2_an_owner_sees_and_drives_only_its_ptys`, `y2_a_list_drops_only_what_it_was_asked_over`, `y2_a_refused_reply_passes_whole_and_changes_nothing`, `y2_a_replaced_daemon_forgets_every_owner` |
| Y3 | `y3_kill_guard_here_and_elsewhere` |
| Y4 | `y4_app_main_table` |
| Y5 | `y5_a_create_tags_its_thread`, `y5_pty_thread_hands_a_shell_over`, `y5_a_list_names_and_drops`, `y5_a_bad_tag_document_reads_as_no_tags` |
| Y6 | `y6_tag_writes_run_in_turn_last_wins`, `y6_a_failed_tag_write_warns_and_the_next_runs` |
| Y7 | `y7_deleted_workspace_tags_go_with_it`, `y7_a_reply_after_the_delete_writes_nothing`, `y7_tags_survive_a_restart`, `channels_area_registers_its_two_deleted_listeners` |
| Y8 | `y8_thread_terminals_from_tags_read` |
| Y9 | `y9_end_thread_shells` (needs copy's `shells_ended`) |
| Y10 | `y10_port_roots_sorted_and_pruned`, `y10_recheck_runs_every_5s_and_stops` |
| Y11 | `y11_rooted_ports_watch_and_push`, `y11_a_list_drops_exited_ptys`, `y11_the_push_leaves_at_close_and_at_the_channels_end` |
| proc_scopes P1, P3 (terminals) | `proc_scopes_terminal_follows_its_thread_and_a_watching_channel_hears_each_move`, `proc_scopes_terminal_on_a_box_is_named_to_the_boxs_daemon_never_this_computers` |

TS cases ported (under `packages/runtime/test/`):

| TS case | Test |
|---|---|
| local-workspace.test.ts:262 | `y10_port_roots_sorted_and_pruned`, `y11_rooted_ports_watch_and_push` |
| local-workspace.test.ts:369, :453, :472 | `y2_an_owner_sees_and_drives_only_its_ptys` |
| local-workspace.test.ts:391 | `y5_a_create_tags_its_thread` (the TS case's "daemon never hears the thread" is copy's strip: here the thread reaches the inner channel) |
| local-workspace.test.ts:405 | `y5_pty_thread_hands_a_shell_over` |
| local-workspace.test.ts:419 | `y7_tags_survive_a_restart` |
| local-workspace.test.ts:437, box-settle.test.ts:172 | `y9_end_thread_shells` |
| local-workspace.test.ts:484 | `y2_a_replaced_daemon_forgets_every_owner` |
| local-workspace.test.ts:505 | `y2_a_list_drops_only_what_it_was_asked_over` |
| local-workspace.test.ts:520, :561 | `y10_recheck_runs_every_5s_and_stops` |
| local-workspace.test.ts:612, box-panes.test.ts:84 | `y3_kill_guard_here_and_elsewhere` |
| box-panels.test.ts:661 (roots part) | `y11_rooted_ports_watch_and_push` |
| deleted-workspace-state.test.ts:20, :38 | `y7_deleted_workspace_tags_go_with_it`, `y7_a_reply_after_the_delete_writes_nothing` |
| proc-scopes.test.ts:195, :211 | the two `proc_scopes_terminal_*` tests |

Fields the implementation needs that `ChannelsArea` does not hold yet (the note says the in-flight reads and writes are
private to ptys.rs, which Rust cannot do without a field): `tags_held` (a map of workspace to the shared read future and
a token telling it from the read that replaced it) and `tag_writes` (the same for the last queued write, which
`tag_write` answers). Add both to `ChannelsArea` when implementing.
