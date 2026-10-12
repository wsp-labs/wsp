// SPDX-License-Identifier: AGPL-3.0-only
// Activity's one reading of a computer's processes: who each is (a thread's,
// wsp's own, or wsp's with no thread known), what it is, and the rules an End
// meets before anything is signalled. The page, the Processes pane, the command
// line, the tools, settle and delete all read placeProcs(). A row is a thread's
// when the daemon names that thread as its owner, when it is the thread's
// agent, when it stands in the thread's cgroup or slate folder, or when its
// parent is. From a root down to the agent's own binary, and the input pump
// left in its group, is the agent, which only the thread's stop ends; a kept
// agent is freed through the thread. Nothing here is aimed by a pid alone: an
// End names the start it read, and a pid that now names another process is
// refused.
import { z } from "zod";
import { byProcColumn, PROC_KINDS, type ProcEntry, type ProcKind } from "../wire/daemon.js";
import { PROCESSES_LEFT_KIND, refusal } from "../exit.js";
import { inCgroup } from "../thread-cgroup.js";
import { threadWord } from "../words/computer.js";

/** One process as Activity draws it, in the order of its thread's tree: depth counts the steps under the first of
 * its parents the group holds. memory is in bytes, cpu the share of one core, startedAt the epoch milliseconds the
 * daemon read, which an End names back. */
export const ActivityProc = z.object({
  pid: z.number().int(),
  ppid: z.number().int(),
  depth: z.number().int().nonnegative(),
  startedAt: z.number(),
  kind: z.enum(PROC_KINDS),
  name: z.string(),
  command: z.string(),
  memory: z.number(),
  cpu: z.number(),
  ports: z.array(z.number().int()),
});
export type ActivityProc = z.infer<typeof ActivityProc>;

export const ActivityThread = z.object({
  threadId: z.string(),
  title: z.string(),
  workspaceId: z.string(),
  memory: z.number(),
  cpu: z.number(),
  processes: z.array(ActivityProc),
});
export type ActivityThread = z.infer<typeof ActivityThread>;

/** A computer's processes: each thread with any, wsp's own (the host, the daemon and what they started outside a
 * thread), and what wsp started with no thread known. What the person started outside wsp, one only a workspace's
 * folder places included, is not listed. */
export const ActivityView = z.object({
  computer: z.string(),
  at: z.number(),
  threads: z.array(ActivityThread),
  wsp: z.array(ActivityProc),
  unowned: z.array(ActivityProc),
});
export type ActivityView = z.infer<typeof ActivityView>;

/** A thread a settle or a delete would leave processes running for, as its refusal carries it: `on` names the box it
 * runs on, where Stop ends the thread's cgroup whole rather than each process by identity. */
export const LeftThread = ActivityThread.extend({ on: z.string().optional() });
export type LeftThread = z.infer<typeof LeftThread>;

/** What an End did: every process it signalled, those still there after TERM's grace that KILL went to, and those
 * still there after that. A kept agent is freed through its thread, which `freed` names. */
export const ActivityEnded = z.object({
  pid: z.number().int(),
  ended: z.array(ActivityProc),
  forced: z.array(z.number().int()),
  left: z.array(z.number().int()),
  freed: z.string().optional(),
});
export type ActivityEnded = z.infer<typeof ActivityEnded>;

/** How settle and delete meet a thread that still runs processes: end them by identity, or leave them running. */
export const PROCESSES_CHOICES = ["stop", "keep"] as const;
export type ProcessesChoice = (typeof PROCESSES_CHOICES)[number];
/** What the host is asked to do with them: a verb that named no choice asks, which refuses while any run. */
export const ProcessesAsk = z.enum(["ask", ...PROCESSES_CHOICES]);
export type ProcessesAsk = z.infer<typeof ProcessesAsk>;

/** How long TERM is given before KILL. */
export const END_GRACE_MS = 2_000;
/** How far two reads of one process's start may sit apart: this computer's daemon reads a start off ps's elapsed
 * time, which a second sampler reads again a second off. */
export const SAME_START_MS = 2_000;

/** A thread as the host names it to the read: its roots are its running turn's process and its kept agent's, each
 * the top of a stretch down to the agent's binary. */
