// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { ProcSnapshot, type ProcEntry } from "@wsp/protocol";
import { resolveActions } from "../src/actions/registry.js";
import { procActions, type ProcVerbs } from "../src/settings/activity/actions.js";
import {
  CPU_HIGH,
  CPU_VERY_HIGH,
  ENERGY_HIGH_MW,
  ENERGY_VERY_HIGH_MW,
  HISTORY_POINTS,
  MEMORY_HIGH,
  MEMORY_SHARE_HIGH,
  MEMORY_SHARE_VERY_HIGH,
  MEMORY_VERY_HIGH,
  NO_PICKS,
  cpuTint,
  energyTint,
  filterGroups,
  flat,
  flatRows,
  kindGroups,
  memoryShareTint,
  memoryTint,
  portRows,
  procName,
  pushHistory,
  scopeOf,
  sortGroups,
  spent,
  type ActivityGroup,
  type ActivityPort,
  type ActivityThread,
} from "../src/settings/activity/model.js";

const MB = 1024 * 1024;
const proc = (pid: number, ppid: number, comm: string, extra: Partial<ProcEntry> = {}): ProcEntry => ({ pid, ppid, user: "me", state: "S", comm, cmdline: comm, cpu: 0, rss: 0, startedAt: 0, ...extra });
const snap = (procs: ProcEntry[], daemon = 20): ProcSnapshot => ({ type: "proc.snapshot", at: 1, daemon, total: procs.length, procs, seq: 1 });
const thread = (id: string, extra: Partial<ActivityThread> = {}): ActivityThread => ({ id, title: `Thread ${id}`, harness: "claude", project: "lab", workspaceId: "ws_lab", sessionId: `s_${id}`, working: true, ...extra });

// The computer the host runs on: the host 10 started the daemon 20 and two agents; the first agent ran a vite whose
// shell is gone (it hangs off the agent by ppid here) and a tool server; the second runs vitest with a worker.
const HERE = [
  proc(1, 0, "launchd"),
  proc(10, 1, "node", { cmdline: "node wsp host", rss: 200 * MB, cpu: 1.5 }),
  proc(20, 10, "wsp-daemon", { rss: 4 * MB, cpu: 0.1 }),
  proc(21, 20, "zsh", { pty: "pty_1", rss: 3 * MB }),
  proc(30, 10, "claude", { rss: 1500 * MB, cpu: 15 }),
  proc(31, 30, "node", { cmdline: "node /opt/wsp/mcp.js --thread t1", rss: 90 * MB, cpu: 0.4 }),
  proc(32, 30, "node", { cmdline: "node node_modules/.bin/vite --port 5173", rss: 300 * MB, cpu: 2.4 }),
  proc(33, 32, "esbuild", { cmdline: "esbuild --service=0.25.0 --ping", rss: 38 * MB, cpu: 0.3 }),
  proc(40, 10, "codex", { rss: 400 * MB }),
  proc(41, 40, "node", { cmdline: "node node_modules/.bin/vitest --watch", rss: 120 * MB, cpu: 0.4 }),
  proc(42, 41, "node", { cmdline: "node vitest/dist/workers/forks.js", rss: 180 * MB }),
  proc(50, 10, "python3", { cmdline: "python3 /tmp/slate/sampler.py", rss: 18 * MB, cpu: 0.6 }),
  proc(90, 1, "Instagram", { rss: 2300 * MB, cpu: 22 }),
];
const THREADS = [thread("t1", { pid: 30 }), thread("t2", { pid: 40, harness: "codex", working: false, project: "acme" })];
const PORTS = new Map<number, ActivityPort[]>([[32, [{ port: 5173, loopback: true }]], [10, [{ port: 7420, loopback: true }]]]);
const here = () => scopeOf({ snapshot: snap(HERE), threads: THREADS, ports: PORTS, host: 10 });
const shape = (groups: readonly ActivityGroup[]) => groups.map(g => [g.id, g.procs.flatMap(flat).map(p => [p.pid, p.kind, p.name])]);

