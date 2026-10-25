# Unit claude-copy: a session copied through one message, and the side question on it

TS at a3b870a6e: `packages/adapter-claude/src/{chain,aside}.ts` (335 lines) and the two adapter closures that drive
them, `forkSession` and `aside` (adapter.ts:1345-1417, specced in claude_adapter.md). Rust: `src/claude/{chain,aside}.rs`.
Tests: `tests/claude_copy.rs` (12 + 19 ported, several on a real sh and real files). The copy is a file on the machine;
a host restart leaves a written copy where it is (the adapter removes a side question's copy after its run, and a
fork's copy only through `drop`). A write cut by the wall leaves its `.<fork>.jsonl.part` beside the source:
`copy_session` removes nothing on that road, the cleanup's globs do not match a part file, and no fork id comes back
on a failure. Nothing sweeps it (an after-port line).

## chain.ts

- Every id that reaches a command is checked against the UUID pattern first:
  `Err("session identifier must be a UUID, got \"<id>\"")`.
- `session_file_line(config, session)`: `src=$(ls '<config>/projects'/*/<session>.jsonl 2>/dev/null | head -n 1); [ -n "$src" ] || { echo '<No conversation found with session ID: <session>>' >&2; exit 1; }`.
- `chain_read_command`: `<session_file_line>; n=$(wc -l < "$src"); echo $n; head -n "$n" "$src" | sed -E '<blank>' | grep -n -F '"uuid":"'`,
  blank = `s/([,:{[])"([^"\\]|\\.){200,}"/\1""/g`. A line with no newline yet is left out; strings of 200 or more
  characters print empty.
- `read_chain(read)`: line 0 must parse as a safe integer (`Number(...)`; `"  12 "` reads, `""` reads 0, `"x"` does
  not) else `Err(last line of read or "the session's file could not be read")`. Each later line is `<n>:<json>`;
  skipped where the JSON fails, is not an object, has no string uuid, or n is not a safe integer. parent is a string
  parentUuid or None; type the string type or `""`; boundary for `system` + subtype `compact_boundary`; messageId for
  an assistant line with a string message.id; calls (assistant) the tool_use blocks with a string id, name `String(name)`
  (a missing name is `"undefined"`), input or `{}`; answers (user) the tool_result blocks' string tool_use_ids.
- `chain_through(lines, anchor)`: `Err("the session holds no message <anchor>")` where absent. Walk parent links from
  the anchor, keeping each line number, stopping after a boundary line or a null parent;
  `Err("the session's chain to message <anchor> is broken at <uuid>")` where a link is missing or seen twice. Answers
  ascending.
- `live_end(lines)`: `None` for no lines. The newest assistant line's messageId; the calls of every line of that
  message not answered by any line. None open (or no assistant, or no messageId) → `{anchor: last line's uuid,
  running: []}`. Otherwise the first line of that message; its parent None → `None`; else `{anchor: that parent,
  running: each open call as "<name> <JSON.stringify(input) cut to 300 UTF-16 units>"}`. The input is written as
  `JSON.stringify` writes it (claude-control-lines: index keys first, JS numbers). A cut that splits a surrogate pair
  leaves half of it in the TS; here the half goes (rule K1). Calls come from every line of the newest message, not
  only adjacent ones, and an answer on any line counts. Duplicate uuids: the later line wins.
- `line_ranges`: consecutive runs as `a-b`, singles as `a`, joined by `,`; `[]` gives `""`.
- `chain_write_command(session, fork, config, count)`: checks the fork's id, then the count, then the session's id
  (rule K2); count must be a safe integer ≥ 1 (`Err("a copy keeps a whole number of lines, got <count>")`, the count
  written as a JS number); awk picks the ranges read from stdin into `${src%/*}/.<fork>.jsonl.part`, checks
  the line count, renames to `<fork>.jsonl`, else removes the part and exits 1 with `the copy of session <session>
  came out short` on stderr.
- `copy_cleanup_command`: `rm -rf '<config>/projects'/*/<fork>.jsonl '<config>/projects'/*/<fork>`.
- `copy_wall_line(ms)`: `the copy of the session had not finished after <fmt_duration(ms)> and was stopped`.
- `copy_session` (:179): run = launch on `exec` with `env` alone for the read (no input channel, never closed) and
  `{env, input: [ranges]}` for the write, whose input is closed once, at once; collect every line; a wall timer (`wall_ms`, default 90 000) ends the run with `end_run(grace_ms)` and the
  call fails with copy_wall_line. The read's code ≠ 0 → `Err(last line or no_conversation_line)`. Live end `None` →
  `Err("the session <session> holds no message to copy")`. Then chain_through, a fresh session id, the write; code ≠ 0
  → `Err(last line or "the copy of session <session> was not written")`. Answers `{session: fork, running}`. A wall
  hit is reported even where the run's code was 0.

## aside.ts

- `aside_command`: `build_command({resume: fork, aside: true, cwd, model, effort, contextWindow, fast (only true),
  mcpServers, serverValues, memoryDir, launch})`; fork must be a UUID first.
- `aside_hooks_line()`: `{"type":"control_request","request_id":"wsp-aside-hooks","request":{"subtype":"initialize","hooks":{"PreToolUse":[{"matcher":null,"hookCallbackIds":["wsp-aside-no-tools"]}]}}}`.
- `hook_deny_line(id)`: `{"type":"control_response","response":{"subtype":"success","request_id":<id>,"response":{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"No tool runs in a side question; answer from the conversation."}}}}`.
- `prompt_deny_line(id)`: `{"type":"control_response","response":{"subtype":"success","request_id":<id>,"response":{"behavior":"deny","message":"No tool runs in a side question; answer from the conversation."}}}`.
- `aside_prompt(question, running)`: the reminder lines of aside.ts:91-101 joined by `\n`, the running line only where
  running is non-empty (calls joined by `; `), then an empty line, then the question.
- `aside_text_of`: only `stream_event` whose event is `content_block_delta` with delta `text_delta` and non-empty
  string text.
- `aside_answer(result)`: `error_max_turns` → `Err(ASIDE_TOOL_LINE)` before anything else; is_error or subtype not
  success → `Err(trimmed result text, else the first non-[ede_diagnostic] error, else "claude ended the side question
  with <subtype or 'no result'>")`, where a first error of `""` is taken as none and the subtype line follows; the
  failure's text is trimmed (JS trim), a success's is kept whole; else `{text: result string or "", usage: the usage object where one}`.

## Rules added for the port (numbered; each value printed by the TS at a3b870a6e)

- **K1.** A Write call whose input is `{"s": "a" + 200 emoji}` runs as `Write {"s":"a` + 146 emoji: 7 units, 146 pairs,
  and the 147th pair's half dropped (the TS keeps the half; a forced difference).
- **K2.** `chain_write_command` refuses the fork's id first, then a count of 0, then the session's id.
- **K3.** `copy_wall_line(90000)` is `the copy of the session had not finished after 1m 30s and was stopped`.
- **K4.** `aside_answer`: `error_max_turns` with is_error false is ASIDE_TOOL_LINE; errors `["", "real"]` give `claude
  ended the side question with error_during_execution`; no subtype gives `... with no result`; a success keeps
  `"  hi \n"` whole; a failure's `"  boom \n"` is `boom`.

## Rules against tests

| Rule | Test |
|---|---|
| walk before and after a compaction, live branch, refusal of a missing anchor or link | `claude_copy` walk cases (ported, chain.test.ts:68-106) |
| live end before a call with no result, naming it | `claude_copy::walk_ends_a_live_copy_before_the_message_whose_call_has_no_result` (ported) |
| ranges text | `claude_copy::walk_writes_line_numbers_as_ranges` (ported) |
| byte-for-byte write under a dot name, source untouched | `claude_copy::shell_writes_exactly_the_anchors_chain_beside_the_source` (ported, real sh) |
| long strings blanked on the read | `claude_copy::shell_reads_a_long_session_as_kilobytes_and_walks_it_as_the_whole_file_would` (ported, real sh) |
| no copy and no part file after a short write | `claude_copy::shell_leaves_no_copy_and_no_part_file_where_the_write_comes_out_short` (ported, real sh) |
| cleanup removes only the copy | `claude_copy::shell_removes_only_the_copy` (ported, real sh) |
| aside launch line, hooks before the question, every call refused | `claude_copy` aside cases (ported, aside.test.ts) |
| the copy is removed on every road out, a wall included | `claude_copy` aside cases on the wall and the transport throw (ported) |
| K1 | `claude_copy::a_running_call_is_cut_at_300_utf16_units` |
| K2 | `claude_copy::the_write_refuses_in_the_ts_order` |
| K3 | `claude_copy::the_copy_wall_says_its_time` |
| K4 | `claude_copy::a_side_answer_reads_each_ending` |
