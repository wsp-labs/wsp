// SPDX-License-Identifier: AGPL-3.0-only
// [CHROME=<path>] node web-shots.mjs <fixture.json> <out-dir> [--states W2-TRM-012,...]: photographs today's terminal in the
// web app, off the host bench/fixture.sh started (with a wsp-daemon staged), for each terminal state parity/wave-2's
// states.mjs drives, in Paper and Graphite. For each it writes <out>/<id>-<theme>.png (the state's clip, else the
// window) and <id>-<theme>.window.png (1280x800 at a real 2x), and <out>/fonts.json (the faces Chrome drew with).
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const web = join(here, "../../../../web");
const bin = join(here, "../../../../../packages/host/dist/bin.js");
const { chromium } = await import(pathToFileURL(join(web, "node_modules/playwright/index.mjs")).href);
const { default: STATES } = await import(pathToFileURL(join(here, "../../../parity/wave-2/states.mjs")).href);

const args = process.argv.slice(2);
const flag = args.indexOf("--states");
const only = flag === -1 ? undefined : args.splice(flag, 2)[1]?.split(",");
const [fixturePath, outArg] = args;
if (fixturePath === undefined || outArg === undefined) {
  console.error("usage: node web-shots.mjs <fixture.json> <out-dir> [--states W2-TRM-012,...]");
  process.exit(2);
}
const fixture = JSON.parse(readFileSync(resolve(fixturePath), "utf8"));
const out = resolve(outArg);
mkdirSync(out, { recursive: true });
const stateDir = dirname(fixture.state);
const home = dirname(stateDir);
const port = JSON.parse(readFileSync(join(stateDir, "host.lock"), "utf8")).port;
const token = readFileSync(join(stateDir, "host-token"), "utf8").trim();
const base = `http://127.0.0.1:${port}`;

const W = 1280;
const H = 800;
const THEMES = { paper: { scheme: "light", dark: false }, graphite: { scheme: "dark", dark: true } };
// The link state is a Ctrl hover the driver made; moving the real pointer off the grid would end it.
const HOVERED = new Set(["W2-TRM-016"]);
// The toolbar moves into the tab column's header under the pointer, and today's app then leaves the tooltip the click
// opened up for good; a press on the active grid, outside it, closes it as a person's would.
const RELEAVE = { "W2-TRM-017": 'aside[data-terminal-owner="drawer"] [data-terminal-viewport] canvas' };
const terminalStates = STATES.filter(s => s.id.startsWith("W2-TRM-") && (only === undefined || only.includes(s.id)));

/** One wsp line against the fixture's host, with the fixture's own HOME and nothing else of this environment. */
function wsp(...line) {
  const env = { HOME: home, PATH: `${home}/bin:${dirname(process.execPath)}:/usr/local/bin:/usr/bin:/bin`, LANG: "en_US.UTF-8", WSP_UPDATE_CHECK: "0" };
  const ran = spawnSync(process.execPath, [bin, ...line, "--state", fixture.state], { cwd: home, env, encoding: "utf8" });
  if (ran.status !== 0) throw new Error(`wsp ${line.slice(0, 2).join(" ")}: ${ran.stderr.trim() || ran.stdout.trim()}`);
  return `${ran.stdout}${ran.stderr}`;
}

/** One op on the host's own socket, as the app's client sends it. */
function ask(op, params = {}) {
  return new Promise((done, fail) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
    const timer = setTimeout(() => (ws.close(), fail(new Error(`${op}: no answer`))), 10_000);
    ws.onopen = () => ws.send(JSON.stringify({ id: 0, op: "auth", token }));
    ws.onmessage = e => {
      const m = JSON.parse(String(e.data));
      if (m.id === 0 && m.ok) ws.send(JSON.stringify({ id: 1, op, ...params }));
      else if (m.id === 0 || m.id === 1) {
        clearTimeout(timer);
        ws.close();
        m.ok ? done(m) : fail(new Error(`${op}: ${JSON.stringify(m)}`));
      }
    };
  });
}

