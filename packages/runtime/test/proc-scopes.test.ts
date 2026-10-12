// SPDX-License-Identifier: AGPL-3.0-only
// The threads the host names to a computer's daemon for the owner it stamps on each process row: what one thread's
// scope carries on the computer the host runs on and on a box, and that a watching channel names them before its
// proc.watch and again each time they move, and never while it does not watch; and that this computer's daemon is
// told whenever a thread here has a root, watched or not.
import { spawn, type ChildProcess } from "node:child_process";
import { afterEach, describe, expect, it } from "vitest";
import { threadCgroup, type DaemonFrame, type DaemonResponse } from "@wsp/protocol";
import { channelsArea } from "../src/account/channels.js";
import { procScopesArea } from "../src/account/proc-scopes.js";
import { memoryStore } from "../src/store.js";
import { eventBus } from "../src/types/events.js";
import type { DaemonChannel } from "../src/daemon-channel.js";
import type { RuntimeContext } from "../src/context.js";

const children: ChildProcess[] = [];
afterEach(() => {
  for (const c of children.splice(0)) c.kill("SIGKILL");
});

/** A process leading a group of its own, as a turn's agent does. */
function leader(): number {
  const child = spawn("sleep", ["30"], { detached: true, stdio: "ignore" });
  children.push(child);
  return child.pid!;
}

function context(o: { local: string[]; box?: { workspace: string; place: string }; here?: () => DaemonChannel }) {
  const live = new Map<string, unknown>();
  for (const id of o.local) live.set(id, { record: { id, kind: "local", place: undefined, path: `/work/${id}` } });
  if (o.box !== undefined) live.set(o.box.workspace, { record: { id: o.box.workspace, kind: "box", place: o.box.place, path: `/box/${o.box.workspace}` } });
  const ctx = {
    live,
    sessions: new Map<string, unknown>(),
    keptAgents: new Map<string, unknown>(),
    turnGroups: new Map<string, Map<number, string>>(),
    threadRecords: new Map<string, { workspaceId: string }>(),
    latestOn: (threadId: string) => (threadId === "t_tree" ? { workspaceId: "ws_a", cwd: "/work/ws_a/.wsp/t_tree" } : undefined),
    checkoutOf: (record: { path: string }) => record.path,
    moduleOf: (kind: string) => ({ sharedDaemon: kind === "local" }),
    threadTerminals: () => [],
    ...(o.here !== undefined ? { localRoad: async () => ({ url: "ws://here" }), channelOver: async () => o.here!() } : {}),
  };
  const area = procScopesArea(ctx as unknown as RuntimeContext);
  Object.assign(ctx, area);
  return { ctx, area };
}

const session = (threadId: string, workspaceId: string, o: { pid?: number; status?: string; startedAt?: number } = {}) => ({
  view: { threadId, workspaceId, status: o.status ?? "running", startedAt: o.startedAt ?? 1 },
  turnId: `turn_${threadId}`,
  ...(o.pid !== undefined ? { pid: o.pid } : {}),
});

function fakeChannel(): { channel: DaemonChannel; sent: DaemonFrame[]; closes: number[] } {
  const sent: DaemonFrame[] = [];
  const closes: number[] = [];
  const channel: DaemonChannel = {
    send: async frame => {
      sent.push(frame);
      return { id: frame.id, ok: true } as DaemonResponse;
    },
    close: () => void closes.push(sent.length),
    closed: new Promise(() => undefined),
  };
  return { channel, sent, closes };
}

const settled = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0));

