// SPDX-License-Identifier: AGPL-3.0-only
// What Settings > Activity and a thread's Processes pane read alike: the
// threads on one computer, its frame read into groups with ports, the app's
// parts and slate titles, and the acts a row reaches with the two dialogs that
// ask first. Each caller holds its own watch on the daemon.
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { agentName } from "@wsp/catalog";
import { fmtBytes, foldThreads, placeOf, threadCgroup, threadUnread, waitingLine, type PlaceView, type SessionView, type ThreadView, type WorkspaceView } from "@wsp/protocol";
import { useThreadVerbs } from "../../actions/verbs.js";
import { copyText } from "../../actions/clipboard.js";
import { restingAge } from "../../components/status/restingAge.js";
import type { ThreadStatusInput } from "../../components/status/kinds/index.js";
import { AlertDialog, AlertDialogClose, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogPopup, AlertDialogTitle } from "../../components/ui/alert-dialog.js";
import { Button, NEUTRAL_RING } from "../../components/ui/button.js";
import { errorText } from "../../lib/utils.js";
import { holdAppProcs, useAppProcs } from "../../machine/appProcs.js";
import { usePortsOf } from "../../machine/ports.js";
import { getProcs, useWorkspaceProcs, type ProcsState } from "../../machine/procs.js";
import { addNotice } from "../../notices/store.js";
import { useStore } from "../../protocol/store.js";
import { openBrowserAt } from "../../rightPanelStore.js";
import { loadSlate, useSlateStore } from "../../slate/store.js";
import { ACTIVITY_WORDS as W } from "../format.js";
import { scopeOf, type ActivityContainer, type ActivityGroup, type ActivityPort, type ActivityProc, type ActivityScope, type ActivityThread } from "./model.js";
import { groupProcs, type Acts } from "./tables.js";

export interface ThreadsOn {
  readonly workspaces: readonly WorkspaceView[];
  readonly sessions: Readonly<Record<string, readonly SessionView[]>>;
  readonly places: readonly PlaceView[];
}

/** The threads on one computer, each as Activity groups by: its own process on the computer the host runs on, its
 * cgroup on a box. */
export function useActivityThreads(placeId: string, here: boolean, on: ThreadsOn): { threads: ActivityThread[]; views: ReadonlyMap<string, ThreadView> } {
  // The slates' titles alone, as one string: a slate's values move every run, its title seldom.
  const titled = useSlateStore(s => JSON.stringify(Object.entries(s.byThread).flatMap(([id, e]) => (e?.record?.document?.title === undefined ? [] : [[id, e.record.document.title]]))));
  const { workspaces, sessions, places } = on;
  return useMemo(() => {
    const slates = new Map<string, string>(JSON.parse(titled) as [string, string][]);
    const views = new Map<string, ThreadView>();
    const threads: ActivityThread[] = [];
    for (const w of workspaces) {
      if (placeOf(places, w)?.id !== placeId) continue;
      for (const t of foldThreads(sessions[w.id] ?? [])) {
        views.set(t.id, t);
        const slate = slates.get(t.id);
        threads.push({
          id: t.id,
          title: t.title,
          harness: t.harness,
          project: w.project.name,
          workspaceId: w.id,
          sessionId: t.sessionId,
          working: t.status === "running",
          ...(here ? { ...(t.pid === undefined ? {} : { pid: t.pid }), ...(t.agentPid === undefined ? {} : { agentPid: t.agentPid }) } : { cgroup: threadCgroup(t.id) }),
          ...(t.parentThreadId === undefined ? {} : { parent: t.parentThreadId }),
          ...(slate === undefined ? {} : { slate }),
          ...(t.subagents === undefined ? {} : { subagents: t.subagents }),
        });
      }
    }
    return { threads, views };
  }, [placeId, here, workspaces, sessions, places, titled]);
}

/** One computer's frame read into Activity's groups, over the link `key` names, with its ports, the app's own parts
 * where the host runs, and each slated thread's title read once. */
