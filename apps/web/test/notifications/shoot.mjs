// SPDX-License-Identifier: AGPL-3.0-only
// Design round for #1977: photographs each ?state= of the prototype page in Graphite and Paper at 1440 and 390, plus
// the open list alone at twice the scale, and writes the boxes it measured beside them. Any vite serving apps/web
// will do; PROTO_URL names the page, CHROME the browser binary.
//   node apps/web/test/notifications/shoot.mjs <outdir> [states=shut,open,empty,toast,settings] [widths=1440,390]
import { mkdirSync, writeFileSync } from "node:fs";
import { chromium } from "playwright";

const [out, statesArg = "shut,open,empty,toast,settings", widthsArg = "1440,390"] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const BASE = process.env.PROTO_URL ?? "http://localhost:5173/test/notifications/index.html";
const browser = await chromium.launch({ ...(process.env.CHROME ? { executablePath: process.env.CHROME } : {}) });
const report = {};
for (const state of statesArg.split(",")) {
  for (const width of widthsArg.split(",").map(Number)) {
    for (const theme of ["dark", "light"]) {
      const page = await browser.newPage({ viewport: { width, height: width < 600 ? 844 : 900 }, deviceScaleFactor: 2, reducedMotion: "reduce" });
      const errors = [];
      page.on("pageerror", e => errors.push(String(e.stack ?? e)));
      page.on("console", m => m.type() === "error" && errors.push(m.text()));
      await page.goto(`${BASE}?state=${state}&theme=${theme}&bar=0`);
      await page.waitForSelector("[data-k=notices-bell]");
      await page.waitForTimeout(state === "toast" ? 1800 : 1200);
      const name = `${state}-${theme === "dark" ? "graphite" : "paper"}-${width}`;
      await page.screenshot({ path: `${out}/${name}.png` });
      if (state === "open" && width === 1440) await page.locator("[data-notices-list]").screenshot({ path: `${out}/${name}-list.png` });
      report[name] = await page.evaluate(() => {
        const box = el => {
          if (el === null) return null;
          const r = el.getBoundingClientRect();
          return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) };
        };
        const px = (el, prop) => (el === null ? null : getComputedStyle(el)[prop]);
        const title = document.querySelector("[data-notice-row] [data-notice-text]");
        return {
          header: box(document.querySelector("[data-shell-center] > header")),
          bell: box(document.querySelector("[data-k=notices-bell]")),
          unread: document.querySelector("[data-k=notices-unread]")?.textContent ?? null,
          list: box(document.querySelector("[data-notices-list], [data-k=notices-empty]")),
          popup: box(document.querySelector("[data-notices-list], [data-k=notices-empty]")?.closest("[data-slot=popover-popup]") ?? null),
          popupRadius: px(document.querySelector("[data-notices-list], [data-k=notices-empty]")?.closest("[data-slot=popover-popup]") ?? null, "borderRadius"),
          rows: [...document.querySelectorAll("[data-notice-row]")].map(el => ({ id: el.getAttribute("data-notice-row"), ...box(el.firstElementChild) })),
          rowText: { size: px(title, "fontSize"), ink: px(title, "color") },
          toast: box(document.querySelector("[data-sonner-toast]")),
          sidebarTile: box(document.querySelector("[data-sidebar] [data-thread-tile], [data-thread-tile]")),
        };
      });
      if (errors.length > 0) report[name].errors = errors.slice(0, 5);
      await page.close();
    }
  }
}
await browser.close();
writeFileSync(`${out}/measure.json`, JSON.stringify(report, null, 1));
console.log(Object.entries(report).map(([k, v]) => `${k} bell=${JSON.stringify(v.bell)} unread=${v.unread} list=${JSON.stringify(v.list)} rows=${v.rows.map(r => r.h).join("/")} errors=${v.errors?.length ?? 0}`).join("\n"));
