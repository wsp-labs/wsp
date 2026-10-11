// SPDX-License-Identifier: AGPL-3.0-only
// Each computer's readings over its last fifteen minutes, keyed by the place, for the band under its row on Settings >
// Computers. Fed a full window once and a sample every two seconds after; a computer that stops answering keeps the
// window it had, read as away.
import { useCallback, useSyncExternalStore } from "react";
import type { SysSample } from "@wsp/protocol";

/** Fifteen minutes, the window the daemon keeps and the band draws. */
export const BAND_WINDOW_MS = 15 * 60_000;

export interface ComputerReadings {
  readonly samples: readonly SysSample[];
  readonly away: boolean;
}

const NONE: ComputerReadings = { samples: [], away: false };
const held = new Map<string, ComputerReadings>();
const fns = new Map<string, Set<() => void>>();

const set = (placeId: string, next: ComputerReadings): void => {
  held.set(placeId, next);
  for (const fn of fns.get(placeId) ?? []) fn();
};

/** A full window, as the history read answers it when the list opens. */
export function feedWindow(placeId: string, samples: readonly SysSample[], away = false): void {
  set(placeId, { samples, away });
}

export function feedSample(placeId: string, sample: SysSample): void {
  const was = held.get(placeId) ?? NONE;
  const from = sample.at - BAND_WINDOW_MS;
  set(placeId, { samples: [...was.samples.filter(s => s.at >= from), sample], away: false });
}

export function useComputerReadings(placeId: string): ComputerReadings {
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
