// SPDX-License-Identifier: AGPL-3.0-only
// The tokens the host mints for a machine whose credentials it keeps: one per
// op whose git or gh reaches the git host, taken away at its reply, and one per
// shell a pane opens, taken away when the shell ends. The daemon is a fake that
// keeps every frame and asks the host's own token table whether each token it
// is handed is live while it answers.
import { afterEach, describe, expect, it } from "vitest";
import { HOST_TOKEN_ENV, PIPE_DAEMON_VERSION, THREAD_ENV, type DaemonFrame, type DaemonResponse, type TurnResult } from "@wsp/protocol";
import { createRuntime, type HarnessAdapterFactory, type Runtime } from "../src/runtime.js";
import type { DaemonChannel, DaemonChannelOptions } from "../src/daemon-channel.js";
import { memoryStore, type Store } from "../src/store.js";
import { createOn, projectOn, stubBackend, tokenGuest, withDaemonRoads } from "./stub-backend.js";
import { answering, box, HETZNER, type Box, type Started } from "./box-fixture.js";
import { ctx, sockets, code, join } from "./places-fixture.js";
import { report, wiring } from "./place-join.js";
import { newPlaceKeyPair } from "../src/places.js";
import { serveRuntime } from "../src/serve.js";

const DAEMON_TOKEN = "cafef00d".repeat(3);

/** The environment each adapter was made with, a turn's launch among them. */
const adapterEnvs: Readonly<Record<string, string>>[] = [];
/** A turn that answers at once, so a thread stands on the machine with no turn running. */
const answers: HarnessAdapterFactory = made => (adapterEnvs.push(made.env), {
  steers: false,
  start: ({ onEvent }) => {
    const sessionId = "44444444-4444-4444-8444-444444444444";
    const result: TurnResult = { status: "completed", text: "done" };
    onEvent({ type: "session.start", sessionId });
    onEvent({ type: "turn.done", sessionId, result });
    onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
    return { localId: sessionId, finished: Promise.resolve(result), interrupt: async () => {} };
  },
});

interface Daemon {
  open: (o: DaemonChannelOptions) => Promise<DaemonChannel>;
  frames: Record<string, unknown>[];
  /** Whether the token each frame carried named a device of the host's while the daemon answered it. */
  liveAtOp: boolean[];
  /** The name of that device, where there was one. */
  namesAtOp: string[];
  /** Pushes an event to every channel still open, as the daemon pushes a pty's exit. */
  push: (event: Record<string, unknown>) => void;
  /** The ptys the daemon holds, which a pty.list answers with. */
  ptys: Set<string>;
  /** Ops the daemon refuses, after asking about the token like any other. */
  refuses: Set<string>;
  /** What a pty.create waits on before the daemon answers it, where set. */
  gate: { create?: Promise<void> };
}

