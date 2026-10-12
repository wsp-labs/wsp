// SPDX-License-Identifier: AGPL-3.0-only
// Settle and delete on a thread in a folder on a box the person joined: what it
// left running there is read off its cgroup, asked about by name, stopped
// through the cgroup end the box already runs, or kept running; another
// thread's cgroup on the same box is never read or ended.
import { describe, expect, it } from "vitest";
import { PROCESSES_LEFT_KIND, processesLeftOf, threadCgroup, type ProcEntry } from "@wsp/protocol";
import { memoryStore } from "../src/store.js";
import { answering, joined, type Started } from "./box-fixture.js";
import { until } from "./until.js";

const row = (pid: number, ppid: number, comm: string, cmdline: string, cgroup: string): ProcEntry => ({ pid, ppid, user: "root", state: "S", comm, cmdline, cpu: 0, rss: 50_000_000, startedAt: 1_760_000_000_000 + pid, cgroup });

/** hetzner holding a finished thread whose agent left vite and its esbuild child in the thread's cgroup, and a
 * process of another thread's cgroup beside them. */
async function boxThread() {
  const starts: Started[] = [];
  const held = await joined({ adapters: { claude: answering(starts) } });
  const at = await held.rt.workspaces.folderFor({ project: held.project.id });
  const turn = await held.rt.sessions.start(at.workspace.id, { prompt: "start the dev server", harness: "claude" });
  const threadId = turn.view().threadId!;
  await until(async () => (await held.rt.sessions.list(at.workspace.id)).every(s => s.status !== "running"));
  const mine = threadCgroup(threadId);
  const theirs = threadCgroup("other-thread");
  held.seen.procs = [
    row(7100, 1, "node", "node /root/spoo-ts/node_modules/.bin/vite", mine),
    row(7101, 7100, "esbuild", "/root/spoo-ts/node_modules/esbuild/bin/esbuild --service", mine),
    row(7200, 1, "node", "node /root/else/node_modules/.bin/vite --port 5174", theirs),
  ];
  held.seen.ports = [
    { port: 5173, pid: 7100 },
    { port: 5174, pid: 7200 },
  ];
  const ends = (): string[] => held.seen.execs.filter(e => e.cmd.includes("wsp_end()")).map(e => /wsp_end '[^']+' \d/.exec(e.cmd)?.[0] ?? e.cmd);
  return { ...held, threadId, workspaceId: at.workspace.id, mine, ends };
}

const pids = (procs: readonly ProcEntry[]): number[] => procs.map(p => p.pid);

describe("a thread on a box with a dev server left running", () => {
  it("is asked about by name and port, the box named, with nothing ended and the other thread's process left out", async () => {
    const b = await boxThread();
    const refused = await b.rt.sessions.settle([b.threadId], undefined, { processes: "ask" }).then(
      () => undefined,
      (e: unknown) => e,
    );
    expect(refused).toMatchObject({ kind: PROCESSES_LEFT_KIND });
    expect((refused as Error).message).toContain(`7100 node (dev, :5173) of ${b.threadId.slice(0, 8)} on hetzner`);
    const left = processesLeftOf(refused).left!;
    expect(left.map(t => ({ threadId: t.threadId, on: t.on, pids: t.processes.map(p => p.pid), ports: t.processes[0]!.ports }))).toEqual([{ threadId: b.threadId, on: "hetzner", pids: [7100, 7101], ports: [5173] }]);
    expect(b.ends()).toEqual([]);
    expect(pids(b.seen.procs)).toEqual([7100, 7101, 7200]);
  });

  it("stop ends the thread's cgroup there, keeping it, and settles; the other thread's process stays", async () => {
    const b = await boxThread();
    const answer = await b.rt.sessions.settle([b.threadId], undefined, { processes: "stop" });
    expect(answer.settled.map(t => t.threadId)).toEqual([b.threadId]);
    expect(answer.ended).toEqual([expect.objectContaining({ pid: 7100, forced: [], left: [] })]);
    expect(b.ends()).toEqual([`wsp_end '${b.mine}' 0`]);
    expect(pids(b.seen.procs)).toEqual([7200]);
  });

  it("stop names what the cgroup end left standing, by the read after it", async () => {
    const b = await boxThread();
    b.seen.endFails = true;
    const answer = await b.rt.sessions.settle([b.threadId], undefined, { processes: "stop" });
    expect(answer.ended).toEqual([expect.objectContaining({ pid: 7100, left: [7100, 7101] })]);
  });

  it("keep settles and ends nothing there", async () => {
    const b = await boxThread();
    const answer = await b.rt.sessions.settle([b.threadId], undefined, { processes: "keep" });
    expect(answer.settled.map(t => t.threadId)).toEqual([b.threadId]);
    expect(b.ends()).toEqual([]);
    expect(pids(b.seen.procs)).toEqual([7100, 7101, 7200]);
  });

  it("a delete with keep leaves the cgroup and what runs in it", async () => {
    const b = await boxThread();
    await b.rt.sessions.delete(b.threadId, undefined, { processes: "keep" });
    expect(b.ends()).toEqual([]);
    expect(pids(b.seen.procs)).toEqual([7100, 7101, 7200]);
  });

  it("a delete with stop ends the cgroup's processes, then takes the cgroup away", async () => {
    const b = await boxThread();
    await b.rt.sessions.delete(b.threadId, undefined, { processes: "stop" });
    expect(b.ends()).toEqual([`wsp_end '${b.mine}' 0`, `wsp_end '${b.mine}' 1`]);
    expect(pids(b.seen.procs)).toEqual([7200]);
  });

  it("on a box whose daemon predates the one-shot list, settles as before and leaves the link's process watch alone", async () => {
    const b = await boxThread();
    b.seen.listless = true;
    const before = b.seen.ops.length;
    const answer = await b.rt.sessions.settle([b.threadId], undefined, { processes: "ask" });
    expect(answer.settled.map(t => t.threadId)).toEqual([b.threadId]);
    expect(b.seen.ops.slice(before)).toContain("proc.list");
    expect(b.seen.ops.slice(before)).not.toContain("proc.watch");
    expect(b.ends()).toEqual([]);
  });

  it("a thread that left nothing settles with nothing asked and nothing ended", async () => {
    const b = await boxThread();
    b.seen.procs = b.seen.procs.filter(p => p.pid === 7200);
    const answer = await b.rt.sessions.settle([b.threadId], undefined, { processes: "ask" });
    expect(answer.settled.map(t => t.threadId)).toEqual([b.threadId]);
    expect(b.ends()).toEqual([]);
  });
});

