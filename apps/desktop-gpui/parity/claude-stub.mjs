#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// The claude the parity driver points the fixture's host at. A message the driver scripted in $HOME/parity-turns.json
// (its prompt mapped to a reply) plays that reply in stream-json as the claude adapter reads it; any other message plays
// what the bench's stub plays. A reply is one part or a list of parts, each a kind or { kind, ...options }:
// markdown, code, table, long, tools, error, thinking, question, permission, plan, rate-limit, retry, files, hold, and
// unstarted first of all.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { turnsOf } from "../bench/content.mjs";

const argv = process.argv.slice(2);
const at = flag => (argv.indexOf(flag) >= 0 ? argv[argv.indexOf(flag) + 1] : undefined);
const sid = at("--session-id") ?? at("--resume");
const say = o => process.stdout.write(`${JSON.stringify(o)}\n`);

if (sid === undefined) {
  if (argv.includes("--help")) console.log('--permission-mode <mode>  (choices: "default", "bypassPermissions")');
  else if (argv.includes("--bare")) say({ type: "control_response", response: { subtype: "success", request_id: "init", response: { models: [{ value: "default", resolvedModel: "claude-haiku-5-5" }] } } });
  else console.log("2.1.295 (Claude Code)");
  process.exit(0);
}
// The host stops a held turn's process by closing its input or by a signal; either is a clean end.
for (const signal of ["SIGTERM", "SIGINT", "SIGHUP"]) process.on(signal, () => process.exit(0));

const usage = { input_tokens: 1, output_tokens: 1 };
// The adapter answers prompts itself on a turn whose init says bypass, so the init names the mode it was launched at.
const mode = argv.includes("--dangerously-skip-permissions") ? "bypassPermissions" : (at("--permission-mode") ?? "default");
const MODEL = "claude-sonnet-5";
let seq = 0;
const next = () => `${Date.now().toString(36)}-${++seq}`;
const assistant = (content, parent = null) => {
  const id = next();
  say({ type: "assistant", message: { id: `msg_${id}`, type: "message", role: "assistant", model: MODEL, content, stop_reason: null, usage }, parent_tool_use_id: parent, session_id: sid, uuid: `a_${id}` });
};
const toolResult = (toolUseId, content, { isError = false, parent = null, facts } = {}) =>
  say({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: toolUseId, content, is_error: isError }] }, parent_tool_use_id: parent, session_id: sid, uuid: `u_${next()}`, ...(facts !== undefined ? { tool_use_result: facts } : {}) });
const call = (name, input, parent = null) => {
  const id = `toolu_${next()}`;
  assistant([{ type: "tool_use", id, name, input }], parent);
  return id;
};

/** Answers to the control requests this process sent the host, by request id. */
const answers = new Map();
const ask = (toolName, toolUseId, input) =>
  new Promise(done => {
    const requestId = `ask_${next()}`;
    answers.set(requestId, done);
    say({ type: "control_request", request_id: requestId, request: { subtype: "can_use_tool", tool_name: toolName, tool_use_id: toolUseId, input } });
  });
/** Ends the turn being played when the host interrupts it. */
let interrupted;

