# channels_daemon: facts of a record and the roads to a daemon (`account/channels.ts:1-280`)

The first third of the channels area: what a record's view and turns read (its project, checkout, place, caps and
turn wall), and every road this host takes to a workspace's daemon or this computer's own. TS tests:
`test/channels.test.ts` (the road cases), `test/agents-brokered.test.ts` (op tokens), `test/box-panes.test.ts` (the
stdio road on a box), `test/pull-request.test.ts` (`onThisComputer`), and the place guard cases in
`test/places-settings.test.ts`. The case list is in `tests/channels_daemon.rs`' header.

## The area (shared by the three channels units)

`ChannelsArea::new(ctx: Rc<dyn ChannelsCtx>) -> Rc<ChannelsArea>`; its fields are every `Map`/`Set` the TS closure
holds, `daemon_notes` first. It registers two `workspace.deleted` listeners at construction (ptys' and copy's), in
the TS order (429, 778).

## Rules

**Facts of a record**

- D1 `startedAs`, `cutBefore` (33, 37) read the transcript index of the workspace: the start a thread announced, and
  whether its last turn was cut (missing reads false).
- D2 `resumedFact(workspace, resume, fact)` (44-51): the newest session row (last in map order) of that workspace
  whose `claudeSessionId` is `resume` and that carries the fact; else the transcript index's `facts[resume][fact]`.
  `folderOf` is the `cwd` fact. `accessOf` (61-62) is the thread record's `permissionMode`, else (with a resume) the
  `permissionMode` fact. Tests `d2_resumed_fact_newest_row_then_index`, `d2_access_of_record_then_fact`.
- D3 `homeOf` (70-73) is `{ home }` where the kind's module answers one, else `{}`.
- D4 `providerOf` (80-81): a record naming a place reads `placeDoor.offerOf(place)` (nothing without a door); a
  driven kind reads `places.wired`; else nothing. `placeIdOf` (84): `here` for a local record, else its `place`,
  else its provider. `settingsAt` (86) is the door's settings for a place id, nothing for none. Test
  `d4_provider_and_place_of_each_kind`.
- D5 `turnLimitOf` (89-93): the door's `turnLimitAt(placeId)`; nothing where it answers nothing (`undefined`), `{
  deadlineMs: +inf }` where it answers `null` (no limit), else `{ deadlineMs: n }`. Test `d5_turn_limit_three_states`.
- D6 `agentsHeld(r)` (95) is the record's own `agents`, else `spawnAt(placeIdOf(r))`; `spawnAt` (97) is
  `agentsFrom(undefined, settingsAt(place)?.spawn ?? {})`: the person's parts over `AGENTS_ON`. Test
  `d6_agents_held_record_then_place_then_default`.
- D7 `projectHeld(id)` (101-105) throws plain `noSuchProjectLine(id, names of every held project in map order)`.
  `refOf` (109) is `{ id, name, path, computer }` in that key order. Test `d7_project_held_refuses_in_its_words`.
- D8 `checkoutOf(r)` (114-121): a sub-machine's `sub.folder`; no worktree or a gone one → the project's path; else
  `join(worktree.path, relative(project.git.top ?? project.path, project.path))` (node path semantics: `relative` of
  equal paths is `""`, so the worktree path itself). Test `d8_checkout_of_table` (sub, none, gone, top equal, subfolder).
- D9 `computerOf(entry)` (124-125): `placeDoor.nameOf(place)` (the place id without a door) for a record with a
  place, else `machineWord(kind)`.

**The roads**

- D10 `servedByItsComputer` (130-131) is the machine's `daemonFrame`, when it has one.
- D11 `channelOver(reach, name, onEvent)` (146-149): a reach with no `daemonToken` throws plain `"<name> is not
  answering yet"` before any dial; else `ctx.openChannel({ url, token, onEvent })`. Test `d11_no_token_is_not_answering_yet`.
- D12 `ownDaemonChannel` (138-145) dials the kind's `daemonRoad(entry)`. A failed dial under a record reading
  `running` asks the machine's state once (a failed state read counts as `running`); a `paused` answer adopts the
  pause (`ctx.adoptPause(entry)`), then the dial's own error is thrown. Test `d12_a_failed_dial_adopts_a_pause`.
