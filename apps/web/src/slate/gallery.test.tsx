// SPDX-License-Identifier: AGPL-3.0-only
// The renderer problems #2439's gallery found drawing 48 slates (round 2), one test each in the ticket's order; the
// seventeenth, GPUI's every.slate, is the GPUI app's own.
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { parseSlate, slateStartValues, type SlateDoc } from "@wsp/protocol/slate";
import { ActionRunner, StateSender } from "./actions";
import { SlateEngine } from "./engine";
import { SLATE_VIEWS } from "./pieces";
import { isFigure } from "./pieces/look";
import { isStatCell } from "./pieces/riders";
import { SlateView } from "./SlateView";
import { fakeLink, manualScheduler } from "./testing";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

function draw(text: string) {
  const r = parseSlate(text);
  expect(r.errors).toEqual([]);
  const doc: SlateDoc = r.document!;
  const engine = new SlateEngine("t1", () => undefined, manualScheduler());
  engine.setRecord(doc, slateStartValues(doc), 3, 3);
  const link = fakeLink();
  const view = render(<SlateView engine={engine} views={SLATE_VIEWS} runner={new ActionRunner(engine, () => link)} sender={new StateSender(engine, () => link)} />);
  return { engine, c: view.container };
}
const piece = (c: HTMLElement, id: string): HTMLElement => c.querySelector<HTMLElement>(`[data-slate-piece="${id}"]`)!;
const ticks = (c: HTMLElement, id: string): string[] => [...piece(c, id).querySelectorAll("[data-k=tick]")].map(t => t.textContent ?? "");
const DAY = 86_400_000;
const at = (width: number) => {
  vi.stubGlobal("ResizeObserver", class {
    constructor(private readonly seen: (entries: Array<{ contentRect: { width: number } }>) => void) {}
    observe() { this.seen([{ contentRect: { width } }]); }
    disconnect() {}
  });
};