/** A daemon of `version` that says hello as each channel opens and answers what a machine would. */
function fakeDaemon(live: () => Runtime, version = PIPE_DAEMON_VERSION): Daemon {
  const frames: Record<string, unknown>[] = [];
  const liveAtOp: boolean[] = [];
  const namesAtOp: string[] = [];
  const listeners = new Set<(event: Record<string, unknown>) => void>();
  const ptys = new Set<string>();
  const refuses = new Set<string>();
  const gate: { create?: Promise<void> } = {};
  let made = 0;
  const answer = (frame: Record<string, unknown>): Record<string, unknown> => {
    switch (frame["op"]) {
      case "git.push":
        return { branch: "pricing-page", base: String(frame["base"] ?? ""), remote: "origin", ahead: 1, uncommitted: 0, stat: [] };
      case "git.pr":
        return { pr: { number: 7, url: "https://github.com/acme/lab/pull/7", state: "open", host: "github.com", draft: false, base: "main", branch: "pricing-page", headOid: "abc1234", headSubject: "Work", mergeable: "unknown", mergeState: "unknown", review: "none", checks: [], additions: 1, deletions: 0, changedFiles: 1, commits: 1 }, created: true };
      case "pty.create": {
        const ptyId = `p${++made}`;
        ptys.add(ptyId);
        return { ptyId, pid: 4000 + made };
      }
      case "pty.kill":
        ptys.delete(String(frame["ptyId"]));
        return {};
      case "pty.list":
        return { ptys: [...ptys].map(id => ({ id })) };
      default:
        return {};
    }
  };
  return {
    frames,
    liveAtOp,
    namesAtOp,
    ptys,
    refuses,
    gate,
    push: event => {
      for (const fn of listeners) fn(event);
    },
    open: async o => {
      const heard = (event: Record<string, unknown>): void => o.onEvent?.(event);
      listeners.add(heard);
      // The hello follows the auth reply, which is what resolves the dial.
      setImmediate(() => o.onEvent?.({ type: "daemon.hello", version }));
      return {
        send: async (frame: DaemonFrame) => {
          const held = frame as unknown as Record<string, unknown>;
          frames.push(held);
          if (held["op"] === "pty.create") await gate.create;
          const env = held["env"] as Record<string, string> | undefined;
          const token = typeof held["token"] === "string" ? held["token"] : env?.[HOST_TOKEN_ENV];
          if (token !== undefined) {
            const device = await live().devices!.match(token);
            liveAtOp.push(device !== undefined);
            if (device !== undefined) namesAtOp.push(device.name);
          }
          if (refuses.has(String(held["op"]))) return { id: 1, ok: false, error: `${String(held["op"])} was refused` };
          return { id: 1, ok: true, ...answer(held) } as DaemonResponse;
        },
        close: () => listeners.delete(heard),
        closed: new Promise(() => {}),
      };
    },
  };
}

/** The branch a brokered create would otherwise read with gh on this computer, which no runtime here wires. */
const ofRecord = (branch: string) => ({ repoId: 4242, owner: "acme", name: "lab", branch });

let rt: Runtime | undefined;
afterEach(async () => {
  await rt?.close();
  rt = undefined;
});

/** A runtime whose broker reads `ready`, with one fork at its own provider and a thread on it. */
async function brokeredFork(o: { ready?: () => boolean; version?: number; store?: Store; noThread?: true } = {}): Promise<{ daemon: Daemon; id: string; threadId: string; folder: string; store: Store; backend: ReturnType<typeof stubBackend>; projectId: string }> {
  const backend = stubBackend();
  backend.execImpl = tokenGuest;
  withDaemonRoads(backend);
  const daemon = fakeDaemon(() => rt!, o.version);
  const store = o.store ?? memoryStore();
  rt = createRuntime({ backend, store, adapters: { claude: answers }, daemonToken: DAEMON_TOKEN, daemonChannel: daemon.open, broker: { ready: o.ready ?? (() => true), codexClaims: async () => undefined } });
  const project = await projectOn(rt, undefined, undefined, { base: "main" });
  const ws = await createOn(rt, { project: project.id, golden: "snap_g", name: "pricing page", branchOfRecord: ofRecord("wsp/pricing-page") });
  if (o.noThread === true) return { daemon, id: ws.id, threadId: "", folder: "", store, backend, projectId: project.id };
  const turn = await rt.sessions.start(ws.id, { prompt: "build the pricing page" });
  await turn.finished;
  // Where the thread works, as its turn's own reads of the checkout named it.
  const folder = String(daemon.frames.find(f => typeof f["cwd"] === "string")?.["cwd"]);
  return { daemon, id: ws.id, threadId: turn.view().threadId!, folder, store, backend, projectId: project.id };
}

const tokensOf = (frames: Record<string, unknown>[]): unknown[] => frames.map(f => f["token"]);
/** The bring back's two ops, which reach the git host, out of every frame the daemon got. */
const bringBackOps = (frames: Record<string, unknown>[]): Record<string, unknown>[] => frames.filter(f => f["op"] === "git.push" || f["op"] === "git.pr");

