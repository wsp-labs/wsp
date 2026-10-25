# tools
Source: `packages/protocol/src/words/tools.ts` (719 lines) at the cut, 383708c78 (733 lines at a3b870a6e). Tests: `packages/protocol/test/format.test.ts` (30 cases ported), plus `pinned` in `tests/tools.rs` (every value printed by the TS under node).

## What it does
One table of the tools a harness reports, keyed by tool name (`TOOL_ROWS`, tools.ts:269-287): the line a call reads as before and after it runs, the field a client shows, its item and request kinds, the paths it changed, whether it searched code. The question tool's questions, picks and the answer input written back. One table of how each kind of call is put to a person when it asks permission (`PERMISSION_ASKS`, tools.ts:545-556).

## Behaviour
1. A call's input is parsed with `serde_json` into an object; anything that does not parse or is not an object (array, string, null) is "no input" (tools.ts:293-301). `toolField(input, name)`: the value when it is a string and not blank by `js_trim`; the value itself, untrimmed (tools.ts:41-44).
2. `server_tool(name)`: split on "__"; at least three parts, the first "mcp", the second non-empty; tool = parts from the third joined by "__" (may be "") (tools.ts:47-50). `server_tool_words`: `Use <server>'s <tool with _ as space>` (tools.ts:53-56).
3. `tool_activity_line(name, input)`: name defaults to "tool". No row: server words, else the name. Row and no input: the name. Else the row's asked line, else the name (tools.ts:307-313). Rows: shell `$ <title_line(command)>`; path rows `<doing|did> <path>`; change rows `<editing|edited> <path>` for one path, `<verb> N files` for more (paths are `changes[].path` strings that are non-empty); about rows `<doing|did ?? doing> <title_line(field)>`; the question row is the first question's text (tools.ts:66-118, 243-252).
4. `tool_done_line`: None without a row or input, or when the done line equals the asked line (tools.ts:318-324).
5. `tool_call_facts(name, input)`: keys in this order, each only when set: itemType (the row's, else `mcp_tool_call` for a server tool), requestKind, title (the row's, else the server words), detail, command, description, changedFiles (non-empty only). Unparsed input: `detail` is the raw input when non-empty, nothing else past title. detail = first non-blank field of the row's `shows`, then `file_path, notebook_path, pattern, query, url, description, prompt`, then `title, message` for a server tool. command and description only for a command row (tools.ts:340-365).
6. `tool_result_line(text, is_error)`: None for a text holding "This tool result is internal metadata"; first = `title_line(text)`; not an error: None when "", else first; an error: "failed" or `failed: <first>` (tools.ts:386-391). `tool_answered_line`: a file-change call that is not an error says its done line where it has one; else the result line (tools.ts:398-404).
7. Questions: `questions` must be an array; each entry an object with a non-blank `question`, else skipped; `index` is its position in the array, skipped entries counted; options are entries that are objects with a non-blank `label`, ids `q<question>:o<n>` with n counting kept options; description "" when absent; header "" when absent; multiSelect only for `true`; None when no question is kept (tools.ts:152-184).
8. `other_option_id(q, words)`: `q<q>:other:<encodeURIComponent(words)>` (tools.ts:196). A typed answer is read back by `^q(\d+):other:(.*)$` and `decodeURIComponent`, None where decoding fails (tools.ts:199-207).
9. `picked_options(options, picked)`: split on "|"; each id is an option on the prompt, or, where every option's effect is answer, a typed answer becoming `{id, label: words, effect: answer}`; None when any id names nothing (tools.ts:212-219).
10. `question_answer_input`: `{...fields, answers}` where answers maps each question's key to its picked labels then its typed words (multi-select: the list; else the first); questions with none are left out; None when no question was answered. A spread keeps an existing `answers` key in its first position (tools.ts:227-239).
11. `permission_ask_line` and `permission_prompt_words` lead: the kind's lead, else `Permission for <tool>` with `: <detail>` for a non-empty detail. Kinds: Write `Write <file>[ in <folder>][ (<fmt_bytes(utf8 bytes of content)>)]`; Edit/MultiEdit `Edit <file in folder>[ (N places)]` for N > 1 edits; Bash and command_execution `Run a command:` with the command as code; file_change `Change <file in folder>` or `Change N files: <names>`; WebFetch `Fetch a page:` with the url as code; Skill `Run the skill <skill>`; a server tool its server words; anything else no lead (tools.ts:452-560).
12. `permission_prompt_words`: lead joined with code by a space; a question call answers `{lead, says, code?, rest: "", questions}`; unparsed input `{lead, says, code?, rest: input}`; else rest = every field not named by the kind, `key: value` joined by "\n", value a string as is, an array of strings and numbers joined by ", ", anything else JSON-written, then whitespace collapsed and trimmed; body `{label: "show the file", text}` for Write's content; note (command_execution: a non-empty detail; WebFetch: the prompt field); file = `file_path`; edit = the first of `edits` (else the call's own) `old_string`/`new_string`, both untrimmed strings (tools.ts:612-647).
13. `permission_outcome_line`: allowed: `You answered: <label>` for an answer pick, else `Allowed` plus `: <label>` for a mode pick; denied `Denied` plus the same; unanswered `Nobody answered; denied`; cancelled `Cancelled with the turn` (tools.ts:659-675). `denied_line`: the base line for a reason None or blank, else `, and said what to do instead: <trimmed>` (tools.ts:691-692).

