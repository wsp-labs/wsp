# serve_workspaces: events, status, the workspace verbs, daemon channels, readings and exec

serve.ts:369-373 (`handed`), 1170-1473, 1634-1678 at `65f6b89c7`. Rust: `src/serve/workspaces.rs`. Tests:
`tests/serve_workspaces.rs`. Every op here passes the socket's `origin` (serve_door.md, D30-D31) last.

## Streams

- S1. `events.subscribe` (serve.ts:1170-1188): `rt.events().since(after, stream)` and the listeners are taken in
  one step with no await between, so no event falls between the replay and the live feed. Each event, replayed or
  live, passes only when `seen_by(e, origin)` and `cost_moved()(e)` (one `cost_moved` per subscribe, so a cost tick
  that moved nothing is dropped per socket), and is sent as `shown_to(e, origin)`. Listeners: every bus event
  (`"*"`), the forwards source, the doctor door's lines where one is wired, the release door's where one is wired;
  each detach goes on the socket's list. The reply `{"id","ok":true,"seq":head,"stream","gap"?}` (`gap` only when
  true) goes **before** the replayed events; with `creates: true` the last stage of every create in flight
  (`rt.workspaces().creating()`) follows the replay, through the same pass. A doctor or release line carries no
  sequence and is never replayed (it is not in the bus's ring).
- S2. `status.subscribe`: the watch is taken first, its detach pushed, then
  `{"id","ok":true,"statuses":status.list(None, origin)}` (serve.ts:1189-1194).
- S3. `status.list`: `status.list({reconcile: "on-failure", zombieProbe: false, reader: "table"}, origin)`, each
  row through `WorkspaceListing` so the route the reach carries is dropped: `{"statuses":[..]}`
  (serve.ts:1195-1206).

## Workspace verbs

- S4. Every verb that answers a workspace sends it through `handed` (`WorkspaceOut`, serve.ts:372): the display
  stream a provider minted for a desktop machine is not in the reply. Verbs: `workspaces.create` (`workspace`),
  `workspaces.list` (`workspaces`), `resolve`, `get` (with `threadId`), `nap`, `wake`, `resize` (with `size`),
  `stopWake`, `upgrade`, `rename`, `look` (`theme`? `glyph`?), `folder.make`, `workspaces.rebuild`.
- S5. `workspaces.create` (serve.ts:1207-1213): the frame less `id`, `op` and `origin` is the create's options; the
  answer less `notice` and `sub` is the workspace. Reply `{"id","ok":true,"workspace":handed,"notice"?,"sub"?}`.
  The client's `origin` never reaches the create as an option.
- S6. Verbs whose answer is spread after `ok`: `workspaces.landing` (`project`, `sized: true` only when true),
  `bringBack` (`workspaceId`, `title`?, `body`?), `worktree.make`, `workspaces.checkout` (`fresh === true`),
  `discard` (`check: true` only when true, `threadId`?), `commit` (`paths`?, `threadId`?), `commitDraft`,
  `viewed` (`path`?, `blob`?), `pullRequestView` (`fresh: true` only when true), `pullRequestDiff`,
  `pullRequestReply` (`replyTo`?, `threadId`?), `pullRequestResolve`, `pullRequestReact`, `pullRequestSend`
  (`threadId`?), `fix` (`check`?, `child`?, `threadId`?, `childThreadId`?), `merge` (`method`?, `whenChecksPass`?,
  `head`?, `threadId`?), `update` (`threadId`?), `mergeIn` (`threadId`?, `childThreadId`?), `reviewDraft`
  (`summary`?, `verdict`?, `on`?), `reviewPost` (`threadId`?) (serve.ts:1214-1394).
- S7. `workspaces.start` and `workspaces.review` answer a `StartResult` whose `workspace` goes through `handed`
  (`started_out`, serve.ts:373). `start` passes `url`, then `project`? `agent`? `model`? `effort`? `access`?;
  `review` passes `url`? `workspaceId`? `agent`? `model`? `effort`? (serve.ts:1344-1376).
- S8. Verbs that answer `{"id","ok":true}` after their await: `restartDaemon`, `worktree.remove` (`force`?,
  `check: true` only when true), `workspaces.delete`, `workspaces.forget`, `workspaces.touch`.
- S9. `workspaces.portReach` (`reach`), `workspaces.portProbe` (`probe`) (serve.ts:1634-1639).

## Daemon channels (serve.ts:1407-1467)

- S10. `daemon.open` names exactly one of `placeId` and `workspaceId`; both or neither fails `DAEMON_OPEN_ONE_OF`
  through the catch. A `placeId` is the person's own road (gated as serve_places.md's gate). `HERE_PLACE_ID` opens
  `rt.here_channel(on_event)`; another place opens the place door's `channel(placeId, on_event)`, and no link fails
  `absent_computer(name_of(placeId), null).sentence`; that channel is wrapped by `rt.place_kills`. A `workspaceId`
  opens `rt.workspaces().daemon_channel(id, on_event, origin)`, whose refusal (not this socket's to drive, an
  unknown id, a daemon that refused) is the reply.
