# codex_adapter
Source: packages/adapter-codex/src/adapter.ts:104-249, 513-519, 1229-1454; turn-pages.ts (116); lost-session.ts:59-96 at a3b870a6e.
Tests: adapter.test.ts's side question (5), rewind and fork (16), kept server (2), copy (2), lost session (1), attach (1), finished
process (2) cases; catalog.test.ts's 4 brokered probe cases; placeholder-login.test.ts's 3 adapter cases; session-title.test.ts's and
session-held.test.ts's adapter cases (2). Rust: `src/codex/adapter.rs`, `turn_pages.rs`, `lost_session.rs`; tests `tests/codex_adapter.rs`.

## What it does
Assembles the adapter: the turn's environment, launching a turn (start, resume, copy, fork, kept), re-opening a run a later host attaches
to, the side question on an ephemeral fork, side runs that revert a thread or find a fork's turn, the lost-session fallback, and every
read outside a turn (probe, version, account models, title, rename, draft, session held, conversations).

## Behaviour
1. `create_codex_adapter` (:513): `env = build_env({base, home, api_key, broker})`, an error refusing the adapter; grace defaults to 5000.
   The question environment (:1331) is `env` where brokered, else `build_env({base, home})` (no key).
2. `start` (:1262): refuses, in this order, the first that applies (CA1): `codex takes no context window` where one is named (`""` too); maps each image through its path (no path:
   `codex reads images off the machine's disk; this one has no path on it`, then `image_path`'s refusal); access from the permission mode (`access_params`' refusal, `""` refused too);
   local id a new uuid for a copy, else the resume id (`""` is a resume, no slug check, CA2), else a new uuid; config is `server_values.config` (empty is none); thread options
   `{cwd?, model?, serviceTier: fast where fast, access, config where non-empty}`; a fork with a resume refuses `a codex start forks a thread or
   resumes one, never both`. The thread line: fork → `thread_copy_line(session, last_turn_id: turn)`; no resume → `thread_start_line`; copy →
   `thread_copy_line`; else `thread_resume_line`. All refusals come before anything launches.
3. The launch: `build_command({cwd, mcp_servers, off_servers, launch, broker}, now)`; exec with `env` copied, input
   `[initialize_line, INITIALIZED_LINE, ACCOUNT_READ_LINE, rate_limits_read_line(limit_details), thread line]`, and `secret {input: true}`
   where config is non-empty. With `keep`, a background `{commands: {}, held: None}` and `keep_run(run, between = ended_between)`; the turn
   reads `keeper.turn()`'s view. `follow` gets the command, cwd, thread options, `turn_line_for(prompt, images, effort)`, prompt_after.
4. `start` goes through `start_seeded` (rule 8). The session is registered under its local id; a kept server's `next` registers a
   second entry under the thread id beside it (CA3).
5. `attach` (:1397): only where the exec factory attaches; attaches with `{input: true, started_at, from}`; gone answers gone; else
   `follow` with the row's session id as local id, started_at, model, cwd; a `from` above 0 also gives `opened {threadId: session id}`;
   a row prompt gives an owed turn `turn_line_for(prompt, [], effort)`. The attached session has no command.
6. Side question `aside` (:1303): command `build_command({cwd, launch: {program} only, broker})`; input `[initialize, initialized,
   thread_fork_line(session, cwd, model, ASIDE_INSTRUCTIONS)]` (no account or limits read); follow as `aside` with turn line
   `turn_start_line(thread, question)` and no events. A result that is not completed rejects with its error (a failed one with no words
   is `codex reported a failed turn`), else `codex did not answer the question` (interrupted); completed answers `{text or "", usage: tokens where any}`.
7. Side runs (:1320): command with `side_run: true` (TS `turn: false`) and the program only; input the handshake then the run's lines; follow with the side run.
   `revert` and `fork_session` are `revert_on` and `fork_session_on` over it.
