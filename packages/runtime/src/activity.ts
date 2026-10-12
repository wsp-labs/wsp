// SPDX-License-Identifier: AGPL-3.0-only
// Activity for the command line and the tools: this computer's processes by
// the thread that started them, and the End of one by the pid and start it was
// listed with. Each read opens its own socket to this computer's daemon, takes
// one whole snapshot and the listeners, and closes it, so nothing is read while
// nobody asks. A thread here is named to the read by its running turn's process
// and the agent it keeps; the daemon's own owner on a row, where it names one,
// places the rest.
import { CATALOG_AGENTS } from "@wsp/catalog";
import {
  BOX_ACTIVITY_FIX,
  DAEMON_PROC_INTERVAL_MS,
  END_GRACE_MS,
  boxActivityLine,
  elsewhereActivityLine,
  HERE_PLACE_ID,
  THIS_COMPUTER,
  activityOf,
  endPlan,
  isLocalWorkspace,
  noSuchPlaceRefusal,
  notFoundRefusal,
  ProcKillReply,
  ProcListReply,
  ProcSnapshot,
  listLateLine,
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
  processesLeft,
  processesLeftRefusal,
  threadCgroup,
  unknownOpLine,
} from "@wsp/protocol";
import { DaemonRefusal } from "./types/internal.js";
import type { LiveWorkspace } from "./types/wiring.js";
import type { DaemonChannel } from "./daemon-channel.js";
import type { RuntimeContext } from "./context.js";

/** How long a read waits for the daemon's first snapshot, which comes one sampling interval after the watch. */
const SNAPSHOT_WAIT_MS = 10_000;
/** How often an End looks again for what TERM has not ended yet. */
const END_POLL_MS = 250;

export const NO_DAEMON_HERE = "this host runs no daemon on this computer, so it reads no processes";

