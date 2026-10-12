// SPDX-License-Identifier: AGPL-3.0-only
// A window whose page process died shows nothing until something loads it
// again, so the app reloads it on the address it was on. A page that dies as
// it loads would reload for ever, so the reloads are paced.

/** The reloads a minute may hold; one past it waits for the oldest to turn a minute old. */
export const RELOADS_PER_MINUTE = 3;
export const RELOAD_WINDOW_MS = 60_000;

/** The wait before the next reload, in ms: none while the last minute holds fewer than the cap, else until the
 * oldest of the last cap reloads is a minute old. */
export function reloadPacer(o: { now?: () => number; cap?: number; windowMs?: number } = {}): () => number {
  const { now = Date.now, cap = RELOADS_PER_MINUTE, windowMs = RELOAD_WINDOW_MS } = o;
  const at: number[] = [];
  return () => {
    const t = now();
    while (at.length > 0 && at[0]! <= t - windowMs) at.shift();
    const next = at.length < cap ? t : at[at.length - cap]! + windowMs;
    at.push(next);
    return next - t;
  };
}

/** The part of a WebContents the reload reads. */
export interface DeadPage {
  isDestroyed(): boolean;
  isCrashed(): boolean;
  reload(): void;
}

/** Reloads the page once the wait is over, unless it is gone or Cmd+R or a move to another host brought it back. */
export function reloadAfter(page: DeadPage, waitMs: number): void {
  setTimeout(() => {
    if (!page.isDestroyed() && page.isCrashed()) page.reload();
  }, waitMs);
}
