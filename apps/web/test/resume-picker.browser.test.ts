// SPDX-License-Identifier: AGPL-3.0-only
// The resume picker in a real Chromium, where its dialog closes on an animation and hands the focus back as it goes,
// which jsdom does not do. With nothing selected the palette's Resume a conversation stands on the project the New
// thread page names; base-ui draws the row Enter takes, the first at rest and the second after one down press, and
// keeps it when the pointer rests off the rows; Enter opens the draft with the focus in its composer, opened off the
// palette or off the project's menu, whose trigger the closing dialogs would hand the focus back to. Like the other
// render tests it runs only when asked for (WSP_RENDER=1) and skips without Playwright's Chromium.
import type { Browser, Page } from "playwright";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { launchRender, renderSkipped, stopRender } from "./render-browser";
import { startVite, type ViteChild } from "./vite-child";

const WEB_DIR = resolve(dirname(fileURLToPath(import.meta.url)), "..");

if (renderSkipped !== undefined) console.info(`resume picker render test skipped: ${renderSkipped}`);

describe.skipIf(renderSkipped !== undefined)("the resume picker in Chromium", () => {
  let vite: ViteChild | undefined;
  let browser: Browser | undefined;
  let page: Page;

  beforeAll(async () => {
    vite = await startVite(WEB_DIR, "/test/shell/index.html");
    browser = await launchRender();
  }, 60_000);

  afterAll(async () => {
    await stopRender(browser, vite?.child);
  });

  beforeEach(async () => {
    page = await browser!.newPage({ viewport: { width: 1440, height: 900 } });
    await page.goto(`${vite!.base}/test/shell/index.html?conversations=1`);
    await page.locator("[data-k=project-home]").waitFor({ timeout: 50_000 });
  }, 60_000);

  const drawn = (): Promise<(string | null)[]> => page.$$eval("[data-resume-row][data-highlighted]", rows => rows.map(r => r.getAttribute("data-resume-row")));
  /** Opens the picker off the palette as a person does, the focus left on the sidebar first as a menu leaves it. */
  async function openPicker(): Promise<void> {
    await page.locator("[data-k=project-switcher]").first().focus();
    await page.keyboard.press("ControlOrMeta+K");
    await page.locator("[data-command-palette] input[role=combobox]").first().fill("resume");
    await page.locator("[data-command-palette] [data-slot=command-item][data-highlighted]", { hasText: "Resume a conversation" }).waitFor();
    await page.keyboard.press("Enter");
    await page.locator("[data-resume-row]").first().waitFor();
  }

  it("opens off the palette with nothing selected, on the project the page names, its first row drawn at rest", async () => {
    await openPicker();
    await expect.poll(drawn).toEqual(["c1"]);
    expect(await page.locator("[data-resume-scope=project]").textContent()).toBe("wsp");
    await page.close();
  }, 60_000);

  it("moves the drawn row to the second on the first down press, and Enter opens that one as a draft with the focus in its composer", async () => {
    await openPicker();
    await expect.poll(drawn).toEqual(["c1"]);
    await page.keyboard.press("ArrowDown");
    await expect.poll(drawn).toEqual(["c2"]);
    await page.keyboard.press("Enter");
    await page.locator("[data-k=resume-draft]").waitFor();
    expect(await page.locator("[data-resume-banner]").textContent()).toContain("WSP website rework");
    await page.waitForTimeout(600);
    expect(await page.evaluate(() => document.querySelector("[data-k=resume-draft] [data-chat-composer]")?.contains(document.activeElement) ?? false)).toBe(true);
    await page.close();
  }, 60_000);

  it("opened off the project's menu, opens the draft with the focus in its composer, not back on the menu's trigger", async () => {
    await page.locator("[data-k=project-switcher]").first().click();
    await page.locator("[data-switcher-option=pr_wsp]").click();
    await page.locator("[data-k=project-switcher]").first().click({ button: "right" });
    await page.locator("[data-menu-item=resume-conversation]").click();
    await expect.poll(drawn).toEqual(["c1"]);
    await page.keyboard.press("Enter");
    await page.locator("[data-k=resume-draft]").waitFor();
    await page.waitForTimeout(600);
    expect(await page.evaluate(() => document.querySelector("[data-k=resume-draft] [data-chat-composer]")?.contains(document.activeElement) ?? false)).toBe(true);
    await page.close();
  }, 60_000);

  it("hands the focus back to the search after a scope pick, so the arrows and Enter drive the list", async () => {
    await openPicker();
    await expect.poll(drawn).toEqual(["c1"]);
    await page.locator("[data-resume-scope=project]").click();
    await page.getByRole("menuitemradio", { name: "All projects" }).click();
    await expect.poll(() => page.evaluate(() => document.activeElement?.closest("[data-resume-picker]") !== null && document.activeElement?.tagName === "INPUT")).toBe(true);
    await page.keyboard.press("ArrowDown");
    await expect.poll(drawn).toEqual(["c2"]);
    await page.keyboard.press("Enter");
    await page.locator("[data-k=resume-draft]").waitFor();
    expect(await page.locator("[data-resume-banner]").textContent()).toContain("WSP website rework");
    await page.close();
  }, 60_000);

  it("keeps the drawn row with the pointer resting off the rows, and Enter takes that row", async () => {
    await openPicker();
    await expect.poll(drawn).toEqual(["c1"]);
    const label = await page.locator("[data-resume-picker] [data-slot=command-group-label]").boundingBox();
    await page.mouse.move(label!.x + label!.width / 2, label!.y + label!.height / 2);
    await page.waitForTimeout(200);
    await expect.poll(drawn).toEqual(["c1"]);
    await page.keyboard.press("Enter");
    await page.locator("[data-k=resume-draft]").waitFor();
    expect(await page.locator("[data-resume-banner]").textContent()).toContain("Resume picker plan");
    await page.close();
  }, 60_000);
});
