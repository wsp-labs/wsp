// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's spawn past Boat's machines at once or its minute of starts
// waits rather than failing (05, Q4 to Q9): its record is queued with no
// machine, its thread's first turn is answered held and reads Waiting, and the
// queue starts the spawns first in, first out when a slot frees or a minute
// turns. A stop drops a held spawn with no start spent, and a restarted host
// starts it all the same. Boat is a stub backend that keeps every create.
import { describe, expect, it } from "vitest";
import { SUB_DROPPED_LINE, spawnCapRefusal, subOpening, type Caller, type ThreadCapWait } from "@wsp/protocol";
import type { Runtime } from "../src/runtime.js";
import { fakeClock } from "./fake-clock.js";
import { NAME_LABEL, type MachineSpec } from "@wsp/engine";
import { PAID, leadOnThisComputer } from "./sub-fixture.js";

const TASK = "Fix the flaky upload test in packages/upload.";

/** A sub-thread's machine and its first turn, as `run --on boat --detach --notify me` asks for them: the turn answered
 * held where the queue holds it, with the count it waits on. */
async function spawnAndStart(rt: Runtime, project: string, lead: Caller, token: string, name: string) {
  return startOf(rt, await rt.workspaces.create({ project, name, sub: { on: "boat" } }, lead), lead, token, name);
}

/** The start of a spawn's thread, answered held where it waits, else once it starts. */
async function startOf(rt: Runtime, made: Awaited<ReturnType<Runtime["workspaces"]["create"]>>, lead: Caller, token: string, name: string) {
  let held: ThreadCapWait | undefined;
  let answered!: () => void;
  const heard = new Promise<void>(r => (answered = r));
  const started = rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, TASK), harness: made.sub!.harness, notify: ["me"], turnToken: token, title: name, cwd: made.sub!.folder, onHeld: h => ((held = h.wait), answered()) }, lead);
  void started.then(answered, answered);
  await heard;
  return { made, started, held: () => held };
}

const recordOf = async (rt: Runtime, id: string) => (await rt.workspaces.list()).find(w => w.id === id);

describe("a spawn at Boat's machines at once waits for a slot", () => {
  it("is held as queued and answered held with capped, then starts by itself, first in first out, as sub-threads are done", async () => {
    const { rt, turns, store, boat, starts, project, lead, token, threadId } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 5 } });
    // A done sub-thread naps its machine at once, so the first one works on, holding its slot, until it is ended.
    turns.hold = true;
    const a = await spawnAndStart(rt, project.id, lead, token, "First");
    expect(a.held()).toBeUndefined();
    await a.started;
    expect(boat.creates).toHaveLength(1);

    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    expect(boat.creates).toHaveLength(1);
    expect(b.held()).toEqual({ placeId: "box", place: "Boat", running: 1, atOnce: 1, why: "slot" });
    const stored = (await store.get("workspaces", b.made.id)) as Record<string, unknown>;
    expect(stored).toMatchObject({ phase: "queued", machineId: "", queue: { why: "slot", since: expect.any(String) } });
    // Its thread reads Waiting: a running row carrying the count, under the lead.
    const row = (await rt.sessions.list()).find(r => r.workspaceId === b.made.id)!;
    expect(row).toMatchObject({ status: "running", capped: { why: "slot", running: 1, atOnce: 1 }, parentThreadId: threadId, rootThreadId: threadId });

    // Its status asks Boat nothing, and a machine id of none never reads as a machine gone.
    expect((await rt.status.list()).find(w => w.id === b.made.id)).toMatchObject({ phase: "queued", machineState: "starting", reach: { state: "napping" }, queue: { why: "slot" } });

    const c = await spawnAndStart(rt, project.id, lead, token, "Third");
    expect(c.held()).toMatchObject({ why: "slot" });
    expect(boat.creates).toHaveLength(1);

    // The first sub-thread is done and its machine naps: the second spawn, and only it, starts on a machine of its own,
    // and works on in turn.
    turns.hold = true;
    turns.release!();
    await expect.poll(() => boat.creates.length).toBe(2);
    const second = await b.started;
    expect(second.view().threadId).toBe(row.threadId);
    await expect.poll(() => starts.some(s => s.o.prompt.startsWith("This thread runs on a machine") && s.o.cwd === "/root/web" && starts.indexOf(s) > 1)).toBe(true);
    expect((await recordOf(rt, b.made.id))?.phase).toBe("running");
    expect((await recordOf(rt, c.made.id))?.phase).toBe("queued");

    turns.release!();
    await expect.poll(() => boat.creates.length).toBe(3);
    await c.started;
    expect((await recordOf(rt, c.made.id))?.phase).not.toBe("queued");
    expect(await store.list("sub-starts")).toEqual([]);
  });
});

