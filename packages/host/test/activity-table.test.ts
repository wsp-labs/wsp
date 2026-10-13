// SPDX-License-Identifier: AGPL-3.0-only
// wsp activity's table against a read shaped like a real one: a Claude Code turn
// with its run script, its input pump and two tool servers, a second thread with
// a title longer than any terminal, wsp's host and daemon, and a test run with no
// thread. It reads as Settings > Activity names it at 80 and 160 columns.
import { describe, expect, it } from "vitest";
import { CATALOG_AGENTS } from "@wsp/catalog";
import { ACTIVITY_GROUP_WORDS, ACTIVITY_KIND_WORDS, activityOf, UNTAGGED_KINDS, type ActivityProc, type ActivityView } from "@wsp/protocol";
import { OUTSIDE_NO_THREAD, OUTSIDE_PORTS, OUTSIDE_PROCS, OUTSIDE_START } from "../../protocol/test/fixtures/activity-outside.js";
import { activityTable } from "../src/verbs/activity-table.js";
import { cellWidth } from "../src/init-layout.js";

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

const AT_80 = `lab-mac.local   wsp 1.5 GB   CPU 102%   11 processes   2 threads

                                                                   MEMORY    CPU
wsp  The app and what runs it                                      570 MB   1.4%
  Host :4400 :4420                                                 514 MB   0.3%
    Daemon :59312                                                   56 MB   1.1%
Build: Kept-run tests leave their bash and tail running after th…  452 MB  12.5%
  Codex                                                            210 MB   2.5%
    vite :5174            Dev server                               227 MB  10.0%
    popbar                Terminal                                  15 MB   0.0%
Fix the lab dashboard                                              395 MB   0.4%
  Claude Code                                                      198 MB   0.4%
    mcp-search-console    Tool server                               30 MB   0.0%
      mcp-search-console                                            64 MB   0.0%
    mcp-proxy-for-aws     Tool server                               23 MB   0.0%
      mcp-proxy-for-aws                                             79 MB   0.0%
No thread  Started with no thread wsp knows of                     140 MB  88.0%
  vitest                  Test run                                 140 MB  88.0%`;

