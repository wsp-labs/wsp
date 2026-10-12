// SPDX-License-Identifier: AGPL-3.0-only
// Activity's reading of one snapshot: a thread's tree from its roots and the daemon's owners, the stretch from a root
// to the agent's binary, and the rules an End meets before anything is signalled.
import { describe, expect, it } from "vitest";
import { activityOf, agentEndLine, agentUnderLine, endGoneLine, endPlan, endReusedLine, notStartedLine, notThreadsLine, processesLeft, processesLeftLine, wspOwnLine, type ActivityRead, type ActivityScope, type ProcEntry } from "../src/index.js";

const START = 1_760_000_000_000;
const row = (pid: number, ppid: number, comm: string, cmdline: string, over: Partial<ProcEntry> = {}): ProcEntry => ({ pid, ppid, user: "ada", state: "S", comm, cmdline, cpu: 1, rss: 1_000, startedAt: START + pid, ...over });

const LAB: ActivityScope = { threadId: "lab-thread-1", title: "dev server", workspaceId: "ws-1", roots: [100] };
const KEPT: ActivityScope = { threadId: "kept-thread-2", title: "kept", workspaceId: "ws-1", roots: [300], kept: 300 };

/** A turn's run script and its shell down to a native install of Claude Code, which runs as its version, with vite
 * and esbuild under it; a kept agent; a server the daemon owns by the ladder with no live parent; the daemon, the host
 * and a shell of the person's own. */
const PROCS: ProcEntry[] = [
  row(1, 0, "launchd", "/sbin/launchd"),
  row(100, 50, "bash", "bash runs/53af26c40316.sh"),
  row(101, 100, "bash", "bash"),
  row(102, 101, "2.1.296", "claude -p --input-format stream-json"),
  row(200, 102, "node", "node /lab/node_modules/.bin/vite", { cpu: 9 }),
  row(201, 200, "esbuild", "esbuild --service"),
  row(300, 50, "node", "/usr/local/bin/cursor-agent --print"),
  row(400, 1, "node", "node detached.js", { owner: { by: "fork", thread: LAB.threadId } }),
  row(50, 1, "node", "node wsp host"),
  row(60, 1, "wsp-daemon", "wsp-daemon serve"),
  row(70, 1, "zsh", "-zsh"),
];

const read = (procs: ProcEntry[] = PROCS): ActivityRead => ({ computer: "lab-mac", at: START, procs, daemon: 60, host: 50, scopes: [LAB, KEPT], agentBins: ["claude", "cursor-agent"] });

describe("a snapshot read by thread", () => {
  it("puts everything under a thread's root in its tree, the stretch down to the agent's binary as the agent, and a row the daemon owns beside it", () => {
    const view = activityOf(read());
    expect(view.threads.map(t => [t.threadId, t.processes.map(p => [p.pid, p.depth, p.kind])])).toEqual([
      [LAB.threadId, [[100, 0, "agent"], [101, 1, "agent"], [102, 2, "agent"], [200, 3, "dev"], [201, 4, "dev"], [400, 0, "other"]]],
      [KEPT.threadId, [[300, 0, "agent"]]],
    ]);
    expect(view.threads[0]!.cpu).toBe(14);
    expect(view.wsp.map(p => p.pid)).toEqual([50, 60]);
    expect(view.unowned).toEqual([]);
    expect(JSON.stringify(view)).not.toContain("-zsh");
  });

  it("leaves a settle what outlives the agent: what runs under the agent ends with it", () => {
    const left = processesLeft(activityOf(read()), [LAB.threadId, KEPT.threadId]);
    expect(left.map(t => [t.threadId, t.processes.map(p => p.pid)])).toEqual([[LAB.threadId, [400]]]);
    expect(processesLeftLine("settle", left)).toBe("a thread to settle still runs a process: 400 node (other) of lab-thre:");
  });

  it("counts a kept agent's input pump and the script's shell it runs in, left under pid 1 in the turn's group, as the agent's, and a dev server left in that group as what outlives it", () => {
    const kept: ActivityScope = { threadId: "kept-thread-3", title: "kept on linux", workspaceId: "ws-1", roots: [500], kept: 500 };
    const procs = [
      row(500, 50, "bash", "bash /tmp/lab/runs/3d518f162d41.sh", { pgid: 500 }),
      row(501, 500, "claude", "claude -p --input-format stream-json", { pgid: 500 }),
      row(502, 1, "tail", "tail -n +1 -f /tmp/lab/runs/3d518f162d41.in", { pgid: 500, owner: { by: "group", thread: kept.threadId } }),
      row(503, 1, "bash", "bash /tmp/lab/runs/3d518f162d41.sh", { pgid: 500, owner: { by: "group", thread: kept.threadId } }),
      row(504, 1, "node", "node node_modules/.bin/vite", { pgid: 500, owner: { by: "group", thread: kept.threadId } }),
      row(505, 1, "tail", "tail -n +1 -f /tmp/notes.in", { pgid: 500, owner: { by: "group", thread: kept.threadId } }),
    ];
    const view = activityOf({ ...read(procs), scopes: [kept] });
    expect(view.threads[0]!.processes.map(p => [p.pid, p.kind])).toEqual(expect.arrayContaining([[502, "agent"], [503, "agent"], [504, "dev"], [505, "other"]]));
    expect(processesLeft(view, [kept.threadId])[0]!.processes.map(p => p.pid)).toEqual([504, 505]);
  });
});

