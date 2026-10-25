// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the channels_daemon tests compare to: channelsArea (packages/runtime/src/account/channels.ts at
// 65f6b89c7, lines 1-280) over a fake context, every scenario's inputs written beside what the TS answered, as
// tests/fixtures/oracle/channels_daemon.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 channels_daemon.oracle.ts
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { afterAll, it, vi } from "vitest";
import {
  absentComputer, machineWord, noSuchProjectLine, placeBlocked, placeServesDaemonLine, slateServerBehindLine, spendCapRefusal,
  AGENTS_ON, PIPE_DAEMON_VERSION, STDIO_DAEMON_VERSION, HTTP_POST_DAEMON_VERSION, THIS_COMPUTER,
} from "@wsp/protocol";

const here = (rel: string): string => fileURLToPath(new URL(rel, import.meta.url));
const { channelsArea } = await import("../../../../packages/runtime/src/account/channels.ts");
const { eventBus } = await import("../../../../packages/runtime/src/types/events.ts");
const { memoryStore } = await import("../../../../packages/runtime/src/store.ts");
const out = here("../tests/fixtures/oracle/channels_daemon.json");
const channelsSource = readFileSync(here("../../../../packages/runtime/src/account/channels.ts"), "utf8");

type Frame = Record<string, unknown>;
type Chan = { send(frame: never): Promise<never>; close(): void; closed: Promise<unknown> };
const tick = () => new Promise(r => setImmediate(r));
const flush = async () => {
  for (let i = 0; i < 20; i++) await tick();
};
const json = <T>(v: T): T => (v === undefined ? (null as T) : (JSON.parse(JSON.stringify(v)) as T));
const said = (e: unknown): unknown => {
  if (!(e instanceof Error)) return { thrown: String(e) };
  if (e instanceof TypeError) throw e;
  const x = e as Error & { fix?: string; kind?: string; code?: string };
  return { message: x.message, ...(x.fix !== undefined ? { fix: x.fix } : {}), ...(x.kind !== undefined ? { kind: x.kind } : {}), ...(x.name === "DaemonRefusal" ? { refusal: true, code: x.code ?? null } : {}) };
};
const trying = async (f: () => unknown): Promise<unknown> => {
  try {
    return { ok: json(await f()) };
  } catch (e) {
    return { error: said(e) };
  }
};

/** A daemon that answers each frame by its own function, recording what it heard. */
function scripted(answer: (frame: Frame) => Frame | Promise<Frame>) {
  const heard: Frame[] = [];
  let closes = 0;
  let end: (v: unknown) => void = () => {};
  const closed = new Promise<unknown>(resolve => (end = resolve));
  const channel: Chan = {
    send: (async (frame: Frame) => {
      heard.push(structuredClone(frame));
      return answer(frame);
    }) as never,
    close: () => (closes++, end({ code: 1000, reason: "" })),
    closed,
  };
  return { channel, heard, closes: () => closes, end: () => end({ code: 1006, reason: "" }) };
}

const reach = { url: "wss://box/ws1", expiresAt: 9_999_999, daemonToken: "daemon-token" };
const NOW = 1_700_000_000_000;

interface Opts {
  door?: boolean;
  broker?: boolean;
  localDaemon?: boolean;
  homeDir?: string | undefined;
}

function world(o: Opts = {}) {
  const warns: string[] = [];
  vi.spyOn(console, "warn").mockImplementation((m: unknown) => void warns.push(String(m)));
  const state = {
    broker: o.broker ?? false,
    road: reach as unknown,
    roadError: undefined as Error | undefined,
    roadsAsked: [] as string[],
    dialled: [] as { url: string; token: string }[],
    dials: [] as (Chan | Error)[],
    listeners: [] as ((e: Frame) => void)[],
    adopted: [] as string[],
    minted: [] as unknown[],
    revoked: [] as string[],
    live: new Map<string, string>(),
    failRevoke: false,
    rootOf: {} as Record<string, string>,
    offers: {} as Record<string, string>,
    settings: {} as Record<string, unknown>,
    turnLimits: {} as Record<string, number | null | undefined>,
    names: {} as Record<string, string>,
    joined: new Set<string>(),
    reports: {} as Record<string, unknown>,
    rows: [] as unknown[],
    reportsAsked: [] as string[],
    links: {} as Record<string, Chan>,
    linkListeners: [] as { place: string; fn: (e: Frame) => void }[],
    spend: [] as { place: string; todayUsd?: number }[],
    spendAsked: [] as unknown[],
    localRoad: reach as unknown,
    wired: "solari",
  };
  const door = {
    offerOf: (p: string) => state.offers[p],
    settingsAt: (p: string) => state.settings[p],
    turnLimitAt: (p: string) => state.turnLimits[p],
    nameOf: (p: string) => state.names[p] ?? p,
    joined: (p: string) => state.joined.has(p),
    reportOf: async (p: string) => (state.reportsAsked.push(p), state.reports[p]),
    rows: async () => state.rows,
    channel: (p: string, fn: (e: Frame) => void) => (state.linkListeners.push({ place: p, fn }), state.links[p]),
  };
  const ctx = {
    local: o.localDaemon === true ? { daemonRoad: async () => reach, homeDir: "/home/me" } : { homeDir: "/home/me" },
    placeDoor: o.door === false ? undefined : door,
    clock: { now: () => Date.now() },
    projectsHeld: new Map<string, unknown>(),
    threadRecords: new Map<string, Frame>(),
    sessions: new Map<string, unknown>(),
    transcriptIndex: new Map<string, unknown>(),
    places: { get wired() { return state.wired; } },
    moduleOf: (_kind: string) => ({
      homeDir: () => o.homeDir,
      daemonRoad: async (entry: { record: { id: string } }) => {
        state.roadsAsked.push(entry.record.id);
        if (state.roadError !== undefined) throw state.roadError;
        return state.road;
      },
      turnRoad: "relayed",
    }),
    openChannel: async (c: { url: string; token: string; onEvent: (e: Frame) => void }) => {
      state.dialled.push({ url: c.url, token: c.token });
      state.listeners.push(c.onEvent);
      const next = state.dials.shift();
      if (next === undefined) throw new Error("no dial scripted");
      if (next instanceof Error) throw next;
      return next;
    },
    adoptPause: async (entry: { record: { id: string } }) => void state.adopted.push(entry.record.id),
    opts: { broker: { ready: () => state.broker } },
    rootOf: (id: string) => state.rootOf[id] ?? id,
    deviceDoor: {
      mint: async (name: string, scope: unknown, now: number, more: unknown) => {
        const n = state.minted.length + 1;
        state.minted.push(json({ name, scope, now, more }));
        state.live.set(`tok_${n}`, name);
        return { deviceId: `dev_${n}`, deviceToken: `tok_${n}` };
      },
      revoke: async (id: string) => {
        state.revoked.push(id);
        state.live.delete(`tok_${id.replace("dev_", "")}`);
        if (state.failRevoke) throw new Error("the door is shut");
        return true;
      },
    },
    status: { spend: async (rows: { id: string }[], now: number) => (state.spendAsked.push(json({ ids: rows.map(r => r.id), now })), state.spend) },
    localRoad: async () => state.localRoad,
    bus: eventBus(),
    store: memoryStore(),
    state: {},
    procScopesMoved: () => {},
  };
  const area = channelsArea(ctx as never);
  return { ctx, area, state, warns };
}
type World = ReturnType<typeof world>;

