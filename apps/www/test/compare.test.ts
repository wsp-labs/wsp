// SPDX-License-Identifier: AGPL-3.0-only
// What /compare holds, read from the data its pages render: every answer short enough for a table cell, every answer
// sourced, and every tool with its own page.
import { describe, expect, it } from "vitest";
import { AHEAD, aheadOf, ROWS, TOOLS, WSP, type Answers } from "../src/compare";
import { fileOf, ROUTES, routeAt } from "../src/routes";

const words = (text: string): number => text.trim().split(/\s+/).length;
const everyAnswers: [string, Answers][] = [["wsp", WSP], ...TOOLS.map((t): [string, Answers] => [t.name, t.cells])];

describe("every answer", () => {
  it("asks the sixteen questions, in order, each with one short line under it", () => {
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
  });

  it("says what wsp lacks in its own column: a worktree only on a branch, the cloud only for sub-threads, no phone app, no teams", () => {
    expect([WSP.worktrees.short, WSP.cloud.short, WSP.phone.short, WSP.teams.short]).toEqual(["On a branch", "Sub-threads, your Boat account", "No app; a browser pairs", "No, one person"]);
    expect([WSP.phone.good, WSP.teams.good]).toEqual([false, false]);
  });

  it("answers each question in six words or fewer, beside the long sentence it sums", () => {
    for (const [name, cells] of everyAnswers)
      for (const r of ROWS) {
        const { short, says } = cells[r.key];
        expect(words(short), `${name}, ${r.label}: "${short}"`).toBeLessThanOrEqual(6);
        expect(short.trim(), `${name}, ${r.label}`).not.toBe("");
        expect(says[0].trim(), `${name}, ${r.label}`).not.toBe("");
      }
  });

  it("names a page for every answer", () => {
    for (const [name, cells] of everyAnswers)
      for (const r of ROWS) {
        const sources = cells[r.key].says.slice(1);
        expect(sources.length, `${name}, ${r.label}`).toBeGreaterThan(0);
        for (const url of sources) expect(url, `${name}, ${r.label}`).toMatch(/^https:\/\/[^\s]+$/);
      }
  });

  it("marks an answer it cannot give with a dash", () => {
    for (const [name, cells] of everyAnswers)
      for (const r of ROWS) if (/not stated/i.test(cells[r.key].short)) expect(cells[r.key].good, `${name}, ${r.label}`).toBe(false);
  });

  it("carries no em dash and never says AI", () => {
    const copy = [...everyAnswers.flatMap(([, cells]) => ROWS.flatMap(r => [cells[r.key].short, cells[r.key].says[0]])), ...TOOLS.flatMap(t => [t.line, t.differs, t.well, t.fit]), ...AHEAD.flatMap(a => [a.title, a.wsp[0], ...Object.values(a.them).map(s => s![0])]), ...ROWS.map(r => r.about)];
    for (const line of copy) {
      expect(line, line).not.toContain(String.fromCharCode(0x2014));
      expect(line, line).not.toMatch(/\bAI\b/);
    }
  });
});

describe("every tool", () => {
  it("has its own page, titled wsp vs its name, with its own preview image", () => {
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
  });

  it("names a page for wsp's side of every row of AHEAD", () => {
    for (const a of AHEAD) {
      expect(a.wsp.length, a.title).toBeGreaterThan(1);
      for (const url of a.wsp.slice(1)) expect(url, a.title).toMatch(/^https:\/\/[^\s]+$/);
    }
  });

  it("is ahead of wsp on at least one thing, and every row of AHEAD names real tools", () => {
    for (const tool of TOOLS) expect(aheadOf(tool).length, tool.name).toBeGreaterThan(0);
    const slugs = new Set(TOOLS.map(t => t.slug));
    for (const a of AHEAD) for (const slug of Object.keys(a.them)) expect(slugs, `${a.title}: ${slug}`).toContain(slug);
  });
});
