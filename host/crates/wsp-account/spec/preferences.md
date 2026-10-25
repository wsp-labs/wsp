# preferences: the person's defaults and the agents read (`account/preferences.ts`, 156 lines)

The preferences record (read, patched, kept, pushed) and the wiring of the agents read (`agentsReads`, ported in
wsp-machines' `agents_read`). TS tests: `test/preferences.test.ts` (12), `test/thread-defaults.test.ts`,
`test/thread-keeps-picks.test.ts`, `test/older-agent-lists.test.ts`, `test/places-agents.test.ts`,
`test/project-icons.test.ts`. Case list in `tests/preferences.rs`' header.

`PreferencesCtx`: `env()` (the options' env, else the process's, read once at construction), `server_icons()` (the
folder and `forget() -> Result<(), String>`), `home_dir()`, `adapter_ids()` (map order), `harness_table(id)`,
`agent_label(id)`, `project_held(id) -> bool`, `default_agent_of(prefs)`, `session_thread_keys()`,
`thread_record_held(id)`, `ready().await`, store `get`/`put` of `preferences`/`PREFERENCES_ID`, `preferences_held()`
and `set_preferences_held(p)` (the `ctx.state` slot other areas read), `emit(event)`, and the agents-read members (C
below).

## Rules

- F1 `labs` (19) is read once from the env at construction and stamped on the record read from the store; a stored
  `labs` cannot outvote it.
- F2 `get()` (85): the held record (`ctx.state.preferencesHeld`) if any; else `preferencesFrom(stored)` with `labs`
  set after its keys, held from then on. A missing or malformed stored record reads as the defaults. Test
  `f2_get_reads_once_and_stamps_labs`.
- F3 Writes run one after another (17, 72-83): each waits for the one before (a failed one does not stop the next),
  then `ready()`, reads the record, applies the change, puts it, holds it, emits `{ type: "preferences.changed",
  preferences }`. Two sets of different fields at once both land. A change that throws writes nothing and fails only
  its caller. Tests `f3_two_sets_at_once_keep_both_fields`, `f3_a_refused_set_writes_nothing_and_the_next_runs`.
- F4 `set(patch)` (86-95): `applyPreferencesPatch(held, patch)`; where the patch names `threadsShut` and the merge
  has one, it keeps only the leads this host still holds (a session row's thread key, or a thread record) (67-70);
  then the defaults refusal (F5) against the merged record; the reply `{ preferences }`, plus `notice` where
  `serverIcons: false` was set and the folder stayed (F6). Tests `f4_threads_shut_keeps_held_leads_only`,
  `f4_set_answers_the_record`.
- F5 `defaultsRefusal(patch, next)` (35-64), every refusal a usage error, in order: `defaultAgent` with no adapter →
  `noAdapterLine(id, adapter ids)`; each `agentDefaults` entry not null: no adapter → same; an access word the
  harness table (or `{ label: agentLabel(id), permissionModes: [] }`) refuses → `accessRefusal`'s line; each model
  named (`model`, then `models.custom`) → `modelIdRefusal`'s line. Each `projectDefaults` entry not null: a project not
  held → `bareNoSuchProjectLine(id)`; its agent, its model; its access checked against the agent the merged record
  lands on (`next.projectDefaults[id].agent`, else `defaultAgentOf(next)`). A `null` access is no check. Tests
  `f5_defaults_refusal_table` (oracle-free: every sentence from the TS test file's own expectations plus
  `tests/fixtures/oracle/preferences.json`).
- F6 `iconsForgotten` (21-32): no icons → nothing; `forget()` ok → nothing; a failure becomes
  `serverIconsLeftLine(homeShortened(folder, home), said)` where `said` strips a leading `CODE: ` and a trailing
  `, call '<path>'`. The record is kept either way. Test `f6_icons_left_notice`.
- F7 The agents read (99-151): `workspace(id, origin)` answers `{ name, phase, local, machine, project: { id, name, path:
  checkoutOf }, stores, seeds, brokered?, pauseMode? }`; `stores` and `seeds` hold, for a non-local workspace, each
  catalog agent's state-home folder and plugin seed off the thread env where the catalog names one; `brokered` only
  when true. `projects(placeId)` waits for `ready()` and lists the place's projects with `checkout ?? path`.
  `latestOn` reads `agentVersions` off the record. `icons` asks the icon only while `serverIcons` reads on, at each
  ask. `channel(target)`: a workspace's daemon channel; this computer's local road; a place's link, or plain
  `absentComputer(nameOf ?? id).sentence`. `changed(target?)` emits `agents.changed` with the target when given.
  `setupOf` is nothing for an agent with no adapter. `setupWrite` refuses (usage `noAdapterLine`) an agent with no
  adapter, writes the setup, and where a refusal was held for that place and agent drops it and says it moved.
  `relayed` reads the backend's `callbackRelay`. A `workspace.deleted` event forgets the workspace in the read. Tests
  `f7_workspace_read_shape`, `f7_projects_per_place`, `f7_channel_targets`, `f7_setup_write_refusal_and_moved`.

## What survives a restart

The record (store `preferences`, id `PREFERENCES_ID`), less `labs`, which is the environment's each start. The
write chain and the held record are memory.

## Tests

`tests/preferences.rs`. Expected values come from `tests/fixtures/oracle/preferences.json`, written by
`oracle/preferences.oracle.ts`: it runs the TS `preferencesArea` on a fake context, once per script of sets (the
script stays in the fixture beside what it answered, put, emitted and read back after a restart), and calls the options
the area hands `agentsReads`. `check_script` holds every script to all of those.

Context changes made for the tests: `PreferencesCtx::project_of` (the agents read's `ctx.projectHeld(id)`, id and name)
and `setup_view` (`setupView(setups.get(...))`); area methods `agents_setup_of`, `agents_latest_on`, `agents_relayed`
(the TS options `setupOf`, `latestOn`, `relayed`, which F7 names and the first interface left out).

| Rule | Tests |
|---|---|
| F1, F2 | `f2_get_reads_once_and_stamps_labs`, `ported_labs_follow_the_env_the_area_was_handed` |
| F3 | `f3_two_sets_at_once_keep_both_fields`, `f3_a_refused_set_writes_nothing_and_the_next_runs` |
| F4 | `f4_threads_shut_keeps_held_leads_only`, `f4_set_answers_the_record` |
| F5 | `f5_defaults_refusal_table` (36 patches in order, every sentence the TS's) |
| F6 | `f6_icons_left_notice` (seven message shapes and four folders) |
| F7 | `f7_workspace_read_shape`, `f7_projects_per_place`, `f7_channel_targets`, `f7_agents_changed_names_its_target`, `f7_setup_write_refusal_and_moved`, `f7_setup_of_is_nothing_for_an_agent_with_no_adapter`, `f7_latest_on_follows_the_agent_versions_switch`, `f7_icons_are_asked_only_while_the_switch_reads_on`, `f7_relayed_reads_the_backends_answer`, `f7_a_deleted_workspace_is_forgotten_by_the_read` |

Ported TS cases:

| TS case | Test |
|---|---|
| `test/preferences.test.ts:29` | `ported_preferences_over_the_wire` |
| `test/preferences.test.ts:57` | `ported_a_dismissed_release_lands_on_the_record_and_the_next_area` |
| `test/preferences.test.ts:71` | `ported_usage_counts_read_on_until_turned_off_and_the_record_keeps_the_switch` |
| `test/preferences.test.ts:81` | `f5_defaults_refusal_table` (its `failed hook` script) |
| `test/preferences.test.ts:125` | `ported_a_record_kept_before_computers_had_icons_reads_with_none` |
| `test/preferences.test.ts:132` | `ported_two_patches_landing_at_once_keep_both_fields`, `f3_two_sets_at_once_keep_both_fields` |
| `test/preferences.test.ts:143` | `ported_labs_follow_the_env_the_area_was_handed` |
| `test/preferences.test.ts:159` | `f7_latest_on_follows_the_agent_versions_switch` |
| `test/preferences.test.ts:173` | `f6_icons_left_notice` |
| `test/thread-defaults.test.ts:168` | `ported_a_model_id_no_agent_runs_is_refused_and_nothing_of_the_change_is_stored` |
| `test/permission-relay.test.ts:540` | `ported_an_access_word_the_agent_maps_to_no_mode_is_refused_never_run_looser` |
| `test/thread-lines.test.ts:157` | `ported_a_lead_shut_on_the_record_and_a_write_drops_a_lead_the_host_does_not_hold` |
| `test/thread-lines.test.ts:170` | `ported_a_shut_section_stands_while_the_thread_record_does_after_its_rows_fell_off` |
| `test/places-agents.test.ts:39` | `f7_projects_per_place` |
| `test/places-agents.test.ts:363` | `f7_icons_are_asked_only_while_the_switch_reads_on` |
| `test/agents-brokered.test.ts:28`, `:42` | `f7_workspace_read_shape` |
| `host/test/verbs-threads.test.ts:392` | `ported_a_refused_access_word_leaves_the_stored_default_where_it_was` |
| `test/thread-defaults.test.ts:238`, `:251` | `f7_setup_of_is_nothing_for_an_agent_with_no_adapter`, `f7_setup_write_refusal_and_moved` |

Weak cases not ported because the unit does nothing in them (the patch schema or the thread start decides):
`preferences.test.ts:89, :111, :151`, `thread-defaults.test.ts:92 to :133, :180, :225, :259 to :288`,
`permission-relay.test.ts:520, :551`, `folder-threads.test.ts:326, :339`, `older-agent-lists.test.ts:126`,
`places-agents.test.ts:55 to :570` (but `:39`), `thread-keeps-picks.test.ts:132`, `editor-door.test.ts:55`,
`project-icons.test.ts:189`, `host/test/agents-cloud.test.ts:26`, `host/test/verbs-threads.test.ts:434`.
