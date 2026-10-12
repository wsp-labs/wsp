// SPDX-License-Identifier: AGPL-3.0-only
import { afterEach, describe, expect, it, vi } from "vitest";
import { RELOAD_WINDOW_MS, RELOADS_PER_MINUTE, reloadAfter, reloadPacer, type DeadPage } from "../src/revive.js";

describe("reloadPacer", () => {
  it("reloads at once up to the cap, then waits for the oldest reload to turn a minute old", () => {
    let t = 1_000_000;
    const wait = reloadPacer({ now: () => t });
    expect(Array.from({ length: RELOADS_PER_MINUTE }, () => wait())).toEqual(Array(RELOADS_PER_MINUTE).fill(0));
    t += 10_000;
    expect(wait()).toBe(RELOAD_WINDOW_MS - 10_000);
  });

  it("never lets a minute hold more than the cap, however fast the page dies", () => {
    let t = 0;
    const wait = reloadPacer({ now: () => t });
    const ran: number[] = [];
    for (let n = 0; n < 50; n++, t += 500) ran.push(t + wait());
    for (const start of ran) expect(ran.filter(r => r >= start && r < start + RELOAD_WINDOW_MS).length).toBeLessThanOrEqual(RELOADS_PER_MINUTE);
  });

  it("reloads at once again once the last minute is clear", () => {
    let t = 0;
    const wait = reloadPacer({ now: () => t });
    for (let n = 0; n < RELOADS_PER_MINUTE; n++) wait();
    t += RELOAD_WINDOW_MS;
    expect(wait()).toBe(0);
  });
});

describe("reloadAfter", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  /** A page whose renderer died; `back` brings it back the way a reload by hand does. */
  const deadPage = (): DeadPage & { reloads: number; back(): void; destroy(): void } => {
    let crashed = true;
    let destroyed = false;
    return {
      reloads: 0,
      isDestroyed: () => destroyed,
      isCrashed: () => crashed,
      reload() {
        this.reloads++;
        crashed = false;
      },
      back: () => void (crashed = false),
      destroy: () => void (destroyed = true),
    };
  };

  it("reloads a page still dead when the wait is over", () => {
    vi.useFakeTimers();
    const page = deadPage();
    reloadAfter(page, 40_000);
    vi.advanceTimersByTime(39_999);
    expect(page.reloads).toBe(0);
    vi.advanceTimersByTime(1);
    expect(page.reloads).toBe(1);
  });

  it("leaves alone a page brought back during the wait, and one whose window went", () => {
    vi.useFakeTimers();
    const back = deadPage();
    const gone = deadPage();
    reloadAfter(back, 40_000);
    reloadAfter(gone, 40_000);
    back.back();
    gone.destroy();
    vi.advanceTimersByTime(40_000);
    expect([back.reloads, gone.reloads]).toEqual([0, 0]);
  });
});
