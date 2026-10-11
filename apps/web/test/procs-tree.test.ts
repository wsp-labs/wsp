// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import type { ProcEntry } from "@wsp/protocol";
import { compactBytes } from "@wsp/protocol";
import type { AppProc } from "@wsp/protocol";
import { agentRows, procLabel, procTable, runAgentPids, type ProcThread } from "../src/components/procs/tree.js";

const proc = (pid: number, ppid: number, comm: string, extra: Partial<ProcEntry> = {}): ProcEntry => ({
  pid,
  ppid,
  user: "root",
  state: "S",
  comm,
  cmdline: comm,
  cpu: 0,
  rss: 0,
  startedAt: 0,
  ...extra,
});

const procs: ProcEntry[] = [
  proc(1, 0, "init"),
  proc(40, 1, "node", { cpu: 3, rss: 300, cmdline: "node daemon.js" }),
  proc(41, 40, "bash", { cpu: 1, rss: 50, pty: "pty_1" }),
  proc(42, 40, "claude", { cpu: 30, rss: 900 }),
  proc(50, 1, "nginx", { cpu: 2, rss: 400 }),
  // An orphan whose parent was cut by the daemon's cap roots itself.
  proc(77, 9999, "cron", { cpu: 0, rss: 10 }),
];

const flat = (table: ReturnType<typeof procTable>) => table.rest.map(r => [r.proc.pid, r.depth]);

describe("procTable", () => {
  it("nests by ppid with siblings sorted by cpu, ties by pid, orphans at the root", () => {
    expect(flat(procTable(procs, "cpu", ""))).toEqual([
      [1, 0],
      [40, 1],
      [42, 2],
      [41, 2],
      [50, 1],
      [77, 0],
    ]);
  });

  it("draws a thread known by its cgroup over every process standing there, a server it detached included, with what they spend together", () => {
    const thread: ProcThread = { threadId: "t1", title: "serve it", cgroup: "/wsp-threads/t1" };
    const box: ProcEntry[] = [
      proc(1, 0, "systemd"),
      // The turn: its shell and the agent under it.
      proc(200, 1, "bash", { cgroup: "/wsp-threads/t1", cpu: 1, rss: 100 }),
      proc(201, 200, "claude", { cgroup: "/wsp-threads/t1", cpu: 20, rss: 600 }),
      // A dev server the turn detached: init's child now, still in the thread's cgroup.
      proc(300, 1, "node", { cgroup: "/wsp-threads/t1", cpu: 5, rss: 300, cmdline: "node server.js" }),
      // Another thread whose id begins the same way, and the computer's own.
      proc(400, 1, "node", { cgroup: "/wsp-threads/t10", cpu: 9, rss: 900 }),
      proc(500, 1, "sshd", { cgroup: "/system.slice/ssh.service" }),
    ];
    const table = procTable(box, "cpu", "", [thread]);
    expect(table.threads).toHaveLength(1);
    // Each process under the first of its parents standing there too, heads sorted by the column as siblings are.
    expect(table.threads[0]!.rows.map(r => [r.proc.pid, r.depth])).toEqual([
      [300, 0],
      [200, 0],
      [201, 1],
    ]);
    expect(table.threads[0]!.spent).toEqual({ cpu: 26, rss: 1000 });
    // Everything else on the computer is the rest, the other thread's server among it.
    expect(table.rest.map(r => r.proc.pid)).toEqual([1, 400, 500]);
    // A filter hides rows, never what the thread spends.
    expect(procTable(box, "cpu", "server", [thread]).threads[0]!.spent).toEqual({ cpu: 26, rss: 1000 });
  });

  it("sorting by mem reorders siblings, not the tree", () => {
    expect(procTable(procs, "mem", "").rest.map(r => r.proc.pid)).toEqual([77, 1, 50, 40, 42, 41]);
  });

  it("a filter keeps every match under its own parents, matching pid, comm, cmdline and user", () => {
    // nginx matches; init is its parent and stays above it, at the depth it always had.
    expect(flat(procTable(procs, "cpu", "ngin"))).toEqual([
      [1, 0],
      [50, 1],
    ]);
    expect(flat(procTable(procs, "cpu", "42"))).toEqual([
      [1, 0],
      [40, 1],
      [42, 2],
    ]);
    expect(procTable(procs, "mem", "DAEMON.JS").rest.map(r => r.proc.pid)).toEqual([1, 40]);
    expect(procTable(procs, "cpu", "root").rest.map(r => r.proc.pid)).toEqual([1, 40, 42, 41, 50, 77]);
    expect(procTable(procs, "cpu", "nothing here").rest).toEqual([]);
  });
});

