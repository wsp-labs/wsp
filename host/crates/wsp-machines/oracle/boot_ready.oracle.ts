// SPDX-License-Identifier: AGPL-3.0-only
// machines/boot.ts's ready(), the host's start, driven through scenes with a recording runtime context (any member a
// scene does not give logs its call with its arguments and answers nothing). Date is the scene clock; randomBytes(n)
// answers byte k repeated n times on its k-th call. Every call, store write, warning and the maps ready() fills are
// written in order to tests/fixtures/oracle/boot_ready.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

const rec = vi.hoisted(() => ({ log: [] as unknown[], nonce: 0 }));
vi.mock("node:crypto", async orig => ({ ...(await orig<typeof import("node:crypto")>()), randomBytes: (n: number) => Buffer.alloc(n, ++rec.nonce) }));
const { bootArea } = await import("../../../../packages/runtime/src/machines/boot.js");
const { fakeClock } = await import("../../../../packages/runtime/test/fake-clock.js");

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const T0 = 1_790_000_000_000;
const flush = async (n = 40): Promise<void> => {
  for (let i = 0; i < n; i++) await new Promise<void>(r => setImmediate(r));
};
const short = (a: unknown): unknown => {
  if (typeof a === "function") return "fn";
  if (typeof a === "object" && a !== null && "record" in a) return `entry:${(a as { record: { id: string } }).record.id}`;
  if (typeof a === "object" && a !== null && "view" in a && "turnId" in a) return `row:${(a as { turnId: string }).turnId}`;
  return a;
};

function world(o: { store?: Record<string, Record<string, unknown>>; reattach?: Record<string, string>; devices?: unknown[]; shells?: string[]; running?: string[]; ctx?: Record<string, unknown>; local?: boolean; states?: Record<string, string> } = {}) {
  rec.log = [];
  rec.nonce = 0;
  const log = rec.log;
  const clock = fakeClock(T0);
  const tables = new Map<string, Map<string, unknown>>();
  for (const [t, rows] of Object.entries(o.store ?? {})) tables.set(t, new Map(Object.entries(JSON.parse(JSON.stringify(rows)))));
  const table = (t: string) => tables.get(t) ?? (tables.set(t, new Map()), tables.get(t)!);
  const store = {
    get: async (t: string, k: string) => table(t).get(k),
    put: async (t: string, k: string, v: unknown) => (log.push(["store.put", t, k, v]), void table(t).set(k, v)),
    delete: async (t: string, k: string) => (log.push(["store.delete", t, k]), void table(t).delete(k)),
    keys: async (t: string) => [...table(t).keys()],
    list: async (t: string) => (log.push(["store.list", t]), [...table(t).values()]),
  };
  const live = new Map<string, unknown>();
  const sessions = new Map<string, { view: Record<string, unknown>; turnId: string; [k: string]: unknown }>();
  const threadRecords = new Map<string, unknown>();
  const projectsHeld = new Map<string, unknown>();
  const viewedMarks = new Map<string, unknown>();
  const bootWork = new Map<string, Promise<void>>();
  const transcriptIndex = new Map<string, unknown>();
  const backend = {
    capabilities: {}, pricing: { defaultSize: { cpu: 1, memMb: 2048 } },
    get: async (id: string) => (log.push(["backend.get", id]), { id, kind: "sandbox", seen: { state: o.states?.[id] ?? "running" }, state: async () => o.states?.[id] ?? "running" }),
  };
  const base: Record<string, unknown> = {
    store, clock: clock.clock, hostId: "host:A", live, sessions, threadRecords, projectsHeld, viewedMarks, bootWork, transcriptIndex, builders: new Map(),
    local: o.local === false ? undefined : { here: true }, rows: undefined,
    placeDoor: { load: async () => void log.push(["placeDoor.load"]) },
    setups: { load: async () => void log.push(["setups.load"]) },
    bus: { on: () => () => undefined, emit: (e: unknown) => void log.push(["emit", e]), pass: () => undefined },
    state: { owner: "", closed: false, closing: false, readsSince: 0 },
    places: { wired: "solari", backend: () => backend },
    rereading: new Set<string>(),
    moduleOf: () => ({ backend: () => backend }),
    imageOf: async () => ({}),
    attach: (record: Record<string, unknown>, machine: { id: string }) => {
      log.push(["attach", record.id, record.phase]);
      const entry = { record, machine };
      live.set(record.id as string, entry);
      return entry;
    },
    persist: async (r: { id: string; phase: string }) => void log.push(["persist", r.id, r.phase]),
    idle: { touch: (id: string) => void log.push(["idle.touch", id]) },
    gitTopOf: async (p: string) => (log.push(["gitTopOf", p]), p === "/nogit" ? Promise.reject(new Error("not a repo")) : `${p}-top`),
    rememberProject: async (p: unknown) => void log.push(["rememberProject", p]),
    reattach: async (row: { turnId: string }, again: boolean) => (log.push(["reattach", row.turnId, again]), o.reattach?.[row.turnId] ?? "reopened"),
    settleCut: (row: { turnId: string; view: { startedAt?: number } }, reason: string, line: (endedAt: number) => string) => {
      log.push(["settleCut", row.turnId, reason, line(T0 + 65_000)]);
      row.view.status = "failed";
    },
    persistSessions: async (ws: string) => void log.push(["persistSessions", ws]),
    deviceDoor: {
      list: async () => (log.push(["deviceDoor.list"]), o.devices ?? []),
      revoke: async (id: string) => void log.push(["deviceDoor.revoke", id]),
      revokeAsides: async () => void log.push(["deviceDoor.revokeAsides"]),
    },
    shellHeldDevices: async () => (log.push(["shellHeldDevices"]), new Set(o.shells ?? [])),
    threadRuns: (t: string) => (o.running ?? []).includes(t),
    tellAs: (o2: unknown) => ({ tellAs: o2 }),
    loadIndex: async (id: string) => void log.push(["loadIndex", id]),
    slates: { ready: async () => void log.push(["slates.ready"]) },
    moveOldCopies: async () => void log.push(["moveOldCopies"]),
    deliverOwed: async () => void log.push(["deliverOwed"]),
    resumeQueue: async () => void log.push(["resumeQueue"]),
    sweepRuns: async (e: { record: { id: string } }) => void log.push(["sweepRuns", e.record.id]),
    syncDaemon: async (e: { record: { id: string } }) => void log.push(["syncDaemon", e.record.id]),
    worktreeMounted: async (e: { record: { id: string } }) => void log.push(["worktreeMounted", e.record.id]),
    ...o.ctx,
  };
  const ctx = new Proxy(base, {
    get(target, prop) {
      if (prop in target) return target[prop as string];
      if (typeof prop !== "string" || prop === "then") return undefined;
      return (...args: unknown[]) => (log.push([prop, ...args.map(short)]), undefined);
    },
  });
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(T0);
  const warn = vi.spyOn(console, "warn").mockImplementation((line: unknown) => void log.push(["warn", String(line)]));
  const area = bootArea(ctx as never);
  const after = () => ({
    owner: (base.state as { owner: string }).owner,
    readsSince: (base.state as { readsSince: number }).readsSince,
    projects: [...projectsHeld.keys()],
    threadRecords: [...threadRecords],
    viewedMarks: [...viewedMarks],
    sessions: [...sessions].map(([id, s]) => [id, Object.fromEntries(Object.entries(s).filter(([k]) => k !== "end").map(([k, v]) => [k, v]))]),
    live: [...live.keys()],
    bootWork: [...bootWork.keys()],
    stores: Object.fromEntries([...tables].map(([t, rows]) => [t, [...rows]])),
  });
  return { log, area, clock, base, sessions, after, done: () => (warn.mockRestore(), vi.useRealTimers()) };
}

