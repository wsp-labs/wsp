// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the channels_copy tests compare to: channelsArea (packages/runtime/src/account/channels.ts at
// 65f6b89c7) run on a fake context over scripted worlds. Each scenario is a world, a script of steps and the trace the
// TS made of it: what each step answered and, in order, every effect the area had (frames the daemon got, events the
// client heard, store writes, revokes, warnings, execs). Written as tests/fixtures/oracle/channels_copy.json with
// the worlds and scripts beside the traces; the Rust fake builds the same context from each world and runs the same
// script. Run from the repository root, one oracle at a time:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 channels_copy.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";
import { remoteHost } from "@wsp/catalog";
import { INLINE_EXEC_MS } from "@wsp/engine";
import { THREAD_ENV, ownerRepoOf } from "@wsp/protocol";

const hoisted = vi.hoisted(() => ({ fx: null as unknown[] | null, origins: {} as Record<string, string | null> }));

vi.mock("node:child_process", async orig => ({
  ...(await orig<typeof import("node:child_process")>()),
  spawnSync: (cmd: string, args: string[], opts: { timeout?: number }) => {
    const want = JSON.stringify(["git", "-C", args[1], "remote", "get-url", "origin"]);
    if (JSON.stringify([cmd, ...args]) !== want) throw new Error(`unexpected spawnSync ${cmd} ${args.join(" ")}`);
    hoisted.fx?.push(["origin-read", args[1], opts.timeout]);
    const remote = hoisted.origins[args[1]!];
    return remote === null || remote === undefined ? { status: 128, stdout: "", stderr: "" } : { status: 0, stdout: remote, stderr: "" };
  },
}));
vi.mock("../../../../packages/runtime/src/machines/brokered.js", async orig => ({
  ...(await orig<Record<string, unknown>>()),
  brokeredEnv: (token: string) => ({ HOST_TOKEN: token, GH_TOKEN: token }),
  terminalLine: (harness: string, program: string) => `write-terminal ${harness} ${program}`,
  terminalSignIn: (harness: string, _launch: unknown, token: string) =>
    harness === "claude"
      ? { program: "claude-bin", env: { SIGN_IN: `claude:${token}` } }
      : harness === "codex"
        ? { program: "codex-bin", env: { SIGN_IN: `codex:${token}`, CODEX_EXTRA: "1" } }
        : undefined,
}));
vi.mock("../../../../packages/runtime/src/projects/branch-of-record.js", async orig => ({
  ...(await orig<Record<string, unknown>>()),
  publicMarketplaces: () => [],
}));

const { channelsArea } = await import("../../../../packages/runtime/src/account/channels.ts");
const { TERMINAL_ENV } = await import("../../../../packages/runtime/src/machines/brokered.ts");
const out = fileURLToPath(new URL("../tests/fixtures/oracle/channels_copy.json", import.meta.url));

type Frame = Record<string, unknown>;
type Step = Record<string, unknown>;
type World = {
  record: Frame;
  projects: Frame[];
  machine?: { served?: boolean; exec?: unknown };
  door?: { names?: Record<string, string>; reports?: Record<string, Frame | null>; link?: boolean } | null;
  broker?: boolean;
  threads?: { id: string; workspaceId: string; harness?: string }[];
  latest?: Record<string, string>;
  threadFolder?: string;
  threadEnv?: Record<string, string>;
  live?: string[];
  deleting?: boolean;
  store?: Record<string, Record<string, unknown>>;
  daemon?: { replies?: Record<string, Frame | Frame[]>; throws?: Record<string, string> };
  revokeFails?: boolean;
  putFails?: boolean;
  persistFails?: boolean;
  deleteFails?: boolean;
  scoped?: boolean;
  carries?: string[];
};

/** A place report as a computer sends it, with only what the world varies. */
const report = (over: Frame = {}): Frame => ({
  name: "studio",
  platform: "linux",
  arch: "x64",
  os: "Ubuntu 24.04",
  shape: { cpu: 4, memMb: 4096 },
  login: { HOME: "/root", USER: "root", PATH: "/usr/bin" },
  runsWorkspaces: true,
  engine: "none",
  daemonVersion: 200,
  agents: [],
  wsp: ["/usr/local/bin/wsp"],
  dialed: "http://192.168.1.20:14621",
  ...over,
});

const errOf = (e: unknown): unknown => {
  const err = e as Error & { fix?: string; kind?: string };
  return { message: err.message, ...(err.fix !== undefined ? { fix: err.fix } : {}), ...(err.kind !== undefined ? { kind: err.kind } : {}) };
};
const settle = async (): Promise<void> => {
  for (let i = 0; i < 80; i++) await Promise.resolve();
};

/** Closing a channel closes the link, releases the readings hold and the like in no order a port must keep, so each
 * run of those entries is put in name order. Every other effect keeps the order it happened in. */
const LOOSE = ["link-closed", "own-closed", "released"];
const canon = (fx: unknown[]): unknown[] => {
  const out: unknown[] = [];
  let run: unknown[] = [];
  const flush = (): void => {
    out.push(...run.sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))));
    run = [];
  };
  for (const e of fx) {
    if (LOOSE.includes(String((e as unknown[])[0]))) run.push(e);
    else (flush(), out.push(e));
  }
  flush();
  return out;
};

