// SPDX-License-Identifier: AGPL-3.0-only
// The live band under each of the person's own computers on Settings > Computers, measured in a real Chromium at
// 1440 and 390 in both themes against the same list drawn with no band: no first-line cell moves, each owned row
// grows by the band alone and a cloud row not at all, and the band keeps the locked spacing (12 px under the name,
// 10 px between its lanes, 16 px to the row's edge, each figure level with its lane). Runs only when asked for
// (WSP_RENDER=1) and skips without Playwright's Chromium.
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Browser, Page } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { launchRender, renderSkipped, stopRender } from "./render-browser";
import { startVite, type ViteChild } from "./vite-child";

const WEB_DIR = resolve(dirname(fileURLToPath(import.meta.url)), "..");

if (renderSkipped !== undefined) console.info(`computer band render test skipped: ${renderSkipped}`);

/** Each row's box and the box of every first-line cell it draws, by the cell's name. */
interface Measured {
  rows: Record<string, { top: number; height: number; cells: Record<string, { x: number; y: number }>; band: { top: number; bottom: number; outer: number } | null; name: number }>;
}

describe.skipIf(renderSkipped !== undefined)("the band under each computer on the Computers list", () => {
  let vite: ViteChild | undefined;
  let browser: Browser | undefined;
  let page: Page | undefined;
  let base = "";

  beforeAll(async () => {
    vite = await startVite(WEB_DIR, "/test/wireframe/index.html");
    base = `${vite.base}/test/wireframe/index.html`;
    browser = await launchRender();
    page = await browser.newPage({ viewport: { width: 1440, height: 1200 } });
  }, 60_000);
  afterAll(() => stopRender(browser, vite?.child));

  const measure = (): Promise<Measured> =>
    page!.evaluate(() => {
      const list = document.querySelector<HTMLElement>("[data-settings-page]")!.getBoundingClientRect();
      const at = (el: Element | null): { x: number; y: number } | null => {
        if (el === null || getComputedStyle(el).display === "none") return null;
        const b = el.getBoundingClientRect();
        return { x: Math.round((b.left - list.left) * 10) / 10, y: Math.round((b.top - list.top) * 10) / 10 };
      };
      const rows: Measured["rows"] = {};
      for (const row of document.querySelectorAll<HTMLElement>("[data-settings-page] [data-grid-row][data-place-row]")) {
        const cells: Record<string, { x: number; y: number }> = {};
        const pick: Record<string, Element | null> = {
          name: row.querySelector("[data-grid-name]"),
          version: row.querySelector("[data-grid-fact]"),
          cores: row.querySelector("[data-k=cores]"),
          memory: row.querySelector("[data-k=memory]"),
          machines: row.querySelector("[data-k=machines]"),
          threads: row.querySelector("[data-k=threads]"),
          state: row.querySelector("[data-state-cell]"),
          mark: row.querySelector("[data-state-cell] [data-state-mark]"),
          chevron: row.querySelector("svg.lucide-chevron-right"),
        };
        for (const [k, el] of Object.entries(pick)) {
          const got = at(el);
          if (got !== null) cells[k] = got;
        }
        const box = row.getBoundingClientRect();
        const bandEl = row.querySelector<HTMLElement>("[data-live-band]");
        const cs = bandEl === null ? null : getComputedStyle(bandEl);
        const b = bandEl?.getBoundingClientRect();
        const nameEl = row.querySelector("[data-grid-fact]") ?? row.querySelector("[data-grid-name]");
        rows[row.dataset["placeRow"]!] = {
          top: box.top - list.top,
          height: box.height,
          cells,
          band: b === undefined || cs === null ? null : { top: b.top - box.top, bottom: box.bottom - b.bottom, outer: b.height + parseFloat(cs.marginTop) + parseFloat(cs.marginBottom) },
          name: nameEl!.getBoundingClientRect().bottom - box.top,
        };
      }
      return { rows };
    });

  const open = async (theme: "dark" | "light", bands: boolean): Promise<Measured> => {
    await page!.emulateMedia({ colorScheme: theme });
    await page!.goto(`${base}?screen=settings-computers-live&theme=${theme}${bands ? "" : "&bands=0"}`);
    await page!.waitForSelector("[data-settings-page] [data-place-row='solari']");
    if (bands) await page!.waitForFunction(() => document.querySelectorAll("[data-settings-page] [data-live-band]").length === 3);
    return measure();
  };

  for (const width of [1440, 390]) {
    it.each(["dark", "light"] as const)(`at ${width} in the %s theme no first-line cell moves, each owned row grows by its band alone and a cloud row not at all`, async theme => {
      await page!.setViewportSize({ width, height: 1200 });
      const without = await open(theme, false);
      const withBand = await open(theme, true);
      expect(Object.keys(withBand.rows)).toEqual(["here", "p_hetzner", "p_lab", "solari"]);
      // The cells measured: every one the first line draws at this width, the state mark where a row draws one.
      const wide = width === 1440 ? ["cores", "memory"] : [];
      expect(Object.keys(withBand.rows["here"]!.cells).sort()).toEqual(["chevron", "name", "state", "threads", "version", ...wide].sort());
      expect(Object.keys(withBand.rows["p_lab"]!.cells)).toContain("mark");
      let grown = 0;
      for (const [id, row] of Object.entries(withBand.rows)) {
        const was = without.rows[id]!;
        expect(was.band).toBeNull();
        // Every cell of the first line where it stood, against the list, to a tenth of a pixel: the rows above have
        // grown, so each row is read against its own top.
        const shift = row.top - was.top;
        expect(shift).toBeCloseTo(grown, 1);
        expect(Object.keys(row.cells).sort()).toEqual(Object.keys(was.cells).sort());
        for (const [k, cell] of Object.entries(row.cells)) expect({ k, x: cell.x, y: Math.round((cell.y - shift) * 10) / 10 }).toEqual({ k, ...was.cells[k] });
        if (id === "solari") {
          expect(row.band).toBeNull();
          expect(row.height).toBe(was.height);
        } else {
          expect(row.band).not.toBeNull();
          expect(row.height - was.height).toBeCloseTo(row.band!.outer, 1);
        }
        grown += row.height - was.height;
      }
    }, 60_000);

    it.each(["dark", "light"] as const)(`at ${width} in the %s theme the band stands 12 px under the name, its lanes 10 px apart, 16 px from the row's edge, each figure level with its lane`, async theme => {
      await page!.setViewportSize({ width, height: 1200 });
      await open(theme, true);
      const got = await page!.evaluate(() =>
        [...document.querySelectorAll<HTMLElement>("[data-settings-page] [data-live-band]")].map(band => {
          const row = band.closest<HTMLElement>("[data-grid-row]")!;
          const box = row.getBoundingClientRect();
          const above = (row.querySelector("[data-grid-fact]") ?? row.querySelector("[data-grid-name]"))!.getBoundingClientRect();
          const name = row.querySelector("[data-grid-name]")!.getBoundingClientRect();
          const [cpu, mem] = [...band.querySelectorAll("svg[role=img]")].map(svg => svg.getBoundingClientRect());
          const [cpuFig, memFig] = ["cpu", "mem"].map(k => band.querySelector(`[data-k=${k}]`)!.getBoundingClientRect());
          const mid = (b: DOMRect): number => b.top + b.height / 2;
          const chevron = row.querySelector("svg.lucide-chevron-right")?.getBoundingClientRect();
          return {
            id: row.dataset["placeRow"],
            underName: Math.round(cpu!.top - Math.max(above.bottom, name.bottom)),
            between: Math.round(mem!.top - cpu!.bottom),
            toEdge: Math.round(box.bottom - mem!.bottom),
            level: [Math.abs(mid(cpuFig!) - mid(cpu!)), Math.abs(mid(memFig!) - mid(mem!))].map(d => d <= 0.5),
            insideRow: cpu!.left >= box.left && memFig!.right <= (chevron?.right ?? box.right) + 0.5,
          };
        }),
      );
      expect(got.map(g => g.id)).toEqual(["here", "p_hetzner", "p_lab"]);
      for (const band of got) expect(band).toEqual({ id: band.id, underName: 12, between: 10, toEdge: 16, level: [true, true], insideRow: true });
    }, 60_000);
  }
});
