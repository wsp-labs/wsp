// SPDX-License-Identifier: AGPL-3.0-only
// The process table's order: a tree by ppid, siblings sorted by the chosen
// column. A filter keeps that shape rather than flattening it: a match is
// shown under its parents, so a row whose command is twenty characters of the
// same flags is still told apart by what started it.
import { CATALOG_AGENTS } from "@wsp/catalog";
import { appProcLine, byProcColumn, placeProcs, procBins, RUN_SCRIPT, type AppProc, type ProcEntry, type ProcSort } from "@wsp/protocol";

export type { ProcSort };

/** A thread with a turn's process on this computer, which its agent's row stops through. */
export interface ProcThread {
  threadId: string;
  title: string;
  /** The process the host started for the turn. */
  pid: number;
  /** The running turn's, which its stop names; absent while none runs. */
  sessionId?: string;
  /** The agent the turn's run recorded as it started it, under pid. */
  agentPid?: number;
}

export interface ProcRow {
  proc: ProcEntry;
  depth: number;
}

const hit = (p: ProcEntry, needle: string): boolean => [String(p.pid), p.comm, p.cmdline, p.user].some(s => s.toLowerCase().includes(needle));

/** The table for one snapshot, nested by parent. Under a filter a process is shown when it matches or something under
 * it does, which is what keeps a match's parents above it. */
export function procTable(procs: readonly ProcEntry[], sort: ProcSort, filter: string): ProcRow[] {
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
  const keep = new Set<number>();
  const mark = (p: ProcEntry): boolean => {
    // Every child is read, never short-circuited: each one that stays has to be marked for the walk below.
    const under = kidsOf(p.pid).map(mark).includes(true);
    const stays = under || needle === "" || hit(p, needle);
    if (stays) keep.add(p.pid);
    return stays;
  };
  for (const root of roots) mark(root);
  const out: ProcRow[] = [];
  const walk = (list: readonly ProcEntry[], depth: number): void => {
    for (const proc of [...list].sort(byProcColumn(sort))) {
      if (!keep.has(proc.pid)) continue;
      out.push({ proc, depth });
      walk(kidsOf(proc.pid), depth + 1);
    }
  };
  walk(roots, 0);
  return out;
}

const NO_APP: ReadonlyMap<number, AppProc> = new Map();

const AGENT_BINS = CATALOG_AGENTS.map(a => a.bin);
const isAgent = (p: ProcEntry): boolean => procBins(p).some(b => AGENT_BINS.includes(b));

/** The rows that are a running turn's agent, each with its thread, as `placeProcs` reads them: the process the host
 * started for the turn and every process from it down to the agent its run recorded, or the agent's binary. A signal
 * to any of them ends the turn as failed, so the pane stops the thread instead. A tool server under the agent is none
 * of them. */
export function agentRows(procs: readonly ProcEntry[], threads: readonly ProcThread[]): Map<number, ProcThread> {
  const running = new Map(threads.filter(t => t.sessionId !== undefined).map(t => [t.threadId, t]));
  const placed = placeProcs({ procs, daemon: -1, threads: [...running.values()].map(t => ({ id: t.threadId, roots: [t.pid], ...(t.agentPid !== undefined ? { agents: [t.agentPid] } : {}) })), agentBins: AGENT_BINS, listens: () => false });
  const rows = new Map<number, ProcThread>();
  for (const [pid, id] of placed.agent) rows.set(pid, running.get(id)!);
  for (const [pid, at] of placed.foldsInto) rows.set(pid, running.get(placed.agent.get(at)!)!);
  return rows;
}

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
