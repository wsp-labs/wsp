// SPDX-License-Identifier: AGPL-3.0-only
// One draft and one queue of waiting messages per thread, kept outside the
// composer and in local storage so a glance at another thread or a reload
// does not lose what was typed. Both go by the composer's thread key, since
// every thread of a project folder shares one workspace. The prompt editor is
// controlled from here; a queue holds what was entered while its thread's
// turn ran, oldest first, until the composer sends its head at the turn's end.
// A queue read back from storage, or one whose thread has a send in flight
// that has not yet landed its start, is held: its rows stay put until the
// person's next Enter or send-now on this thread.
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export interface ComposerDraft {
  readonly prompt: string;
  readonly cursor: number;
}

export interface QueuedMessage {
  readonly id: string;
  readonly prompt: string;
}

export const EMPTY_DRAFT: ComposerDraft = { prompt: "", cursor: 0 };
const NO_QUEUE: ReadonlyArray<QueuedMessage> = [];
const STORAGE_KEY = "wsp:composer-drafts:v1";

interface DraftState {
  /** Keyed by the composer's thread key, as the queues are. */
  drafts: Record<string, ComposerDraft>;
  /** Keyed by the composer's thread key: the thread's runtime id, or the workspace id before it has one. */
  queues: Record<string, ReadonlyArray<QueuedMessage>>;
  /** A draft kept by an earlier build under its workspace alone, by workspace: the first thread opened there takes it. */
  unclaimed: Record<string, ComposerDraft>;
  /** The thread key of the composer on screen for each workspace, where words from outside the composer land. */
  shown: Record<string, string>;
  /** Queues whose rows must not go by themselves; not persisted, so every reload holds what it restores. */
  held: Record<string, true>;
  setDraft(threadKey: string, draft: ComposerDraft): void;
  /** Says which thread's composer stands for a workspace, and hands it a draft an earlier build kept there. */
  show(workspaceId: string, threadKey: string): void;
  /** Forgets a composer that left, unless another has stood up for its workspace since. */
  hide(workspaceId: string, threadKey: string): void;
  /** Behind the rows already waiting, or ahead of them when the person's new row must go before held ones. */
  /** Queues a message and answers with its id, which its files are kept under. */
  enqueue(threadKey: string, prompt: string, at?: "head" | "tail"): string;
  removeQueued(threadKey: string, id: string): void;
  /** Moves one row to the head, so it is the next to send. */
  promoteQueued(threadKey: string, id: string): void;
  hold(threadKey: string): void;
  /** Puts a row the runtime refused back at the head and holds the queue. */
  requeue(threadKey: string, row: QueuedMessage): void;
  /** The person acted on this thread: its rows may go again. */
  release(threadKey: string): void;
  /** Moves every row from one key behind the rows at another, and the draft where the other has none, and drops the
   * hold on the key left: a thread's first start gives it the id its rows and words were waiting under. */
  rekeyQueue(from: string, to: string): void;
}