8. `start_seeded` (lost-session.ts:81): where the start resumes with a seed and is neither a copy nor a fork, `start_after_loss` with
   `no_rollout_line(resume)` = `codex could not open the thread: no rollout found for thread id <id>`; the session answered reads the launch
   running now (thread id, command, run, pid, from, steer, answer, interrupt, stop_task, kept) and finishes with the turn's end; it is
   registered under the first launch's local id, and the second launch stays registered under its own fresh id too (CA4). The seed
   or the second exec throwing rejects `finished` with no `turn.done` and no `session.end`. Else the launch alone.
9. `revert_on` (turn-pages.ts:44): a count that starts at 0 (no turn the server opened) writes nothing past the resume and answers
   `Cut` (CA5). Input `thread_resume_line({cwd, access: read-only}, session)`. By count, `remaining` is the number of
   cut turns the server opened (`server_opened`: an anchor, or no refusal and an error not starting `codex could not `). On the thread
   answer: the thread id updates; a `legacy` history ends at once; a count with turns remaining pages `thread_turns_list_line(cursor,
   min(remaining, 100))`, else the cut. Each page walks ids newest first, the last one taken is the boundary, stopping at zero; zero
   remaining cuts with `thread_revert_line`; no next cursor fails `CODEX_FEWER_TURNS`; a cursor already asked fails `PAGE_AGAIN`. A legacy
   thread, or a failure holding `only supports paginated threads`, answers `Kept(CODEX_LEGACY_HISTORY)`; any other non-completed result
   rejects with its error (default `codex did not cut the thread`).
10. `fork_session_on` (turn-pages.ts:89): pages with `limit: 100` from a null cursor; an anchor matches by id; else it skips the turns after
    it that the server opened and takes the next. Found answers `(session, turn)`. No more pages fails `codex_no_turn_line(anchor)` or
    `CODEX_FEWER_TURNS`; a repeated cursor fails `PAGE_AGAIN`. With an anchor, a failure holding `only supports paginated threads` forks at
    the anchor as given; else it rejects with the error (default `codex did not find the turn to fork the thread at`).
11. A side run never writes `thread/start`, `thread/fork` or `turn/start`; stdin closes at its end.
12. Probes (:1389-1451), each on the question env: `probe_catalog` runs `catalog_probe_command(launch, broker)` then
    `read_catalog_probe(stdout, login, account models)`, the account models being the broker's only in ChatGPT mode; `probe_version`
    `parse_version`; `account_models` exists only in ChatGPT mode with a read handed; `session_title` runs `session_title_command(home, id)`
    with no env; `session_held` likewise; `rename_session`, `title_for` (`title_prompt(opening, reply)`, model), `draft_for` on the
    question env. `session_file` is `rollout_in(home, id)`. `compacts` is `/compact`.
13. Flags (:1392): attachments file, steers, reports edits, MCP servers, denies servers, waits for prompt, forks by count; `env` is the
    turn's environment.

## Edge cases
- A brokered revert or fork lookup runs on no login (`cli_auth_credentials_store="ephemeral"`, `features.plugins=false`) and names no model
  address at the door: `openai_base_url` and every provider's `base_url` point at `http://127.0.0.1:0` with the mode's path
  (`/backend-api/codex` on ChatGPT, `/v1` on a key); `chatgpt_base_url` stays at the door. A side question uses the full brokered
  turn build, not the no-login one.
- A side question's fork asks neither the account nor its limits.

## Behaviour nobody wrote down
- Rule 1.4: `now` is `deps.now` where given.

## Decisions
1. The adapter exposes inherent methods; `impl HarnessAdapter` is written over them once the port settles (as the claude adapter does).

## After the port
- A kept `next` and a lost session each leave a second registry entry nothing removes.
- A lost session whose seed or second launch throws ends with no `turn.done` or `session.end`.
- Side runs (revert, fork lookup) have no wall: a silent server holds them for good.
- An attach with a prompt but no `taken`, or a turn already in `taken`, writes no owed turn and waits for a stop.
- `attach` is decided once at creation: a factory that gains it later is never used.

