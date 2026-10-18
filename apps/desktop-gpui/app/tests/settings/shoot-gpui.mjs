// SPDX-License-Identifier: AGPL-3.0-only
// Every Settings state of the GPUI app beside the web shots of the same state:
//   node shoot-gpui.mjs --fixture <out.json> --web <dir> --out <dir> --bin <desktop-gpui> [--theme paper|graphite] [--only <state>,...] [--scale 2]
// For each state the web run logged in <web>/web-<theme>.json, the app is opened on the same Settings page in its frame
// under an Xvfb of its own with no window manager, its window given the input focus, in TIME_ZONE as the web's page is,
// at the web run's scale (GPUI_X11_SCALE_FACTOR), the web's steps
// are replayed with xdotool at the points the web logged in CSS px, the pointer is parked where the web parked it, and
// the window's 1280x800 at that scale is saved as <out>/<state>-<theme>-gpui.png, with the web's shot copied beside it
// for compare.sh. A state with its own width has the window sized to it once it opens, and its `gpui` steps run
// before the web's. A page shot one window lower is
// opened scrolled that far (`--screen ...@<px>`), for states with no steps; a state with steps opens at the top and
// wheels its page as far as each step scrolled the web's into view, checked by a shot before it clicks. The window is
// shot before and after each step with the pointer already on its point, the after shot once the step's target is
// redrawn (a debug build can take seconds a frame on a display with no GPU), and a step that moved too little of the
// window to be what it names (a click that missed its button) fails the state and ends its replay, so the run exits
// non-zero. On Linux a throwaway systemd of the login's own holds an enabled unit for the fixture's state file, so
// General draws Start wsp at login as the web's bridge says.
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { hostOf, ops, sendOps } from "./host.mjs";
import { STATES, THEMES, TIME_ZONE } from "./states.mjs";

const W = 1280;
const H = 800;
const parkAt = width => ({ x: width - 2, y: 48 });
/** The least share of the window a step must move: a chord's keycaps turning to Press keys is about 0.3%, a caret's
 * blink well under this. */
const MOVED = 0.0005;
const argv = process.argv.slice(2);
const flag = name => {
  const at = argv.indexOf(`--${name}`);
  return at < 0 ? undefined : argv[at + 1];
};
const fixture = flag("fixture");
const web = flag("web");
const out = flag("out");
const bin = flag("bin");
const themes = flag("theme") === undefined ? Object.keys(THEMES) : [flag("theme")];
const only = flag("only")?.split(",");
const scale = Number(flag("scale") ?? 2);
if (!fixture || !web || !out || !bin || !(scale > 0)) {
  console.error("usage: node shoot-gpui.mjs --fixture <out.json> --web <dir> --out <dir> --bin <desktop-gpui> [--theme paper|graphite] [--only <state>,...] [--scale 2]");
  process.exit(2);
}
mkdirSync(out, { recursive: true });
const host = hostOf(fixture);
/** The fixture's own folder, the HOME its host runs with: a HOME with the person's ~/.wsp in it would aim the app at
 * their account's host whenever the fixture's host is gone. */
const home = dirname(dirname(host.state));
const scratch = mkdtempSync(join(tmpdir(), "shoot-gpui-"));
/** The app's store on a named state (main.rs keeps it beside the state file), put back as it was when the run ends. */
const store = join(dirname(host.state), "desktop-gpui-local-store.json");
const storeWas = existsSync(store) ? readFileSync(store) : null;
/** The app of the shot being taken, stopped with the run. */
let running;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
/** The keys the web pressed, in xdotool's names. */
const KEYS = { Escape: "Escape", Enter: "Return", Tab: "Tab", ArrowDown: "Down", ArrowUp: "Up", Backspace: "BackSpace", Space: "space", "Control+b": "ctrl+b" };
const px = n => String(Math.round(n * scale));

/** An X server of this run's own on a display it picks free itself, and how to stop it. */
async function xvfb() {
  const server = spawn("/usr/bin/Xvfb", ["-displayfd", "3", "-screen", "0", `${W * scale + 200}x${H * scale + 200}x24`], {
    env: { PATH: "/usr/bin:/bin" },
    stdio: ["ignore", "ignore", "ignore", "pipe"],
  });
  const n = await new Promise((resolve, reject) => {
    let said = "";
    server.stdio[3].on("data", d => {
      said += d;
      if (said.includes("\n")) resolve(said.trim());
    });
    server.once("exit", code => reject(new Error(`Xvfb exited ${code}`)));
  });
  return { display: `:${n}`, stop: () => server.kill("SIGTERM") };
}