async function run(w: World, script: Step[]): Promise<{ steps: unknown[]; fx: unknown[] }> {
  const fx: unknown[] = [];
  const steps: unknown[] = [];
  hoisted.fx = fx;
  hoisted.origins = {};
  const warn = vi.spyOn(console, "warn").mockImplementation((line: unknown) => void fx.push(["warn", String(line)]));

  // Gates: a named wait a step holds and releases.
  const gates = new Map<string, { open: boolean; waiters: (() => void)[] }>();
  const waitGate = async (name: string): Promise<void> => {
    const g = gates.get(name);
    if (g === undefined || g.open) return;
    await new Promise<void>(resolve => g.waiters.push(resolve));
  };

  // The daemon: its ptys, its scripted replies and what it was sent.
  const ptys: string[] = [];
  let made = 0;
  const replies = new Map<string, Frame[]>(Object.entries(w.daemon?.replies ?? {}).map(([op, r]) => [op, Array.isArray(r) ? [...r] : [r]]));
  const throws = new Map<string, string>(Object.entries(w.daemon?.throws ?? {}));
  const daemonSend = async (frame: Frame): Promise<Frame> => {
    fx.push(["frame", structuredClone(frame)]);
    const op = String(frame["op"]);
    await waitGate(op);
    const thrown = throws.get(op);
    if (thrown !== undefined) throw new Error(thrown);
    const queue = replies.get(op);
    if (queue !== undefined) return structuredClone(queue.length > 1 ? queue.shift()! : queue[0]!);
    switch (op) {
      case "pty.create": {
        const ptyId = `p${++made}`;
        ptys.push(ptyId);
        return { ok: true, ptyId, pid: 4000 + made };
      }
      case "pty.kill":
        ptys.splice(0, ptys.length, ...ptys.filter(id => id !== String(frame["ptyId"])));
        return { ok: true };
      case "pty.list":
        return { ok: true, ptys: ptys.map(id => ({ id })) };
      default:
        return { ok: true };
    }
  };

  // The machine's exec.
  let execMode: unknown = w.machine?.exec ?? "ok";
  const exec = async (line: string, o: { timeoutMs: number }): Promise<{ exitCode: number; stdout: string; stderr: string }> => {
    fx.push(["exec", line, o.timeoutMs]);
    const mode = execMode as string | { exit: number; stderr: string } | { throw: string };
    if (mode === "never") return new Promise(() => {});
    if (mode === "gate") await waitGate("exec");
    if (typeof mode === "object" && "throw" in mode) throw new Error(mode.throw);
    if (typeof mode === "object" && "exit" in mode) return { exitCode: mode.exit, stdout: "", stderr: mode.stderr };
    return { exitCode: 0, stdout: "", stderr: "" };
  };

  // The own road's channels and the served road's link.
  const listeners = new Map<number, (event: Frame) => void>();
  let opened = 0;
  let linkOnEvent: ((event: Frame) => void) | undefined;
  let linkDrops: () => void = () => {};
  const names = w.door?.names ?? {};

  // The store, the live map, the bus.
  const docs = new Map<string, Map<string, unknown>>(Object.entries(w.store ?? {}).map(([c, rows]) => [c, new Map(Object.entries(structuredClone(rows)))]));
  const live = new Set<string>(w.live ?? [String(w.record["id"])]);
  let busListeners: ((e: Frame) => void)[] = [];
  const threadRecords = new Map((w.threads ?? []).map(t => [t.id, { workspaceId: t.workspaceId, ...(t.harness !== undefined ? { harness: t.harness } : {}) }]));
  let minted = 0;

  const placeDoor =
    w.door === null || w.door === undefined
      ? undefined
      : {
          nameOf: (id: string) => names[id] ?? id,
          reportOf: async (id: string) => w.door!.reports?.[id] ?? undefined,
          joined: () => false,
          offerOf: () => undefined,
          settingsAt: () => undefined,
          link: () => ({}),
          channel: (_id: string, onEvent: (event: Frame) => void) => {
            if (w.door!.link === false) return undefined;
            linkOnEvent = onEvent;
            return {
              send: async () => ({}),
              close: () => void fx.push(["link-closed"]),
              closed: new Promise<void>(resolve => (linkDrops = resolve)),
            };
          },
          watchSys: async () => () => void fx.push(["released", "sys"]),
        };

  const ctx = {
    local: undefined,
    placeDoor,
    clock: { now: () => 0 },
    places: { wired: "solari" },
    projectsHeld: new Map(w.projects.map(p => [String(p["id"]), p])),
    threadRecords,
    sessions: new Map(),
    transcriptIndex: new Map(),
    opts: { broker: { ready: () => w.broker === true } },
    state: {},
    bus: { on: (_type: string, fn: (e: Frame) => void) => void busListeners.push(fn) },
    live: { has: (id: string) => live.has(id) },
    moduleOf: () => ({ daemonRoad: async () => ({ url: "ws://daemon", daemonToken: "dt", expiresAt: 0 }), homeDir: () => undefined }),
    openChannel: async (o: { url: string; token: string; onEvent: (event: Frame) => void }) => {
      const id = ++opened;
      listeners.set(id, o.onEvent);
      return {
        send: daemonSend,
        close: () => {
          listeners.delete(id);
          fx.push(["own-closed"]);
        },
        closed: new Promise(() => {}),
      };
    },
    threadLaunch: async (_entry: unknown, threadId: string, root: string) => {
      fx.push(["launch", threadId, root]);
      const env = { LAUNCH: "1", HOST_TOKEN: "launch" };
      if (w.scoped === false) return { scoped: undefined, env };
      const n = ++minted;
      return { scoped: { deviceId: `d${n}`, deviceToken: `tok${n}` }, env };
    },
    rootOf: (threadId: string) => `root-${threadId}`,
    latestOn: (threadId: string) => (w.latest?.[threadId] === undefined ? undefined : { harness: w.latest[threadId] }),
    brokeredLaunch: () => undefined,
    deviceDoor: {
      revoke: async (id: string) => {
        fx.push(["revoke", id]);
        if (w.revokeFails === true) throw new Error("revoke refused");
        return true;
      },
    },
    store: {
      get: async (c: string, id: string) => docs.get(c)?.get(id) ?? null,
      put: async (c: string, id: string, value: unknown) => {
        if (w.putFails === true) throw new Error("disk full");
        fx.push(["put", c, id, structuredClone(value)]);
        const rows = docs.get(c) ?? new Map<string, unknown>();
        rows.set(id, structuredClone(value));
        docs.set(c, rows);
      },
      delete: async (c: string, id: string) => {
        if (c !== "pty-threads") fx.push(["store-delete", c, id]);
        docs.get(c)?.delete(id);
      },
      keys: async (c: string) => [...(docs.get(c)?.keys() ?? [])],
    },
    threadFolder: async () => {
      fx.push(["thread-folder"]);
      return w.threadFolder ?? "/srv/alpha/work";
    },
    threadEnv: () => w.threadEnv ?? {},
    scopesOn: () => [],
    persist: async (record: { id: string; worktree?: { branch?: string } }) => {
      if (w.persistFails === true) throw new Error("store refused");
      fx.push(["persist", record.id, record.worktree?.branch ?? null]);
    },
    readCheckout: (entry: { record: { id: string } }) => void fx.push(["read-checkout", entry.record.id]),
    workspaces: {
      delete: async (id: string) => {
        fx.push(["delete-workspace", id]);
        if (w.deleteFails === true) throw new Error("delete refused");
      },
    },
  };

  const entry = {
    record: structuredClone(w.record),
    machine: { id: String(w.record["machineId"]), ...(w.machine?.served === true ? { daemonFrame: daemonSend } : {}), exec },
    deleting: w.deleting === true ? { phase: "running", line: "x" } : undefined,
  };
  let area = channelsArea(ctx as never);
  const channels = new Map<string, { send: (f: Frame) => Promise<unknown>; close: () => void }>();
  const background = new Map<string, Promise<unknown>>();
  const carries = w.carries ?? area.WORKSPACE_FRAMES;
  const projectOf = (id: string) => ctx.projectsHeld.get(id) as never;

  for (const step of script) {
    const name = String(step["on"] ?? "a");
    switch (step["step"]) {
      case "open": {
        const as = String(step["as"] ?? "a");
        try {
          const channel = await area.copyChannel(entry as never, event => void fx.push(["client", as, structuredClone(event)]), carries);
          channels.set(as, channel as never);
          steps.push({ open: "ok" });
        } catch (e) {
          steps.push({ open: { thrown: errOf(e) } });
        }
        break;
      }
      case "send": {
        const frame = step["frame"] as Frame;
        const sent = channels.get(name)!.send(structuredClone(frame)).then(
          reply => ({ op: frame["op"], reply }),
          (e: unknown) => ({ op: frame["op"], thrown: (e as Error).message }),
        );
        if (step["bg"] !== undefined) background.set(String(step["bg"]), sent);
        else steps.push(await sent);
        break;
      }
      case "finish":
        steps.push(await background.get(String(step["name"]))!);
        break;
      case "close":
        channels.get(name)!.close();
        break;
      case "push":
        if (w.machine?.served === true) linkOnEvent?.(structuredClone(step["event"] as Frame));
        else for (const fn of [...listeners.values()]) fn(structuredClone(step["event"] as Frame));
        break;
      case "linkDrop":
        linkDrops();
        break;
      case "hold":
        gates.set(String(step["gate"]), { open: false, waiters: [] });
        break;
      case "release": {
        const g = gates.get(String(step["gate"]))!;
        g.open = true;
        for (const wake of g.waiters.splice(0)) wake();
        break;
      }
      case "machine":
        execMode = step["exec"];
        break;
      case "daemon":
        if (step["drop"] !== undefined) ptys.splice(0, ptys.length, ...ptys.filter(id => id !== step["drop"]));
        if (step["replies"] !== undefined) for (const [op, r] of Object.entries(step["replies"] as Record<string, Frame>)) replies.set(op, [r]);
        break;
      case "advance":
        await vi.advanceTimersByTimeAsync(Number(step["ms"]));
        break;
      case "delete":
        live.delete(String(step["workspace"]));
        for (const fn of busListeners) fn({ type: "workspace.deleted", workspaceId: String(step["workspace"]) });
        break;
      case "restart":
        busListeners = [];
        area = channelsArea(ctx as never);
        break;
      case "heldDevices":
        steps.push({ held: [...(await area.shellHeldDevices())].sort() });
        break;
      case "lead":
        try {
          steps.push({ lead: await area.leadStart(entry as never, String(step["child"]), step["road"] as "fork" | "run") });
        } catch (e) {
          steps.push({ lead: { thrown: errOf(e) } });
        }
        break;
      case "startChild":
        try {
          steps.push({ startChild: await area.startChildOn(entry as never, String(step["branch"]), String(step["lead"])) });
        } catch (e) {
          steps.push({ startChild: { thrown: errOf(e) } });
        }
        break;
      case "origins":
        hoisted.origins = structuredClone(step["map"] as Record<string, string | null>);
        break;
      case "same": {
        const caller = step["caller"] === "scoped" ? { origin: "here", by: { kind: "thread", threadId: "t1", workspaceId: "w1", rootThreadId: "t1" } } : step["caller"] === "here" ? "here" : undefined;
        const before = fx.length;
        const answer = area.sameRepository(projectOf(String(step["a"])), projectOf(String(step["b"])), caller as never);
        steps.push({ same: answer, reads: fx.slice(before).map(e => (e as unknown[])[1]) });
        break;
      }
      case "settle":
        break;
      case "probe":
        steps.push({ probe: fx.filter(e => (e as unknown[])[0] === "frame").length });
        break;
      default:
        throw new Error(`unknown step ${String(step["step"])}`);
    }
    await settle();
  }
  warn.mockRestore();
  hoisted.fx = null;
  return { steps, fx: canon(fx) };
}