/** Every workspace's terminal zoom back to none: the states share one workspace, and W2-TRM-019 zooms it. */
async function unzoom() {
  const { preferences } = await ask("preferences.get");
  const held = Object.keys(preferences.terminalZoom ?? {});
  if (held.length > 0) await ask("preferences.set", { patch: { terminalZoom: Object.fromEntries(held.map(k => [k, null])) } });
}

/** Ends every shell the host's workspaces hold, which killed windows of earlier states leave behind. */
async function clearShells() {
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  let next = 0;
  const pending = new Map();
  const op = (name, params = {}) => new Promise(done => (pending.set(++next, done), ws.send(JSON.stringify({ id: next, op: name, ...params }))));
  ws.onmessage = e => {
    const m = JSON.parse(String(e.data));
    pending.get(m.id)?.(m);
    pending.delete(m.id);
  };
  await new Promise(done => (ws.onopen = done));
  await new Promise(done => (pending.set(0, done), ws.send(JSON.stringify({ id: 0, op: "auth", token }))));
  const { workspaces = [] } = await op("workspaces.list");
  for (const w of workspaces) {
    const open = await op("daemon.open", { workspaceId: w.id });
    if (!open.ok) continue;
    const list = await op("daemon.send", { channel: open.channel, frame: { op: "pty.list" } });
    for (const p of list.reply?.ptys ?? []) await op("daemon.send", { channel: open.channel, frame: { op: "pty.kill", ptyId: p.id } });
    await op("daemon.close", { channel: open.channel });
  }
  ws.close();
}

const done = new Map();
const host = {
  home,
  fixture,
  wsp: async (...line) => wsp(...line),
  once: (name, work) => {
    if (!done.has(name)) done.set(name, work());
    return done.get(name);
  },
  pref: () => Promise.reject(new Error("no terminal state sets a preference")),
};

const selector = target => (target.startsWith("css:") ? target.slice(4) : target);
const call = (fn, list) => `(${fn})(...${JSON.stringify(list)})`;

/** The `app` states.mjs's drivers are written against, over one Playwright page. */
function appOn(page) {
  let loads = 0;
  return {
    fixture,
    route: {
      thread: async id => {
        loads += 1;
        await page.goto(`${base}/?load=${loads}#open/thread/${id}`, { waitUntil: "domcontentloaded" });
        await page.addStyleTag({ content: "[data-sonner-toaster] { display: none !important; }" });
        await page.waitForFunction(() => document.body.innerText.includes("Ask anything"), undefined, { timeout: 30_000 });
        // The thread's keys are bound a beat after its composer shows; a chord pressed sooner opens nothing.
        await page.waitForTimeout(2500);
      },
    },
    key: async combo => page.keyboard.press(combo.split("+").map(k => ({ mod: "Control", enter: "Enter", escape: "Escape" })[k] ?? k).join("+")),
    type: async text => page.keyboard.type(text, { delay: 5 }),
    click: async target => page.locator(selector(target)).first().click(),
    scroll: async (target, dy) => {
      await page.locator(selector(target)).first().hover();
      await page.mouse.wheel(0, dy);
      await page.waitForTimeout(400);
    },
    sleep: ms => page.waitForTimeout(ms),
    size: async (w, h) => sizeTo(page, w, h),
    page: async (fn, ...list) => page.evaluate(call(fn, list)),
    waitFor: async (fn, ms, ...list) => page.waitForFunction(call(fn, list), undefined, { timeout: ms }),
  };
}

/** Sizes the window so the page itself is w by h CSS px, whatever the window keeps around it. */
async function sizeTo(page, w, h) {
  const cdp = await page.context().newCDPSession(page);
  const { windowId, bounds } = await cdp.send("Browser.getWindowForTarget");
  const [innerW, innerH] = await page.evaluate(() => [innerWidth, innerHeight]);
  await cdp.send("Browser.setWindowBounds", { windowId, bounds: { width: bounds.width + w - innerW, height: bounds.height + h - innerH } });
  await cdp.detach();
  const got = await page.evaluate(() => [innerWidth, innerHeight]);
  if (got[0] !== w || got[1] !== h) throw new Error(`the page is ${got.join("x")}, not ${w}x${h}`);
}