export interface ActivityScope {
  threadId: string;
  title: string;
  workspaceId: string;
  roots: readonly number[];
  /** The root of the agent the thread keeps for its next turn, which an End frees through the thread. */
  kept?: number;
  /** On a box, the cgroup every process the thread started stands in. */
  cgroup?: string;
  /** The command its agent puts on PATH, where the read knows the thread's agent. */
  bin?: string;
}

export interface ActivityRead {
  computer: string;
  at: number;
  procs: readonly ProcEntry[];
  /** The daemon's pid, as its snapshot names it, and this host's. */
  daemon: number;
  host: number;
  scopes: readonly ActivityScope[];
  /** The command each agent puts on PATH, which ends the stretch from a root down to the agent. */
  agentBins: readonly string[];
  ports?: ReadonlyMap<number, readonly number[]>;
}

/** What a script is run by, so the script it runs names the process. */
export const SCRIPT_RUNTIMES: ReadonlySet<string> = new Set(["node", "python", "python3", "bun", "deno", "ruby"]);

/** The names a process runs by: its comm, argv[0]'s, and the script argv[0] runs where that is a runtime. A Mac names a
 * process by the file the kernel ran, which for Claude Code's native install is its version (`2.1.296`) and for an npm
 * install or cursor-agent is node, so the name a person knows is read off the command line beside it. */
export function procBins(p: ProcEntry): string[] {
  const argv = p.cmdline.split(" ").filter(w => w !== "");
  const base = (w: string | undefined): string | undefined => w?.split("/").at(-1);
  const first = base(argv[0]);
  const script = first !== undefined && SCRIPT_RUNTIMES.has(first) ? base(argv.slice(1).find(w => !w.startsWith("-"))) : undefined;
  return [p.comm, first, script].filter((b): b is string => b !== undefined && b !== "");
}

/** From a root down to the shallowest process under it that is the agent, and on through an only child that is the agent
 * too, root first, or undefined where none is: Codex's npm launcher runs the native codex as its one child. Every
 * process on the way is the agent's, the run script and any shell it forked: only the thread's stop ends them. */
export function agentPath(root: number, kidsOf: (pid: number) => readonly ProcEntry[], isAgent: (p: ProcEntry) => boolean, byPid: ReadonlyMap<number, ProcEntry>): number[] | undefined {
  const via = new Map<number, number>();
  const queue = [root];
  while (queue.length > 0) {
    const pid = queue.shift()!;
    const p = byPid.get(pid);
    if (p !== undefined && isAgent(p)) {
      const path = [pid];
      for (let at = via.get(pid); at !== undefined; at = via.get(at)) path.unshift(at);
      for (let kids = kidsOf(pid); kids.length === 1 && isAgent(kids[0]!) && !path.includes(kids[0]!.pid); kids = kidsOf(kids[0]!.pid)) path.push(kids[0]!.pid);
      return path;
    }
    for (const kid of kidsOf(pid)) {
      if (via.has(kid.pid) || kid.pid === root) continue;
      via.set(kid.pid, pid);
      queue.push(kid.pid);
    }
  }
  return undefined;
}

/** A turn's run script as either road starts it, `bash <run folder>/<12 hex>.sh`, and the pump that feeds the agent
 * its input. The pump and its shell, a fork of the script that reads as it, outlive the script's own shell under pid 1
 * in the turn's group: they are the agent's, and end with it. */
export const RUN_SCRIPT = /^bash \S*\/[0-9a-f]{12}\.sh$/;
const RUN_PUMP = /^tail -n \+1 -f \S*\/[0-9a-f]{12}\.in$/;

const SHELLS = new Set(["sh", "bash", "zsh", "fish", "dash"]);
const TEST_LINE = /\b(vitest|jest|mocha|pytest|playwright test|cargo test|cargo nextest|go test|bun test|node --test)\b/;
const BUILD_LINE = /\b(rustc|cargo build|cargo check|cc1|cc1plus|clang|ld|tsc|go build|webpack(?! serve)|swiftc)\b/;
const DEV_LINE = /\b(vite|next dev|next-server|webpack serve|astro dev|nuxt dev|remix dev|wrangler dev|electron-vite|http\.server|flask run|uvicorn|rails server)\b/;
/** A slate's runs read their files from the slate's folder, the host's `slates/<thread>` or a box's `wsp-slates/<thread>`. */
export const SLATE_FOLDER = /(^|\/)(wsp-)?slates\/[^/\s]+\//;

