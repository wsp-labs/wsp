// SPDX-License-Identifier: AGPL-3.0-only
// The pieces #2468 adds or changes, measured in Chromium at the panel's 400 and 640 px in graphite and paper: a stat
// row's labels and windows on one line and cut with their whole words on hover, its figures on one baseline, each ring
// beside its figure, and under 480 px two or four tiles two a row and three one a line; a facts line one fact a line
// under 480 px; a decision, a progress, a summary-first output, choices and a table drawn inside the panel. Each case
// is photographed into artifacts/render. Like the other render tests it runs only when asked for (WSP_RENDER=1).
import { mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Browser, Page } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { launchRender, renderSkipped, stopRender } from "./render-browser";
import { serveHarness, type Harness } from "./slate-render/serve";

if (renderSkipped !== undefined) console.info(`slate glance render test skipped: ${renderSkipped}`);

const SHOTS = join(resolve(dirname(fileURLToPath(import.meta.url)), ".."), "artifacts", "render");
const SIZES = [400, 640] as const;
const THEMES = ["graphite", "paper"] as const;
const MATRIX = SIZES.flatMap(w => THEMES.map(theme => ({ w, theme })));

interface Tile { top: number; label: { lines: number; cut: boolean; whole: boolean }; window: { lines: number; cut: boolean; whole: boolean } | null; base: number; ring: { gap: number; inside: boolean } | null }

