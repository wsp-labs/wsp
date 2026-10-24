import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseSlate, sketchSlate, slateStartValues, SLATE_GLANCE, type SlateDoc, type SlateJson } from "../../src/slate/index.js";

// The GPUI coordinator's slate the owner shot on 2026-10-25 (#2439), as its agent wrote it and with every value it held,
// and the same values written the visual-first way.
const read = (name: string): string => readFileSync(new URL(`./glance/${name}`, import.meta.url), "utf8");
const data = JSON.parse(read("values.json")) as { now: number; values: Record<string, SlateJson> };

function sketched(name: string): { sketch: string; codes: string[]; doc: SlateDoc } {
  const parsed = parseSlate(read(name));
  expect(parsed.errors ?? []).toEqual([]);
  const doc = parsed.document!;
  const sketch = sketchSlate(doc, { ...slateStartValues(doc), ...data.values }, { version: 25, now: data.now });
  const codes = [...sketch.matchAll(/^ {2}(W03\d) /gm)].map(m => m[1]!);
  return { sketch, codes, doc };
}

const GLANCE = ["W030", "W031", "W032", "W033", "W034", "W035", "W036", "W037", "W038"];

describe("a slate read at a glance", () => {
  const before = sketched("before.slate");
  const after = sketched("after.slate");

  it.each(GLANCE)("%s fires on the coordinator's slate as the owner saw it", code => {
    expect(before.codes).toContain(code);
  });

  it("is silent on the same values written visual-first", () => {
    expect(after.codes).toEqual([]);
    expect(after.sketch.split("\n")[0]).toMatch(/, 0 problems$/);
  });

  it("names what it read: seven open sections, two tables, the log line, the drawn table and the green column", () => {
    const lines = before.sketch.split("\n");
    const said = (code: string) => lines.filter(l => l.startsWith(`  ${code} `)).join("\n");
    expect(said("W030")).toContain("7 sections are open (where, asks, choices, screens, helpers, gate, steps)");
    expect(said("W031")).toContain("2 tables are open (screens-table, helpers-table)");
    expect(said("W034")).toContain("[exited with code 0]");
    expect(said("W035")).toContain("gate-table");
    expect(said("W032")).toContain("8 columns");
    expect(said("W036")).toContain("State is good on all 14 rows of screens-table");
    expect(said("W033")).toContain("asks-list");
    expect(said("W038")).toContain("section gate");
  });

  it("holds the words a glance takes in: the before's sketch runs past the budget and the after's well under it", () => {
    const words = (s: string) => Number(/the sketch runs to (\d+) words/.exec(s)?.[1] ?? 0);
    expect(words(before.sketch)).toBeGreaterThan(SLATE_GLANCE.sketchWords);
    expect(after.sketch).not.toContain("the sketch runs to");
  });

  // Written by a fresh agent given only the catalog's index and a situation (subagent aefc8ebcebf041c86, Opus 5.5),
  // the coordinator's once more after slate_write refused a + between texts.
  it.each(["agent-coordinator.slate", "agent-single.slate"])("%s, written from the catalog alone, passes with no warning", name => {
    const parsed = parseSlate(read(name));
    expect(parsed.errors).toEqual([]);
    const sketch = sketchSlate(parsed.document!, slateStartValues(parsed.document!), { version: 1, now: data.now });
    expect(sketch.split("\n")[0]).toMatch(/, 0 problems$/);
  });

  it("prints both counts for the report", () => {
    const count = (codes: string[]) => GLANCE.map(c => `${c} ${codes.filter(x => x === c).length}`).join(", ");
    console.log(`before: ${count(before.codes)}\nafter: ${count(after.codes)}`);
  });
});
