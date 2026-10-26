// SPDX-License-Identifier: AGPL-3.0-only
// Review probe: rulesArea (packages/runtime/src/account/rules.ts at 65f6b89c7) over many generated worlds, the same
// fake context as oracle/rules.oracle.ts, every door asked across every record and caller of each world. Writes
// tests/fixtures/oracle/zz_review_rules_fuzz.json for tests/review_rules.rs.
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { kindForComputer, type Caller } from "@wsp/protocol";

const { rulesArea } = await import("../../../../packages/runtime/src/account/rules.ts");
const out = fileURLToPath(new URL("../tests/fixtures/oracle/zz_review_rules_fuzz.json", import.meta.url));
const WORLDS = Number(process.env.WORLDS ?? 400);

const rng = (seed: number) => () => {
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};

type World = ReturnType<typeof gen>;
function gen(seed: number) {
  const r = rng(seed);
  const pick = <T,>(a: readonly T[]): T => a[Math.floor(r() * a.length)]!;
  const n = (k: number) => Math.floor(r() * k);
  const computers = ["here", "box1", "box2", "solari"];
  const projects = Array.from({ length: 2 + n(4) }, (_, i) => {
    const repo = pick(["acme/a", "acme/b"]);
    return { id: `p${i}`, name: `proj${i}`, computer: pick(computers), repo, remote: `https://github.com/${repo}`, path: `/srv/p${i}`, source: { kind: "git" } };
  });
  const threads = ["t1", "t2", "t3", "t4", "t5", "t6"];
  const nL = 3 + n(7);
  const ids = Array.from({ length: nL }, (_, i) => `w${i}`);
  const live = ids.map((id, i) => {
    const kind = pick(["local", "place", "cloud", "cloud"]);
    const record: Record<string, unknown> = {
      id,
      name: `ws${i}`,
      kind,
      project: r() < 0.06 ? "pGone" : pick(projects).id,
      machineId: kind === "cloud" ? (r() < 0.85 ? `m${i}` : "") : "",
      phase: pick(["running", "running", "running", "napping", "gone", "released", "queued", "waking", "pausing"]),
    };
    if (kind === "place") record.place = pick(["box1", "box2"]);
    if (kind === "cloud" && r() < 0.2) record.place = "bare";
    if (r() < 0.6) record.rootThreadId = pick(threads);
    if (r() < 0.4) record.parentThreadId = pick(threads);
    if (kind !== "cloud" && r() < 0.3) record.worktree = { path: `/t/${id}`, branch: `wsp/${id}`, made: true, madeFor: pick(threads) };
    if (r() < 0.15) record.lead = { threadId: pick(threads), workspaceId: pick(ids) };
    if (r() < 0.25) record.agents = { maxMachines: n(4), maxDepth: n(3) };
    return { record, ...(r() < 0.1 ? { creating: true } : {}) };
  });
  const threadRecords = threads.filter(() => r() < 0.75).map(threadId => {
    const t: Record<string, string> = { threadId, workspaceId: r() < 0.9 ? pick(ids) : "wMissing" };
    if (r() < 0.6) t.parentThreadId = pick(threads);
    if (r() < 0.6) t.rootThreadId = pick(threads);
    return t;
  });
  const rows: Record<string, Record<string, string>[]> = {};
  for (const t of [...threads, "t9"]) {
    if (r() < 0.25) {
      rows[t] = Array.from({ length: 1 + n(2) }, () => {
        const row: Record<string, string> = {};
        if (r() < 0.8) row.workspaceId = pick(ids);
        if (r() < 0.5) row.parentThreadId = pick([...threads, "t9"]);
        if (r() < 0.5) row.rootThreadId = pick([...threads, "t9"]);
        return row;
      });
    }
  }
  const placeAgents: Record<string, { maxMachines: number; maxDepth: number }> = {};
  for (const p of ["here", "box1", "box2", "solari", "bare"]) if (r() < 0.8) placeAgents[p] = { maxMachines: n(5), maxDepth: n(4) };
  const landing: Record<string, number> = {};
  for (const t of threads) if (r() < 0.2) landing[t] = 1 + n(2);
  const callers: Record<string, unknown> = { none: null, here: "here", relayed: "relayed" };
  for (let k = 0; k < 9; k++) {
    const by = { kind: "thread", threadId: pick(threads), workspaceId: r() < 0.92 ? pick(ids) : "wNowhere", rootThreadId: r() < 0.6 ? pick(threads) : "" };
    if (by.rootThreadId === "") by.rootThreadId = by.threadId;
    callers[`c${k}`] = { origin: r() < 0.85 ? "here" : "relayed", by };
  }
  return {
    joined: ["box1", "box2"],
    placeNames: { here: "this computer", box1: "studio", box2: "garage", solari: "Solari" } as Record<string, string>,
    projects,
    placeAgents,
    live,
    threadRecords,
    rows,
    sessionsSize: n(5),
    landing,
    callers,
    threads: [...threads, "t9", "tNone"],
  };
}