/** The login's own systemd with the fixture's host unit enabled in it, as Electron's install leaves it; nothing where
 * this computer has no systemd. Every file it adds under the fixture's HOME is taken back by `stop`. */
async function loginService() {
  const systemd = "/usr/lib/systemd/systemd";
  if (!existsSync(systemd)) {
    console.log(`no ${systemd}: General draws no Start wsp at login row`);
    return { env: {}, stop: async () => {} };
  }
  const run = join(scratch, "run");
  mkdirSync(run, { mode: 0o700 });
  const units = join(home, ".config", "systemd", "user");
  const wants = join(units, "default.target.wants");
  const made = [join(home, ".config"), join(home, ".config", "systemd"), units, wants].filter(dir => !existsSync(dir));
  mkdirSync(units, { recursive: true });
  // serviceTag in packages/host's service.ts.
  const unit = `wsp-host-${createHash("sha256").update(host.state).digest("hex").slice(0, 8)}.service`;
  writeFileSync(join(units, unit), "[Unit]\nDescription=shoot-gpui.mjs\n[Service]\nExecStart=/bin/true\n[Install]\nWantedBy=default.target\n");
  const env = { HOME: scratch, XDG_RUNTIME_DIR: run, XDG_CONFIG_HOME: join(home, ".config"), XDG_DATA_HOME: join(scratch, "data"), XDG_STATE_HOME: join(scratch, "state"), XDG_CACHE_HOME: join(scratch, "cache") };
  const manager = spawn(systemd, ["--user"], { env, stdio: "ignore" });
  const stop = async () => {
    manager.kill("SIGTERM");
    await new Promise(resolve => (manager.exitCode !== null || manager.signalCode !== null ? resolve() : manager.once("exit", resolve)));
    for (const file of [join(wants, unit), join(units, unit)]) rmSync(file, { force: true });
    for (const dir of made.reverse()) rmSync(dir, { recursive: true, force: true });
  };
  for (let i = 0; i < 50 && !existsSync(join(run, "systemd", "private")); i++) await sleep(100);
  const enabled = spawnSync("/usr/bin/systemctl", ["--user", "enable", unit], { env: { XDG_RUNTIME_DIR: run }, encoding: "utf8" });
  if (enabled.status !== 0) {
    await stop();
    throw new Error(`systemctl --user enable ${unit}: ${enabled.stderr.trim()}`);
  }
  // The app reaches this systemd by its runtime folder alone: a session bus there would start the desktop's portals
  // for it, and the document portal's mount outlives the run.
  return { env: { XDG_RUNTIME_DIR: run, DBUS_SESSION_BUS_ADDRESS: `unix:path=${join(scratch, "no-bus")}` }, stop };
}

function x(display, ...args) {
  return spawnSync("/usr/bin/xdotool", args, { env: { DISPLAY: display }, encoding: "utf8" });
}

/** The app's window on `display`, once it has one: its id and where it stands. */
async function windowOf(display, until) {
  while (Date.now() < until) {
    const id = x(display, "search", "--name", "^wsp$").stdout.trim().split("\n")[0];
    if (id) {
      const geo = x(display, "getwindowgeometry", "--shell", id).stdout;
      const v = key => Number(new RegExp(`${key}=(-?\\d+)`).exec(geo)?.[1] ?? 0);
      return { id, x: v("X"), y: v("Y") };
    }
    await sleep(200);
  }
  return undefined;
}

/** The window as it stands, saved to `file`. */
function cut(display, win, file) {
  const done = spawnSync("/usr/bin/import", ["-display", display, "-window", "root", "-crop", `${win.width * scale}x${H * scale}+${win.x}+${win.y}`, "+repage", file], {
    env: {},
    encoding: "utf8",
  });
  if (done.status !== 0) throw new Error(`import: ${done.stderr}`);
  return file;
}

/** The share of pixels two shots of a window `width` wide differ in by more than a 2% fuzz. */
function moved(a, b, width) {
  const said = spawnSync("/usr/bin/compare", ["-metric", "AE", "-fuzz", "2%", a, b, "null:"], { env: {}, encoding: "utf8" });
  const pixels = Number(said.stderr.trim().split(/\s/)[0]);
  if (!Number.isFinite(pixels)) throw new Error(`compare: ${said.stderr}`);
  return pixels / (width * scale * H * scale);
}

