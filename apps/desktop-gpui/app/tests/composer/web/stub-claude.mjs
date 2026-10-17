#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// The claude the composer shots' host runs, put first on its PATH by shots.mjs: it answers stream-json as the claude
// adapter reads it. A prompt that is one of the bench's turns (bench/content.mjs) plays that turn; one holding SLOW
// runs until it is interrupted, QUESTION asks to run `rm -rf build` and waits for the answer, TASKS writes a four-step
// plan and waits, LIMIT ends on the plan's usage limit with its reset two hours ahead; anything else is answered at
// once. Each turn's process leaves its pid in $HOME/turns while it runs, so shots.mjs can stop any left behind.
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { THREADS, turnsOf } from "../../../../bench/content.mjs";

const argv = process.argv.slice(2);
const at = flag => (argv.indexOf(flag) >= 0 ? argv[argv.indexOf(flag) + 1] : undefined);
const sid = at("--session-id") ?? at("--resume");
const say = o => process.stdout.write(`${JSON.stringify(o)}\n`);

if (sid === undefined) {
  if (argv.includes("--help")) console.log('--permission-mode <mode>  (choices: "default", "acceptEdits", "plan", "bypassPermissions")');
  else if (argv.includes("--bare")) say({ type: "control_response", response: { subtype: "success", request_id: "init", response: { models: [{ value: "default", resolvedModel: "claude-haiku-5-5" }] } } });
  else if (argv[0] === "agents") console.log("[]");
  else if (argv[0] === "auth") console.log(JSON.stringify({ loggedIn: true, authMethod: "claude.ai", subscriptionType: "max" }));
  else console.log("2.1.295 (Claude Code)");
  process.exit(0);
}

const mark = join(process.env.HOME, "turns", String(process.pid));
mkdirSync(join(process.env.HOME, "turns"), { recursive: true });
writeFileSync(mark, "");
process.on("exit", () => rmSync(mark, { force: true }));
for (const signal of ["SIGTERM", "SIGINT", "SIGHUP"]) process.on(signal, () => process.exit(0));

const bench = new Map(Object.keys(THREADS).flatMap(name => turnsOf(name).map(turn => [turn.prompt, turn])));
const model = at("--model") ?? "claude-sonnet-5";
// The adapter answers every prompt itself under bypassPermissions, which it reads off this line.
const mode = argv.includes("--dangerously-skip-permissions") ? "bypassPermissions" : (at("--permission-mode") ?? "default");
const usage = { input_tokens: 1, output_tokens: 1 };
let n = 0;
let waiting = false;

const init = () => say({ type: "system", subtype: "init", cwd: process.cwd(), session_id: sid, tools: [], mcp_servers: [], model, permissionMode: mode, slash_commands: [], apiKeySource: "none", uuid: `init_${++n}` });
const assistant = content => {
  const id = ++n;
  say({ type: "assistant", message: { id: `msg_${id}`, type: "message", role: "assistant", model, content, stop_reason: null, usage }, parent_tool_use_id: null, session_id: sid, uuid: `a_${id}` });
  return id;
};
const toolResult = (id, content) => say({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: id, content, is_error: false }] }, parent_tool_use_id: null, session_id: sid, uuid: `u_${++n}` });
const result = (subtype, more) => say({ type: "result", subtype, is_error: false, duration_ms: 10, num_turns: 1, session_id: sid, total_cost_usd: 0, usage, uuid: `r_${++n}`, ...more });
const reply = words => {
  assistant([{ type: "text", text: words }]);
  result("success", { result: words });
};

const TODOS = [
  { content: "Read how the transcript list measures its rows", status: "completed", activeForm: "Reading how the transcript list measures its rows" },
  { content: "Pin the scroll anchor before tool output mounts", status: "in_progress", activeForm: "Pinning the scroll anchor before tool output mounts" },
  { content: "Add a test for a long tool output landing", status: "pending", activeForm: "Adding a test for a long tool output landing" },
  { content: "Run the transcript tests", status: "pending", activeForm: "Running the transcript tests" },
];

function play(prompt) {
  init();
  const turn = bench.get(prompt.trim());
  if (turn !== undefined) {
    for (const call of turn.tools) {
      const id = `toolu_${++n}`;
      assistant([{ type: "tool_use", id, name: call.name, input: call.input }]);
      toolResult(id, call.output);
    }
    return reply(turn.reply);
  }
  if (prompt.includes("SLOW")) {
    waiting = true;
  } else if (prompt.includes("QUESTION")) {
    const id = `toolu_${++n}`;
    const input = { command: "rm -rf build", description: "Remove the build folder" };
    assistant([{ type: "tool_use", id, name: "Bash", input }]);
    say({ type: "control_request", request_id: `ask_${++n}`, request: { subtype: "can_use_tool", tool_name: "Bash", input, tool_use_id: id } });
    waiting = true;
  } else if (prompt.includes("TASKS")) {
    assistant([{ type: "tool_use", id: `toolu_${++n}`, name: "TodoWrite", input: { todos: TODOS } }]);
    waiting = true;
  } else if (prompt.includes("LIMIT")) {
    const resetsAt = Math.floor(Date.now() / 1000) + 2 * 60 * 60;
    say({ type: "rate_limit_event", rate_limit_info: { status: "rejected", rateLimitType: "five_hour", utilization: 1, resetsAt }, uuid: `l_${++n}`, session_id: sid });
    const words = `You've hit your limit · resets at ${new Date(resetsAt * 1000).toISOString()}`;
    assistant([{ type: "text", text: words }]);
    say({ type: "result", subtype: "success", is_error: true, duration_ms: 10, num_turns: 1, session_id: sid, total_cost_usd: 0, usage, result: words, uuid: `r_${++n}` });
  } else {
    reply(`Done: ${prompt.trim().split("\n")[0]}`);
  }
}

const textOf = content => (typeof content === "string" ? content : (content ?? []).filter(c => c.type === "text").map(c => c.text).join(""));

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
      if (m.request?.subtype === "interrupt" && waiting) {
        waiting = false;
        result("error_during_execution", { errors: ["interrupted by the person"] });
      }
    } else if (m.type === "control_response" && waiting) {
      waiting = false;
      reply("Removed the build folder.");
    } else if (m.type === "user") {
      const prompt = textOf(m.message?.content);
      if (waiting) assistant([{ type: "text", text: `steered: ${prompt}` }]);
      else play(prompt);
    }
  }
});
process.stdin.on("end", () => process.exit(0));
