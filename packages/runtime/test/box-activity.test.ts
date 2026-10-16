// SPDX-License-Identifier: AGPL-3.0-only
// wsp activity on a box the person joined: a thread's processes are what stands
// in its cgroup there, read once over the box's link, named by the box; an End
// stays on this computer, since a box ends a thread's processes through its
// cgroup when the thread settles.
import { describe, expect, it, vi } from "vitest";
import { boxEndLine, cloudActivityLine, subOpening, threadCgroup, type ProcEntry } from "@wsp/protocol";
import type { WorkspaceRecord } from "../src/types/wiring.js";
import { answering, joined, type Started } from "./box-fixture.js";
import { leadOnThisComputer } from "./sub-fixture.js";
import { until } from "./until.js";

const row = (pid: number, ppid: number, comm: string, cmdline: string, cgroup: string): ProcEntry => ({ pid, ppid, user: "root", state: "S", comm, cmdline, cpu: 1, rss: 50_000_000, startedAt: 1_760_000_000_000 + pid, cgroup });

/** hetzner holding a finished thread whose agent left vite and its esbuild child in the thread's cgroup, and a process
 * of a cgroup no thread here names beside them. */
async function boxThread() {
  const starts: Started[] = [];
  const held = await joined({ adapters: { claude: answering(starts) } });
  const at = await held.rt.workspaces.folderFor({ project: held.project.id });
  const turn = await held.rt.sessions.start(at.workspace.id, { prompt: "start the dev server", harness: "claude" });
  const threadId = turn.view().threadId!;
  await until(async () => (await held.rt.sessions.list(at.workspace.id)).every(s => s.status !== "running"));
  const mine = threadCgroup(threadId);
  held.seen.procs = [
    row(7100, 1, "node", "node /root/lab/node_modules/.bin/vite", mine),
    row(7101, 7100, "esbuild", "/root/lab/node_modules/esbuild/bin/esbuild --service", mine),
    row(7200, 1, "node", "node /root/else/node_modules/.bin/vite --port 5174", threadCgroup("other-thread")),
  ];
  held.seen.ports = [{ port: 5173, pid: 7100 }];
  return { ...held, threadId };
}

describe("wsp activity on a box", () => {
  it("lists a box's threads by their cgroups there, named by the box, and nothing of a cgroup no thread here names", async () => {
    const b = await boxThread();
    // A clock whose epoch milliseconds hold the digits of the other cgroup's pid.
    const clock = vi.spyOn(Date, "now").mockReturnValue(1_792_172_009_000);
    const view = await b.rt.activity.read({ computer: "hetzner" }).finally(() => clock.mockRestore());
    expect(view.at).toBe(1_792_172_009_000);
    expect(view.computer).toBe("hetzner");
    expect(view.threads.map(t => ({ threadId: t.threadId, pids: t.processes.map(p => p.pid), memory: t.memory }))).toEqual([{ threadId: b.threadId, pids: [7100, 7101], memory: 100_000_000 }]);
    expect(view.threads[0]!.processes[0]!.ports).toEqual([5173]);
    expect([...view.threads.flatMap(t => t.processes), ...view.wsp, ...view.unowned].map(p => p.pid)).not.toContain(7200);
  });

  it("totals a box thread by its cgroup's memory where the daemon read it, not its rows' resident sum", async () => {
    const b = await boxThread();
    b.seen.cgroups = [{ cgroup: threadCgroup(b.threadId), memory: 30_000_000 }];
    for (const asked of [{ computer: "hetzner" }, { thread: b.threadId }]) {
      const view = await b.rt.activity.read(asked);
      expect(view.threads.map(t => [t.threadId, t.memory])).toEqual([[b.threadId, 30_000_000]]);
    }
  });

  it("reads a thread named alone on the box it runs on, with no computer named", async () => {
    const b = await boxThread();
    const view = await b.rt.activity.read({ thread: b.threadId });
    expect(view.computer).toBe("hetzner");
    expect(view.threads.map(t => t.threadId)).toEqual([b.threadId]);
    expect(view.wsp).toEqual([]);
  });

  it("refuses an End on a box, signalling nothing there", async () => {
    const b = await boxThread();
    await expect(b.rt.activity.end({ computer: "hetzner", pid: 7100, startedAt: 1_760_000_007_100 })).rejects.toThrow(boxEndLine("hetzner"));
    expect(b.seen.ops).not.toContain("proc.kill");
    expect(b.seen.procs.map(p => p.pid)).toEqual([7100, 7101, 7200]);
  });

  it("on a box one version behind, with no one-shot list, refuses in the usage class naming the update, for the box and for a thread on it", async () => {
    const b = await boxThread();
    b.seen.listless = true;
    for (const asked of [{ computer: "hetzner" }, { thread: b.threadId }]) {
      const refused = await b.rt.activity.read(asked).then(
        () => undefined,
        (e: unknown) => e,
      );
      expect(refused).toMatchObject({ kind: "usage" });
      expect((refused as Error).message).toContain("hetzner is behind");
      expect((refused as Error).message).toContain("wsp add hetzner --update");
    }
  });

  it("names the box a thread runs on when this host has not joined it, never a cloud machine", async () => {
    const b = await boxThread();
    vi.spyOn(b.rt.places!, "joined").mockReturnValue(false);
    const refused = await b.rt.activity.read({ thread: b.threadId }).then(
      () => undefined,
      (e: unknown) => e,
    );
    expect((refused as Error).message).toContain(`runs on hetzner, which this host has not joined`);
    expect((refused as Error).message).not.toContain("cloud");
  });

  it("says a cloud machine for a thread on a provider this host does not fork on, whose record names that provider's place", async () => {
    const held = await leadOnThisComputer({ spendPerDayUsd: 5 });
    const made = await held.rt.workspaces.create({ project: held.project.id, name: "Fix it", sub: { on: "boat" } }, held.lead);
    const turn = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", turnToken: held.token, cwd: made.sub!.folder }, held.lead);
    await turn.finished;
    await held.rt.close();
    // As placeFor() records a machine on any provider but the wired one.
    await held.store.put("workspaces", made.id, { ...((await held.store.get("workspaces", made.id)) as WorkspaceRecord), place: "box" });
    const rt = held.restart();
    const refused = await rt.activity.read({ thread: turn.view().threadId! }).then(
      () => undefined,
      (e: unknown) => e as Error,
    );
    await rt.close();
    expect(refused?.message).toContain(cloudActivityLine(turn.view().threadId!));
  });
});