describe("the threads named to the daemon on the computer the host runs on", () => {
  it("carry each thread's agents, its turns' live groups, what the host started for it and its folder, and each folder's newest thread", () => {
    const { ctx, area } = context({ local: ["ws_a", "ws_b"] });
    const agent = leader();
    const kept = leader();
    const gone = 4_194_303;
    ctx.sessions.set("s1", session("t_tree", "ws_a", { pid: agent }));
    ctx.sessions.set("s2", session("t_old", "ws_b", { status: "completed", startedAt: 5 }));
    ctx.keptAgents.set("t_kept", { workspaceId: "ws_a", pid: kept });
    ctx.turnGroups.set("ws_a", new Map([[agent, "t_tree"], [gone, "t_tree"]]));
    area.ownProcess("t_tree", 777, "slate");
    area.ownProcess("t_tree", 778, "tool");
    expect(area.scopesHere()).toEqual([
      { thread: "t_tree", workspace: "ws_a", agents: [agent], groups: [agent], folder: "/work/ws_a/.wsp/t_tree", pids: [{ pid: 777, kind: "slate" }, { pid: 778, kind: "tool" }] },
      { thread: "t_kept", workspace: "ws_a", agents: [kept], folder: "/work/ws_a" },
      { thread: "t_old", workspace: "ws_b", folder: "/work/ws_b" },
    ]);
  });

  it("are named before a watch and again as they move, and not while the channel does not watch", async () => {
    const { area } = context({ local: ["ws_a"] });
    const { channel, sent } = fakeChannel();
    const scoped = area.scopedProcs(area.scopesHere, channel);
    const release = area.ownProcess("t_x", 900, "slate");
    expect(sent).toEqual([]);
    await scoped.send({ id: 1, op: "proc.watch" } as DaemonFrame);
    expect(sent.map(f => f.op)).toEqual(["proc.scopes", "proc.watch"]);
    expect((sent[0] as unknown as { scopes: unknown }).scopes).toEqual([{ thread: "t_x", pids: [{ pid: 900, kind: "slate" }] }]);
    area.procScopesMoved();
    expect(sent).toHaveLength(2);
    release();
    await Promise.resolve();
    expect(sent.map(f => f.op)).toEqual(["proc.scopes", "proc.watch", "proc.scopes"]);
    expect((sent[2] as unknown as { scopes: unknown }).scopes).toEqual([]);
    await scoped.send({ id: 2, op: "proc.unwatch" } as DaemonFrame);
    area.ownProcess("t_x", 901, "tool");
    await Promise.resolve();
    expect(sent.map(f => f.op)).toEqual(["proc.scopes", "proc.watch", "proc.scopes", "proc.unwatch"]);
  });
});

describe("a warm agent", () => {
  it("is named by the id its thread takes, as warm, until a send claims it", () => {
    const { ctx, area } = context({ local: ["ws_a"] });
    const kept = { workspaceId: "ws_a", warm: { harness: "claude", claimed: false } };
    ctx.keptAgents.set("t_warm", kept);
    expect(area.scopesHere().find(s => s.thread === "t_warm")).toEqual({ thread: "t_warm", workspace: "ws_a", folder: "/work/ws_a", warm: true });
    kept.warm.claimed = true;
    expect(area.scopesHere().find(s => s.thread === "t_warm")).toBeUndefined();
  });
});

describe("this computer's daemon, with no channel watching", () => {
  it("is told the scopes once a thread here has a root and each time they move, and the channel closes when none is left", async () => {
    const { channel, sent, closes } = fakeChannel();
    let opened = 0;
    const { area } = context({ local: ["ws_a"], here: () => (opened++, channel) });
    area.procScopesMoved();
    await settled();
    expect([opened, sent]).toEqual([0, []]);
    const release = area.ownProcess("t_x", 900, "slate");
    await settled();
    area.procScopesMoved();
    await settled();
    expect(opened).toBe(1);
    expect(sent).toEqual([{ id: null, op: "proc.scopes", scopes: [{ thread: "t_x", pids: [{ pid: 900, kind: "slate" }] }] }]);
    const second = area.ownProcess("t_x", 901, "tool");
    await settled();
    expect(sent).toHaveLength(2);
    second();
    release();
    await settled();
    expect([opened, sent.length, closes]).toEqual([1, 4, [4]]);
    expect(sent[3]).toEqual({ id: null, op: "proc.scopes", scopes: [] });
  });

  it("is never dialled by a host that wired no daemon here", async () => {
    const { area } = context({ local: ["ws_a"] });
    area.ownProcess("t_x", 900, "slate");
    await settled();
    expect(area.scopesHere()).toHaveLength(1);
  });
});

