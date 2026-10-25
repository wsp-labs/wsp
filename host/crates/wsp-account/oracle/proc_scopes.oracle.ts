// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the proc_scopes tests compare to: procScopesArea (packages/runtime/src/account/proc-scopes.ts at
// 65f6b89c7) over a fake context, each scenario's world written beside what the TS answered, as
// tests/fixtures/oracle/proc_scopes.json. The eight cases of test/proc-scopes.test.ts keep the literals that file
// holds; this one answers what that file leaves open: the order and the caps, a release that is not the owner's, the
// box's terminals, a refused scopes frame, and the standing channel's re-dial. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 proc_scopes.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { afterAll, it, vi } from "vitest";
import { PROC_SCOPE_PIDS_MAX, PROC_SCOPES_MAX, threadCgroup } from "@wsp/protocol";

const here = (rel: string): string => fileURLToPath(new URL(rel, import.meta.url));
const { procScopesArea } = await import("../../../../packages/runtime/src/account/proc-scopes.ts");
const out = here("../tests/fixtures/oracle/proc_scopes.json");

type Frame = Record<string, unknown>;
const tick = () => new Promise(r => setImmediate(r));
const settled = async () => {
  for (let i = 0; i < 20; i++) await tick();
};
const json = <T>(v: T): T => {
  const s = JSON.stringify(v);
  return s === undefined ? (null as T) : (JSON.parse(s) as T);
};

const groups = new Set<number>();
const realKill = process.kill.bind(process);
vi.spyOn(process, "kill").mockImplementation(((pid: number, sig?: string | number) => {
  if (sig === 0 && pid < 0) {
    if (groups.has(-pid)) return true;
    throw Object.assign(new Error("ESRCH"), { code: "ESRCH" });
  }
  return realKill(pid, sig as never);
}) as never);
afterAll(() => vi.restoreAllMocks());

interface WorldJson {
  live: { id: string; kind: string; place?: string; path: string }[];
  turnGroups: [string, [number, string][]][];
  groupsAlive: number[];
  sessions: { thread?: string; workspace: string; status: string; startedAt?: number; pid?: number }[];
  kept: [string, { workspace: string; pid?: number; warmUnclaimed?: boolean }][];
  latest: Record<string, { workspace: string; cwd?: string }>;
  threadRecords: [string, string][];
  terminals: { thread: string; workspace: string; pid: number }[];
  owned: [string, number, string][];
}

function fakeChannel(o: { failFirstSends?: number } = {}) {
  const sent: Frame[] = [];
  const closes: number[] = [];
  let failing = o.failFirstSends ?? 0;
  let end: (v: unknown) => void = () => {};
  const closed = new Promise<unknown>(resolve => (end = resolve));
  const channel = {
    send: async (frame: Frame) => {
      sent.push(structuredClone(frame));
      if (failing > 0) {
        failing--;
        throw new Error("connection lost");
      }
      return { id: frame["id"], ok: true };
    },
    close: () => void closes.push(sent.length),
    closed,
  };
  return { channel, sent, closes, end: () => end({ code: 1006, reason: "" }) };
}

function build(w: WorldJson, here?: () => unknown) {
  groups.clear();
  for (const g of w.groupsAlive) groups.add(g);
  const live = new Map(w.live.map(l => [l.id, { record: { id: l.id, kind: l.kind, ...(l.place !== undefined ? { place: l.place } : {}), path: l.path } }]));
  const ctx: Record<string, unknown> = {
    live,
    sessions: new Map(w.sessions.map((s, i) => [`s${i}`, { view: { ...(s.thread !== undefined ? { threadId: s.thread } : {}), workspaceId: s.workspace, status: s.status, ...(s.startedAt !== undefined ? { startedAt: s.startedAt } : {}) }, ...(s.pid !== undefined ? { pid: s.pid } : {}) }])),
    keptAgents: new Map(w.kept.map(([t, k]) => [t, { workspaceId: k.workspace, ...(k.pid !== undefined ? { pid: k.pid } : {}), ...(k.warmUnclaimed === true ? { warm: { claimed: false } } : {}) }])),
    turnGroups: new Map(w.turnGroups.map(([ws, gs]) => [ws, new Map(gs)])),
    threadRecords: new Map(w.threadRecords.map(([t, ws]) => [t, { workspaceId: ws }])),
    latestOn: (t: string) => (w.latest[t] === undefined ? undefined : { workspaceId: w.latest[t]!.workspace, ...(w.latest[t]!.cwd !== undefined ? { cwd: w.latest[t]!.cwd } : {}) }),
    checkoutOf: (r: { path: string }) => r.path,
    moduleOf: (kind: string) => ({ sharedDaemon: kind === "local" }),
    threadTerminals: () => w.terminals,
    ...(here !== undefined ? { localRoad: async () => ({ url: "ws://here" }), channelOver: async () => here() } : {}),
  };
  const area = procScopesArea(ctx as never);
  for (const [t, pid, kind] of w.owned) area.ownProcess(t, pid, kind as never);
  return { ctx, area };
}

