// SPDX-License-Identifier: AGPL-3.0-only
// Boat's plan and starts as a cloud's row carries them, off the account's one limits read.
import { describe, expect, it } from "vitest";
import { BoxBackend, limitsOf, placeLimitsOf } from "../src/box-backend.js";
import type { RetryClock } from "../src/errors.js";

/** GET /limits on the owner's account, 2026-10-11 17:06 UTC, trimmed to the fields phase 1 reads (spec 01). */
const OWNER_LIMITS = {
  accessTier: "standard",
  maxActiveBoxes: 100,
  activeBoxes: 11,
  startLimits: { perMinute: 12, perHour: 60, perDay: 200 },
  starts: { unlimited: false, minute: { limit: 12, used: 0, remaining: 12 }, hour: { limit: 60, used: 9, remaining: 51 }, day: { limit: 200, used: 187, remaining: 13 } },
  activeStates: ["provisioned", "cloning", "ready", "idle", "running"],
  canStart: true,
  startBlockedReason: null,
  creditBalanceHours: 81.6,
  boxPlanKey: "box_20",
};

/** The trial's limits as Boat's pricing states them: two machines, 5, 25 and 75 starts. */
const TRIAL_LIMITS = {
  accessTier: "trial",
  maxActiveBoxes: 2,
  starts: { minute: { limit: 5, used: 1, remaining: 4 }, hour: { limit: 25, used: 3, remaining: 22 }, day: { limit: 75, used: 3, remaining: 72 } },
};

const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

describe("Boat's plan and starts on its row", () => {
  it("reads the owner's plan, its machines at once, the sizes it sells and the starts used in each window", () => {
    expect(placeLimitsOf(limitsOf(OWNER_LIMITS))).toEqual({
      plan: { key: "box_20", trial: false, machinesAtOnce: 100, sizes: ["2x4", "4x8", "8x16"] },
      starts: { minute: { used: 0, limit: 12 }, hour: { used: 9, limit: 60 }, day: { used: 187, limit: 200 } },
    });
  });

  it("sells 16x32 from the $100 plan up, and two sizes on the trial", () => {
    expect(placeLimitsOf(limitsOf({ ...OWNER_LIMITS, boxPlanKey: "box_100" }))?.plan.sizes).toEqual(["2x4", "4x8", "8x16", "16x32"]);
    expect(placeLimitsOf(limitsOf(TRIAL_LIMITS))).toEqual({
      plan: { trial: true, machinesAtOnce: 2, sizes: ["2x4", "4x8"] },
      starts: { minute: { used: 1, limit: 5 }, hour: { used: 3, limit: 25 }, day: { used: 3, limit: 75 } },
    });
  });

  it("carries nothing for a read that names no machine cap", () => {
    const { maxActiveBoxes: _cap, ...rest } = OWNER_LIMITS;
    expect(placeLimitsOf(limitsOf(rest))).toBeUndefined();
  });

  it("reads the backend's own kept read, a start counted on it, and nothing once a read fails", async () => {
    let at = 0;
    const clock: RetryClock = { now: () => at, sleep: async () => {} };
    let status = 200;
    const asked: string[] = [];
    const fetch = (async (url: string | URL | Request) => {
      asked.push(String(url));
      return status === 200 ? json(OWNER_LIMITS) : json({ code: "internal", message: "down" }, status);
    }) as typeof globalThis.fetch;
    const boat = new BoxBackend({ apiKey: "box_x", fetch, clock });
    expect((await boat.placeLimits())?.starts?.day).toEqual({ used: 187, limit: 200 });
    boat.counted();
    expect((await boat.placeLimits())?.starts?.day).toEqual({ used: 188, limit: 200 });
    expect(asked).toHaveLength(1);
    at = 60_000;
    status = 400;
    expect(await boat.placeLimits()).toBeUndefined();
  });
});