- D13 `replyOf` (208-211): `ok === true` passes the reply whole; else throws `DaemonRefusal(code if a string,
  String(error ?? "<op> was refused"))`. `overChannel` (212-218) closes the channel however the work ends. Tests
  `d13_reply_of_refusal_keeps_the_code`, `d13_over_channel_closes_on_error`.
- D14 `withDaemon(entry, work)` (158-168): a served machine's frames go up its computer's link (`replyOf` each), and
  nothing is dialled. An unbrokered machine dials its daemon and closes it after. A brokered one dials with a listener
  that resolves the daemon's `daemon.hello` version (a non-number reads unknown) and wraps `ask` in the op tokens (D16).
  Test `d14_with_daemon_three_roads`.
- D15 `brokered(entry)` (172-177): the broker reports ready, the record is not local, its kind does not run in a
  folder, its machine is not served by its computer, and it names no place the door says is joined. Test
  `d15_brokered_table`.
- D16 `withOpTokens` (189-206): only `git.push`, `git.startOn`, `git.update`, `git.mergeIn`, `git.pr`,
  `git.prCheckout` (`OP_TOKEN_OPS`) take a token; every other op passes as it came. The scoping thread is `threadOn`
  (180-185: the named thread when it is a thread record of that workspace, else the last such record in map order); none
  → usage refusal `"<name> has no thread yet, so wsp cannot reach GitHub for it."` with fix `"Start a thread on it
  first."`. The op waits for the hello at most `HELLO_WAIT_MS` = 2,000 ms on the clock; a version unknown or under
  `PIPE_DAEMON_VERSION` sends the frame without a token. Else a device is minted as `deviceDoor.mint("op <op>", {
  kind: "thread", threadId, workspaceId, rootThreadId: rootOf(threadId) }, now, { road: module.turnRoad })`, the
  frame sent with `token` appended last, and the device revoked after the reply whether it was refused or not; a
  failed revoke warns `"the token of <op> on <name> was not taken away: <message>"`. Tests
  `d16_only_the_git_host_ops_carry_a_token`, `d16_no_thread_is_a_usage_refusal`, `d16_an_old_or_silent_daemon_gets_no_token`,
  `d16_the_token_is_revoked_after_a_refusal`, `d16_hello_waits_two_seconds_on_the_clock`.
- D17 `stdioRoad(entry, need = STDIO_DAEMON_VERSION)` (223-241): one listener set per road. Unserved: the machine's
  daemon dialled with the listeners as its events; `send` is `replyOf(channel.send(frame))`. Served: no place or no door
  → plain `placeServesDaemonLine(name, computerOf)`; a reported version under `need` → plain
  `slateServerBehindLine(nameOf(place))` (an unknown version passes); no link → `absentComputer(name).sentence`;
  else frames go through `served`, events off the link, `closed`/`close` the link's. Test `d17_stdio_road_table`.
- D18 `onThisComputer(work)` (247-251): no local daemon road → `DaemonRefusal("no-host-cli", "this host runs no daemon
  on this computer")`; else one channel to this computer's daemon (`channelOver(localRoad(), THIS_COMPUTER)`), the
  work handed `ask` and `local.homeDir`, the channel closed after. Test `d18_on_this_computer_without_a_daemon_is_no_host_cli`.

**The guards**