/** How long a step's target may take to be redrawn: a debug build draws a long table (Every process) in seconds a frame
 * on a display with no GPU, and the frame a click asks for comes after the one in progress. */
const REDRAW_WAIT = 10_000;

/** Waits until the step's `target` box (CSS px) in a fresh cut of the window differs from `before`, which was taken
 * with the pointer already on it; answers whether it did within REDRAW_WAIT. */
async function redrawn(display, win, before, after, target) {
  const box = [target.w, target.h, target.x, target.y].map(n => Math.max(1, Math.round(n * scale)));
  const geometry = `${box[0]}x${box[1]}+${box[2]}+${box[3]}`;
  const a = join(scratch, "target-before.png");
  const b = join(scratch, "target-after.png");
  spawnSync("/usr/bin/convert", [before, "-crop", geometry, "+repage", a], { env: {} });
  for (const until = Date.now() + REDRAW_WAIT; ; await sleep(250)) {
    spawnSync("/usr/bin/convert", [cut(display, win, after), "-crop", geometry, "+repage", b], { env: {} });
    const said = spawnSync("/usr/bin/compare", ["-metric", "AE", "-fuzz", "2%", a, b, "null:"], { env: {}, encoding: "utf8" });
    if (Number(said.stderr.trim().split(/\s/)[0]) > 0) return true;
    if (Date.now() >= until) return false;
  }
}

/** What a step names, for the line that says it missed. */
const named = step => (step.click !== undefined ? `click ${step.click}` : step.reveal !== undefined ? `scroll to ${step.reveal}` : step.hover !== undefined ? `hover ${step.hover}` : step.type !== undefined ? `type "${step.type}" into ${step.field}` : `press ${step.press}`);

/** The pixels the page moves for one wheel notch: SCROLL_LINES (3) in gpui-pre-linux's X11 client times the page's
 * 26 px line, measured at scale 1 and 2. A replay checks each scroll landed, so a change here fails loudly. */
const NOTCH = 78;

/** Whether the page in `after` stands `by` CSS px further down than in `before`: the page column below the header,
 * shifted by that much, is the same picture but for a hover. */
function scrolledBy(before, after, by, windowWidth) {
  const [left, top, width] = (windowWidth === W ? [340, 120, 920] : [8, 120, windowWidth - 16]).map(n => n * scale);
  const height = (680 - Math.abs(by)) * scale;
  if (height <= 0) return false;
  const a = join(scratch, "shift-a.png");
  const b = join(scratch, "shift-b.png");
  const crop = (file, y, to) => spawnSync("/usr/bin/convert", [file, "-crop", `${width}x${height}+${left}+${y}`, "+repage", to], { env: {} });
  crop(before, top + Math.max(by, 0) * scale, a);
  crop(after, top + Math.max(-by, 0) * scale, b);
  const said = spawnSync("/usr/bin/compare", ["-metric", "AE", "-fuzz", "2%", a, b, "null:"], { env: {}, encoding: "utf8" });
  return Number(said.stderr.trim().split(/\s/)[0]) / (width * height) < 0.01;
}

/** Scrolls the page as far as the web's step scrolled its own into view, in whole notches, and answers how far short
 * of it the page stands, which the step's point moves by. Throws where the page did not move as the notches say, so
 * nothing is clicked at a point the web never clicked. */
async function scrollAsWeb(display, win, step, n) {
  if (step.from === undefined) throw new Error(`step ${n} (${named(step)}): the web log says nowhere where its page stood; shoot the web again`);
  const delta = step.scrollTop - step.from;
  const notches = Math.trunc(delta / NOTCH);
  if (notches === 0) return delta;
  const x0 = step.at !== undefined ? step.at.x : step.box !== undefined ? step.box.x + step.box.w / 2 : (win.width === W ? 800 : win.width / 2);
  x(display, "mousemove", String(win.x + Number(px(x0))), String(win.y + Number(px(H / 2))));
  await sleep(600);
  // In runs short enough that the page column before and after still overlap, so each run can be checked.
  const most = Math.floor(600 / NOTCH);
  for (let left = Math.abs(notches); left > 0; left -= most) {
    const run = Math.min(left, most);
    const before = cut(display, win, join(scratch, "scroll-before.png"));
    x(display, "click", "--repeat", String(run), "--delay", "50", notches > 0 ? "5" : "4");
    await sleep(1000);
    const after = cut(display, win, join(scratch, "scroll-after.png"));
    if (!scrolledBy(before, after, Math.sign(notches) * run * NOTCH, win.width)) throw new Error(`step ${n} (${named(step)}): the page did not scroll the ${notches * NOTCH} px of the ${delta} the web scrolled, so its point is not the web's`);
  }
  return delta - notches * NOTCH;
}