export function useActivityScope(key: string, here: boolean, threads: readonly ActivityThread[]): { procs: ProcsState; scope: ActivityScope | null } {
  const procs = useWorkspaceProcs(key);
  const known = usePortsOf(key);
  const ports = useMemo(() => {
    const byPid = new Map<number, ActivityPort[]>();
    for (const k of known) if (k.pid !== null) (byPid.get(k.pid) ?? byPid.set(k.pid, []).get(k.pid)!).push({ port: k.port, loopback: k.loopback ?? null });
    return byPid;
  }, [known]);
  // The app runs where the host runs, so only this computer's frame names its processes.
  useEffect(() => (here ? holdAppProcs() : undefined), [here]);
  const appProcs = useAppProcs();
  const app = here ? appProcs : undefined;
  const snapshot = procs.snapshot;
  const host = useMemo(() => {
    if (!here || snapshot === null) return undefined;
    const ppid = snapshot.procs.find(p => p.pid === snapshot.daemon)?.ppid;
    return ppid === undefined || ppid <= 1 ? undefined : ppid;
  }, [here, snapshot]);
  // Read again each frame, so a running subagent's time moves with the figures and an ended one leaves after its minute.
  const scope = useMemo(() => (snapshot === null ? null : scopeOf({ snapshot, threads, ports, now: Date.now(), ...(host === undefined ? {} : { host }), ...(app === undefined ? {} : { app }) })), [snapshot, threads, ports, host, app]);

  // A thread's slate is named by its title, read once for each thread with a slate run this window has not read.
  const slated = useMemo(() => (scope === null ? [] : scope.groups.flatMap(g => (g.thread !== undefined && g.procs.some(p => p.heading === true) && g.thread.slate === undefined ? [g.thread.id] : []))), [scope]);
  const asked = useRef(new Set<string>());
  useEffect(() => {
    for (const id of slated) {
      if (asked.current.has(id) || useSlateStore.getState().byThread[id] !== undefined) continue;
      asked.current.add(id);
      void loadSlate(id).catch(() => undefined);
    }
  }, [slated]);
  return { procs, scope };
}

const statusInputOf = (t: ThreadView): ThreadStatusInput => ({
  status: t.status,
  asking: waitingLine(t) ?? null,
  ...(t.capped !== undefined ? { capped: t.capped } : {}),
  startedAt: t.startedAt !== undefined ? new Date(t.startedAt).toISOString() : null,
  unread: threadUnread(t),
  limit: t.limit ?? null,
  resumeAt: t.resumeAt ?? null,
});

/** A thread head's status mark and, at rest, how long ago it ran. */
export function threadLooks(views: ReadonlyMap<string, ThreadView>): { statusOf: (id: string) => ThreadStatusInput | undefined; ageOf: (id: string) => string | undefined } {
  return {
    statusOf: id => {
      const t = views.get(id);
      return t === undefined ? undefined : statusInputOf(t);
    },
    ageOf: id => {
      const t = views.get(id);
      return t === undefined || t.status === "running" ? undefined : restingAge({ startedAt: t.startedAt !== undefined ? new Date(t.startedAt).toISOString() : null, endedAt: t.endedAt !== undefined ? new Date(t.endedAt).toISOString() : null });
    },
  };
}

/** What a row's acts reach over the link `key` names, and the dialog Stop and End all ask in. `workspaceId` is where a
 * port of a row with no thread opens. */
