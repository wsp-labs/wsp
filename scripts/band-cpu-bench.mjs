// SPDX-License-Identifier: AGPL-3.0-only
// Measures what the live bands on Settings > Computers cost the page: five computers sending a sample every 2 s
// against the same list with no band, on the wireframe fixture's live screen.
//
// It builds the fixture page for production into a temp folder and serves it with vite preview (or serves it from
// the dev server with --mode dev), then opens it in Playwright's Chromium, one fresh browser per run, alternating
// the two pages. Each run waits out a warm-up, then reads every Chromium process's CPU seconds over the window
// (SystemInfo.getProcessInfo) and the page's heap after a forced GC. Wall-clock task time is not used: a loaded
// computer stretches it up to tenfold (measured 2026-10-11).
//
// Headed by default, which on a Mac rasters on the GPU as the desktop app does, so the GPU process's CPU is read
// and counted with the renderer's. Keep the window in front while it runs: an occluded window stops drawing.
//
//   node scripts/band-cpu-bench.mjs [--mode release|dev] [--seconds 60] [--warmup 10] [--rounds 2] [--headless]
//     [--theme dark|light] [--out <file.json>] [--assert]
//
// --assert exits 1 when the page with five live bands costs 1% of a core or more (renderer and GPU together).
// It stops everything it started and removes the folder it built into.
import { createRequire } from "node:module";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { cpus, loadavg, platform, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const WEB_DIR = join(REPO, "apps", "web");
const PAGE = "/test/wireframe/index.html";
const require = createRequire(join(WEB_DIR, "package.json"));
const { chromium } = require("playwright");
const vite = await import(pathToFileURL(join(WEB_DIR, "node_modules", "vite", "dist", "node", "index.js")).href);

/** The bar: the page with five live bands costs under this share of one core. */
const BAR_PCT = 1;

function args(argv) {
  const out = { mode: "release", seconds: 60, warmup: 10, rounds: 2, headless: false, theme: "dark", assert: false };
  for (let i = 0; i < argv.length; i += 1) {
    const value = argv[i + 1];
    switch (argv[i]) {
      case "--mode": out.mode = value; i += 1; break;
      case "--seconds": out.seconds = Number(value); i += 1; break;
      case "--warmup": out.warmup = Number(value); i += 1; break;
      case "--rounds": out.rounds = Number(value); i += 1; break;
      case "--theme": out.theme = value; i += 1; break;
      case "--out": out.out = resolve(value); i += 1; break;
      case "--headless": out.headless = true; break;
      case "--assert": out.assert = true; break;
      default: throw new Error(`unknown flag ${argv[i]}`);
    }
  }
  if (out.mode !== "release" && out.mode !== "dev") throw new Error("--mode is release or dev");
  return out;
}

const opts = args(process.argv.slice(2));
const sleep = ms => new Promise(r => setTimeout(r, ms));

/** Serves the fixture page and returns its address and how to stop it. */
async function serve() {
  if (opts.mode === "dev") {
    const server = await vite.createServer({ root: WEB_DIR, configFile: join(WEB_DIR, "vite.config.ts"), logLevel: "warn", server: { host: "127.0.0.1", port: 0 } });
    await server.listen();
    return { base: server.resolvedUrls.local[0].replace(/\/$/, ""), stop: () => server.close(), built: undefined };
  }
  const built = mkdtempSync(join(tmpdir(), "band-cpu-"));
  try {
    await vite.build({ root: WEB_DIR, configFile: join(WEB_DIR, "vite.config.ts"), logLevel: "warn", build: { outDir: built, emptyOutDir: true, rollupOptions: { input: join(WEB_DIR, PAGE.slice(1)) } } });
  } catch (error) {
    rmSync(built, { recursive: true, force: true });
    throw error;
  }
  const server = await vite.preview({ root: WEB_DIR, configFile: join(WEB_DIR, "vite.config.ts"), logLevel: "warn", build: { outDir: built }, preview: { host: "127.0.0.1", port: 0 } });
  return { base: server.resolvedUrls.local[0].replace(/\/$/, ""), stop: () => new Promise(done => server.httpServer.close(() => done())), built };
}

/** CPU seconds of every Chromium process so far, summed by its type. */
async function cpuByType(cdp) {
  const { processInfo } = await cdp.send("SystemInfo.getProcessInfo");
  const out = {};
  for (const p of processInfo) out[p.type] = (out[p.type] ?? 0) + p.cpuTime;
  return out;
}

/** One run: a fresh browser on one page, its warm-up waited out, then each process type's share of a core over the
 * window and the page's heap after a forced GC. */
async function run(base, live) {
  const url = `${base}${PAGE}?screen=settings-computers-live&computers=5&theme=${opts.theme}${live ? "" : "&bands=0"}`;
  const browser = await chromium.launch({ headless: opts.headless });
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    await page.goto(url);
    await page.waitForSelector("[data-settings-page] [data-place-row]");
    const bands = await page.locator("[data-live-band]").count();
    const watching = await page.evaluate(() => window.liveWatching?.().length ?? 0);
    if (live && (bands !== 5 || watching !== 5)) throw new Error(`expected 5 live bands and 5 watches, found ${bands} bands and ${watching} watches`);
    if (!live && bands !== 0) throw new Error(`expected no band, found ${bands}`);
    await sleep(opts.warmup * 1000);
    const cdp = await browser.newBrowserCDPSession();
    const tab = await page.context().newCDPSession(page);
    const t0 = performance.now();
    const c0 = await cpuByType(cdp);
    await sleep(opts.seconds * 1000);
    const c1 = await cpuByType(cdp);
    const wall = (performance.now() - t0) / 1000;
    await tab.send("HeapProfiler.collectGarbage");
    await tab.send("Performance.enable");
    const { metrics } = await tab.send("Performance.getMetrics");
    const heap = metrics.find(m => m.name === "JSHeapUsedSize").value;
    const pct = {};
    for (const type of Object.keys(c1)) pct[type] = ((c1[type] - (c0[type] ?? 0)) / wall) * 100;
    const renderer = pct.renderer ?? 0;
    const gpu = pct.gpu ?? 0;
    return { live, bands, wall, renderer, gpu, page: renderer + gpu, browser: pct.browser ?? 0, heapMb: heap / 1024 / 1024, load1: loadavg()[0] };
  } finally {
    await browser.close();
  }
}

