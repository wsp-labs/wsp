// SPDX-License-Identifier: AGPL-3.0-only
// The process table's order: a tree by ppid, siblings sorted by the chosen
// column, and on the computer the host runs on the threads of this workspace
// first, each one the tree its own process leads. A filter keeps that shape
// rather than flattening it: a match is shown under its parents, so a row
// whose command is twenty characters of the same flags is still told apart by
// what started it.
import { CATALOG_AGENTS } from "@wsp/catalog";
import { appProcLine, byProcColumn, inCgroup, type AppProc, type ProcEntry, type ProcSort } from "@wsp/protocol";

export type { ProcSort };

/** One thread of this workspace: the process the host started for it, which that thread's tree is rooted at, or on a
 * computer the person joined, the cgroup every process it started stands in. */
export type ProcThread = {
  threadId: string;
  title: string;
  /** The running turn's, which its stop names; absent while none runs. */
  sessionId?: string;
  /** The agent the turn's run recorded as it started it, under pid. */
  agentPid?: number;
} & (
  | { pid: number; cgroup?: never }
  | { cgroup: string; pid?: never }
);

export interface ProcRow {
  proc: ProcEntry;
  depth: number;
}

/** One thread's own tree: the process the host started for it and everything under it. A thread known by its cgroup
 * carries what that cgroup's processes spend together, whatever the filter shows of them. */
export interface ProcThreadTree {
  thread: ProcThread;
  rows: ProcRow[];
  spent?: { cpu: number; rss: number };
}

/** What the pane draws: this workspace's threads first, then everything else the machine is running. A process
 * under a thread's tree is in no other, so the rows can be counted by adding the two up. */
export interface ProcTable {
  threads: ProcThreadTree[];
  rest: ProcRow[];
}

const hit = (p: ProcEntry, needle: string): boolean => [String(p.pid), p.comm, p.cmdline, p.user].some(s => s.toLowerCase().includes(needle));

/**
 * The table for one snapshot: `threads` are the trees this workspace's own threads lead, in the order they were
 * given, and `rest` is every other process on the machine, nested by parent. A thread whose process is not in the
 * snapshot has no tree. Under a filter a process is shown when it matches or something under it does, which is what
 * keeps a match's parents above it.
 */
export function procTable(procs: readonly ProcEntry[], sort: ProcSort, filter: string, threads: readonly ProcThread[] = []): ProcTable {
  const needle = filter.trim().toLowerCase();
  const pids = new Set(procs.map(p => p.pid));
  const children = new Map<number, ProcEntry[]>();
  const roots: ProcEntry[] = [];
  for (const p of procs) {
    if (p.ppid === p.pid || !pids.has(p.ppid)) roots.push(p);
    else {
      const list = children.get(p.ppid);
      if (list) list.push(p);
      else children.set(p.ppid, [p]);
    }
  }
  const kidsOf = (pid: number): ProcEntry[] => children.get(pid) ?? [];

  /** The rows for one forest, with a set of pids that belong to somebody else's tree left out of it whole. Each
   * scope marks its own keepers: a process is drawn when it matches or something under it does, so a parent stands
   * above its match and a tree whose only match was claimed elsewhere is not drawn for an empty reason. */
  const rowsOf = (heads: readonly ProcEntry[], skip: ReadonlySet<number>): ProcRow[] => {
    const keep = new Set<number>();
    const mark = (p: ProcEntry): boolean => {
      if (skip.has(p.pid)) return false;
      // Every child is read, never short-circuited: each one that stays has to be marked for the walk below.
      const under = kidsOf(p.pid).map(mark).includes(true);
      const stays = under || needle === "" || hit(p, needle);
      if (stays) keep.add(p.pid);
      return stays;
    };
    for (const head of heads) mark(head);
    const out: ProcRow[] = [];
    const walk = (list: readonly ProcEntry[], depth: number): void => {
      for (const proc of [...list].sort(byProcColumn(sort))) {
        if (!keep.has(proc.pid)) continue;
        out.push({ proc, depth });
        walk(kidsOf(proc.pid), depth + 1);
      }
    };
    walk(heads, 0);
    return out;
  };

  const byPid = new Map(procs.map(p => [p.pid, p]));
  const claimed = new Set<number>();
  const claim = (p: ProcEntry): void => {
    claimed.add(p.pid);
    for (const kid of kidsOf(p.pid)) claim(kid);
  };
  const trees: ProcThreadTree[] = [];
  for (const thread of threads) {
    if (thread.cgroup !== undefined) {
      // Every process in the cgroup, each drawn under the first of its parents that stands there too: a server the
      // thread detached heads a tree of its own beside the turn's.
      const members = procs.filter(p => !claimed.has(p.pid) && inCgroup(p.cgroup, thread.cgroup));
      if (members.length === 0) continue;
      const inside = new Set(members.map(p => p.pid));
      const heads = members.filter(p => p.ppid === p.pid || !inside.has(p.ppid));
      const rows = rowsOf(heads, claimed);
      for (const head of heads) claim(head);
      let cpu = 0;
      let rss = 0;
      for (const p of members) {
        cpu += p.cpu;
        rss += p.rss;
      }
      if (rows.length > 0) trees.push({ thread, rows, spent: { cpu, rss } });
      continue;
    }
    const head = byPid.get(thread.pid);
    // A thread whose process already sits inside an earlier thread's tree is drawn there and not a second time.
    if (head === undefined || claimed.has(head.pid)) continue;
    const rows = rowsOf([head], claimed);
    // Claimed whether or not the filter left anything of it: what a filter hid here is hidden, not moved into the
    // rest of the machine. A thread with nothing left under a filter drops its heading with its rows.
    claim(head);
    if (rows.length > 0) trees.push({ thread, rows });
  }
  const rest = rowsOf(roots, claimed);
  return { threads: trees, rest };
}

