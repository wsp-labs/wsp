// SPDX-License-Identifier: AGPL-3.0-only
// The panes of a folder on a computer the person joined, over the one link that
// computer holds to this host. Its daemon reads the whole computer and keeps one
// watch of each kind per socket, and every folder there rides that one socket,
// the host's own sign-in watch among them: so the ports a folder's threads hold
// are a watch of its own named for the folder's record, its threads' cgroups in
// its scope, the processes are one watch per link that every pane there
// shares, reading what the panes asked between them, the readings are the
// computer's one watch that every reader on this host shares, and a terminal
// opens in the folder as the login the computer was added with, under the
// environment its turns get.
import { DAEMON_VERSION, inCgroup, placeBehindLine, placeDaemonBehind, threadCgroup } from "@wsp/protocol";
import type { LiveWorkspace } from "../types/wiring.js";
import type { RuntimeContext } from "../context.js";

type Frame = Record<string, unknown>;
type Served = (frame: Frame) => Promise<Frame>;

/** The most cgroups one watch names, the daemon's own cap: a folder with more threads names the rest in further watches
 * of its own on the same socket, so no thread's server drops out of its ports however many threads it has had. */
const WATCH_CGROUPS_MAX = 16;

/** What every pane on one link asked its process watch to read, as one watch: every process once any pane asks for them
 * all, else each cgroup a pane named, a cgroup under another one named taking nothing more, and the whole table past the
 * daemon's cap. */
export function procWatchScope(asks: Iterable<readonly string[] | null>): string[] | null {
  const named = new Set<string>();
  for (const ask of asks) {
    if (ask === null) return null;
    for (const cg of ask) named.add(cg);
  }
  const kept = [...named].filter(cg => ![...named].some(other => other !== cg && inCgroup(cg, other))).sort();
  return kept.length > WATCH_CGROUPS_MAX ? null : kept;
}

/** What a pane's channel into a folder there answers itself rather than passing up as it came. */
const PANEL_OPS: ReadonlySet<string> = new Set(["ports.watch", "sys.watch", "sys.unwatch", "proc.watch", "proc.unwatch", "proc.inspect", "proc.kill", "container.stop", "pty.create"]);

export interface BoxPanels {
  answers(op: string): boolean;
  send(frame: Frame): Promise<Frame>;
  /** Whether an event the link carried is this pane's: its folder's ports, and the readings and processes it watches. */
  heard(event: Frame): boolean;
  close(): void;
}

/** The refusal of a pane's own port watch on a computer whose daemon cannot name one: a daemon too old, or one that
 * never said its version, would take the pane's watch for the socket's own and drop the host's. */
export function namedWatchRefusal(name: string, daemonVersion: number | undefined): Frame | undefined {
  const gap = daemonVersion === undefined ? `daemon version unknown, host ${DAEMON_VERSION}` : placeDaemonBehind({ daemonVersion });
  return gap === undefined ? undefined : { ok: false, code: "unsupported", error: placeBehindLine(name, gap) };
}

/** One opener per runtime, holding what every pane on one computer shares: how many panes watch processes over each
 * of its links. `watchers` is the push each channel registers under its workspace to name its ports again when a
 * thread there starts. `placements` is the watch each thread's cgroup is in, per folder, kept while the thread is: the
 * watch names are the folder's on every pane, and one that moved would close its servers in one watch and open them in
 * another, so a thread that starts takes room where there is some. */
