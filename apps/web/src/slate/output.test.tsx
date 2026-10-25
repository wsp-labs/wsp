// SPDX-License-Identifier: AGPL-3.0-only
// The output piece reads summary first (#2468): what passed and failed, the lines that failed, the raw lines folded.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { parseSlate, slateStartValues, type SlateJson } from "@wsp/protocol/slate";
import { ActionRunner, StateSender } from "./actions";
import { SlateEngine } from "./engine";
import { SLATE_VIEWS } from "./pieces";
import { SlateView } from "./SlateView";
import { fakeLink, manualScheduler } from "./testing";

afterEach(cleanup);

const VITEST = readFileSync(join(__dirname, "../../../../packages/protocol/test/slate/output/vitest.txt"), "utf8");

function draw(values: Record<string, SlateJson>) {
  const doc = parseSlate(`<slate><run name="tests" cmd="pnpm test" /><column><output id="out" label="Unit tests" run={$tests} /></column></slate>`).document!;
  const engine = new SlateEngine("t1", () => undefined, manualScheduler());
  engine.setRecord(doc, { ...slateStartValues(doc), ...values }, 3, 3);
  const link = fakeLink();
  return render(<SlateView engine={engine} views={SLATE_VIEWS} runner={new ActionRunner(engine, () => link)} sender={new StateSender(engine, () => link)} />).container;
}

describe("the output piece", () => {
  it("leads with the runner's counts, failed first in the error ink, then the lines that failed, and folds the raw lines", () => {
    const c = draw({ tests: { state: "failed", exit: 1, out: VITEST, runs: 1 } });
    const counts = [...c.querySelectorAll<HTMLElement>("[data-slate-count]")];
    expect(counts.map(n => n.textContent)).toEqual(["1 failed", "1 passed", "1 skipped"]);
    expect(counts[0]!.className).toContain("text-error-foreground");
    expect(c.querySelector("[data-slate-matters]")!.textContent).toContain("× checkout > rounds a refund 7ms");
    expect(c.querySelector("[role=log]")).toBeNull();
    const fold = c.querySelector<HTMLButtonElement>("[data-slate-fold]")!;
    expect(fold.getAttribute("aria-expanded")).toBe("false");
    expect(fold.textContent).toBe(`All ${VITEST.replace(/\n$/, "").split("\n").length} lines`);
    fireEvent.click(fold);
    expect(c.querySelector("[role=log]")!.textContent).toContain("Duration  336ms");
  });

  it("leads with the last line where no runner's tally reads", () => {
    const c = draw({ tests: { state: "done", exit: 0, out: "building acme\nwrote dist/acme.js\n", runs: 1 } });
    expect(c.querySelector("[data-slate-count]")).toBeNull();
    expect(c.querySelector("[data-slate-summary]")!.textContent).toBe("wrote dist/acme.js");
  });
});
