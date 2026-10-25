// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the channels_ptys tests compare to: channelsArea (packages/runtime/src/account/channels.ts at
// 65f6b89c7, lines 280-580) driven over a fake daemon and a fake context, each scenario's observations written as
// tests/fixtures/oracle/channels_ptys.json. Every input a Rust test feeds is written beside what the TS answered.
// Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 channels_ptys.oracle.ts
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { hostname, tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, it, vi } from "vitest";
import { APP_DATA_FOLDER, APP_LOCK_FILE } from "@wsp/protocol";
import { CATALOG_AGENTS } from "@wsp/catalog";

// The host name the lock link is compared with, fixed like the pids.
vi.mock("node:os", async orig => ({ ...(await orig<typeof import("node:os")>()), hostname: () => "oracle-host" }));

const here = (rel: string): string => fileURLToPath(new URL(rel, import.meta.url));
const { channelsArea } = await import("../../../../packages/runtime/src/account/channels.ts");
const { eventBus } = await import("../../../../packages/runtime/src/types/events.ts");
const { memoryStore } = await import("../../../../packages/runtime/src/store.ts");
const out = here("../tests/fixtures/oracle/channels_ptys.json");
const channelsSource = readFileSync(here("../../../../packages/runtime/src/account/channels.ts"), "utf8");

type Frame = Record<string, unknown>;
type Chan = { send(frame: never): Promise<never>; close(): void; closed: Promise<unknown> };
const tick = () => new Promise(r => setTimeout(r, 0));
const flush = async () => {
  for (let i = 0; i < 20; i++) await tick();
};
const json = <T>(v: T): T => (v === undefined ? (null as T) : (JSON.parse(JSON.stringify(v)) as T));
const send = async (c: Chan, frame: Frame): Promise<Frame> => json((await c.send(frame as never)) as unknown as Frame);

/** The lists the TS keeps inside channelsArea's closure, read off the source text: the one place they are written. */
function listIn(name: string): string[] {
  const m = new RegExp(`const ${name} = \\[([^\\]]*)\\];`).exec(channelsSource);
  if (m === null) throw new Error(`${name} not found`);
  return JSON.parse(`[${m[1]}]`) as string[];
}

/** Process ids this oracle says exist, so no number depends on the machine it runs on. */
const procs = new Set<number>();
const groups = new Set<number>();
// The host's own pids, fixed so the fixture does not move with the machine it was written on.
Object.defineProperty(process, "pid", { value: 4100, configurable: true });
Object.defineProperty(process, "ppid", { get: () => 4099, configurable: true });
const realKill = process.kill.bind(process);
vi.spyOn(process, "kill").mockImplementation(((pid: number, sig?: string | number) => {
  if (sig === 0) {
    if (pid < 0 ? groups.has(-pid) : procs.has(pid)) return true;
    throw Object.assign(new Error("ESRCH"), { code: "ESRCH" });
  }
  return realKill(pid, sig as never);
}) as never);
afterAll(() => vi.restoreAllMocks());

/** The one daemon every folder here dials: lists every pty it holds to whoever asks, numbers them from pty_1 up. */
function sharedDaemon() {
  const held = new Map<string, boolean>();
  const heard: Frame[] = [];
  let made = 0;
  let listGate: Promise<void> | null = null;
  const channel = (): Chan => ({
    send: (async (frame: Frame) => {
      heard.push(structuredClone(frame));
      if (frame["op"] === "pty.create") {
        const ptyId = `pty_${++made}`;
        held.set(ptyId, false);
        return { id: null, ok: true, ptyId, pid: 4000 + made };
      }
      if (frame["op"] === "pty.list") {
        const ptys = [...held].map(([id, exited], i) => ({ id, pid: 4001 + i, cols: 80, rows: 24, exited }));
        if (listGate !== null) await listGate;
        return { id: null, ok: true, ptys };
      }
      if (frame["ptyId"] !== undefined && !held.has(String(frame["ptyId"]))) return { id: null, ok: false, error: `no such pty: ${String(frame["ptyId"])}` };
      if (frame["op"] === "pty.kill") held.delete(String(frame["ptyId"]));
      return { id: null, ok: true };
    }) as never,
    close: () => {},
    closed: new Promise(() => {}),
  });
  return {
    channel,
    held,
    heard,
    restart: () => (held.clear(), (made = 0)),
    holdLists: (): (() => void) => {
      let release = (): void => {};
      listGate = new Promise<void>(resolve => (release = resolve));
      return () => {
        listGate = null;
        release();
      };
    },
  };
}

/** A daemon that answers each frame by its own function, recording what it heard; `hold` parks the next reply. */
function scripted(answer: (frame: Frame) => Frame) {
  const heard: Frame[] = [];
  let parked: (() => void) | undefined;
  let hold = false;
  let end: (v: unknown) => void = () => {};
  const closed = new Promise<unknown>(resolve => (end = resolve));
  const channel: Chan = {
    send: ((frame: Frame) => {
      heard.push(structuredClone(frame));
      const reply = answer(frame);
      if (!hold) return Promise.resolve(reply);
      hold = false;
      return new Promise(resolve => (parked = () => resolve(reply)));
    }) as never,
    close: () => end({ code: 1000, reason: "" }),
    closed,
  };
  return { channel, heard, holdNext: () => (hold = true), land: () => parked?.(), end: () => end({ code: 1006, reason: "" }) };
}

/** channelsArea over a context holding what lines 280-580 read: the store (logging every write), the bus, the state,
 * the sessions, the process the host is. */
