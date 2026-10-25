// SPDX-License-Identifier: AGPL-3.0-only
// Draws cold writers' slates in a static harness build from file:// in headless Chromium, graphite, at 640 and 400 px,
// and prints per slate the words a person sees (the panel's rendered text), its height in px and any console error.
//   node measure.mjs <harness build folder> <name>...
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { join } from "node:path";
const { chromium } = createRequire(import.meta.url)(process.env.PLAYWRIGHT_CORE ?? "playwright-core");

const [build, ...names] = process.argv.slice(2);
const browser = await chromium.launch();
const words = text => text.split(/\s+/).filter(w => /[\p{L}\p{N}]/u.test(w)).length;
for (const name of names) {
  const row = { name };
  for (const width of [640, 400]) {
    const page = await browser.newPage({ viewport: { width, height: 900 } });
    const errors = [];
    page.on("console", m => { if (m.type() === "error") errors.push(m.text()); });
    page.on("pageerror", e => errors.push(String(e)));
    await page.goto(`${pathToFileURL(join(build, "screenshots/slate-glance/index.html")).href}?slate=${name}&runs=${name}&theme=graphite`);
    await page.waitForSelector("[data-glance], [data-glance-error]", { timeout: 30_000 });
    const error = await page.$("[data-glance-error]");
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(1200);
    if (error !== null) row[width] = { error: await error.textContent(), errors };
    else {
      const el = page.locator("[data-glance]");
      const box = await el.boundingBox();
      const text = await el.innerText();
      row[width] = { words: words(text), height: Math.round(box.height), errors };
      if (width === 640) row.text = text;
    }
    await page.close();
  }
  console.log(JSON.stringify(row));
}
await browser.close();