describe("procTable with this workspace's threads", () => {
  // Two threads of this workspace, each the shell the host started for a turn, with the agent and its tools under it.
  const own: ProcEntry[] = [
    ...procs,
    proc(100, 40, "bash", { cpu: 1, rss: 20, cmdline: "bash -c claude -p docs" }),
    proc(101, 100, "claude", { cpu: 40, rss: 800, cmdline: "claude --settings {} -p docs" }),
    proc(102, 101, "rg", { cpu: 5, rss: 30, cmdline: "rg needle" }),
    proc(200, 40, "bash", { cpu: 1, rss: 20, cmdline: "bash -c claude -p tests" }),
    proc(201, 200, "claude", { cpu: 10, rss: 700, cmdline: "claude --settings {} -p tests" }),
  ];
  const threads: ProcThread[] = [
    { threadId: "th_docs", title: "the docs agent", pid: 100 },
    { threadId: "th_tests", title: "the tests agent", pid: 200 },
  ];

  it("gives each thread its own tree, nested, and leaves those processes out of the rest", () => {
    const table = procTable(own, "cpu", "", threads);
    expect(table.threads.map(t => t.thread.title)).toEqual(["the docs agent", "the tests agent"]);
    expect(table.threads[0]!.rows.map(r => [r.proc.pid, r.depth])).toEqual([
      [100, 0],
      [101, 1],
      [102, 2],
    ]);
    expect(table.threads[1]!.rows.map(r => [r.proc.pid, r.depth])).toEqual([
      [200, 0],
      [201, 1],
    ]);
    // The rest is the computer without those two trees; nothing is counted twice.
    expect(table.rest.map(r => r.proc.pid)).toEqual([1, 40, 42, 41, 50, 77]);
  });

  it("a thread whose process is not in the snapshot has no tree", () => {
    const table = procTable(own.filter(p => p.pid !== 200 && p.pid !== 201), "cpu", "", threads);
    expect(table.threads.map(t => t.thread.threadId)).toEqual(["th_docs"]);
  });

  it("the filter reads the threads' trees too, keeping matches under their parents and dropping a thread with none", () => {
    const table = procTable(own, "cpu", "docs", threads);
    expect(table.threads.map(t => t.thread.threadId)).toEqual(["th_docs"]);
    expect(table.threads[0]!.rows.map(r => r.proc.pid)).toEqual([100, 101]);
    expect(table.rest).toEqual([]);
  });
});

describe("procLabel", () => {
  const titles = new Map([["pty_1", "zsh"]]);
  it("names the daemon, a pty shell by its tab and the harness by its comm", () => {
    expect(procLabel(procs[1]!, 40, titles)).toBe("wsp");
    expect(procLabel(procs[2]!, 40, titles)).toBe("terminal zsh");
    expect(procLabel(procs[2]!, 40, new Map())).toBe("terminal");
    expect(procLabel(procs[3]!, 40, titles)).toBe("agent");
    expect(procLabel(procs[4]!, 40, titles)).toBeNull();
    // Any catalog agent is named by the command its entry puts on PATH.
    expect(procLabel(proc(43, 40, "codex"), 40, titles)).toBe("agent");
  });

  it("names the desktop app's processes by what the app said each one is", () => {
    const app = new Map<number, AppProc>([
      [70, { pid: 70, kind: "main" }],
      [71, { pid: 71, kind: "gpu" }],
      [72, { pid: 72, kind: "network" }],
      [73, { pid: 73, kind: "renderer" }],
      [74, { pid: 74, kind: "tab", origins: ["https://github.com"] }],
      [75, { pid: 75, kind: "frame", origins: ["https://ads.example.com", "https://cdn.example.com"] }],
    ]);
    const named = [70, 71, 72, 73, 74, 75].map(pid => procLabel(proc(pid, 1, "electron"), 40, titles, app));
    expect(named).toEqual(["wsp app", "wsp graphics", "wsp network", "wsp window", "browser tab github.com", "frame ads.example.com +1"]);
    expect(procLabel(proc(76, 1, "electron"), 40, titles, app)).toBeNull();
  });
});

describe("compactBytes", () => {
  it("one unit, no more than three significant digits", () => {
    expect(compactBytes(0)).toBe("0");
    expect(compactBytes(900)).toBe("900");
    expect(compactBytes(12 * 1024)).toBe("12K");
    expect(compactBytes(1.5 * 1024 ** 2)).toBe("1.5M");
    expect(compactBytes(123.4 * 1024 ** 2)).toBe("123M");
    expect(compactBytes(2.25 * 1024 ** 3)).toBe("2.3G");
  });
});

