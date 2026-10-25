// SPDX-License-Identifier: AGPL-3.0-only
// machines/machines.ts driven through scenes with a recording runtime context: any member a scene does not give is a
// function that logs its call and answers nothing. The engine's readGone, killUntilGone, applyMachineContext and
// syncDisk are recorded fakes; Date is the scene clock; randomBytes(n) answers byte k repeated n times on its k-th call.
// Pids: "self" is this process, "alive" a live other process, "dead" a pid no process holds. Written to
// tests/fixtures/oracle/machines.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

const rec = vi.hoisted(() => ({ log: [] as unknown[], reads: [] as unknown[], nonce: 0 }));
vi.mock("node:crypto", async orig => {
  const real = await orig<typeof import("node:crypto")>();
  return { ...real, randomBytes: (n: number) => Buffer.alloc(n, ++rec.nonce) };
});
vi.mock("@wsp/engine", async orig => ({
  ...(await orig<object>()),
  readGone: async (_b: unknown, id: string) => {
    rec.log.push(["readGone", id]);
    const r = rec.reads.shift() ?? "gone";
    if (r instanceof Error) throw r;
    return r;
  },
  killUntilGone: async (_b: unknown, m: { id: string }, confirm: unknown) => void rec.log.push(["killUntilGone", m.id, confirm ?? null]),
  syncDisk: async (m: { id: string }) => void rec.log.push(["syncDisk", m.id]),
  applyMachineContext: async (m: { id: string }, o: unknown) => (rec.log.push(["applyMachineContext", m.id, o]), { summary: "ok" }),
}));

const engine = await import("@wsp/engine");
const { machinesArea } = await import("../../../../packages/runtime/src/machines/machines.js");
const { fakeClock } = await import("../../../../packages/runtime/test/fake-clock.js");

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const T0 = 1_790_000_000_000;
const DEAD = 99_999_999;
const pidOf = (p: unknown): number => (p === "self" ? process.pid : p === "alive" ? process.ppid : p === "dead" ? DEAD : (p as number));
const pidName = (p: unknown): unknown => (p === process.pid ? "self" : p === process.ppid ? "alive" : p === DEAD ? "dead" : p);
const scrub = (v: unknown): unknown => JSON.parse(JSON.stringify(v, (k, x) => (k === "pid" ? pidName(x) : x)));
const flush = async (): Promise<void> => {
  for (let i = 0; i < 30; i++) await new Promise<void>(r => setImmediate(r));
};

