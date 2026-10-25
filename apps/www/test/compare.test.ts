// SPDX-License-Identifier: AGPL-3.0-only
// What /compare holds, read from the data its pages render: every answer short enough for a table cell, every answer
// sourced, and every tool with its own page.
import { describe, expect, it } from "vitest";
import { AHEAD, aheadOf, KINDS, ROWS, saidOf, STEPS, strongerOf, TOOLS, WSP, type Answers, type Point, type Tool } from "../src/compare";
import { fileOf, ROUTES, routeAt } from "../src/routes";

const words = (text: string): number => text.trim().split(/\s+/).length;
const pointsOf = (t: Tool): [string, Point[]][] => [["stronger", t.stronger], ["weaker", t.weaker], ["pickThem", t.pickThem], ["pickWsp", t.pickWsp]];
const everyAnswers: [string, Answers][] = [["wsp", WSP], ...TOOLS.map((t): [string, Answers] => [t.name, t.cells])];

describe("every answer", () => {
  it("asks the sixteen questions in order, each with a short line, and says what wsp lacks in its own column", () => {
    expect(ROWS.map(r => r.label)).toEqual([
      "Runs agents on",
      "Another computer",
      "What that computer has when the agent starts",
      "Where your keys and sign-ins live",
      "Worktrees per thread",
      "Agents start other agents",
      "Cloud machines on demand",
      "Resource monitor",
      "A panel the agent builds",
      "Review and pull requests in the app",
      "Which agents",
      "Platforms",
      "Phone app",
      "Teams",
      "Price",
      "Source code",
    ]);
    for (const r of ROWS) expect(words(r.about), r.about).toBeLessThanOrEqual(12);
    expect([WSP.worktrees.short, WSP.cloud.short, WSP.phone.short, WSP.teams.short]).toEqual(["On a branch", "Sub-threads, your Boat account", "No app; a browser pairs", "No, one person"]);
    expect([WSP.phone.good, WSP.teams.good]).toEqual([false, false]);
  });

  it("answers each question in six words or fewer from a named page, a dash where it cannot, with no em dash and never AI", () => {
    for (const [name, cells] of everyAnswers)
      for (const r of ROWS) {
        const { short, says } = cells[r.key];
        expect(words(short), `${name}, ${r.label}: "${short}"`).toBeLessThanOrEqual(6);
        expect(short.trim(), `${name}, ${r.label}`).not.toBe("");
        expect(says[0].trim(), `${name}, ${r.label}`).not.toBe("");
      }
    for (const [name, cells] of everyAnswers)
      for (const r of ROWS) {
        const sources = cells[r.key].says.slice(1);
        expect(sources.length, `${name}, ${r.label}`).toBeGreaterThan(0);
        for (const url of sources) expect(url, `${name}, ${r.label}`).toMatch(/^https:\/\/[^\s]+$/);
      }
    for (const [name, cells] of everyAnswers)
      for (const r of ROWS) if (/not stated/i.test(cells[r.key].short)) expect(cells[r.key].good, `${name}, ${r.label}`).toBe(false);
    const copy = [...everyAnswers.flatMap(([, cells]) => ROWS.flatMap(r => [cells[r.key].short, cells[r.key].says[0]])), ...TOOLS.flatMap(t => [t.line, t.differs, ...pointsOf(t).flatMap(([, ps]) => ps.map(p => saidOf(t, p)[0]))]), ...AHEAD.flatMap(a => [a.title, a.wsp[0], ...Object.values(a.them).map(s => s![0])]), ...ROWS.map(r => r.about)];
    for (const line of copy) {
      expect(line, line).not.toContain(String.fromCharCode(0x2014));
      expect(line, line).not.toMatch(/\bAI\b/);
    }
  });

});

describe("switching", () => {
  it("takes three steps, each from its docs page, the computer marked optional, and no time promised", () => {
    expect(STEPS.map(s => s.title)).toEqual(["Install wsp", "Add a computer, if you have one", "Start your first thread"]);
    for (const { title, said } of STEPS) {
      expect(said.length, title).toBeGreaterThan(1);
      for (const url of said.slice(1)) expect(url, title).toMatch(/^https:\/\/github\.com\/wsp-labs\/wsp\/blob\/[0-9a-f]{40}\/apps\/docs-next\/content\/\S+$/);
      expect(said[0], title).not.toMatch(/\b\d+\s*(s|sec|seconds?|min|minutes?|hours?)\b/i);
      expect(said[0], title).not.toContain(String.fromCharCode(0x2014));
    }
  });
});

