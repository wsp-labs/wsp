# agents_lists: what each agent offers on a machine, and thread titles

TS: `T/agents.ts:26-324, 682-703` at `65f6b89c7`. Rust: `src/threads/agents/lists.rs`, methods on `AgentsArea`.
Area state (memory only): `catalogs: IndexMap<"<machine>:<harness>", {at, catalog: Shared}>`, `account_reads:
IndexMap<"<harness>:<version or empty>", {at, models: Shared}>`, `heard_versions: IndexMap<harness, version>`,
`agent_asks` (the asks in flight a close waits for), `title_reads: IndexMap<"<machine>:<session>", TitleRead {at, done,
live, failed}>`, `titles_asked: IndexSet<thread>`. Constants (`types/events.ts`): `CATALOG_TTL_MS` 300,000,
`CATALOG_PROBE_TIMEOUT_MS` 25,000, `FIRST_RUN_READ_MS` 3,000, `AGENT_VERSION_READ_MS` 3,000,
`SESSION_TITLE_TTL_MS` 10,000, `SESSION_TITLE_TIMEOUT_MS` 15,000, `SESSION_TITLE_REFRESH_MAX` 20,
`TITLE_MAKE_TIMEOUT_MS` 25,000. Collection `agent-lists`.

## The catalog

L1. `known` (`:60-76`): the table's catalog with, in this key order after its own keys: `steers`, `renames` (adapter
renames), `images` (adapter takes attachments), `mcpServers: true`?, `movesAccess: true`?, `steersImages: true`?,
`asides`, `compacts`?, `terminalResume`?, `rewindsConversation: true` (forks or reverts)?, `rewindsByCount: true`
(reverts)?, `forks: true`?, `forksByCount: true`?, `screenCommands` (a copy)?. An adapter with no catalog probe
answers `known` at once.

L2. `catalog_on(table, entry, adapter)` (`:58-156`), keyed `<machine id>:<harness>`:
1. an entry asked less than 5 minutes ago answers its catalog (failed probes included);
2. while the host closes, the entry's catalog (stale or not), else `known`; nothing is asked;
3. `installing = first_run_here(entry, harness)`, read once for the probe and the version check;
4. the probe (held as an agent ask): skipped when the first run would install (answers as no binary), else
   `probe_catalog(harnessExec(machine, 25000))`. No answer reads the account (L3). A refusal answers `{...known,
   refusal}`. An answer: its version is heard (L3) when the adapter reads account models, the version is set and newer
   (`compareVersions`) than the one heard; the answer is put to `agent-lists/<key>` detached (a failure warns
   `<harness> on <machine>: its lists were not kept for the next host (<message>); its first send there waits on the
   probe`); `catalogFromProbe(known, answer)` answers. A failed probe warns `<harness> on <machine>: the probe of the
   agent failed (<message>); wsp's built-in list answers until the next probe` and reads the account;
5. a probe that was skipped drops its entry once it settles, so the next ask looks again;
6. `before` is the stale entry's catalog, else the lists kept in `agent-lists/<key>` read through `catalogFromProbe`
   (one that does not read, or a store failure, is no lists);
7. the answer: no lists is the probe; lists not from the harness (`source` other than `harness`) are the lists; lists
   from a harness stand only while the binary answers their version (`probe_version(harnessExec(machine, 3000))`, held
   as an ask, skipped when installing, a failure read as no answer); no version reader or no version on the lists is
   the probe;
8. the entry `{at: now, catalog: answer}` is stored at once; once the probe settles, the entry (if still this one)
   holds the probe's catalog.

L3. The account's models (`fromAccount`, `:80-93`): an adapter with no account reader answers `known`. Read keyed
`<harness>:<heard version or "">`, once per 5 minutes (a failed read is none and is kept too); none or an empty list
answers `known`. When the list holds the table's default model, every row's `isDefault` is moved to that model.
Answers `catalogFromProbe(known, {version: null, models, efforts: known's values, permissionModes: known's values})`.

L4. `first_run_here(entry, harness)` (`:52-57`): an agent with no catalog `bin` is false; else
`installsOnFirstRun(run, [bin])` where `run` execs with `timeoutMs: 3000` and answers stdout on exit 0, else none
(an exec failure is none).

## Titles