/** The rungs that name a process a thread started. The folder names only where it runs, which the person's own
 * terminal shares, so a row it alone places is not wsp's; the coalition names wsp's own launch on a Mac, so a row it
 * places is wsp's, with no thread and no End. */
const STARTED_BY = new Set<string>(["root", "fork", "group", "env", "cgroup"]);
const startedByWsp = (p: ProcEntry): boolean => p.owner !== undefined && STARTED_BY.has(p.owner.by);

/** A thread as the placement reads it. */
export interface PlaceThread {
  id: string;
  /** Its turn's process, the groups its turns led and the agent it keeps: each the top of a stretch down to its agent. */
  roots: readonly number[];
  /** On a box, the cgroup everything it started stands in; where it names no root, the heads there are its roots. */
  cgroup?: string;
  /** The command its agent puts on PATH; any agent's where left out. */
  bin?: string;
  /** The agent's pids its turns' runs recorded as they started it, the agent whatever it runs as. */
  agents?: readonly number[];
}

export interface PlaceInput {
  procs: readonly ProcEntry[];
  daemon: number;
  /** The host's pid, on the computer it runs on. */
  host?: number;
  threads: readonly PlaceThread[];
  agentBins: readonly string[];
  /** Whether a process listens on a port. */
  listens: (pid: number) => boolean;
  /** The desktop app's own processes, each a browser tab or frame, or another part of the app. */
  app?: ReadonlyMap<number, "tab" | "app">;
}

/** Whose a process is: a thread's, wsp's own parts', or wsp's with no thread known. */
export type Whose = { thread: string } | "wsp" | "loose";

/** One computer's processes placed, the one rule Activity, `wsp activity`, settle and delete read. */
export interface Placement {
  /** Every live process: a zombie holds no memory and takes no signal. */
  procs: readonly ProcEntry[];
  byPid: ReadonlyMap<number, ProcEntry>;
  kidsOf: (pid: number) => readonly ProcEntry[];
  /** Absent for a process that is not wsp's. */
  whose: ReadonlyMap<number, Whose>;
  /** The process each thread's agent runs as, or the root that stands for it until it shows, by its thread. */
  agent: ReadonlyMap<number, string>;
  /** Every other process the agent's row holds, by that agent's pid: the run script and any shell from a root down to
   * it, and the input pump and the script's shells left in its group. */
  foldsInto: ReadonlyMap<number, number>;
  /** The root each of those stands under. */
  rootOf: ReadonlyMap<number, number>;
  kindOf: (pid: number) => ProcKind;
  /** wsp's own: the daemon, the host, and the app's parts other than its tabs. */
  needed: (pid: number) => boolean;
  /** Whether End reaches it: a process a thread or wsp started, never an agent or what wsp needs. */
  endable: (pid: number) => boolean;
}

