// SPDX-License-Identifier: AGPL-3.0-only
// machines/create.ts driven through scenes with a fake runtime context; the landing road, the branch of record and
// the engine's kill are recorded fakes. Every call, stage, event, warning and the record as persisted, in order, go to
// tests/fixtures/oracle/create.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

const rec = vi.hoisted(() => ({ log: [] as unknown[], road: {} as Record<string, unknown> }));
vi.mock("../../../../packages/runtime/src/project-landing.js", async orig => ({
  ...(await orig<object>()),
  projectLanding: (kind: string) => {
    rec.log.push(["projectLanding", kind]);
    const r = rec.road;
    return {
      kind: r.kind ?? "provider",
      refusal: (project: { id: string }, deps: unknown) => (rec.log.push(["road.refusal", project.id, deps]), r.refusal),
      places: (o: { memoryKey?: string }) => (rec.log.push(["road.places", o.memoryKey ?? null]), { memoryDir: r.memoryDir ?? "/mem" }),
      workspaceBinds: (p: { id: string }) => (rec.log.push(["road.workspaceBinds", p.id]), r.binds ?? []),
    };
  },
}));
vi.mock("../../../../packages/runtime/src/projects/branch-of-record.js", async orig => ({
  ...(await orig<object>()),
  brokeredStart: (_ctx: unknown, provider: boolean) => (rec.log.push(["brokeredStart", provider]), rec.road.brokered === true && provider),
  namedOfRecord: async (_ctx: unknown, o: unknown) => (rec.log.push(["namedOfRecord", o]), { owner: "acme", name: "site", branch: "wsp/w1", public: false }),
}));
vi.mock("@wsp/engine", async orig => ({
  ...(await orig<object>()),
  killUntilGone: async (b: { id: string }, m: { id: string }, confirm: unknown) => void rec.log.push(["killUntilGone", b.id, m.id, confirm ?? null]),
}));

const { createArea } = await import("../../../../packages/runtime/src/machines/create.js");
const { fakeClock } = await import("../../../../packages/runtime/test/fake-clock.js");

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const T0 = 1_790_000_000_000;

interface Scene {
  o?: Record<string, unknown>;
  project?: Record<string, unknown>;
  spawned?: Record<string, unknown>;
  childBase?: string;
  placeId?: string;
  keepsImages?: boolean;
  sizes?: { cpu: number; memMb: number }[];
  forks?: ("ok" | "cap" | "cap-status" | "fails")[];
  builders?: { id: string; name: string; life: string; sealed?: number; place?: string }[];
  liveOthers?: { name: string; holds: boolean }[];
  machine?: { notice?: string; previewUrl?: string; daemonAnswers?: true };
  served?: boolean;
  reach?: "ok" | "fails";
  fault?: string;
  memory?: { cpu: number; memMb: number } | false;
  road?: Record<string, unknown>;
  landFails?: boolean;
}