const FILES = ["apps/web/src/components/chat/timeline/list.tsx", "packages/protocol/src/thread-read.ts", "apps/desktop/src/host-lifecycle.ts", "packages/runtime/src/session.ts"];
const MARKDOWN = [
  "## What I changed",
  "",
  "The list now keeps the **measured height** of each row, so a long tool output no longer moves the rows above it. See `useChatThread` in [list.tsx](https://example.com/list.tsx).",
  "",
  "### Steps",
  "",
  "1. Read the scroller and its `scrollTop` writes.",
  "2. Held the anchor row through a resize.",
  "   - nested: the anchor survives a page load",
  "3. Added a test for the paging stop.",
  "",
  "- `HostClient` keeps one socket",
  "- `threadMessages` pages 200 events at a time",
  "",
  "| File | Lines | Change |",
  "| --- | ---: | --- |",
  "| list.tsx | 42 | anchor kept |",
  "| rows.tsx | 7 | height cached |",
  "",
  "> The second read is a no-op, so the count matched.",
  "",
  "Run `pnpm test` to check it.",
].join("\n");
const codeBlock = lines => `Here is the whole file:\n\n\`\`\`ts\n${Array.from({ length: lines }, (_, i) => (i % 9 === 0 ? `export function row${i}(rows: readonly Row[], at: number): number {` : i % 9 === 8 ? "}" : `  const height${i} = rows[at + ${i}]?.height ?? measure(rows, at + ${i}, { cache: true, fallback: ${i * 3} });`)).join("\n")}\n\`\`\``;
const table = columns => {
  const heads = Array.from({ length: columns }, (_, i) => ["File", "Owner", "Lines", "Added", "Removed", "Tests", "Status", "Notes", "Branch", "Reviewer", "Opened", "Merged"][i] ?? `Column ${i + 1}`);
  const rows = Array.from({ length: 6 }, (_, r) => heads.map((_, c) => (c === 0 ? FILES[r % FILES.length] : c === columns - 1 ? "kept the anchor row through every resize of the list" : `${(r + 1) * (c + 3)}`)));
  return [`| ${heads.join(" | ")} |`, `| ${heads.map(() => "---").join(" | ")} |`, ...rows.map(r => `| ${r.join(" | ")} |`)].join("\n");
};
const WORDS = "the list keeps every row it has measured so nothing jumps when a long tool output lands and the anchor holds through the resize".split(" ");
const longText = () => {
  const paragraphs = [];
  for (let p = 0, n = 0; n < 2000; p++) {
    const words = Array.from({ length: 100 }, (_, i) => WORDS[(p * 7 + i) % WORDS.length]);
    n += words.length;
    paragraphs.push(words.join(" ").replace(/^./, c => c.toUpperCase()) + ".");
  }
  paragraphs.splice(3, 0, `The path was ${"averyveryverylongunbrokenidentifierwithnospaces".repeat(6)} and nothing wrapped it.`);
  return paragraphs.join("\n\n");
};
const diffFacts = file => ({ filePath: file, structuredPatch: [{ oldStart: 40, oldLines: 4, newStart: 40, newLines: 5, lines: ["   const rows = list.rows;", "-  const height = rows[at].height;", "+  const height = rows[at]?.height ?? measure(rows, at);", "+  anchor.hold(at);", "   return height;", " }"] }] });
const TOOLS = {
  Bash: () => {
    const id = call("Bash", { command: "rg -n scrollTop apps/web/src", description: "Find the scroll writes" });
    toolResult(id, `${FILES[0]}:418: scroller.scrollTop = offset;\n${FILES[0]}:692: const top = scroller.scrollTop;`, { facts: { stdout: "", stderr: "" } });
  },
  Read: () => {
    const id = call("Read", { file_path: `/work/${FILES[1]}` });
    toolResult(id, Array.from({ length: 12 }, (_, i) => `${String(i + 1).padStart(6)}\texport const page${i} = ${i * 200};`).join("\n"));
  },
  Edit: () => {
    const file = `/work/${FILES[0]}`;
    const id = call("Edit", { file_path: file, old_string: "  const height = rows[at].height;", new_string: "  const height = rows[at]?.height ?? measure(rows, at);\n  anchor.hold(at);" });
    toolResult(id, `The file ${file} has been updated.`, { facts: diffFacts(file) });
  },
  Grep: () => {
    const id = call("Grep", { pattern: "HISTORY_PAGE_EVENTS", path: "packages" });
    toolResult(id, `Found 3 files\n${FILES.slice(1).join("\n")}`);
  },
  Write: () => {
    const file = "/work/apps/web/src/components/chat/timeline/anchor.ts";
    const content = "export function hold(at: number): void {\n  held = at;\n}\n";
    const id = call("Write", { file_path: file, content });
    toolResult(id, `File created successfully at: ${file}`, { facts: { type: "create", filePath: file, content, structuredPatch: [] } });
  },
  TodoWrite: () => {
    const id = call("TodoWrite", { todos: [{ content: "Read the scroller", status: "completed", activeForm: "Reading the scroller" }, { content: "Hold the anchor row", status: "in_progress", activeForm: "Holding the anchor row" }, { content: "Add the paging test", status: "pending", activeForm: "Adding the paging test" }] });
    toolResult(id, "Todos have been modified successfully.");
  },
  // Without listed, the host is never told of the subagent, so the app folds its lines under the call instead of a tile.
  Task: ({ listed = true } = {}) => {
    const id = call("Task", { description: "Trace the scroll writes", prompt: "Find every write to scrollTop in apps/web and say which one runs on resize.", subagent_type: "general-purpose" });
    const task = `agent_${next()}`;
    if (listed) say({ type: "system", subtype: "task_started", task_id: task, tool_use_id: id, task_type: "local_agent", description: "Trace the scroll writes", session_id: sid, uuid: `s_${next()}` });
    assistant([{ type: "text", text: "I will look for the scroll writes first." }], id);
    const child = call("Grep", { pattern: "scrollTop", path: "apps/web/src" }, id);
    toolResult(child, `${FILES[0]}:418\n${FILES[0]}:692`, { parent: id });
    assistant([{ type: "text", text: "Two writes; the one at line 418 runs on resize." }], id);
    if (listed) say({ type: "system", subtype: "task_notification", task_id: task, status: "completed", session_id: sid, uuid: `s_${next()}` });
    toolResult(id, "Two writes to scrollTop: list.tsx:418 runs on resize, list.tsx:692 on paging.");
  },
};

/** Plays one part of a reply; answers the text it said, if any. */
async function part(p) {
  switch (p.kind) {
    case "text":
    case "markdown": {
      const text = p.text ?? MARKDOWN;
      assistant([{ type: "text", text }]);
      return text;
    }
    case "code": {
      const text = codeBlock(p.lines ?? 80);
      assistant([{ type: "text", text }]);
      return text;
    }
    case "table": {
      const text = `The changes by file:\n\n${table(p.columns ?? 8)}`;
      assistant([{ type: "text", text }]);
      return text;
    }
    case "long": {
      const text = longText();
      assistant([{ type: "text", text }]);
      return text;
    }
    case "thinking":
      assistant([{ type: "thinking", thinking: p.text ?? "The anchor moves when the tool output lands.\nThe list measures rows after paint.\nSo the fix is to hold the anchor through the resize.", signature: "s" }]);
      return undefined;
    case "tools": {
      const names = p.names ?? Object.keys(TOOLS);
      for (let i = 0; i < (p.n ?? names.length); i++) TOOLS[names[i % names.length]](p);
      return undefined;
    }
    case "plan": {
      const plan = p.text ?? ["# Plan", "", ...Array.from({ length: 28 }, (_, i) => (i % 7 === 0 ? `## Step ${i / 7 + 1}` : `- Change \`${FILES[i % FILES.length]}\` so the anchor row holds (${i}).`))].join("\n");
      const id = call("ExitPlanMode", { plan });
      toolResult(id, "User has approved your plan. You can now start coding.");
      return undefined;
    }
    case "rate-limit": {
      const resetsAt = p.resetsAt ?? Math.floor(Date.now() / 1000) + 2 * 3600;
      const five = (p.fiveHour ?? 42) / 100;
      const week = (p.sevenDay ?? 63) / 100;
      say({ type: "rate_limit_event", rate_limit_info: { status: "allowed", rateLimitType: "five_hour", utilization: five, resetsAt, unifiedWindows: { five_hour: { utilization: five, resetsAt }, seven_day: { utilization: week, resetsAt: resetsAt + 4 * 86400 } } }, session_id: sid, uuid: `r_${next()}` });
      return undefined;
    }
    case "retry":
      say({ type: "system", subtype: "api_retry", attempt: p.attempt ?? 2, max_retries: p.max ?? 10, retry_delay_ms: 4000, error: "overloaded", error_status: 529, session_id: sid, uuid: `s_${next()}` });
      return undefined;
    case "files": {
      for (const file of p.paths ?? ["src/a.ts", "src/b/c.ts"]) {
        mkdirSync(dirname(join(process.cwd(), file)), { recursive: true });
        writeFileSync(join(process.cwd(), file), `export const changed = "${file}";\n`);
        const id = call("Write", { file_path: join(process.cwd(), file), content: `export const changed = "${file}";\n` });
        toolResult(id, `File created successfully at: ${join(process.cwd(), file)}`);
      }
      return undefined;
    }
    case "question": {
      const input = { questions: [{ question: "Which file first?", header: "Order", multiSelect: false, options: [{ label: "list.tsx", description: "The list" }, { label: "rows.tsx", description: "The rows" }] }] };
      const id = call("AskUserQuestion", input);
      const answer = await held(ask("AskUserQuestion", id, input));
      toolResult(id, `User answered: ${JSON.stringify(answer?.updatedInput?.answers ?? {})}`);
      return undefined;
    }
    case "permission": {
      const input = { command: p.command ?? "rm -rf build", description: "Remove the build folder" };
      const id = call("Bash", input);
      const answer = await held(ask("Bash", id, input));
      if (answer?.behavior === "allow") toolResult(id, "", { facts: { stdout: "", stderr: "" } });
      else toolResult(id, answer?.message ?? "The user doesn't want to proceed with this tool use.", { isError: true });
      return undefined;
    }
    case "error":
      return undefined;
    case "hold":
      if (p.call !== false) call("Bash", { command: p.command ?? "pnpm test", description: "Run the tests" });
      await held(new Promise(() => {}));
      return undefined;
    default:
      throw new Error(`the parity stub has no reply kind ${p.kind}`);
  }
}

/** Waits for `promise`, or ends the turn when the host interrupts it first. */
const held = promise => Promise.race([promise, new Promise((_, stop) => (interrupted = stop))]);
class Stopped extends Error {}

function scripted(prompt) {
  let turns;
  try {
    turns = JSON.parse(readFileSync(join(process.env.HOME, "parity-turns.json"), "utf8"));
  } catch {
    return undefined;
  }
  const key = Object.keys(turns).filter(k => prompt.includes(k)).sort((a, b) => b.length - a.length)[0];
  return key === undefined ? undefined : turns[key];
}

async function playScripted(spec) {
  const parts = (Array.isArray(spec) ? spec : [spec]).map(p => (typeof p === "string" ? { kind: p } : p));
  let last = "";
  try {
    for (const p of parts) last = (await part(p)) ?? last;
  } catch (e) {
    if (!(e instanceof Stopped)) throw e;
    say({ type: "result", subtype: "error_during_execution", is_error: false, terminal_reason: "aborted_streaming", errors: ["Request was aborted."], duration_ms: 10, num_turns: 1, session_id: sid, total_cost_usd: 0, usage, uuid: `r_${next()}` });
    return;
  }
  if (parts.some(p => p.kind === "error")) {
    const id = call("Bash", { command: "pnpm test", description: "Run the tests" });
    toolResult(id, "Exit code 1\nFAIL apps/web/src/components/chat/timeline/list.test.tsx\n  the anchor holds: expected 418, got 0", { isError: true, facts: "Error: Exit code 1" });
    const words = parts.find(p => p.kind === "error").text ?? "API Error: 529 Overloaded. The service is busy; try again in a moment.";
    say({ type: "result", subtype: "success", is_error: true, result: words, duration_ms: 10, num_turns: 1, session_id: sid, total_cost_usd: 0, usage, uuid: `r_${next()}` });
    return;
  }
  if (last === "") assistant([{ type: "text", text: (last = "Done.") }]);
  say({ type: "result", subtype: "success", is_error: false, result: last, duration_ms: 10, num_turns: 1, session_id: sid, total_cost_usd: 0, usage, uuid: `r_${next()}` });
}

const bench = new Map();
function playBench() {
  const { thread, turn } = JSON.parse(readFileSync(join(process.env.HOME, "bench-turn.json"), "utf8"));
  if (!bench.has(thread)) bench.set(thread, turnsOf(thread));
  const { tools, reply } = bench.get(thread)[turn];
  const at = `${thread}-${turn}`;
  const line = (id, content) => say({ type: "assistant", message: { id: `msg_${id}`, type: "message", role: "assistant", model: MODEL, content, stop_reason: null, usage }, parent_tool_use_id: null, session_id: sid, uuid: `a_${id}` });
  tools.forEach((c, i) => {
    const id = `${at}-${i}`;
    line(id, [{ type: "tool_use", id: `toolu_${id}`, name: c.name, input: c.input }]);
    say({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: `toolu_${id}`, content: c.output, is_error: false }] }, parent_tool_use_id: null, session_id: sid, uuid: `u_${id}` });
  });
  line(`${at}-reply`, [{ type: "text", text: reply }]);
  say({ type: "result", subtype: "success", is_error: false, result: reply, duration_ms: 10, num_turns: 1, session_id: sid, total_cost_usd: 0, usage, uuid: `r_${at}` });
}