- D19 `placeGuard(place)` (255-263): nothing without a door, for a place with no row, or a row with no spend limit;
  else today's spend from `ctx.status.spend(rows, now)` for that place, refused plain `spendCapRefusal(name,
  todayUsd, limit)` where `placeAtLimitLine(row, todayUsd)` answers. Test `d19_place_guard_refuses_at_the_limit`.
- D20 `placeRefuses(place)` (268-271) throws `placeBlocked(nameOf, report)` where it answers; no place, no door or no
  report pass. `copyBlocked(entry)` (278) passes a folder kind without asking, else asks for the record's place.
  Tests `d20_place_refuses_a_blocked_computer`, `d20_copy_blocked_skips_a_folder`.

## Null and empty inputs

`resumedFact` with no rows and no index entry: nothing. `threadOn` on a workspace with no thread record: nothing.
`turnLimitOf` for a record with no place id: nothing.

## Unclear in the TS

- D16 mixes `Date.now()` (the mint's time) with `setTimeout` (the hello wait); the port reads both off the context's
  clock (README A7).

## Tests

`tests/channels_daemon.rs` (32 tests). Every expected value is `tests/fixtures/oracle/channels_daemon.json`, written by
`oracle/channels_daemon.oracle.ts` (the TS area run over the same worlds, inputs beside answers). `thread_on` (D16) is not
exposed by the TS area, so its table is read off channels.ts:180-185 and backed by channels.test.ts:559. Tests settle
spawned work with `settle()` and drive time with the fake clock; D16's hello wait runs as a local task.

| Rule | Tests |
|---|---|
| D1 | `d1_started_as_and_cut_before` |
| D2 | `d2_resumed_fact_newest_row_then_index`, `d2_access_of_record_then_fact` |
| D3 | `d3_home_of_is_the_home_the_kind_answers_else_nothing` |
| D4 | `d4_provider_and_place_of_each_kind` (with and without a door, `settings_at`) |
| D5 | `d5_turn_limit_three_states` |
| D6 | `d6_agents_held_record_then_place_then_default` (`spawn` is stripped from the Rust value: wsp-wire still carries it) |
| D7 | `d7_project_held_refuses_in_its_words` |
| D8 | `d8_checkout_of_table` (node `join`/`relative`: no shared helper exists, the implementation writes one) |
| D9 | `d9_computer_of_names_the_place_or_the_kind` |
| D10 | `d10_served_by_its_computer_is_the_machines_daemon_frame` |
| D11 | `d11_no_token_is_not_answering_yet` |
| D12 | `d12_a_failed_dial_adopts_a_pause` |
| D13 | `d13_reply_of_refusal_keeps_the_code`, `d13_over_channel_closes_on_error` |
| D14 | `d14_with_daemon_three_roads` |
| D15 | `d15_brokered_table` |
| D16 | `d16_only_the_git_host_ops_carry_a_token`, `d16_no_thread_is_a_usage_refusal`, `d16_an_old_or_silent_daemon_gets_no_token`, `d16_the_token_is_revoked_after_a_refusal`, `d16_hello_waits_two_seconds_on_the_clock`, `d16_a_failed_revoke_warns_and_the_reply_stands`, `d16_thread_on_picks_the_named_thread_of_this_workspace_else_the_newest`, `d16_constants_match_the_ts` |
| D17 | `d17_stdio_road_table` |
| D18 | `d18_on_this_computer_without_a_daemon_is_no_host_cli`, `d18_on_this_computer_dials_its_own_daemon_in_its_home_and_closes_it`, `d18_on_this_computer_carries_a_refusal_with_its_code_and_a_missing_token` |
| D19 | `d19_place_guard_refuses_at_the_limit` |
| D20 | `d20_place_refuses_a_blocked_computer`, `d20_copy_blocked_skips_a_folder` |

TS cases ported (test file:line, under `packages/runtime/test/`):

| TS case | Test |
|---|---|
| channels.test.ts:162, :180, :189, :202 | `d16_only_the_git_host_ops_carry_a_token`, `d16_the_token_is_revoked_after_a_refusal`, `d16_an_old_or_silent_daemon_gets_no_token`, `d16_no_thread_is_a_usage_refusal` |
| channels.test.ts:211, :226 | `d15_brokered_table` |
| channels.test.ts:559 | `d16_thread_on_picks_the_named_thread_of_this_workspace_else_the_newest` |
| spawn-guard.test.ts:1038 | `d6_agents_held_record_then_place_then_default` |
| usage.test.ts:422, bring-back.test.ts:192, :230 | `d13_reply_of_refusal_keeps_the_code` |
| bring-back.test.ts:88 | `d14_with_daemon_three_roads` |
| serve-daemon.test.ts:198 | `d11_no_token_is_not_answering_yet` |
| runtime-index.test.ts:81, :134, local-workspace.test.ts:1715 | `d1_started_as_and_cut_before` |
| runtime-index.test.ts:585, thread-keeps-picks.test.ts:82, :118 | `d2_resumed_fact_newest_row_then_index` |
| permission-relay.test.ts:504, :551 | `d2_access_of_record_then_fact` |
| places-settings.test.ts:354, :402 | `d5_turn_limit_three_states` |
| places-settings.test.ts:441 | `d19_place_guard_refuses_at_the_limit` |
| box-folder.test.ts:564 | `d20_copy_blocked_skips_a_folder` |
| slate-box-server.test.ts:242, slate-box-http.test.ts:186 | `d17_stdio_road_table` |
| pull-request.test.ts:215, :231, :610 | `d18_*` |

Fields the implementation needs that `ChannelsArea` does not hold yet: none for this file.
