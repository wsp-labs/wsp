// SPDX-License-Identifier: AGPL-3.0-only
// Activity's reading of one snapshot: a thread's tree from its roots and the daemon's owners, the stretch from a root
// to the agent's binary, and the rules an End meets before anything is signalled.
import { describe, expect, it } from "vitest";
import { activityOf, agentEndLine, foldTree, agentUnderLine, endGoneLine, endHeads, endPlan, endReusedLine, notStartedLine, notThreadsLine, placeProcs, processesLeft, processesLeftLine, wspOwnLine, type ActivityProc, type ActivityRead, type ActivityScope, type ActivityView, type ProcEntry } from "../src/index.js";
import { OUTSIDE_PORTS, OUTSIDE_PROCS, OUTSIDE_START } from "./fixtures/activity-outside.js";

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

  it("lets no thread end a process by the WSP_THREAD mark alone, which any process can carry with any thread's id; the person still can", () => {
    const lab = (threadId: string | undefined): boolean => threadId === LAB.threadId;
    const marked = read([...PROCS, row(500, 1, "node", "node lookalike.js", { owner: { by: "env", thread: LAB.threadId } })]);
    // Drawn under the thread it names, and ended by nobody but the person.
    expect(activityOf(marked).threads[0]!.processes.map(p => p.pid)).toContain(500);
    expect(refusal(() => endPlan(marked, 500, START + 500, lab))).toContain(notThreadsLine(500));
    expect(endPlan(marked, 500, START + 500, anyone)).toEqual({ targets: [marked.procs.at(-1)] });
    // The same process placed by the fork tracker, or standing in the thread's cgroup on a box, is the thread's to end.
    const forked = read([...PROCS, row(500, 1, "node", "node vite.js", { owner: { by: "fork", thread: LAB.threadId } })]);
    expect(endPlan(forked, 500, START + 500, lab)).toEqual({ targets: [forked.procs.at(-1)] });
    const boxed: ActivityRead = { ...read([row(500, 1, "node", "node vite.js", { owner: { by: "env", thread: LAB.threadId }, cgroup: "/wsp-threads/lab-thread-1" })]), scopes: [{ ...LAB, roots: [], cgroup: "/wsp-threads/lab-thread-1" }] };
    expect(endPlan(boxed, 500, START + 500, lab)).toEqual({ targets: [boxed.procs[0]] });
  });
});

describe("what a thread leaves running", () => {
  const proc = (pid: number, ppid: number, kind: ActivityProc["kind"], command: string): ActivityProc => ({ pid, ppid, depth: 0, startedAt: START + pid, kind, name: command.split(" ")[0]!, command, memory: 1_000, cpu: 0, ports: [] });
  const view = (processes: ActivityProc[]): ActivityView => ({ computer: "lab-mac", at: START, threads: [{ threadId: LAB.threadId, title: "lab", workspaceId: "ws-1", memory: 0, cpu: 0, processes }], wsp: [], unowned: [] });

  it("keeps the turn's input pump and run script with the agent, however a reader kinds them, and the agent's tool servers under it", () => {
    const left = processesLeft(
      view([
        proc(100, 1, "agent", "claude -p --input-format stream-json"),
        proc(110, 100, "tool", "node wsp mcp --scoped"),
        proc(120, 1, "other", "tail -n +1 -f /tmp/lab/runs/3d518f162d41.in"),
        proc(121, 1, "other", "bash /tmp/lab/runs/3d518f162d41.sh"),
        proc(200, 1, "dev", "node node_modules/.bin/vite"),
        proc(201, 200, "other", "esbuild --service"),
        proc(300, 1, "test", "node node_modules/.bin/vitest"),
      ]),
      [LAB.threadId],
    );
    expect(left[0]!.processes.map(p => p.pid)).toEqual([200, 201, 300]);
    expect(endHeads(left[0]!.processes).map(p => p.pid)).toEqual([200, 300]);
  });
});

