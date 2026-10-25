# cursor
Source: packages/adapter-cursor/src/adapter.ts (264 lines), command.ts (64) at a3b870a6e.
Tests: adapter.test.ts (11 cases), command.test.ts (5 cases). Rust: `src/cursor/{adapter,command}.rs`; tests `tests/cursor.rs` over the
scripted stream in `tests/common/codex_scripted.rs`; fixtures `turn.jsonl`, `no-login.jsonl` byte for byte.

## What it does
Runs one Cursor turn as `cursor-agent -p --output-format stream-json --stream-partial-output --trust`, the prompt on a quoted heredoc
and no stdin channel, and reads each line into the runtime's events: init names the chat, streamed assistant text, tool calls started
and completed, and the result. A failure prints no event: its sentence goes to stderr and the process exits non-zero.

## Behaviour
1. `build_env` (command.ts:28): the base without absent values; the key under `key_env` only where both are set. Never `--api-key`.
2. `build_command` (command.ts:46) refuses, in order: a prompt line exactly `WSP_PROMPT_END`; a permission mode other than `default` or
   `force` (`permissionMode must be one of default, force, got "<m>"`); a model starting `-` or holding a newline (`model must be one model
   name, got "<m>"`); a resume not matching `^[A-Za-z0-9][A-Za-z0-9-]*$` (`resume must be a chat id, got "<r>"`).
3. The words: `agentWord("cursor-agent", launch) -p`, the launch words, `--output-format stream-json --stream-partial-output --trust`,
   `--workspace '<cwd>'`, `--model '<model>'` (shell-quoted), `--resume <id>`, `--force` for force mode only; then
   `inFolder(cwd, "<words> <<'WSP_PROMPT_END'\n<prompt>\nWSP_PROMPT_END")`; no cwd is `cd ~ && `, an empty one `cd '' && `.
4. `start` (adapter.ts:219): refuses `cursor takes no context window`, then `cursor takes its effort inside the model name, not as its own
   pick`; launches with the env copied and no input; local id the resume id else a new uuid; registered. No images (the adapter declares
   no attachments).
5. Lines: as opencode's rule 6, the stderr tail 5 lines; a stderr line matching `^Error: Authentication required\b` is kept as the
   no-credential sentence (the last one wins).
6. `system` with subtype `init`: the session id becomes the string `session_id` (`""` too; else it stays, C7) and `session.start {sessionId, model?, cwd?}` is
   emitted, every time such a line comes.
7. `assistant`: the text is every `message.content[].text` string joined with no separator; only a line with a `timestamp_ms` key and
   no `model_call_id` key (present means present: `null` counts, C5), and a non-empty text (blank is not empty), counts; parts are
   not filtered by type: it is added to the segment and emitted as a `text` delta (no messageId).
8. `tool_call` (adapter.ts:152): the call is the first key of `tool_call` in JS order (integer-like keys first) where its value is an
   object; where that first value is no object there is no call and the line is skipped, later keys never tried (C1); key `function` gives name its string
   `name` (else `function`) and input its string `arguments` (else `{}`); any other key gives name the key less one trailing `ToolCall` and
   input the JSON of `args` (`{}` where absent or null; a string is JSON-quoted, C2). No call skips the line. toolUseId the string `call_id`, else "". `started` empties the
   segment and emits `tool_use`; `completed` emits `tool_result` with the result (rule 9).
9. A result (adapter.ts:102): the first entry of `result` (object, else `{}`), defaulting to `success` with no value. Success: text the
   value's string `content` (`""` wins, C6), else `stdout`, else the JSON of the value (`{}` where none); not an error. Otherwise: why is the value's string
   `reason`, else `error`, else `stderr`, else the JSON of the value where there is one; text `<kind>: <why>`, or the kind alone; an error.
10. `result` (once): durationMs where `duration_ms` is a number; `is_error` exactly `true` fails (a string `"true"` completes) with the string `result` or `cursor reported a
    failed turn` (an empty string reads as none); else completed, always with a `text`: the segment, else the result's string, else `""` (C3), and tokens
    `{input, output}` from `usage.inputTokens`/`outputTokens` (non-numbers 0) where `usage` is an object. Emitted as `turn.done`;
    `end_after_result(result_exit_ms, grace)` started.
11. Exit: with no result, `interrupted` under a stop; else the no-credential sentence through `refused_turn` with `{cause: sign-in, road}`;
    else the stream's error, else `cursor-agent exited with code <code> before its turn ended[: <stderr joined by \n>]`. Then
    `session.end {exitCode, sawResult, failure?}`, the failure being the stream's own error, handed on as it was thrown.