async function shoot(display, state, at, shot, theme, replay, extraEnv) {
  const width = state.width ?? W;
  // A state with steps opens at the top, as the web's page loads, and scrolls as each step scrolled the web's.
  const top = shot.scrollTop;
  const word = `settings:${at}${top ? `@${top}` : ""}`;
  // Each web shot is a fresh page with nothing in its localStorage, so each app opens on an empty store: the page
  // one shot kept would open the next where a state names none.
  rmSync(store, { force: true });
  const app = spawn(bin, ["--screen", word, "--state", host.state], {
    env: { DISPLAY: display, HOME: home, PATH: "/usr/bin:/bin", TZ: TIME_ZONE, GPUI_X11_SCALE_FACTOR: String(scale), ...extraEnv },
    stdio: ["ignore", "ignore", "pipe"],
  });
  running = app;
  let said = "";
  app.stderr.on("data", d => (said += d));
  const missed = [];
  try {
    const opened = await windowOf(display, Date.now() + 15_000);
    if (opened === undefined) throw new Error(`no window: ${said.slice(-400)}`);
    if (width !== W) x(display, "windowsize", opened.id, px(width), px(H));
    // The host's reads land and the first frames settle.
    await sleep(4000);
    const win = { ...(await windowOf(display, Date.now() + 5_000)), width };
    // The web's page has the focus; with no window manager to give it, the app's window would never be active, and
    // what follows its focus (a previewed theme) would stand down.
    x(display, "windowfocus", "--sync", win.id);
    const size = x(display, "getwindowgeometry", "--shell", win.id).stdout;
    if (Number(/WIDTH=(\d+)/.exec(size)?.[1]) !== width * scale) throw new Error(`the window did not take ${width} px: ${size.replace(/\n/g, " ")}`);
    const before = join(scratch, "before.png");
    const after = join(scratch, "after.png");
    const steps = [...(state.gpui ?? []), ...replay.filter(step => step.press !== "Control+Comma" && step.web !== true)];
    for (const [n, step] of steps.entries()) {
      // Where the step's target stands once the page has scrolled, which a step that lands redraws.
      let target;
      if (step.reveal !== undefined) {
        // Scrolled in whole notches as far as the web's page moved, so the row stands within a notch of the web's;
        // scrollAsWeb checks each run of notches landed.
        await scrollAsWeb(display, win, step, n);
        continue;
      } else if (step.hover !== undefined) {
        // Shot first with the pointer parked, so what the hover draws is what the step moved.
        const short = await scrollAsWeb(display, win, step, n);
        const y = step.at.y + short;
        if (y < 0 || y >= H) throw new Error(`step ${n} (${named(step)}): its point stands ${y} px down a ${H} px window once the page has scrolled`);
        target = step.box && { ...step.box, y: step.box.y + short };
        const park = parkAt(width);
        x(display, "mousemove", String(win.x + Number(px(park.x))), String(win.y + Number(px(park.y))));
        await sleep(600);
        cut(display, win, before);
        x(display, "mousemove", String(win.x + Number(px(step.at.x))), String(win.y + Number(px(y))));
      } else if (step.click !== undefined || step.type !== undefined) {
        // The pointer stands on the step's point in both shots, so a hover is not taken for the step landing.
        const short = await scrollAsWeb(display, win, step, n);
        const point = step.click !== undefined ? step.at : { x: step.box.x + step.box.w / 2, y: step.box.y + step.box.h / 2 };
        const y = point.y + short;
        if (y < 0 || y >= H) throw new Error(`step ${n} (${named(step)}): its point stands ${y} px down a ${H} px window once the page has scrolled`);
        target = step.box && { ...step.box, y: step.box.y + short };
        x(display, "mousemove", String(win.x + Number(px(point.x))), String(win.y + Number(px(y))));
        await sleep(600);
        if (step.type !== undefined) {
          x(display, "click", "1");
          await sleep(600);
        }
        cut(display, win, before);
        if (step.click !== undefined) x(display, "click", "1");
        else x(display, "type", "--delay", "30", step.type);
      } else if (step.press !== undefined) {
        // With no window manager the keys go to the window under the pointer, which a narrower window may have left.
        const park = parkAt(width);
        x(display, "mousemove", String(win.x + Number(px(park.x))), String(win.y + Number(px(park.y))));
        await sleep(300);
        cut(display, win, before);
        x(display, "key", KEYS[step.press] ?? step.press);
      }
      await sleep(1200);
      if (target !== undefined && !(await redrawn(display, win, before, after, target))) {
        console.log(`${shot.name}-${theme}: step ${n} (${named(step)}) left its target as it was for ${REDRAW_WAIT / 1000} s`);
      }
      const share = moved(before, cut(display, win, after), width);
      if (share < MOVED) {
        missed.push(`step ${n} (${named(step)}) moved ${(share * 100).toFixed(3)}% of the window, under the ${MOVED * 100}% a step that lands moves`);
        // What comes after a step that missed would land on whatever stands there instead.
        break;
      }
    }
    // A hover is the state itself, so the pointer stays on what the last step hovered.
    if (steps.at(-1)?.hover === undefined) {
      const park = parkAt(width);
      x(display, "mousemove", String(win.x + Number(px(park.x))), String(win.y + Number(px(park.y))));
    }
    await sleep(1500);
    const file = cut(display, win, join(out, `${shot.name}-${theme}-gpui.png`));
    const webShot = `${shot.name}-${theme}-web.png`;
    if (resolve(web) !== resolve(out) && existsSync(join(web, webShot))) copyFileSync(join(web, webShot), join(out, webShot));
    return { file, missed };
  } finally {
    app.kill("SIGTERM");
    await new Promise(resolve => (app.exitCode !== null ? resolve() : app.once("exit", resolve)));
  }
}