## Rules added for the port (each value printed by the TS at 65f6b89c7)

- **CA1.** Refusal order, first match wins, nothing launched: the context window (`""` counts); each image in order (`rel` gives
  `an image path must be one absolute path on the machine, got "rel"`, one with no path `codex reads images off the machine's disk;
  this one has no path on it`); the permission mode (`""` gives `permissionMode must be one of read-only, workspace-write,
  danger-full-access, got ""`); then fork with resume (`a codex start forks a thread or resumes one, never both`, copy or not).
- **CA2.** `resume: ""` is a resume: local id `""`, `thread/resume` with `threadId: ""`, registry key `""`.
- **CA3.** A kept server's `next` follows under the thread id as its local id; the registry then holds `[first local id, thread id]`,
  and nothing new launched.
- **CA4.** A lost session whose second launch's thread is refused: the result is `{failed, error: "codex could not open the thread:
  boom on second"}` (no `durationMs`), the caller sees only `turn.done` and `session.end {exitCode: 0, sawResult: true}` under the
  second launch's fresh id, two launches ran, `local_id` stays the resume id, and the registry holds `[resume id, fresh id]`.
- **CA5.** A revert by a count of 250 pages with limits 100, 100, 50 and cuts before `r249`; a count of 0 writes only
  `initialize`, `initialized`, `thread/resume` and answers `Cut`.
- **CA6.** An attached run that never printed: thread id is the row's; `steer` not running after the join, `answer` Gone,
  `stop_task` not running, `kept()` none; `interrupt` writes nothing and tears down once; the result is `interrupted`, said as
  `turn.done` then `session.end {exitCode: 0, sawResult: false}` under the row's id.

## Tests (tests/codex_adapter.rs, 33; TS 5 + 14 + 4 + 1 + 4 + 3 + 1 + 1)
- Rules 1, 12: `a_brokered_codex_launch_adapter_cases::the_adapter_hands_a_title_its_brokered_word_and_the_turns_own_environment`,
  `the_title_codex_makes_for_a_thread::runs_under_the_sessions_own_codex_home_which_a_guest_exec_would_not_carry_read_o`,
  `what_the_machine_holds_for_a_session_codex_would_not_resume::is_what_the_adapters_own_reading_answers_through_the_exec_it_is_handed`,
  `a_probe_on_a_machine_whose_chatgpt_sign_in_the_host_holds::*` (4).
- Rules 2-4: the launch cases of `codex_turn.rs` `codex_adapter_over_codex_app_server::*`; copy: `a_codex_thread_picked_up_from_outside_wsp::runs_a_copy_on_thread_fork_written_to_disk_as_a_thread_of_its_own_and_turns_on_t`.
- Rule 5: `codex_turn_more.rs` `a_turn_a_later_host_attaches_to::*`.
- Rule 6: `a_side_question_on_a_codex_thread::*` (5).
- Rules 7, 9, 11: `what_a_rewind_needs_of_a_codex_turn::*` (8), `a_brokered_codex_launch_adapter_cases::a_revert_or_a_forks_turn_lookup_*` (2).
- Rule 8: `a_codex_turn_that_does_not_complete::a_resume_whose_rollout_is_gone_starts_a_new_thread_handed_the_thread_so_far_as_c`.
- Rule 10: `a_fork_of_a_codex_thread::*` (6).
- Kept servers: `a_codex_thread_picked_up_from_outside_wsp::lets_go_of_*` (2), writer held: `...::fails_a_turn_whose_thread_another_process_writes_in_two_halves_and_sends_no_turn`.
- Rule 13: `codex_turn.rs` flag asserts (`denies_servers`, `waits_for_prompt`, `steers`).
- CA1 to CA6: `rules_added::*` in `codex_adapter.rs` (6 tests).
