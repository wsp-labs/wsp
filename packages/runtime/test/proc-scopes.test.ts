// SPDX-License-Identifier: AGPL-3.0-only
// The threads the host names to a computer's daemon for the owner it stamps on each process row: what one thread's
// scope carries on the computer the host runs on and on a box, and that a watching channel names them before its
// proc.watch and again each time they move, and never while it does not watch; and that this computer's daemon is
// told whenever a thread here has a root, watched or not.
import { spawn, type ChildProcess } from "node:child_process";
import { afterEach, describe, expect, it } from "vitest";
import { threadCgroup, type DaemonFrame, type DaemonResponse } from "@wsp/protocol";
import { procScopesArea } from "../src/account/proc-scopes.js";
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
