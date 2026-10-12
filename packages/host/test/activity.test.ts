// SPDX-License-Identifier: AGPL-3.0-only
// wsp activity, wsp activity end and --processes on settle and delete, against a host whose daemon lists a table
// the case writes: a thread's agent with a dev server under it, read as the thread's tree, never ended by pid alone.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, realpathSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { ActivityEnded, ActivityView, EXIT_CODES, END_AGAIN_FIX, END_GRACE_MS, endGraceWord, HERE_PLACE_ID, PROCESSES_LEFT_KIND, VerbFailure, agentEndFix, agentEndLine, endReusedLine, NOT_STARTED_FIX, endGoneSaid, notStartedLine, notThreadsLine, processesLeft, processesLeftFix, processesLeftLine, processesLeftOf, refusalLine } from "@wsp/protocol";
import { projectOn } from "./verbs-fixture.js";
import { FAKE_DAEMON_PID, procRow } from "./fake-procs.js";
import { verbsHost } from "./verbs-host.js";
import { ACTIVITY_VERBS, endedLines } from "../src/verbs/activity.js";

const STARTED = 1_760_000_000_000;
/** The unique id the daemon read vite's process by, which an End sends back. */
const VITE_ID = "88000";

describe("activity on the command line", () => {
  const h = verbsHost();

  /** A finished thread in a folder on this computer, and the daemon listing its kept agent with its tool server under
   * it, and a dev server the agent detached with that server's child under it, each owned by the thread as the
   * daemon's ladder names it. */
  async function threadWithServer(): Promise<{ threadId: string; title: string }> {
    const folder = realpathSync(mkdtempSync(join(h.dir, "repo-lab-")));
    execFileSync("git", ["init", "-q", folder]);
    await projectOn(h.rt, HERE_PLACE_ID, folder, { name: "lab" });
    const ran = await h.run("run", "lab", "start the dev server", "--json");
    expect(ran.code, ran.io.errors.join("\n")).toBe(0);
    const { threadId } = h.json(ran.io).at(-1) as { threadId: string };
    const owner = { by: "fork" as const, thread: threadId };
    h.daemon.procs.procs = [
      procRow({ pid: FAKE_DAEMON_PID, ppid: 0, comm: "wsp-daemon", cmdline: "wsp-daemon serve", rss: 9_000_000 }),
      procRow({ pid: 4100, comm: "2.1.296", cmdline: "claude -p --input-format stream-json", rss: 300_000_000, cpu: 3, kind: "agent", owner, startedAt: STARTED }),
      procRow({ pid: 4150, ppid: 4100, comm: "node", cmdline: "node wsp mcp --scoped", rss: 70_000_000, owner, startedAt: STARTED + 500 }),
      procRow({ pid: 4200, comm: "node", cmdline: "node /lab/node_modules/.bin/vite", rss: 80_000_000, cpu: 10, kind: "dev", owner, startedAt: STARTED + 1_000, id: VITE_ID }),
      procRow({ pid: 4300, ppid: 4200, comm: "esbuild", cmdline: "/lab/node_modules/esbuild/bin/esbuild --service", rss: 20_000_000, owner, startedAt: STARTED + 2_000, id: "88001" }),
      // The person's own shell, its terminal app not listed: the daemon's children are wsp's.
      procRow({ pid: 5000, ppid: 4999, comm: "zsh", cmdline: "-zsh", rss: 4_000_000 }),
    ];
    h.daemon.procs.ports = [{ port: 5173, pid: 4200 }];
    const title = (h.json((await h.run("threads", "--json")).io).at(-1) as { threads: { id: string; threadId?: string; title: string }[] }).threads.find(t => (t.threadId ?? t.id) === threadId)!.title;
    return { threadId, title };
  }

  const failure = (io: { errors: string[] }): VerbFailure => VerbFailure.parse(JSON.parse(io.errors.at(-1)!));

  it("lists a thread's tree from its agent down, with kinds, ports and its totals, then wsp's own; a process nobody started through wsp is left out", async () => {
    const { threadId, title } = await threadWithServer();
    const read = await h.run("activity", "--json");
    expect(read.code, read.io.errors.join("\n")).toBe(0);
    const view = ActivityView.strict().parse(h.json(read.io).at(-1));
    expect(view.threads).toEqual([
      {
        threadId,
        title,
        workspaceId: expect.any(String),
        memory: 470_000_000,
        cpu: 13,
        processes: [
          { pid: 4200, ppid: 1, depth: 0, startedAt: STARTED + 1_000, kind: "dev", name: "node", command: "node /lab/node_modules/.bin/vite", memory: 80_000_000, cpu: 10, ports: [5173] },
          { pid: 4300, ppid: 4200, depth: 1, startedAt: STARTED + 2_000, kind: "dev", name: "esbuild", command: "/lab/node_modules/esbuild/bin/esbuild --service", memory: 20_000_000, cpu: 0, ports: [] },
          { pid: 4100, ppid: 1, depth: 0, startedAt: STARTED, kind: "agent", name: "2.1.296", command: "claude -p --input-format stream-json", memory: 300_000_000, cpu: 3, ports: [] },
          { pid: 4150, ppid: 4100, depth: 1, startedAt: STARTED + 500, kind: "tool", name: "node", command: "node wsp mcp --scoped", memory: 70_000_000, cpu: 0, ports: [] },
        ],
      },
    ]);
    expect(view.wsp.map(p => [p.pid, p.kind])).toEqual([[FAKE_DAEMON_PID, "wsp"]]);
    expect(view.unowned).toEqual([]);
    expect(JSON.stringify(view)).not.toContain("-zsh");
    const one = await h.run("activity", threadId.slice(0, 8), "--json");
    expect((h.json(one.io).at(-1) as ActivityView).threads.map(t => t.threadId)).toEqual([threadId]);
    const prose = (await h.run("activity")).io.lines.join("\n");
    expect(prose).toMatch(/^ {2}Claude Code {2,}\S/m);
    expect(prose).toMatch(/^ {2}vite :5173 {2,}Dev server {2,}76 MB {2}10\.0%$/m);
    expect(prose).toMatch(/^ {2}Daemon {2,}9 MB {2,}0\.0%$/m);
    expect(prose).not.toContain("claude -p");
    const wide = (await h.run("activity", "--wide")).io.lines.join("\n");
    expect(wide).toMatch(/^ {2}vite :5173 {2,}Dev server {2,}4200 {2,}76 MB {2,}10\.0% {2}node \/lab\/node_modules\/\.bin\/vite$/m);
  });

  it("--json prints the host's read whole and alone, --wide or not: the tool and every agent read what they read before", async () => {
    await threadWithServer();
    const plain = await h.run("activity", "--json");
    const wide = await h.run("activity", "--json", "--wide");
    expect(plain.io.lines).toHaveLength(1);
    // Each read stamps its own time.
    const timeless = (lines: string[]): string[] => lines.map(l => l.replace(/"at":\d+/, '"at":0'));
    expect(timeless(wide.io.lines)).toEqual(timeless(plain.io.lines));
    expect(plain.io.lines[0]).toBe(JSON.stringify(ActivityView.strict().parse(JSON.parse(plain.io.lines[0]!))));
  });

  it("lists the newest thread first", async () => {
    const ids: string[] = [];
    for (const name of ["one", "two"]) {
      const folder = realpathSync(mkdtempSync(join(h.dir, `repo-${name}-`)));
      execFileSync("git", ["init", "-q", folder]);
      await projectOn(h.rt, HERE_PLACE_ID, folder, { name });
      ids.push((h.json((await h.run("run", name, "go", "--json")).io).at(-1) as { threadId: string }).threadId);
    }
    h.daemon.procs.procs = ids.map((thread, i) => procRow({ pid: 4100 + i, owner: { by: "fork", thread } }));
    const view = ActivityView.parse(h.json((await h.run("activity", "--json")).io).at(-1));
    expect(view.threads.map(t => t.threadId)).toEqual([...ids].reverse());
  });

  it("end refuses the agent, sending nothing, and points at the thread's stop", async () => {
    const { threadId, title } = await threadWithServer();
    const agent = await h.run("activity", "end", "4100", "--started", String(STARTED), "--json");
    expect(agent.code).toBe(EXIT_CODES.usage);
    const scope = { threadId, title, workspaceId: "", roots: [] };
    expect(failure(agent.io)).toEqual({ error: refusalLine(agentEndLine(4100, scope), agentEndFix(scope)), class: "usage", exit: EXIT_CODES.usage });
    expect(h.daemon.procs.signals).toEqual([]);
  });

  it("end refuses a pid that names another process now, sending nothing", async () => {
    await threadWithServer();
    const reused = await h.run("activity", "end", "4200", "--started", String(STARTED - 60_000), "--json");
    expect(reused.code).toBe(EXIT_CODES.usage);
    expect(failure(reused.io).error).toBe(refusalLine(endReusedLine(4200, procRow({ pid: 4200, comm: "node", startedAt: STARTED + 1_000 }), STARTED - 60_000), END_AGAIN_FIX));
    expect(h.daemon.procs.signals).toEqual([]);
  });

  it("end refuses a process wsp did not start, sending nothing, whatever start it names", async () => {
    await threadWithServer();
    for (const started of [STARTED, 1]) {
      const theirs = await h.run("activity", "end", "5000", "--started", String(started), "--json");
      expect(theirs.code).toBe(EXIT_CODES.usage);
      expect(failure(theirs.io).error).toBe(refusalLine(notStartedLine(5000), NOT_STARTED_FIX));
    }
    expect(h.daemon.procs.signals).toEqual([]);
  });

  it("end takes the process listed and everything under it in one tree kill, by the start and identity it was listed with", async () => {
    await threadWithServer();
    const ended = await h.run("activity", "end", "4200", "--started", String(STARTED + 1_000), "--json");
    expect(ended.code, ended.io.errors.join("\n")).toBe(0);
    const answer = ActivityEnded.parse(h.json(ended.io).at(-1));
    expect(answer.ended.map(p => p.pid)).toEqual([4200, 4300]);
    expect(answer).toMatchObject({ forced: [], left: [] });
    expect(h.daemon.procs.kills).toHaveLength(1);
    expect(h.daemon.procs.kills[0]).toMatchObject({ op: "proc.kill", pid: 4200, signal: "TERM", mode: "tree", startedAt: STARTED + 1_000, identity: VITE_ID });
    expect(h.daemon.procs.signals).toEqual([
      { pid: 4200, signal: "TERM" },
      { pid: 4300, signal: "TERM" },
    ]);
  });

  it("end signals through the host's guard, so each kill carries this computer's lists for the daemon to check at the signal", async () => {
    await threadWithServer();
    const ended = await h.run("activity", "end", "4200", "--started", String(STARTED + 1_000), "--json");
    expect(ended.code, ended.io.errors.join("\n")).toBe(0);
    expect(h.daemon.procs.kills).toHaveLength(1);
    for (const frame of h.daemon.procs.kills) {
      expect(frame).toMatchObject({ op: "proc.kill", signal: "TERM", protect: [process.pid, process.ppid], agentBins: expect.arrayContaining(["claude"]) });
      expect(Array.isArray(frame["agents"]) && Array.isArray(frame["apps"])).toBe(true);
    }
  });

  it("end leaves TERM's grace and the KILL after it to the daemon, naming what it sent KILL to, and sends nothing more itself", async () => {
    await threadWithServer();
    h.daemon.procs.stubborn.add(4300);
    const ended = await h.run("activity", "end", "4200", "--started", String(STARTED + 1_000), "--json");
    expect(ended.code, ended.io.errors.join("\n")).toBe(0);
    expect(ActivityEnded.parse(h.json(ended.io).at(-1))).toMatchObject({ forced: [4300], left: [] });
    expect(h.daemon.procs.kills).toHaveLength(1);
    expect(h.daemon.procs.signals).toEqual([
      { pid: 4200, signal: "TERM" },
      { pid: 4300, signal: "TERM" },
      { pid: 4300, signal: "KILL" },
    ]);
  });

  it("end signals nothing when the pid holds another process by the signal: the daemon checks the identity it was sent", async () => {
    await threadWithServer();
    // Between the host's read and its signal, vite exits and its pid goes to another process.
    h.daemon.procs.afterRead = () => {
      h.daemon.procs.procs = h.daemon.procs.procs.map(p => (p.pid === 4200 ? { ...p, comm: "sleep", cmdline: "sleep 600", id: "99999" } : p));
    };
    const ended = await h.run("activity", "end", "4200", "--started", String(STARTED + 1_000), "--json");
    expect(ended.code, ended.io.errors.join("\n")).toBe(0);
    expect(ActivityEnded.parse(h.json(ended.io).at(-1))).toMatchObject({ ended: [], forced: [], left: [] });
    expect(h.daemon.procs.signals).toEqual([]);
    expect(h.daemon.procs.procs.map(p => p.pid)).toContain(4200);
  });

  it("end says nothing was signalled when the pid holds another process by the signal", async () => {
    await threadWithServer();
    h.daemon.procs.afterRead = () => {
      h.daemon.procs.procs = h.daemon.procs.procs.map(p => (p.pid === 4200 ? { ...p, comm: "sleep", cmdline: "sleep 600", id: "99999" } : p));
    };
    const ended = await h.run("activity", "end", "4200", "--started", String(STARTED + 1_000));
    expect(ended.code, ended.io.errors.join("\n")).toBe(0);
    expect(ended.io.lines).toEqual([endGoneSaid(4200)]);
    expect(h.daemon.procs.signals).toEqual([]);
  });

  it("a thread settling itself hears at the first ask which processes only the person can end, and stop refuses them before any signal", async () => {
    const { threadId } = await threadWithServer();
    // A server the thread's turn detached with setsid, placed by its mark alone, which gives a thread no reach.
    h.daemon.procs.procs.push(procRow({ pid: 4800, comm: "python3", cmdline: "python3 -m http.server", owner: { by: "env", thread: threadId }, startedAt: STARTED + 8_000 }));
    const workspaceId = (await h.rt.sessions.list()).find(v => v.threadId === threadId)!.workspaceId;
    const self = { origin: "here" as const, by: { kind: "thread" as const, threadId, workspaceId, rootThreadId: threadId } };
    const said = async (processes: "ask" | "stop"): Promise<string> =>
      h.rt.activity.meet([threadId], processes, "settle", self).then(
        () => "",
        (e: unknown) => (e as Error).message,
      );
    const asked = await said("ask");
    expect(asked).toContain(`4800 python3 (dev, the person's to end) of ${threadId.slice(0, 8)}`);
    expect(asked).toContain("ask the person to end 4800");
    expect(await said("stop")).toContain(notThreadsLine(4800));
    expect(h.daemon.procs.signals).toEqual([]);
  });

  it("end on a daemon from before kill modes names only the pid it signalled", async () => {
    await threadWithServer();
    h.daemon.procs.modes = false;
    const ended = await h.run("activity", "end", "4200", "--started", String(STARTED + 1_000), "--json");
    expect(ended.code, ended.io.errors.join("\n")).toBe(0);
    expect(ActivityEnded.parse(h.json(ended.io).at(-1)).ended.map(p => p.pid)).toEqual([4200]);
    expect(h.daemon.procs.signals).toEqual([{ pid: 4200, signal: "TERM" }]);
    h.daemon.procs.modes = true;
  });

  it("settle with processes stop ends each tree the thread left in one kill at once, every head by its identity", async () => {
    const { threadId } = await threadWithServer();
    h.daemon.procs.procs.push(procRow({ pid: 4400, comm: "node", cmdline: "node /lab/node_modules/.bin/vitest", kind: "test", owner: { by: "fork", thread: threadId }, startedAt: STARTED + 3_000, id: "88002" }));
    const stopped = await h.run("thread", "settle", threadId, "--processes", "stop", "--json");
    expect(stopped.code, stopped.io.errors.join("\n")).toBe(0);
    expect(h.daemon.procs.kills.map(k => ({ pid: k["pid"], mode: k["mode"], startedAt: k["startedAt"], identity: k["identity"] }))).toEqual(
      expect.arrayContaining([
        { pid: 4200, mode: "tree", startedAt: STARTED + 1_000, identity: VITE_ID },
        { pid: 4400, mode: "tree", startedAt: STARTED + 3_000, identity: "88002" },
      ]),
    );
    expect(h.daemon.procs.kills).toHaveLength(2);
    expect(h.daemon.procs.procs.map(p => p.pid)).toEqual([FAKE_DAEMON_PID, 4100, 4150, 5000]);
  });

  it("settle with processes left and no --processes refuses, naming what outlives the agent; stop ends those and settles", async () => {
    const { threadId } = await threadWithServer();
    h.daemon.procs.asked = [];
    const refused = await h.run("thread", "settle", threadId, "--json");
    expect(refused.code).toBe(EXIT_CODES.usage);
    // One list, answered at once, never a watch waiting on the sampler's first tick.
    expect(h.daemon.procs.asked).toEqual(["proc.list"]);
    const left = processesLeft(ActivityView.parse(h.json((await h.run("activity", threadId, "--json")).io).at(-1)), [threadId]);
    expect(left[0]!.processes.map(p => p.pid)).toEqual([4200, 4300]);
    expect(failure(refused.io).error).toBe(refusalLine(processesLeftLine("settle", left), processesLeftFix("settle")));
    expect(failure(refused.io).error).toContain(`4200 node (dev, :5173) of ${threadId.slice(0, 8)}, 4300 esbuild (dev) of ${threadId.slice(0, 8)}`);
    expect(h.daemon.procs.signals).toEqual([]);
    expect((h.json((await h.run("threads", "--json")).io).at(-1) as { threads: { settledAt?: number; id: string }[] }).threads.every(t => t.settledAt === undefined)).toBe(true);

    const stopped = await h.run("thread", "settle", threadId, "--processes", "stop", "--json");
    expect(stopped.code, stopped.io.errors.join("\n")).toBe(0);
    expect(h.json(stopped.io).at(-1)).toMatchObject({ settled: [{ threadId }], left: [], ended: [{ pid: 4200, forced: [], left: [] }] });
    expect(h.daemon.procs.signals).toEqual([
      { pid: 4200, signal: "TERM" },
      { pid: 4300, signal: "TERM" },
    ]);
    expect(h.daemon.procs.procs.map(p => p.pid)).toEqual([FAKE_DAEMON_PID, 4100, 4150, 5000]);
  });

  it("delete refuses the same way before it asks, and keep deletes with the processes left running", async () => {
    const { threadId } = await threadWithServer();
    const refused = await h.run("delete", threadId, "--yes", "--json");
    expect(refused.code).toBe(EXIT_CODES.usage);
    expect(failure(refused.io).error).toContain("the thread to delete still runs 2 processes: 4200 node (dev, :5173)");
    const kept = await h.run("delete", threadId, "--processes", "keep", "--yes", "--json");
    expect(kept.code, kept.io.errors.join("\n")).toBe(0);
    expect(h.daemon.procs.signals).toEqual([]);
    expect(h.daemon.procs.procs.map(p => p.pid)).toContain(4200);
  });

  /** A shell the thread's pane opened, a dev server running in it, as the daemon stamps them once the host names the shell;
   * and the shell's tag in the host's state, which a delete reads to end the thread's shells. */
  async function withShell(threadId: string): Promise<void> {
    const owner = { by: "root" as const, thread: threadId };
    h.daemon.procs.procs.push(
      procRow({ pid: 5100, comm: "zsh", cmdline: "-zsh", pty: "pty_1", kind: "terminal", owner, startedAt: STARTED + 5_000, id: "88100" }),
      procRow({ pid: 5110, ppid: 5100, comm: "node", cmdline: "node /lab/node_modules/.bin/vite --port 5199", kind: "terminal", owner, startedAt: STARTED + 5_100, id: "88110" }),
    );
    const workspaceId = (await h.rt.sessions.list()).find(v => v.threadId === threadId)!.workspaceId;
    await h.store.put("pty-threads", workspaceId, { pty_1: threadId });
  }

  it("settle asks about a shell the thread's pane opened and what runs in it, and stop ends it, the thread itself too", async () => {
    const { threadId } = await threadWithServer();
    await withShell(threadId);
    const refused = await h.run("thread", "settle", threadId, "--json");
    expect(refused.code).toBe(EXIT_CODES.usage);
    expect(failure(refused.io).error).toContain(`5100 zsh (terminal) of ${threadId.slice(0, 8)}, 5110 node (dev) of ${threadId.slice(0, 8)}`);
    const workspaceId = (await h.rt.sessions.list()).find(v => v.threadId === threadId)!.workspaceId;
    const self = { origin: "here" as const, by: { kind: "thread" as const, threadId, workspaceId, rootThreadId: threadId } };
    const ended = await h.rt.activity.meet([threadId], "stop", "settle", self);
    expect(ended.map(e => e.pid).sort()).toEqual([4200, 5100]);
    expect(h.daemon.procs.kills.map(k => ({ pid: k["pid"], mode: k["mode"], identity: k["identity"] }))).toContainEqual({ pid: 5100, mode: "tree", identity: "88100" });
    expect(h.daemon.procs.procs.map(p => p.pid)).not.toContain(5110);
  });

  it("delete asks about a shell the thread's pane opened, and keep leaves it running", async () => {
    const { threadId } = await threadWithServer();
    await withShell(threadId);
    const refused = await h.run("delete", threadId, "--yes", "--json");
    expect(refused.code).toBe(EXIT_CODES.usage);
    expect(failure(refused.io).error).toContain("the thread to delete still runs 4 processes:");
    expect(failure(refused.io).error).toContain("5100 zsh (terminal)");
    const kept = await h.run("delete", threadId, "--processes", "keep", "--yes", "--json");
    expect(kept.code, kept.io.errors.join("\n")).toBe(0);
    expect(h.daemon.procs.signals).toEqual([]);
    expect(h.daemon.ops).not.toContain("pty.kill");
    expect(h.daemon.procs.procs.map(p => p.pid)).toEqual(expect.arrayContaining([5100, 5110]));
  });

  it("delete with stop ends a shell the thread's pane opened with the rest", async () => {
    const { threadId } = await threadWithServer();
    await withShell(threadId);
    const stopped = await h.run("delete", threadId, "--processes", "stop", "--yes", "--json");
    expect(stopped.code, stopped.io.errors.join("\n")).toBe(0);
    expect(h.daemon.procs.kills.map(k => k["pid"])).toEqual(expect.arrayContaining([4200, 5100]));
    expect(h.daemon.procs.procs.map(p => p.pid)).not.toContain(5100);
  });

  it("a thread beside it in the same folder keeps its own processes: the refusal carries none of them, and neither stop nor keep touches them", async () => {
    const { threadId } = await threadWithServer();
    const ran = await h.run("run", "lab", "start the docs server", "--json");
    expect(ran.code, ran.io.errors.join("\n")).toBe(0);
    const beside = (h.json(ran.io).at(-1) as { threadId: string }).threadId;
    expect(beside).not.toBe(threadId);
    const theirs = { by: "fork" as const, thread: beside };
    h.daemon.procs.procs.push(
      procRow({ pid: 4600, comm: "node", cmdline: "node /lab/node_modules/.bin/vite --port 5174", rss: 90_000_000, kind: "dev", owner: theirs, startedAt: STARTED + 3_000 }),
      procRow({ pid: 4650, ppid: 4600, comm: "esbuild", cmdline: "/lab/node_modules/esbuild/bin/esbuild --service", rss: 20_000_000, owner: theirs, startedAt: STARTED + 4_000 }),
    );
    h.daemon.procs.ports.push({ port: 5174, pid: 4600 });

    // What the app reads off the refusal: this thread's tree alone, with its port and memory.
    const refused = await h.rt.sessions.settle([threadId], undefined, { processes: "ask" }).then(
      () => undefined,
      (e: unknown) => e,
    );
    expect(refused).toMatchObject({ kind: PROCESSES_LEFT_KIND });
    const left = processesLeftOf(refused).left!;
    expect(left.map(t => t.threadId)).toEqual([threadId]);
    expect(left[0]!.processes.map(p => ({ pid: p.pid, ports: p.ports, memory: p.memory }))).toEqual([
      { pid: 4200, ports: [5173], memory: 80_000_000 },
      { pid: 4300, ports: [], memory: 20_000_000 },
    ]);
    expect(failure((await h.run("thread", "settle", threadId, "--json")).io).error).not.toContain("4600");

    const stopped = await h.run("thread", "settle", threadId, "--processes", "stop", "--json");
    expect(stopped.code, stopped.io.errors.join("\n")).toBe(0);
    expect(h.daemon.procs.signals.map(s => s.pid)).toEqual([4200, 4300]);
    expect(h.daemon.procs.procs.map(p => p.pid)).toEqual(expect.arrayContaining([4600, 4650]));

    const kept = await h.run("thread", "settle", beside, "--processes", "keep", "--json");
    expect(kept.code, kept.io.errors.join("\n")).toBe(0);
    expect(h.json(kept.io).at(-1)).toMatchObject({ settled: [{ threadId: beside }], left: [] });
    expect(h.daemon.procs.signals.map(s => s.pid)).toEqual([4200, 4300]);
    expect(h.daemon.procs.procs.map(p => p.pid)).toEqual(expect.arrayContaining([4600, 4650]));
  });

  it("settle on a daemon with no list waits for the watch's snapshot instead, and still refuses", async () => {
    const { threadId } = await threadWithServer();
    h.daemon.procs.lists = false;
    h.daemon.procs.asked = [];
    const refused = await h.run("thread", "settle", threadId, "--json");
    expect(refused.code).toBe(EXIT_CODES.usage);
    expect(failure(refused.io).error).toContain("a thread to settle still runs 2 processes");
    expect(h.daemon.procs.asked).toEqual(["proc.list", "proc.watch"]);
    h.daemon.procs.lists = true;
  });

  it("a request relayed from a machine reads no process outside a thread and ends none, by the workspace rule's own road check", async () => {
    await threadWithServer();
    h.daemon.procs.procs.push(procRow({ pid: 4700, comm: "node", cmdline: "node watch.js", owner: { by: "env", thread: "t-gone" }, startedAt: STARTED + 7_000 }));
    expect((await h.rt.activity.read({})).unowned.map(p => p.pid)).toEqual([4700]);
    const relayed = await h.rt.activity.read({}, "relayed");
    expect(relayed.unowned).toEqual([]);
    expect(relayed.wsp).toEqual([]);
    await expect(h.rt.activity.end({ pid: 4700, startedAt: STARTED + 7_000 }, "relayed")).rejects.toThrow(notThreadsLine(4700));
    expect(h.daemon.procs.signals).toEqual([]);
  });

  it("says TERM's grace as the daemon's own, in the answer, the verb's words and the skill's row", () => {
    expect(endGraceWord).toBe(`${END_GRACE_MS / 1000} s`);
    const proc = { pid: 1, ppid: 0, depth: 0, startedAt: 0, kind: "other" as const, name: "node", command: "node", memory: 0, cpu: 0, ports: [] };
    expect(endedLines({ pid: 1, ended: [proc], forced: [2], left: [] })).toBe(`ended 1 node\nKILL after TERM's ${endGraceWord}: 2`);
    const end = ACTIVITY_VERBS.find(v => v.name === "activity end")!;
    expect(end.about).toContain(`KILL after ${endGraceWord}`);
    const row = readFileSync(new URL("../../../skills/wsp/SKILL.md", import.meta.url), "utf8").split("\n").find(l => l.includes("`activity_end` (pid, started, computer)"))!;
    expect(row).toContain(`KILL after ${endGraceWord}`);
  });

  it("a computer this host does not hold is refused", async () => {
    const other = await h.run("activity", "--computer", "nowhere", "--json");
    expect(other.code).toBe(EXIT_CODES.usage);
    expect(failure(other.io).error).toContain("no place named nowhere");
  });
});
