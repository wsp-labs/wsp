import { it } from "vitest";
import { writeFileSync } from "node:fs";
import { CATALOG_AGENTS } from "@wsp/catalog";
import { activityOf, type ActivityProc, type ActivityView, CONVERSATIONS_AGAIN_FIX, conversationsFailedLine } from "@wsp/protocol";
import { OUTSIDE_PORTS, OUTSIDE_PROCS, OUTSIDE_START } from "/root/wsp-vm/wt-2441-cut/packages/protocol/test/fixtures/activity-outside.ts";
import { activityTable } from "/root/wsp-vm/wt-2441-cut/packages/host/src/verbs/activity-table.ts";
import { activityLines, endedLines } from "/root/wsp-vm/wt-2441-cut/packages/host/src/verbs/activity.ts";
import { conversationsLines } from "/root/wsp-vm/wt-2441-cut/packages/host/src/verbs/conversations.ts";
const MB = 1024 * 1024;
let started = 1_760_000_000_000;
const proc = (pid: number, ppid: number, depth: number, kind: ActivityProc["kind"], name: string, command: string, mb: number, cpu = 0, ports: number[] = []): ActivityProc => ({
  pid,
  ppid,
  depth,
  startedAt: started++,
  kind,
  name,
  command,
  memory: mb * MB,
  cpu,
  ports,
});

const RUN = "bash /home/dev/.wsp/runs/af35cb2fea39.sh";
const LONG = "Build: Kept-run tests leave their bash and tail running after they finish, and a second clause long enough to pass any terminal (#2137)";

const turn: ActivityProc[] = [
  proc(69887, 48420, 0, "agent", "bash", RUN, 1.1),
  proc(69888, 69887, 1, "agent", "bash", RUN, 1.1),
  proc(69891, 69888, 2, "agent", "2.1.296", "claude -p --input-format stream-json --output-format stream-json --verbose --dangerously-skip-permissions --model claude-opus-5-5 --effort high -", 193.6, 0.4),
  proc(69959, 69891, 3, "tool", "uv", "/opt/homebrew/bin/uv tool uvx mcp-search-console", 29.6),
  proc(69998, 69959, 4, "other", "python3.13", "/home/dev/.cache/uv/archive-v0/djbB/bin/python /home/dev/.cache/uv/archive-v0/djbB/bin/mcp-search-console", 63.8),
  proc(69961, 69891, 3, "tool", "uv", "/opt/homebrew/bin/uv tool uvx mcp-proxy-for-aws@latest https://aws-mcp.example.test/mcp --metadata INSTALL_SOURCE=aws-cli", 23.4),
  proc(70003, 69961, 4, "other", "python3.13", "/home/dev/.cache/uv/archive-v0/RBQV/bin/python /home/dev/.cache/uv/archive-v0/RBQV/bin/mcp-proxy-for-aws https://aws-mcp.example.test/mcp", 79.1),
  proc(69890, 1, 0, "agent", "tail", "tail -n +1 -f /home/dev/.wsp/runs/af35cb2fea39.in", 0.9),
  proc(69892, 1, 0, "agent", "bash", RUN, 1.2),
];

const VIEW: ActivityView = {
  computer: "lab-mac.local",
  at: 1_760_000_100_000,
  threads: [
    {
      threadId: "11111111-1111-4111-8111-111111111111",
      title: "Fix the lab dashboard",
      workspaceId: "ws_lab",
      memory: 395.3 * MB,
      cpu: 0.4,
      processes: turn,
    },
    {
      threadId: "22222222-2222-4222-8222-222222222222",
      title: LONG,
      workspaceId: "ws_lab",
      memory: 452 * MB,
      cpu: 12.5,
      processes: [
        proc(71000, 1, 0, "agent", "codex", "/opt/homebrew/bin/codex exec --json -", 210, 2.5),
        proc(71010, 71000, 1, "terminal", "popbar", "popbar --watch", 15),
        proc(71020, 71000, 1, "dev", "node", "node /home/dev/acme/lab/node_modules/.bin/vite --port 5174", 227, 10, [5174]),
      ],
    },
  ],
  wsp: [
    proc(48000, 1, 0, "wsp", "node", "/usr/local/bin/node /opt/wsp/packages/host/dist/bin.js host serve", 514, 0.3, [4400, 4420]),
    proc(48001, 48000, 1, "wsp", "wsp-daemon", "/opt/wsp/daemon/aarch64-apple-darwin/wsp-daemon serve --port 59312", 56, 1.1, [59312]),
  ],
  unowned: [proc(73000, 1, 0, "test", "node", "node /home/dev/acme/lab/node_modules/.bin/vitest run", 140, 88)],
};


