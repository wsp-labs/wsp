// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the rules tests compare to: rulesArea (packages/runtime/src/account/rules.ts at 65f6b89c7) run on a
// fake context built from one world, every door asked across every record and caller, written as
// tests/fixtures/oracle/rules.json with the world beside it. The Rust fake builds the same context from that world.
// Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 rules.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { kindForComputer, type Caller } from "@wsp/protocol";

const { rulesArea } = await import("../../../../packages/runtime/src/account/rules.ts");
const out = fileURLToPath(new URL("../tests/fixtures/oracle/rules.json", import.meta.url));

/** The world: projects, live workspaces, thread records, rows, place names and caps, as plain data. `repo` on a
 * project stands for what `sameRepository` reads; the fake answers it by comparing those. */
const world = {
  joined: ["box1", "box2"],
  placeNames: { here: "this computer", box1: "studio", box2: "garage", solari: "Solari" },
  projects: [
    { id: "pA", name: "alpha", computer: "here", repo: "acme/alpha", remote: "git@github.com:acme/alpha.git", path: "/home/u/alpha", source: { kind: "git" } },
    { id: "pA2", name: "alpha-studio", computer: "box1", repo: "acme/alpha", remote: "https://github.com/acme/alpha", path: "/srv/alpha", source: { kind: "git" } },
    { id: "pA4", name: "alpha-garage", computer: "box2", repo: "acme/alpha", remote: "https://github.com/acme/alpha", path: "/srv/alpha", source: { kind: "git" } },
    { id: "pAc", name: "alpha-cloud", computer: "solari", repo: "acme/alpha", remote: "https://github.com/acme/alpha", path: "/work/alpha", source: { kind: "git" } },
    { id: "pB", name: "beta", computer: "here", repo: "acme/beta", remote: "git@github.com:acme/beta.git", path: "/home/u/beta", source: { kind: "git" } },
    { id: "pB2", name: "beta-studio", computer: "box1", repo: "acme/beta", remote: "https://github.com/acme/beta", path: "/srv/beta", source: { kind: "git" } },
  ],
  /** Place defaults for the agents switch, by place id; a record's own `agents` outranks them. */
  placeAgents: { here: { maxMachines: 3, maxDepth: 2 }, solari: { maxMachines: 2, maxDepth: 3 }, box1: { maxMachines: 4, maxDepth: 1 }, box2: { maxMachines: 4, maxDepth: 2 }, bare: { maxMachines: 5, maxDepth: 5 } },
  live: [
    { record: { id: "wL", name: "alpha-here", kind: "local", project: "pA", machineId: "", phase: "running" } },
    { record: { id: "wB1", name: "alpha-studio", kind: "place", project: "pA2", machineId: "", phase: "running", place: "box1" } },
    { record: { id: "wB2", name: "alpha-garage", kind: "place", project: "pA4", machineId: "", phase: "running", place: "box2" } },
    { record: { id: "wBb", name: "beta-studio", kind: "place", project: "pB2", machineId: "", phase: "running", place: "box1" } },
    { record: { id: "wC1", name: "alpha-fork", kind: "cloud", project: "pAc", machineId: "m1", phase: "running", rootThreadId: "t1", parentThreadId: "t1" } },
    { record: { id: "wC2", name: "alpha-other", kind: "cloud", project: "pAc", machineId: "m2", phase: "napping", rootThreadId: "t9", parentThreadId: "t9" } },
    { record: { id: "wC3", name: "beta-fork", kind: "cloud", project: "pB", machineId: "m3", phase: "running", rootThreadId: "t1", parentThreadId: "t1" } },
    { record: { id: "wC4", name: "bare-cloud", kind: "cloud", project: "pAc", machineId: "m4", phase: "running", place: "bare" } },
    { record: { id: "wCa", name: "alpha-capped", kind: "cloud", project: "pAc", machineId: "m5", phase: "running", agents: { maxMachines: 1, maxDepth: 1 } } },
    { record: { id: "wWt", name: "alpha-tree", kind: "local", project: "pA", machineId: "", phase: "running", worktree: { path: "/home/u/.wsp/trees/alpha-tree", branch: "wsp/t", made: true, madeFor: "t1" } } },
    { record: { id: "wSub", name: "alpha-sub", kind: "cloud", project: "pAc", machineId: "m6", phase: "running", rootThreadId: "t1", parentThreadId: "t1", lead: { threadId: "t1", workspaceId: "wL" } } },
    { record: { id: "wGone", name: "alpha-gone", kind: "cloud", project: "pAc", machineId: "m7", phase: "gone", rootThreadId: "t1" } },
    { record: { id: "wRel", name: "alpha-released", kind: "cloud", project: "pAc", machineId: "m8", phase: "released", rootThreadId: "t1" } },
    { record: { id: "wNew", name: "alpha-new", kind: "cloud", project: "pAc", machineId: "", phase: "running", rootThreadId: "t1" }, creating: true },
    { record: { id: "wOrphan", name: "orphan-fork", kind: "cloud", project: "pAc", machineId: "m9", phase: "running", rootThreadId: "tGone" } },
  ],
  threadRecords: [
    { threadId: "t1", workspaceId: "wL" },
    { threadId: "t2", workspaceId: "wC1", parentThreadId: "t1", rootThreadId: "t1" },
    { threadId: "t6", workspaceId: "wC1", parentThreadId: "t2", rootThreadId: "t1" },
    { threadId: "t3", workspaceId: "wB1" },
    { threadId: "t4", workspaceId: "wB1", parentThreadId: "t1", rootThreadId: "t1" },
    { threadId: "t5", workspaceId: "wSub", parentThreadId: "t1", rootThreadId: "t1" },
    { threadId: "t7", workspaceId: "wB2", parentThreadId: "t3", rootThreadId: "t3" },
    { threadId: "tc", workspaceId: "wCa" },
    { threadId: "tcc", workspaceId: "wCa", parentThreadId: "tc", rootThreadId: "tc" },
    { threadId: "tO2", workspaceId: "wOrphan" },
    { threadId: "tLoopA", workspaceId: "wC2", parentThreadId: "tLoopB", rootThreadId: "t9" },
    { threadId: "tLoopB", workspaceId: "wC2", parentThreadId: "tLoopA", rootThreadId: "t9" },
  ],
  /** Rows (newest last in insertion order) for threads from before thread records. */
  rows: { t9: [{ workspaceId: "wL" }], tOld: [{ workspaceId: "wC2" }, { workspaceId: "wC2", parentThreadId: "t9", rootThreadId: "t9" }] },
  sessionsSize: 4,
  landing: { t3: 1 },
  noMachinesLine: null as string | null,
};