async function scene(name: string, w: Parameters<typeof world>[0], act: (x: ReturnType<typeof world>) => Promise<unknown>) {
  const x = world(w);
  let answer: unknown;
  try {
    answer = await act(x);
    await flush();
  } catch (e) {
    answer = { threw: (e as Error).message };
  } finally {
    x.done();
  }
  return { name, given: w ?? {}, log: x.log, answer: answer ?? null, after: x.after() };
}

const ws = (id: string, o: Record<string, unknown> = {}) => ({ id, name: `w-${id}`, kind: "cloud", machineId: `m-${id}`, phase: "running", golden: "img", project: "p1", ...o });
const row = (id: string, ws: string, o: Record<string, unknown> = {}) => ({ id, workspaceId: ws, harness: "claude", status: "completed", threadId: `t-${id}`, startedAt: T0 - 100_000, turnId: `turn-${id}`, ...o });

it("boot: ready", async () => {
  const scenes = [
    await scene("a fresh store mints the owner and the reads stamp and reads nothing else", {}, async x => {
      await x.area.ready();
      const again = x.area.ready();
      x.log.push(["joined", again === x.area.ready()]);
      await again;
    }),
    await scene("a kept owner and reads stamp are read back; a blank owner is minted again", { store: { owner: { id: { id: "" } }, reads: { since: { at: "not a number" } } } }, async x => x.area.ready()),
    await scene("projects are held, and a folder project with no repo kept is read for one on this computer", {
      store: { owner: { id: { id: "h_kept" } }, reads: { since: { at: 5 } }, projects: {
        p1: { id: "p1", name: "site", computer: "here", path: "/src/site", source: { kind: "folder" } },
        p2: { id: "p2", name: "kept", computer: "here", path: "/src/kept", source: { kind: "folder" }, git: { top: "/src/kept" } },
        p3: { id: "p3", name: "nogit", computer: "here", path: "/nogit", source: { kind: "folder" } },
        p4: { id: "p4", name: "remote", computer: "here", path: "/src/r", source: { kind: "github" } },
      } },
    }, async x => x.area.ready()),
    await scene("no local wiring reads no repo", { local: false, store: { owner: { id: { id: "h_kept" } }, reads: { since: { at: 5 } }, projects: { p1: { id: "p1", name: "site", computer: "here", path: "/src/site", source: { kind: "folder" } } } } }, async x => x.area.ready()),
    await scene("the sessions documents fill the rows and the thread records, and running rows are re-opened", {
      states: { "m-b": "paused" },
      reattach: { "turn-r1": "reopened", "turn-r2": "cannot", "turn-r3": "gone", "turn-r4": "unreached" },
      running: ["t-r1"],
      devices: [{ id: "d-r1", scope: { threadId: "t-r1" } }, { id: "d-r2", scope: { threadId: "t-r2" } }, { id: "d-shell", scope: { threadId: "t-r3" } }, { id: "d-plain" }, { id: "d-old", scope: { threadId: "t-old" } }],
      shells: ["d-shell"],
      store: {
        owner: { id: { id: "h_kept" } }, reads: { since: { at: 5 } },
        workspaces: { a: ws("a"), b: ws("b", { phase: "napping" }), w: ws("w", { worktree: { made: true } }) },
        sessions: {
          a: {
            workspaceId: "a",
            viewed: { "t-1": "turn-x", "t-bad": 3 },
            threads: {
              "t-1": { harness: "claude", permissionMode: "auto", parentThreadId: "p", rootThreadId: "r", readAt: 1, settledAt: 2, settleNamedAt: 3, pinnedAt: 4, order: 5, foldedAt: 6, replaces: "t-0", snoozedUntil: T0 + 60_000, section: { name: "done", whileState: "idle" }, resumeAt: "cut-1", forkedFrom: { threadId: "t-src", title: "src" }, limitResume: { at: T0 + 30_000, turnId: "turn-x" }, rewound: { before: "e1", at: 7 }, worked: true, ended: "completed", filesIn: ["x"], clouds: { y: 1 } },
              "t-2": { harness: "codex", permissionMode: 3, section: { name: "nonsense" }, cutOwed: "cut-2", forkedFrom: { threadId: 1 }, limitResume: { at: "x" }, ended: "weird" },
              "t-none": { permissionMode: "auto" },
            },
            sessions: [
              row("r1", "a", { status: "running", notify: "lead-1", notifyBy: { kind: "thread", threadId: "t-lead", workspaceId: "a", rootThreadId: "t-lead" }, notifyRoad: "relayed", reply: "partial", told: 3, waited: 9, run: "run-1", from: 120, asked: { prompt: "go", effort: "high", junk: 1 }, turnToken: "tok", scopeDeviceId: "d-r1", toldLast: 2 }),
              row("r2", "a", { status: "running", notifyBy: { kind: "nonsense" }, notifyRoad: "elsewhere", from: -1, waitingSince: 4 }),
              row("r3", "a", { status: "running", from: 1.5, prompt: "x".repeat(700) }),
              row("r4", "a", { status: "running", startedAt: undefined }),
              row("done1", "a", { snapshot: "snap-card" }),
              row("done2", "a", { prompt: "y".repeat(600) }),
            ],
          },
          b: { workspaceId: "b", threads: {}, sessions: [row("rb", "b", { status: "running" })] },
          w: { workspaceId: "w", threads: {} },
          gone: { workspaceId: "gone", threads: { "t-g": { harness: "claude" } }, sessions: [row("g1", "gone")] },
        },
        "held-starts": {
          "turn-h1": { workspaceId: "a", threadId: "t-h1", turnId: "turn-h1", sessionId: "s-h1", harness: "claude", place: "hetzner", prompt: "do it", notify: ["lead"], notifyBy: { kind: "thread", threadId: "t-lead", workspaceId: "a", rootThreadId: "t-lead" }, notifyRoad: "here" },
          "turn-h2": { workspaceId: "a", threadId: "t-h2", turnId: "turn-h2", sessionId: "turn-h2", harness: "codex", place: "box" },
          "turn-h3": { workspaceId: "nolive", threadId: "t-h3", turnId: "turn-h3", sessionId: "s-h3", harness: "claude", place: "box" },
        },
      },
    }, async x => {
      await x.area.ready();
      await flush(80);
      x.log.push(["mark", "advance the link window"]);
      x.clock.advance(60_000);
      await flush(80);
    }),
  ];
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "boot_ready.json"), `${JSON.stringify({ T0, scenes }, null, 1)}\n`);
});
