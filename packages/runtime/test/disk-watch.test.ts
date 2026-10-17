// SPDX-License-Identifier: AGPL-3.0-only
// The disk of a machine whose stop snapshots it (Boat), read on a beat while it runs: past the mark what wsp made
// there is cleared once per crossing and every thread on it is told how full it is and the next size up with its
// disk; a move to that size is a nap and a wake that resumes at it.
import { describe, expect, it } from "vitest";
import { DISK_USE_CMD, SWEEP_STAGED_CMD, type MachineShape } from "@wsp/engine";
import { diskMarkLine, type Caller, type EventUnion, type SessionNoticeEvent, type TurnResult } from "@wsp/protocol";
import { createRuntime, type HarnessAdapterFactory } from "../src/runtime.js";
import { DISK_CLOSE_WAIT_MS, DISK_READ_MS } from "../src/machines/reach.js";
import { RESIZE_NAP_MS } from "../src/projects/workspaces.js";
import { memoryStore } from "../src/store.js";
import { stubBackend, createOn, type StubBackend, type StubMachine } from "./stub-backend.js";
import { fakeClock } from "./fake-clock.js";

/** Boat's root disk per size as measured inside a box (2026-10-12): small and default share one, large grows it, and
 * xlarge was never measured, so it has none here. */
const ROOT_DISKS: Record<string, { name: string; diskGb: number }> = { "2x4096": { name: "small", diskGb: 75 }, "4x8192": { name: "default", diskGb: 75 }, "8x16384": { name: "large", diskGb: 160 } };

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

/** Boat as the stub plays it: a stop that snapshots the disk, two sizes with their disks, and df answering in turn
 * off `readings` (used and free in kB), the last one standing once they run out. */
function boat(readings: string[]): { backend: StubBackend; swept: string[]; reads: () => number } {
  const backend = stubBackend();
  backend.capabilities.pauseMode = "disk";
  backend.capabilities.sizes = [
    { cpu: 2, memMb: 4096, rateUsdPerHour: 0.018 },
    { cpu: 4, memMb: 8192, rateUsdPerHour: 0.036 },
    { cpu: 8, memMb: 16384, rateUsdPerHour: 0.072 },
    { cpu: 16, memMb: 32768, rateUsdPerHour: 0.2 },
  ];
  Object.assign(backend, { rootDiskOf: (size: { cpu: number; memMb: number }) => ROOT_DISKS[`${size.cpu}x${size.memMb}`] });
  const answer = backend.execImpl;
  const swept: string[] = [];
  let reads = 0;
  backend.execImpl = (m, cmd) => {
    if (cmd === DISK_USE_CMD) return { exitCode: 0, stdout: `${readings[Math.min(reads++, readings.length - 1)]}\n`, stderr: "" };
    if (cmd === SWEEP_STAGED_CMD || cmd.includes("rm -rf /root/.npm")) swept.push(cmd === SWEEP_STAGED_CMD ? "staged" : "caches");
    return answer(m, cmd);
  };
  return { backend, swept, reads: () => reads };
}

/** df's used and free on a box's root, in kB: 68928192 on a 75 GB disk (read on a small Boat box, 2026-10-12), the
 * filesystem's own blocks and its reserve taken, and the 12 and 160 GB disks at the same share. */
const DISK_75_GB_KB = 68928192;
const DISK_12_GB_KB = 11004751;
const DISK_160_GB_KB = 146730014;
/** One df answer for a disk of that many kB at that share used. */
const full = (diskKb: number, pct: number): string => `${Math.round((diskKb * pct) / 100)} ${diskKb - Math.round((diskKb * pct) / 100)}`;

const settle = async (): Promise<void> => {
  for (let i = 0; i < 20; i++) await new Promise(r => setTimeout(r, 2));
};

