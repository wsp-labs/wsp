// SPDX-License-Identifier: AGPL-3.0-only
// [CHROME=<path>] node web-shots.mjs <state.json> <out-dir> [--states a,b]: photographs Settings > Usage as the web app draws it off a
// host already serving <state.json>, in Paper and Graphite, one state at a time. For each state it writes
// <out>/<theme>/<state>.png (1280x800 at 2x), .content.png (the centre column, x >= 256 css px), .full.png (the
// centre column at the settings scroll area's whole height) and .json (the words on the page), and <out>/fonts.json (the fonts Chrome drew
// with). It starts nothing: the host's port and token are read beside the state file.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const web = join(dirname(fileURLToPath(import.meta.url)), "../../../../web");
const { chromium } = await import(pathToFileURL(join(web, "node_modules/playwright/index.mjs")).href);

const args = process.argv.slice(2);
const flag = args.indexOf("--states");
const only = flag === -1 ? undefined : args.splice(flag, 2)[1]?.split(",");
const [statePath, outArg] = args;
if (statePath === undefined || outArg === undefined) {
  console.error("usage: node web-shots.mjs <state.json> <out-dir> [--states used-day,limits,...]");
  process.exit(2);
}
const stateDir = dirname(resolve(statePath));
const out = resolve(outArg);
const port = JSON.parse(readFileSync(join(stateDir, "host.lock"), "utf8")).port;
const token = readFileSync(join(stateDir, "host-token"), "utf8").trim();
const base = `http://127.0.0.1:${port}`;
// The page has to read the clock the host filed hours by, TZ or /etc/localtime, which Chrome need not share: run this
// with the host's TZ and the page is pinned to it.
const timezoneId = Intl.DateTimeFormat().resolvedOptions().timeZone;

const W = 1280;
const H = 800;
const SIDEBAR = 256;

// The page's own words for each control's segments and heads (apps/web/src/settings/format.ts USAGE_PAGE_WORDS).
const SPLIT_WORDS = { agent: "Agent", account: "Account", computer: "Computer", project: "Project", model: "Model", source: "Source" };
const CHART_HEAD = { day: "Tokens an hour", week: "Tokens a day", month: "Tokens a day" };
const STEPS = { day: 24, week: 7, month: 30 };

const STATES = {
  "used-day": { range: "day", split: "agent" },
  "used-week": { range: "week", split: "agent" },
  "used-month": { range: "month", split: "agent" },
  "used-week-account": { range: "week", split: "account" },
  "used-week-computer": { range: "week", split: "computer" },
  "used-week-project": { range: "week", split: "project" },
  "used-week-model": { range: "week", split: "model" },
  "used-week-source": { range: "week", split: "source" },
  limits: { tab: "limits" },
};
// A fresh host leaves the theme to the computer, and each side defaults to these, so the emulated scheme picks them.
const THEMES = {
  paper: { scheme: "light", dark: false, background: [252, 252, 252] },
  graphite: { scheme: "dark", dark: true, background: [10, 10, 10] },
};

const names = only ?? Object.keys(STATES);
for (const name of names) if (STATES[name] === undefined) throw new Error(`no state ${name}; the states are ${Object.keys(STATES).join(", ")}`);

/** The theme the page drew: its side, its sheet and --background as sRGB, through a canvas so any colour syntax reads. */
const drawnTheme = page =>
  page.evaluate(() => {
    const html = document.documentElement;
    const probe = document.createElement("div");
    probe.style.background = "var(--background)";
    document.body.append(probe);
    const css = getComputedStyle(probe).backgroundColor;
    probe.remove();
    const ctx = document.createElement("canvas").getContext("2d");
    ctx.fillStyle = css;
    ctx.fillRect(0, 0, 1, 1);
    return { dark: html.classList.contains("dark"), id: html.dataset.theme ?? "", css, rgb: [...ctx.getImageData(0, 0, 1, 1).data.slice(0, 3)] };
  });

