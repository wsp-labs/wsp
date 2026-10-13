// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { CATALOG_AGENTS } from "@wsp/catalog";
import { ProcSnapshot, activityOf, agentByBin, endPlan, procName, type ProcEntry } from "@wsp/protocol";
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
  groupsUnder,
  nestGroups,
  pickChoices,
  portRows,
  pushHistory,
  scopeOf,
  sortGroups,
  spent,
  totalSpent,
  type ActivityGroup,
  type ActivityPort,
  type ActivityProc,
  type ActivityThread,
} from "../src/settings/activity/model.js";
import { OUTSIDE_NO_THREAD, OUTSIDE_PORTS, OUTSIDE_PROCS, OUTSIDE_START } from "../../../packages/protocol/test/fixtures/activity-outside.js";

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
  it("puts wsp's parts and what the host started outside a thread first, each thread with what it started, then a terminal with no thread, and the rest apart", () => {
    const s = here();
    expect(shape(s.groups)).toEqual([
      ["wsp", [[10, "wsp", "Host"], [20, "wsp", "Daemon"], [50, "other", "python3 sampler.py"]]],
      ["t1", [[30, "agent", "Claude Code"], [31, "tool", "mcp"], [32, "dev", "vite"], [33, "dev", "esbuild"]]],
      ["t2", [[40, "agent", "Codex"], [41, "test", "vitest"], [42, "test", "forks"]]],
      ["loose", [[21, "terminal", "zsh"]]],
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

  it("puts a browser tab under the thread whose pane holds its page, a frame of it with it, and Graphics and Network under wsp as shared", () => {
    const procs = [...HERE, proc(60, 1, "wsp Helper"), proc(61, 60, "wsp Helper", { rss: 300 * MB }), proc(62, 60, "wsp Helper"), proc(63, 60, "wsp Helper"), proc(64, 60, "wsp Helper"), proc(65, 60, "wsp Helper")];
    const app = new Map([
      [60, { pid: 60, kind: "main" as const }],
      [61, { pid: 61, kind: "tab" as const, origins: ["http://localhost:5199"], contents: [7] }],
      [62, { pid: 62, kind: "frame" as const, origins: ["https://acme.dev"], contents: [7] }],
      [63, { pid: 63, kind: "gpu" as const }],
      [64, { pid: 64, kind: "network" as const }],
      [65, { pid: 65, kind: "tab" as const, origins: ["http://localhost:4000"], contents: [9] }],
    ]);
    const s = scopeOf({ snapshot: snap(procs), threads: THREADS, ports: PORTS, host: 10, app, guests: new Map([[7, "t2"]]) });
    const rows = (id: string) => s.groups.find(g => g.id === id)!.procs.flatMap(flat);
    expect(rows("t2").filter(p => p.pid >= 60).map(p => [p.pid, p.kind, p.name, p.endable === true])).toEqual([
      [61, "tab", "localhost:5199", false],
      [62, "tab", "acme.dev", false],
    ]);
    const wsp = rows("wsp");
    expect(wsp.filter(p => p.pid === 63 || p.pid === 64).map(p => [p.name, p.note])).toEqual([
      ["Graphics", "Shared by the app and its tabs"],
      ["Network", "Shared by the app and its tabs"],
    ]);
    expect(wsp.map(p => p.pid)).toContain(65);
  });

  it("reads the owner a daemon stamps as an object: the thread it names, No thread for wsp's launch on a Mac, and not wsp's for a folder alone", () => {
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
      ["t1", [[60, "slate", "python3 run.py"], [61, "tool", "server"]]],
      ["loose", [[63, "other", "helper"]]],
    ]);
    expect(s.rest.map(p => p.pid)).toEqual([62]);
    expect(s.groups.at(-1)!.procs[0]!.endable).toBeUndefined();
  });

  it("keeps a frame whose row carries a kind or an owner a newer daemon added, reading them as other and no owner", () => {
    const row = proc(60, 1, "python3");
    const frame = (p: Record<string, unknown>) => ProcSnapshot.parse({ type: "proc.snapshot", at: 1, daemon: 20, total: 1, procs: [{ ...row, ...p }], seq: 1 }).procs[0]!;
    expect(frame({ kind: "slateRun" }).kind).toBe("other");
    expect(frame({ owner: "t1" }).owner).toBeUndefined();
    expect(frame({ kind: "slate", owner: { by: "cgroup", thread: "t1", warm: true } })).toMatchObject({ kind: "slate", owner: { by: "cgroup", thread: "t1", warm: true } });
  });

  it("keeps wsp's own parts under wsp where a Mac's daemon stamps them with its coalition", () => {
    const procs = [
      proc(1, 0, "launchd"),
      proc(10, 1, "node", { cmdline: "node wsp host", owner: { by: "coalition" } }),
      proc(20, 10, "wsp-daemon", { owner: { by: "coalition" } }),
      proc(25, 10, "wsp", { cmdline: "/Applications/wsp.app/Contents/Resources/wsp mcp --scoped --guest", owner: { by: "coalition" } }),
    ];
    const s = scopeOf({ snapshot: snap(procs), threads: [], ports: new Map(), host: 10 });
    expect(shape(s.groups)).toEqual([["wsp", [[10, "wsp", "Host"], [20, "wsp", "Daemon"], [25, "other", "wsp"]]]]);
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
    const verbs: ProcVerbs = { ending: new Set(), end: () => {}, free: () => {}, stop: () => {}, openPort: () => {}, openThread: () => {}, copy: () => {}, endAll: () => {} };
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
      ["t1", [[70, "agent", "Claude Code"], [71, "other", "python bot.py"]]],
    ]);
  });

  it("finds a box thread's agent under the heads of its cgroup, and never offers End on it", () => {
    const box = [
      proc(1, 0, "systemd"),
      proc(5, 1, "wsp-daemon"),
      proc(100, 1, "bash", { cmdline: "bash /root/.wsp/run/r1.sh", cgroup: "/wsp-threads/t1", rss: 2 * MB }),
      proc(101, 100, "claude", { cmdline: "claude -p --input-format stream-json", cgroup: "/wsp-threads/t1", rss: 300 * MB }),
      proc(102, 101, "node", { cmdline: "node /opt/wsp/mcp.js", cgroup: "/wsp-threads/t1" }),
      proc(110, 1, "python3", { cmdline: "python3 -m http.server 8110", cgroup: "/wsp-threads/t1" }),
    ];
    const t1 = thread("t1", { cgroup: "/wsp-threads/t1" });
    const s = scopeOf({ snapshot: snap(box, 5), threads: [t1], ports: new Map() });
    const group = s.groups.find(g => g.id === "t1")!;
    expect(group.procs.flatMap(flat).map(p => [p.pid, p.kind, p.name])).toEqual([
      [101, "agent", "Claude Code"],
      [102, "tool", "mcp"],
      [110, "dev", "http.server"],
    ]);
    expect(group.procs[0]!.mem).toBe(302 * MB);
    const verbs: ProcVerbs = { ending: new Set(), end: () => {}, free: () => {}, stop: () => {}, openPort: () => {}, openThread: () => {}, copy: () => {}, endAll: () => {} };
    expect(resolveActions(procActions, { p: group.procs[0]!, group }, verbs).map(a => a.id)).toEqual(["stop", "open-thread", "copy-command", "copy-pid"]);
    expect(resolveActions(procActions, { p: group.procs[1]!, group }, verbs).map(a => a.id)).toContain("end");
  });

  it("takes the native codex as a Codex thread's agent, its npm launcher folded in, and offers End on neither", () => {
    const procs = [
      proc(10, 1, "node", { cmdline: "node wsp host" }),
      proc(20, 10, "wsp-daemon"),
      proc(30, 10, "bash", { cmdline: "bash /root/.wsp/runs/r1.sh", rss: 2 * MB }),
      proc(31, 30, "node", { cmdline: "node /usr/local/bin/codex app-server", rss: 40 * MB }),
      proc(32, 31, "codex", { cmdline: "/usr/local/lib/node_modules/@openai/codex/vendor/x86_64-unknown-linux-musl/bin/codex app-server", rss: 200 * MB }),
      proc(33, 32, "node", { cmdline: "node /opt/wsp/mcp.js" }),
    ];
    const s = scopeOf({ snapshot: snap(procs), threads: [thread("t1", { pid: 30, harness: "codex" })], ports: new Map(), host: 10 });
    const group = s.groups.find(g => g.id === "t1")!;
    expect(group.procs.flatMap(flat).map(p => [p.pid, p.kind, p.name])).toEqual([[32, "agent", "Codex"], [33, "tool", "mcp"]]);
    expect(group.procs[0]!.mem).toBe(242 * MB);
    const verbs: ProcVerbs = { ending: new Set(), end: () => {}, free: () => {}, stop: () => {}, openPort: () => {}, openThread: () => {}, copy: () => {}, endAll: () => {} };
    expect(resolveActions(procActions, { p: group.procs[0]!, group }, verbs).map(a => a.id)).toEqual(["stop", "open-thread", "copy-command", "copy-pid"]);
  });

  it("frees a kept agent between turns here through its thread, and never one on a box, whose end takes the cgroup", () => {
    const verbs: ProcVerbs = { ending: new Set(), end: () => {}, free: () => {}, stop: () => {}, openPort: () => {}, openThread: () => {}, copy: () => {}, endAll: () => {} };
    const kept = [proc(10, 1, "node", { cmdline: "node wsp host" }), proc(20, 10, "wsp-daemon"), proc(30, 10, "claude", { kind: "agent", owner: { by: "root", thread: "t1" } })];
    const here = scopeOf({ snapshot: snap(kept), threads: [thread("t1", { working: false })], ports: new Map(), host: 10 }).groups.find(g => g.id === "t1")!;
    expect(resolveActions(procActions, { p: here.procs[0]!, group: here }, verbs).map(a => a.id)).toEqual(["free", "open-thread", "copy-command", "copy-pid"]);
    const box = [proc(1, 0, "systemd"), proc(5, 1, "wsp-daemon"), proc(70, 1, "claude", { cgroup: "/wsp-threads/t1" })];
    const there = scopeOf({ snapshot: snap(box, 5), threads: [thread("t1", { working: false, cgroup: "/wsp-threads/t1" })], ports: new Map() }).groups.find(g => g.id === "t1")!;
    expect(resolveActions(procActions, { p: there.procs[0]!, group: there }, verbs).map(a => a.id)).toEqual(["open-thread", "copy-command", "copy-pid"]);
  });

  it("carries energy where the computer counts it, and none on one that does not", () => {
    const withEnergy = HERE.map(p => (p.pid === 30 ? { ...p, energyMw: 94 } : p));
    const s = scopeOf({ snapshot: snap(withEnergy), threads: THREADS, ports: PORTS, host: 10 });
    expect(s.energy).toBe(true);
    expect(spent(s.groups[1]!.procs).energy).toBe(94);
    expect(spent(here().groups[1]!.procs).energy).toBeNull();
  });
});

