# 10 The command line and the MCP tools

The `wsp` command line, its `--json` contract, and the MCP tool server. The root `AGENTS.md` states the contract once ("The command line, the MCP tools and the skill are one contract"); this chapter pins how the code enforces it. Every `file:line` is at `a3b870a6e`; V is `packages/host/src/verbs.ts`, VC is `packages/host/src/verbs/client.ts`, CMD is `packages/host/src/cli/commands.ts`.

The Rust tool server already exists (`daemon/crates/wsp-mcp`, 15,959 lines) and answers byte for byte like the TS server, held by a record the TS tests regenerate. Under Proposal 1 the command line is `wsp-cli`, a socket client of the host that depends on the wire and the words alone (rule 1.6), and the tool server stays `wsp-mcp`.

## The verb table

One table, `ALL_VERBS`, seven groups in order: computers, agents, projects, workspaces, threads, conversations, activity (V:388-396). Its order is the TS server's `tools/list` order. An entry is one of four shapes (VC:859-905):

| Shape | Fields | Count |
|---|---|---|
| `CliVerb` | `name` (the words), `usage`, `about`, `page`, `options` (node `parseArgs` rows plus `valueWith`), `run(ctx) -> exit code`, `tool`, and `readsHere`, `startsNoHost`, `anyRelease`, `cloud`, `cloudFlags` | most |
| `ToolOnlyVerb` | `name`, `tool`, `toolOnly` (why) | 1: `projects add` |
| `CliOnlyVerb` | a `CliVerb` without `tool`, with `cliOnly` (why) and `hostSide` | 6: `usage reset`, `agents signin`, `agents key`, `servers signin`, `image export`, `ssh` |
| `CommandToolVerb` | `name`, `tool`, `command: true` | 1: `add` |

