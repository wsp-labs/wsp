// SPDX-License-Identifier: AGPL-3.0-only
// The bench's table, from each app's result JSON and its three memory readings:
//   node table.mjs <app>=<result.json>,<MB at rest>,<MB with long open>,<MB after scrolling long> ...
// An app whose result is missing or empty prints dashes rather than stopping the table.
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";

export function median(values) {
  if (values.length === 0) return NaN;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

const gapsOf = frameMs => frameMs.slice(1).map((t, i) => t - frameMs[i]);

/** Frames per second from presented frame times: the median frame interval, and the mean of the slowest 1% of
 * intervals (at least one), each as a rate. Before the slowest are taken each interval counts as the whole number of
 * refreshes it spans: Electron's times are frame starts and GPUI's are presents, and either clock's jitter around one
 * refresh would otherwise read as a slow frame in one app only. The table hands every app one refresh, the shortest
 * median interval among them, so a hitch counts in the same units whichever app's median it is. */
export function fps(frameMs, refresh = median(gapsOf(frameMs))) {
  const gaps = gapsOf(frameMs);
  if (gaps.length === 0) return { median: NaN, low1: NaN, frames: frameMs.length };
  const whole = gaps.map(g => Math.max(1, Math.round(g / refresh)) * refresh);
  const slowest = whole.sort((a, b) => b - a).slice(0, Math.max(1, Math.floor(gaps.length / 100)));
  return { median: 1000 / median(gaps), low1: 1000 / (slowest.reduce((s, g) => s + g, 0) / slowest.length), frames: frameMs.length };
}

export function row(app, result, restMb, longMb, scrolledMb, refresh) {
  const scroll = result === undefined ? undefined : fps(result.scroll.frame_ms, refresh);
  const fmt = (n, digits = 0) => (n === undefined || Number.isNaN(n) ? "-" : n.toFixed(digits));
  // The timed switches after the bench's own, which only the GPUI bench writes.
  const timed = result?.switches;
  const ms = value => (value === undefined || value === null ? undefined : value);
  return [
    app,
    fmt(restMb),
    fmt(longMb),
    fmt(scrolledMb),
    fmt(scroll?.median, 1),
    fmt(scroll?.low1, 1),
    fmt(result === undefined ? undefined : median(result.switch.map(s => s.ms)), 1),
    fmt(ms(timed?.median_ms), 1),
    fmt(ms(timed?.p90_ms), 1),
    fmt(ms(timed?.max_ms), 1),
    result === undefined ? "-" : String(result.thread_rows),
  ];
}

export function table(rows) {
  const head = ["app", "MB at rest", "MB long open", "MB after scrolling long", "scroll fps median", "scroll fps slowest 1%", "switch ms median", "30 switches ms median", "30 switches ms p90", "30 switches ms max", "rows shown"];
  const all = [head, ...rows];
  const widths = head.map((_, i) => Math.max(...all.map(r => r[i].length)));
  const line = r => r.map((cell, i) => (i === 0 ? cell.padEnd(widths[i]) : cell.padStart(widths[i]))).join("  ");
  return [line(head), widths.map(w => "-".repeat(w)).join("  "), ...rows.map(line)].join("\n");
}

if (process.argv[1] !== undefined && fileURLToPath(import.meta.url) === realpathSync(process.argv[1])) {
  const runs = process.argv.slice(2).map(arg => {
    const [app, rest] = arg.split("=");
    const [file, restMb, longMb, scrolledMb] = rest.split(",");
    const result = existsSync(file) && readFileSync(file, "utf8").trim() !== "" ? JSON.parse(readFileSync(file, "utf8")) : undefined;
    const mb = s => (s === undefined || s === "" ? undefined : Number(s));
    return { app, result, mbs: [mb(restMb), mb(longMb), mb(scrolledMb)] };
  });
  const medians = runs.filter(r => r.result !== undefined && r.result.scroll.frame_ms.length > 1).map(r => median(gapsOf(r.result.scroll.frame_ms)));
  const refresh = medians.length > 0 ? Math.min(...medians) : undefined;
  const rows = runs.map(r => row(r.app, r.result, ...r.mbs, refresh));
  console.log(table(rows));
}