12. `interrupt` marks the stop and `end_run(grace)`. `attach` attaches with `{input: false, started_at}`, reading model and cwd off the row.
13. Flags: no steers, no attachments, no MCP servers.

## Edge cases
- The two buffered repeats of streamed text (one with a model_call_id, one with no timestamp_ms) are never emitted.
- The reply is the text after the last tool call, not the result's own field, which runs every segment together unspaced.

## Behaviour nobody wrote down
- Every trim is JS `trim`; `\b` is JS's ASCII word boundary (`required-x` matches, `requiredX` does not).

## Decisions
1. None.

## After the port
- `init` emits `session.start` each time it is printed; a CLI printing it twice would say the start twice.

## Rules added for the port (each value printed by the TS at 65f6b89c7)

- **C1.** `{a: 5, readToolCall: {..}}` draws nothing; `{b: {..}, "1": {..}}` draws tool name `1`.
- **C2.** `ToolCall` names `""`; `fooToolCallToolCall` names `fooToolCall`; `args: null` is `{}`; `args: "s"` is `"\"s\""`; a function
  with `name: ""` and `arguments: ""` keeps both empty; one with no name is `function` and a non-string `arguments` `{}`; a numeric
  `call_id` reads `""`.
- **C3.** `result: ""` completes with `text: ""`; `is_error: "true"` completes; `is_error: true` with `result: "  "` fails with `"  "`.
  Every result carries `durationMs` 5 here, read off `duration_ms`.
- **C4.** The no-credential sentence is kept apart from the 5-line tail, so 7 later lines leave it standing; it beats a stream failure;
  an exit 0 with it and no result is the refusal too. Words: `<sentence>; <road>`, refusal sign-in.
- **C5.** `timestamp_ms: null` counts as streamed; `model_call_id: null` suppresses; `"  "` is said; parts of every type are joined
  (`hmm` + `c`), non-objects and non-string texts skipped; the reply is the segment `a  hmmc`.
- **C6.** Result texts: `{content: "", stdout: "out"}` is `""`; `success: "yay"` is `"\"yay\""`; `failure: null` is `failure: null`;
  `failure: {reason: ""}` is `failure: `; `rejected: {stderr: "no"}` is `rejected: no`.
- **C7.** An init with no `session_id` says `session.start` under the local id; `session_id: ""` makes every event's id `""`.
- **C8.** The tail is the last 5 non-blank trimmed lines, a typeless JSON line among them.
- **C9.** `TurnResult` key order per road (no test; wsp-wire's struct order, the accepted difference ruled for claude): completed
  `status, durationMs?, text, tokens?{input, output}`; failed `status, durationMs?, error`; refused `status, error, refusal`;
  died `status, error`; interrupted `status`.

## Recorded sessions
`no-login.jsonl` is cursor-agent 2026.09.26's own output (no credential, exit 1); `turn.jsonl` and every inline line are hand-written
from the output-format doc. Replayed: `no_credential_is_a_sign_in_refusal_the_cli_s_sentence_first_and_wsp_s_road_besid` (no-login).
No recording yet of: a real model turn (init, streamed assistant lines, tool calls, the result line), a refused or failed tool, a
result marked as an error, an attach replay.

## Tests (tests/cursor.rs, 17; TS 11 + 5, and 1 written for this port)
- Rules 1-3: `the_agent_print_line::*` (5), `a_person_s_setup_for_cursor_on_a_computer::*`.
- Rules 4, 13: `...::launches_the_print_line_with_the_key_in_the_environment_and_none_on_the_line_and` (flags asserted).
- Rules 6-10: `...::normalizes_a_turn_session_start_off_init_only_the_streamed_text_and_never_its_tw`,
  `...::a_tool_the_cli_refused_is_an_error_result_in_its_own_words_and_a_function_call_k`, `...::a_result_marked_as_an_error_fails_the_turn_with_its_own_text`,
  `...::a_process_that_lingers_past_its_result_is_ended`, `rules_this_port_pins::every_init_line_names_the_chat_and_says_the_start_again`.
- Rules 5, 11: `...::no_credential_is_a_sign_in_refusal_the_cli_s_sentence_first_and_wsp_s_road_besid`,
  `...::a_stream_its_transport_failed_hands_that_very_error_to_the_runtime_on_session_en`, `...::a_process_that_dies_before_its_result_fails_the_turn_with_what_it_printed_last`.
- Rule 12: `...::interrupt_ends_the_process_and_the_turn_reads_interrupted`, `...::an_attach_re_opens_the_run_with_no_channel`.
- C1 to C8: `rules_added::*` in `cursor.rs` (8 tests). C9: no test.
