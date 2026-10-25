# Unit claude-normalize: what one line says, what a result costs

TS at a3b870a6e: `packages/adapter-claude/src/adapter.ts:213-625` (413 lines, module-private in the TS). Rust:
`src/claude/normalize.rs`. Tests: `tests/claude_normalize.rs` (hard cases, each checked by running a copy of these TS
functions), and every turn test in `tests/claude_turn*.rs` reaches them. Pure, except `normalize_event`, which moves
the plan and hand-back books it is handed.

## Result status (adapter.ts:221)

In this order: `terminal_reason` aborted_tools or aborted_streaming → interrupted (even on a success with is_error);
subtype success → failed where is_error is `true`, else completed; errors text (joined by space, lower case) contains
"interrupt" or "cancel" → interrupted; subtype error_during_execution with is_error exactly `false` and the text
containing "aborted" → interrupted; else failed.

## Tokens and running totals

- `models_of(raw)`: each entry of a modelUsage object in key order, non-object values skipped; cached =
  cacheReadInputTokens, cacheWrite = cacheCreationInputTokens (0 where absent); input = inputTokens + cached +
  cacheWrite; output = outputTokens; reasoning = thinkingTokens; costUsd = costUSD where numeric. `None` raw → `[]`.
- `start_of(models, model)`: an exact name anywhere first, then the first equal base_model (the last trailing `[...]`
  dropped); `""` finds nothing.
