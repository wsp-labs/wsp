#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// Reads a frame-time trace that `desktop-gpui --bench-replay` or `--record-scroll ... --trace` wrote: prints its
// summary, every frame over the limit, each with what the app did in it, and every run of wheel events that found room
// to scroll and left the list where it stood (the wheel went to something under the pointer), with the kind of row
// under it; exits 1 when there is a slow frame or a stuck run, 2 when the
// trace cannot stand as a measurement (a debug build, a window without focus, no frames, no wheel event that moved the
// list). With --app a frame is judged by the UI thread's own time alone (its draw and the traced work before it), for a
// machine with no GPU, where the present is the software rasteriser's. With --latency it is judged as the person sees
// it: late when a wheel event it shows reached the screen more than the limit after the event was due. With --switches
// it reads a `--bench-themes` trace instead: one line per theme switch, and exits 1 when any took over 100 ms to reach
// the screen, drew a frame over the limit, drew nothing, or kept drawing after it settled; --app judges both by the
// app's own time.
//   node trace.mjs [--app | --latency] <trace.json>
//   node trace.mjs --switches [--app] <trace.json>
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** A frame is slow when its draw and present together, or the wait of an input it shows, pass the limit; with `app`,
 * when the UI thread's own time for it does; with `latency`, when the wait of an input it shows does. */
export function slowFrames(trace, { app = false, latency = false } = {}) {
  const limit = trace.limit_ms ?? 50;
  return trace.frames.filter(f => (app ? f.app > limit : latency ? f.late > limit : f.draw + f.present > limit || f.late > limit));
}

/** Why the trace cannot stand as a measurement, or null when it can. */
export function unfit(trace) {
  if (trace.release !== true) return "the trace comes from a debug build";
  if (!Array.isArray(trace.frames) || trace.frames.length === 0) return "the trace holds no frames";
  if (trace.frames.some(f => !f.focused)) return "the window lost focus during the trace, and GPUI caps a window without focus at 30 fps";
  if (trace.summary?.shown === 0 && trace.inputs?.length > 0) return "no wheel event moved the list, so the trace scrolled nothing";
  return null;
}

const ms = v => `${v.toFixed(1)} ms`;

/** A theme switch must be on screen this soon after it was sent. */
export const SWITCH_MS = 100;

/** Each theme switch of a `--bench-themes` trace as one line, with why it fails, if it does; with `app`, its time and
 * frames are the app's own, without the presents. */
export function switchVerdicts(trace, { app = false } = {}) {
  const limit = trace.limit_ms ?? 50;
  return (trace.switches ?? []).map((s, ix) => {
    const took = app ? s.took_app_ms : s.took_ms;
    const slowest = app ? s.slowest_app_ms : s.slowest_ms;
    const fails = [];
    if (took == null) fails.push("drew no frame");
    else if (took > SWITCH_MS) fails.push(`took over ${SWITCH_MS} ms`);
    if (slowest > limit) fails.push(`a frame over ${limit} ms`);
    if (s.frames_after > 0) fails.push("kept drawing after it settled");
    const after = Object.entries(s.views_after ?? {}).map(([view, n]) => `${view} ${n}`).join(", ");
    const line = `switch ${ix + 1} ${s.word}: on screen in ${took == null ? "never" : ms(took)}, slowest frame ${ms(slowest)}, write ${ms(s.write_ms)}, ${s.frames} frame${s.frames === 1 ? "" : "s"}; ${s.frames_after === 0 ? "none" : `${s.frames_after} (${s.renders_after_per_frame.toFixed(1)} renders each${after ? `: ${after}` : ""})`} after it settled${fails.length > 0 ? `; FAIL: ${fails.join(", ")}` : ""}`;
    return { line, fails };
  });
}