describe("an op token on a machine whose credentials the host keeps", () => {
  it("rides each git op as a fresh token, live while the daemon answers it and taken away at its reply", async () => {
    const { daemon, id, threadId } = await brokeredFork();
    await rt!.workspaces.bringBack({ workspaceId: id, title: "the pricing page" });
    const ops = bringBackOps(daemon.frames);
    expect(ops.map(f => f["op"])).toEqual(["git.push", "git.pr"]);
    const [push, pr] = tokensOf(ops) as string[];
    expect(typeof push).toBe("string");
    expect(typeof pr).toBe("string");
    expect(push).not.toBe(pr);
    expect(daemon.liveAtOp).toEqual([true, true]);
    expect(daemon.namesAtOp).toEqual(["op git.push", "op git.pr"]);
    expect(await rt!.devices!.match(push!)).toBeUndefined();
    expect(await rt!.devices!.match(pr!)).toBeUndefined();
    // Scoped to this workspace's thread, which is what the broker matches against the link the pipe came on.
    const scopes = (await rt!.devices!.list()).flatMap(d => (d.scope === undefined ? [] : [d.scope]));
    expect(scopes.every(s => s.workspaceId === id && s.threadId === threadId)).toBe(true);
  });

  it("is taken away when the daemon refuses the op too", async () => {
    const { daemon, id } = await brokeredFork();
    daemon.refuses.add("git.push");
    await expect(rt!.workspaces.bringBack({ workspaceId: id })).rejects.toThrow("git.push was refused");
    const [push] = tokensOf(daemon.frames.filter(f => f["op"] === "git.push")) as string[];
    expect(daemon.liveAtOp).toEqual([true]);
    expect(await rt!.devices!.match(push!)).toBeUndefined();
  });

  it("is not minted while the broker is not ready, nor for a daemon from before op tokens", async () => {
    const off = await brokeredFork({ ready: () => false });
    await rt!.workspaces.bringBack({ workspaceId: off.id });
    expect(tokensOf(bringBackOps(off.daemon.frames))).toEqual([undefined, undefined]);
    await rt!.close();
    const old = await brokeredFork({ version: PIPE_DAEMON_VERSION - 1 });
    await rt!.workspaces.bringBack({ workspaceId: old.id });
    expect(tokensOf(bringBackOps(old.daemon.frames))).toEqual([undefined, undefined]);
    expect((await rt!.devices!.list()).filter(d => d.scope !== undefined)).toEqual([]);
  });
});

describe("an op on a machine whose credentials the host keeps, with no thread on it yet", () => {
  it("is refused once the broker is ready, before anything reaches the daemon", async () => {
    const { daemon, id } = await brokeredFork({ noThread: true });
    const refused = await rt!.workspaces.bringBack({ workspaceId: id }).catch((e: unknown) => e as Error & { kind?: string });
    expect(refused).toBeInstanceOf(Error);
    expect((refused as Error).message).toBe("pricing page has no thread yet, so wsp cannot reach GitHub for it. Start a thread on it first.");
    expect((refused as { kind?: string }).kind).toBe("usage");
    expect(bringBackOps(daemon.frames)).toEqual([]);
  });

  it("runs as it always has while the broker is not ready", async () => {
    const { daemon, id } = await brokeredFork({ noThread: true, ready: () => false });
    await rt!.workspaces.bringBack({ workspaceId: id });
    expect(tokensOf(bringBackOps(daemon.frames))).toEqual([undefined, undefined]);
  });
});