function world(o: { statePath?: boolean; stateFolder?: string; store?: ReturnType<typeof memoryStore>; sessions?: [string, unknown][] } = {}) {
  const bus = eventBus();
  const inner = o.store ?? memoryStore();
  const ops: unknown[] = [];
  const reads: unknown[] = [];
  const warns: string[] = [];
  const control = { gate: undefined as Promise<void> | undefined, failPuts: [] as string[], failGet: undefined as string | undefined };
  const store = {
    get: async (c: string, i: string) => {
      reads.push([c, i]);
      if (control.failGet !== undefined) throw new Error(control.failGet);
      return inner.get(c, i);
    },
    put: async (c: string, i: string, v: unknown) => {
      ops.push(["put", c, i, structuredClone(v)]);
      if (control.gate !== undefined) await control.gate;
      const fail = control.failPuts.shift();
      if (fail !== undefined) throw new Error(fail);
      await inner.put(c, i, v);
    },
    delete: async (c: string, i: string) => {
      ops.push(["delete", c, i]);
      await inner.delete(c, i);
    },
    keys: (c: string) => inner.keys(c),
  };
  const moved = { count: 0 };
  const revoked: string[] = [];
  const state: { rootsRecheck?: ReturnType<typeof setInterval> } = {};
  vi.spyOn(console, "warn").mockImplementation((m: unknown) => void warns.push(String(m)));
  const ctx = {
    bus,
    store,
    state,
    sessions: new Map<string, unknown>(o.sessions ?? []),
    opts: o.statePath === false ? {} : { statePath: "/state/host.json" },
    stateFolder: () => o.stateFolder ?? "/unused",
    procScopesMoved: () => void moved.count++,
    deviceDoor: { revoke: async (id: string) => (revoked.push(id), true) },
  };
  const area = channelsArea(ctx as never);
  return { ctx, area, bus, inner, ops, reads, warns, control, moved, revoked, state, deleted: (id: string) => bus.emit({ type: "workspace.deleted", workspaceId: id } as never) };
}
type World = ReturnType<typeof world>;
const docs = async (w: World, collection: string, ids: string[]) => Object.fromEntries(await Promise.all(ids.map(async id => [id, ((await w.inner.get(collection, id)) ?? null) as unknown])));

const PTY_BY_ID: Frame[] = [{ op: "pty.attach" }, { op: "pty.write", data: "x" }, { op: "pty.resize", cols: 80, rows: 24 }, { op: "pty.tab" }, { op: "pty.detach" }, { op: "pty.kill" }];

