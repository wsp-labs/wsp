// SPDX-License-Identifier: AGPL-3.0-only
// The drawer's arrangement per thread: open or closed, height, and how the
// link's ptys are grouped into tabs and splits. The ptys themselves live in
// WorkspaceTerminals, one link per workspace, which every thread of a project
// folder shares; reconcile keeps this in step with the ones a drawer holds.
// Every action takes a workspace id and acts on the drawer of the thread it
// shows now, or a key from stateKey that names one.
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { isThreadKey, onNewThreadPage, stateKeyOf, threadOfKey, threadStateKey, workspaceOfKey } from "../stateKey.js";
import { getTerminals, type PtyTabView, type WorkspaceTerminals } from "./link.js";
import {
  closeTerminal,
  DEFAULT_TERMINAL_UI_STATE,
  isDefaultTerminalUiState,
  newTerminal,
  normalizeTerminalUiState,
  reconcileTerminalIds,
  setActiveTerminal,
  setTerminalHeight,
  setTerminalOpen,
  splitTerminal,
  type TerminalUiState,
} from "./groups.js";

const STORAGE_KEY = "wsp:terminal-drawer:v1";

interface TerminalDrawerStoreState {
  byWorkspaceId: Record<string, TerminalUiState>;
  setOpen: (workspaceId: string, open: boolean) => void;
  toggle: (workspaceId: string) => void;
  setHeight: (workspaceId: string, height: number) => void;
  add: (workspaceId: string, terminalId: string) => void;
  split: (workspaceId: string, terminalId: string, direction?: "horizontal" | "vertical") => void;
  activate: (workspaceId: string, terminalId: string) => void;
  remove: (workspaceId: string, terminalId: string) => void;
  reconcile: (workspaceId: string, terminalIds: string[]) => void;
  removeWorkspace: (workspaceId: string) => void;
}

export function selectTerminalUiState(
  byWorkspaceId: Record<string, TerminalUiState>,
  key: string | null | undefined,
): TerminalUiState {
  if (!key) return DEFAULT_TERMINAL_UI_STATE;
  return byWorkspaceId[key] ?? DEFAULT_TERMINAL_UI_STATE;
}

/** Whether a drawer of the workspace stands open, its threads' off screen too. */
export function anyDrawerOpen(byWorkspaceId: Record<string, TerminalUiState>, workspaceId: string): boolean {
  return Object.entries(byWorkspaceId).some(([key, state]) => workspaceOfKey(key) === workspaceId && state.terminalOpen);
}

const NEW_THREAD_PAGE_KEY = "wsp:new-thread-page";
const NEW_THREAD_PAGE_MARK = "new:";

/** This window's New thread page as a shell's thread names it on the host, so a first send's carry hands on this
 * window's page shells alone. Per tab, so a reload keeps its page. */
export const NEW_THREAD_PAGE: string = (() => {
  // randomUUID exists only in a secure context, and `wsp up --listen` serves the app over plain http.
  const made = `${NEW_THREAD_PAGE_MARK}${[...crypto.getRandomValues(new Uint8Array(16))].map(b => b.toString(16).padStart(2, "0")).join("")}`;
  try {
    const kept = window.sessionStorage.getItem(NEW_THREAD_PAGE_KEY);
    if (kept !== null && kept.startsWith(NEW_THREAD_PAGE_MARK)) return kept;
    window.sessionStorage.setItem(NEW_THREAD_PAGE_KEY, made);
  } catch {}
  return made;
})();

/** The thread a shell opened from a drawer or pane under this key is named for on the host; none for a workspace's own. */
export function ptyThreadOf(key: string): string | undefined {
  const thread = threadOfKey(key);
  return thread === "" ? NEW_THREAD_PAGE : thread;
}

/** The thread a host's tag puts a shell under: any window's New thread page is the empty thread id, so a shell a
 * closed tab's page opened can still be seen and ended there. */
const shownUnder = (tag: string): string => (tag.startsWith(NEW_THREAD_PAGE_MARK) ? "" : tag);

/** The ptys of the workspace's link one drawer holds: a pty the host names a thread for is that thread's drawer's,
 * and one it names none for (a host that restarted, a reply's run moved to a tab) is the drawer's that arranged it,
 * else the first drawer that is shown with it, so no shell is in two threads' drawers. */
export function drawerPtyIds(byWorkspaceId: Record<string, TerminalUiState>, key: string, tabs: readonly PtyTabView[]): string[] {
  const workspaceId = workspaceOfKey(key);
  // The workspace's own key is no thread's: what it arranged (an older build's drawer, a run named for no thread) is
  // still free to the thread on screen.
  const claims = (other: string): boolean => other !== key && workspaceOfKey(other) === workspaceId && (other !== workspaceId || !isThreadKey(key));
  const elsewhere = new Set(Object.entries(byWorkspaceId).flatMap(([other, state]) => (claims(other) ? state.terminalIds : [])));
  const holds = (t: PtyTabView): boolean => {
    if (t.thread === undefined) return !elsewhere.has(t.ptyId);
    return threadStateKey(workspaceId, shownUnder(t.thread)) === key;
  };
  return tabs.filter(holds).map(t => t.ptyId);
}

const update = (
  byWorkspaceId: Record<string, TerminalUiState>,
  key: string,
  fn: (current: TerminalUiState) => TerminalUiState,
): Record<string, TerminalUiState> => {
  const current = byWorkspaceId[key] ?? DEFAULT_TERMINAL_UI_STATE;
  const next = fn(current);
  if (isDefaultTerminalUiState(next)) {
    if (!(key in byWorkspaceId)) return byWorkspaceId;
    const { [key]: _removed, ...rest } = byWorkspaceId;
    return rest;
  }
  if (next === current) return byWorkspaceId;
  return { ...byWorkspaceId, [key]: next };
};

