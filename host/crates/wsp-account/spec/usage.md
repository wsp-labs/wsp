# usage: what was used, the plan readings and resets (`account/usage.ts`, 344 lines)

The usage door (`used`, `readings`, `accounts`, `reset`), the agent logs read off every connected computer, the plan
limits read on request, and two row sentences (`awayLine`, `rowReason`). The ledger, prices, burn and the account
rows are wsp-machines' (`usage.rs`), the alerts wsp-core's (`plan_alerts`), the resets the other wsp-account
writer's (`usage_reset.rs`); this unit only wires them. TS tests: `test/usage.test.ts` (25), `test/usage-boxes.test.ts`,
`test/usage-reset-joined.test.ts`. Case list in `tests/usage.rs`' header.

`UsageCtx` reads: `preferences().await` (`usageLogs`), the place door (`list(now)`, `channel`, `link`, `name_of`,
`report_login_path`, `exec`, `place_for`, `sign_ins_at`), `local` (`has_daemon_road`, `env`, `home(agent)`,
`exec_stream`), `on_this_computer`, `over_channel`, `channel_over(local_road)`, `with_daemon`, `entry_of`, `ready()`,
`sessions()` (claude session ids), `projects()`, `vaulted_for(agent)`, `launch_of(place, agent)` (setups), the ledger
(`limits`, `file_logs`, `limit`, `used`, `account_labels`), alerts `read(before, after)`, `burn_of`, `agents_read_here()`
(the sign-ins), the clock, `placeAway(place)`, the row parts (`wakeRefused`, `nap_refused_reason`, `stop_refused_reason`,
`disk_reason`), `warn`.

## Rules

- U1 `awayLine(record)` (32-38): a sub-machine whose cloud a forget took (`sub.cloudGone`, phase not released) →
  `subCloudGoneLine(cloudSaid(sub.on))`; a record on a place the door reads away → `absentComputer(nameOf(place)).sentence`;
  else nothing. Test `u1_away_line_table`.
- U2 `rowReason(e)` (41-44), the first present: the delete line while its phase is the record's; the wake's words;
  `record.wakeRefused`; the nap refusal; the stop refusal; the disk reason. Test `u2_row_reason_order`.
- U3 `USAGE_STORES` (18-22): per catalog agent in catalog order, the history where its format is `claude-jsonl` or
  `codex-rollout`, else its usage log, else none. Test `u3_usage_stores_from_the_catalog`.
- U4 `readLogs` (86-159): off when `usageLogs` is off; at most once per `LOG_READ_EVERY_MS` (the stamp set before the
  read). Targets: this computer where it has a daemon road, then every joined computer (not here) with a link, in list
  order. Each read is bounded by `LOG_READ_MS` with label `usage.logs on <computer name>`; a failure warns `"the agent
  logs on <name> were not read for usage: <message>"`, except the unknown-op line of an old daemon, which is quiet.
  No answers → nothing filed. Rows of a session a wsp thread ran (`claudeSessionId` of any session row) are skipped. Each
  row is filed `{ at, agent, account, accountLabel, computer, project, model, tokens, costUsd?, session, source:
  "log" }`, the project the first project of that computer the folder is under, else `""`; the account is that
  computer's own login of the agent (`accountOnComputer`). Each limit snapshot the agent's `LOG_LIMITS` reads is filed
  as a reading `{ key, agent, label, road (the ledger's for that key, else "own"), computer, limit, at }`. The ledger
  files the entries with the ids read; each reading whose `before` and `after` differ is told to the alerts. Tests
  `u4_logs_read_off_when_switched_off`, `u4_logs_read_once_a_window`, `u4_logs_skip_wsp_sessions`,
  `u4_logs_project_and_account`, `u4_an_old_daemon_is_quiet`.
- U5 `hereSignIns` (164-174): held `SIGN_INS_HELD_MS` = 60,000 ms and shared while out; a failed or absent read lists
  none; only `signed-in` and `vault-key` states are kept. An `agents.changed` event drops the hold. Test
  `u5_sign_ins_held_a_minute_and_dropped_on_change`.