export interface ActivityDoor {
  /** The computer's threads, one thread's alone, or those of threadIds this computer runs. */
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

const sleep = (ms: number): Promise<void> => new Promise(resolve => setTimeout(resolve, ms));

export function activityDoor(ctx: RuntimeContext): ActivityDoor {
  /** A computer named must be this one: a box's processes ride its link, which this read does not take yet. */
  const here = async (computer: string | undefined): Promise<string> => {
    const name = ctx.placeDoor?.nameOf(HERE_PLACE_ID) ?? THIS_COMPUTER;
    if (computer === undefined || computer === HERE_PLACE_ID || computer.toLowerCase() === name.toLowerCase() || computer === THIS_COMPUTER) return name;
    const rows = ctx.placeDoor === undefined ? [] : await ctx.placeDoor.rows();
    const row = rows.find(r => r.id === computer || r.name.toLowerCase() === computer.toLowerCase());
    if (row === undefined) throw usageRefusal(noSuchPlaceRefusal(computer, rows.map(r => r.name)), "Name one wsp computers lists.");
    throw usageRefusal(boxActivityLine(row.name), BOX_ACTIVITY_FIX);
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

  /** Each thread on this computer: its roots as the daemon is told them (`ctx.scopesHere`, the one rule), with the agent
   * it keeps for its next turn while no turn runs, then every other thread a local folder ran, rootless, so what a
   * finished thread left still lists under it; running threads first, then newest first. A warm agent no send has
   * claimed names no thread yet. */
  const scopesHere = (): ActivityScope[] => {
    const running = new Set<string>();
    const latest = new Map<string, { workspaceId: string; at: number }>();
    for (const { view } of ctx.sessions.values()) {
      const record = ctx.live.get(view.workspaceId)?.record;
      if (view.threadId === undefined || record === undefined || !isLocalWorkspace(record)) continue;
      if (view.status === "running") running.add(view.threadId);
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
      scopes.push({ threadId: s.thread, title: titled(s.thread), workspaceId, roots, ...(kept !== undefined ? { kept } : {}) });
    }
    const named = new Set(scopes.map(s => s.threadId));
    for (const [threadId, t] of latest) if (!named.has(threadId)) scopes.push({ threadId, title: titled(threadId), workspaceId: t.workspaceId, roots: [] });
    const started = (threadId: string): number => latest.get(threadId)?.at ?? 0;
    return scopes.sort((a, b) => Number(running.has(b.threadId)) - Number(running.has(a.threadId)) || started(b.threadId) - started(a.threadId));
  };

  const readOf = async (computer: string, once = false): Promise<ActivityRead> => {
    const { snap, ports } = await snapshot({ once });
    return { computer, at: snap.at, procs: snap.procs, daemon: snap.daemon, host: process.pid, scopes: scopesHere(), agentBins: CATALOG_AGENTS.map(a => a.bin), ports };
  };

  /** Whether a caller may read or end what a thread runs: a person any thread's, a thread its own and those under it. */
  const reaches = (threadId: string | undefined, caller: Caller | undefined): boolean => scopeOf(caller) === undefined || (threadId !== undefined && ctx.settlesThread(threadId, caller));

  /** The box a thread runs on, where its processes stand in its cgroup there; a cloud machine's thread has none. */
  const boxOf = (id: string): { placeId: string; entry: LiveWorkspace } | undefined => {
    const entry = ctx.live.get(ctx.threadRecords.get(id)?.workspaceId ?? ctx.latestOn(id)?.workspaceId ?? "");
    const placeId = entry?.record.place;
    return entry === undefined || isLocalWorkspace(entry.record) || placeId === undefined || ctx.placeDoor?.joined(placeId) !== true ? undefined : { placeId, entry };
  };

  /** One read of a box's processes by its threads' cgroups, once; nothing while the box is away or its daemon fails the
   * read, since nothing there can be read or ended then, and a box's fault never holds a settle here. */
  const boxRead = async (placeId: string, threadIds: readonly string[]): Promise<{ read: ActivityRead; view: ActivityView } | undefined> => {
    const door = ctx.placeDoor;
    if (door === undefined || door.link(placeId) === undefined) return undefined;
    // The box's link is one socket with one process watch its panes share, which closing this channel would not end.
    const taken = await snapshot({ once: true, watch: false }, async onEvent => door.channel(placeId, onEvent) ?? Promise.reject(new Error(`${door.nameOf(placeId)} is away`))).catch((e: unknown) => {
      console.warn(`${door.nameOf(placeId)}'s processes were not read for a settle or a delete: ${e instanceof Error ? e.message : String(e)}`);
      return undefined;
    });
    if (taken === undefined) return undefined;
    const { snap, ports } = taken;
    const scopes = threadIds.map(id => ({ threadId: id, title: ctx.threadFacts(id)?.title ?? threadWord(id), workspaceId: boxOf(id)?.entry.record.id ?? "", roots: [], cgroup: threadCgroup(id) }));
    const read: ActivityRead = { computer: door.nameOf(placeId), at: snap.at, procs: snap.procs, daemon: snap.daemon, host: 0, scopes, agentBins: CATALOG_AGENTS.map(a => a.bin), ports };
    return { read, view: activityOf(read) };
  };

  /** Ends a box thread's cgroup through the end that box already runs, then reads again what is still there, each
   * process by the start it was listed with. */
  const endOnBox = async (placeId: string, entry: LiveWorkspace, t: ActivityThread): Promise<ActivityEnded> => {
    await ctx.moduleOf(entry.record.kind).endThread?.(entry, t.threadId, {});
    const after = await boxRead(placeId, [t.threadId]).catch(() => undefined);
    const left = t.processes.filter(p => after === undefined || after.read.procs.some(q => q.pid === p.pid && Math.abs(q.startedAt - p.startedAt) <= SAME_START_MS)).map(p => p.pid);
    return { pid: t.processes[0]!.pid, ended: t.processes, forced: [], left };
  };

  const elsewhere = (id: string): boolean => {
    const record = ctx.live.get(ctx.threadRecords.get(id)?.workspaceId ?? ctx.latestOn(id)?.workspaceId ?? "")?.record;
    return record !== undefined && !isLocalWorkspace(record);
  };

  /** What read answers, off a read already taken or a fresh one. */
  const viewOf = async (o: { computer?: string; thread?: string; threadIds?: readonly string[] }, caller: Caller | undefined, once: boolean): Promise<{ view: ActivityView; read: ActivityRead }> => {
    await ctx.ready();
    const computer = await here(o.computer);
    const asked = o.thread !== undefined ? [o.thread] : o.threadIds;
    for (const id of asked ?? []) {
      if ((ctx.threadRecords.get(id) === undefined && ctx.threadFacts(id) === undefined) || !reaches(id, caller)) throw notFoundRefusal(`no thread ${threadWord(id)}`);
    }
    if (o.thread !== undefined && elsewhere(o.thread)) throw usageRefusal(elsewhereActivityLine(o.thread), BOX_ACTIVITY_FIX);
    const wanted = asked === undefined ? undefined : new Set(asked.filter(id => !elsewhere(id)));
    const read = await readOf(computer, once);
    const view = activityOf(read);
    const threads = view.threads.filter(t => (wanted === undefined || wanted.has(t.threadId)) && reaches(t.threadId, caller));
    // What runs outside the asked threads is the person's to read: a thread sees no command line but its tree's.
    const whole = wanted === undefined && scopeOf(caller) === undefined;
    return { view: { ...view, threads, wsp: whole ? view.wsp : [], unowned: whole ? view.unowned : [] }, read };
  };

  /** An End off a read already taken: the plan is made on it, before any signal. */
  const endOn = async (read: ActivityRead, o: { pid: number; startedAt: number }, caller: Caller | undefined): Promise<ActivityEnded> => {
    const plan = endPlan(read, o.pid, o.startedAt, threadId => reaches(threadId, caller));
    const view = activityOf(read);
    const rowOf = (pid: number): ActivityProc | undefined => [...view.threads.flatMap(t => t.processes), ...view.unowned].find(p => p.pid === pid);
    if ("kept" in plan) {
      ctx.reapKept(plan.kept.threadId, { now: true });
      const freed = view.threads.find(t => t.threadId === plan.kept.threadId)?.processes.filter(p => p.kind === "agent") ?? [];
      return { pid: o.pid, ended: freed, forced: [], left: [], freed: plan.kept.threadId };
    }
    const ended = plan.targets.map(p => rowOf(p.pid) ?? { pid: p.pid, ppid: p.ppid, depth: 0, startedAt: p.startedAt, kind: "other" as const, name: p.comm, command: p.cmdline === "" ? p.comm : p.cmdline, memory: p.rss, cpu: p.cpu, ports: [] });
    // The daemon answers a TERM once its own grace is over, with what it sent KILL to, which no list after shows.
    const killedThere = new Set<number>();
    const signal = async (pids: readonly number[], sig: "TERM" | "KILL"): Promise<void> => {
      if (ctx.opts.local?.daemonRoad === undefined) throw new Error(NO_DAEMON_HERE);
      // Through the guard the pane's road has, so the daemon checks the host's lists at the signal too.
      await ctx.overChannel(ctx.killGuarded(await ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, () => {}), true), async ask => {
        for (const pid of pids) {
          const reply = await ask({ op: "proc.kill", pid, signal: sig }).catch((e: unknown) => {
            if (!(e instanceof DaemonRefusal && e.code === "not-found")) throw e;
            return undefined;
          });
          for (const f of reply === undefined ? [] : (ProcKillReply.parse(reply).forced ?? [])) killedThere.add(f);
        }
      });
    };
    /** Rows the daemon read after `after`, never a running sampler's tick from before it: a list without one waits
     * for the next tick, which comes within the sampler's interval. */
    const listAfter = async (after: number): Promise<ProcSnapshot> => {
      const until = Date.now() + DAEMON_PROC_INTERVAL_MS + END_GRACE_MS;
      for (;;) {
        const { snap } = await snapshot({ once: true, ports: false });
        if (snap.at > after) return snap;
        if (Date.now() > until) throw new Error(listLateLine(after));
        await sleep(END_POLL_MS);
      }
    };
    /** The targets a list still holds, each by its pid and the start it was listed with: a pid handed out again is not one. */
    const standing = (snap: ProcSnapshot, targets: readonly ProcEntry[]): ProcEntry[] =>
      targets.filter(t => snap.procs.some(p => p.pid === t.pid && Math.abs(p.startedAt - t.startedAt) <= SAME_START_MS));
    const termAt = Date.now();
    await signal(plan.targets.map(p => p.pid), "TERM");
    const graceEnd = termAt + END_GRACE_MS;
    let still: ProcEntry[] = plan.targets;
    let readAt = termAt;
    while (still.length > 0 && readAt < graceEnd) {
      await sleep(END_POLL_MS);
      const snap = await listAfter(readAt);
      readAt = snap.at;
      still = standing(snap, still);
    }
    // What is still standing was read at the grace's end or after it, so KILL goes only to a process there then.
    const forced = still.map(p => p.pid);
    let left: number[] = [];
    if (forced.length > 0) {
      const killAt = Date.now();
      await signal(forced, "KILL");
      await sleep(END_POLL_MS);
      left = standing(await listAfter(killAt), still).map(p => p.pid);
    }
    return { pid: o.pid, ended, forced: [...new Set([...killedThere, ...forced])], left };
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
        const there = await boxRead(placeId, ids);
        for (const t of there === undefined ? [] : processesLeft(there.view, ids)) {
          onBoxes.push({ placeId, t });
          left.push({ ...t, on: there!.read.computer });
        }
      }
      if (left.length === 0) return [];
      if (processes === "ask") throw processesLeftRefusal(verb, left);
      const ended: ActivityEnded[] = [];
      if (here !== undefined) {
        for (const t of processesLeft(here.view, local)) {
          const ending = new Set(t.processes.map(p => p.pid));
          for (const p of t.processes) if (!ending.has(p.ppid)) ended.push(await endOn(here.read, { pid: p.pid, startedAt: p.startedAt }, caller));
        }
      }
      for (const { placeId, t } of onBoxes) ended.push(await endOnBox(placeId, boxOf(t.threadId)!.entry, t));
      return ended;
    },

  };
}
