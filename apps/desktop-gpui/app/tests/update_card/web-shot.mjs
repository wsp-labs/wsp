// SPDX-License-Identifier: AGPL-3.0-only
// [CHROME=<path>] node web-shot.mjs <out-dir>: photographs card.tsx's update card in each stage the sidebar_open_row
// test draws (get, downloading, restart) through the web app's own vite config, in headless Chrome at a real 2x device
// scale. Writes <out>/update-<state>-web.png (the card's box alone) and <out>/update-fonts.json (the faces Chrome drew
// with), then stops the server it started.
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const web = resolve(here, "../../../../web");
const load = path => import(pathToFileURL(join(web, "node_modules", path)).href);
const { chromium } = await load("playwright/index.mjs");
const { createServer } = await load("vite/dist/node/index.js");

const outArg = process.argv[2];
if (outArg === undefined) {
  console.error("usage: node web-shot.mjs <out-dir>");
  process.exit(2);
}
const out = resolve(outArg);
mkdirSync(out, { recursive: true });

const PAGE = `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="root"></div>
<script type="module" src="/@fs${join(here, "card.tsx")}"></script></body></html>`;
const server = await createServer({
  configFile: join(web, "vite.config.ts"),
  root: web,
  logLevel: "warn",
  server: { host: "127.0.0.1", port: 0, strictPort: false, fs: { allow: [resolve(web, "../..")] } },
  resolve: { dedupe: ["react", "react-dom"] },
  plugins: [
    {
      name: "update-card-page",
      configureServer: s => {
        s.middlewares.use(async (req, res, next) => {
          if (!req.url?.startsWith("/update-card")) return next();
          res.setHeader("content-type", "text/html");
          res.end(await s.transformIndexHtml(req.url, PAGE));
        });
      },
    },
  ],
});
await server.listen();
const base = server.resolvedUrls.local[0];

const home = mkdtempSync(join(tmpdir(), "update-card-web-home-"));
const env = { HOME: home, ...Object.fromEntries(["PATH", "DISPLAY"].filter(n => process.env[n] !== undefined).map(n => [n, process.env[n]])) };
// A real 2x device scale rather than Playwright's emulated one, under which Blink rounds font metrics in CSS px.
const browser = await chromium.launch({
  executablePath: process.env.CHROME ?? "/usr/bin/google-chrome",
  env,
  args: ["--no-sandbox", "--hide-scrollbars", "--force-device-scale-factor=2", "--window-size=400,600"],
});

/** The faces Chrome drew the card's title, line and buttons with, off CDP. */
async function fontsOf(page) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("DOM.enable");
  await cdp.send("CSS.enable");
  const { root } = await cdp.send("DOM.getDocument", { depth: -1 });
  const found = {};
  for (const k of ["update-title", "update-line", "update-act", "update-notes"]) {
    const { nodeId } = await cdp.send("DOM.querySelector", { nodeId: root.nodeId, selector: `[data-k=${k}]` });
    if (nodeId === 0) continue;
    const { fonts } = await cdp.send("CSS.getPlatformFontsForNode", { nodeId });
    found[k] = fonts.map(f => ({ family: f.familyName, postScript: f.postScriptName, glyphs: f.glyphCount }));
  }
  await cdp.detach();
  return found;
}

const fonts = {};
try {
  const context = await browser.newContext({ viewport: null, reducedMotion: "reduce", colorScheme: "light" });
  for (const state of ["get", "downloading", "restart"]) {
    const page = await context.newPage();
    await page.goto(`${base}update-card?state=${state}`);
    const card = page.locator("[data-update-card]");
    await card.waitFor({ timeout: 60_000 });
    await page.evaluate(() => document.fonts.ready);
    await page.mouse.move(399, 599);
    await card.screenshot({ path: join(out, `update-${state}-web.png`), animations: "disabled" });
    fonts[state] = await fontsOf(page);
    await page.close();
  }
} finally {
  writeFileSync(join(out, "update-fonts.json"), `${JSON.stringify(fonts, null, 2)}\n`);
  await browser.close();
  await server.close();
  rmSync(home, { recursive: true, force: true });
}
