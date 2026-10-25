# launch: `threads/launch.ts` (167 lines)

Four pieces the start road and the composer use (launch.ts:212-358): `served_to`, `clouds_told`, the launch area
(`launch_picks`, `launch_fixed`, `warm`) and `seed_after_loss`.

## `served_to(wsp, root_thread_id, thread_id)` (launch.ts:212-215)

- **L1** No server: none. A thread that is its own root: the server unchanged.
- **L2** A thread under another: the server with `noSlate: true`, and where its `args` hold `SCOPED_MCP_ARG` the
  args gain `NO_SLATE_MCP_ARG` appended last; otherwise the args are unchanged. Key order: the server's own, then
  `noSlate`, then `args` overwritten in place (a spread keeps the existing key's position).

## `clouds_told(door, records, thread_id, prompt)` (launch.ts:221-227)

- **L3** `held`: the door's rows of kind `provider`, each named by `place_said`, sorted by code unit. No door: `[]`.
- **L4** A thread with no record, or a record with no `clouds`: no lines, the prompt unchanged.
- **L5** Otherwise one `cloud_added_line` per held cloud the record lacks, in `held` order, then one
  `cloud_removed_line` per recorded cloud no longer held, in the record's order; the prompt becomes
  `lines.join("\n") + "\n\n" + prompt`. No lines: the prompt is untouched (no blank lines added).
- **L6** Answers `held` whatever the record says; the record is not written here (the start road writes it once the
  turn launched, sessions_launch N22).

## `launch_picks(entry, harness, adapter, prefs, thread, asked)` (launch.ts:237-265)

- **L7** `table = harness_catalog(harness)`. With a table: `resolved = ctx.defaults_on(ctx.catalog_on(table, entry,
  adapter), prefs, prefs.projectDefaults[project])`; `catalog = resolved.catalog`. With none: no catalog, no defaults.
- **L8** `named = asked.permissionMode`, else `ctx.named_mode(catalog, harness, asked.access)` where an access is named.
- **L9** `picks_for(session)`: access is `named`, else (with a catalog) `listed_pick(catalog.permissionModes,
  ctx.access_of(ws, thread, session))`. `ran = ctx.ran_on(ws, thread, session)`. `kept = kept_picks(catalog, ran,
  {model?, effort?, contextWindow?} as asked)`. `open` is the defaults' `open` only for a new session (`session`
  none) on a thread that ran no model and no effort. `model = kept.model ?? open.model`, `effort` the same.
- **L10** `start_picks(catalog, {...asked, model?, effort?, permissionMode: access}, session.is_none(),
  session ? ctx.resumed_fact(ws, session, "model") : None)`; a refusal from it is replaced by
  `not_run_refused(catalog, model ?? asked.model)` where that answers one, else stands.
- **L11** The answer is the picks with `contextWindow: kept.contextWindow` added where present.

## `launch_fixed(entry, harness, cwd, mcp_servers, version)` (launch.ts:268-271)

- **L12** `JSON.stringify({ harness, cwd, mcpServers, version, setup })` where `setup` is
  `setups.launch_of(place, harness)` for `ctx.setup_place(entry)`, else absent. An absent value writes no key
  (`mcpServers` undefined, `setup` undefined); `version: null` writes `"version":null`. Key order as listed.

## `warm(workspace, asked, origin)` (launch.ts:314-332, 274-312)

