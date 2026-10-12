// SPDX-License-Identifier: AGPL-3.0-only
// Settings > Activity in a real Chromium, since jsdom lays nothing out, on the
// wireframe page's computer: every row's glyph centred on its row within a
// pixel, one line or two, in the Threads, Processes and Kinds views; the
// computer picker, the kind and thread selects and the filter at the height of
// the Agents page's computer picker, measured on the same build; no chip row and
// no act button on a row, End in its menu; photographed in both themes at 1440
// and 390. Vite serves test/wireframe, so like the other render tests it runs
// only when asked for (WSP_RENDER=1) and skips without Playwright's Chromium.
import { mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Browser, Page } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { launchRender, renderSkipped, stopRender } from "./render-browser";
import { startVite, type ViteChild } from "./vite-child";

const WEB_DIR = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SHOTS_DIR = join(tmpdir(), "wsp-render");
const THEMES = ["dark", "light"] as const;
const WIDTHS = [1440, 390] as const;
const CONTROLS = ["activity-computer", "activity-kind", "activity-thread", "activity-filter"] as const;

if (renderSkipped !== undefined) console.info(`activity layout render test skipped: ${renderSkipped}`);

describe.skipIf(renderSkipped !== undefined)("Settings > Activity laid out in Chromium", () => {
  let vite: ViteChild | undefined;
  let browser: Browser | undefined;
  let page: Page | undefined;
  let base = "";

  beforeAll(async () => {
    vite = await startVite(WEB_DIR, "/test/wireframe/index.html");
    base = `${vite.base}/test/wireframe/index.html`;
    browser = await launchRender();
    mkdirSync(SHOTS_DIR, { recursive: true });
  }, 60_000);
  afterAll(() => stopRender(browser, vite?.child));

  const open = async (query: string, width: number, theme: string): Promise<Page> => {
    await page?.close();
    page = await browser!.newPage({ viewport: { width, height: 1400 }, colorScheme: theme === "light" ? "light" : "dark" });
    await page.addInitScript(() => window.localStorage.clear());
    await page.goto(`${base}?${query}&theme=${theme}`);
    return page;
  };
  const activity = async (width: number, theme: string, view?: string): Promise<Page> => {
    const p = await open(`screen=settings-activity${view === undefined ? "" : `&view=${view}`}`, width, theme);
    await p.waitForSelector("[data-proc-row], [data-group]");
    return p;
  };

  /** Each row's glyph centre against its row's, and how many lines of text the row's name cell holds. */
  const centres = (p: Page) =>
    p.evaluate(() =>
      [...document.querySelectorAll<HTMLElement>("[data-proc-row], [data-subagent-row], [data-container-row]")].flatMap(row => {
        const glyph = row.querySelector<Element>("svg, [data-harness-mark], img");
        if (glyph === null) return [];
        const r = row.getBoundingClientRect();
        const g = glyph.getBoundingClientRect();
        const lines = row.querySelector(":scope > span .flex-col") === null ? 1 : 2;
        return [{ row: row.dataset["procRow"] ?? row.dataset["subagentRow"] ?? row.dataset["containerRow"] ?? "", off: Math.abs(g.top + g.height / 2 - (r.top + r.height / 2)), lines, height: Math.round(r.height) }];
      }),
    );

  it("centres every row's glyph on its row within a pixel, the two-line rows of the Processes view among them", async () => {
    for (const width of WIDTHS) {
      for (const view of [undefined, "processes", "kinds"]) {
        const p = await activity(width, "dark", view);
        if (view === undefined) for (const id of ["th_design", "wsp", "loose"]) await p.click(`[data-group="${id}"]`);
        if (view === "kinds") for (const kind of ["agent", "tab", "terminal"]) await p.click(`[data-group="${kind}"]`);
        const read = await centres(p);
        expect(read.length, `${width} ${view ?? "threads"}`).toBeGreaterThan(5);
        if (view === "processes" && width === 1440) expect(read.some(r => r.lines === 2), "two-line rows").toBe(true);
        expect(read.filter(r => r.off > 1), `${width} ${view ?? "threads"}`).toEqual([]);
      }
    }
  });

  it("stands the computer picker, both selects and the filter at the Agents page's picker height, with no chip row", async () => {
    for (const width of WIDTHS) {
      const agents = await open("screen=settings-agents", width, "dark");
      await agents.waitForSelector("[data-k=agents-picker]");
      const picker = Math.round((await agents.locator("[data-k=agents-picker]").boundingBox())!.height * 10) / 10;
      const p = await activity(width, "dark");
      const heights = await p.evaluate(ids => ids.map(id => {
        const el = document.querySelector<HTMLElement>(`[data-k=${id}]`)!;
        // The filter's data-k sits on its input; its box is the field around it.
        const box = el.tagName === "INPUT" ? el.parentElement! : el;
        return Math.round(box.getBoundingClientRect().height * 10) / 10;
      }), [...CONTROLS]);
      expect(heights, `${width}`).toEqual(CONTROLS.map(() => picker));
      expect(await p.locator("[data-k=activity-chips], [data-chip]").count()).toBe(0);
    }
  });

  it("puts no act button on a row or a thread's head, End standing in a row's menu", async () => {
    const p = await activity(1440, "dark");
    for (const id of ["th_design", "wsp", "loose"]) await p.click(`[data-group="${id}"]`);
    expect(await p.locator("[data-act=end], [data-act=stop], [data-act=free], [data-act=end-all], [data-act=end-container]").count()).toBe(0);
    await p.locator("[data-proc-row='41877']").hover();
    await p.locator("[data-proc-row='41877'] [data-act=menu]").click();
    await p.getByRole("menuitem", { name: "End" }).waitFor();
    await p.keyboard.press("Escape");
    await p.locator("[data-proc-row=slate-heading] [data-act=menu]").click({ force: true });
    await p.getByRole("menuitem", { name: "End all" }).waitFor();
  });

  it("photographs the page in both themes at 1440 and 390, a thread opened onto its shell, tab and slate", async () => {
    for (const theme of THEMES) {
      for (const width of WIDTHS) {
        const p = await activity(width, theme);
        for (const id of ["th_design", "wsp"]) await p.click(`[data-group="${id}"]`);
        await p.screenshot({ path: join(SHOTS_DIR, `activity-${theme}-${width}.png`), fullPage: true });
      }
    }
  });
});
