// SPDX-License-Identifier: AGPL-3.0-only
// The band under each of the person's own computers on Settings > Computers: a CPU lane over a memory lane, full
// from the host's fifteen minutes when the list opens, moved by every two-second sample, in the Usage page's first
// two inks with memory warning from 90%, greyed with when it was last read for a computer that stopped answering,
// and one watch per computer for as long as the list is open.
import { act, cleanup, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaceSysEvent, PlaceView, ReadingsAnswer, SysPoint, SysSample } from "@wsp/protocol";
import type { Api } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { BAND_WINDOW_MS, STEP_MS, feedSample, feedWindow, forgetComputerReadings, readingsOf } from "../src/machine/computerLive.js";
import { useSettingsStore } from "../src/settings/settingsStore.js";
import { mountSettings, resetSettings, settingsApi, settle } from "./settings-harness.js";

const GB = 1024 ** 3;

const here: PlaceView = { id: "here", kind: "computer", name: "acme-mbp", label: "acme's MacBook Pro", default: true, present: true, takesForks: false, shape: { cpu: 8, memMb: 16384 } };
const box: PlaceView = { id: "p_2", kind: "computer", name: "lab", default: false, present: true, takesForks: true, os: "Ubuntu 24.04", shape: { cpu: 2, memMb: 4096 } };
const away: PlaceView = { ...box, id: "p_3", name: "attic", present: false, lastSeenAt: "2026-10-11T06:03:00.000Z" };
const cloud: PlaceView = { id: "solari", kind: "provider", name: "solari", default: false, rateUsdPerHour: 0.11, takesForks: true };

const point = (at: number, cpu: number, usedGb: number, totalGb = 16): SysPoint => ({ at, cpu, load1: 0, mem: { used: usedGb * GB, total: totalGb * GB }, disk: { used: 0, total: 1 } });
const sample = (at: number, cpu: number, usedGb: number, totalGb = 16): SysSample => ({ type: "sys.sample", ...point(at, cpu, usedGb, totalGb) });

/** The host's side: the fifteen minutes it answers for each computer, the watches it holds, and a push per sample. */
function fakeHost(windows: Record<string, (to: number) => SysPoint[]>) {
  const watching = new Set<string>();
  const asked: string[] = [];
  const left: string[] = [];
  const reads: string[] = [];
  const fns = new Set<(e: PlaceSysEvent) => void>();
  const api: Partial<Api> = {
    placesReadings: async (placeId, range) => {
      reads.push(`${placeId} ${range}`);
      const to = Date.now();
      const of = windows[placeId];
      if (of === undefined) throw new Error(`${placeId} is not answering`);
      return { points: of(to), stepMs: 15_000, from: to - BAND_WINDOW_MS, to } satisfies ReadingsAnswer;
    },
    watchPlaceSys: async placeId => {
      asked.push(placeId);
      watching.add(placeId);
    },
    unwatchPlaceSys: async placeId => {
      left.push(placeId);
      watching.delete(placeId);
    },
    onPlaceSysSample: fn => {
      fns.add(fn);
      return () => fns.delete(fn);
    },
  };
  const push = (placeId: string, s: SysSample): void =>
    act(() => {
      for (const fn of fns) fn({ type: "place.sys", placeId, sample: s });
    });
  return { api, watching, asked, left, reads, push };
}

/** A full fifteen minutes at the daemon's fifteen-second reads, ending at `to`. */
const quarter = (cpu: (i: number) => number, usedGb: number, totalGb = 16) => (to: number): SysPoint[] =>
  Array.from({ length: 61 }, (_, i) => point(to - BAND_WINDOW_MS + i * 15_000, cpu(i), usedGb, totalGb));

