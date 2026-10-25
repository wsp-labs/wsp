# serve_helpers: the seven serve-*.ts files

serve-app-procs.ts, serve-slates.ts, serve-start.ts, serve-projects.ts, serve-broker.ts, serve-processes.ts,
serve-conversations.ts at `65f6b89c7` (344 lines). Rust: one module each, `src/serve_*.rs`. Tests:
`tests/serve_helpers.rs` (the hub and the answers, called directly) and `tests/serve_app_procs.rs` (the hub through
`answer_frame` and a recording sink).

## The app's processes (serve-app-procs.ts)

- H1. `is_app_procs_request`: the op starts `appProcs.`.
- H2. The hub is one per runtime (`app_procs_hub_of(owner)`, a weak map keyed by the runtime, serve-app-procs.ts:92-102):
  two sockets on one runtime meet in one hub; another runtime gets its own.
- H3. A socket is known by its detach list (the same list every time it asks), its send and whether it is the own
  road. `appProcs.watch` and `appProcs.offer` from a socket off the own road throw `APP_PROCS_WATCH_OFF_ROAD` and
  `APP_PROCS_OFF_ROAD` respectively.
- H4. The first ask of either kind from a socket pushes one close onto its detach list, and only one however often
  it watches and unwatches (`joined`). That close unwatches it and, when it had offered, drops its offer and pushes
  `{"type":"app.procs","procs":all}` to every watcher.
- H5. `watch`: the socket becomes a watcher; the first watcher turns every offer on (`{"type":"app.procs.wanted",
  "wanted":true}` to each offering socket). Answers `{"procs":all}`, the offers' lists concatenated in offer order.
- H6. `unwatch`: the last watcher leaving turns every offer off: each offer's list is emptied first, then
  `{"type":"app.procs.wanted","wanted":false}` is pushed. Unwatching a socket that was not watching does nothing.
  Answers `{}`.
- H7. `offer`: kept (its list kept when it had offered before); answers `{"wanted": watchers > 0}`.
- H8. `set`: no offer from this socket throws `APP_PROCS_UNOFFERED`; with no watcher the list is dropped and `{}`
  answered; otherwise the list is kept and `{"type":"app.procs","procs":all}` pushed to every watcher. The list's
  shape (the app's process kinds, an origin's length) is the schema's (`AppProcList`), refused at the door.

## The prefix answers

- H20. `thread_places(rt, origin, now)` (serve-projects.ts:39-44): the thread's workspace is read by its scope's
  `workspaceId` (a failure is no workspace). Where that workspace runs in a folder (`runs_in_folder(kind)`), the
  computers of `projects.list(origin)` are reached. The answer is the place rows (none with no place door) that
  are providers or reached, each less `road`, a provider's with `label: place_said(row)` last. A thread on a
  machine reads the providers alone.
- H21. `answer_project`: `projects.add` passes `source`, then `on`? `name`? `base`? `into`? `seed`? and answers
  `{"project", "notice"?}` (the add's answer less `notice`); `projects.seed` (`keep`?, `cut`?, `none: true` and
  `save: true` only when true; spread), `projects.list` (`projects`), `projects.defaults` (`defaults`),
  `projects.resolve` (`project`), `projects.branch` (spread), `projects.remove` (`force: true`, `check: true` only
  when true; spread), `projects.icon` (spread), `projects.icons` (`icons`).
- H22. `answer_slate`: the request less `id` and `origin` is the params. `slates.get` (`slate`); `write`, `state`,
  `read`, `catalog` pass the params less `op` with the origin, spread; `shown`, `approve` (`threadId`, `key`,
  `scope`), `cancel` (`threadId`, `run`), `revoke` (`threadId`, `key`) answer `{}`; `event` passes the params less
  `op` (no origin), spread; `resolve` (`threadId`, `paths`) and `image` (`threadId`, `src`, `have`?) spread.