const server = await xvfb();
let login = { env: {}, stop: async () => {} };
const stopAll = async () => {
  running?.kill("SIGTERM");
  await login.stop();
  server.stop();
  rmSync(scratch, { recursive: true, force: true });
  if (storeWas === null) rmSync(store, { force: true });
  else writeFileSync(store, storeWas);
};
// Xvfb takes a HUP as a reset, not a stop, so a closed terminal would leave it running.
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.once(signal, () => stopAll().finally(() => process.exit(1)));
}
let failed = 0;
try {
  login = await loginService();
  for (const theme of themes) {
    const log = JSON.parse(readFileSync(join(web, `web-${theme}.json`), "utf8"));
    const webScale = log.viewport?.scale ?? 1;
    if (webScale !== scale) throw new Error(`the web run in ${web} shot at scale ${webScale}, this one at ${scale}: pass --scale ${webScale}`);
    await ops(host.url, host.token, [{ op: "preferences.set", patch: THEMES[theme] }]);
    const done = { theme, scale, states: {} };
    for (const state of STATES.filter(s => (only === undefined || only.includes(s.name)) && s.compare !== false && s.skip === undefined)) {
      const logged = log.states[state.name];
      if (logged === undefined || logged.error !== undefined) {
        console.log(`${state.name}-${theme}: skipped, the web run has no shot of it`);
        continue;
      }
      try {
        await sendOps(host, state.before);
        const stepped = logged.replay.some(step => step.press !== "Control+Comma" && step.web !== true);
        for (const shot of logged.shots) {
          // The app opens scrolled before its steps run, so a state whose steps come first is shot at its top only.
          if (shot.scrollTop && stepped) {
            console.log(`${shot.name}-${theme}: skipped, its steps run before the web scrolled`);
            continue;
          }
          const began = Date.now();
          const { missed } = await shoot(server.display, state, logged.at, shot, theme, logged.replay, login.env);
          if (missed.length > 0) {
            failed++;
            done.states[shot.name] = { ms: Date.now() - began, error: missed.join("; ") };
            console.log(`${shot.name}-${theme}: FAILED, a step left the window unchanged: ${missed.join("; ")}`);
          } else {
            done.states[shot.name] = { ms: Date.now() - began };
            console.log(`${shot.name}-${theme}: ${Date.now() - began} ms`);
          }
        }
      } catch (e) {
        failed++;
        done.states[state.name] = { error: String(e?.message ?? e) };
        console.log(`${state.name}-${theme}: FAILED ${e?.message ?? e}`);
      } finally {
        await sendOps(host, state.after).catch(e => console.log(`${state.name}: putting back failed: ${e.message}`));
      }
    }
    writeFileSync(join(out, `gpui-${theme}.json`), `${JSON.stringify(done, null, 2)}\n`);
  }
} finally {
  await stopAll();
}
if (failed > 0) console.log(`${failed} state(s) failed`);
process.exit(failed === 0 ? 0 : 1);
