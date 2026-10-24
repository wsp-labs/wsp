// SPDX-License-Identifier: AGPL-3.0-only
// Shoots slates from the harness page at 400 and 640 px in Graphite and Paper, in Chrome at a device scale of 2.
//   node shoot.mjs <out folder> <base url> <name>[:<shot name>][?set=<json>] ...
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

const [out, base, ...shots] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const browser = await chromium.launch({ executablePath: process.env.CHROME ?? "/usr/bin/google-chrome", args: ["--force-device-scale-factor=2", "--no-sandbox"] });
for (const spec of shots) {
  const [head, query = ""] = spec.split("?");
  const [slate, named = slate] = head.split(":");
  for (const width of [400, 640]) {
    for (const theme of ["graphite", "paper"]) {
      const page = await browser.newPage({ viewport: { width, height: 900 } });
      await page.goto(`${base}/screenshots/slate-glance/index.html?slate=${slate}&theme=${theme}${query === "" ? "" : `&${query}`}`);
      await page.waitForSelector("[data-glance], [data-glance-error]", { timeout: 30_000 });
      const error = await page.$("[data-glance-error]");
      if (error !== null) throw new Error(`${slate}: ${await error.textContent()}`);
      await page.evaluate(() => document.fonts.ready);
      await page.waitForTimeout(1200);
      const file = join(out, `${named}-${width}-${theme}.png`);
      await page.locator("[data-glance]").screenshot({ path: file, animations: "disabled" });
      console.log(file);
      await page.close();
    }
  }
}
await browser.close();