L5. `refresh_title(view, force)` (`:167-224`): nothing (settled at once) for a row with no agent session, a workspace
not live, not `running` by `workspaceState`, unreached (`ctx.unreached_of`), or a harness with no adapter. Keyed
`<machine>:<session>`: a read in flight answers its future; without `force` a read less than 10 s old answers its
future. An adapter with no `session_title` does nothing. A read: `confine_setup` (agents_launch), then
`session_title(session, harnessExec(machine, 15000))`; when the store answers, success or failure, the entry is
stamped `at = now` and stops being live. A title: `failed` clears; for each row of the workspace on that session, the
source is `seed` when `storedTitleSource(title, row.prompt)` is `seed`, else `storedTitleSource(title, opening of the
thread)`; a `seed` title on a row whose own source is not `seed` changes nothing (and the row is remembered when it
carries another title); every other row takes the title and source, and a thread whose title moved is pushed. Then
`persist_sessions`, then, when forced, the remembered row's own title is written into the harness again
(`name_in_harness`, detached), then `push_head` for each moved thread. No title changes nothing. A failure warns
`noTitleLogLine(session, ws, providerSaid(e))` only when the entry had not failed before, then marks it failed. The
answered future never fails.

L6. `source_of(view)` is `titleSource`, `seed` when absent. `rows_on(thread)` is the thread's rows in map order.
The opening (`:233`) is the prompt of the row with the smallest `startedAt` (absent 0, the first on a tie).
`thread_source(thread)` is `person` when any row is, else `auto` when any is, else `seed`. `carried_title(thread)` is
the newest titled row's `{harnessTitle, titleSource}`, empty when none.

L7. `name_in_harness(view, title)` (`:257-273`): nothing for no session, a workspace not live or not running, no
adapter, or an adapter that renames nothing. Else `launch_adapter_for`, then `ctx.write_session(session, rename(session,
title, harnessExec(machine, 15000)))`; an answer `failed` warns `noNameWriteLogLine(session, ws, error)`, a failure
warns the same with the message. Never fails.

L8. `make_title(view)` (`:284-314`): nothing for a row with no thread, a workspace not live, a thread already asked, a
row with no prompt, a thread not `seed`, not running, or no adapter. `launch_adapter_for` (its refusal fails the call);
an adapter with no `title_for` does nothing. The thread is marked asked. The model is `smallestModel` of `catalog_on`
when the harness has a table. Nothing more while closing. `title_for({opening: prompt, model?},
harnessExec(machine, 25000))`: none warns `noMadeTitleLogLine(thread, ws, "the harness answered with no title")`; a
thread that a person named meanwhile is left alone; else every row of the thread takes the title with source `auto`,
`persist_sessions`, `push_head`, then `name_in_harness`. In memory only: a restarted host asks once more.

L9. `title_rows(rows)` (`:317-323`): newest `startedAt` first, the first row of each agent session (rows with none
left out), at most 20.

## Lists

L10. `agent_lists(entry)` (`:689-703`): the table catalogs whose harness has an adapter and is not turned off on the
entry's computer. No entry or a phase other than `running` answers them as they are. Else each, at once: the config
folder check (`setup_refusal`); a `PlaceAbsent` failure answers the table's catalog; any other failure or refusal
answers `{...catalog, refusal}`; then `ctx.know_broker(entry, harness)`, whose failure answers `{...catalog, refusal:
<message>}`; else `catalog_on`. In table order.

## Tests (`tests/agents_lists.rs`)

| Rule | Test |
|---|---|
| L1 | `known_carries_the_adapters_abilities_in_order`, `an_adapter_with_no_probe_answers_known` |
| L2 | `catalog_on_probes_once_per_machine_and_ttl`, `catalog_on_keeps_the_answer_for_the_next_host`, `catalog_on_answers_kept_lists_while_the_binary_answers_their_version`, `catalog_on_skips_the_probe_when_the_first_run_installs`, `catalog_on_answers_the_refusal_with_known_lists`, `catalog_on_warns_on_a_failed_probe_and_answers_the_table`, `catalog_on_asks_nothing_while_closing` |
| L3 | `account_models_answer_where_the_binary_said_nothing_and_keep_the_tables_default` |
| L4 | `first_run_here_reads_the_installer_without_running_the_agent` |
| L5 | `refresh_title_reads_once_per_window_and_keeps_it_on_every_row_of_the_session`, `refresh_title_keeps_a_persons_name_over_the_seed`, `refresh_title_warns_once_until_a_read_answers`, `refresh_title_does_nothing_on_an_unreached_machine` |
| L6 | `thread_source_is_person_over_auto_over_seed`, `carried_title_is_the_newest_titled_row` |
| L7 | `name_in_harness_writes_through_write_session_and_warns_on_refusal` |
| L8 | `make_title_asks_once_per_thread_and_names_every_row`, `make_title_leaves_a_thread_a_person_named_meanwhile` |
| L9 | `title_rows_asks_the_newest_row_of_each_session_up_to_twenty` |
| L10 | `agent_lists_answers_the_table_off_a_paused_machine`, `agent_lists_answers_a_refused_config_folder_as_its_refusal` |