const errOf = (e: unknown): unknown => (e === undefined ? null : { message: (e as Error).message, ...((e as { fix?: string }).fix !== undefined ? { fix: (e as { fix?: string }).fix } : {}), ...((e as { kind?: string }).kind !== undefined ? { kind: (e as { kind?: string }).kind } : {}) });
const caught = (f: () => unknown): unknown => {
  try {
    const v = f();
    return v === undefined ? null : v;
  } catch (e) {
    return { thrown: errOf(e) };
  }
};

function build(world: World) {
  const projectsHeld = new Map(world.projects.map(p => [p.id, p as never]));
  const live = new Map(world.live.map(e => [e.record.id as string, { record: structuredClone(e.record), creating: e.creating ?? false, machine: { id: e.record.machineId } } as never]));
  const threadRecords = new Map(world.threadRecords.map(t => [t.threadId!, t as never]));
  const landing = new Map(Object.entries(world.landing));
  const placeIdOf = (r: { kind: string; place?: string }): string | undefined => (r.kind === "local" ? "here" : (r.place ?? (r.kind === "cloud" ? "solari" : undefined)));
  const ctx = {
    opts: { noMachinesLine: undefined },
    store: { put: async () => undefined },
    placeDoor: { nameOf: (id: string) => world.placeNames[id] ?? id },
    landing,
    live,
    projectsHeld,
    sessions: { size: world.sessionsSize },
    threadRecords,
    preferences: { get: async () => ({}), set: async () => ({}) },
    backendFor: (r: { kind: string; place?: string }) => ({ capabilities: r.kind === "cloud" ? (r.place === "bare" ? { sizes: [], snapshot: false } : { sizes: [{ name: "s" }], pauseMode: "pause", snapshot: true, fork: false }) : { sizes: [], snapshot: false } }),
    moduleOf: (kind: string) => ({
      relayed: (machineId?: string) => kind === "cloud" && machineId !== undefined && machineId !== "",
      turnReach: () => undefined,
      wspMcp: () => undefined,
    }),
    sameRepository: (a: { repo: string }, b: { repo: string }) => a.repo === b.repo,
    projectHeld: (id: string) => {
      const p = projectsHeld.get(id);
      if (p === undefined) throw new Error(`no project ${id}`);
      return p;
    },
    kindOf: (computer: string) => kindForComputer(computer, world.joined.includes(computer)),
    placeName: (id: string) => world.placeNames[id] ?? id,
    rowsOn: (threadId: string) => world.rows[threadId] ?? [],
    agentsHeld: (r: { agents?: unknown; kind: string; place?: string }) => r.agents ?? world.placeAgents[placeIdOf(r) ?? ""] ?? { maxMachines: 3, maxDepth: 1 },
    placeIdOf,
  };
  return { area: rulesArea(ctx as never), landing, live };
}