const base = (o: Partial<WorldJson> = {}): WorldJson => ({
  live: [
    { id: "ws_a", kind: "local", path: "/work/ws_a" },
    { id: "ws_b", kind: "local", path: "/work/ws_b" },
    { id: "ws_c", kind: "local", path: "/work/ws_c" },
    { id: "ws_box", kind: "place", place: "pl_1", path: "/box/ws_box" },
    { id: "ws_box2", kind: "place", place: "pl_2", path: "/box2/ws_box2" },
  ],
  turnGroups: [], groupsAlive: [], sessions: [], kept: [], latest: {}, threadRecords: [], terminals: [], owned: [],
  ...o,
});

const digest = (scopes: { thread: string; agents?: number[]; groups?: number[]; pids?: unknown[] }[]) => ({
  count: scopes.length,
  first: scopes[0]?.thread ?? null,
  last: scopes.at(-1)?.thread ?? null,
  threads: scopes.map(s => s.thread),
});

it("writes the proc_scopes fixture", async () => {
  try {
    await main();
  } catch (e) {
    console.log((e as Error).stack);
    throw e;
  }
});

async function main(): Promise<void> {
  const fixture: Record<string, unknown> = {};
  fixture["constants"] = { PROC_SCOPES_MAX, PROC_SCOPE_PIDS_MAX };

  // The first TS case, with pids that depend on no machine.
  {
    const w = base({
      groupsAlive: [5000, 5001],
      sessions: [{ thread: "t_tree", workspace: "ws_a", status: "running", startedAt: 1, pid: 5000 }, { thread: "t_old", workspace: "ws_b", status: "completed", startedAt: 5 }],
      kept: [["t_kept", { workspace: "ws_a", pid: 5001 }]],
      turnGroups: [["ws_a", [[5000, "t_tree"], [4194303, "t_tree"]]]],
      latest: { t_tree: { workspace: "ws_a", cwd: "/work/ws_a/.wsp/t_tree" } },
      owned: [["t_tree", 777, "slate"], ["t_tree", 778, "tool"]],
    });
    const { area } = build(w);
    fixture["main"] = { world: w, scopes: json(area.scopesHere()) };
  }

  // The order of the sources, the workspace a draft names, the newest thread of each folder.
  {
    const w = base({
      live: [...base().live, { id: "ws_d", kind: "local", path: "/work/ws_d" }],
      groupsAlive: [5000],
      turnGroups: [["ws_a", [[5000, "th_1"], [5001, "th_dead"]]]],
      sessions: [
        { thread: "th_2", workspace: "ws_a", status: "running", startedAt: 5, pid: 6001 },
        { thread: "th_3", workspace: "ws_b", status: "completed", startedAt: 3, pid: 6002 },
        { thread: "th_4", workspace: "ws_a", status: "running", startedAt: 4 },
        { workspace: "ws_a", status: "running", startedAt: 4, pid: 6003 },
        { thread: "th_11", workspace: "ws_a", status: "completed", startedAt: 9 },
        { thread: "th_12", workspace: "ws_b", status: "completed", startedAt: 3 },
        { thread: "th_13", workspace: "ws_c", status: "completed" },
        { thread: "th_14", workspace: "ws_x", status: "running", startedAt: 2, pid: 6004 },
        { thread: "th_15", workspace: "ws_box", status: "completed", startedAt: 8, pid: 6005 },
        { thread: "th_16", workspace: "ws_d", status: "running", startedAt: 1, pid: 6001 },
      ],
      kept: [["th_5", { workspace: "ws_a", pid: 7001 }], ["th_6", { workspace: "ws_b", warmUnclaimed: true }], ["th_7", { workspace: "ws_b" }], ["th_1", { workspace: "ws_b", pid: 7002, warmUnclaimed: true }]],
      latest: { th_8: { workspace: "ws_b" }, th_9: { workspace: "ws_a", cwd: "/elsewhere/th_9" }, th_5: { workspace: "ws_a", cwd: "/work/ws_a/.wsp/th_5" } },
      threadRecords: [["th_8", "ws_d"], ["th_17", "ws_c"]],
      terminals: [{ thread: "th_9", workspace: "ws_a", pid: 4242 }, { thread: "th_10", workspace: "ws_box", pid: 77 }, { thread: "th_9", workspace: "ws_a", pid: 4243 }, { thread: "th_18", workspace: "ws_gone", pid: 4444 }],
      owned: [["th_1", 801, "slate"], ["th_8", 802, "tool"], ["th_17", 803, "slate"], ["th_19", 804, "tool"], ["th_1", 801, "tool"]],
    });
    const { area } = build(w);
    fixture["order"] = { world: w, scopes: json(area.scopesHere()) };
  }

  // The caps: 256 scopes, 64 of each pid list; live threads before a folder's newest thread.
  {
    const heavy = (n: number, from: number) => Array.from({ length: n }, (_, i) => from + i);
    const w = base({
      live: [{ id: "ws_a", kind: "local", path: "/work/ws_a" }],
      groupsAlive: heavy(70, 9000),
      turnGroups: [["ws_a", heavy(70, 9000).map(g => [g, "th_heavy"] as [number, string])]],
      sessions: heavy(70, 1000).map(pid => ({ thread: "th_heavy", workspace: "ws_a", status: "running", startedAt: 1, pid })),
      owned: heavy(70, 3000).map(pid => ["th_heavy", pid, "tool"] as [string, number, string]),
    });
    const { area } = build(w);
    const [only] = area.scopesHere() as unknown as { thread: string; agents: number[]; groups: number[]; pids: { pid: number; kind: string }[] }[];
    fixture["pid_caps"] = { agents: { length: only!.agents.length, first: only!.agents[0], last: only!.agents.at(-1) }, groups: { length: only!.groups.length, first: only!.groups[0], last: only!.groups.at(-1) }, pids: { length: only!.pids.length, first: only!.pids[0], last: only!.pids.at(-1) } };

    // 300 live threads, then 255 live and three folders' newest threads.
    const live300 = base({
      live: [{ id: "ws_a", kind: "local", path: "/work/ws_a" }],
      sessions: Array.from({ length: 300 }, (_, i) => ({ thread: `th_${String(i).padStart(3, "0")}`, workspace: "ws_a", status: "running", startedAt: i, pid: 20_000 + i })),
    });
    const l = digest(build(live300).area.scopesHere() as never);
    fixture["scope_cap_live"] = { count: l.count, first: l.first, last: l.last };
    const mixed = base({
      live: [{ id: "ws_a", kind: "local", path: "/work/ws_a" }, { id: "ws_n1", kind: "local", path: "/work/n1" }, { id: "ws_n2", kind: "local", path: "/work/n2" }, { id: "ws_n3", kind: "local", path: "/work/n3" }],
      sessions: [
        { thread: "n1", workspace: "ws_n1", status: "completed", startedAt: 1 },
        { thread: "n2", workspace: "ws_n2", status: "completed", startedAt: 2 },
        ...Array.from({ length: 255 }, (_, i) => ({ thread: `th_${String(i).padStart(3, "0")}`, workspace: "ws_a", status: "running", startedAt: i, pid: 20_000 + i })),
        { thread: "n3", workspace: "ws_n3", status: "completed", startedAt: 3 },
      ],
    });
    const m = digest(build(mixed).area.scopesHere() as never);
    fixture["scope_cap_mixed"] = { ...m, threads: undefined, tail: m.threads.slice(-3), head: m.threads.slice(0, 2) };
    const room = base({
      live: [{ id: "ws_a", kind: "local", path: "/work/ws_a" }, { id: "ws_n1", kind: "local", path: "/work/n1" }, { id: "ws_n2", kind: "local", path: "/work/n2" }],
      sessions: [
        { thread: "n1", workspace: "ws_n1", status: "completed", startedAt: 1 },
        { thread: "n2", workspace: "ws_n2", status: "completed", startedAt: 2 },
        ...Array.from({ length: 254 }, (_, i) => ({ thread: `th_${String(i).padStart(3, "0")}`, workspace: "ws_a", status: "running", startedAt: i, pid: 20_000 + i })),
      ],
    });
    const r = digest(build(room).area.scopesHere() as never);
    fixture["scope_cap_room"] = { count: r.count, tail: r.threads.slice(-3) };
  }

  // A thread in a folder with no latest row and no held workspace; the folder rule.
  {
    const w = base({
      sessions: [{ thread: "th_x", workspace: "ws_a", status: "running", startedAt: 1, pid: 600 }, { thread: "th_y", workspace: "ws_gone", status: "running", startedAt: 1, pid: 601 }],
      latest: { th_x: { workspace: "ws_a" }, th_y: { workspace: "ws_gone", cwd: "/gone/th_y" } },
    });
    fixture["folders"] = { world: w, scopes: json(build(w).area.scopesHere()) };
  }

  // A warm agent until a send claims it.
  {
    const w = base({ kept: [["t_warm", { workspace: "ws_a", warmUnclaimed: true }]] });
    const { ctx, area } = build(w);
    const named = json((area.scopesHere() as { thread: string }[]).find(s => s.thread === "t_warm"));
    (ctx["keptAgents"] as Map<string, { warm?: { claimed: boolean } }>).get("t_warm")!.warm!.claimed = true;
    fixture["warm"] = { world: w, named, claimed: json((area.scopesHere() as { thread: string }[]).find(s => s.thread === "t_warm")) };
  }

  // ownProcess: a release takes only its own pid.
  {
    const { area } = build(base());
    const { channel, sent } = fakeChannel();
    const scoped = area.scopedProcs(area.scopesHere, channel as never) as unknown as { send(f: Frame): Promise<unknown> };
    await scoped.send({ id: 1, op: "proc.watch" });
    const log: unknown[] = [];
    const step = async (name: string, f: () => void) => {
      const before = sent.length;
      f();
      await settled();
      log.push({ name, scopes: json(sent.slice(before).map(s => s["scopes"])), now: json(area.scopesHere()) });
    };
    let first: () => void = () => {};
    let second: () => void = () => {};
    await step("t_x owns 900", () => (first = area.ownProcess("t_x", 900, "slate")));
    await step("t_y is given the same pid", () => (second = area.ownProcess("t_y", 900, "tool")));
    await step("the first release finds the pid another thread's and leaves it", () => first());
    await step("the second release takes it away", () => second());
    await step("the second release again does nothing", () => second());
    await step("a pid owned again by the first thread", () => (first = area.ownProcess("t_x", 901, "slate")));
    await step("its release", () => first());
    await step("its release again", () => first());
    fixture["release"] = log;
  }

  // scopesOn: a computer's threads by session rows, thread records, then terminals.
  {
    const w = base({
      sessions: [
        { thread: "t_new", workspace: "ws_box", status: "running", startedAt: 9 },
        { thread: "t_new", workspace: "ws_box", status: "completed", startedAt: 5 },
        { thread: "t_old", workspace: "ws_box", status: "completed", startedAt: 2 },
        { thread: "t_here", workspace: "ws_a", status: "running", startedAt: 10 },
        { thread: "t_other", workspace: "ws_box2", status: "running", startedAt: 6 },
        { thread: "t_moved", workspace: "ws_box", status: "completed", startedAt: 3 },
        { thread: "t_moved", workspace: "ws_box2", status: "completed", startedAt: 4 },
        { workspace: "ws_box", status: "running", startedAt: 99 },
      ],
      latest: { t_old: { workspace: "ws_box", cwd: "/box/ws_box/.wsp/t_old" } },
      threadRecords: [["t_quiet", "ws_box"], ["t_old", "ws_box"], ["t_quiet2", "ws_box2"], ["t_lost", "ws_gone"]],
      terminals: [
        { thread: "t_new", workspace: "ws_box", pid: 77 },
        { thread: "t_term", workspace: "ws_box", pid: 78 },
        { thread: "t_new", workspace: "ws_box", pid: 79 },
        { thread: "t_local", workspace: "ws_a", pid: 80 },
        { thread: "t_other", workspace: "ws_box2", pid: 81 },
        ...Array.from({ length: 70 }, (_, i) => ({ thread: "t_many", workspace: "ws_box", pid: 1000 + i })),
      ],
    });
    const { area } = build(w);
    const on = (p: string) => json(area.scopesOn(p));
    fixture["scopes_on"] = { world: w, pl_1: on("pl_1"), pl_2: on("pl_2"), pl_none: on("pl_none"), cgroup: { t_new: threadCgroup("t_new") } };
  }

  // scopedProcs.
  {
    const { area } = build(base());
    const { channel, sent, closes, end } = fakeChannel({ failFirstSends: 1 });
    const scoped = area.scopedProcs(area.scopesHere, channel as never) as unknown as { send(f: Frame): Promise<unknown>; close(): void };
    const log: unknown[] = [];
    const step = async (name: string, f: () => unknown) => {
      const before = sent.length;
      let reply: unknown;
      let thrown: unknown;
      try {
        reply = await f();
      } catch (e) {
        thrown = (e as Error).message;
      }
      await settled();
      log.push({ name, reply: json(reply), thrown: thrown ?? null, sent: json(sent.slice(before)) });
    };
    area.ownProcess("t_x", 900, "slate");
    await step("a watch is told the scopes first, a refused scopes frame ignored", () => scoped.send({ id: 1, op: "proc.watch", cgroups: ["a"] }));
    await step("a second watch is told them again though nothing moved", () => scoped.send({ id: 2, op: "proc.watch" }));
    await step("a move names them again", () => area.ownProcess("t_x", 901, "tool"));
    await step("a move that changes nothing names nothing", () => area.procScopesMoved());
    await step("an unwatch stops the pushes", () => scoped.send({ id: 3, op: "proc.unwatch" }));
    await step("a move while not watching names nothing", () => area.ownProcess("t_x", 902, "tool"));
    await step("another op passes whole", () => scoped.send({ id: 4, op: "pty.list" }));
    await step("a watch after an unwatch names them", () => scoped.send({ id: 5, op: "proc.watch" }));
    await step("close takes the push away", () => scoped.close());
    await step("a move after close names nothing", () => area.ownProcess("t_x", 903, "tool"));
    const ended = fakeChannel();
    const s2 = area.scopedProcs(area.scopesHere, ended.channel as never) as unknown as { send(f: Frame): Promise<unknown> };
    await s2.send({ id: 1, op: "proc.watch" });
    const watched = ended.sent.length;
    ended.end();
    await settled();
    area.ownProcess("t_x", 904, "tool");
    await settled();
    fixture["scoped"] = { log, closes: json(closes), afterEnd: ended.sent.length - watched };
    void end;
  }

  // The standing channel here.
  {
    const dialled: ReturnType<typeof fakeChannel>[] = [];
    let failNext = 1;
    const { area } = build(base(), () => {
      const c = fakeChannel();
      dialled.push(c);
      if (failNext > 0) {
        failNext--;
        throw new Error("no daemon here");
      }
      return c.channel;
    });
    const log: unknown[] = [];
    const step = async (name: string, f: () => unknown) => {
      f();
      await settled();
      log.push({ name, opened: dialled.length, sent: dialled.map(d => json(d.sent)), closes: dialled.map(d => d.closes) });
    };
    await step("a move with no root dials nothing", () => area.procScopesMoved());
    await step("the first dial fails and is dropped", () => (void area.ownProcess("t_x", 900, "slate")));
    let two: () => void = () => {};
    await step("the next move dials again", () => (two = area.ownProcess("t_x", 901, "tool")));
    await step("a move that changes nothing sends nothing", () => area.procScopesMoved());
    dialled[1]!.end();
    await settled();
    await step("a channel that closed lets the next move dial again", () => void area.ownProcess("t_x", 902, "tool"));
    await step("a root given up is told, the others stand", () => two());
    fixture["standing"] = log;
  }

  // A host with no daemon road here, and the standing channel closing when no root is left.
  {
    const w = base();
    const { area } = build(w);
    area.ownProcess("t_x", 900, "slate");
    await settled();
    fixture["no_daemon"] = { scopes: json(area.scopesHere()) };
  }

  writeFileSync(out, `${JSON.stringify(fixture, null, 1)}\n`);
}
