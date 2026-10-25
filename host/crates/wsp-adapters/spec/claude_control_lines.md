# Unit claude-control-lines: the control channel, steers and hand-backs

TS at a3b870a6e: `packages/adapter-claude/src/{permissions,steers,handback}.ts` (470 lines). Rust:
`src/claude/{permissions,steers,handback}.rs`. Tests: `tests/claude_lines.rs` (ported permissions and steers cases),
`tests/claude_control_lines.rs` (hard cases). Everything here is per turn; nothing survives a host restart except what
`Steers::new` reads back off an attached run's input (`taken`) and the ids the row kept (`kept`).

## Written lines are exact bytes

Every `*_line` is `JSON.stringify` of the TS literal: keys in the literal's order, no spaces, `/` not escaped, non-ASCII
as is. serde_json with `preserve_order` writes the same for the literals. A value parsed from text and written again
(`ask.input`, `updatedInput`) is written as `JSON.stringify` writes it: keys that read as array indexes first in
ascending order (wsp-wire's `js_key_order`, at every depth), numbers as `JsNumber::js_string`. The CLI's own lines
already hold that order, so this bites only on text a person or the runtime wrote. Lines (permissions.ts):
- `control_answer_line` (:122): `{"type":"control_response","response":{"subtype":"success","request_id":<askId>,"response":R}}`
  where R is `{"behavior":"deny","message":<denyMessage>}` for a deny option; for a mode option
  `{"behavior":"allow","updatedInput":I,"updatedPermissions":[{"type":"setMode","mode":<option.mode>,"destination":"session"}]}`;
  otherwise `{"behavior":"allow","updatedInput":I}`. I is `question_answer_input(...)` for an answer option where it
  gives one, else `JSON.parse(ask.input)`, and `{}` where that parses to null. An id `picked_options` does not resolve
  is `Err("<optionId> is not an option on this permission prompt")`. Only the first picked option decides the effect.
  A mode option with no mode writes no `mode` key (rule C2). `ask.input` that is not JSON is an `Err` too (the TS
  throws a SyntaxError; its words are not ported).
- `control_allow_line(ask)` (:139) is `control_answer_line(ask, "allow", "")`; on a question, which offers no allow,
  it is that `Err` (rule C3).
- `set_mode_line` (:156) `{"type":"control_request","request_id":..,"request":{"subtype":"set_permission_mode","mode":..}}`.
- `stop_task_line` (:164) `{..,"request":{"subtype":"stop_task","task_id":..}}`.
- `interrupt_line` (:171) `{..,"request":{"subtype":"interrupt"}}`, with `,"cancel_queued":true` inside the request only
  when asked.
- `control_error_line` (:178) `{"type":"control_response","response":{"subtype":"error","request_id":..,"error":"wsp answers no <subtype> control request"}}`.

## control_line (permissions.ts:59)

- `control_cancel_request` with a string request_id: Cancel; without one: `None`.
- `control_response`: needs `response.request_id`; Answer with `error` = `response.error` (or "the CLI refused it
  without saying why") only where `response.subtype == "error"`.
- `control_request`: needs a string request_id and an object request, else `None`. subtype (missing reads `""`) other
  than can_use_tool, or can_use_tool with no string tool_name: Unknown{request_id, subtype}.
- can_use_tool: options are allow (`{id:"allow",label:"Allow",effect:"allow"}`) then deny (`{id:"deny",label:"Deny",
  effect:"deny"}`), then one per `setMode` suggestion with a string mode, first of each mode only, id `mode:<mode>`,
  label the mode slug, effect mode, mode set. A non-array permission_suggestions reads as none. Where
  `asked_questions(toolName, input)` is Some, the options are `question_options(...)` instead (no allow, no deny).
  `input` is `JSON.stringify(request.input ?? null)` (absent input is `"null"`). `detail` = description where non-empty.
  `toolUseId` where a string. `agent_id` on the line, not on the ask; the turn maps it to parentToolUseId.
- `mode_option_on(ask, mode)`: the id of the first mode option with that mode, else `None`.

## Steers (steers.ts)

- `new(taken, kept)`: ids `steered_ids(taken)` then `kept`, deduplicated first-place, each not started.
- `read(event)`: an init line (type system, subtype init) ORs in `reports` from capability `msg_lifecycle_v1` and
  `cancels_queued` from `interrupt_cancel_queued_v1`, and answers false. A `command_lifecycle` line sets `reports`
  true and answers true whatever it names. For a known uuid: `queued` changes nothing; `started` marks it started;
  any other state (completed, cancelled, a new word, a number, none at all) removes it and, where it had not started,
  records it dropped. Drops are recorded in order and never deduplicated (rule C5).
  Unknown uuids change nothing.
- `open = reports && any steer left`; `working = any steer started` (reports not needed).
- `settle()`: the first call fixes the unread list as `dropped ++ not started` where reports, else `[]`; later calls
  keep that list, a report that comes later notwithstanding (rule C6). Answers `reports && any not-started steer remains` on every call (not fixed).
- `tell_unread`: the first call emits `turn.unread{sessionId, ids}` where ids is the fixed list (or, unfixed, the list
  computed now) and non-empty; every later call emits nothing.
- `steered_ids(taken)`: walk lines in order; skip any that does not parse as an object or whose type is not "user"
  (case-sensitive); a user line with no string uuid (absent, null, a number) clears the list; one with a uuid appends
  it unless present, `""` included (rule C7).

## HandbackBook (handback.ts)

- `note_line`: a `system/task_started` with a task_id records `owned[task] = parent_task_id` where named, a
  `local_agent` task as a subagent, and a backgrounded local_agent's `tool_use_id` in backgrounded. Any
  task_notification with a task_id removes it from owned. An assistant line with a parent_tool_use_id and an array of
  content stores each text block's text (last one wins) under that parent in last_text.
- `held_call`: true and held only for a call with parentToolUseId, toolName `SubagentHandback` and a toolUseId.
- `read_answer(answer)`: no toolUseId → `[answer]`. A held hand-back: unhold; text parsing to `{success: true}` → `[]`;
  else `[held call, answer]` with the answer's text replaced by `message` and isError true where message is a string.
  An answered launch: take it out; lead's own (no parentToolUseId) → `[]`, else `[answer with text = given]`.
  Otherwise a text starting `[Subagent hand-back] `: the following lines that start with two spaces, unindented,
  joined, trimmed, as the text; a frame with no indented line gives `""`. Anything else unchanged.
- `interim_end`: a notification for a known subagent with no `handback` key while some owned task's owner is it.
- `later_end`: a notification for a task already in notified.
- `task_ended`: system lines only. task_notification reads `status`; task_updated reads `patch.status` and counts only
  `killed`. Map: completed→done, failed→failed, stopped and killed→stopped, else failed. Summary on a notification:
  no handback key → `summary`; with handback (`null` counts as present) → the report's warning where it has a string
  one (a blank warning gives `""`, then dropped), else its text, else NO_REPORT_LINE. An empty summary is dropped. A
  notification with no string status, or a task_updated with no patch, is `None`; the status words are case-sensitive
  (rule C8).
- `end_answer`: only a notification with tool_use_id and task_id of a known subagent; it marks notified. With a
  handback key: answer is the report (warning, blank line, text where warned) or NO_REPORT_LINE; with none and a
  backgrounded call ending completed or failed: answer is `summary` or `""`; otherwise `[]` (notified still marked).
  Records `answered[call]`. Deltas: with handback and no report, a note `NO_REPORT_LINE` under the call; with a report,
  a text delta of the report under the call unless last_text for the call trims equal to it; then always the
  tool_result `{text: answer, toolUseId: call, isError: status == "failed"}`.

## Rules added for the port (numbered; each value printed by the TS at a3b870a6e)

- **C1.** `updatedInput` for an input of `0`, `false`, `""`, `[]`, `null` is `0`, `false`, `""`, `[]`, `{}`.
- **C2.** A mode option with no mode: `"updatedPermissions":[{"type":"setMode","destination":"session"}]`.
- **C3.** `control_allow_line` on a question: `Err("allow is not an option on this permission prompt")`.
- **C4.** `control_line` keeps `""` ids (`toolUseId`, `agent_id`) and drops ids that are no strings; a description
  that is no string is no detail; a mode suggestion needs `type: "setMode"` and a string mode (`""` gives
  `{id: "mode:", label: "", effect: mode, mode: ""}`), first of each mode only.
- **C5.** Drops: `a` (no state), `b` (state 5), `c` ("weird"), `a` again after a re-add, then `d` still open: told
  `[a, b, c, a, d]`.
- **C6.** A settle before any report tells nothing, ever.
- **C7.** `steered_ids`: `["", "a", ""]` gives `["", "a"]`; `[a, null, b, 5, c]` gives `[c]`.
- **C8.** `task_ended`: a kill is stopped with no summary; `Completed` is failed; no status is `None`; `handback: null`
  is NO_REPORT_LINE; a warning `""` gives no summary, a warning `W` gives `W`.

## Rules against tests

| Rule | Test |
|---|---|
| answer line bytes for allow, deny, mode | `claude_lines::answer_allows_the_call_with_the_input_the_prompt_carried_unchanged` and the two after it (ported) |
| question answer written into input | `claude_lines::an_answer_typed_in_other_is_written_into_the_input_and_a_question_with_no_choice_asks_no_consent` (ported) |
| unknown option refused | `claude_lines::answer_refuses_an_option_the_prompt_never_offered` (ported) |
| options order, mode dedupe, null input | `claude_control_lines::ask_options_are_allow_deny_then_each_mode_once` |
| interrupt line bytes with and without cancel_queued | `claude_control_lines::interrupt_line_carries_cancel_queued_only_when_asked` |
| steers from taken then kept, once each | `claude_control_lines::steers_start_from_the_channel_then_the_kept_ids` |
| settle fixes once; tell_unread once | `claude_control_lines::unread_is_fixed_at_the_reply_and_told_once` |
| a CLI that never reports tells nothing | `claude_control_lines::a_cli_that_reports_nothing_tells_nothing_unread` |
| steered_ids reset at a uuid-less user line | `claude_lines::a_runs_channel_read_back_names_the_steers_of_its_last_turn_alone` (ported) |
| framed report unframed; refused hand-back drawn | `claude_control_lines::a_framed_report_is_unframed_and_a_refused_handback_drawn` |
| task_ended word map and empty summary | `claude_control_lines::task_ended_maps_every_word_and_drops_an_empty_summary` |
| end_answer once per report, note with no report | `claude_control_lines::end_answer_says_a_report_once_and_none_in_words` |
| C1 | `claude_control_lines::an_answer_hands_back_any_parsed_input_but_null` |
| C2 | `claude_control_lines::a_mode_option_with_no_mode_writes_no_mode_key` |
| C3 | `claude_control_lines::allowing_a_question_unasked_is_refused` |
| C4 | `claude_control_lines::an_ask_keeps_empty_ids_and_reads_only_string_mode_suggestions` |
| C5 | `claude_control_lines::every_drop_is_told_in_order_and_never_deduped` |
| C6 | `claude_control_lines::a_settle_before_any_report_tells_nothing_ever` |
| C7 | `claude_control_lines::steered_ids_keep_the_empty_id_and_clear_on_a_line_with_no_string_uuid` |
| C8 | `claude_control_lines::a_task_end_reads_each_word_exactly` |
