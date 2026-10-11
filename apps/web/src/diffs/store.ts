// SPDX-License-Identifier: AGPL-3.0-only
// What the diff surface remembers: the scope per workspace, and per thread the
// one turn a reply's changed files opened it on, since every thread of a project
// folder shares its workspace. The folder git runs in is the panes' shared root
// (files/root.ts) for a scope and the turn's own folder for a turn.
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import type { GitDiffScope } from "@wsp/protocol";
import { shownThreadIdOf, useStore } from "../protocol/store.js";

export type DiffRenderMode = "stacked" | "split";

/** The branch against its merge-base: the one scope that holds what a thread changed whether it committed or not.
 * A thread that commits as it goes leaves nothing uncommitted, and a pane first read there showed nothing. */
export const DEFAULT_SCOPE: GitDiffScope = "branch";

/** One turn's changes: the snapshots at its launch and its end, the folder they were taken in, and the file asked for.
 * only narrows the range to the files of one list on the turn's card, in a folder other threads worked in too. */
export interface TurnRange {
  readonly threadId: string;
  readonly turnId: string;
  readonly cwd: string;
  readonly from: string;
  readonly to: string;
  readonly path?: string;
  readonly only?: ReadonlyArray<string>;
}

/** The files a turn's range opens narrowed to, where the turn's own files are known: the others' when the row asked
 * for is theirs or the turn has none of its own, else its own. Undefined leaves the range whole. */
export function onlyOf(changes: { readonly files: ReadonlyArray<{ readonly path: string }>; readonly others?: ReadonlyArray<{ readonly path: string }> }, path?: string): string[] | undefined {
  const { files, others } = changes;
  if (others === undefined) return undefined;
  return (others.some(f => f.path === path) || files.length === 0 ? others : files).map(f => f.path);
}

/** The path a turn's changes name a file by, from the top of the checkout, for a path an edit named whole; undefined
 * where the turn recorded no change to it. The longest match wins: git lists package.json before packages/x/package.json. */
export function changedPathOf(changes: { readonly files: ReadonlyArray<{ readonly path: string }>; readonly others?: ReadonlyArray<{ readonly path: string }> }, named: string): string | undefined {
  let found: string | undefined;
  for (const { path } of [...changes.files, ...(changes.others ?? [])]) if ((path === named || named.endsWith(`/${path}`)) && path.length > (found?.length ?? -1)) found = path;
  return found;
}

/** The same turn's range with nothing narrowed. */
export function wholeRange(range: TurnRange): TurnRange {
  const { only: _narrowed, ...whole } = range;
  return whole;
}

/** Turns kept on this browser: a thread deleted on another client leaves no word here to drop its turn. */
export const KEPT_TURNS = 100;

export const turnKey = (workspaceId: string, threadId: string): string => `${workspaceId}#${threadId}`;

/** The key of the thread a workspace shows now, as the view keys it (useChatThread's threadKey): the selected thread,
 * else the workspace id a view with no thread id yet goes by; null for a workspace not on screen. */
export const shownThreadOf = (s: { selectedId: string | null; selectedThreadId: string | null }, workspaceId: string): string | null =>
  shownThreadIdOf(s, workspaceId) ?? (s.selectedId === workspaceId ? workspaceId : null);

interface DiffStoreState {
  scopeByWorkspaceId: Record<string, GitDiffScope>;
  turnByThread: Record<string, TurnRange>;
  renderMode: DiffRenderMode;
  /** Takes the workspace's scope, leaving the turn the thread on screen had open. */
  setScope: (workspaceId: string, scope: GitDiffScope) => void;
  openTurn: (workspaceId: string, range: TurnRange) => void;
  setRenderMode: (mode: DiffRenderMode) => void;
}

/** The turn the thread on screen in a workspace opened, if any. */
export function useShownTurn(workspaceId: string): TurnRange | undefined {
  const threadId = useStore(s => shownThreadOf(s, workspaceId));
  return useDiffStore(s => (threadId === null ? undefined : s.turnByThread[turnKey(workspaceId, threadId)]));
}

export const useDiffStore = create<DiffStoreState>()(
  persist(
    set => ({
      scopeByWorkspaceId: {},
      turnByThread: {},
      renderMode: "stacked",
      setScope: (workspaceId, scope) =>
        set(s => {
          const threadId = shownThreadOf(useStore.getState(), workspaceId);
          const turnByThread = { ...s.turnByThread };
          if (threadId !== null) delete turnByThread[turnKey(workspaceId, threadId)];
          return { scopeByWorkspaceId: { ...s.scopeByWorkspaceId, [workspaceId]: scope }, turnByThread };
        }),
      // Written last goes last, so the turns past the cap that go are the ones opened longest ago.
      openTurn: (workspaceId, range) =>
        set(s => {
          const key = turnKey(workspaceId, range.threadId);
          const { [key]: _was, ...rest } = s.turnByThread;
          const kept = Object.entries(rest).slice(Math.max(0, Object.keys(rest).length + 1 - KEPT_TURNS));
          return { turnByThread: { ...Object.fromEntries(kept), [key]: range } };
        }),
      setRenderMode: renderMode => set({ renderMode }),
    }),
    {
      name: "wsp:diff-surface:v2",
      storage: createJSONStorage(() => window.localStorage),
      // The turn a reply opened is kept too, so a reload shows that turn and not the branch against its base.
      partialize: s => ({ scopeByWorkspaceId: s.scopeByWorkspaceId, turnByThread: s.turnByThread, renderMode: s.renderMode }),
    },
  ),
);
