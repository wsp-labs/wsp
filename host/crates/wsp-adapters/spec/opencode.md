# opencode
Source: packages/adapter-opencode/src/adapter.ts (304 lines), command.ts (83), catalog.ts (69) at a3b870a6e.
Tests: adapter.test.ts (14 cases), command.test.ts (7 cases). Rust: `src/opencode/{adapter,command,catalog}.rs`; tests `tests/opencode.rs`
over the scripted stream in `tests/common/codex_scripted.rs`; fixtures `turn.jsonl`, `model-not-found.jsonl`, `catalog-probe.txt` byte for byte.

## What it does
Runs one OpenCode turn as `opencode run --format json`, the prompt on a quoted heredoc and no stdin channel, and reads each JSON line
(`{type, timestamp, sessionID, ...}`) into the runtime's events: text parts once whole, a tool part once it completed or failed, and the
turn's end at the step whose reason ends the session loop. Also the probe: `opencode --version` then `opencode models --verbose`.

## Behaviour
1. `build_env` (command.ts:26): the base without absent values (an empty string kept), in order; then the key under `key_env` where
   both are set (an empty key too), overwriting a base entry of that name in its place.
2. `build_command` (command.ts:63) refuses, in this order: a prompt holding a line that is exactly `WSP_PROMPT_END` (`the prompt has a line
   that reads WSP_PROMPT_END, which ends the prompt`); a permission mode other than `auto` (`permissionMode must be auto, got "<m>"`);
   then while building: resume a slug `^[A-Za-z0-9][A-Za-z0-9._:-]*$` (`resume must be a plain slug, got "<v>"`); model
   `^[A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._:@/-]*$` (`model must be provider/model, got "<v>"`); effort a slug; each image
   starting `/` with no newline (`an image path must be one absolute path on the machine, got "<p>"`).
3. The words, space-joined: `agentWord("opencode", launch) run`, the launch words, `--format json`, `--print-logs --log-level ERROR`,
   `--dir '<cwd>'`, `-s <resume>`, `-m <model>`, `--variant <effort>`, `--title='<title>'` (shell-quoted), `--auto` for auto mode, then
   `-f '<path>'` per image. The line is `inFolder(cwd, "<words> <<'WSP_PROMPT_END'\n<prompt>\nWSP_PROMPT_END")`: no cwd is `cd ~ && ` with no `--dir`, an empty
   cwd is a folder (`cd '' && `, `--dir ''`, O9).
4. A prompt of any length travels on stdin (a 200,000-character prompt stays on the heredoc).
5. `start` (adapter.ts:252): refuses `opencode takes no context window`; an image with no path refuses `opencode reads images off the
   machine's disk; this one has no path on it`; launches with the env copied and no input; local id the resume id, else a new uuid;
   started at now; registered under the local id.
6. Lines (adapter.ts:166): a line that trims to start with `{`, parses to an object and has a string `type` is an event; anything else is
   stderr, trimmed, kept if non-empty, the last 10 only.
