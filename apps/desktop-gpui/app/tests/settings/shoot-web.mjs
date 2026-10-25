// SPDX-License-Identifier: AGPL-3.0-only
// Every Settings state of the web app as headless Chromium draws it, for the pixel compare against the GPUI app:
//   node shoot-web.mjs --fixture <out.json> --out <dir> [--theme paper|graphite] [--only <state>,...] [--scale 2]
//   node shoot-web.mjs --url <page url> --token-file <host-token> --out <dir> ...
//   node shoot-web.mjs --list [--json]
// The theme is set on the host's own preferences, which the GPUI app reads too. Each state is a fresh page load on
// the stored Settings page, opened with the Settings chord, then its steps, then the mouse parked at the top bar's
// right end and a wait until nothing moves; it is saved as <dir>/<state>-<theme>-web.png, a 1280x800 window at scale 2
// (2560x1600), or the state's own width by 800. Chrome
// runs at that scale itself (--force-device-scale-factor), never Playwright's emulated one, which rounds font metrics in
// CSS px and lands text a device px off where Electron draws it. A page taller than the
// window is also saved one window further down at a time, as <state>-p2, -p3... <dir>/web-<theme>.json holds, per
// state, each step as done in CSS pixels (a click's box and point, a field's box and its text, a key), each
// shot's scrollTop and the notices the state raised (hidden from the shot); and the faces Chromium drew with.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { hostOf, ops, sendOps } from "./host.mjs";
import { STATES, THEMES, TIME_ZONE, atOf } from "./states.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const W = 1280;
const H = 800;
/** Where the mouse rests for every shot, on the top bar past any header act, so nothing is hovered and no scrollbar shows. */
const parkAt = width => ({ x: width - 2, y: 48 });

const argv = process.argv.slice(2);
const flag = name => {
  const at = argv.indexOf(`--${name}`);
  return at < 0 ? undefined : argv[at + 1];
};
if (argv.includes("--list")) {
  if (argv.includes("--json")) console.log(JSON.stringify(STATES, null, 2));
  else for (const s of STATES) console.log(`${s.name.padEnd(30)} ${s.at.padEnd(18)} ${s.how}`);
  process.exit(0);
}
const usage = "usage: node shoot-web.mjs (--fixture <out.json> | --url <url> --token-file <file>) --out <dir> [--theme paper|graphite] [--only <state>,...] [--scale 2] [--browser] | --list [--json]";
const host = flag("fixture") !== undefined ? hostOf(flag("fixture")) : { url: flag("url"), token: flag("token-file") && readFileSync(flag("token-file"), "utf8").trim() };
const out = flag("out");
const themes = flag("theme") === undefined ? Object.keys(THEMES) : [flag("theme")];
const only = flag("only")?.split(",");
const scale = Number(flag("scale") ?? 2);
// The app as Electron draws it: a desktop bridge stands on the window with the read-only answers every Electron shell
// gives Settings (the font list, start at login, the log folder), so the desktop-only rows show as Electron draws them
// and the browser-only ones hide. --browser shoots it as a plain tab.
const desktop = !argv.includes("--browser");
if (!host.url || !host.token || !out || themes.some(t => THEMES[t] === undefined) || !(scale > 0)) {
  console.error(usage);
  process.exit(2);
}
const unknown = only?.filter(name => !STATES.some(s => s.name === name)) ?? [];
if (unknown.length > 0) {
  console.error(`no such state: ${unknown.join(", ")} (--list names them)`);
  process.exit(2);
}
mkdirSync(out, { recursive: true });

/** Playwright off the repo's web package, or off WSP_ROOT's where this tree has no install. */
function playwright() {
  for (const root of [resolve(here, "../../../../.."), process.env.WSP_ROOT].filter(Boolean)) {
    try {
      return createRequire(join(root, "apps/web/package.json"))("playwright");
    } catch {}
  }
  throw new Error("playwright is not installed: run pnpm install, or set WSP_ROOT to a tree that has it");
}

const host1 = frames => ops(host.url, host.token, frames);

