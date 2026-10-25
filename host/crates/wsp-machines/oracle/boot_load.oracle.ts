// SPDX-License-Identifier: AGPL-3.0-only
// machines/boot.ts's builders and hydration driven through scenes with a recording runtime context (any member a
// scene does not give logs its call and answers nothing). The engine's readGone and sightMachine are recorded fakes;
// Date is the scene clock. Pids: "self", "alive" (a live other process), "dead". Written to
// tests/fixtures/oracle/boot_load.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

const rec = vi.hoisted(() => ({ log: [] as unknown[], gone: [] as unknown[], sights: [] as unknown[] }));
vi.mock("@wsp/engine", async orig => ({
  ...(await orig<object>()),
  readGone: async (_b: unknown, id: string) => (rec.log.push(["readGone", id]), (rec.gone.shift() as string) ?? "gone"),
  sightMachine: async (_b: unknown, id: string) => {
    rec.log.push(["sightMachine", id]);
    const s = rec.sights.shift();
    if (s instanceof Error) throw s;
    return s ?? { state: "gone" };
  },
}));
const { bootArea } = await import("../../../../packages/runtime/src/machines/boot.js");
const { fakeClock } = await import("../../../../packages/runtime/test/fake-clock.js");

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const T0 = 1_790_000_000_000;
const DEAD = 99_999_999;
const pidOf = (p: unknown): number => (p === "self" ? process.pid : p === "alive" ? process.ppid : p === "dead" ? DEAD : (p as number));
const scrub = (v: unknown): unknown => JSON.parse(JSON.stringify(v ?? null, (k, x) => (k === "pid" ? (x === process.pid ? "self" : x === process.ppid ? "alive" : x === DEAD ? "dead" : x) : x)));
const flush = async (): Promise<void> => {
  for (let i = 0; i < 30; i++) await new Promise<void>(r => setImmediate(r));
};
type Answer = { state?: string; labels?: Record<string, string>; seen?: Record<string, unknown>; streamUrl?: string; throws?: Record<string, unknown>; hang?: true; stateThrows?: true };

