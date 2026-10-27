#!@@NODE@@
const args = process.argv.slice(2);
let sid = "";
for (let i = 0; i < args.length; i++) if (args[i] === "--session-id" || args[i] === "--resume") sid = args[i + 1];
if (!sid && args.includes("--safe-mode") && process.env.STUB_TITLE) {
  setTimeout(() => { console.log(JSON.stringify({ type: "result", is_error: false, result: process.env.STUB_TITLE })); process.exit(0); }, Number(process.env.STUB_TITLE_MS || 0));
  return;
}
if (!sid) { console.log("2.1.289 (Claude Code)"); process.exit(0); }

const fs = require("node:fs");
const say = o => process.stdout.write(JSON.stringify(o) + "\n");
const heard = text => process.env.STUB_HEARD && fs.appendFileSync(process.env.STUB_HEARD, text + "\n");
const gated = (text, go) => {
  if (!text.includes("later") || !process.env.STUB_GATE) return go();
  const poll = setInterval(() => {
    if (!fs.existsSync(process.env.STUB_GATE)) return;
    clearInterval(poll);
    go();
  }, 20);
};
const signed = (text, turns) => text + " from " + process.pid + " turn " + turns + " token " + (process.env["WSP_TURN"] || "none");

heard("up " + process.pid + " " + args.join(" "));
if (process.env.STUB_EARLY) say({ type: "system", subtype: "hook_started", hook_id: "h1", hook_name: "SessionStart:startup", hook_event: "SessionStart", session_id: sid });
let cost = 0;
let turns = 0;
let hanging = false;
let deaf = false;
let background;
const result = (text, extra) => say({ type: "result", subtype: "success", is_error: false, duration_ms: 5, num_turns: 1, result: text, session_id: sid, total_cost_usd: cost, usage: { input_tokens: 1, output_tokens: 1 }, ...extra });
const endBackground = () => {
  if (background === undefined) return;
  say({ type: "system", subtype: "task_notification", task_id: background, status: "stopped", summary: "Wait 75 seconds", session_id: sid });
  say({ type: "system", subtype: "background_tasks_changed", tasks: [], session_id: sid });
  background = undefined;
};
require("node:readline").createInterface({ input: process.stdin }).on("line", line => {
  const m = JSON.parse(line);
  if (m.type === "control_request" && m.request.subtype === "stop_task") {
    heard("stop_task " + m.request.task_id);
    say({ type: "control_response", response: { subtype: "success", request_id: m.request_id } });
    if (m.request.task_id === background) endBackground();
    return;
  }
  if (m.type === "control_request" && m.request.subtype === "interrupt") {
    say({ type: "control_response", response: { subtype: "success", request_id: m.request_id } });
    if (!hanging || deaf) return;
    hanging = false;
    result("", { subtype: "error_during_execution", terminal_reason: "aborted_streaming" });
    // The CLI stops a background command of an interrupted turn on its own only seconds later.
    if (background !== undefined) setTimeout(endBackground, 3000);
    return;
  }
  if (m.type !== "user") return;
  const text = m.message.content.at(-1).text;
  heard(text);
  if (process.env.STUB_SESSIONS) fs.appendFileSync(require("node:path").join(process.env.STUB_SESSIONS, sid + ".jsonl"), JSON.stringify({ type: "user", text }) + "\n");
  gated(text, () => {
    say({ type: "system", subtype: "init", session_id: sid, cwd: process.cwd(), model: "claude-sonnet-4-5", tools: [], permissionMode: "default", slash_commands: [], apiKeySource: "none" });
    turns += 1;
    if (text.startsWith("hang")) {
      hanging = true;
      deaf = text === "hang deaf";
      if (text === "hang background") {
        background = "bg" + turns;
        say({ type: "system", subtype: "background_tasks_changed", tasks: [{ task_id: background, description: "Wait 75 seconds" }], session_id: sid });
      }
      if (text === "hang talk") {
        const own = (id, block) => say({ type: "assistant", session_id: sid, parent_tool_use_id: null, uuid: id + block.type, message: { id, role: "assistant", content: [block] } });
        own("talk1", { type: "text", text: "alpha" });
        own("talk1", { type: "tool_use", id: "call1", name: "Bash", input: { command: "echo one" } });
        say({ type: "user", session_id: sid, parent_tool_use_id: null, message: { role: "user", content: [{ type: "tool_result", tool_use_id: "call1", content: "one", is_error: false }] } });
        own("talk2", { type: "text", text: "beta" });
      }
      return;
    }
    if (text === "leave a child") heard("child " + require("node:child_process").execSync("sh -c 'sleep 30 > /dev/null 2>&1 & echo $!'", { encoding: "utf8" }).trim());
    cost += 0.25;
    const said = signed(text, turns);
    say({ type: "assistant", session_id: sid, parent_tool_use_id: null, uuid: "a" + turns, message: { id: "msg" + turns, role: "assistant", content: [{ type: "text", text: said }] } });
    result(said);
  });
}).on("close", () => setTimeout(() => process.exit(0), Number(process.env.STUB_EXIT_MS || 0)));