// Worlds ----------------------------------------------------------------------------------------------------

const lab = (over: Frame = {}): Frame => ({ id: "p1", name: "lab", computer: "box1", remote: "https://github.com/acme/lab", path: "/srv/alpha", source: { kind: "git" }, defaultBranch: "main", ...over });
const cloudOnBox = (over: Frame = {}): Frame => ({ id: "w1", name: "pricing page", kind: "cloud", project: "p1", machineId: "m1", phase: "running", place: "box1", ...over });
const folderOnBox = (over: Frame = {}): Frame => ({ id: "w1", name: "spoo-ts", kind: "place", project: "p1", machineId: "", phase: "running", place: "box1", ...over });
const cloudAtProvider = (over: Frame = {}): Frame => ({ id: "w1", name: "pricing page", kind: "cloud", project: "p1", machineId: "m1", phase: "running", ...over });
const localCopy = (over: Frame = {}): Frame => ({ id: "w1", name: "lab", kind: "local", project: "p1", machineId: "", phase: "running", ...over });
const door = (over: Partial<NonNullable<World["door"]>> = {}): World["door"] => ({ names: { box1: "studio" }, reports: { box1: report() }, link: true, ...over });
const served = (w: World): World => ({ machine: { served: true }, door: door(), ...w });
const thread = (id: string, harness?: string) => ({ id, workspaceId: "w1", ...(harness !== undefined ? { harness } : {}) });
const create = (over: Frame = {}): Step => ({ step: "send", frame: { op: "pty.create", cols: 80, rows: 24, ...over } });
const brokeredFork = (over: Partial<World> = {}): World => ({
  record: cloudAtProvider(),
  projects: [lab({ computer: "solari" })],
  broker: true,
  threads: [thread("t1", "claude")],
  threadEnv: { WSP_THREAD_HOME: "/srv/alpha", PATH: "/usr/bin" },
  ...over,
});

const status = (o: { head: string; upstream?: string; ahead?: number; entries?: string[]; defaultBranch?: string; countsUnknown?: boolean }): Frame => ({
  ok: true,
  branch: { oid: "abc1234", head: o.head, ...(o.upstream !== undefined ? { upstream: o.upstream } : {}), ahead: o.ahead ?? 0, behind: 0 },
  entries: (o.entries ?? []).map((xy, i) => ({ xy, path: `file-${i}.ts` })),
  root: "/srv/alpha",
  ...(o.defaultBranch !== undefined ? { defaultBranch: o.defaultBranch } : {}),
  ...(o.countsUnknown === true ? { countsUnknown: true } : {}),
});
const pushed = (branch: string): Frame => ({ ok: true, branch, base: "main", remote: "origin", ahead: 1, uncommitted: 0, stat: [] });

const scenarios: Record<string, { world: World; script: Step[] }> = {};
const add = (name: string, world: World, script: Step[]): void => void (scenarios[name] = { world, script });

// C1: the served channel's refusals.
add("c1 no place on the record", served({ record: cloudAtProvider(), projects: [lab()] }), [{ step: "open" }]);
add("c1 no door", { record: cloudOnBox(), projects: [lab()], machine: { served: true }, door: null }, [{ step: "open" }]);
add("c1 no link", served({ record: cloudOnBox(), projects: [lab()], door: door({ link: false }) }), [{ step: "open" }]);
add("c1 no place on a folder record", served({ record: localCopy(), projects: [lab({ computer: "here" })] }), [{ step: "open" }]);

// C2: the hello.
add("c2 hello with the report's version", served({ record: cloudOnBox(), projects: [lab()] }), [{ step: "open" }]);
add("c2 hello without a report", served({ record: cloudOnBox(), projects: [lab()], door: door({ reports: {} }) }), [{ step: "open" }]);
add("c2 hello root is the worktree", served({ record: cloudOnBox({ worktree: { path: "/srv/trees/kid", branch: "kid", made: true } }), projects: [lab()] }), [{ step: "open" }]);

