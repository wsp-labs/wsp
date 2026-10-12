// SPDX-License-Identifier: AGPL-3.0-only
// Activity for the command line and the tools: this computer's processes and a
// box's by the thread that started them, and the End of one here by the pid,
// start and identity it was listed with. Each read opens its own socket to the
// computer's daemon, or a channel over a box's link, takes one whole snapshot
// and the listeners, and closes it, so nothing is read while nobody asks. A
// thread here is named to the read by its running turn's process and the agent
// it keeps, a box's by its cgroup there; the daemon's own owner on a row, where
// it names one, places the rest.
import { CATALOG_AGENTS } from "@wsp/catalog";
import {
  BOX_BEHIND_ACTIVITY_FIX,
  BOX_END_FIX,
  CLOUD_ACTIVITY_FIX,
  boxBehindActivityLine,
  notJoinedActivityFix,
  notJoinedActivityLine,
  NOT_READ_FIX,
  boxEndLine,
  cloudActivityLine,
  notReadLine,
  endHeads,
  HERE_PLACE_ID,
  THIS_COMPUTER,
  activityOf,
  endPlan,
  folderOnJoined,
  isLocalWorkspace,
  noSuchPlaceRefusal,
  notFoundRefusal,
  ProcKillReply,
  ProcListReply,
  ProcSnapshot,
  SAME_START_MS,
  scopeOf,
  threadWord,
  usageRefusal,
  type ActivityEnded,
  type ActivityProc,
  type ActivityThread,
  type LeftThread,
  type ActivityRead,
  type ActivityScope,
  type ActivityView,
  type Caller,
  type ProcessesAsk,
  type ProcEntry,
  type WorkspaceKind,
  processesLeft,
  processesLeftRefusal,
  threadCgroup,
  unknownOpLine,
} from "@wsp/protocol";
import { agentPidOf } from "./local-exec.js";
import { DaemonRefusal } from "./types/internal.js";
import type { LiveWorkspace } from "./types/wiring.js";
import type { DaemonChannel } from "./daemon-channel.js";
import type { RuntimeContext } from "./context.js";

/** How long a read waits for the daemon's first snapshot, which comes one sampling interval after the watch. */
const SNAPSHOT_WAIT_MS = 10_000;

export const NO_DAEMON_HERE = "this host runs no daemon on this computer, so it reads no processes";

export interface ActivityDoor {
  /** A computer's threads, this one's or a box's, one thread's alone wherever it runs, or those of threadIds this
   * computer runs. */
  read(o: { computer?: string; thread?: string; threadIds?: readonly string[] }, caller?: Caller): Promise<ActivityView>;
  end(o: { computer?: string; pid: number; startedAt: number }, caller?: Caller): Promise<ActivityEnded>;
  /** Before a settle or a delete takes these threads: ask refuses while one runs a process that outlives its agent,
   * naming each; stop ends each by the pid and start just read, the top of each tree taking what is under it; keep
   * reads nothing. This computer's threads and a box's are read, each computer once, never waiting on the daemon's sampler:
   * a box thread's are what stands in its cgroup there, and stop ends that cgroup. A cloud machine's thread is not read. */
  meet(threadIds: readonly string[], processes: ProcessesAsk, verb: "settle" | "delete", caller?: Caller): Promise<ActivityEnded[]>;
}

export interface ActivityArea {
  readonly activity: ActivityDoor;
}

/** What one proc.kill of a tree answered: every pid it signalled, those KILL went to after TERM's grace, and those still
 * there after that. */
interface Killed {
  signalled: number[];
  forced: number[];
  left: number[];
}

/** The computer a read or an End is on: this one, or a box the person joined, by its place id. */
type Where = { name: string; placeId?: string };