export const newId = (): string => `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

function without<T>(record: Record<string, T>, key: string): Record<string, T> {
  if (!(key in record)) return record;
  const { [key]: _gone, ...rest } = record;
  return rest;
}

function draftsOf(raw: unknown): Record<string, ComposerDraft> {
  const drafts: Record<string, ComposerDraft> = {};
  if (raw && typeof raw === "object") {
    for (const [key, draft] of Object.entries(raw as Record<string, unknown>)) {
      const d = draft as { prompt?: unknown; cursor?: unknown } | null;
      if (d && typeof d.prompt === "string" && d.prompt !== "") drafts[key] = { prompt: d.prompt, cursor: typeof d.cursor === "number" ? d.cursor : d.prompt.length };
    }
  }
  return drafts;
}

function normalizePersisted(persisted: unknown): Pick<DraftState, "drafts" | "queues" | "held" | "unclaimed"> {
  const raw = persisted && typeof persisted === "object" ? (persisted as { drafts?: unknown; queues?: unknown; unclaimed?: unknown }) : {};
  const drafts = draftsOf(raw.drafts);
  const unclaimed = draftsOf(raw.unclaimed);
  const queues: Record<string, ReadonlyArray<QueuedMessage>> = {};
  if (raw.queues && typeof raw.queues === "object") {
    for (const [key, rows] of Object.entries(raw.queues as Record<string, unknown>)) {
      if (!Array.isArray(rows)) continue;
      const clean = rows.filter((row): row is QueuedMessage => {
        const r = row as { id?: unknown; prompt?: unknown } | null;
        return r !== null && typeof r === "object" && typeof r.id === "string" && typeof r.prompt === "string";
      });
      if (clean.length > 0) queues[key] = clean;
    }
  }
  const held: Record<string, true> = {};
  for (const key of Object.keys(queues)) held[key] = true;
  return { drafts, queues, held, unclaimed };
}

/** Version 1 kept one draft per workspace, which every thread of a project folder showed: each waits for the first
 * thread opened in its workspace. */
function migrate(persisted: unknown, version: number): Pick<DraftState, "drafts" | "queues" | "held" | "unclaimed"> {
  const state = normalizePersisted(persisted);
  return version >= 2 ? state : { ...state, drafts: {}, unclaimed: state.drafts };
}

export const useComposerDraftStore = create<DraftState>()(
  persist(
    set => {
      const patchQueue = (threadKey: string, fn: (rows: ReadonlyArray<QueuedMessage>) => ReadonlyArray<QueuedMessage>) =>
        set(s => {
          const rows = s.queues[threadKey] ?? NO_QUEUE;
          const next = fn(rows);
          if (next === rows) return s;
          if (next.length === 0) return { queues: without(s.queues, threadKey), held: without(s.held, threadKey) };
          return { queues: { ...s.queues, [threadKey]: next } };
        });
      return {
        drafts: {},
        queues: {},
        held: {},
        unclaimed: {},
        shown: {},
        setDraft(threadKey, draft) {
          set(s => {
            const current = s.drafts[threadKey];
            if (current !== undefined && current.prompt === draft.prompt && current.cursor === draft.cursor) return s;
            return { drafts: { ...s.drafts, [threadKey]: draft } };
          });
        },
        show(workspaceId, threadKey) {
          set(s => {
            const kept = threadKey === workspaceId ? undefined : s.unclaimed[workspaceId];
            if (s.shown[workspaceId] === threadKey && kept === undefined) return s;
            const shown = { ...s.shown, [workspaceId]: threadKey };
            if (kept === undefined) return { shown };
            return { shown, unclaimed: without(s.unclaimed, workspaceId), drafts: (s.drafts[threadKey]?.prompt ?? "") === "" ? { ...s.drafts, [threadKey]: kept } : s.drafts };
          });
        },
        hide(workspaceId, threadKey) {
          set(s => (s.shown[workspaceId] === threadKey ? { shown: without(s.shown, workspaceId) } : s));
        },
        enqueue(threadKey, prompt, at = "tail") {
          const id = newId();
          patchQueue(threadKey, rows => (at === "head" ? [{ id, prompt }, ...rows] : [...rows, { id, prompt }]));
          return id;
        },
        removeQueued(threadKey, id) {
          patchQueue(threadKey, rows => (rows.some(r => r.id === id) ? rows.filter(r => r.id !== id) : rows));
        },
        promoteQueued(threadKey, id) {
          patchQueue(threadKey, rows => {
            const row = rows.find(r => r.id === id);
            if (row === undefined || rows[0] === row) return rows;
            return [row, ...rows.filter(r => r !== row)];
          });
        },
        hold(threadKey) {
          set(s => (threadKey in s.held ? s : { held: { ...s.held, [threadKey]: true } }));
        },
        requeue(threadKey, row) {
          set(s => ({ queues: { ...s.queues, [threadKey]: [row, ...(s.queues[threadKey] ?? NO_QUEUE)] }, held: { ...s.held, [threadKey]: true } }));
        },
        release(threadKey) {
          set(s => (threadKey in s.held ? { held: without(s.held, threadKey) } : s));
        },
        rekeyQueue(from, to) {
          set(s => {
            if (from === to) return s;
            const moving = s.queues[from];
            const held = without(s.held, from);
            const draft = s.drafts[from];
            const drafts = draft === undefined || (s.drafts[to]?.prompt ?? "") !== "" ? s.drafts : { ...without(s.drafts, from), [to]: draft };
            if (moving === undefined) return held === s.held && drafts === s.drafts ? s : { held, drafts };
            return { queues: { ...without(s.queues, from), [to]: [...(s.queues[to] ?? NO_QUEUE), ...moving] }, held, drafts };
          });
        },
      };
    },
    {
      name: STORAGE_KEY,
      version: 2,
      storage: createJSONStorage(() => window.localStorage),
      partialize: s => ({ drafts: s.drafts, queues: s.queues, unclaimed: s.unclaimed }),
      migrate,
      // migrate runs only on a version change; a bad shape stored at this version must be caught on every hydrate.
      merge: (persisted, current) => ({ ...current, ...normalizePersisted(persisted) }),
    },
  ),
);

export function useComposerDraft(threadKey: string): ComposerDraft {
  return useComposerDraftStore(s => s.drafts[threadKey] ?? EMPTY_DRAFT);
}

/** The thread key words from outside the composer go under: the composer on screen for the workspace, else the
 * workspace's own key, which a new thread's page writes under. */
export const composerKeyIn = (workspaceId: string): string => useComposerDraftStore.getState().shown[workspaceId] ?? workspaceId;

export function useComposerQueue(threadKey: string): ReadonlyArray<QueuedMessage> {
  return useComposerDraftStore(s => s.queues[threadKey] ?? NO_QUEUE);
}

export function useComposerQueueHeld(threadKey: string): boolean {
  return useComposerDraftStore(s => s.held[threadKey] === true);
}
