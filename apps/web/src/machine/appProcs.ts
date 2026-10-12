// SPDX-License-Identifier: AGPL-3.0-only
// The desktop app's processes by pid, as the host on this computer holds
// them, for the rows that list this computer's processes. The host has the app
// read Electron only while a socket watches, so this socket watches only while
// a pane holds it, and asks again on each socket that comes live, since a watch
// dies with the socket that made it. A host that predates the watch leaves the
// rows unnamed.
import { useSyncExternalStore } from "react";
import type { AppProc } from "@wsp/protocol";
import type { Api } from "../protocol/client.js";
import type { useStore } from "../protocol/store.js";

const NONE: ReadonlyMap<number, AppProc> = new Map();
let byPid: ReadonlyMap<number, AppProc> = NONE;
let holds = 0;
const fns = new Set<() => void>();
const held = new Set<() => void>();

const set = (procs: readonly AppProc[]): void => {
  if (procs.length === 0 && byPid.size === 0) return;
  byPid = procs.length === 0 ? NONE : new Map(procs.map(p => [p.pid, p]));
  for (const fn of fns) fn();
};

/** A pane's hold on the names; the first holder starts the watch and the last one leaving ends it. */
export function holdAppProcs(): () => void {
  holds++;
  for (const fn of held) fn();
  let released = false;
  return () => {
    if (released) return;
    released = true;
    holds--;
    for (const fn of held) fn();
  };
}

export function useAppProcs(): ReadonlyMap<number, AppProc> {
  return useSyncExternalStore(
    fn => (fns.add(fn), () => fns.delete(fn)),
    () => byPid,
  );
}

/** Keeps this page's socket watching while a pane holds the names, on whichever socket the store holds now. */
export function wireAppProcs(store: typeof useStore): () => void {
  let bound: Api | null = null;
  let off: (() => void) | null = null;
  let asked = false;

  const sync = (): void => {
    const { api, conn } = store.getState();
    if (api !== bound) {
      off?.();
      bound = api;
      asked = false;
      set([]);
      off = api?.onAppProcs?.(set) ?? null;
    }
    if (api?.watchAppProcs === undefined) return;
    if (conn !== "live") {
      asked = false;
      return;
    }
    if (holds > 0 && !asked) {
      asked = true;
      api.watchAppProcs().then(procs => void (asked && bound === api && set(procs)), () => undefined);
    } else if (holds === 0 && asked) {
      asked = false;
      set([]);
      void api.unwatchAppProcs?.().catch(() => undefined);
    }
  };

  const unsubscribe = store.subscribe(sync);
  held.add(sync);
  sync();
  return () => {
    unsubscribe();
    held.delete(sync);
    off?.();
    off = null;
    set([]);
  };
}
