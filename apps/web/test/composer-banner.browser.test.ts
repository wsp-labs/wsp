// SPDX-License-Identifier: AGPL-3.0-only
// A draft's banner over the composer in a real Chromium: the fork's banner on
// a thread's New thread page, in both themes, wide and phone-wide, with the
// drawer's row under it and without, and a resumed conversation's banner on a
// project's New thread page, each sit on the box's top edge as the tray sits on
// its foot. Runs only when asked for (WSP_RENDER=1) and skips without
// Playwright's Chromium.
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Browser, Page } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { launchRender, renderSkipped, stopRender } from "./render-browser";
import { startVite, type ViteChild } from "./vite-child";

const WEB_DIR = resolve(dirname(fileURLToPath(import.meta.url)), "..");

if (renderSkipped !== undefined) console.info(`composer banner render test skipped: ${renderSkipped}`);

/** From the foot of the banner's glass to the top of what stands under it, the drawer or else the box; 0 is flush. The
 * banner's own box runs on under them by the overlap it holds as padding below its row, where its glass is masked off. */
const seam = (page: Page, banner: string) =>
  page.evaluate(sel => {
    const el = document.querySelector<HTMLElement>(sel)!;
    const style = getComputedStyle(el);
    const overlap = parseFloat(style.paddingBottom) - parseFloat(style.paddingTop);
    const glassFoot = el.getBoundingClientRect().bottom - overlap;
    const composer = el.closest("[data-chat-composer]")!;
    const drawer = composer.querySelector("[data-composer-drawer]");
    const under = (drawer ?? composer.querySelector("[data-chat-composer-main-surface]")!).getBoundingClientRect();
    // The strip the banner runs on under them is masked glass: its dismiss and the drawer's row still take the pointer.
    const onTop = (target: Element | null): boolean => {
      if (target === null) return true;
      const r = target.getBoundingClientRect();
      return target.contains(document.elementFromPoint(r.left + r.width / 2, r.top + 2));
    };
    return { gap: under.top - glassFoot, drawer: drawer !== null, reachable: onTop(el.querySelector("button")) && onTop(drawer?.firstElementChild ?? null) };
  }, banner);

describe.skipIf(renderSkipped !== undefined)("a draft's banner on the composer in Chromium", () => {
  let vite: ViteChild | undefined;
  let browser: Browser | undefined;
  let base = "";

  beforeAll(async () => {
    vite = await startVite(WEB_DIR, "/test/shell/index.html");
    base = `${vite.base}/test/shell/index.html`;
    browser = await launchRender();
  }, 60_000);
  afterAll(() => stopRender(browser, vite?.child));

  const forked = async (theme: "dark" | "light", width: number, extra = ""): Promise<Page> => {
    const page = await browser!.newPage({ viewport: { width, height: 900 } });
    await page.addInitScript(() => window.localStorage.clear());
    await page.goto(`${base}?theme=${theme}&ws=ws_a&fork=1${extra}`);
    await page.waitForSelector("[data-fork-banner]");
    return page;
  };

  it.each([
    ["dark", 1440],
    ["light", 1440],
    ["dark", 390],
    ["light", 390],
  ] as const)("in the %s theme at %i px the fork's banner sits on the box", async (theme, width) => {
    const page = await forked(theme, width);
    const at = await seam(page, "[data-fork-banner]");
    expect(at.drawer).toBe(false);
    expect(Math.abs(at.gap)).toBeLessThanOrEqual(1);
    expect(at.reachable).toBe(true);
    await page.close();
  }, 60_000);

  it.each(["dark", "light"] as const)("in the %s theme the fork's banner sits on the box with the drawer's row between them", async theme => {
    const page = await forked(theme, 1440, "&queued=1");
    await page.waitForSelector("[data-composer-drawer]");
    const at = await seam(page, "[data-fork-banner]");
    expect(at.drawer).toBe(true);
    expect(Math.abs(at.gap)).toBeLessThanOrEqual(1);
    expect(at.reachable).toBe(true);
    await page.close();
  }, 60_000);

  it.each([
    ["dark", 1440],
    ["light", 1440],
    ["dark", 390],
    ["light", 390],
  ] as const)("in the %s theme at %i px a resumed conversation's banner sits on the box on the project's New thread page", async (theme, width) => {
    const page = await browser!.newPage({ viewport: { width, height: 900 } });
    await page.goto(`${base}?conversations=1&theme=${theme}`);
    await page.locator("[data-k=project-home]").waitFor({ timeout: 50_000 });
    await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
    await page.keyboard.press("ControlOrMeta+K");
    await page.locator("[data-command-palette] input[role=combobox]").first().fill("resume");
    await page.locator("[data-command-palette] [data-slot=command-item][data-highlighted]", { hasText: "Resume a conversation" }).waitFor();
    await page.keyboard.press("Enter");
    await page.locator("[data-resume-row]").first().waitFor();
    await page.keyboard.press("Enter");
    await page.waitForSelector("[data-resume-banner]");
    const at = await seam(page, "[data-resume-banner]");
    expect(Math.abs(at.gap)).toBeLessThanOrEqual(1);
    expect(at.reachable).toBe(true);
    await page.close();
  }, 60_000);
});
