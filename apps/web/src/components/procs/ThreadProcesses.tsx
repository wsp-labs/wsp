// SPDX-License-Identifier: AGPL-3.0-only
// The Processes pane of a thread on this computer or on a box the person
// joined: that thread's own processes and, folded under it, each of its child
// threads', in Activity's rows, read by Activity's one rule over the whole
// computer's threads so a sibling in the same folder is never drawn here. The
// watch is held while the pane stands, on a box over every thread's cgroup as Activity reads one.
import { useEffect, useMemo, useState } from "react";
import { foldThreads, isLocalWorkspace, placeOf, THREAD_CGROUPS, threadKeyOf, type WorkspaceView } from "@wsp/protocol";
import { cn } from "../../lib/utils.js";
import { getProcs } from "../../machine/procs.js";
import { threadRows } from "../../protocol/store/selectors.js";
import { useAbsentComputer, useStore } from "../../protocol/store.js";
import { useActivityActs, useActivityScope, useActivityThreads, threadLooks } from "../../settings/activity/live.js";
import { sortGroups, sortProcs, type ActivitySort } from "../../settings/activity/model.js";
import { ThreadsTable, type Look } from "../../settings/activity/tables.js";
import { ACTIVITY_WORDS as W } from "../../settings/format.js";
import { NOTE } from "../../settings/layout.js";
import { placeName } from "../../settings/places.js";
import { openSettingsGroup } from "../../settings/openAt.js";
import { useSettingsStore } from "../../settings/settingsStore.js";
import { StartDaemonButton } from "../DaemonDown.js";
import { ScrollArea } from "../ui/scroll-area.js";

/** The thread the workspace shows now: the one picked, or with none picked its latest row's; none on its New thread
 * page, nor while another workspace is on screen. */
function useShownThread(workspaceId: string): string | null {
  return useStore(s => {
    if (s.selectedId !== workspaceId || s.freshThread) return null;
    const shown = threadRows(s.sessions[workspaceId] ?? [], workspaceId, s.selectedThreadId ?? workspaceId).at(-1);
    return shown === undefined ? s.selectedThreadId : threadKeyOf(shown);
  });
}

export function ThreadProcesses({ workspaceId, workspace }: { workspaceId: string; workspace: WorkspaceView }) {
  const threadId = useShownThread(workspaceId);
  const workspaces = useStore(s => s.workspaces);
  const sessions = useStore(s => s.sessions);
  const places = useStore(s => s.places);
  const absent = useAbsentComputer(workspaceId);
  const place = placeOf(places, workspace);
  const here = isLocalWorkspace(workspace);
  const { threads, views } = useActivityThreads(place?.id ?? "", here, { workspaces, sessions, places });

  // The thread and every thread under it, its children first: each of them on this computer has a group of its own.
  const tree = useMemo(() => {
    if (threadId === null) return [];
    const all = workspaces.flatMap(w => foldThreads(sessions[w.id] ?? []));
    const ids = [threadId];
    for (let i = 0; i < ids.length; i++) for (const t of all) if (t.parentThreadId === ids[i] && !ids.includes(t.id)) ids.push(t.id);
    return ids;
  }, [threadId, workspaces, sessions]);

  // Every process here; on a box the threads' cgroups as one, since proc.watch takes 16 cgroups at most.
  useEffect(() => getProcs(workspaceId).hold(here ? null : [THREAD_CGROUPS]).release, [workspaceId, here]);

  const { procs, scope } = useActivityScope(workspaceId, here, threads);
  const { acts, dialog } = useActivityActs(workspaceId, workspaceId, procs.snapshot);
  const [sort, setSort] = useState<ActivitySort>("mem");
  const energy = scope?.energy ?? false;
  const look: Look = useMemo(() => ({ sort, onSort: setSort, energy }), [sort, energy]);
  const groups = useMemo(() => {
    if (scope === null) return [];
    const mine = scope.groups.filter(g => g.kind === "thread" && tree.includes(g.id));
    const own = mine.filter(g => g.id === threadId).map(g => ({ ...g, procs: sortProcs(g.procs, sort) }));
    return [...own, ...sortGroups(mine.filter(g => g.id !== threadId), sort)];
  }, [scope, tree, threadId, sort]);
  const opened = useMemo(() => (threadId === null ? [] : [threadId]), [threadId]);
  const unavailable = procs.snapshot === null && absent === null ? procs.unavailable : null;

  const openActivity = (): void => {
    if (place !== undefined) useSettingsStore.getState().pickActivityPlace(place.id);
    openSettingsGroup("activity");
  };

  return (
    <div data-procs data-procs-thread={threadId ?? ""} className="@container/activity flex min-h-0 flex-1 flex-col">
      <ScrollArea className="min-h-0 flex-1">
        <div className="flex min-h-full flex-col p-3">
          {absent !== null ? (
            <div className="flex flex-col items-start gap-2 py-1" data-procs-unavailable>
              <p className={NOTE}>{absent.said}</p>
              {absent.will !== undefined ? <p className={NOTE}>{absent.will}</p> : null}
              <StartDaemonButton absent={absent} workspaceId={workspaceId} />
            </div>
          ) : unavailable !== null ? (
            <p className={NOTE} data-procs-unavailable>
              {unavailable}
            </p>
          ) : scope === null ? (
            <p className={NOTE} data-procs-pending>
              {W.pending}
            </p>
          ) : groups.length === 0 ? (
            <p className={cn(NOTE, "m-auto text-center")} data-procs-nothing>
              {W.paneNothing}
            </p>
          ) : (
            <ThreadsTable key={threadId} name={place === undefined ? "" : placeName(place)} groups={groups} {...threadLooks(views)} box={!here} cost={null} filtering={false} acts={acts} look={look} opened={opened} />
          )}
        </div>
      </ScrollArea>
      <div className="flex h-9 shrink-0 items-center justify-end border-t border-border/50 px-3">
        <button type="button" data-k="procs-open-activity" onClick={openActivity} className={cn(NOTE, "cursor-pointer transition-colors duration-150 hover:text-foreground")}>
          {W.openInActivity}
        </button>
      </div>
      {dialog}
    </div>
  );
}
