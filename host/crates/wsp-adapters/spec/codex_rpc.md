# codex_rpc
Source: packages/adapter-codex/src/rpc.ts (161 lines), fields.ts (12), shell-script.ts (47) at a3b870a6e.
Tests: packages/adapter-codex/test/rpc.test.ts (14 cases), shell-script.test.ts (2 cases). Rust: `src/codex/rpc.rs`, `src/codex/fields.rs`, `src/codex/shell_script.rs`; tests `tests/codex_rpc.rs`.

## What it does
The JSON-RPC lines wsp writes to `codex app-server` on stdin, one object per line, and the reader that sorts each stdout line into
one of four message kinds. Codex prints no `jsonrpc` field and takes none. Also the three loose readers of a JSON value
(`rec`, `str`, `count`) the turn reader uses, and `shell_script_of`, which takes the script back out of the `<shell> -lc <script>`
argv codex reports for each command.

## Behaviour
1. Request ids are fixed strings (rpc.ts:9): `wsp-initialize`, `wsp-thread`, `wsp-turn`, `wsp-revert`, `wsp-turns`, `wsp-account`,
   `wsp-rate-limits`, `wsp-unsubscribe`. Steers and interrupts are numbered by the caller (`wsp-steer-<n>`, `wsp-interrupt-<n>`).
2. Every line is `JSON.stringify` of one object: no spaces, no newline, keys in the order the TS literal writes them (rule 4.26).
   `initialize_line` is `{"id":"wsp-initialize","method":"initialize","params":{"clientInfo":{"name":"wsp","title":"wsp","version":"1"}}}`
   (rpc.ts:23). `INITIALIZED_LINE` is `{"method":"initialized"}`. `ACCOUNT_READ_LINE` is `{"id":"wsp-account","method":"account/read","params":{}}`.
3. `rate_limits_read_line(false)` has params `{"excludeResetCreditDetails":true}`; `(true)` has params `{}` (rpc.ts:34).
4. `named` (rpc.ts:17) keeps only the fields that are set, in the order cwd, model, serviceTier. Thread params are laid out as
   `named(...)`, then `sandbox`, `approvalPolicy` (the access pair, in that order), then `config` only where it holds at least one key
   (rpc.ts:48). `thread/resume` puts `threadId`, `excludeTurns: true` first (rpc.ts:57).
5. `thread_fork_line` (side question): `threadId, ephemeral: true, excludeTurns: true, sandbox: "read-only", approvalPolicy: "never"`,
   then named cwd, model (no serviceTier), then `developerInstructions` last (rpc.ts:63).
6. `thread_copy_line`: `threadId`, then `lastTurnId` only where given, `ephemeral: false, excludeTurns: true`, named, access, config (rpc.ts:75).
7. `thread_turns_list_line` writes `cursor` always, `null` where none (rpc.ts:93), then `limit`, `sortDirection: "desc"`, `itemsView: "summary"`.
8. `turn_start_line`: input is the text first as `{"type":"text","text":...}`, then each image `{"type":"localImage","path":...}` in order,
   then `effort` only where given (rpc.ts:99). The text travels as written: quotes, newlines and `WSP_PROMPT_END` never meet a shell.
9. `turn_steer_line(id, ..)` names `expectedTurnId`; `turn_interrupt_line` names `turnId` (rpc.ts:111-118).
10. `decision_line(id, d)` is `{"id":<id as it came>,"result":{"decision":"accept"|"decline"}}`; an integer id stays a JSON number (rpc.ts:122).
11. `refuse_request_line(id, method)` is `{"id":..,"error":{"code":-32601,"message":"wsp does not answer <method>"}}` (rpc.ts:127).
12. `read_message` (rpc.ts:144): trims with JS `trim` (`wsp_words::js_trim`, rule 8.6); a line not starting with `{` after the trim, one that
    does not parse, or one that parses to anything but an object, is `None`. With a string `method`: a string-or-number `id` makes it a
    request, anything else a notification, carrying `emittedAtMs` only when it is a JSON number. `params` is the object or an empty map
    when absent or not an object. Without a method: an `id` that is not a string or number is `None`; an `error` object makes an error
    whose message is `error.message` when a string, else `codex refused the request without saying why`; else a present `result` key
    (even `null`) makes a response carrying that value; else `None`.
13. `rec` is an object (not an array, not null); `str` is any string, the empty string included (fields.ts:8); `count` is a finite number.
    Other files carry their own `str` that refuses `""`: never share this one with them.
14. `shell_script_of` (shell-script.ts:30) splits like POSIX sh: space, tab and newline separate words; `'...'` keeps every byte; inside
    `"..."` a backslash is dropped only before `"`, `\`, `$`, a backtick or a newline; a bare backslash takes the next byte; an unclosed
    quote or a trailing backslash refuses the line. Exactly three words, the first's last path segment one of `sh`, `bash`, `zsh`, `dash`,
    and the second `-lc` or `-c`, give the third word; anything else gives the command unchanged.

## Edge cases
- `read_message("")`, `"{not json"`, `'{"neither":true}'` and an ANSI-coloured stderr line are all `None` (rpc.test "reads anything else").
- `shell_script_of("/bin/zsh -lc 'a' extra")` and `"/bin/zsh -lc 'unclosed"` give the input back; `python3 -c ..` is not a shell.
- An empty quoted word (`''`) is a word of its own, `""` included.

## Behaviour nobody wrote down
- Rule 4.26: object key order is the TS literal's, so serde_json's `preserve_order` and `json!` in the written order, or `Serialize`
  structs with fields declared in that order.
- Rule 8.6: the trim in `read_message` is `js_trim`; a line wrapped in U+FEFF still reads as a message.

## Decisions
1. `RequestId` is `Str(String) | Num(serde_json::Number)`: the number is echoed as it was parsed, so `0` writes `0`. The ask id the turn
   derives from it (`String(id)`) is the number written by JS rules, `wsp_wire`'s JS formatter (rule 4.25).
2. A line holding a lone surrogate escape parses in JS and fails in serde_json, so it reads as `None` here. Accepted difference: codex never
   prints one.

## After the port
- Nothing.

## Tests (tests/codex_rpc.rs, 24; TS 14 + 2)
- Rules 1-3, 5-11: `the_lines_wsp_writes_to_codex_app_server::*` (10), `the_revert_line::*`; key order as text (rules 2, 4, 6, 7, 10):
  `key_order_as_written::*` (5, written for this port: a parsed value's equality ignores key order).
- Rule 12: `read_message_cases::*` (2) and `read_message_cases::a_response_whose_result_is_null_is_still_a_response_carrying_null`.
- Rule 13: `fields_readers::str_accepts_the_empty_string`, `fields_readers::count_refuses_a_non_number`.
- Rule 14: `shell_script_of_cases::*` (2).
