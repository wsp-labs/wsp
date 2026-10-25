# plugins (plugins, plugins-claude)

Source: packages/catalog/src/plugins.ts (130) and plugins-claude.ts (287) at a3b870a6e. Tests: the Claude half of
plugins.test.ts (9). Rust: tests/plugins.rs (claude_codes_plugins_off_its_files, 9, and the_whole_read_is_the_ts_read,
4: every row of three reads held to tests/fixtures/plugins.json), tests/oracle_text.rs (the path helpers).

## What it does

Reads Claude Code's plugins off the files under its config folder and the image's seed, in at most three peeks of
the computer, and switches one by writing `enabledPlugins` in the user settings. Never runs `claude plugin list` or
`details`.

## Behaviour

1. Helpers (plugins.ts): `plain_values` the `"key": "..."` values with no backslash; `plugin_id_parts` split at the
   last `@` (none, or at index 0, is the whole id with an empty marketplace); `in_store` maps `~/<stateHome>[/...]`
   under the store where one is named, `~` to home, `~/x` under home, anything else as is; `join_path` joins and
   resolves `.` and `..` (never above `/`); `base_name` the last part after trailing slashes.
2. The first peek (plugins-claude.ts:190-197): the index, marketplaces and settings under the store; each project's
   two settings files; every plugin folder read off the index and the seeds' indexes by `installPath`; every
   marketplace folder off the marketplaces files by `installLocation`; the seeds' own two files. No answer refuses
   "plugins: Claude Code's plugin files could not be read".
3. No index and no seed index is no plugins and no refusal; an index that does not parse refuses "plugins: <path>
   does not parse, so Claude Code's plugins were not read".
4. Records: `installPath` absolute (trailing slashes dropped), scope user, project or local. Listed: each id's user
   install (with the seed's install beside it where a seed holds the id), each project or local install of a project
   the read covers; then each seed-only id. Marketplaces: the config's, overwritten by each seed's from the last seed
   to the first, so the first seed wins (plugins-claude.ts:206).
5. A second peek only for plugin folders and marketplace folders the first did not look under; a third only for
   folders and files a manifest declares outside its default places.
6. A row uses the seed's install where it is the seed's or the config's folder is not there (`seeded`). The manifest is
   plugin.json merged with its marketplace entry unless the entry is `strict: false`; the entry alone with no
   plugin.json. `on` is local over project over user settings, then false for a seeded row, then the entry's then the
   manifest's `defaultEnabled`, then true. A folder not there is `missing: folder` and brings nothing.
7. What a plugin brings (plugins-claude.ts:136-159): skills from `skills/` and the manifest's extra folders (a folder
   with SKILL.md is one skill; a plugin with SKILL.md at its root and no other skills is one skill named after it);
   commands and agents from the manifest's list or the default folder's `.md` files; hooks, servers (as
   `plugin:<prefix>:<name>`) and language servers as the keys of the default file, declared `.json` files and inline
   maps; each list sorted (UTF-16 order). `skills` dirs are the folders holding skills that are not themselves one.
8. `set_in` is the project's local settings where the local switch decided or the install is local and no shared
   switch, else its shared settings; none for a user row.
9. The switch (plugins-claude.ts:281): a road that writes no settings refuses "<id> was not switched: this road writes
   no settings file."; else `edit_json(text or "{}", [["enabledPlugins", id], on])` on the store's settings inside
   `store_base`.

## Edge cases

- The read under the home with the store-less fixture finds no index: no rows, no refusal.

## Behaviour nobody wrote down

- Rule 4.26: rows keep listing order: the index's order, then seed-only ids.

## Tapes

agents.read (plugins) and plugins.toggle (#2443).

## Decisions

1. `PluginIo` is a trait with async `peek`, `run` and `edit`; `can_edit` stands for the TS `edit?`.
2. `PluginShelf::turn` answers `Result<Option<String>, HostError>`: `Ok(Some(sentence))` is the TS `{ refused }`, which the host's
   `plugins-acts.ts:50` maps to a usage error (exit 3), and `Err` is what `io.edit` threw, kept with its kind so a refused
   write (exit 3) stays apart from a failed one (exit 1). The first draft answered `Option<String>` and lost the kind.

## After the port

- None found.

## Rules to tests

| Rule | Tests (plugins unless named) |
|---|---|
| 1 | oracle_text: the_plugin_path_helpers_answer_as_the_ts_helpers |
| 2, 5 | claude_codes_plugins_off_its_files::never_runs_claude_plugin_list_or_details_it_reads_files_alone, ::reads_every_plugin_missing_where_the_cache_is_not_there_as_on_the_boat_machine_k |
| 3, 4 | claude_codes_plugins_off_its_files::lists_the_owners_13_user_installs_out_of_18_plugins_in_30_records_6_on_and_none, ::reads_under_the_home_where_no_store_is_named; the_whole_read_is_the_ts_read::claude_codes_read_under_its_store_is_every_ts_row_field_for_field, ::claude_codes_read_under_the_home_finds_no_index_and_no_rows |
| 6, 8 | claude_codes_plugins_off_its_files::falls_back_to_the_plugins_defaultenabled_where_no_settings_file_names_it, ::merges_a_projects_switch_local_over_project_over_user_and_names_the_file_that_se; the_whole_read_is_the_ts_read::claude_codes_read_with_a_project_adds_the_projects_own_rows_field_for_field |
| 7 | claude_codes_plugins_off_its_files::brings_what_the_recorded_turn_loaded_for_each_plugin_the_turn_loaded, ::takes_a_plugin_with_no_plugin_json_from_its_marketplace_entry_its_language_serve |
| 9 | claude_codes_plugins_off_its_files::switches_by_writing_enabledplugins_in_the_user_settings_under_the_store_every_ot |
