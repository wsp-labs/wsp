// SPDX-License-Identifier: AGPL-3.0-only
// A daemon's process frames for the verbs' hosts: one table a case writes, sent whole on every proc.watch and
// proc.list, a listener per port, and a signal that takes its row away, so Activity and an End run with no process of this
// computer's touched. A tree kill is checked against the row's identity and start, takes the pid and everything under
// it, and sends KILL itself to what TERM left, as the daemon does.
import { SAME_START_MS, unknownOpLine, type ProcEntry } from "@wsp/protocol";

/** The pid the fake daemon names as its own in every snapshot. */
export const FAKE_DAEMON_PID = 1;

export interface FakeProcs {
  procs: ProcEntry[];
  ports: { port: number; pid: number }[];
  signals: { pid: number; signal: string }[];
  /** Every proc.kill frame whole, as it reached the daemon. */
  kills: Record<string, unknown>[];
  /** Every process op asked, in order. */
  asked: string[];
  /** False plays a daemon a version behind, which has no proc.list. */
  lists: boolean;
  /** Pids a TERM leaves running, which a tree kill's KILL after the grace ends. */
  stubborn: Set<number>;
  /** False plays a daemon from before kill modes: it signals the pid alone and answers no field. */
  modes: boolean;
  /** Runs once after the next snapshot or list is answered, between a host's read and its signal. */
  afterRead: (() => void) | undefined;
  /** The daemon's answer to a process frame, or nothing for any other op. */
  answer(frame: { op: string } & Record<string, unknown>, onEvent: (event: Record<string, unknown>) => void): Record<string, unknown> | undefined;
}

export function fakeProcs(): FakeProcs {
  const listed = (): { at: number; procs: ProcEntry[] } => ({ at: Date.now(), procs: f.procs.map(p => ({ ...p })) });
  const afterRead = (): void => {
    const after = f.afterRead;
    f.afterRead = undefined;
    after?.();
  };
  const treeOf = (pid: number): number[] => [pid, ...f.procs.filter(p => p.ppid === pid && p.pid !== pid).flatMap(p => treeOf(p.pid))];
  const f: FakeProcs = {
    procs: [],
    ports: [],
    signals: [],
    kills: [],
    asked: [],
    lists: true,
    stubborn: new Set(),
    modes: true,
    afterRead: undefined,
    answer(frame, onEvent) {
      const pid = frame["pid"];
      if (frame.op.startsWith("proc.")) f.asked.push(frame.op);
      const gone = { id: 1, ok: false, code: "not-found", error: `no process ${String(pid)}` };
      switch (frame.op) {
        case "proc.watch":
          queueMicrotask(() => {
            onEvent({ type: "proc.snapshot", at: Date.now(), daemon: FAKE_DAEMON_PID, total: f.procs.length, procs: f.procs.map(p => ({ ...p })) });
            afterRead();
          });
          return { id: 1, ok: true };
        case "proc.list":
        {
          if (!f.lists) return { id: 1, ok: false, error: unknownOpLine("proc.list") };
          const answered = { id: 1, ok: true, daemon: FAKE_DAEMON_PID, total: f.procs.length, ...listed() };
          afterRead();
          return answered;
        }
        case "proc.unwatch":
          return { id: 1, ok: true };
        case "ports.watch":
          return { id: 1, ok: true, ports: f.ports.map(p => ({ port: p.port, pid: p.pid, uid: 0, loopback: false })) };
        case "proc.inspect":
          return listed().procs.some(p => p.pid === pid) ? { id: 1, ok: true, pid, cwd: null, ports: [], children: [] } : gone;
        case "proc.kill": {
          f.kills.push(frame);
          const row = f.procs.find(p => p.pid === pid);
          if (row === undefined) return gone;
          const notListed = { id: 1, ok: false, code: "not-found", error: `pid ${String(pid)} is not the process listed` };
          const identity = frame["identity"];
          const startedAt = frame["startedAt"];
          if (f.modes && typeof identity === "string" && identity !== row.id) return notListed;
          if (f.modes && identity === undefined && typeof startedAt === "number" && Math.abs(row.startedAt - startedAt) > SAME_START_MS) return notListed;
          const signal = String(frame["signal"]);
          const taken = f.modes && frame["mode"] === "tree" ? treeOf(row.pid) : [row.pid];
          for (const t of taken) f.signals.push({ pid: t, signal });
          const forced = f.modes && signal === "TERM" ? taken.filter(t => f.stubborn.has(t)) : [];
          for (const t of forced) f.signals.push({ pid: t, signal: "KILL" });
          f.procs = f.procs.filter(p => !taken.includes(p.pid) || (signal === "TERM" && !f.modes && f.stubborn.has(p.pid)));
          return { id: 1, ok: true, ...(f.modes ? { signalled: taken, forced, left: [] } : {}) };
        }
        default:
          return undefined;
      }
    },
  };
  return f;
}

/** One row of the table, a quiet node process unless the case says otherwise. */
export const procRow = (row: Partial<ProcEntry> & { pid: number }): ProcEntry => ({ ppid: FAKE_DAEMON_PID, user: "ada", state: "S", comm: "node", cmdline: "node", cpu: 0, rss: 1_000_000, startedAt: 1_760_000_000_000, ...row });