describe("scopeOf", () => {
  it("puts wsp's parts first, each thread with what it started, then what wsp started with no thread, and the rest apart", () => {
    const s = here();
    expect(shape(s.groups)).toEqual([
      ["wsp", [[10, "wsp", "Host"], [20, "wsp", "Daemon"]]],
      ["t1", [[30, "agent", "Claude Code"], [31, "tool", "mcp"], [32, "dev", "vite"], [33, "dev", "esbuild"]]],
      ["t2", [[40, "agent", "Codex"], [41, "test", "vitest"], [42, "test", "forks"]]],
      ["loose", [[21, "terminal", "zsh"], [50, "other", "python3 sampler.py"]]],
    ]);
    expect(s.rest.map(p => p.pid)).toEqual([1, 90]);
    expect(s.groups[1]!.procs[0]!.harness).toBe("claude");
    expect(s.groups[0]!.procs[0]!.needed).toBe(true);
    expect(s.groups[1]!.procs[0]!.children.map(c => c.pid)).toEqual([31, 32]);
    expect(s.energy).toBe(false);
  });

  it("reads the agent as the process running its binary, the host's run script folded into it, and no zombie", () => {
    const procs = [
      proc(10, 1, "node", { cmdline: "node wsp host" }),
      proc(20, 10, "wsp-daemon"),
      proc(30, 10, "bash", { cmdline: "bash /state/runs/caf9.sh", rss: 2 * MB, cpu: 0.1 }),
      proc(31, 30, "claude", { cmdline: "claude -p --input-format stream-json", rss: 300 * MB, cpu: 4 }),
      proc(32, 31, "bash", { state: "Z", cmdline: "" }),
      proc(33, 31, "node", { cmdline: "/usr/local/bin/node /opt/wsp/packages/host/dist/bin.js mcp --scoped", rss: 60 * MB }),
      proc(34, 31, "zsh", { cmdline: "/usr/bin/zsh -c sleep 600" }),
      proc(35, 34, "sleep", { cmdline: "sleep 600" }),
    ];
    const s = scopeOf({ snapshot: snap(procs), threads: [thread("t1", { pid: 30 })], ports: new Map(), host: 10 });
    const t1 = s.groups.find(g => g.id === "t1")!;
    expect(t1.procs.map(p => [p.pid, p.kind, p.name, p.mem, p.cpu])).toEqual([[31, "agent", "Claude Code", 302 * MB, 4.1]]);
    expect(t1.procs[0]!.children.map(c => [c.pid, c.kind])).toEqual([
      [33, "tool"],
      [34, "other"],
    ]);
    expect(spent(t1.procs).count).toBe(4);
  });

  it("folds a run script that forked a subshell before the agent", () => {
    const procs = [
      proc(10, 1, "node", { cmdline: "node wsp host" }),
      proc(20, 10, "wsp-daemon"),
      proc(30, 10, "bash", { cmdline: "bash /state/runs/8807.sh", rss: 2 * MB }),
      proc(31, 30, "bash", { cmdline: "bash /state/runs/8807.sh", rss: 2 * MB }),
      proc(32, 31, "claude", { cmdline: "claude -p", rss: 130 * MB }),
      proc(33, 32, "zsh", { cmdline: "zsh -c sleep 110" }),
    ];
    const s = scopeOf({ snapshot: snap(procs), threads: [thread("t1", { pid: 30 })], ports: new Map(), host: 10 });
    const t1 = s.groups.find(g => g.id === "t1")!;
    expect(t1.procs.map(p => [p.pid, p.kind, p.name, p.mem])).toEqual([[32, "agent", "Claude Code", 134 * MB]]);
    expect(t1.procs[0]!.children.map(c => c.pid)).toEqual([33]);
  });

  it("takes the thread and kind the daemon stamped, and the app's own names for its parts", () => {
    const procs = [...HERE.filter(p => p.pid !== 50), proc(50, 10, "python3", { cmdline: "python3 /tmp/slate/sampler.py", owner: { by: "root", thread: "t2" }, kind: "slate" }), proc(60, 1, "wsp Helper"), proc(61, 60, "wsp Helper"), proc(62, 60, "wsp Helper", { owner: { by: "fork", thread: "t1" } })];
    const app = new Map([
      [60, { pid: 60, kind: "renderer" as const }],
      [61, { pid: 61, kind: "tab" as const, origins: ["http://localhost:5173", "https://acme.dev"] }],
      [62, { pid: 62, kind: "tab" as const, origins: ["http://localhost:5174"] }],
    ]);
    const s = scopeOf({ snapshot: snap(procs), threads: THREADS, ports: PORTS, host: 10, app });
    expect(shape(s.groups.filter(g => g.id === "t2" || g.id === "wsp"))).toEqual([
      ["wsp", [[10, "wsp", "Host"], [20, "wsp", "Daemon"], [60, "app", "Window"], [61, "tab", "localhost:5173 +1"]]],
      ["t2", [[40, "agent", "Codex"], [41, "test", "vitest"], [42, "test", "forks"], [50, "slate", "python3 sampler.py"]]],
    ]);
    const t1 = s.groups.find(g => g.id === "t1")!.procs.flatMap(flat).at(-1)!;
    expect([t1.pid, t1.kind, t1.name]).toEqual([62, "tab", "localhost:5174"]);
    expect(shape(s.groups.filter(g => g.id === "loose"))).toEqual([["loose", [[21, "terminal", "zsh"]]]]);
  });

  it("reads the owner a daemon stamps as an object: the thread it names, No thread for one that names none", () => {
    const owned = [
      proc(10, 1, "node", { cmdline: "node wsp host" }),
      proc(20, 10, "wsp-daemon"),
      proc(60, 10, "python3", { cmdline: "python3 run.py", owner: { by: "root", thread: "t1", workspace: "ws_lab" }, kind: "slate" }),
      proc(61, 10, "node", { cmdline: "node /opt/mcp/server.js", owner: { by: "group", thread: "t1" }, kind: "tool" }),
      proc(62, 1, "node", { cmdline: "node dev.js", owner: { by: "folder", workspace: "ws_lab" } }),
      proc(63, 1, "node", { cmdline: "node helper.js", owner: { by: "coalition" } }),
    ];
    const frame = { type: "proc.snapshot", at: 1, daemon: 20, total: owned.length, procs: owned, seq: 1 };
    expect(ProcSnapshot.safeParse(frame).success).toBe(true);
    const s = scopeOf({ snapshot: ProcSnapshot.parse(frame), threads: [thread("t1")], ports: new Map(), host: 10 });
    expect(shape(s.groups)).toEqual([
      ["wsp", [[10, "wsp", "Host"], [20, "wsp", "Daemon"]]],
      ["t1", [[61, "tool", "server"], [60, "slate", "python3 run.py"]]],
      ["loose", [[62, "other", "dev"], [63, "other", "helper"]]],
    ]);
  });

  it("refuses a kind outside the eleven words and an owner that is not the object", () => {
    const row = proc(60, 1, "python3");
    const frame = (p: Record<string, unknown>) => ({ type: "proc.snapshot", at: 1, daemon: 20, total: 1, procs: [{ ...row, ...p }], seq: 1 });
    expect(ProcSnapshot.safeParse(frame({ kind: "slateRun" })).success).toBe(false);
    expect(ProcSnapshot.safeParse(frame({ owner: "t1" })).success).toBe(false);
    expect(ProcSnapshot.safeParse(frame({ kind: "slate", owner: { by: "cgroup", thread: "t1" } })).success).toBe(true);
  });

  it("finds the agent by argv[0] where a Mac names it by its file, and never offers End on it", () => {
    const procs = [
      proc(10, 1, "node", { cmdline: "node wsp host" }),
      proc(20, 10, "wsp-daemon"),
      proc(30, 10, "bash", { cmdline: "bash /state/runs/r1.sh" }),
      proc(31, 30, "2.1.296", { cmdline: "/Users/dev/.local/bin/claude -p --input-format stream-json" }),
      proc(32, 31, "node", { cmdline: "node /opt/wsp/mcp.js" }),
      proc(40, 10, "bash", { cmdline: "bash /state/runs/r2.sh" }),
      proc(41, 40, "node", { cmdline: "node /Users/dev/.local/bin/cursor-agent -p" }),
    ];
    const s = scopeOf({ snapshot: snap(procs), threads: [thread("t1", { pid: 30 }), thread("t2", { pid: 40, harness: "cursor" })], ports: new Map(), host: 10 });
    const tree = (p: { pid: number; name: string; kind: string; children: readonly unknown[] }): unknown[] => [p.pid, p.name, p.kind, ...(p.children.length === 0 ? [] : [(p.children as (typeof p)[]).map(tree)])];
    expect(s.groups.find(g => g.id === "t1")!.procs.map(tree)).toEqual([[31, "Claude Code", "agent", [[32, "mcp", "tool"]]]]);
    expect(s.groups.find(g => g.id === "t2")!.procs.map(p => [p.pid, p.kind])).toEqual([[41, "agent"]]);
    const t1 = s.groups.find(g => g.id === "t1")!;
    const verbs: ProcVerbs = { ending: new Set(), end: () => {}, stop: () => {}, openPort: () => {}, openThread: () => {}, copy: () => {} };
    expect(resolveActions(procActions, { p: t1.procs[0]!, group: t1 }, verbs).map(a => a.id)).toEqual(["stop", "open-thread", "copy-command", "copy-pid"]);
    expect(resolveActions(procActions, { p: t1.procs[0]!.children[0]!, group: t1 }, verbs).map(a => a.id)).toContain("end");
  });

  it("draws a thread's slate as one heading beside its agent, its runs under it, counted once", () => {
    const procs = [
      proc(10, 1, "node", { cmdline: "node wsp host" }),
      proc(20, 10, "wsp-daemon"),
      proc(30, 10, "claude", { rss: 100 * MB }),
      proc(50, 10, "python3", { cmdline: "python3 /Users/dev/.wsp/slates/t1/sampler.py", rss: 18 * MB, cpu: 0.5 }),
      proc(51, 10, "bash", { cmdline: "bash -c curl -s https://acme.dev", owner: { by: "root", thread: "t1" }, kind: "slate", rss: 2 * MB }),
      proc(52, 10, "python3", { cmdline: "python3 /Users/dev/.wsp/slates/t9/x.py" }),
    ];
    const s = scopeOf({ snapshot: snap(procs), threads: [thread("t1", { pid: 30, slate: "Gold price" })], ports: new Map(), host: 10 });
    const t1 = s.groups.find(g => g.id === "t1")!;
    expect(t1.procs.map(p => [p.pid, p.kind, p.name, p.heading === true, p.mem, p.children.map(c => c.pid)])).toEqual([
      [30, "agent", "Claude Code", false, 100 * MB, []],
      [-1, "slate", "Gold price", true, 20 * MB, [50, 51]],
    ]);
    expect(spent(t1.procs)).toMatchObject({ mem: 120 * MB, count: 3 });
    expect(flatRows(s.groups, s.rest, false, "mem", "").map(r => r.p.pid)).not.toContain(-1);
    expect(shape(s.groups.filter(g => g.id === "loose"))).toEqual([["loose", [[52, "slate", "python3 x.py"]]]]);
  });

  it("reads a box's threads by their cgroup, a server the thread detached included", () => {
    const box = [
      proc(1, 0, "systemd"),
      proc(5, 1, "wsp-daemon", { rss: 11 * MB }),
      proc(70, 5, "claude", { cgroup: "/wsp-threads/t1" }),
      proc(71, 1, "python", { cmdline: "python bot.py", cgroup: "/wsp-threads/t1" }),
      proc(72, 1, "pg_upgrade", { cgroup: "/system.slice/x" }),
    ];
    const s = scopeOf({ snapshot: snap(box, 5), threads: [thread("t1", { cgroup: "/wsp-threads/t1" })], ports: new Map() });
    expect(shape(s.groups)).toEqual([
      ["wsp", [[5, "wsp", "Daemon"]]],
      ["t1", [[70, "other", "claude"], [71, "other", "python bot.py"]]],
    ]);
  });

  it("carries energy where the computer counts it, and none on one that does not", () => {
    const withEnergy = HERE.map(p => (p.pid === 30 ? { ...p, energyMw: 94 } : p));
    const s = scopeOf({ snapshot: snap(withEnergy), threads: THREADS, ports: PORTS, host: 10 });
    expect(s.energy).toBe(true);
    expect(spent(s.groups[1]!.procs).energy).toBe(94);
    expect(spent(here().groups[1]!.procs).energy).toBeNull();
  });
});

