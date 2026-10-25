# serve_ops: sessions, images, usage, folders, agents, editors, the host's doors, and the catch

serve.ts:333-347 (`bundler_from`, `door_from`), 419-440 (`place_folders`), 857-862 (`awake_for_ssh`), 869-878 (the
prefix ops), 1474-1633, 1643-1655, 1679-1895 at `65f6b89c7`. Rust: `src/serve/ops.rs` and `answer_frame`
(`src/serve/answer.rs`), the switch every op unit hangs off. Tests: `tests/serve_ops.rs`.

## The switch (serve.ts:869-884, 1885-1895)

- O1. Order: `slates.*`, `projects.*`, `appProcs.*`, the processes ops, `conversations.*`, the broker ops, the
  sign-in ops, then the op table. The first five spread their helper's answer after `{"id","ok":true}`
  (serve_helpers.md). `appProcs.*` hands the hub the socket's send, its detaches and `own_road()`; the hub is one
  per runtime. The broker ops are answered with `own_road()`. A sign-in op is gated to the own road, then goes to
  the socket's sign-in road with the socket's send and origin.
- O2. **The catch.** Any error from an op becomes `{"id","ok":false,"error":failure_words(e),"kind"?,"fix"?,
  "errors"?,"warnings"?,"left"?}`: `kind` and `fix` only where the error carries them, then `refusal_fields(e)`
  (`left` only for kind `processes-left`). Then `opts.failed(op, e)`. The socket stays.
- O3. **A door the host did not wire** refuses its ops through the catch, in its own sentence (serve.ts:378-384):
  landing `this runtime cannot write folders on this computer`; folders `this runtime cannot browse the folders on
  this computer`; terminal config `this runtime cannot read the terminal config on this computer`; editor `this
  runtime cannot open an editor on this computer`; ssh `this runtime cannot carry an editor's ssh from this
  computer`; release `this runtime does not read the newest release`; init `this runtime has no cloud setup; the
  host that serves the app wires one`; projects (the bundler) `this runtime cannot read folders on this computer`.
  The door is read when the op runs, never at start.

## Sessions (serve.ts:1474-1551)

- O4. `sessions.start` goes to `serve_start` (serve_helpers.md, H10-H14); each answer it hands back is sent as
  `{"id","ok":true,...answer}`, so a held start can answer twice (held, then started) under one id.
- O5. `sessions.warm`: the frame less `id`, `op`, `workspaceId`, `project` and `cwd` is the picks; the folder is
  `start_at(workspaces, msg, origin)`; `warm(at.workspaceId, {...picks, cwd?}, origin)` spread.
