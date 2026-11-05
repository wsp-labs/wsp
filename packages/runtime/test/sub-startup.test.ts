// SPDX-License-Identifier: AGPL-3.0-only
import { expect, it } from "vitest";
import { subOpening, type SessionRowEvent } from "@wsp/protocol";
import { eventBus } from "../src/types/events.js";
import { subLifeArea } from "../src/machines/sub-life.js";
import type { RuntimeContext } from "../src/context.js";
import type { SessionEntry } from "../src/types/internal.js";
import type { WorkspaceRecord } from "../src/types/wiring.js";
import { fakeClock } from "./fake-clock.js";
import { leadOnThisComputer, PAID } from "./sub-fixture.js";

async function pending(broker = false) {
  const clock = fakeClock();
  const f = await leadOnThisComputer({ clock: clock.clock, ...(broker ? { broker: true, hostGh: true } : {}) });
  f.boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, minute: { limit: 12, used: 12 } } };
  const made = await f.rt.workspaces.create({ project: f.project.id, name: "Pending child", sub: { on: "boat" } }, f.lead);
  let held!: () => void;
  const waiting = new Promise<void>(done => { held = done; });
  const starting = f.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Say hello"), harness: made.sub!.harness, turnToken: f.token, notify: ["me"], title: "Pending child", cwd: made.sub!.folder, onHeld: held }, f.lead)
    .then(handle => ({ handle, error: undefined }), (error: unknown) => ({ handle: undefined, error }));
  await waiting;
  const row = (await f.rt.sessions.list()).find(row => row.workspaceId === made.id)!;
  const release = () => { f.boat.limitsRead = PAID; clock.advance(20_000); };
  return { ...f, made, row, starting, release };
}

it.each(["unread", "read", "marked"])("starts a queued child exactly once when %s before provisioning", async action => {
  const f = await pending();
  try {
    if (action === "read") await f.rt.sessions.read(f.row.threadId!);
    if (action === "marked") await f.rt.sessions.mark([f.row.threadId!], { pinned: true });
    f.release();
    const result = await f.starting;
    expect(result.error).toBeUndefined();
    await result.handle!.finished;
    const rows = (await f.rt.sessions.list()).filter(row => row.workspaceId === f.made.id);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({ threadId: f.row.threadId, status: "completed", parentThreadId: f.threadId, rootThreadId: f.threadId });
    expect(f.boat.creates).toHaveLength(1);
    expect(f.starts.filter(s => s.o.cwd === f.made.sub!.folder)).toHaveLength(1);
    expect(await f.store.list("sub-starts")).toEqual([]);
  } finally { await f.rt.close(); }
});

it.each([false, true])("a refused handoff publishes failure and retains cleanup evidence when cleanup refuses=%s", async refusesCleanup => {
  const f = await pending();
  const pushed: SessionRowEvent[] = [];
  f.rt.events.on("*", e => { if (e.type === "session.row") pushed.push(e); });
  try {
    const create = f.boat.create.bind(f.boat);
    f.boat.create = async (...args) => {
      const machine = await create(...args);
      if (refusesCleanup) machine.kill = async () => { throw new Error("cleanup unavailable"); };
      return machine;
    };
    const start = f.rt.sessions.start.bind(f.rt.sessions);
    f.rt.sessions.start = async (workspace, options, origin) => {
      if (options.as !== undefined) throw new Error("first turn refused after provisioning");
      return start(workspace, options, origin);
    };
    f.release();
    const result = await f.starting;
    expect(String(result.error)).toContain("first turn refused after provisioning");
    const rows = (await f.rt.sessions.list()).filter(row => row.workspaceId === f.made.id);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({ threadId: f.row.threadId, status: "failed", parentThreadId: f.threadId, rootThreadId: f.threadId, failure: "first turn refused after provisioning" });
    expect(rows[0]!.endedAt).toBeTypeOf("number");
    expect(rows[0]!.capped).toBeUndefined();
    await expect.poll(() => pushed.some(e => e.row?.threadId === f.row.threadId && e.row?.status === "failed")).toBe(true);
    expect(f.starts.filter(s => s.o.cwd === f.made.sub!.folder)).toHaveLength(0);
    const machine = f.boat.machines.at(-1)!;
    const record = (await f.rt.workspaces.list()).find(w => w.id === f.made.id)!;
    expect(machine.killed).toBe(!refusesCleanup);
    expect(record.machineId).toBe(refusesCleanup ? machine.id : "");
    expect(record.phase).toBe(refusesCleanup ? "running" : "released");
    expect(await f.store.list("sub-starts")).toEqual([]);
    await f.endLead();
    await expect.poll(() => f.starts.filter(s => s.o.prompt.includes("first turn refused after provisioning")).length).toBe(1);
  } finally { await f.rt.close(); }
});


it.each([false, true])("preserves a read child's identity across a queued host restart with old partial marks=%s", async oldMarks => {
  const f = await pending();
  let again: ReturnType<typeof f.restart> | undefined;
  try {
    await f.rt.sessions.read(f.row.threadId!);
    await f.rt.close();
    if (oldMarks) {
      const index = await f.store.get("sessions", f.made.id) as { threads: Record<string, { parentThreadId?: string; rootThreadId?: string }> };
      delete index.threads[f.row.threadId!]!.parentThreadId;
      delete index.threads[f.row.threadId!]!.rootThreadId;
      await f.store.put("sessions", f.made.id, index);
    }
    const nextClock = fakeClock();
    again = f.restart(nextClock.clock);
    await expect.poll(async () => (await again!.sessions.list()).some(row => row.threadId === f.row.threadId && row.capped !== undefined)).toBe(true);
    f.boat.limitsRead = PAID;
    nextClock.advance(20_000);
    await expect.poll(() => f.starts.filter(s => s.o.cwd === f.made.sub!.folder).length).toBe(1);
    await expect.poll(async () => (await again!.sessions.list()).find(row => row.threadId === f.row.threadId)?.status).toBe("completed");
    expect((await again.sessions.list()).find(row => row.threadId === f.row.threadId)).toMatchObject({ parentThreadId: f.threadId, rootThreadId: f.threadId });
    expect(await f.store.list("sub-starts")).toEqual([]);
  } finally { await again?.close(); await f.rt.close(); }
});

