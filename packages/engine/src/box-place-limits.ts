// SPDX-License-Identifier: AGPL-3.0-only
// A cloud row's plan and starts off Boat's GET /limits, kept a few seconds per backend so a page that lists every
// second reads Boat once in ten, a failed read kept as long so a Boat that does not answer slows one list in ten
// seconds rather than every one.
import { sizeWord, type CloudLimits, type StartsWindow } from "@wsp/protocol";
import type { BoxClass } from "./box-backend.js";

/** How long one /limits answer stands for a row. */
const PLACE_LIMITS_KEPT_MS = 10_000;
/** How long a list waits on /limits before it leaves the plan off the row. */
export const PLACE_LIMITS_READ_MS = 5_000;

/** The classes the trial sells. */
const TRIAL_CLASSES: readonly BoxClass["type"][] = ["small", "default"];

/** The fields of a /limits answer a row reads; boat.dev's base spells the machine cap with Sandboxes. */
export interface BoxLimitsAnswer {
  accessTier?: string;
  maxActiveBoxes?: number;
  maxActiveSandboxes?: number;
  boxPlanKey?: string;
  starts?: Partial<Record<"minute" | "hour" | "day", { limit?: number; used?: number }>>;
}

/** The plan and the starts a /limits answer reads as, the sizes off the classes sold: nothing without a machine cap,
 * and no starts where a window lacks a figure, as an account with unlimited starts answers. */
export function boxLimitsOf(answer: BoxLimitsAnswer, classes: readonly BoxClass[]): CloudLimits | undefined {
  const machinesAtOnce = answer.maxActiveBoxes ?? answer.maxActiveSandboxes;
  if (machinesAtOnce === undefined) return undefined;
  const trial = answer.accessTier === "trial";
  const sizes = classes.filter(c => !trial || TRIAL_CLASSES.includes(c.type)).map(c => sizeWord(c));
  const window = (w: { limit?: number; used?: number } | undefined): StartsWindow | undefined => (w?.limit === undefined || w.used === undefined ? undefined : { used: w.used, limit: w.limit });
  const [minute, hour, day] = [window(answer.starts?.minute), window(answer.starts?.hour), window(answer.starts?.day)];
  return {
    plan: { key: answer.boxPlanKey ?? answer.accessTier ?? "", trial, machinesAtOnce, sizes },
    ...(minute !== undefined && hour !== undefined && day !== undefined ? { starts: { minute, hour, day } } : {}),
  };
}

/** The last read per backend and when it was asked; one entry each, replaced in place. */
const kept = new WeakMap<object, { at: number; answer: Promise<CloudLimits | undefined> }>();

/** The plan and starts for one backend's row: the read kept for it, else a new one through `read`. */
export function keptPlaceLimits(backend: object, now: number, read: () => Promise<BoxLimitsAnswer>, classes: readonly BoxClass[]): Promise<CloudLimits | undefined> {
  const last = kept.get(backend);
  if (last !== undefined && now - last.at < PLACE_LIMITS_KEPT_MS) return last.answer;
  const answer = read().then(
    a => boxLimitsOf(a, classes),
    () => undefined,
  );
  kept.set(backend, { at: now, answer });
  return answer;
}