describe("the minute of starts, a stop, a restart and the counts", () => {
  it("holds a spawn with no start left in the minute, and starts it when a read shows one", async () => {
    const fc = fakeClock();
    const { rt, boat, project, lead, token } = await leadOnThisComputer({ clock: fc.clock });
    boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, minute: { limit: 12, used: 12 } } };
    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    expect(b.held()).toEqual({ placeId: "box", place: "Boat", running: 12, atOnce: 12, why: "minute" });
    expect((await recordOf(rt, b.made.id))?.queue?.why).toBe("minute");
    expect(boat.creates).toEqual([]);
    // Still used at the next read: nothing starts.
    fc.advance(10_000);
    await new Promise(r => setTimeout(r, 20));
    expect(boat.creates).toEqual([]);
    // The minute turns at Boat; the next read sees a start left and the spawn starts on its own.
    boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, minute: { limit: 12, used: 0 } } };
    fc.advance(10_000);
    await expect.poll(() => boat.creates.length).toBe(1);
    await b.started;
    // Its turn is over at once here, so its machine may already nap.
    expect((await recordOf(rt, b.made.id))?.phase).not.toBe("queued");
  });

  it("drops a held spawn whose lead is stopped: record, row and written start gone, no start spent", async () => {
    const { rt, store, boat, project, lead, token, threadId, endLead } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 5 } });
    // Boat's own machines at once is full with boxes of another client's, so nothing under the lead is working.
    boat.limitsRead = { ...PAID, machinesAtOnce: 1, activeBoxes: 1 };
    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    expect(b.held()).toMatchObject({ why: "slot", running: 1, atOnce: 1 });
    expect(await store.list("sub-starts")).toHaveLength(1);
    const leadRow = (await rt.sessions.list()).find(r => r.threadId === threadId && r.status === "running")!;
    const subThread = (await rt.sessions.list()).find(r => r.workspaceId === b.made.id)!.threadId;
    // The lead's own agent here ends its turn only when told, so its stop is answered once it is.
    const stopping = rt.sessions.interrupt(leadRow.id);
    await expect(b.started).rejects.toThrow(SUB_DROPPED_LINE);
    await endLead();
    expect((await stopping).under).toEqual([subThread]);
    expect(await store.get("workspaces", b.made.id)).toBeUndefined();
    expect(await store.list("sub-starts")).toEqual([]);
    expect((await rt.sessions.list()).some(r => r.workspaceId === b.made.id)).toBe(false);
    // A slot freeing later starts nothing for it.
    boat.limitsRead = PAID;
    await rt.places!.set("box", { machines: 2 });
    await new Promise(r => setTimeout(r, 50));
    expect(boat.creates).toEqual([]);
  });


  it("a held spawn's record deleted goes with its start: nothing asked of Boat, and its caller told", async () => {
    const { rt, turns, store, boat, project, lead, token } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 5 } });
    // A done sub-thread naps its machine at once, so the first one works on, holding its slot, until it is ended.
    turns.hold = true;
    const a = await spawnAndStart(rt, project.id, lead, token, "First");
    await a.started;
    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    await rt.workspaces.delete(b.made.id);
    await expect(b.started).rejects.toThrow("Second was deleted before this turn started");
    expect(await store.get("workspaces", b.made.id)).toBeUndefined();
    expect(await store.list("sub-starts")).toEqual([]);
    turns.release!();
    await new Promise(r => setTimeout(r, 50));
    expect(boat.creates).toHaveLength(1);
  });

  it("a held spawn's row reads waiting, and the lead deletes its sub-thread off the queue with no start spent", async () => {
    const { rt, turns, store, boat, project, lead, token } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 5 } });
    // A done sub-thread naps its machine at once, so the first one works on, holding its slot, until it is ended.
    turns.hold = true;
    const a = await spawnAndStart(rt, project.id, lead, token, "First");
    await a.started;
    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    const row = (await rt.sessions.list()).find(r => r.workspaceId === b.made.id)!;
    expect(row.sub?.state).toBe("waiting");
    expect(await rt.sessions.delete(row.threadId!, lead)).toMatchObject({ workspaceId: b.made.id, machine: "deleted" });
    await expect(b.started).rejects.toThrow("Second was deleted before this turn started");
    expect(await store.get("workspaces", b.made.id)).toBeUndefined();
    turns.release!();
    await new Promise(r => setTimeout(r, 50));
    expect(boat.creates).toHaveLength(1);
  });

  it("survives a host restart: the queued record and its start come back Waiting and start when a slot frees", async () => {
    const { rt, turns, make, store, boat, starts, project, lead, token } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 5 } });
    // A done sub-thread naps its machine at once, so the first one works on, holding its slot, until it is ended.
    turns.hold = true;
    const a = await spawnAndStart(rt, project.id, lead, token, "First");
    await a.started;
    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    const before = (await rt.sessions.list()).find(r => r.workspaceId === b.made.id)!;
    await rt.close();
    const again = make();
    expect((await recordOf(again, b.made.id))).toMatchObject({ phase: "queued", queue: { why: "slot" } });
    await expect.poll(async () => (await again.sessions.list()).find(r => r.workspaceId === b.made.id)?.capped).toEqual({ placeId: "box", place: "Boat", running: 1, atOnce: 1, why: "slot" });
    const row = (await again.sessions.list()).find(r => r.workspaceId === b.made.id)!;
    expect(row).toMatchObject({ threadId: before.threadId, status: "running" });
    const onBoat = () => starts.filter(s => s.o.cwd === "/root/web");
    expect(onBoat()).toHaveLength(1);
    await again.workspaces.nap(a.made.id);
    await expect.poll(() => boat.creates.length).toBe(2);
    await expect.poll(() => onBoat().length).toBe(2);
    expect(onBoat()[1]!.o.prompt.endsWith(TASK)).toBe(true);
    await expect.poll(async () => (await again.sessions.list()).find(r => r.threadId === before.threadId)?.workspaceId).toBe(b.made.id);
    expect(await store.list("sub-starts")).toEqual([]);
  });

  it("a restart while a let-through spawn's machine was being made puts it back in line, and it starts", async () => {
    const { rt, turns, make, store, boat, starts, project, lead, token } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 5 } });
    // A done sub-thread naps its machine at once, so the first one works on, holding its slot, until it is ended.
    turns.hold = true;
    const a = await spawnAndStart(rt, project.id, lead, token, "First");
    await a.started;
    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    await rt.close();
    // The old host had let it through and gone down before its machine stood: queued, its wait over.
    const { queue: _queue, ...starting } = (await store.get("workspaces", b.made.id)) as Record<string, unknown>;
    await store.put("workspaces", b.made.id, starting);
    const again = make();
    await expect.poll(async () => (await again.sessions.list()).find(r => r.workspaceId === b.made.id)?.capped?.why).toBe("slot");
    await again.workspaces.nap(a.made.id);
    await expect.poll(() => boat.creates.length).toBe(2);
    await expect.poll(() => starts.filter(s => s.o.cwd === "/root/web").length).toBe(2);
  });

  it("counts a queued spawn against its root's machines and a released machine not at all (04, L2)", async () => {
    const { rt, turns, make, store, boat, project, lead, token, threadId } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 2 } });
    // A done sub-thread naps its machine at once, so the first one works on, holding its slot, until it is ended.
    turns.hold = true;
    const a = await spawnAndStart(rt, project.id, lead, token, "First");
    await a.started;
    await spawnAndStart(rt, project.id, lead, token, "Second");
    // Two under the root, the queued one among them: a third is refused at the root's cap.
    const refused = await rt.workspaces.create({ project: project.id, name: "Third", sub: { on: "boat" } }, lead).then(() => undefined, (e: unknown) => (e as Error).message);
    expect(refused).toBe(spawnCapRefusal(threadId, 2, 2));
    // The first machine released: it counts no more, and the third waits behind the second.
    await store.put("workspaces", a.made.id, { ...((await store.get("workspaces", a.made.id)) as object), phase: "released", machineId: "" });
    await rt.close();
    // The new host lets the second through at boot. Its machine is never made here, so it holds the slot when the
    // third is asked; made, its turn would end at once and nap it, freeing the slot before the third is counted.
    boat.create = () => new Promise<never>(() => {});
    const again = make();
    const third = await again.workspaces.create({ project: project.id, name: "Third", sub: { on: "boat" } }, lead);
    expect((await recordOf(again, third.id))).toMatchObject({ phase: "queued" });
    expect((await recordOf(again, a.made.id))?.phase).toBe("released");
  });
});

