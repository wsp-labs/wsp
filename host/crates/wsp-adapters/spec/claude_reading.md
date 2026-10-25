# Unit claude-reading: the loose-field readers

TS at a3b870a6e: `packages/adapter-claude/src/{fields,tokens,limits,tool-results,shell-cwd,plans,subagent-spend}.ts`
(364 lines). Rust: `src/claude/{fields,tokens,limits,tool_results,shell_cwd,plans,subagent_spend}.rs`. Tests:
`tests/claude_lines.rs` (tool-results, shell-cwd cases), `tests/claude_reading.rs` (hard cases). Pure functions; no
state survives anything but the books the caller holds for one turn.

## fields (fields.ts)

- `rec`: an object only. Array, null, string, number, bool and absence are `None` (fields.ts:5).
- `string`: any JSON string, `""` included. `num`: any JSON number (fields.ts:15 rejects NaN and infinities, which
  JSON cannot carry). `str_arr`: an array's string items in order, every other item dropped; `Some(vec![])` for an
  array of none, `None` for anything not an array (fields.ts:19).
- `harness_of(init)`: `slash_commands` (str_arr), `permissionMode` (string), `agents` (str_arr). `None` only where all
  three are absent; an empty array is present (fields.ts:23-33).
- `parse_line(raw)`: trim; must start with `{`; must parse; must be an object; its `type` must be a string. Anything
  else is `None`, never an error (fields.ts:35-46). A line `{"type":1}` is `None`.

## tokens (tokens.ts)

- `input_tokens = input_tokens + cache_creation_input_tokens + cache_read_input_tokens`, each 0 where absent or not a
  number. `held_tokens = input_tokens(usage) + output_tokens`.
- `usage_tokens`: `{input: input_tokens(usage), output, cached?, cacheWrite?}`. cached is set only where
  `cache_read_input_tokens` is a number, cacheWrite only where `cache_creation_input_tokens` is. Never reasoning,
  context or window (tokens.ts:19-23).
- `plus_tokens(None, b) = b` unchanged, context and window included. Otherwise input and output add; each of cached,
  cacheWrite, reasoning is present where either side has it, absent sides counting 0; context and window are dropped
  (tokens.ts:25-29).
- `usage_of`: every count present, absent ones 0 (tokens.ts:32). UsageTokens holds u64: the counts are whole tokens.

## limits (limits.ts)

- Kinds (limits.ts:8): five_hour→session, seven_day→week, seven_day_opus→week_opus, seven_day_sonnet→week_sonnet,
  seven_day_overage_included→overage, overage→overage. Any other type name adds no window.
- Status (limits.ts:16): allowed→ok, allowed_warning→warning, rejected→reached; any other word leaves `status` off.
- `limit_of(None) = None`. Windows come from `unifiedWindows` in its key order, then the top-level reading named by
  `rateLimitType` (the info object itself). A window needs a numeric `utilization`; the first window of each kind wins
  and later ones of that kind are dropped (so `overage` after `seven_day_overage_included` is dropped).
  `usedPercent = round(utilization * 1000) / 10` with JS `Math.round` (halves round up: 0.12345 → 12.3, 1.5 → 150),
  `resetsAt = resetsAt * 1000` where numeric. No window read is `None`. `account`, `plan`, `keyed` are never set here.
- `note_rejected(None)` does nothing. Type key is `rateLimitType` or `""`. Overage counts where `isUsingOverage ===
  true` or `overageStatus` is `allowed` or `allowed_warning`. Not rejected, or overage: remove the key. Rejected with
  no overage: set `key → resetsAt * 1000` or `None` (limits.ts:42-53). Re-setting an existing key keeps its place.
- `with_limit`: only a `failed` result with a non-empty map changes; `limit = {resetsAt: max}` where every value is
  `Some`, else `limit = {}` (TurnLimit with no reset). Every other field is kept (limits.ts:56-60).

## tool_results (tool-results.ts)

- `flatten_content`: a string as is; an array as each block's `text` (blocks with no string text give `""`), empty
  ones dropped, joined by `\n`; anything else `""`.
- `result_facts(blocks, text, is_error, tool_use_result)`: only where exactly one block of `blocks` has
  `type: "tool_result"`, else all `None` (tool-results.ts:46).
  - Error: `exit_code` only where tool_use_result is a string starting `Error: Exit code ` and `text` matches
    `^Exit code (-?\d+)(\n|$)`; the code is that number. Nothing else on an error.
  - Not an error and tool_use_result an object: a patch where `filePath` is a string and `structuredPatch` an array.
    Hunks are the entries with numeric oldStart, oldLines, newStart, newLines and a `lines` array (non-string lines
    dropped), others skipped. With no hunk, `type: "create"` and non-empty string `content` give one hunk,
    `whole_file_hunk(content, '+')`. Still no hunk: no patch. A patch is `[{path, hunks}]` and wins over output.
  - Otherwise, numeric `persistedOutputSize` and string `stdout`: `text = [stdout, stderr]` with empty ones dropped,
    joined by `\n`, and `bytes = persistedOutputSize`. Either missing: nothing.

