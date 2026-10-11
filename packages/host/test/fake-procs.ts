// SPDX-License-Identifier: AGPL-3.0-only
// A daemon's process frames for the verbs' hosts: one table a case writes, sent whole on every proc.watch and
// proc.list, a listener per port, and a signal that takes its row away, so Activity and an End run with no process of this
// computer's touched.
import { unknownOpLine, type ProcEntry } from "@wsp/protocol";

/** The pid the fake daemon names as its own in every snapshot. */
export const FAKE_DAEMON_PID = 1;

export interface FakeProcs {
  procs: ProcEntry[];
  ports: { port: number; pid: number }[];
  signals: { pid: number; signal: string }[];
  /** Every process op asked, in order. */
  asked: string[];
  /** False plays a daemon a version behind, which has no proc.list. */
  lists: boolean;
  /** Pids a TERM leaves running. */
  stubborn: Set<number>;
  /** Plays a running sampler between its ticks: proc.list and proc.inspect answer the rows and the clock of this
   * moment for ms, whatever a signal did since. */
  hold(ms: number): void;
  /** The daemon's answer to a process frame, or nothing for any other op. */
  answer(frame: { op: string } & Record<string, unknown>, onEvent: (event: Record<string, unknown>) => void): Record<string, unknown> | undefined;
}

export function fakeProcs(): FakeProcs {
  let tick: { at: number; procs: ProcEntry[] } | undefined;
  const listed = (): { at: number; procs: ProcEntry[] } => tick ?? { at: Date.now(), procs: f.procs.map(p => ({ ...p })) };
  const f: FakeProcs = {
    procs: [],
    ports: [],
    signals: [],
    asked: [],
    lists: true,
    stubborn: new Set(),
    hold(ms) {
      const held = { at: Date.now(), procs: f.procs.map(p => ({ ...p })) };
      tick = held;
      setTimeout(() => {
        if (tick === held) tick = undefined;
      }, ms);
    },
    answer(frame, onEvent) {
      const pid = frame["pid"];
      if (frame.op.startsWith("proc.")) f.asked.push(frame.op);
      const gone = { id: 1, ok: false, code: "not-found", error: `no process ${String(pid)}` };
      switch (frame.op) {
        case "proc.watch":
          queueMicrotask(() => onEvent({ type: "proc.snapshot", at: Date.now(), daemon: FAKE_DAEMON_PID, total: f.procs.length, procs: f.procs.map(p => ({ ...p })) }));
          return { id: 1, ok: true };
        case "proc.list":
          return f.lists ? { id: 1, ok: true, daemon: FAKE_DAEMON_PID, total: listed().procs.length, ...listed() } : { id: 1, ok: false, error: unknownOpLine("proc.list") };
        case "proc.unwatch":
          return { id: 1, ok: true };
        case "ports.watch":
          return { id: 1, ok: true, ports: f.ports.map(p => ({ port: p.port, pid: p.pid, uid: 0, loopback: false })) };
        case "proc.inspect":
          return listed().procs.some(p => p.pid === pid) ? { id: 1, ok: true, pid, cwd: null, ports: [], children: [] } : gone;
        case "proc.kill": {
          f.signals.push({ pid: Number(pid), signal: String(frame["signal"]) });
          const had = f.procs.some(p => p.pid === pid);
          if (frame["signal"] === "KILL" || !f.stubborn.has(Number(pid))) f.procs = f.procs.filter(p => p.pid !== pid);
          return had ? { id: 1, ok: true } : gone;
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
