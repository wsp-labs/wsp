# agents_launch: config folders, files a turn carries, environments, servers, the adapter

TS: `T/agents.ts:325-681` at `65f6b89c7`. Rust: `src/threads/agents/launch.rs`, methods on `AgentsArea`. Area state:
`setup_refusals: IndexMap<keyOf(place, agent), String>`, `cloud_servers: IndexMap<"<ws>\0<harness>\0<folder>",
Shared<Option<Vec<TurnServer>>>>`. `INLINE_EXEC_MS` 20,000 bounds every exec here. `here` is `HERE_PLACE_ID`.

## Where an agent's setup lives

A1. `setup_place(entry)` (`:327`): `here` for a local workspace, else the record's `place` (`None` at a provider).
`agent_off(entry, agent)` (`:329-332`): the person turned the agent off on that place (`ctx.setups().off`); never at
a provider. `agent_label(agent)`: the harness table's label, else the id.

A2. `config_folder_on(place, path)` (`:337-349`). On `here`: the home is `realFolderHere(local home dir, else the
process home)`; `kept` is `[realFolderHere(home/.wsp)]` plus `realFolderHere(dirname(resolve(statePath)))` when the
host has a state path; the folder is `realFolderHere(path)`. Elsewhere: `ctx.place_exec(place,
realFolderScript(path), 20000)`; exit 3 fails with the trimmed stderr, kind `usage`; the stdout's first two trimmed
lines are the folder and the home; a non-zero exit, a missing line or a home that is not absolute fails with `<place
name> did not say where <path> is: <trimmed stderr, else "exit <code>">`, kind `invalid`; else `{folder:
posix.normalize(folder), home, kept: [home/.wsp]}`.

A3. `setup_refusal(entry, named)` (`:362-366`): `named` defaults to `DEFAULT_AGENT`. No kept config folder for it on
the entry's place answers `None` without reading anything. Else (`keptRefusal`, `:367-375`) `configDirRefusal(label,
folder, config_folder_on(place, folder))`; a reason becomes `configDirLaunchRefusal(label, harness, folder, why)`.
The answer is remembered in `setup_refusals` (removed when it stands); when it differs from the one remembered,
`ctx.setup_refusal_moved(place, harness)` runs.

A4. `homes_here()` (`:376-386`): awaits `ctx.ready()`; for every catalog agent in catalog order: a kept folder is
checked (A3) and a refusal fails with kind `usage`; its home is the kept folder, else the local wiring's home for it,
else `agentHomes(process home)[id]`. `stores_here()` (`:390-394`): the agents whose home is not `<local home>/<its
stateHome>`.

A5. `confine_setup(entry, named)` (`:396-401`): settles at once with no kept folder; else a refusal fails it with
kind `usage`. `launch_adapter_for(entry, named, turn_env, ..)` (`:403-407`): `confine_setup`, then, unless the turn's
env carries `HOST_TOKEN_ENV`, `ctx.know_broker(entry, named ?? default)`, then `adapter_for`.

## Defaults and access

A6. `default_agent_of(prefs, entry)` (`:412-416`): `resolveThreadDefaults` with `firstAgent` the first catalog agent
that runs (has an adapter and is not off on the entry's place), else `DEFAULT_AGENT`; `runs` the same test; the
project's overrides when the entry's project has any.

A7. `defaults_on(catalog, prefs, project)` (`:421-425`): `withCustomModels(catalog, prefs.agentDefaults[harness].models)`
marked by `resolveThreadDefaults({firstAgent: harness, named: harness, ..})`: `{catalog: markedFor(..), open:
openDefaults(..)}`. Never cached.

A8. `named_mode(catalog, harness, word)` (`:428-432`): `accessMode(catalog, word)`; none fails with
`accessRefusal(catalog, word)` (a catalog of `{label, permissionModes: []}` when there is none), kind `usage`.

## Files a turn carries

A9. `land_images(entry, road, folder, dir, images)` (`:442-456`): no images or no road answers `{images: []}`;
`inline` answers each `{mediaType, bytes}` and lands nothing; `file` runs `landFilesLine(folder, dir)` (a non-zero
exit fails with `filesNotLandedLine(folder)`), then lands each image's bytes at `imagePathIn(dir, index, mediaType)`,
one after another, and answers `{images: [{mediaType, bytes, path}], dir}`.

A10. `land_files(entry, folder, dir, files)` (`:464-476`): none answers `[]` and runs nothing; else the same ready
line, then each file at `filePathIn(dir, name, taken)` (names deduplicated across the send), in order; answers the
paths.

A11. `drop_thread_files(entry, threads)` (`:481-492`): nothing unless the record's phase is `running`. Each thread's
record's `filesIn` folders, grouped by folder in first-seen order with their threads; one `dropFilesLine(folder,
threads)` per folder, in order; a failed exec warns `attached files of <threads joined ", "> not removed from <folder>:
<message>`; an exit code is not read.

A12. `drop_images(entry, dir)` (`:498-502`): `rm -rf <shellQuote(dir)>` detached; a failure warns `images for a
finished turn not removed from <workspace id>: <message>`.

## Environments and stores

A13. `agent_home(entry, id)` (`:506-509`): the kept config folder on the entry's place, else the kind's `home`.
`thread_env(entry, harness)` (`:514-518`): the kind's `env(entry, harness, agent_home)`, with the person's variables
on top: the harness's own, or with no harness every catalog agent's in catalog order (a later agent's value wins).
`place_stores(place, home)` (`:524-530`): for each catalog agent with a store variable, the person's value for that
variable, else the kept folder, else `ctx.place_agent_home(place, home, id)`.

A14. `turn_configs_on(entry, harness, folder)` (`:534-555`): `None` for an agent with no launch road or not an MCP
agent. Reads the checkout with `git <checkoutArgs(folder) quoted> 2>/dev/null` (a failed exec reads as none), the
config files `turnServerFiles(mcp, folder, HOME or "~", the store variable's value, top)`, the scopes (the user's,
then each project file with base the top when the file is under it, else its folder), then `readConfigsCmd(scopes)`.
On exit 0 with output not whole: fails `serverValuesCutLine(agent name, every file)`. Output that does not parse or a
non-zero exit: fails `serverValuesUnreadLine(agent name, readReason(res, 20))`. The first scope that did not read:
fails `serverValuesLeftLine(agent name, configUnreadLine(path, unread))`. Else `{road, configs: {user?, projects
(those read), folder, key: checkout key}}`.

