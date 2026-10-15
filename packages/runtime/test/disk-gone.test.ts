// SPDX-License-Identifier: AGPL-3.0-only
// A wake the provider refuses because it deleted the paused machine's disk, as
// Boat answers a resume past the 14 days it keeps one (409 snapshot_data_expired):
// a workspace goes onto a new machine from its image in that same wake, says why
// on its row, and never reads napping over a disk that is gone.
import { describe, expect, it, vi } from "vitest";
import { DiskGoneError } from "@wsp/engine";
import { BOAT_DISK_GONE_LINE, diskGoneReplacedLine, type EventUnion } from "@wsp/protocol";
import { createRuntime } from "../src/runtime.js";
import { memoryStore } from "../src/store.js";
import { fakeClock } from "./fake-clock.js";
import { createOn, stubBackend } from "./stub-backend.js";

const SAID = "The sandbox stayed stopped for 14 days. Boat deleted its data.";

function rig() {
  const backend = stubBackend();
  const fc = fakeClock();
  const rt = createRuntime({ backend, store: memoryStore(), adapters: {}, clock: fc.clock });
  const events: EventUnion[] = [];
  rt.events.on("*", e => events.push(e));
  return { backend, rt, events };
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
});
