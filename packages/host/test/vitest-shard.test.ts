// SPDX-License-Identifier: AGPL-3.0-only
// CI's eight test parts, dealt by each file's time in CI: every file in exactly one part, the parts even, and the same
// deal on every runner whatever order vitest hands the files in.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import config from "../../../vitest.config.js";
import { deal, TimedSequencer } from "../../../vitest.shard.js";

const TIMES: Record<string, number> = JSON.parse(readFileSync(fileURLToPath(new URL("../../../scripts/test-times.json", import.meta.url)), "utf8"));
const KEYS = Object.keys(TIMES);

const parts = (keys: string[], count: number, times = TIMES): string[][] => Array.from({ length: count }, (_, i) => deal(keys, i + 1, count, times));
const load = (part: string[], times = TIMES): number => part.reduce((sum, key) => sum + (times[key] ?? 0), 0);

describe("the test parts by time", () => {
  it("puts every file of the suite in one part, with no two parts further apart than the heaviest file", () => {
    const eight = parts(KEYS, 8);
    expect(eight.flat().sort()).toEqual([...KEYS].sort());
    const loads = eight.map(part => load(part));
    expect(Math.max(...loads) - Math.min(...loads)).toBeLessThanOrEqual(Math.max(...Object.values(TIMES)));
  });

  it("deals the same parts whatever order the files come in", () => {
    expect(parts([...KEYS].reverse(), 8)).toEqual(parts(KEYS, 8));
  });

  it("deals a file the table lacks into one part, at the table's median", () => {
    const times = { "node:a.test.ts": 9000, "node:b.test.ts": 1000, "node:c.test.ts": 1000, "node:d.test.ts": 1000 };
    const two = parts(["node:a.test.ts", "node:b.test.ts", "node:c.test.ts", "node:new.test.ts"], 2, times);
    expect(two).toEqual([["node:a.test.ts"], ["node:b.test.ts", "node:c.test.ts", "node:new.test.ts"]]);
  });

  it("leaves a part empty when the run picked fewer files than there are parts", () => {
    const eight = parts(KEYS.slice(0, 3), 8);
    expect(eight.flat().sort()).toEqual(KEYS.slice(0, 3).sort());
    expect(eight.filter(part => part.length === 0)).toHaveLength(5);
  });
});

describe("the root config", () => {
  it("deals CI's parts with the timed sequencer", () => {
    expect(config.test?.sequence?.sequencer).toBe(TimedSequencer);
  });
});