- S11. The channel id is 6 random bytes in lower hex. Events the daemon pushes before the open reply is written are
  queued and sent after it, in order: a page hears a channel's id before anything arrives on it. Each later event
  is `{"type":"daemon.event","channel","event"}`.
- S12. A socket that closed while the dial was in flight closes the new channel and sends nothing.
- S13. A channel is this socket's alone. `daemon.send` and `daemon.close` on an id this socket did not open fail
  `no such daemon channel on this socket` with no kind.
- S14. `daemon.send` answers `{"id","ok":true,"reply":<the daemon's reply as it wrote it>}`: a daemon's own
  refusal rides inside an ok reply. A channel whose daemon socket ended fails the send.
- S15. A daemon socket that ends while the channel is still this socket's sends
  `{"type":"daemon.closed","channel","code","reason"}` once and forgets the channel; a channel the page closed
  (`daemon.close`, answered `{"id","ok":true}`) says nothing then or later.
- S16. The socket's close closes every channel it holds (serve_door.md, D35).

## Readings and exec

- S17. `sys.subscribe` and `sys.unsubscribe` go to the socket's sys road (`ServedRuntime::sys_ops`); a
  `sys.subscribe` naming a `placeId` is gated to the own road first (serve.ts:1468-1473). The road's own rules
  (one subscription however many panes, one sampler, a machine that reads its own refused) are wsp-core's
  `sys_socket`, not this unit's.
- S18. `workspaces.exec` (serve.ts:1656-1678): `exec_stream(id, argv, cwd, origin)`; the exec id is 6 random bytes
  in lower hex. While it runs, the socket's close tears the stream down. The reply
  `{"id","ok":true,"execId","cwd"?}` (`cwd` the stream's `ran_in` where it has one) goes first; then each line as
  `{"type":"exec.output","execId","text"}` to the asking socket only; then
  `{"type":"exec.exit","execId","exitCode","error"?}` once, `error` the lines' failure where reading them failed.
  An exec that ended is not torn down by a later close.

## Rules against tests (`tests/serve_workspaces.rs`)

| Rule | Tests | TS case |
|---|---|---|
| S1 | `events_reach_only_subscribed_sockets`, `subscribe_answers_head_and_stream_then_replays_after_the_cursor`, `a_cursor_older_than_the_ring_is_a_gap_with_no_replay`, `a_cursor_past_head_is_a_gap`, `a_cursor_from_another_stream_is_a_gap`, `an_event_the_caller_may_not_see_is_never_sent`, `a_cost_tick_that_moved_nothing_is_dropped`, `doctor_lines_reach_subscribers_before_the_reply_and_are_never_replayed`, `creates_follow_the_replay_once_and_a_plain_subscribe_hears_none`, `forward_events_reach_subscribers_and_die_with_the_socket` | serve.test.ts:345, 911, 1257, 1274, 1335; serve-replay.test.ts:83-186 |
| S2, S3 | `status_subscribe_watches_before_the_snapshot`, `status_list_reads_the_table_and_drops_the_route` | status.test.ts |
| S4, S5 | `create_strips_origin_and_splits_notice_and_sub`, `every_workspace_answer_goes_out_without_the_display_stream` | runtime.test.ts:288 |
| S6-S9 | `spread_verbs_pass_only_the_fields_sent` (one loop over the table), `start_and_review_hand_the_workspace_out`, `unit_verbs_answer_ok_after_the_await`, `port_reach_and_probe_name_their_field` | serve.test.ts:676, 693, 718 |
| S10 | `daemon_open_names_one_of_place_and_workspace`, `daemon_open_here_is_the_own_road`, `daemon_open_an_absent_computer_says_so`, `daemon_open_carries_the_workspace_refusal` | serve-daemon.test.ts:198-348 |
| S11 | `daemon_open_replies_before_any_event_and_queues_the_hello` | serve-daemon.test.ts:86 |
| S12 | `a_socket_gone_mid_dial_closes_the_channel` | serve.ts:1436 |
| S13 | `a_channel_is_never_reachable_from_another_socket` | serve-daemon.test.ts:149 |
| S14 | `daemon_send_carries_the_daemons_refusal_under_ok` | serve-daemon.test.ts:120 |
| S15 | `a_daemon_end_is_said_once_and_a_page_close_says_nothing` | serve-daemon.test.ts:177 |
| S16 | `a_socket_that_goes_takes_its_channels` | serve-daemon.test.ts:165 |
| S17 | `sys_subscribe_on_a_place_is_the_own_road` | serve-sys.test.ts:177 |
| S18 | `exec_replies_then_streams_lines_and_exit_to_the_asker_only`, `exec_names_the_folder_it_ran_in`, `the_asking_socket_closing_ends_the_command`, `an_ended_exec_is_not_torn_down_again`, `a_failed_read_rides_the_exit` | serve.test.ts:992, 1033, 1072, 1088, 1109 |

Not ported here, each its area's: a create's stages, the daemon's token path (machines), the readings' sampler and
its sharing (wsp-core `sys_socket`), a nap or delete ending an exec with its reason (machines), the ring's cap
(wsp-core `events`).