async function run(name: string, s: Scene) {
  rec.log = [];
  rec.road = s.road ?? {};
  const log = rec.log;
  const clock = fakeClock(T0);
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
  const live = new Map<string, unknown>();
  for (const [i, o] of (s.liveOthers ?? []).entries()) live.set(`other${i}`, { record: { id: `other${i}`, name: o.name, holds: o.holds } });
  const builders = new Map((s.builders ?? []).map(b => [b.id, { life: b.life, record: { id: b.id, name: b.name, ...(b.sealed !== undefined ? { sealed: { version: b.sealed, at: 1 } } : {}), ...(b.place !== undefined ? { place: b.place } : {}) }, builder: { machine: { id: `bm-${b.id}` } } }]));
  const forks = [...(s.forks ?? ["ok"])];
  const project = { id: "p_1", name: "site", computer: "here", path: "/src/site", remote: "git@github.com:acme/site.git", memoryKey: "mk", memoryDir: "/mem", ...s.project };
  const ctx = {
    opts: { killConfirm: "confirm" },
    placeDoor: { markUsed: async (p: string | undefined) => void log.push(["markUsed", p ?? null]) },
    bus, clock: clock.clock, live, builders,
    places: { wired: "solari" },
    landingPlace: async (computer: string) => (log.push(["landingPlace", computer]), s.placeId !== undefined ? { placeId: s.placeId } : {}),
    placeRefuses: async (p: string | undefined) => void log.push(["placeRefuses", p ?? null]),
    landingBackend: async (p: string | undefined) => (log.push(["landingBackend", p ?? null]), { id: "landing", pricing: { defaultSize: { cpu: 1, memMb: 2048 } }, capabilities: { sizes: s.sizes ?? [{ cpu: 1, memMb: 2048, rateUsdPerHour: 0.05 }, { cpu: 2, memMb: 4096, rateUsdPerHour: 0.1 }, { cpu: 4, memMb: 8192, rateUsdPerHour: 0.2 }] } }),
    landingKind: (computer: string, at: { id: string }) => (log.push(["landingKind", computer, at.id]), "provider"),
    landingDeps: async (computer: string) => (log.push(["landingDeps", computer]), { deps: { on: computer } }),
    rememberProject: async (p: { memoryDir: string }) => void log.push(["rememberProject", p.memoryDir]),
    projectHeld: (id: string) => (log.push(["projectHeld", id]), { id }),
    keepsImages: (at: { id: string }) => (log.push(["keepsImages", at.id]), s.keepsImages ?? true),
    copyForFork: async (golden: string, placeId: string | undefined, building: (where: string, rate: number) => void) => {
      log.push(["copyForFork", golden, placeId ?? null]);
      if (placeId !== undefined) building("box", 0.5);
      return `${golden}@copy`;
    },
    imageHead: async () => (log.push(["imageHead"]), "snap-head"),
    imageOf: async (g: string) => (log.push(["imageOf", g]), { version: { size: { cpu: 2, memMb: 4096 } } }),
    placeName: (p: string) => (log.push(["placeName", p]), `name-of-${p}`),
    spawnAt: (p: string) => (log.push(["spawnAt", p]), { machines: 3, threads: 2 }),
    treeOf: (sp: unknown) => (sp === undefined ? {} : { rootThreadId: "root", threadId: "t-asker" }),
    placeDoorOf: () => ({ nameOf: (p: string) => `door-${p}` }),
    attach: (record: { id: string }, m: { id: string }) => {
      log.push(["attach", record.id, m.id]);
      const entry = { record, machine: { id: m.id, ...s.machine }, ws: { daemonReach: () => (s.reach === "fails" ? Promise.reject(new Error("no route")) : Promise.resolve("route")) } };
      live.set(record.id, entry);
      return entry;
    },
    fork: async (record: Record<string, unknown>, bind: (m: { id: string }) => void, override: unknown, _report: unknown, readsMemory: boolean) => {
      log.push(["fork", JSON.parse(JSON.stringify(record)), override ?? null, readsMemory]);
      const how = forks.shift() ?? "ok";
      if (how === "cap") throw Object.assign(new Error("cap"), { kind: "concurrency", said: "you have 3 machines" });
      if (how === "cap-status") throw Object.assign(new Error("cap"), { kind: "concurrency", status: 429, said: "" });
      if (how === "fails") throw Object.assign(new Error("provider down"), { kind: "provider" });
      bind({ id: "m-new" });
      return { id: "m-new" };
    },
    refreshBuilders: async () => void log.push(["refreshBuilders"]),
    graceTimers: new Map([["b1", () => log.push(["grace cancelled", "b1"])]]),
    backendAt: (p: string) => ({ id: `backend-${p}` }),
    forgetBuilder: async (id: string) => (log.push(["forgetBuilder", id]), void builders.delete(id)),
    holdsSlot: (r: { holds?: boolean }) => r.holds === true,
    servedByItsComputer: () => (s.served ? "computer" : undefined),
    lifecycleOf: () => ({ budgets: { daemonAnswersMs: 30_000 } }),
    pingDaemon: async (e: { record: { id: string } }) => (log.push(["pingDaemon", e.record.id]), s.fault),
    syncDaemon: async (e: { record: { id: string } }) => void log.push(["syncDaemon", e.record.id]),
    readMemory: async (record: { size: { cpu: number; memMb: number } }) => {
      log.push(["readMemory"]);
      if (s.memory === undefined || s.memory === false) return false;
      record.size = s.memory;
      return true;
    },
    moduleOf: (kind: string) => ({ landProject: async (e: { record: { id: string } }, p: { id: string }) => {
      log.push(["landProject", kind, e.record.id, p.id]);
      if (s.landFails) throw new Error("clone failed");
    } }),
    writeDaemonRoots: async (e: { record: { id: string } }) => void log.push(["writeDaemonRoots", e.record.id]),
    persist: async (r: unknown) => void log.push(["persist", JSON.parse(JSON.stringify(r))]),
    view: (r: { id: string; name: string }) => ({ id: r.id, name: r.name, view: true }),
    refOf: (p: { id: string; name: string }) => ({ id: p.id, name: p.name }),
  };
  const warn = vi.spyOn(console, "warn").mockImplementation((line: unknown) => void log.push(["warn", String(line)]));
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(T0);
  const area = createArea(ctx as never);
  const report = (stage: string, message: string, said?: unknown) => void log.push(["report", stage, message, said ?? null]);
  let answer: unknown;
  try {
    answer = await area.createStaged({ name: "w1", ...s.o } as never, project as never, "ws_1", report as never, s.spawned as never, () => void log.push(["landed"]), s.childBase);
  } catch (e) {
    answer = { threw: (e as Error).message, kind: (e as { kind?: string }).kind ?? null, status: (e as { status?: number }).status ?? null, cause: ((e as { cause?: Error }).cause?.message) ?? null };
  } finally {
    warn.mockRestore();
    vi.useRealTimers();
  }
  const entry = live.get("ws_1") as { creating?: boolean; opensThread?: string; ofRecordHeld?: boolean } | undefined;
  return { name, scene: s, log, answer, entry: entry === undefined ? null : { creating: entry.creating ?? null, opensThread: entry.opensThread === undefined ? null : typeof entry.opensThread, ofRecordHeld: entry.ofRecordHeld ?? null } };
}

