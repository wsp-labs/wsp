// SPDX-License-Identifier: AGPL-3.0-only
// Every Settings state of the GPUI app beside the web shots of the same state:
//   node shoot-gpui.mjs --fixture <out.json> --web <dir> --out <dir> --bin <desktop-gpui> [--theme paper|graphite] [--only <state>,...]
// For each state the web run logged in <web>/web-<theme>.json, the app is opened alone on the same page under an
// Xvfb of its own with no window manager (so no frame), the web's steps are replayed with xdotool at the points the
// web logged, the pointer is parked where the web parked it, and the window's 1280x800 is saved as
// <out>/<state>-<theme>-gpui.png. A page shot one window lower is opened scrolled that far (`--screen ...@<px>`),
// for states with no steps.
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { hostOf, ops, sendOps } from "./host.mjs";
import { STATES, THEMES } from "./states.mjs";

const W = 1280;
const H = 800;
const PARK = { x: 1278, y: 48 };
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
if (!fixture || !web || !out || !bin) {
  console.error("usage: node shoot-gpui.mjs --fixture <out.json> --web <dir> --out <dir> --bin <desktop-gpui> [--theme paper|graphite] [--only <state>,...]");
  process.exit(2);
}
mkdirSync(out, { recursive: true });
const host = hostOf(fixture);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
/** The keys the web pressed, in xdotool's names. */
const KEYS = { Escape: "Escape", Enter: "Return", Tab: "Tab", ArrowDown: "Down", ArrowUp: "Up", Backspace: "BackSpace", Space: "space" };

/** An X server of this run's own on a display it picks free itself, and how to stop it. */
async function xvfb() {
  const server = spawn("Xvfb", ["-displayfd", "3", "-screen", "0", "1400x900x24"], {
    env: { PATH: process.env.PATH },
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

function x(display, ...args) {
  return spawnSync("xdotool", args, { env: { DISPLAY: display, PATH: process.env.PATH }, encoding: "utf8" });
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

async function shoot(display, state, at, shot, theme, replay) {
  // A click the web scrolled into view first is replayed on a page opened at that scroll.
  const scrolled = replay.find(step => step.scrollTop)?.scrollTop;
  const top = shot.scrollTop || scrolled;
  const word = `settings:${at}${top ? `@${top}` : ""}`;
  const app = spawn(bin, ["--screen", word, "--state", host.state], {
    env: { DISPLAY: display, HOME: process.env.HOME, PATH: process.env.PATH },
    stdio: ["ignore", "ignore", "pipe"],
  });
  let said = "";
  app.stderr.on("data", d => (said += d));
  try {
    const win = await windowOf(display, Date.now() + 15_000);
    if (win === undefined) throw new Error(`no window: ${said.slice(-400)}`);
    // The host's reads land and the first frames settle.
    await sleep(4000);
    for (const step of replay) {
      if (step.press === "Control+Comma") continue;
      if (step.click !== undefined) {
        x(display, "mousemove", String(win.x + step.at.x), String(win.y + step.at.y), "click", "1");
      } else if (step.type !== undefined) {
        x(display, "mousemove", String(win.x + step.box.x + Math.round(step.box.w / 2)), String(win.y + step.box.y + Math.round(step.box.h / 2)), "click", "1");
        await sleep(300);
        x(display, "type", "--delay", "30", step.type);
      } else if (step.press !== undefined) {
        x(display, "key", KEYS[step.press] ?? step.press);
      }
      await sleep(1200);
    }
    x(display, "mousemove", String(win.x + PARK.x), String(win.y + PARK.y));
    await sleep(1500);
    const file = join(out, `${shot.name}-${theme}-gpui.png`);
    const cut = spawnSync("import", ["-display", display, "-window", "root", "-crop", `${W}x${H}+${win.x}+${win.y}`, "+repage", file], {
      env: { PATH: process.env.PATH },
      encoding: "utf8",
    });
    if (cut.status !== 0) throw new Error(`import: ${cut.stderr}`);
    return file;
  } finally {
    app.kill("SIGTERM");
    await new Promise(resolve => (app.exitCode !== null ? resolve() : app.once("exit", resolve)));
  }
}

const server = await xvfb();
let failed = 0;
try {
  for (const theme of themes) {
    const log = JSON.parse(readFileSync(join(web, `web-${theme}.json`), "utf8"));
    await ops(host.url, host.token, [{ op: "preferences.set", patch: THEMES[theme] }]);
    const done = { theme, states: {} };
    for (const state of STATES.filter(s => (only === undefined || only.includes(s.name)) && s.compare !== false)) {
      const logged = log.states[state.name];
      if (logged === undefined || logged.error !== undefined) {
        console.log(`${state.name}-${theme}: skipped, the web run has no shot of it`);
        continue;
      }
      try {
        await sendOps(host, state.before);
        const stepped = logged.replay.some(step => step.press !== "Control+Comma");
        for (const shot of logged.shots) {
          // The app opens scrolled before its steps run, so a state whose steps come first is shot at its top only.
          if (shot.scrollTop && stepped) {
            console.log(`${shot.name}-${theme}: skipped, its steps run before the web scrolled`);
            continue;
          }
          const began = Date.now();
          await shoot(server.display, state, logged.at, shot, theme, logged.replay);
          done.states[shot.name] = { ms: Date.now() - began };
          console.log(`${shot.name}-${theme}: ${Date.now() - began} ms`);
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
  server.stop();
}
process.exit(failed === 0 ? 0 : 1);
