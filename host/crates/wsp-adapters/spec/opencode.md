# opencode
Source: packages/adapter-opencode/src/adapter.ts (304 lines), command.ts (83), catalog.ts (69) at a3b870a6e.
Tests: adapter.test.ts (14 cases), command.test.ts (7 cases). Rust: `src/opencode/{adapter,command,catalog}.rs`; tests `tests/opencode.rs`
over the scripted stream in `tests/common/codex_scripted.rs`; fixtures `turn.jsonl`, `model-not-found.jsonl`, `catalog-probe.txt` byte for byte.

## What it does
Runs one OpenCode turn as `opencode run --format json`, the prompt on a quoted heredoc and no stdin channel, and reads each JSON line
(`{type, timestamp, sessionID, ...}`) into the runtime's events: text parts once whole, a tool part once it completed or failed, and the
turn's end at the step whose reason ends the session loop. Also the probe: `opencode --version` then `opencode models --verbose`.

## Behaviour
1. `build_env` (command.ts:25): the base without absent values, in order; the key under `key_env` only where both are set.
2. `build_command` (command.ts:62) refuses, in this order: a prompt holding a line that is exactly `WSP_PROMPT_END` (`the prompt has a line
   that reads WSP_PROMPT_END, which ends the prompt`); a permission mode other than `auto` (`permissionMode must be auto, got "<m>"`);
   then while building: resume a slug `^[A-Za-z0-9][A-Za-z0-9._:-]*$` (`resume must be a plain slug, got "<v>"`); model
   `^[A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._:@/-]*$` (`model must be provider/model, got "<v>"`); effort a slug; each image
   starting `/` with no newline (`an image path must be one absolute path on the machine, got "<p>"`).
3. The words, space-joined: `agentWord("opencode", launch) run`, the launch words, `--format json`, `--print-logs --log-level ERROR`,
   `--dir '<cwd>'`, `-s <resume>`, `-m <model>`, `--variant <effort>`, `--title='<title>'` (shell-quoted), `--auto` for auto mode, then
   `-f '<path>'` per image. The line is `inFolder(cwd, "<words> <<'WSP_PROMPT_END'\n<prompt>\nWSP_PROMPT_END")`.
4. A prompt of any length travels on stdin (a 200,000-character prompt stays on the heredoc).
5. `start` (adapter.ts:252): refuses `opencode takes no context window`; an image with no path refuses `opencode reads images off the
   machine's disk; this one has no path on it`; launches with the env copied and no input; local id the resume id, else a new uuid;
   started at now; registered under the local id.
6. Lines (adapter.ts:166): a line that trims to start with `{`, parses to an object and has a string `type` is an event; anything else is
   stderr, trimmed, kept if non-empty, the last 10 only.