function world(o: { store?: Record<string, Record<string, unknown>>; ctx?: Record<string, unknown>; reads?: unknown[]; goneConfirmMs?: number; backend?: unknown } = {}) {
  rec.log = [];
  rec.reads = [...(o.reads ?? [])];
  rec.nonce = 0;
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
    putBlob: async (t: string, k: string, v: unknown) => void log.push(["store.putBlob", t, k, String(v)]),
    getBlob: async (t: string, k: string) => (log.push(["store.getBlob", t, k]), undefined),
    deleteBlob: async (t: string, k: string) => void log.push(["store.deleteBlob", t, k]),
  };
  const listeners = new Map<string, Set<(e: unknown) => void>>();
  const bus = {
    on: (type: string, l: (e: unknown) => void) => {
      const set = listeners.get(type) ?? new Set();
      listeners.set(type, set);
      set.add(l);
      return () => void set.delete(l);
    },
    emit: (e: { type: string }) => {
      log.push(["emit", e]);
      for (const l of [...(listeners.get(e.type) ?? []), ...(listeners.get("*") ?? [])]) l(e);
    },
  };
  const base: Record<string, unknown> = {
    opts: { killConfirm: "confirm" }, store, bus, clock: clock.clock, goneConfirmMs: o.goneConfirmMs ?? 0, lateReadMs: 180_000, vaultCapBytes: 1000, defaultIdleWindowMs: 1_200_000, hostId: "host:ABC-1",
    live: new Map(), threadRecords: new Map(), sessions: new Map(), execs: new Set(), indexFlushes: new Map(), transcripts: new Map(), rows: { clear: (id: string) => log.push(["rows.clear", id]) }, unreadIndexes: new Set(),
    pendingEvents: new Map(), pendingBytes: new Map(), transcriptIndex: new Map(), transcriptBytes: new Map(),
    state: { owner: "h_0a993e4c", closed: false },
    backend: o.backend,
    places: { wired: "solari", backend: (p: string) => (p === "pl1" ? o.backend : undefined) },
    projectsHeld: new Map([["p_1", { id: "p_1" }]]),
    gone: { stop: async (_b: unknown, m: { id: string }) => void log.push(["gone.stop", m.id]) },
    unreached: new Map(), napRefusals: new Map(), stopRefusals: new Map(), revivedAt: new Map(), unreadAt: new Map(), polledReach: new Map(), daemonNotes: new Map(), keptAgents: new Map(), viewedMarks: new Map(), transcriptQueue: new Map(), keptWrites: new Map(),
    view: (r: { id: string; phase: string }) => ({ id: r.id, phase: r.phase }),
    persist: async (r: unknown) => void log.push(["persist", scrub(r)]),
    emitStatus: async (e: { record: { id: string } }, reach: string, said?: string) => void log.push(["emitStatus", e.record.id, reach, said ?? null]),
    reachOf: () => "reachable",
    napRefusedOf: (e: { record: { id: string } }) => (base.napRefusals as Map<string, unknown>).get(e.record.id),
    entryOf: async (id: string) => (base.live as Map<string, unknown>).get(id),
    backendFor: () => ({ id: "solari" }),
    settingsAt: () => undefined,
    slates: { forget: async (t: string) => void log.push(["slates.forget", t]) },
    moduleOf: (kind: string) => ({ dropped: async (e: { record: { id: string } }) => void log.push(["module.dropped", kind, e.record.id]) }),
    ...o.ctx,
  };
  const ctx = new Proxy(base, {
    get(target, prop) {
      if (prop in target) return target[prop as string];
      if (typeof prop !== "string" || prop === "then") return undefined;
      return (...args: unknown[]) => (log.push([prop, ...args.map(a => (typeof a === "object" && a !== null && "record" in a ? (a as { record: { id: string } }).record.id : typeof a === "function" ? "fn" : a))]), undefined);
    },
  });
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(T0);
  const warn = vi.spyOn(console, "warn").mockImplementation((line: unknown) => void log.push(["warn", String(line)]));
  const area = machinesArea(ctx as never);
  const done = () => {
    warn.mockRestore();
    vi.useRealTimers();
  };
  return { log, area, ctx: base, clock, tables, done, rows: (t: string) => scrub([...table(t)]) };
}

const machine = (id: string, o: Record<string, unknown> = {}) => {
  const { state, ...rest } = o;
  return { id, ...rest, state: async () => (rec.log.push(["machine.state", id]), (state as string) ?? "running") };
};
const backendWith = (o: { creates?: unknown[]; replays?: unknown[]; request?: true; named?: unknown[] } = {}) => {
  const creates = [...(o.creates ?? [])];
  const replays = [...(o.replays ?? [])];
  return {
    capabilities: { sizes: [] }, pricing: { defaultSize: { cpu: 1, memMb: 2048 } },
    get: async (id: string) => (rec.log.push(["backend.get", id]), machine(id)),
    list: async () => [],
    deleteSnapshot: async () => undefined,
    ...(o.request ? { createRequest: async (spec: { labels?: Record<string, string> }) => (rec.log.push(["createRequest", spec.labels ?? null]), { body: "bytes" }) } : {}),
    create: async (spec: { idempotencyKey?: string; labels?: Record<string, string> }, waiting?: (line: string, box?: { id: string }) => void, request?: unknown) => {
      rec.log.push(["backend.create", spec.idempotencyKey ?? null, spec.labels ?? null, request ?? null]);
      waiting?.("waiting for a box", { id: "box-early" });
      const next = creates.shift() ?? { id: "m-made" };
      if (next instanceof Error) throw next;
      return machine((next as { id: string }).id, next as Record<string, unknown>);
    },
    ...(o.replays !== undefined
      ? { replayCreate: async (key: string, request: unknown, createdAt: string) => {
          rec.log.push(["replayCreate", key, request, createdAt]);
          const next = replays.shift();
          if (next instanceof Error) throw next;
          return next === null ? undefined : machine((next as { id: string }).id, next as Record<string, unknown>);
        } }
      : {}),
    ...(o.named !== undefined ? { namedAfter: async (names: Set<string>) => (rec.log.push(["namedAfter", [...names]]), o.named) } : {}),
  };
};
const statusErr = (status: number, message: string) => Object.assign(new Error(message), { status });
const missing = (message = "404 not found") => Object.assign(new Error(message), { kind: "missing", status: 404 });