export function placeProcs(input: PlaceInput): Placement {
  const procs = input.procs.filter(p => !p.state.startsWith("Z"));
  const byPid = new Map(procs.map(p => [p.pid, p]));
  const children = new Map<number, ProcEntry[]>();
  for (const p of procs) if (p.ppid !== p.pid && byPid.has(p.ppid)) (children.get(p.ppid) ?? children.set(p.ppid, []).get(p.ppid)!).push(p);
  const kidsOf = (pid: number): readonly ProcEntry[] => children.get(pid) ?? [];
  const threadById = new Map(input.threads.map(t => [t.id, t]));
  const app = input.app ?? new Map<number, "tab" | "app">();
  const bins = new Set(input.agentBins);

  const agent = new Map<number, string>();
  const foldsInto = new Map<number, number>();
  const rootOf = new Map<number, number>();
  const standIns: [number, PlaceThread][] = [];
  for (const t of input.threads) {
    const isAgent = (p: ProcEntry): boolean => t.agents?.includes(p.pid) === true || (p.kind === "agent" && p.owner?.thread === t.id) || procBins(p).some(b => (t.bin !== undefined ? b === t.bin : bins.has(b)));
    let roots = t.roots.filter(r => byPid.has(r));
    const cgroup = t.cgroup;
    if (roots.length === 0 && cgroup !== undefined) {
      const inside = new Set(procs.filter(p => inCgroup(p.cgroup, cgroup)).map(p => p.pid));
      roots = [...inside].filter(pid => !inside.has(byPid.get(pid)!.ppid));
    } else standIns.push(...roots.map((r): [number, PlaceThread] => [r, t]));
    for (const root of roots) {
      const path = agentPath(root, kidsOf, isAgent, byPid);
      if (path === undefined) continue;
      const at = path.at(-1)!;
      agent.set(at, t.id);
      foldsInto.delete(at);
      rootOf.set(at, root);
      for (const pid of path.slice(0, -1)) {
        if (agent.has(pid)) continue;
        foldsInto.set(pid, at);
        rootOf.set(pid, root);
      }
    }
  }
  for (const [root, t] of standIns) {
    if (agent.has(root) || foldsInto.has(root)) continue;
    agent.set(root, t.id);
    rootOf.set(root, root);
  }
  // The pump and the script's shells stand in the group of the agent's run, under pid 1 once the script's shell is gone.
  const groupAgent = new Map<number, number>();
  for (const pid of [...agent.keys(), ...foldsInto.keys()]) {
    const at = agent.has(pid) ? pid : foldsInto.get(pid)!;
    const pgid = byPid.get(pid)?.pgid;
    if (pgid !== undefined && !groupAgent.has(pgid)) groupAgent.set(pgid, at);
    const root = rootOf.get(pid);
    if (root !== undefined && !groupAgent.has(root)) groupAgent.set(root, at);
  }
  for (const p of procs) {
    if (p.pgid === undefined || agent.has(p.pid) || foldsInto.has(p.pid) || !(RUN_SCRIPT.test(p.cmdline) || RUN_PUMP.test(p.cmdline))) continue;
    const at = groupAgent.get(p.pgid);
    const stamped = p.owner?.thread;
    if (at === undefined || (stamped !== undefined && threadById.has(stamped) && stamped !== agent.get(at))) continue;
    foldsInto.set(p.pid, at);
    rootOf.set(p.pid, rootOf.get(at)!);
  }

  const wspPart = (p: ProcEntry): boolean => p.pid === input.daemon || p.pid === input.host || app.has(p.pid) || p.kind === "app" || p.kind === "wsp";
  const whose = new Map<number, Whose>();
  const nobody = new Set<number>();
  const place = (p: ProcEntry, seen: Set<number>): Whose | undefined => {
    const held = whose.get(p.pid);
    if (held !== undefined || nobody.has(p.pid)) return held;
    let said: Whose | undefined;
    const stamped = p.owner?.thread;
    const stretch = agent.get(p.pid) ?? agent.get(foldsInto.get(p.pid) ?? -1);
    if (stamped !== undefined && threadById.has(stamped)) said = { thread: stamped };
    else if (stretch !== undefined) said = { thread: stretch };
    else if (wspPart(p)) said = "wsp";
    else if (startedByWsp(p)) said = "loose";
    else {
      const cg = input.threads.find(t => t.cgroup !== undefined && inCgroup(p.cgroup, t.cgroup));
      const slate = SLATE_FOLDER.test(p.cmdline) ? (input.threads.find(t => p.cmdline.includes(`slates/${t.id}/`)) ?? null) : undefined;
      if (cg !== undefined) said = { thread: cg.id };
      else if (slate !== undefined) said = slate === null ? "loose" : { thread: slate.id };
      else if (p.pty !== undefined) said = "loose";
      else {
        const parent = p.ppid !== p.pid ? byPid.get(p.ppid) : undefined;
        // What the host and the app start outside a thread, the guest tool server among them, is wsp's own.
        if (parent !== undefined && !seen.has(parent.pid)) said = place(parent, seen.add(p.pid));
        if (said === undefined && p.owner?.by === "coalition") said = "loose";
      }
    }
    if (said === undefined) nobody.add(p.pid);
    else whose.set(p.pid, said);
    return said;
  };
  for (const p of procs) place(p, new Set());

  const kinds = new Map<number, ProcKind>();
  const kindOf = (pid: number): ProcKind => {
    const held = kinds.get(pid);
    if (held !== undefined) return held;
    const p = byPid.get(pid);
    if (p === undefined) return "other";
    kinds.set(pid, "other");
    const part = app.get(pid);
    const parent = p.ppid !== p.pid ? byPid.get(p.ppid) : undefined;
    const above = parent === undefined ? null : kindOf(parent.pid);
    let kind: ProcKind;
    if (part !== undefined) kind = part;
    else if (agent.has(pid) || foldsInto.has(pid)) kind = "agent";
    else if ((pid === input.daemon || pid === input.host) && p.kind === undefined) kind = "wsp";
    else if (p.kind !== undefined) kind = p.kind;
    else if (p.pty !== undefined) kind = "terminal";
    else if (SLATE_FOLDER.test(p.cmdline)) kind = "slate";
    else if (TEST_LINE.test(p.cmdline)) kind = "test";
    else if (BUILD_LINE.test(p.cmdline)) kind = "build";
    else if (input.listens(pid) || DEV_LINE.test(p.cmdline)) kind = "dev";
    else if (above === "dev" || above === "test" || above === "build") kind = above;
    else if (above === "agent" && !foldsInto.has(parent!.pid) && !SHELLS.has(p.comm)) kind = "tool";
    else kind = "other";
    kinds.set(pid, kind);
    return kind;
  };
  const needed = (pid: number): boolean => {
    const p = byPid.get(pid);
    return p !== undefined && wspPart(p) && app.get(pid) !== "tab";
  };
  const endable = (pid: number): boolean => {
    const p = byPid.get(pid);
    const w = whose.get(pid);
    return p !== undefined && w !== undefined && kindOf(pid) !== "agent" && !needed(pid) && (typeof w === "object" || startedByWsp(p));
  };
  return { procs, byPid, kidsOf, whose, agent, foldsInto, rootOf, kindOf, needed, endable };
}