describe("totals, sort and the views", () => {
  it("adds a group's figures over its whole tree", () => {
    expect(spent(here().groups[1]!.procs)).toEqual({ mem: (1500 + 90 + 300 + 38) * MB, cpu: 15 + 0.4 + 2.4 + 0.3, energy: null, count: 4 });
  });

  it("keeps wsp first and No thread last, the threads heaviest first on the column, and siblings sorted", () => {
    const byCpu = sortGroups(here().groups, "cpu");
    expect(byCpu.map(g => g.id)).toEqual(["wsp", "t1", "t2", "loose"]);
    expect(byCpu[1]!.procs[0]!.children.map(c => c.pid)).toEqual([32, 31]);
    const lighter = here().groups.map(g => (g.id === "t1" ? { ...g, procs: g.procs.map(p => ({ ...p, mem: 1, children: [] })) } : g));
    expect(sortGroups(lighter, "mem").map(g => g.id)).toEqual(["wsp", "t2", "t1", "loose"]);
  });

  it("filters incrementally by name, port, kind word or the thread's title, a match standing under its parents", () => {
    const g = here().groups;
    expect(shape(filterGroups(g, "5173", NO_PICKS))).toEqual([["t1", [[30, "agent", "Claude Code"], [32, "dev", "vite"]]]]);
    expect(shape(filterGroups(g, "test run", NO_PICKS)).map(x => x[0])).toEqual(["t2"]);
    expect(filterGroups(g, "Thread t2", NO_PICKS).map(x => x.id)).toEqual(["t2"]);
    expect(filterGroups(g, "nothing like it", NO_PICKS)).toEqual([]);
  });

  it("narrows by the thread, kind and project chips", () => {
    const g = here().groups;
    expect(filterGroups(g, "", { ...NO_PICKS, threads: ["t2"] }).map(x => x.id)).toEqual(["t2"]);
    expect(filterGroups(g, "", { ...NO_PICKS, projects: ["acme"] }).map(x => x.id)).toEqual(["t2"]);
    expect(shape(filterGroups(g, "", { ...NO_PICKS, kinds: ["dev"] }))).toEqual([["t1", [[30, "agent", "Claude Code"], [32, "dev", "vite"], [33, "dev", "esbuild"]]]]);
  });

  it("lists every owned process flat, and the rest of the computer only with Every process", () => {
    const s = here();
    expect(flatRows(s.groups, s.rest, false, "mem", "").map(r => r.p.pid).slice(0, 3)).toEqual([30, 40, 32]);
    expect(flatRows(s.groups, s.rest, true, "mem", "").map(r => r.p.pid).slice(0, 2)).toEqual([90, 30]);
    expect(flatRows(s.groups, s.rest, true, "mem", "insta").filter(r => r.group === null).map(r => r.p.pid)).toEqual([90]);
  });

  it("groups by kind in one order and lists ports lowest first with their thread", () => {
    const s = here();
    expect(kindGroups(s.groups, "mem").map(k => [k.kind, k.rows.map(r => r.p.pid)])).toEqual([
      ["agent", [30, 40]],
      ["tool", [31]],
      ["dev", [32, 33]],
      ["test", [42, 41]],
      ["terminal", [21]],
      ["wsp", [10, 20]],
      ["other", [50]],
    ]);
    expect(portRows(s.groups).map(r => [r.port.port, r.p.pid, r.group.id])).toEqual([
      [5173, 32, "t1"],
      [7420, 10, "wsp"],
    ]);
  });

  it("names a process by the script a runtime runs, and by its own name otherwise", () => {
    expect(procName(proc(1, 0, "node", { cmdline: "node node_modules/.bin/next dev" }))).toBe("next");
    expect(procName(proc(1, 0, "python3", { cmdline: "python3 -u bot.py" }))).toBe("python3 bot.py");
    expect(procName(proc(1, 0, "rustc", { cmdline: "rustc --crate-name x" }))).toBe("rustc");
    expect(procName(proc(1, 0, "cargo", { cmdline: "cargo test -p wsp-daemon" }))).toBe("cargo test");
    expect(procName(proc(1, 0, "node", { cmdline: "node" }))).toBe("node");
    expect(procName(proc(1, 0, "node", { cmdline: "/usr/local/bin/node /opt/wsp/packages/host/dist/bin.js mcp --scoped" }))).toBe("host mcp");
    expect(procName(proc(1, 0, "node", { cmdline: "node /home/dev/.npm/_npx/playwright-mcp/cli.js" }))).toBe("playwright-mcp");
  });
});

