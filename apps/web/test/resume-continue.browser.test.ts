// SPDX-License-Identifier: AGPL-3.0-only
// The first Continue press off a resumed conversation's draft in a real Chromium: a conversation picked off the resume
// picker by the pointer, a message typed into the composer the pick focused, and one press of Continue at once, which
// asks the host to open the thread on it exactly once and empties the box. Half the runs press while the draft's read
// of the earlier messages is still out, where #2156 looked for a press that does nothing. The host never answers
// the start here, so the page holds still for the count. Like the other render tests it runs only when asked for
// (WSP_RENDER=1) and skips without Playwright's Chromium.
import type { Browser, Page } from "playwright";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { TRANSCRIPT_LOADING } from "../src/transcript-words.js";
import { launchRender, renderSkipped, stopRender } from "./render-browser";
import { startVite, type ViteChild } from "./vite-child";

const WEB_DIR = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const MESSAGE = "Repeat your last answer in five words.";
const RUNS = Array.from({ length: 10 }, (_, i) => (i % 2 === 0 ? { run: i + 1, readMs: 0, when: "after the read" } : { run: i + 1, readMs: 5_000, when: "while the read is out" }));

if (renderSkipped !== undefined) console.info(`resume continue render test skipped: ${renderSkipped}`);

describe.skipIf(renderSkipped !== undefined)("the first Continue press off a resumed conversation in Chromium", () => {
  let vite: ViteChild | undefined;
  let browser: Browser | undefined;

  beforeAll(async () => {
    vite = await startVite(WEB_DIR, "/test/shell/index.html");
    browser = await launchRender();
  }, 60_000);

  afterAll(async () => {
    await stopRender(browser, vite?.child);
  });

  const resumes = (page: Page): Promise<string[]> => page.evaluate(() => (window as unknown as { __resumes?: string[] }).__resumes ?? []);

  it.each(RUNS)("run $run, $when: one press opens the thread", async ({ readMs }) => {
    const page = await browser!.newPage({ viewport: { width: 1440, height: 900 } });
    await page.goto(`${vite!.base}/test/shell/index.html?conversations=1&readMs=${readMs}`);
    await page.locator("[data-k=project-home]").waitFor({ timeout: 50_000 });
    await page.locator("[data-k=project-switcher]").first().focus();
    await page.keyboard.press("ControlOrMeta+K");
    await page.locator("[data-command-palette] input[role=combobox]").first().fill("resume");
    await page.locator("[data-command-palette] [data-slot=command-item][data-highlighted]", { hasText: "Resume a conversation" }).waitFor();
    await page.keyboard.press("Enter");
    await page.locator("[data-resume-row=c1]").click();
    await page.locator("[data-k=resume-draft]").waitFor();
    await page.keyboard.type(MESSAGE);
    const draft = page.locator("[data-k=resume-draft]");
    expect(await draft.locator("[data-chat-composer] [contenteditable]").first().textContent()).toBe(MESSAGE);
    expect(await draft.getByText(TRANSCRIPT_LOADING).count()).toBe(readMs > 0 ? 1 : 0);
    const button = await page.getByRole("button", { name: "Continue", exact: true }).boundingBox();
    await page.mouse.click(button!.x + button!.width / 2, button!.y + button!.height / 2);
    await expect.poll(() => resumes(page), { timeout: 2_000 }).toEqual([MESSAGE]);
    expect(await draft.locator("[data-chat-composer] [contenteditable]").first().textContent()).toBe("");
    await page.waitForTimeout(300);
    expect(await resumes(page)).toEqual([MESSAGE]);
    await page.close();
  }, 60_000);
});