describe("one model for the page, the pane and wsp activity", () => {
  // The review's frame: a turn here with the wsp bridge under its agent and its input pump left under pid 1 in
  // the turn's group; vite on :5199 that the Bash tool detached, and a vitest run; the person's own next dev that only
  // the project's folder places; and a helper only wsp's launch on a Mac places.
  const START = 1_760_000_000_000;
  const frame = [
    proc(1, 0, "launchd"),
    proc(10, 1, "node", { cmdline: "node wsp host" }),
    proc(20, 10, "wsp-daemon"),
    proc(30, 10, "bash", { cmdline: "bash /state/runs/3d518f162d41.sh", pgid: 30, rss: 2 * MB }),
    proc(31, 30, "claude", { cmdline: "claude -p --input-format stream-json", pgid: 30, rss: 500 * MB }),
    proc(32, 31, "node", { cmdline: "node /opt/wsp/mcp.js --thread t1", pgid: 30 }),
    proc(33, 1, "tail", { cmdline: "tail -n +1 -f /state/runs/3d518f162d41.in", pgid: 30, rss: 1 * MB, owner: { by: "group", thread: "t1" } }),
    proc(40, 1, "node", { cmdline: "node /lab/node_modules/.bin/vite --port 5199", pgid: 40, owner: { by: "fork", thread: "t1" } }),
    proc(41, 40, "esbuild", { cmdline: "esbuild --service", pgid: 40, owner: { by: "fork", thread: "t1" } }),
    proc(50, 31, "node", { cmdline: "node /lab/node_modules/.bin/vitest run", pgid: 50 }),
    proc(60, 1, "node", { cmdline: "node /lab/node_modules/.bin/next dev", owner: { by: "folder", workspace: "ws_lab" } }),
    proc(70, 1, "node", { cmdline: "node helper.js", owner: { by: "coalition" } }),
  ].map(p => ({ ...p, startedAt: START + p.pid }));
  const page = scopeOf({ snapshot: snap(frame), threads: [thread("t1", { pid: 30 })], ports: new Map([[40, [{ port: 5199, loopback: true }]]]), host: 10 });
  const read = { computer: "lab", at: START, procs: frame, daemon: 20, host: 10, scopes: [{ threadId: "t1", title: "Thread t1", workspaceId: "ws_lab", roots: [30], bin: "claude" }], agentBins: CATALOG_AGENTS.map(a => a.bin), ports: new Map([[40, [5199]]]) };
  const cli = activityOf(read);
  const rows = (id: string) => page.groups.find(g => g.id === id)!.procs.flatMap(flat);

  it("places every process with one owner and one kind in both", () => {
    const kinds = new Map(cli.threads[0]!.processes.map(p => [p.pid, p.kind]));
    expect(Object.fromEntries(kinds)).toEqual({ 30: "agent", 31: "agent", 32: "tool", 33: "agent", 40: "dev", 41: "dev", 50: "test" });
    expect(rows("t1").map(p => [p.pid, p.kind]).sort((a, b) => Number(a[0]) - Number(b[0]))).toEqual([[31, "agent"], [32, "tool"], [40, "dev"], [41, "dev"], [50, "test"]]);
    // The run script and the pump are the agent's row on the page: its figures hold theirs.
    expect(rows("t1").find(p => p.pid === 31)!.mem).toBe(503 * MB);
    expect([cli.wsp.map(p => p.pid), rows("wsp").map(p => p.pid)]).toEqual([[10, 20], [10, 20]]);
    expect([cli.unowned.map(p => p.pid), rows("loose").map(p => p.pid)]).toEqual([[70], [70]]);
    expect(page.rest.map(p => p.pid)).toEqual([1, 60]);
    expect(JSON.stringify(cli)).not.toContain("next dev");
  });

  it("offers End on the page exactly where wsp activity end would signal", () => {
    const ends = (pid: number): boolean => {
      try {
        return "targets" in endPlan(read, pid, START + pid, () => true);
      } catch {
        return false;
      }
    };
    const offered = [...page.groups.flatMap(g => g.procs.flatMap(flat)), ...page.rest];
    for (const p of offered) expect(p.endable === true, `${p.pid} ${p.name}`).toBe(ends(p.pid));
    expect(offered.filter(p => p.endable === true).map(p => p.pid).sort()).toEqual([32, 40, 41, 50]);
    expect([30, 33].map(ends)).toEqual([false, false]);
  });
});

