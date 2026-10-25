# box_panels: the panes of a folder on a joined computer (`account/box-panels.ts`, 263 lines)

A pane's channel into a folder on a computer the person joined answers nine ops itself (`PANEL_OPS`, 37): the
ports, readings and process watches, a process inspect, kill and container stop, and a terminal's create; every other
op passes to that computer's link (`served`) untouched. Expected values: `oracle/box_panels.oracle.ts` drives the
real opener through scripted pane sessions against a fake context and a recording daemon,
`tests/fixtures/oracle/box_panels.json`. TS tests: `test/box-panels.test.ts` (37 cases over a whole host and a fake
computer); the cases decided here are ported as the sessions below.

## Interface

`proc_watch_scope(asks) -> Option<Vec<String>>`, `named_watch_refusal(name, daemon_version) -> Option<Value>`,
`BoxPanelsOpener::new(ctx: Rc<dyn BoxPanelsCtx>, watchers, placements)` (the two shared maps are
`Rc<RefCell<IndexMap<..>>>` the channels area owns) and `opener.open(entry, place_id, served, folder) -> BoxPanels`
with `answers(op)`, `send(frame).await`, `heard(event)`, `close()`. `served` is `Rc<dyn Fn(Map) -> LocalBoxFuture<Map>>`.

`BoxPanelsCtx`: `on_workspace_deleted(f)`, `sessions_views()` (each row's workspace, thread and `startedAt`),
`thread_records()` (thread id → workspace id, in map order), `link(place_id)` (an `Rc` handle or none),
`name_of(place_id)`, `report_daemon_version(place_id).await`, `watch_sys(place_id).await -> Result<release>`,
`scopes_on(place_id)` (the proc scopes area's), `thread_env(entry)`, `spawn(fut)`.

## Rules

- B1 `procWatchScope` (26-34): any ask of `null` (every process) makes the whole answer `null`. Else every cgroup
  named once, a cgroup under another named one dropped (`inCgroup`: equal or under `x/`; `/a/x` goes under `/a`, `/ab`
  stays), sorted by code unit; more than `WATCH_CGROUPS_MAX` = 16 left → `null`. No asks → `[]`. Test
  `b1_proc_watch_scope_table` (10 rows).
- B2 `namedWatchRefusal` (50-53): `{ ok: false, code: "unsupported", error: placeBehindLine(name, gap) }` where
  `namedWatchGap(version)` answers (no version; a version under `NAMED_WATCH_DAEMON_VERSION` = 136), else nothing.
  Test `b2_named_watch_refusal_table`.
- B3 `ports.watch` (200-205, 111-145) asks B2 first and sends nothing when behind. The roots are the frame's integer
  entries in order (`7.5` and `"x"` dropped). The folder's cgroups are its threads newest first: every session row of
  the workspace by `startedAt` (missing reads 0, the newest per thread kept), then thread records with no row at 0;
  the sort is stable, so equal starts keep map order. Test `b3_ports_three_threads`.
- B4 Placement (114-123): a cgroup keeps the watch it was placed in while its thread stands (`placements[workspace]`,
  shared by every pane of the folder); a cgroup no longer listed leaves; a new one takes the first watch with room
  (fewer than 16). Shares = `max(1, highest used + 1)`. The watches sent are `max(shares, watches last named)`, each
  named `workspaceId` for the first and `workspaceId-<n+1>` after; the first carries `roots`, `folder`, `cgroups`,
  `watch` in that key order, the others `roots: []`, `cgroups`, `watch`. A watch no longer needed is named again over
  nothing (`cgroups: []`), then dropped from the pane's list. Test `b4_ports_twenty_threads_place_and_shrink`.
- B5 The reply (133-144): the first refused share's reply whole, holders untouched. Else holders cleared and refilled
  from each share's `ports`, a port in two shares answered once, its first row kept; the reply is the first share's
  reply with `ports` replaced. Tests `b5_one_share_refused_answers_its_refusal`, `b5_ports_merged_once`.
- B6 `heard(port.open/close)` (147-161): a watch the pane did not last name is never heard. An open is heard only when
  no other watch already holds the port. A close is heard when the closing watch held it and none still does; a close
  from a watch that did not hold it is heard only when no watch holds the port. Test `b6_port_holders_table` (8 events).
- B7 Processes (220-230, 181-191, 88-91): `proc.watch` marks the pane watching, records its ask on the link (string
  entries only, `null` when the frame has no array), forgets what it last told and sends `proc.scopes` (B9), then one
  `proc.watch` with the link's merged scope (B1), no `cgroups` key for `null`. A refused watch takes the pane's ask
  back. `proc.unwatch` and `close` take the ask back once: the last pane on the link sends `proc.unwatch`; otherwise a
  scope that moved sends the merged `proc.watch` again. Nothing is sent where the link was replaced since the pane
  opened (counted per link handle, A9 of the README). Tests `b7_procs_two_panes`, `b7_procs_whole_and_refused`,
  `b7_link_replaced_says_nothing`.
- B8 Readings (206-219, 193-196): one hold per pane, reused by a second `sys.watch`; a failed hold answers `{ ok:
  false, error }` and is let go so the next ask tries again; `sys.unwatch` releases the hold once and always answers
  `{ ok: true }`. Test `b8_readings_hold_refuse_release`.
- B9 Scopes (164-170): `proc.scopes` is sent only when its JSON differs from what this pane last told; a push sends it
  again for a watching pane. A refused `proc.scopes` is ignored. Push (172-176) also re-watches ports when the joined
  cgroups changed. Test `b7_procs_two_panes` (steps "a thread starts", "nothing moved").
- B10 `pty.create` (234-238): `env` = the turn's environment (`threadEnv(entry)`) under the frame's object `env`
  (a non-object reads `{}`), `cwd` the frame's string `cwd` else the folder; the frame's other keys keep their order,
  `cwd` then `env` set after them (a key already present keeps its slot). Other ops pass whole. Test
  `b10_terminal_and_pass_through`.
- B11 `heard` for `sys.sample` while a hold stands, `proc.snapshot`/`proc.changes` while watching; any other type
  false. `close` is idempotent; it removes the pane's push, takes back its ask and lets its readings go. A
  `workspace.deleted` event drops the workspace's placements. Test `b11_close_and_delete`.

## Null and empty inputs

`procWatchScope([])` is `[]`, `[[]]` is `[]`. A `ports.watch` with no `roots` array watches with `roots: []`. A
folder with no threads sends one watch with `cgroups: []`.

## What survives a restart

Nothing: placements, holders and asks are memory.

## Unclear in the TS

- B6's close from a watch that never held the port is heard (`!held.delete(watch)` then `held.size === 0`): the pane is
  told of a close it never heard open. Pinned; after-port ticket 2.

## Tests (`tests/box_panels.rs`, 15)

| Rule | Tests |
|---|---|
| B1 | `b1_proc_watch_scope_table` |
| B2 | `b2_named_watch_refusal_table`, `b3_a_daemon_behind_is_refused_before_anything_is_sent` |
| B3 | `b3_ports_three_threads` |
| B4 | `b4_ports_twenty_threads_place_and_shrink` |
| B5 | `b5_ports_merged_once`, `b5_one_share_refused_answers_its_refusal` |
| B6 | `b6_port_holders_table` |
| B7, B9 | `b7_procs_two_panes`, `b7_procs_whole_and_refused`, `b7_link_replaced_says_nothing` |
| B8 | `b8_readings_hold_refuse_release` |
| B10 | `b10_terminal_and_pass_through` |
| B11 | `b11_close_and_delete` |

TS cases (`test/box-panels.test.ts`): the map lists 19 strong; the ones this file decides are the ports watch over a
folder's threads and its cgroups, the named watches past 16, the daemon behind `NAMED_WATCH_DAEMON_VERSION`, the
process watch shared on one link and taken back, the readings hold and `sys.unwatch`, and the terminal's folder and
environment, each a session above. The cases about forwards, cgroup ends, a turn's launch order and the stop are
other areas' (places' `pane_ports`, machines' kinds and thread ends) and stay with them.
