// SPDX-License-Identifier: AGPL-3.0-only
// A command's output read summary first (#2468), off what vitest 2.1.9 and cargo 1.97.1 printed on aws-mumbai.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseSlate, sketchSlate, slateRunSummary, slateStartValues, slateSummaryCounts } from "../../src/slate/index.js";

const lines = (name: string): string[] => readFileSync(new URL(`./output/${name}`, import.meta.url), "utf8").split("\n");

describe("a run's summary", () => {
  it("reads vitest's Tests line over its Test Files line, and its failure lines", () => {
    const s = slateRunSummary(lines("vitest.txt"));
    expect([s.failed, s.passed, s.skipped]).toEqual([1, 1, 1]);
    expect(slateSummaryCounts(s).map(c => c.words)).toEqual(["1 failed", "1 passed", "1 skipped"]);
    expect(s.matters).toEqual([
      "× checkout > rounds a refund 7ms",
      "FAIL |node|  src/checkout.test.ts > checkout > rounds a refund",
      "AssertionError: expected 0.30000000000000004 to be 0.3 // Object.is equality",
    ]);
  });

  it("reads the same through the terminal's colours", () => {
    // Only the last line differs: the coloured run took a millisecond less to transform.
    expect({ ...slateRunSummary(lines("vitest-color.txt")), last: "" }).toEqual({ ...slateRunSummary(lines("vitest.txt")), last: "" });
  });

  it("sums cargo's test result lines and names the failed test and its panic", () => {
    const s = slateRunSummary(lines("cargo.txt"));
    expect([s.failed, s.passed, s.skipped]).toEqual([1, 1, 1]);
    expect(s.matters[0]).toBe("test tests::rounds_refund ... FAILED");
    expect(s.matters).toContain("thread 'tests::rounds_refund' (2383223) panicked at src/lib.rs:6:34:");
    const twice = slateRunSummary([...lines("cargo.txt"), "test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"]);
    expect([twice.failed, twice.passed]).toEqual([1, 5]);
  });

  it("leads with the last line where no runner's tally reads, and counts nothing", () => {
    const s = slateRunSummary(["building acme", "wrote dist/acme.js", ""]);
    expect(slateSummaryCounts(s)).toEqual([]);
    expect(s.last).toBe("wrote dist/acme.js");
  });

  it("is what the sketch says of an output: state and counts, the failed lines, the raw lines folded", () => {
    const doc = parseSlate(`<slate><run name="tests" cmd="pnpm test" /><column><output run={$tests} /></column></slate>`).document!;
    const out = lines("vitest.txt").join("\n");
    const sketch = sketchSlate(doc, { ...slateStartValues(doc), tests: { state: "failed", exit: 1, out, runs: 1 } }, { now: 0 });
    expect(sketch).toContain(`output $tests: failed, 1 failed, 1 passed, 1 skipped; ${out.split("\n").filter(l => l !== "").length} raw lines folded  [output-1 output]\n  × checkout > rounds a refund 7ms\n`);
  });
});