async function scene(name: string, w: Parameters<typeof world>[0], act: (x: ReturnType<typeof world>) => Promise<unknown>) {
  const x = world(w);
  let answer: unknown;
  try {
    answer = await act(x);
    await flush();
  } catch (e) {
    answer = { threw: (e as Error).message, kind: (e as { kind?: string }).kind ?? null };
  } finally {
    x.done();
  }
  return { name, store: w?.store ?? {}, log: x.log, answer: scrub(answer ?? null), creates: x.rows("creates"), inflight: [...(x.ctx as unknown as { inflight?: Set<string> }).inflight ?? []] };
}

const spec = { kind: "sandbox", template: "img-1", cpu: 2, memMb: 4096, labels: { a: "1", createdAt: "stamp" } };
const old = new Date(T0 - 120_000).toISOString();
const young = new Date(T0 - 119_999).toISOString();

it("machines: creates", async () => {
  const create = (x: ReturnType<typeof world>, b: unknown, purpose = "workspace/ws_1", s: unknown = spec, place?: string) =>
    x.area.claiming(purpose, async h => {
      const m = await h.create(s as never, (line: string, box?: { id: string }) => void x.log.push(["waiting", line, box?.id ?? null]));
      x.log.push(["inflight inside", [...x.area.inflight]]);
      return m.id;
    }, b as never, place);
  const scenes = [
    await scene("a keyed create puts its attempt, creates under its key and drops the attempt", {}, async x => create(x, backendWith())),
    await scene("a create with a request reads it first and keeps it on the attempt", {}, async x => create(x, backendWith({ request: true }), "workspace/ws_1", spec, "pl1")),
    await scene("a held attempt of a dead pid of this host with the same body is replayed under its key and stamp", { store: { creates: { "workspace/ws_1": { key: "workspace/ws_1:aaaaaaaaaaaaaaaa", createdAt: old, body: "", host: "host:ABC-1", pid: "dead" } } } }, async x => {
      const body = (await import("../../../../packages/runtime/src/types/internal.js")).fingerprint(spec as never);
      (x.tables.get("creates")!.get("workspace/ws_1") as { body: string }).body = body;
      return create(x, backendWith());
    }),
    await scene("a held attempt of this very process is replayed", { store: { creates: { "workspace/ws_1": { key: "k:1", createdAt: old, body: "B", host: "host:ABC-1", pid: "self" } } } }, async x => {
      (x.tables.get("creates")!.get("workspace/ws_1") as { body: string }).body = (await import("../../../../packages/runtime/src/types/internal.js")).fingerprint(spec as never);
      return create(x, backendWith());
    }),
    await scene("an attempt another live process holds is never joined", { store: { creates: { "workspace/ws_1": { key: "k:1", createdAt: old, body: "B", host: "host:ABC-1", pid: "alive" } } } }, async x => {
      (x.tables.get("creates")!.get("workspace/ws_1") as { body: string }).body = (await import("../../../../packages/runtime/src/types/internal.js")).fingerprint(spec as never);
      return create(x, backendWith());
    }),
    await scene("an attempt of another host is never joined", { store: { creates: { "workspace/ws_1": { key: "k:1", createdAt: old, body: "B", host: "host:OTHER", pid: "dead" } } } }, async x => {
      (x.tables.get("creates")!.get("workspace/ws_1") as { body: string }).body = (await import("../../../../packages/runtime/src/types/internal.js")).fingerprint(spec as never);
      return create(x, backendWith());
    }),
    await scene("a changed request starts a new attempt", { store: { creates: { "workspace/ws_1": { key: "k:1", createdAt: old, body: "other body", host: "host:ABC-1", pid: "dead" } } } }, async x => create(x, backendWith())),
    await scene("an attempt held from before its bytes were kept is never replayed where the backend sends bytes", { store: { creates: { "workspace/ws_1": { key: "k:1", createdAt: old, body: "B", host: "host:ABC-1", pid: "dead" } } } }, async x => {
      (x.tables.get("creates")!.get("workspace/ws_1") as { body: string }).body = (await import("../../../../packages/runtime/src/types/internal.js")).fingerprint(spec as never);
      return create(x, backendWith({ request: true }));
    }),
    await scene("a refusal with a status ends the attempt", {}, async x => create(x, backendWith({ creates: [statusErr(409, "conflict")] }))),
    await scene("a failure with no status keeps the attempt for a retry", {}, async x => create(x, backendWith({ creates: [new Error("socket hang up")] }))),
    await scene("a replay naming a gone machine is created anew once", {}, async x => create(x, backendWith({ creates: [{ id: "m-corpse", replayed: true, state: "gone" }, { id: "m-fresh" }] }))),
    await scene("a second gone replay is thrown", {}, async x => create(x, backendWith({ creates: [{ id: "m-corpse", replayed: true, state: "gone" }, { id: "m-corpse2", replayed: true, state: "gone" }] }))),
    await scene("a replay of a live machine is said and kept", {}, async x => create(x, backendWith({ creates: [{ id: "m-again", replayed: true, state: "running" }] }))),
    await scene("a purpose past the key's room is hashed", {}, async x => create(x, backendWith(), `workspace/${"x".repeat(240)}`)),
    await scene("a purpose exactly at the key's room is kept", {}, async x => create(x, backendWith(), `w/${"x".repeat(236)}`)),
    await scene("a spec with no created-at label is sent as it is", {}, async x => create(x, backendWith(), "workspace/ws_1", { kind: "sandbox", fromSnapshot: "snap-9", cpu: 1, memMb: 2048 })),
  ];
  const replayRow = (o: Record<string, unknown> = {}) => ({ key: "workspace/ws_9:0101010101010101", createdAt: old, body: "B", host: "host:ABC-1", pid: "dead", request: { body: "bytes" }, image: "wsp-abc-img", ...o });
  const replay = (x: ReturnType<typeof world>, known: string[] = []) => {
    const failed: unknown[] = [];
    return x.area.replayCreates(() => known, failed as never).then(deleted => ({ deleted, failed }));
  };
  scenes.push(
    await scene("replay: a lost attempt's machine no record waits for is stopped and reported lost", { store: { creates: { "workspace/ws_9": replayRow() } } , backend: backendWith({ replays: [{ id: "m-lost", labels: { w: "1" } }] }) }, async x => replay(x)),
    await scene("replay: rows it may not touch are skipped", { store: { creates: { a: replayRow({ request: undefined }), b: replayRow({ host: "host:OTHER" }), c: replayRow({ pid: "alive" }), d: replayRow({ createdAt: young }), e: replayRow({ place: "pl-none" }) } } , backend: backendWith({ replays: [] }) }, async x => replay(x)),
    await scene("replay: a row at its place is asked there", { store: { creates: { "workspace/ws_9": replayRow({ place: "pl1" }) } } , backend: backendWith({ replays: [{ id: "m-lost" }] }) }, async x => replay(x)),
    await scene("replay: a backend with no replay keeps the row", { store: { creates: { "workspace/ws_9": replayRow() } } , backend: backendWith() }, async x => replay(x)),
    await scene("replay: missing drops the row, another failure keeps it and says so", { store: { creates: { a: replayRow(), b: replayRow() } } , backend: backendWith({ replays: [missing(), new Error("timeout")] }) }, async x => replay(x)),
    await scene("replay: past the key's window the row goes and the box is the stray sweep's", { store: { creates: { a: replayRow() } } , backend: backendWith({ replays: [null] }) }, async x => replay(x)),
    await scene("replay: a known machine or one already gone is only forgotten", { store: { creates: { a: replayRow(), b: replayRow() } } , backend: backendWith({ replays: [{ id: "m-known" }, { id: "m-gone", seen: { state: "gone" } }] }) }, async x => replay(x, ["m-known"])),
    await scene("replay: a kept name whose project is held waits for the sweep to record it", { store: { creates: { "workspace/ws_9": replayRow() }, "workspace-names": { ws_9: { workspaceId: "ws_9", name: "w9", project: "p_1" } } } , backend: backendWith({ replays: [{ id: "m-lost" }] }) }, async x => replay(x)),
    await scene("replay: a kept name whose project is gone is stopped", { store: { creates: { "workspace/ws_9": replayRow() }, "workspace-names": { ws_9: { workspaceId: "ws_9", name: "w9", project: "p_gone" } } } , backend: backendWith({ replays: [{ id: "m-lost" }] }) }, async x => replay(x)),
    await scene("replay: a stop that fails keeps the row", { store: { creates: { "workspace/ws_9": replayRow() } }, ctx: { gone: { stop: async () => Promise.reject(new Error("busy")) } } , backend: backendWith({ replays: [{ id: "m-lost" }] }) }, async x => replay(x)),
    await scene("replay: a stop that finds it missing reports it", { store: { creates: { "workspace/ws_9": replayRow() } }, ctx: { gone: { stop: async () => Promise.reject(missing()) } } , backend: backendWith({ replays: [{ id: "m-lost" }] }) }, async x => replay(x)),
    await scene("strays: only this host's image names, never known or asked", { store: { creates: { a: { image: "wsp-abc1h0a993e4c-asked" } } }, ctx: { recordedImages: async () => ["wsp-abc1h0a993e4c-kept", "wsp-otherhost-x", "plain"] } }, async x => {
      const b = backendWith({ named: [
        { id: "s-old", name: "wsp-abc1h0a993e4c-kept", createdAt: new Date(T0 - 600_000).toISOString(), state: "running" },
        { id: "s-young", name: "wsp-abc1h0a993e4c-kept", createdAt: new Date(T0 - 599_999).toISOString(), state: "running" },
        { id: "s-known", name: "wsp-abc1h0a993e4c-kept", createdAt: old, state: "running" },
        { id: "s-asked", name: "wsp-abc1h0a993e4c-asked", createdAt: new Date(T0 - 900_000).toISOString(), state: "running" },
        { id: "s-gone", name: "wsp-abc1h0a993e4c-kept", createdAt: new Date(T0 - 900_000).toISOString(), state: "gone" },
        { id: "s-nodate", name: "wsp-abc1h0a993e4c-kept", state: "running" },
      ] });
      const failed: unknown[] = [];
      x.log.push(["mark", x.area.imageMark()]);
      return { deleted: await x.area.sweepStrays(() => ["s-known"], failed as never, [b as never, backendWith() as never]), failed };
    }),
    await scene("strays: nothing asks with no backend that lists by name", {}, async x => x.area.sweepStrays(() => [], [] as never, [backendWith() as never])),
  );
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "machines_creates.json"), `${JSON.stringify({ T0, scenes }, null, 1)}\n`);
});