- U6 `usageAccounts` (183-203): the door's list (this computer's row given the here sign-ins where it has none; a row
  for this computer added last when the list has none) into `accountRows` with the ledger's limits, the namer, labels,
  plan brands, vaulted, prints-limits and burn. Test `u6_accounts_add_this_computer`.
- U7 `resetRun(place, script, agent)` (208-225): a box runs `withEnvFromInput(script)` through the door's exec with
  `RESET_EXEC_MS` and the env (`PATH` from the box's login report, then the agent's launch env) on stdin; no door →
  plain `NO_PLACE_DOOR`. Here: no local wiring → `absentComputer(name).sentence`; else a stream with idle and deadline
  `RESET_EXEC_MS`, env `PATH`, `HOME` (each where set) then the launch env; the lines joined by `\n`. Test
  `u7_reset_run_box_and_here`.
- U8 `planPlaces` and `planRoad` (228-255): here, a box (joined, connected when linked), or a provider (never connected).
  The road: a setup config dir wins; here, `local.home(agent)`; a box: the logins listed for that place and the agent's
  shared login home, else plain `resetNoLoginsLine(name, label)`; `launch` only when the setup names a program; `env`
  always `{}`. Test `u8_plan_road_table`.
- U9 `readPlansNow` (268-290): one read in flight shared; for every agent with plan resets and every computer here or
  joined: skipped for a provider, an unconnected computer, one not signed in, or one asked within
  `PLAN_READ_EVERY_MS` (the stamp set before the read); a reading with a limit is filed (`filePlanLimit`) and told to
  the alerts; a failure warns `"<label>'s limits on <name> were not read: <message>"`. Test `u9_plans_read_once_a_window`.
- U10 `reset(ask)` (306-309): `on` read as a place (`placeFor`), unknown → `here`; passed with `account` and
  `creditId` when given. Test `u10_reset_names_the_place`.
- U11 `used(q)` (313-322): the logs are read (failures warned `"the agent logs were not read for usage: <message>"`)
  where `outside` or a `source` split asks and `usageLogs` is on; labels per split: agent label, `"No project"` for
  `""` else the project's name, the computer's name, the account's label (the ledger's, overridden by the readings'
  names), the model's label, `USAGE_WORDS.outsideWsp` for `log` else `wspThreads`. Test `u11_used_labels_table`.
- U12 `readings(target, range)` (323-337): `sys.history` `{ op, from: now - span, to: now, stepMs }` to the workspace's
  daemon, this computer's, or a place's link (absent → `absentComputer`), answering `{ points, stepMs, from, to }`.
  Test `u12_readings_frame_and_answer`.
- U13 `accounts(ask)`: a `fresh` ask reads the plans first. Test `u13_fresh_accounts_read_the_plans`.

## What survives a restart

Nothing of this unit's own: the ledger keeps its rows in the store (wsp-machines). The log stamp, sign-in hold and
plan stamps are memory, so a restarted host reads logs and plans at its first ask.

## Tests

`tests/usage.rs` (30 tests). The expected values are the TS's: `oracle/usage.oracle.ts` runs `usageArea` at 65f6b89c7 on a
scripted context (the ledger, alerts, account rows, plan scripts and reset spender are recorders) over the worlds it writes
beside the answers in `tests/fixtures/oracle/usage.json`; `World` in the test file is the same context built from a world.
Frames compare as JSON strings (key order counts), entries and readings as values with integral numbers read as integers.