const scope = (threadId: string, workspaceId: string, rootThreadId: string): Caller => ({ origin: "here", by: { kind: "thread", threadId, workspaceId, rootThreadId } });
const callers: Record<string, Caller | undefined> = {
  none: undefined,
  here: "here",
  relayed: "relayed",
  t1: scope("t1", "wL", "t1"),
  t2: scope("t2", "wC1", "t1"),
  t6: scope("t6", "wC1", "t1"),
  t3: scope("t3", "wB1", "t3"),
  t4: scope("t4", "wB1", "t1"),
  t5: scope("t5", "wSub", "t1"),
  t7: scope("t7", "wB2", "t3"),
  tc: scope("tc", "wCa", "tc"),
  tcc: scope("tcc", "wCa", "tc"),
  tLost: scope("tLost", "wNowhere", "tLost"),
  tOrphan: scope("tOrphan", "wOrphan", "tGone"),
  tO2: scope("tO2", "wOrphan", "tO2"),
  tLoop: scope("tLoopA", "wC2", "t9"),
  tRelayed: { origin: "relayed", by: { kind: "thread", threadId: "t2", workspaceId: "wC1", rootThreadId: "t1" } },
};

type Thrown = { thrown: { message: string; fix?: string; kind?: string } };
const caught = (f: () => unknown): unknown => {
  try {
    const v = f();
    return v === undefined ? null : v;
  } catch (e) {
    const err = e as Error & { fix?: string; kind?: string };
    return { thrown: { message: err.message, ...(err.fix !== undefined ? { fix: err.fix } : {}), ...(err.kind !== undefined ? { kind: err.kind } : {}) } } satisfies Thrown;
  }
};
const errOf = (e: unknown): unknown => (e === undefined ? null : { message: (e as Error).message, ...((e as { fix?: string }).fix !== undefined ? { fix: (e as { fix?: string }).fix } : {}), ...((e as { kind?: string }).kind !== undefined ? { kind: (e as { kind?: string }).kind } : {}) });