## shell_cwd (shell-cwd.ts)

- No tool name or a non-object input: `None`.
- Bash: `command` (non-string reads as `""`) must match
  `^\s*cd\s+(?:"([^"$\x60]*)"|'([^']*)'|([^\s;&|$\x60*?[{~]+))\s*(?:$|&&|;|\|\||\n)` (JS regex: `$` is end of input,
  no multiline). The target must be absolute; normalized by posix rules (`.`, `..`, doubled slashes) and a trailing
  slash dropped unless it is `/`. Answers it where it differs from `current`.
- Write and Edit: `file_path` absolute, normalized; its folder (posix dirname). `None` where the folder is
  `harness_cwd` or not within it (`within`: equal, or starts with `folder/`; under `/` everything is within), and
  where the shell left `harness_cwd` and the folder is within `current`. Any other tool: `None`.

## plans (plans.ts)

`read_plan_call` answers NotPlan (TS undefined), Nothing (TS null), Steps or Text.
- TodoWrite: `todos` must be an array, else NotPlan; non-object entries dropped; each step `content` (or `""`) and
  state completed→done, in_progress→working, anything else→pending. An empty array is `Steps(vec![])`.
- ExitPlanMode: `plan` string → Text, else NotPlan.
- TaskCreate: where `id` is Some, `unnamed[id] = tasks.len()`; push `{id: None, text: subject or "", pending}`;
  answer the whole list as Steps.
- TaskUpdate: find the first task whose id is Some and equals `taskId` (a string); none → Nothing. `status:
  "deleted"` removes it; otherwise status (if the key is present at all, any value through the state map) and
  `subject` (if a string) replace. Answer the whole list. Note: a status key present with a non-string value maps
  to pending (plans.ts:46).
- TaskList, TaskGet: Nothing. Any other name or no name: NotPlan.
- The caller (normalize.rs) adds the call id to `calls` and names a task from its result; see claude_normalize.md.

## subagent_spend (subagent-spend.ts)

- `note_child_call`: needs `message.id` (string) and `message.usage` (object), else nothing. The call's tokens are
  `usage_of(usage_tokens(usage))` with output raised to the largest output this message id has shown for this task.
  `totals[task]` moves by (call - held) per field, so a message seen three times counts once at its largest output.
  Map orders: first-seen.
- `note_progress`: only `type: "system"` with subtype task_progress or task_notification, a string task_id, an object
  `usage`, and a task the caller says is a subagent. `said[task] = {toolUses: usage.tool_uses?, durationMs:
  usage.duration_ms?}`, replacing the last. Answers the task id. `total_tokens` is never read.
- `subagent_spent`: `None` where neither totals nor said has the task; else `{tokens?, toolUses?, durationMs?}`.
- `subagent_spent_at_end`: `None` where subagent_spent is; else its value with durationMs taken from the end line's
  `usage.duration_ms` only, dropped where the end line has none (a kill).

## Rules against tests

| Rule | Test |
|---|---|
| parse_line rejects non-object, non-string type, leading junk | `claude_reading::parse_line_reads_only_objects_with_a_string_type` |
| str_arr keeps strings, empty array is Some | `claude_reading::str_arr_keeps_the_strings_and_an_empty_array` |
| harness_of None only when all three absent | `claude_reading::harness_of_reads_each_field_alone` |
| usage_tokens sets cached and cacheWrite only where named | `claude_reading::usage_tokens_names_only_the_counts_the_usage_names` |
| plus_tokens(None) keeps window; sum drops it | `claude_reading::plus_tokens_drops_context_and_window_on_a_sum` |
| limit_of order, first kind wins, rounding | `claude_reading::limit_of_takes_unified_windows_first_and_one_window_per_kind` |
| limit_of None on empty and on no utilization | `claude_reading::limit_of_reads_nothing_from_no_window` |
| note_rejected sets, keeps place, clears on overage | `claude_reading::note_rejected_moves_the_stop_by_each_reading` |
| with_limit on failed only, `{}` where a reset is unknown | `claude_reading::with_limit_stamps_only_a_failed_turn` |
| result_facts exit code rule | `claude_lines::carries_a_failing_command_s_exit_code...` (ported) |
| result_facts two blocks give nothing | `claude_lines::reads_no_tool_use_result_for_a_line_that_answers_two_calls` (ported) |
| result_facts create as whole-file hunk | `claude_lines::carries_a_new_file...` (ported) |
| shell_cwd regex and Write rule | `claude_lines::shell_cwd_*` (ported, 5 cases) |
| TaskUpdate deleted, unknown id Nothing | `claude_reading::task_update_moves_deletes_and_ignores_unknown_ids` |
| TodoWrite empty list is Steps([]) | `claude_reading::todo_write_reads_its_list_whole` |
| spend counts a message once at its largest output | `claude_reading::child_calls_count_once_at_the_largest_output` |
| spend at end drops progress duration | `claude_reading::spend_at_end_takes_the_run_time_from_the_end_line_alone` |