/** The read's threads as the placement takes them. */
const readPlaced = (read: ActivityRead): Placement =>
  placeProcs({
    procs: read.procs,
    daemon: read.daemon,
    host: read.host,
    threads: read.scopes.map(s => ({ id: s.threadId, roots: s.roots, ...(s.cgroup !== undefined ? { cgroup: s.cgroup } : {}), ...(s.bin !== undefined ? { bin: s.bin } : {}) })),
    agentBins: read.agentBins,
    listens: pid => (read.ports?.get(pid)?.length ?? 0) > 0,
  });

const threadIdOf = (w: Whose | undefined): string | undefined => (typeof w === "object" ? w.thread : undefined);

/** A group's rows as a tree: each process under the first of its parents the group holds, siblings busiest first. */
function treeRows(placed: Placement, members: readonly ProcEntry[], ports: ReadonlyMap<number, readonly number[]> | undefined): ActivityProc[] {
  const inside = new Set(members.map(p => p.pid));
  const heads = members.filter(p => p.ppid === p.pid || !inside.has(p.ppid));
  const out: ActivityProc[] = [];
  const walk = (list: readonly ProcEntry[], depth: number): void => {
    for (const p of [...list].sort(byProcColumn("cpu"))) {
      out.push({ pid: p.pid, ppid: p.ppid, depth, startedAt: p.startedAt, kind: placed.kindOf(p.pid), name: p.comm, command: p.cmdline === "" ? p.comm : p.cmdline, memory: p.rss, cpu: p.cpu, ports: [...(ports?.get(p.pid) ?? [])] });
      walk(placed.kidsOf(p.pid).filter(k => inside.has(k.pid)), depth + 1);
    }
  };
  walk(heads, 0);
  return out;
}

/** The computer's processes by thread, each thread in the order the host named it and only where it has any. */
export function activityOf(read: ActivityRead): ActivityView {
  const placed = readPlaced(read);
  const threads: ActivityThread[] = [];
  for (const scope of read.scopes) {
    const members = placed.procs.filter(p => threadIdOf(placed.whose.get(p.pid)) === scope.threadId);
    if (members.length === 0) continue;
    let memory = 0;
    let cpu = 0;
    for (const p of members) {
      memory += p.rss;
      cpu += p.cpu;
    }
    threads.push({ threadId: scope.threadId, title: scope.title, workspaceId: scope.workspaceId, memory, cpu, processes: treeRows(placed, members, read.ports) });
  }
  const of = (w: "wsp" | "loose"): ProcEntry[] => placed.procs.filter(p => placed.whose.get(p.pid) === w);
  return { computer: read.computer, at: read.at, threads, wsp: treeRows(placed, of("wsp"), read.ports), unowned: treeRows(placed, of("loose"), read.ports) };
}

