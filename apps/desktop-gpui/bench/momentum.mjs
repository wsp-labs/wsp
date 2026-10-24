#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// Trackpad scrolling as a Mac sends it, for `desktop-gpui --bench-replay`: gestures of a Started event, Moved events
// every 8.33 ms ramping up while the fingers move, Ended when they lift, then the momentum's Moved events decaying
// to under half a pixel (GPUI's Mac platform reads a momentum event as Moved). Between gestures a pause, sometimes a
// long one; some gestures catch the momentum of the last one; most scroll up, toward older messages, so a thread
// opened at its end pages back while it is scrolled. One JSON line an event:
// { t: ms from the first, dx, dy: pixels (dy > 0 scrolls up), unit: "px", phase: "started" | "moved" | "ended", x, y },
// x and y the pointer in the thread list's own pixels. Fingers never swipe straight, so each gesture drifts sideways a
// few percent of its speed, and its momentum keeps that drift as it decays.
//   node momentum.mjs [--seed <n>] [--seconds <s>] [--x <px> --y <px>] [--out <events.jsonl>]
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { rng } from "./real.mjs";

/** A 120 Hz trackpad's report spacing. */
export const SPACING = 1000 / 120;
/** Momentum ends once a report would move less than this. */
export const MOMENTUM_FLOOR = 0.5;

const round = v => Math.round(v * 100) / 100;

/** The events of `seconds` of scrolling from `seed`, the same on every call. */
export function momentum({ seed = 1, seconds = 60, x = 420, y = 320 } = {}) {
  const r = rng(seed);
  const between = (lo, hi) => lo + r() * (hi - lo);
  const end = seconds * 1000;
  const events = [];
  let t = 0;
  while (t < end) {
    const up = r() < 0.85 ? 1 : -1;
    const flick = r() < 0.6;
    const peak = flick ? between(25, 90) : between(2, 12);
    const fingers = Math.max(4, Math.round(between(60, 350) / SPACING));
    const ramp = Math.max(3, fingers * 0.4);
    const at = { x: round(x + between(-3, 3)), y: round(y + between(-3, 3)) };
    const drift = between(-0.06, 0.06);
    const say = (phase, dy, wobble = 0) => events.push({ t: round(t), dx: round(dy * drift + wobble), dy: round(up * dy), unit: "px", phase, ...at });
    say("started", peak * 0.1);
    let v = 0;
    for (let k = 1; k <= fingers; k++) {
      t += SPACING;
      v = peak * Math.min(1, k / ramp) * between(0.9, 1.1);
      say("moved", v, between(-0.3, 0.3));
    }
    t += SPACING;
    say("ended", 0);
    let caught = false;
    if (flick || r() < 0.3) {
      const decay = between(0.955, 0.985);
      const catchAt = r() < 0.2 ? Math.floor(between(0.1, 0.6) * Math.log(MOMENTUM_FLOOR / v) / Math.log(decay)) : Infinity;
      for (let k = 0; (v *= decay) >= MOMENTUM_FLOOR; k++) {
        if (k >= catchAt) {
          caught = true;
          break;
        }
        t += SPACING;
        say("moved", v);
      }
    }
    t += caught ? between(0, 30) : r() < 0.1 ? between(1500, 4000) : between(80, 600);
  }
  return events.filter(e => e.t <= end);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2);
  const at = flag => (argv.indexOf(flag) >= 0 ? argv[argv.indexOf(flag) + 1] : undefined);
  const lines = momentum({ seed: Number(at("--seed") ?? 1), seconds: Number(at("--seconds") ?? 60), x: Number(at("--x") ?? 420), y: Number(at("--y") ?? 320) }).map(e => JSON.stringify(e)).join("\n") + "\n";
  if (at("--out") !== undefined) writeFileSync(at("--out"), lines);
  else process.stdout.write(lines);
}
