#!@@NODE@@
if (process.argv[2] !== "app-server") process.exit(2);

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

let turns = 0;
let open;
let command;
const terminals = new Set();
const record = text => {
  if (!process.env.CODEX_HOME) return;
  const sessions = require("node:path").join(process.env.CODEX_HOME, "sessions", "2026", "10", "12");
  fs.mkdirSync(sessions, { recursive: true });
  fs.appendFileSync(require("node:path").join(sessions, "rollout-019fd0aa-0000-7000-8000-00000000c0de.jsonl"), JSON.stringify({ text }) + "\n");
};
const endTerminals = () => {
  for (const t of terminals) try { process.kill(-t.pid, "SIGKILL"); } catch {}
};
const quit = () => {
  if (command !== undefined) process.kill(-command.pid, "SIGTERM");
  endTerminals();
  process.exit(0);
};
process.on("SIGTERM", quit);
require("node:readline").createInterface({ input: process.stdin }).on("line", line => {
  const m = JSON.parse(line);
  if (m.method === "initialize") say({ id: m.id, result: {} });
  if (m.method === "account/read" || m.method === "account/rateLimits/read") say({ id: m.id, error: { code: -32600, message: "no account here" } });
  if (m.method === "thread/start" || m.method === "thread/resume") say({ id: m.id, result: { thread: { id: "019fd0aa-0000-7000-8000-00000000c0de", turns: [] }, model: "gpt-5.5" } });
  if (m.method === "thread/unsubscribe") {
    heard("unsubscribe");
    say({ id: m.id, result: { status: "unsubscribed" } });
    setTimeout(endTerminals, Number(process.env.STUB_UNLOAD_MS || 60000));
  }
  if (m.method === "turn/interrupt") {
    say({ id: m.id, result: {} });
    if (open !== undefined && m.params.turnId === open && !process.env.STUB_DEAF) {
      say({ method: "turn/completed", params: { threadId: "019fd0aa-0000-7000-8000-00000000c0de", turn: { id: open, items: [], status: "interrupted" } } });
      open = undefined;
    }
  }
  if (m.method === "turn/start") {
    const text = m.params.input[0].text;
    heard(text);
    say({ id: m.id, result: {} });
    gated(text, () => {
      turns += 1;
      const id = "turn-" + turns;
      say({ method: "turn/started", params: { threadId: "019fd0aa-0000-7000-8000-00000000c0de", turn: { id, items: [], status: "inProgress" } } });
      if (text.startsWith("hang")) {
        open = id;
        if (text === "hang command") {
          command = require("node:child_process").spawn("sleep", ["30"], { detached: true, stdio: "ignore" });
          heard("command " + command.pid);
          say({ method: "item/started", params: { threadId: "019fd0aa-0000-7000-8000-00000000c0de", turnId: id, item: { type: "commandExecution", id: "c" + turns, command: "sleep 30", status: "inProgress" } } });
        }
        return;
      }
      if (text.startsWith("background")) {
        for (let n = 0; n < Number(text.split(" ")[1]); n++) {
          const item = { type: "commandExecution", id: "bg" + turns + "-" + n, command: "sleep 30", source: "unifiedExecStartup" };
          const terminal = require("node:child_process").spawn("sleep", ["30"], { detached: true, stdio: "ignore" });
          terminals.add(terminal);
          heard("terminal " + terminal.pid);
          say({ method: "item/started", params: { threadId: "019fd0aa-0000-7000-8000-00000000c0de", turnId: id, item: { ...item, status: "inProgress" } } });
          terminal.on("exit", () => {
            terminals.delete(terminal);
            say({ method: "item/completed", params: { threadId: "019fd0aa-0000-7000-8000-00000000c0de", turnId: id, item: { ...item, status: "completed" } } });
            record("ended " + item.id);
            heard("ended " + terminal.pid);
          });
        }
      }
      record(text);
      const raced = { type: "commandExecution", id: "raced" + turns, command: "true", source: "unifiedExecStartup" };
      if (text === "raced") say({ method: "item/started", params: { threadId: "019fd0aa-0000-7000-8000-00000000c0de", turnId: id, item: { ...raced, status: "inProgress" } } });
      say({ method: "item/completed", params: { threadId: "019fd0aa-0000-7000-8000-00000000c0de", turnId: id, item: { type: "agentMessage", id: "m" + turns, text: signed(text, turns) } } });
      const end = { method: "turn/completed", params: { threadId: "019fd0aa-0000-7000-8000-00000000c0de", turn: { id, items: [], status: "completed" } } };
      const behind = { method: "item/completed", params: { threadId: "019fd0aa-0000-7000-8000-00000000c0de", turnId: id, item: { ...raced, status: "completed" } } };
      if (text === "raced") process.stdout.write(JSON.stringify(end) + "\n" + JSON.stringify(behind) + "\n");
      else say(end);
    });
  }
}).on("close", quit);