async function small() {
  const log: unknown[] = [];
  const clock = fakeClock(T0);
  const listeners: ((e: unknown) => void)[] = [];
  const bus = { on: (type: string, l: (e: unknown) => void) => (type === "*" ? listeners.push(l) : undefined, () => undefined), emit: (e: unknown) => (log.push(["emit", e]), listeners.forEach(l => l(e))) };
  const live = new Map<string, unknown>([
    ["a", { record: { id: "a", name: "taken", kind: "cloud", project: "p1" } }],
    ["b", { record: { id: "b", name: "going", kind: "cloud", project: "p1" }, deleting: Promise.resolve() }],
    ["c", { record: { id: "c", name: "folder", kind: "local", project: "p1" } }],
    ["d", { record: { id: "d", name: "other", kind: "local", project: "p2" } }],
  ]);
  const area = createArea({ bus, clock: clock.clock, live, builders: new Map(), places: { wired: "solari" }, refOf: (p: { id: string; name: string }) => ({ id: p.id, name: p.name }) } as never);
  const facts: Record<string, unknown> = {};
  facts.nameGiven = ["  w1 ", "\tw ", "", "﻿x﻿"].map(n => [n, area.nameGiven(n)]);
  area.forking.add("forking");
  facts.nameRefusal = ["", "   ", "taken", "going", "forking", "free", " taken"].map(n => [n, area.nameRefusal(n) ?? null]);
  facts.failedView = area.failedView("ws_f", "w1", "cloud", "", T0, { id: "p1", name: "site" } as never, "it failed");
  area.failedCreates.set("f1", { name: "w1" } as never);
  area.failedCreates.set("f2", { name: "w2" } as never);
  area.failedCreates.set("f3", { name: "w1" } as never);
  area.supersedeFailed("w1");
  facts.afterSupersede = [...area.failedCreates.keys()];
  const reporter = area.stageReporter("ws_9", "w9", T0 - 1500, { threadId: "t1", rootThreadId: "r1", extra: "x" } as never);
  reporter("fork-requested", "starting", { notice: "n", detail: "d", waiting: true } as never);
  area.stageReporter("ws_8", "w8", T0, undefined)("ready", "ready", { waiting: false } as never);
  facts.stagesHeld = [...area.createStages];
  bus.emit({ type: "workspace.creating", workspaceId: "ws_7", name: "w7", stage: "x", message: "m", elapsedMs: 0, seq: 44 });
  facts.stagesWithSeqStripped = [...area.createStages];
  bus.emit({ type: "workspace.created", workspace: { id: "ws_9" } });
  bus.emit({ type: "workspace.deleted", workspaceId: "ws_8" });
  facts.stagesAfter = [...area.createStages];
  area.showCreate({ id: "s1", name: "shown" } as never);
  facts.shown = [...area.shownCreates];
  facts.foldersOf = area.foldersOf("p1").map(e => (e as { record: { id: string } }).record.id);
  let made = 0;
  let release!: (v: unknown) => void;
  const make = () => (made++, new Promise<never>(r => (release = r as never)));
  const one = area.oneFolder("k", make);
  const two = area.oneFolder("k", make);
  facts.oneFolderSame = one === two;
  release({ record: { id: "made" } });
  await one;
  const three = area.oneFolder("k", () => (made++, Promise.reject(new Error("no"))));
  await three.catch(() => undefined);
  const four = area.oneFolder("k", () => (made++, Promise.resolve({ record: { id: "again" } } as never)));
  await four;
  facts.oneFolderMade = made;
  return { facts, log };
}