## Edge cases
- `server_tool("mcp____x")` None (empty server); `"mcp__srv__"` has tool "" (`a_server_tool_is_mcp_then_a_server_then_the_rest`).
- `file_change` with no usable path reads its own name and has no done line; a null and an empty path are skipped in the count (`every_row_reads_its_call_at_both_moments`).
- `tool_call_facts("Bash", "")` has no detail (`tool_call_facts_keep_the_ts_key_order`).
- A malformed percent escape in a typed answer makes the whole pick None; a consent prompt takes no typed answer (`question_picks_encode_join_and_answer_as_the_ts_does`).
- Write's size counts UTF-8 bytes: "héllo" is 6 B (`every_ask_kind_leads_its_prompt_its_own_way`).

## Behaviour nobody wrote down
- Rule 4.26: rest keeps the call's own key order; `question_answer_input` keeps the call's keys in order.
- Rule 4.28: a value JSON-written in rest is `JSON.stringify`'s text (no spaces).
- Rule 8.6: blank and collapse are JavaScript's whitespace.

## Tapes
`session.permission` frames and every `session.delta` a client draws; no op of the host's alone.

## Decisions
1. `ToolInput` is a `serde_json::Map` with `preserve_order`; `JSON.parse` accepts what serde_json accepts on these inputs (objects from harnesses).
2. `tool_answered_line` takes the result's text and isError as two arguments.
3. `answer_words(Answered)` is None: the TS table leaves that outcome out.

## After the port
- `permission_ask_line("Weird", "{}", "")` and an unparsed input both fall back to the harness phrase, even where the input had fields a kind could have used.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `a_server_tool_is_mcp_then_a_server_then_the_rest` |
| 3, 4 | `a_turn_s_activity_in_one_line_each::*`, `every_row_reads_its_call_at_both_moments` |
| 5 | `the_one_registry_every_client_reads_a_tool_call_from::*`, `tool_call_facts_keep_the_ts_key_order` |
| 6 | `what_a_tool_call_answered_in_one_line::*`, `results_answers_and_subagent_lines` |
| 7 to 10 | `the_words_a_relayed_permission_prompt_shows::*`, `question_picks_encode_join_and_answer_as_the_ts_does` |
| 11 | `every_ask_kind_leads_its_prompt_its_own_way` |
| 12 | `prompt_words_carry_the_rest_body_note_and_edit` |
| 13 | `outcome_denied_and_access_lines` |
