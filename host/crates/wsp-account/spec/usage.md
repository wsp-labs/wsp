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