it("create", async () => {
  const scenes = [
    await run("a provider fork lands and is ready", {}),
    await run("a size not offered is refused before any machine", { o: { cpu: 3, memMb: 1000 } }),
    await run("an offered size is kept", { o: { cpu: 4, memMb: 8192, envs: { A: "1" }, labels: { l: "v" }, idleWindowMs: 60000, parent: "ws_p", agents: { machines: 1 } } }),
    await run("the road refuses before anything is written", { road: { refusal: "this computer cannot hold it" } }),
    await run("a memory folder the road reads otherwise is remembered", { road: { memoryDir: "/elsewhere", binds: [{ from: "/elsewhere", to: "/mem" }] } }),
    await run("a project with a checkout is copied, a child starts from its base", { project: { checkout: "/co/site", base: "main" }, childBase: "feature" }),
    await run("a computer that keeps no image forks from none", { keepsImages: false }),
    await run("a named golden at a place is copied there first", { o: { golden: "snap-x" }, placeId: "pl1" }),
    await run("a spawned fork carries its tree and no agents", { spawned: { threadId: "t-asker", rootThreadId: "root" }, o: { agents: { machines: 1 } } }),
    await run("a brokered fork names its branch and the thread it opens", { road: { brokered: true }, project: { base: "main" } }),
    await run("a brokered fork with the branch named keeps it held and opens the named thread", { road: { brokered: true }, o: { branchOfRecord: { owner: "acme", name: "site", branch: "given" }, opensThread: "t-open" } }),
    await run("at the machine cap a kept builder is stopped and the fork goes", { forks: ["cap", "ok"], builders: [{ id: "b0", name: "held-one", life: "held", sealed: 1 }, { id: "b1", name: "kept", life: "reusable", sealed: 3 }, { id: "b2", name: "own", life: "own", sealed: 4, place: "pl9" }] }),
    await run("at the cap with builders refused each time the refusal names the holders", { forks: ["cap", "cap", "cap"], builders: [{ id: "b1", name: "kept", life: "reusable", sealed: 3 }, { id: "b2", name: "own", life: "own", sealed: 4 }, { id: "b3", name: "unsealed", life: "own" }], liveOthers: [{ name: "busy", holds: true }, { name: "idle", holds: false }] }),
    await run("at the cap with nothing to stop the status is kept", { forks: ["cap-status"] }),
    await run("a fork that fails another way is thrown as it is", { forks: ["fails"] }),
    await run("a refusal after a builder stopped that is not the cap is thrown", { forks: ["cap", "fails"], builders: [{ id: "b1", name: "kept", life: "reusable", sealed: 3 }] }),
    await run("a route and a daemon: both asked, the sync started", { machine: { previewUrl: "https://p" } }),
    await run("a route that is not minted is a notice, the daemon answering", { machine: { previewUrl: "https://p" }, reach: "fails" }),
    await run("a daemon that does not answer is noted and the memory is not read", { machine: { daemonAnswers: true }, fault: "no daemon on 7070" }),
    await run("a served workspace asks neither", { machine: { previewUrl: "https://p" }, served: true }),
    await run("memory read to another size says so", { memory: { cpu: 2, memMb: 3900 } }),
    await run("memory read to another size with a size named", { o: { cpu: 2, memMb: 4096 }, memory: { cpu: 2, memMb: 3900 } }),
    await run("memory at a place is not read here", { placeId: "pl1", memory: { cpu: 8, memMb: 1 } }),
    await run("a machine notice joins the answer", { machine: { notice: "forked at 2 cpu, not 4" }, memory: { cpu: 2, memMb: 3900 } }),
    await run("a clone that fails ends the create before it is persisted", { landFails: true }),
  ];
  const rest = await small();
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "create.json"), `${JSON.stringify({ T0, scenes, ...rest }, null, 1)}\n`);
});
