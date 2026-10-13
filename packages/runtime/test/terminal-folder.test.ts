// SPDX-License-Identifier: AGPL-3.0-only
// A pane's terminal on a machine whose daemon is its own: a sub-thread on Boat
// and a cloud fork. Their daemon opens a pty that names no folder in its home,
// so the host names the folder the thread works in. The daemon is the
// fixture's fake that keeps every frame.
import { afterEach, describe, expect, it } from "vitest";
import { tokenGuest } from "./stub-backend.js";
import { leadOnThisComputer } from "./sub-fixture.js";

type Held = Awaited<ReturnType<typeof leadOnThisComputer>>;
let held: Held | undefined;
afterEach(async () => {
  await held?.endLead();
  await held?.rt.close();
  held = undefined;
});

/** The pty.create frames the daemon got after a pane on the workspace sent each frame in `sent`. */
async function created(h: Held, workspaceId: string, sent: Record<string, unknown>[]): Promise<Record<string, unknown>[]> {
  // The fake answers whatever address it is dialled at, so the address is only one the cloud road can read.
  for (const m of h.boat.machines) m.previewUrl = async () => ({ url: "ws://127.0.0.1:1", token: "e", expiresAt: Date.now() + 3_600_000 });
  const before = h.daemon.frames.length;
  const channel = await h.rt.workspaces.daemonChannel(workspaceId, () => {});
  for (const frame of sent) await channel.send({ op: "pty.create", cols: 80, rows: 24, ...frame } as never);
  channel.close();
  return h.daemon.frames.slice(before).filter(f => f["op"] === "pty.create");
}

describe("a pane's terminal on a machine whose daemon is its own", () => {
  it("opens any tab that names no folder in the sub-thread's folder on Boat, and keeps a folder the pane named", async () => {
    held = await leadOnThisComputer();
    held.boat.execImpl = tokenGuest;
    const made = await held.rt.workspaces.create({ project: held.project.id, name: "k6 full suite", sub: { on: "boat" } }, held.lead);
    expect(made.folder).toBe("/root/web");
    const frames = await created(held, made.id, [{ thread: "thr_a" }, {}, { cwd: "/tmp" }]);
    expect(frames.map(f => f["cwd"])).toEqual(["/root/web", "/root/web", "/tmp"]);
    expect(frames[0]).not.toHaveProperty("thread");
  });

  it("opens a cloud fork's first tab in the project's folder inside the fork", async () => {
    held = await leadOnThisComputer();
    held.boat.execImpl = tokenGuest;
    const project = await held.rt.projects.add({ source: "https://github.com/acme/lab.git", on: "boat", name: "lab" });
    const fork = await held.rt.workspaces.create({ project: project.id, name: "lab" });
    const frames = await created(held, fork.id, [{}]);
    expect(frames.map(f => f["cwd"])).toEqual([project.path]);
  });
});