describe("a joined computer's frames", () => {
  afterEach(async () => {
    for (const ws of sockets.splice(0)) ws.close();
    await ctx.srv?.close();
    ctx.srv = undefined;
    ctx.runtime = undefined;
  });

  it("carry no token while the broker is ready, from a thread in a folder there", async () => {
    const starts: Started[] = [];
    // The box fixture's own joined(), with a broker that is ready.
    const hostKey = newPlaceKeyPair();
    ctx.runtime = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: answering(starts) }, placeLinks: wiring(hostKey), broker: { ready: () => true, codexClaims: async () => undefined } });
    ctx.srv = await serveRuntime(ctx.runtime, { port: 0, authToken: "host-token", devices: ctx.runtime.devices });
    let seen!: Box;
    const { client } = await join(hostKey, { code: await code(), report: report("hetzner", { login: { HOME: HETZNER.home, USER: "root", PATH: "/usr/bin" } }), answers: c => (seen = box(c, HETZNER)) });
    sockets.push(client.ws);
    const project = await ctx.runtime.projects.add({ source: "https://github.com/spoo-me/spoo-ts", on: "hetzner", name: "spoo-ts" });
    const joined = { rt: ctx.runtime, project, seen };
    const folder = await joined.rt.workspaces.create({ project: joined.project.id, name: "spoo-ts" });
    await (await joined.rt.sessions.start(folder.id, { prompt: "fix the redirect" })).finished;
    await joined.rt.workspaces.bringBack({ workspaceId: folder.id }).catch(() => undefined);
    const channel = await joined.rt.workspaces.daemonChannel(folder.id, () => {});
    await channel.send({ op: "pty.create", cols: 80, rows: 24 } as never);
    channel.close();
    const push = joined.seen.frames.filter(f => f["op"] === "git.push");
    expect(push.length).toBe(1);
    expect(push[0]).not.toHaveProperty("token");
    const pty = joined.seen.frames.filter(f => f["op"] === "pty.create");
    expect(pty.length).toBe(1);
    expect(Object.keys((pty[0]!["env"] ?? {}) as Record<string, string>)).not.toContain(HOST_TOKEN_ENV);
    expect((await joined.rt.devices!.list()).filter(d => d.scope !== undefined && d.scope.workspaceId === folder.id)).toEqual([]);
  });
});