it("does not treat a failed accepted-start record deletion as a refused agent start", async () => {
  const f = await pending();
  try {
    const drop = f.store.delete.bind(f.store);
    f.store.delete = async (collection, id) => {
      if (collection === "sub-starts" && id === f.made.id) throw new Error("start record unavailable");
      return drop(collection, id);
    };
    f.release();
    const result = await f.starting;
    expect(result.error).toBeUndefined();
    await result.handle!.finished;
    expect(f.starts.filter(s => s.o.cwd === f.made.sub!.folder)).toHaveLength(1);
    expect(f.boat.machines.at(-1)!.killed).toBe(false);
    expect((await f.rt.sessions.list()).find(row => row.threadId === f.row.threadId)?.status).toBe("completed");
  } finally { await f.rt.close(); }
});

it.each(["retry", "delete"])("does not let %s claim or restore a computer during failed-start cleanup", async action => {
  const f = await pending();
  let cleaned!: () => void;
  const cleanupWait = new Promise<void>(done => { cleaned = done; });
  let entered!: () => void;
  const cleaning = new Promise<void>(done => { entered = done; });
  try {
    await f.rt.sessions.read(f.row.threadId!);
    const create = f.boat.create.bind(f.boat);
    f.boat.create = async (...args) => {
      const machine = await create(...args);
      const kill = machine.kill.bind(machine);
      let calls = 0;
      machine.kill = async () => {
        if (++calls === 1) { entered(); await cleanupWait; }
        await kill();
      };
      return machine;
    };
    const start = f.rt.sessions.start.bind(f.rt.sessions);
    f.rt.sessions.start = async (workspace, options, origin) => {
      if (options.as !== undefined) throw new Error("first turn refused after provisioning");
      return start(workspace, options, origin);
    };
    f.release();
    await cleaning;
    if (action === "retry") {
      await expect(f.rt.sessions.start(f.made.id, { thread: f.row.threadId!, prompt: "Try again", harness: f.row.harness }, f.lead)).rejects.toThrow("being cleaned up after a failed start");
    } else {
      await f.rt.workspaces.delete(f.made.id);
      expect(await f.store.get("workspaces", f.made.id)).toBeUndefined();
    }
    cleaned();
    expect(String((await f.starting).error)).toContain("first turn refused after provisioning");
    expect(f.starts.filter(s => s.o.cwd === f.made.sub!.folder)).toHaveLength(0);
    if (action === "delete") {
      expect(await f.store.get("workspaces", f.made.id)).toBeUndefined();
      expect((await f.rt.workspaces.list()).some(w => w.id === f.made.id)).toBe(false);
      expect((await f.rt.sessions.list()).some(row => row.workspaceId === f.made.id)).toBe(false);
    } else {
      expect((await f.rt.sessions.list()).find(row => row.threadId === f.row.threadId)?.status).toBe("failed");
    }
  } finally { cleaned(); await f.rt.close(); }
});

it("an accepted retry restores broker access after refused cleanup while a queued row leaves it blocked", async () => {
  const f = await pending(true);
  try {
    const create = f.boat.create.bind(f.boat);
    f.boat.create = async (...args) => {
      const machine = await create(...args);
      machine.kill = async () => { throw new Error("cleanup unavailable"); };
      return machine;
    };
    const start = f.rt.sessions.start.bind(f.rt.sessions);
    f.rt.sessions.start = async (workspace, options, origin) => {
      if (options.as !== undefined) throw new Error("first turn refused before holding");
      return start(workspace, options, origin);
    };
    f.release();
    expect(String((await f.starting).error)).toContain("first turn refused before holding");
    expect(f.brokerSwitch.off.has(f.made.id)).toBe(true);
    const record = await f.store.get("workspaces", f.made.id) as WorkspaceRecord;
    expect(record.sub?.held).toBeUndefined();
    expect(record.sub?.deleteAt).toBeUndefined();
    // Exercise the accepted-turn event separately from asynchronous end-of-turn result reads and naps.
    const bus = eventBus();
    const sessions = new Map<string, SessionEntry>();
    subLifeArea({ live: new Map([[record.id, { record }]]), sessions, bus, opts: { broker: {
      lift: (id: string) => f.brokerSwitch.off.delete(id),
    } } } as unknown as RuntimeContext);
    const retry = { view: { ...f.row, status: "running" }, turnId: "retry", queued: true } as SessionEntry;
    sessions.set("retry", retry);
    bus.emit({ type: "session.held", workspaceId: record.id, threadId: f.row.threadId! });
    expect(f.brokerSwitch.off.has(record.id)).toBe(true);
    delete retry.queued;
    bus.emit({ type: "session.held", workspaceId: record.id, threadId: f.row.threadId! });
    expect(f.brokerSwitch.off.has(record.id)).toBe(false);
  } finally { await f.rt.close(); }
});
