// SPDX-License-Identifier: AGPL-3.0-only
// Each computer's readings over its last fifteen minutes, keyed by its place, for the band under its row on Settings >
// Computers. The list fills each window from the daemon's kept reads when it opens and folds the two-second samples
// into it while it stays open. A window outlives the list, so a computer that stops answering still shows what it
// last read. Held as ten-second steps, at most 91 a computer: refolding every raw sample on each one cost five live
// bands over 1% of a core, most of it garbage collection (measured 2026-10-11).
import { useCallback, useEffect, useSyncExternalStore } from "react";
import { READINGS_SPAN_MS, type SysSample } from "@wsp/protocol";
import type { Api } from "../protocol/client.js";
import { useStore } from "../protocol/store.js";

/** Fifteen minutes, the window the daemon keeps and the band draws. */
export const BAND_WINDOW_MS = READINGS_SPAN_MS["15m"];

export const STEP_MS = 10_000;

export type Reading = Pick<SysSample, "at" | "cpu" | "mem">;
/** One ten-second step: the mean CPU over the readings in it and its last memory, at its last reading, with how many
 * readings it holds and the CPU of its last one, the figure the band says now. */
export type Step = Reading & { readonly n: number; readonly cpuNow: number };

const NONE: readonly Step[] = [];
const held = new Map<string, readonly Step[]>();
const fns = new Map<string, Set<() => void>>();

const stepOf = (at: number): number => Math.floor(at / STEP_MS);

const set = (placeId: string, next: readonly Step[]): void => {
  const from = next.length === 0 ? 0 : next.at(-1)!.at - BAND_WINDOW_MS;
  let drop = 0;
  while (drop < next.length && next[drop]!.at < from) drop += 1;
  held.set(placeId, drop === 0 ? next : next.slice(drop));
  for (const fn of fns.get(placeId) ?? []) fn();
};

/** Folds readings, oldest first, onto steps already held. */
const fold = (steps: readonly Step[], readings: readonly Reading[]): Step[] => {
  const out = [...steps];
  for (const { at, cpu, mem } of readings) {
    const last = out.at(-1);
    if (last !== undefined && at <= last.at) continue;
    if (last !== undefined && stepOf(last.at) === stepOf(at)) out[out.length - 1] = { at, cpu: (last.cpu * last.n + cpu) / (last.n + 1), mem, n: last.n + 1, cpuNow: cpu };
    else out.push({ at, cpu, mem, n: 1, cpuNow: cpu });
  }
  return out;
};

/** The kept reads a history answered, with any step already held past its newest one after them. */
export function feedWindow(placeId: string, points: readonly Reading[]): void {
  if (points.length === 0) return;
  const last = points.at(-1)!.at;
  set(placeId, [...fold([], points), ...(held.get(placeId) ?? NONE).filter(s => s.at > last)]);
}

export function feedSample(placeId: string, sample: Reading): void {
  set(placeId, fold(held.get(placeId) ?? NONE, [sample]));
}

/** A computer's steps as held now. */
export const readingsOf = (placeId: string): readonly Step[] => held.get(placeId) ?? NONE;

export function useComputerReadings(placeId: string): readonly Step[] {
  return useSyncExternalStore(
    useCallback(
      (fn: () => void) => {
        const mine = fns.get(placeId) ?? new Set();
        fns.set(placeId, mine.add(fn));
        return () => mine.delete(fn);
      },
      [placeId],
    ),
    () => held.get(placeId) ?? NONE,
  );
}

/** Asks for a computer's readings on this socket until the returned release runs. */
export function watchComputer(api: Api, placeId: string): () => void {
  const { watchPlaceSys, unwatchPlaceSys, onPlaceSysSample } = api;
  if (watchPlaceSys === undefined || onPlaceSysSample === undefined) return () => {};
  const off = onPlaceSysSample(e => {
    if (e.placeId === placeId) feedSample(placeId, e.sample);
  });
  void watchPlaceSys(placeId).catch(() => undefined);
  return () => {
    off();
    void unwatchPlaceSys?.(placeId).catch(() => undefined);
  };
}

/** Holds a computer's readings while the caller is mounted and the host's socket is live, asking again on a socket
 * that comes back, since a watch dies with the socket that made it. Its fifteen minutes so far are read on mount
 * and again when it comes back from away, to fill the stretch it was gone for. */
export function useComputerLive(placeId: string, present: boolean): void {
  const api = useStore(s => s.api);
  const live = useStore(s => s.conn === "live");
  useEffect(() => (api === null || !live ? undefined : watchComputer(api, placeId)), [api, live, placeId]);
  useEffect(() => {
    if (api?.placesReadings === undefined || !live || !present) return;
    let on = true;
    api.placesReadings(placeId, "15m").then(
      answer => {
        if (on) feedWindow(placeId, answer.points);
      },
      () => undefined,
    );
    return () => {
      on = false;
    };
  }, [api, live, placeId, present]);
}

/** Test isolation: forget every computer's window. */
export function forgetComputerReadings(): void {
  held.clear();
}