function world(o: { store?: Record<string, Record<string, unknown>>; answers?: Record<string, Answer>; ctx?: Record<string, unknown>; gone?: string[]; sights?: unknown[]; snapshotsAnyLife?: boolean; noBackendAt?: string[] } = {}) {
  rec.log = [];
  rec.gone = [...(o.gone ?? [])];
  rec.sights = [...(o.sights ?? [])];
  const log = rec.log;
  const clock = fakeClock(T0);
  const tables = new Map<string, Map<string, unknown>>();
  for (const [t, rows] of Object.entries(o.store ?? {})) tables.set(t, new Map(Object.entries(rows).map(([k, v]) => [k, JSON.parse(JSON.stringify(v), (key, x) => (key === "pid" ? pidOf(x) : x))])));
  const table = (t: string) => tables.get(t) ?? (tables.set(t, new Map()), tables.get(t)!);
  const store = {
    get: async (t: string, k: string) => table(t).get(k),
    put: async (t: string, k: string, v: unknown) => (log.push(["store.put", t, k, scrub(v)]), void table(t).set(k, v)),
    delete: async (t: string, k: string) => (log.push(["store.delete", t, k]), void table(t).delete(k)),
    keys: async (t: string) => [...table(t).keys()],
    list: async (t: string) => [...table(t).values()],
  };
  const pending: (() => void)[] = [];
  const machineFor = (id: string, a: Answer) => ({
    id, kind: "sandbox",
    ...(a.labels !== undefined ? { labels: a.labels } : {}),
    ...(a.seen !== undefined ? { seen: a.seen } : {}),
    ...(a.streamUrl !== undefined ? { streamUrl: a.streamUrl } : {}),
    state: async () => {
      log.push(["machine.state", id]);
      if (a.stateThrows) throw new Error("state read failed");
      return a.state ?? "running";
    },
  });
  const backend = (place: string) => ({
    capabilities: { snapshotsAnyLife: o.snapshotsAnyLife === true },
    pricing: { defaultSize: { cpu: 1, memMb: 2048 } },
    get: async (id: string) => {
      log.push(["backend.get", place, id]);
      const a = o.answers?.[id] ?? {};
      if (a.hang) await new Promise<void>(r => pending.push(r));
      if (a.throws !== undefined) throw Object.assign(new Error(String(a.throws.message ?? "refused")), a.throws);
      return machineFor(id, a);
    },
  });
  const live = new Map<string, unknown>();
  const builders = new Map<string, unknown>();
  const base: Record<string, unknown> = {
    store, clock: clock.clock, hostId: "host:A", live, builders, local: undefined, placeDoor: undefined, rows: undefined,
    bus: { on: () => () => undefined, emit: (e: unknown) => void log.push(["emit", e]), pass: (e: unknown) => void log.push(["pass", e]) },
    threadRecords: new Map(), sessions: new Map(), transcriptIndex: new Map(), projectsHeld: new Map(),
    state: { owner: "h_me", closed: false, closing: false, readsSince: 0 },
    places: { wired: "solari", backend: (p: string) => (o.noBackendAt?.includes(p) ? undefined : backend(p)) },
    rereading: new Set<string>(),
    observed: (m: unknown) => m,
    shapeOf: async (m: { id: string }) => (log.push(["shapeOf", m.id]), { cpu: 2, memMb: 4096 }),
    sizeBuilt: (shape: unknown, d: unknown) => (log.push(["sizeBuilt", shape, d]), { cpu: 2, memMb: 4096 }),
    armGrace: (id: string, at: unknown) => void log.push(["armGrace", id, at]),
    moduleOf: (kind: string) => ({ backend: () => (kind === "unwired" ? (() => { throw new Error("no module wired for unwired"); })() : backend("solari")) }),
    imageOf: async (g: string) => (log.push(["imageOf", g]), g === "desktop-img" ? { version: { kind: "desktop" } } : g === "bad" ? Promise.reject(new Error("no image")) : {}),
    attach: (record: Record<string, unknown>, machine: { id: string; kind: string }) => {
      log.push(["attach", scrub(record), machine.id, machine.kind]);
      const entry = { record, machine };
      live.set(record.id as string, entry);
      return entry;
    },
    attachQueued: (record: { id: string }) => void log.push(["attachQueued", record.id]),
    persist: async (r: unknown) => void log.push(["persist", scrub(r)]),
    idle: { touch: (id: string) => void log.push(["idle.touch", id]) },
    syncDaemon: async (e: { record: { id: string } }) => void log.push(["syncDaemon", e.record.id]),
    backendFor: () => backend("solari"),
    markGone: async (e: { record: { id: string } }, m: string, words: string) => void log.push(["markGone", e.record.id, m, words]),
    ...o.ctx,
  };
  const ctx = new Proxy(base, {
    get(target, prop) {
      if (prop in target) return target[prop as string];
      if (typeof prop !== "string" || prop === "then") return undefined;
      return (...args: unknown[]) => (log.push([prop, ...args.map(a => (typeof a === "function" ? "fn" : a))]), undefined);
    },
  });
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(T0);
  const warn = vi.spyOn(console, "warn").mockImplementation((line: unknown) => void log.push(["warn", String(line)]));
  const area = bootArea(ctx as never);
  return { log, area, base, clock, pending, live, builders, tables, done: () => (warn.mockRestore(), vi.useRealTimers()), rows: (t: string) => scrub([...table(t)]) };
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
  const lives = [...x.builders].map(([id, b]) => ({ id, life: (b as { life: string }).life, sealable: (b as { sealable: boolean }).sealable, firstLife: (b as { record: { firstLife: boolean } }).record.firstLife, size: (b as { record: { size: unknown } }).record.size, builder: scrub({ ...(b as { builder: Record<string, unknown> }).builder, machine: undefined }) }));
  return { name, store: w?.store ?? {}, answers: w?.answers ?? {}, log: x.log, answer: scrub(answer === undefined ? null : typeof answer === "object" && answer !== null && "record" in answer ? { entry: (answer as { record: { id: string } }).record.id } : answer), builders: lives, live: [...x.live.keys()] };
}

const builder = (id: string, o: Record<string, unknown> = {}) => ({ id, name: `b-${id}`, kind: "sandbox", baseTemplate: "base", setupSha: "sha", createdAt: "2026-09-01T00:00:00.000Z", firstLife: true, ...o });
const beat = (ms: number) => new Date(T0 - ms).toISOString();