async function pick(page, group, value) {
  const segment = page.locator(`[data-k=${group}] [data-segment="${value}"]`);
  if ((await segment.getAttribute("aria-checked")) !== "true") await segment.click();
  await page.waitForFunction(([g, v]) => document.querySelector(`[data-k=${g}] [data-segment="${v}"]`)?.getAttribute("aria-checked") === "true", [group, value]);
}

/** Until the answer for this range and split is the one drawn: the last range's figures stand while the next is read,
 * so no skeleton alone says the new one landed. */
async function usedLanded(page, { range, split }) {
  await page.waitForFunction(
    ({ steps, head, chart, nested }) => {
      if (document.querySelector("[data-k=used-loading], [data-slot=skeleton]") !== null) return false;
      const headOf = id => document.querySelector(`[data-settings-card="${id}"] [data-settings-head]`)?.textContent ?? "";
      if (headOf("usage-used") !== head || headOf("usage-chart") !== chart) return false;
      if (document.querySelectorAll("[data-usage-chart] [data-k=point]").length !== steps) return false;
      return !nested || document.querySelector("[data-used-row][data-sub]") !== null;
    },
    { steps: STEPS[range], head: `By ${SPLIT_WORDS[split].toLowerCase()}`, chart: CHART_HEAD[range], nested: split === "agent" },
    { timeout: 30_000 },
  );
}

const limitsLanded = page =>
  page.waitForFunction(() => document.querySelector("[data-k=limits-loading], [data-slot=skeleton]") === null && document.querySelector("[data-usage-pool], [data-usage-account], [data-k=no-accounts], [data-k=limits-refused]") !== null, undefined, { timeout: 30_000 });

/** The words of every part a port has to say the same, read off the page as drawn. */
const wordsOf = page =>
  page.evaluate(() => {
    const text = (el, sel) => (sel === undefined ? el : el?.querySelector(sel))?.textContent?.trim() ?? null;
    const all = (sel, root = document) => [...root.querySelectorAll(sel)];
    const checked = k => text(document.querySelector(`[data-k=${k}] [aria-checked=true]`));
    return {
      tab: checked("usage-tabs"),
      range: checked("usage-range"),
      split: checked("usage-split"),
      readAt: text(document.querySelector("[data-k=usage-read-at]")),
      heads: all("[data-settings-card]").map(card => ({ card: card.dataset.settingsCard, head: text(card, ":scope > div > [data-settings-head]") })),
      totals: all("[data-k=usage-totals] > [data-k]").map(cell => ({ k: cell.dataset.k, words: [...cell.children].map(c => c.textContent.trim()) })),
      legend: all("[data-k=usage-legend] > span").map(s => s.textContent.trim()),
      yAxis: all("[data-k=y-tick]").map(s => s.textContent.trim()),
      ticks: all("[data-k=tick]").map(s => s.textContent.trim()),
      lines: all("[data-usage-chart] [data-line]").map(g => g.dataset.line),
      tableHead: all("[data-k=used-head] > span").map(s => s.textContent.trim()),
      rows: all("[data-used-row]").map(row => ({
        key: row.dataset.usedRow,
        sub: row.hasAttribute("data-sub"),
        label: text(row, "[data-settings-title], [data-k=label]"),
        tokens: text(row, "[data-k=tokens]"),
        cacheHit: text(row, "[data-k=cache-hit]"),
        turns: text(row, "[data-k=turns]"),
        price: text(row, "[data-k=price]"),
      })),
      mix: all("[data-k^=mix-]").map(s => ({ k: s.dataset.k, words: s.textContent.trim() })),
      logs: text(document.querySelector("[data-k=logs]")),
      noUse: text(document.querySelector("[data-k=no-use]")),
      pools: all("[data-usage-pool]").map(pool => ({
        agent: pool.dataset.usagePool,
        title: text(pool, "[data-settings-title]"),
        line: text(pool, "[data-settings-description]"),
        burn: text(pool, "[data-k=burn]"),
        readAt: text(pool, "[data-k=read-at]"),
        resets: all("[data-usage-resets]", pool).map(r => ({ key: r.dataset.usageResets, words: [...r.firstElementChild.children].map(c => c.textContent.trim()), button: text(r, "[data-k=use-reset]") })),
        windows: all("[data-window]", pool).map(w => ({
          kind: w.dataset.window,
          name: text(w, "[data-k=window]"),
          left: text(w, "[data-k=left]"),
          verdict: text(w, "[data-k=verdict]"),
          meters: all("[data-k=meter-fill]", w).map(m => ({ width: m.style.width, warning: m.classList.contains("bg-warning") })),
        })),
      })),
      accounts: all("[data-usage-account]").map(a => ({ key: a.dataset.usageAccount, title: text(a, "[data-settings-title]"), line: text(a, "[data-settings-description]"), state: text(a, "[data-k=state]") })),
      noLimit: text(document.querySelector("[data-k=no-limit]")),
    };
  });

