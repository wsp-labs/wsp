// SPDX-License-Identifier: AGPL-3.0-only
// What the two gallery rounds of #2439 found drawing 48 slates: each a rule the renderer now keeps.
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

describe("the gallery's findings", () => {
  it("draws a single line in the tone it names, and a small chart at its own height", () => {
    const { c } = draw(`<slate><value name="h" start={[{ t: 1, v: 3 }, { t: 2, v: 5 }]} /><column><chart id="ch" label="Errors" items={$h} x={item.t} value={item.v} tone="bad" height="small" /></column></slate>`);
    expect(piece(c, "ch").querySelector("figcaption [aria-hidden]")!.className).toContain("text-error-foreground");
    expect(piece(c, "ch").querySelector<HTMLElement>("[data-k=y-axis]")!.style.height).toBe("120px");
  });

  it("says a held button's reason under it and every answer's note under a decision", () => {
    const { c } = draw(`<slate><value name="pick" start="" /><column>
      <button id="go" label="Deploy" held="Checks are still running" onPress={send("Deploy")} />
      <decision id="d" label="Ship?" value={$pick} pick="now" onChange={send("Ship", $pick)}><option value="now" label="Now" note="Ready" /><option value="later" label="Later" note="Waits a day" /></decision>
    </column></slate>`);
    expect(piece(c, "go").querySelector("[data-slate-held-why]")!.textContent).toBe("Checks are still running");
    expect([...piece(c, "d").querySelectorAll("[data-slate-answer-note]")].map(n => n.textContent)).toEqual(["Later: Waits a day"]);
  });

  it("pads a strip's number with a status once, as a strip cell, and keeps the strip a card of its own", () => {
    const { engine, c } = draw(`<slate><column>
      <row id="strip"><ring label="Up" value={209} max={214} format="fraction" /><number label="Today" value={3} /><status tone="warning">over</status></row>
      <facts id="after"><fact label="a" value="b" /></facts>
    </column></slate>`);
    expect(isStatCell(engine, "strip")).toBe(false);
    expect(piece(c, "strip").closest("[data-slate-card]")).toBeNull();
    expect(piece(c, "after").closest("[data-slate-card]")).not.toBeNull();
  });

  it("counts a ring's whole figure, 209/214 and its ring, when it decides how many cells stand across", () => {
    const { c } = draw(`<slate><column><row id="strip">
      <ring label="Up" value={209} max={214} format="fraction" /><ring label="Healthy" value={198} max={214} format="fraction" /><ring label="Ready" value={201} max={214} format="fraction" />
    </row></column></slate>`);
    // 209/214 is 7 characters of 15.6 px and a 30 px ring: three do not fit the 366 px card; two would leave a short row, so one a row.
    expect(piece(c, "strip").querySelector("[data-slate-strip]")!.getAttribute("data-across")).toBe("1 3");
  });

  it("reads a count of a total and a true minus as figures", () => {
    for (const word of ["24/24", "−2%", "-2%", "1,284/1,290", "$4.20"]) expect(isFigure(word), word).toBe(true);
    for (const word of ["a/b", "2026-10-25", "v2/api"]) expect(isFigure(word), word).toBe(false);
  });

  it("names a row by a mono first column in its own face and folds its header inside the card", () => {
    const { c } = draw(`<slate><value name="f" start={[{ at: "migrate.ts:88", what: "Drops rows", w: "Blocking" }]} /><value name="g" start={[{ a: "one", b: "two", c: "three", d: "four" }]} /><column>
      <table id="t" items={$f}><col title="Where" value={item.at} mono={true} /><col title="Finding" value={item.what} /><col title="Weight" value={item.w} /></table>
      <table id="folded" items={$g}><col title="A" value={item.a} /><col title="B" value={item.b} /><col title="C" value={item.c} /><col title="D" value={item.d} /></table>
    </column></slate>`);
    const cells = [...piece(c, "t").querySelectorAll("[role=row]:not([data-slate-head]) [role=cell]")];
    expect(cells[0]!.className).toContain("font-mono");
    expect(cells[1]!.className).toContain("text-muted-foreground");
    expect(cells[1]!.className).not.toContain("text-sm");
    const head = piece(c, "folded").querySelector("[data-slate-head]")!;
    expect(head.closest("[data-slate-folded] > div")).not.toBeNull();
  });

  it("folds a table whose machine columns leave its name less than its longest word, rather than break it", () => {
    const at = (width: number) => {
      vi.stubGlobal("ResizeObserver", class {
        constructor(private readonly seen: (entries: Array<{ contentRect: { width: number } }>) => void) {}
        observe() { this.seen([{ contentRect: { width } }]); }
        disconnect() {}
      });
      const { c } = draw(`<slate><value name="r" start={[{ t: "proration across a DST change", b: "v2026.10.25-rc3", g: "ap-south-1", f: 24, n: 240 }]} /><column>
        <table id="t" items={$r}><col title="Test" value={item.t} /><col title="Build" value={item.b} mono={true} /><col title="Region" value={item.g} mono={true} /><col title="Fails" value={item.f} /><col title="Runs" value={item.n} /></table>
      </column></slate>`);
      const folded = piece(c, "t").querySelector("[data-slate-folded]") !== null;
      cleanup();
      vi.unstubAllGlobals();
      return folded;
    };
    expect(at(368)).toBe(true);
    expect(at(608)).toBe(false);
  });
});