const NO_APP: ReadonlyMap<number, AppProc> = new Map();

/** argv[0]'s basename off the row's command line, the name a process was run by: a native Claude Code runs as comm
 * `2.1.296` through the link named claude, cursor-agent as node under argv[0] cursor-agent. */
const runAs = (p: ProcEntry): string => p.cmdline.split(" ", 1)[0]!.split("/").pop()!;

const isAgent = (p: ProcEntry): boolean => CATALOG_AGENTS.some(a => a.bin === p.comm || a.bin === runAs(p));

/** The pids from p up to the one under root, or null where root is not above p. */
function pathUp(byPid: ReadonlyMap<number, ProcEntry>, p: ProcEntry, root: number): number[] | null {
  const path: number[] = [];
  for (let at: ProcEntry | undefined = p; at !== undefined; at = byPid.get(at.ppid)) {
    if (at.pid === root) return path;
    if (path.includes(at.pid)) return null;
    path.push(at.pid);
  }
  return null;
}

/** The rows that are a running turn's agent, each with its thread: the process the host started for the turn and
 * every process from it down to the agent its run recorded, or where none was recorded down to each process run as an
 * agent's binary; on a computer the person joined each agent binary in the thread's cgroup and what stands above it there. A signal to any of them
 * ends the turn as failed, so the pane stops the thread instead. A tool server under the agent is none of them. */
export function agentRows(procs: readonly ProcEntry[], threads: readonly ProcThread[]): Map<number, ProcThread> {
  const byPid = new Map(procs.map(p => [p.pid, p]));
  const rows = new Map<number, ProcThread>();
  for (const thread of threads) {
    if (thread.sessionId === undefined) continue;
    if (thread.cgroup !== undefined) {
      // A box run records no agent: its run script and shell stand above the agent binary in the thread's cgroup.
      for (const p of procs) {
        if (!isAgent(p) || !inCgroup(p.cgroup, thread.cgroup)) continue;
        let at: ProcEntry | undefined = p;
        while (at !== undefined && inCgroup(at.cgroup, thread.cgroup) && !rows.has(at.pid)) {
          rows.set(at.pid, thread);
          at = byPid.get(at.ppid);
        }
      }
      continue;
    }
    if (!byPid.has(thread.pid)) continue;
    rows.set(thread.pid, thread);
    const recorded = thread.agentPid === undefined ? undefined : byPid.get(thread.agentPid);
    const known = recorded === undefined ? null : pathUp(byPid, recorded, thread.pid);
    // A recorded process that runs as no agent's name is a launcher, npm's codex.js under node: the agent's own binary
    // it starts is the agent too.
    const launched = known === null || isAgent(recorded!) ? [] : procs.filter(p => isAgent(p) && pathUp(byPid, p, recorded!.pid) !== null);
    const agents = known !== null ? [recorded!, ...launched] : procs.filter(isAgent);
    for (const p of agents) for (const pid of pathUp(byPid, p, thread.pid) ?? []) rows.set(pid, thread);
  }
  return rows;
}

/** A turn's run script as either road starts it: `bash <run folder>/<12 hex>.sh`. */
const RUN_SCRIPT = /^bash \S*\/[0-9a-f]{12}\.sh$/;

/** Every process from a turn's run script down to the first process under it run as an agent's binary, whatever
 * thread it is: on a computer whose turns stand in no cgroup a row there cannot be tied to its thread, and a signal
 * to any of them still ends the turn as failed, so none of them is offered kill. */
export function runAgentPids(procs: readonly ProcEntry[]): Set<number> {
  const byPid = new Map(procs.map(p => [p.pid, p]));
  const pids = new Set<number>();
  for (const p of procs) {
    if (!isAgent(p)) continue;
    const path: number[] = [];
    for (let at = byPid.get(p.ppid); at !== undefined && !path.includes(at.pid); at = byPid.get(at.ppid)) {
      // An agent under another agent below the run is that agent's own child, killable as any tool is.
      if (isAgent(at)) break;
      path.push(at.pid);
      // The run's shell is a fork of the script and reads as it, so the run is every such process up the chain.
      if (RUN_SCRIPT.test(at.cmdline) && !RUN_SCRIPT.test(byPid.get(at.ppid)?.cmdline ?? "")) {
        for (const pid of [p.pid, ...path]) pids.add(pid);
        break;
      }
    }
  }
  return pids;
}

/** The harness pieces the daemon can name: itself, the shells behind its ptys (by the tab's title) and an agent by the
 * command its catalog entry puts on PATH; and the desktop app's own processes, which only the app can name. */
export function procLabel(p: ProcEntry, daemonPid: number, terminalTitles: ReadonlyMap<string, string>, app: ReadonlyMap<number, AppProc> = NO_APP): string | null {
  if (p.pid === daemonPid) return "wsp";
  const part = app.get(p.pid);
  if (part !== undefined) return appProcLine(part);
  if (p.pty !== undefined) {
    const title = terminalTitles.get(p.pty);
    return title !== undefined ? `terminal ${title}` : "terminal";
  }
  if (isAgent(p)) return "agent";
  return null;
}