function normalizePersisted(persisted: unknown): { byWorkspaceId: Record<string, TerminalUiState> } {
  if (!persisted || typeof persisted !== "object") return { byWorkspaceId: {} };
  const raw = (persisted as { byWorkspaceId?: unknown }).byWorkspaceId;
  if (!raw || typeof raw !== "object") return { byWorkspaceId: {} };
  const byWorkspaceId: Record<string, TerminalUiState> = {};
  for (const [key, state] of Object.entries(raw as Record<string, Partial<TerminalUiState> | null>)) {
    byWorkspaceId[key] = normalizeTerminalUiState({
      terminalOpen: state?.terminalOpen === true,
      terminalHeight: typeof state?.terminalHeight === "number" ? state.terminalHeight : DEFAULT_TERMINAL_UI_STATE.terminalHeight,
      terminalIds: Array.isArray(state?.terminalIds) ? state.terminalIds.filter((id): id is string => typeof id === "string") : [],
      activeTerminalId: typeof state?.activeTerminalId === "string" ? state.activeTerminalId : "",
      terminalGroups: Array.isArray(state?.terminalGroups) ? state.terminalGroups : [],
      activeTerminalGroupId: typeof state?.activeTerminalGroupId === "string" ? state.activeTerminalGroupId : "",
    });
  }
  return { byWorkspaceId };
}

export const useTerminalDrawerStore = create<TerminalDrawerStoreState>()(
  persist(
    set => ({
      byWorkspaceId: {},
      setOpen: (workspaceId, open) =>
        set(s => ({ byWorkspaceId: update(s.byWorkspaceId, stateKeyOf(workspaceId), c => setTerminalOpen(c, open)) })),
      toggle: workspaceId =>
        set(s => ({ byWorkspaceId: update(s.byWorkspaceId, stateKeyOf(workspaceId), c => setTerminalOpen(c, !c.terminalOpen)) })),
      setHeight: (workspaceId, height) =>
        set(s => ({ byWorkspaceId: update(s.byWorkspaceId, stateKeyOf(workspaceId), c => setTerminalHeight(c, height)) })),
      add: (workspaceId, terminalId) =>
        set(s => ({ byWorkspaceId: update(s.byWorkspaceId, stateKeyOf(workspaceId), c => newTerminal(c, terminalId)) })),
      split: (workspaceId, terminalId, direction = "horizontal") =>
        set(s => ({ byWorkspaceId: update(s.byWorkspaceId, stateKeyOf(workspaceId), c => splitTerminal(c, terminalId, direction)) })),
      activate: (workspaceId, terminalId) =>
        set(s => ({ byWorkspaceId: update(s.byWorkspaceId, stateKeyOf(workspaceId), c => setActiveTerminal(c, terminalId)) })),
      remove: (workspaceId, terminalId) =>
        set(s => ({ byWorkspaceId: update(s.byWorkspaceId, stateKeyOf(workspaceId), c => closeTerminal(c, terminalId)) })),
      reconcile: (workspaceId, terminalIds) =>
        set(s => ({ byWorkspaceId: update(s.byWorkspaceId, stateKeyOf(workspaceId), c => reconcileTerminalIds(c, terminalIds)) })),
      removeWorkspace: workspaceId =>
        set(s => {
          const kept = Object.entries(s.byWorkspaceId).filter(([key]) => workspaceOfKey(key) !== workspaceId);
          return kept.length === Object.keys(s.byWorkspaceId).length ? s : { byWorkspaceId: Object.fromEntries(kept) };
        }),
    }),
    {
      name: STORAGE_KEY,
      version: 1,
      storage: createJSONStorage(() => window.localStorage),
      partialize: s => ({ byWorkspaceId: s.byWorkspaceId }),
      migrate: normalizePersisted,
      // migrate runs only on a version change; a bad shape stored at this version must be caught on every hydrate.
      merge: (persisted, current) => ({ ...current, ...normalizePersisted(persisted) }),
    },
  ),
);

const spawned = new WeakMap<WorkspaceTerminals, Map<string, Promise<PtyTabView>>>();

/** The first-open spawn each drawer on a link made, by drawer key. */
export function firstSpawns(terms: WorkspaceTerminals): Map<string, Promise<PtyTabView>> {
  const held = spawned.get(terms) ?? new Map<string, Promise<PtyTabView>>();
  spawned.set(terms, held);
  return held;
}

/** The New thread page's drawer and shells go to the thread its first send opened, which the page goes on showing:
 * asked where that send's thread is taken, as the right pane's carry is, never on a selection. */
export function carryNewThreadDrawer(workspaceId: string, threadId: string): void {
  if (!onNewThreadPage(workspaceId)) return;
  const terms = getTerminals(workspaceId);
  const draft = threadStateKey(workspaceId, "");
  for (const tab of terms?.tabs() ?? []) if (tab.thread === NEW_THREAD_PAGE) void terms!.retag(tab.ptyId, threadId);
  // The next New thread page is a page of its own, which opens its first shell again.
  if (terms !== null) firstSpawns(terms).delete(draft);
  const drawers = useTerminalDrawerStore.getState().byWorkspaceId;
  if (!(draft in drawers)) return;
  const { [draft]: carried, ...rest } = drawers;
  const key = threadStateKey(workspaceId, threadId);
  useTerminalDrawerStore.setState({ byWorkspaceId: key in rest ? rest : { ...rest, [key]: carried! } });
}