- `with_calls(models, calls)`: copies; each call joins its start_of entry (usage_of(plus_tokens), tokens only: the
  call's cost is dropped) or is appended with its cost; a later call may join an entry appended earlier.
- `result_tokens(result)`: tokens = usage_tokens(usage) plus `window` = the largest numeric contextWindow of
  modelUsage, where usage is an object; models = models_of(modelUsage) where the key exists at all.
- `saved_use_of(line, session)`: only where `sessionId` equals session, `totalCostUSD` is numeric and modelUsage an
  object: `{costUsd, models}`.
- `gained(now, before)`: before None → now. Any field of now under before's → `None`. Else each field less before's;
  cost `max(0, now - (before.cost or 0))` where now has a cost, else no cost.
- `own_use(result, saved, fresh, spent)` (adapter.ts:325), f64 arithmetic in exactly this order:
  1. Take total = result.costUsd and running = result.models out; window = result.tokens.window.
  2. rest = the rest, with tokens replaced by `spent` (+ window where any) where spent is Some.
  3. Not fresh and saved None → answer rest (no cost, no models, no model).
  4. before = saved or `{0, []}`. restarted = total < before.cost. paid = total (restarted) or total - before.cost.
  5. own = each running model: itself where restarted, else gained(m, start_of(before.models, m)). Any None → counted
     None; else counted = those with input + output > 0.
  6. unpriced = [] where restarted, else base_model of each before.unpriced. models = counted with each unpriced
     model's cost removed.
  7. costUsd = paid where unpriced is empty or paid is None; else None where no model of `models` kept a cost; else
     `max(0, paid - sum)` where sum adds, in model order from 0, the cost of each counted model that is unpriced.
  8. model = the model with the largest input + output (first wins a tie). tokens = the sum of models' tokens
     (plus_tokens folded from None) plus window, where models is non-empty; else rest.tokens.
  9. Answer rest with tokens, costUsd, models, model set where Some (rest's own value stays where None).
  Checked: a turn of 0.5 total over 0.3 saved costs 0.2 with opus at 0.15000000000000002; with b unpriced the cost is
  0.30000000000000004 (tests pin these exact f64 values).

## Results

- `normalize_result(line, refusal)`: errors = str_arr(errors) or []; `{status, durationMs: duration_ms,
  costUsd: total_cost_usd, tokens, models, text: result, error: first error not starting "[ede_diagnostic]"}`. Where
  is_error is true and status is failed: `refused_turn(result, refusal)`.
- `refusal_cause`: authentication_failed → sign-in; nothing else.
- `api_error_cause(line)`: NotOne unless an assistant line with `is_api_error_message: true`; then Cause or Unclaimed.
- `answered_nothing`: completed, text trims empty by JS trim (rule N1), tokens input + output == 0 (absent tokens
  count 0; cached alone still answers nothing).
- `with_context(result, context, init_model)`: model = result.model or init_model; tokens.context = context where
  both exist.
- `compacted_to`: system/compact_boundary with numeric `compact_metadata.post_tokens`; before where pre_tokens.
- `retry_of`: system/api_retry with numeric attempt; `{attempt, of: max_retries?, why: anthropic_retry_why(error,
  error_status)}`. `subagent_retry_of`: any line with `subagent_retry.attempt` numeric, of = max_retries, why off
  error_category and error_status.
- `no_output_error(result, tail)`: `claude answered with no output and no usage after <fmt_duration(durationMs or
  0)>` then `: ` and the tail joined by `\n` where the tail is non-empty. The tail keeps the last 5 non-JSON lines.
- `subagent_launch`: system/task_started with string task_id and tool_use_id. `subagent_started`: a task_started with
  a string task_id and task_type local_agent, tool_use_id not required; parent = tool_use_id, title = description, depth = spawn_depth, each where present.
- `subagent_prompts`: the lead's own assistant lines only (parent_tool_use_id not a string: absent, null, a number;
  `""` is a subagent's); each tool_use of Agent
  or Task with a string id and a string `input.prompt`, in block order.
- `background_tasks_of`: system/background_tasks_changed; a non-array tasks is `Some([])`; entries need a string
  task_id; description defaults to the id.
- `task_finished_of`: system/task_notification with string task_id and status.
- `drained_notice`: a result whose `origin.kind` is task-notification, num_turns exactly 0, and normalize_result of it
  answered nothing.

## normalize_event (adapter.ts:502)

sessionId = the line's session_id or the fallback. parentToolUseId = the line's string parent_tool_use_id.
- system: retry_of first; else only init: `session.start{sessionId, model?, cwd?, tools?, harness?}`, then
  `limit{windows: [], keyed: true}` where apiKeySource is a string other than "none".
- assistant: a non-array content → `[]`. Per object block: text → text delta (text or "", messageId where the
  message has an id, parent); thinking → thinking delta (no messageId); tool_use → on the lead's own line a plan call
  (read_plan_call) adds the id to plans.calls and emits `turn.plan` unless Nothing; otherwise a tool_use delta with
  `text: JSON.stringify(input ?? null)`, toolName, toolUseId, parent, dropped where held_call holds it. Others skipped.
- user: per tool_result block: a plan call's result (id in plans.calls, removed) names the TaskCreate's task id off
  the first `#(\d+)` in its flattened content and draws nothing; any other draws read_answer of `{kind:
  tool_result, text: flatten_content(content), toolUseId, isError: is_error === true, parent, ...result_facts}`.
- result: `turn.done` with normalize_result. rate_limit_event: `limit` where limit_of reads one. tool_progress:
  subagent_retry_of. Any other type: `[]`.
- Delta key order (when serialized) is the struct's: sessionId, kind, text, messageId, toolName, toolUseId, isError,
  parentToolUseId, cwd, bytes, exitCode, durationMs, patch. It matches the TS construction for every delta built here.
- `session_file_in(config, id)`: `{folder: <config>/projects, name: <id>.jsonl, depth: 1}`.

## Rules added for the port (numbered; each value printed by the TS at a3b870a6e)

- **N1.** `answered_nothing` on text U+0085, U+FEFF, U+00A0, U+2028, U+3000, U+000B, U+200B, U+180E, U+001C: false,
  true, true, true, true, true, false, false, false (JS trim, not `str::trim`).
- **N2.** `result_tokens`: `modelUsage: null` gives `models: Some([])`; `usage: {}` gives `{input: 0, output: 0}` with
  no cached or cacheWrite; `usage: [1]` gives nothing. Never a `reasoning` key.
- **N3.** `with_calls([m[1m] 10/1 $0.5], [m 2/3 $0.7, n 1/1 $0.2, n[1m] 4/4 $0.3])` is `[m[1m] 12/4 $0.5, n 5/5 $0.2]`;
  `start_of([m[1m], m], "m")` is the second, `start_of([m[1m], m[200k]], "m")` the first.
- **N4.** A resume with saved totals and no running models costs the gain (0.2 over 0.1 is 0.1) and names no models.
- **N5.** A bare tool_use block draws `{kind: tool_use, text: "null"}` with no name or id.
- **N6.** A tool result with no `is_error` says `isError: false`.
- **N7.** `api_retry` with `attempt: 0` alone is `{attempt: 0, why: "Anthropic did not answer"}`.
- **N8.** `no_output_error` for 1500, 999.6, 59999, 61000, -5 and no duration: `1.5s`, `1000ms`, `60s`, `1m 1s`, `0ms`,
  `1ms`; the tail's blank entries stay (`a\n\nb`).

## Rules against tests

| Rule | Test |
|---|---|
| status order: abort first, is_error over success, words, aborted | `claude_normalize::result_status_reads_the_abort_first` |
| own use on a resume, split by model, exact f64 | `claude_normalize::own_use_is_the_gain_over_the_saved_totals` |
| restarted total is all the turn's | `claude_normalize::own_use_takes_a_total_under_the_saved_one_whole` |
| no saved line on a resume: no cost, spent tokens stand | `claude_normalize::own_use_leaves_cost_out_with_nothing_saved` |
| a model under its saved totals drops the split | `claude_normalize::own_use_drops_the_split_where_a_model_fell` |
| unpriced models lose their cost and the turn's cost | `claude_normalize::own_use_withholds_unpriced_models` |
| zero-spend models dropped on a fresh session | `claude_normalize::own_use_drops_models_that_spent_nothing` |
| gained floors cost at 0 | `claude_normalize::gained_floors_the_cost_at_zero` |
| refusal sentence and cause | `claude_normalize::a_refused_result_carries_the_sentence_and_the_road` |
| errors, ede noise, windows, models of odd entries | `claude_normalize::a_failed_result_reads_its_models_and_largest_window` |
| init and key source | `claude_normalize::an_init_says_the_session_and_a_key_has_no_plan` |
| assistant blocks, null input, plan calls | `claude_normalize::assistant_blocks_become_deltas_and_plans` |
| TaskCreate named off its result, no row | `claude_normalize::a_task_create_result_names_the_task_and_draws_nothing` |
| retries, background tasks, drained notices | `claude_normalize::retries_tasks_and_drained_notices` |
| N1 | `claude_normalize::answered_nothing_trims_as_javascript_does` |
| N2 | `claude_normalize::result_tokens_read_null_model_usage_as_no_model` |
| N3 | `claude_normalize::a_joined_call_adds_tokens_only_and_an_exact_name_beats_its_base` |
| N4 | `claude_normalize::own_use_with_no_running_models_costs_the_gain_alone` |
| N5 | `claude_normalize::a_bare_tool_use_block_draws_null` |
| N6 | `claude_normalize::a_tool_result_always_says_whether_it_failed` |
| N7 | `claude_normalize::a_first_retry_with_no_words_says_anthropic_did_not_answer` |
| N8 | `claude_normalize::the_empty_answer_line_times_any_duration` |
