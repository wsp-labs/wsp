// SPDX-License-Identifier: AGPL-3.0-only
// Activity as the command line and the tools read it: one computer's processes
// grouped under the thread that started them, off one snapshot of the daemon
// and the threads the host names, and the rules an End meets before anything
// is signalled. A row is a thread's when the daemon names that thread as its
// owner, or when it stands under one of the thread's roots: its running turn's
// process and the agent it keeps. From a root down to the agent's own binary
// is the agent, which only the thread's stop ends; a kept agent is freed
// through the thread. Nothing here is aimed by a pid alone: an End names the
// start it read, and a pid that now names another process is refused.
import { z } from "zod";
import { byProcColumn, PROC_KINDS, type ProcEntry, type ProcKind } from "../wire/daemon.js";
import { refusal } from "../exit.js";
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

/** A computer's processes: each thread with any, wsp's own (the host and the daemon), and what the daemon names as
 * wsp's with no thread, a folder's or the app's. What the person started outside wsp is not listed. */
export const ActivityView = z.object({
  computer: z.string(),
  at: z.number(),
  threads: z.array(ActivityThread),
  wsp: z.array(ActivityProc),
  unowned: z.array(ActivityProc),
});
export type ActivityView = z.infer<typeof ActivityView>;

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

const KINDS: ReadonlySet<string> = new Set(PROC_KINDS);

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

interface Placed {
  byPid: ReadonlyMap<number, ProcEntry>;
  kidsOf: (pid: number) => readonly ProcEntry[];
  /** Each process's thread, where one claims it. */
  threadOf: ReadonlyMap<number, ActivityScope>;
  /** Every process from a root down to the agent's binary, by the thread it is the agent of, and the root it is under. */
  agentOf: ReadonlyMap<number, ActivityScope>;
  rootOf: ReadonlyMap<number, number>;
  kindOf: (p: ProcEntry) => ProcKind;
}

function place(read: ActivityRead): Placed {
  const byPid = new Map(read.procs.map(p => [p.pid, p]));
  const children = new Map<number, ProcEntry[]>();
  for (const p of read.procs) {
    if (p.ppid === p.pid || !byPid.has(p.ppid)) continue;
    const list = children.get(p.ppid);
    if (list) list.push(p);
    else children.set(p.ppid, [p]);
  }
  const kidsOf = (pid: number): readonly ProcEntry[] => children.get(pid) ?? [];
  const bins = new Set(read.agentBins);
  const isAgentBin = (p: ProcEntry): boolean => procBins(p).some(b => bins.has(b));
  /** The root and every process on the way to the shallowest agent binary under it, or the root alone. */
  const stretch = (root: number): number[] => agentPath(root, kidsOf, isAgentBin, byPid) ?? [root];

  const threadOf = new Map<number, ActivityScope>();
  const agentOf = new Map<number, ActivityScope>();
  const rootOf = new Map<number, number>();
  const byId = new Map(read.scopes.map(s => [s.threadId, s]));
  for (const p of read.procs) {
    const owner = p.owner?.thread === undefined ? undefined : byId.get(p.owner.thread);
    if (owner !== undefined) threadOf.set(p.pid, owner);
  }
  const claim = (pid: number, scope: ActivityScope): void => {
    if (threadOf.has(pid) && threadOf.get(pid) !== scope) return;
    threadOf.set(pid, scope);
    for (const kid of kidsOf(pid)) claim(kid.pid, scope);
  };
  for (const scope of read.scopes) {
    for (const root of scope.roots) {
      if (!byPid.has(root)) continue;
      claim(root, scope);
      for (const pid of stretch(root)) {
        agentOf.set(pid, scope);
        rootOf.set(pid, root);
      }
    }
  }
  const kindOf = (p: ProcEntry): ProcKind => {
    if (agentOf.has(p.pid) || p.kind === "agent") return "agent";
    if (p.pid === read.daemon || p.pid === read.host) return "wsp";
    if (p.kind !== undefined) return KINDS.has(p.kind) ? (p.kind as ProcKind) : "other";
    if (p.pty !== undefined) return "terminal";
    return "other";
  };
  return { byPid, kidsOf, threadOf, agentOf, rootOf, kindOf };
}

/** A group's rows as a tree: each process under the first of its parents the group holds, siblings busiest first. */
function treeRows(placed: Placed, members: readonly ProcEntry[], ports: ReadonlyMap<number, readonly number[]> | undefined): ActivityProc[] {
  const inside = new Set(members.map(p => p.pid));
  const heads = members.filter(p => p.ppid === p.pid || !inside.has(p.ppid));
  const out: ActivityProc[] = [];
  const walk = (list: readonly ProcEntry[], depth: number): void => {
    for (const p of [...list].sort(byProcColumn("cpu"))) {
      out.push({ pid: p.pid, ppid: p.ppid, depth, startedAt: p.startedAt, kind: placed.kindOf(p), name: p.comm, command: p.cmdline === "" ? p.comm : p.cmdline, memory: p.rss, cpu: p.cpu, ports: [...(ports?.get(p.pid) ?? [])] });
      walk(placed.kidsOf(p.pid).filter(k => inside.has(k.pid)), depth + 1);
    }
  };
  walk(heads, 0);
  return out;
}

