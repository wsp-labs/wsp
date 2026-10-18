// SPDX-License-Identifier: AGPL-3.0-only
// DISPLAY=:<n> node gpui-shots.mjs <fixture.json> <app> <out-dir> [--states W2-TRM-012,...] [--clips <index.json>]: photographs the GPUI terminal
// in the states web-shots.mjs photographs today's, off the same kind of host (bench/fixture.sh, a wsp-daemon staged),
// with the same thread, typed lines and Ghostty config from parity/wave-2/states.mjs, at 1280x800 and a real 2x
// (GPUI_X11_SCALE_FACTOR=2), in Paper and Graphite. Needs an X server with a window manager on DISPLAY, xdotool and
// ImageMagick's import. Writes <out>/<id>-<theme>.png, cropped as the web shot of the same name, and .window.png; with
// --clips, cropped to the clip parity/baseline.mjs kept for the same state and theme (its index.json).
import { spawn, spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const bin = join(here, "../../../../../packages/host/dist/bin.js");
const { default: STATES } = await import(pathToFileURL(join(here, "../../../parity/wave-2/states.mjs")).href);

const args = process.argv.slice(2);
const flag = args.indexOf("--states");
const only = flag === -1 ? undefined : args.splice(flag, 2)[1]?.split(",");
const clipsAt = args.indexOf("--clips");
const clips = clipsAt === -1 ? undefined : JSON.parse(readFileSync(resolve(args.splice(clipsAt, 2)[1]), "utf8"));
const [fixturePath, appArg, outArg] = args;
if (fixturePath === undefined || appArg === undefined || outArg === undefined || !process.env.DISPLAY) {
  console.error("usage: DISPLAY=:<n> node gpui-shots.mjs <fixture.json> <desktop-gpui binary> <out-dir> [--states W2-TRM-012,...]");
  process.exit(2);
}
const fixture = JSON.parse(readFileSync(resolve(fixturePath), "utf8"));
const app = resolve(appArg);
const out = resolve(outArg);
mkdirSync(out, { recursive: true });
const stateDir = dirname(fixture.state);
const home = dirname(stateDir);
const marks = join(home, "parity-marks");
const port = JSON.parse(readFileSync(join(stateDir, "host.lock"), "utf8")).port;
const token = readFileSync(join(stateDir, "host-token"), "utf8").trim();
const display = process.env.DISPLAY;

const SCALE = 2;
const W = 1280;
const H = 800;
const THEMES = { paper: "light", graphite: "dark" };
// The drawer's box in today's shots (their .json), which the frame here shares: the 256 px sidebar, a 280 px drawer.
const DRAWER = { x: 256, y: 520, width: 1024, height: 280 };
const PANEL_W = 540;
const sleep = ms => new Promise(done => setTimeout(done, ms));

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
  pref: (key, value) => ask("preferences.set", { patch: { [key]: value } }),
};

const x = (...line) => {
  const ran = spawnSync("xdotool", line.map(String), { encoding: "utf8", env: { DISPLAY: display, PATH: "/usr/bin:/bin" } });
  if (ran.status !== 0) throw new Error(`xdotool ${line.join(" ")}: ${ran.stderr.trim()}`);
  return ran.stdout.trim();
};

/** The app on one thread, its window found, sized and in front; answers what drives it. */
async function launch(thread, appHome) {
  const env = { HOME: appHome, PATH: "/usr/bin:/bin", LANG: "en_US.UTF-8", DISPLAY: display, GPUI_X11_SCALE_FACTOR: String(SCALE) };
  const child = spawn(app, ["--state", fixture.state, "--open", thread], { env, stdio: ["ignore", "ignore", "pipe"] });
  let log = "";
  child.stderr.on("data", d => (log += d));
  let win = "";
  for (const until = Date.now() + 30_000; win === ""; await sleep(300)) {
    if (Date.now() > until) throw new Error(`no window in 30 s: ${log.slice(-400)}`);
    win = spawnSync("xdotool", ["search", "--pid", String(child.pid)], { encoding: "utf8", env: { DISPLAY: display, PATH: "/usr/bin:/bin" } }).stdout.trim().split("\n").pop() ?? "";
  }
  const at = {
    child,
    win,
    log: () => log,
    size: async (w, h) => (x("windowsize", win, w * SCALE, h * SCALE), await sleep(800)),
    key: async combo => (x("windowactivate", "--sync", win), x("key", "--window", win, "--clearmodifiers", combo)),
    type: async text => x("type", "--delay", 8, "--window", win, text),
    click: async (cx, cy) => (x("mousemove", "--window", win, Math.round(cx * SCALE), Math.round(cy * SCALE)), await sleep(150), x("click", 1)),
    move: async (cx, cy) => x("mousemove", "--window", win, Math.round(cx * SCALE), Math.round(cy * SCALE)),
    shot: file => {
      const ran = spawnSync("import", ["-window", win, file], { encoding: "utf8", env: { DISPLAY: display, PATH: "/usr/bin:/bin" } });
      if (ran.status !== 0) throw new Error(`import: ${ran.stderr.trim()}`);
    },
  };
  await at.size(W, H);
  x("windowactivate", "--sync", win);
  // The thread's transcript and the host's answers land a beat after the window maps.
  await sleep(6000);
  return at;
}

