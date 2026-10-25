# Unit claude-adapter: launches, probes and questions

TS at a3b870a6e: `packages/adapter-claude/src/adapter.ts:1-212, 627-645, 1272-1476` (about 420 lines). Rust:
`src/claude/adapter.rs`. Tests: the turn tests (claude-turn), `tests/claude_store.rs` (probe, title, rename, title
and draft questions through the adapter), `tests/claude_copy.rs` (aside), `tests/claude_landmines.rs` (sign-in order
through the adapter). Implemented in one packet with claude-turn.

## Construction (adapter.ts:627-641)

- `create_claude_adapter`: config_dir trimmed must start with `/`, else
  `Err("configDir must be an absolute path, got \"<config_dir as given>\"")`.
- signIn = `{base: base_env, api_key, oauth_token?, brokered?}`. env = build_env(signIn + project_dir_name).
  guard = sign_in_guard(signIn, launch). memory_dir = claude_memory_dir(config_dir, project_dir_name) where named.
- Declared capabilities (HarnessAdapter): steers, reports_edits, moves_access, steers_images, mcp_servers,
  denies_servers, waits_for_prompt, aside_servers all true; attachments Inline; compacts "/compact"; screen_commands
  = claude_screen_commands(); terminal_resume = terminal_resume_command(base_env, launch); env = env;
  forks_by_count false; no revert.

## Launch (adapter.ts:1284)

- copy: `copyOf = resume`, no resume. localId = resume or a new session id.
- valued = server_values_file(server_values). command = launch_command({options less resume, resume, copyOf,
  memory_dir, launch}, localId, valued is Some). A refused value is the start's `Err`, before any launch.
- launch line = command, prefixed by saved_spend_command(config_dir, resume) on a resume (not a copy).
- exec.launch(`<guard><launch line>`, {env: env copy, input: [user_message_line(prompt, localId, images)],
  input_after: prompt_after, secret: valued}).
- keep → keep_run(run) and follow on its first view; else follow on the run. follow{announced: false, fresh: resume
  is None, command: launch line}. The session's `command()` is the launch line without the guard.

## start (adapter.ts:1312)

No resume or no seed → launch. Else start_after_loss(options, `No conversation found with session ID: <resume>`,
launch): the session answers the running launch's ids, run, pid, command and tells_unread, delegates every action to
it, and `finished` is start_after_loss's. It is registered under the first launch's local id.

## warm (adapter.ts:1298) and the kept process

A new session id; launch_command of the warm options (no resume, no title) with memory and launch; exec.launch(guard
+ command, {env, input: [], secret}); keep_run; the first view's input closed at once (the process rests). `next` runs
the first turn with `fresh: true`, `saved: None`, the command recorded, sessionId = the warm id; `close(now)` =
keeper.close(now ? 0 : result_exit_ms, grace). No session file on a warm process.

## attach (adapter.ts:1419-1429)

Present only where the exec has attach. exec.attach(run, {input: true, started_at, from}); Gone → Gone (no event, no
session registered); a stream → follow{local_id: options.session_id, announced: true, fresh: false, from, steered}.
A rejection rejects.

## Probes and questions

- probe_catalog: exec(catalog_probe_command(launch), build_env({base: base_env})) then parse_catalog_probe.
- probe_version: exec(version_probe_command(launch), same env) then parse_version.
- session_title: exec(session_title_command(config, id)) with no env, parse_session_title.
- rename_session: exec(rename_command(config, id, title)), parse_rename. session_file = session_file_in(config, id).
- title_for: exec(title_for_command(title_prompt(opening, reply), model, launch, sign_in: guard), build_env(signIn)),
  parse_title_for. draft_for: the same with draft_for_command(prompt_file, ...), parse_draft_for. Note the question's
  env is build_env(signIn) without the project key, unlike a turn's.
- conversations = claude_conversations({config_dir, project_dir_name, launch, base_env}).

## fork_session (adapter.ts:1357)