// C3: the events the channel hears.
add("c3 guest and pty events", served({ record: cloudOnBox(), projects: [lab()] }), [
  { step: "open" },
  { step: "push", event: { type: "guest.opened", machineId: "m1", sessionId: "g1" } },
  { step: "push", event: { type: "guest.message", machineId: "m2", sessionId: "g2" } },
  { step: "push", event: { type: "guest.closed", sessionId: "g3" } },
  { step: "push", event: { type: "tunnel.data", machineId: "m1", id: 1 } },
  { step: "push", event: { type: "tunnel.end", machineId: "m2", id: 2 } },
  { step: "push", event: { type: "pty.data", ptyId: "p1", data: "before" } },
  create(),
  { step: "push", event: { type: "pty.data", ptyId: "p1", data: "hello" } },
  { step: "push", event: { type: "pty.data", ptyId: "p99", data: "another pane's" } },
  { step: "push", event: { type: "pty.mode", ptyId: "p1", mode: "raw" } },
  { step: "push", event: { type: "pty.data" } },
  { step: "send", frame: { op: "pty.attach", ptyId: "p5" } },
  { step: "push", event: { type: "pty.data", ptyId: "p5", data: "attached" } },
  { step: "push", event: { type: "pty.exit", ptyId: "p5", exitCode: 0 } },
  { step: "push", event: { type: "pty.exit", ptyId: "p99", exitCode: 0 } },
  { step: "send", frame: { op: "pty.list" } },
  { step: "push", event: { type: "proc.snapshot", at: 1 } },
  { step: "push", event: { type: "sys.sample", at: 1 } },
  { step: "push", event: { type: "ports.changed", ports: [22] } },
  { step: "push", event: { type: "daemon.hello", version: 1 } },
]);
add("c3 listed ptys are named", served({ record: cloudOnBox(), projects: [lab()], daemon: { replies: { "pty.list": { ok: true, ptys: [{ id: "p3" }, { id: "p4" }] } } } }), [
  { step: "open" },
  { step: "send", frame: { op: "pty.list" } },
  { step: "push", event: { type: "pty.data", ptyId: "p4", data: "listed" } },
  { step: "push", event: { type: "pty.data", ptyId: "p6", data: "not listed" } },
]);
add("c3 ptys are named by String of whatever came", served({ record: cloudOnBox(), projects: [lab()], daemon: { replies: { "pty.create": [{ ok: true }, { ok: true, ptyId: 7 }], "pty.list": { ok: true, ptys: [{ id: 8 }, { nothing: 1 }] } } } }), [
  { step: "open" },
  { step: "push", event: { type: "pty.data", data: "before" } },
  create(),
  { step: "push", event: { type: "pty.data", data: "named undefined" } },
  { step: "push", event: { type: "pty.data", ptyId: null, data: "null" } },
  create(),
  { step: "push", event: { type: "pty.data", ptyId: 7, data: "number" } },
  { step: "push", event: { type: "pty.data", ptyId: "7", data: "string" } },
  { step: "send", frame: { op: "pty.list" } },
  { step: "push", event: { type: "pty.data", ptyId: 8, data: "listed number" } },
]);
add("c3 a refused create names nothing", served({ record: cloudOnBox(), projects: [lab()], daemon: { replies: { "pty.create": { ok: false, error: "no ptys left", ptyId: "p7" } } } }), [
  { step: "open" },
  create(),
  { step: "push", event: { type: "pty.data", ptyId: "p7", data: "x" } },
]);
add("c3 the panels hear first on a folder", served({ record: folderOnBox(), projects: [lab()] }), [
  { step: "open" },
  { step: "push", event: { type: "sys.sample", at: 1 } },
  { step: "send", frame: { op: "sys.watch" } },
  { step: "push", event: { type: "sys.sample", at: 2 } },
  { step: "push", event: { type: "proc.snapshot", at: 2 } },
  { step: "push", event: { type: "ports.changed", ports: [22] } },
  { step: "push", event: { type: "guest.opened", machineId: "m1" } },
  { step: "push", event: { type: "guest.opened", machineId: "" } },
]);

// C4: the frame table.
add("c4 frame table", served({ record: cloudOnBox(), projects: [lab()], daemon: { replies: { "fs.list": { ok: true, id: 4242, entries: [] }, "git.status": { ok: false, code: "no-repo", error: "not a repository" } } } }), [
  { step: "open" },
  { step: "send", frame: { op: "ports.watch", roots: [] } },
  { step: "send", frame: { op: "sys.watch" } },
  { step: "send", frame: { op: "proc.watch" } },
  { step: "send", frame: { op: "proc.unwatch" } },
  { step: "send", frame: { op: "proc.inspect", pid: 900 } },
  { step: "send", frame: { op: "proc.kill", pid: 900, signal: "TERM" } },
  { step: "send", frame: { op: "container.stop", id: "c1" } },
  { step: "send", frame: { op: "exec", cmd: "kill -9 1" } },
  { step: "send", frame: { op: "guest.watch" } },
  { step: "send", frame: { op: "fs.list", path: "/", id: 9 } },
  { step: "send", frame: { op: "git.status", cwd: "/srv/alpha" } },
  { step: "send", frame: { op: "ping" } },
]);
add("c4 the panels answer on a folder", served({ record: folderOnBox(), projects: [lab()], threadEnv: { WSP_THREAD_HOME: "/srv/alpha", PATH: "/usr/bin" } }), [
  { step: "open" },
  { step: "send", frame: { op: "sys.watch" } },
  { step: "send", frame: { op: "sys.unwatch" } },
  create(),
  { step: "push", event: { type: "pty.data", ptyId: "p1", data: "from the pty the panels made" } },
  create({ cwd: "/srv/alpha/docs", env: { PATH: "/opt/bin", TERM: "xterm" } }),
  { step: "send", frame: { op: "fs.list", path: "/" } },
  { step: "send", frame: { op: "exec", cmd: "ls" } },
  { step: "send", frame: { op: "proc.kill", pid: 1 } },
]);

// C5: close.
add("c5 close detaches what stands attached", served({ record: cloudOnBox(), projects: [lab()] }), [
  { step: "open" },
  { step: "send", frame: { op: "pty.attach", ptyId: "pa" } },
  { step: "send", frame: { op: "pty.attach", ptyId: "pb" } },
  { step: "send", frame: { op: "pty.attach", ptyId: "pc" } },
  { step: "send", frame: { op: "pty.attach", ptyId: "pd" } },
  { step: "send", frame: { op: "pty.detach", ptyId: "pb" } },
  { step: "send", frame: { op: "pty.kill", ptyId: "pc" } },
  { step: "push", event: { type: "pty.exit", ptyId: "pd", exitCode: 0 } },
  { step: "close" },
]);
add("c5 a refused attach is not held", served({ record: cloudOnBox(), projects: [lab()], daemon: { replies: { "pty.attach": { ok: false, error: "no such pty" } } } }), [
  { step: "open" },
  { step: "send", frame: { op: "pty.attach", ptyId: "pz" } },
  { step: "close" },
]);
add("c5 the link dropping closes the panels", served({ record: folderOnBox(), projects: [lab()] }), [
  { step: "open" },
  { step: "send", frame: { op: "sys.watch" } },
  { step: "linkDrop" },
  { step: "push", event: { type: "sys.sample", at: 3 } },
]);
add("c5 close closes the panels and the link", served({ record: folderOnBox(), projects: [lab()] }), [
  { step: "open" },
  { step: "send", frame: { op: "sys.watch" } },
  { step: "close" },
]);

