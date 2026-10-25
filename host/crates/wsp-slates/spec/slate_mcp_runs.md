# slate_mcp_runs
Source: packages/runtime/src/slate-mcp.ts:558-1045 (`createSlateMcp`) at 65f6b89c7, with the records of slate-runs.ts (slates-a's) it writes. TS tests: `test/slate-mcp.test.ts` (all 13 cases; 5 call `createSlateMcp` directly, 8 drive it through the whole runtime, whose assertions about this module are ported as direct cases with fakes, the rest left to the runtime join), `test/slate-box.test.ts:552-657` (servers kept and closed). Rust: `src/slate_mcp/runs.rs`; tests `tests/slate_mcp_runs.rs`.

## What it does
One live MCP connection per thread and server; runs of `tool` and `resource` kind; when to ask the person (once per piece of code, as the thread's agent would ask, never at its bypass; every start of a run that names `confirm`); two start budgets; scrubbing every secret out of what a server says; the idle sweep; `slate_catalog <server>`; the K713 refusal of a write.

## State
`threads: IndexMap<thread, ThreadMcp>` with `runs: IndexMap<run, Live>`, `conns: IndexMap<server, shared future of Conn>`, `named`, `kept` (sets), `shown: bool`, `pending` (the tracked futures). `lists: (thread, server) -> tools`, kept past a closed connection. `seen: (thread, server, tool) -> bool`, whether that tool ever answered with structured content. `Live = {record, gen, inflight?: (conn, id), reshaping?, result?, held?: {req, ask: code|budget|pressed, key?}, starts: [ms], pressed?: [ms]}`. `Conn = {client, spec, resources: bool, tools?: shared future, used_at, busy}`. Memory only: nothing here survives a restart (the person's Always lives in `approvals`, the record in the slate store, both slates-a's).

## Behaviour
1. `live(thread, run, last)` (`:605-614`) makes a run's entry once: record `{state: idle, runs: last.runs or 0}`, gen 0, no starts, and `result = last_result(last)` when that is some. An entry already there ignores `last`.
2. `clean(thread, spec, text)` (`:624-628`): `secrets.scrub(thread, text)`, then each of the spec's `secrets` values, longest first, of length at least 4, replaced everywhere by `***`.
3. `connect(thread, server)` (`:630-656`) shares one connection future per (thread, server): a second caller gets the first's. It asks `deps.server` (a failure fails the connect with its message unchanged), makes the stdio or http client, and when the client is open and has a pid calls `own(thread, pid, "tool")`, keeping the release. It sends `initialize` `{protocolVersion: "2025-06-18", capabilities: {}, clientInfo: {name: "wsp", version: "1"}}` with CONNECT_MS, then the notification `notifications/initialized`. `resources` is whether the answer's `capabilities.resources` is present. A failed initialize closes the client and fails `clean("<server> did not start: <why>[; it said: <last non-empty stderr line>]")`. When the client closes for any reason the release runs and the entry is removed if it is still this one; a failed connect removes its entry too, so the next start connects again.
4. `tools_of(thread, server, conn)` (`:658-673`), once per connection (shared): `tools/list` with `{}` then `{cursor}` while the answer's `nextCursor` is a string, at most 20 pages; keeps entries that are objects with a string `name`; stores them in `lists`. A failure forgets the shared future so the next call asks again.
5. `list_for` (`:676-682`): connect and list; a failure answers the held list or empty. Tracked in `pending`.
6. `write(thread, run, l, record)` (`:684-690`): sets the record; when it is not running and `last_result(record)` is some, that becomes `l.result`; calls `on_record` once.
7. `shown_args` (`:692-698`): every string inside the args with each mark replaced by `•••• (<length of the plaintext in UTF-16 units>)`, or `•••• (not filled)` when the secret has no plaintext.
8. `asks_now(thread, decl, info, args)` (`:704-711`): `true` for a tool with `confirm`; `false` at `never` or when `approvals.has(code_key(decl, args))`; `true` at `every`; `false` for a resource; else `None` when the tool is not listed yet, else whether its `readOnlyHint` is not exactly `true`.
9. `ask_of` (`:713-731`), the sheet, keys in this order: `key, run, kind: "tool", server, tool` (the uri for a resource), `resource: true` (resource only), `computer`, `why: HELD_APPROVAL`, `description` (the declaration's, else `sentence(listed description, 240)`, absent when neither), `args` (a tool's `shown_args(req.args())`, `{}` for a resource), `confirm` when a string, `then`, `files`.
10. `hold(req, l, ask, runs, beside)` (`:733-741`): stores `held` (with `code_key` for a code ask); unless `beside`, writes `{...l.result, state: "held", why, runs}` with why HELD_APPROVAL, HELD_PRESSED or HELD_BUDGET; for a code ask whose server has no list yet, lists it in the background and then calls `on_asks(thread)`. Answers `{outcome: held, record}` (the unchanged running record when beside).
11. `start(asked)` (`:898-919`):
    - reads `asked.args()` once; every later read sees that same value, so the person allows exactly what goes;
    - `beside` = the record is running. A tool with `once: true` started beside answers `{outcome: noop, record}`;
    - clears any `held`;
    - budgets: `by = person` spends `l.pressed`, every other `by` spends `l.starts` (`spent`, STARTS_PER_MINUTE per 60 s); past it: `hold(pressed or budget, runs = record.runs, beside)`;
    - when `asks_now(...) == true` (with the listed info): `hold(code)`, and `on_asks` when beside;
    - else `launch(req, l, false)`.
12. `launch(req, l, confirmed)` (`:782-851`): a running record is stopped first (rule 14 with no why: the old call is told cancelled, no record written); `gen += 1`; `runs = record.runs + 1`; writes `running_record(l.result, runs, now)` and answers `{outcome: running, record}` at once. The work, tracked in `pending`, drops itself silently at every step where `gen` moved:
    - connect; `busy += 1` until it ends, then `busy -= 1` and `used_at = now`;
    - a tool: list; no such tool fails `<server> has no tool <tool>; slate_catalog <server> lists its tools`; when not `confirmed` and `asks_now(...) == true` (the list just arrived and says it asks): `hold(code, runs = runs - 1)`, `on_asks`, and stop. Then the args with every mark replaced by its plaintext; an unfilled one fails `the secret $<name> is not filled` (the first unfilled name). Method `tools/call` `{name, arguments}`;
    - a resource: `resources/read` `{uri}`;
    - the deadline is the stdio transport's `toolTimeoutSec * 1000` when set, else `deps.call_ms`, else 60,000; `inflight = (conn, id)`;
    - the result (rule 13); with a `then` and a done result: `deps.reshape(thread, then, raw)` held as `reshaping`; its JSON replaces `json` and drops `text`; a failure gives `{state: failed, why, exit, err}` over the kept `exit`/`out` fields;
    - writes the end record `{...record, startedAt, endedAt, ms, runs}`;
    - any failure: `{state: failed, why: clean(message), exit: null, startedAt, endedAt, ms, runs}`.
13. `result_of` (`:757-779`):
    - resource: the `text` of each content entry joined by `\n`, cleaned, is `out` (`run_output_tail`); exactly one text that parses (rule 7 of the client) gives `json` with every string cleaned. `{state: done, exit: 0, out, json?}`; raw = out;
    - tool with `isError: true`: `{state: failed, why: "the tool said it failed", exit: 1, out: "", err: tail(text)}`;
    - tool: `structured` = `structuredContent` is present and not null, which also sets `seen[(thread, server, tool)]` to true (once true it stays); `json` = the structured content, else the texts (one, or all joined by `\n`) parsed; every string cleaned. `{state: done, exit: 0, out: tail(text), json?, text: true (only when not structured)}`; raw = the cleaned JSON's text when structured, else the text.
14. `stop(thread, run, l, why)` (`:853-865`): `gen += 1`; a call in flight gets `notifications/cancelled` `{requestId, reason: why or "cancelled"}`; a reshape is killed; `held` is cleared. Only with a `why` and a record that was running or held is `{state: cancelled, why, exit: null, runs, endedAt: now}` written (the kept result is not carried).
15. `approve(thread, key, scope)` (`:921-928`): `thread` scope calls `approvals.allow(thread, key)` first; then every run held on that code key, in run order, clears its hold and launches confirmed. `once` stores nothing.
16. `deny(thread, key)` (`:930-936`): each run held on the key: when its record is running only the hold goes (the running call finishes); else stopped with `you said not to run it`.
17. `owns(key)` is `key.starts_with("mcp:")`. `held(thread)` lists the code asks in run order (budget and pressed holds are not asks). `named(thread, key)`: `{run, cmd: "<server>.<tool> <JSON of the shown args>"}` cut to 120 UTF-16 units.
18. `pending(thread, ms = CONNECT_MS)` (`:953-958`): up to 4 rounds, while anything is tracked, of waiting for all tracked work to settle or the one deadline, whichever comes first. A thread never seen returns at once.
19. `cancel(thread, run)`: stop with `cancelled`. `stop_all(thread, {quiet, why})`: every run stopped with no why when quiet, else `why` or `cancelled`.
20. `servers(thread, names, kept)`: replaces `named` and `kept`; every connection whose server is not named closes. `shown(thread, on)`: going from shown to hidden sets every connection's `used_at` to now (the idle clock starts at the hide).
21. The sweep (`:873-886`), every `min(30_000, idle_ms)`: a connection closes when `busy == 0`, its server is not `kept`, the thread is not shown, `now - used_at >= idle_ms`, and the entry is still this connection. `close_servers(thread)` closes every connection of the thread and keeps its runs (a nap); the next start connects again.
22. `catalog(thread, server)` (`:988-1016`): connect and list (failures propagate); with resources offered, `resources/list` (a failure lists none; entries with a string uri kept); `used_at = now`. The text, cleaned, is: the head line `<server>: an MCP server of this thread's agent, <n> tool(s)[, <m> resource(s)].`, the ASKS_LINE, the description rule line, the syntax line, the shape line, a blank line, each tool as `<name> [<hints>][: <sentence(description, 240)>]` with its folded properties or `  (no arguments)`, and when resources exist a blank line and at most 20 lines `resource <uri>[ (<mimeType>)][: <sentence(description)>]`. Pinned verbatim from the TS against the stub fixtures.
23. `refused_runs(thread, before, doc, away)` (`:1018-1028`): `away` when not empty; nothing for no doc; else the bare runs: tool runs with no description whose `stable_json` differs from the same name's run in `before` (absent counts as null). For their servers, the held list or (racing DESCRIBE_MS) a fresh `list_for`. K713 `slate_description_missing("$name", server, tool)` for each bare run whose tool is not marked read-only (an unlisted tool, an unanswered server and a hint-less tool all count as not read-only).
24. `drop(thread, keep = false)`: without `keep`, every run stopped quietly and every connection closed; with `keep`, both left running. Either way the thread's entry, lists and seen marks go. `close()`: stops the sweep, drops every thread.
25. `provisional(thread, decl, last, args)`: `{state: held, why: HELD_APPROVAL, runs: last.runs or 0}` when `asks_now == true` with the listed info, else `running_record(last_result(last), runs + 1, now)`.

## Invariants (named tests)
- A secret's plaintext never reaches a record, an ask or a catalog: `a_secret_reaches_the_call_and_comes_back_scrubbed_from_every_road`.
- One connection per thread and server serves every call: `one_connection_serves_every_call_of_a_thread` (the stub counts its starts).
- A start held beside a running call leaves that call and its record alone: `a_start_held_beside_a_running_call_leaves_it_running`.
- A late answer of a replaced start never writes: `a_replaced_start_never_writes_its_late_answer`.
- The kept result stays in view, marked refreshing, until the new one lands: `a_tool_run_started_again_keeps_its_last_result_marked_refreshing` (TS :605-620).

## Edge cases
- `once: true` beside a running call: `noop`, nothing written.
- Approve on a key nothing holds: only the `thread` scope's `allow`.
- Deny of a held start beside a running call: the call finishes, nothing cancelled.
- A tool found asking only after its list arrived: held with `runs` back at the prior count, no `running` record left behind (the `running` written at launch is replaced by `held`).

## Decisions
1. `SlateMcpDeps` takes `clock: Rc<dyn Clock>` for `now` and every timer (rule 3.15), replacing `now?`.
2. `start` answers synchronously, as the TS; the work is a local task (`spawn_local`), so it needs a `LocalSet`.

## After the port
- `stop` on a held start beside a running call is never reached by `cancel` for the call itself: `cancel` stops both (it bumps gen), kept.
- A `then` failure keeps the call's `out` but drops its `json`, and `exit` becomes the then's: kept.

## Tests (`tests/slate_mcp_runs.rs`)
| Rule | Test |
|---|---|
| 3, 20, 21 | `closes_a_servers_connection_once_hidden_and_idle_unless_an_always_timer_keeps_it` (TS :135-173), `closes_a_server_the_document_no_longer_names` |
| 3 | `names_a_stdio_servers_process_as_the_threads_tool_server_until_closed` (TS :175-204), `a_failed_start_says_what_the_server_said_and_connects_again_next_time` |
| client caps via runs | `cuts_a_server_off_past_2_mb_and_fails_too_deep_or_too_wide_answers` (TS :206-254) |
| 8, 11, 15 | `skips_the_ask_for_a_read_only_tool_and_asks_every_start_of_a_confirm_run` (TS :481-518), `asks_once_per_piece_of_code_and_new_arguments_ask_again`, `at_bypass_starts_a_tool_not_marked_read_only_with_no_ask` |
| 7, 9, 17 | `the_sheet_names_the_tool_its_code_and_dots_for_a_secret`, `named_cuts_to_120`, `a_then_reading_a_slate_file_carries_its_text_in_the_ask` (TS :639-662) |
| 12, 13 | `a_tool_run_fills_json_and_a_text_only_run_says_text`, `a_resource_run_parses_its_one_text`, `a_tool_that_says_it_failed_fails`, `a_missing_tool_or_secret_fails_the_run`, `a_then_reshapes_a_text_tool_into_json`, `a_tools_structured_answer_moves_the_catalog_to_returns_data` |
| 11 | `a_persons_presses_meet_the_start_budget` (TS :622-637), `once_beside_a_running_call_is_noop` |
| 14, 16, 19 | `deny_cancels_a_held_start_and_leaves_a_running_call`, `cancel_tells_the_server_and_writes_cancelled`, `stop_all_quiet_writes_nothing` |
| 22 | `catalog_answers_the_stub_servers_tools_folded_with_how_the_thread_asks` (verbatim TS text) |
| 23 | `refused_runs_refuses_a_new_or_changed_tool_run_with_no_description` (TS :403-427 at the module), `refused_runs_never_judges_a_run_kept_as_it_was` |
| 24 | `drop_with_keep_leaves_the_server_running`, `drop_forgets_lists_and_seen_marks` |
| 25 | `provisional_is_held_where_it_will_ask_else_running_with_the_kept_result` |
| Invariants | the five named above |