export function activityDoor(ctx: RuntimeContext): ActivityDoor {
  /** The computer named: this one where none is, else a box the person joined. */
  const where = async (computer: string | undefined): Promise<Where> => {
    const name = ctx.placeDoor?.nameOf(HERE_PLACE_ID) ?? THIS_COMPUTER;
    if (computer === undefined || computer === HERE_PLACE_ID || computer.toLowerCase() === name.toLowerCase() || computer === THIS_COMPUTER) return { name };
    const rows = ctx.placeDoor === undefined ? [] : await ctx.placeDoor.rows();
    const row = rows.find(r => r.id === computer || r.name.toLowerCase() === computer.toLowerCase());
    if (row === undefined) throw usageRefusal(noSuchPlaceRefusal(computer, rows.map(r => r.name)), "Name one wsp computers lists.");
    if (ctx.placeDoor?.joined(row.id) !== true) throw usageRefusal(notReadLine(row.name), NOT_READ_FIX);
    return { name: row.name, placeId: row.id };
  };
  /** An End is on this computer only: a box ends a thread's processes through its cgroup there. */
  const here = async (computer: string | undefined): Promise<string> => {
    const at = await where(computer);
    if (at.placeId !== undefined) throw usageRefusal(boxEndLine(at.name), BOX_END_FIX);
    return at.name;
  };

  type Open = (onEvent: (event: Record<string, unknown>) => void) => Promise<DaemonChannel>;
  const localChannel: Open = async onEvent => {
    if (ctx.opts.local?.daemonRoad === undefined) throw new Error(NO_DAEMON_HERE);
    return ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, onEvent);
  };

  /** One whole snapshot of this computer's processes and its listeners by pid, on a socket of its own. Once takes the
   * rows at once, with no cpu where no sampler runs, instead of the watch's first snapshot a sampling interval on;
   * a daemon too old for the list is watched, unless watch is false. Without ports the listeners are not read. */
  const snapshot = async (o: { once?: boolean; ports?: boolean; watch?: boolean } = {}, open: Open = localChannel): Promise<{ snap: ProcSnapshot; ports: Map<number, number[]> }> => {
    let take: (snap: ProcSnapshot) => void = () => {};
    const got = new Promise<ProcSnapshot>(resolve => (take = resolve));
    const channel = await open(event => {
      if (event["type"] !== "proc.snapshot") return;
      const read = ProcSnapshot.safeParse(event);
      if (read.success) take(read.data);
    });
    return ctx.overChannel(channel, async ask => {
      const listening = o.ports === false ? {} : await ask({ op: "ports.watch" });
      const ports = new Map<number, number[]>();
      for (const row of Array.isArray(listening["ports"]) ? (listening["ports"] as { port?: unknown; pid?: unknown }[]) : []) {
        if (typeof row.pid !== "number" || typeof row.port !== "number") continue;
        ports.set(row.pid, [...(ports.get(row.pid) ?? []), row.port].sort((a, b) => a - b));
      }
      if (o.once === true) {
        const listed = await ask({ op: "proc.list" }).catch((e: unknown) => {
          if (e instanceof Error && e.message === unknownOpLine("proc.list")) return undefined;
          throw e;
        });
        if (listed !== undefined) return { snap: { type: "proc.snapshot" as const, ...ProcListReply.parse(listed) }, ports };
      }
      if (o.watch === false) throw new Error(unknownOpLine("proc.list"));
      await ask({ op: "proc.watch" });
      let timer: ReturnType<typeof setTimeout> | undefined;
      const late = new Promise<never>((_, reject) => (timer = setTimeout(() => reject(new Error(`${THIS_COMPUTER}'s daemon sent no processes in ${SNAPSHOT_WAIT_MS / 1000} s`)), SNAPSHOT_WAIT_MS)));
      try {
        return { snap: await Promise.race([got, late]), ports };
      } finally {
        clearTimeout(timer);
      }
    });
  };

  /** The command a thread's agent runs as, which the read finds its agent by. */
  const binOf = (threadId: string): { bin?: string } => {
    const bin = CATALOG_AGENTS.find(a => a.id === ctx.threadFacts(threadId)?.harness)?.bin;
    return bin === undefined ? {} : { bin };
  };

  /** Each thread on this computer: its roots as the daemon is told them (`ctx.scopesHere`, the one rule), with the agent
   * it keeps for its next turn while no turn runs, then every other thread a local folder ran, rootless, so what a
   * finished thread left still lists under it; running threads first, then newest first. A warm agent no send has
   * claimed names no thread yet. */
  const scopesHere = (): ActivityScope[] => {
    const running = new Set<string>();
    const latest = new Map<string, { workspaceId: string; at: number }>();
    /** The agent each running turn's run recorded, the read's first word on which process is the agent. */
    const recorded = new Map<string, number[]>();
    for (const { view, run } of ctx.sessions.values()) {
      const record = ctx.live.get(view.workspaceId)?.record;
      if (view.threadId === undefined || record === undefined || !isLocalWorkspace(record)) continue;
      if (view.status === "running") running.add(view.threadId);
      const agent = view.status === "running" && run !== undefined ? agentPidOf(run) : undefined;
      if (agent !== undefined) recorded.set(view.threadId, [...(recorded.get(view.threadId) ?? []), agent]);
      const at = view.startedAt ?? 0;
      if ((latest.get(view.threadId)?.at ?? -1) <= at) latest.set(view.threadId, { workspaceId: view.workspaceId, at });
    }
    const titled = (threadId: string): string => ctx.threadFacts(threadId)?.title ?? threadWord(threadId);
    const scopes: ActivityScope[] = [];
    for (const s of ctx.scopesHere()) {
      if (s.warm === true) continue;
      const kept = running.has(s.thread) ? undefined : ctx.keptAgents.get(s.thread)?.pid;
      const roots = [...new Set([...(s.agents ?? []), ...(s.groups ?? [])])];
      const workspaceId = s.workspace ?? latest.get(s.thread)?.workspaceId ?? "";
      const agents = recorded.get(s.thread);
      scopes.push({ threadId: s.thread, title: titled(s.thread), workspaceId, roots, ...(kept !== undefined ? { kept } : {}), ...(agents !== undefined ? { agents } : {}), ...binOf(s.thread) });
    }
    const named = new Set(scopes.map(s => s.threadId));
    for (const [threadId, t] of latest) if (!named.has(threadId)) scopes.push({ threadId, title: titled(threadId), workspaceId: t.workspaceId, roots: [], ...binOf(threadId) });
    const started = (threadId: string): number => latest.get(threadId)?.at ?? 0;
    return scopes.sort((a, b) => Number(running.has(b.threadId)) - Number(running.has(a.threadId)) || started(b.threadId) - started(a.threadId));
  };

  const readOf = async (computer: string, once = false): Promise<ActivityRead> => {
    const { snap, ports } = await snapshot({ once });
    return { computer, at: snap.at, procs: snap.procs, daemon: snap.daemon, host: process.pid, scopes: scopesHere(), agentBins: CATALOG_AGENTS.map(a => a.bin), ports };
  };

  /** Whether a caller may read or end what a thread runs, by the one rule for a workspace (`refusalFor`) and a thread's
   * reach under it: a person any thread's, a thread its own and those under it. What runs with no thread is the
   * person's alone, on a computer they drive, which a relayed caller's road may not. */
  const reaches = (threadId: string | undefined, caller: Caller | undefined, kind: WorkspaceKind = "local"): boolean => {
    if (threadId === undefined) return scopeOf(caller) === undefined && ctx.drives({ kind }, caller);
    const record = ctx.live.get(ctx.threadRecords.get(threadId)?.workspaceId ?? ctx.latestOn(threadId)?.workspaceId ?? "")?.record;
    const driven = record === undefined ? ctx.drives({ kind }, caller) : ctx.refusalFor(record, caller) === undefined;
    return driven && ctx.settlesThread(threadId, caller);
  };

  /** The box a thread runs on, where its processes stand in its cgroup there; a cloud machine's thread has none. */
  const boxOf = (id: string): { placeId: string; entry: LiveWorkspace } | undefined => {
    const entry = ctx.live.get(ctx.threadRecords.get(id)?.workspaceId ?? ctx.latestOn(id)?.workspaceId ?? "");
    const placeId = entry?.record.place;
    return entry === undefined || isLocalWorkspace(entry.record) || placeId === undefined || ctx.placeDoor?.joined(placeId) !== true ? undefined : { placeId, entry };
  };

  /** One read of a box's processes by its threads' cgroups, once, over the link the host holds; refused while the box
   * is away, and in the usage class on a box one version behind, whose daemon has no one-shot list. */
  const boxRead = async (placeId: string, threadIds: readonly string[]): Promise<{ read: ActivityRead; view: ActivityView }> => {
    const door = ctx.placeDoor;
    const away = (): Error => new Error(`${door?.nameOf(placeId) ?? placeId} is away`);
    if (door === undefined || door.link(placeId) === undefined) throw away();
    // The box's link is one socket with one process watch its panes share, which closing this channel would not end.
    const { snap, ports } = await snapshot({ once: true, watch: false }, async onEvent => door.channel(placeId, onEvent) ?? Promise.reject(away())).catch((e: unknown) => {
      if (e instanceof Error && e.message === unknownOpLine("proc.list")) throw usageRefusal(boxBehindActivityLine(door.nameOf(placeId)), BOX_BEHIND_ACTIVITY_FIX);
      throw e;
    });
    const scopes = threadIds.map(id => ({ threadId: id, title: ctx.threadFacts(id)?.title ?? threadWord(id), workspaceId: boxOf(id)?.entry.record.id ?? "", roots: [], cgroup: threadCgroup(id), ...binOf(id) }));
    const read: ActivityRead = { computer: door.nameOf(placeId), at: snap.at, procs: snap.procs, daemon: snap.daemon, host: 0, scopes, agentBins: CATALOG_AGENTS.map(a => a.bin), ports, ...(snap.cgroups === undefined ? {} : { cgroups: snap.cgroups }) };
    return { read, view: activityOf(read) };
  };
  /** The same for a settle or a delete: nothing while the box is away or its daemon fails the read, since nothing there
   * can be read or ended then, and a box's fault never holds a settle here. */
  const boxReadQuiet = (placeId: string, threadIds: readonly string[]): Promise<{ read: ActivityRead; view: ActivityView } | undefined> =>
    boxRead(placeId, threadIds).catch((e: unknown) => {
      console.warn(`${ctx.placeDoor?.nameOf(placeId) ?? placeId}'s processes were not read for a settle or a delete: ${e instanceof Error ? e.message : String(e)}`);
      return undefined;
    });

  /** Every thread a box runs, newest first. */
  const threadsOn = (placeId: string): string[] => {
    const latest = new Map<string, number>();
    for (const { view } of ctx.sessions.values()) {
      if (view.threadId === undefined || boxOf(view.threadId)?.placeId !== placeId) continue;
      latest.set(view.threadId, Math.max(latest.get(view.threadId) ?? 0, view.startedAt ?? 0));
    }
    return [...latest].sort((a, b) => b[1] - a[1]).map(([id]) => id);
  };

  /** Ends a box thread's cgroup through the end that box already runs, then reads again what is still there, each
   * process by the start it was listed with. */
  const endOnBox = async (placeId: string, entry: LiveWorkspace, t: ActivityThread): Promise<ActivityEnded> => {
    await ctx.moduleOf(entry.record.kind).endThread?.(entry, t.threadId, {});
    const after = await boxReadQuiet(placeId, [t.threadId]);
    const left = t.processes.filter(p => after === undefined || after.read.procs.some(q => q.pid === p.pid && Math.abs(q.startedAt - p.startedAt) <= SAME_START_MS)).map(p => p.pid);
    return { pid: t.processes[0]!.pid, ended: t.processes, forced: [], left };
  };

  const elsewhere = (id: string): boolean => {
    const record = ctx.live.get(ctx.threadRecords.get(id)?.workspaceId ?? ctx.latestOn(id)?.workspaceId ?? "")?.record;
    return record !== undefined && !isLocalWorkspace(record);
  };
  /** The box a thread runs on that this host no longer has joined, by its place id. A machine on any provider but the
   * one this host forks on names that provider's place too, which is never joined. */
  const unjoinedBoxOf = (id: string): string | undefined => {
    const record = ctx.live.get(ctx.threadRecords.get(id)?.workspaceId ?? ctx.latestOn(id)?.workspaceId ?? "")?.record;
    return record === undefined || !folderOnJoined(record.kind) || record.place === undefined || ctx.placeDoor?.joined(record.place) === true ? undefined : record.place;
  };

  /** What read answers, off a read already taken or a fresh one: this computer's, or a box's when the computer or the
   * one thread asked for is there. */
  const viewOf = async (o: { computer?: string; thread?: string; threadIds?: readonly string[] }, caller: Caller | undefined, once: boolean): Promise<{ view: ActivityView; read: ActivityRead }> => {
    await ctx.ready();
    const asked = o.thread !== undefined ? [o.thread] : o.threadIds;
    for (const id of asked ?? []) {
      if ((ctx.threadRecords.get(id) === undefined && ctx.threadFacts(id) === undefined) || !reaches(id, caller)) throw notFoundRefusal(`no thread ${threadWord(id)}`);
    }
    const box = o.thread === undefined ? undefined : boxOf(o.thread);
    const unjoined = o.thread === undefined || box !== undefined ? undefined : unjoinedBoxOf(o.thread);
    if (o.thread !== undefined && unjoined !== undefined) {
      const name = ctx.placeDoor?.nameOf(unjoined) ?? unjoined;
      throw usageRefusal(notJoinedActivityLine(o.thread, name), notJoinedActivityFix(name));
    }
    if (o.thread !== undefined && box === undefined && elsewhere(o.thread)) throw usageRefusal(cloudActivityLine(o.thread), CLOUD_ACTIVITY_FIX);
    const named = await where(o.computer);
    const at: Where = o.computer === undefined && box !== undefined ? { name: ctx.placeDoor?.nameOf(box.placeId) ?? box.placeId, placeId: box.placeId } : named;
    if (o.thread !== undefined && at.placeId !== box?.placeId) throw notFoundRefusal(`no thread ${threadWord(o.thread)} on ${at.name}`);
    const onBox = at.placeId !== undefined;
    const wanted = asked === undefined ? undefined : new Set(asked.filter(id => (onBox ? boxOf(id)?.placeId === at.placeId : !elsewhere(id))));
    const { read, view } = onBox ? await boxRead(at.placeId!, [...(wanted ?? threadsOn(at.placeId!))]) : await (async () => {
      const r = await readOf(at.name, once);
      return { read: r, view: activityOf(r) };
    })();
    const kind = onBox ? "place" : "local";
    const threads = view.threads.filter(t => (wanted === undefined || wanted.has(t.threadId)) && reaches(t.threadId, caller, kind));
    // What runs outside the asked threads is the person's to read: a thread sees no command line but its tree's.
    const whole = wanted === undefined && reaches(undefined, caller, kind);
    return { view: { ...view, threads, wsp: whole ? view.wsp : [], unowned: whole ? view.unowned : [] }, read };
  };

  /** Ends each head and everything under it, one proc.kill each, by the start and the identity it was read with, so a
   * pid handed on since is never signalled. The daemon gives TERM its grace and sends KILL itself, all heads at once. */
  const killTrees = async (heads: readonly ProcEntry[]): Promise<Killed[]> => {
    if (ctx.opts.local?.daemonRoad === undefined) throw new Error(NO_DAEMON_HERE);
    // Through the guard the pane's road has, so the daemon checks the host's lists at the signal too.
    return ctx.overChannel(ctx.killGuarded(await ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, () => {}), true), ask =>
      Promise.all(
        heads.map(async (p): Promise<Killed> => {
          const reply = await ask({ op: "proc.kill", pid: p.pid, signal: "TERM", mode: "tree", startedAt: p.startedAt, ...(p.id !== undefined ? { identity: p.id } : {}) }).catch((e: unknown) => {
            // Gone on its own since the read: the end wanted.
            if (!(e instanceof DaemonRefusal && e.code === "not-found")) throw e;
            return undefined;
          });
          const r = reply === undefined ? {} : ProcKillReply.parse(reply);
          // A daemon from before kill modes signals the pid alone and answers nothing more.
          return { signalled: r.signalled ?? (reply === undefined ? [] : [p.pid]), forced: r.forced ?? [], left: r.left ?? [] };
        }),
      ),
    );
  };

  /** What an End did, said by the rows it was read with. */
  const endedOf = (view: ActivityView, plan: readonly ProcEntry[], pid: number, done: Killed): ActivityEnded => {
    const rows = [...view.threads.flatMap(t => t.processes), ...view.unowned];
    const rowOf = (q: ProcEntry): ActivityProc => rows.find(r => r.pid === q.pid) ?? { pid: q.pid, ppid: q.ppid, depth: 0, startedAt: q.startedAt, kind: "other" as const, name: q.comm, command: q.cmdline === "" ? q.comm : q.cmdline, memory: q.rss, cpu: q.cpu, ports: [] };
    const signalled = new Set(done.signalled);
    return { pid, ended: plan.filter(q => signalled.has(q.pid)).map(rowOf), forced: [...new Set(done.forced)], left: [...new Set(done.left)] };
  };

  /** An End off a read already taken: the plan is made on it, before any signal. */
  const endOn = async (read: ActivityRead, o: { pid: number; startedAt: number }, caller: Caller | undefined): Promise<ActivityEnded> => {
    const plan = endPlan(read, o.pid, o.startedAt, threadId => reaches(threadId, caller));
    const view = activityOf(read);
    if ("kept" in plan) {
      ctx.reapKept(plan.kept.threadId, { now: true });
      const freed = view.threads.find(t => t.threadId === plan.kept.threadId)?.processes.filter(p => p.kind === "agent") ?? [];
      return { pid: o.pid, ended: freed, forced: [], left: [], freed: plan.kept.threadId };
    }
    const [done] = await killTrees(plan.targets.slice(0, 1));
    return endedOf(view, plan.targets, o.pid, done!);
  };

  return {
    async read(o, caller) {
      return (await viewOf(o, caller, false)).view;
    },

    async end(o, caller) {
      await ctx.ready();
      return endOn(await readOf(await here(o.computer)), o, caller);
    },

    async meet(threadIds, processes, verb, caller) {
      if (processes === "keep") return [];
      const local = threadIds.filter(id => !elsewhere(id));
      const here = local.length === 0 ? undefined : await viewOf({ threadIds: local }, caller, true);
      const left: LeftThread[] = here === undefined ? [] : processesLeft(here.view, local);
      const boxes = new Map<string, string[]>();
      for (const id of threadIds) {
        const box = boxOf(id);
        if (box === undefined) continue;
        if (!reaches(id, caller)) throw notFoundRefusal(`no thread ${threadWord(id)}`);
        boxes.set(box.placeId, [...(boxes.get(box.placeId) ?? []), id]);
      }
      const onBoxes: { placeId: string; t: ActivityThread }[] = [];
      for (const [placeId, ids] of boxes) {
        const there = await boxReadQuiet(placeId, ids);
        for (const t of there === undefined ? [] : processesLeft(there.view, ids)) {
          onBoxes.push({ placeId, t });
          left.push({ ...t, on: there!.read.computer });
        }
      }
      if (left.length === 0) return [];
      // Every head's plan is made before the ask is answered or anything signalled: a head the caller may not end is
      // the person's, which the ask names and a stop refuses before its first signal.
      const theirs = new Set<number>();
      let reach: unknown;
      const plans = (here === undefined ? [] : processesLeft(here.view, local)).flatMap(t =>
        endHeads(t.processes).flatMap(p => {
          let unreached = false;
          const may = (threadId: string | undefined): boolean => {
            const ok = reaches(threadId, caller);
            if (!ok) unreached = true;
            return ok;
          };
          try {
            const plan = endPlan(here!.read, p.pid, p.startedAt, may);
            if ("kept" in plan) throw new Error(`${p.pid} is the agent thread ${threadWord(plan.kept.threadId)} keeps, which a settle frees itself`);
            return [{ pid: p.pid, targets: plan.targets }];
          } catch (e) {
            if (!unreached) throw e;
            theirs.add(p.pid);
            reach ??= e;
            return [];
          }
        }),
      );
      if (processes === "ask") throw processesLeftRefusal(verb, left, theirs);
      if (reach !== undefined) throw reach;
      const ended: ActivityEnded[] = [];
      if (here !== undefined && plans.length > 0) {
        const done = await killTrees(plans.map(x => x.targets[0]!));
        plans.forEach((x, i) => ended.push(endedOf(here.view, x.targets, x.pid, done[i]!)));
      }
      for (const { placeId, t } of onBoxes) ended.push(await endOnBox(placeId, boxOf(t.threadId)!.entry, t));
      return ended;
    },

  };
}
