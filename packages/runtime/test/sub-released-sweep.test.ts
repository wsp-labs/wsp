// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread whose machine is released holds no machine to ask: the sweep
// leaves it, so no "is not gone" line repeats every pass. A record that still
// names a machine whose reads fail another way is read again at each sweep.
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Clock } from "../src/clock.js";
import type { Runtime } from "../src/runtime.js";
import { PUSHED, answering, jumpClock, leadOnThisComputer, until, type Answers } from "./sub-fixture.js";
import type { WorkspaceRecord } from "../src/types/wiring.js";
import { subOpening } from "@wsp/protocol";

const MIN = 60_000;
let open: Runtime[] = [];
afterEach(async () => {
  for (const rt of open) await rt.close().catch(() => undefined);
  open = [];
});

async function doneOnce(clock?: Clock) {
  const answers: Answers = { read: PUSHED, session: Buffer.from("gzipped session bytes") };
  const held = await leadOnThisComputer({ ...(clock !== undefined ? { clock } : {}), spendPerDayUsd: 5 });
  open.push(held.rt);
  answering(held.boat, answers);
  const made = await held.rt.workspaces.create({ project: held.project.id, name: "Fix it", sub: { on: "boat" } }, held.lead);
  const child = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", notify: ["me"], turnToken: held.token, title: "Fix it", cwd: made.sub!.folder }, held.lead);
  const machine = held.boat.machines.at(-1)!;
  const threadId = child.view().threadId!;
  await child.finished;
  const record = (): Promise<WorkspaceRecord> => held.store.get("workspaces", made.id) as Promise<WorkspaceRecord>;
  await until(async () => (await record()).sub?.result, "the done read");
  return { ...held, made, machine, threadId, record };
}

const notGone = (warn: ReturnType<typeof vi.spyOn>) => warn.mock.calls.map(c => String(c[0])).filter(l => l.includes("is not gone"));

describe("the sweep over a sub-thread", () => {
  it("settles a released one once: no 'is not gone' line at any pass, the thread kept", async () => {
    const { clock, jump } = jumpClock();
    const held = await doneOnce(clock);
    jump(31 * MIN);
    const released = await until(async () => ((await held.record()).phase === "released" ? held.record() : undefined), "the release");
    expect(released.machineId).toBe("");
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      await held.rt.reap();
      await held.rt.reap();
      expect(notGone(warn)).toEqual([]);
      expect((await held.record()).phase).toBe("released");
      expect((await held.rt.sessions.list()).find(s => s.threadId === held.threadId)?.sub?.state).toBe("released");
    } finally {
      warn.mockRestore();
    }
  });

  it("reads a record that names a machine again at each sweep when the reads after the 404 fail another way", async () => {
    const held = await doneOnce();
    await until(async () => ((await held.record()).phase === "napping" ? true : undefined), "the nap");
    const m = held.machine;
    const list = held.boat.list.bind(held.boat);
    held.boat.list = async labels => (await list(labels)).filter(r => r.id !== m.id);
    m.state = async () => {
      throw Object.assign(new Error("Sandbox not found"), { kind: "missing", status: 404 });
    };
    const get = held.boat.get.bind(held.boat);
    held.boat.get = async id => {
      if (id === m.id) throw Object.assign(new Error("Bad gateway"), { kind: "provider", status: 502 });
      return get(id);
    };
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      await held.rt.reap();
      await held.rt.reap();
      expect(notGone(warn)).toHaveLength(2);
      expect(notGone(warn)[0]).toContain("the reads that followed failed: Bad gateway");
      expect((await held.record()).phase).toBe("napping");
    } finally {
      warn.mockRestore();
    }
  });
});
