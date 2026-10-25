# context_render (machine-context.ts:304-547)

The document, the short text, the skill, each agent's hooks, and the run that lands them. Rust:
`src/machine_context/render.rs`.

## Renders (pure; byte for byte, oracle `render`)

- `render_machine_context(input)` (:341) and `render_short_context(input)` (:409): the TS line lists exactly, joined
  `\n`, ending in `\n`. Every branch the TS takes is pinned by the oracle's eleven inputs: kernel absent; versions
  empty; brew; golden-path with `on_npm_bin(golden.npm_bin, TOOLS_PATH)`; overlay false with and without docker or
  podman; overlay true with neither; wsp-open shim; at a provider (`SERVERS_OFF_LINE`); fish in the secrets file
  (`<sh> and <fish>`); workspace or builder; golden `v<version>, sealed <first 10 of createdAt>, setup <first 12 of
  setupSha>`, else `version not recorded` (workspace) or `not sealed yet` (builder); disk via wsp-words `fmt_bytes`
  with `TOOLS_DISK_FLOOR`; secrets, tools, agents, files lists joined `"; "` or `none`; aliases `name runs word`
  joined `"; "`, or the shared or late line where unread; tmux in the keep-alive line.
- Per agent: the turn line uses `background_work_words(agent.background_road)`; the cd fact and how-to are the
  agent's context module's `cd()` where it has one, else the shared words (:325, :327).
- `render_skill(doc)` (:440): frontmatter `name: wsp-machine`, `description: SKILL_DESCRIPTION`, a blank line, doc.
- `agent_files(agent, short, skill, probe, roots)` (:449): the catalog module's `hooks` with `source = context_path`,
  `conflict = probe.conflicts has the agent`, `fish = probe.has has fish`.
- `context_line(r)` (:469): `context.<agent>: <path>`; `... fallback <path>, <note>`; or
  `... not loaded, <note>; only the wsp-machine skill's description is in the prompt`; each then `; skill <skill>`.
- `summarize(results, agents, bytes)` (:498): no agents: `<bytes> written; no agent on the machine`. Else
  `<bytes> written for <names of written, by catalog name, joined ", ">` or `<bytes> written` when none was, then per
  result in order `<name> by its fallback (<note>)` or `<name> not loaded (<note>), skill only`, joined `"; "`.

## apply_machine_context (:514). Never fails; nothing changes on the guest when it reports a failure.

1. One exec of `probe_command(roots, path, shells)` with `timeout_ms = INLINE_EXEC_MS` (exec-detached, 30,000). An
   exec that fails: `failure = "probe failed: <message>"`. An answer without both markers:
   `"probe answered exit <code> without its markers"`. Either way `context` is empty and `summary` is
   `"not written (<failure>)"`.
2. `facts = merge_facts(probe.facts or empty, opts.result)`; the input carries workspace, golden and at_provider.
3. Files in this order: the short text at `context_path`, the skill of the long document at `skill_path`, the facts
   JSON (two spaces, newline) at `facts_path`, each mode `MODE` (0644); then, for each context agent **in catalog
   order** that the probe found, that agent's renders (with `agent` set) through `agent_files`, its files appended and
   its `ContextResult { agent, outcome, path, note, skill }` recorded. An agent the probe did not find is skipped.
4. One tar of every file (vault's `tar_of`, entries as files) landed through vault's
   `import_into(machine, tar, "/", VaultOptions { overlay: true, fetch: opts.fetch, .. })`. A failure:
   `"write failed: <message trimmed, every run of newlines as \"; \">"`, context empty.
5. Success: `ContextOutcome { context, summary: summarize(context, probe.agents, tar.len()), failure: None }`.

## Open points

1. machine-context.test.ts:416 injects a fake catalog agent with its own hooks; wsp-catalog's `AgentContext` is a
   closed enum, so it is not ported.
2. The upload road's words (`vault import upload failed: HTTP 413 ...`) are vault's; those two cases go green only
   with the provision slice's vault.

## Rules against tests

| Rule | Tests |
|---|---|
| long and short renders, every branch | `context_render::renders_a_workspace_on_a_sealed_golden`, `context_render::renders_a_builder_before_the_seal_with_nothing_missing_and_no_tmux_brew_or_fish`, `context_render::says_the_aliases_were_not_read_where_no_shell_was_opened_rather_than_none`, `context_render::the_short_always_loaded_text_carries_the_facts_an_agent_must_not_get_wrong_and_points_at_the_skill`, `context_render::a_machine_at_a_provider_says_servers_needing_the_persons_sign_in_do_not_run_there_and_no_other_does`, `context_render::names_the_base_floors_versions_when_the_probe_read_them_and_says_nothing_when_it_read_none`, `context_oracle::every_render_is_the_ts_text_byte_for_byte` |
| skill frontmatter | `context_render::the_skill_is_the_full_document_under_a_name_and_a_one_line_description_every_parser_reads` |
| hooks per agent, fallbacks, never a person's file | `context_render::writes_the_short_text_through_each_hook_and_the_skill_into_each_global_skills_directory`, `context_render::the_codex_string_is_a_literal_toml_*`, `context_render::gemini_and_opencode_get_json_that_names_ours_and_nothing_of_the_persons`, `context_render::hermes_reads_the_short_text_into_its_hint_*`, `context_render::falls_back_when_the_persons_own_file_sets_the_key_*`, `context_render::never_names_the_two_paths_that_would_replace_a_file_of_the_persons` |
| context line and summary words | `context_oracle::the_context_line_says_each_outcome_in_the_ts_words`, `context_oracle::the_summary_names_what_was_written_and_for_whom` |
| the run: probe, files, order, agents found only, failures change nothing | `context_render::probes_renders_with_facts_folded_in_lands_every_hook_and_skill_through_the_upload_road_and_records_each_agent`, `context_render::takes_the_fallback_for_a_claimed_hook_the_skill_alone_for_hermes_and_skips_agents_not_installed`, `context_render::reports_a_guest_that_does_not_answer_and_writes_nothing`, `context_render::a_refused_upload_is_a_failure_that_names_the_cap_and_nothing_lands`, `context_render::an_untar_that_fails_is_a_failure_with_the_guests_last_line` |