describe("the frame a real daemon sends", () => {
  // As this computer's daemon stamps it (measured): the turn's run script, which the host names, is the one row with
  // kind agent; the subshell under it and the agent the run recorded carry the owner and no kind. The wsp bridge runs
  // under the agent, and the input pump stands under pid 1 in the script's group.
  const START = 1_760_000_000_000;
  const owner = { by: "root" as const, thread: "t1" };
  const frame = [
    proc(10, 1, "node", { cmdline: "node wsp host" }),
    proc(20, 10, "wsp-daemon"),
    proc(2750, 10, "bash", { cmdline: "bash /state/runs/53af26c40316.sh", pgid: 2750, rss: 2 * MB, owner, kind: "agent" }),
    proc(2751, 2750, "bash", { cmdline: "bash /state/runs/53af26c40316.sh", pgid: 2750, rss: 2 * MB, owner }),
    proc(2756, 2751, "claude", { cmdline: "claude -p --input-format stream-json", pgid: 2750, rss: 500 * MB, owner }),
    proc(2760, 2756, "node", { cmdline: "node /opt/wsp/mcp.js --thread t1", pgid: 2750, rss: 60 * MB, owner }),
    proc(2770, 1, "tail", { cmdline: "tail -n +1 -f /state/runs/53af26c40316.in", pgid: 2750, rss: 1 * MB, owner: { by: "group" as const, thread: "t1" } }),
  ].map(p => ({ ...p, startedAt: START + p.pid }));
  const scope = { threadId: "t1", title: "Thread t1", workspaceId: "ws_lab", bin: "claude" };
  const reads = {
    running: { computer: "lab", at: START, procs: frame, daemon: 20, host: 10, scopes: [{ ...scope, roots: [2750], agents: [2756] }], agentBins: CATALOG_AGENTS.map(a => a.bin) },
    kept: { computer: "lab", at: START, procs: frame, daemon: 20, host: 10, scopes: [{ ...scope, roots: [2750], kept: 2750 }], agentBins: CATALOG_AGENTS.map(a => a.bin) },
  };
  // The page knows a running turn's root and recorded agent; once the turn is over it knows neither.
  const pages = {
    running: scopeOf({ snapshot: snap(frame), threads: [thread("t1", { pid: 2750, agentPid: 2756 })], ports: new Map(), host: 10 }),
    kept: scopeOf({ snapshot: snap(frame), threads: [thread("t1", { working: false })], ports: new Map(), host: 10 }),
  };
  const plan = (read: Parameters<typeof endPlan>[0], pid: number): string => {
    try {
      const p = endPlan(read, pid, START + pid, () => true);
      return "kept" in p ? "kept" : `targets ${p.targets.map(t => t.pid).join(" ")}`;
    } catch (e) {
      return (e as Error).message.split(":")[0]!;
    }
  };

  for (const state of ["running", "kept"] as const) {
    it(`reads the recorded agent under the stamped script as the agent, its turn ${state}, in wsp activity, its End and the page`, () => {
      const kinds = Object.fromEntries(activityOf(reads[state]).threads[0]!.processes.map(p => [p.pid, p.kind]));
      expect(kinds).toEqual({ 2750: "agent", 2751: "agent", 2756: "agent", 2760: "tool", 2770: "agent" });
      const agentSaid = state === "running" ? "2756 is the agent of thread t1 \"Thread t1\", which only its thread's stop ends" : "kept";
      expect([2750, 2751, 2756, 2770].map(pid => plan(reads[state], pid))).toEqual([2750, 2751, 2756, 2770].map(pid => (state === "kept" ? "kept" : agentSaid.replace("2756", String(pid)))));
      expect(plan(reads[state], 2760)).toBe("targets 2760");
      const rows = pages[state].groups.find(g => g.id === "t1")!.procs.flatMap(flat);
      expect(rows.map(p => [p.pid, p.kind, p.name, p.endable === true])).toEqual([
        [2756, "agent", "Claude Code", false],
        [2760, "tool", "mcp", true],
      ]);
      expect(rows[0]!.mem).toBe(505 * MB);
    });
  }
});

