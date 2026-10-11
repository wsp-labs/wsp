// SPDX-License-Identifier: AGPL-3.0-only
// Design round for #2054: photographs each ?screen= of the prototype page at the widths and themes asked for, with
// `pnpm --filter @wsp/web dev` serving it. PROTO_URL overrides the page's address, CHROME the browser binary.
//   node apps/web/test/resume-picker/shoot.mjs <outdir> <screen[,screen...]> [widths=1440,390] [themes=dark,light]
import { mkdirSync, writeFileSync } from "node:fs";
import { chromium } from "playwright";

const [out, screensArg, widthsArg = "1440,390", themesArg = "dark,light"] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const BASE = process.env.PROTO_URL ?? "http://localhost:5173/test/resume-picker/index.html";
const browser = await chromium.launch({ ...(process.env.CHROME ? { executablePath: process.env.CHROME } : {}), args: ["--enable-low-end-device-mode"] });
const report = {};
for (const screen of screensArg.split(",")) {
  for (const width of widthsArg.split(",").map(Number)) {
    for (const theme of themesArg.split(",")) {
      const page = await browser.newPage({ viewport: { width, height: width < 600 ? 844 : 900 }, deviceScaleFactor: 2, reducedMotion: "reduce" });
      const errors = [];
      page.on("pageerror", e => errors.push(String(e.stack ?? e)));
      page.on("console", m => m.type() === "error" && errors.push(m.text()));
      await page.goto(`${BASE}?screen=${screen}&theme=${theme}&bar=0`);
      await page.waitForTimeout(1200);
      if (screen === "rec-pick") {
        await page.locator("[data-resume-picker] input[placeholder]").focus();
        for (let i = 0; i < 6; i++) await page.keyboard.press("ArrowDown");
        await page.waitForTimeout(250);
      }
      if (screen === "search") {
        await page.locator("[data-resume-picker] input[placeholder]").fill("website");
        await page.waitForTimeout(250);
      }
      const name = `${screen}-${theme === "dark" ? "graphite" : "paper"}-${width}`;
      await page.screenshot({ path: `${out}/${name}.png` });
      report[name] = await page.evaluate(() => {
        const box = sel => {
          const el = document.querySelector(sel);
          if (el === null) return null;
          const r = el.getBoundingClientRect();
          return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) };
        };
        return {
          palette: box("[data-command-palette]"),
          picker: box("[data-resume-picker]"),
          preview: box("[data-resume-preview]"),
          scopes: box("[data-resume-scopes]"),
          firstRow: box("[data-resume-row]"),
          paletteRow: box("[data-command-palette] [data-slot=command-item]"),
          input: box("[data-resume-picker] [data-slot=autocomplete-input], [data-command-palette] [data-slot=autocomplete-input]"),
          footer: box("[data-slot=command-footer]"),
        };
      });
      if (errors.length > 0) report[name].errors = errors.slice(0, 5);
      await page.close();
    }
  }
}
await browser.close();
writeFileSync(`${out}/measure-${screensArg.replace(/,/g, "_")}.json`, JSON.stringify(report, null, 1));
console.log(JSON.stringify(report, null, 1));