describe("the threads named to a box's daemon", () => {
  it("are every thread of every folder on that computer, newest first, each by its cgroup", () => {
    const { ctx, area } = context({ local: ["ws_a"], box: { workspace: "ws_box", place: "pl_1" } });
    ctx.sessions.set("s1", session("t_new", "ws_box", { startedAt: 9 }));
    ctx.sessions.set("s2", session("t_here", "ws_a", { startedAt: 10 }));
    ctx.threadRecords.set("t_quiet", { workspaceId: "ws_box" });
    expect(area.scopesOn("pl_1")).toEqual([
      { thread: "t_new", workspace: "ws_box", folder: "/box/ws_box", cgroup: threadCgroup("t_new") },
      { thread: "t_quiet", workspace: "ws_box", folder: "/box/ws_box", cgroup: threadCgroup("t_quiet") },
    ]);
  });
});

describe("a shell a thread's pane opened", () => {
  /** Both areas on one host, as the runtime wires them: the channels keep whose each shell is, the scopes name it. */
  function host() {
    const { ctx } = context({ local: ["ws_a"], box: { workspace: "ws_box", place: "pl_1" } });
    const { checkoutOf, latestOn } = ctx;
    Object.assign(ctx, { bus: eventBus(), store: memoryStore(), state: {} });
    Object.assign(ctx, channelsArea(ctx as unknown as RuntimeContext), { checkoutOf, latestOn });
    Object.assign(ctx, procScopesArea(ctx as unknown as RuntimeContext));
    return ctx as unknown as RuntimeContext;
  }
  /** A daemon whose shells lead the pids it hands out in turn. */
  function shells(...pids: number[]): DaemonChannel {
    let n = 0;
    return {
      send: async frame => (frame.op === "pty.create" ? { id: frame.id, ok: true, ptyId: `pty_${n + 1}`, pid: pids[n++] } : { id: frame.id, ok: true }) as never,
      close: () => {},
      closed: new Promise(() => {}),
    };
  }
  const opened = (ctx: RuntimeContext, workspaceId: string, daemon: DaemonChannel): DaemonChannel => {
    const ptys = new Map<string, number>();
    ctx.portPids.set(workspaceId, ptys);
    return ctx.rootedPorts(workspaceId, `/work/${workspaceId}`, ptys, ctx.heldToOwner(workspaceId, ctx.threadTagged(workspaceId, daemon)));
  };
  const terminalsOf = (scopes: readonly { thread: string; pids?: readonly { pid: number; kind: string }[] }[], thread: string) =>
    scopes.find(s => s.thread === thread)?.pids?.filter(p => p.kind === "terminal").map(p => p.pid) ?? [];

  it("is named its thread's terminal on this computer, follows the thread it is handed to, and a watching channel hears each move", async () => {
    const ctx = host();
    const { channel, sent } = fakeChannel();
    await ctx.scopedProcs(ctx.scopesHere, channel).send({ id: 1, op: "proc.watch" } as DaemonFrame);
    const pane = opened(ctx, "ws_a", shells(4242, 4343));
    await pane.send({ id: 2, op: "pty.create", thread: "th_a" } as never);
    await pane.send({ id: 3, op: "pty.create" } as never);
    expect(terminalsOf(ctx.scopesHere(), "th_a")).toEqual([4242]);
    expect(ctx.scopesHere().flatMap(s => s.pids ?? []).map(p => p.pid)).not.toContain(4343);
    await pane.send({ id: 4, op: "pty.thread", ptyId: "pty_1", thread: "th_b" } as never);
    await settled();
    expect([terminalsOf(ctx.scopesHere(), "th_a"), terminalsOf(ctx.scopesHere(), "th_b")]).toEqual([[], [4242]]);
    const told = sent.filter(f => f.op === "proc.scopes").at(-1) as unknown as { scopes: { thread: string; pids?: { pid: number; kind: string }[] }[] };
    expect(terminalsOf(told.scopes, "th_b")).toEqual([4242]);
  });

  it("is named to the box's daemon, never to this computer's, when the folder is on a box", async () => {
    const ctx = host();
    const pane = opened(ctx, "ws_box", shells(77));
    await pane.send({ id: 1, op: "pty.create", thread: "th_box" } as never);
    expect(terminalsOf(ctx.scopesOn("pl_1"), "th_box")).toEqual([77]);
    expect(ctx.scopesHere().flatMap(s => s.pids ?? []).map(p => p.pid)).not.toContain(77);
  });
});
