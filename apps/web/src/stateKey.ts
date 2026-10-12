// SPDX-License-Identifier: AGPL-3.0-only
// The key a thread's own state sits under in a store keyed by workspace: every
// thread of a project folder shares one workspace record, so what one thread
// opened, followed or pinned goes under its own key there. The New thread page
// is the empty thread id; a workspace with no thread on screen keeps its own id.
import { useStore } from "./protocol/store.js";

const THREAD_MARK = "#";

export const threadStateKey = (workspaceId: string, threadId: string): string => `${workspaceId}${THREAD_MARK}${threadId}`;

export const isThreadKey = (key: string): boolean => key.includes(THREAD_MARK);

export const workspaceOfKey = (key: string): string => {
  const at = key.indexOf(THREAD_MARK);
  return at < 0 ? key : key.slice(0, at);
};

/** The thread a key is of; none for a workspace's own key. */
export const threadOfKey = (key: string): string | undefined => {
  const at = key.indexOf(THREAD_MARK);
  return at < 0 ? undefined : key.slice(at + 1);
};

type Selection = Pick<ReturnType<typeof useStore.getState>, "selectedId" | "selectedThreadId" | "freshThread">;

/** The key a workspace's state is shown under now: the selected thread's, the New thread page's, else its own. */
export function stateKeyIn(s: Selection, workspaceId: string): string {
  if (isThreadKey(workspaceId) || s.selectedId !== workspaceId) return workspaceId;
  if (s.selectedThreadId !== null) return threadStateKey(workspaceId, s.selectedThreadId);
  return s.freshThread ? threadStateKey(workspaceId, "") : workspaceId;
}

export const stateKeyOf = (workspaceId: string): string => stateKeyIn(useStore.getState(), workspaceId);

export function useStateKey(workspaceId: string): string {
  return useStore(s => stateKeyIn(s, workspaceId));
}

/** Whether the workspace's New thread page is on screen, with no thread taken yet: what a first send's carry asks. */
export function onNewThreadPage(workspaceId: string): boolean {
  const s = useStore.getState();
  return s.freshThread && s.selectedId === workspaceId && s.selectedThreadId === null;
}
