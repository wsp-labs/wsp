// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the preferences tests compare to: preferencesArea (packages/runtime/src/account/preferences.ts at
// 65f6b89c7) run on a fake context, once per script of sets over a world of adapters and held projects, and its
// agents read (the options it hands agentsReads) asked every question the Rust area answers. Written as
// tests/fixtures/oracle/preferences.json with each script's world beside its results. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 preferences.oracle.ts
import { writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";
import { CATALOG_AGENTS } from "@wsp/catalog";

const held = vi.hoisted(() => ({ options: [] as any[] }));
vi.mock("../../../../packages/runtime/src/agents-read.ts", async importOriginal => {
  const actual = await importOriginal<typeof import("../../../../packages/runtime/src/agents-read.ts")>();
  return { ...actual, agentsReads: (o: unknown) => (held.options.push(o), actual.agentsReads(o as never)) };
});

const { preferencesArea } = await import("../../../../packages/runtime/src/account/preferences.ts");
const { harnessCatalog } = await import("../../../../packages/runtime/src/harness-catalog.ts");
const out = fileURLToPath(new URL("../tests/fixtures/oracle/preferences.json", import.meta.url));

const ADAPTERS = ["claude", "codex", "mystery"];
const HOME = homedir();
const agentLabel = (id: string): string => `the ${id} agent`;
const plain = (v: unknown): unknown => JSON.parse(JSON.stringify(v ?? null));
const errorOf = (e: unknown): { message: string; fix?: string; kind?: string } => {
  const x = e as { message: string; fix?: string; kind?: string };
  return { message: x.message, ...(x.fix !== undefined ? { fix: x.fix } : {}), ...(x.kind !== undefined ? { kind: x.kind } : {}) };
};

/** What the Rust fake serves for a harness's access table: the catalog row the TS reads, or none (the TS then reads
 * `{ label: ctx.agentLabel(id), permissionModes: [] }`). */
const tables = Object.fromEntries(
  ADAPTERS.flatMap(id => {
    const row = harnessCatalog(id);
    return row === undefined ? [] : [[id, plain({ label: row.label, access: (row as { access?: unknown }).access, permissionModes: row.permissionModes })]];
  }),
);

/** Each catalog agent's variables for a thread on a machine, the last agent's left out to show a missing one. */
const catalogAgents = CATALOG_AGENTS.map(a => ({ id: a.id, stateHomeEnv: a.stateHomeEnv ?? null, pluginSeedEnv: a.plugins?.seed?.env ?? null }));
const threadEnv: Record<string, Record<string, string>> = Object.fromEntries(
  CATALOG_AGENTS.map((a, i) => [
    a.id,
    i === CATALOG_AGENTS.length - 1
      ? {}
      : { ...(a.stateHomeEnv !== undefined ? { [a.stateHomeEnv]: a.id === "claude" ? "/root/.claude-cfg" : `/state/${a.id}` } : {}), ...(a.plugins?.seed !== undefined ? { [a.plugins.seed.env]: a.id === "claude" ? "/root/.claude-plugin-seed" : `/seed/${a.id}` } : {}) },
  ]),
);

interface Script {
  name: string;
  adapters?: string[];
  projects?: string[];
  stored?: unknown;
  env?: Record<string, string>;
  sessions?: string[];
  threadRecords?: string[];
  /** The icons the options wire: `forget` throws this message, or succeeds on null. */
  icons?: { forget: string | null };
  concurrent?: boolean;
  steps: { patch: unknown }[];
  /** Asks of the agents read after every step. */
  probe?: boolean;
}

type Doc = { current: unknown };

function world(o: Script, doc: Doc) {
  const emitted: unknown[] = [];
  const puts: unknown[] = [];
  const counts = { storeGets: 0, forgets: 0 };
  const state: Record<string, unknown> = {};
  const adapters = Object.fromEntries((o.adapters ?? ADAPTERS).map(id => [id, {}]));
  const serverIcons = o.icons === undefined ? undefined : { folder: `${HOME}/.wsp/icons`, icon: async () => null, forget: () => { counts.forgets++; if (o.icons!.forget !== null) throw new Error(o.icons!.forget); } };
  const ctx = {
    opts: { env: o.env ?? {}, ...(serverIcons !== undefined ? { serverIcons } : {}) },
    backend: { capabilities: { callbackRelay: false } },
    store: {
      get: async () => (counts.storeGets++, doc.current === undefined ? undefined : structuredClone(doc.current)),
      put: async (_c: string, _i: string, v: unknown) => void (puts.push(plain(v)), (doc.current = structuredClone(v))),
    },
    adapters,
    bus: { emit: (e: unknown) => void emitted.push(plain(e)), on: () => undefined },
    clock: { now: () => 1_000 },
    projectsHeld: new Map((o.projects ?? ["p1", "p2", "p3"]).map(id => [id, { id, name: `project ${id}`, computer: "here", path: `/work/${id}` }])),
    agentLabel,
    defaultAgentOf: (prefs: { defaultAgent?: string }) => prefs.defaultAgent ?? "claude",
    sessions: new Map((o.sessions ?? []).map((k, i) => [`s${i}`, { view: { id: `row${i}`, threadId: k } }])),
    threadRecords: new Map((o.threadRecords ?? []).map(id => [id, {}])),
    ready: async () => {},
    state,
    placeDoor: undefined,
    setups: { get: () => undefined, set: async () => {} },
    setupRefusals: new Map(),
    setupRefusalMoved: () => {},
  };
  held.options.length = 0;
  const area = preferencesArea(ctx as never);
  const reads = held.options.at(-1);
  return { area, ctx, emitted, puts, counts, reads };
}

async function runScript(o: Script) {
  const doc: Doc = { current: o.stored };
  const w = world(o, doc);
  const attempt = async (patch: unknown) => {
    try {
      const r = await w.area.preferences.set(patch as never);
      return { ok: true, preferences: plain(r.preferences), ...("notice" in r ? { notice: r.notice } : {}) };
    } catch (e) {
      return { ok: false, error: errorOf(e) };
    }
  };
  const results: unknown[] = [];
  const probes: unknown[] = [];
  const latest = async () => (w.reads.latestOn === undefined ? null : await w.reads.latestOn());
  const before = o.probe === true ? await latest() : undefined;
  if (o.concurrent === true) results.push(...(await Promise.all(o.steps.map(s => attempt(s.patch)))));
  else
    for (const s of o.steps) {
      results.push(await attempt(s.patch));
      if (o.probe === true) probes.push(await latest());
    }
  const after = plain(await w.area.preferences.get());
  await w.area.preferences.get();
  const storeGets = w.counts.storeGets;
  const second = world({ ...o, steps: [] }, doc);
  const reopened = plain(await second.area.preferences.get());
  return { script: o, results, puts: w.puts, emitted: w.emitted, forgets: w.counts.forgets, storeGets, after, reopened, ...(o.probe === true ? { latestBefore: before, latestAfter: probes } : {}) };
}

it("writes the preferences fixture", async () => {
  const scripts: Script[] = [];
  const add = (s: Script) => scripts.push(s);
  const set = (...patches: unknown[]) => patches.map(patch => ({ patch }));

  // F1 and F2: what get reads from the store, with the env's labs over it.
  add({ name: "get defaults", steps: [] });
  add({ name: "get stored older labs", stored: { theme: "light", labs: true }, steps: [] });
  add({ name: "get env labs over stored", stored: { theme: "dark", labs: false }, env: { WSP_LABS: "1" }, steps: [] });
  add({ name: "get env labs not one", env: { WSP_LABS: "yes" }, steps: [] });
  add({ name: "get malformed stored", stored: { theme: "sepia" }, steps: [] });
  add({ name: "get stored before computer icons", stored: { theme: "light", projectLook: { pr_1: { icon: "rocket" } } }, steps: [] });
  // F3: the chain.
  add({ name: "two sets at once", concurrent: true, steps: set({ theme: "light" }, { sidebarMode: "spaces" }) });
  add({ name: "a refused set among two at once", concurrent: true, steps: set({ defaultAgent: "nope" }, { theme: "light" }, { sidebarMode: "spaces" }) });
  add({ name: "refused then next", steps: set({ defaultAgent: "nope" }, { theme: "dark" }) });
  // preferences.test.ts:29
  add({ name: "over the wire", steps: set({ theme: "light", sidebarWidth: 312, terminalZoom: { ws_a: 2 } }, { sidebarMode: "spaces", sidebarWidth: null, terminalZoom: { ws_b: -1 } }, { terminalZoom: { ws_b: null } }) });
  add({ name: "dismissed release", steps: set({ updateDismissed: "0.3.3" }) });
  add({ name: "usage counts", steps: set({ productUsage: false }) });
  add({ name: "failed hook", adapters: [], steps: set({ defaultAgent: "nope" }) });
  // F4
  add({ name: "threads shut leads", sessions: ["lead1"], threadRecords: ["lead2"], steps: set({ threadsShut: { lead1: true, lead2: true, thr_gone: true } }, { threadsShut: { lead1: null } }, { threadsShut: { lead2: null } }) });
  add({ name: "threads shut stale kept when the patch names none", stored: { threadsShut: { thr_stale: true } }, steps: set({ theme: "dark" }, { threadsShut: { thr_new: true } }) });
  add({ name: "threads shut record alone", threadRecords: ["lead1"], steps: set({ threadsShut: { lead1: true, thr_gone: true } }) });
  add({ name: "threads shut refused default still prunes nothing", sessions: ["lead1"], steps: set({ threadsShut: { lead1: true }, defaultAgent: "nope" }) });
  // F5: every refusal, in the order the TS asks.
  add({
    name: "refusal table",
    steps: set(
      { defaultAgent: "nope" },
      { defaultAgent: "codex" },
      { defaultAgent: null },
      { agentDefaults: { nope: { model: "m" } } },
      { agentDefaults: { claude: { access: "plan" } } },
      { agentDefaults: { codex: { access: "auto-edit" } } },
      { agentDefaults: { codex: { access: "plan" } } },
      { agentDefaults: { mystery: { access: "ask" } } },
      { agentDefaults: { claude: { access: null } } },
      { agentDefaults: { claude: { model: "claude opus" } } },
      { agentDefaults: { claude: { model: "-x", models: { custom: ["a b"] } } } },
      { agentDefaults: { claude: { models: { custom: ["ok-id", "a;b"] } } } },
      { agentDefaults: { claude: { model: "" } } },
      { agentDefaults: { nope: null } },
      { agentDefaults: { claude: { access: "plan" }, nope: {} } },
      { agentDefaults: { nope: {}, claude: { access: "plan" } } },
      { agentDefaults: { claude: { access: "ask", model: "claude-sonnet-5" } } },
      { agentDefaults: { codex: { model: "gpt;5" } } },
      { projectDefaults: { pX: { model: "m" } } },
      { projectDefaults: { p1: { agent: "nope" } } },
      { projectDefaults: { p1: { model: "a b" } } },
      { projectDefaults: { p1: { model: "-x" } } },
      { projectDefaults: { p1: { access: "plan" } } },
      { projectDefaults: { p1: { agent: "codex" } } },
      { projectDefaults: { p1: { access: "auto-edit" } } },
      { projectDefaults: { p1: { access: "plan" } } },
      { projectDefaults: { p1: { agent: "claude", access: "plan" } } },
      { projectDefaults: { p1: { agent: "mystery", access: "ask" } } },
      { projectDefaults: { p1: { access: null } } },
      { projectDefaults: { p2: { agent: "claude", model: "provider/model:tag", access: "auto-edit" } } },
      { projectDefaults: { p1: null } },
      { defaultAgent: "codex", projectDefaults: { p3: { access: "auto-edit" } } },
      { defaultAgent: "nope", agentDefaults: { claude: { access: "plan" } }, projectDefaults: { pX: {} } },
      { agentDefaults: { claude: { access: "plan" } }, projectDefaults: { pX: {} } },
      { projectDefaults: { pX: {}, p1: { agent: "nope" } } },
      { theme: "light", defaultAgent: "nope" },
    ),
  });
  // F6
  const folderless = `EACCES: permission denied, rmdir '${HOME}/.wsp/icons'`;
  add({ name: "icons forget fails", icons: { forget: folderless }, steps: set({ serverIcons: false }, { theme: "dark" }, { serverIcons: true }, { serverIcons: false }) });
  add({ name: "icons forget ok", icons: { forget: null }, steps: set({ serverIcons: false }, { serverIcons: false }) });
  add({ name: "icons absent", steps: set({ serverIcons: false }) });
  add({ name: "icons message without a code", icons: { forget: "disk is busy" }, steps: set({ serverIcons: false }) });
  add({ name: "icons message with a call", icons: { forget: `EBUSY: resource busy or locked, unlink '${HOME}/.wsp/icons/a b'` }, steps: set({ serverIcons: false }) });
  add({ name: "icons message with a lower case code", icons: { forget: "eacces: nope" }, steps: set({ serverIcons: false }) });
  add({ name: "icons message with two codes", icons: { forget: "ENOENT: EPERM: nested, call '/x'" }, steps: set({ serverIcons: false }) });
  // preferences.test.ts:159
  add({ name: "agent versions switch", probe: true, steps: set({ agentVersions: false }, { theme: "dark" }, { agentVersions: true }) });

  const ran = [];
  for (const s of scripts) ran.push(await runScript(s));

  // F6 with a folder outside the home, and the home itself.
  const folders: unknown[] = [];
  for (const folder of ["/srv/icons", HOME, `${HOME}x/icons`, `${HOME}/a`]) {
    const doc: Doc = { current: undefined };
    const w = world({ name: "folder", icons: { forget: "EACCES: no" }, steps: [] }, doc);
    (w.ctx.opts as { serverIcons: { folder: string } }).serverIcons.folder = folder;
    const r = await w.area.preferences.set({ serverIcons: false } as never);
    folders.push({ folder, notice: (r as { notice?: string }).notice ?? null });
  }

  // F7: the agents read.
  const agents = await agentsRead();
  writeFileSync(out, `${JSON.stringify({ home: HOME, tables, catalogAgents, threadEnv, adapters: ADAPTERS, scripts: ran, folders, agents }, null, 1)}\n`);
});

const channel = (answer: object) => ({ send: async () => answer, close() {}, closed: async () => ({}) });

async function agentsRead() {
  const entries: Record<string, { record: Record<string, unknown>; machine: object }> = {
    wCloud: { record: { id: "wCloud", name: "alpha", phase: "running", project: "p1", kind: "cloud", place: "box", machineId: "m1" }, machine: {} },
    wNap: { record: { id: "wNap", name: "beta", phase: "napping", project: "p2", kind: "cloud", place: "box", machineId: "m2" }, machine: {} },
    wAway: { record: { id: "wAway", name: "gamma", phase: "running", project: "p1", kind: "cloud", place: "elsewhere", machineId: "m3" }, machine: {} },
    wNoPlace: { record: { id: "wNoPlace", name: "delta", phase: "running", project: "p1", kind: "cloud", machineId: "m4" }, machine: {} },
    wLocal: { record: { id: "wLocal", name: "mac", phase: "running", project: "p1", kind: "local", machineId: "" }, machine: {} },
    wBad: { record: { id: "wBad", name: "epsilon", phase: "running", project: "p1", kind: "cloud", machineId: "m5" }, machine: {} },
  };
  const projectRows = [
    { id: "p1", name: "landing", computer: "pl_box", path: "/home/maya/landing", checkout: "/srv/landing" },
    { id: "p2", name: "api", computer: "pl_box", path: "/home/maya/api" },
    { id: "p3", name: "mac", computer: "here", path: "/Users/maya/mac" },
  ];
  const log: unknown[] = [];
  const emitted: unknown[] = [];
  const refusals = new Set<string>();
  let setFails: string | null = null;
  const sets: unknown[] = [];
  const names: Record<string, string> = { pl_gone: "studio" };
  const doc: Doc = { current: undefined };
  const ctx = {
    opts: { env: {} },
    backend: { capabilities: { callbackRelay: true } },
    store: { get: async () => doc.current, put: async (_c: string, _i: string, v: unknown) => void (doc.current = v) },
    adapters: Object.fromEntries(ADAPTERS.map(id => [id, {}])),
    bus: { emit: (e: unknown) => void emitted.push(plain(e)), on: () => undefined },
    clock: { now: () => 1_000 },
    projectsHeld: new Map(projectRows.map(p => [p.id, p])),
    agentLabel,
    defaultAgentOf: () => "claude",
    sessions: new Map(),
    threadRecords: new Map(),
    ready: async () => void log.push("ready"),
    state: {},
    placeDoor: {
      channel: (placeId: string, onEvent: unknown) => (placeId === "pl_box" ? channel({ chan: `link ${placeId}`, sameOnEvent: onEvent === lastOnEvent }) : undefined),
      nameOf: (placeId: string) => names[placeId],
    },
    places: { list: () => ["box"] },
    entryOf: async (id: string, origin: unknown) => {
      log.push(["entryOf", id, origin !== undefined]);
      const e = entries[id];
      if (e === undefined) throw new Error(`no workspace ${id}`);
      return e;
    },
    projectHeld: (id: string) => ({ id, name: projectRows.find(p => p.id === id)?.name ?? id }),
    threadEnv: (_entry: unknown, agent: string) => threadEnv[agent] ?? {},
    placeIdOf: (record: { place?: string }) => record.place,
    view: (record: { id: string }) => (record.id === "wNap" ? { pauseMode: "disk" } : {}),
    checkoutOf: (record: { id: string; project: string }) => {
      if (record.id === "wBad") throw new Error("no checkout here");
      return `/work/${record.project}`;
    },
    workspaces: { daemonChannel: async (id: string, _onEvent: unknown, origin: unknown) => channel({ chan: `workspace ${id}`, origin: origin !== undefined }) },
    channelOver: (_road: unknown, _name: string, _onEvent: unknown) => channel({ chan: "local" }),
    localRoad: async () => (log.push("localRoad"), {}),
    placeStores: () => ({}),
    storesHere: async () => ({}),
    configFolderOn: () => "",
    setups: {
      get: (placeId: string, agent: string) => (agent === "claude" ? { on: false, env: { B: "1", A: "2" }, program: "claude-wsp" } : undefined),
      set: async (placeId: string, agent: string, change: unknown) => {
        sets.push(["set", placeId, agent, change]);
        if (setFails !== null) throw new Error(setFails);
      },
    },
    setupRefusals: refusals,
    setupRefusalMoved: (placeId: string, agent: string) => void sets.push(["moved", placeId, agent]),
  };
  let lastOnEvent: unknown;
  held.options.length = 0;
  preferencesArea(ctx as never);
  const o = held.options.at(-1);

  const workspaces: unknown[] = [];
  for (const [id, origin] of [["wCloud", "here"], ["wNap", undefined], ["wAway", undefined], ["wNoPlace", undefined], ["wLocal", "here"], ["wBad", undefined], ["wMissing", undefined]] as const) {
    log.length = 0;
    try {
      const r = await o.workspace(id, origin);
      const { machine: _machine, ...rest } = r;
      workspaces.push({ id, origin: origin !== undefined, answer: plain(rest), log: plain(log) });
    } catch (e) {
      workspaces.push({ id, origin: origin !== undefined, error: errorOf(e), log: plain(log) });
    }
  }
  const projects: unknown[] = [];
  for (const place of ["pl_box", "here", "pl_none"]) {
    log.length = 0;
    projects.push({ place, answer: plain(await o.projects(place)), log: plain(log) });
  }
  const channels: unknown[] = [];
  const targets: [unknown, string | undefined][] = [[{ workspaceId: "wCloud" }, "here"], [{ workspaceId: "wCloud" }, undefined], [{ placeId: "here" }, undefined], [{ placeId: "pl_box" }, undefined], [{ placeId: "pl_gone" }, undefined], [{ placeId: "pl_nameless" }, undefined]];
  for (const [target, origin] of targets) {
    log.length = 0;
    const onEvent = () => {};
    lastOnEvent = onEvent;
    try {
      const c = await o.channel(target, onEvent, origin);
      channels.push({ target, origin: origin !== undefined, answer: plain(await c.send({})), log: plain(log) });
    } catch (e) {
      channels.push({ target, origin: origin !== undefined, error: errorOf(e), log: plain(log) });
    }
  }
  const changed: unknown[] = [];
  for (const target of [undefined, { placeId: "pl_box" }, { workspaceId: "wCloud" }]) {
    emitted.length = 0;
    o.changed(target);
    changed.push({ target: target ?? null, emitted: plain(emitted) });
  }
  const setupOf = ADAPTERS.concat(["ghost"]).map(agent => ({ agent, answer: plain(o.setupOf("here", agent)) }));
  const writes: unknown[] = [];
  const cases: { place: string; agent: string; change: unknown; refusals: string[]; fails: string | null }[] = [
    { place: "here", agent: "ghost", change: { on: false }, refusals: ["here:ghost"], fails: null },
    { place: "here", agent: "claude", change: { on: false }, refusals: ["here:claude", "pl_box:claude"], fails: null },
    { place: "here", agent: "codex", change: { on: true }, refusals: ["here:claude"], fails: null },
    { place: "pl_box", agent: "claude", change: { env: { A: "1" } }, refusals: ["pl_box:claude"], fails: "config folder is the home" },
  ];
  for (const c of cases) {
    sets.length = 0;
    refusals.clear();
    for (const r of c.refusals) refusals.add(r);
    setFails = c.fails;
    try {
      await o.setupWrite(c.place, c.agent, c.change);
      writes.push({ ...c, ok: true, calls: plain(sets), refusalsLeft: [...refusals] });
    } catch (e) {
      writes.push({ ...c, ok: false, error: errorOf(e), calls: plain(sets), refusalsLeft: [...refusals] });
    }
  }
  const relayed = o.relayed();
  return { entries: Object.fromEntries(Object.entries(entries).map(([k, v]) => [k, v.record])), projectRows, names, workspaces, projects, channels, changed, setupOf, writes, relayed };
}