describe("every tool", () => {
  it("has its own page, titled wsp vs its name, with its own preview image, and is ahead of wsp on at least one thing", () => {
    for (const tool of TOOLS) {
      const route = routeAt(`/compare/${tool.slug}`);
      expect(route.path).toBe(`/compare/${tool.slug}`);
      expect(route.title).toBe(`wsp vs ${tool.name}`);
      expect(route.description).toBe(tool.differs);
      expect(route.image).toEqual({ path: `/og-compare-${tool.slug}.png`, line: `wsp vs ${tool.name}.`, alt: `wsp vs ${tool.name}.` });
      expect(route.index).toBe(true);
      expect(fileOf(route.path)).toBe(`compare/${tool.slug}/index.html`);
    }
    expect(ROUTES.filter(r => r.path.startsWith("/compare/"))).toHaveLength(TOOLS.length);
    for (const tool of TOOLS) expect(aheadOf(tool).length, tool.name).toBeGreaterThan(0);
    const slugs = new Set(TOOLS.map(t => t.slug));
    for (const a of AHEAD) for (const slug of Object.keys(a.them)) expect(slugs, `${a.title}: ${slug}`).toContain(slug);
  });

  it("names a page for wsp's side of AHEAD, and carries three to five points each way and two or three pick lines, each one sentence with a page, none denied by its row", () => {
    for (const a of AHEAD) {
      expect(a.wsp.length, a.title).toBeGreaterThan(1);
      for (const url of a.wsp.slice(1)) expect(url, a.title).toMatch(/^https:\/\/[^\s]+$/);
    }
    for (const tool of TOOLS) expect(Object.keys(tool).sort(), tool.name).toEqual(["cells", "differs", "line", "name", "pickThem", "pickWsp", "slug", "stronger", "weaker"]);
    for (const tool of TOOLS) {
      expect(strongerOf(tool).length, `${tool.name} stronger`).toBeGreaterThanOrEqual(3);
      expect(strongerOf(tool).length, `${tool.name} stronger`).toBeLessThanOrEqual(5);
      expect(tool.weaker.length, `${tool.name} weaker`).toBeGreaterThanOrEqual(3);
      expect(tool.weaker.length, `${tool.name} weaker`).toBeLessThanOrEqual(5);
      for (const picks of [tool.pickThem, tool.pickWsp]) {
        expect(picks.length, tool.name).toBeGreaterThanOrEqual(2);
        expect(picks.length, tool.name).toBeLessThanOrEqual(3);
      }
    }
    for (const tool of TOOLS)
      for (const [side, points] of pointsOf(tool))
        for (const point of points) {
          const [text, ...pages] = saidOf(tool, point);
          expect(text, `${tool.name}, ${side}`).toMatch(/^[A-Zw][^]*\.$/);
          expect(text, `${tool.name}, ${side}`).not.toMatch(/\.\s+[A-Z]/);
          expect(pages.length, `${tool.name}, ${side}: ${text}`).toBeGreaterThan(0);
          for (const url of pages) expect(url, `${tool.name}, ${side}: ${text}`).toMatch(/^https:\/\/[^\s]+$/);
        }
    for (const tool of TOOLS)
      for (const [side, points] of pointsOf(tool))
        for (const point of points) {
          if (!("row" in point)) continue;
          const wspAhead = side === "weaker" || side === "pickWsp";
          expect([WSP[point.row].good, tool.cells[point.row].good], `${tool.name}, ${side}, ${point.row}: ${point.text}`).toEqual([wspAhead, !wspAhead]);
        }
    for (const a of AHEAD) {
      if (a.row === undefined) continue;
      expect(WSP[a.row].good, a.title).toBe(false);
      for (const tool of TOOLS.filter(t => a.them[t.slug] !== undefined)) expect(tool.cells[a.row].good, `${a.title}: ${tool.name}`).toBe(true);
    }
  });

});
