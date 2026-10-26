# place_roads

Source: `packages/host/src/place-signin.ts` (127 lines) and `packages/host/src/setup-follow.ts` (128 lines) at
65f6b89c7. TS tests: `test/place-signin.test.ts` (5; the catalog case `sharedOn` is wsp-catalog's), and the five
add-tool cases of `test/places-add.test.ts` (the `addCommand` cases and the `followSetup` race are the add command's,
not this unit's). Rust tests: `tests/place_roads.rs` (19). Both files talk to the host through cli-a's
`verbs::client::HostClient` (`request` answers the reply frame; a reply with `ok !== true` is the client's error).

## What it does

`target_link` turns one daemon channel the host holds into a `PtyLink`; `relay_sign_in` runs a sign-in the host
planned on that computer's own terminal and shows it here; `watch_setup` follows one add or setup stream;
`add_computer` is the MCP tool's add, which answers at the first wait, the end or its ceiling.

## Rules

1. `target_link(client, target)` (place-signin.ts:32-63): `request("daemon.open", target)` (the target's own keys,
   `placeId` or `workspaceId`), its `channel`. The link:
   1. `op(op, extra)` = `request("daemon.send", {channel, frame: {...extra, op}})` and answers its `reply` object;
      the frame's keys are extra's then `op` last.
   2. events: a client frame with this `channel` and `type: "daemon.event"` hands its `event` to every listener
      when it is a JSON object (not null, not a string); other channels, types and events are not this link's.
   3. `closed` settles on a `daemon.closed` frame for this channel, on the client's own close, or on `close()`.
   4. `bare` is `pty_bare_on(target)`: true for a place id other than this computer's, false for a workspace.
   5. `close()` takes the frame listener off, settles `closed`, then `request("daemon.close", {channel})`, any
      failure swallowed. A refused `daemon.open` is the client's error unchanged.
2. `prepare_failed(link, cmd)` (:67-74): `exec {cmd}`; a link failure is read as `{ok: false, error: message}`.
   `ok === false`: the error when a non-empty string, else `the computer refused it`. Exit not a number, or 0:
   none. Otherwise the last line of stderr, else of stdout, else `it exited <n>`.
3. `relay_sign_in` (:104-127): with `line.prepare`, a failure throws `the sign-in on <where> did not start, since the
   step before it failed: <said>` (no kind) and no pty is created. Then `relay_pty` with the command, the
   terminal, `open`, `line.env` when present, and `timeout_ms` or `BOX_SIGN_IN_MS` (600 000). With no agent, no
   status check on the agent's login row (`wsp_catalog::catalog::login_sign_in(agent)?.status`) or no
   `line.status`: `{signedIn: exitCode == 0}`. Otherwise `run_quiet(status, STATUS_MS = 20 000, line.env or {})`,
   and `signedIn` by the row's check; `detail` only when signed in and the row names one; `said` only when not,
   the row's `why` or the last line of the output. Serialized `signedIn`, `detail`?, `said`?.
4. `watch_setup(client, addId, say)` (setup-follow.ts:27-68): listens to every client frame.
   1. A `place.stage` frame of a followed id pushes `{step, state, note?}` to `lines` (no `ms`, no `placeId`), sets
      `failed` to its note when the state is `failed`, tells `say.line`, and wakes `next()`.
   2. A `place.setup` frame of a followed id: a `line` is pushed and told; a `wait` replaces the row's wait in
      `waits` (first-heard order kept) and is told; an `end` sets `end` to `{end, said?}` (a later end without said
      drops the earlier said); then wakes `next()`. A frame that does not parse as either is ignored.
   3. `next()` settles at the first wake after it was called. `also(id)` follows a second id. `off()` takes the
      listener off; nothing is heard after it.
5. `open_waits` keeps the waits whose state is `waiting`, in order (:71).
6. `is_computer_word(word)` (:90): no `/` or `:`, not opening with `~` or `.`, not ending `.git`, not a key of
   `PROVIDER_KEY_WORDS` (`box`, and the cloud's provider words in a build that carries the cloud).
7. `add_computer(client, ask, ceiling)` (:96-128):
   1. Unless `resume`, a word that is not a computer's refuses with usage `add_tool_refusal(word)` + ` ` +
      `ADD_TOOL_FIX`, fix `ADD_TOOL_FIX`, before anything is sent.
   2. Listen for `place.setup` frames (any id) as wakes; `events()`; then `places.setup {ref, recipe?}` on resume,
      else `places.add {address, name?, sshPort?, keyPath?, hostKey?, recipe?}` in that key order.
   3. `until = now + (ceiling or ADD_TOOL_MS = 600 000)`. Loop: read `places.list`, the row with the add's place id
      or the add's own place; `waiting` = its setup's waiting or []; answer `{computer, setup: steps or [],
      waiting}` when the setup is absent or not `running`, when not `later` and a wait is open, or when the time is
      up; else wait for a wake, the client closing, or the time left.
   4. The frame listener comes off on every road out.

## Decisions

1. `PlaceLink.close()` consumes the road.
2. `watch_setup` keeps its state in `heard: Arc<Mutex<Heard>>`; `waits` is an `IndexMap` by row.

## After the port

1. `add_computer`'s loop wakes on any `place.setup` frame, another add's included.
2. A wait the host stops listing reads as landed; `open_waits` never sees `expired` rows the host dropped.

## Rules to tests

| Rule | Test |
|---|---|
| 1 | one_channel_is_opened_the_ptys_frames_ride_it_and_only_its_own_events_come_back, closing_the_road_swallows_a_refused_close_and_settles_closed, a_refused_daemon_open_is_the_hosts_refusal |
| 2 | a_refused_or_failing_first_step_stops_before_the_pty_naming_the_computer_and_its_last_line, a_first_step_the_link_fails_is_read_as_the_link_s_own_words |
| 3 | the_first_step_runs_then_the_command_with_its_environment_then_the_status_the_same_way, a_status_that_says_nothing_of_a_login_is_not_signed_in_and_carries_what_the_tool_said, a_line_with_nothing_first_and_no_status_is_read_by_its_own_exit |
| 4 | a_followed_add_hears_its_lines_waits_and_end_and_nothing_of_another_stream, next_settles_at_the_next_thing_heard_and_not_before |
| 5 | the_open_waits_are_the_ones_still_waiting |
| 6 | the_tool_takes_a_computer_word_and_nothing_else |
| 7.1 | the_add_tool_refuses_a_projects_source_and_a_provider_asking_the_host_nothing |
| 7.2 | the_add_tool_sends_the_adds_params_in_order, the_add_tool_resumes_with_places_setup_by_ref_and_takes_the_runs_next_wait |
| 7.3 | the_add_tool_answers_at_the_first_sign_in_waiting_on_the_person, the_add_tool_answers_at_the_end_once_nothing_waits_and_goes_past_a_wait_it_was_told_to_leave, the_add_tool_answers_at_its_ceiling_with_how_far_the_add_got, a_row_missing_from_the_listing_reads_as_the_place_the_add_answered |