it("machines: phases", async () => {
  type Ent = { record: Record<string, unknown>; machine: Record<string, unknown>; ws: Record<string, unknown>; [k: string]: unknown };
  const entryIn = (x: ReturnType<typeof world>, o: { phase?: string; nap?: unknown; machineState?: string | Error; prove?: unknown; sub?: true; record?: Record<string, unknown> } = {}): Ent => {
    const e: Ent = {
      record: { id: "ws_1", name: "w1", kind: "cloud", machineId: "m-1", phase: o.phase ?? "running", ...(o.sub ? { sub: { on: "solari" } } : {}), ...o.record },
      machine: { id: "m-1", state: async () => {
        x.log.push(["machine.state"]);
        if (o.machineState instanceof Error) throw o.machineState;
        return o.machineState ?? "running";
      }, ...(o.prove !== undefined ? { proveRoad: o.prove } : {}) },
      ws: {
        currentPhase: "running", machineId: "m-1", isFirstLife: false,
        nap: async () => {
          x.log.push(["ws.nap"]);
          if (o.nap instanceof Error) throw o.nap;
        },
        notePaused: () => x.log.push(["ws.notePaused"]),
        noteRunning: () => x.log.push(["ws.noteRunning"]),
      },
      generation: 0,
    };
    (x.ctx.live as Map<string, unknown>).set("ws_1", e);
    return e;
  };
  const after = (x: ReturnType<typeof world>, e: Ent) => ({ record: scrub(e.record), marks: Object.keys(e).filter(k => !["record", "machine", "ws", "generation"].includes(k) && e[k] !== undefined).sort(), napRefusals: [...(x.ctx.napRefusals as Map<string, unknown>)], stopRefusals: [...(x.ctx.stopRefusals as Map<string, unknown>)], rereading: [...x.area.rereading] });
  const sessionsOf = (x: ReturnType<typeof world>) => {
    (x.ctx.sessions as Map<string, unknown>).set("s1", { view: { workspaceId: "ws_1", status: "running" }, end: (r: string) => x.log.push(["session.end", "s1", r]) });
    (x.ctx.sessions as Map<string, unknown>).set("s2", { view: { workspaceId: "ws_2", status: "running" }, end: (r: string) => x.log.push(["session.end", "s2", r]) });
    (x.ctx.execs as Set<unknown>).add({ workspaceId: "ws_1", end: (r: string) => x.log.push(["exec.end", r]) });
  };
  const scenes = [
    await scene("nap: pausing is written and pushed before the provider is asked, then napping", {}, async x => {
      const e = entryIn(x);
      sessionsOf(x);
      (x.ctx.napRefusals as Map<string, unknown>).set("ws_1", { machineId: "m-1", said: "old" });
      e.unchecked = true;
      x.clock.advance(5);
      const a = x.area.napWith("ws_1", "idle 20 min");
      const b = x.area.napWith("ws_1");
      x.log.push(["joined", a === b]);
      return { answer: await a, ...after(x, e) };
    }),
    await scene("nap: a record not running answers its view", {}, async x => {
      const e = entryIn(x, { phase: "napping", machineState: "paused" });
      return { answer: await x.area.napWith("ws_1"), ...after(x, e) };
    }),
    await scene("nap: a napping record over a running machine is adopted first, then napped", {}, async x => {
      const e = entryIn(x, { phase: "napping", machineState: "running" });
      return { answer: await x.area.napWith("ws_1"), ...after(x, e) };
    }),
    await scene("nap: a refused pause puts the engine's phase back and stands the refusal", {}, async x => {
      const e = entryIn(x, { nap: new engine.NapRefusedError("m-1", "provider will not pause") });
      await x.area.napWith("ws_1").catch((err: Error) => x.log.push(["rejected", err.message]));
      await x.area.napWith("ws_1").catch((err: Error) => x.log.push(["rejected", err.message]));
      return after(x, e);
    }),
    await scene("nap: a refused stop is kept too", {}, async x => {
      const e = entryIn(x, { nap: new engine.StopRefusedError("m-1", "stop refused") });
      await x.area.napWith("ws_1").catch((err: Error) => x.log.push(["rejected", err.message]));
      return after(x, e);
    }),
    await scene("nap: a 404 confirmed gone settles gone and is thrown", { reads: ["gone"] }, async x => {
      const e = entryIn(x, { nap: missing() });
      sessionsOf(x);
      await x.area.napWith("ws_1").catch((err: Error) => x.log.push(["rejected", err.message]));
      return after(x, e);
    }),
    await scene("nap: a 404 the reads do not confirm takes the refused road", { reads: ["running"] }, async x => {
      const e = entryIn(x, { nap: missing() });
      await x.area.napWith("ws_1").catch((err: Error) => x.log.push(["rejected", err.message]));
      return after(x, e);
    }),
    await scene("adopt pause: the record follows a pause outside wsp", {}, async x => {
      const e = entryIn(x);
      sessionsOf(x);
      e.unchecked = true;
      e.record.diskGone = "x";
      await x.area.adoptPause(e as never);
      return after(x, e);
    }),
    await scene("adopt pause: not while napping, waking or not running", {}, async x => {
      const e = entryIn(x);
      e.napping = Promise.resolve();
      await x.area.adoptPause(e as never);
      delete e.napping;
      e.waking = Promise.resolve();
      await x.area.adoptPause(e as never);
      delete e.waking;
      e.record.phase = "napping";
      await x.area.adoptPause(e as never);
      return after(x, e);
    }),
    await scene("adopt running: the record follows a machine the provider runs, joined", {}, async x => {
      const e = entryIn(x, { phase: "napping", record: { gone: "old words", firstLife: true } });
      const a = x.area.adoptRunning(e as never);
      const b = x.area.adoptRunning(e as never);
      x.log.push(["joined", a === b]);
      await a;
      return after(x, e);
    }),
    await scene("runs under napping: only a napping record, a failed read reads paused", {}, async x => {
      const e = entryIn(x, { phase: "napping", machineState: new Error("timeout") });
      const r1 = await x.area.runsUnderNapping(e as never);
      e.record.phase = "running";
      const r2 = await x.area.runsUnderNapping(e as never);
      return { r1, r2 };
    }),
    await scene("settle gone: a confirmed gone marks the record and ends its sessions", { reads: ["gone"], goneConfirmMs: 0 }, async x => {
      const e = entryIn(x, { record: { firstLife: true } });
      sessionsOf(x);
      e.unchecked = true;
      return { outcome: await x.area.settleGone(e as never, "machine m-1 is gone"), ...after(x, e) };
    }),
    await scene("settle gone: a read that says running is not gone", { reads: ["running"] }, async x => {
      const e = entryIn(x);
      return { outcome: await x.area.settleGone(e as never, "the poll found it"), ...after(x, e) };
    }),
    await scene("settle gone: reads that fail leave it unchecked", { reads: [new Error("503 down")] }, async x => {
      const e = entryIn(x);
      return { outcome: await x.area.settleGone(e as never, "the poll found it"), ...after(x, e) };
    }),
    await scene("mark gone: moot when already gone, on another machine, dropped or deleting", {}, async x => {
      const e = entryIn(x);
      const outs = [await x.area.markGone(e as never, "m-other", "r")];
      e.deleting = Promise.resolve();
      outs.push(await x.area.markGone(e as never, "m-1", "r"));
      delete e.deleting;
      (x.ctx.live as Map<string, unknown>).delete("ws_1");
      outs.push(await x.area.markGone(e as never, "m-1", "r"));
      (x.ctx.live as Map<string, unknown>).set("ws_1", e);
      e.record.phase = "gone";
      outs.push(await x.area.markGone(e as never, "m-1", "r"));
      return { outs, ...after(x, e) };
    }),
    await scene("mark gone: a sub-thread's record is released by its life", { ctx: { subLost: async (e: { record: { id: string } }, r: string) => (rec.log.push(["subLost", e.record.id, r]), "settled") } }, async x => {
      const e = entryIn(x, { sub: true });
      return { outcome: await x.area.markGone(e as never, "m-1", "r"), ...after(x, e) };
    }),
    await scene("adopt gone: deferred to a nap or wake, dropped while reread, unchecked said", { reads: [new Error("503 down")] }, async x => {
      const e = entryIn(x);
      e.napping = Promise.resolve();
      await x.area.adoptGone(e as never, "seen");
      delete e.napping;
      const first = x.area.adoptGone(e as never, "seen");
      await x.area.adoptGone(e as never, "seen again");
      await first;
      return after(x, e);
    }),
    await scene("recover gone: a running machine brings the record back running", {}, async x => {
      const e = entryIn(x, { phase: "gone", machineState: "running", record: { gone: "words", diskGone: "d" } });
      e.unchecked = true;
      return { phase: await x.area.recoverGone(e as never), ...after(x, e) };
    }),
    await scene("recover gone: a paused machine brings it back napping", {}, async x => {
      const e = entryIn(x, { phase: "gone", machineState: "paused", record: { gone: "words", diskGone: "d" } });
      return { phase: await x.area.recoverGone(e as never), ...after(x, e) };
    }),
    await scene("recover gone: gone, unknown or failed reads move nothing", {}, async x => {
      const e = entryIn(x, { phase: "gone", machineState: "gone" });
      const a = await x.area.recoverGone(e as never);
      e.machine.state = async () => Promise.reject(new Error("x"));
      const b = await x.area.recoverGone(e as never);
      return { a: a ?? null, b: b ?? null, ...after(x, e) };
    }),
    await scene("status: a running row the poll reads paused is adopted as a pause", {}, async x => {
      const e = entryIn(x);
      (x.ctx.bus as { emit: (e: unknown) => void }).emit({ type: "workspace.status", status: { id: "ws_1", phase: "running", machineState: "paused" } });
      (x.ctx.bus as { emit: (e: unknown) => void }).emit({ type: "workspace.status", status: { id: "ws_none", phase: "running", machineState: "paused" } });
      await flush();
      return after(x, e);
    }),
    await scene("proven: a machine with no proof road is ready at once", {}, async x => {
      const e = entryIn(x);
      e.unchecked = true;
      return { view: await x.area.proven(e as never), ...after(x, e) };
    }),
    await scene("proven: waking while the proof runs, running after, joined", {}, async x => {
      let land!: () => void;
      const e = entryIn(x, { prove: () => new Promise<void>(r => (land = r)) });
      e.unchecked = true;
      const a = x.area.proven(e as never);
      const b = x.area.proven(e as never);
      await flush();
      x.log.push(["during", e.record.phase, a === b]);
      land();
      return { view: await a, ...after(x, e) };
    }),
    await scene("proven: a stop ends it with the stopped line", {}, async x => {
      const e = entryIn(x, { prove: () => new Promise<void>(() => undefined) });
      e.unchecked = true;
      const a = x.area.proven(e as never);
      await flush();
      (e.wakeStop as AbortController).abort();
      await a.catch((err: Error) => x.log.push(["rejected", err.message]));
      return after(x, e);
    }),
    await scene("proven: a restore not finished says no commands yet", {}, async x => {
      const e = entryIn(x, { prove: () => Promise.reject(new engine.RestoreUnfinishedError("m-1", new Error("disk streaming"))) });
      await x.area.proven(e as never).catch((err: Error) => x.log.push(["rejected", err.message]));
      return after(x, e);
    }),
    await scene("proven: any other failure is said and thrown as itself", {}, async x => {
      const e = entryIn(x, { prove: () => Promise.reject(new Error("exec refused")) });
      await x.area.proven(e as never).catch((err: Error) => x.log.push(["rejected", err.message]));
      return after(x, e);
    }),
    await scene("proven: a machine the provider lost takes the gone road", { reads: ["gone"] }, async x => {
      const e = entryIn(x, { prove: () => Promise.reject(missing("404 no such box")) });
      await x.area.proven(e as never).catch((err: Error) => x.log.push(["rejected", err.message]));
      return after(x, e);
    }),
    await scene("proven: a delete that took the record has the last word", {}, async x => {
      let land!: () => void;
      const e = entryIn(x, { prove: () => new Promise<void>(r => (land = r)) });
      const a = x.area.proven(e as never);
      await flush();
      (x.ctx.live as Map<string, unknown>).delete("ws_1");
      land();
      await a.catch((err: Error) => x.log.push(["rejected", err.message, (err as { kind?: string }).kind ?? null]));
      return after(x, e);
    }),
    await scene("drop: everything the workspace left goes, in order, and the event last", { store: { "agent-lists": { "m-1:claude": {}, "m-1:codex": {}, "m-2:claude": {} } } }, async x => {
      const e = entryIn(x, { sub: true });
      e.lateRead = () => x.log.push(["lateRead cancelled"]);
      e.prPoll = () => x.log.push(["prPoll cancelled"]);
      const threads = x.ctx.threadRecords as Map<string, unknown>;
      threads.set("t1", { workspaceId: "ws_1" });
      threads.set("t9", { workspaceId: "ws_9" });
      (x.ctx.sessions as Map<string, unknown>).set("s1", { view: { workspaceId: "ws_1", threadId: "t2" } });
      (x.ctx.sessions as Map<string, unknown>).set("s9", { view: { workspaceId: "ws_9", threadId: "t9" } });
      (x.ctx.keptAgents as Map<string, unknown>).set("t1", { workspaceId: "ws_1" });
      await x.area.drop("ws_1");
      return { live: [...(x.ctx.live as Map<string, unknown>).keys()], threads: [...threads.keys()], sessions: [...(x.ctx.sessions as Map<string, unknown>).keys()], agentLists: x.rows("agent-lists") };
    }),
  ];
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "machines_phases.json"), `${JSON.stringify({ T0, scenes }, null, 1)}\n`);
});