A15. `server_values_for(entry, harness, folder)` (`:561-568`): `None` unless the kind hands values `launch` and the
vault holds a server value; `None` when the configs are `None`; the road's `fill`; all-empty answers `None`.

A16. `off_servers_for(entry, harness, folder)` (`:592-611`): `None` unless the entry's place is at a provider
(`atProvider`). The servers read once per `<ws>\0<harness>\0<folder>` and kept; a read failure warns `<harness> on
<machine>: its servers that need a sign-in were not read (<message>); the launch leaves none out`, is dropped from the
cache and answers `None`. With servers: the agents reader's `signsIn` over `addressesToAsk(..)` (none answers
nothing), and the distinct names of the servers `needsOwnCredential`, `[]` when none. The cache drops one workspace's
reads on `agents.changed` for that workspace (every read when it names no target), `workspace.deleted` and
`workspace.upgraded`.

A17. `launch_servers_for(entry, harness, adapter, folder)` (`:615-619`): `serverValues` when the adapter takes MCP
servers, then `offServers` when it denies servers; each only where set.

## The adapter

A18. `adapter_for(entry, named, turn_env, waiting, servers, thread, asks_until_stopped)` (`:632-667`): no factory for
the harness fails with `noAdapterLine(harness, <adapter ids>)`. The brokered launch is `ctx.brokered_launch(entry,
harness, turn_env[HOST_TOKEN_ENV])`; the vault is its vault, else `ctx.vault_on(entry)`. The factory gets: the
machine, the workspace id, `exec_stream: ctx.exec_factory_for(entry, limits, waiting)` with the place's turn limits,
plus `{thread, asksUntilStopped?}` when a thread is named and the kind ends threads (else `{asksUntilStopped}` alone
when set), plus `killedWhy` on a sub-thread's workspace; `home`; `env` = the servers' values (only unbrokered on a kind
that hands them by environment), then `thread_env`, then the broker's env, then the turn's env, a later one winning;
the person's `launch` setup when set; the kind's `projectKey` when it answers one; `signInRefusalLine({kind})`; the
vault; the broker's `claude` and `codex` parts when set; `loginStands`.

A19. `project_home(ref, origin)` (`:671-675`): the project resolved through the projects door; `None` when its
computer's kind does not run in a folder, and when the folder cannot be held now (the failure is dropped).
`computer_home(computer, origin)` (`:678-681`): the first listed project on that computer whose kind runs in a
folder, then `project_home`.

## Tests (`tests/agents_launch.rs`)

| Rule | Test |
|---|---|
| A1 | `setup_place_is_here_the_record_place_or_none` |
| A2 | `config_folder_on_a_joined_computer_reads_its_answer`, `config_folder_on_refuses_an_answer_with_no_absolute_home` |
| A3 | `setup_refusal_reads_nothing_without_a_kept_folder`, `setup_refusal_remembers_and_says_when_it_moved` |
| A4 | `homes_here_refuses_a_kept_folder_that_leads_out_and_stores_here_lists_moved_homes` |
| A5 | `launch_adapter_for_checks_the_folder_then_the_broker` |
| A6, A7, A8 | `default_agent_of_skips_an_agent_turned_off`, `named_mode_refuses_a_word_the_agent_has_no_mode_for` |
| A9 | `land_images_inline_lands_nothing`, `land_images_on_files_lands_each_in_the_sends_folder`, `land_images_fails_when_the_folder_is_not_made` |
| A10 | `land_files_names_each_once_and_lands_in_order` |
| A11 | `drop_thread_files_runs_one_line_per_folder_on_a_running_machine`, `drop_thread_files_warns_and_goes_on` |
| A12 | `drop_images_removes_the_dir_detached` |
| A13 | `thread_env_puts_the_persons_variables_over_the_kinds`, `place_stores_reads_variable_then_folder_then_default` |
| A14 | `turn_configs_on_refuses_a_cut_read_an_unread_read_and_a_scope_left` |
| A15, A17 | `server_values_for_fills_only_on_a_launch_kind_with_vault_values` |
| A16 | `off_servers_for_reads_once_and_drops_on_an_agents_change`, `off_servers_for_warns_and_reads_again_after_a_failure` |
| A18 | `adapter_for_refuses_an_agent_with_no_adapter`, `adapter_for_layers_env_servers_broker_then_turn` |
| A19 | `project_home_is_none_off_a_folder_kind_or_a_folder_not_held` |
