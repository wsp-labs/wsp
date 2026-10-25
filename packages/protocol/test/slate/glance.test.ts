import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseSlate, sketchSlate, slateGlanceProblems, slateStartValues, SLATE_GLANCE, SLATE_LIMITS, type SlateDoc, type SlateJson } from "../../src/slate/index.js";

// The GPUI coordinator's slate the owner shot on 2026-10-25, as its agent wrote it and with every value it held,
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

  it("counts each warning on the slate as the owner saw it, and none on its rewrite", () => {
    const count = (codes: string[]) => GLANCE.map(c => `${c} ${codes.filter(x => x === c).length}`).join(", ");
    expect(count(before.codes)).toBe("W030 1, W031 1, W032 1, W033 5, W034 2, W035 1, W036 1, W037 1, W038 2");
    expect(count(after.codes)).toBe(GLANCE.map(c => `${c} 0`).join(", "));
  });
});

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

  it.each(["✗ failed (2)", "✗ 2 failed", "FAILED", "ERROR", "1) Pick a plan", "Last error: the token expired"])(
    "W034 stays silent on a status a person reads: %s",
    status => {
      const table = `<slate><value name="r" start={[]} /><column><table items={$r}><col title="Package" value={item.n} /><col title="Result" value={item.v} /></table></column></slate>`;
      expect(codes(table, { r: [{ n: "api", v: "✓ passed (42)" }, { n: "web", v: "passed (311)" }, { n: "cli", v: status }] })).not.toContain("W034");
    },
  );

  it("W036 fires on a status column whose rows all say passed, whatever their counts and marks, and asks for a <progress>", () => {
    const pkgs = [{ n: "api", r: "✓ passed (42)" }, { n: "web", r: "passed (311)" }, { n: "cli", r: "Passed in 3s" }];
    const table = `<slate><value name="pkgs" start={[]} /><column><table items={$pkgs}><col title="Package" value={item.n} /><col title="Result" value={item.r} /></table></column></slate>`;
    expect(codes(table, { pkgs })).toContain("W036");
    const doc = parseSlate(table).document!;
    const line = sketchSlate(doc, { ...slateStartValues(doc), pkgs }, { version: 1, now: data.now }).split("\n").find(l => l.startsWith("  W036 "));
    expect(line).toContain("<progress>");
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

// The slate a fresh thread wrote with the new catalog for the cold test's task 3 (a monorepo's test run), and the runs
// the harness fed it.
describe("the cold test's task 3 slate, as the thread wrote it, with its runs", () => {
  const runs = JSON.parse(read("task3.runs.json")) as { suite: Record<string, SlateJson> & { json: { failures: { package: string; error: string }[] } } };
  const task3 = (values: Record<string, SlateJson>): string => {
    const parsed = parseSlate(read("task3.slate"));
    expect(parsed.errors ?? []).toEqual([]);
    return sketchSlate(parsed.document!, { ...slateStartValues(parsed.document!), ...values }, { version: 1, now: 1761384600000 });
  };
  const codesOf = (sketch: string): string[] => [...sketch.matchAll(/^ {2}(W03\d) /gm)].map(m => m[1]!);

  it("W034 and W036 fire on it", () => {
    const said = codesOf(task3(runs));
    expect(said).toContain("W034");
    expect(said).toContain("W036");
  });

  it("W034 fires on its error column whatever class the runner's message names", () => {
    const suite = structuredClone(runs.suite);
    suite.json.failures = suite.json.failures.filter(f => f.package === "billing");
    expect(suite.json.failures.every(f => f.error.startsWith("AssertionError:"))).toBe(true);
    expect(codesOf(task3({ suite }))).toContain("W034");
  });

  it("W034 fires on each of its error cells alone", () => {
    expect(runs.suite.json.failures).toHaveLength(11);
    const table = `<slate><value name="r" start={[]} /><column><table items={$r}><col title="Package" value={item.n} /><col title="Error" value={item.e} /></table></column></slate>`;
    const doc = parseSlate(table).document!;
    for (const f of runs.suite.json.failures) {
      const sketch = sketchSlate(doc, { ...slateStartValues(doc), r: [{ n: f.package, e: f.error }] }, { version: 1, now: 0 });
      expect(codesOf(sketch), f.error).toContain("W034");
    }
  });

  it("W036 on a column red on every row of a list of failures asks to drop the tone, not to swap the table for a <progress>", () => {
    const line = task3(runs).split("\n").find(l => l.startsWith("  W036 ") && l.includes("Error is bad"));
    expect(line).toBeDefined();
    expect(line).not.toContain("<progress>");
    expect(line).toContain("drop the tone");
  });
});

describe("the glance checks over a long table", () => {
  it("read only the rows the person sees", () => {
    let furthest = -1;
    const rows = Array.from({ length: SLATE_LIMITS.listItems }, (_, i) => ({ a: `row ${i}`, b: "passed", c: i, d: "Error: no", e: "x", f: "y", g: "z", h: "w" }));
    const counted = new Proxy(rows, { get(target, key, receiver) {
      if (typeof key === "string" && /^\d+$/.test(key)) furthest = Math.max(furthest, Number(key));
      return Reflect.get(target, key, receiver);
    } });
    const cols = ["a", "b", "c", "d", "e", "f", "g", "h"].map(k => `<col title="${k}" value={item.${k}} tone={item.c > 10 ? 'bad' : 'good'} />`).join("");
    const doc = parseSlate(`<slate><value name="rows" start={[]} /><column><table items={$rows}>${cols}</table></column></slate>`).document!;
    const glance = slateGlanceProblems(doc, { resolve: path => (path === "$rows" ? (counted as unknown as SlateJson) : undefined), now: data.now }, 0);
    expect(glance.map(p => p.code)).toContain("W036");
    expect(furthest).toBeLessThan(SLATE_LIMITS.rowsDrawn);
  });

  it("W036 on a table longer than it draws says how many rows it read and how many there are", () => {
    const rows = Array.from({ length: 5000 }, (_, i) => ({ n: `row ${i}`, v: i < SLATE_LIMITS.rowsDrawn ? "passed" : "failed" }));
    const table = `<slate><value name="r" start={[]} /><column><table items={$r}><col title="Package" value={item.n} /><col title="Result" value={item.v} /></table></column></slate>`;
    const doc = parseSlate(table).document!;
    const line = sketchSlate(doc, { ...slateStartValues(doc), r: rows }, { version: 1, now: 0 }).split("\n").find(l => l.startsWith("  W036 "));
    expect(line).toContain("on all 200 rows of ");
    expect(line).toContain("that draw, of 5000");
  });

  it("a checklist draws every row, so its done check reads to the first row not done", () => {
    const steps = Array.from({ length: 300 }, (_, i) => ({ t: `Step ${i}`, d: i < SLATE_LIMITS.rowsDrawn }));
    const list = `<slate><value name="s" start={[]} /><column><checklist id="todo" items={$s} key={item.t} title={item.t} done={item.d} /></column></slate>`;
    const doc = parseSlate(list).document!;
    const sketch = (s: SlateJson): string => sketchSlate(doc, { ...slateStartValues(doc), s }, { version: 1, now: 0 });
    expect(sketch(steps)).not.toContain("W036");
    expect(sketch(steps.map(s => ({ ...s, d: true })))).toContain("every row of todo is done, all 300");
  });
});

describe("W034 on a status a person writes", () => {
  const table = `<slate><value name="r" start={[]} /><column><table items={$r}><col title="Service" value={item.n} /><col title="Last check" value={item.v} /></table></column></slate>`;
  const codes = (v: string): string[] => {
    const doc = parseSlate(table).document!;
    const rows = [{ n: "api", v: "up" }, { n: "web", v: "up, slow" }, { n: "cli", v }];
    return [...sketchSlate(doc, { ...slateStartValues(doc), r: rows }, { version: 1, now: 0 }).matchAll(/^ {2}(W03\d) /gm)].map(m => m[1]!);
  };

  it.each(["Warning: disk 91% full", "✓ deployed in 300 ms", "✔ api healthy 12 ms"])("is silent on %s", status => {
    expect(codes(status)).not.toContain("W034");
  });

  it.each(["warning: unused variable: `x`", "   × checkout > rounds a refund 7ms", "src/cart.ts(2,14): error TS2322: Type 'string' is not assignable to type 'number'."])("still fires on %s", line => {
    expect(codes(line)).toContain("W034");
  });
});

describe("the glance checks over a long checklist", () => {
  /** A row that counts how often its title and its tick are read. */
  const counting = (done: (i: number) => boolean): { rows: SlateJson[]; titles: Set<number>; ticks: Set<number> } => {
    const titles = new Set<number>();
    const ticks = new Set<number>();
    const rows = Array.from({ length: SLATE_LIMITS.listItems }, (_, i) => ({ get t() { titles.add(i); return `Step ${i}`; }, get d() { ticks.add(i); return done(i); } }));
    return { rows: rows as unknown as SlateJson[], titles, ticks };
  };
  const glance = (type: "checklist" | "progress", rows: SlateJson[]): string[] => {
    const parsed = parseSlate(`<slate><value name="s" start={[]} /><column><${type} id="p" items={$s} key={item.t} ${type === "checklist" ? "title" : `label="steps" name`}={item.t} done={item.d} /></column></slate>`);
    expect(parsed.errors ?? []).toEqual([]);
    return slateGlanceProblems(parsed.document!, { resolve: path => (path === "$s" ? rows : undefined), now: 0 }, 0).map(p => p.code);
  };

  it.each(["checklist", "progress"] as const)("read at most SLATE_GLANCE.rowsRead rows of a %s's words", type => {
    const { rows, titles } = counting(i => i !== 3);
    glance(type, rows);
    expect(titles.size).toBeLessThanOrEqual(SLATE_GLANCE.rowsRead);
  });

  it("stop the done check at the first row not done: it reads rows 0 to 3 and no more", () => {
    const { rows, ticks } = counting(i => i !== 3);
    glance("checklist", rows);
    expect([...ticks].sort((a, b) => a - b)).toEqual([0, 1, 2, 3]);
  });

  it("still say every row is done when all 5000 are", () => {
    const { rows, ticks } = counting(() => true);
    expect(glance("checklist", rows)).toContain("W036");
    expect(ticks.size).toBe(SLATE_LIMITS.listItems);
  });
});