- H23. `slates.subscribe` takes a hold (`subscribe({threadId, sources})`), keeps it under
  `<threadId>\u0000<sources sorted, joined with ",">`, and pushes it onto the socket's detaches. `slates.unsubscribe`
  lets go of the **oldest** hold under that key and forgets the key when none is left; an unknown key does
  nothing. Both answer `{}`. The same sources in another order are the same key.
- H24. `answer_broker`: `broker.status` with no door or no status fails `BROKER_UNSERVED` (any road);
  `providers.checkKey` off the own road fails `PLACES_TICKET_REFUSAL` kind `ticket` before the door is read, then
  no door or no `check_key` fails `BROKER_UNSERVED`; both spread the door's answer.
- H25. `answer_processes`: `computer` and `processes` are passed only when present. `activity.read` (`computer`?,
  `thread`?; `{"activity"}`), `activity.end` (`computer`?, `pid`, `startedAt`; spread), `sessions.settle`
  (`finished: true` only when true, `processes`?; spread), `sessions.delete` (`processes`?, `check: true` only when
  true; spread).
- H26. `answer_conversations`: `conversations.list` (`project`?, `agent`?), `conversations.read` (`project`?,
  `agent`, `id` from `conversation`, `preview: true` only when true); both spread.

## Starting a thread (serve-start.ts)

- H10. A `workspaceId` with a `branch` refuses `BRANCH_ON_A_THREAD_LINE` kind `usage` before anything is read.
- H11. A `fork` beside any of `workspaceId`, `project`, `cwd`, `thread`, `replaces` refuses
  `usage_refusal(FORK_BESIDE_LINE, FORK_BESIDE_FIX)`; a `fork` with a `resume` refuses
  `usage_refusal(FORK_RESUME_LINE, FORK_RESUME_FIX)`. Checked in that order.
- H12. A fork goes to `sessions.fork({...start_options_of(msg, {}, none, held), fork, branch?}, origin)`; nothing
  else is read.
- H13. Otherwise a `resume` is read first (`conversations.placed(msg, resume, origin)`), and where it answers it is
  the folder; else `start_at(workspaces, msg, origin, {opens: true} only when no thread)`. Then
  `sessions.start(at.workspaceId, start_options_of(msg, at, resumed.outside, held), origin)`.
- H14. `answer_held` true hands the start an `on_held`; the answering (once at the hold, else once at the start)
  is threads' `answered_start`, which this crate calls and does not restate.
- H15. `start_options_of` copies by hand, in this order: `prompt`, `onHeld`?, `followed: true` (only when true),
  `harness`?, `thread`?, `cwd`? and `setup`? (from the folder), `model`?, `effort`?, `permissionMode`?, `access`?,
  `contextWindow`?, `fast`?, `startedBy`?, `requestId`?, `attempt`?, `notify`?, `turnToken`?, `title`?, `replaces`?,
  `attachments`?, `outside`?. Nothing else of the frame rides.

## Rules against tests

Generated from the rule each test names in its first comment; 33 tests.

