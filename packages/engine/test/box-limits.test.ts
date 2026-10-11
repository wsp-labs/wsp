// SPDX-License-Identifier: AGPL-3.0-only
// Boat's plan and starts as a cloud's row reads them, off GET /limits, and how often that read reaches Boat.
import { describe, expect, it } from "vitest";
import { BOX_CLASSES, BoxBackend } from "../src/box-backend.js";
import { boxLimitsOf } from "../src/box-place-limits.js";
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

function limitsApi(answer: () => Response): { fetch: typeof fetch; asked: string[] } {
  const asked: string[] = [];
  const fetch = (async (url: string | URL | Request, init?: RequestInit) => {
    asked.push(`${init?.method ?? "GET"} ${String(url)}`);
    return answer();
  }) as typeof globalThis.fetch;
  return { fetch, asked };
}

const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

describe("Boat's plan and starts off /limits", () => {
  it("reads the owner's plan, its machines at once, the sizes it sells and the starts used in each window", () => {
    expect(boxLimitsOf(OWNER_LIMITS, BOX_CLASSES)).toEqual({
      plan: { key: "box_20", trial: false, machinesAtOnce: 100, sizes: ["2x4", "4x8", "8x16"] },
      starts: { minute: { used: 0, limit: 12 }, hour: { used: 9, limit: 60 }, day: { used: 187, limit: 200 } },
    });
  });

  it("reads a trial as two machines and the two sizes it sells", () => {
    expect(boxLimitsOf(TRIAL_LIMITS, BOX_CLASSES)).toEqual({
      plan: { key: "trial", trial: true, machinesAtOnce: 2, sizes: ["2x4", "4x8"] },
      starts: { minute: { used: 1, limit: 5 }, hour: { used: 3, limit: 25 }, day: { used: 3, limit: 75 } },
    });
  });

  it("takes boat.dev's spelling of the machine cap, leaves the starts out where a window has no figure, and reads nothing without a cap", () => {
    const { maxActiveBoxes: _cap, ...rest } = OWNER_LIMITS;
    expect(boxLimitsOf({ ...rest, maxActiveSandboxes: 100 }, BOX_CLASSES)?.plan.machinesAtOnce).toBe(100);
    expect(boxLimitsOf({ ...OWNER_LIMITS, starts: { minute: { limit: 12, used: 0 }, hour: { used: 9 }, day: { limit: 200, used: 187 } } }, BOX_CLASSES)).toEqual({ plan: expect.objectContaining({ key: "box_20" }) });
    expect(boxLimitsOf(rest, BOX_CLASSES)).toBeUndefined();
  });

  it("asks Boat once in ten seconds whatever the answer, and nothing for a read that failed", async () => {
    let at = 0;
    const clock: RetryClock = { now: () => at, sleep: async () => {} };
    let status = 200;
    const api = limitsApi(() => (status === 200 ? json(OWNER_LIMITS) : json({ code: "internal", message: "down" }, status)));
    const boat = new BoxBackend({ apiKey: "box_x", fetch: api.fetch, clock });
    expect((await boat.placeLimits())?.starts?.day).toEqual({ used: 187, limit: 200 });
    at = 9_999;
    await boat.placeLimits();
    expect(api.asked).toEqual(["GET https://ascii.dev/api/box/v1/limits"]);
    at = 10_000;
    status = 500;
    expect(await boat.placeLimits()).toBeUndefined();
    at = 15_000;
    expect(await boat.placeLimits()).toBeUndefined();
    expect(api.asked).toHaveLength(2);
    at = 20_000;
    status = 200;
    expect((await boat.placeLimits())?.plan.key).toBe("box_20");
    expect(api.asked).toHaveLength(3);
  });
});