describe("wsp activity's table", () => {
  it("names each process as the page does, folds the run script and input pump into the agent's row and fits 80 columns", () => {
    const text = activityTable(VIEW, { width: 80, wide: false });
    expect(text).toBe(AT_80);
    for (const line of text.split("\n")) expect(line.length, line).toBeLessThanOrEqual(80);
    expect(text).not.toMatch(/runs\/|tail -n|claude -p|uv tool|\b69887\b/);
  });

  it("at 160 columns cuts only a title wider than the names' room, and every figure stands in one column", () => {
    const lines = activityTable(VIEW, { width: 160, wide: false }).split("\n");
    for (const line of lines) expect(line.length, line).toBeLessThanOrEqual(160);
    expect(lines.find(l => l.startsWith("Build: "))).toBe(`${LONG.slice(0, 71)}…  452 MB  12.5%`);
    const rows = lines.slice(2);
    expect(new Set(rows.map(l => l.length)).size, rows.join("\n")).toBe(1);
    expect(new Set(rows.slice(1).map(l => l.indexOf(" MB") + 3)).size, rows.join("\n")).toBe(1);
    expect(lines.find(l => l.includes("vite"))).toMatch(/^ {4}vite :5174 {12}Dev server {2}/);
  });

  it("drops the kind column before the names get narrower than a name", () => {
    const text = activityTable(VIEW, { width: 40, wide: false });
    for (const line of text.split("\n")) expect(line.length, line).toBeLessThanOrEqual(40);
    expect(text).not.toContain("Tool server");
    expect(text).toContain("Claude Code");
  });

  it("--wide lists every process with its pid and whole command line, folding nothing, the run script by its own name", () => {
    const text = activityTable(VIEW, { width: 80, wide: true });
    for (const p of [...turn, ...VIEW.wsp, ...VIEW.unowned, ...VIEW.threads[1]!.processes]) {
      const line = text.split("\n").find(l => new RegExp(`\\s${p.pid}\\s`).test(l));
      expect(line, String(p.pid)).toBeDefined();
      expect(line!.endsWith(`  ${p.command}`), line).toBe(true);
    }
    expect(text.split("\n").find(l => l.includes("tail -n"))).toMatch(/^ {2}tail {2,}69890 /);
    expect(text.split("\n")[2]).toMatch(/PID\s+MEMORY\s+CPU {2}COMMAND$/);
  });

  it("names the run script the agent until the agent's binary shows under it", () => {
    const early: ActivityView = { ...VIEW, threads: [{ ...VIEW.threads[0]!, processes: [turn[0]!, turn[7]!] }], wsp: [], unowned: [] };
    const lines = activityTable(early, { width: 80, wide: false }).split("\n");
    expect(lines.slice(3)).toEqual([expect.stringMatching(/^Fix the lab dashboard {2,}395 MB/), expect.stringMatching(/^ {2}Agent {2,}2 MB {2}0\.0%$/)]);
  });

  it("counts a wide title's characters as two cells and never cuts through one, so no line passes 80 or leaves its column", () => {
    const wideTitles: ActivityView = {
      ...VIEW,
      threads: [
        { ...VIEW.threads[0]!, title: "修复实验室仪表板上的内存图表在窗口缩小时溢出的问题并补上测试用例与说明文档的更新" },
        { ...VIEW.threads[1]!, title: "🚀🔥✅ Ship the lab dashboard 🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪🧪" },
      ],
    };
    const text = activityTable(wideTitles, { width: 80, wide: false });
    expect(text, "a character cut in half").not.toMatch(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/);
    expect(text).not.toContain("\uFFFD");
    const rows = text.split("\n").slice(2);
    for (const line of text.split("\n")) expect(cellWidth(line), line).toBeLessThanOrEqual(80);
    expect(new Set(rows.map(cellWidth)).size, rows.join("\n")).toBe(1);
    expect(rows.find(l => l.startsWith("修复"))).toMatch(/…\s+395 MB {3}0\.4%$/);
    expect(rows.find(l => l.startsWith("🚀"))).toMatch(/…\s+452 MB {2}12\.5%$/);
  });

  it("on a box, whose wsp group has a daemon and no host, heads that group as the page does there", () => {
    const box: ActivityView = { ...VIEW, computer: "lab-box", wsp: [VIEW.wsp[1]!] };
    const lines = activityTable(box, { width: 80, wide: false }).split("\n");
    expect(lines[3]).toMatch(/^wsp {2}What runs wsp here {2,}56 MB {3}1\.1%$/);
    expect(lines[4]).toMatch(/^ {2}Daemon :59312 /);
    expect(activityTable(VIEW, { width: 80, wide: false }).split("\n")[3]).toMatch(/^wsp {2}The app and what runs it /);
  });

  it("says so when nothing runs", () => {
    expect(activityTable({ computer: "lab-mac.local", at: 0, threads: [], wsp: [], unowned: [] }, { width: 80, wide: false })).toBe("no thread runs a process on lab-mac.local");
  });
});

describe("wsp activity's table over an agent outside this host's threads", () => {
  const view = activityOf({ computer: "lab-mac.local", at: OUTSIDE_START, procs: OUTSIDE_PROCS, daemon: 20, host: 10, scopes: [], agentBins: CATALOG_AGENTS.map(a => a.bin), ports: OUTSIDE_PORTS });

  it("names a Claude Code another wsp's thread runs by its agent and its servers as tool servers, under a listening process, as the page does", () => {
    const lines = activityTable(view, { width: 100, wide: false }).split("\n");
    const loose = lines.slice(lines.findIndex(l => l.startsWith(ACTIVITY_GROUP_WORDS.loose)) + 1);
    const rows = loose.map(l => {
      const [, lead, name, tag] = /^( +)(\S+(?: \S+)*?)(?: :\d+)*(?: {2,}(\S+(?: \S+)?))? {2,}[\d.]+ [KMG]?B/.exec(l)!;
      return [lead!.length / 2 - 1, name, tag ?? ""];
    });
    const tagOf = (kind: ActivityProc["kind"]): string => (UNTAGGED_KINDS.has(kind) ? "" : ACTIVITY_KIND_WORDS[kind]);
    expect(rows.sort()).toEqual(OUTSIDE_NO_THREAD.map(([depth, name, kind]) => [depth, name, tagOf(kind)]).sort());
    expect(lines.join("\n")).not.toContain("2.1.296");
    expect(loose.find(l => l.includes("Claude Code"))).toMatch(/^ {4}Claude Code {2,}640 MB {2}3\.0%$/);
  });
});
