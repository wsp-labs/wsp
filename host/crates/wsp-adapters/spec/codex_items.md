# codex_items
Source: packages/adapter-codex/src/adapter.ts:251-511 (the pure helpers above `createCodexAdapter`) at a3b870a6e.
Tests: no TS file of its own; every case reaches it through adapter.test.ts. Rust: `src/codex/items.rs`; tests `tests/codex_items.rs`,
written for this port from the expected values of the adapter.test.ts cases named on each line below, plus a direct case per rule.

## What it does
Turns one app-server item into the deltas a timeline draws (a call when it starts, its result when it completes), reads a file change's
hunks, sums a turn's tokens from the server's running totals, words a failure the CLI reported, and holds the constants the turn reader
keys on.

## Behaviour
1. `failure_words(message, login, key_env)` (:288): a message holding `401 Unauthorized` is `{line, cause: sign-in}`, the line
   `codexNotSignedInLine(login)` with no key env, else `codexKeyRefusedLine(key_env, refused_because(message), login)`. A message matching
   ``Missing environment variable: `([^`]+)` `` is `{line: codexMissingEnvLine(name)}` with no cause. Else `None`.
2. `refused_because` (:277): the text after the first `401 Unauthorized`, to the first `\n`, its leading run of `:` and JS whitespace taken
   off, cut at the first `, url:` and then at the first `)`, then `js_trim`med. No match is `""`. ("unexpected status 401 Unauthorized:
   token expired, url: https://.., cf-ray: x" gives `token expired`.)
3. `retry_of(message, info, session)` (:267): only a message starting `Reconnecting...` followed by ` <n>/<m>` gives a `turn.retry`
   `{attempt: n, of: m, why}`; why is `the provider answered <status>` where `info.responseStreamDisconnected.httpStatusCode` is a number,
   else `the connection dropped`. `Reconnecting...` alone gives none.
4. `item_of(params)` (:297): `params.item` an object with string `id` and string `type` (`""` counts).
5. `changes_of` (:305): each object of an array as `{path: string or "", kind: kind.type (`""` counts, CI2), else kind as a string, else "change"}`; not an
   array is `[]`. `change_lines` is each `"<kind> <path>"` trimmed, joined by `\n`.
6. `file_patches` (:319): each change object with a non-empty string `path`; the kind has no default here: anything but `add` or `delete`, absent included, takes the diff road (CI2). `add` and `delete` with a non-empty diff are one
   `wholeFileHunk(diff, "+" | "-")`; empty diff gives nothing. Any other kind: a diff ending `\n\nMoved to: <path>` (no newline in the path,
   at the very end) is cut there and carries `movedTo`; `unifiedHunks` of the rest; no hunks and no move gives nothing. No patch at all is
   absent, never `[]`. Read only off a completed item (rule 8).
7. `item_deltas(done, item, session)` (:397), by type; "failed" means `status` is not the string `completed`:
   - `agentMessage`: on completion one `text` delta, `text` (string or ""), `messageId` the item id.
   - `reasoning`: on completion, the string entries of `summary` if any, else of `content`, joined by `\n`; one `thinking` delta when non-empty.
   - `commandExecution`: start is `tool_use`, toolName `command_execution`, text `{"command":<shell_script_of(command or "")>}`;
     completion is `tool_result`, text `aggregatedOutput` or "", `isError` failed, `exitCode` and `durationMs` where finite numbers.
   - `fileChange`: start `tool_use` `file_change` with `{"changes":[{path,kind}..]}`; completion `tool_result` with `change_lines`,
     `isError` failed, and the patch only when not failed (a declined change carries none).
   - `mcpToolCall`: the server is `wsp` where it is `wsp_slate`, else its string, else `mcp`; start `tool_use` `mcp__<server>__<tool or "tool">`
     with text the JSON of `arguments` (`{}` where absent or null); completion: failed with a non-empty `error.message` gives that message,
     failed with no `result.content` key gives `""`, else the JSON of `result.content` (`[]` where null or absent); `isError` failed.
   - `collabAgentToolCall` with tool `spawnAgent` only: start `tool_use` `spawn_agent` with `{"prompt":<prompt or "">}`; completion
     `tool_result` with `""`. Other tools give nothing.
   - `webSearch`: nothing on start; on completion a `tool_use` `web_search` `{"query":..}` then a `tool_result` with the query, not an error.
   - anything else: nothing. Every delta's toolUseId is the item id where the TS sets one; every delta carries `session`.
8. `tool_call_of(item)` (:387): needs string `id` and `type`; the first `tool_use` delta with a tool name of the start deltas, then the
   completion deltas, as `{tool, text}`.
9. Tokens (:347-382): the breakdown pairs, in order, `input/inputTokens`, `cached/cachedInputTokens`, `cacheWrite/cacheWriteInputTokens`,
   `output/outputTokens`, `reasoning/reasoningOutputTokens`. `held_of(last)` is a finite `totalTokens`, else input plus output (absent
   reads 0). `total_before(total, last)` is, for each key the total names, total minus last (absent last reads 0). `turn_tokens_of(seen)`:
   each field is total minus before where both name it, else last's; `input` and `output` default 0; `context` is `held_of(last)`;
   `window` where seen.
10. `child_usage_of(total)` (:470): `None` when no breakdown key is a finite number, else `{tokens: {input, cached, cacheWrite, output,
    reasoning}}` each defaulting 0. `with_child_tokens(lead, children)` (:476): no child tokens gives `lead` as is; else lead (or
    `{input:0, output:0, context:0}`) with each child field added; `context` and `window` stay the lead's.
11. `child_end(status)`: `completed` done, `interrupted` stopped, else failed. `depth_of(path)`: `max(1, non-empty segments - 1)`.
    `step_state`: `completed` done, `inProgress` working, else pending.
12. `turn_line_for` (:495): a prompt that `js_trim`s to exactly `/compact` is `thread_compact_start_line`; anything else `turn_start_line`
    with images and effort. `is_turn_start(line)`: the line parses and its `method` is `turn/start` or `thread/compact/start`.
13. `rollout_in(home, id)` is `{folder: "<home>/sessions", name: "-<id>.jsonl", depth: 3}`. `ASIDE_INSTRUCTIONS` and `APPROVALS`
    (`item/commandExecution/requestApproval` → `command_execution`, `item/fileChange/requestApproval` → `file_change`) verbatim (:484-491).
    `STDERR_TAIL_LINES = 5`, `RECONNECT_STALL_MS = 90_000`, `COMPACT = "/compact"`.

## Edge cases
- `/compact the notes please` is a turn, not a compaction. `"  /compact\n"` is a compaction.
- A `fileChange` whose diffs are all empty strings carries no `patch` key.
- `Reconnecting... 2/5` with `httpStatusCode: null` says `the connection dropped`.

## Behaviour nobody wrote down
- Rules 8.6 and 8.7: the regexes are ported by hand (`\s` is JS's set); `$` in the move pattern is end of text, no multiline.
- Rule 4.25: a number written into a sentence (`the provider answered 401`) or re-stringified JSON uses the JS formatter.

## Decisions
1. `JSON.stringify(item.arguments)` re-writes a parsed value: integer-like keys first ascending, then the rest as inserted, and every
   number by JS rules (`1.0` is `1`, `1e21` is `1e+21`). The port writes it through `wsp_wire`'s whole-value JS writer (the coordinator's
   ruling on #2447, item 4), not serde_json (CI8).

## After the port
- Nothing.

## Rules added for the port (each value printed by the TS at 65f6b89c7; the private helpers run from a copy of adapter.ts)

- **CI1.** A refused key's reason is cut with `trim().slice(0, 80).trim()` in UTF-16 units and dropped with its `: ` where empty; an
  empty `key_env` is a key env (`OpenAI refused your : bad. ...`); `401 Unauthorized` beats a missing variable in one message; a
  missing variable named `` ` ` `` (empty) is no match.
- **CI2.** `file_patches`: a change with no kind and a diff `@@ -1 +1 @@\n-a\n+b\n` is one hunk `{1,1,1,1,["-a","+b"]}`; an add of
  `"\n"` is one hunk `{0,0,1,1,["+"]}`; an update of `"\n\nMoved to: b.ts"` is `{path, hunks: [], movedTo: "b.ts"}`. `changes_of` keeps
  `kind: {type: ""}` as `""`, drops `null`, and reads `{}` as `{path: "", kind: "change"}`; `change_lines` of `{p, ""}` is `p`.
- **CI3.** Reasoning reads `summary`'s string entries whenever there is one, even `""`: `summary: [""]` with `content: ["c"]` says
  nothing; `["", ""]` says thinking `"\n"`; `[5]` falls to `content`.
- **CI4.** Only the status string `completed` is success: a `commandExecution` with no status ends `isError: true` with text `""`; a
  failed `mcpToolCall` whose `result.content` is `null` says `[]`.
- **CI5.** `depth_of`: `""` 1, `"/"` 1, `"x/y/z/w"` 3, `"//root//a//"` 1.
- **CI6.** `/compact` with images and effort is the compaction line alone; JS trim takes U+2003 and U+FEFF, not U+200B; `effort: ""` is written.
- **CI7.** `is_turn_start`: `null`, `5`, `[]`, `"turn/start"`, `{"method":5}`, `""` are false; a padded turn start and a compaction are true.
- **CI8.** A call's `arguments` `{b:1, "2":"x", a:2, "1":"y", f:1.0, g:1e21}` draw `{"1":"y","2":"x","b":1,"a":2,"f":1,"g":1e+21}`.
- **CI9.** `with_child_tokens(None, {bogus: 5})` is `{input 0, output 0, context 0}` (a name outside the breakdown never lands);
  lead `{6, 2, context 3, window 9}` with children `{reasoning 2, input 1}` is `{input 7, output 2, reasoning 2, context 3, window 9}`.
  The TS key order (lead's keys, then new ones) is wsp-wire's struct order here, the same accepted difference as the TurnResult's.
- After-port: a `reasoning` item with `summary: [""]` drops a non-empty `content`; `file_patches` and `changes_of` read the kind
  differently; `failure_words` is fed `message + " " + additionalDetails`, so the reason runs on into the details.
- **CI10.** `retry_of` matches the prefix `Reconnecting... <n>/<m>` only (`(extra)` after it still counts; no space, a leading space or
  no count give none); `02/005` is attempt 2 of 5 and `5/3` attempt 5 of 3; the why names the provider for any number under
  `responseStreamDisconnected.httpStatusCode` (`1.5` gives `the provider answered 1.5`), a string or nothing reads `the connection
  dropped`. `is_reconnecting` is true for `Reconnecting...` alone.

## Tests (tests/codex_items.rs, 30, written for this port; red on todo!())
- Rule 1: `a_401_with_a_key_handed_is_the_refused_key_line_with_the_providers_reason_and_no`,
  `a_401_with_no_key_handed_is_the_not_signed_in_line_claimed_as_sign_in`, `a_missing_env_key_names_the_variable_and_claims_no_cause`,
  `any_other_message_has_no_words_of_wsps`. Rule 2: `refused_because_takes_the_clause_after_the_status_to_the_url_or_bracket`.
- Rule 3: `a_reconnecting_error_with_a_count_is_a_retry_naming_the_status_where_codex_kept`, `reconnecting_with_no_count_or_any_other_message_is_no_retry`.
- Rules 4, 5: `an_item_needs_a_string_id_and_type_the_empty_string_counting`, `changes_read_the_kinds_own_word_and_default_to_change`.
- Rule 6: `each_file_change_carries_its_hunks_and_a_rename_its_moved_to`, `changes_whose_diffs_are_empty_carry_no_patch_at_all`.
- Rule 7: `a_command_starts_as_its_script_and_completes_with_its_output_exit_code_and_durat`,
  `a_declined_file_change_is_an_error_with_no_patch_and_a_completed_one_carries_it`, `an_mcp_call_the_tool_failed_keeps_its_words_one_that_never_reached_it_the_error`,
  `an_mcp_call_to_the_slate_server_is_named_as_wsps_and_its_arguments_ride_as_json`, `reasoning_reads_its_summary_else_its_content_and_says_nothing_when_both_are_empt`,
  `an_agent_message_is_text_on_completion_under_its_own_id_and_nothing_on_start`, `a_web_search_draws_its_call_and_its_result_on_completion_alone`,
  `a_spawn_draws_spawn_agent_with_its_prompt_and_other_collab_tools_draw_nothing`.
- Rule 8: `a_tool_call_as_earlier_messages_show_it_is_the_items_tool_use`.
- Rule 9: `a_resumed_threads_turn_counts_from_where_its_running_total_stood`, `a_field_the_total_names_not_falls_back_to_the_latest_calls`,
  `what_the_model_held_is_the_total_tokens_else_input_and_output`.
- Rule 10: `a_childs_usage_is_its_whole_breakdown_each_missing_field_zero_and_none_without_a`, `the_child_tokens_add_to_the_leads_and_leave_its_context_and_window`.
- Rule 11: `a_childs_end_depth_and_a_plan_steps_state_read_in_the_shared_words`, `the_plan_reads_each_step_and_its_state_skipping_what_is_not_an_object`.
- Rules 12, 13: `compact_alone_is_the_servers_compaction_and_with_words_after_it_a_turn`, `a_line_already_starting_a_turn_is_told_apart_from_any_other`,
  `a_threads_rollout_is_found_three_folders_under_sessions_by_its_name_end`.
- CI1 to CI9: `rules_added::*` in `codex_items.rs` (9 tests).
- CI10: `rules_added_retry::a_retry_is_a_prefix_match_with_a_count_and_only_a_numeric_status_names_the_provider`.