// C6: the terminal token's environment, in order, through the pty.create that carries it.
add("c6 terminal token env order", brokeredFork(), [{ step: "open" }, create({ thread: "t1" })]);
add("c6 a thread of another workspace takes the newest here", brokeredFork({ threads: [thread("t1", "claude"), { id: "tx", workspaceId: "w9", harness: "codex" }, thread("t2", "claude")] }), [
  { step: "open" },
  create({ thread: "tx" }),
  create({ thread: "t1" }),
  create(),
  create({ thread: 7 }),
]);
add("c6 no thread no token", brokeredFork({ threads: [] }), [{ step: "open" }, create({ thread: "t1" })]);
add("c6 no scoped device no token", brokeredFork({ scoped: false }), [{ step: "open" }, create({ thread: "t1" })]);
add("c6 the harness off the latest row", brokeredFork({ threads: [thread("t1")], latest: { t1: "codex" } }), [{ step: "open" }, create({ thread: "t1" })]);
add("c6 no harness at all", brokeredFork({ threads: [thread("t1")] }), [{ step: "open" }, create({ thread: "t1" })]);
add("c6 a harness with no sign-in", brokeredFork({ threads: [thread("t1", "gemini")] }), [{ step: "open" }, create({ thread: "t1" })]);

// C7 and C8: the shell's sign-in and the terminal programs.
add("c8 the first shell waits for the write, later ones do not", brokeredFork({ threads: [thread("t1", "codex")], machine: { exec: "gate" } }), [
  { step: "hold", gate: "exec" },
  { step: "open" },
  { step: "send", bg: "first", frame: { op: "pty.create", cols: 80, rows: 24, thread: "t1" } },
  { step: "settle" },
  { step: "probe" },
  { step: "release", gate: "exec" },
  { step: "finish", name: "first" },
  { step: "probe" },
  { step: "machine", exec: "never" },
  create({ thread: "t1" }),
  { step: "probe" },
]);
add("c8 a write that never answers is waited on for an inline exec's span", brokeredFork({ threads: [thread("t1", "codex")], machine: { exec: "never" } }), [
  { step: "open" },
  { step: "send", bg: "first", frame: { op: "pty.create", cols: 80, rows: 24, thread: "t1" } },
  { step: "advance", ms: 19000 },
  { step: "probe" },
  { step: "advance", ms: 1000 },
  { step: "finish", name: "first" },
  { step: "probe" },
  { step: "send", bg: "second", frame: { op: "pty.create", cols: 80, rows: 24, thread: "t1" } },
  { step: "advance", ms: 19000 },
  { step: "probe" },
  { step: "advance", ms: 1000 },
  { step: "finish", name: "second" },
  { step: "probe" },
]);
add("c8 a refused write warns and the shell opens", brokeredFork({ threads: [thread("t1", "codex")], machine: { exec: { exit: 1, stderr: "mkdir: cannot create directory '/usr/local/libexec/wsp-terminal': Read-only file system\nsecond line\n" } } }), [
  { step: "open" },
  create({ thread: "t1" }),
  create({ thread: "t1" }),
]);
add("c8 a failed exec counts as exit minus one", brokeredFork({ threads: [thread("t1", "claude")], machine: { exec: { throw: "the machine is gone" } } }), [{ step: "open" }, create({ thread: "t1" })]);
add("c8 a refused write with no stderr", brokeredFork({ threads: [thread("t1", "claude")], machine: { exec: { exit: 2, stderr: "" } } }), [{ step: "open" }, create({ thread: "t1" })]);
add("c8 each agent on a machine has its own write", brokeredFork({ threads: [thread("t1", "claude"), thread("t2", "codex")], machine: { exec: "gate" } }), [
  { step: "hold", gate: "exec" },
  { step: "open" },
  { step: "send", bg: "claude", frame: { op: "pty.create", cols: 80, rows: 24, thread: "t1" } },
  { step: "settle" },
  { step: "probe" },
  { step: "release", gate: "exec" },
  { step: "finish", name: "claude" },
  { step: "hold", gate: "exec" },
  { step: "send", bg: "codex", frame: { op: "pty.create", cols: 80, rows: 24, thread: "t2" } },
  { step: "settle" },
  { step: "probe" },
  { step: "release", gate: "exec" },
  { step: "finish", name: "codex" },
  { step: "machine", exec: "never" },
  create({ thread: "t1" }),
  create({ thread: "t2" }),
  { step: "probe" },
]);
add("c8 a deleted workspace forgets its written terminals", brokeredFork({ threads: [thread("t1", "codex")], machine: { exec: "gate" } }), [
  { step: "hold", gate: "exec" },
  { step: "open" },
  { step: "release", gate: "exec" },
  create({ thread: "t1" }),
  { step: "hold", gate: "exec" },
  { step: "delete", workspace: "w1" },
  { step: "send", bg: "again", frame: { op: "pty.create", cols: 80, rows: 24, thread: "t1" } },
  { step: "settle" },
  { step: "probe" },
  { step: "release", gate: "exec" },
  { step: "finish", name: "again" },
  { step: "probe" },
]);