describe("totals, sort and the views", () => {
  it("adds a group's figures over its whole tree", () => {
    expect(spent(here().groups[1]!.procs)).toEqual({ mem: (1500 + 90 + 300 + 38) * MB, cpu: 15 + 0.4 + 2.4 + 0.3, energy: null, count: 4 });
  });

  it("sorts a lead on the total its head draws, the threads folded under it included", () => {
    const procs = [proc(10, 1, "node", { cmdline: "node wsp host" }), proc(20, 10, "wsp-daemon"), proc(30, 10, "claude", { rss: 100 * MB }), proc(40, 10, "claude", { rss: 3000 * MB }), proc(50, 10, "claude", { rss: 1000 * MB })];
    const threads = [thread("lead", { pid: 30 }), thread("child", { pid: 40, parent: "lead" }), thread("other", { pid: 50 })];
    const s = scopeOf({ snapshot: snap(procs), threads, ports: new Map(), host: 10 });
    expect(sortGroups(s.groups, "mem").map(g => g.id)).toEqual(["wsp", "lead", "child", "other"]);
  });

  it("adds each group's own figures for a lead on a box, where a thread's memory is its cgroup's and not its rows'", () => {
    const box = [proc(1, 0, "systemd"), proc(5, 1, "wsp-daemon"), proc(70, 1, "claude", { cgroup: "/wsp-threads/lead", rss: 900 * MB }), proc(80, 1, "claude", { cgroup: "/wsp-threads/child", rss: 900 * MB }), proc(90, 1, "claude", { cgroup: "/wsp-threads/other", rss: 900 * MB })];
    const cgroups = [
      { cgroup: "/wsp-threads/lead", memory: 100 * MB },
      { cgroup: "/wsp-threads/child", memory: 300 * MB },
      { cgroup: "/wsp-threads/other", memory: 350 * MB },
    ];
    const threads = [thread("lead", { cgroup: "/wsp-threads/lead" }), thread("child", { cgroup: "/wsp-threads/child", parent: "lead" }), thread("other", { cgroup: "/wsp-threads/other" })];
    const s = scopeOf({ snapshot: { ...snap(box, 5), cgroups }, threads, ports: new Map() });
    const sorted = sortGroups(s.groups, "mem");
    expect(sorted.map(g => g.id)).toEqual(["wsp", "lead", "other", "child"]);
    const { kids } = nestGroups(sorted);
    expect(totalSpent([sorted[1]!, ...groupsUnder(kids, "lead")]).mem).toBe(400 * MB);
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

  it("narrows by the thread and the kind picked, and offers the kinds running and the threads with a group", () => {
    const g = here().groups;
    expect(filterGroups(g, "", { ...NO_PICKS, thread: "t2" }).map(x => x.id)).toEqual(["t2"]);
    expect(shape(filterGroups(g, "", { ...NO_PICKS, kind: "dev" }))).toEqual([["t1", [[30, "agent", "Claude Code"], [32, "dev", "vite"], [33, "dev", "esbuild"]]]]);
    expect(filterGroups(g, "", { thread: "t2", kind: "dev" })).toEqual([]);
    const offered = pickChoices(g, NO_PICKS);
    expect(offered.kinds).toEqual(["agent", "tool", "dev", "test", "terminal", "wsp", "other"]);
    expect(offered.threads).toEqual([{ id: "t1", title: "Thread t1" }, { id: "t2", title: "Thread t2" }]);
    expect(pickChoices(g, { ...NO_PICKS, kind: "tab" }).kinds).toContain("tab");
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

  it("names what a runner fetches and runs by its package, its version left off", () => {
    expect(procName(proc(1, 0, "uv", { cmdline: "/opt/homebrew/bin/uv tool uvx mcp-search-console" }))).toBe("mcp-search-console");
    expect(procName(proc(1, 0, "uv", { cmdline: "/opt/homebrew/bin/uv tool uvx mcp-proxy-for-aws@latest https://example.test/mcp" }))).toBe("mcp-proxy-for-aws");
    expect(procName(proc(1, 0, "npx", { cmdline: "npx -y @acme/lab-mcp@1.2.0" }))).toBe("@acme/lab-mcp");
    expect(procName(proc(1, 0, "pnpm", { cmdline: "pnpm dlx lab-tool" }))).toBe("lab-tool");
    expect(procName(proc(1, 0, "pnpm", { cmdline: "pnpm run dev" }))).toBe("pnpm run");
    expect(procName(proc(1, 0, "uv", { cmdline: "uv sync" }))).toBe("uv sync");
  });

  it("skips a runner's flags and the values they take, and never names a process by a URL or a version", () => {
    expect(procName(proc(1, 0, "uv", { cmdline: "/opt/homebrew/bin/uv tool uvx --python 3.12 mcp-server-time" }))).toBe("mcp-server-time");
    expect(procName(proc(1, 0, "uv", { cmdline: "/opt/homebrew/bin/uv tool uvx --from git+https://github.com/acme/lab-tool lab-tool" }))).toBe("lab-tool");
    expect(procName(proc(1, 0, "uv", { cmdline: "/opt/homebrew/bin/uv tool uvx --from git+https://github.com/acme/lab-tool" }))).toBe("uv tool");
    expect(procName(proc(1, 0, "uv", { cmdline: "uv run --with rich python app.py" }))).toBe("uv run");
    expect(procName(proc(1, 0, "npx", { cmdline: "npx -p @acme/cli acme serve" }))).toBe("acme");
    expect(procName(proc(1, 0, "uvx", { cmdline: "uvx 3.12" }))).toBe("uvx 3.12");
  });

  it("names an agent by the binary its command runs, the one lookup the command line reads too", () => {
    expect(agentByBin(proc(1, 0, "2.1.296", { cmdline: "claude -p --input-format stream-json" }), CATALOG_AGENTS)).toBe("claude");
    expect(agentByBin(proc(1, 0, "node", { cmdline: "node /opt/homebrew/bin/codex exec" }), CATALOG_AGENTS)).toBe("codex");
    expect(agentByBin(proc(1, 0, "bash", { cmdline: "bash /lab/runs/af35cb2fea39.sh" }), CATALOG_AGENTS)).toBeUndefined();
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

describe("an agent outside this host's threads", () => {
  const page = scopeOf({ snapshot: snap(OUTSIDE_PROCS), threads: [], ports: new Map([...OUTSIDE_PORTS].map(([pid, ports]) => [pid, ports.map(port => ({ port, loopback: true }))])), host: 10 });
  const tree = (rows: readonly ActivityProc[], depth = 0): (readonly [number, string, string])[] => rows.flatMap(p => [[depth, p.name, p.kind] as const, ...tree(p.children, depth + 1)]);

  it("names a Claude Code another wsp's thread runs by its agent and its servers as tool servers, as wsp activity does", () => {
    expect(tree(page.groups.find(g => g.id === "loose")!.procs).sort()).toEqual([...OUTSIDE_NO_THREAD].sort());
  });

  it("names the person's own Claude Code by its agent among the rest of the computer, with no End", () => {
    expect(page.rest.filter(p => p.pid === 700).map(p => [p.name, p.kind, p.endable === true])).toEqual([["Claude Code", "agent", false]]);
  });

  it("offers End on an agent no thread of this host holds, and its servers, exactly where wsp activity end would signal", () => {
    const read = { computer: "lab", at: OUTSIDE_START, procs: OUTSIDE_PROCS, daemon: 20, host: 10, scopes: [], agentBins: CATALOG_AGENTS.map(a => a.bin), ports: OUTSIDE_PORTS };
    const ends = (pid: number): boolean => {
      try {
        return "targets" in endPlan(read, pid, OUTSIDE_START + pid, () => true);
      } catch {
        return false;
      }
    };
    const offered = [...page.groups.flatMap(g => g.procs.flatMap(flat)), ...page.rest];
    for (const p of offered) expect(p.endable === true, `${p.pid} ${p.name}`).toBe(ends(p.pid));
    expect(offered.filter(p => p.endable === true).map(p => p.pid).sort()).toEqual([450, 500, 510, 511, 520, 530, 600, 601]);
  });
});