it("dumps", () => {
  const early: ActivityView = { ...VIEW, threads: [{ ...VIEW.threads[0]!, processes: [turn[0]!, turn[7]!] }], wsp: [], unowned: [] };
  const wideTitles: ActivityView = { ...VIEW, threads: [{ ...VIEW.threads[0]!, title: "修复实验室仪表板上的内存图表在窗口缩小时溢出的问题并补上测试用例与说明文档的更新" }, { ...VIEW.threads[1]!, title: "🚀🔥✅ Ship the lab dashboard 🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪" }] };
  const box: ActivityView = { ...VIEW, computer: "lab-box", wsp: [VIEW.wsp[1]!] };
  const empty: ActivityView = { computer: "lab-mac.local", at: 0, threads: [], wsp: [], unowned: [] };
  const outside = activityOf({ computer: "lab-mac.local", at: OUTSIDE_START, procs: OUTSIDE_PROCS, daemon: 20, host: 10, scopes: [], agentBins: CATALOG_AGENTS.map(a => a.bin), ports: OUTSIDE_PORTS });
  const ties: ActivityView = { computer: "c", at: 0, threads: [
    { threadId: "b", title: "B", workspaceId: "w", memory: 10 * MB, cpu: 0, processes: [proc(9, 1, 0, "dev", "node", "node x", 1)] },
    { threadId: "a", title: "A", workspaceId: "w", memory: 10 * MB, cpu: 0, processes: [proc(8, 1, 0, "dev", "node", "node y", 1), proc(3, 1, 0, "dev", "node", "node z", 1)] },
  ], wsp: [], unowned: [] };
  const views = { VIEW, early, wideTitles, box, empty, outside, ties };
  const tables: Record<string, string> = {};
  for (const [name, v] of Object.entries(views)) for (const [w, wide] of [[80, false], [160, false], [40, false], [100, false], [Infinity, false], [80, true], [20, false], [1, false]] as const) tables[`${name} ${w} ${wide ? "wide" : "fold"}`] = activityTable(v, { width: w, wide });
  const lines: Record<string, string> = {};
  for (const [name, v] of Object.entries(views)) lines[name] = activityLines(v);
  const p = (pid: number, name: string) => proc(pid, 1, 0, "dev", name, `${name} --x`, 1);
  const ended = {
    freed: endedLines({ pid: 1, ended: [], forced: [], left: [], freed: "11111111-2222" }),
    gone: endedLines({ pid: 42, ended: [], forced: [], left: [] }),
    one: endedLines({ pid: 42, ended: [p(42, "node")], forced: [], left: [] }),
    under: endedLines({ pid: 42, ended: [p(42, "node"), p(43, "esbuild"), p(44, "sh")], forced: [43], left: [] }),
    left: endedLines({ pid: 42, ended: [p(42, "node"), p(43, "esbuild")], forced: [42, 43], left: [43] }),
    freedWithEnded: endedLines({ pid: 1, ended: [p(1, "x")], forced: [1], left: [], freed: "t" }),
  };
  const failed = { agent: "claude" as const, said: conversationsFailedLine("Claude Code", "at most 64 entries, got 74"), fix: CONVERSATIONS_AGAIN_FIX, why: "at most 64 entries, got 74" };
  const conv = {
    none: conversationsLines({ rows: [], held: [] }),
    heldOnly: conversationsLines({ rows: [], held: [failed] }),
    rowsAndHeld: conversationsLines({ rows: [{ agent: "codex", id: "01a12813-cd12-7a12-9b13-e76892906ff0", title: "lab terminal thread", cwd: "/work/acme/lab", lastAt: 0, origin: "terminal", live: false }], held: [failed] }),
    marks: conversationsLines({ rows: [
      { agent: "claude", id: "a", title: "t\tab\nnew", cwd: "/w", lastAt: 1791673748000, bytes: 0, origin: "wsp", live: false },
      { agent: "claude", id: "b", title: "x", cwd: "/w", lastAt: 1791673748123, bytes: 1536, origin: "terminal", live: true, thread: "0123456789abcdef" },
      { agent: "codex", id: "c", title: "", cwd: "/w", lastAt: -1, branch: "main", origin: "script", live: false },
    ], held: [] }),
  };
  writeFileSync("/root/wsp-vm/wt-2449-cli-b/host/crates/wsp-cli/tests/fixtures/cli_b/pure.json", JSON.stringify({ source: "TS at 65f6b89c7, views from packages/host/test/activity-table.test.ts", views, tables, lines, ended, conversations: conv }, null, 1) + "\n");
});