- O6. `harnesses.list(workspaceId, origin, {project?, computer?})`: `{"harnesses"}`.
- O7. `sessions.history` (serve.ts:1489-1497): a `threadId` without `whole: true` reads one page
  (`page(workspaceId, {threadId, before?, limit?}, origin)`, spread). With no thread and no `whole`, a `before` or a
  `limit` refuses `usage_refusal("before and limit page one thread's events.", "Name the thread with threadId.")`.
  `whole: true` reads `thread_history(workspaceId, msg, origin)` (its own refusal for no thread or a window is the
  area's); otherwise `history(workspaceId, origin)`. Both answer `{"events"}`.
- O8. Plain session ops, each with its fields: `sessions.list` (`sessions`), `sessions.head` (`afterRead: true` only
  when true; spread), `sessions.attachment` (`attachment`), `interrupt` (`task`), `answer` (`askId`, `optionId`,
  `reason`?), `access`, `rename`, `restore`, `search`, `aside` (`askId`), `steer` (`prompt`, `requestId`?,
  `attachments`?), `rewind` (`turnId`?, `files`?, `undo`?): spread. `forget` (`check: true` only when true),
  `read`, `replaceable`, `mark`: `{"id","ok":true}` after the await. `sessions.run`: the frame less `id` and `op`
  is the step, `{"run"}`.

## Images, usage and forwards (serve.ts:1552-1655)

- O9. `golden.get` (`manifest`), `capabilities.get` (`capabilities`: the backend's flags, no await),
  `golden.prepare` (`builder`; `kind`? passed), `golden.seal` (spread), `image.get` (`view`), `image.build`
  (`build`; `place`, `name`?, `force`?).
- O10. `snapshots.list`: the name is `default` when absent; `{"lineage":{"name","head":manifest.head or null,
  "versions":manifest.versions or []}}`. `snapshots.storage`: `{"storage":<storage or null>}`.
  `snapshots.rollback`: `{"lineage":{"name","head","versions"},"existingWorkspaces":"untouched"}`.
- O11. `cost.history` (`points`). `cost.spend`: a thread's socket reads `spend(thread_places(..))` before the gate;
  otherwise gated to the own road, then `spend(place rows or [])`: `{"places"}`.
- O12. `usage.used`, `usage.accounts` and `usage.reset` are gated to the own road. `used` passes `range`, `split`,
  `outside`? (`{"used"}`); `accounts` `fresh: true` only when true (spread); `reset` `account`, `creditId`?, `on`?
  (spread).
- O13. `places.readings`: gated; a `workspaceId` wins over a `placeId`; neither sends
  `{"id","ok":false,"error":"places.readings names a placeId or a workspaceId","kind":"usage"}`; spread.
- O14. `forwards.list`: every forward of the source whose workspace this caller may drive
  (`origin_refusal(id, origin)` none), awaited one at a time, in the source's order: `{"forwards"}`.
  `forwards.stop`: the refusal throws; a forward the source does not hold fails
  `nothing is forwarding localhost:<port> there`; else `{"id","ok":true}`. With no source the list is empty and a
  stop fails.

## Folders, agents and editors (serve.ts:419-440, 1679-1833)

- O15. `host.folders` with no `on` or `on: here` lists this computer: `wide` only for a socket whose road is
  `here` and that is no thread; `dir`?, `hidden`?, `repos: true` only when true: `{"listing"}`. Another computer is
  gated to the own road, then `place_folders`: an id not in the rows refuses `no_such_place_refusal` kind `usage`;
  a row that is not a joined computer refuses `provider_folders_refusal` kind `usage`; a daemon below
  `FS_FOLDERS_DAEMON_VERSION` (73) refuses `place_behind_line(name, place_daemon_behind(..))`; no link refuses
  `absent_computer(name, null).sentence`; else `fs.folders` with `projects` (the checkouts or paths of that
  computer's projects, `projects.list(origin)`) over the link, an `ok: false` reply failing with its error, the
  link closed whatever happens; `HostFolderListing` parsed.
- O16. `agents.read` and `servers.tools`: not own road sends `PLACES_TICKET_REFUSAL` with **no** kind.
  `servers.icon`, `agents.addTools`, `agents.setup`, `skills.*`, `servers.add|remove|toggle`, `plugins.toggle` are
  gated (kind `ticket`).
- O17. `agents.signInLine`: host only; a non-host socket on its own road sends `SIGN_IN_LINE_REFUSAL`; off its own
  road `PLACES_TICKET_REFUSAL` kind `ticket`. `agents.key`: host only, else `AGENTS_KEY_REFUSAL`; `{"id","ok":true}`.
- O18. `agents.setup`: the frame less `id`, `op`, `placeId`, `agent` is the change; a socket that is not the host's
  sending any env value that is not `null` refuses `ENV_VALUE_REFUSAL`; `{"agent"}`.
- O19. Skills: `skills.search` (`skills`), `skills.get` (`preview`), `skills.preview` (`preview`), `skills.add`
  (`added`; `agents`?, `project`?), `skills.remove`, `skills.toggle` (spread). Servers: `servers.add` passes the
  frame less `id`, `op`, `target`; remove and toggle pass `{agent, name, scope?}`, toggle with `on` (spread).
- O20. `editor.list` and `editor.open`: not own road `EDITOR_TICKET_REFUSAL` kind `ticket`. Open reads the
  workspace (`get(id, origin)`), the editor picked or the preferences' own. A workspace on this computer opens with
  `inside` its worktree path then its project path. Any other: no ssh door fails `editor_opens_here_line(name)`;
  the editor's `remote_refusal` throws; ssh's include not on sends `{"id","ok":false,"error":ssh_include_line(name),
  "kind":"sshInclude"}`; else the port is opened on the workspace woken first when it is not running, then the
  editor opens with `inside: []` and `remote: {alias: ssh_alias(name), folder: folder or project path, name}`.
- O21. `ssh.port` and `ssh.include`: not own road `SSH_TICKET_REFUSAL` kind `ticket`. `ssh.include` reads or sets
  (`on`) `{"sshInclude"}`; `ssh.port` on a workspace on this computer fails `ssh_copy_here_line(name)`, else the
  port on the woken workspace: `{"port"}`.

## The host's doors (serve.ts:1781-1883)

- O22. `host.terminalConfig` (`config`), `init.get` (`setup`; `on`?), `init.keys` (`setup`; `provider`?, `key`?),
  `preferences.get` (`preferences`), `preferences.set` (spread), `release.get` (`release`, no await),
  `release.check` (`force === true`).
- O23. `host.restart`: not own road `HOST_RESTART_TICKET_REFUSAL` kind `ticket`; no door fails
  `HOST_NO_RESTART_LINE`; a door's refusal fails with it. Otherwise `{"id","ok":true}` is sent first, then the
  restart starts once: asks that overlap join it. A failed restart forgets itself (the next ask tries again) and
  logs `host.restart failed: <message>`.
- O24. `project.seed.plan` (`plan`), `project.plan` (`plan`, through the bundler), `project.import` (`imported`;
  `workspaceId`, `source`, `dest`, `replace`, `carry`, `rewrite`, `agents`, the bundler), `project.export`
  (`exported`; the lander).

## Rules against tests (`tests/serve_ops.rs`)

Generated from the rule each test names in its first comment; 42 tests.

| Rule | Tests | TS case |
|---|---|---|
| O1 | `prefix_ops_spread_their_helpers_answer`, `sign_in_ops_are_the_own_road` | serve.ts:869-878; serve.ts:877 |
| O2 | `a_failed_op_carries_kind_fix_and_left_and_tells_failed`, `a_refusal_carries_the_fix_beside_the_sentence`, `a_failure_carries_only_what_it_has`, `a_refusal_the_switch_sends_is_not_told_to_failed` | A failure with no kind and no fix carries its sentence alone; a slate write's lists ride beside it.; A refusal the switch sends itself is not told to failed.; serve-refusal.test.ts:42; serve-refusal.test.ts:50 |
| O3 | `each_unwired_door_refuses_in_its_own_sentence` | serve.test.ts:1186 |
| O4 | `sessions_start_sends_every_answer_under_one_id` | serve.test.ts:634 |
| O5 | `sessions_warm_strips_the_folder_fields` | thread-warm-agent.test.ts |
| O6 | `harnesses_list_passes_the_catalog_place_only_when_named` | serve.test.ts:508 |
| O7 | `history_with_a_thread_reads_a_page`, `history_with_a_window_and_no_thread_is_usage`, `history_whole_reads_thread_history`, `history_plain_reads_the_folder` | serve.test.ts:374; serve.test.ts:388; serve.test.ts:426 |
| O8 | `session_ops_pass_only_the_fields_sent`, `interrupting_twice_reaches_the_runtime_twice` | serve.test.ts:661 |
| O9 | `capabilities_are_the_backends_flags`, `golden_and_image_ops_pass_only_the_fields_sent` | serve.test.ts:362; serve.test.ts:735 |
| O10 | `snapshots_list_names_default_and_null_head`, `snapshots_rollback_says_untouched`, `snapshots_storage_null_when_none` | serve.test.ts:834; serve.test.ts:834, 855; serve.test.ts:855 |
| O11 | `cost_spend_for_a_thread_reads_its_places`, `cost_spend_is_the_own_road_and_reads_the_places_it_holds`, `cost_history_passes_the_origin` | serve-refusal.test.ts:164; serve.test.ts |
| O12 | `usage_ops_are_the_own_road` | serve-refusal.test.ts:164 |
| O13 | `readings_name_a_place_or_a_workspace` | serve.ts:1649 |
| O14 | `forwards_list_shows_only_drivable_and_stop_refuses_an_unknown`, `without_a_source_the_list_is_empty_and_a_stop_fails` | serve.test.ts:893; serve.test.ts:928 |
| O15 | `host_folders_wide_only_for_this_computers_own_window`, `place_folders_refuse_in_order_and_close_the_link` | places-agents.test.ts |
| O16 | `agents_ops_gates_and_sentences` | places-agents.test.ts |
| O17 | `agents_ops_gates_and_sentences` | places-agents.test.ts |
| O18 | `agents_setup_env_values_only_from_the_host` | vault.test.ts |
| O19 | `skills_and_servers_pass_their_fields` | places-agents.test.ts, vault.test.ts |
| O20 | `editor_open_here_and_over_ssh`, `editor_refuses_without_ssh_include` | editor-door.test.ts |
| O21 | `ssh_port_refuses_a_workspace_here` | ssh-door.test.ts |
| O22 | `init_keys_reaches_the_door_and_the_key_never_comes_back`, `host_doors_pass_their_fields` | serve.test.ts, release.test.ts; serve.test.ts:1174 |
| O23 | `host_restart_replies_then_restarts_once`, `host_restart_is_the_own_road_and_names_what_would_stop_it`, `a_failed_restart_is_logged_and_tried_again` | restart-door.test.ts |
| O24 | `project_ops_hand_the_bundler_and_lander` | project-import.test.ts, project-export.test.ts |
| R5 | `a_refusal_the_switch_sends_is_not_told_to_failed` | A refusal the switch sends itself is not told to failed. |