describe("agentRows", () => {
  // A local turn as the host launches it: the run's bash leads, a subshell runs the agent, the agent runs a tool
  // server and a dev server.
  const turn: ProcEntry[] = [
    proc(1, 0, "init"),
    proc(300, 1, "bash", { cmdline: "bash run.sh" }),
    proc(301, 300, "bash"),
    proc(302, 301, "claude"),
    proc(303, 302, "node", { cmdline: "node mcp-server.js" }),
    proc(304, 302, "vite"),
    proc(400, 1, "claude", { cmdline: "claude in a terminal of the person's own" }),
  ];

  it("names the turn's root and every process down to the agent, never a tool server, another claude or a stopped thread", () => {
    const running: ProcThread = { threadId: "th_a", title: "a", pid: 300, sessionId: "s_a" };
    expect([...agentRows(turn, [running]).keys()].sort()).toEqual([300, 301, 302]);
    expect(agentRows(turn, [{ threadId: "th_a", title: "a", pid: 300 }]).size).toBe(0);
  });

  it("takes the agent the run recorded whatever its name, and then no name under it, a claude the agent ran included", () => {
    // A native Claude Code runs as its version; the turn's run recorded its pid. The agent ran a claude of its own.
    const native = [...turn.map(p => (p.pid === 302 ? { ...p, comm: "2.1.296", cmdline: "claude -p --input-format stream-json" } : p)), proc(305, 302, "claude", { cmdline: "claude -p hi" })];
    const thread: ProcThread = { threadId: "th_a", title: "a", pid: 300, sessionId: "s_a", agentPid: 302 };
    expect([...agentRows(native, [thread]).keys()].sort()).toEqual([300, 301, 302]);
    // A recorded pid that is not under the turn's root names nothing, and the names stand in.
    expect([...agentRows(native, [{ ...thread, agentPid: 400 }]).keys()].sort()).toEqual([300, 301, 302, 305]);
  });

  it("takes the agent binary a recorded launcher starts, npm's codex under node, and what the agent starts stays out", () => {
    const npm = [
      proc(1, 0, "init"),
      proc(300, 1, "bash", { cmdline: "bash run.sh" }),
      proc(301, 300, "bash"),
      proc(302, 301, "node", { cmdline: "node /usr/lib/node_modules/@openai/codex/bin/codex.js app-server" }),
      proc(303, 302, "codex", { cmdline: "/usr/lib/node_modules/@openai/codex-linux-x64/vendor/x86_64-unknown-linux-musl/bin/codex app-server" }),
      proc(304, 303, "node", { cmdline: "node mcp-server.js" }),
    ];
    const thread: ProcThread = { threadId: "th_a", title: "a", pid: 300, sessionId: "s_a", agentPid: 302 };
    expect([...agentRows(npm, [thread]).keys()].sort()).toEqual([300, 301, 302, 303]);
  });

  it("on a computer whose turns stand in no cgroup, names every process from a run script down to its agent, and no agent's own child", () => {
    const box = [
      proc(1, 0, "init"),
      proc(500, 1, "bash", { cmdline: "bash /root/.wsp/run/0123456789ab.sh" }),
      proc(501, 500, "bash", { cmdline: "bash /root/.wsp/run/0123456789ab.sh" }),
      proc(502, 501, "2.1.296", { cmdline: "claude -p --input-format stream-json" }),
      proc(503, 502, "node", { cmdline: "node mcp-server.js" }),
      proc(504, 502, "claude", { cmdline: "claude -p hi" }),
      proc(600, 1, "claude", { cmdline: "claude in a terminal of the person's own" }),
    ];
    expect([...runAgentPids(box)].sort()).toEqual([500, 501, 502]);
  });

  it("where no agent was recorded, knows one by argv[0]'s basename beside its accounting name", () => {
    // The native install through the link ~/.local/bin/claude; cursor-agent's exec -a "$0" node.
    const running: ProcThread = { threadId: "th_a", title: "a", pid: 300, sessionId: "s_a" };
    const native = turn.map(p => (p.pid === 302 ? { ...p, comm: "2.1.296", cmdline: "/Users/a/.local/bin/claude -p --verbose" } : p));
    expect([...agentRows(native, [running]).keys()].sort()).toEqual([300, 301, 302]);
    const cursor = turn.map(p => (p.pid === 302 ? { ...p, comm: "node", cmdline: "/opt/homebrew/bin/cursor-agent -p --output-format stream-json" } : p));
    expect([...agentRows(cursor, [running]).keys()].sort()).toEqual([300, 301, 302]);
    expect(procLabel(native.find(p => p.pid === 302)!, 0, new Map())).toBe("agent");
  });

  it("on a computer the person joined names the agent binaries in a running thread's cgroup and the run's processes above them there", () => {
    // A box run records no agent: its bash run.sh (300) and shell (301) run the agent (302) in the thread's cgroup.
    const boxed = turn.map(p => (p.pid >= 300 && p.pid < 400 ? { ...p, cgroup: "/wsp-threads/th_b" } : p));
    const thread: ProcThread = { threadId: "th_b", title: "b", cgroup: "/wsp-threads/th_b", sessionId: "s_b" };
    expect([...agentRows(boxed, [thread]).keys()].sort()).toEqual([300, 301, 302]);
  });
});