| Rule | Tests | TS case |
|---|---|---|
| H1 | `serve_helpers::only_appprocs_ops_are_the_hubs` |  |
| H2 | `serve_helpers::two_runtimes_have_two_hubs`, `serve_app_procs::two_servers_over_one_runtime_share_the_hub` | Two servers over one runtime meet in one hub: an app on one is asked for by a page on the other.; serve-app-procs.ts:92-102 |
| H3 | `serve_helpers::the_hub_takes_nothing_off_the_own_road`, `serve_app_procs::the_hub_takes_nothing_off_the_own_road_through_a_socket` | serve-app-procs.test.ts:102; serve-app-procs.test.ts:102, through the door: a socket let in on a ticket is not the own road. |
| H4 | `serve_helpers::the_hub_keeps_one_close_per_socket`, `serve_helpers::a_close_drops_the_offer_and_tells_every_watcher`, `serve_app_procs::app_procs_are_asked_for_while_a_page_watches`, `serve_app_procs::app_procs_leave_with_the_socket_that_sent_them` | serve-app-procs.test.ts:109; serve-app-procs.test.ts:46; serve-app-procs.test.ts:77; serve-app-procs.ts:60-66: a close unwatches and, for an offering socket, drops its offer and tells watchers. |
| H5 | `serve_helpers::the_first_watcher_turns_every_offer_on_once_and_an_offer_says_whether_anyone_watches`, `serve_helpers::offering_again_keeps_the_list_and_a_watch_answers_every_offer_in_order`, `serve_app_procs::app_procs_are_asked_for_while_a_page_watches` | A re-offer keeps the list the socket already named.; serve-app-procs.test.ts:46 |
| H6 | `serve_helpers::unwatching_the_last_page_empties_every_offer_and_says_so`, `serve_app_procs::app_procs_are_asked_for_while_a_page_watches` | serve-app-procs.test.ts:46 |
| H7 | `serve_helpers::the_first_watcher_turns_every_offer_on_once_and_an_offer_says_whether_anyone_watches`, `serve_helpers::offering_again_keeps_the_list_and_a_watch_answers_every_offer_in_order`, `serve_app_procs::app_procs_are_asked_for_while_a_page_watches` | A re-offer keeps the list the socket already named.; serve-app-procs.test.ts:46 |
| H8 | `serve_helpers::the_hub_names_both_apps_processes`, `serve_helpers::set_comes_after_offer_and_with_no_watcher_the_list_is_dropped`, `serve_app_procs::app_procs_leave_with_the_socket_that_sent_them`, `serve_app_procs::app_procs_refuse_a_list_that_is_not_the_apps` | serve-app-procs.test.ts:119; serve-app-procs.test.ts:77; serve-app-procs.test.ts:92. The schema refuses what is not the app's process kinds, at the door. |
| H10 | `serve_helpers::start_refusals_in_order` | sub-spawn.test.ts |
| H11 | `serve_helpers::start_refusals_in_order` | sub-spawn.test.ts |
| H12 | `serve_helpers::a_fork_reads_nothing_else` | sub-spawn.test.ts |
| H13 | `serve_helpers::a_resume_is_read_before_the_folder`, `serve_helpers::opens_only_without_a_thread` | folder-threads.test.ts |
| H14 | `serve_helpers::answers_reach_the_reply_each_in_turn` | Each answer the start road makes reaches the reply, in turn, whatever the road's own words are. |
| H15 | `serve_helpers::start_options_copy_only_the_named_fields_in_order` | The fields a frame names ride by hand; nothing else does. |
| H20 | `serve_helpers::thread_places_on_a_machine_are_the_providers_alone`, `serve_helpers::thread_places_in_a_folder_reach_its_projects_computers`, `serve_helpers::thread_places_without_a_workspace_or_a_door_read_the_providers_or_nothing`, `serve_helpers::thread_places_drop_the_road_and_label_providers` | A failed read of the workspace is no workspace; no scope is no workspace; no place door is no rows.; spawn-guard.test.ts |
| H21 | `serve_helpers::project_answers_pass_only_the_fields_sent` | projects.test.ts |
| H22 | `serve_helpers::slate_answers_split_params_and_origin` | slates.test.ts |
| H23 | `serve_helpers::slate_unsubscribe_lets_go_of_the_oldest_hold`, `serve_helpers::slate_subscribe_sources_in_any_order_share_a_key`, `serve_helpers::a_socket_close_lets_go_of_every_slate_hold` | A close runs every detach; a hold already let go is not let go twice.; slates.test.ts |
| H24 | `serve_helpers::broker_status_unserved`, `serve_helpers::check_key_off_the_own_road_is_ticket_before_the_door` | serve.ts:875 |
| H25 | `serve_helpers::processes_and_conversations_pass_only_the_fields_sent` | serve-refusal.test.ts:50 |
| H26 | `serve_helpers::processes_and_conversations_pass_only_the_fields_sent` | serve-refusal.test.ts:50 |