const bandOf = (id: string): HTMLElement | null => document.querySelector<HTMLElement>(`[data-settings-page] [data-place-row='${id}'] [data-live-band]`);
const lanes = (band: HTMLElement) => [...band.querySelectorAll<SVGSVGElement>("svg[role=img]")];
const lineOf = (lane: SVGSVGElement): string => lane.querySelector("[data-chart-line]")!.getAttribute("d")!;
/** Every point of a lane's line, in the plot's own units: x 0 to 100 over the window, y down from the top. */
const pointsOf = (lane: SVGSVGElement): { x: number; y: number }[] =>
  [...lineOf(lane).matchAll(/[ML]([\d.]+) ([\d.]+)/g)].map(m => ({ x: Number(m[1]), y: Number(m[2]) }));
const figure = (band: HTMLElement, k: "cpu" | "mem"): HTMLElement => band.querySelector<HTMLElement>(`[data-k='${k}']`)!;

const mount = async (places: PlaceView[], api: Partial<Api>) => {
  useStore.setState({ places });
  const view = mountSettings({ api: settingsApi(api).api, at: { kind: "group", group: "computers" } });
  await settle();
  return view;
};

beforeEach(() => {
  resetSettings();
  forgetComputerReadings();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("the band under each computer on the Computers list", () => {
  it("draws a CPU lane over a memory lane under this computer and every box, each lane's figure at its right, and none under a cloud", async () => {
    const host = fakeHost({ here: quarter(() => 12, 9.6), p_2: quarter(() => 3, 2) });
    await mount([here, box, cloud], host.api);
    await waitFor(() => expect(bandOf("p_2")).not.toBeNull());
    for (const id of ["here", "p_2"]) {
      const band = bandOf(id)!;
      expect(lanes(band).map(lane => lane.getAttribute("aria-label"))).toEqual(["CPU over the last 15 minutes", "Memory over the last 15 minutes"]);
      // The CPU lane, its figure, then the memory lane and its figure: a lane over a lane on the band's two grid rows,
      // each figure after its lane.
      expect([...band.children].map(el => el.querySelector("svg")?.getAttribute("aria-label")?.split(" ")[0] ?? el.getAttribute("data-k"))).toEqual(["CPU", "cpu", "Memory", "mem"]);
    }
    expect(figure(bandOf("here")!, "cpu").textContent).toBe("CPU12%");
    expect(figure(bandOf("here")!, "mem").textContent).toBe(`Memory${"9.6 GB"}`);
    expect(document.querySelector("[data-settings-page] [data-place-row='solari']")).not.toBeNull();
    expect(bandOf("solari")).toBeNull();
    expect(host.asked).not.toContain("solari");
  });

  it("holds the last fifteen minutes as ten-second steps: the mean CPU over each, its last memory and its last CPU, and nothing older", () => {
    const to = 1_800_000_000_000;
    const two = Array.from({ length: BAND_WINDOW_MS / 2_000 }, (_, i) => sample(to - BAND_WINDOW_MS + i * 2_000, i % 5 === 0 ? 50 : 0, 1 + i / 1000));
    for (const s of two) feedSample("p_2", s);
    const folded = readingsOf("p_2");
    expect(folded).toHaveLength(BAND_WINDOW_MS / STEP_MS);
    expect(new Set(folded.slice(1).map((s, i) => s.at - folded[i]!.at))).toEqual(new Set([STEP_MS]));
    expect(new Set(folded.map(s => s.cpu))).toEqual(new Set([10]));
    expect(folded.at(-1)!.mem.used).toBe(two.at(-1)!.mem.used);
    expect(folded.at(-1)!.cpuNow).toBe(0);
    // Another fifteen minutes on, the first window has gone: a computer holds 91 steps at most, however long it runs.
    for (const s of two) feedSample("p_2", { ...s, at: s.at + BAND_WINDOW_MS });
    expect(readingsOf("p_2").length).toBeLessThanOrEqual(BAND_WINDOW_MS / STEP_MS + 1);
    expect(readingsOf("p_2")[0]!.at).toBeGreaterThanOrEqual(two.at(-1)!.at);
  });

  it("is full the moment the list opens, and moves with each two-second sample of every computer at once, with no animation", async () => {
    const host = fakeHost({ here: quarter(i => i % 60, 9), p_2: quarter(() => 20, 2) });
    await mount([here, box], host.api);
    await waitFor(() => expect(bandOf("here")).not.toBeNull());
    await waitFor(() => expect(bandOf("p_2")).not.toBeNull());
    expect(host.reads.sort()).toEqual(["here 15m", "p_2 15m"]);
    for (const id of ["here", "p_2"]) {
      const xs = pointsOf(lanes(bandOf(id)!)[0]!).map(p => p.x);
      // Full: the line starts at the window's left edge and ends at its right.
      expect(xs[0]).toBeLessThan(1);
      expect(xs.at(-1)).toBe(100);
    }
    const before = { here: lineOf(lanes(bandOf("here")!)[0]!), p_2: lineOf(lanes(bandOf("p_2")!)[0]!) };
    const now = Date.now();
    host.push("here", sample(now + 2_000, 77, 9.2));
    host.push("p_2", sample(now + 2_100, 41, 2.5));
    expect(figure(bandOf("here")!, "cpu").textContent).toBe("CPU77%");
    expect(figure(bandOf("p_2")!, "cpu").textContent).toBe("CPU41%");
    expect(lineOf(lanes(bandOf("here")!)[0]!)).not.toBe(before.here);
    expect(lineOf(lanes(bandOf("p_2")!)[0]!)).not.toBe(before.p_2);
    host.push("here", sample(now + 4_000, 5, 9.2));
    expect(figure(bandOf("here")!, "cpu").textContent).toBe("CPU5%");
    // No easing between samples: nothing in a band animates or transitions, and no SVG animation is drawn.
    for (const id of ["here", "p_2"]) {
      const band = bandOf(id)!;
      expect([band, ...band.querySelectorAll<Element>("*")].filter(el => /\b(animate|transition)/.test(el.getAttribute("class") ?? ""))).toEqual([]);
      expect(band.querySelector("animate, animateTransform, animateMotion")).toBeNull();
    }
  });

  it("draws CPU in sky and memory in violet, memory's line, fill and figure in the warning ink from 90%, and a 0% as 0% on the baseline", async () => {
    const host = fakeHost({ here: quarter(() => 0, 8), p_2: quarter(() => 30, 3.6, 4) });
    await mount([here, box], host.api);
    await waitFor(() => expect(bandOf("p_2")).not.toBeNull());
    const calm = bandOf("here")!;
    const [cpuLane, memLane] = lanes(calm).map(lane => lane.parentElement!);
    expect(cpuLane!.className).toContain("text-sky-500");
    expect(memLane!.className).toContain("text-violet-500");
    expect(figure(calm, "cpu").textContent).toBe("CPU0%");
    // Every CPU point sits on the plot's zero baseline, the line drawn under the lane.
    const baseline = Number(lanes(calm)[0]!.querySelector("line")!.getAttribute("y1"));
    expect(new Set(pointsOf(lanes(calm)[0]!).map(p => p.y))).toEqual(new Set([baseline]));
    // 3.6 of 4 GB is 90%: the memory lane and its figure take the warning ink, CPU keeps sky.
    const full = bandOf("p_2")!;
    const [fullCpu, fullMem] = lanes(full).map(lane => lane.parentElement!);
    expect(fullCpu!.className).toContain("text-sky-500");
    expect(fullMem!.className).toContain("text-warning-foreground");
    expect(fullMem!.className).not.toContain("text-violet-500");
    expect(figure(full, "mem").querySelector(".text-warning-foreground")?.textContent).toBe("3.6 GB");
    // One reading under 90% and it is violet again.
    host.push("p_2", sample(Date.now() + 2_000, 30, 3.5, 4));
    expect(lanes(bandOf("p_2")!)[1]!.parentElement!.className).toContain("text-violet-500");
  });

  it("keeps a computer's last window greyed when it stops answering, with when it was last read, and the date when not today", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    const last = new Date(2026, 9, 11, 11, 33).getTime();
    vi.setSystemTime(new Date(2026, 9, 11, 11, 44));
    feedWindow(away.id, quarter(() => 25, 5, 8)(last));
    const host = fakeHost({});
    await mount([away], host.api);
    const band = bandOf(away.id)!;
    expect(band).not.toBeNull();
    for (const lane of lanes(band)) expect(lane.parentElement!.className).toContain("text-muted-foreground/45");
    expect(figure(band, "mem").textContent).toBe("Last read 11:33");
    // The last figure it sent, greyed, never a zero.
    expect(figure(band, "cpu").textContent).toBe("CPU25%");
    cleanup();
    vi.setSystemTime(new Date(2026, 9, 12, 9, 0));
    await mount([away], host.api);
    expect(figure(bandOf(away.id)!, "mem").textContent).toBe("Last read 11 Oct 11:33");
  });

  it("draws no band for a computer it holds nothing for", async () => {
    const host = fakeHost({ here: quarter(() => 10, 8) });
    await mount([here, away], host.api);
    await waitFor(() => expect(bandOf("here")).not.toBeNull());
    expect(bandOf(away.id)).toBeNull();
    expect(document.querySelector(`[data-place-row='${away.id}']`)).not.toBeNull();
  });

  it("greys a band that was live once its computer stops answering, and reads its fifteen minutes again when it comes back", async () => {
    const host = fakeHost({ p_2: quarter(() => 20, 2) });
    await mount([box], host.api);
    await waitFor(() => expect(bandOf("p_2")).not.toBeNull());
    act(() => useStore.setState({ places: [{ ...box, present: false }] }));
    expect(lanes(bandOf("p_2")!)[0]!.parentElement!.className).toContain("text-muted-foreground/45");
    expect(figure(bandOf("p_2")!, "mem").textContent).toMatch(/^Last read /);
    // Its watch stays asked for: the host asks the computer again when its link comes back.
    expect(host.watching).toEqual(new Set(["p_2"]));
    act(() => useStore.setState({ places: [box] }));
    await waitFor(() => expect(host.reads).toEqual(["p_2 15m", "p_2 15m"]));
    expect(lanes(bandOf("p_2")!)[0]!.parentElement!.className).toContain("text-sky-500");
  });

  it("asks for one watch per computer of the person's when the list opens and stops every one when the list is left", async () => {
    const host = fakeHost({ here: quarter(() => 10, 8), p_2: quarter(() => 20, 2) });
    await mount([here, box, away, cloud], host.api);
    await waitFor(() => expect(bandOf("p_2")).not.toBeNull());
    expect(host.asked.sort()).toEqual(["here", "p_2", "p_3"]);
    expect(host.watching).toEqual(new Set(["here", "p_2", "p_3"]));
    act(() => useSettingsStore.getState().go({ kind: "group", group: "appearance" }));
    await settle();
    expect(host.watching).toEqual(new Set());
    expect(host.left.sort()).toEqual(["here", "p_2", "p_3"]);
    // A sample that arrives after the list is gone draws nothing and asks for nothing.
    host.push("here", sample(Date.now() + 2_000, 50, 8));
    expect(host.asked).toHaveLength(3);
  });

  it("asks again on a socket that comes back, since a watch dies with the socket that made it", async () => {
    const host = fakeHost({ here: quarter(() => 10, 8) });
    await mount([here], host.api);
    await waitFor(() => expect(bandOf("here")).not.toBeNull());
    act(() => useStore.setState({ conn: "reconnecting" }));
    expect(lanes(bandOf("here")!)[0]!.parentElement!.className).toContain("text-muted-foreground/45");
    act(() => useStore.setState({ conn: "live" }));
    await waitFor(() => expect(host.asked).toEqual(["here", "here"]));
    expect(host.watching).toEqual(new Set(["here"]));
  });
});
