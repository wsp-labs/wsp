// SPDX-License-Identifier: AGPL-3.0-only
// A wake the provider refuses because it deleted the paused machine's disk, as
// Boat answers a resume past the 14 days it keeps one (409 snapshot_data_expired):
// a workspace goes onto a new machine from its image in that same wake, tells
// each thread on it why and keeps the line on its record for every row and
// answer until the next nap, and never reads napping over a disk that is gone.
import { describe, expect, it, vi } from "vitest";
import { DiskGoneError } from "@wsp/engine";
import { BOAT_DISK_GONE_LINE, diskGoneReplacedLine, foldThreads, type EventUnion, type TurnResult } from "@wsp/protocol";
import { createRuntime, type HarnessAdapterFactory } from "../src/runtime.js";
import { memoryStore } from "../src/store.js";
import { fakeClock } from "./fake-clock.js";
import { createOn, stubBackend } from "./stub-backend.js";

const SAID = "The sandbox stayed stopped for 14 days. Boat deleted its data.";
const LINE = diskGoneReplacedLine(BOAT_DISK_GONE_LINE);

/** A turn that answers at once, so a thread stands on the machine with no turn running. */
const answers: HarnessAdapterFactory = () => ({
  steers: false,
  start: ({ onEvent }) => {
    const sessionId = "33333333-3333-4333-8333-333333333333";
    const result: TurnResult = { status: "completed", text: "built" };
    onEvent({ type: "session.start", sessionId });
    onEvent({ type: "turn.done", sessionId, result });
    onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
    return { localId: sessionId, finished: Promise.resolve(result), interrupt: async () => {} };
  },
});

function rig(o: { diskPause?: true } = {}) {
  const backend = stubBackend();
  // A provider whose pause keeps the disk and that seals images, as Boat is: a rebuild of a paused machine wakes it.
  if (o.diskPause === true) Object.assign(backend.capabilities, { pauseMode: "disk", images: true });
  const fc = fakeClock();
  const store = memoryStore();
  const open = () => createRuntime({ backend, store, adapters: { claude: answers }, clock: fc.clock });
  const rt = open();
  const events: EventUnion[] = [];
  rt.events.on("*", e => events.push(e));
  return { backend, rt, events, open };
}