/** The floating strip's buttons over a lone shell, in today's order, 21 px each with a 1 px divider, 8 px in from the top right. */
const STRIP = ["split", "splitVertical", "new", "font", "close"];
const strip = (box, name) => ({ x: box.x + box.width - 9 - 109 + STRIP.indexOf(name) * 22 + 10.5, y: box.y + 9 + 10.5 });
/** The same buttons in the 144 px tab column's 22 px head, right-aligned, each after the first with a 1 px rule on its left. */
const head = (box, name) => {
  const fromRight = STRIP.length - 1 - STRIP.indexOf(name);
  return { x: box.x + box.width - fromRight * 22 - 10.5, y: box.y + 11 };
};

async function drawer(at, box = DRAWER) {
  await at.key("ctrl+j");
  await sleep(4000);
  await at.click(box.x + 200, box.y + 140);
  await sleep(300);
}

async function shell(at, line, mark) {
  const file = join(marks, mark);
  rmSync(file, { force: true });
  await at.type(`${line}; touch ${file}`);
  await at.key("Return");
  for (const until = Date.now() + 20_000; !existsSync(file); await sleep(100)) if (Date.now() > until) throw new Error(`the shell never ran: ${line}`);
  await sleep(700);
}

const COLOURS = String.raw`clear; printf '\e[31mred \e[32mgreen \e[33myellow \e[34mblue \e[35mmagenta \e[36mcyan \e[1;37mbold\e[0m\n\e[38;5;208m256 \e[38;2;120;200;255mtruecolor\e[0m \e[4munderline\e[0m \e[7mreverse\e[0m\n'; ls --color=always /`;

/** Per state: the steps after the thread opens, mirroring states.mjs's drivers, and the box to crop (none: the window). */
const DRIVE = {
  "W2-TRM-012": { go: async at => (await drawer(at), await shell(at, "clear", "016")) },
  "W2-TRM-013": { go: async at => (await drawer(at), await shell(at, COLOURS, "017")) },
  "W2-TRM-014": {
    go: async at => (await drawer(at), await at.type("seq 1 1000 | sed 's/^/line /' | less"), await at.key("Return"), await sleep(1500)),
  },
  "W2-TRM-015": { go: async at => (await drawer(at), await shell(at, COLOURS, "019")) },
  "W2-TRM-016": {
    hover: true,
    go: async at => {
      await drawer(at);
      await shell(at, "clear; echo https://example.com/docs/terminal", "020");
      x("keydown", "--window", at.win, "ctrl");
      await at.move(DRAWER.x + 60, DRAWER.y + 12);
      await sleep(400);
    },
    after: () => x("keyup", "ctrl"),
  },
  "W2-TRM-017": {
    go: async at => {
      await drawer(at);
      const v = strip(DRAWER, "splitVertical");
      await at.click(v.x, v.y);
      await sleep(3000);
      const n = head(DRAWER, "new");
      await at.click(n.x, n.y);
      await sleep(3000);
      await at.move(W / 2, 2);
    },
  },
  "W2-TRM-019": {
    go: async at => {
      await drawer(at);
      await shell(at, COLOURS, "023");
      for (let i = 0; i < 3; i++) await at.key("ctrl+equal"), await sleep(300);
      await sleep(800);
    },
  },
  "W2-TRM-020": {
    window: true,
    go: async at => {
      await drawer(at);
      const f = strip(DRAWER, "font");
      await at.click(f.x, f.y);
      await sleep(800);
    },
  },
  "W2-TRM-021": {
    box: { x: W - PANEL_W, y: 0, width: PANEL_W, height: H },
    go: async at => {
      await at.key("ctrl+alt+b");
      await sleep(1500);
      await at.key("t");
      await sleep(4000);
      await at.move(W / 2, 2);
    },
  },
  "W2-TRM-022": {
    go: async at => {
      await drawer(at);
      await shell(at, "seq 1 5000", "026");
      await at.move(DRAWER.x + 300, DRAWER.y + 140);
      x("click", "--repeat", 12, "--delay", 30, 4);
      await sleep(600);
    },
  },
  "W2-TRM-023": {
    window: true,
    go: async at => {
      await at.size(640, H);
      const box = { x: 0, y: DRAWER.y, width: 640, height: DRAWER.height };
      await drawer(at, box);
      const s = strip(box, "split");
      await at.click(s.x, s.y);
      await sleep(2500);
      await at.move(320, 2);
    },
  },
  "W2-TRM-024": {
    go: async at => {
      await drawer(at);
      const c = strip(DRAWER, "close");
      await at.click(c.x, c.y);
      await sleep(800);
      await at.key("ctrl+j");
      await sleep(1500);
      await at.move(W / 2, 2);
    },
  },
  "W2-TRM-025": {
    go: async at => {
      await drawer(at);
      const n = strip(DRAWER, "new");
      await at.click(n.x, n.y);
      await sleep(3000);
      await at.click(DRAWER.x + 200, DRAWER.y + 140);
      await at.type("exit");
      await at.key("Return");
      await sleep(2500);
      await at.move(W / 2, 2);
    },
  },
};

