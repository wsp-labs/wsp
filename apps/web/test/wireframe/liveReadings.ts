// SPDX-License-Identifier: AGPL-3.0-only
// The readings the settings-computers-live screen feeds the Computers list: each computer's last fifteen minutes when
// the page opens, then a sample every two seconds on its own clock, offset as separate daemon frames would be. Every
// reading is a function of its time, so the window read on open and the samples after it are one series.
import type { SysSample } from "@wsp/protocol";
import { BAND_WINDOW_MS, feedSample, feedWindow } from "../../src/machine/computerLive";

const GB = 1024 ** 3;
const TICK_MS = 2_000;

/** A steady figure in [0, 1) for a seed and a tick, so a reload draws the same history. */
const noise = (seed: number, tick: number): number => {
  const x = Math.sin(seed * 127.1 + tick * 311.7) * 43_758.5453;
  return x - Math.floor(x);
};

const ramp = (at: number, from: number, to: number): number => Math.min(1, Math.max(0, (at - from) / (to - from)));

interface Fixture {
  readonly id: string;
  readonly memTotal: number;
  readonly read: (at: number) => { cpu: number; mem: number };
}

/** A build every six minutes: four minutes of CPU past 85% with memory climbing to 14.7 of 16 GB, then rest. */
const MAC: Fixture = {
  id: "here",
  memTotal: 16 * GB,
  read: at => {
    const tick = Math.floor(at / TICK_MS);
    const s = (at / 1000) % 360;
    const building = s < 240;
    const cpu = building ? 12 + ramp(s, 0, 20) * (74 + noise(1, tick) * 12) + (noise(2, tick) > 0.85 ? 8 : 0) : 5 + noise(3, tick) * 9;
    const mem = building ? 9.6 + ramp(s, 0, 90) * (4.95 + noise(4, tick) * 0.25) : 9.6 + (1 - ramp(s, 240, 290)) * 5 + noise(5, tick) * 0.1;
    return { cpu: Math.min(99, cpu), mem: mem * GB };
  },
};

/** Idle at an honest 0 to 3%, with a test run every five minutes. */
const HETZNER: Fixture = {
  id: "p_hetzner",
  memTotal: 16 * GB,
  read: at => {
    const tick = Math.floor(at / TICK_MS);
    const s = (at / 1000) % 300;
    const testing = s >= 200 && s < 260;
    const cpu = testing ? 34 + noise(6, tick) * 22 : noise(7, tick) > 0.6 ? Math.floor(noise(8, tick) * 4) : 0;
    const mem = 3.1 + (testing ? ramp(s, 200, 215) * 0.9 : 0) + noise(9, tick) * 0.05;
    return { cpu, mem: mem * GB };
  },
};

/** A box that stopped answering eleven minutes ago, mid-job. */
const LAB: Fixture = {
  id: "p_lab",
  memTotal: 8 * GB,
  read: at => {
    const tick = Math.floor(at / TICK_MS);
    return { cpu: 18 + noise(10, tick) * 14, mem: (4.9 + noise(11, tick) * 0.3) * GB };
  },
};

export const LAB_AWAY_MS = 11 * 60_000;

const sample = (f: Fixture, at: number): SysSample => {
  const { cpu, mem } = f.read(at);
  return { type: "sys.sample", cpu, load1: cpu / 25, mem: { used: mem, total: f.memTotal }, disk: { used: 120 * GB, total: 460 * GB }, at };
};

const history = (f: Fixture, to: number): SysSample[] => {
  const out: SysSample[] = [];
  for (let at = to - BAND_WINDOW_MS; at <= to; at += TICK_MS) out.push(sample(f, at));
  return out;
};

/** Feeds the full windows, then each live computer every two seconds; the box away stays as it was left. */
export function startLiveReadings(now = Date.now()): void {
  feedWindow(LAB.id, history(LAB, now - LAB_AWAY_MS), true);
  [MAC, HETZNER].forEach((f, i) => {
    feedWindow(f.id, history(f, now));
    setTimeout(() => setInterval(() => feedSample(f.id, sample(f, Date.now())), TICK_MS), i * 700);
  });
}