describe("a wake whose disk the provider deleted", () => {
  it("starts a new machine from the image in the same wake, retires the old one and says the disk is gone on the row", async () => {
    const { backend, rt, events } = rig();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const ws = await createOn(rt, { golden: "snap_g", name: "b1" });
      await rt.workspaces.nap(ws.id);
      const old = backend.machines[0]!;
      let resumes = 0;
      old.resume = async () => {
        resumes++;
        throw new DiskGoneError(old.id, BOAT_DISK_GONE_LINE, SAID);
      };
      const woken = await rt.workspaces.wake(ws.id);
      expect(resumes).toBe(1);
      expect(woken.phase).toBe("running");
      expect(backend.machines).toHaveLength(2);
      expect(woken.machineId).toBe(backend.machines[1]!.id);
      expect(old.killed).toBe(true);
      expect((await rt.workspaces.get(ws.id)).machineId).toBe(backend.machines[1]!.id);
      const reasons = events.flatMap(e => (e.type === "workspace.status" && e.status.reason !== undefined ? [e.status.reason] : []));
      expect(reasons.at(-1)).toBe(diskGoneReplacedLine(BOAT_DISK_GONE_LINE));
      expect(reasons.at(-1)).toBe("Boat deleted this machine's disk once it had been paused for 14 days, so it cannot wake; it runs on a new machine from the image now");
      // The next wake has nothing to resume: the new machine runs.
      expect((await rt.workspaces.wake(ws.id)).machineId).toBe(backend.machines[1]!.id);
      expect(backend.machines).toHaveLength(2);
    } finally {
      warn.mockRestore();
      await rt.close();
    }
  });

  it("a new machine that does not start leaves the record napping on the old one and says both", async () => {
    const { backend, rt } = rig();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const ws = await createOn(rt, { golden: "snap_g", name: "b1" });
      await rt.workspaces.nap(ws.id);
      const old = backend.machines[0]!;
      old.resume = async () => {
        throw new DiskGoneError(old.id, BOAT_DISK_GONE_LINE, SAID);
      };
      backend.create = async () => {
        throw new Error("no machine");
      };
      await expect(rt.workspaces.wake(ws.id)).rejects.toThrow(`${BOAT_DISK_GONE_LINE}, and no new machine started: no machine`);
      const after = await rt.workspaces.get(ws.id);
      expect(after.phase).toBe("napping");
      expect(after.machineId).toBe(old.id);
    } finally {
      warn.mockRestore();
      await rt.close();
    }
  });

  it("tells each thread on the workspace in its own timeline, where the person reads it", async () => {
    const { backend, rt, events } = rig();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const ws = await createOn(rt, { golden: "snap_g", name: "b1" });
      const turn = await rt.sessions.start(ws.id, { prompt: "build" });
      await turn.finished;
      await rt.workspaces.nap(ws.id);
      const old = backend.machines[0]!;
      old.resume = async () => {
        throw new DiskGoneError(old.id, BOAT_DISK_GONE_LINE, SAID);
      };
      await rt.workspaces.wake(ws.id);
      const told = events.flatMap(e => (e.type === "session.notice" ? [{ thread: e.threadId, text: e.text }] : []));
      expect(told).toEqual([{ thread: turn.view().threadId, text: LINE }]);
    } finally {
      warn.mockRestore();
      await rt.close();
    }
  });

  it("answers the wake with the line, and every row of its threads carries it until the next nap", async () => {
    const { backend, rt } = rig();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const ws = await createOn(rt, { golden: "snap_g", name: "b1" });
      const turn = await rt.sessions.start(ws.id, { prompt: "build" });
      await turn.finished;
      const threadId = turn.view().threadId!;
      await rt.workspaces.nap(ws.id);
      const old = backend.machines[0]!;
      old.resume = async () => {
        throw new DiskGoneError(old.id, BOAT_DISK_GONE_LINE, SAID);
      };
      expect((await rt.workspaces.wake(ws.id)).diskGone).toBe(LINE);
      expect((await rt.workspaces.get(ws.id)).diskGone).toBe(LINE);
      const rows = async () => (await rt.sessions.list()).filter(r => r.threadId === threadId);
      expect((await rows()).at(-1)!.diskGone).toBe(LINE);
      expect(foldThreads(await rows())[0]!.diskGone).toBe(LINE);
      await rt.workspaces.nap(ws.id);
      expect((await rt.workspaces.get(ws.id)).diskGone).toBeUndefined();
      expect((await rows()).at(-1)!.diskGone).toBeUndefined();
    } finally {
      warn.mockRestore();
      await rt.close();
    }
  });

  it("a host started over the new machine, which its cloud paused while no host was up, reads the nap and no line", async () => {
    const { backend, rt, open } = rig();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    let again: ReturnType<typeof open> | undefined;
    try {
      const ws = await createOn(rt, { golden: "snap_g", name: "b1" });
      await rt.workspaces.nap(ws.id);
      const old = backend.machines[0]!;
      old.resume = async () => {
        throw new DiskGoneError(old.id, BOAT_DISK_GONE_LINE, SAID);
      };
      expect((await rt.workspaces.wake(ws.id)).diskGone).toBe(LINE);
      await rt.close();
      backend.machines[1]!.paused = true;
      again = open();
      const napped = await again.workspaces.get(ws.id);
      expect(napped.phase).toBe("napping");
      expect(napped.diskGone).toBeUndefined();
      const woken = await again.workspaces.wake(ws.id);
      expect(woken.machineId).toBe(backend.machines[1]!.id);
      expect(woken.diskGone).toBeUndefined();
    } finally {
      warn.mockRestore();
      await (again ?? rt).close();
    }
  });

  it("a rebuild of a paused workspace whose disk is gone makes one new machine, not two", async () => {
    const { backend, rt } = rig({ diskPause: true });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const ws = await createOn(rt, { golden: "snap_g", name: "b1" });
      await rt.workspaces.nap(ws.id);
      const old = backend.machines[0]!;
      old.resume = async () => {
        throw new DiskGoneError(old.id, BOAT_DISK_GONE_LINE, SAID);
      };
      const rebuilt = await rt.workspaces.rebuild(ws.id);
      expect(backend.machines).toHaveLength(2);
      expect(backend.machines.map(m => m.killed)).toEqual([true, false]);
      expect(rebuilt.machineId).toBe(backend.machines[1]!.id);
      expect(rebuilt.phase).toBe("running");
    } finally {
      warn.mockRestore();
      await rt.close();
    }
  });
});