const promptOf = m => {
  const content = m.message?.content;
  return typeof content === "string" ? content : Array.isArray(content) ? content.map(b => b?.text ?? "").join("\n") : "";
};
/** Messages wait their turn: a message sent while one is played is played after it, as the CLI queues it. */
let queue = Promise.resolve();
function turn(m) {
  queue = queue.then(async () => {
    let spec = scripted(promptOf(m));
    // The init line is what starts the turn on the host, so holding it keeps the send standing before its start.
    const [first, ...rest] = spec === undefined ? [] : [spec].flat().map(p => (typeof p === "string" ? { kind: p } : p));
    if (first?.kind === "unstarted") {
      try {
        await held(first.ms === undefined ? new Promise(() => {}) : new Promise(done => setTimeout(done, first.ms)));
      } catch {
        process.exit(0);
      }
      spec = rest;
    }
    say({ type: "system", subtype: "init", cwd: process.cwd(), session_id: sid, tools: [], mcp_servers: [], model: MODEL, permissionMode: mode, slash_commands: [], apiKeySource: "none", uuid: `init_${next()}` });
    if (spec !== undefined) await playScripted(spec);
    else playBench();
  });
}

let buf = "";
process.stdin.on("data", d => {
  buf += d;
  for (let i = buf.indexOf("\n"); i >= 0; i = buf.indexOf("\n")) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    if (line.trim() === "") continue;
    const m = JSON.parse(line);
    if (m.type === "control_request") {
      say({ type: "control_response", response: { subtype: "success", request_id: m.request_id } });
      if (m.request?.subtype === "interrupt") interrupted?.(new Stopped());
    } else if (m.type === "control_response") {
      const done = answers.get(m.response?.request_id);
      answers.delete(m.response?.request_id);
      done?.(m.response?.response);
    } else if (m.type === "user") turn(m);
  }
});
process.stdin.on("end", () => process.exit(0));
