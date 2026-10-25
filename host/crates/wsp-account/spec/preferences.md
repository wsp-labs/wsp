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