const HIDE = "[data-sonner-toaster] { visibility: hidden !important; }";

/** Waits until fonts are loaded, no spinner turns and two shots 1.2 s apart are the same: a read that lands later than that (agents.read) is drawn before the shot. */
async function settle(page) {
  await page.evaluate(() => document.fonts.ready);
  const until = Date.now() + 15_000;
  let spinning = 0;
  while (Date.now() < until) {
    spinning = await page.locator(".animate-spin:visible").count();
    if (spinning === 0) break;
    await page.waitForTimeout(200);
  }
  let last;
  while (Date.now() < until) {
    await page.waitForTimeout(1200);
    const shot = await page.screenshot({ animations: "disabled", caret: "hide" });
    if (last !== undefined && shot.equals(last)) return { spinning, still: true };
    last = shot;
  }
  return { spinning, still: false };
}

/** The box of what a selector names, in viewport pixels, once it is scrolled into view the way a click scrolls it. */
async function boxOf(locator) {
  await locator.scrollIntoViewIfNeeded({ timeout: 10_000 });
  const b = await locator.boundingBox();
  return { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
}

/** Runs one step and answers what it did in viewport coordinates, for the GPUI side to replay at the same points: a
 * click or a field also says where the page stood before the step scrolled it into view (`from`) and after
 * (`scrollTop`), so the replay scrolls its page as far as this one moved. */
async function run(page, step) {
  if (step.click !== undefined) {
    const target = page.locator(step.click).first();
    const from = await scrollTopOf(page);
    const box = await boxOf(target);
    const scrollTop = await scrollTopOf(page);
    await target.click({ timeout: 10_000 });
    return [{ click: step.click, box, at: { x: box.x + Math.round(box.w / 2), y: box.y + Math.round(box.h / 2) }, from, scrollTop }];
  }
  if (step.type !== undefined) {
    const field = page.locator(step.type[0]).first();
    const from = await scrollTopOf(page);
    const box = await boxOf(field);
    const scrollTop = await scrollTopOf(page);
    await field.fill(step.type[1], { timeout: 10_000 });
    return [{ type: step.type[1], field: step.type[0], box, from, scrollTop }];
  }
  if (step.hover !== undefined) {
    const target = page.locator(step.hover).first();
    const from = await scrollTopOf(page);
    const box = await boxOf(target);
    const scrollTop = await scrollTopOf(page);
    const at = { x: box.x + Math.round(box.w / 2), y: box.y + Math.round(box.h / 2) };
    await page.mouse.move(at.x, at.y);
    await page.waitForTimeout(800);
    return [{ hover: step.hover, box, at, from, scrollTop }];
  }
  if (step.reveal !== undefined) {
    const from = await scrollTopOf(page);
    await page.locator(step.reveal).first().evaluate(el => el.scrollIntoView({ block: "center" }));
    await page.waitForTimeout(300);
    return [{ reveal: step.reveal, from, scrollTop: await scrollTopOf(page) }];
  }
  if (step.press !== undefined) {
    await page.keyboard.press(step.press);
    return [{ press: step.press, ...(step.web ? { web: true } : {}) }];
  }
  await page.locator(step.wait).first().waitFor({ timeout: 10_000 });
  return [];
}

/** How far the settings page is scrolled now, as a click's scroll into view left it. */
const scrollTopOf = page =>
  page.evaluate(() => {
    let el = document.querySelector("[data-settings-page]");
    while (el && !(el.scrollHeight > el.clientHeight + 1 && /auto|scroll/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
    return el?.scrollTop ?? 0;
  });

/** The settings page's scroller: how far it can move and how far one window takes it. */
const scroller = page =>
  page.evaluate(() => {
    let el = document.querySelector("[data-settings-page]");
    while (el && !(el.scrollHeight > el.clientHeight + 1 && /auto|scroll/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
    return el ? { max: el.scrollHeight - el.clientHeight, step: el.clientHeight } : { max: 0, step: 0 };
  });
const scrollTo = (page, top) =>
  page.evaluate(top => {
    let el = document.querySelector("[data-settings-page]");
    while (el && !(el.scrollHeight > el.clientHeight + 1 && /auto|scroll/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
    if (el) el.scrollTop = top;
    return el?.scrollTop ?? 0;
  }, top);

/** The faces Chromium drew a sans and a mono run of the settings page with, off the DevTools protocol. */
async function facesOf(page) {
  await page.evaluate(() => {
    const page = document.querySelector("[data-settings-page]") ?? document.body;
    const leaf = test => [...page.querySelectorAll("*")].find(e => e.childElementCount === 0 && e.textContent.trim() !== "" && test(getComputedStyle(e).fontFamily));
    leaf(f => !/mono/i.test(f))?.setAttribute("data-face", "sans");
    leaf(f => /mono/i.test(f))?.setAttribute("data-face", "mono");
  });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("DOM.enable");
  await cdp.send("CSS.enable");
  const { root } = await cdp.send("DOM.getDocument", { depth: -1 });
  const faces = {};
  for (const face of ["sans", "mono"]) {
    const { nodeId } = await cdp.send("DOM.querySelector", { nodeId: root.nodeId, selector: `[data-face=${face}]` });
    if (!nodeId) continue;
    const said = await page.evaluate(f => {
      const e = document.querySelector(`[data-face=${f}]`);
      return { text: e.textContent.trim().slice(0, 40), family: getComputedStyle(e).fontFamily };
    }, face);
    const { fonts } = await cdp.send("CSS.getPlatformFontsForNode", { nodeId });
    faces[face] = { ...said, drawn: fonts.map(f => `${f.familyName} (${f.postScriptName}), ${f.glyphCount} glyphs`) };
  }
  await cdp.detach();
  return faces;
}

/** Grows or shrinks the page's real window until the page inside it is `width` x H: headless Chrome takes part of
 * --window-size for its frame, and a Playwright viewport would bring back the emulated scale. */
async function fitWindow(page, width) {
  const [w, h] = await page.evaluate(() => [innerWidth, innerHeight]);
  if (w === width && h === H) return;
  const cdp = await page.context().newCDPSession(page);
  const { windowId, bounds } = await cdp.send("Browser.getWindowForTarget");
  await cdp.send("Browser.setWindowBounds", { windowId, bounds: { width: bounds.width + width - w, height: bounds.height + H - h } });
  await cdp.detach();
  const now = await page.evaluate(() => [innerWidth, innerHeight, devicePixelRatio]);
  if (now[0] !== width || now[1] !== H || now[2] !== scale) throw new Error(`the page is ${now[0]}x${now[1]} at scale ${now[2]}, not ${width}x${H} at ${scale}`);
}

const projects = Object.fromEntries((await host1([{ op: "projects.list" }]))[0].projects.map(p => [p.name, p.id]));

const { chromium } = playwright();
const browser = await chromium.launch({
  executablePath: process.env.CHROME ?? "/usr/bin/google-chrome",
  args: [`--force-device-scale-factor=${scale}`, `--window-size=${W},${H}`, ...(process.getuid?.() === 0 ? ["--no-sandbox"] : [])],
  // The HOME shoot-gpui.mjs gives the app, so both read the same fonts.
  env: { PATH: process.env.PATH, HOME: host.state === undefined ? process.env.HOME : dirname(dirname(host.state)) },
});
let failed = 0;
try {
  for (const theme of themes) {
    await host1([{ op: "preferences.set", patch: THEMES[theme] }]);
    // A run of some states keeps what an earlier run logged of the others.
    const logged = join(out, `web-${theme}.json`);
    const kept = only !== undefined && existsSync(logged) ? JSON.parse(readFileSync(logged, "utf8")).states : {};
    const log = { theme, url: host.url, viewport: { width: W, height: H, scale }, states: { ...kept } };
    for (const state of STATES.filter(s => only === undefined || only.includes(s.name))) {
      if (state.skip !== undefined) {
        console.log(`${state.name}-${theme}: not reached, ${state.skip}`);
        continue;
      }
      const at = atOf(state.at, projects);
      // No viewport: Playwright emulates one at scale 1 whenever it is handed one, so the window's own size stands.
      const context = await browser.newContext({ viewport: null, timezoneId: TIME_ZONE });
      await context.addInitScript(
        ([token, at, css, desktop]) => {
          if (desktop) window.wsp = { fontFamilies: async () => ["DejaVu Sans Mono", "Liberation Sans"], loginStart: async () => true, openLogs: async () => {} };
          localStorage.setItem("wsp:device-token", token);
          if (at !== "") localStorage.setItem("wsp:settings-at", at);
          document.addEventListener("DOMContentLoaded", () => document.head.append(Object.assign(document.createElement("style"), { textContent: css })));
        },
        [host.token, at, HIDE, desktop],
      );
      const page = await context.newPage();
      const width = state.width ?? W;
      await fitWindow(page, width);
      const began = Date.now();
      try {
        await sendOps(host, state.before);
        await page.goto(host.url);
        // Under 768 px the sidebar holding the switcher is a sheet, shut; the header's toggle stands at every width.
        await page.locator(width < 768 ? "[data-slot=sidebar-trigger]" : "[data-k=project-switcher]").first().waitFor({ timeout: 30_000 });
        await page.keyboard.press("Control+Comma");
        const replay = [{ press: "Control+Comma" }];
        await page.locator(`[data-settings-at="${at || state.atShown}"]`).waitFor({ timeout: 15_000 });
        for (const step of state.steps ?? []) replay.push(...(await run(page, step)));
        if (state.atShown !== undefined) await page.locator(`[data-settings-at="${state.atShown}"]`).waitFor({ timeout: 10_000 });
        // A hover is the state itself, so the pointer stays on what the last step hovered.
        if (replay.at(-1)?.hover === undefined) {
          const park = parkAt(width);
          await page.mouse.move(park.x, park.y);
        }
        const foot = state.scroll === "foot" ? await scrollTo(page, (await scroller(page)).max) : null;
        const settled = await settle(page);
        const shots = [{ name: state.name, scrollTop: foot }];
        await page.screenshot({ path: join(out, `${state.name}-${theme}-web.png`), animations: "disabled", caret: "hide" });
        if (state.pages !== false && state.scroll === undefined) {
          const { max, step } = await scroller(page);
          for (let n = 2, top = step; step > 0 && top - step < max; n++, top += step) {
            const scrollTop = await scrollTo(page, Math.min(top, max));
            await settle(page);
            shots.push({ name: `${state.name}-p${n}`, scrollTop });
            await page.screenshot({ path: join(out, `${state.name}-p${n}-${theme}-web.png`), animations: "disabled", caret: "hide" });
          }
        }
        const notices = (await page.locator("[data-notice]").allTextContents()).map(n => n.replace(/\s+/g, " ").trim());
        log.states[state.name] = { at, how: state.how, width, ms: Date.now() - began, ...settled, replay, shots, notices };
        if (log.faces?.sans === undefined || log.faces?.mono === undefined) log.faces = { ...(await facesOf(page)), ...log.faces };
        console.log(`${state.name}-${theme}: ${shots.length} shot(s), ${Date.now() - began} ms${settled.spinning ? `, ${settled.spinning} spinner(s) still turning` : ""}${settled.still ? "" : ", never still"}${notices.length ? `, notices: ${notices.join(" | ")}` : ""}`);
      } catch (e) {
        failed++;
        await page.screenshot({ path: join(out, `${state.name}-${theme}-failed.png`) }).catch(() => {});
        log.states[state.name] = { at, how: state.how, error: String(e?.message ?? e).split("\n")[0] };
        console.log(`${state.name}-${theme}: FAILED ${log.states[state.name].error}`);
      } finally {
        await context.close();
        await sendOps(host, state.after).catch(e => console.log(`${state.name}: putting back failed: ${e.message}`));
      }
    }
    writeFileSync(join(out, `web-${theme}.json`), `${JSON.stringify(log, null, 2)}\n`);
  }
} finally {
  await browser.close();
}
process.exit(failed === 0 ? 0 : 1);