7. Announce: the first event with a string `sessionID` sets the session id and emits `session.start {sessionId, model?, cwd?}` (the
   start's model and cwd); once.
8. `text`: the part's text (string or ""); a text that trims empty is skipped; else it is the last text, emitted as a `text` delta with
   `messageId` where the part names a string `messageID`.
9. `tool_use` with a part: two deltas, `tool_use` (text the JSON of `state.input`, `{}` where absent; toolName the part's `tool` or `tool`;
   toolUseId `callID`, else `id`, else "") and `tool_result` (text `state.error` when `state.status == "error"`, else `state.output`, a
   non-string reading ""; `isError` that failure).
10. `step_finish` with a part: the step is summed (cost; input, output, reasoning, cache read, cache write, each a finite number else 0);
    where `tokens` is an object, `held` becomes input + cache read + cache write + output of this step. Then, unless a result or a
    failure is already set, or the reason (default `unknown`) is one of `tool-calls`, `unknown`, `content-filter`: the turn completes
    (rule 12), `turn.done` is emitted and `end_after_result(result_exit_ms, grace)` started.
11. `error`: the first failure alone is kept (`??=`): its line is the logged error for the error's `data.ref` (rule 13), else
    `data.message`, else the error's `name`, else `opencode reported an error`; it claims sign-in where the name is `ProviderAuthError`, or
    `APIError` with `data.statusCode == 401`.
12. Completed is `{status, durationMs: now - started, costUsd, tokens, text?}`, tokens `{input: input + cache read + cache write, cached:
    cache read, cacheWrite: cache write, output, reasoning, context?: held}`.
13. The logged error (adapter.ts:96): the first stderr line holding `ref=<ref> ` whose logfmt `error="..."` field (backslash escapes
    allowed inside) is there; each `\X` in it becomes `X`.
14. Exit (adapter.ts:212): await the code; with no result: `interrupted` under a stop; else the failure through `refused_turn` with
    `{cause: sign-in, road: sign_in_refusal}` where it claims a cause; else, exit 0 and announced, completed; else `{failed, error: the
    stream's error, else "opencode exited with code <code> before its turn ended[: <stderr joined by \n>]"}`; emitted as `turn.done`.
    Then `session.end {exitCode, sawResult, failure?}`.
15. `interrupt` marks the stop and `end_run(grace)`. `attach` attaches with `{input: false, started_at}` and reads from the row's model and cwd.
16. Probe (catalog.ts:15-28): the version probe is `cd ~ && export PATH='<base PATH>'; <program> --version` (the export only where the
    base names PATH); the catalog probe adds `; echo __WSP_CATALOG_SEP__; <program> models --verbose`. Neither carries the launch words.
    `parse_version` is the last line of the trimmed text, trimmed, `None` where empty.
17. `parse_catalog_probe` (catalog.ts:46): no separator is `None`; after it, a line matching `^[A-Za-z0-9][\w.-]*/\S+$` opens a model and
    every other line joins its body; a body that parses to an object is `{slug, label: non-empty name else slug, efforts: keys of
    variants (`[]` where none), contextWindows: [], isDefault: false}`. No model is `None`; else `{version, models, efforts: the
    union in first-seen order, permissionModes: ["auto"]}`.

## Edge cases
- A step asking for tools is not the end: a process that exits 0 after it, announced, completes with the last text.
- A failure line and an exit with code 1 give the failure; a refusal reads `<words>; <road>` with `refusal: sign-in`.
- `\w` and `\S` in rule 17 are JS's (ASCII word characters; JS whitespace), rule 8.7.

## Behaviour nobody wrote down
- Rule 8.6 for every trim.

## Decisions
1. Time is `deps.now` where given.

## After the port
- Nothing.

## Tests (tests/opencode.rs, 23; TS 14 + 7, and 2 written for this port)
- Rules 1-4: `the_opencode_run_line::*` (6), `the_opencode_turn_s_environment::*`, `a_person_s_setup_for_opencode_on_a_computer::*`.
- Rule 5: `...::launches_the_run_line_with_the_picks_under_the_login_environment_the_prompt_on_t`,
  `...::an_image_reaches_the_line_as_a_file_on_the_machine_and_one_with_no_path_is_refus`.
- Rules 6-10, 12: `...::normalizes_a_turn_into_session_start_text_and_tool_deltas_turn_done_at_the_last` (tokens and context),
  `...::a_tool_part_that_failed_is_an_error_result_carrying_the_tool_s_own_error`, `...::the_turn_ends_at_the_step_whose_reason_ends_the_loop_and_a_process_that_lingers`,
  `...::a_step_that_asked_for_tools_does_not_end_the_turn_the_process_exiting_after_it_w`,
  `rules_this_port_pins::a_text_part_that_trims_empty_is_neither_said_nor_the_reply`.
- Rules 11, 13, 14: `...::an_unknown_model_fails_the_turn_in_the_log_s_own_words_not_the_stream_s_server_e`,
  `...::a_provider_that_refuses_the_credential_is_a_sign_in_refusal_with_wsp_s_road_besi`, `...::a_process_that_dies_before_any_event_fails_the_turn_with_what_it_printed`,
  `rules_this_port_pins::the_first_error_is_the_turns_failure_and_a_later_one_changes_nothing`.
- Rule 15: `...::interrupt_ends_the_process_and_the_turn_reads_interrupted`, `...::an_attach_re_opens_the_run_with_no_channel_and_takes_the_model_and_folder_off_th`.
- Rules 16, 17: `what_opencode_reports_about_itself::*` (2).