A `Tool` is `{description, input: zod shape, output: zod shape, stream?, call(args, deps)}` (VC:808-827). `stream` has no run-time effect: only `packages/host/test/contract.test.ts:638-646` reads it. With the cloud on there are 91 entries and 85 tools; with it off, 80 entries and 76 tools (the 11 cloud verbs leave: `rename, snapshot, fork, pause, wake, rebuild, image export, image remove, forget, ssh, export`). The cloud is on only when the build carries it and `WSP_CLOUD` is exactly `1` (`packages/host/src/cloud.ts:15-17`). The cut (#2441) removes `snapshot`, `image remove`, `recipe`, `recipe scan` and `init`.

Other command lines live in `COMMANDS` (CMD:115-423): `up, down, status, host pair, host devices, host link, host unlink, login, logout, hosts, init, add, remove, join, leave, doctor`, parsed by one `parseArgs` over the union of their flags (`SHARED_OPTIONS`, `packages/host/src/cli/flags.ts:145-183`); `mcp` and `mcp install` have their own parse (`cli/mcp.ts:21-87`). `COMMAND_LINES` (CMD:499-533) joins every verb, `mcp`, `mcp install`, the 16 commands and `host devices revoke`: 108 lines with the cloud on, 97 off. Pages, skill examples and parity tests are all built from it.

10.1. A verb MUST be found by the longest name, measured in characters, whose words equal the opening words of the line (`findVerb`, V:440-445; `findCommand`, CMD:445-450 alike), after the common flags `--host`, `--state`, `--json`, `--help` and `-h` are lifted off the whole line (`takeCommon`, VC:939-965; `cli.ts:76-83`). Any other flag before the verb's words stops the lookup. `--` ends the lift. A string flag with no value at the end of the line stays for the verb's own parse to refuse.

10.2. Tool names MUST be the verb's words joined with `_` (`toolName`, VC:920-922) and tool inputs the flag words with `-` as `_`. A list input is a `multiple` flag (`parity.test.ts:216-257`).

## A verb's run

10.3. From argv to exit code (V:729-799; VC:173-421):
1. parse with strict `parseArgs`, positionals allowed, the verb's options plus the common ones; a relative `--cwd` is refused here (`--cwd is a path on the machine, absolute, and got "packages". Give a path that opens with /, since whoever reads it works in a folder this line cannot see.`, VC:993-1002);
2. `--help` prints the verb's page and returns 0;
3. pick the state file (11) and the aim, in the order `--host` or `WSP_HOST`, the launch pair `WSP_HOST_URL` and `WSP_HOST_TOKEN`, a live lock here, the account's one host, else here with a start (`aimedHost`, `packages/host/src/hosts.ts:296-356`);
4. dial lazily, once per line: one deadline of 5 s (here or loopback) or 15 s covers the socket's open and the first answer; the first frame is `auth` with the token, or `seal.open` first when the aim pins a host key; a host started when none serves, as rule 6.6 says;
5. check the release: the auth reply's `version` must equal this wsp's, unless the verb is `anyRelease` (only `restart`) or the host names none; otherwise refuse with kind `usage`: `this line runs wsp <mine> and the host serving <state> runs wsp <theirs>: <fix>` (VC:331-336,1017-1048);
6. run; a request is `JSON.stringify({...params, id, op})` with ids from 1, a reply with `ok !== true` becomes an error carrying `kind` (a string) and `errors` and `warnings` (arrays) (VC:229-283);
7. a socket closed with 4001 fails what waits with `the host is restarting; the turn goes on and its reply lands in the thread`; a long wait then redials every 200 ms for 20 s, starting nothing (VC:254-258,399-421).

10.4. A refused token MUST class as auth (exit 2): a reply with kind `auth`, or a reply with no kind followed within 500 ms by close 4401 (VC:311-327). A missing token file is `the host's token file is missing: <path>` (auth). No lock is `no wsp host is serving <state>; start one with wsp up --state <state>` (provider). A dial that times out is kind `unreachable`: `the host at <where> did not answer: nothing came back within <ms> ms` (`hosts.ts:240-249`).

## The --json contract

10.5. With `--json`, stdout MUST carry JSON alone: one object per line written by `jsonLine(value)` = `escapeC1(JSON.stringify(value))` (`packages/protocol/src/exit.ts:49-54`), so DEL and the C1 controls are `\u007f` to `\u009f` escapes; frames first, the result last. Frames are simply the `emit` calls before the last one (VC:461-471). The declared streams: `exec` (`output`), `slate write`, `slate state`, `slate read` (`text`), `fork` (`workspace`, `notice`, cloud), `add` (`setup`, `waiting`). `wsp exec --json` prints `{"type":"exec.output","execId":"<id>","text":"ok"}` per line then `{"exitCode":0,"cwd":"/root/alpha"}`; `wsp run --json` prints a `thread` frame, every `session.*` event as the host pushed it, then the result.

10.6. A refusal or failure MUST be one line on stderr and nothing on stdout: under `--json` the failure object `{error, class, exit}` in that key order, plus `errors` and `warnings` when they are arrays (`verbFailure`, exit.ts:56-67); without it the sentence, with the prefix `wsp <verb>: ` dropped when the sentence already opens with the verb's name (`sayOnce`, `protocol/src/words/thread.ts:414-418`). `left` rides on the host's reply but never reaches this object (VC:267). An error escaping `cli()` is printed as plain text even under `--json` (`entry.ts:47-52`).

10.7. The exit code MUST be the class's: 0 ok, 1 provider, 2 auth, 3 usage (05). Exceptions the contract names: `wsp exec` exits with the command's own code, or 1 when it has none (`verbs/threads.ts:575-576`); a confirmation answered no at a terminal prints `<name> kept` and exits 1 with no JSON; `wsp status` exits 1 when no host answers.

10.8. A line that takes something away MUST ask only after the host's own refusals pass, and off a terminal without `--yes` MUST refuse with `<question's first line> There is no terminal to answer on. Pass --yes to say yes.` (usage) (`verbs/io.ts:217-223`; `parity.test.ts:592-604`).

## Refusal sentences

10.9. The command line's own refusals MUST be word for word (each pinned by the test named):

| Case | Sentence | Pinned by |
|---|---|---|
| unknown verb | `unknown command: nope. Run wsp --help for the list.` | `refusals.test.ts:45` |
| unknown under mcp | `unknown command: mcp nope. Run wsp mcp --help for the list.` | `refusals.test.ts:53` |
| a word that opens lines | `wsp host opens a line rather than being one.` then the usages | `refusals.test.ts:56-61` |
| a flag another verb reads | `--ssh belongs to wsp computers set; wsp run does not read it` then `usage: ...` | `refusals.test.ts:122` |
| an unknown flag | node's own `Unknown option '--nope'...` then `usage: <verb usage>` | `contract.test.ts:671-675` |
| a flag on a shared line | `--recipe belongs to wsp add and wsp init; wsp down does not read it. usage: wsp down` | `pages.test.ts:208` |
| `--json` on a prose command | `Unknown option '--json' for wsp status: it answers in prose. That flag belongs to wsp init, wsp add, wsp remove, and to every verb.` | `contract.test.ts:909-910` |
| missing arguments | `wsp exec takes a thread, then -- and the command. usage: wsp exec <thread> [--cwd <dir>] -- <command...>` | `verbs-exec-host.test.ts:326` |
| empty message | `wsp run: the message is empty; say what the thread is to do. Put the message in quotes: wsp run <project> "say hi".` | `refusals.test.ts:138` |
| a page that is not one | `wsp --help takes a page, and got nope. The pages are wsp --help agent, wsp --help dev and wsp host --help.` | `pages.test.ts:184` |

The unknown-flag text is node's `parseArgs` message, which the Rust parser does not produce; Proposal 9 (14) covers it with the schema refusal.

## Help pages

10.10. The help pages MUST be the TS pages' text (`packages/host/src/cli/help.ts`; V:462-710): `wsp`, `wsp --help` and `wsp help` print `HELP`; `--help agent`, `--help dev` and `wsp host --help` the three pages; `wsp <verb> --help` the verb's page (usage wrapped at 80 columns, the about, one row per flag from `FLAG_WORDS`, then `--json --state --host`); `wsp help <verb>` only the verb's `usage:` lines. No golden file pins the text: `pages.test.ts` and `parity.test.ts` pin its structure, words and widths, so the harness's CLI tapes (#2443) are what hold it byte for byte.

## The MCP tool server

The TS server (`packages/host/src/mcp.ts:56-87`) is the SDK's `McpServer` on stdio, with every write passed through `escapeC1`. `wsp mcp` hands off to the daemon binary's server when the binary carries one (`binary mcp --help` exits 0 and prints `--wsp-argv`, `mcp-install.ts:109-112`), else serves in process. An agent's config launches `<binary> mcp --wsp-argv <word>... --state <state> [--host <alias>]` (`mcpServerSpec`, `mcp-install.ts:143-169`).

10.11. `initialize` MUST answer `{"result":{"protocolVersion":..,"capabilities":{"tools":{"listChanged":true}},"serverInfo":{"name":"wsp","version":"<release>"},"instructions":".."},"jsonrpc":"2.0","id":N}`, `result` first; the protocol version is the client's if supported, else `2025-11-25` (supported: `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`, `2024-10-07`). The instructions are one of six recorded texts by scoped, slate and cloud, each at most 2,048 characters (`packages/host/src/skill.ts:34-66`; `daemon/crates/wsp-mcp/record/server.json`).

10.12. `tools/list` MUST list one entry per tool as `{name, description, inputSchema, outputSchema, execution: {taskSupport: "forbidden"}, _meta}` in that key order, schemas as draft-07 JSON Schema with `additionalProperties: false`; only the four `slate_*` tools carry `"anthropic/alwaysLoad": true` (`mcp.ts:62-69`; `parity.test.ts:391-418`). A guest server omits `recipe`, `recipe_scan` and `terminal_config`; `--no-slate` omits the four slate tools.

10.13. `tools/call` MUST answer a refusal as a result, never a JSON-RPC error: `{content: [{type: "text", text: error}], structuredContent: {error, class, exit[, errors, warnings]}, isError: true}` (`toolFailure`, VC:1092-1095). Arguments that fail the input schema answer `MCP error -32602: Input validation error: Invalid arguments for tool <tool>: <zod issue lines>` as an `isError` result, and an unknown tool `MCP error -32602: Tool <name> not found` likewise; an unknown method is JSON-RPC `-32601`; a request with no `id` or `method` is dropped; `ping` answers `{}` (`wsp-mcp/src/stdio.rs:85-183`; 33 recorded cases in `wsp-mcp/tests/refusals.json`). A tool never prompts: only `delete`, `image_remove` and `projects_seed` take `confirm`, and without it answer what they would take with `isError: true` and do nothing.

10.14. `wsp mcp --scoped` MUST refuse with `this thread's tools were launched without its own host token, so they refuse rather than act as the person; the agent did not pass WSP_HOST_URL and WSP_HOST_TOKEN to its MCP server` (auth, exit 2) unless both launch variables are set, before reading the state (`cli/mcp.ts:36-38`; `protocol/src/words/computer.ts:233`). A scoped server's aim is the launch pair, ahead of any host this computer holds, and an aim with a token must hold the host's key fingerprint unless its address is loopback (`hosts.ts:313-354`).

## What the Rust side must keep passing

The byte hold works by recording, not by building TS: `packages/host/test/mcp-record.test.ts` regenerates `daemon/crates/wsp-mcp/record/` (85 tool files, the six instruction texts, host constants, words, exit classes) and `tests/answers/*.json` (561 cases) from the real TS server, and fails on any byte of drift; `daemon/crates/wsp-mcp/tests/contract.rs` replays each case against a scripted host on both the host road and the guest road; `packages/host/test/mcp-binary.test.ts` holds the binary's tool list, greeting and answers equal to the TS command line's `--json`. The traps that test found (`daemon/crates/wsp-mcp/CONTRIBUTING.md:29-37`): a view is raw JSON, not a re-serialized `Value`, so JavaScript's key order survives; a row the TS parses with zod is a struct in the schema's field order; numbers are never `f64`; sentences come from the record.

10.15. When the TS host is retired, the record's source moves: until cutover the TS server regenerates the record; after it, the Rust command line's `--json` output is the record's source, and `mcp-binary.test.ts`'s comparison runs against the Rust command line instead. The parity, skill and contract tests (`packages/host/test/parity.test.ts`, `skill.test.ts`, `contract.test.ts`) MUST be ported as the Rust command line's own contract suite before the TS command line is deleted (12).

10.16. Four differences between the TS command line and `wsp-mcp` exist today and MUST be settled toward the TS behaviour in the port (`wsp-mcp` notes): the Rust lock reader skips the program-gone check (rule 6.1); the Rust aim skips an empty `--host` where TS refuses it (`hosts.ts:297-300`); the Rust starter skips the service-owns-this-state refusal (rule 6.6 step 1); `wsp-daemon mcp`'s clap parse exits 2 on a bad flag where the TS command line's usage class is 3.

## State files the command line reads

10.17. The state file MUST be chosen in this order (`statePick`, `packages/host/src/cli/state.ts:48-68`): `--state`; `$WSP_HOME/state.json` when `WSP_HOME` is non-empty; `./.wsp/state.json` when the folder's `package.json` names `wsp` (a checkout, a worktree included); else `~/.wsp/state.json`. A named state that differs from a checkout's mark prints `this runs against <chosen>, named by --state|WSP_HOME; the checkout in <cwd> marks <dev>, which this run does not use.` once. The path is made absolute. `--state` on a line aimed at another host is ignored with a note.

10.18. A verb line writes nothing beside the state except through the host, with two exceptions: `hosts/<alias>.json` rewritten after an admission wrote a new device token, and the starter's `host.log` (VC:196; `host-start.ts:73-78`).