/** The faces Chrome drew the drawer's chrome with, off CDP, which names the face used; the grid is a canvas CDP cannot read. */
async function fontsOf(page) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("DOM.enable");
  await cdp.send("CSS.enable");
  const { root } = await cdp.send("DOM.getDocument", { depth: -1 });
  const found = {};
  for (const [name, sel] of Object.entries({ tabs: "[data-terminal-tabs]", empty: "[data-terminal-empty]", crumbs: "header" })) {
    const { nodeId } = await cdp.send("DOM.querySelector", { nodeId: root.nodeId, selector: sel });
    if (nodeId === 0) continue;
    const { fonts } = await cdp.send("CSS.getPlatformFontsForNode", { nodeId });
    if (fonts.length > 0) found[name] = fonts.map(f => ({ family: f.familyName, postScript: f.postScriptName, glyphs: f.glyphCount }));
  }
  await cdp.detach();
  return found;
}

const chromeHome = mkdtempSync(join(tmpdir(), "wsp-shots-chrome-"));
const env = { PATH: process.env.PATH ?? "/usr/bin:/bin", HOME: chromeHome, ...(process.env.DISPLAY ? { DISPLAY: process.env.DISPLAY } : {}) };
const browser = await chromium.launch({
  executablePath: process.env.CHROME ?? "/usr/bin/google-chrome",
  env,
  args: ["--no-sandbox", "--hide-scrollbars", "--force-device-scale-factor=2", `--window-size=${W},${H}`],
});
const failed = [];
const fonts = {};
try {
  for (const state of terminalStates) {
    if (state.skip !== undefined) {
      console.log(`web-shots: ${state.id} skipped: ${state.skip}`);
      continue;
    }
    for (const [theme, want] of Object.entries(THEMES)) {
      const context = await browser.newContext({ viewport: null, reducedMotion: "reduce", colorScheme: want.scheme });
      await context.addInitScript(held => {
        try {
          window.localStorage.setItem("wsp:device-token", held);
        } catch {}
      }, token);
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", e => errors.push(String(e)));
      try {
        await sizeTo(page, W, H);
        await unzoom();
        await clearShells();
        const data = await state.data(host);
        const app = appOn(page);
        const began = Date.now();
        await state.go(app, data);
        const dark = await page.evaluate(() => document.documentElement.classList.contains("dark"));
        if (dark !== want.dark) throw new Error(`asked for ${theme}, the page drew ${dark ? "dark" : "light"}`);
        if (RELEAVE[state.id] !== undefined) await page.locator(RELEAVE[state.id]).last().click();
        if (!HOVERED.has(state.id)) await page.mouse.move(W / 2, 2);
        await page.evaluate(() => document.fonts.ready);
        // A tooltip a driver's click opened takes this long to close once the pointer leaves.
        await page.waitForTimeout(1200);
        const name = `${state.id}-${theme}`;
        await page.screenshot({ path: join(out, `${name}.window.png`) });
        if (state.clip !== undefined) await page.locator(state.clip).first().screenshot({ path: join(out, `${name}.png`) });
        else await page.screenshot({ path: join(out, `${name}.png`) });
        const box = state.clip === undefined ? null : await page.locator(state.clip).first().boundingBox();
        writeFileSync(join(out, `${name}.json`), `${JSON.stringify({ id: state.id, theme, clip: state.clip ?? null, box, errors }, null, 2)}\n`);
        Object.assign(fonts, await fontsOf(page));
        console.log(`web-shots: ${name} in ${((Date.now() - began) / 1000).toFixed(1)} s${errors.length ? `, ${errors.length} page errors` : ""}`);
      } catch (e) {
        failed.push(`${state.id}-${theme}: ${e.message.split("\n")[0]}`);
        await page.screenshot({ path: join(out, `${state.id}-${theme}.failed.png`) }).catch(() => {});
        console.log(`web-shots: ${state.id}-${theme} FAILED: ${e.message.split("\n")[0]}`);
      }
      await context.close();
    }
  }
  writeFileSync(join(out, "fonts.json"), `${JSON.stringify(fonts, null, 2)}\n`);
} finally {
  await browser.close();
  rmSync(chromeHome, { recursive: true, force: true });
}
if (failed.length > 0) {
  console.error(`web-shots: ${failed.length} failed\n${failed.join("\n")}`);
  process.exit(1);
}