const fmt = n => n.toFixed(2);
const served = await serve();
const runs = [];
try {
  console.log(`${platform()} ${cpus()[0]?.model ?? ""}, ${cpus().length} cores, ${opts.mode} build, ${opts.headless ? "headless" : "headed"}, ${opts.seconds} s a run after ${opts.warmup} s`);
  for (let round = 0; round < opts.rounds; round += 1) {
    for (const live of [false, true]) {
      const r = await run(served.base, live);
      runs.push(r);
      console.log(`${live ? "five live" : "no band  "}  renderer ${fmt(r.renderer)}%  gpu ${fmt(r.gpu)}%  page ${fmt(r.page)}%  browser ${fmt(r.browser)}%  heap ${fmt(r.heapMb)} MB  load ${fmt(r.load1)}`);
    }
  }
} finally {
  await served.stop();
  if (served.built !== undefined) rmSync(served.built, { recursive: true, force: true });
}

const mean = xs => xs.reduce((a, b) => a + b, 0) / xs.length;
const none = mean(runs.filter(r => !r.live).map(r => r.page));
const five = mean(runs.filter(r => r.live).map(r => r.page));
const held = five < BAR_PCT;
const heap = runs.filter(r => r.live).map(r => r.heapMb);
console.log(`page (renderer + gpu), mean: no band ${fmt(none)}%, five live ${fmt(five)}% of a core: ${held ? "under" : "over"} the ${BAR_PCT}% bar; heap with five live ${fmt(Math.min(...heap))} to ${fmt(Math.max(...heap))} MB`);
if (opts.out !== undefined) writeFileSync(opts.out, `${JSON.stringify({ opts, runs, none, five, held }, null, 2)}\n`);
if (opts.assert && !held) process.exit(1);
