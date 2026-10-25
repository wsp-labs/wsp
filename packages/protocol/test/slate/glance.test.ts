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

// The cold test's task 3 (#2439): a monorepo test run that drew raw run output and the same status on every row.
describe("raw output and a same-status column", () => {
  const vitest = readFileSync(new URL("./output/vitest.txt", import.meta.url), "utf8");
  const codes = (text: string, values: Record<string, SlateJson> = {}): string[] => {
    const parsed = parseSlate(text);
    expect(parsed.errors ?? []).toEqual([]);
    const sketch = sketchSlate(parsed.document!, { ...slateStartValues(parsed.document!), ...values }, { version: 1, now: data.now });
    return [...sketch.matchAll(/^ {2}(W03\d) /gm)].map(m => m[1]!);
  };
  const ran = { tests: { state: "failed", exit: 1, out: vitest, runs: 1 } };

  it("W034 stays silent on an <output>, which leads with the summary and folds the raw lines", () => {
    expect(codes(`<slate><run name="tests" cmd="pnpm test" /><column><output run={$tests} /></column></slate>`, ran)).not.toContain("W034");
  });

  it("W034 fires on a run's raw output inside a folded section, and on a log pasted into a value", () => {
    expect(codes(`<slate><run name="tests" cmd="pnpm test" /><column><text>Tests ran</text><section title="Log" collapsible open={false}><text mono>{$tests.out}</text></section></column></slate>`, ran)).toContain("W034");
    expect(codes(`<slate><value name="log" start="" /><column><text mono>{$log}</text></column></slate>`, { log: vitest })).toContain("W034");
    expect(codes(`<slate><value name="note" start="" /><column><text>{$note}</text></column></slate>`, { note: "Rounding fails on refunds; the fix is in review." })).not.toContain("W034");
  });

  it("W036 fires on a status column whose rows all say passed, whatever their counts and marks", () => {
    const pkgs = [{ n: "api", r: "✓ passed (42)" }, { n: "web", r: "passed (311)" }, { n: "cli", r: "Passed in 3s" }];
    const table = `<slate><value name="pkgs" start={[]} /><column><table items={$pkgs}><col title="Package" value={item.n} /><col title="Result" value={item.r} /></table></column></slate>`;
    expect(codes(table, { pkgs })).toContain("W036");
    expect(codes(table, { pkgs: [...pkgs.slice(0, 2), { n: "cli", r: "✗ failed (2)" }] })).not.toContain("W036");
    // A column of durations differs row by row and names no state.
    expect(codes(table, { pkgs: [{ n: "api", r: "1.2s" }, { n: "web", r: "3.4s" }, { n: "cli", r: "0.8s" }] })).not.toContain("W036");
  });

  it("W036 fires on facts that all say the same state, and on a checklist the person does not tick with every row done", () => {
    expect(codes(`<slate><column><facts><fact label="Lint" value="passed" /><fact label="Unit" value="passed" /><fact label="E2E" value="Passed" /></facts></column></slate>`)).toContain("W036");
    const list = (editable: string) => `<slate><value name="jobs" start={[{ t: "Lint", d: true }, { t: "Unit", d: true }, { t: "E2E", d: true }]} /><column><checklist items={$jobs} title={item.t} done={item.d}${editable} /></column></slate>`;
    expect(codes(list(""))).toContain("W036");
    expect(codes(list(" editable"))).not.toContain("W036");
  });
});