Interface changes made to write the tests (all in `src/account/usage.rs`): `UsageCtx::schedule(ms, f) -> Cancel` (the
bound `bounded` puts on a read; the area writes its own `bounded`: `<what> was not answered in <ms/1000 rounded>s`);
`agent_label` and `model_label` (the catalog's labels); `computer_name` split into `kept_computer_names` and a sync
`computer_name(places, id, kept: Option<..>)`; `account_rows(limits, places, name_of)` and `account_names(limits,
name_of)` take the limits and the namer the area passes; `NameOf` and `LabelOf` aliases (a bare `Rc<dyn Fn(&str)>` in an
`async_trait` method takes the method's lifetime and cannot be called); `LimitReading.at` is `Option<f64>` (a plan's own
read sets none); `account_of` takes `Option<&HarnessLimitAccount>` (the label rides it); `file_limit`'s before is
`Value::Null` where the ledger held none.

| Rule | Tests |
|---|---|
| U1 | `u1_away_line_table` |
| U2 | `u2_row_reason_order` |
| U3 | `u3_usage_stores_from_the_catalog` |
| U4 | `u4_logs_off_when_switched_off`, `u4_logs_read_once_a_window` (59,999 ms no, 60,000 ms yes), `u4_the_stamp_is_set_before_the_read`, `u4_logs_skip_wsp_sessions`, `u4_logs_project_and_account` (the snapshot handed to `log_limit` keeps the reply's key order), `u4_targets_are_this_computer_then_linked_boxes`, `u4_an_old_daemon_is_quiet`, `u4_a_silent_computer_times_out_with_its_label` (29,999 ms no, 30,000 ms yes), `u4_a_read_that_filed_nothing_is_not_filed`, `u4_a_stale_reading_is_not_told_to_the_alerts` |
| U5 | `u5_sign_ins_held_a_minute_and_dropped_on_change`, `u5_a_failed_or_absent_read_lists_none_and_is_held` |
| U6 | `u6_accounts_add_this_computer` (also the order: places, kept names, sign-ins, limits, rows) |
| U7 | `u7_reset_run_box_and_here` |
| U8 | `u8_plan_road_table`, `u8_plan_places_table` |
| U9 | `u9_plans_read_once_a_window`, `u9_plans_skip_a_vault_key_here`, `u9_a_failed_plan_read_warns_and_the_rest_go_on`, `u9_a_silent_plan_read_times_out_with_its_label`, `u9_one_plan_read_is_shared_while_it_is_out` |
| U10 | `u10_reset_names_the_place` |
| U11 | `u11_used_labels_table`, `u11_logs_are_read_where_outside_or_source_asks`, `u11_a_logs_failure_is_warned_and_used_answers` |
| U12 | `u12_readings_frame_and_answer` |
| U13 | `u13_fresh_accounts_read_the_plans` |

Ported TS cases: `test/usage.test.ts:146` and `:152` (one name per account: the merge of the ledger's labels and the
names read off the limits) in `u11_used_labels_table`; `:182` in `u6_accounts_add_this_computer`; `:207` and `:231` in
`u5_sign_ins_held_a_minute_and_dropped_on_change`; `:274` in `u4_logs_project_and_account`, `u4_logs_off_when_switched_off`
and `u11_logs_are_read_where_outside_or_source_asks`; `:308` in `u4_logs_project_and_account`; `:331` in
`u11_logs_are_read_where_outside_or_source_asks`; `:361` in `u4_logs_skip_wsp_sessions`; `:406`, `:415`, `:422` in
`u12_readings_frame_and_answer`; `:466` and `:494` in `u7_reset_run_box_and_here` and `u10_reset_names_the_place`;
`test/usage-boxes.test.ts:48` in `u4_logs_project_and_account` and `u4_targets_are_this_computer_then_linked_boxes`;
`:83` in `u4_logs_skip_wsp_sessions`; `:105` in `u4_targets_are_this_computer_then_linked_boxes` and
`u4_a_stale_reading_is_not_told_to_the_alerts`; `test/usage-reset-joined.test.ts:18` in `u7_reset_run_box_and_here`;
`:52` in `u9_plans_read_once_a_window` and `u13_fresh_accounts_read_the_plans`; `test/places-settings.test.ts:515` in
`u8_plan_road_table` and `u7_reset_run_box_and_here`.
