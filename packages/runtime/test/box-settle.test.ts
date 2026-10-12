// SPDX-License-Identifier: AGPL-3.0-only
// Settle and delete on a thread in a folder on a box the person joined: what it
// left running there is read off its cgroup, asked about by name, stopped
// through the cgroup end the box already runs, or kept running; another
// thread's cgroup on the same box is never read or ended.
import { describe, expect, it } from "vitest";
import { PROCESSES_LEFT_KIND, processesLeftOf, threadCgroup, type ProcEntry } from "@wsp/protocol";
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
    expect((refused as Error).message).toContain(`7100 node (other, :5173) of ${b.threadId.slice(0, 8)} on hetzner`);
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