describe("a terminal token on a machine whose credentials the host keeps", () => {
  /** The pty.create frames the daemon got after a pane on the workspace sent each frame in `sent`, on one channel. */
  async function opened(daemon: Daemon, id: string, sent: Record<string, unknown>[]): Promise<{ frames: Record<string, unknown>[]; channel: DaemonChannel }> {
    const before = daemon.frames.length;
    const channel = await rt!.workspaces.daemonChannel(id, () => {});
    for (const frame of sent) await channel.send({ op: "pty.create", cols: 80, rows: 24, ...frame } as never);
    return { frames: daemon.frames.slice(before).filter(f => f["op"] === "pty.create"), channel };
  }
  const tokenIn = (frame: Record<string, unknown>): string => (frame["env"] as Record<string, string>)[HOST_TOKEN_ENV]!;

  it("opens the first tab in the thread's folder with a token of its own, and every later tab with another", async () => {
    const { daemon, id, threadId, folder } = await brokeredFork();
    const { frames, channel } = await opened(daemon, id, [{ thread: threadId }, {}]);
    channel.close();
    expect(frames.map(f => f["cwd"])).toEqual([folder, folder]);
    expect(frames[0]).not.toHaveProperty("thread");
    const [first, second] = frames.map(tokenIn);
    expect(typeof first).toBe("string");
    expect(first).not.toBe(second);
    expect((frames[0]!["env"] as Record<string, string>)[THREAD_ENV]).toBe(threadId);
    expect(daemon.liveAtOp).toEqual([true, true]);
    const device = await rt!.devices!.match(first!);
    expect(device?.scope).toEqual({ kind: "thread", threadId, workspaceId: id, rootThreadId: threadId });
    // A pane that goes leaves its shells running on the machine, and their tokens with them.
    expect(await rt!.devices!.match(second!)).toBeDefined();
  });

  it("carries the git and gh variables a turn's launch does, on the shell's own token", async () => {
    const { daemon, id, threadId } = await brokeredFork();
    const turn = [...adapterEnvs].reverse().find(env => env[HOST_TOKEN_ENV] !== undefined)!;
    const { frames, channel } = await opened(daemon, id, [{ thread: threadId }]);
    channel.close();
    const shell = frames[0]!["env"] as Record<string, string>;
    const gitAndGh = (env: Readonly<Record<string, string>>): Record<string, string> =>
      Object.fromEntries(Object.entries(env).filter(([name]) => /^(GIT_|GH_)/.test(name)).map(([name, value]) => [name, value.replaceAll(env[HOST_TOKEN_ENV]!, "<token>")]));
    expect(gitAndGh(turn)).toMatchObject({ GIT_TERMINAL_PROMPT: "0", GH_TOKEN: "<token>" });
    expect(gitAndGh(shell)).toEqual(gitAndGh(turn));
    expect(shell["GH_TOKEN"]).toBe(tokenIn(frames[0]!));
  });

  it("dies with its shell: at the exit the daemon pushes, at a kill, and at a list that no longer holds it", async () => {
    const { daemon, id } = await brokeredFork();
    const { frames, channel } = await opened(daemon, id, [{}, {}, {}]);
    const [exits, killed, vanished] = frames.map(tokenIn);
    const [exitsPty, killedPty, vanishedPty] = ["p1", "p2", "p3"];
    daemon.push({ type: "pty.exit", ptyId: exitsPty, code: 0 });
    await expect.poll(async () => rt!.devices!.match(exits!)).toBeUndefined();
    expect(await rt!.devices!.match(killed!)).toBeDefined();
    await channel.send({ op: "pty.kill", ptyId: killedPty } as never);
    expect(await rt!.devices!.match(killed!)).toBeUndefined();
    channel.close();
    // Gone while no pane listened: the next pane's list is what says so.
    daemon.ptys.delete(vanishedPty!);
    const next = await rt!.workspaces.daemonChannel(id, () => {});
    await next.send({ op: "pty.list" } as never);
    next.close();
    expect(await rt!.devices!.match(vanished!)).toBeUndefined();
  });

  it("outlives a host restart while its shell stands, with no turn running, and dies with the shell after", async () => {
    const store = memoryStore();
    const { daemon, id, backend } = await brokeredFork({ store });
    const { frames, channel } = await opened(daemon, id, [{}]);
    channel.close();
    const token = tokenIn(frames[0]!);
    await rt!.close();
    rt = createRuntime({ backend, store, adapters: { claude: answers }, daemonToken: DAEMON_TOKEN, daemonChannel: daemon.open, broker: { ready: () => true, codexClaims: async () => undefined } });
    // The boot has run, and with it the sweep of every token whose thread has no turn running.
    await rt.workspaces.list();
    expect(await rt.devices!.match(token)).toBeDefined();
    const pane = await rt.workspaces.daemonChannel(id, () => {});
    await pane.send({ op: "pty.list" } as never);
    pane.close();
    expect(await rt.devices!.match(token)).toBeDefined();
    daemon.ptys.clear();
    const next = await rt.workspaces.daemonChannel(id, () => {});
    await next.send({ op: "pty.list" } as never);
    next.close();
    expect(await rt.devices!.match(token)).toBeUndefined();
  });

  it("outlives a host restart that finds its thread's turn gone from the machine", async () => {
    const backend = stubBackend();
    backend.execImpl = tokenGuest;
    withDaemonRoads(backend);
    const daemon = fakeDaemon(() => rt!);
    // A turn still running when the host stops, whose run the machine no longer holds when the next host asks.
    const gone: HarnessAdapterFactory = () => ({
      steers: false,
      start: ({ onEvent }) => {
        queueMicrotask(() => onEvent({ type: "session.start", sessionId: "55555555-5555-4555-8555-555555555555" }));
        return { localId: "55555555-5555-4555-8555-555555555555", run: "/tmp/wsp-run/000000000001", finished: new Promise<TurnResult>(() => {}), interrupt: async () => {} };
      },
      attach: async () => "gone",
    });
    const o = { backend, store: memoryStore(), adapters: { claude: gone }, daemonToken: DAEMON_TOKEN, daemonChannel: daemon.open, broker: { ready: () => true, codexClaims: async () => undefined } };
    rt = createRuntime(o);
    const ws = await createOn(rt, { project: (await projectOn(rt, undefined, undefined, { base: "main" })).id, golden: "snap_g", name: "pricing page", branchOfRecord: ofRecord("wsp/pricing-page") });
    const turn = await rt.sessions.start(ws.id, { prompt: "build the pricing page" });
    await expect.poll(async () => (await rt!.sessions.history(ws.id)).some(e => e.type === "session.start")).toBe(true);
    const { frames, channel } = await opened(daemon, ws.id, [{ thread: turn.view().threadId }]);
    channel.close();
    await rt.close();
    rt = createRuntime(o);
    await rt.workspaces.list();
    await expect.poll(async () => (await rt!.sessions.list(ws.id))[0]?.status).toBe("failed");
    await expect.poll(async () => (await rt!.devices!.list()).filter(d => d.scope !== undefined).length).toBe(1);
    expect(await rt.devices!.match(tokenIn(frames[0]!))).toBeDefined();
  });

  it("is not minted while the broker is not ready", async () => {
    const { daemon, id, folder } = await brokeredFork({ ready: () => false });
    const { frames, channel } = await opened(daemon, id, [{}]);
    channel.close();
    expect(frames.map(f => f["cwd"])).toEqual([folder]);
    expect(frames[0]).not.toHaveProperty("env");
  });

  it("is taken away at once when the shell's open answers after its workspace was deleted, and no record is kept", async () => {
    const { daemon, id, store } = await brokeredFork();
    let answer!: () => void;
    daemon.gate.create = new Promise<void>(resolve => (answer = resolve));
    const channel = await rt!.workspaces.daemonChannel(id, () => {});
    const opening = channel.send({ op: "pty.create", cols: 80, rows: 24 } as never);
    await expect.poll(() => daemon.frames.some(f => f["op"] === "pty.create")).toBe(true);
    const token = tokenIn(daemon.frames.find(f => f["op"] === "pty.create")!);
    await rt!.workspaces.delete(id);
    answer();
    await opening;
    channel.close();
    expect(await rt!.devices!.match(token)).toBeUndefined();
    expect(await store.keys("pty-tokens")).not.toContain(id);
  });

  it("is scoped to this workspace's own thread when the pane names a thread of another workspace", async () => {
    const { daemon, id, threadId, projectId } = await brokeredFork();
    const other = await createOn(rt!, { project: projectId, golden: "snap_g", name: "checkout page", branchOfRecord: ofRecord("wsp/checkout-page") });
    const theirs = (await (async () => {
      const turn = await rt!.sessions.start(other.id, { prompt: "build the checkout page" });
      await turn.finished;
      return turn.view().threadId!;
    })());
    expect(theirs).not.toBe(threadId);
    const { frames, channel } = await opened(daemon, id, [{ thread: theirs }]);
    channel.close();
    const device = await rt!.devices!.match(tokenIn(frames[0]!));
    expect(device?.scope).toEqual({ kind: "thread", threadId, workspaceId: id, rootThreadId: threadId });
    expect((frames[0]!["env"] as Record<string, string>)[THREAD_ENV]).toBe(threadId);
  });

  it("goes with its workspace", async () => {
    const { daemon, id } = await brokeredFork();
    const { frames, channel } = await opened(daemon, id, [{}]);
    channel.close();
    const token = tokenIn(frames[0]!);
    await rt!.workspaces.delete(id);
    await expect.poll(async () => rt!.devices!.match(token)).toBeUndefined();
  });
});
