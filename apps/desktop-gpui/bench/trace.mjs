#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// Reads a frame-time trace that `desktop-gpui --bench-replay` or `--record-scroll ... --trace` wrote: prints its
// summary, every frame over the limit, each with what the app did in it, and every run of wheel events that found room
// to scroll and left the list where it stood (the wheel went to something under the pointer), with the kind of row
// under it; exits 1 when there is a slow frame or a stuck run, 2 when the
// trace cannot stand as a measurement (a debug build, a window without focus, no frames). With --app a frame is judged
// by the UI thread's own time alone (its draw and the traced work before it), for a machine with no GPU, where the
// present is the software rasteriser's.
//   node trace.mjs [--app] <trace.json>
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** A frame is slow when its draw and present together, or the wait of an input it shows, pass the limit; with `app`,
 * when the UI thread's own time for it does. */
export function slowFrames(trace, { app = false } = {}) {
  const limit = trace.limit_ms ?? 50;
  return trace.frames.filter(f => (app ? f.app > limit : f.draw + f.present > limit || f.late > limit));
}

/** Why the trace cannot stand as a measurement, or null when it can. */
export function unfit(trace) {
  if (trace.release !== true) return "the trace comes from a debug build";
  if (!Array.isArray(trace.frames) || trace.frames.length === 0) return "the trace holds no frames";
  if (trace.frames.some(f => !f.focused)) return "the window lost focus during the trace, and GPUI caps a window without focus at 30 fps";
  return null;
}

const ms = v => `${v.toFixed(1)} ms`;

/** One line for a slow frame: when, how long, and what the app did since the frame before. */
export function describe(f) {
  const did = [];
  if (f.pages.length > 0) did.push(`${f.pages.length} page${f.pages.length > 1 ? "s" : ""} landed (${f.pages.map(p => `${p.events} events in ${ms(p.ms)}`).join(", ")})`);
  if (f.rows > 0) did.push(`${f.rows} rows built in ${ms(f.row_ms)}`);
  if (f.parsed > 0) did.push(`${f.parsed} parsed in ${ms(f.parse_ms)}`);
  if (f.images > 0) did.push(`${f.images} images laid into their rows`);
  if (f.view_ms > 1) did.push(`thread view ${ms(f.view_ms)}`);
  if (f.input_ms > 1) did.push(`${f.inputs} inputs handled in ${ms(f.input_ms)}`);
  const cpu = f.draw_cpu === undefined ? "" : ` (CPU ${ms(f.draw_cpu)}${f.switches ? `, ${f.switches[0]} blocked, ${f.switches[1]} preempted` : ""})`;
  return `frame ${f.frame} at ${(f.at / 1000).toFixed(2)} s: draw ${ms(f.draw)}${cpu}, present ${ms(f.present)}, app ${ms(f.app)}, ${ms(f.since)} since the last, input waited ${ms(f.late)}; ${did.join("; ") || "nothing traced"}`;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const app = process.argv.includes("--app");
  const path = process.argv.slice(2).find(a => a !== "--app");
  if (path === undefined) {
    console.error("usage: node trace.mjs [--app] <trace.json>");
    process.exit(2);
  }
  const trace = JSON.parse(readFileSync(path, "utf8"));
  const slow = slowFrames(trace, { app });
  const draws = trace.frames.map(f => f.draw + f.present).sort((a, b) => a - b);
  const pages = trace.frames.flatMap(f => f.pages);
  console.log(`${trace.mode} of ${trace.thread}: ${trace.frames.length} frames over ${((trace.frames.at(-1)?.at ?? 0) / 1000).toFixed(1)} s, ${trace.inputs.length} wheel events, ${pages.length} pages landed (${pages.reduce((n, p) => n + p.events, 0)} events), scale ${trace.scale}`);
  const apps = trace.frames.map(f => f.app).sort((a, b) => a - b);
  console.log(`draw and present: median ${ms(draws[Math.floor(draws.length / 2)] ?? 0)}, slowest ${ms(draws.at(-1) ?? 0)}; longest input wait ${ms(Math.max(0, ...trace.frames.map(f => f.late)))}`);
  console.log(`app (draw and traced work): median ${ms(apps[Math.floor(apps.length / 2)] ?? 0)}, slowest ${ms(apps.at(-1) ?? 0)}; frames over ${trace.limit_ms} ms by app ${slowFrames(trace, { app: true }).length}, by draw, present and input wait ${slowFrames(trace).length}`);
  for (const f of slow) console.log(describe(f));
  const stuck = trace.stuck ?? [];
  for (const s of stuck) console.log(`stuck: ${s.events} wheel events from ${(s.from / 1000).toFixed(2)} s to ${(s.to / 1000).toFixed(2)} s left the list where it stood, the pointer over ${s.under ?? "nothing traced"}`);
  if (trace.under !== undefined) console.log(`under the pointer: ${Object.entries(trace.under).map(([kind, u]) => `${kind} ${u.moved}/${u.events} moved`).join(", ")}`);
  const why = unfit(trace);
  if (why !== null) {
    console.error(`trace: ${why}`);
    process.exit(2);
  }
  const by = app ? " by app" : "";
  console.log(slow.length === 0 ? `no frame over ${trace.limit_ms} ms${by}` : `${slow.length} frames over ${trace.limit_ms} ms${by}`);
  console.log(stuck.length === 0 ? "no stuck wheel" : `${stuck.length} stuck runs`);
  process.exit(slow.length === 0 && stuck.length === 0 ? 0 : 1);
}