it("boot: builders", async () => {
  const scenes = [
    await scene("admit: every life a stored builder can read as", {
      store: { builders: {
        foreign: builder("foreign"),
        mineHeld: builder("mineHeld", { heldBy: { host: "host:A", pid: "self", heartbeat: beat(0) } }),
        otherHostFresh: builder("otherHostFresh", { heldBy: { host: "host:B", pid: "dead", heartbeat: beat(15 * 60_000 - 1) } }),
        otherHostStale: builder("otherHostStale", { heldBy: { host: "host:B", pid: "dead", heartbeat: beat(15 * 60_000) } }),
        unreadableBeat: builder("unreadableBeat", { heldBy: { host: "host:B", pid: "dead", heartbeat: "garbage" } }),
        sameHostLivePid: builder("sameHostLivePid", { heldBy: { host: "host:A", pid: "alive", heartbeat: beat(0) } }),
        sameHostDeadPid: builder("sameHostDeadPid", { heldBy: { host: "host:A", pid: "dead", heartbeat: beat(0) } }),
        building: builder("building", { building: true }),
        paused: builder("paused"),
        notFirst: builder("notFirst", { firstLife: false }),
        sealed: builder("sealed", { sealed: { version: 3, at: 1_789_999_000_000 }, size: { cpu: 4, memMb: 8192 } }),
      } },
      answers: { foreign: { labels: { "wsp-owner": "h_other" } }, paused: { state: "paused" }, sealed: { seen: { state: "running" }, labels: { "wsp-owner": "h_me" } } },
    }, async x => x.area.refreshBuilders()),
    await scene("admit: a provider whose copy is the disk as it stands seals any life", { snapshotsAnyLife: true, store: { builders: { notFirst: builder("notFirst", { firstLife: false }) } } }, async x => x.area.refreshBuilders()),
    await scene("admit: unreadable places leave the record, a confirmed gone drops it, another error is thrown", {
      noBackendAt: ["away"],
      gone: ["gone", "running"],
      store: { builders: { away: builder("away", { place: "away" }), absent: builder("absent"), noKey: builder("noKey"), gone: builder("gone"), notGone: builder("notGone"), boom: builder("boom") } },
      answers: { absent: { throws: { kind: "absent" } }, noKey: { throws: { kind: "noProvider" } }, gone: { throws: { kind: "missing" } }, notGone: { throws: { kind: "missing" } }, boom: { throws: { message: "500 down" } } },
    }, async x => x.area.refreshBuilders().catch((e: Error) => x.log.push(["rejected", e.message]))),
    await scene("refresh: own builders stay, others re-derive, dropped rows go", { store: { builders: { a: builder("a", { firstLife: false }), b: builder("b") } } }, async x => {
      x.builders.set("own1", { life: "own", record: builder("own1"), builder: { machine: { id: "own1" }, firstLife: true } });
      x.builders.set("gone1", { life: "reusable", record: builder("gone1"), builder: { machine: { id: "gone1" }, firstLife: true } });
      x.builders.set("a", { life: "reusable", record: builder("a", { size: { cpu: 9, memMb: 9 } }), builder: { machine: { id: "a", labels: {} }, firstLife: true }, sealable: true });
      await x.area.refreshBuilders();
    }),
    await scene("refresh: two admits of one row share one read", { store: { builders: { slow: builder("slow") } }, answers: { slow: { hang: true } } }, async x => {
      const a = x.area.refreshBuilders();
      const b = x.area.refreshBuilders();
      await flush();
      x.pending.shift()!();
      await Promise.all([a, b]);
    }),
    await scene("admit: a host closed while the read was out admits nothing", { store: { builders: { slow: builder("slow") } }, answers: { slow: { hang: true } } }, async x => {
      const a = x.area.refreshBuilders();
      await flush();
      (x.base.state as { closed: boolean }).closed = true;
      x.pending.shift()!();
      await a;
    }),
  ];
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "boot_builders.json"), `${JSON.stringify({ T0, scenes }, null, 1)}\n`);
});

