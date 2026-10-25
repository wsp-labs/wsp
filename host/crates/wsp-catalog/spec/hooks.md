# hooks (hooks, codex-hooks)

Source: packages/catalog/src/hooks.ts (183) and codex-hooks.ts (83) at a3b870a6e. Tests: hooks.test.ts (14). Rust:
tests/hooks.rs (14), tests/oracle_text.rs (hook_words).

## What it does

A copy of an agent's settings brings each hook script it names to the script's guest path, or takes the hook out
where the script cannot travel: Claude Code's JSON `hooks`, edited in place; Codex's top-level `notify`, edited line
by line.

## Behaviour

1. `path_of(word, home)`: an absolute word, or `~/`, `$HOME/`, `${HOME}/` expanded; else nothing. `on_machine`: under
   /bin/, /usr/bin/, /usr/local/bin/ or /usr/sbin/ with a name after it. `runs_a_file`: the word (or its basename for
   a path) is env, bash, sh, zsh, dash, node, bun, deno, python, python3, ruby or perl.
2. `place_word`: a word that is no path or is on the machine stands; else the placer's answer is the new word with
   the file carried, or the word is left.
3. Claude's command rewrite (hooks.ts:50-128): the line is cut into whitespace runs and words (quotes keep spaces in a
   word; a trailing `;` is its own token); `&& || | ;` start a new command; the first word of each command (after
   `NAME=` assignments) and, after an interpreter, its first non-flag word are placed; arguments pass through;
   quotes around a placed word are kept; the first refusal leaves the hook out.
4. Claude's carry (hooks.ts:134): a text that does not parse, has no `hooks` object, or moves nothing comes back byte
   for byte. Rewrites first, then removals from the last index back; a group whose hooks all went goes whole, an
   event whose groups all went goes whole, and `hooks` itself goes where every event went. Each carried file once.
5. Codex's carry (codex-hooks.ts:221): only a `notify` before the first `[` line counts; its array may span lines;
   an unclosed array stays as written. A program that is a path not on the machine takes the key out with the comment
   lines right above it; an interpreter's script is placed; a changed word is rewritten in place by
   `rewrite_toml_line`; nothing moved returns the text as it was.
6. `hook_files`: every carried file under home, by an identity placer.

## Edge cases

- `/usr/bin/` alone is not on the machine (no name after it).

## Behaviour nobody wrote down

- Rule 4.26: `carried` and `left` keep the order the hooks appear in.

## Tapes

image.build and project import copy settings (#2443).

## Decisions

1. `HookCarry::carry` returns a `Result`: Claude's in-place edit may refuse as `edit_json` refuses.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1, 2 | oracle_text: hook_words::each_word_reads_as_a_path_a_machine_command_or_an_interpreter_as_the_ts_reads_it; hooks: claude_code_s_hooks::a_path_the_machine_has_passes_through_unchanged_and_is_never_asked_of_the_placer |
| 3 | hooks: claude_code_s_hooks::asks_the_placer_about_the_first_word_of_each_simple_command_only_redirect_target, ::rewrites_every_path_word_the_placer_knows_written_as_home_home_or_in_full |
| 4 | hooks: claude_code_s_hooks::takes_out_a_hook_whose_script_the_placer_refuses_by_the_path_as_written, ::carries_the_hooks_of_a_settings_file_with_comments_in_place_every_comment_outsid, ::leaves_a_command_that_names_no_file_a_prompt_hook_a_file_that_is_not_json_or_has; oracle_text: hook_words::claude_and_codex_carry_their_hooks_as_the_ts_modules_do |
| 5 | hooks: codex_s_notify::* (6), claude_code_s_hooks::carries_a_notify_in_a_config_toml_full_of_comments_and_leaves_every_comment_it_d |
| 6 | oracle_text: hook_words::claude_and_codex_carry_their_hooks_as_the_ts_modules_do; hooks: the_catalog_s_hook_carries::are_registered_on_the_agent_entries_whose_settings_run_hooks_each_naming_a_file |