function build() {
  const projectsHeld = new Map(world.projects.map(p => [p.id, p as never]));
  const live = new Map(world.live.map(e => [e.record.id, { record: structuredClone(e.record), creating: e.creating ?? false, machine: { id: e.record.machineId } } as never]));
  const threadRecords = new Map(world.threadRecords.map(t => [t.threadId, t as never]));
  const landing = new Map(Object.entries(world.landing));
  const puts: unknown[] = [];
  const prefSets: unknown[] = [];
  let prefs: { target?: { workspace?: string; project?: string } } = { target: { workspace: "wC1" } };
  const placeIdOf = (r: { kind: string; place?: string }): string | undefined => (r.kind === "local" ? "here" : (r.place ?? (r.kind === "cloud" ? "solari" : undefined)));
  const ctx = {
    opts: { noMachinesLine: world.noMachinesLine ?? undefined },
    store: { put: async (c: string, id: string, v: unknown) => void puts.push([c, id, v]) },
    placeDoor: { nameOf: (id: string) => (world.placeNames as Record<string, string>)[id] ?? id },
    landing,
    live,
    projectsHeld,
    sessions: { size: world.sessionsSize },
    threadRecords,
    preferences: { get: async () => prefs, set: async (p: unknown) => (prefSets.push(p), (prefs = { ...prefs, ...(p as object) }), { preferences: prefs }) },
    backendFor: (r: { kind: string; place?: string }) => ({ capabilities: r.kind === "cloud" ? (r.place === "bare" ? { sizes: [], snapshot: false } : { sizes: [{ name: "s" }], pauseMode: "pause", snapshot: true, fork: false }) : { sizes: [], snapshot: false } }),
    moduleOf: (kind: string) => ({
      relayed: (machineId?: string) => kind === "cloud" && machineId !== undefined && machineId !== "",
      turnReach: (entry: { record: { id: string; kind: string } }) => (kind === "cloud" ? { url: `wss://host/${entry.record.id}` } : kind === "place" ? {} : undefined),
      wspMcp: (entry: { record: { id: string } }) => (kind === "cloud" ? { command: "wsp", args: ["mcp", entry.record.id] } : undefined),
    }),
    sameRepository: (a: { repo: string }, b: { repo: string }) => a.repo === b.repo,
    projectHeld: (id: string) => {
      const p = projectsHeld.get(id);
      if (p === undefined) throw new Error(`no project ${id}`);
      return p;
    },
    kindOf: (computer: string) => kindForComputer(computer, world.joined.includes(computer)),
    placeName: (id: string) => (world.placeNames as Record<string, string>)[id] ?? id,
    rowsOn: (threadId: string) => (world.rows as Record<string, unknown[]>)[threadId] ?? [],
    agentsHeld: (r: { agents?: unknown; kind: string; place?: string }) => r.agents ?? (world.placeAgents as Record<string, unknown>)[placeIdOf(r) ?? ""] ?? { maxMachines: 3, maxDepth: 1 },
    placeIdOf,
  };
  const area = rulesArea(ctx as never);
  return { area, puts, prefSets, landing, live, projectsHeld };
}