describe("a create Boat refuses at its machines at once", () => {
  it("puts the spawn back at the head of the queue, not the tail, and asks again a read later (Q9)", async () => {
    const fc = fakeClock();
    const { rt, boat, project, lead, token } = await leadOnThisComputer({ clock: fc.clock, machines: 1, agents: { spawn: true, maxMachines: 5 } });
    // Boat's machines at once is full with another client's box.
    boat.limitsRead = { ...PAID, machinesAtOnce: 1, activeBoxes: 1 };
    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    const c = await spawnAndStart(rt, project.id, lead, token, "Third");
    // Another client takes Boat's last slot between the read and the create.
    const create = boat.create.bind(boat);
    let refuse = 1;
    boat.create = async (spec: MachineSpec) => {
      if (refuse-- > 0) {
        boat.creates.push(spec);
        throw Object.assign(new Error("Box limit reached"), { kind: "concurrency", status: 429, code: "limit_reached" });
      }
      return create(spec);
    };
    const named = () => boat.creates.map(s => s.labels?.[NAME_LABEL]);
    // Boat reads a slot free; a setting moving runs the queue at once.
    boat.limitsRead = PAID;
    await rt.places!.set("box", { spendPerDayUsd: 20 });
    await expect.poll(named).toEqual(["Second"]);
    await new Promise(r => setTimeout(r, 20));
    // The third is not let through past it while it waits its read.
    expect(named()).toEqual(["Second"]);
    expect(await recordOf(rt, b.made.id)).toMatchObject({ phase: "queued", queue: { why: "slot" } });
    // It waits again; it did not fail.
    expect((await rt.sessions.list()).find(r => r.workspaceId === b.made.id)?.sub?.spawn?.failed).toBeUndefined();
    expect((await rt.workspaces.list()).filter(w => w.queue !== undefined).sort((x, y) => x.queue!.since.localeCompare(y.queue!.since)).map(w => w.name)).toEqual(["Second", "Third"]);
    fc.advance(30_000);
    await expect.poll(() => named().slice(0, 2)).toEqual(["Second", "Second"]);
    await b.started;
    expect(c.held()).toMatchObject({ why: "slot" });
  });
});

