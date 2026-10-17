// SPDX-License-Identifier: AGPL-3.0-only
// The ports each daemon link reports, for Activity, by the link's key: this
// computer's own link names no roots, so its watch sees the whole computer,
// and a box's folder link sees what that folder's threads hold.
// terminal/wiring.ts feeds the replies and the pushes here, beside the
// browser's own directory, which Settings does not import.
import { useCallback, useSyncExternalStore } from "react";
import { applyPortEvent, applyPortsSnapshot, type KnownPort, type PortsSnapshot } from "../adapt/ports.js";

type PortEvent = Parameters<typeof applyPortEvent>[1];

const NONE: readonly KnownPort[] = [];
const held = new Map<string, readonly KnownPort[]>();
const fns = new Set<() => void>();

const set = (key: string, next: readonly KnownPort[]): void => {
  held.set(key, next);
  for (const fn of fns) fn();
};

export function feedPortEvent(key: string, e: PortEvent): void {
  set(key, applyPortEvent(held.get(key) ?? NONE, e));
}

/** A ports.watch reply; anything that is not a list of {port} rows leaves the set as the pushes built it. */
export function syncPorts(key: string, reply: unknown): void {
  if (!Array.isArray(reply) || !reply.every(r => typeof r === "object" && r !== null && typeof (r as { port?: unknown }).port === "number")) return;
  set(key, applyPortsSnapshot({ ports: reply as PortsSnapshot["ports"] }));
}

export function usePortsOf(key: string): readonly KnownPort[] {
  return useSyncExternalStore(
    useCallback((fn: () => void) => (fns.add(fn), () => fns.delete(fn)), []),
    () => held.get(key) ?? NONE,
  );
}
