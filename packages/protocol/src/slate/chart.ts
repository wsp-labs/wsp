// SPDX-License-Identifier: AGPL-3.0-only
// How a chart labels its axes, a timeline reads its spans and a donut its parts, read by the window that draws them and
// the sketch that tells the agent what it drew.
import { fmtClock } from "../format.js";
import type { SlateJson } from "./types.js";

const STEPS = [1, 2, 2.5, 5];

/** The y axis in four or five round steps holding every value, whichever reaches less far past the top: from 0 when
 * the values sit nearer 0 than their spread, and a band of 2 percent of the value (or 1) around a flat line. */
export function slateChartAxis(values: readonly number[]): { from: number; to: number; parts: number } {
  const lo = Math.min(...values);
  const hi = Math.max(...values);
  const band = hi > lo ? 0 : (Math.abs(lo) * 0.02 || 1) / 2;
  const low = lo >= 0 && lo <= hi - lo ? 0 : lo - band;
  const high = hi + band;
  const tidy = (v: number): number => Number(v.toPrecision(12));
  const span = (parts: number): { from: number; to: number; parts: number } => {
    let magnitude = 10 ** Math.floor(Math.log10((high - low) / parts));
    if (!(magnitude > 0 && Number.isFinite(magnitude))) return { from: low, to: high, parts };
    for (let i = 0; ; i++) {
      if (i === STEPS.length) (i = 0), (magnitude *= 10);
      const step = STEPS[i]! * magnitude;
      if (!Number.isFinite(step * parts)) return { from: low, to: high, parts };
      if (step * parts < high - low) continue;
      const from = tidy(Math.floor(tidy(low / step)) * step);
      if (from + step * parts >= high) return { from, to: tidy(from + step * parts), parts };
    }
  };
  const four = span(4);
  const five = span(5);
  return five.to - five.from < four.to - four.from ? five : four;
}

const DAY = 86_400_000;
const DATE_ONLY = /^\d{4}-\d{2}-\d{2}$/;

/** An x as a moment: ms past 2001, or an ISO date or time. */
function axisTime(x: SlateJson | undefined): number | undefined {
  if (typeof x === "number") return x > 1e12 ? x : undefined;
  if (typeof x !== "string" || !(DATE_ONLY.test(x) || /^\d{4}-\d{2}-\d{2}T/.test(x))) return undefined;
  const ms = Date.parse(x);
  return Number.isFinite(ms) ? ms : undefined;
}

/** How long a list of x values spans, in ms, where they are times; 0 otherwise. */
export function slateAxisSpan(xs: readonly (SlateJson | undefined)[]): number {
  const times = xs.map(axisTime).filter((t): t is number => t !== undefined);
  return times.length < 2 ? 0 : Math.max(...times) - Math.min(...times);
}

/** An x as the axis labels it: a time by the clock, by its date (Oct 12) where the axis spans a day or more or the x is
 * a date alone, anything else as written. */
export function slateAxisWord(x: SlateJson | undefined, span = 0): string {
  const ms = axisTime(x);
  if (ms !== undefined) {
    const dateOnly = typeof x === "string" && DATE_ONLY.test(x);
    if (!dateOnly && span < DAY) return fmtClock(ms).slice(0, 5);
    // A date alone is midnight UTC, so it reads as that day in every time zone.
    return new Date(ms).toLocaleDateString("en-US", { month: "short", day: "numeric", ...(dateOnly ? { timeZone: "UTC" } : {}) });
  }
  return x === undefined || x === null ? "" : typeof x === "string" ? x : JSON.stringify(x);
}

/** A time as a slate reads one: ms, or a string Date.parse takes; anything else is not a time. */
export function slateTime(value: SlateJson | undefined): number | undefined {
  if (typeof value === "number") return Number.isFinite(value) ? value : undefined;
  if (typeof value !== "string" || value.trim() === "") return undefined;
  const ms = Date.parse(value);
  return Number.isFinite(ms) ? ms : undefined;
}

export const SLATE_SPAN_STATES = ["done", "running", "failed", "waiting"] as const;
export type SlateSpanState = (typeof SLATE_SPAN_STATES)[number];

/** A span's state: the one given, else waiting with no start, running with no end and done with both. */
export function slateSpanState(given: SlateJson | undefined, start: number | undefined, end: number | undefined): SlateSpanState {
  if (typeof given === "string" && (SLATE_SPAN_STATES as readonly string[]).includes(given)) return given as SlateSpanState;
  return start === undefined ? "waiting" : end === undefined ? "running" : "done";
}

/** The most parts a donut names; past that, the smallest are summed as Other. */
export const SLATE_DONUT_PARTS = 5;

/** A donut's parts, biggest first, a negative or missing value as 0, and past five parts the rest as one Other. */
export function slateShares(given: ReadonlyArray<{ name: string; value: number }>): Array<{ name: string; value: number; other?: true }> {
  const all = given.map(p => ({ name: p.name, value: Number.isFinite(p.value) ? Math.max(0, p.value) : 0 })).sort((a, b) => b.value - a.value);
  if (all.length <= SLATE_DONUT_PARTS) return all;
  return [...all.slice(0, SLATE_DONUT_PARTS), { name: "Other", value: all.slice(SLATE_DONUT_PARTS).reduce((sum, p) => sum + p.value, 0), other: true }];
}

/** The earliest a slate time is taken to mean: a number before it is most likely seconds read as milliseconds. */
export const SLATE_TIME_FLOOR = Date.UTC(2000, 0, 1);