it("writes the rules fixture", async () => {
  const { area } = build();
  const ids = world.live.map(e => e.record.id);
  const rec = (id: string) => (build().live.get(id) as { record: never }).record;
  const records = Object.fromEntries(ids.map(id => [id, rec(id)]));
  const unheld = { id: "wX", name: "x-fork", kind: "cloud", project: "pA", machineId: "mx", phase: "running" };
  const noProject = { name: "nameless", kind: "cloud", machineId: "" };
  const refusalFor: unknown[] = [];
  for (const [c, caller] of Object.entries(callers)) {
    for (const id of ids) refusalFor.push({ record: id, caller: c, answer: caught(() => area.refusalFor(records[id], caller)) });
    refusalFor.push({ record: "none", caller: c, answer: caught(() => area.refusalFor(undefined, caller)) });
    refusalFor.push({ record: "noProject", caller: c, answer: caught(() => area.refusalFor(noProject as never, caller)) });
  }
  const listedFor = Object.fromEntries(Object.entries(callers).map(([c, caller]) => [c, caught(() => area.listedFor(caller).map((e: { record: { id: string } }) => e.record.id))]));
  const refuseRelayed: unknown[] = [];
  for (const [c, caller] of Object.entries(callers)) {
    for (const id of ids) {
      refuseRelayed.push({ record: id, caller: c, act: "work", answer: caught(() => area.refuseRelayed(records[id], caller)) });
      refuseRelayed.push({ record: id, caller: c, act: "exec", thread: "t2", answer: caught(() => area.refuseRelayed(records[id], caller, "exec", "t2")) });
    }
    refuseRelayed.push({ record: "none", caller: c, act: "work", answer: caught(() => area.refuseRelayed(undefined, caller)) });
  }
  const refuseNamed: unknown[] = [];
  for (const [c, caller] of Object.entries(callers)) for (const id of [...ids, "wNowhere"]) refuseNamed.push({ workspaceId: id, caller: c, answer: caught(() => area.refuseNamed(id, caller)) });
  const perCaller = (f: (caller: Caller | undefined) => unknown) => Object.fromEntries(Object.entries(callers).map(([c, caller]) => [c, caught(() => f(caller))]));
  const projectIds = world.projects.map(p => p.id);
  const projectReached = perCaller(caller => Object.fromEntries(projectIds.map(p => [p, caught(() => area.projectReached(caller, p))])));
  const ofThreadsRepository = perCaller(caller => Object.fromEntries(projectIds.map(p => [p, caught(() => area.ofThreadsRepository(caller, p))])));
  const elsewhereRefusal = perCaller(caller => Object.fromEntries(world.projects.map(p => [p.id, errOf(area.elsewhereRefusal(caller, p as never, `the ${p.name} word`))])));
  const talksToItsTree = perCaller(caller => area.talksToItsTree(caller));
  const actsOn = perCaller(caller => Object.fromEntries(ids.map(id => [id, caught(() => area.actsOn(records[id], caller))])));
  const opensIn = perCaller(caller => Object.fromEntries(ids.map(id => [id, caught(() => area.opensIn(records[id], caller))])));
  const leadActsOn = perCaller(caller => Object.fromEntries(ids.map(id => [id, caught(() => area.leadActsOn(records[id], caller) ?? null)])));
  const awayFor = perCaller(caller => Object.fromEntries(ids.map(id => [id, caught(() => errOf(area.awayFor(records[id], caller, "wake", `"${id}"`)))])));
  const drives = perCaller(caller => Object.fromEntries([...ids.map(id => [id, area.drives(records[id], caller)]), ["unheld", area.drives(unheld as never, caller)], ["noMachine", area.drives({ kind: "cloud" } as never, caller)]]));
  const refuseRecording = perCaller(caller => caught(() => area.refuseRecording("box9", caller)));
  const threadIds = [...world.threadRecords.map(t => t.threadId), "t9", "tOld", "tNone"];
  const parentOf = Object.fromEntries(threadIds.map(t => [t, area.parentOf(t) ?? null]));
  const rootOf = Object.fromEntries(threadIds.map(t => [t, area.rootOf(t)]));
  const agentsOf = Object.fromEntries(ids.map(id => [id, area.agentsOf(records[id]) ?? null]));
  const pauses = Object.fromEntries(ids.map(id => [id, area.pauses(records[id])]));
  const holdsSlot = Object.fromEntries(ids.map(id => [id, area.holdsSlot(records[id])]));
  const entry = (id: string) => ({ record: records[id] });
  const refuseCannot = Object.fromEntries(ids.map(id => [id, Object.fromEntries((["snapshot", "fork", "resize"] as const).map(can => [can, caught(() => area.refuseCannot(entry(id) as never, can as never, `take a ${can}`))]))]));
  const refusePauseless = Object.fromEntries(ids.map(id => [id, caught(() => area.refusePauseless(entry(id) as never, "nap"))]));
  const agentsReach = Object.fromEntries(ids.map(id => [id, area.agentsReach(entry(id) as never) ?? null]));
  const treeOf = { none: area.treeOf(undefined), t2: area.treeOf({ kind: "thread", threadId: "t2", workspaceId: "wC1", rootThreadId: "t1" }) };
  const scopes = Object.fromEntries(Object.entries(callers).filter(([, c]) => typeof c === "object").map(([n, c]) => [n, (c as { by: never }).by]));
  const placeOfThread = Object.fromEntries(Object.entries(scopes).map(([n, s]) => [n, area.placeOfThread(s) ?? null]));
  const actRefusal = Object.fromEntries(Object.entries(scopes).map(([n, s]) => [n, errOf(area.actRefusal(s, "merge" as never))]));
  // The guard, each ask on a fresh world so the landing counts start as the world has them.
  const acts = ["thread_new", "fork", "send", "bring_back", "delete_sub", "commit", "merge_in", "merge", "delete", "fix", "update"];
  const spawnGuard: unknown[] = [];
  for (const [c, caller] of Object.entries(callers)) {
    for (const act of acts) {
      const w = build();
      const answer = caught(() => (w.area.spawnGuard(act as never, caller), "free"));
      spawnGuard.push({ caller: c, act, answer, landing: Object.fromEntries(w.landing) });
    }
  }
  // The landing slot: two forks of one root in one tick, then the releases, each called twice.
  const slots = (() => {
    const w = build();
    const steps: unknown[] = [];
    const take = () => {
      try {
        return w.area.spawnGuard("fork", callers["t3"]);
      } catch (e) {
        steps.push({ step: "refused", message: (e as Error).message, landing: Object.fromEntries(w.landing) });
        return undefined;
      }
    };
    const a = take();
    steps.push({ step: "first", landing: Object.fromEntries(w.landing) });
    const b = take();
    steps.push({ step: "second", landing: Object.fromEntries(w.landing) });
    const c = take();
    steps.push({ step: "third", landing: Object.fromEntries(w.landing) });
    a?.();
    steps.push({ step: "free first", landing: Object.fromEntries(w.landing) });
    a?.();
    steps.push({ step: "free first again", landing: Object.fromEntries(w.landing) });
    b?.();
    c?.();
    steps.push({ step: "free rest", landing: Object.fromEntries(w.landing) });
    return steps;
  })();
  const remember = await (async () => {
    const w = build();
    const project = { ...world.projects[0]!, name: "alpha renamed" };
    await w.area.rememberProject(project as never);
    const sameTarget = build();
    await sameTarget.area.rememberTarget(records["wC1"] as never);
    const newTarget = build();
    await newTarget.area.rememberTarget(records["wB1"] as never);
    return { puts: w.puts, mapOrder: [...w.projectsHeld.keys()], heldName: (w.projectsHeld.get("pA") as { name: string }).name, sameTargetSets: sameTarget.prefSets, newTargetSets: newTarget.prefSets };
  })();
  const noMachines = (() => {
    world.noMachinesLine = "No provider here: add one with wsp add solari.";
    const w = build();
    const r = caught(() => w.area.refuseCannot({ record: records["wC4"] } as never, "snapshot" as never, "take a snapshot"));
    world.noMachinesLine = null;
    return r;
  })();
  writeFileSync(
    out,
    `${JSON.stringify(
      { world, callers, refusalFor, listedFor, refuseRelayed, refuseNamed, projectReached, ofThreadsRepository, elsewhereRefusal, talksToItsTree, actsOn, opensIn, leadActsOn, awayFor, drives, refuseRecording, parentOf, rootOf, agentsOf, pauses, holdsSlot, refuseCannot, refuseCannotNoMachinesLine: noMachines, refusePauseless, agentsReach, treeOf, placeOfThread, actRefusal, spawnGuard, slots, remember },
      null,
      1,
    )}\n`,
  );
});