/** One line for a slow frame: when, how long, and what the app did since the frame before. */
export function describe(f) {
  const did = [];
  if (f.pages.length > 0) did.push(`${f.pages.length} page${f.pages.length > 1 ? "s" : ""} landed (${f.pages.map(p => `${p.events} events in ${ms(p.ms)}`).join(", ")})`);
  if (f.rows > 0) did.push(`${f.rows} rows built in ${ms(f.row_ms)}`);
  if (f.parsed > 0) did.push(`${f.parsed} parsed in ${ms(f.parse_ms)}`);
  if (f.images > 0) did.push(`${f.images} images laid into their rows`);
  if (f.view_ms > 1) did.push(`thread view ${ms(f.view_ms)}`);
  if (f.input_ms > 1) did.push(`${f.inputs} inputs handled in ${ms(f.input_ms)}`);
  const drew = Object.entries(f.renders ?? {});
  if (drew.length > 0) did.push(`drew ${drew.map(([view, n]) => `${view} ${n}`).join(", ")}`);
  const cpu = f.draw_cpu === undefined ? "" : ` (CPU ${ms(f.draw_cpu)}${f.switches ? `, ${f.switches[0]} blocked, ${f.switches[1]} preempted` : ""})`;
  const delivered = f.delivered > 0 ? ` (${ms(f.delivered)} of it before the app had it)` : "";
  if (f.roots_ms != null) {
    const callbacks = Math.max(0, f.draw - f.before_ms - f.roots_ms - f.after_ms);
    did.push(`draw in parts: callbacks ${ms(callbacks)}, before roots ${ms(f.before_ms)}, roots ${ms(f.roots_ms)}, after ${ms(f.after_ms)}`);
  }
  if (f.drawable_wait_ms != null) did.push(`present waited ${ms(f.drawable_wait_ms)} for a drawable${f.scheduled_wait_ms > 0 ? ` and ${ms(f.scheduled_wait_ms)} to be scheduled` : ""}`);
  if (f.gpu_ms != null) did.push(`GPU ${ms(f.gpu_ms)}`);
  return `frame ${f.frame} at ${(f.at / 1000).toFixed(2)} s: draw ${ms(f.draw)}${cpu}, present ${ms(f.present)}, app ${ms(f.app)}, ${ms(f.since)} since the last, input waited ${ms(f.late)}${delivered}; ${did.join("; ") || "nothing traced"}`;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const app = process.argv.includes("--app");
  const latency = process.argv.includes("--latency");
  const switches = process.argv.includes("--switches");
  const path = process.argv.slice(2).find(a => !a.startsWith("--"));
  if (path === undefined || (app && latency) || (switches && latency)) {
    console.error("usage: node trace.mjs [--app | --latency] <trace.json>\n       node trace.mjs --switches [--app] <trace.json>");
    process.exit(2);
  }
  const trace = JSON.parse(readFileSync(path, "utf8"));
  if (switches) {
    const by = app ? " by app" : "";
    const verdicts = switchVerdicts(trace, { app });
    console.log(`${trace.thread}: ${verdicts.length} theme switches, ${trace.frames.length} frames over ${((trace.frames.at(-1)?.at ?? 0) / 1000).toFixed(1)} s, scale ${trace.scale}, judged${by || " by draw and present"}`);
    for (const v of verdicts) console.log(v.line);
    const why = verdicts.length === 0 ? "the trace holds no theme switch" : unfit(trace);
    if (why !== null) {
      console.error(`trace: ${why}`);
      process.exit(2);
    }
    const failed = verdicts.filter(v => v.fails.length > 0).length;
    console.log(failed === 0 ? `every switch on screen within ${SWITCH_MS} ms${by}, no frame over ${trace.limit_ms} ms, no frame after it settled` : `${failed} of ${verdicts.length} switches failed`);
    process.exit(failed === 0 ? 0 : 1);
  }
  const slow = slowFrames(trace, { app, latency });
  const draws = trace.frames.map(f => f.draw + f.present).sort((a, b) => a - b);
  const pages = trace.frames.flatMap(f => f.pages);
  console.log(`${trace.mode} of ${trace.thread}: ${trace.frames.length} frames over ${((trace.frames.at(-1)?.at ?? 0) / 1000).toFixed(1)} s, ${trace.inputs.length} wheel events, ${pages.length} pages landed (${pages.reduce((n, p) => n + p.events, 0)} events), scale ${trace.scale}`);
  const apps = trace.frames.map(f => f.app).sort((a, b) => a - b);
  console.log(`draw and present: median ${ms(draws[Math.floor(draws.length / 2)] ?? 0)}, slowest ${ms(draws.at(-1) ?? 0)}; longest input wait ${ms(Math.max(0, ...trace.frames.map(f => f.late)))}`);
  console.log(`app (draw and traced work): median ${ms(apps[Math.floor(apps.length / 2)] ?? 0)}, slowest ${ms(apps.at(-1) ?? 0)}; frames over ${trace.limit_ms} ms by app ${slowFrames(trace, { app: true }).length}, by draw, present and input wait ${slowFrames(trace).length}`);
  const s = trace.summary ?? {};
  if (s.shown !== undefined) {
    console.log(`input to screen: ${s.shown} of ${trace.inputs.length} wheel events moved the list; their wait median ${ms(s.wait_median_ms)}, p95 ${ms(s.wait_p95_ms)}, of which before the app had them median ${ms(s.delivery_median_ms)}, p95 ${ms(s.delivery_p95_ms)}; frames late by over ${trace.limit_ms} ms ${s.late}`);
  }
  if (s.gpu > 0) {
    console.log(`GPU per frame: median ${ms(s.gpu_median_ms)}, p95 ${ms(s.gpu_p95_ms)}, slowest ${ms(s.gpu_slowest_ms)}, over ${trace.limit_ms} ms ${s.gpu_over}; present's wait for a drawable median ${ms(s.drawable_median_ms)}, p95 ${ms(s.drawable_p95_ms)}, slowest ${ms(s.drawable_slowest_ms)}`);
  }
  if (s.handover > 0) {
    console.log(`the replay's own lateness handing events to the platform: median ${ms(s.handover_median_ms)}, p95 ${ms(s.handover_p95_ms)}, left out of the waits above`);
  }
  for (const f of slow) console.log(describe(f));
  const appSlow = slowFrames(trace, { app: true });
  if (!app && appSlow.length > 0) {
    console.log(`over ${trace.limit_ms} ms by app time:`);
    for (const f of appSlow) console.log(`  ${describe(f)}`);
  }
  const cost = f => (app ? f.app : latency ? f.late : Math.max(f.draw + f.present, f.late));
  console.log("slowest five:");
  for (const f of [...trace.frames].sort((a, b) => cost(b) - cost(a)).slice(0, 5)) console.log(`  ${describe(f)}`);
  const stuck = trace.stuck ?? [];
  for (const s of stuck) console.log(`stuck: ${s.events} wheel events from ${(s.from / 1000).toFixed(2)} s to ${(s.to / 1000).toFixed(2)} s left the list where it stood, the pointer over ${s.under ?? "nothing traced"}`);
  if (trace.under !== undefined) console.log(`under the pointer: ${Object.entries(trace.under).map(([kind, u]) => `${kind} ${u.moved}/${u.events} moved`).join(", ")}`);
  const why = unfit(trace);
  if (why !== null) {
    console.error(`trace: ${why}`);
    process.exit(2);
  }
  const by = app ? " by app" : latency ? " from input to screen" : "";
  console.log(slow.length === 0 ? `no frame over ${trace.limit_ms} ms${by}` : `${slow.length} frames over ${trace.limit_ms} ms${by}`);
  console.log(stuck.length === 0 ? "no stuck wheel" : `${stuck.length} stuck runs`);
  process.exit(slow.length === 0 && stuck.length === 0 ? 0 : 1);
}