describe("tints", () => {
  it("inks a figure only past its high mark, warn then danger, neutral under it", () => {
    expect([memoryTint(MEMORY_HIGH), memoryTint(MEMORY_HIGH + 1), memoryTint(MEMORY_VERY_HIGH), memoryTint(MEMORY_VERY_HIGH + 1)]).toEqual([null, "warn", "warn", "danger"]);
    expect([cpuTint(CPU_HIGH), cpuTint(CPU_HIGH + 0.1), cpuTint(CPU_VERY_HIGH + 0.1), cpuTint(3)]).toEqual([null, "warn", "danger", null]);
    expect([energyTint(null), energyTint(ENERGY_HIGH_MW), energyTint(ENERGY_HIGH_MW + 1), energyTint(ENERGY_VERY_HIGH_MW + 1)]).toEqual([null, null, "warn", "danger"]);
    const total = 1000;
    expect([memoryShareTint(MEMORY_SHARE_HIGH * total, total), memoryShareTint(MEMORY_SHARE_HIGH * total + 1, total), memoryShareTint(MEMORY_SHARE_VERY_HIGH * total + 1, total), memoryShareTint(999, null)]).toEqual([null, "warn", "danger", null]);
  });
});

describe("pushHistory", () => {
  it("adds one point per frame, holds a frame once and keeps the last five minutes", () => {
    let h = pushHistory([], { at: 1, mem: 1, cpu: 1, energy: null });
    expect(pushHistory(h, { at: 1, mem: 2, cpu: 2, energy: null })).toBe(h);
    for (let at = 2; at <= HISTORY_POINTS + 10; at++) h = pushHistory(h, { at, mem: at, cpu: at, energy: null });
    expect(h.length).toBe(HISTORY_POINTS);
    expect(h[0]!.at).toBe(11);
  });
});