// C9: the tokens the host keeps.
add("c9 tokens kept and revoked", brokeredFork(), [
  { step: "open" },
  create({ thread: "t1" }),
  create(),
  create(),
  { step: "push", event: { type: "pty.exit", ptyId: "p1", code: 0 } },
  { step: "send", frame: { op: "pty.kill", ptyId: "p2" } },
  { step: "send", frame: { op: "pty.kill", ptyId: "p77" } },
  { step: "close" },
  { step: "daemon", drop: "p3" },
  { step: "open", as: "b" },
  { step: "send", on: "b", frame: { op: "pty.list" } },
  { step: "close", on: "b" },
]);
add("c9 a kill the daemon refused ends nothing", brokeredFork({ daemon: { replies: { "pty.kill": { ok: false, error: "no such pty" } } } }), [
  { step: "open" },
  create({ thread: "t1" }),
  { step: "send", frame: { op: "pty.kill", ptyId: "p1" } },
]);
add("c9 a list ends the exited and the missing and keeps the standing", brokeredFork({ daemon: { replies: { "pty.list": { ok: true, ptys: [{ id: "p1", exited: true }, { id: "p3" }, { id: "p9" }] } } } }), [
  { step: "open" },
  create({ thread: "t1" }),
  create(),
  create(),
  { step: "send", frame: { op: "pty.list" } },
]);
add("c9 a list that names no ptys ends nothing", brokeredFork({ daemon: { replies: { "pty.list": { ok: true } } } }), [
  { step: "open" },
  create({ thread: "t1" }),
  { step: "send", frame: { op: "pty.list" } },
]);
add("c9 a failed revoke warns", brokeredFork({ revokeFails: true }), [
  { step: "open" },
  create({ thread: "t1" }),
  { step: "push", event: { type: "pty.exit", ptyId: "p1", code: 0 } },
]);
add("c9 a failed store write warns and the shell still opens", brokeredFork({ putFails: true }), [{ step: "open" }, create({ thread: "t1" }), create()]);
add("c9 held devices skip dead workspaces", brokeredFork({ store: { "pty-tokens": { w1: { p1: "d1", p2: "d2", p3: 5, p4: null }, w2: { p1: "d3" }, w3: { p9: "d4" }, w5: "junk" } }, live: ["w1", "w3", "w5"] }), [{ step: "heldDevices" }]);
add("c9 deleted workspace ends its shells", brokeredFork({ store: { "pty-tokens": { w1: { p1: "d7", p2: "d8" } } } }), [{ step: "delete", workspace: "w1" }, { step: "heldDevices" }]);
add("c9 delete of a workspace with no shells writes nothing", brokeredFork(), [{ step: "delete", workspace: "w1" }]);
add("c9 shell tokens survive a restart", brokeredFork(), [
  { step: "open" },
  create({ thread: "t1" }),
  create(),
  { step: "close" },
  { step: "restart" },
  { step: "heldDevices" },
  { step: "open", as: "b" },
  { step: "send", on: "b", frame: { op: "pty.list" } },
  { step: "daemon", drop: "p2" },
  { step: "send", on: "b", frame: { op: "pty.list" } },
  { step: "heldDevices" },
]);

// C10: a blocked computer.
const blocked = (over: Frame = {}) => door({ reports: { box1: report({ runsWorkspaces: false, workspacesBlocked: "docker is not installed", ...over }) } });
add("c10 a blocked computer carries status and ping", served({ record: cloudOnBox(), projects: [lab()], door: blocked() }), [
  { step: "open" },
  { step: "send", frame: { op: "git.status", cwd: "/srv/alpha" } },
  { step: "send", frame: { op: "ping" } },
  { step: "send", frame: { op: "fs.list", path: "/" } },
  create(),
  { step: "send", frame: { op: "pty.attach", ptyId: "p1" } },
  { step: "push", event: { type: "guest.opened", machineId: "m1" } },
  { step: "push", event: { type: "pty.data", ptyId: "p1", data: "x" } },
  { step: "push", event: { type: "daemon.hello", version: 9 } },
  { step: "close" },
]);
add("c10 a blocked computer with no reason", served({ record: cloudOnBox(), projects: [lab()], door: blocked({ workspacesBlocked: undefined }) }), [{ step: "open" }, { step: "send", frame: { op: "fs.list", path: "/" } }]);
add("c10 a folder is never blocked", served({ record: folderOnBox(), projects: [lab()], door: blocked() }), [
  { step: "open" },
  { step: "send", frame: { op: "fs.list", path: "/" } },
  { step: "push", event: { type: "guest.opened", machineId: "" } },
]);
add("c10 a blocked computer's own daemon road", { record: cloudOnBox(), projects: [lab()], door: blocked() }, [
  { step: "open" },
  { step: "send", frame: { op: "git.status", cwd: "/srv/alpha" } },
  { step: "send", frame: { op: "fs.list", path: "/" } },
  { step: "push", event: { type: "pty.data", ptyId: "p1", data: "x" } },
  { step: "push", event: { type: "daemon.hello", version: 9 } },
]);

// C11: who mints.
const kept = { "pty-tokens": { w1: { p1: "d1" } } };
const exits = (): Step[] => [{ step: "open" }, { step: "push", event: { type: "pty.exit", ptyId: "p1", code: 0 } }, { step: "send", frame: { op: "pty.list" } }];
add("c11 a machine's own daemon mints, broker not ready", brokeredFork({ broker: false, store: kept }), exits());
add("c11 a machine's own daemon mints, broker ready", brokeredFork({ store: kept }), exits());
add("c11 a local copy never mints", brokeredFork({ record: localCopy(), projects: [lab({ computer: "here" })], store: kept }), exits());
add("c11 a folder on a computer never mints", { record: folderOnBox(), projects: [lab()], store: kept, door: door() }, exits());
add("c11 a machine on a joined computer's link never mints", served({ record: cloudOnBox(), projects: [lab()], store: kept }), exits());
add("c11 a machine on a box with its own daemon mints", brokeredFork({ record: cloudOnBox(), door: door(), store: kept }), exits());
add("c11 a folder with a ready broker mints nothing", served({ record: folderOnBox(), projects: [lab()], broker: true, threads: [thread("t1", "claude")], threadEnv: { WSP_THREAD_HOME: "/srv/alpha", PATH: "/usr/bin" } }), [
  { step: "open" },
  create({ thread: "t1" }),
  { step: "send", frame: { op: "pty.list" } },
]);
add("c11 a kill answered ok on a minting channel ends the token", brokeredFork({ broker: false, store: kept }), [{ step: "open" }, { step: "send", frame: { op: "pty.kill", ptyId: "p1" } }]);
add("c11 a kill answered on a local copy ends nothing", brokeredFork({ record: localCopy(), projects: [lab({ computer: "here" })], store: kept }), [{ step: "open" }, { step: "send", frame: { op: "pty.kill", ptyId: "p1" } }]);

// C12: pty.create.
add("c12 pty.create mints and keeps", brokeredFork(), [
  { step: "open" },
  { step: "send", frame: { op: "pty.create", thread: "t1", cols: 80, rows: 24 } },
  { step: "send", frame: { op: "pty.create", cwd: "/srv/alpha/docs", cols: 80, env: { HOST_TOKEN: "from the page", TERM: "xterm" }, thread: "t1" } },
  { step: "send", frame: { op: "pty.create", env: "not an object", cols: 1 } },
  { step: "send", frame: { op: "pty.create", env: null } },
  { step: "send", frame: { op: "ping" } },
]);
add("c12 cwd is the frame's or the thread's folder", { record: localCopy(), projects: [lab({ computer: "here" })], threadFolder: "/srv/alpha/work" }, [
  { step: "open" },
  create(),
  create({ cwd: "/tmp" }),
  create({ cwd: 7 }),
  create({ cwd: null }),
  create({ thread: "t1" }),
]);
add("c12 a pty.create with no token keeps the frame's own env", brokeredFork({ broker: false }), [
  { step: "open" },
  create({ env: { A: "1" } }),
  create(),
]);
add("c12 a late reply revokes", brokeredFork(), [
  { step: "open" },
  { step: "hold", gate: "pty.create" },
  { step: "send", bg: "late", frame: { op: "pty.create", cols: 80, rows: 24, thread: "t1" } },
  { step: "settle" },
  { step: "delete", workspace: "w1" },
  { step: "release", gate: "pty.create" },
  { step: "finish", name: "late" },
  { step: "heldDevices" },
]);
add("c12 a reply while the workspace is deleting revokes", brokeredFork({ deleting: true }), [{ step: "open" }, create({ thread: "t1" })]);
add("c12 a reply while the workspace is gone from live revokes", brokeredFork({ live: [] }), [{ step: "open" }, create({ thread: "t1" })]);
add("c12 a refused reply revokes", brokeredFork({ daemon: { replies: { "pty.create": { ok: false, error: "no ptys left" } } } }), [{ step: "open" }, create({ thread: "t1" })]);
add("c12 a reply with no ptyId revokes", brokeredFork({ daemon: { replies: { "pty.create": { ok: true, ptyId: 7 } } } }), [{ step: "open" }, create({ thread: "t1" })]);
add("c12 a failed send revokes", brokeredFork({ daemon: { throws: { "pty.create": "connection lost" } } }), [{ step: "open" }, create({ thread: "t1" }), { step: "heldDevices" }]);