it("boot: hydrate", async () => {
  const ws = (id: string, o: Record<string, unknown> = {}) => ({ id, name: `w-${id}`, kind: "cloud", machineId: `m-${id}`, phase: "running", golden: "img", project: "p", ...o });
  const one = async (name: string, stored: Record<string, unknown>, answer: Answer, ctx: Record<string, unknown> = {}) =>
    scene(name, { answers: { [stored.machineId as string]: answer }, ctx }, async x => x.area.hydrateWorkspace(stored));
  const phases = ["running", "napping", "pausing", "waking", "gone"];
  const reads: [string, Answer][] = [["running", { state: "running" }], ["paused", { state: "paused" }], ["starting", { state: "starting" }], ["seen-paused", { seen: { state: "paused" } }], ["404", { throws: { kind: "missing", status: 404, message: "no such machine" } }], ["refused", { throws: { message: "503 busy" } }], ["state-fails", { stateThrows: true }]];
  const table = [];
  for (const phase of phases) for (const [read, answer] of reads) table.push(await one(`phase ${phase}, provider ${read}`, ws("a", { phase, ...(phase === "gone" ? { gone: "kept words" } : {}) }), answer));
  const scenes = [
    ...table,
    await one("a queued spawn waits as it was", ws("a", { phase: "queued" }), {}),
    await one("a released record stands on no machine", ws("a", { phase: "released" }), {}),
    await one("a sub whose cloud left is not read", ws("a", { sub: { cloudGone: true } }), {}),
    await one("a kind with no module is left as it was", ws("a", { kind: "unwired" }), {}),
    await one("a desktop golden's stand-in is a desktop", ws("a", { golden: "desktop-img" }), { throws: { message: "503 busy" } }),
    await one("a folder record names no image and reads none", ws("a", { golden: "" }), { throws: { message: "503 busy" } }),
    await one("an image that does not read is a sandbox", ws("a", { golden: "bad" }), { throws: { message: "503 busy" } }),
    await one("a nap the provider read moves nappedAt and writes once", ws("a", { phase: "napping", nappedAt: 5 }), { state: "paused", seen: { state: "paused", pausedAt: new Date(T0 - 1000).toISOString() } }),
    await one("the same nappedAt writes nothing", ws("a", { phase: "napping", nappedAt: T0 - 1000 }), { seen: { state: "paused", pausedAt: new Date(T0 - 1000).toISOString() } }),
    await one("a nap clears the disk-gone line a wake left", ws("a", { phase: "running", diskGone: "deleted" }), { state: "paused" }),
    await one("a stream url rides the record", ws("a"), { state: "running", streamUrl: "wss://screen" }),
    await one("a running record that napped or woke is unchecked", ws("a", { phase: "waking" }), { state: "running" }),
    await scene("a found machine is read once from the sighting", {}, async x => x.area.hydrateWorkspace(ws("a", { phase: "napping" }), { state: "running", machine: { id: "m-a", seen: { state: "running" }, state: async () => (x.log.push(["machine.state"]), "running") } } as never)),
    await scene("the load's deadline holds a late read and loads it when it lands", { answers: { "m-a": { hang: true, state: "running" } } }, async x => {
      const loadOut = x.area.hydrateWorkspace(ws("a"), undefined, { until: T0 + 1500, late: new Map() } as never);
      await flush();
      x.clock.advance(1499);
      await flush();
      x.log.push(["mark", "at 1499"]);
      x.clock.advance(1);
      const first = await loadOut;
      x.log.push(["answered", first === undefined ? null : "entry", "isHeldAway", x.area.isHeldAway("a"), "isHeldAway of none", x.area.isHeldAway("none")]);
    }),
  ];
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "boot_hydrate.json"), `${JSON.stringify({ T0, scenes }, null, 1)}\n`);
});

it("boot: reread held", async () => {
  const ws = (id: string, o: Record<string, unknown> = {}) => ({ id, name: `w-${id}`, kind: "cloud", machineId: `m-${id}`, phase: "running", golden: "img", project: "p", ...o });
  const held = async (x: ReturnType<typeof world>, answer: Answer, record = ws("a")) => {
    (x.tables.get("workspaces") ?? (x.tables.set("workspaces", new Map()), x.tables.get("workspaces")!)).set("a", record);
    await x.area.hydrateWorkspace(record, undefined, undefined, Promise.reject(Object.assign(new Error(String(answer.throws?.message ?? "x")), answer.throws)));
  };
  const scenes = [
    await scene("a held record the provider finds is loaded onto and synced", { sights: [{ state: "running", machine: { id: "m-a", seen: { state: "running" } } }] }, async x => {
      await held(x, { throws: { message: "503 busy" } });
      x.log.push(["mark", "reread"]);
      await x.area.rereadHeld("a", "sweep");
    }),
    await scene("a held record read gone settles gone with the load's answer", { sights: [{ state: "gone" }] }, async x => {
      await held(x, { throws: { kind: "missing", status: 404, message: "no such machine" } });
      x.log.push(["mark", "reread"]);
      await x.area.rereadHeld("a", "record load");
      x.log.push(["isHeldAway after", x.area.isHeldAway("a")]);
    }),
    await scene("a sighting that fails leaves it held", { sights: [new Error("timeout")] }, async x => {
      await held(x, { throws: { message: "503 busy" } });
      await x.area.rereadHeld("a", "sweep");
      x.log.push(["isHeldAway after", x.area.isHeldAway("a")]);
    }),
    await scene("a record not held, or being reread, is not read", {}, async x => {
      x.live.set("a", { record: ws("a"), machine: { id: "m-a" } });
      await x.area.rereadHeld("a", "sweep");
      await x.area.rereadHeld("none", "sweep");
    }),
  ];
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "boot_reread.json"), `${JSON.stringify({ T0, scenes }, null, 1)}\n`);
});
