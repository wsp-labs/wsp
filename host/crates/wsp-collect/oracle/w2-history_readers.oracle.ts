// SPDX-License-Identifier: AGPL-3.0-only
// history_readers: time_of, stamps, session keys and what each reader yields on odd lines.
//   pnpm exec vitest run --config host/crates/wsp-collect/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 host/crates/wsp-collect/oracle/w2-history_readers.oracle.ts
import { it } from "vitest";
import { claudeCall, claudeReader, claudeSession } from "../../../../packages/collect/src/history/claude.js";
import { codexCall, codexReader } from "../../../../packages/collect/src/history/codex.js";
import { hermesCalls, hermesReader } from "../../../../packages/collect/src/history/hermes.js";
import { stampOf, stem, timeOf } from "../../../../packages/collect/src/history/reader.js";
import { host, plain, settle, write, type Laptop } from "./w2-host.js";

const HOME = "/Users/dev";
const ROOT = `${HOME}/.claude/projects`;
const CODEX_ROOT = `${HOME}/.codex/sessions`;
const DB = `${HOME}/.hermes/state.db`;
const Q = "select session_id, tool_calls from messages where tool_calls is not null";
const QC = "select m.session_id, m.tool_calls, s.cwd from messages m left join sessions s on s.id = m.session_id where m.tool_calls is not null";

type Case = Record<string, unknown>;

async function drain(it: AsyncIterable<unknown>): Promise<unknown[]> {
  const out: unknown[] = [];
  for await (const c of it) out.push(c);
  return out;
}

/** A host case over a laptop: the call is built from the host the fixture's laptop makes. */
async function hostCase(name: string, laptop: Laptop, args: unknown[], f: (h: ReturnType<typeof host>) => unknown): Promise<Case> {
  const h = host(laptop);
  const r = await settle(() => f(h));
  return { name, laptop, args, ...r, calls: h.calls };
}

const asst = (blocks: unknown[], extra: Record<string, unknown> = {}): string => JSON.stringify({ type: "assistant", message: { role: "assistant", content: blocks }, ...extra });
const bash = (command: unknown): unknown => ({ type: "tool_use", id: "t", name: "Bash", input: { command } });
const fc = (name: string, args: unknown, extra: Record<string, unknown> = {}): string => JSON.stringify({ type: "response_item", timestamp: "2026-09-06T10:01:00.000Z", payload: { type: "function_call", name, arguments: args }, ...extra });
const s = (o: unknown): string => JSON.stringify(o);