// C13: where a child starts.
const lead = (name: string, record: Frame, project: Frame, replies: Record<string, Frame | Frame[]>, road: "fork" | "run" = "fork", child = "helper", more: Partial<World> = {}): void =>
  add(`c13 ${name}`, { record, projects: [project], machine: { served: true }, daemon: { replies }, ...more }, [{ step: "lead", road, child }]);
const rec = (over: Frame = {}): Frame => ({ id: "w1", name: "lead", kind: "cloud", project: "p1", machineId: "m1", phase: "running", ...over });
lead("status refused", rec(), lab(), { "git.status": { ok: false, error: "the daemon did not answer" } });
lead("status refused with a code", rec(), lab(), { "git.status": { ok: false, code: "unreachable", error: "machine is paused" } });
lead("default branch ahead, a fork", rec(), lab(), { "git.status": status({ head: "main", ahead: 1, defaultBranch: "main" }) });
lead("default branch ahead, a run", rec(), lab(), { "git.status": status({ head: "main", ahead: 1, defaultBranch: "main" }) }, "run");
lead("default branch master ahead", rec(), lab(), { "git.status": status({ head: "master", ahead: 2, defaultBranch: "master" }) });
lead("default branch counts unknown", rec(), lab(), { "git.status": status({ head: "master", ahead: 0, defaultBranch: "master", countsUnknown: true }) });
lead("a branch called main where the default is master", rec(), lab(), { "git.status": status({ head: "main", ahead: 1, upstream: "origin/main", defaultBranch: "master" }), "git.push": pushed("main") });
lead("on the start branch with changes", rec(), lab(), { "git.status": status({ head: "main", defaultBranch: "main", entries: ["M.", "??", "!!", ".M"] }) });
lead("detached head", rec(), lab(), { "git.status": status({ head: "(detached)", ahead: 3, entries: ["M."] }) });
lead("empty branch", rec(), lab(), { "git.status": status({ head: "", ahead: 3 }) });
lead("tracks itself with nothing new", rec(), lab(), { "git.status": status({ head: "tree/lead", upstream: "origin/tree/lead", entries: ["M."] }) });
lead("tracks the base with nothing new", rec(), lab(), { "git.status": status({ head: "tree/lead", upstream: "origin/main" }) });
lead("no upstream and nothing new", rec(), lab(), { "git.status": status({ head: "tree/empty" }) });
lead("an upstream of another name", rec(), lab(), { "git.status": status({ head: "tree/lead", upstream: "origin/feature" }), "git.push": pushed("tree/lead") });
lead("ahead pushes with changes staying", rec(), lab(), { "git.status": status({ head: "only-here", ahead: 2, entries: ["M.", "M.", "??"] }), "git.push": pushed("only-here") });
lead("counts unknown pushes", rec(), lab(), { "git.status": status({ head: "tree/lead", upstream: "origin/tree/lead", countsUnknown: true }), "git.push": pushed("tree/lead") });
lead("push answers another branch name", rec(), lab(), { "git.status": status({ head: "tree/lead", ahead: 1 }), "git.push": pushed("wsp/tree-lead") });
lead("push refused on the default branch", rec(), lab(), { "git.status": status({ head: "tree/lead", ahead: 1 }), "git.push": { ok: false, code: "on-default-branch", error: "refusing to push main" } }, "run");
lead("push refused with no credential", rec(), lab(), {
  "git.status": status({ head: "tree/lead", ahead: 1 }),
  "git.push": { ok: false, code: "no-git-credential", error: "this computer has no git credential for github.com, so nothing was pushed" },
});
lead("push refused with no code", rec(), lab(), { "git.status": status({ head: "tree/lead", ahead: 1 }), "git.push": { ok: false, error: "remote hung up" } });
lead("the record's base outranks the project's", rec({ base: "develop" }), lab({ base: "trunk" }), { "git.status": status({ head: "develop", entries: ["M."] }) });
lead("the project's base outranks its default branch", rec(), lab({ base: "trunk" }), { "git.status": status({ head: "trunk" }) });
lead("the push names the record's base", rec({ base: "develop" }), lab({ base: "trunk" }), { "git.status": status({ head: "tree/lead", ahead: 1 }), "git.push": pushed("tree/lead") });
lead("the status is read at the worktree", rec({ worktree: { path: "/srv/trees/lead", branch: "tree/lead", made: true } }), lab(), { "git.status": status({ head: "tree/lead", upstream: "origin/tree/lead" }) });
add("c13 a blocked lead reads nothing", { record: cloudOnBox({ name: "lead" }), projects: [lab()], machine: { served: true }, door: door({ reports: { box1: report({ runsWorkspaces: false, workspacesBlocked: "docker is not installed" }) } }) }, [{ step: "lead", road: "fork", child: "helper" }]);
add("c13 a lead on a folder is never blocked", { record: folderOnBox({ name: "lead" }), projects: [lab()], machine: { served: true }, door: door({ reports: { box1: report({ runsWorkspaces: false, workspacesBlocked: "docker is not installed" }) } }), daemon: { replies: { "git.status": status({ head: "main" }) } } }, [{ step: "lead", road: "fork", child: "helper" }]);
add("c13 a lead on its own daemon reads over a channel it closes", { record: rec(), projects: [lab()], daemon: { replies: { "git.status": status({ head: "tree/lead", upstream: "origin/tree/lead" }) } } }, [{ step: "lead", road: "fork", child: "helper" }]);

// C14: a child put on a branch.
const child = (name: string, record: Frame, replies: Record<string, Frame>, more: Partial<World> = {}, branch = "pricing-page"): void =>
  add(`c14 ${name}`, { record, projects: [lab()], machine: { served: true }, daemon: { replies }, ...more }, [{ step: "startChild", branch, lead: "pricing page" }]);
const kid = (over: Frame = {}): Frame => ({ id: "w2", name: "helper", kind: "cloud", project: "p1", machineId: "m2", phase: "running", ...over });
child("a worktree takes the branch put", kid({ worktree: { path: "/srv/trees/kid", branch: "old", made: true } }), { "git.startOn": { ok: true, branch: "pricing-page-2", oid: "c0ffee" } });
child("a copy without a worktree", kid(), { "git.startOn": { ok: true, branch: "pricing-page", oid: "c0ffee" } });
child("a refused start deletes the child", kid(), { "git.startOn": { ok: false, error: "cannot reach origin" } });
child("a refused start whose delete fails", kid(), { "git.startOn": { ok: false, error: "cannot reach origin" } }, { deleteFails: true });
child("a failed persist deletes the child", kid(), { "git.startOn": { ok: true, branch: "pricing-page", oid: "c0ffee" } }, { persistFails: true });