export function useActivityActs(key: string, workspaceId: string | null, snapshot: ProcsState["snapshot"]): { acts: Acts; dialog: ReactNode } {
  // A process that left the frame takes its spinner with it.
  const [ending, setEnding] = useState<ReadonlySet<number>>(new Set());
  useEffect(() => {
    if (snapshot === null || ending.size === 0) return;
    const listed = new Set(snapshot.procs.map(p => p.pid));
    const left = [...ending].filter(pid => listed.has(pid));
    if (left.length !== ending.size) setEnding(new Set(left));
  }, [snapshot, ending]);
  const verbs = useThreadVerbs();
  const [asking, setAsking] = useState<{ kind: "stop" | "endAll"; group: ActivityGroup } | null>(null);
  const end = useCallback(
    (p: ActivityProc): void => {
      setEnding(s => new Set([...s, p.pid]));
      // The process and everything under it, TERM then KILL in the daemon, checked against the row's identity.
      getProcs(key)
        .kill(p.pid, { signal: "TERM", mode: "tree", ...(p.startedAt !== undefined ? { startedAt: p.startedAt } : {}), ...(p.id !== undefined ? { identity: p.id } : {}) })
        .catch((e: unknown) => {
          setEnding(s => new Set([...s].filter(pid => pid !== p.pid)));
          addNotice({ kind: "error", text: errorText(e) });
        });
    },
    [key],
  );
  const free = useCallback((p: ActivityProc): void => {
    const call = useStore.getState().api?.endActivity;
    if (call === undefined || p.startedAt === undefined) return;
    setEnding(s => new Set([...s, p.pid]));
    call(p.pid, p.startedAt).catch((e: unknown) => {
      setEnding(s => new Set([...s].filter(pid => pid !== p.pid)));
      addNotice({ kind: "error", text: errorText(e) });
    });
  }, []);
  // A container's stop takes its spinner off once the frame no longer lists it, or once the stop failed.
  const [stopping, setStopping] = useState<ReadonlySet<string>>(new Set());
  useEffect(() => {
    if (snapshot === null || stopping.size === 0) return;
    const listed = new Set((snapshot.containers ?? []).map(c => c.id));
    const left = [...stopping].filter(id => listed.has(id));
    if (left.length !== stopping.size) setStopping(new Set(left));
  }, [snapshot, stopping]);
  const endContainer = useCallback(
    (c: ActivityContainer): void => {
      setStopping(s => new Set([...s, c.id]));
      getProcs(key)
        .stopContainer(c.id)
        .catch((e: unknown) => {
          setStopping(s => new Set([...s].filter(id => id !== c.id)));
          addNotice({ kind: "error", text: errorText(e) });
        });
    },
    [key],
  );
  const acts: Acts = useMemo(
    () => ({
      ending,
      end,
      free,
      stopping,
      endContainer,
      stop: (_p, group) => setAsking({ kind: "stop", group }),
      endAll: group => setAsking({ kind: "endAll", group }),
      workspaceOf: group => group?.thread?.workspaceId ?? workspaceId ?? undefined,
      openPort: (at, port, threadId) => {
        useStore.getState().select(at, threadId);
        openBrowserAt(at, { port, path: "/" });
      },
      openThread: group => {
        if (group.thread !== undefined) useStore.getState().select(group.thread.workspaceId, group.thread.id);
      },
      copy: text => void copyText(text).catch((e: unknown) => addNotice({ kind: "error", text: errorText(e) })),
    }),
    [ending, end, free, stopping, endContainer, workspaceId],
  );
  const confirm = (): void => {
    if (asking === null) return;
    const t = asking.group.thread;
    if (t?.working === true) void verbs.stop?.(t.sessionId);
    if (asking.kind === "endAll") {
      for (const p of endsOf(asking.group)) end(p);
      for (const c of asking.group.containers) if (c.owned) endContainer(c);
    }
    setAsking(null);
  };
  return { acts, dialog: <AskDialog asking={asking} onClose={() => setAsking(null)} onConfirm={confirm} /> };
}

/** The processes End all ends: every one under the group but its agent and what wsp needs. */
const endsOf = (g: ActivityGroup): ActivityProc[] => groupProcs(g).filter(p => p.kind !== "agent" && p.needed !== true);

/** The memory those processes and containers hold. The rows are flat already, each still carrying its children, so each
 * counts its own figure and never its children's again. */
const heldBy = (procs: readonly ActivityProc[], containers: readonly ActivityContainer[]): number => procs.reduce((n, p) => n + p.mem, 0) + containers.reduce((n, c) => n + (c.mem ?? 0), 0);

/** Stop asks before a running turn ends; End all asks naming what it takes, its containers among them. */
function AskDialog({ asking, onClose, onConfirm }: { asking: { kind: "stop" | "endAll"; group: ActivityGroup } | null; onClose: () => void; onConfirm: () => void }) {
  const t = asking?.group.thread;
  const procs = asking === null ? [] : endsOf(asking.group);
  const containers = asking === null ? [] : asking.group.containers.filter(c => c.owned);
  const title = t?.title ?? W.loose;
  return (
    <AlertDialog open={asking !== null} onOpenChange={o => (o ? undefined : onClose())}>
      <AlertDialogPopup>
        <AlertDialogHeader>
          <AlertDialogTitle>{asking?.kind === "endAll" ? W.endAllTitle(title) : W.stopTitle(title)}</AlertDialogTitle>
          <AlertDialogDescription>{asking?.kind === "endAll" ? W.endAllSentence(procs.length, containers.length, fmtBytes(heldBy(procs, containers))) : W.stopSentence(agentName(t?.harness ?? "claude"), t?.cgroup !== undefined)}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogClose render={<Button variant="outline" className={NEUTRAL_RING} />}>{W.cancel}</AlertDialogClose>
          <Button variant="destructive" data-k="activity-confirm" onClick={onConfirm}>
            {asking?.kind === "endAll" ? W.endAllPress : W.stopTurn}
          </Button>
        </AlertDialogFooter>
      </AlertDialogPopup>
    </AlertDialog>
  );
}