A turn named by count → `Err("claude forks a session at the reply a turn ended on, and this turn named none")`.
Else copy_session through the anchor (env, grace = interrupt grace) and answer `Resume{resume: copy, drop}` where drop
runs copy_cleanup_command on the exec with env and waits its lines and exit.

## aside (adapter.ts:1366)

copy_session through `cut` or the live end. The command = aside_command({fork, cwd, model, effort, context window,
fast only where true, mcp servers, serverValues where valued, memory_dir, launch}); exec.launch(guard + command,
{env, input: [aside_hooks_line(), user_message_line(aside_prompt(question, running), fork)], secret}). A wall
(aside_wall_ms) ends the run (end_run, grace) and marks it walled. Per line: non-JSON non-blank lines are kept as the
last thing said; unknown control requests are refused (hook_callback with hook_deny_line, others control_error_line);
asks are refused with prompt_deny_line; the CLI refusing the hooks request (answer to `wsp-aside-hooks` with an error)
ends the run and that error is the answer; stream text pieces go to on_text until an answer is held; the first result
that is not a drained notice is the answer (aside_answer), the input closes and end_after_result runs. Every road out
waits the run's exit and then removes the copy (removal failures ignored). Answer: an error answer → `Err(words)`;
walled → `Err(aside_wall_line(wall))`; nothing → `Err(last line said, else harness_exit_line("claude", code, PATH,
{reached: false}))`.

## What survives a host restart

The adapter keeps nothing on disk. A re-attached turn's command is unknown (`None`). A side question or a fork copy
whose host went mid-way leaves its copy's file under the projects folder (nothing sweeps it).

## Rules added for the port (numbered; each value printed by the TS at a3b870a6e)

- **A1.** `warm` launches with `input: Some([])` (a line channel holding nothing) and no `input_after`; its first view's
  input closes at once and nothing reaches the process. The warm process has no `session_file`. The command ends
  `--model '<model>' --session-id <warm id>` where a model is named.
- **A2.** `next` on a warm process is a fresh session (the whole total is the turn's cost) under the warm id: local id
  and claude session id both the warm id, `command()` the warm command without the guard, registered under that id,
  its line `user_message_line(prompt, warm id, images)` with no uuid. `close(false)` waits `result_exit_ms`, then
  `closeInput` and `teardown`.
- **A3.** The start's own refusals (a bad slug, a window with no model, a session id that is no UUID) come back as the
  start's `Err` before anything is launched; `warm` refuses the same way.



| Rule | Test |
|---|---|
| relative config dir refused | `claude_turn::refuses_a_relative_config_dir` (ported) |
| launch line, env and guard on every launch | `claude_turn::spawns_with_the_landmine_safe_command_and_env` (ported) |
| resume reads the saved spend first; a new session reads none | `claude_turn_results::a_new_session_has_nothing_saved_so_the_launch_reads_no_file_and_the_turn_cost_its_whole_total` (ported) |
| prompt_after holds the seed with the run | `claude_turn::with_prompt_after_starts_the_cli_at_once_and_holds_the_seed` (ported) |
| lost session: new session seeded, note said | `claude_turn_results` lost-session pair (ported) |
| attach gone emits nothing; no attach without exec attach | `claude_turn` attach cases (ported) |
| fork through an anchor, never resume-at | `claude_turn_results::forks_a_session_through_a_turns_anchor_on_a_copy_and_never_resumes_at_a_message` (ported) |
| aside removes its copy on every road | `claude_copy` aside cases (ported) |
| the person's program everywhere, launch words on turns and asides only | `claude_turn_results::runs_the_program_everywhere_and_adds_the_launch_words_to_a_turn_and_its_side_question_alone` (ported) |
| capabilities declared | `claude_restart::the_adapter_declares_what_claude_code_takes` |
| A3: a refused value fails the start before any launch | `claude_turn_edges::a_refused_value_fails_the_start_before_any_launch` |
| A1, A2: a warm process and its first turn | `claude_kept::a_warm_process_rests_until_its_first_turn_which_is_a_fresh_session_under_the_warm_id` |
