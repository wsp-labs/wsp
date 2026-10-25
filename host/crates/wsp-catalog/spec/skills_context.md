# skills_context (skills, context)

Source: packages/catalog/src/skills.ts (232) and context.ts (168) at a3b870a6e. Tests: skills.test.ts (4) and the
plugin seed cases of catalog.test.ts. Rust: tests/skills.rs (4), tests/oracle_data.rs (every context's files),
tests/oracle_text.rs (bundled skill names), tests/catalog.rs (the seed's links and the marketplace reads).

## What it does

Where each agent loads skills from and where an install puts one; the skills an agent's own install brings; how
Claude Code's plugins go to another computer (marketplaces, enabled plugins, install lines, the image's seed); and
how each agent loads the machine context on a guest (the always-loaded hook, the skill, and the fallback where the
person's own file already claims the hook).

## Behaviour

1. `own_skill_folder(roots, project)`: nothing where the agent already reads the shared folder (project or user);
   else its first folder (skills.ts:218). `skills_dir_of` is the first user folder. `is_system_skill` is `wsp` or
   `wsp-machine`.
2. Hermes's bundled names: each line's text before its first `:`, trimmed, empty ones dropped (skills.ts:68).
3. Claude's plugin road (skills.ts:120-214): `source_of` a github source's repo else its url; `github_repos` every
   github repo matching OWNER_REPO; `enabled` the `enabledPlugins` keys set to `true`, none off bad JSON; `install`
   `claude plugin marketplace add '<src>' || true` then `claude plugin install '<p>' --json </dev/null`; `asked` the
   accept-command sentence where the output names a `shownCommand` sha256; `uninstall`; `copied_before` the two
   index files under the guest config dir; `used_only` drops `extraKnownMarketplaces` no enabled plugin comes from,
   the text unchanged where it does not parse, names none or cannot be edited in place.
4. The seed (skills.ts:163-195): `build` runs the install under a fresh config folder with the seed as the plugin
   cache and exits with its code; `links` reads the seed's `installed_plugins.json`, each marketplace file once, and
   links `cache/<marketplace>/<name>/<first 12 of the pinned sha>` to the plugin's folder where they differ.
5. Context hooks (context.ts:326-400), each agent's files at mode 0644 with the skill at
   `<dir>/wsp-machine/SKILL.md`: Claude Code `<etc>/claude-code/CLAUDE.md`; Codex `<etc>/codex/requirements.toml`
   with `additional_developer_instructions` as a TOML literal multi-line string (a basic string where the text holds
   `'''`); Gemini the system defaults naming WSP-MACHINE.md, or an extension where the person sets
   `context.fileName`; OpenCode an `instructions` file pointing at the shared source; Pi APPEND_SYSTEM.md or an
   extension where one exists; Hermes a profile.d export (and a fish one) or not loaded where the person sets
   `agent.environment_hint`.
6. The conflict probes for Gemini, Pi and Hermes are the TS shell lines byte for byte (context.ts:305-323, 346, 371,
   391); Claude Code alone has the `cd` facts.

## Edge cases

- `toml_doc_string("")` is `'''\n'''`.

## Behaviour nobody wrote down

- Rule 4.26: `files` keeps the order each TS hook lists them.

## Tapes

image.build and workspaces.create land these files (#2443).

## Decisions

1. `PluginSeed::links` takes a `SeedReader` trait for the TS async `read` callback.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1 | skills: where_an_install_puts_a_skill_for_each_agent::*, the_folders_each_agent_loads_skills_from::*, the_skills_wsp_writes_itself::* |
| 2 | oracle_text: the_plugin_path_helpers_answer_as_the_ts_helpers |
| 3, 4 | catalog: catalog::claude_code_s_plugin_seed_only_a_plugin_its_marketplace_pins_to_a_commit_of_its, ::names_the_github_repos_the_marketplace_index_fetches_from_and_nothing_a_git_conf, ::claude_code_s_plugin_indexes_never_travel_they_name_this_computer_s_plugin_folde |
| 5, 6 | oracle_data: every_agents_context_lands_the_ts_files_with_and_without_a_conflict, a_document_holding_three_quotes_goes_into_toml_as_a_basic_string |