/** The rungs that name a process a thread started; the folder and the coalition name only where it runs, which the
 * person's own terminal shares, so a row they alone own is not wsp's to list or end. */
const STARTED_BY = new Set<string>(["root", "fork", "group", "env", "cgroup"]);
const startedByWsp = (p: ProcEntry): boolean => p.owner !== undefined && STARTED_BY.has(p.owner.by);

/** The computer's processes by thread, each thread in the order the host named it and only where it has any. */
export function activityOf(read: ActivityRead): ActivityView {
  const placed = place(read);
  const threads: ActivityThread[] = [];
  for (const scope of read.scopes) {
    const members = read.procs.filter(p => placed.threadOf.get(p.pid) === scope);
    if (members.length === 0) continue;
    let memory = 0;
    let cpu = 0;
    for (const p of members) {
      memory += p.rss;
      cpu += p.cpu;
    }
    threads.push({ threadId: scope.threadId, title: scope.title, workspaceId: scope.workspaceId, memory, cpu, processes: treeRows(placed, members, read.ports) });
  }
  const free = read.procs.filter(p => !placed.threadOf.has(p.pid));
  const wsp = free.filter(p => placed.kindOf(p) === "wsp");
  const unowned = free.filter(p => startedByWsp(p) && placed.kindOf(p) !== "wsp");
  return { computer: read.computer, at: read.at, threads, wsp: treeRows(placed, wsp, read.ports), unowned: treeRows(placed, unowned, read.ports) };
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
  const placed = place(read);
  const p = placed.byPid.get(pid);
  if (p === undefined) throw refusal(endGoneLine(pid), END_AGAIN_FIX, "not-found");
  // Reach before the start, so a caller learns nothing of a process it may not end, not even its name.
  const owner = placed.threadOf.get(pid);
  if (!may(owner?.threadId)) throw refusal(notThreadsLine(pid), NOT_THREADS_FIX, "usage");
  if (pid === read.daemon || pid === read.host) throw refusal(wspOwnLine(pid), WSP_OWN_FIX, "usage");
  if (owner === undefined && !startedByWsp(p)) throw refusal(notStartedLine(pid), NOT_STARTED_FIX, "usage");
  if (Math.abs(p.startedAt - startedAt) > SAME_START_MS) throw refusal(endReusedLine(pid, p, startedAt), END_AGAIN_FIX, "not-found");
  const agent = placed.agentOf.get(pid);
  if (agent !== undefined) {
    if (agent.kept !== undefined && placed.rootOf.get(pid) === agent.kept) return { kept: agent };
    throw refusal(agentEndLine(pid, agent), agentEndFix(agent), "usage");
  }
  if (p.kind === "agent") {
    const scope = placed.threadOf.get(pid);
    if (scope !== undefined) throw refusal(agentEndLine(pid, scope), agentEndFix(scope), "usage");
    throw refusal(`${pid} is an agent, which only its thread's stop ends:`, "stop its thread's turn with wsp stop.", "usage");
  }
  const targets: ProcEntry[] = [];
  const take = (q: ProcEntry): void => {
    const under = placed.agentOf.get(q.pid);
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

export const processesLeftLine = (verb: "settle" | "delete", left: readonly ActivityThread[]): string => {
  const rows = left.flatMap(t => t.processes.map(p => ({ t, p })));
  const named = rows.slice(0, LEFT_NAMED).map(({ t, p }) => `${p.pid} ${p.name} (${p.kind}${p.ports.length > 0 ? `, :${p.ports.join(" :")}` : ""}) of ${threadWord(t.threadId)}`);
  const more = rows.length > LEFT_NAMED ? ` and ${rows.length - LEFT_NAMED} more` : "";
  return `${verb === "settle" ? "a thread to settle" : "the thread to delete"} still runs ${rows.length === 1 ? "a process" : `${rows.length} processes`}: ${named.join(", ")}${more}:`;
};

export const processesLeftFix = (verb: "settle" | "delete"): string =>
  `${verb} it again with processes stop to end them first, or processes keep to leave them running (--processes on the command line).`;

export const processesLeftRefusal = (verb: "settle" | "delete", left: readonly ActivityThread[]): Error => refusal(processesLeftLine(verb, left), processesLeftFix(verb), "usage");