- **L13** After `ctx.ready()`. A caller with a thread scope (`scope_of(origin)`): `none`, nothing read.
- **L14** `entry_of(workspace, origin)` (its refusal is the caller's). `none` where the kind keeps no agents, the
  kept box's cap is 0, or the host is closing.
- **L15** `prefs = ctx.preferences_held() ?? ctx.preferences()`; `harness = asked.harness ?? default_agent_of(prefs,
  entry)`; `none` where `agent_off(entry, harness)`.
- **L16** One warm in flight per `(workspace, harness)`: a second ask waits for the first to settle (either way)
  and then runs its own; the key is dropped once the newest run settles and only if it is still the newest.
- **L17** `warm_now`: a fresh thread id (Ids). A probe adapter with no env; no `warm` road: `none`. Then
  `confine_setup`, `launch_picks(..., picks_for(None))`, `thread_folder(entry, asked)`; a local workspace whose
  folder does not exist: `none`. `cwd = runs_in(entry, None, folder)`.
- **L18** `launch = { fixed: launch_fixed(entry, harness, cwd, None, catalog.version), picks }`. A standing warm agent
  (`warm_on(ws, harness)`) whose launch equals it both ways (`launches_as` each way): `rewarm` it, answer `standing`.
  A standing one that differs is reaped (`reap_kept`) first.
- **L19** A 16-byte hex turn token (Ids), then `thread_launch(entry, thread, thread)`. The adapter is built with env
  `{ WSP_TURN_TOKEN, WSP_THREAD, ...launchEnv }` in that order, the `waiting` box, the vault's server values and the
  thread. Its `warm(...)` gets the picks, `cwd`, `mcpServers: { wsp: served }` only where the adapter takes servers,
  the launch servers where it takes or denies servers, and `version` only where the catalog came from the harness
  with a non-null version. `hold_warm(thread, { workspaceId, agent, launch, turnToken, scopeDeviceId?, waiting,
  warm: { harness, claimed: false } })`; answers `started`.
- **L20** A failure after `thread_launch` revokes the scoped device (its own failure swallowed) and is the caller's.

## `seed_after_loss(adapter, exec, at, seed)` (launch.ts:340-358)

- **L21** The first call where the adapter has `session_held` starts, unawaited, one read of it with the session id
  and a harness exec bounded by `SESSION_TITLE_TIMEOUT_MS`, and warns `lost_session_log_line(session, thread,
  workspace, words)`; a read that fails warns with `words = "not read: <message>"`. Later calls read nothing.
- **L22** Every call answers `seed()`; neither the read nor its failure holds the seed back or fails it.
- **L23** An adapter with no `session_held`: no read and no line, ever.

## Tests (`tests/launch.rs`)

| Rule | Test |
|---|---|
| L1, L2 | `a_root_thread_keeps_its_server`, `a_child_gets_no_slate_and_the_scoped_arg_list_gains_the_flag`, `a_child_server_without_the_scoped_arg_keeps_its_args` |
| L3-L6 | `a_thread_with_no_record_is_told_nothing`, `added_then_removed_clouds_lead_the_prompt`, `nothing_changed_leaves_the_prompt`, `held_clouds_are_sorted` |
| L7-L11 | `picks_named_win_over_the_threads_own`, `a_thread_with_none_opens_on_the_defaults`, `a_resume_never_takes_the_defaults_open_picks`, `a_refused_model_reads_the_not_run_words`, `no_table_no_catalog` |
| L12 | `the_fixed_key_writes_only_present_keys_in_order` |
| L13-L15 | `a_threads_token_warms_nothing`, `a_kind_that_keeps_no_agents_warms_nothing`, `a_closing_host_warms_nothing`, `an_agent_turned_off_warms_nothing` |
| L16 | `a_second_ask_waits_for_the_first_and_one_stands` (thread-warm-agent.test.ts:124) |
| L17 | `an_adapter_with_no_warm_road_answers_none`, `a_missing_local_folder_answers_none` |
| L18 | `a_standing_agent_at_the_same_launch_is_rewarmed` , `a_changed_pick_reaps_the_standing_one_and_starts_another` (thread-warm-agent.test.ts:141) |
| L19 | `the_warm_launch_carries_the_token_thread_and_servers_in_order` |
| L20 | `a_failed_warm_revokes_its_device` |
| L21-L23 | `logs_one_line_naming_what_the_machine_holds` (lost-session-line.test.ts:102), `hands_the_seed_over_and_logs_why_when_the_reading_throws` (:114), `logs_nothing_and_reads_nothing_without_session_held` (:125), `a_second_ask_reads_nothing_more` |