it("writes the channels_ptys fixture", async () => {
  const fixture: Record<string, unknown> = {};

  // Y1: the lists.
  {
    const w = world();
    fixture["lists"] = {
      GUEST_EVENTS: listIn("GUEST_EVENTS"),
      PTY_EVENTS: listIn("PTY_EVENTS"),
      COMPUTER_WATCHES: listIn("COMPUTER_WATCHES"),
      COMPUTER_PROCS: listIn("COMPUTER_PROCS"),
      WORKSPACE_FRAMES: w.area.WORKSPACE_FRAMES,
      GUEST_ROAD_FRAMES: w.area.GUEST_ROAD_FRAMES,
      PORT_ROOTS_RECHECK_MS: Number(/const PORT_ROOTS_RECHECK_MS = ([\d_]+);/.exec(channelsSource)![1]!.replace(/_/g, "")),
    };
  }

  // Y2: three owners on one daemon.
  {
    const w = world();
    const d = sharedDaemon();
    const own = (owner: string) => w.area.heldToOwner(owner, d.channel() as never) as unknown as Chan;
    const a = own("ws_a");
    const b = own("ws_b");
    const hereChannel = own("pl_here");
    const listed = async (c: Chan) => ((await send(c, { id: null, op: "pty.list" })) as unknown as { ptys: { id: string }[] }).ptys.map(p => p.id);
    const created: Frame[] = [];
    for (const c of [a, b, hereChannel]) created.push(await send(c, { id: null, op: "pty.create" }));
    const lists = { a: await listed(a), b: await listed(b), here: await listed(hereChannel) };
    d.held.set("pty_1", true);
    const reloaded = await listed(own("ws_a"));
    await send(a, { id: null, op: "pty.kill", ptyId: "pty_1" });
    const afterKill = { a: await listed(a), b: await listed(b) };
    // Another owner's shells: every op by id is told there is no such pty, and none reaches the daemon.
    const aAgain = own("ws_a");
    await send(aAgain, { id: null, op: "pty.create" });
    const before = d.heard.length;
    const refused: unknown[] = [];
    for (const ptyId of ["pty_2", "pty_3"]) for (const frame of PTY_BY_ID) refused.push({ frame: { id: 7, ...frame, ptyId }, reply: await send(a, { id: 7, ...frame, ptyId }) });
    const reachedDaemon = d.heard.slice(before);
    const mine = ((await send(a, { id: null, op: "pty.list" })) as unknown as { ptys: { id: string }[] }).ptys[0]!.id;
    const answered: unknown[] = [];
    for (const frame of PTY_BY_ID) answered.push({ op: frame["op"], ok: (await send(a, { id: 7, ...frame, ptyId: mine }))["ok"] });
    // A frame naming a ptyId that is not a string is read as its string; one naming none passes.
    const odd = [await send(a, { id: 8, op: "pty.attach", ptyId: 2 }), await send(a, { id: 9, op: "ping" })];
    fixture["owners"] = { created, lists, reloaded, afterKill, refused, reachedDaemon, mine, answered, odd };
  }

  // Y2: a list answered before another window opened a shell, and what a list drops.
  {
    const w = world();
    const d = sharedDaemon();
    const own = () => w.area.heldToOwner("ws_a", d.channel() as never) as unknown as Chan;
    const ids = async (c: Chan) => ((await send(c, { id: null, op: "pty.list" })) as unknown as { ptys: { id: string }[] }).ptys.map(p => p.id);
    const one = own();
    const two = own();
    const release = d.holdLists();
    const late = ids(one);
    const shell = ((await send(two, { id: null, op: "pty.create" })) as unknown as { ptyId: string }).ptyId;
    release();
    const lateList = await late;
    const attach = await send(two, { id: 5, op: "pty.attach", ptyId: shell });
    const lists = { late: lateList, one: await ids(one) };
    // The daemon forgets one of two: a list drops that id from the owner's set, and the other stays.
    const second = ((await send(one, { id: null, op: "pty.create" })) as unknown as { ptyId: string }).ptyId;
    d.held.delete(shell);
    const dropped = { list: await ids(one), attachForgotten: await send(one, { id: 6, op: "pty.attach", ptyId: shell }), attachKept: await send(one, { id: 7, op: "pty.attach", ptyId: second }) };
    // A refused reply passes whole and changes nothing.
    const refusing = scripted(f => (f["op"] === "pty.create" ? { id: null, ok: false, error: "no room" } : f["op"] === "pty.list" ? { id: null, ok: false, error: "busy" } : { id: null, ok: true, ptyId: "pty_x" }));
    const r = w.area.heldToOwner("ws_r", refusing.channel as never) as unknown as Chan;
    const refusedCreate = await send(r, { id: 1, op: "pty.create" });
    const refusedAttach = await send(r, { id: 2, op: "pty.attach", ptyId: "pty_x" });
    const refusedList = await send(r, { id: 3, op: "pty.list" });
    fixture["list_over"] = { shell, second, attach, lists, dropped, refusedCreate, refusedAttach, refusedList, refusedHeard: refusing.heard };
  }

  // Y2: the shared daemon replaced.
  {
    const w = world();
    const d = sharedDaemon();
    const own = (owner: string) => w.area.heldToOwner(owner, d.channel() as never) as unknown as Chan;
    const ids = async (c: Chan) => ((await send(c, { id: null, op: "pty.list" })) as unknown as { ptys: { id: string }[] }).ptys.map(p => p.id);
    const a = own("ws_a");
    const b = own("ws_b");
    const first = [(await send(a, { id: null, op: "pty.create" }))["ptyId"], (await send(b, { id: null, op: "pty.create" }))["ptyId"]];
    d.restart();
    w.area.sharedDaemonReplaced();
    const bAgain = ((await send(b, { id: null, op: "pty.create" })) as unknown as { ptyId: string }).ptyId;
    const aList = await ids(own("ws_a"));
    const before = d.heard.length;
    const aAttach = await send(a, { id: 3, op: "pty.attach", ptyId: "pty_1" });
    const reached = d.heard.slice(before);
    const bList = await ids(b);
    const bAttach = await send(b, { id: 4, op: "pty.attach", ptyId: "pty_1" });
    fixture["replaced"] = { first, bAgain, aList, aAttach, reached, bList, bAttach };
  }

  // Y3: the kill guard.
  {
    const dir = mkdtempSync(join(tmpdir(), "ptys-oracle-"));
    const run1 = join(dir, "run1");
    const run2 = join(dir, "run2");
    const run3 = join(dir, "run3");
    writeFileSync(`${run1}.agent`, "5100\n");
    writeFileSync(`${run3}.agent`, "5300\n");
    procs.add(5100);
    procs.add(5300);
    const sessions: [string, unknown][] = [
      ["s1", { view: { status: "running" }, pid: 5000, run: run1 }],
      ["s2", { view: { status: "running" }, pid: 5001 }],
      ["s3", { view: { status: "completed" }, pid: 5002, run: run3 }],
      ["s4", { view: { status: "running" }, run: run3 }],
      ["s5", { view: { status: "running" }, pid: 5003, run: run2 }],
      ["s6", { view: { status: "running" }, pid: 5100, run: run1 }],
    ];
    const lock = join(dir, "state", APP_DATA_FOLDER);
    mkdirSync(lock, { recursive: true });
    symlinkSync(`${hostname()}-4999`, join(lock, APP_LOCK_FILE));
    procs.add(4999);
    const w = world({ stateFolder: join(dir, "state"), sessions });
    const inner = scripted(() => ({ id: null, ok: true }));
    const guarded = (here: boolean) => w.area.killGuarded(inner.channel as never, here) as unknown as Chan;
    const asked = [
      { id: 1, op: "proc.kill", pid: 50, protect: [], agents: [], apps: [], agentBins: [] },
      { id: 2, op: "proc.kill", pid: 51, signal: "TERM", protect: [1], mode: "tree" },
      { id: 3, op: "proc.kill", agentBins: ["x"], pid: 52, apps: [2], agents: [3] },
    ];
    for (const f of asked) await send(guarded(true), f);
    for (const f of asked) await send(guarded(false), f);
    // Another op passes whole, key order and all, and a pty frame is none of its business.
    await send(guarded(true), { id: 4, op: "pty.list", protect: [9] });
    await send(guarded(false), { id: 5, op: "ping" });
    const hereHeard = json(inner.heard);
    // The agent the run recorded is gone, then the app.
    procs.delete(5100);
    const gone = scripted(() => ({ id: null, ok: true }));
    await send(w.area.killGuarded(gone.channel as never, true) as unknown as Chan, { id: 6, op: "proc.kill", pid: 53 });
    procs.delete(4999);
    await send(w.area.killGuarded(gone.channel as never, true) as unknown as Chan, { id: 7, op: "proc.kill", pid: 54 });
    const noState = world({ statePath: false, stateFolder: join(dir, "state") });
    const bare = scripted(() => ({ id: null, ok: true }));
    await send(noState.area.killGuarded(bare.channel as never, true) as unknown as Chan, { id: 8, op: "proc.kill", pid: 55 });
    fixture["kill_guard"] = {
      hostPid: process.pid,
      hostParentPid: process.ppid,
      agentBins: CATALOG_AGENTS.map(a => a.bin),
      appPid: 4999,
      agentPids: { run1: 5100, run3: 5300 },
      runs: { run1: "run1", run2: "run2", run3: "run3" },
      sessions: sessions.map(([id, s]) => ({ id, ...(s as object), run: ((s as { run?: string }).run ?? "").replace(dir, "") === "" ? undefined : (s as { run: string }).run.replace(`${dir}/`, "") })),
      asked,
      heard: hereHeard,
      afterAgentGone: json(gone.heard),
      noState: json(bare.heard),
    };
    rmSync(dir, { recursive: true, force: true });
    procs.clear();
  }

  // Y4: the app's main process off the lock.
  {
    const dir = mkdtempSync(join(tmpdir(), "ptys-oracle-"));
    const host = hostname();
    const cases: { name: string; link: string | null; statePath: boolean; alive: number[]; expected: number[] }[] = [];
    const run = (name: string, link: string | null, o: { statePath?: boolean; alive?: number[] } = {}) => ({ name, link, statePath: o.statePath ?? true, alive: o.alive ?? [], expected: [] as number[] });
    const table = [
      run("a lock of this host's own and a pid that answers", `${host}-4999`, { alive: [4999] }),
      run("no state path", `${host}-4999`, { statePath: false, alive: [4999] }),
      run("another host's name", "other-4999", { alive: [4999] }),
      run("a pid that answers nothing", `${host}-4998`),
      run("an unreadable link", null),
      run("a link that is no host-pid", "garbage", { alive: [4999] }),
      run("a host name with a dash in it", `${host}-x-4999`, { alive: [4999] }),
      run("a name that is only a pid", "4999", { alive: [4999] }),
      run("the greedy name", `${host}-1-4999`, { alive: [4999] }),
    ];
    const reads: unknown[] = [];
    for (const c of table) {
      const folder = mkdtempSync(join(dir, "s-"));
      if (c.link !== null) {
        mkdirSync(join(folder, APP_DATA_FOLDER), { recursive: true });
        symlinkSync(c.link, join(folder, APP_DATA_FOLDER, APP_LOCK_FILE));
      }
      procs.clear();
      for (const p of c.alive) procs.add(p);
      const w = world({ stateFolder: folder, statePath: c.statePath });
      const guarded = w.area.killGuarded(scripted(() => ({ id: null, ok: true })).channel as never, true) as unknown as Chan;
      const sent = scripted(() => ({ id: null, ok: true }));
      const probe = w.area.killGuarded(sent.channel as never, true) as unknown as Chan;
      await send(probe, { id: 1, op: "proc.kill", pid: 1 });
      void guarded;
      c.expected = (sent.heard[0]!["apps"] as number[]) ?? [];
      reads.push(c.link);
    }
    procs.clear();
    fixture["app_main"] = { host, lockPath: [APP_DATA_FOLDER, APP_LOCK_FILE], cases: table };
    rmSync(dir, { recursive: true, force: true });
  }

  // Y5: a create tags its thread; a kill drops it; a list names and drops.
  {
    const w = world();
    const d = sharedDaemon();
    const pane = w.area.threadTagged("ws_a", d.channel() as never) as unknown as Chan;
    const replies: Frame[] = [];
    replies.push(await send(pane, { id: 1, op: "pty.create", thread: "th_a" }));
    replies.push(await send(pane, { id: 2, op: "pty.create", thread: "th_b" }));
    replies.push(await send(pane, { id: 3, op: "pty.create", thread: 7 }));
    replies.push(await send(pane, { id: 4, op: "pty.create" }));
    replies.push(await send(pane, { id: 5, op: "pty.create", thread: "" }));
    await flush();
    const afterCreates = { ops: json(w.ops), doc: await docs(w, "pty-threads", ["ws_a"]) };
    const list = await send(pane, { id: 6, op: "pty.list" });
    const opsBeforeKill = w.ops.length;
    const killUntagged = await send(pane, { id: 7, op: "pty.kill", ptyId: "pty_4" });
    await flush();
    const writesByUntaggedKill = w.ops.length - opsBeforeKill;
    const kill = await send(pane, { id: 8, op: "pty.kill", ptyId: "pty_1" });
    await flush();
    const refusedKill = await send(pane, { id: 9, op: "pty.kill", ptyId: "pty_1" });
    await flush();
    const afterKill = { ops: json(w.ops.slice(opsBeforeKill)), doc: await docs(w, "pty-threads", ["ws_a"]), list: await send(pane, { id: 10, op: "pty.list" }) };
    fixture["create_tags"] = { replies, heardCreates: d.heard.filter(f => f["op"] === "pty.create"), afterCreates, list, killUntagged, writesByUntaggedKill, kill, refusedKill, afterKill, warns: w.warns };
  }

  // Y5: pty.thread is answered here and never by the daemon.
  {
    const w = world();
    const d = sharedDaemon();
    const mk = (id: string) => w.area.heldToOwner(id, w.area.threadTagged(id, d.channel() as never) as never) as unknown as Chan;
    const pane = mk("ws_a");
    const page = ((await send(pane, { id: null, op: "pty.create", thread: "" })) as unknown as { ptyId: string }).ptyId;
    const theirs = ((await send(mk("ws_b"), { id: null, op: "pty.create" })) as unknown as { ptyId: string }).ptyId;
    await flush();
    const movedBefore = w.moved.count;
    const opsBefore = w.ops.length;
    const handed = await send(pane, { id: 3, op: "pty.thread", ptyId: page, thread: "th_new" });
    await flush();
    const movedByHandover = w.moved.count - movedBefore;
    const writes = json(w.ops.slice(opsBefore));
    const notOwned = await send(pane, { id: 4, op: "pty.thread", ptyId: theirs, thread: "th_new" });
    const noThread = await send(pane, { id: 5, op: "pty.thread", ptyId: page });
    const noPty = await send(pane, { id: 6, op: "pty.thread", thread: "th_x" });
    const numberPty = await send(pane, { id: 7, op: "pty.thread", ptyId: 7, thread: "th_x" });
    const numberThread = await send(pane, { id: 8, op: "pty.thread", ptyId: page, thread: 7 });
    await flush();
    const rows = ((await send(w.area.heldToOwner("ws_a", w.area.threadTagged("ws_a", d.channel() as never) as never) as unknown as Chan, { id: 9, op: "pty.list" })) as unknown as { ptys: unknown[] }).ptys;
    // A bare tagged channel retags any id, listed or not; the next list forgets it.
    const bare = w.area.threadTagged("ws_c", d.channel() as never) as unknown as Chan;
    const ghost = await send(bare, { id: 10, op: "pty.thread", ptyId: "ghost", thread: "th_g" });
    await flush();
    const ghostDoc = await docs(w, "pty-threads", ["ws_c"]);
    const ghostList = await send(bare, { id: 11, op: "pty.list" });
    await flush();
    fixture["handover"] = {
      page, theirs, handed, movedByHandover, writes, notOwned, noThread, noPty, numberPty, numberThread, rows,
      heardThread: d.heard.filter(f => f["op"] === "pty.thread"), ghost, ghostDoc, ghostList, ghostAfter: await docs(w, "pty-threads", ["ws_c"]),
    };
  }

  // Y5: a list names and drops; the tags are read from the store once, and read badly they are none.
  {
    const w = world();
    await w.inner.put("pty-threads", "ws_a", { pty_1: "th_a", pty_2: "th_b", pty_9: "th_gone" });
    const rowsOf = (...ids: string[]) => ids.map((id, i) => ({ id, pid: 100 + i, cols: 80 }));
    const d = scripted(f => (f["op"] === "pty.list" ? { id: null, ok: true, ptys: rowsOf("pty_1", "pty_2", "pty_3") } : { id: null, ok: false, error: "no" }));
    const pane = w.area.threadTagged("ws_a", d.channel as never) as unknown as Chan;
    const first = await send(pane, { id: 1, op: "pty.list" });
    await flush();
    const afterFirst = { ops: json(w.ops), doc: await docs(w, "pty-threads", ["ws_a"]) };
    const second = await send(pane, { id: 2, op: "pty.list" });
    await flush();
    const writesAfterSecond = w.ops.length;
    const refused = await send(pane, { id: 3, op: "pty.kill", ptyId: "pty_1" });
    fixture["list_names"] = { first, afterFirst, second, writesAfterSecond, refused, reads: w.reads };

    const bad = world();
    await bad.inner.put("pty-threads", "ws_obj", { pty_1: "th_a", pty_2: 7, pty_3: null, pty_4: ["x"], pty_5: "" });
    await bad.inner.put("pty-threads", "ws_str", "th_a" as never);
    await bad.inner.put("pty-threads", "ws_null", null as never);
    const lister = () => scripted(f => (f["op"] === "pty.list" ? { id: null, ok: true, ptys: rowsOf("pty_1", "pty_2", "pty_3", "pty_4", "pty_5") } : { id: null, ok: true }));
    const read: Record<string, unknown> = {};
    for (const id of ["ws_obj", "ws_str", "ws_null", "ws_none"]) read[id] = await send(bad.area.threadTagged(id, lister().channel as never) as unknown as Chan, { id: 1, op: "pty.list" });
    bad.control.failGet = "disk gone";
    read["ws_failed"] = await send(bad.area.threadTagged("ws_failed", lister().channel as never) as unknown as Chan, { id: 1, op: "pty.list" });
    fixture["tags_read"] = read;
  }

  // Y6: writes run in turn, each the tags as they stand when it runs.
  {
    const w = world();
    const d = sharedDaemon();
    const pane = w.area.threadTagged("ws_a", d.channel() as never) as unknown as Chan;
    let open = (): void => {};
    w.control.gate = new Promise<void>(resolve => (open = resolve));
    await send(pane, { id: 1, op: "pty.create", thread: "th_a" });
    await flush();
    await send(pane, { id: 2, op: "pty.create", thread: "th_b" });
    await send(pane, { id: 3, op: "pty.kill", ptyId: "pty_1" });
    await flush();
    const whileHeld = { ops: json(w.ops), doc: await docs(w, "pty-threads", ["ws_a"]) };
    w.control.gate = undefined;
    open();
    await flush();
    const afterRelease = { ops: json(w.ops), doc: await docs(w, "pty-threads", ["ws_a"]) };
    // The last tag goes: the document goes with it.
    await send(pane, { id: 4, op: "pty.kill", ptyId: "pty_2" });
    await flush();
    fixture["writes_in_turn"] = { whileHeld, afterRelease, last: { ops: json(w.ops), doc: await docs(w, "pty-threads", ["ws_a"]) }, warns: w.warns };
  }

  // Y6: a failed write warns and the next one runs.
  {
    const w = world();
    const d = sharedDaemon();
    const pane = w.area.threadTagged("ws_a", d.channel() as never) as unknown as Chan;
    w.control.failPuts.push("disk full");
    await send(pane, { id: 1, op: "pty.create", thread: "th_a" });
    await flush();
    const afterFailure = { ops: json(w.ops), doc: await docs(w, "pty-threads", ["ws_a"]), warns: [...w.warns] };
    await send(pane, { id: 2, op: "pty.create", thread: "th_b" });
    await flush();
    fixture["failed_write"] = { afterFailure, ops: json(w.ops), doc: await docs(w, "pty-threads", ["ws_a"]), warns: w.warns };
  }

  // Y7: a deleted workspace's tags go with it.
  {
    const w = world();
    const ptys = (n = 0) => {
      let made = n;
      return scripted(f => (f["op"] === "pty.create" ? { id: null, ok: true, ptyId: `pty_${++made}` } : f["op"] === "pty.list" ? { id: null, ok: true, ptys: [{ id: "pty_7" }, { id: "pty_1" }] } : { id: null, ok: true }));
    };
    for (const ws of ["ws_gone", "ws_kept"]) await send(w.area.threadTagged(ws, ptys().channel as never) as unknown as Chan, { id: null, op: "pty.create", thread: "th_a" });
    await flush();
    const before = { doc: await docs(w, "pty-threads", ["ws_gone", "ws_kept"]), ops: json(w.ops) };
    w.ops.length = 0;
    w.deleted("ws_gone");
    await flush();
    const afterDelete = { doc: await docs(w, "pty-threads", ["ws_gone", "ws_kept"]), ops: json(w.ops), writes: [...w.area.tagWrites.keys()] };
    // Held no more: a document that appeared since is read afresh for the deleted one, and not for the other.
    await w.inner.put("pty-threads", "ws_gone", { pty_7: "th_z" });
    await w.inner.put("pty-threads", "ws_kept", { pty_1: "th_changed" });
    const listed = async (ws: string) => ((await send(w.area.threadTagged(ws, ptys().channel as never) as unknown as Chan, { id: null, op: "pty.list" })) as unknown as { ptys: unknown[] }).ptys;
    const reread = { ws_gone: await listed("ws_gone"), ws_kept: await listed("ws_kept") };

    // A reply that lands after the delete writes nothing.
    const late = world();
    const slow = scripted(() => ({ id: null, ok: true, ptyId: "pty_late" }));
    slow.holdNext();
    const opening = send(late.area.threadTagged("ws_gone", slow.channel as never) as unknown as Chan, { id: null, op: "pty.create", thread: "th_a" });
    await flush();
    late.deleted("ws_gone");
    await flush();
    const opsAtDelete = json(late.ops);
    slow.land();
    await opening;
    await flush();
    const afterLate = { ops: json(late.ops), doc: await docs(late, "pty-threads", ["ws_gone"]), writes: [...late.area.tagWrites.keys()] };

    // A host started again over the same store reads the tags the first kept.
    const first = world();
    const d = sharedDaemon();
    const pane = first.area.threadTagged("ws_c", d.channel() as never) as unknown as Chan;
    await send(pane, { id: null, op: "pty.create", thread: "th_a" });
    await send(pane, { id: null, op: "pty.create", thread: "th_b" });
    await send(pane, { id: null, op: "pty.thread", ptyId: "pty_2", thread: "th_c" });
    await flush();
    const second = world({ store: first.inner });
    const restarted = ((await send(second.area.threadTagged("ws_c", d.channel() as never) as unknown as Chan, { id: null, op: "pty.list" })) as unknown as { ptys: unknown[] }).ptys;
    fixture["deleted"] = { before, afterDelete, reread, late: { opsAtDelete, afterLate }, restart: { doc: await docs(first, "pty-threads", ["ws_c"]), restarted } };
  }

  // Y8: the thread each shell of a local workspace belongs to.
  {
    const w = world();
    w.area.portPids.set("ws_a", new Map([["pty_1", 4242], ["pty_2", 4343], ["pty_3", 4444], ["pty_4", 4545]]));
    w.area.portPids.set("ws_b", new Map([["pty_9", 77]]));
    await w.inner.put("pty-threads", "ws_a", { pty_1: "th_a", pty_2: "", pty_4: "th_b" });
    await w.inner.put("pty-threads", "ws_b", { pty_9: "th_x" });
    const unread = w.area.threadTerminals();
    const pane = w.area.threadTagged("ws_a", sharedDaemon().channel() as never) as unknown as Chan;
    await send(pane, { id: 1, op: "pty.thread", ptyId: "pty_3", thread: "th_c" });
    await flush();
    const afterOneRead = w.area.threadTerminals();
    await send(w.area.threadTagged("ws_b", sharedDaemon().channel() as never) as unknown as Chan, { id: 2, op: "pty.thread", ptyId: "pty_8", thread: "th_y" });
    await flush();
    fixture["terminals"] = { unread, afterOneRead, afterTwoRead: w.area.threadTerminals() };
  }

  // Y9: a deleted thread's shells are killed through the daemon, their tokens ended, their tags dropped.
  {
    const w = world();
    await w.inner.put("pty-threads", "ws_a", { pty_1: "th_gone", pty_2: "th_kept", pty_3: "th_gone", pty_4: "th_gone" });
    await w.inner.put("pty-tokens", "ws_a", { pty_1: "dev_1", pty_2: "dev_2", pty_4: "dev_4" });
    const served: Frame[] = [];
    const entry = {
      record: { id: "ws_a", name: "alpha", kind: "place", place: "pl_1", phase: "running" },
      machine: {
        id: "m1",
        daemonFrame: async (frame: Frame) => {
          served.push(structuredClone(frame));
          if (frame["ptyId"] === "pty_3") return { id: null, ok: false, error: "no such pty: pty_3" };
          if (frame["ptyId"] === "pty_4") return { ok: false, code: "busy" };
          return { id: null, ok: true };
        },
        state: async () => "running",
      },
    };
    await w.area.endThreadShells(entry as never, "th_gone");
    await flush();
    const none = await w.area.endThreadShells(entry as never, "th_nobody");
    void none;
    fixture["end_shells"] = {
      served, revoked: w.revoked, warns: w.warns,
      tags: await docs(w, "pty-threads", ["ws_a"]), tokens: await docs(w, "pty-tokens", ["ws_a"]),
    };
  }

  // Y10: the roots of a workspace, sorted and pruned; and the recheck.
  {
    const w = world();
    groups.add(3100);
    groups.add(3200);
    const watched: { ws: string; frame: Frame }[] = [];
    const opened = (ws: string) => {
      const inner = scripted(() => ({ id: null, ok: true, ports: [] }));
      const c = w.area.rootedPorts(ws, `/work/${ws}`, w.area.portPids.get(ws) ?? new Map(), inner.channel as never) as unknown as Chan;
      return { c, inner };
    };
    const rootsOf = async (ws: string) => {
      const { c, inner } = opened(ws);
      await send(c, { id: null, op: "ports.watch" });
      watched.push({ ws, frame: json(inner.heard[0]!) });
      return inner.heard[0]!["roots"] as number[];
    };
    w.area.turnGroups.set("ws_a", new Map([[3200, "th_2"], [2900, "th_3"], [3100, "th_1"]]));
    w.area.portPids.set("ws_a", new Map([["pty_1", 4242], ["pty_2", 3100], ["pty_0", 17]]));
    w.area.turnGroups.set("ws_b", new Map([[2800, "th_4"]]));
    w.area.portPids.set("ws_c", new Map([["pty_5", 5]]));
    w.area.portPids.set("ws_b", new Map());
    const roots = { ws_a: await rootsOf("ws_a"), ws_b: await rootsOf("ws_b"), ws_c: await rootsOf("ws_c"), ws_none: await rootsOf("ws_none") };
    const groupsAfter = Object.fromEntries([...w.area.turnGroups].map(([ws, g]) => [ws, Object.fromEntries(g)]));
    fixture["roots"] = { roots, groupsAfter, watched };

    // The recheck.
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const r = world();
    groups.clear();
    groups.add(3100);
    groups.add(3200);
    r.area.turnGroups.set("ws_a", new Map([[3100, "th_1"], [3200, "th_2"]]));
    r.area.turnGroups.set("ws_b", new Map([[3300, "th_3"]]));
    groups.add(3300);
    const rooted = (ws: string) => {
      const inner = scripted(() => ({ id: null, ok: true, ports: [] }));
      const c = r.area.rootedPorts(ws, `/work/${ws}`, new Map(), inner.channel as never) as unknown as Chan;
      return { c, inner };
    };
    const wa = rooted("ws_a");
    await send(wa.c, { id: null, op: "ports.watch" });
    const steps: unknown[] = [];
    const watchesOf = () => wa.inner.heard.filter(f => f["op"] === "ports.watch").map(f => f["roots"]);
    const snap = (name: string) => steps.push({ name, moved: r.moved.count, armed: r.state.rootsRecheck !== undefined, watches: json(watchesOf()), groups: Object.fromEntries([...r.area.turnGroups].map(([ws, g]) => [ws, [...g.keys()]])) });
    snap("watching");
    r.area.armRootsRecheck();
    const first = r.state.rootsRecheck;
    r.area.armRootsRecheck();
    steps.push({ name: "armed twice, one stands", same: first === r.state.rootsRecheck });
    vi.advanceTimersByTime(4_999);
    snap("4999 ms");
    vi.advanceTimersByTime(1);
    snap("5000 ms, everything still stands");
    groups.delete(3200);
    vi.advanceTimersByTime(5_000);
    snap("10000 ms, one group of ws_a gone");
    groups.delete(3100);
    groups.delete(3300);
    vi.advanceTimersByTime(5_000);
    snap("15000 ms, every group gone");
    vi.advanceTimersByTime(5_000);
    snap("20000 ms, nothing left to move");
    r.area.turnGroups.set("ws_z", new Map([[3100, "th_9"]]));
    groups.add(3100);
    r.area.armRootsRecheck();
    steps.push({ name: "armed again after it stopped", armed: r.state.rootsRecheck !== undefined });
    vi.useRealTimers();
    fixture["recheck"] = steps;
    groups.clear();
  }

  // Y11: a local workspace's channel with its ports.watch rooted.
  {
    const w = world();
    groups.add(3100);
    w.area.turnGroups.set("ws_a", new Map([[3100, "th_1"]]));
    const ptys = new Map<string, number>();
    w.area.portPids.set("ws_a", ptys);
    let refuse = false;
    const inner = scripted(f => {
      if (f["op"] === "pty.create") return refuse ? { id: null, ok: false, error: "no room", pid: 1 } : { id: null, ok: true, ptyId: `pty_${inner.heard.filter(h => h["op"] === "pty.create").length}`, pid: f["pid"] ?? 4242 };
      if (f["op"] === "pty.list") return refuse ? { id: null, ok: false, error: "busy" } : { id: null, ok: true, ptys: [{ id: "pty_1" }, { id: "pty_2", exited: true }, { id: "pty_3", exited: false }] };
      return { id: null, ok: true, ports: [] };
    });
    const c = w.area.rootedPorts("ws_a", "/work/ws_a", ptys, inner.channel as never) as unknown as Chan;
    const steps: unknown[] = [];
    const step = async (name: string, f: () => Promise<unknown> | unknown) => {
      const before = inner.heard.length;
      const movedBefore = w.moved.count;
      const reply = await f();
      await flush();
      steps.push({ name, reply: json(reply), heard: json(inner.heard.slice(before)), moved: w.moved.count - movedBefore, ptys: [...ptys] });
    };
    await step("before a watch, a move names nothing", () => w.area.portRootsMoved("ws_a"));
    await step("a watch names the roots, the folder after the frame's own keys", () => send(c, { id: 1, op: "ports.watch" }));
    await step("a move with the same roots names nothing", () => w.area.portRootsMoved("ws_a"));
    await step("a terminal with a pid joins and the roots are named again", () => send(c, { id: 2, op: "pty.create" }));
    await step("a watch carrying roots and folder of its own keeps their slots", () => send(c, { id: 3, op: "ports.watch", roots: [9], folder: "/elsewhere", extra: true }));
    await step("a second watch with nothing changed still carries the roots", () => send(c, { id: 4, op: "ports.watch" }));
    await step("a terminal with no numeric pid is no root", () => send(c, { id: 5, op: "pty.create", pid: "x" }));
    refuse = true;
    await step("a refused create joins nothing", () => send(c, { id: 6, op: "pty.create" }));
    await step("a refused list drops nothing", () => {
      ptys.set("pty_8", 4848);
      return send(c, { id: 7, op: "pty.list" });
    });
    refuse = false;
    await step("a list that holds every pty it was given standing moves nothing", () => {
      ptys.delete("pty_8");
      return send(c, { id: 8, op: "pty.list" });
    });
    await step("a pty the host holds that the daemon does not list is dropped and the roots are named again", () => {
      ptys.set("pty_8", 4848);
      w.area.portRootsMoved("ws_a");
      return send(c, { id: 9, op: "pty.list" });
    });
    await step("a pty listed as exited is dropped and the roots are named again", () => {
      ptys.set("pty_2", 4949);
      w.area.portRootsMoved("ws_a");
      return send(c, { id: 10, op: "pty.list" });
    });
    await step("another op passes whole", () => send(c, { id: 11, op: "fs.list", path: "/" }));
    // The push leaves at close, and when the channel's own end settles.
    const leaving = async (how: (inner: ReturnType<typeof scripted>, c: Chan) => void) => {
      const ch = scripted(() => ({ id: null, ok: true, ports: [] }));
      const rc = w.area.rootedPorts("ws_a", "/work/ws_a", ptys, ch.channel as never) as unknown as Chan;
      await send(rc, { id: 1, op: "ports.watch" });
      ptys.set("pty_20", 2020);
      w.area.portRootsMoved("ws_a");
      await flush();
      const whileWatching = json(ch.heard.slice(1));
      how(ch, rc);
      await flush();
      ptys.set("pty_21", 2121);
      w.area.portRootsMoved("ws_a");
      await flush();
      ptys.delete("pty_20");
      ptys.delete("pty_21");
      return { whileWatching, afterLeaving: json(ch.heard.slice(1 + whileWatching.length)) };
    };
    const closed = await leaving((_i, rc) => rc.close());
    const ended = await leaving(i => i.end());
    fixture["rooted"] = { steps, closed, ended };
    groups.clear();
  }

  // Y11: a list drops every pty not listed standing, an exited one among them, and names the roots again once.
  {
    const w = world();
    const ptys = new Map<string, number>([["pty_1", 4242], ["pty_2", 4949], ["pty_8", 4848]]);
    w.area.portPids.set("ws_a", ptys);
    const inner = scripted(f => (f["op"] === "pty.list" ? { id: null, ok: true, ptys: [{ id: "pty_1" }, { id: "pty_2", exited: true }, { id: "pty_3", exited: false }] } : { id: null, ok: true, ports: [] }));
    const c = w.area.rootedPorts("ws_a", "/work/ws_a", ptys, inner.channel as never) as unknown as Chan;
    await send(c, { id: 1, op: "ports.watch" });
    const movedBefore = w.moved.count;
    const reply = await send(c, { id: 2, op: "pty.list" });
    await flush();
    fixture["list_exited"] = { reply, heard: json(inner.heard), moved: w.moved.count - movedBefore, ptys: [...ptys] };
  }

  writeFileSync(out, `${JSON.stringify(fixture, null, 1)}\n`);
});
