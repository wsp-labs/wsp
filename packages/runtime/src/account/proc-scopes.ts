// SPDX-License-Identifier: AGPL-3.0-only
// The threads whose processes run on a computer, as this host names them to
// that computer's daemon in proc.scopes, which stamps every row of proc.watch
// with its owner off them. On the computer the host runs on a thread is its
// agent processes, the groups its turns led and what the host itself started
// for it (slate runs, tool servers); on a box, its cgroup. The list is the
// whole computer's each time and goes before every proc.watch a channel sends,
// then again whenever it moves while that channel watches. This computer's
// daemon is also told whenever a thread here has a root, watched or not: a
// Mac's daemon follows each root's forks as they happen, since a Bash call's
// shell lives milliseconds and no later read sees it.
import { PROC_SCOPE_PIDS_MAX, PROC_SCOPES_MAX, THIS_COMPUTER, threadCgroup, type DaemonFrame, type ProcKind, type ProcScope } from "@wsp/protocol";
import { groupExists } from "../local-exec.js";
import type { DaemonChannel } from "../daemon-channel.js";
import type { RuntimeContext, ProcScopesArea } from "../context.js";

interface Draft {
  workspace?: string;
  agents: Set<number>;
  groups: Set<number>;
  pids: { pid: number; kind: ProcKind }[];
  /** A warm agent's, kept for a thread no send has claimed yet. */
  warm: boolean;
  /** Whether anything of the thread runs now, which puts it ahead of a folder's newest thread under the cap. */
  live: boolean;
}