it("history_readers", async () => {
  const fixture: Record<string, Case[]> = {};

  const LOCAL = new Set(["2026-09-06T10:01:00", "2026-09-06 10:01:00", "2026/09/06", "Sep 6, 2026", "6 Sep 2026", "12"]);
  const times: unknown[] = [
    "2026-09-06T10:01:00.000Z", "2026-09-06T10:01:00Z", "2026-09-06T10:01:00.5+05:30", "2026-09-06T10:01:00", "2026-09-06", "2026-13-01T00:00:00Z", "not a time", "", "1999-12-31T23:59:59.999Z",
    "2026-09-06T10:01:00.123456Z", "2026-09-06T10:01Z", "2026-09-06T10:01:00+0530", "2026-09-06T10:01:00+05", "2026-09-06T24:00:00Z", "2026-09-06T25:00:00Z",
    "2026-02-30T00:00:00Z", "2026-02-28T00:00:00Z", "2026-09-06 10:01:00", "2026-09-06 10:01:00Z", "2026/09/06", "Sep 6, 2026", "Sun, 06 Sep 2026 10:01:00 GMT",
    "1757152860000", "2026", "2026-09", "  2026-09-06T10:01:00Z  ", "2026-09-06t10:01:00z", "2026-09-06T10:01:00.Z", "2026-09-06T10:01:00,5Z", "+002026-09-06T00:00:00Z",
    "-000001-01-01T00:00:00Z", "1969-12-31T23:59:59Z", "1970-01-01T00:00:00Z", "0000-01-01T00:00:00Z", "275760-09-13T00:00:00Z", "+275760-09-13T00:00:00.000Z", "+275760-09-13T00:00:00.001Z",
    "2026-09-06T10:01:00.5Z", "2026-09-06T10:01:00.12Z", "2026-09-06T10:01:00-00:00", "2026-09-06T10:01:00+14:00", "2026-09-06T10:01:00+99:00", "2026-09-06T10:01:00 Z", "2026-09-06T10:01:60Z", "2026-09-06T10:60:00Z",
    "2026-09-06T", "T10:01:00Z", "true", "null", "NaN", "Infinity", "12", "1e3", " ", "2026-09-06T10:01:00Z\n", "2026-09-06T10:01:00Z junk", "Sep 6 2026 10:01:00 UTC", "6 Sep 2026",
    1757152860000, null, {}, [], true, ["2026-09-06T10:01:00Z"],
  ];
  fixture["time_of"] = [
    ...times.map(v => ({ name: `${LOCAL.has(v as string) ? "[local time, oracle ran at UTC] " : ""}${JSON.stringify(v)}`, args: [v], out: plain(timeOf(v)) })),
    { name: "absent (undefined)", args: [], out: plain(timeOf(undefined)) },
  ];

  fixture["stem"] = ["/a/b/c.jsonl", "/a/b/.hidden", "/a/b/c.d.jsonl", "c", "/a/b/", "/a.b/c", ".jsonl", "", "/", "a.", "/a/.b.c"].map(p => ({ name: JSON.stringify(p), args: [p], out: stem(p) }));

  fixture["claude_session"] = [
    "-p/abc.jsonl", "-p/abc", "-p/abc/subagents/a.jsonl", "-p/_cleared_sessions/old.jsonl", "-p/_cleared_sessions", "-p/_cleared_sessions/x/y.jsonl", "-p", "", "-p/.jsonl", "-p/a.b.jsonl",
    "-p/memory/notes.jsonl", "-p/s1.jsonl/x.jsonl", "-q/.jsonl", "-p/s1/a/b/c.jsonl",
  ].map(rel => ({ name: JSON.stringify(rel), args: [rel], out: plain(claudeSession(rel)) }));

  const claudeCalls: [string, unknown[]][] = [
    ["shell without folder or time", ["s", "Bash", { command: "ls" }]],
    ["empty folder and zero time are kept", ["s", "Bash", { command: "" }, "", 0]],
    ["Bash with a null input is other", ["s", "Bash", null]],
    ["Bash with a numeric command is other", ["s", "Bash", { command: 5 }, "/f", 7]],
    ["lowercase bash is other", ["s", "bash", { command: "ls" }]],
    ["a string input is other", ["s", "Bash", "ls"]],
    ["another tool", ["s", "Read", { file_path: "/x" }, "/f"]],
  ];
  fixture["claude_call"] = claudeCalls.map(([name, args]) => ({ name, args, out: plain(claudeCall(...(args as [string, string, unknown, string?, number?]))) }));

  const codexCalls: [string, unknown[]][] = [
    ["array joined by a space", ["s", "shell", { command: ["a", "b"] }]],
    ["cmd array wins over command string", ["s", "shell", { cmd: ["a"], command: "z" }]],
    ["an object cmd is other", ["s", "shell", { cmd: { x: 1 } }]],
    ["a numeric cmd does not fall through to command", ["s", "exec_command", { cmd: 5, command: "ls" }]],
    ["a null cmd falls through to command", ["s", "exec_command", { cmd: null, command: "pwd" }]],
    ["an empty cmd string does not fall through", ["s", "exec_command", { cmd: "", command: "pwd" }]],
    ["an empty array is a shell call with an empty line", ["s", "shell", { command: [] }]],
    ["an array with a non-string is other", ["s", "shell", { command: ["ls", 5] }]],
    ["container.exec", ["s", "container.exec", { command: ["ls"] }]],
    ["local_shell with a spaced word", ["s", "local_shell", { command: ["a b", "c"] }, "/f", 3]],
    ["Shell is case sensitive", ["s", "Shell", { command: "ls" }]],
    ["update_plan is other even with a cmd", ["s", "update_plan", { cmd: "ls" }]],
    ["no arguments", ["s", "shell"]],
    ["null arguments", ["s", "shell", null]],
    ["bash -lc line", ["s", "shell", { command: ["bash", "-lc", "git status && ls"] }]],
  ];
  fixture["codex_call"] = codexCalls.map(([name, args]) => ({ name, args, out: plain(codexCall(...(args as [string, string, unknown?]))) }));

  const fn = (name: unknown, args: unknown): unknown => ({ type: "function", function: { name, arguments: args } });
  const tc = (...items: unknown[]): string => JSON.stringify(items);
  const hermes: [string, unknown[]][] = [
    ["absent tool_calls", ["s"]],
    ["null tool_calls", ["s", null]],
    ["a number", ["s", 5]],
    ["an empty string", ["s", ""]],
    ["a string that is not JSON", ["s", "not json"]],
    ["a JSON string of null", ["s", "null"]],
    ["an array passed as is", ["s", [fn("terminal", { command: "tail" })]]],
    ["an object with a function is not a list", ["s", { function: { name: "terminal", arguments: { command: "obj" } } }]],
    ["arguments of every wrong shape", ["s", tc(fn("terminal", "not json"), fn("terminal", "5"), fn("terminal", "[]"), fn("terminal", null), fn("terminal", { command: 5 }), fn("terminal", { command: "" }), fn("Terminal", { command: "x" }))]],
    ["items of every wrong shape", ["s", tc({ function: { name: "x" } }, { function: "x" }, { function: null }, { name: "terminal" }, [fn("terminal", {})], fn("a", 1))]],
    ["a folder is carried, an empty one too", ["s", tc(fn("terminal", JSON.stringify({ command: "ls" }))), ""]],
    ["a folder", ["s", tc(fn("terminal", JSON.stringify({ command: "ls" }))), "/a"]],
  ];
  fixture["hermes_calls"] = hermes.map(([name, args]) => ({ name, args, out: plain(hermesCalls(...(args as [string, unknown, string?]))) }));

  fixture["holder"] = [
    { name: "claude: the project folder", args: ["claude", ROOT, `${ROOT}/-p/s1/a/b/c.jsonl`], out: claudeReader.holder(ROOT, `${ROOT}/-p/s1/a/b/c.jsonl`) },
    { name: "claude: a cleared session", args: ["claude", ROOT, `${ROOT}/-p/_cleared_sessions/old.jsonl`], out: claudeReader.holder(ROOT, `${ROOT}/-p/_cleared_sessions/old.jsonl`) },
    { name: "codex: the file itself", args: ["codex", CODEX_ROOT, `${CODEX_ROOT}/a/x.jsonl`], out: codexReader.holder(CODEX_ROOT, `${CODEX_ROOT}/a/x.jsonl`) },
    { name: "hermes: the file itself", args: ["hermes", DB, DB], out: hermesReader.holder(DB, DB) },
  ];

  const stamps: [string, Laptop, string][] = [];
  for (const m of [1000, 1500.25, 1788688860123.4568, 0, 0.1 + 0.2, 1e21, 1e-7, -5, 123456789012345680000]) stamps.push([`mtime ${m}`, { files: { "~/a": 4 }, mtimes: { "~/a": m } }, `${HOME}/a`]);
  stamps.push(["an empty file", { files: { "~/a": "" } }, `${HOME}/a`], ["a folder", { files: { "~/d/x": 1 } }, `${HOME}/d`], ["a missing path", { files: { "~/a": 1 } }, `${HOME}/none`], ["a text file's bytes are UTF-8", { files: { "~/a": "é\u{1F600}" } }, `${HOME}/a`]);
  fixture["stamp_of"] = [];
  for (const [name, laptop, path] of stamps) fixture["stamp_of"]!.push(await hostCase(name, laptop, [path], h => stampOf(h, path)));

  const line = (o: unknown): string => JSON.stringify(o);
  fixture["claude_reader_files"] = [
    await hostCase("which files a claude root keeps", { files: {
      "~/.claude/projects/stray.jsonl": asst([bash("ls")], { cwd: "/x" }),
      "~/.claude/projects/-p/memory/notes.jsonl": asst([bash("ls")], { cwd: "/x" }),
      "~/.claude/projects/-p/s1/a/b/c.jsonl": asst([bash("ls")], { cwd: "/x" }),
      "~/.claude/projects/-p/s1.jsonl/x.jsonl": asst([bash("ls")], { cwd: "/x" }),
      "~/.claude/projects/-p/_cleared_sessions/old.jsonl": asst([bash("ls")], { cwd: "/x" }),
      "~/.claude/projects/-p/_cleared_sessions/z.txt": "x",
      "~/.claude/projects/-p/UP.JSONL": asst([bash("ls")], { cwd: "/x" }),
      "~/.claude/projects/-p/_cleared_sessions": "a file named cleared",
      "~/.claude/projects/-p/readme.md": "x",
      "~/.claude/projects/-q/.jsonl": asst([bash("ls")], { cwd: "/x" }),
    } }, [ROOT], h => claudeReader.files(h, ROOT)),
    await hostCase("no root", {}, [ROOT], h => claudeReader.files(h, ROOT)),
  ];

  const claudeLines = [
    asst([bash("gh pr view"), { type: "tool_use", name: "Read", input: { file_path: "/x" } }], { cwd: "/a", timestamp: "2026-09-06T10:01:00.000Z" }),
    asst([bash("ls")], { cwd: "" }),
    asst([bash("pwd")], { cwd: 5 }),
    asst([{ type: "tool_use", name: "Bash" }], { timestamp: 5 }),
    asst([{ type: "tool_use", name: "Bash", input: { command: 5 } }]),
    asst([{ type: "tool_use", name: "Bash", input: { command: "" } }]),
    asst([{ type: "tool_use", name: "Bash", input: "ls" }]),
    asst([{ type: "tool_use", name: "bash", input: { command: "ls" } }]),
    asst([{ type: "tool_use", name: 5, input: {} }, { type: "tool_use" }, "x", null, { type: "text", text: "tool_use" }, { type: "tool_use", name: "Task", input: null }]),
    line({ type: "user", message: { content: [{ type: "tool_use", name: "Bash", input: { command: "ls" } }] } }),
    line({ type: "assistant", message: { content: "tool_use assistant" } }),
    line({ type: "assistant", message: "tool_use" }),
    line({ type: "assistant", message: null, x: "tool_use" }),
    line([{ type: "assistant", x: "tool_use" }]),
    line({ type: "assistant", message: { content: [{ type: "tool_use", name: "A", input: {} }] }, extra: { type: "assistant" } }).replace('"assistant"', '"Assistant"'),
    '{"type":"assistant" "tool_use" broken',
    "",
    asst([{ type: "tool_use", name: "WebFetch", input: { url: "http://x" } }], { cwd: "/a" }),
  ];
  const S1 = "~/.claude/projects/-p/s1.jsonl";
  fixture["claude_reader_read"] = [
    await hostCase("eighteen lines of every shape", { files: { [S1]: claudeLines.join("\n") } }, [ROOT, `${ROOT}/-p/s1.jsonl`], h => drain(claudeReader.read(h, ROOT, `${ROOT}/-p/s1.jsonl`))),
    await hostCase("a path with no session yields nothing and opens nothing", { files: { "~/.claude/projects/stray.jsonl": claudeLines[0]! } }, [ROOT, `${ROOT}/stray.jsonl`], h => drain(claudeReader.read(h, ROOT, `${ROOT}/stray.jsonl`))),
    await hostCase("a failing line read ends the stream with its error", { files: { [S1]: `${claudeLines[0]!}\n\u0000fail:EIO` } }, [ROOT, `${ROOT}/-p/s1.jsonl`], h => drain(claudeReader.read(h, ROOT, `${ROOT}/-p/s1.jsonl`))),
    await hostCase("sub-agent transcript is the top-level session", { files: { "~/.claude/projects/-p/s9/subagents/agent-a.jsonl": asst([bash("gh x")], { cwd: "/a" }) } }, [ROOT, `${ROOT}/-p/s9/subagents/agent-a.jsonl`], h => drain(claudeReader.read(h, ROOT, `${ROOT}/-p/s9/subagents/agent-a.jsonl`))),
  ];

  const R = `${CODEX_ROOT}/2026/06/01/rollout-x-t1.jsonl`;
  const meta = (cwd: unknown): string => line({ type: "session_meta", payload: { id: "t1", cwd } });
  const rollout = [
    fc("exec_command", s({ cmd: "ls" })),
    meta("/first"), meta("/second"),
    fc("exec_command", s({ cmd: 5, command: "ls" })),
    fc("exec_command", s({ cmd: null, command: "pwd" })),
    fc("exec_command", s({ cmd: "", command: "pwd" })),
    fc("shell", s({ command: ["bash", "-lc", "git status && ls"] })),
    fc("shell", s({ command: [] })),
    fc("shell", s({ command: ["ls", 5] })),
    fc("shell", s({ command: "ls -la" })),
    fc("shell", s({})),
    fc("shell", "not json"),
    fc("shell", "5"),
    fc("shell", "[]"),
    fc("shell", { command: "obj" }),
    fc("shell", undefined),
    fc("shell", null),
    fc("container.exec", s({ command: ["ls"] })),
    fc("local_shell", s({ command: ["a b", "c"] })),
    fc("Shell", s({ command: "ls" })),
    fc("update_plan", s({ cmd: "ls" })),
    line({ type: "response_item", payload: { type: "function_call", name: 5, arguments: "{}" } }),
    line({ type: "event_msg", payload: { type: "function_call", name: "shell", arguments: s({ command: "ls" }) } }),
    line({ type: "response_item", payload: { type: "custom_tool_call", name: "apply_patch", input: "function_call" } }),
    line({ type: "response_item", payload: "function_call" }),
    line({ type: "turn_context", payload: { cwd: "/third", note: "session_meta" } }),
    "garbage function_call",
    fc("exec_command", s({ cmd: "after" }), { timestamp: "not a time" }),
  ];
  const codexRead = (name: string, text: string, file = R): Promise<Case> => hostCase(name, { files: { [file.replace(HOME, "~")]: text } }, [CODEX_ROOT, file], h => drain(codexReader.read(h, CODEX_ROOT, file)));
  const RY = `${CODEX_ROOT}/2026/06/01/rollout-y.jsonl`;
  const RZ = `${CODEX_ROOT}/2026/06/01/rollout-z.jsonl`;
  fixture["codex_reader_read"] = [
    await codexRead("a rollout of every shape", rollout.join("\n")),
    await codexRead("a turn_context line that mentions session_meta sets the folder", [line({ type: "turn_context", payload: { cwd: "/third", note: "session_meta" } }), meta("/fourth"), fc("shell", s({ command: "ls" }))].join("\n"), RY),
    await codexRead("an empty cwd wins and blocks the later one", ["session_meta nonsense", meta(5), meta(""), meta("/late"), fc("shell", s({ command: "ls" }))].join("\n"), RZ),
    await codexRead("a failing line read", `${fc("shell", s({ command: "ls" }))}\n\u0000fail:EIO`),
  ];
  fixture["codex_reader_files"] = [
    await hostCase("which files a codex root keeps", { files: { "~/.codex/sessions/a/x.jsonl": "", "~/.codex/sessions/b.jsonl": "x", "~/.codex/sessions/.jsonl": "x", "~/.codex/sessions/c.json": "x", "~/.codex/sessions/d/e.JSONL": "x", "~/.codex/sessions/archived/f.jsonl": "x" } }, [CODEX_ROOT], h => codexReader.files(h, CODEX_ROOT)),
  ];

  const hermesLaptop = (exec: Record<string, string>, files: Record<string, number | string> = { "~/.hermes/state.db": 4096 }): Laptop => ({ files, exec });
  const J = (x: unknown): string => JSON.stringify(x);
  const read = (l: Laptop, name: string): Promise<Case> => hostCase(name, l, [DB, DB], h => drain(hermesReader.read(h, DB, DB)));
  const kq = (q: string): string => `sqlite3 -readonly -json ${DB} ${q}`;
  fixture["hermes_reader_files"] = [
    await hostCase("the database is there", { files: { "~/.hermes/state.db": 4096 } }, [DB], h => hermesReader.files(h, DB)),
    await hostCase("no database", {}, [DB], h => hermesReader.files(h, DB)),
    await hostCase("the root is a folder", { files: { "~/.hermes/state.db/x": 1 } }, [DB], h => hermesReader.files(h, DB)),
  ];
  fixture["hermes_reader_read"] = [
    await read(hermesLaptop({ [kq(QC)]: "Error: no such table", [kq(Q)]: "[]" }), "a first answer that is not JSON: no fallback"),
    await read(hermesLaptop({ [kq(QC)]: "null", [kq(Q)]: "[]" }), "a first answer of null: no fallback"),
    await read(hermesLaptop({ [kq(QC)]: "[]", [kq(Q)]: J([{ session_id: "x", tool_calls: tc(fn("terminal", {})) }]) }), "a first answer of [] is not None: no fallback"),
    await read(hermesLaptop({ [kq(QC)]: "\n", [kq(Q)]: "[]" }), "a blank first answer: no rows, one query"),
    await read(hermesLaptop({ [kq(Q)]: J([{ session_id: "h1", tool_calls: tc(fn("terminal", J({ command: "ls" }))) }]) }), "the plain query answers after the joined one has no answer"),
    await read(hermesLaptop({}), "no answer for either query"),
    await read(hermesLaptop({ [kq(QC)]: J({ x: 1 }) }), "an object answer"),
    await read(hermesLaptop({
      [kq(QC)]: J([
        { session_id: "h1", tool_calls: tc(fn("terminal", J({ command: "ls" }))), cwd: "/a" },
        { session_id: "h1", tool_calls: tc(fn("terminal", J({ command: "pwd" }))), cwd: null },
        { session_id: "h1", tool_calls: tc(fn("terminal", J({ command: "pwd" }))), cwd: "" },
        { session_id: "h2", tool_calls: [fn("terminal", { command: "tail" })] },
        { session_id: "h3", tool_calls: tc(fn("terminal", "not json"), fn("terminal", "5"), fn("terminal", "[]"), fn("terminal", null), fn("terminal", { command: 5 }), fn("terminal", { command: "" }), fn("Terminal", { command: "x" })) },
        { session_id: "h4", tool_calls: "[]" },
        { session_id: "h5", tool_calls: "null" },
        { session_id: "h6", tool_calls: { function: { name: "terminal", arguments: { command: "obj" } } } },
        { session_id: "h7", tool_calls: tc({ function: { name: "x" } }, { function: "x" }, { function: null }, { name: "terminal" }, [fn("terminal", {})], fn("a", 1)) },
        { session_id: "", tool_calls: tc(fn("terminal", { command: "emptysession" })) },
        { session_id: null, tool_calls: tc(fn("terminal", {})) },
        "row", null,
        { tool_calls: tc(fn("terminal", {})) },
      ]),
    }), "a row table of every shape"),
  ];

  write("history_readers", fixture);
});