describe("the disk of a machine whose stop snapshots it", () => {
  it("is read on the beat with no turn ending, and past 90% what wsp made is cleared and the thread told, again only once it fell 5 points under", async () => {
    // 50%, then 93% (the crossing), 92% after the sweep, 88% and 91% (never under 85, so nothing new), 84% (under), and
    // 91% a second crossing.
    const { backend, swept } = boat([50, 93, 92, 88, 91, 84, 91].map(pct => full(DISK_75_GB_KB, pct)));
    const { clock, advance } = fakeClock();
    const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: answers }, clock });
    const notices: SessionNoticeEvent[] = [];
    rt.events.on("session.notice", (e: EventUnion) => void (e.type === "session.notice" && notices.push(e)));
    const ws = await createOn(rt, { golden: "snap_g", name: "web", cpu: 2, memMb: 4096 });
    const turn = await rt.sessions.start(ws.id, { prompt: "build" });
    await turn.finished;
    await settle();
    expect(notices).toEqual([]);

    advance(DISK_READ_MS);
    await settle();
    expect(swept).toEqual(["staged", "caches"]);
    const thread = turn.view().threadId!;
    const text = diskMarkLine({ machine: "web", on: "default", pct: 92, usedBytes: 63413937 * 1024, sizeBytes: DISK_75_GB_KB * 1024, freedBytes: 689282 * 1024, machineGoes: false, move: { thread, name: "large", cpu: 8, memMb: 16384, diskGb: 160 } });
    expect(notices.map(n => [n.threadId, n.turnId, n.text])).toEqual([[thread, expect.any(String), text]]);
    // default keeps small's root disk, so the move offered skips it for large, the first size whose disk is bigger.
    expect(text).toContain("wsp cleared 706 MB of its own staging and build caches there");
    expect(text).toContain(`Free space on its disk. Or move it to large (8 vCPU, 16 GB, 160 GB disk): wsp resize ${thread} --size 8x16 stops it and resumes it at that size, with its files, which ends a turn running there.`);
    expect(text).not.toMatch(/default \(|for files/);
    expect((await rt.status.list())[0]!.reason).toMatch(/^its disk is 92% full: /);

    // Under the line at 88%, past it at 91%: never 5 points under, so nothing is cleared or said again.
    advance(DISK_READ_MS);
    await settle();
    expect((await rt.status.list())[0]!.reason).toBeUndefined();
    advance(DISK_READ_MS);
    await settle();
    expect(notices).toHaveLength(1);
    expect(swept).toEqual(["staged", "caches"]);
    // 84%, then past the line again: a second crossing is said again.
    advance(DISK_READ_MS);
    await settle();
    advance(DISK_READ_MS);
    await settle();
    expect(notices).toHaveLength(2);
    expect(swept).toEqual(["staged", "caches", "staged", "caches"]);
    await rt.close();
  });

  it("a machine still clearing holds back no other machine's read on the beats after", async () => {
    const { backend } = boat(["500000 500000"]);
    let release: () => void = () => {};
    const held = new Promise<void>(resolve => (release = resolve));
    const answer = backend.execImpl;
    const reads = new Map<string, number>();
    backend.execImpl = (m, cmd) => {
      if (cmd === DISK_USE_CMD) {
        const n = (reads.get(m.id) ?? 0) + 1;
        reads.set(m.id, n);
        // Under the line at the turn's end, past it from the first beat on.
        return { exitCode: 0, stdout: n === 1 ? "500000 500000\n" : "950000 50000\n", stderr: "" };
      }
      // The first machine's clearing sits on Boat for minutes; the other's answers at once.
      if (cmd === SWEEP_STAGED_CMD && m.id === backend.machines[0]!.id) return held.then(() => answer(m, cmd));
      return answer(m, cmd);
    };
    const { clock, advance } = fakeClock();
    const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: answers }, clock });
    for (const name of ["web", "api"]) await (await rt.sessions.start((await createOn(rt, { golden: "snap_g", name, cpu: 2, memMb: 4096 })).id, { prompt: "build" })).finished;
    await settle();
    const [stuck, other] = backend.machines.map(m => m.id);
    advance(DISK_READ_MS);
    await settle();
    const afterCrossing = { stuck: reads.get(stuck!)!, other: reads.get(other!)! };
    advance(DISK_READ_MS);
    await settle();
    advance(DISK_READ_MS);
    await settle();
    expect(reads.get(other!)).toBe(afterCrossing.other + 2);
    expect(reads.get(stuck!)).toBe(afterCrossing.stuck);
    release();
    await settle();
    await rt.close();
  });

  it("a host closing waits on a read still clearing only so long", async () => {
    const { backend } = boat(["950000 50000"]);
    const answer = backend.execImpl;
    // The clearing never answers, as a sweep can sit for its whole five minute guard.
    backend.execImpl = (m, cmd) => (cmd === SWEEP_STAGED_CMD ? new Promise(() => {}) : answer(m, cmd));
    const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: answers } });
    const ws = await createOn(rt, { golden: "snap_g", name: "web", cpu: 2, memMb: 4096 });
    await (await rt.sessions.start(ws.id, { prompt: "build" })).finished;
    await settle();
    const began = Date.now();
    await rt.close();
    expect(Date.now() - began).toBeLessThan(DISK_CLOSE_WAIT_MS + 2_000);
  }, 20_000);

  it("tells a sub-thread's machine it goes once its thread is done, and offers a move only where one is given", () => {
    const words = { machine: "fix-upload", on: "Boat", pct: 93.4, usedBytes: 65.6e9, sizeBytes: 70.6e9, freedBytes: 0 };
    expect(diskMarkLine({ ...words, machineGoes: true })).toBe(
      "The disk of fix-upload is 93.4% used (65.6 of 70.6 GB) and still filling. Boat cannot save a stop of a full disk, and deletes one whose stops keep failing with its disk. wsp found none of its own staging or build caches there to clear. Free space on its disk, or let the thread finish: its machine is deleted once it is done.",
    );
    expect(diskMarkLine({ ...words, machineGoes: false })).toMatch(/to clear\. Free space on its disk\.$/);
  });

  it("names the disk and the size it moves to in one unit, decimal GB as Boat sells them", () => {
    // df on a small box's 75 GB disk (2026-10-12): 25745280 kB used, 43182912 free.
    const text = diskMarkLine({ machine: "web", on: "Boat", pct: 37.4, usedBytes: 25745280 * 1024, sizeBytes: (25745280 + 43182912) * 1024, freedBytes: 689282 * 1024, machineGoes: false, move: { thread: "t1", name: "large", cpu: 8, memMb: 16384, diskGb: 160 } });
    expect(text).toContain("(26.4 of 70.6 GB)");
    expect(text).toContain("wsp cleared 706 MB");
    expect(text).toContain("160 GB disk");
  });

  it("offers the first size up that grows the disk the machine reads, and none on large", async () => {
    // A small box on Boat's older 12 GB disk moves to default; one on the 75 GB disk default shares moves to large; on
    // large nothing is offered, since xlarge's disk was never measured.
    const offers: (string | undefined)[] = [];
    for (const [cpu, memMb, diskKb] of [[2, 4096, DISK_12_GB_KB], [2, 4096, DISK_75_GB_KB], [8, 16384, DISK_160_GB_KB]] as const) {
      const { backend } = boat([full(diskKb, 95)]);
      const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: answers } });
      const notices: SessionNoticeEvent[] = [];
      rt.events.on("session.notice", (e: EventUnion) => void (e.type === "session.notice" && notices.push(e)));
      const ws = await createOn(rt, { golden: "snap_g", name: "web", cpu, memMb });
      await (await rt.sessions.start(ws.id, { prompt: "build" })).finished;
      await settle();
      expect(notices).toHaveLength(1);
      offers.push(/Or move it to (.*?): wsp resize \S+ --size (\S+) /.exec(notices[0]!.text)?.slice(1).join(" "));
      await rt.close();
    }
    expect(offers).toEqual(["default (4 vCPU, 8 GB, 75 GB disk) 4x8", "large (8 vCPU, 16 GB, 160 GB disk) 8x16", undefined]);
  });

  it("reads nothing on a machine whose stop keeps memory", async () => {
    const { clock, advance } = fakeClock();
    const memory = boat(["950000 50000"]);
    memory.backend.capabilities.pauseMode = "memory";
    const other = createRuntime({ backend: memory.backend, store: memoryStore(), adapters: { claude: answers }, clock });
    const on = await createOn(other, { golden: "snap_g", name: "web", cpu: 2, memMb: 4096 });
    await (await other.sessions.start(on.id, { prompt: "build" })).finished;
    advance(DISK_READ_MS);
    await settle();
    expect(memory.reads()).toBe(0);
    await other.close();
  });
});