describe("what a thread's pane and its slate started", () => {
  const root = (thread: string) => ({ by: "root" as const, thread });
  const procs = [
    ...PROCS,
    row(600, 60, "zsh", "-zsh", { pty: "pty_1", kind: "terminal", owner: root(LAB.threadId) }),
    row(601, 600, "node", "node /lab/node_modules/.bin/vite --port 5199", { kind: "terminal", owner: root(LAB.threadId) }),
    row(602, 60, "zsh", "-zsh", { pty: "pty_2" }),
    row(700, 50, "python3", "python3 /state/slates/lab-thread-1/feed.py", { kind: "slate", owner: root(LAB.threadId) }),
    row(701, 50, "node", "node /state/servers/github/index.js", { kind: "tool", owner: root(LAB.threadId) }),
  ];

  it("puts a shell its pane opened under the thread as a terminal, what runs in it by its own kind, and a shell no pane named under no thread", () => {
    const view = activityOf(read(procs));
    const lab = view.threads.find(t => t.threadId === LAB.threadId)!;
    expect(lab.processes.filter(p => p.pid >= 600).map(p => [p.pid, p.kind])).toEqual([
      [600, "terminal"],
      [601, "dev"],
      [700, "slate"],
      [701, "tool"],
    ]);
    expect(view.unowned.map(p => [p.pid, p.kind])).toEqual([[602, "terminal"]]);
  });

  it("lets the thread end a shell its pane opened, and what runs in it, as the person can, and no other thread", () => {
    const person = (): boolean => true;
    const thread = (threadId: string | undefined): boolean => threadId === LAB.threadId;
    const other = (threadId: string | undefined): boolean => threadId === KEPT.threadId;
    for (const may of [person, thread]) {
      expect(endPlan(read(procs), 601, START + 601, may)).toMatchObject({ targets: [{ pid: 601 }] });
      expect(endPlan(read(procs), 600, START + 600, may)).toMatchObject({ targets: [{ pid: 600 }, { pid: 601 }] });
    }
    for (const pid of [600, 601]) expect(() => endPlan(read(procs), pid, START + pid, other)).toThrow(notThreadsLine(pid));
  });

  it("leaves a settle or a delete what its slate runs and its pane's shells with what runs in them, to ask about", () => {
    const left = processesLeft(activityOf(read(procs)), [LAB.threadId]);
    expect(left[0]!.processes.map(p => p.pid)).toEqual([400, 600, 601, 700, 701]);
    expect(endHeads(left[0]!.processes).map(p => p.pid)).toEqual([400, 600, 700, 701]);
  });
});

describe("one fold walk for the page and the command line", () => {
  const row = (pid: number, ppid: number) => ({ pid, ppid });
  it("hangs each process under its parent, a folded row held by the row it folds into, and what a folded row started under that row", () => {
    // 10 run script, 11 its fork, 12 the agent, 13 a tool server, 14 the pump under pid 1, 20 a dev server under the script
    const list = [row(10, 1), row(11, 10), row(12, 11), row(13, 12), row(14, 1), row(20, 10)];
    const into = new Map([[10, 12], [11, 12], [14, 12]]);
    const shape = (f: ReturnType<typeof foldTree<{ pid: number; ppid: number }>>[number]): unknown => ({ pid: f.p.pid, held: f.held.map(h => h.pid), kids: f.kids.map(shape) });
    expect(foldTree(list, pid => into.get(pid)).map(shape)).toEqual([{ pid: 12, held: [10, 11, 14], kids: [{ pid: 13, held: [], kids: [] }, { pid: 20, held: [], kids: [] }] }]);
  });
  it("leaves a row whose target is outside the list standing on its own", () => {
    expect(foldTree([row(10, 1)], () => 99).map(f => f.p.pid)).toEqual([10]);
  });
});

describe("an agent outside this host's threads", () => {
  const anyone = (): boolean => true;
  const outside: ActivityRead = { computer: "lab-mac", at: OUTSIDE_START, procs: OUTSIDE_PROCS, daemon: 20, host: 10, scopes: [], agentBins: ["claude", "codex"], ports: OUTSIDE_PORTS };
  const kinds = (rows: readonly ActivityProc[]) => Object.fromEntries(rows.map(p => [p.pid, p.kind]));

  it("reads a Claude Code another wsp's thread runs as an agent, though a listening process stands above it, and its servers as tool servers", () => {
    expect(kinds(activityOf(outside).unowned)).toEqual({ 450: "dev", 500: "agent", 510: "tool", 511: "other", 520: "tool", 530: "other", 600: "agent", 601: "agent" });
  });

  it("leaves an agent binary a thread of this host left running to its settle, as no agent of that thread", () => {
    const orphan: ActivityRead = { ...read([...PROCS, row(900, 1, "2.1.296", "claude -p summarise the diff", { owner: { by: "env", thread: "th_1" } })]), scopes: [LAB, KEPT, { threadId: "th_1", title: "summary", workspaceId: "ws-1", roots: [] }] };
    const view = activityOf(orphan);
    expect(view.threads.find(t => t.threadId === "th_1")!.processes.map(p => [p.pid, p.kind])).toEqual([[900, "other"]]);
    expect(processesLeft(view, ["th_1"]).flatMap(t => t.processes.map(p => p.pid))).toEqual([900]);
  });

  it("reads the person's own Claude Code as an agent too, and offers no End on it", () => {
    const placed = placeProcs({ procs: OUTSIDE_PROCS, daemon: 20, host: 10, threads: [], agentBins: ["claude", "codex"], listens: pid => OUTSIDE_PORTS.has(pid) });
    expect([placed.whose.get(700), placed.kindOf(700), placed.endable(700)]).toEqual([undefined, "agent", false]);
  });

  it("still ends it from No thread, since no thread's stop will", () => {
    const plan = endPlan(outside, 500, OUTSIDE_START + 500, anyone);
    expect("targets" in plan && plan.targets.map(p => p.pid)).toEqual([500, 510, 511, 520, 530]);
  });
});