/** How much taller than the window the settings scroll area's content runs. */
const overflowOf = page =>
  page.evaluate(() => {
    let el = document.querySelector("[data-settings-page]");
    while (el !== null && !(el.scrollHeight > el.clientHeight + 1 && /auto|scroll/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
    return el === null ? 0 : el.scrollHeight - el.clientHeight;
  });

/** The families Chrome drew a few kinds of text with, off CDP, which names the face it actually used. */
async function fontsOf(page) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("DOM.enable");
  await cdp.send("CSS.enable");
  const { root } = await cdp.send("DOM.getDocument", { depth: -1 });
  const probes = {
    sectionHead: "[data-settings-card] [data-settings-head]",
    rowTitle: "[data-used-row] [data-settings-title]",
    statLabel: "[data-k=stat-tokens] > span",
    statFigure: "[data-k=stat-tokens] > span:nth-child(2) > span",
    rowFigure: "[data-used-row] [data-k=tokens]",
    yTick: "[data-k=y-tick]",
    segment: "[data-k=usage-split] [data-segment]",
  };
  const found = {};
  for (const [name, selector] of Object.entries(probes)) {
    const { nodeId } = await cdp.send("DOM.querySelector", { nodeId: root.nodeId, selector });
    if (nodeId === 0) continue;
    const { fonts } = await cdp.send("CSS.getPlatformFontsForNode", { nodeId });
    const family = await page.locator(selector).first().evaluate(el => getComputedStyle(el).fontFamily);
    found[name] = { selector, cssFamily: family, drawn: fonts.map(f => ({ family: f.familyName, postScript: f.postScriptName, glyphs: f.glyphCount, custom: f.isCustomFont })) };
  }
  await cdp.detach();
  return found;
}

const executablePath = process.env.CHROME ?? "/usr/bin/google-chrome";
const env = Object.fromEntries(["PATH", "HOME", "TZ", "DISPLAY"].filter(name => process.env[name] !== undefined).map(name => [name, process.env[name]]));
// Chrome draws at a real 2x device scale, as on the owner's Mac, rather than under Playwright's emulated one; the
// window is the page's whole size, so its height is moved through the browser itself.
const browser = await chromium.launch({
  executablePath,
  env,
  args: ["--no-sandbox", "--hide-scrollbars", "--force-device-scale-factor=2", `--window-size=${W},${H}`],
});
/** Sizes the window so the page itself is W by `height` CSS px, whatever the window keeps around it. */
const sizeTo = async (page, height) => {
  const cdp = await page.context().newCDPSession(page);
  const { windowId, bounds } = await cdp.send("Browser.getWindowForTarget");
  const [innerW, innerH] = await page.evaluate(() => [innerWidth, innerHeight]);
  await cdp.send("Browser.setWindowBounds", { windowId, bounds: { width: bounds.width + W - innerW, height: bounds.height + height - innerH } });
  await cdp.detach();
  const [w, h] = await page.evaluate(() => [innerWidth, innerHeight]);
  if (w !== W || h !== height) throw new Error(`the page is ${w}x${h}, not ${W}x${height}`);
};
let fonts;
try {
  for (const [theme, want] of Object.entries(THEMES)) {
    const folder = join(out, theme);
    mkdirSync(folder, { recursive: true });
    const context = await browser.newContext({ viewport: null, reducedMotion: "reduce", colorScheme: want.scheme, timezoneId });
    // Init scripts run in every frame, and an opaque-origin one throws on localStorage.
    await context.addInitScript(held => {
      try {
        window.localStorage.setItem("wsp:device-token", held);
      } catch {}
    }, token);
    for (const name of names) {
      const state = STATES[name];
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", e => errors.push(String(e)));
      await sizeTo(page, H);
      await page.goto(`${base}/#open/settings/usage`, { waitUntil: "domcontentloaded" });
      // A notice the host raises about the throwaway computer is the shell's, not the page's, and would cover it.
      await page.addStyleTag({ content: "[data-sonner-toaster] { display: none !important; }" });
      await page.locator("[data-k=usage-tabs]").waitFor({ state: "visible", timeout: 30_000 });
      const drawn = await drawnTheme(page);
      if (drawn.dark !== want.dark || drawn.id !== theme || drawn.rgb.join() !== want.background.join()) {
        throw new Error(`asked for ${theme}, the page drew ${JSON.stringify(drawn)}`);
      }
      if (state.tab === "limits") {
        await pick(page, "usage-tabs", "limits");
        await limitsLanded(page);
      } else {
        await page.locator("[data-k=usage-range]").waitFor({ state: "visible", timeout: 30_000 });
        await pick(page, "usage-range", state.range);
        await pick(page, "usage-split", state.split);
        await usedLanded(page, state);
      }
      // Both reads answered, so the top bar's "checked" word stands in every shot.
      await page.locator("[data-k=usage-read-at]").waitFor({ state: "visible", timeout: 30_000 });
      // Off every control and the chart, so no hover state or tooltip is in the shot.
      await page.mouse.move(W / 2, 2);
      await page.evaluate(() => document.fonts.ready);
      // Reduced motion stills DigitRoll and the chart's tween; this covers a canvas repaint and the settle-in fade.
      await page.waitForTimeout(600);

      await page.screenshot({ path: join(folder, `${name}.png`) });
      await page.screenshot({ path: join(folder, `${name}.content.png`), clip: { x: SIDEBAR, y: 0, width: W - SIDEBAR, height: H } });
      const words = await wordsOf(page);
      fonts ??= await fontsOf(page);

      let fullHeight = H;
      let extra = await overflowOf(page);
      for (let tries = 0; extra > 0 && tries < 3; tries++) {
        fullHeight += extra;
        await sizeTo(page, fullHeight);
        await page.waitForTimeout(300);
        extra = await overflowOf(page);
      }
      await page.mouse.move(W / 2, 2);
      await page.screenshot({ path: join(folder, `${name}.full.png`), clip: { x: SIDEBAR, y: 0, width: W - SIDEBAR, height: fullHeight } });

      writeFileSync(join(folder, `${name}.json`), `${JSON.stringify({ theme, state: name, ...state, drawnTheme: drawn, fullHeight, errors, ...words }, null, 2)}\n`);
      console.log(`web-shots: ${theme}/${name} (full ${W}x${fullHeight})${errors.length > 0 ? `, ${errors.length} page errors` : ""}`);
      await page.close();
    }
    await context.close();
  }
  writeFileSync(join(out, "fonts.json"), `${JSON.stringify(fonts ?? {}, null, 2)}\n`);
} finally {
  await browser.close();
}