describe("a move to another size", () => {
  /** The stub's machines resume at a size as Boat does: the shape the provider reports follows it. */
  const movable = (backend: StubBackend): { resumedAt: { cpu: number; memMb: number }[]; pausedWith: unknown[] } => {
    const resumedAt: { cpu: number; memMb: number }[] = [];
    const pausedWith: unknown[] = [];
    const create = backend.create.bind(backend);
    backend.create = async spec => {
      const m = (await create(spec)) as StubMachine & { resumeAt?: (size: { cpu: number; memMb: number }) => Promise<void>; shape: MachineShape };
      const pause = m.pause.bind(m);
      m.pause = async (o?: { waitMs?: number }) => {
        pausedWith.push(o);
        await pause();
      };
      m.resumeAt = async size => {
        resumedAt.push(size);
        m.paused = false;
        m.shape = { ...m.shape, cpu: size.cpu, memMb: size.memMb };
      };
      return m;
    };
    return { resumedAt, pausedWith };
  };

  it("naps the machine and wakes it at the size asked, the record and its rate following", async () => {
    const { backend } = boat(["500000 500000"]);
    const { resumedAt, pausedWith } = movable(backend);
    const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: answers } });
    const ws = await createOn(rt, { golden: "snap_g", name: "web", cpu: 2, memMb: 4096 });
    await (await rt.sessions.start(ws.id, { prompt: "build" })).finished;
    const m = backend.machines[0]!;
    const moved = await rt.workspaces.resize(ws.id, { cpu: 4, memMb: 8192 });
    expect(resumedAt).toEqual([{ cpu: 4, memMb: 8192 }]);
    // The stop before the move waits as long as a full disk's archive takes (14 minutes for 67 GB, measured 2026-10-01).
    expect(pausedWith).toEqual([{ waitMs: RESIZE_NAP_MS }]);
    expect(RESIZE_NAP_MS).toBe(30 * 60_000);
    expect(m.resumes).toBe(0);
    expect(moved.phase).toBe("running");
    const status = (await rt.status.list()).find(s => s.id === ws.id)!;
    expect(status.size).toMatchObject({ cpu: 4, memMb: 8192 });
    // The next wake checks the machine against the shape it now has, so it is no fault.
    await rt.workspaces.nap(ws.id);
    expect((await rt.workspaces.wake(ws.id)).phase).toBe("running");
    // The same size again is no move at all.
    await rt.workspaces.resize(ws.id, { cpu: 4, memMb: 8192 });
    expect(resumedAt).toHaveLength(1);
    await rt.close();
  });

  it("says so when the machine came back at another size than asked, and the record keeps the size it has", async () => {
    const { backend } = boat(["500000 500000"]);
    movable(backend);
    const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: answers } });
    const ws = await createOn(rt, { golden: "snap_g", name: "web", cpu: 2, memMb: 4096 });
    // A plain resume someone else sent: the machine comes back as it was.
    (backend.machines[0] as StubMachine & { resumeAt: (size: { cpu: number; memMb: number }) => Promise<void> }).resumeAt = async () => {
      backend.machines[0]!.paused = false;
    };
    await expect(rt.workspaces.resize(ws.id, { cpu: 4, memMb: 8192 })).rejects.toThrow("web came back at 2x4, not the 4x8 asked for. Run wsp resize again.");
    expect((await rt.status.list()).find(s => s.id === ws.id)!.size).toMatchObject({ cpu: 2, memMb: 4096 });
    await rt.close();
  });

  it("keeps the size the provider moved it to when the wake fails after the resume landed, so a later wake checks the right shape", async () => {
    const { backend } = boat(["500000 500000"]);
    movable(backend);
    const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: answers } });
    const ws = await createOn(rt, { golden: "snap_g", name: "web", cpu: 2, memMb: 4096 });
    const m = backend.machines[0] as StubMachine & { resumeAt: (size: { cpu: number; memMb: number }) => Promise<void>; shape: MachineShape };
    // Boat took the resume at large, and the wake failed after it, as a restore still streaming in fails one.
    m.resumeAt = async size => {
      m.paused = false;
      m.shape = { ...m.shape, cpu: size.cpu, memMb: size.memMb };
      throw new Error("still restoring");
    };
    await expect(rt.workspaces.resize(ws.id, { cpu: 8, memMb: 16384 })).rejects.toThrow("still restoring");
    expect((await rt.status.list()).find(s => s.id === ws.id)!.size).toMatchObject({ cpu: 8, memMb: 16384 });
    await rt.workspaces.wake(ws.id);
    await rt.workspaces.nap(ws.id);
    expect((await rt.workspaces.wake(ws.id)).phase).toBe("running");
    await rt.close();
  });

  it("is refused for a size not sold, on a machine that cannot move, and while a turn runs there", async () => {
    const { backend } = boat(["500000 500000"]);
    const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: answers } });
    const ws = await createOn(rt, { golden: "snap_g", name: "web", cpu: 2, memMb: 4096 });
    await expect(rt.workspaces.resize(ws.id, { cpu: 4, memMb: 8192 })).rejects.toThrow("default changes no machine's size once it is made. Start a new thread at the size you want instead.");
    // A thread's own token is refused in the move's own words: the bigger size is the person's spend.
    const opened = await rt.sessions.start(ws.id, { prompt: "build" });
    await opened.finished;
    const thread = opened.view().threadId!;
    const asThread: Caller = { origin: "here", by: { kind: "thread", threadId: thread, workspaceId: ws.id, rootThreadId: thread } };
    await expect(rt.workspaces.resize(ws.id, { cpu: 4, memMb: 8192 }, asThread)).rejects.toThrow(/never move a machine to another size$/);
    await rt.close();

    const held = boat(["500000 500000"]);
    movable(held.backend);
    let release: () => void = () => {};
    const slow: HarnessAdapterFactory = () => ({
      steers: false,
      start: ({ onEvent }) => {
        const sessionId = "44444444-4444-4444-8444-444444444444";
        onEvent({ type: "session.start", sessionId });
        const finished = new Promise<TurnResult>(resolve => {
          release = () => {
            onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: false });
            resolve({ status: "completed", text: "" });
          };
        });
        return { localId: sessionId, finished, interrupt: async () => {} };
      },
    });
    const busy = createRuntime({ backend: held.backend, store: memoryStore(), adapters: { claude: slow } });
    const on = await createOn(busy, { golden: "snap_g", name: "web", cpu: 2, memMb: 4096 });
    await expect(busy.workspaces.resize(on.id, { cpu: 32, memMb: 65536 })).rejects.toThrow(/^32x64 is not a size this provider offers; the sizes are 2x4 \(.*\), 4x8 \(.*\), 8x16 \(.*\), 16x32 \(.*\)\. Ask for one of those instead\.$/);
    const turn = await busy.sessions.start(on.id, { prompt: "build" });
    await expect(busy.workspaces.resize(on.id, { cpu: 4, memMb: 8192 })).rejects.toThrow("web has a turn running, and the stop a move to another size takes would end it. Stop the turn or wait for it to end, then move it.");
    expect(held.backend.machines[0]!.paused).toBe(false);
    release();
    await turn.finished;
    await busy.close();
  });
});
