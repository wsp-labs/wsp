# Unit claude-control-lines: the control channel, steers and hand-backs

TS at a3b870a6e: `packages/adapter-claude/src/{permissions,steers,handback}.ts` (470 lines). Rust:
`src/claude/{permissions,steers,handback}.rs`. Tests: `tests/claude_lines.rs` (ported permissions and steers cases),
`tests/claude_control_lines.rs` (hard cases). Everything here is per turn; nothing survives a host restart except what
`Steers::new` reads back off an attached run's input (`taken`) and the ids the row kept (`kept`).

## Written lines are exact bytes

Every `*_line` is `JSON.stringify` of the TS literal: keys in the literal's order, no spaces, `/` not escaped, non-ASCII
as is. serde_json with `preserve_order` writes the same for these shapes. Lines (permissions.ts):
- `control_answer_line` (:122): `{"type":"control_response","response":{"subtype":"success","request_id":<askId>,"response":R}}`
  where R is `{"behavior":"deny","message":<denyMessage>}` for a deny option; for a mode option
  `{"behavior":"allow","updatedInput":I,"updatedPermissions":[{"type":"setMode","mode":<option.mode>,"destination":"session"}]}`;
  otherwise `{"behavior":"allow","updatedInput":I}`. I is `question_answer_input(...)` for an answer option where it
  gives one, else `JSON.parse(ask.input)`, and `{}` where that parses to null. An id `picked_options` does not resolve
  is `Err("<optionId> is not an option on this permission prompt")`. Only the first picked option decides the effect.
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
  any other state (completed, cancelled, a new word) removes it and, where it had not started, records it dropped.
  Unknown uuids change nothing.
- `open = reports && any steer left`; `working = any steer started` (reports not needed).
- `settle()`: the first call fixes the unread list as `dropped ++ not started` where reports, else `[]`; later calls
  keep that list. Answers `reports && any not-started steer remains` on every call (not fixed).
- `tell_unread`: the first call emits `turn.unread{sessionId, ids}` where ids is the fixed list (or, unfixed, the list
  computed now) and non-empty; every later call emits nothing.
- `steered_ids(taken)`: walk lines in order; skip any that does not parse as an object or whose type is not "user";
  a user line with no string uuid clears the list; one with a uuid appends it unless present.

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
  no handback key → `summary`; with handback → the report's warning, else its text, else NO_REPORT_LINE. An empty
  summary is dropped.
- `end_answer`: only a notification with tool_use_id and task_id of a known subagent; it marks notified. With a
  handback key: answer is the report (warning, blank line, text where warned) or NO_REPORT_LINE; with none and a
  backgrounded call ending completed or failed: answer is `summary` or `""`; otherwise `[]` (notified still marked).
  Records `answered[call]`. Deltas: with handback and no report, a note `NO_REPORT_LINE` under the call; with a report,
  a text delta of the report under the call unless last_text for the call trims equal to it; then always the
  tool_result `{text: answer, toolUseId: call, isError: status == "failed"}`.

## Rules against tests

| Rule | Test |
|---|---|
| answer line bytes for allow, deny, mode | `claude_lines::allows_the_call_with_the_input_the_prompt_carried_unchanged` and the two after it (ported) |
| question answer written into input | `claude_lines::writes_an_answer_typed_in_other...` (ported) |
| unknown option refused | `claude_lines::refuses_an_option_the_prompt_never_offered` (ported) |
| options order, mode dedupe, null input | `claude_control_lines::ask_options_are_allow_deny_then_each_mode_once` |
| interrupt line bytes with and without cancel_queued | `claude_control_lines::interrupt_line_carries_cancel_queued_only_when_asked` |
| steers from taken then kept, once each | `claude_control_lines::steers_start_from_the_channel_then_the_kept_ids` |
| settle fixes once; tell_unread once | `claude_control_lines::unread_is_fixed_at_the_reply_and_told_once` |
| a CLI that never reports tells nothing | `claude_control_lines::a_cli_that_reports_nothing_tells_nothing_unread` |
| steered_ids reset at a uuid-less user line | `claude_lines::a_run_s_channel_read_back_names_the_steers_of_its_last_turn_alone` (ported) |
| framed report unframed; refused hand-back drawn | `claude_control_lines::a_framed_report_is_unframed_and_a_refused_handback_drawn` |
| task_ended word map and empty summary | `claude_control_lines::task_ended_maps_every_word_and_drops_an_empty_summary` |
| end_answer once per report, note with no report | `claude_control_lines::end_answer_says_a_report_once_and_none_in_words` |