export function boxPanelsOpener(ctx: RuntimeContext, watchers: Map<string, Set<() => void>>, placements: Map<string, Map<string, number>>): (entry: LiveWorkspace, placeId: string, served: Served, folder: string) => BoxPanels {
  /** Each link's watching panes and what each asked its process watch to read. */
  const procAsks = new WeakMap<object, Map<object, readonly string[] | null>>();
  ctx.bus.on("workspace.deleted", e => {
    if (e.type === "workspace.deleted") placements.delete(e.workspaceId);
  });

  /** The cgroups of the folder's threads, newest first. */
  const cgroupsOf = (workspaceId: string): string[] => {
    const newest = new Map<string, number>();
    for (const { view } of ctx.sessions.values()) {
      if (view.workspaceId !== workspaceId || view.threadId === undefined) continue;
      newest.set(view.threadId, Math.max(newest.get(view.threadId) ?? 0, view.startedAt ?? 0));
    }
    for (const [threadId, record] of ctx.threadRecords) if (record.workspaceId === workspaceId && !newest.has(threadId)) newest.set(threadId, 0);
    return [...newest].sort((a, b) => b[1] - a[1]).map(([threadId]) => threadCgroup(threadId));
  };

  return (entry, placeId, served, folder) => {
    const door = ctx.placeDoor!;
    const workspaceId = entry.record.id;
    /** The link this pane rides: what it watches is counted against that link alone, so a link that dropped takes
     * nothing of its count onto the one that replaced it. */
    const link = door.link(placeId);
    let watchingProcs = false;
    const pane = {};
    const asksOn = (at: object): Map<object, readonly string[] | null> => procAsks.get(at) ?? procAsks.set(at, new Map()).get(at)!;
    /** The one watch the daemon holds for every pane on the link, asked again with what they now read between them. */
    const watchOnLink = (): Promise<Frame> => {
      const scope = link === undefined ? null : procWatchScope(asksOn(link).values());
      return served({ op: "proc.watch", ...(scope === null ? {} : { cgroups: scope }) });
    };
    /** This pane's hold on the computer's readings watch; the pane hears the samples on its own channel. */
    let readingsHeld: Promise<() => void> | undefined;
    let closed = false;
    /** The roots and cgroups this pane's ports watch last named, undefined until it watches. */
    let named: { roots: number[]; cgroups: string } | undefined;
    /** The threads this pane last named for the computer's process rows, undefined until it watches them. */
    let scoped: string | undefined;
    /** The folder's watches on the socket, the first its own id: the first carries the roots and the folder, each a
     * share of the cgroups. One no longer needed is named again over nothing. */
    let watches: string[] = [];
    const placed = placements.get(workspaceId) ?? new Map<string, number>();
    placements.set(workspaceId, placed);
    const watchName = (n: number): string => (n === 0 ? workspaceId : `${workspaceId}-${n + 1}`);
    /** Which of the folder's watches hold each port: a server in the folder and in a thread's cgroup is in two, and
     * the pane hears it open once and close once. */
    const holders = new Map<number, Set<string>>();

    const behind = async (): Promise<Frame | undefined> => namedWatchRefusal(door.nameOf(placeId), (await door.reportOf(placeId))?.daemonVersion);
    /** The roots are the terminals the folder's panes opened, which the channel under this one names. */
    const watchPorts = async (roots: number[]): Promise<Frame> => {
      const cgroups = cgroupsOf(workspaceId);
      named = { roots, cgroups: cgroups.join(",") };
      for (const cgroup of placed.keys()) if (!cgroups.includes(cgroup)) placed.delete(cgroup);
      const room: number[] = [];
      for (const n of placed.values()) room[n] = (room[n] ?? 0) + 1;
      for (const cgroup of cgroups) {
        if (placed.has(cgroup)) continue;
        let n = 0;
        while ((room[n] ?? 0) >= WATCH_CGROUPS_MAX) n += 1;
        placed.set(cgroup, n);
        room[n] = (room[n] ?? 0) + 1;
      }
      const shares = Math.max(1, room.length);
      const names = Array.from({ length: Math.max(shares, watches.length) }, (_, n) => watchName(n));
      watches = names.slice(0, shares);
      const said = await Promise.all(
        names.map((watch, n) => {
          const share = cgroups.filter(cgroup => placed.get(cgroup) === n);
          return served(n === 0 ? { op: "ports.watch", roots, folder, cgroups: share, watch } : { op: "ports.watch", roots: [], cgroups: share, watch });
        }),
      );
      const refused = said.find(reply => reply["ok"] !== true);
      if (refused !== undefined) return refused;
      holders.clear();
      const ports = new Map<number, unknown>();
      said.slice(0, shares).forEach((reply, n) => {
        for (const row of Array.isArray(reply["ports"]) ? (reply["ports"] as Frame[]) : []) {
          const port = Number(row["port"]);
          ports.set(port, ports.get(port) ?? row);
          holders.set(port, (holders.get(port) ?? new Set()).add(watchName(n)));
        }
      });
      return { ...said[0], ports: [...ports.values()] };
    };
    /** Whether a port event from one of the folder's watches is news to the pane. */
    const portMoved = (event: Frame): boolean => {
      const watch = String(event["watch"]);
      if (!watches.includes(watch)) return false;
      const port = Number(event["port"]);
      const held = holders.get(port) ?? new Set<string>();
      if (event["type"] === "port.open") {
        const elsewhere = [...held].some(other => other !== watch);
        holders.set(port, held.add(watch));
        return !elsewhere;
      }
      if (!held.delete(watch)) return held.size === 0;
      if (held.size > 0) return false;
      holders.delete(port);
      return true;
    };
    /** Every thread on the computer, each by its cgroup, for the owner the daemon stamps on its process rows; a
     * daemon from before proc.scopes refuses it and its rows carry none. */
    const scopeProcs = async (): Promise<void> => {
      const scopes = ctx.scopesOn(placeId);
      const said = JSON.stringify(scopes);
      if (said === scoped) return;
      scoped = said;
      await served({ op: "proc.scopes", scopes }).catch(() => undefined);
    };
    /** A thread that started since names its cgroup in the watch again; the daemon answers the moves as events. */
    const push = (): void => {
      if (watchingProcs) void scopeProcs();
      if (named === undefined || cgroupsOf(workspaceId).join(",") === named.cgroups) return;
      void watchPorts(named.roots).catch(() => undefined);
    };
    const pushes = watchers.get(workspaceId) ?? new Set<() => void>();
    watchers.set(workspaceId, pushes);
    pushes.add(push);

    const unwatchProcs = (): void => {
      if (!watchingProcs) return;
      watchingProcs = false;
      if (link === undefined) return;
      const asks = asksOn(link);
      const before = procWatchScope(asks.values());
      asks.delete(pane);
      if (door.link(placeId) !== link) return;
      if (asks.size === 0) void served({ op: "proc.unwatch" }).catch(() => undefined);
      else if (JSON.stringify(procWatchScope(asks.values())) !== JSON.stringify(before)) void watchOnLink().catch(() => undefined);
    };

    const unwatchReadings = (): void => {
      void readingsHeld?.then(release => release(), () => undefined);
      readingsHeld = undefined;
    };

    const answer = async (frame: Frame): Promise<Frame> => {
      switch (frame["op"]) {
        case "ports.watch": {
          const refused = await behind();
          if (refused !== undefined) return refused;
          const roots = Array.isArray(frame["roots"]) ? frame["roots"].filter((r): r is number => Number.isInteger(r)) : [];
          return watchPorts(roots);
        }
        case "sys.watch": {
          const holding = readingsHeld ?? door.watchSys(placeId, () => {});
          readingsHeld = holding;
          return holding.then(
            () => ({ ok: true }),
            (e: unknown) => {
              if (readingsHeld === holding) readingsHeld = undefined;
              return { ok: false, error: e instanceof Error ? e.message : String(e) };
            },
          );
        }
        case "sys.unwatch":
          unwatchReadings();
          return { ok: true };
        case "proc.watch": {
          const cgroups = Array.isArray(frame["cgroups"]) ? frame["cgroups"].filter((c): c is string => typeof c === "string") : null;
          watchingProcs = true;
          if (link !== undefined) asksOn(link).set(pane, cgroups);
          scoped = undefined;
          await scopeProcs();
          // Asked again on a link already watching, the daemon sends the whole list next, which every pane can take.
          const said = await watchOnLink();
          if (said["ok"] !== true) unwatchProcs();
          return said;
        }
        case "proc.unwatch":
          unwatchProcs();
          return { ok: true };
        case "pty.create": {
          const asked = typeof frame["env"] === "object" && frame["env"] !== null ? (frame["env"] as Record<string, string>) : {};
          const env = { ...ctx.threadEnv(entry), ...asked };
          return served({ ...frame, cwd: typeof frame["cwd"] === "string" ? frame["cwd"] : folder, env });
        }
        default:
          return served(frame);
      }
    };

    return {
      answers: op => PANEL_OPS.has(op),
      send: answer,
      heard: event => {
        const type = event["type"];
        if (type === "port.open" || type === "port.close") return portMoved(event);
        if (type === "sys.sample") return readingsHeld !== undefined;
        if (type === "proc.snapshot" || type === "proc.changes") return watchingProcs;
        return false;
      },
      close: () => {
        if (closed) return;
        closed = true;
        pushes.delete(push);
        unwatchProcs();
        unwatchReadings();
      },
    };
  };
}