export function procScopesArea(ctx: RuntimeContext): ProcScopesArea {
  /** The processes this host started for a thread itself, by pid, while each runs. */
  const owned = new Map<number, { threadId: string; kind: ProcKind }>();
  /** Each watching channel's push, which names the scopes again when they moved. */
  const pushes = new Set<() => void>();

  /** The channel that tells this computer's daemon the scopes while any thread here has a root, and what it said. */
  let standing: { channel: Promise<DaemonChannel | undefined>; told: string } | undefined;
  const standHere = (): void => {
    const now = scopesHere();
    if (!now.some(s => s.agents !== undefined || s.groups !== undefined || s.pids !== undefined)) {
      const was = standing;
      standing = undefined;
      void was?.channel.then(async c => {
        await c?.send({ id: null, op: "proc.scopes", scopes: now } as DaemonFrame).catch(() => undefined);
        c?.close();
      });
      return;
    }
    const said = JSON.stringify(now);
    if (standing?.told === said) return;
    if (standing === undefined) {
      const held: { channel: Promise<DaemonChannel | undefined>; told: string } = { channel: Promise.resolve(undefined), told: "" };
      held.channel = (async () => ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, () => {}))().then(
        c => {
          void c.closed.then(() => {
            if (standing === held) standing = undefined;
          });
          return c;
        },
        () => {
          if (standing === held) standing = undefined;
          return undefined;
        },
      );
      standing = held;
    }
    standing.told = said;
    void standing.channel.then(c => c?.send({ id: null, op: "proc.scopes", scopes: now } as DaemonFrame)).catch(() => undefined);
  };

  const procScopesMoved = (): void => {
    standHere();
    for (const push of pushes) push();
  };

  const ownProcess = (threadId: string, pid: number, kind: ProcKind): (() => void) => {
    owned.set(pid, { threadId, kind });
    procScopesMoved();
    return () => {
      if (owned.get(pid)?.threadId !== threadId) return;
      owned.delete(pid);
      procScopesMoved();
    };
  };

  const folderOf = (threadId: string, workspaceId: string | undefined): string | undefined => {
    const entry = workspaceId === undefined ? undefined : ctx.live.get(workspaceId);
    if (entry === undefined) return undefined;
    return ctx.latestOn(threadId)?.cwd ?? ctx.checkoutOf(entry.record);
  };

  /** Every thread with a process on the computer the host runs on, then each local folder's newest thread, so a
   * process left in a folder whose threads all ended still names its workspace. */
  const scopesHere = (): ProcScope[] => {
    const drafts = new Map<string, Draft>();
    const draft = (threadId: string, workspace: string | undefined, live: boolean): Draft => {
      let d = drafts.get(threadId);
      if (d === undefined) {
        d = { agents: new Set(), groups: new Set(), pids: [], warm: false, live };
        drafts.set(threadId, d);
      }
      d.workspace ??= workspace;
      d.live ||= live;
      return d;
    };
    for (const [workspaceId, groups] of ctx.turnGroups) {
      for (const [group, threadId] of groups) if (groupExists(group)) draft(threadId, workspaceId, true).groups.add(group);
    }
    for (const s of ctx.sessions.values()) {
      if (s.pid !== undefined && s.view.status === "running" && s.view.threadId !== undefined) draft(s.view.threadId, s.view.workspaceId, true).agents.add(s.pid);
    }
    // A warm agent holds no pid of its own here, and its processes carry the id its thread takes once a send claims it.
    for (const [threadId, kept] of ctx.keptAgents) {
      const warm = kept.warm?.claimed === false;
      if (kept.pid === undefined && !warm) continue;
      const d = draft(threadId, kept.workspaceId, true);
      if (kept.pid !== undefined) d.agents.add(kept.pid);
      d.warm ||= warm;
    }
    for (const [pid, o] of owned) draft(o.threadId, ctx.latestOn(o.threadId)?.workspaceId ?? ctx.threadRecords.get(o.threadId)?.workspaceId, true).pids.push({ pid, kind: o.kind });
    const newest = new Map<string, { threadId: string; at: number }>();
    for (const { view } of ctx.sessions.values()) {
      const entry = ctx.live.get(view.workspaceId);
      if (view.threadId === undefined || entry === undefined || !ctx.moduleOf(entry.record.kind).sharedDaemon) continue;
      const at = view.startedAt ?? 0;
      if ((newest.get(view.workspaceId)?.at ?? -1) < at) newest.set(view.workspaceId, { threadId: view.threadId, at });
    }
    for (const [workspaceId, { threadId }] of newest) draft(threadId, workspaceId, false);
    const scopes = [...drafts].sort(([, a], [, b]) => Number(b.live) - Number(a.live)).slice(0, PROC_SCOPES_MAX);
    return scopes.map(([thread, d]) => {
      const folder = folderOf(thread, d.workspace);
      const cap = <T>(list: Iterable<T>): T[] => [...list].slice(0, PROC_SCOPE_PIDS_MAX);
      return {
        thread,
        ...(d.workspace !== undefined ? { workspace: d.workspace } : {}),
        ...(d.agents.size > 0 ? { agents: cap(d.agents) } : {}),
        ...(d.groups.size > 0 ? { groups: cap(d.groups) } : {}),
        ...(folder !== undefined ? { folder } : {}),
        ...(d.warm ? { warm: true } : {}),
        ...(d.pids.length > 0 ? { pids: cap(d.pids) } : {}),
      };
    });
  };

  /** Every thread of every folder on a computer the person joined, newest first, each by the cgroup its turns stand in
   * there. */
  const scopesOn = (placeId: string): ProcScope[] => {
    const at = new Map<string, { workspace: string; folder: string; startedAt: number }>();
    const consider = (threadId: string, workspaceId: string, startedAt: number): void => {
      const entry = ctx.live.get(workspaceId);
      if (entry === undefined || entry.record.place !== placeId) return;
      if ((at.get(threadId)?.startedAt ?? -1) >= startedAt) return;
      at.set(threadId, { workspace: workspaceId, folder: ctx.latestOn(threadId)?.cwd ?? ctx.checkoutOf(entry.record), startedAt });
    };
    for (const { view } of ctx.sessions.values()) if (view.threadId !== undefined) consider(view.threadId, view.workspaceId, view.startedAt ?? 0);
    for (const [threadId, record] of ctx.threadRecords) if (!at.has(threadId)) consider(threadId, record.workspaceId, 0);
    return [...at]
      .sort(([, a], [, b]) => b.startedAt - a.startedAt)
      .slice(0, PROC_SCOPES_MAX)
      .map(([thread, t]) => ({ thread, workspace: t.workspace, folder: t.folder, cgroup: threadCgroup(thread) }));
  };

  /** A channel whose proc.watch goes after the scopes, and which names them again while it watches whenever they
   * move. A daemon from before proc.scopes refuses it, and its rows carry no owner. */
  const scopedProcs = (scopes: () => ProcScope[], channel: DaemonChannel): DaemonChannel => {
    /** What this channel last told the daemon, undefined while it does not watch. */
    let told: string | undefined;
    const tell = async (): Promise<void> => {
      const now = scopes();
      const said = JSON.stringify(now);
      if (said === told) return;
      told = said;
      await channel.send({ id: null, op: "proc.scopes", scopes: now } as DaemonFrame).catch(() => undefined);
    };
    const push = (): void => {
      if (told !== undefined) void tell();
    };
    pushes.add(push);
    const stop = (): void => void pushes.delete(push);
    void channel.closed.then(stop, stop);
    return {
      async send(frame) {
        if (frame.op === "proc.watch") {
          told = "";
          await tell();
        }
        if (frame.op === "proc.unwatch") told = undefined;
        return channel.send(frame);
      },
      close: () => {
        stop();
        channel.close();
      },
      closed: channel.closed,
    };
  };

  return { ownProcess, procScopesMoved, scopesHere, scopesOn, scopedProcs };
}