describe("the gallery's findings, round 2", () => {
  it("1. a disabled button says why under it", () => {
    const { c } = draw(`<slate><column><button id="go" label="Deploy" held="Checks are still running" onPress={send("Deploy")} /></column></slate>`);
    expect(piece(c, "go").querySelector("[data-slate-held-why]")?.textContent).toBe("Checks are still running");
  });

  it("2. a decision shows the other answers' notes, not only in a tooltip", () => {
    const { c } = draw(`<slate><value name="pick" start="" /><column>
      <decision id="d" label="Ship?" value={$pick} pick="now" onChange={send("Ship", $pick)}><option value="now" label="Now" note="Ready" /><option value="later" label="Later" note="Waits a day" /></decision>
    </column></slate>`);
    expect([...piece(c, "d").querySelectorAll("[data-slate-answer-note]")].map(n => n.textContent)).toEqual(["Later: Waits a day"]);
  });

  it("3. a single line draws in the tone it names", () => {
    const { c } = draw(`<slate><value name="h" start={[{ t: 1, v: 3 }, { t: 2, v: 5 }]} /><column><chart id="ch" label="Errors" items={$h} x={item.t} value={item.v} tone="bad" /></column></slate>`);
    expect(piece(c, "ch").querySelector("figcaption [aria-hidden]")?.className).toContain("text-error-foreground");
  });

  it("4. an axis over days says dates, not clock times, and an ISO date alone reads as its day", () => {
    const from = Date.UTC(2026, 9, 12, 12);
    const days = Array.from({ length: 14 }, (_, i) => `{ at: ${from + i * DAY}, v: ${i} }`).join(", ");
    const { c } = draw(`<slate><value name="ms" start={[${days}]} /><value name="iso" start={[{ at: "2026-10-12", v: 1 }, { at: "2026-10-25", v: 2 }]} /><column>
      <chart id="ms" label="Spend" items={$ms} x={item.at} value={item.v} />
      <chart id="iso" label="Deploys" items={$iso} x={item.at} value={item.v} />
    </column></slate>`);
    expect(ticks(c, "ms")).toEqual(["Oct 12", "Oct 25"]);
    expect(ticks(c, "iso")).toEqual(["Oct 12", "Oct 25"]);
  });

  it("5. a small chart keeps its own height", () => {
    const { c } = draw(`<slate><value name="h" start={[{ t: 1, v: 3 }, { t: 2, v: 5 }]} /><column><chart id="ch" label="Errors" items={$h} x={item.t} value={item.v} height="small" /></column></slate>`);
    expect(piece(c, "ch").querySelector<HTMLElement>("[data-k=y-axis]")?.style.height).toBe("120px");
  });

  it("6. whole dollars say no cents, and dollars and cents keep them", () => {
    const { c } = draw(`<slate><row><number id="plan" label="Annual plan" value={22900} format="usd" /><number id="seat" label="Per seat" value={38.4} format="usd" /></row></slate>`);
    expect(piece(c, "plan").textContent).toContain("$22,900");
    expect(piece(c, "plan").textContent).not.toContain("$22,900.00");
    expect(piece(c, "seat").textContent).toContain("$38.40");
  });

  it("7. a status alone in a section stands bare, not in a card of one short line", () => {
    const { c } = draw(`<slate><column><section title="Then"><status id="lone" tone="bad">Down</status></section></column></slate>`);
    expect(piece(c, "lone").closest("[data-slate-card]")).toBeNull();
  });

  it("8. a list's rows know a field only the rows not done carry", () => {
    const r = parseSlate(`<slate><value name="jobs" start={[{ name: "Lint", done: true }, { name: "Unit", done: false, note: "Running" }]} /><column><checklist items={$jobs} title={item.name} done={item.done} note={item.note} /></column></slate>`);
    expect(r.errors.map(e => e.code)).toEqual([]);
  });

  it("9. a ring beside a number and its status is a strip tile, and the strip pads that number once, not as a stat cell again", () => {
    const { engine, c } = draw(`<slate><column><row id="strip"><ring label="Up" value={209} max={214} format="fraction" /><number label="Today" value={3} /><status tone="warning">over</status></row></column></slate>`);
    expect(piece(c, "strip").querySelector("[data-slate-strip]")).not.toBeNull();
    expect(isStatCell(engine, "strip")).toBe(false);
  });

  it("10. a strip counts a ring's whole figure, 209/214 and its ring, when it decides how many cells stand across", () => {
    const { c } = draw(`<slate><column><row id="strip">
      <ring label="Up" value={209} max={214} format="fraction" /><ring label="Healthy" value={198} max={214} format="fraction" /><ring label="Ready" value={201} max={214} format="fraction" />
    </row></column></slate>`);
    // 209/214 is 7 characters of 15.6 px and a 30 px ring: three do not fit the 366 px card; two would leave a short row, so one a row.
    expect(piece(c, "strip").querySelector("[data-slate-strip]")?.getAttribute("data-across")).toBe("1 3");
  });

  it("11. a table names its rows by a mono first column, in its own face, and its next column reads as a cell", () => {
    const { c } = draw(`<slate><value name="f" start={[{ at: "migrate.ts:88", what: "Drops rows", w: "Blocking" }]} /><column>
      <table id="t" items={$f}><col title="Where" value={item.at} mono={true} /><col title="Finding" value={item.what} /><col title="Weight" value={item.w} /></table>
    </column></slate>`);
    const cells = [...piece(c, "t").querySelectorAll("[role=row]:not([data-slate-head]) [role=cell]")];
    expect(cells[0]?.className).toContain("font-mono");
    expect(cells[1]?.className).toContain("text-muted-foreground");
    expect(cells[1]?.className).not.toContain("text-sm");
  });

  it("12. a table folds at 400 px rather than break its name column's words, and stays a grid at 640", () => {
    const folded = (width: number) => {
      at(width);
      const { c } = draw(`<slate><value name="r" start={[{ t: "proration across a DST change", b: "v2026.10.25-rc3", g: "ap-south-1", f: 24, n: 240 }]} /><column>
        <table id="t" items={$r}><col title="Test" value={item.t} /><col title="Build" value={item.b} mono={true} /><col title="Region" value={item.g} mono={true} /><col title="Fails" value={item.f} /><col title="Runs" value={item.n} /></table>
      </column></slate>`);
      const yes = piece(c, "t").querySelector("[data-slate-folded]") !== null;
      cleanup();
      vi.unstubAllGlobals();
      return yes;
    };
    expect(folded(368)).toBe(true);
    expect(folded(608)).toBe(false);
  });

  it("13. a count of a total, like 24/24, reads as a figure", () => {
    for (const word of ["24/24", "1,284/1,290"]) expect(isFigure(word), word).toBe(true);
    for (const word of ["a/b", "2026-10-25", "v2/api"]) expect(isFigure(word), word).toBe(false);
  });

  it("14. a true minus sign reads as a figure", () => {
    for (const word of ["−2%", "-2%", "−$4.20"]) expect(isFigure(word), word).toBe(true);
  });

  it("15. bytes under 10 of a unit keep one decimal", () => {
    const { c } = draw(`<slate><row><number id="b" label="Bundle" value={1268000} format="bytes" /><number id="k" label="Chunk" value={130000} format="bytes" /></row></slate>`);
    expect(piece(c, "b").textContent).toContain("1.2 MB");
    expect(piece(c, "k").textContent).toContain("127 KB");
  });

  it("16. a strip is a card of its own, and the facts after it take theirs", () => {
    const { c } = draw(`<slate><column>
      <row id="strip"><ring label="Up" value={209} max={214} format="fraction" /><number label="Today" value={3} /></row>
      <facts id="after"><fact label="a" value="b" /></facts>
    </column></slate>`);
    expect(piece(c, "strip").closest("[data-slate-card]")).toBeNull();
    expect(piece(c, "after").closest("[data-slate-card]")).not.toBeNull();
  });
});