// C15: one repository.
const sameProjects = [
  lab({ id: "home", name: "wsp", computer: "here", remote: "git@github.com:Acme/Lab.git", path: "/home/u/wsp", source: { kind: "folder", path: "/home/u/wsp" } }),
  lab({ id: "git-here", name: "wsp-git", computer: "here", remote: "git@github.com:Acme/Lab.git", path: "/home/u/other", source: { kind: "git" } }),
  lab({ id: "boat", name: "wsp-boat", computer: "default", remote: "https://github.com/acme/lab", path: "/work/lab" }),
  lab({ id: "hertzner", name: "wsp-hertzner", computer: "default", remote: "https://github.com/lab-co/lab.git", path: "/work/lab2" }),
  lab({ id: "else", name: "else", computer: "default", remote: "https://github.com/lab-co/else.git", path: "/work/else" }),
  lab({ id: "box-old", name: "box-old", computer: "srv", remote: "https://github.com/acme/lab", path: "/srv/lab" }),
  lab({ id: "box-new", name: "box-new", computer: "srv", remote: "https://github.com/lab-co/lab.git", path: "/srv/lab2" }),
  lab({ id: "dotgit", name: "dotgit", computer: "srv", remote: "https://github.com/acme/lab.git", path: "/srv/lab3" }),
  lab({ id: "slash", name: "slash", computer: "srv", remote: "https://github.com/ACME/Lab/", path: "/srv/lab4" }),
  lab({ id: "dotgit-slash", name: "dotgit-slash", computer: "srv", remote: "https://github.com/acme/lab.git/", path: "/srv/lab5" }),
  lab({ id: "scp", name: "scp", computer: "srv", remote: "git@github.com:acme/lab.git", path: "/srv/lab6" }),
  lab({ id: "other", name: "other", computer: "srv", remote: "https://github.com/acme/lab-other", path: "/srv/lab7" }),
  lab({ id: "nohost", name: "nohost", computer: "srv", remote: "", path: "/srv/lab8" }),
  lab({ id: "nohost2", name: "nohost2", computer: "srv", remote: "not a remote", path: "/srv/lab9" }),
  lab({ id: "gitlab", name: "gitlab", computer: "srv", remote: "https://gitlab.com/acme/lab", path: "/srv/lab10" }),
  lab({ id: "bare-folder", name: "bare", computer: "here", remote: "", path: "/home/u/bare", source: { kind: "folder", path: "/home/u/bare" } }),
  lab({ id: "folder-box", name: "folder-box", computer: "srv", remote: "git@github.com:Acme/Lab.git", path: "/srv/folder", source: { kind: "folder", path: "/srv/folder" } }),
];
const pairs = (a: string, bs: string[], caller = "none"): Step[] => bs.map(b => ({ step: "same", a, b, caller }));
add("c15 saved remotes", { record: rec(), projects: sameProjects }, [
  ...pairs("git-here", ["dotgit", "slash", "dotgit-slash", "scp", "boat", "box-old", "other", "hertzner", "else", "nohost", "nohost2", "gitlab", "git-here"]),
  ...pairs("boat", ["boat", "dotgit", "scp", "other"]),
  ...pairs("nohost", ["nohost", "nohost2", "boat"]),
]);
add("c15 a folder here's origin now", { record: rec(), projects: sameProjects }, [
  { step: "origins", map: { "/home/u/wsp": "git@github.com:Acme/Lab.git\n" } },
  ...pairs("home", ["boat", "hertzner", "else", "box-old", "box-new", "other"]),
  { step: "origins", map: { "/home/u/wsp": "https://github.com/lab-co/lab.git\n" } },
  ...pairs("home", ["boat", "hertzner", "else", "box-old", "box-new", "scp", "other"]),
  { step: "origins", map: { "/home/u/wsp": "  git@github.com:me/lab.git \n\n" } },
  ...pairs("home", ["boat", "hertzner", "else", "box-old", "box-new"]),
  { step: "origins", map: { "/home/u/wsp": null } },
  ...pairs("home", ["boat", "hertzner"]),
  { step: "origins", map: { "/home/u/wsp": "git@github.com:acme/lab.git" } },
  ...pairs("home", ["boat"], "here"),
  ...pairs("home", ["boat"], "scoped"),
  { step: "origins", map: { "/home/u/wsp": "https://github.com/acme/lab" } },
  ...pairs("home", ["home", "dotgit"]),
]);
add("c15 an origin is read only for a folder here", { record: rec(), projects: sameProjects }, [
  { step: "origins", map: { "/home/u/other": "https://github.com/lab-co/lab.git", "/home/u/bare": "https://github.com/lab-co/lab.git", "/srv/folder": "https://github.com/lab-co/lab.git", "/home/u/wsp": "https://github.com/lab-co/lab.git" } },
  ...pairs("git-here", ["hertzner", "box-new"]),
  ...pairs("folder-box", ["hertzner", "box-new"]),
  ...pairs("bare-folder", ["hertzner", "box-new"]),
  ...pairs("home", ["hertzner"]),
  ...pairs("boat", ["hertzner"]),
]);

const repositories = [
  "https://github.com/acme/lab.git",
  "https://github.com/acme/lab",
  "https://github.com/ACME/Lab/",
  "https://github.com/acme/lab.git/",
  "git@github.com:acme/lab.git",
  "ssh://git@github.com/acme/lab.git",
  "https://github.com/acme/lab-other",
  "https://gitlab.com/acme/lab",
  "https://GitHub.com/acme/lab",
  "https://github.com/acme",
  "https://example.com/acme/lab",
  "not a remote",
  "",
].map(remote => ({ remote, repo: (() => { const host = remoteHost(remote); const repo = ownerRepoOf(remote)?.toLowerCase(); return host === undefined || repo === undefined ? null : `${host}/${repo}`; })() }));

it("writes the channels_copy fixture", async () => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  const written: Record<string, unknown> = {};
  try {
    for (const [name, s] of Object.entries(scenarios)) {
      const { steps, fx } = await run(s.world, s.script);
      written[name] = { world: s.world, script: s.script, trace: { steps, fx } };
    }
  } finally {
    vi.useRealTimers();
  }
  const area = channelsArea({ projectsHeld: new Map(), threadRecords: new Map(), sessions: new Map(), transcriptIndex: new Map(), bus: { on: () => {} } } as never);
  writeFileSync(out, `${JSON.stringify({ constants: { INLINE_EXEC_MS, THREAD_ENV, TERMINAL_ENV, WORKSPACE_FRAMES: area.WORKSPACE_FRAMES }, repositories, scenarios: written }, null, 1)}\n`);
});
