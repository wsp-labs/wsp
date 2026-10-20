#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// Reads the trace `desktop-gpui --bench-ctrl-tab` wrote: prints each step of the held Ctrl+Tab, the thread it opened,
// how long its key took to reach the first frame presented after it and to the first frame holding the thread's rows;
// exits 1 when a step opened no other thread or its rows missed the first frame after its key, 2 when the trace
// cannot stand as a measurement (a debug build, no steps).
//   node ctrl-tab.mjs <trace.json>
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** The steps that opened no other thread, or whose thread's rows were not in the first frame presented after the key. */
export function lateSteps(trace) {
  return trace.steps.filter(s => s.to === null || s.to === s.from || !s.within_frame);
}

/** Why the trace cannot stand as a measurement, or null when it can. */
export function unfit(trace) {
  if (trace.release !== true) return "the trace comes from a debug build";
  if (!Array.isArray(trace.steps) || trace.steps.length === 0) return "the trace holds no steps";
  if (trace.frame_ms === null) return "the trace holds too few frames to read the frame interval";
  return null;
}

const ms = v => (v === null ? "none" : `${v.toFixed(1)} ms`);

/** One line for a step: when its key went, the thread it opened, and how long to the next frame and to its rows. */
export function describe(s) {
  const to = s.to === null ? "nothing" : s.to.slice(0, 8);
  const rows = s.rows_ms === null ? "rows not drawn before the next key" : `rows ${ms(s.rows_latency_ms).padStart(8)}`;
  const late = s.within_frame ? "" : s.frames_before_rows === null ? "  LATE" : `  LATE by ${s.frames_before_rows} frame${s.frames_before_rows === 1 ? "" : "s"}`;
  return `${String(s.n).padStart(3)} ${s.held ? "repeat" : "press "} sent ${ms(s.sent_ms).padStart(9)}  opened ${to}  key to next frame ${ms(s.latency_ms).padStart(8)}, ${rows}${late}`;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const path = process.argv[2];
  if (path === undefined) {
    console.error("usage: node ctrl-tab.mjs <trace.json>");
    process.exit(2);
  }
  const trace = JSON.parse(readFileSync(path, "utf8"));
  console.log(`ctrl-tab held for ${trace.steps.length} steps: first repeat at ${trace.repeat.delay_ms} ms, then every ${trace.repeat.every_ms} ms; frame interval ${ms(trace.frame_ms)}`);
  for (const s of trace.steps) console.log(describe(s));
  const sum = trace.summary;
  const spread = d => `median ${ms(d.median_ms)}, p90 ${ms(d.p90_ms)}, slowest ${ms(d.max_ms)}`;
  console.log(`opened ${sum.opened} of ${sum.steps} steps, ${sum.threads} threads; ${sum.within_frame} with the thread's rows in the first frame after the key`);
  console.log(`key to next frame: ${spread(sum.key_to_frame)}; key to rows: ${spread(sum.key_to_rows)}`);
  const why = unfit(trace);
  if (why !== null) {
    console.error(`ctrl-tab: ${why}`);
    process.exit(2);
  }
  const late = lateSteps(trace);
  console.log(late.length === 0 ? "every step opened its thread within one frame" : `${late.length} steps late or opening nothing`);
  process.exit(late.length === 0 ? 0 : 1);
}