describe.skipIf(renderSkipped !== undefined)("the glance pieces in Chromium", () => {
  let harness: Harness | undefined;
  let browser: Browser | undefined;
  const errors: string[] = [];

  beforeAll(async () => {
    mkdirSync(SHOTS, { recursive: true });
    harness = await serveHarness();
    browser = await launchRender();
  }, 120_000);
  afterAll(async () => {
    await stopRender(browser, undefined);
    harness?.stop();
  });

  const open = async (doc: string, w: number, theme: string): Promise<Page> => {
    const page = await browser!.newPage({ viewport: { width: w + 40, height: 900 } });
    page.on("console", m => { if (m.type() === "error") errors.push(`${doc} ${w} ${theme}: ${m.text()}`); });
    page.on("pageerror", e => errors.push(`${doc} ${w} ${theme}: ${String(e)}`));
    await page.goto(`${harness!.base}?w=${w}&doc=${doc}&theme=${theme}`);
    await page.waitForSelector("[data-slate]");
    await page.evaluate(() => document.fonts.ready);
    // A figure rolls to its value when it first draws; the shot waits for it to settle.
    await page.waitForTimeout(1200);
    return page;
  };
  const shoot = (page: Page, doc: string, w: number, theme: string) => page.locator("[data-panel]").screenshot({ path: join(SHOTS, `${doc}-${w}-${theme}.png`) });
  const overflow = (page: Page) => page.evaluate(() => { const p = document.querySelector<HTMLElement>("[data-panel]")!; return p.scrollWidth - p.clientWidth; });

  const tiles = (page: Page): Promise<Tile[]> =>
    page.evaluate(() => {
      const oneLine = (el: HTMLElement | null) => {
        if (el === null) return null;
        const line = parseFloat(getComputedStyle(el).lineHeight);
        return { lines: Math.round(el.getBoundingClientRect().height / line), cut: el.scrollWidth > el.clientWidth + 1, whole: el.title === el.textContent };
      };
      return [...document.querySelectorAll<HTMLElement>("[data-slate-tile]")].map(tile => {
        const figure = tile.querySelector<HTMLElement>("[data-slate-figure]")!;
        // The figure's own glyphs: a range over its text, whose box bottom sits at its baseline's descent for one font.
        const range = document.createRange();
        range.selectNodeContents(figure);
        const text = [...range.getClientRects()].filter(r => r.width > 0);
        const glyphs = text.reduce((a, r) => ({ left: Math.min(a.left, r.left), right: Math.max(a.right, r.right), top: Math.min(a.top, r.top), bottom: Math.max(a.bottom, r.bottom) }), { left: Infinity, right: -Infinity, top: Infinity, bottom: -Infinity });
        const ring = tile.querySelector<SVGElement>("[data-slate-ring]");
        const r = ring?.getBoundingClientRect();
        // The figure's baseline: an empty inline-block set on it sits with its bottom on the baseline.
        const probe = document.createElement("span");
        probe.style.display = "inline-block";
        probe.style.height = "0";
        figure.appendChild(probe);
        const base = probe.getBoundingClientRect().bottom;
        probe.remove();
        return {
          top: Math.round(tile.getBoundingClientRect().top),
          label: oneLine(tile.querySelector<HTMLElement>("[data-slate-label]"))!,
          window: oneLine(tile.querySelector<HTMLElement>("[data-slate-window]")),
          base,
          ring: r === undefined ? null : { gap: r.left - glyphs.right, inside: r.top + r.height / 2 >= glyphs.top && r.top + r.height / 2 <= glyphs.bottom },
        };
      });
    });
  const rows = (all: Tile[]): Tile[][] => {
    const out: Tile[][] = [];
    for (const t of all) {
      const row = out.find(r => Math.abs(r[0]!.top - t.top) <= 2);
      if (row !== undefined) row.push(t);
      else out.push([t]);
    }
    return out;
  };

  describe.each([
    { doc: "glance-stats-2", narrow: [2] },
    { doc: "glance-stats-3", narrow: [1, 1, 1] },
    { doc: "glance-stats-4", narrow: [2, 2] },
    { doc: "glance-stats-3-short", narrow: [1, 1, 1], wide: [3] },
  ])("the stat row $doc", ({ doc, narrow, wide }) => {
    it.each(MATRIX)("at $w px in $theme: labels and windows on one line, whole on hover; one baseline a row; every ring beside its figure", async ({ w, theme }) => {
      const page = await open(doc, w, theme);
      const all = await tiles(page);
      await shoot(page, doc, w, theme);
      expect(all.length).toBe(narrow.reduce((a, b) => a + b, 0));
      for (const t of all) {
        expect(t.label.lines, "label lines").toBe(1);
        if (t.label.cut) expect(t.label.whole, "a cut label says itself whole on hover").toBe(true);
        if (t.window !== null) {
          expect(t.window.lines, "window lines").toBe(1);
          if (t.window.cut) expect(t.window.whole, "a cut window says itself whole on hover").toBe(true);
        }
        if (t.ring !== null) {
          expect(t.ring.gap, "ring right of its figure").toBeGreaterThanOrEqual(0);
          expect(t.ring.gap, "ring beside its figure").toBeLessThanOrEqual(16);
          expect(t.ring.inside, "ring level with its figure").toBe(true);
        }
      }
      const laid = rows(all);
      for (const row of laid) {
        const bases = row.map(t => t.base);
        expect(Math.max(...bases) - Math.min(...bases), "figures on one baseline").toBeLessThanOrEqual(0.5);
      }
      if (w < 480) expect(laid.map(r => r.length)).toEqual(narrow);
      else if (wide !== undefined) expect(laid.map(r => r.length)).toEqual(wide);
      expect(await overflow(page)).toBeLessThanOrEqual(0);
      await page.close();
    });
  });

  it.each(MATRIX)("a facts line runs one fact a line under 480 px and shares lines at 640, at $w px in $theme", async ({ w, theme }) => {
    const page = await open("glance-facts", w, theme);
    const tops = await page.evaluate(() => new Set([...document.querySelectorAll<HTMLElement>("[data-slate-facts] [data-settings-label]")].map(l => Math.round(l.getBoundingClientRect().top))).size);
    await shoot(page, "glance-facts", w, theme);
    if (w < 480) expect(tops).toBe(4);
    else expect(tops).toBeLessThan(4);
    expect(await overflow(page)).toBeLessThanOrEqual(0);
    await page.close();
  });

  it.each(MATRIX)("a decision, a progress, an output, choices and a table draw inside the panel at $w px in $theme", async ({ w, theme }) => {
    for (const doc of ["glance-decision", "glance-progress", "glance-output", "glance-choices", "glance-table"]) {
      const page = await open(doc, w, theme);
      await shoot(page, doc, w, theme);
      const read = await page.evaluate(() => ({
        answers: document.querySelectorAll("[data-slate-answer]").length,
        notes: [...document.querySelectorAll("[data-slate-answer-note]")].map(n => n.textContent),
        progress: document.querySelector("[data-slate-progress]")?.getAttribute("data-slate-progress") ?? null,
        exceptions: document.querySelectorAll("[data-slate-exception]").length,
        counts: [...document.querySelectorAll("[data-slate-count]")].map(n => n.textContent),
        log: document.querySelector("[role=log]") !== null,
        fold: document.querySelector("[data-slate-fold]")?.textContent ?? null,
        options: document.querySelectorAll("[data-slate-piece=choices] button").length,
        // The header is the card's first row, inside its border (#2439).
        head: document.querySelector("[data-slate-head]")?.parentElement?.className.includes("bg-card") === true,
      }));
      if (doc === "glance-decision") {
        expect(read.answers).toBe(3);
        expect(read.notes).toEqual(["Wait for morning: Support is awake then"]);
      }
      if (doc === "glance-progress") expect([read.progress, read.exceptions]).toEqual(["4/6", 2]);
      if (doc === "glance-output") expect([read.counts, read.log, read.fold]).toEqual([["1 failed", "1 passed", "1 skipped"], false, "All 12 lines"]);
      if (doc === "glance-choices") expect(read.options).toBe(3);
      if (doc === "glance-table" && w >= 480) expect(read.head).toBe(true);
      expect(await overflow(page), doc).toBeLessThanOrEqual(0);
      await page.close();
    }
  });

  it("logged no console error on any page", () => {
    expect(errors).toEqual([]);
  });
});