const crop = (from, box, to) => {
  const g = `${box.width * SCALE}x${box.height * SCALE}+${box.x * SCALE}+${box.y * SCALE}`;
  const ran = spawnSync("convert", [from, "-crop", g, "+repage", to], { encoding: "utf8", env: { PATH: "/usr/bin:/bin" } });
  if (ran.status !== 0) throw new Error(`convert: ${ran.stderr.trim()}`);
};

const states = STATES.filter(s => DRIVE[s.id] !== undefined && (only === undefined || only.includes(s.id)));
const failed = [];
for (const state of states) {
  for (const [theme, mode] of Object.entries(THEMES)) {
    const name = `${state.id}-${theme}`;
    const appHome = mkdtempSync(join(tmpdir(), "wsp-gpui-shots-"));
    let at;
    try {
      await ask("preferences.set", { patch: { theme: mode } });
      await unzoom();
      await clearShells();
      const data = await state.data(host);
      const thread = typeof data === "string" ? data : data.id;
      const began = Date.now();
      at = await launch(thread, appHome);
      const drive = DRIVE[state.id];
      try {
        await drive.go(at);
        // As today's shots, the pointer leaves the window's top edge so no tooltip a click opened is caught.
        if (!drive.hover) await at.move(W / 2, 2), await sleep(1200);
        at.shot(join(out, `${name}.window.png`));
        // The cursor blinks here, where today's shots ran with reduced motion; three shots 350 ms apart catch both of
        // its 530 ms phases.
        for (const n of ["alt", "alt2"]) {
          await sleep(350);
          at.shot(join(out, `${name}.${n}.window.png`));
        }
      } finally {
        drive.after?.();
        if (data.config) rmSync(data.config, { force: true });
      }
      const kept = clips?.find(e => e.state === state.id && e.theme === theme && e.reached);
      if (kept) {
        const box = kept.clip ?? { x: 0, y: 0, width: kept.width / SCALE, height: kept.height / SCALE };
        crop(join(out, `${name}.window.png`), box, join(out, `${name}.png`));
        for (const n of ["alt", "alt2"]) crop(join(out, `${name}.${n}.window.png`), box, join(out, `${name}.${n}.png`));
      } else if (drive.window) {
        for (const n of ["", ".alt", ".alt2"]) copyFileSync(join(out, `${name}${n}.window.png`), join(out, `${name}${n}.png`));
      }
      else {
        crop(join(out, `${name}.window.png`), drive.box ?? DRAWER, join(out, `${name}.png`));
        for (const n of ["alt", "alt2"]) crop(join(out, `${name}.${n}.window.png`), drive.box ?? DRAWER, join(out, `${name}.${n}.png`));
      }
      console.log(`gpui-shots: ${name} in ${((Date.now() - began) / 1000).toFixed(1)} s`);
    } catch (e) {
      if (at) at.shot(join(out, `${name}.failed.png`));
      failed.push(`${name}: ${e.message.split("\n")[0]}`);
      console.log(`gpui-shots: ${name} FAILED: ${e.message.split("\n")[0]}`);
    } finally {
      if (at) {
        at.child.kill("SIGTERM");
        await new Promise(done => (at.child.exitCode !== null ? done() : at.child.once("exit", done)));
      }
      rmSync(appHome, { recursive: true, force: true });
    }
  }
}
await ask("preferences.set", { patch: { theme: "system" } }).catch(() => {});
if (failed.length > 0) {
  console.error(`gpui-shots: ${failed.length} failed\n${failed.join("\n")}`);
  process.exit(1);
}