const rec = (o: Record<string, unknown>) => ({ id: "ws1", name: "alpha", machineId: "m1", phase: "running", kind: "cloud", project: "p1", ...o });
function machine(o: { served?: (f: Frame) => Promise<Frame>; state?: () => Promise<string> } = {}) {
  const calls = { state: 0 };
  return {
    id: "m1",
    ...(o.served !== undefined ? { daemonFrame: o.served } : {}),
    state: async () => (calls.state++, o.state === undefined ? "running" : o.state()),
    calls,
  };
}
const entryOf = (record: Record<string, unknown>, m: ReturnType<typeof machine> = machine()) => ({ record, machine: m });

afterAll(() => vi.restoreAllMocks());

it("writes the channels_daemon fixture", async () => {
  const fixture: Record<string, unknown> = {};

  // Constants and the sentences the area builds from the protocol.
  {
    const m = (re: RegExp): number => Number(re.exec(channelsSource)![1]!.replace(/_/g, ""));
    const ops = /const OP_TOKEN_OPS[^=]*= new Set\(\[([^\]]*)\]\)/.exec(channelsSource)![1]!;
    const reads = /const BLOCKED_READS[^=]*= new Set\(\[([^\]]*)\]\)/.exec(channelsSource)![1]!;
    fixture["constants"] = {
      HELLO_WAIT_MS: m(/const HELLO_WAIT_MS = ([\d_]+);/),
      ORIGIN_READ_MS: m(/const ORIGIN_READ_MS = ([\d_]+);/),
      OP_TOKEN_OPS: JSON.parse(`[${ops}]`),
      BLOCKED_READS: JSON.parse(`[${reads}]`),
      PIPE_DAEMON_VERSION,
      STDIO_DAEMON_VERSION,
      HTTP_POST_DAEMON_VERSION,
      THIS_COMPUTER,
      AGENTS_ON,
    };
  }

  // D1, D2: what a record's turns read.
  {
    const indexFacts: [string, Record<string, string>][] = [["sess-1", { cwd: "/idx/cwd", permissionMode: "plan", effort: "high" }], ["sess-2", { cwd: "/only/index", model: "opus" }], ["sess-4", { permissionMode: "plan" }]];
    const w = world();
    const index = {
      starts: new Map([["th_a", "sess-1"]]),
      cut: new Map([["th_a", true], ["th_b", false]]),
      facts: new Map<string, Record<string, string>>(indexFacts),
    };
    w.ctx.transcriptIndex.set("ws1", index);
    const rows: Frame[] = [
      { workspaceId: "ws1", claudeSessionId: "sess-1", cwd: "/old", permissionMode: "auto" },
      { workspaceId: "ws1", claudeSessionId: "sess-1", cwd: "/new" },
      { workspaceId: "ws2", claudeSessionId: "sess-1", cwd: "/other-ws" },
      { workspaceId: "ws1", claudeSessionId: "sess-3", cwd: "/x" },
    ];
    rows.forEach((view, i) => w.ctx.sessions.set(`s${i}`, { view }));
    w.ctx.threadRecords.set("th_a", { workspaceId: "ws1", permissionMode: "bypassPermissions" });
    w.ctx.threadRecords.set("th_b", { workspaceId: "ws1" });
    const a = w.area;
    const facts = ["cwd", "permissionMode", "model", "effort"] as const;
    fixture["facts"] = {
      rows,
      index: { starts: [["th_a", "sess-1"]], cut: [["th_a", true], ["th_b", false]], facts: indexFacts },
      threads: { th_a: { permissionMode: "bypassPermissions" }, th_b: {} },
      startedAs: [["ws1", "th_a"], ["ws1", "th_b"], ["ws9", "th_a"]].map(([ws, th]) => ({ ws, th, answer: json(a.startedAs(ws!, th!)) })),
      cutBefore: [["ws1", "th_a"], ["ws1", "th_b"], ["ws1", "th_zzz"], ["ws9", "th_a"]].map(([ws, th]) => ({ ws, th, answer: a.cutBefore(ws!, th!) })),
      resumedFact: [["ws1", "sess-1"], ["ws1", "sess-2"], ["ws2", "sess-1"], ["ws1", "sess-3"], ["ws1", "sess-4"], ["ws9", "sess-1"], ["ws1", "sess-none"]].flatMap(([ws, resume]) =>
        facts.map(fact => ({ ws, resume, fact, answer: json(a.resumedFact(ws!, resume!, fact)) })),
      ),
      folderOf: [["ws1", "sess-1"], ["ws1", "sess-2"], ["ws2", "sess-1"], ["ws1", "sess-none"]].map(([ws, resume]) => ({ ws, resume, answer: json(a.folderOf(ws!, resume!)) })),
      accessOf: [["ws1", "th_a", "sess-1"], ["ws1", "th_b", "sess-1"], ["ws1", "th_b", undefined], ["ws1", "th_none", "sess-1"], ["ws1", "th_none", "sess-2"], ["ws1", "th_none", "sess-4"], ["ws1", "th_a", undefined]].map(([ws, th, resume]) => ({ ws, th, resume: resume ?? null, answer: json(a.accessOf(ws!, th!, resume)) })),
    };
  }

  // D3: the machine's home.
  {
    const withHome = world({ homeDir: "/home/dev" }).area;
    const without = world({ homeDir: undefined }).area;
    fixture["home"] = { with: json(withHome.homeOf(rec({}) as never)), without: json(without.homeOf(rec({}) as never)) };
  }

  // D4, D5, D6: place, provider, settings, the turn wall, the agents.
  {
    const set = (w: World) => {
      w.state.offers = { pl_x: "hetzner" };
      w.state.settings = { pl_x: { spawn: { maxMachines: 1 } }, pl_y: { spawn: { maxDepth: 4 } }, pl_z: { threads: 2 } };
      w.state.turnLimits = { here: undefined, pl_x: null, pl_y: 3_600_000, solari: 21_600_000 };
    };
    const records = [
      { name: "a local folder", record: rec({ kind: "local" }) },
      { name: "a cloud fork with no place", record: rec({ kind: "cloud" }) },
      { name: "a cloud fork on a place the door offers", record: rec({ kind: "cloud", place: "pl_x" }) },
      { name: "a folder on a joined computer", record: rec({ kind: "place", place: "pl_y" }) },
      { name: "a folder on a place nothing is known of", record: rec({ kind: "place", place: "pl_q" }) },
      { name: "a place folder naming no place", record: rec({ kind: "place" }) },
      { name: "a record with its own agents", record: rec({ kind: "cloud", place: "pl_x", agents: { maxMachines: 7, maxDepth: 5 } }) },
    ];
    const rows = (w: World) =>
      records.map(({ name, record }) => ({
        name,
        provider: json(w.area.providerOf(record as never)),
        placeId: json(w.area.placeIdOf(record as never)),
        turnLimit: (() => {
          const l = w.area.turnLimitOf(record as never);
          return l === undefined ? null : { deadlineMs: l.deadlineMs === Number.POSITIVE_INFINITY ? "Infinity" : l.deadlineMs };
        })(),
        agents: json(w.area.agentsHeld(record as never)),
      }));
    const withDoor = world();
    set(withDoor);
    const noDoor = world({ door: false });
    const settingsAt = (w: World) => [undefined, "pl_x", "pl_y", "pl_z", "pl_unknown"].map(p => ({ place: p ?? null, answer: json(w.area.settingsAt(p)) }));
    fixture["places"] = {
      offers: { pl_x: "hetzner" },
      settings: { pl_x: { spawn: { maxMachines: 1 } }, pl_y: { spawn: { maxDepth: 4 } }, pl_z: { threads: 2 } },
      turnLimits: { here: "undefined", pl_x: null, pl_y: 3_600_000, solari: 21_600_000 },
      wired: "solari",
      records: records.map(r => ({ name: r.name, record: r.record })),
      withDoor: rows(withDoor),
      noDoor: rows(noDoor),
      settingsWithDoor: settingsAt(withDoor),
      settingsNoDoor: settingsAt(noDoor),
      spawnAt: [undefined, "pl_x", "pl_y", "pl_z", "pl_unknown"].map(p => ({ place: p ?? null, answer: json(withDoor.area.spawnAt(p)) })),
    };
  }

  // D7: the project a record names.
  {
    const w = world();
    const refusal = (names: string[]) => {
      const x = world();
      names.forEach((n, i) => x.ctx.projectsHeld.set(`p${i}`, { id: `p${i}`, name: n }));
      return trying(() => x.area.projectHeld("nope"));
    };
    const project = { id: "pA", name: "alpha", computer: "here", path: "/home/u/alpha", source: { kind: "folder", path: "/home/u/alpha" }, remote: "", defaultBranch: "main", memoryKey: "", memoryDir: "", createdAt: "2026-10-25T00:00:00.000Z", };
    w.ctx.projectsHeld.set("pA", project);
    fixture["projects"] = {
      empty: await refusal([]),
      two: await refusal(["alpha", "beta"]),
      held: await trying(() => w.area.projectHeld("pA")),
      ref: json(w.area.refOf(project as never)),
      refKeys: Object.keys(w.area.refOf(project as never)),
      sentences: { none: noSuchProjectLine("nope", []), two: noSuchProjectLine("nope", ["alpha", "beta"]) },
    };
  }

  // D8: where a record's threads work.
  {
    const w = world();
    const projects = [
      { id: "pTop", name: "top", computer: "here", path: "/repo", git: { top: "/repo" } },
      { id: "pSub", name: "sub", computer: "here", path: "/repo/pkg/x", git: { top: "/repo" } },
      { id: "pNoGit", name: "nogit", computer: "here", path: "/plain/folder" },
      { id: "pOut", name: "out", computer: "here", path: "/repo/pkg", git: { top: "/repo/sub" } },
      { id: "pSlash", name: "slash", computer: "here", path: "/repo/pkg", git: { top: "/repo/" } },
      { id: "pDeep", name: "deep", computer: "here", path: "/a/b/c/d", git: { top: "/a/b" } },
    ];
    for (const p of projects) w.ctx.projectsHeld.set(p.id, { ...p, source: { kind: "folder", path: p.path }, remote: "" });
    const cases = [
      { name: "a sub-machine works in its own folder", record: rec({ project: "pTop", sub: { on: "solari", repo: "acme/x", base: { branch: "main", sha: "abc" }, branch: "wsp/sub", folder: "/work/sub" } }) },
      { name: "no worktree", record: rec({ project: "pSub" }) },
      { name: "a worktree that is gone", record: rec({ project: "pSub", worktree: { path: "/wt/gone", made: true, gone: true } }) },
      { name: "a worktree of a project at its repo's top", record: rec({ project: "pTop", worktree: { path: "/wt/one", made: true } }) },
      { name: "a worktree of a subfolder", record: rec({ project: "pSub", worktree: { path: "/wt/two", made: true } }) },
      { name: "a worktree of a folder with no git top", record: rec({ project: "pNoGit", worktree: { path: "/wt/three", made: true } }) },
      { name: "a project path outside its recorded top", record: rec({ project: "pOut", worktree: { path: "/wt/four", made: true } }) },
      { name: "a top with a trailing slash", record: rec({ project: "pSlash", worktree: { path: "/wt/five", made: true } }) },
      { name: "a deep subfolder", record: rec({ project: "pDeep", worktree: { path: "/wt/six/", made: true } }) },
      { name: "a project this host does not hold", record: rec({ project: "pGone", worktree: { path: "/wt/seven", made: true } }) },
    ];
    fixture["checkout"] = {
      projects: projects.map(p => ({ ...p, source: { kind: "folder", path: p.path }, remote: "" })),
      cases: await Promise.all(cases.map(async c => ({ name: c.name, record: c.record, answer: await trying(() => w.area.checkoutOf(c.record as never)) }))),
    };
  }

  // D9: what the computer holding a workspace is called.
  {
    const withDoor = world();
    withDoor.state.names = { pl_x: "Hetzner box" };
    const noDoor = world({ door: false });
    const cases = [
      { name: "a cloud fork", record: rec({ kind: "cloud" }) },
      { name: "a local folder", record: rec({ kind: "local" }) },
      { name: "a folder on a named computer", record: rec({ kind: "place", place: "pl_x" }) },
      { name: "a folder on a computer with no name", record: rec({ kind: "place", place: "pl_y" }) },
    ];
    fixture["computer"] = {
      names: { pl_x: "Hetzner box" },
      cases: cases.map(c => ({ name: c.name, record: c.record, withDoor: withDoor.area.computerOf(entryOf(c.record) as never), noDoor: noDoor.area.computerOf(entryOf(c.record) as never) })),
      words: { cloud: machineWord("cloud"), local: machineWord("local"), place: machineWord("place") },
    };
  }

  // D10: the machine's own frames.
  {
    const w = world();
    const heard: Frame[] = [];
    const m = machine({ served: async f => (heard.push(f), { ok: true, id: f["id"] ?? null }) });
    const served = w.area.servedByItsComputer(entryOf(rec({}), m) as never);
    const reply = served === undefined ? null : await served({ op: "git.status", id: 4 });
    fixture["served"] = { with: served !== undefined, reply, heard, without: w.area.servedByItsComputer(entryOf(rec({})) as never) !== undefined };
  }

  // D11: a dial of a reach.
  {
    const w = world();
    const events: Frame[] = [];
    const none = await trying(() => w.area.channelOver({ url: "wss://x", expiresAt: 1 } as never, "alpha", () => {}));
    const dialledAfterNone = json(w.state.dialled);
    const d = scripted(() => ({ ok: true }));
    w.state.dials.push(d.channel);
    const opened = await trying(async () => {
      const c = await w.area.channelOver({ url: "wss://x/ws", expiresAt: 1, daemonToken: "tok" } as never, "alpha", e => void events.push(e));
      return c === (d.channel as unknown);
    });
    w.state.listeners[0]!({ type: "daemon.hello", version: 1 });
    fixture["channel_over"] = { none, dialledAfterNone, opened, dialled: w.state.dialled, events };
  }

  // D12: a dial that fails under a running record asks the machine's state once.
  {
    const run = async (name: string, o: { phase?: string; state?: () => Promise<string>; roadError?: string; dial?: "fail" | "ok" }) => {
      const w = world();
      const m = machine({ state: o.state ?? (async () => "running") });
      if (o.roadError !== undefined) w.state.roadError = new Error(o.roadError);
      else w.state.dials.push(o.dial === "ok" ? scripted(() => ({ ok: true })).channel : new Error("dial refused"));
      const entry = entryOf(rec({ phase: o.phase ?? "running" }), m);
      const result = await trying(async () => {
        // ownDaemonChannel is not exposed: an unbrokered withDaemon dials it and nothing else.
        await w.area.withDaemon(entry as never, async () => "opened");
        return "opened";
      });
      return { name, phase: o.phase ?? "running", state: o.state === undefined ? "running" : "scripted", roadError: o.roadError ?? null, dial: o.dial ?? "fail", result, stateAsked: m.calls.state, adopted: w.state.adopted, roadsAsked: w.state.roadsAsked };
    };
    fixture["own_channel"] = [
      await run("a failed dial on a paused machine adopts the pause", { state: async () => "paused" }),
      await run("a failed dial on a running machine adopts nothing", {}),
      await run("a failed state read counts as running", { state: async () => Promise.reject(new Error("no answer")) }),
      await run("a record that is not running asks nothing", { phase: "napping", state: async () => "paused" }),
      await run("a failed road on a paused machine adopts the pause", { state: async () => "paused", roadError: "the route is gone" }),
      await run("a dial that works asks nothing", { dial: "ok", state: async () => "paused" }),
      await run("a failed dial on a machine the provider calls something else", { state: async () => "stopped" }),
    ];
  }

  // D13: a reply, and a channel closed however the work ends.
  {
    const w = world();
    const replies: Frame[] = [
      { ok: true, id: 1, extra: [1] },
      { ok: false, code: "busy", error: "try later" },
      { ok: false, code: 5, error: "x" },
      { ok: false },
      { ok: false, error: 7 },
      { ok: "true", error: "loose" },
      { error: "no ok at all" },
      { ok: false, code: "no-host-cli", error: "no gh here" },
    ];
    // replyOf is not exposed: overChannel hands it each reply.
    const replying = async (r: Frame) => {
      const d = scripted(() => r);
      const out = await trying(() => w.area.overChannel(d.channel as never, ask => ask({ op: "git.pr" } as never)));
      return { frame: { op: "git.pr" }, reply: r, answer: out };
    };
    fixture["reply_of"] = [];
    for (const r of replies) (fixture["reply_of"] as unknown[]).push(await replying(r));
    const run = async (name: string, answer: (f: Frame) => Frame, work: (ask: (f: Frame) => Promise<unknown>) => Promise<unknown>) => {
      const d = scripted(answer);
      const result = await trying(() => w.area.overChannel(d.channel as never, work as never));
      return { name, result, heard: d.heard, closes: d.closes() };
    };
    fixture["over_channel"] = [
      await run("the work's answer comes back and the channel closes", () => ({ ok: true, v: 1 }), async ask => ({ first: await ask({ op: "git.status" }), second: await ask({ op: "ping" }) })),
      await run("a refused reply throws its refusal and the channel closes", () => ({ ok: false, code: "busy", error: "x" }), ask => ask({ op: "git.push" })),
      await run("the work's own error closes it too", () => ({ ok: true }), async () => Promise.reject(new Error("the work failed"))),
    ];
  }

  // D14: withDaemon, three roads.
  {
    const frames = [{ op: "git.status" }, { op: "git.diff", cwd: "/x" }];
    // served: the frames go up the computer's link, nothing is dialled.
    const s = world();
    const heardServed: Frame[] = [];
    const m = machine({ served: async f => (heardServed.push(f), f["op"] === "git.diff" ? { ok: false, code: "no-diff", error: "nothing to diff" } : { ok: true, id: null }) });
    const served = await trying(() => s.area.withDaemon(entryOf(rec({}), m) as never, async ask => [await ask(frames[0] as never), await trying(() => ask(frames[1] as never))]));
    // unbrokered: dial the daemon, close it after.
    const u = world({ broker: false });
    const ud = scripted(f => ({ ok: true, op: f["op"] }));
    u.state.dials.push(ud.channel);
    const unbrokered = await trying(() => u.area.withDaemon(entryOf(rec({})) as never, async ask => ask(frames[0] as never)));
    // brokered: the hello on the dial's listener, tokens for the git ops.
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] });
    vi.setSystemTime(NOW);
    const hello = async (name: string, version: unknown, ops: string[], o: { threads?: boolean } = {}) => {
      const b = world({ broker: true });
      if (o.threads !== false) {
        b.ctx.threadRecords.set("th_old", { workspaceId: "ws1" });
        b.ctx.threadRecords.set("th_new", { workspaceId: "ws1" });
        b.ctx.threadRecords.set("th_other", { workspaceId: "ws2" });
      }
      b.state.rootOf = { th_new: "th_root" };
      const liveAtOp: boolean[] = [];
      const bd = scripted(f => {
        liveAtOp.push(b.state.live.has(String(f["token"])));
        return f["op"] === "git.push" && ops.includes("refuse") ? { ok: false, error: "git.push was refused" } : { ok: true };
      });
      b.state.dials.push(bd.channel);
      const result = await trying(() =>
        b.area.withDaemon(entryOf(rec({})) as never, async ask => {
          if (version !== "never") b.state.listeners[0]!({ type: "daemon.hello", version });
          const answers: unknown[] = [];
          for (const op of ops.filter(x => x !== "refuse")) answers.push(await trying(() => ask({ op, cwd: "/x" } as never)));
          return answers;
        }),
      );
      return { name, version: version === "never" ? "never" : (version as unknown), ops, threads: o.threads !== false, result, heard: bd.heard, liveAtOp, minted: b.state.minted, revoked: b.state.revoked, closes: bd.closes(), warns: b.warns };
    };
    const brokered = [
      await hello("a git push and a git pr each carry a token of their own", PIPE_DAEMON_VERSION, ["git.push", "git.pr", "git.status", "git.diff"]),
      await hello("every op of the set carries one", PIPE_DAEMON_VERSION, ["git.push", "git.startOn", "git.update", "git.mergeIn", "git.pr", "git.prCheckout", "git.prList", "pty.create"]),
      await hello("a refused op's token is taken away too", PIPE_DAEMON_VERSION, ["refuse", "git.push"]),
      await hello("a daemon from before op tokens gets none", PIPE_DAEMON_VERSION - 1, ["git.push", "git.pr"]),
      await hello("a hello naming no number reads unknown", "x", ["git.push"]),
      await hello("a hello a version above the least still sends one", PIPE_DAEMON_VERSION + 5, ["git.push"]),
      await hello("a workspace with no thread is refused before anything is sent", PIPE_DAEMON_VERSION, ["git.push", "git.status"], { threads: false }),
    ];
    // The hello that never comes: the op waits 2000 ms on the clock and goes without a token.
    const late = world({ broker: true });
    late.ctx.threadRecords.set("th_new", { workspaceId: "ws1" });
    const ld = scripted(() => ({ ok: true }));
    late.state.dials.push(ld.channel);
    let settled = false;
    const pending = late.area.withDaemon(entryOf(rec({})) as never, async ask => ask({ op: "git.push" } as never)).then(r => ((settled = true), r));
    await flush();
    await vi.advanceTimersByTimeAsync(1_999);
    const heardAt1999 = ld.heard.length;
    const settledAt1999 = settled;
    await vi.advanceTimersByTimeAsync(1);
    await pending;
    const hello_wait = { heardAt1999, settledAt1999, heardAfter: ld.heard, minted: late.state.minted };
    // A failed revoke warns and the reply still comes back.
    vi.setSystemTime(NOW);
    const warn = world({ broker: true });
    warn.ctx.threadRecords.set("th_new", { workspaceId: "ws1" });
    warn.state.failRevoke = true;
    const wd = scripted(() => ({ ok: true, v: 1 }));
    warn.state.dials.push(wd.channel);
    const afterFailedRevoke = await trying(async () => {
      const p = warn.area.withDaemon(entryOf(rec({ name: "pricing" })) as never, async ask => ask({ op: "git.pr" } as never));
      await flush();
      warn.state.listeners[0]!({ type: "daemon.hello", version: PIPE_DAEMON_VERSION });
      return p;
    });
    vi.useRealTimers();
    fixture["with_daemon"] = {
      now: NOW,
      served: { result: served, heard: heardServed, dialled: json(s.state.dialled), roadsAsked: s.state.roadsAsked },
      unbrokered: { result: unbrokered, heard: ud.heard, dialled: u.state.dialled, closes: ud.closes(), listeners: u.state.listeners.length },
      brokered,
      hello_wait,
      failed_revoke: { result: afterFailedRevoke, warns: warn.warns, minted: warn.state.minted, revoked: warn.state.revoked },
    };
  }

  // D15: whether the host keeps the machine's credentials.
  {
    const cases: { name: string; ready: boolean; kind: string; served: boolean; place?: string; joined?: boolean; door?: boolean; expected: boolean }[] = [];
    const add = async (name: string, o: { ready?: boolean; kind?: string; served?: boolean; place?: string; joined?: boolean; door?: boolean }) => {
      const w = world({ broker: o.ready ?? true, door: o.door });
      if (o.joined === true && o.place !== undefined) w.state.joined.add(o.place);
      const m = machine(o.served === true ? { served: async () => ({ ok: true }) } : {});
      const entry = entryOf(rec({ kind: o.kind ?? "cloud", ...(o.place !== undefined ? { place: o.place } : {}) }), m);
      // brokered() is not exposed: it shows as whether a git op's frame carries a token minted for it.
      w.ctx.threadRecords.set("th_new", { workspaceId: "ws1" });
      vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] });
      const d = scripted(() => ({ ok: true }));
      w.state.dials.push(d.channel);
      await w.area.withDaemon(entry as never, async ask => {
        w.state.listeners[0]?.({ type: "daemon.hello", version: PIPE_DAEMON_VERSION });
        return ask({ op: "git.push" } as never);
      });
      vi.useRealTimers();
      cases.push({ name, ready: o.ready ?? true, kind: o.kind ?? "cloud", served: o.served === true, ...(o.place !== undefined ? { place: o.place } : {}), ...(o.joined !== undefined ? { joined: o.joined } : {}), ...(o.door !== undefined ? { door: o.door } : {}), expected: w.state.minted.length === 1 });
    };
    await add("a cloud fork, the broker ready", {});
    await add("the broker not ready", { ready: false });
    await add("a local folder", { kind: "local" });
    await add("a folder on a joined computer", { kind: "place", place: "pl_x", joined: true });
    await add("a folder on a place that is not joined", { kind: "place", place: "pl_x" });
    await add("a cloud fork its computer serves", { served: true });
    await add("a cloud fork on a joined place", { place: "pl_x", joined: true });
    await add("a cloud fork on a provider place that is not joined", { place: "solari" });
    await add("a cloud fork naming a place on a host with no door", { place: "pl_x", door: false });
    fixture["brokered"] = cases;
  }

  // D17: the stdio road.
  {
    const link = () => scripted(f => (f["op"] === "stdio.fail" ? { ok: false, code: "bad", error: "the link refused" } : { ok: true, op: f["op"] }));
    const cases: Record<string, unknown>[] = [];
    // unserved: the machine's daemon dialled with the listeners as its events.
    {
      const w = world();
      const d = scripted(f => (f["op"] === "stdio.fail" ? { ok: false, code: "bad", error: "the daemon refused" } : { ok: true, op: f["op"] }));
      w.state.dials.push(d.channel);
      const road = await w.area.stdioRoad(entryOf(rec({})) as never);
      const heardA: Frame[] = [];
      const heardB: Frame[] = [];
      const offA = road.listen(e => void heardA.push(e));
      road.listen(e => void heardB.push(e));
      w.state.listeners[0]!({ type: "stdio.data", n: 1 });
      offA();
      w.state.listeners[0]!({ type: "stdio.data", n: 2 });
      const sent = await trying(() => road.send({ op: "stdio.open" } as never));
      const refused = await trying(() => road.send({ op: "stdio.fail" } as never));
      road.close();
      cases.push({ name: "unserved", sent, refused, heardA, heardB, dialled: w.state.dialled, heard: d.heard, closes: d.closes(), closedSettles: await Promise.race([road.closed.then(() => true), tick().then(() => false)]) });
    }
    const served = async (o: { place?: string; door?: boolean; report?: unknown; link?: boolean; need?: number; name?: string }) => {
      const w = world({ door: o.door });
      w.state.names = { pl_x: "Hetzner box" };
      if (o.report !== undefined) w.state.reports = { pl_x: o.report };
      const l = link();
      if (o.link !== false) w.state.links = { pl_x: l.channel };
      const frames: Frame[] = [];
      const m = machine({ served: async f => (frames.push(f), f["op"] === "stdio.fail" ? { ok: false, code: "bad", error: "the computer refused" } : { ok: true, op: f["op"] }) });
      const entry = entryOf(rec({ kind: "place", ...(o.place !== undefined ? { place: o.place } : {}) }), m);
      const road = await trying(async () => {
        const r = await w.area.stdioRoad(entry as never, o.need);
        const heard: Frame[] = [];
        r.listen(e => void heard.push(e));
        w.state.linkListeners[0]!.fn({ type: "stdio.data", n: 1 });
        const sent = await trying(() => r.send({ op: "stdio.open" } as never));
        const refused = await trying(() => r.send({ op: "stdio.fail" } as never));
        r.close();
        return { sent, refused, heard, frames, linkCloses: l.closes(), linkHeard: l.heard };
      });
      return { name: o.name ?? "", place: o.place ?? null, door: o.door ?? true, report: o.report ?? null, link: o.link ?? true, need: o.need ?? null, road, reportsAsked: w.state.reportsAsked };
    };
    const report = (v: number) => ({ daemonVersion: v });
    cases.push(await served({ name: "a served machine naming no place", place: undefined }));
    cases.push(await served({ name: "a served machine and no door", place: "pl_x", door: false }));
    cases.push(await served({ name: "a daemon one under the default need", place: "pl_x", report: report(STDIO_DAEMON_VERSION - 1) }));
    cases.push(await served({ name: "a daemon at the default need", place: "pl_x", report: report(STDIO_DAEMON_VERSION) }));
    cases.push(await served({ name: "a daemon that reports nothing passes", place: "pl_x" }));
    cases.push(await served({ name: "a daemon one under the http need", place: "pl_x", report: report(HTTP_POST_DAEMON_VERSION - 1), need: HTTP_POST_DAEMON_VERSION }));
    cases.push(await served({ name: "a daemon at the http need", place: "pl_x", report: report(HTTP_POST_DAEMON_VERSION), need: HTTP_POST_DAEMON_VERSION }));
    cases.push(await served({ name: "no link stands", place: "pl_x", report: report(STDIO_DAEMON_VERSION), link: false }));
    fixture["stdio"] = {
      cases,
      sentences: {
        serves: placeServesDaemonLine("alpha", "Hetzner box"),
        servesPlaceId: placeServesDaemonLine("alpha", "pl_x"),
        behind: slateServerBehindLine("Hetzner box"),
        absent: absentComputer("Hetzner box", null).sentence,
      },
    };
  }

  // D18: this computer's own daemon.
  {
    const noDaemon = world({ localDaemon: false });
    const none = await trying(() => noDaemon.area.onThisComputer(async () => "never"));
    const w = world({ localDaemon: true });
    const d = scripted(f => ({ ok: true, op: f["op"] }));
    w.state.dials.push(d.channel);
    const done = await trying(() => w.area.onThisComputer(async (ask, home) => ({ home, reply: await ask({ op: "gh.pr" } as never) })));
    const refusedWorld = world({ localDaemon: true });
    const rd = scripted(() => ({ ok: false, code: "no-host-cli", error: "no gh on this computer" }));
    refusedWorld.state.dials.push(rd.channel);
    const refused = await trying(() => refusedWorld.area.onThisComputer(async ask => ask({ op: "gh.pr" } as never)));
    const tokenless = world({ localDaemon: true });
    tokenless.state.localRoad = { url: "wss://here", expiresAt: 1 };
    const noToken = await trying(() => tokenless.area.onThisComputer(async () => "never"));
    fixture["on_this_computer"] = {
      none,
      done: { result: done, dialled: w.state.dialled, heard: d.heard, closes: d.closes() },
      refused: { result: refused, closes: rd.closes() },
      noToken: { result: noToken, dialled: tokenless.state.dialled },
    };
  }

  // D19, D20: the guards.
  {
    const place = (o: Record<string, unknown>) => ({ id: "solari", kind: "provider", name: "Solari", default: false, cap: { machines: 3, spendPerDayUsd: 0.2 }, ...o });
    const rows = [
      place({}),
      place({ id: "pl_nocap", name: "NoCap", cap: undefined }),
      { id: "here", kind: "computer", name: "This computer", default: true, cap: { threads: 4 } },
      place({ id: "pl_free", name: "Free", cap: { machines: 3, spendPerDayUsd: 5 } }),
    ].map(r => json(r));
    const guard = async (name: string, id: string, o: { door?: boolean; spend?: { place: string; todayUsd?: number }[]; rows?: unknown[] }) => {
      const w = world({ door: o.door });
      w.state.rows = o.rows ?? rows;
      w.state.spend = o.spend ?? [];
      vi.useFakeTimers({ toFake: ["Date"] });
      vi.setSystemTime(NOW);
      const result = await trying(() => w.area.placeGuard(id));
      vi.useRealTimers();
      return { name, place: id, door: o.door ?? true, spend: o.spend ?? [], result, spendAsked: w.state.spendAsked };
    };
    fixture["guard"] = {
      now: NOW,
      rows,
      cases: [
        await guard("a cloud at its day's spend", "solari", { spend: [{ place: "solari", todayUsd: 0.22 }] }),
        await guard("a cloud exactly at the limit", "solari", { spend: [{ place: "solari", todayUsd: 0.2 }] }),
        await guard("a cloud under the limit", "solari", { spend: [{ place: "solari", todayUsd: 0.1 }] }),
        await guard("a cloud with no figure today", "solari", { spend: [{ place: "solari" }] }),
        await guard("a cloud the spend list does not name", "solari", { spend: [{ place: "other", todayUsd: 9 }] }),
        await guard("a place with no row", "pl_gone", { spend: [{ place: "pl_gone", todayUsd: 9 }] }),
        await guard("a row with no cap", "pl_nocap", { spend: [{ place: "pl_nocap", todayUsd: 9 }] }),
        await guard("a computer, which has no spend limit", "here", { spend: [{ place: "here", todayUsd: 9 }] }),
        await guard("a cloud far under a high limit", "pl_free", { spend: [{ place: "pl_free", todayUsd: 4.99 }] }),
        await guard("no door", "solari", { door: false, spend: [{ place: "solari", todayUsd: 9 }] }),
      ],
      sentence: spendCapRefusal("Solari", 0.22, 0.2),
    };

    const report = (o: Record<string, unknown>) => ({ name: "Hetzner box", platform: "linux", arch: "x64", os: "debian", shape: { cpu: 4, memMb: 8192 }, login: {}, runsWorkspaces: true, engine: "none", daemonVersion: 170, wsp: [], agents: [], dialed: "ssh", ...o });
    const blocked = report({ runsWorkspaces: false, workspacesBlocked: "this computer's disk is full" });
    const blockedNoWhy = report({ runsWorkspaces: false });
    const refuses = async (name: string, placeId: string | undefined, o: { door?: boolean; reports?: Record<string, unknown> }) => {
      const w = world({ door: o.door });
      w.state.names = { pl_x: "Hetzner box" };
      w.state.reports = o.reports ?? {};
      return { name, place: placeId ?? null, door: o.door ?? true, reports: o.reports ?? {}, result: await trying(() => w.area.placeRefuses(placeId)), reportsAsked: w.state.reportsAsked };
    };
    const blockedWorld = { pl_x: blocked };
    const copy = async (name: string, kind: string, place: string | undefined, reports: Record<string, unknown>) => {
      const w = world();
      w.state.names = { pl_x: "Hetzner box" };
      w.state.reports = reports;
      const entry = entryOf(rec({ kind, ...(place !== undefined ? { place } : {}) }));
      return { name, kind, place: place ?? null, reports, result: await trying(() => w.area.copyBlocked(entry as never)), reportsAsked: w.state.reportsAsked };
    };
    fixture["refuses"] = {
      cases: [
        await refuses("a blocked computer, with the reason", "pl_x", { reports: blockedWorld }),
        await refuses("a blocked computer, with no reason", "pl_x", { reports: { pl_x: blockedNoWhy } }),
        await refuses("a computer that runs workspaces", "pl_x", { reports: { pl_x: report({}) } }),
        await refuses("a computer with no report", "pl_x", {}),
        await refuses("no place", undefined, { reports: blockedWorld }),
        await refuses("no door", "pl_x", { door: false, reports: blockedWorld }),
      ],
      copy: [
        await copy("a folder on a blocked computer is not asked", "place", "pl_x", blockedWorld),
        await copy("a local folder is not asked", "local", undefined, blockedWorld),
        await copy("a fork on a blocked computer", "cloud", "pl_x", blockedWorld),
        await copy("a fork with no place", "cloud", undefined, blockedWorld),
        await copy("a fork on a computer that runs workspaces", "cloud", "pl_x", { pl_x: report({}) }),
      ],
      sentences: { blocked: placeBlocked("Hetzner box", blocked as never) ?? null, blockedNoWhy: placeBlocked("Hetzner box", blockedNoWhy as never) ?? null },
    };
  }

  writeFileSync(out, `${JSON.stringify(fixture, null, 1)}\n`);
});