/** What an End signals, read before the first signal: the process and every process under it, each by the start it
 * was read with; or, for a kept agent, the thread that frees it. */
export type EndPlan = { targets: ProcEntry[] } | { kept: ActivityScope };


const at = (ms: number): string => new Date(ms).toISOString();

export const boxActivityLine = (name: string): string => `${name} is a box, and wsp activity reads this computer's processes:`;
export const elsewhereActivityLine = (threadId: string): string => `thread ${threadWord(threadId)} runs on a box, and wsp activity reads this computer's processes:`;
export const BOX_ACTIVITY_FIX = "open that computer's Processes pane in the app for its processes.";

/** An End that read no list after its signal in time signals nothing more, since it cannot tell what is still there. */
export const listLateLine = (after: number): string => `this computer's daemon listed no processes read after ${at(after)}, so nothing more was signalled`;

export const END_AGAIN_FIX = "read wsp activity again and end it by the pid and start it lists now.";

export const agentEndLine = (pid: number, scope: ActivityScope): string => `${pid} is the agent of thread ${threadWord(scope.threadId)} "${scope.title}", which only its thread's stop ends:`;
export const agentUnderLine = (pid: number, scope: ActivityScope): string => `${pid} has the agent of thread ${threadWord(scope.threadId)} "${scope.title}" under it, which only its thread's stop ends:`;
export const agentEndFix = (scope: ActivityScope): string => `stop its turn with wsp stop ${threadWord(scope.threadId)}, or end a process under the agent.`;
export const endGoneLine = (pid: number): string => `no process ${pid} is running on this computer:`;
export const endReusedLine = (pid: number, now: ProcEntry, asked: number): string =>
  `pid ${pid} names another process now, ${now.comm} started ${at(now.startedAt)}, not the one started ${at(asked)}:`;
export const wspOwnLine = (pid: number): string => `${pid} is wsp's own process on this computer, which wsp stop and the app end:`;
export const WSP_OWN_FIX = "end a thread's process instead.";
export const notThreadsLine = (pid: number): string => `${pid} is not a process of this thread or a thread under it:`;
export const notStartedLine = (pid: number): string => `wsp did not start ${pid}, and it ends only what a thread or wsp started:`;
export const NOT_STARTED_FIX = "end it where it was started; wsp activity lists every process wsp can end.";
export const NOT_THREADS_FIX = "a thread ends only what it and the threads under it started; ask the person for anything else.";

/** The End of one process, by the pid and the start Activity listed it with: only one Activity lists, so a process
 * wsp did not start is refused. `may` is whether the caller may end a process of that thread, or of no thread when
 * it is undefined. */
