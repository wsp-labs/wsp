// SPDX-License-Identifier: AGPL-3.0-only
// machines/boot.ts's entries, rows and marks driven through scenes with a recording runtime context (any member a
// scene does not give logs its call and answers nothing). agentPidOf and accountOnComputer are recorded fakes; Date
// is the scene clock. Every call, pass, emit and the thread records after, in order, go to
// tests/fixtures/oracle/boot_rows.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

const rec = vi.hoisted(() => ({ log: [] as unknown[], pids: {} as Record<string, number> }));
vi.mock("../../../../packages/runtime/src/local-exec.js", async orig => ({ ...(await orig<object>()), agentPidOf: (run: string) => (rec.log.push(["agentPidOf", run]), rec.pids[run]) }));
vi.mock("../../../../packages/runtime/src/usage.js", async orig => ({ ...(await orig<object>()), accountOnComputer: (o: unknown) => (rec.log.push(["accountOnComputer", o]), { key: "acct-1" }) }));
const { bootArea } = await import("../../../../packages/runtime/src/machines/boot.js");
const { fakeClock } = await import("../../../../packages/runtime/test/fake-clock.js");

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const T0 = 1_790_000_000_000;
const flush = async (n = 30): Promise<void> => {
  for (let i = 0; i < n; i++) await new Promise<void>(r => setImmediate(r));
};
const short = (a: unknown): unknown => (typeof a === "function" ? "fn" : typeof a === "object" && a !== null && "record" in a ? `entry:${(a as { record: { id: string } }).record.id}` : a);

