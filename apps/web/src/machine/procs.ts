// SPDX-License-Identifier: AGPL-3.0-only
// Per-workspace process snapshots for the Processes pane. The daemon reads
// /proc only while a socket watches, so the pane asks with proc.watch when it
// mounts and proc.unwatch when it goes; it sends one whole snapshot and then
// only what changed, which this applies on the frame each names, asking again
// for a whole one when a frame was lost; terminal/wiring.ts feeds the pushes
// and the link's status here, and a link that comes back is asked again while
// a pane still wants it. A daemon that refuses proc.watch leaves its reason
// here, so the pane can say so instead of waiting.
import { useSyncExternalStore } from "react";
import { applyProcChanges, ProcInspectReply, ProcKillReply, type DaemonLinkStatus, type KillMode, type ProcChanges, type ProcSignal, type ProcSnapshot } from "@wsp/protocol";
import { getDaemonWire } from "../files/wire.js";
import { errorText } from "../lib/utils.js";

export interface ProcsState {
  snapshot: ProcSnapshot | null;
  reach: "live" | "unreachable";
  /** The daemon's refusal of proc.watch, null while it streams or has not been asked. */
  unavailable: string | null;
}

export class WorkspaceProcs {
  readonly #workspaceId: string;
  #state: ProcsState = { snapshot: null, reach: "unreachable", unavailable: null };
  #fns = new Set<() => void>();
  /** Each holder's ask: the cgroups it reads, or null for every process. */
  #holders = new Map<symbol, readonly string[] | null>();
  /** The ask last sent, as its key, so a holder that names the same set again sends nothing. */
  #asked: string | null = null;
  /** A whole snapshot has been asked for after a lost frame and has not come yet. */
  #resyncing = false;

  constructor(workspaceId: string) {
    this.#workspaceId = workspaceId;
  }

  /** A whole snapshot replaces what is held. Changes apply only on the frame they name: one that follows anything
   * else means a frame was lost, so the rows stay as they are and the daemon is asked once for a whole list. */
  feed(frame: ProcSnapshot | ProcChanges): void {
    if (frame.type === "proc.snapshot") {
      this.#resyncing = false;
      this.#set({ ...this.#state, snapshot: frame });
      return;
    }
    const next = this.#state.snapshot === null ? undefined : applyProcChanges(this.#state.snapshot, frame);
    if (next !== undefined) this.#set({ ...this.#state, snapshot: next });
    else if (!this.#resyncing && this.#holders.size > 0) {
      this.#resyncing = true;
      this.#request("proc.watch");
    }
  }

  feedStatus(s: DaemonLinkStatus): void {
    const reach = s === "live" ? "live" : "unreachable";
    if (reach === "live" && this.#holders.size > 0) this.#request("proc.watch");
    if (reach !== this.#state.reach) this.#set({ ...this.#state, reach });
  }

  /** A surface's hold on the stream, reading every process or only those in the cgroups it names; the first holder
   * starts it, a holder that changes what it reads asks again, and the last leaving ends it. */
  hold(cgroups: readonly string[] | null = null): { set(cgroups: readonly string[] | null): void; release(): void } {
    const me = Symbol("hold");
    this.#holders.set(me, cgroups);
    this.#reask();
    let released = false;
    return {
      set: next => {
        if (released) return;
        this.#holders.set(me, next);
        this.#reask();
      },
      release: () => {
        if (released) return;
        released = true;
        this.#holders.delete(me);
        if (this.#holders.size === 0) {
          this.#asked = null;
          if (this.#state.reach === "live") this.#request("proc.unwatch");
        } else this.#reask();
      },
    };
  }

  /** A hold on every process, released by the function it returns. */
  watch(): () => void {
    const held = this.hold();
    return held.release;
  }

  /** What every holder reads together: every process once any holder asks for it, else the cgroups they name. */
  #scope(): readonly string[] | null {
    const all = new Set<string>();
    for (const cgroups of this.#holders.values()) {
      if (cgroups === null) return null;
      for (const cg of cgroups) all.add(cg);
    }
    return [...all].sort();
  }

  #reask(): void {
    const key = JSON.stringify(this.#scope());
    if (key === this.#asked) return;
    if (this.#state.reach === "live") this.#request("proc.watch");
  }

  async inspect(pid: number): Promise<ProcInspectReply> {
    const wire = getDaemonWire(this.#workspaceId);
    if (!wire) throw new Error("not answering");
    return ProcInspectReply.parse(await wire.request("proc.inspect", { pid }));
  }

  /** Answers once the daemon is done: after TERM's grace and the KILL behind it where anything stayed. */
  async kill(pid: number, ask: { signal: ProcSignal; mode?: KillMode; startedAt?: number; identity?: string }): Promise<ProcKillReply> {
    const wire = getDaemonWire(this.#workspaceId);
    if (!wire) throw new Error("not answering");
    return ProcKillReply.parse(await wire.request("proc.kill", { pid, ...ask }));
  }

  /** Stops one container through its engine, TERM then KILL two seconds on, and never removes it. */
  async stopContainer(container: string): Promise<void> {
    const wire = getDaemonWire(this.#workspaceId);
    if (!wire) throw new Error("not answering");
    await wire.request("container.stop", { container });
  }

  onChange(fn: () => void): () => void {
    this.#fns.add(fn);
    return () => this.#fns.delete(fn);
  }

  /** The same object until a snapshot or the reach changes. */
  snapshot(): ProcsState {
    return this.#state;
  }

  #request(op: "proc.watch" | "proc.unwatch"): void {
    const wire = getDaemonWire(this.#workspaceId);
    if (!wire) return;
    if (op === "proc.unwatch") {
      wire.request(op).catch(() => {});
      return;
    }
    const scope = this.#scope();
    this.#asked = JSON.stringify(scope);
    wire.request(op, scope === null ? {} : { cgroups: [...scope] }).then(
      () => this.#setUnavailable(null),
      (e: unknown) => {
        // A refused re-watch brings no whole snapshot, so the next gap asks again.
        this.#resyncing = false;
        this.#setUnavailable(errorText(e));
      },
    );
  }

  #setUnavailable(reason: string | null): void {
    if (reason !== this.#state.unavailable) this.#set({ ...this.#state, unavailable: reason });
  }

  #set(next: ProcsState): void {
    this.#state = next;
    for (const fn of this.#fns) fn();
  }
}

const registry = new Map<string, WorkspaceProcs>();

export function getProcs(workspaceId: string): WorkspaceProcs {
  let procs = registry.get(workspaceId);
  if (!procs) {
    procs = new WorkspaceProcs(workspaceId);
    registry.set(workspaceId, procs);
  }
  return procs;
}

/** Test isolation: forget every workspace's snapshots. */
export function resetProcs(): void {
  registry.clear();
}

export function useWorkspaceProcs(workspaceId: string): ProcsState {
  return useSyncExternalStore(
    fn => getProcs(workspaceId).onChange(fn),
    () => getProcs(workspaceId).snapshot(),
  );
}