describe("an End's plan", () => {
  const anyone = (): boolean => true;
  const refusal = (f: () => unknown): string => {
    try {
      f();
    } catch (e) {
      return (e as Error).message;
    }
    throw new Error("expected a refusal");
  };

  it("takes the process and everything under it, each by the start it was read with", () => {
    expect(endPlan(read(), 200, START + 200, anyone)).toEqual({ targets: [PROCS[4], PROCS[5]] });
  });

  it("refuses each process of the stretch to the agent, and one with the agent under it", () => {
    for (const pid of [100, 101, 102]) expect(refusal(() => endPlan(read(), pid, START + pid, anyone))).toContain(agentEndLine(pid, LAB));
    const above = read([...PROCS.map(p => (p.pid === 100 ? { ...p, ppid: 90 } : p)), row(90, 1, "tmux", "tmux", { owner: { by: "env", thread: LAB.threadId, workspace: "ws-1" } })]);
    expect(refusal(() => endPlan(above, 90, START + 90, anyone))).toContain(agentUnderLine(90, LAB));
  });

  it("refuses the native codex that Codex's npm launcher runs as its one child, which is the agent too", () => {
    const codex: ActivityScope = { threadId: "codex-thread-3", title: "codex", workspaceId: "ws-1", roots: [600] };
    const native = "/usr/local/lib/node_modules/@openai/codex/vendor/x86_64-unknown-linux-musl/bin/codex app-server";
    const procs = [...PROCS, row(600, 50, "bash", "bash runs/r3.sh"), row(601, 600, "node", "node /usr/local/bin/codex app-server"), row(602, 601, "codex", native)];
    expect(refusal(() => endPlan({ ...read(procs), scopes: [codex], agentBins: ["codex"] }, 602, START + 602, anyone))).toContain(agentEndLine(602, codex));
  });

  it("frees a kept agent through its thread instead of a signal", () => {
    expect(endPlan(read(), 300, START + 300, anyone)).toEqual({ kept: KEPT });
  });

  it("refuses a pid that is gone, and one that names another process now", () => {
    expect(refusal(() => endPlan(read(), 999, START, anyone))).toContain(endGoneLine(999));
    expect(refusal(() => endPlan(read(), 200, START - 60_000, anyone))).toContain(endReusedLine(200, PROCS[4]!, START - 60_000));
    // A start read a second off by another sampler is the same process.
    expect(endPlan(read(), 201, START + 201 + 1_000, anyone)).toEqual({ targets: [PROCS[5]] });
  });

  it("refuses wsp's own, and for a thread anything outside its tree", () => {
    expect(refusal(() => endPlan(read(), 60, START + 60, anyone))).toContain(wspOwnLine(60));
    expect(refusal(() => endPlan(read(), 70, START + 70, threadId => threadId === LAB.threadId))).toContain(notThreadsLine(70));
    expect(endPlan(read(), 400, START + 400, threadId => threadId === LAB.threadId)).toEqual({ targets: [PROCS[7]] });
  });

  it("tells a thread nothing of a process outside its tree, whatever start it names", () => {
    const lab = (threadId: string | undefined): boolean => threadId === LAB.threadId;
    for (const started of [START + 70, START - 60_000]) {
      const said = refusal(() => endPlan(read(), 70, started, lab));
      expect(said).toContain(notThreadsLine(70));
      expect(said).not.toContain("zsh");
    }
  });

  it("refuses for the person a process wsp did not start, one its folder or coalition alone places included, lists the coalition's as wsp's, and ends one a thread started", () => {
    expect(refusal(() => endPlan(read(), 70, START + 70, anyone))).toContain(notStartedLine(70));
    expect(refusal(() => endPlan(read(), 70, 1, anyone))).toContain(notStartedLine(70));
    // The person's own editor in a workspace's folder, and one in wsp's coalition on a Mac: where it runs, not who started it.
    for (const owner of [{ by: "folder", workspace: "ws-1" }, { by: "coalition" }] as const) {
      const placed = read([...PROCS, row(500, 1, "vim", "vim notes.md", { owner })]);
      expect(refusal(() => endPlan(placed, 500, START + 500, anyone))).toContain(notStartedLine(500));
      expect(activityOf(placed).unowned.map(p => p.pid)).toEqual(owner.by === "coalition" ? [500] : []);
    }
    // A thread's process the daemon names by its mark, the thread no longer among the read's scopes, is still wsp's.
    const marked = read([...PROCS, row(500, 1, "node", "node vite.js", { owner: { by: "env", thread: "t-gone" } })]);
    expect(endPlan(marked, 500, START + 500, anyone)).toEqual({ targets: [marked.procs.at(-1)] });
    expect(activityOf(marked).unowned.map(p => p.pid)).toEqual([500]);
  });
});