describe("a thread on a box whose pane opened a shell with a dev server in it", () => {
  /** The shell and its vite, outside the thread's cgroup as the box daemon's pty.create leaves them, stamped the
   * thread's as that daemon does once the host names the shell; the shell's tag in the host's state; and a daemon
   * whose pty.kill takes the shell and what runs in it. */
  async function withShell(o: { stamped?: boolean } = {}) {
    const store = memoryStore();
    const starts: Started[] = [];
    const killed: string[] = [];
    let held: Awaited<ReturnType<typeof joined>> | undefined;
    const ptys = async (frame: Record<string, unknown>): Promise<Record<string, unknown>> => {
      if (frame["op"] === "pty.kill") {
        killed.push(String(frame["ptyId"]));
        held!.seen.procs = held!.seen.procs.filter(p => p.pid !== 9100 && p.pid !== 9110);
      }
      return { ok: true };
    };
    held = await joined({ adapters: { claude: answering(starts) }, store, ptys });
    const at = await held.rt.workspaces.folderFor({ project: held.project.id });
    const turn = await held.rt.sessions.start(at.workspace.id, { prompt: "start the dev server", harness: "claude" });
    const threadId = turn.view().threadId!;
    await until(async () => (await held!.rt.sessions.list(at.workspace.id)).every(s => s.status !== "running"));
    // Stamped once the host names the shell; a host that restarted over it names it no more, so it stands unowned.
    const owner = o.stamped === false ? {} : { kind: "terminal" as const, owner: { by: "root" as const, thread: threadId } };
    held.seen.procs = [
      row(7100, 1, "node", "node /root/spoo-ts/node_modules/.bin/vite", threadCgroup(threadId)),
      { ...row(9100, 1, "bash", "-bash", "/user.slice"), pty: "pty_1", ...owner },
      { ...row(9110, 9100, "node", "node /root/spoo-ts/node_modules/.bin/vite --port 5199", "/user.slice"), ...owner },
    ];
    await store.put("pty-threads", at.workspace.id, { pty_1: threadId });
    return { ...held, threadId, killed };
  }

  it("settle asks about the shell, and stop ends it over pty.kill with the cgroup, leaving nothing standing", async () => {
    const b = await withShell();
    const refused = await b.rt.sessions.settle([b.threadId], undefined, { processes: "ask" }).then(() => "", (e: unknown) => (e as Error).message);
    expect(refused).toContain(`9100 bash (terminal) of ${b.threadId.slice(0, 8)} on hetzner`);
    const answer = await b.rt.sessions.settle([b.threadId], undefined, { processes: "stop" });
    expect(answer.ended).toEqual([expect.objectContaining({ left: [] })]);
    expect(b.killed).toEqual(["pty_1"]);
    expect(pids(b.seen.procs)).toEqual([]);
  });

  it("settle with stop kills no shell it did not list, though the host's state tags it with the thread", async () => {
    const b = await withShell({ stamped: false });
    const refused = await b.rt.sessions.settle([b.threadId], undefined, { processes: "ask" }).then(() => "", (e: unknown) => (e as Error).message);
    expect(refused).toContain(`still runs a process: 7100 node (dev) of ${b.threadId.slice(0, 8)} on hetzner`);
    const answer = await b.rt.sessions.settle([b.threadId], undefined, { processes: "stop" });
    expect(answer.ended).toEqual([expect.objectContaining({ pid: 7100, left: [] })]);
    expect(b.killed).toEqual([]);
    expect(pids(b.seen.procs)).toEqual([9100, 9110]);
  });

  it("settle with keep leaves the shell running", async () => {
    const b = await withShell();
    await b.rt.sessions.settle([b.threadId], undefined, { processes: "keep" });
    expect(b.killed).toEqual([]);
    expect(pids(b.seen.procs)).toEqual([7100, 9100, 9110]);
  });

  it("delete with stop ends the shell, and its answer names nothing still standing", async () => {
    const b = await withShell();
    const answer = await b.rt.sessions.delete(b.threadId, undefined, { processes: "stop" });
    expect(answer.ended).toEqual([expect.objectContaining({ left: [] })]);
    expect(b.killed).toContain("pty_1");
    expect(pids(b.seen.procs)).toEqual([]);
  });

  it("delete with keep leaves the shell and what runs in it", async () => {
    const b = await withShell();
    await b.rt.sessions.delete(b.threadId, undefined, { processes: "keep" });
    expect(b.killed).toEqual([]);
    expect(pids(b.seen.procs)).toEqual([7100, 9100, 9110]);
  });
});