interface Row { id: string; workspaceId: string; harness?: string; status?: string; threadId?: string; startedAt?: number; endedAt?: number; [k: string]: unknown }
function world(o: { live?: Record<string, Record<string, unknown>>; rows?: { view: Row; turnId?: string; pid?: number; run?: string; replaces?: string }[]; records?: Record<string, Record<string, unknown>>; ctx?: Record<string, unknown>; pids?: Record<string, number>; readsSince?: number } = {}) {
  rec.log = [];
  rec.pids = o.pids ?? {};
  const log = rec.log;
  const clock = fakeClock(T0);
  const live = new Map<string, unknown>(Object.entries(o.live ?? {}).map(([id, r]) => [id, { record: { id, name: `w-${id}`, kind: "cloud", machineId: `m-${id}`, phase: "running", project: "p1", ...r } }]));
  const sessions = new Map<string, unknown>((o.rows ?? []).map(r => [r.view.id, { view: { harness: "claude", status: "completed", ...r.view }, turnId: r.turnId ?? `turn-${r.view.id}`, ...(r.pid !== undefined ? { pid: r.pid } : {}), ...(r.run !== undefined ? { run: r.run } : {}), ...(r.replaces !== undefined ? { replaces: r.replaces } : {}) }]));
  const threadRecords = new Map<string, unknown>(Object.entries(o.records ?? {}));
  const listeners: ((e: unknown) => void)[] = [];
  const base: Record<string, unknown> = {
    store: { get: async () => undefined, put: async () => undefined, delete: async () => undefined, list: async () => [], keys: async () => [] },
    setups: { load: async () => undefined }, deviceDoor: { list: async () => [], revokeAsides: async () => undefined }, slates: { ready: async () => undefined },
    moveOldCopies: async () => undefined, deliverOwed: async () => undefined, clock: clock.clock, live, sessions, threadRecords, bootWork: new Map(), transcriptIndex: new Map(), projectsHeld: new Map([["p1", { id: "p1", name: "site", computer: "hetzner" }]]),
    placeDoor: undefined, local: undefined, builders: new Map(), rows: undefined, viewedMarks: new Map(),
    bus: {
      on: (_t: string, l: (e: unknown) => void) => (listeners.push(l), () => undefined),
      emit: (e: unknown) => (log.push(["emit", e]), listeners.forEach(l => l(e))),
      pass: (e: unknown) => void log.push(["pass", e]),
    },
    state: { owner: "h_me", closed: false, closing: false, readsSince: o.readsSince ?? T0 - 1000 },
    places: { wired: "solari" },
    rereading: new Set<string>(),
    capHeld: new Map(), setupRefusals: new Map(), resumeTimers: new Map([["t-armed", () => log.push(["resume timer cancelled", "t-armed"])]]), leadAsks: new Set(),
    restarts: () => new Map(),
    stoppedBehind: () => undefined,
    setupPlace: () => undefined,
    latestOn: () => undefined,
    rowsOn: () => [],
    titleRows: () => [],
    drivesThread: (t: string | undefined, caller: { by?: { rootThreadId: string } } | string | undefined) => (log.push(["drivesThread", t ?? null, caller ?? null]), typeof caller !== "object" || caller.by === undefined || t === undefined || t.startsWith(caller.by.rootThreadId)),
    actsOn: (r: { id: string }, caller: unknown) => (log.push(["actsOn", r.id, caller ?? null]), true),
    talksToItsTree: () => false,
    refuseRelayed: (r: { id: string }, origin: unknown, act: unknown, thread: unknown) => void log.push(["refuseRelayed", r.id, origin ?? null, act ?? null, thread ?? null]),
    persistSessions: async (ws: string) => void log.push(["persistSessions", ws]),
    wakeAt: (t: string, at: number) => void log.push(["wakeAt", t, at]),
    resumeOnReset: (t: string, at: number) => void log.push(["resumeOnReset", t, at]),
    reapKept: (t: string) => void log.push(["reapKept", t]),
    backendFor: () => ({ lifecycle: undefined }),
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
  return { log, area, clock, base, live, sessions, threadRecords, done: () => (warn.mockRestore(), vi.useRealTimers()) };
}

async function scene(name: string, w: Parameters<typeof world>[0], act: (x: ReturnType<typeof world>) => Promise<unknown>) {
  const x = world(w);
  // The scenes run on a host already started: ready() runs once on an empty store and its log is dropped.
  await x.area.ready();
  await flush();
  x.log.length = 0;
  let answer: unknown;
  try {
    answer = await act(x);
    await flush();
  } catch (e) {
    answer = { threw: (e as Error).message, kind: (e as { kind?: string }).kind ?? null };
  } finally {
    x.done();
  }
  return { name, given: w ?? {}, log: x.log, answer: answer === undefined ? null : JSON.parse(JSON.stringify(answer, (_k, v) => (v instanceof Map ? [...v] : v instanceof Set ? [...v] : typeof v === "object" && v !== null && "record" in v ? `entry:${(v as { record: { id: string } }).record.id}` : v))), records: [...x.threadRecords] };
}

const thread = { origin: "here", by: { kind: "thread", threadId: "t-a", workspaceId: "a", rootThreadId: "t-a" } };

it("boot: rows", async () => {
  const scenes = [
    await scene("listed rows: every fact a listing carries, and none a row does not", {
      live: { a: {} },
      pids: { "run-1": 4242 },
      readsSince: T0 - 1000,
      rows: [
        { view: { id: "r1", workspaceId: "a", threadId: "t1", status: "running", startedAt: T0 - 50 }, pid: 77, run: "run-1" },
        { view: { id: "r2", workspaceId: "a", threadId: "t2", status: "completed", endedAt: T0 - 5000 }, replaces: "t-gone" },
        { view: { id: "r3", workspaceId: "a", threadId: "t3", status: "completed", endedAt: T0 - 10 }, turnId: "turn-r3" },
        { view: { id: "r4", workspaceId: "a", status: "failed" }, pid: 9 },
      ],
      records: {
        t1: { workspaceId: "a", harness: "claude", readAt: 5, settledAt: 6, pinnedAt: 7, order: 8, foldedAt: 9, snoozedUntil: T0 + 1, section: { name: "done", whileState: "idle" }, rewound: { before: "e", at: 3 }, limitResume: { at: T0 + 9, turnId: "turn-r1" } },
        t2: { workspaceId: "a", harness: "claude", snoozedUntil: T0 - 1, replaces: "t1", limitResume: { at: 1, turnId: "other" } },
        t3: { workspaceId: "a", harness: "claude", forkedFrom: { threadId: "t-src", title: "old title" } },
      },
      ctx: { capHeld: new Map([["turn-r1", { waiting: { wait: { place: "box", running: 2, limit: 2 } } }]]), stoppedBehind: (s: { turnId: string }) => (s.turnId === "turn-r1" ? { threadId: "t9", title: "x" } : undefined), restarts: () => new Map([["t2", "t2-new"]]), setupPlace: () => "hetzner", setupRefusals: new Map([["hetzner\nclaude", "claude is refused on hetzner"]]), rowsOn: (t: string) => (t === "t-src" ? [{ id: "src", workspaceId: "a", harness: "claude", status: "completed", threadId: "t-src", title: "new title" }] : []) },
    }, async x => x.area.listedRows([...x.sessions.values()] as never)),
    await scene("thread facts: the listing's thread with the latest row's model and the running turn", {
      live: { a: { phase: "napping", nappedAt: T0 - 100 } },
      rows: [
        { view: { id: "r1", workspaceId: "a", threadId: "t1", status: "completed", model: "m1", effort: "low", startedAt: 1 } },
        { view: { id: "r2", workspaceId: "a", threadId: "t1", status: "running", model: "m2", contextWindow: 200000, startedAt: 2 }, turnId: "turn-run" },
        { view: { id: "r3", workspaceId: "a", threadId: "t1", status: "completed", startedAt: 3 } },
      ],
    }, async x => ({ facts: x.area.threadFacts("t1") ?? null, none: x.area.threadFacts("nobody") ?? null })),
    await scene("settled now: by hand, by quiet, or by the stamp alone with no row", {
      rows: [{ view: { id: "r1", workspaceId: "a", threadId: "t1", status: "completed", startedAt: T0 - 7_200_000, endedAt: T0 - 7_200_000 } }],
      records: { t1: { workspaceId: "a", harness: "claude", readAt: T0 - 7_200_000 }, "t-norow": { workspaceId: "a", harness: "claude", settledAt: 1 } },
      live: { a: {} },
    }, async x => ({ quiet: x.area.settledNow("t1", 3_600_000), off: x.area.settledNow("t1", null), stampOnly: x.area.settledNow("t-norow", 1), nothing: x.area.settledNow("t-none", 1) })),
    await scene("entry of: refusals by caller, a create still landing, and the boot work waited", { live: { a: {}, c: {} } }, async x => {
      const said: unknown[] = [];
      const tryOf = async (f: () => Promise<unknown>) => f().then(e => said.push(short(e)), (e: Error & { kind?: string }) => said.push({ threw: e.message, kind: e.kind ?? null }));
      await tryOf(() => x.area.entryOf("zzz"));
      await tryOf(() => x.area.entryOf("zzz", thread as never));
      (x.live.get("c") as { creating: boolean }).creating = true;
      await tryOf(() => x.area.entryOf("c"));
      await tryOf(() => x.area.entryOf("c", undefined, { beingMade: true }));
      x.threadRecords.set("t-a1", { workspaceId: "a", harness: "claude" });
      await tryOf(() => x.area.entryOf("a", thread as never, { thread: "t-a1", act: "merge" } as never));
      await tryOf(() => x.area.entryOf("a", thread as never, { thread: "t-other" } as never));
      let done!: () => void;
      (x.base.bootWork as Map<string, Promise<void>>).set("a", new Promise<void>(r => (done = r)));
      const waited = x.area.entryOf("a");
      const now = await x.area.entryOf("a", undefined, { now: true });
      said.push(["now answered first", short(now)]);
      x.live.set("a", { record: { id: "a", name: "w-a2" } });
      done();
      said.push(["after boot work", short(await waited), (await waited as { record: { name: string } }).record.name]);
      return said;
    }),
    await scene("child of: a lead's own child by lead or by parent, else refused naming both", { live: { lead: {}, kid: { lead: { workspaceId: "lead" } }, kid2: { parentWorkspaceId: "lead" }, other: {} }, ctx: { workspaces: { resolve: async (ref: string) => ({ id: ref }) } } }, async x => {
      const lead = x.live.get("lead");
      const said: unknown[] = [];
      for (const ref of ["kid", "kid2", "other"]) await x.area.childOf(lead as never, ref).then(e => said.push(short(e)), (e: Error & { kind?: string }) => said.push({ threw: e.message, kind: e.kind ?? null }));
      await x.area.childOf(lead as never, "other", undefined, { lead: "t-lead00000", child: "t-kid000000" }).catch((e: Error) => said.push({ threw: e.message }));
      return said;
    }),
    await scene("entry of row: a thread caller reaches only its tree, and answers nothing otherwise", { live: { a: {} } }, async x => {
      const said: unknown[] = [];
      said.push(short(await x.area.entryOfRow({ threadId: "t-a-own", workspaceId: "a" }, thread as never)));
      said.push(short(await x.area.entryOfRow({ threadId: "t-b", workspaceId: "a" }, thread as never)) ?? null);
      said.push(short(await x.area.entryOfRow({ threadId: "t-a-own", workspaceId: "none" }, thread as never)) ?? null);
      said.push(short(await x.area.entryOfRow({ threadId: "t-b", workspaceId: "a" }, undefined)));
      return said;
    }),
    await scene("mark: every thread is checked before any moves, then stamps land, each workspace written and told once", {
      live: { a: {}, b: {} },
      rows: [{ view: { id: "r1", workspaceId: "a", threadId: "t1" } }, { view: { id: "r2", workspaceId: "b", threadId: "t2" } }, { view: { id: "r3", workspaceId: "a", threadId: "t3" } }],
      records: { t1: { workspaceId: "a", harness: "claude", pinnedAt: 4, snoozedUntil: 9 }, "t-armed": { workspaceId: "a", harness: "claude", limitResume: { at: 5, turnId: "x" } } },
    }, async x => {
      await x.area.mark(["t1", "t2", "t3"], { settledAt: T0, readAt: T0, pinnedAt: undefined } as never, undefined);
      await x.area.mark(["t1"], (id: string) => ({ snoozedUntil: T0 + 5, note: id }) as never, undefined);
      await x.area.mark(["t-armed"], { limitResume: undefined } as never, undefined);
      await x.area.mark(["t1", "t-nobody"], { readAt: 1 } as never, undefined).catch((e: Error & { kind?: string }) => x.log.push(["rejected", e.message, e.kind ?? null]));
    }),
    await scene("end snooze: only a standing snooze of the thread or its root ends, as now", {
      live: { a: {} },
      rows: [{ view: { id: "r1", workspaceId: "a", threadId: "t1" } }],
      records: { t1: { workspaceId: "a", harness: "claude", snoozedUntil: T0 + 10 }, root: { workspaceId: "a", harness: "claude", snoozedUntil: T0 }, other: { workspaceId: "a", harness: "claude", snoozedUntil: T0 + 10 } },
    }, async x => {
      x.area.endSnoozeFor({ threadId: "t1", rootThreadId: "root" } as never);
      await flush();
      x.area.endSnoozeFor({ threadId: "root" } as never);
    }),
    await scene("arm resume: off clears, on needs a known reset on the latest row", {
      live: { a: {} },
      rows: [{ view: { id: "r1", workspaceId: "a", threadId: "t1", limit: { resetsAt: T0 + 3600_000 } }, turnId: "turn-1" }, { view: { id: "r2", workspaceId: "a", threadId: "t2" } }],
      records: { t1: { workspaceId: "a", harness: "claude" } },
    }, async x => {
      await x.area.armResume(["t1"], true, undefined);
      await x.area.armResume(["t2"], true, undefined).catch((e: Error & { kind?: string }) => x.log.push(["rejected", e.message, e.kind ?? null]));
      await x.area.armResume(["t-none"], true, undefined).catch((e: Error & { kind?: string }) => x.log.push(["rejected", e.message, e.kind ?? null]));
      await x.area.armResume(["t1"], false, undefined);
    }),
    ...(await Promise.all([
      ["resume after limit: the stopped turn goes on at the reset", {}],
      ["resume after limit: a newer turn takes the arm off and sends nothing", { newer: true }],
      ["resume after limit: a thread waiting on the person sends nothing", { asks: true }],
      ["resume after limit: a plan still held past the reset moves the row's reset", { held: true }],
      ["resume after limit: a settled thread sends nothing", { settled: true }],
      ["resume after limit: no arm does nothing", { noArm: true }],
    ].map(async ([name, v]) => {
      const f = v as { newer?: true; asks?: true; held?: true; settled?: true; noArm?: true };
      return scene(name as string, {
        live: { a: {} },
        rows: [{ view: { id: "r1", workspaceId: "a", threadId: "t1", harness: "claude", limit: { resetsAt: T0 } }, turnId: "turn-1" }, ...(f.newer ? [{ view: { id: "r2", workspaceId: "a", threadId: "t1", harness: "claude" }, turnId: "turn-2" }] : [])],
        records: { t1: { workspaceId: "a", harness: "claude", ...(f.noArm ? {} : { limitResume: { at: T0, turnId: "turn-1" } }), ...(f.settled ? { settledAt: 1 } : {}) } },
        ctx: {
          leadAsks: new Set(f.asks ? ["t1"] : []),
          ledger: { limits: async () => [{ key: "acct-1", windows: f.held ? [{ usedPercent: 100, resetsAt: T0 + 7200_000 }] : [] }] },
          vaultedFor: () => false, vaultOn: () => ({}), moduleOf: () => ({ loginStands: () => false }), usageComputerOf: () => "box-1", computerOf: () => "Box",
          sessionsApi: { start: async (ws: string, o: unknown) => void rec.log.push(["sessionsApi.start", ws, o]) },
        },
      }, async x => x.area.resumeAfterLimit("t1"));
    }))),
    await scene("row push: one row per moved thread after the tick, the prompt on its first push only, gone ids last", {
      live: { a: {} },
      rows: [{ view: { id: "r1", workspaceId: "a", threadId: "t1", prompt: "the brief" } }, { view: { id: "r2", workspaceId: "a", threadId: "t2", prompt: "other" } }],
      ctx: { computerRows: async () => [] },
    }, async x => {
      const emit = (x.base.bus as { emit: (e: unknown) => void }).emit;
      emit({ type: "session.held", workspaceId: "a", threadId: "t1" });
      emit({ type: "session.held", workspaceId: "a", threadId: "t1" });
      emit({ type: "thread.marked", workspaceId: "a", threadIds: ["t2"] });
      emit({ type: "session.start", workspaceId: "a", sessionId: "s-new", turnId: "turn-old", threadId: "t1" });
      x.log.push(["mark", "before the tick"]);
      await x.area.rowsSent();
      x.log.push(["mark", "second push"]);
      emit({ type: "session.capped", workspaceId: "a", threadId: "t1" });
      await x.area.rowsSent();
      emit({ type: "session.end", workspaceId: "a", sessionId: "s-ended", turnId: "turn-ended", threadId: "t9" });
      emit({ type: "session.end", workspaceId: "a", sessionId: "r1", turnId: "turn-r1", threadId: "t1" });
      await x.area.rowsSent();
    }),
    await scene("computer rows: with no door this computer and the wired provider", {}, async x => ({ rows: await x.area.computerRows(), named: x.area.nameOfComputer("x", [{ id: "x", name: "Box" }]), unnamed: x.area.nameOfComputer("y", []) })),
    await scene("computer rows: a door that lists the provider is read as it is", { ctx: { placeDoor: { load: async () => undefined, rows: async () => [{ id: "here", name: "Mac", kind: "computer", extra: 1 }, { id: "solari", name: "Solari", kind: "provider" }] } } }, async x => x.area.computerRows()),
    await scene("image head: none sealed refuses with no image yet", { ctx: { imagePlace: async () => "solari", copyOf: async () => undefined } }, async x => ({ none: (await x.area.imageHeadOrNone()) ?? null, head: await x.area.imageHead().catch((e: Error) => ({ threw: e.message })) })),
    await scene("tree records: a thread's tree by root, on live workspaces it reaches", { live: { a: {} }, records: { "t-a": { workspaceId: "a", harness: "claude" }, "t-a-kid": { workspaceId: "a", harness: "claude", rootThreadId: "t-a" }, "t-b": { workspaceId: "a", harness: "claude", rootThreadId: "t-b" }, "t-a-gone": { workspaceId: "gone", harness: "claude", rootThreadId: "t-a" } } }, async x => ({ tree: x.area.treeRecords(thread as never, undefined).map(([id]) => id), all: x.area.treeRecords(undefined, undefined).map(([id]) => id), on: x.area.treeStandsOn("a", thread as never), person: x.area.treeStandsOn("a", undefined), talks: x.area.talksToTreeOn("a", thread as never) })),
  ];
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "boot_rows.json"), `${JSON.stringify({ T0, scenes }, null, 1)}\n`);
});