7. Announce: the first event of any type with a string `sessionID` (`""` included) sets the session id; events before it carry the
   local id; the local id and the registry key never change (O7). It and emits `session.start {sessionId, model?, cwd?}` (the
   start's model and cwd); once.
8. `text`: the part's text (string or ""); a text that trims empty is skipped; else it is the last text, emitted as a `text` delta with
   `messageId` where the part names a string `messageID`.
9. `tool_use` with a part: two deltas, `tool_use` (text the JSON of `state.input`, `{}` where absent; toolName the part's `tool` or `tool`;
   toolUseId `callID` where a string (`""` too), else `id` where a string, else ""; a string input is JSON-quoted, O1) and `tool_result` (text `state.error` when `state.status` is exactly `"error"`, else `state.output`, a
   non-string reading "" with no fallback; `isError` that failure).
10. `step_finish` with a part: the step is summed (cost; input, output, reasoning, cache read, cache write, each a finite number else 0);
    where `tokens` is an object, `held` becomes input + cache read + cache write + output of this step. Then, unless a result or a
    failure is already set, or the reason (default `unknown`) is one of `tool-calls`, `unknown`, `content-filter`: the turn completes
    (rule 12), `turn.done` is emitted and `end_after_result(result_exit_ms, grace)` started.
11. `error`: the first failure alone is kept (`??=`): its line is the logged error for the error's `data.ref` (rule 13), else
    `data.message`, else the error's `name`, else `opencode reported an error`; it claims sign-in where the name is `ProviderAuthError`, or
    `APIError` with `data.statusCode == 401`.
12. Completed is `{status, durationMs: now - started, costUsd, tokens, text?}`, tokens `{input: input + cache read + cache write, cached:
    cache read, cacheWrite: cache write, output, reasoning, context?: held}`.
13. The logged error (adapter.ts:96): the first stderr line seen before the error event that holds `ref=<ref> ` (with the space) and a
    logfmt `\berror="..."` field (`xerror=` is not it); each `\X` in it becomes `X` (`\n` is `n`, O5).
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
- `\w` and `\S` in rule 17 are JS's (ASCII word characters; JS whitespace).

## Behaviour nobody wrote down
- Every trim is JS `trim` (`wsp_words::js_trim`), never `str::trim`.

## Decisions
1. Time is `deps.now` where given. The TS reads `Date.now()` itself; `now` is the port's seam for tests, not TS behaviour.

## After the port
- `step_finish` with `reason: "length"` or any unlisted reason ends the turn as completed; a non-string reason continues.
- An empty `data.message` beats the name, so the failure can be `""`; a refusal with no message drops its words for the road alone.
- `callID: ""` shadows a usable `id`. The logged error is looked up when the error event is read, so a log line after it is never used.
- A completed-by-exit turn takes its duration at the exit, not at its last step. `stream.signalled` is never read.

## Rules added for the port (each value printed by the TS at 65f6b89c7)

- **O1.** Tool parts: `callID: ""` with `id: pid` draws toolUseId `""`; `tool: ""` keeps `""`, a non-string tool reads `tool`; no state
  draws `{}` then an empty, non-error result; a string input is JSON-quoted (`"\"raw\""`); `status: "Error"` is no error (text the
  output); `status: "error"` with a non-string error reads `""`, an error.
- **O2.** `reason: ""` ends the turn; `tokens: {}` gives `context: 0`; no `tokens` gives no `context`; the tokens are always the five
  fields, zeros where nothing summed, and `costUsd` is 0.
- **O3.** Lines after the result still emit (`session.start, turn.done, delta:text, session.end`) and a later error changes nothing.
- **O4.** An empty message with a sign-in cause reads as the road alone (`{failed, error: <road>, refusal: sign-in}`), for `APIError`
  401 and `ProviderAuthError`; a string `"401"` is no sign-in (`{failed, error: "Unauthorized"}`).
- **O5.** Lines `INFO ref=r1 hello`, `ERROR ref=r10 error="ten"`, `ERROR ref=r1 xerror="no" error="first \"q\" \n"`, then the error
  event naming `ref: r1`, fail as `first "q" n`.
- **O6.** The tail keeps the last 10 non-blank trimmed lines, a JSON array among them; the exit words join them with `\n`.
- **O7.** Resume `older`: a first text with no `sessionID` is said under `older`; `newer` announces; a later `third` changes nothing;
  `local_id` stays `older`, `session_id` is `newer`, the registry key `older`. `sessionID: ""` announces `""`.
- **O8.** Efforts are `Object.keys(variants)`: `{b, 2, a, 1}` reads `["1", "2", "b", "a"]`, and the probe's union keeps that order.
- **O9.** `cd ~ && <agent word> run --format json --print-logs --log-level ERROR --auto <<'WSP_PROMPT_END'\np\nWSP_PROMPT_END` with
  no cwd; `cd '' && ... --dir '' --auto ...` with `""`.
- **O10.** A stop after the result keeps it completed; the end reads the process's own code (143 from the teardown), `sawResult: true`.
- **O11.** `TurnResult` key order per road (no test; wsp-wire's struct order, the accepted difference ruled for claude): completed
  `status, durationMs, costUsd, tokens{input, cached, cacheWrite, output, reasoning, context?}, text?`; failed `status, error, refusal?`;
  interrupted `status`.

## Recorded sessions
`model-not-found.jsonl` and `catalog-probe.txt` are opencode 1.18.18's own output; `turn.jsonl` and every inline line are hand-written
(no real model turn ran). Replayed: `an_unknown_model_fails_the_turn_in_the_log_s_own_words_not_the_stream_s_server_e` (model-not-found),
the probe cases (catalog-probe). No recording yet of: a real model turn (text, tool and step lines), a tool that failed, a
`ProviderAuthError`, a step that asked for tools, an attach replay.

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
- O1 to O10: `rules_added::*` in `opencode.rs` (10 tests). O11: no test.