export function endPlan(read: ActivityRead, pid: number, startedAt: number, may: (threadId: string | undefined) => boolean): EndPlan {
  const placed = readPlaced(read);
  const p = placed.byPid.get(pid);
  if (p === undefined) throw refusal(endGoneLine(pid), END_AGAIN_FIX, "not-found");
  const scopeOf = (threadId: string | undefined): ActivityScope | undefined => read.scopes.find(s => s.threadId === threadId);
  // Reach before the start, so a caller learns nothing of a process it may not end, not even its name.
  const owner = threadIdOf(placed.whose.get(pid));
  if (!may(owner)) throw refusal(notThreadsLine(pid), NOT_THREADS_FIX, "usage");
  if (pid === read.daemon || pid === read.host) throw refusal(wspOwnLine(pid), WSP_OWN_FIX, "usage");
  if (owner === undefined && !startedByWsp(p)) throw refusal(notStartedLine(pid), NOT_STARTED_FIX, "usage");
  if (Math.abs(p.startedAt - startedAt) > SAME_START_MS) throw refusal(endReusedLine(pid, p, startedAt), END_AGAIN_FIX, "not-found");
  const agentOf = (q: number): ActivityScope | undefined => scopeOf(placed.agent.get(q) ?? placed.agent.get(placed.foldsInto.get(q) ?? -1));
  const agent = agentOf(pid);
  if (agent !== undefined) {
    if (agent.kept !== undefined && placed.rootOf.get(pid) === agent.kept) return { kept: agent };
    throw refusal(agentEndLine(pid, agent), agentEndFix(agent), "usage");
  }
  if (placed.kindOf(pid) === "agent") {
    const scope = scopeOf(owner);
    if (scope !== undefined) throw refusal(agentEndLine(pid, scope), agentEndFix(scope), "usage");
    throw refusal(`${pid} is an agent, which only its thread's stop ends:`, "stop its thread's turn with wsp stop.", "usage");
  }
  if (!placed.endable(pid)) throw refusal(notStartedLine(pid), NOT_STARTED_FIX, "usage");
  const targets: ProcEntry[] = [];
  const take = (q: ProcEntry): void => {
    const under = agentOf(q.pid);
    if (under !== undefined) throw refusal(agentUnderLine(pid, under), agentEndFix(under), "usage");
    if (q.pid === read.daemon || q.pid === read.host) throw refusal(wspOwnLine(q.pid), WSP_OWN_FIX, "usage");
    targets.push(q);
    for (const kid of placed.kidsOf(q.pid)) take(kid);
  };
  take(p);
  return { targets };
}

/** Each thread's processes a settle or a delete would leave running: what outlives its agent. The agent and
 * everything under it, its tool servers among them, end with the agent, which the thread ends itself. */
export function processesLeft(view: ActivityView, threadIds: readonly string[]): ActivityThread[] {
  const asked = new Set(threadIds);
  return view.threads.flatMap(t => {
    if (!asked.has(t.threadId)) return [];
    const byPid = new Map(t.processes.map(p => [p.pid, p]));
    const underAgent = (p: ActivityProc): boolean => {
      for (let at: ActivityProc | undefined = p, steps = 0; at !== undefined && steps <= t.processes.length; at = byPid.get(at.ppid), steps++) if (at.kind === "agent") return true;
      return false;
    };
    const processes = t.processes.filter(p => !underAgent(p));
    return processes.length === 0 ? [] : [{ ...t, processes }];
  });
}

/** The most processes a refusal names before it counts the rest. */
const LEFT_NAMED = 8;

export const processesLeftLine = (verb: "settle" | "delete", left: readonly LeftThread[]): string => {
  const rows = left.flatMap(t => t.processes.map(p => ({ t, p })));
  const named = rows.slice(0, LEFT_NAMED).map(({ t, p }) => `${p.pid} ${p.name} (${p.kind}${p.ports.length > 0 ? `, :${p.ports.join(" :")}` : ""}) of ${threadWord(t.threadId)}${t.on !== undefined ? ` on ${t.on}` : ""}`);
  const more = rows.length > LEFT_NAMED ? ` and ${rows.length - LEFT_NAMED} more` : "";
  return `${verb === "settle" ? "a thread to settle" : "the thread to delete"} still runs ${rows.length === 1 ? "a process" : `${rows.length} processes`}: ${named.join(", ")}${more}:`;
};

export const processesLeftFix = (verb: "settle" | "delete"): string =>
  `${verb} it again with processes stop to end them first, or processes keep to leave them running (--processes on the command line).`;

export const processesLeftRefusal = (verb: "settle" | "delete", left: readonly LeftThread[]): Error =>
  Object.assign(refusal(processesLeftLine(verb, left), processesLeftFix(verb), PROCESSES_LEFT_KIND), { left });

/** The threads a refusal over processes left running carries, as a reply puts them beside its error. */
export function processesLeftOf(e: unknown): { left?: LeftThread[] } {
  if (typeof e !== "object" || e === null || (e as { kind?: unknown }).kind !== PROCESSES_LEFT_KIND) return {};
  const read = z.array(LeftThread).safeParse((e as { left?: unknown }).left);
  return read.success ? { left: read.data } : {};
}

/** The processes a stop signalled that were still there after it, each as it was listed. */
export const standingAfterStop = (ended: readonly ActivityEnded[]): ActivityProc[] => ended.flatMap(e => e.ended.filter(p => e.left.includes(p.pid)));