/** Boat's create answers only after `ms`, as a real one takes seconds: a second spawn asked meanwhile reads the count. */
function slowCreates(boat: { create: (spec: MachineSpec) => Promise<unknown> }, ms: number): void {
  const create = boat.create.bind(boat);
  boat.create = async (spec: MachineSpec) => {
    await new Promise(r => setTimeout(r, ms));
    return create(spec);
  };
}

describe("spawns asked together, and a spawn whose start never comes", () => {
  it("two spawns asked at the same moment under --machines 1: one machine, the other queued", async () => {
    const { rt, turns, store, boat, project, lead, token } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 5 } });
    turns.hold = true;
    slowCreates(boat, 200);
    const both = await Promise.all(["First", "Second"].map(name => spawnAndStart(rt, project.id, lead, token, name)));
    expect(boat.creates).toHaveLength(1);
    expect(both.map(b => b.held()?.why ?? "none").sort()).toEqual(["none", "slot"]);
    const phases = await Promise.all(both.map(async b => ((await store.get("workspaces", b.made.id)) as { phase: string }).phase));
    expect(phases.sort()).toEqual(["queued", "running"]);
  });

  it("answers held a start that came with room and met the wait in the queue, where a spawn beside it took the slot", async () => {
    const { rt, turns, boat, project, lead, token } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 5 } });
    turns.hold = true;
    const first = await rt.workspaces.create({ project: project.id, name: "First", sub: { on: "boat" } }, lead);
    const second = await rt.workspaces.create({ project: project.id, name: "Second", sub: { on: "boat" } }, lead);
    // A slow read of Boat's limits, so the second start is written down before the queue finds it has to wait.
    const limits = boat.limits!.bind(boat);
    boat.limits = async () => (await new Promise(r => setTimeout(r, 100)), limits());
    const [a, b] = await Promise.all([startOf(rt, first, lead, token, "First"), startOf(rt, second, lead, token, "Second")]);
    expect(a.held()).toBeUndefined();
    expect(b.held()).toEqual({ placeId: "box", place: "Boat", running: 1, atOnce: 1, why: "slot" });
    expect(boat.creates).toHaveLength(1);
  });

  it("two spawns asked at the same moment on the minute's last start: one machine, the other held for the minute", async () => {
    const { rt, turns, store, boat, project, lead, token } = await leadOnThisComputer({ agents: { spawn: true, maxMachines: 5 } });
    boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, minute: { limit: 12, used: 11 } } };
    // Boat counts a start once its create is posted, as its own read does.
    const create = boat.create.bind(boat);
    boat.create = async spec => ((boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, minute: { limit: 12, used: 12 } } }), create(spec));
    turns.hold = true;
    slowCreates(boat, 200);
    const both = await Promise.all(["First", "Second"].map(name => spawnAndStart(rt, project.id, lead, token, name)));
    expect(boat.creates).toHaveLength(1);
    const records = await Promise.all(both.map(async b => (await store.get("workspaces", b.made.id)) as { phase: string; queue?: { why: string } }));
    expect(records.find(r => r.phase === "queued")?.queue?.why).toBe("minute");
  });

  it("the queue passes over a queued record whose start never came, and drops it after a short wait", async () => {
    const fc = fakeClock();
    const { rt, turns, store, boat, project, lead, token } = await leadOnThisComputer({ clock: fc.clock, machines: 1, agents: { spawn: true, maxMachines: 5 } });
    // A done sub-thread naps its machine at once, so the first one works on, holding its slot, until it is ended.
    turns.hold = true;
    const a = await spawnAndStart(rt, project.id, lead, token, "First");
    await a.started;
    // Its client went away between the create and the start.
    const orphan = await rt.workspaces.create({ project: project.id, name: "Orphan", sub: { on: "boat" } }, lead);
    const c = await spawnAndStart(rt, project.id, lead, token, "Third");
    expect(c.held()).toMatchObject({ why: "slot" });
    turns.release!();
    await expect.poll(() => boat.creates.map(s => s.labels?.[NAME_LABEL])).toEqual(["First", "Third"]);
    await c.started;
    expect(((await store.get("workspaces", orphan.id)) as { phase: string }).phase).toBe("queued");
    fc.advance(60_000);
    await expect.poll(async () => store.get("workspaces", orphan.id)).toBeUndefined();
    expect(boat.creates).toHaveLength(2);
  });

  it("a startless queued record alone in the queue is still dropped after the wait", async () => {
    const fc = fakeClock();
    const { rt, turns, store, boat, project, lead, token } = await leadOnThisComputer({ clock: fc.clock, machines: 1, agents: { spawn: true, maxMachines: 5 } });
    // A done sub-thread naps its machine at once, so the first one works on, holding its slot, until it is ended.
    turns.hold = true;
    const a = await spawnAndStart(rt, project.id, lead, token, "First");
    await a.started;
    // Its client went away between the create and the start, and no other spawn waits to move the queue.
    const orphan = await rt.workspaces.create({ project: project.id, name: "Orphan", sub: { on: "boat" } }, lead);
    turns.release!();
    fc.advance(120_000);
    await expect.poll(async () => store.get("workspaces", orphan.id)).toBeUndefined();
    expect(boat.creates).toHaveLength(1);
  });

  it("a delete while a let-through spawn's machine is being made stops it: the machine goes as it lands and no thread starts", async () => {
    const { rt, turns, store, boat, starts, project, lead, token, brokerSwitch } = await leadOnThisComputer({ machines: 1, agents: { spawn: true, maxMachines: 5 }, broker: false });
    // A done sub-thread naps its machine at once, so the first one works on, holding its slot, until it is ended.
    turns.hold = true;
    const a = await spawnAndStart(rt, project.id, lead, token, "First");
    await a.started;
    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    slowCreates(boat, 300);
    const create = boat.create.bind(boat);
    const offAtKill: boolean[] = [];
    boat.create = async spec => {
      const m = await create(spec);
      const kill = m.kill.bind(m);
      m.kill = async () => {
        offAtKill.push(brokerSwitch.off.has(b.made.id));
        await kill();
      };
      return m;
    };
    turns.release!();
    await expect.poll(async () => (await store.get("workspaces", b.made.id)) as { queue?: unknown }).not.toHaveProperty("queue");
    await rt.workspaces.delete(b.made.id);
    await expect(b.started).rejects.toThrow();
    await expect.poll(async () => store.get("workspaces", b.made.id)).toBeUndefined();
    expect(boat.creates).toHaveLength(2);
    await expect.poll(() => boat.machines.at(-1)!.killed).toBe(true);
    expect(offAtKill).toEqual([true]);
    expect(starts.filter(s => s.o.cwd === "/root/web")).toHaveLength(1);
  });

  it("a spawn behind the head shows the head's wait, and every held row on the place takes the head's count as it moves (Q18, U5)", async () => {
    const fc = fakeClock();
    const { rt, boat, project, lead, token } = await leadOnThisComputer({ clock: fc.clock, agents: { spawn: true, maxMachines: 5 } });
    boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, minute: { limit: 12, used: 12 } } };
    const b = await spawnAndStart(rt, project.id, lead, token, "Second");
    expect(b.held()).toMatchObject({ why: "minute", running: 12, atOnce: 12 });
    const c = await spawnAndStart(rt, project.id, lead, token, "Third");
    expect(c.held()).toMatchObject({ why: "minute", running: 12, atOnce: 12 });
    // Boat's minute reads 13 used: both rows say the head's new count at the next read.
    boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, minute: { limit: 12, used: 13 } } };
    fc.advance(10_000);
    const capped = async () => (await rt.sessions.list()).filter(r => r.workspaceId === b.made.id || r.workspaceId === c.made.id).map(r => r.capped);
    await expect.poll(capped).toEqual([expect.objectContaining({ why: "minute", running: 13 }), expect.objectContaining({ why: "minute", running: 13 })]);
  });
});
