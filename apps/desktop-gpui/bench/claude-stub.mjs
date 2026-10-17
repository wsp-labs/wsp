#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// The claude the fixture's host runs: it answers stream-json as the claude adapter reads it, and each turn plays the
// turn of content.mjs that $HOME/bench-turn.json names, so no real agent runs and every run writes the same rows.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { turnsOf } from "./content.mjs";

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

const usage = { input_tokens: 1, output_tokens: 1 };
const held = new Map();

function play() {
  const { thread, turn } = JSON.parse(readFileSync(join(process.env.HOME, "bench-turn.json"), "utf8"));
  if (!held.has(thread)) held.set(thread, turnsOf(thread));
  const { tools, reply } = held.get(thread)[turn];
  const at = `${thread}-${turn}`;
  const assistant = (id, content) => say({ type: "assistant", message: { id: `msg_${id}`, type: "message", role: "assistant", model: "claude-sonnet-5", content, stop_reason: null, usage }, parent_tool_use_id: null, session_id: sid, uuid: `a_${id}` });
  say({ type: "system", subtype: "init", cwd: process.cwd(), session_id: sid, tools: [], mcp_servers: [], model: "claude-sonnet-5", permissionMode: "bypassPermissions", slash_commands: [], apiKeySource: "none", uuid: `init_${at}` });
  tools.forEach((call, i) => {
    const id = `${at}-${i}`;
    assistant(id, [{ type: "tool_use", id: `toolu_${id}`, name: call.name, input: call.input }]);
    say({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: `toolu_${id}`, content: call.output, is_error: false }] }, parent_tool_use_id: null, session_id: sid, uuid: `u_${id}` });
  });
  assistant(`${at}-reply`, [{ type: "text", text: reply }]);
  say({ type: "result", subtype: "success", is_error: false, result: reply, duration_ms: 10, num_turns: 1, session_id: sid, total_cost_usd: 0, usage, uuid: `r_${at}` });
}

let buf = "";
process.stdin.on("data", d => {
  buf += d;
  for (let i = buf.indexOf("\n"); i >= 0; i = buf.indexOf("\n")) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    if (line.trim() === "") continue;
    const m = JSON.parse(line);
    if (m.type === "control_request") say({ type: "control_response", response: { subtype: "success", request_id: m.request_id } });
    else if (m.type === "user") play();
  }
});
process.stdin.on("end", () => process.exit(0));
