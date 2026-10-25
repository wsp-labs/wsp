# detect_agents

Source: `packages/collect/src/detect/agents.ts` (52), `sources.ts` (91) at `a3b870a6e`. Tests: `agents.test.ts` (6), `sources.test.ts` (6). Rust: `src/detect/{agents,sources}.rs`; tests `tests/detect_agents.rs` (6), `tests/detect_sources.rs` (6).

## What it does

The agents rung's agent rows (each catalog agent found on the computer, with the config that travels and the scripts its hooks run), and the rc-file sources: the files a zsh or bash rc reads on a bare line, recorded on the rc row and guarded in the carried copy.

## Behaviour

1. `agents()` (agents.ts:23-26): every catalog agent in catalog order as `{id, label: name, bin, config: configPaths, volatile, hooks}`, then Aider (`aider`, `Aider`, bin `aider`, config `~/.aider.conf.yml ~/.aider.model.settings.yml ~/.aider.model.metadata.json`).
2. `detect_agents` (:31-39): per agent, `presence_of(config + hook_scripts, bin)`; present gives `entry(agents, agents/<id>, label, paths, bytes, volatile)`, where `entry` keeps only the volatile paths that are among `paths` (common.ts:65).
3. `hook_scripts` (:42-52): no `hooks`, or a hooks file `read_text` cannot read, gives `[]`; else each absolute file `hook_files(hooks, text, home)` names (an `Err` from it is returned as is and fails `detect_agents`, so the agents rung and `collect` fail with its message: see Decisions; text the catalog cannot parse as JSONC is no `Err`, `hook_files` answers `[]` for it and the row stands on its config), as `tilde(home, abs)`, skipped when it equals a config path or sits under one, kept when it stats as a file. Order kept.
4. `bare_sources` (sources.ts:41-58): split the text on `\r?\n`; a line is bare when `source_command(line)` answers and its `rest` matches `^[ \t]*(#.*)?$`, and `sourced_path(token, home)` answers; its path is `tilde(home, abs)`. Distinct paths in first-seen order.
5. `guard_sources` (:69-76): the bare lines to wrap are those whose token names the computer's home literally (`portable` rewrote it), whose path is outside home, or whose `~/` path `present(path without "~/")` says is missing. None: the text unchanged, byte for byte. Else the text is split on `\r?\n` and each such line becomes `<indent>[ -r <token> ] && <word> <token>` plus ` <comment>` when there was one; lines are joined with `\r\n` when the text holds any `\r\n`, else `\n`.
6. `portable` (:33-39): a token (bare, `"`- or `'`-quoted) whose unquoted text starts `home + "/"` becomes `"$HOME<rest>"` for a `"` token, `"$HOME"<rest>` for a bare one, and `"$HOME"'<rest>'` for a `'` token; any other token is unchanged.
7. `shell_sources` (:82-91): for each row whose id is `shell/<rc>` with `<rc>` in `RC_FILES` but not `inputrc`, read its first path (expanded); a readable text with any bare source sets the row's `sources`.

## Edge cases

- An agent on PATH with no config is still a row, with empty paths (`on_path_with_no_config_is_still_an_installed_agent`).
- A fish config and inputrc never get `sources` (`the_paths_land_on_the_rc_file_s_own_row_*`).

## Decisions

1. Ruled by the coordinator on #2446: a throw from `hook_files` fails the whole agents rung with the TS's words. Interface change: `hook_scripts` and `detect_agents` return `Result<_, HostError>`, and so do `run_detector` and the `Detector` type. The TS `hookFiles` throws `the file could not be changed in place, which keeps its comments; change it by hand` for a settings file whose repeated key sits on a path a hook edit takes (a repeated `hooks` key with `~/` commands in both blocks, a repeated event key, a repeated `command` key); the agents rung and `collect` fail with those words (`w2_oracle::detect_agents_answers_fails_and_reads_as_the_ts_did`). Text that is not JSON, a BOM, JSONC and a repeated key elsewhere (`model`) answer `[]` and the row stands.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/detect_agents.json` (`oracle/w2-detect_agents.oracle.ts`, every value printed by the TS),
replayed by `tests/detect_agents.rs` `w2_oracle::*`.

Corrections to the rules above, found against the TS (each overrides the rule it names):

- C1. Rule 3: the first agent whose `hook_files` fails ends the rung with that error; no later agent is asked.
- C2. Rules 2 and 3: `hook_scripts` runs for every agent with a `hooks` entry before `presence_of`, so Claude Code's settings file and Codex's config are read (and logged) on a computer that has neither agent.
- C3. Rule 5 (sources): the test is `rewritten || !path.startsWith("~/") || !present(path.slice(2))`, so a source of the home folder itself (path `~`) is wrapped; once one line is wrapped, every line is joined with `\r\n` when the text holds any `\r\n`, so mixed endings become CRLF throughout.
- C4. Rule 4 (sources): the SOURCE and TAIL regexes end in `.*`/`(.*)$` with JavaScript's `.`, so a line holding `\r`, U+2028 or U+2029 after the token is not a source line.

- W1. The agent table and row ids (`agents`, `agent_row_id`).
- W2. 52 computers through `detect_agents` with every read, the four repeated-key throws and their counter-cases; the same through `run_detector` and `collect` (`on_rung` stops before agents) (`detect_agents`, `run_detector`, `collect`).
- W3. Bare sources, guarded text with the paths `present` was asked about, and the rows `shell_sources` writes (`bare_sources`, `guard_sources`, `shell_sources`).

## Rules pinned

| Rule | Test |
|---|---|
| 1, 2 | `lists_the_catalog_s_agents_in_its_order_then_aider_*`, `claude_code_brings_settings_memory_skills_*`, `config_travels_without_its_login_file`, `agents_the_laptop_does_not_have_are_not_listed` |
| 3 | `claude_code_s_row_lists_every_local_file_a_hook_in_its_settings_runs_*`, `a_malformed_hooks_file::is_no_hook_scripts_and_the_agents_row_stands` |
| 4 | `a_line_that_is_one_source_or_dot_of_a_literal_path_*`, `a_guarded_redirected_chained_*` |
| 5, 6 | `wraps_each_bare_line_whose_file_is_not_present_*`, `a_line_naming_this_computer_s_home_literally_*`, `everything_present_leaves_the_text_as_it_was_*` |
| 7 | `the_paths_land_on_the_rc_file_s_own_row_*` |