function answers(world: World) {
  const { area, live } = build(world);
  const callers = world.callers as Record<string, Caller | undefined>;
  const C = Object.entries(callers).map(([k, v]) => [k, v === null ? undefined : v] as const);
  const ids = world.live.map(e => e.record.id as string);
  const rec = (id: string) => (live.get(id) as unknown as { record: never }).record;
  const per = <T,>(f: (c: Caller | undefined) => T) => Object.fromEntries(C.map(([k, c]) => [k, f(c)]));
  const perRec = (f: (c: Caller | undefined, id: string) => unknown) => per(c => Object.fromEntries(ids.map(id => [id, caught(() => f(c, id))])));
  const acts = ["thread_new", "fork", "send", "bring_back", "delete_sub", "commit", "merge_in", "merge", "delete", "fix", "update"];
  const spawnGuard: unknown[] = [];
  for (const [k, c] of C) {
    for (const act of acts) {
      const w = build(world);
      const answer = caught(() => (w.area.spawnGuard(act as never, c), "free"));
      spawnGuard.push({ caller: k, act, answer, landing: Object.fromEntries(w.landing) });
    }
    // Three forks in one tick, then each release twice.
    const w = build(world);
    const steps: unknown[] = [];
    const frees: ((() => void) | undefined)[] = [];
    for (let i = 0; i < 3; i++) {
      try {
        frees.push(w.area.spawnGuard("fork" as never, c));
        steps.push({ took: Object.fromEntries(w.landing) });
      } catch (e) {
        frees.push(undefined);
        steps.push({ refused: errOf(e), landing: Object.fromEntries(w.landing) });
      }
    }
    for (const f of frees) {
      f?.();
      f?.();
      steps.push({ freed: Object.fromEntries(w.landing) });
    }
    spawnGuard.push({ caller: k, act: "fork x3", steps });
  }
  return {
    refusalFor: perRec((c, id) => area.refusalFor(rec(id), c)),
    refusalForNone: per(c => caught(() => area.refusalFor(undefined, c))),
    listedFor: per(c => caught(() => area.listedFor(c).map((e: { record: { id: string } }) => e.record.id))),
    refuseRelayedWork: perRec((c, id) => area.refuseRelayed(rec(id), c)),
    refuseRelayedThread: perRec((c, id) => area.refuseRelayed(rec(id), c, "exec", "t2")),
    refuseNamed: per(c => Object.fromEntries([...ids, "wNowhere"].map(id => [id, caught(() => area.refuseNamed(id, c))]))),
    projectReached: per(c => Object.fromEntries([...world.projects.map(p => p.id), "pGone"].map(p => [p, caught(() => area.projectReached(c, p))]))),
    ofThreadsRepository: per(c => Object.fromEntries([...world.projects.map(p => p.id), "pGone"].map(p => [p, caught(() => area.ofThreadsRepository(c, p))]))),
    elsewhereRefusal: per(c => Object.fromEntries(world.projects.map(p => [p.id, caught(() => errOf(area.elsewhereRefusal(c, p as never, `the ${p.name} word`)))]))),
    talksToItsTree: per(c => caught(() => area.talksToItsTree(c))),
    actsOn: perRec((c, id) => area.actsOn(rec(id), c)),
    opensIn: perRec((c, id) => area.opensIn(rec(id), c)),
    leadActsOn: perRec((c, id) => area.leadActsOn(rec(id), c) ?? null),
    awayFor: perRec((c, id) => errOf(area.awayFor(rec(id), c, "wake", `"${id}"`))),
    parentOf: Object.fromEntries(world.threads.map(t => [t, caught(() => area.parentOf(t))])),
    rootOf: Object.fromEntries(world.threads.map(t => [t, caught(() => area.rootOf(t))])),
    agentsOf: Object.fromEntries(ids.map(id => [id, caught(() => area.agentsOf(rec(id)))])),
    spawnGuard,
  };
}

it("writes the fuzz fixture", () => {
  const worlds = Array.from({ length: WORLDS }, (_, i) => {
    const world = gen(1000 + i);
    return { seed: 1000 + i, world, answers: answers(world) };
  });
  writeFileSync(out, `${JSON.stringify(worlds)}\n`);
  console.log(`wrote ${worlds.length} worlds`);
});
