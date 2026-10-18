// SPDX-License-Identifier: AGPL-3.0-only
// The GPUI side of wave 2's palette, switcher, notice and Computer pane baselines: drives desktop-gpui under Xvfb with
// xdotool to each state, cuts the Electron entry's clip out of the window and diffs it against the Electron shot.
//   node gpui-shots.mjs --fixture <fx.json> --app <desktop-gpui> --electron <electron shots dir> --out <dir> [--only <globs>] [--theme paper|graphite] [--display :96]
import { execFile, spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const USAGE =
  "usage: node gpui-shots.mjs --fixture <fx.json> --app <desktop-gpui> --electron <electron shots dir> --out <dir> [--only <globs>] [--theme paper|graphite] [--display :96]";
export const THEMES = {
  paper: { theme: "light", lightTheme: "paper" },
  graphite: { theme: "dark", darkTheme: "graphite" },
};
export const SCALE = 2;
export const WINDOW = { width: 1280, height: 800 };
export const FUZZ = ["0%", "3%", "10%"];
const SCREEN = "2800x1800x24";
const STATE_MS = 120_000;

export function parseArgs(argv) {
  const known = new Set(["fixture", "app", "electron", "out", "only", "theme", "display"]);
  const got = {};
  for (let i = 0; i < argv.length; i += 2) {
    const name = argv[i]?.startsWith("--") ? argv[i].slice(2) : undefined;
    if (name === undefined || !known.has(name)) throw new Error(`${USAGE}; ${argv[i]} is not a flag it takes`);
    if (argv[i + 1] === undefined || argv[i + 1].startsWith("--")) throw new Error(`${USAGE}; --${name} takes a value`);
    got[name] = argv[i + 1];
  }
  for (const name of ["fixture", "app", "electron", "out"]) if (got[name] === undefined) throw new Error(`${USAGE}; --${name} is missing`);
  if (got.theme !== undefined && !(got.theme in THEMES)) throw new Error(`--theme takes ${Object.keys(THEMES).join(" or ")}, not ${got.theme}`);
  const display = got.display ?? ":96";
  if (!/^:\d+$/.test(display)) throw new Error(`--display takes a display such as :96, not ${display}`);
  return {
    fixture: resolve(got.fixture),
    app: resolve(got.app),
    electron: resolve(got.electron),
    out: resolve(got.out),
    only: got.only,
    themes: got.theme === undefined ? Object.keys(THEMES) : [got.theme],
    display,
  };
}

export const globsOf = only =>
  only?.split(",").map(g => new RegExp(`^${g.trim().replace(/[.+^${}()|[\]\\]/g, "\\$&").replaceAll("*", ".*").replaceAll("?", ".")}$`));

export const picked = (ids, only) => {
  const globs = globsOf(only);
  return ids.filter(id => globs === undefined || globs.some(g => g.test(id)));
};

/** A clip in CSS px as device pixels at `scale`, clamped to the window; the whole window for none. */
export function scaleClip(clip, scale = SCALE, win = WINDOW) {
  const [w, h] = [Math.round(win.width * scale), Math.round(win.height * scale)];
  if (clip == null) return { x: 0, y: 0, width: w, height: h };
  const x = Math.min(Math.max(0, Math.round(clip.x * scale)), w);
  const y = Math.min(Math.max(0, Math.round(clip.y * scale)), h);
  return { x, y, width: Math.min(Math.round(clip.width * scale), w - x), height: Math.min(Math.round(clip.height * scale), h - y) };
}

export const geometry = r => `${r.width}x${r.height}+${r.x}+${r.y}`;

/** The element alone inside a `w` by `h` capture: the clip's 16 CSS px of page around it and 4 px of its own edge cut
 * off each side, so what shows through a scrim, which is other screens', is not counted. */
export function inner(w, h, scale = SCALE) {
  const cut = Math.round(20 * scale);
  if (w <= 2 * cut || h <= 2 * cut) throw new Error(`a ${w}x${h} capture has nothing inside its ${cut} px edge`);
  return { x: cut, y: cut, width: w - 2 * cut, height: h - 2 * cut };
}

/** The share of `differing` pixels among `total`, in percent with two decimals. */
export function share(differing, total) {
  if (!(total > 0)) throw new Error(`a share of ${total} pixels`);
  return Math.round((10_000 * differing) / total) / 100;
}

/** The differing pixel count `compare -metric AE` writes, which ImageMagick 6 may put in exponent form. */
export function parseAE(said) {
  const n = Number(said.trim().split(/\s+/)[0]);
  if (!Number.isFinite(n)) throw new Error(`compare said ${JSON.stringify(said.trim())}, which holds no pixel count`);
  return Math.round(n);
}

/** Runs one shot with its own handle; past `ms` the handle is marked aborted and `stop` gets it, so only that shot's app is stopped. */
export async function deadline(ms, run, stop) {
  const shot = { aborted: false, app: undefined };
  let timer;
  const late = new Promise((_, fail) => {
    timer = setTimeout(() => ((shot.aborted = true), fail(new Error(`ran past ${ms / 1000} s`))), ms);
  });
  const running = run(shot);
  running.catch(() => {});
  try {
    return await Promise.race([running, late]);
  } catch (e) {
    shot.aborted = true;
    await stop(shot);
    throw e;
  } finally {
    clearTimeout(timer);
  }
}

const key = chord => ({ say: chord, do: x => x("key", "--clearmodifiers", chord) });
const down = k => ({ say: `${k} down`, do: x => x("keydown", k) });
const up = k => ({ say: `${k} up`, do: x => x("keyup", k) });
const type = text => ({ say: `type "${text}"`, do: x => x("type", "--delay", "40", text) });
const AGES = "relative ages on thread rows";
const LIVE = "the live readings (load, memory, disk and their plots) and the computer's name, the machine's hostname";
const EMPTY_PANE = "the clip also changes when the tab opens on a pane with no screen behind it (Nothing open here), so read the pair";

/** Each state as GPUI reaches it; `open` is the thread launched with --open ("data" is what the Electron state's data made), `wait` how long the clip may take to change. */
export const STATES = [
  { id: "W2-PAL-016", open: "short-1", steps: [key("ctrl+k")], settle: 800 },
  { id: "W2-PAL-017", open: "short-1", steps: [key("ctrl+k"), type("Parity")], settle: 800, moves: [AGES] },
  { id: "W2-PAL-018", open: "short-1", steps: [key("ctrl+k"), type("LegendList")], settle: 1500, moves: [AGES] },
  { id: "W2-PAL-019", open: "short-1", steps: [key("ctrl+k"), type("zzqx")], settle: 800 },
  { id: "W2-PAL-020", open: "short-1", steps: [key("ctrl+k"), type("new thread in"), key("Return")], settle: 800 },
  { id: "W2-PAL-022", open: "short-1", steps: [down("ctrl"), { say: "Tab", do: x => x("key", "Tab") }], settle: 800, after: [up("ctrl")], moves: [AGES] },
  {
    id: "W2-PAL-034",
    open: "short-1",
    steps: [
      {
        say: "a detached run on bench with --notify me",
        do: async (_, h) => {
          writeFileSync(join(h.home, "bench-turn.json"), JSON.stringify({ thread: "short-2", turn: 0 }));
          await h.wsp("run", "bench", "--agent", "claude", "--notify", "me", "--detach", "Parity note.");
        },
      },
    ],
    wait: 30_000,
    settle: 500,
  },
  { id: "W2-PAL-035", open: "data", steps: [key("ctrl+shift+e")], wait: 15_000, settle: 500 },
  { id: "W2-PAL-036", open: "short-1", steps: [key("ctrl+o")], wait: 15_000, settle: 500 },
  { id: "W2-UNO-016", open: "data", steps: [key("ctrl+alt+b"), key("m")], wait: 20_000, settle: 4000, moves: [LIVE], caveat: EMPTY_PANE },
  { id: "W2-UNO-017", steps: [key("ctrl+alt+b"), key("m")], wait: 20_000, settle: 3000, moves: [LIVE], caveat: EMPTY_PANE },
];

const sleep = ms => new Promise(done => setTimeout(done, ms));
const say = what => console.log(`gpui-shots: ${what}`);

async function main() {
  let args;
  try {
    args = parseArgs(process.argv.slice(2));
  } catch (e) {
    console.error(`gpui-shots: ${e.message}`);
    process.exit(2);
  }
  const refuse = why => {
    console.error(`gpui-shots: ${why}`);
    process.exit(2);
  };
  const fixture = JSON.parse(readFileSync(args.fixture, "utf8"));
  const stateDir = dirname(fixture.state);
  const home = dirname(stateDir);
  if (!existsSync(join(home, ".wsp-bench-fixture"))) refuse(`${home} has no .wsp-bench-fixture mark, so fixture.sh did not make it`);
  const hostArgs = existsSync(`/proc/${fixture.pid}/cmdline`) ? readFileSync(`/proc/${fixture.pid}/cmdline`, "utf8").split("\0").filter(Boolean) : [];
  if (!hostArgs.includes(fixture.state)) refuse(`the fixture's host (pid ${fixture.pid}) is not running on ${fixture.state}`);
  const [hostNode, hostBin] = hostArgs;
  if (!existsSync(args.app)) refuse(`no app at ${args.app}`);
  const index = JSON.parse(readFileSync(join(args.electron, "index.json"), "utf8"));
  const ids = picked(STATES.map(s => s.id), args.only);
  if (ids.length === 0) refuse(`no state matches ${args.only}; it knows ${STATES.map(s => s.id).join(", ")}`);
  if (existsSync(`/tmp/.X11-unix/X${args.display.slice(1)}`)) refuse(`display ${args.display} is taken; pass --display with a free one`);
  const { default: electronStates } = await import(pathToFileURL(join(here, "states.mjs")).href);

  const tmp = mkdtempSync(join(tmpdir(), "gpui-shots-"));
  const scratch = join(tmp, "shots");
  mkdirSync(scratch);
  mkdirSync(join(tmp, "home"));
  mkdirSync(args.out, { recursive: true });
  const PATH = "/usr/bin:/bin";
  const xEnv = { HOME: join(tmp, "home"), PATH, DISPLAY: args.display };
  const appEnv = { ...xEnv, GPUI_X11_SCALE_FACTOR: String(SCALE) };
  const tool = (cmd, argv, ms = 30_000) => {
    const r = spawnSync(cmd, argv, { env: xEnv, encoding: "utf8", timeout: ms });
    if (r.error) throw new Error(`${cmd} ${argv.join(" ")}: ${r.error.message}`);
    return r;
  };
  const xdo = (...argv) => {
    const r = tool("xdotool", argv);
    if (r.status !== 0) throw new Error(`xdotool ${argv.join(" ")}: ${r.stderr.trim()}`);
    return r.stdout.trim();
  };
  const magick = (cmd, ...argv) => {
    const r = tool(cmd, argv, 60_000);
    if (r.status !== 0 && !(cmd === "compare" && r.status === 1)) throw new Error(`${cmd} ${argv.join(" ")}: ${r.stderr.trim()}`);
    return r;
  };
  const differing = (a, b, fuzz) => parseAE(magick("compare", "-metric", "AE", "-fuzz", fuzz, a, b, "null:").stderr);

  // The fixture's own command line and socket, with a clean environment and the fixture's throwaway home.
  const wspEnv = { HOME: home, PATH: [join(home, "bin"), dirname(hostNode), PATH].join(":"), LANG: "en_US.UTF-8", WSP_UPDATE_CHECK: "0" };
  const wsp = (...argv) =>
    new Promise((done, fail) =>
      execFile(hostNode, [hostBin, ...argv, "--state", fixture.state], { cwd: home, env: wspEnv, encoding: "utf8", timeout: 120_000 }, (e, stdout, stderr) =>
        e ? fail(new Error(`wsp ${argv.join(" ")}: ${(stderr || stdout || e.message).trim()}`)) : done(stdout),
      ),
    );
  const port = JSON.parse(readFileSync(join(stateDir, "host.lock"), "utf8")).port;
  const token = readFileSync(join(stateDir, "host-token"), "utf8").trim();
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  await new Promise((done, fail) => ((ws.onopen = done), (ws.onerror = () => fail(new Error(`the host refused a socket on port ${port}`)))));
  let next = 0;
  const waiting = new Map();
  ws.onmessage = m => {
    const msg = JSON.parse(m.data);
    if (typeof msg.id === "number") waiting.get(msg.id)?.(msg), waiting.delete(msg.id);
  };
  const op = (name, params = {}) =>
    new Promise((done, fail) => {
      const id = next++;
      const late = setTimeout(() => (waiting.delete(id), fail(new Error(`${name}: no answer in 30 s`))), 30_000);
      waiting.set(id, msg => (clearTimeout(late), msg.ok === false ? fail(new Error(`${name}: ${msg.error}`)) : done(msg)));
      ws.send(JSON.stringify({ id, op: name, ...params }));
    });
  await op("auth", { token });
  const { preferences: before } = await op("preferences.get");
  const touched = new Set();
  const setPrefs = async patch => {
    for (const name of Object.keys(patch)) touched.add(name);
    await op("preferences.set", { patch });
  };
  const restorePrefs = async (keepTheme = false) => {
    const patch = {};
    for (const name of touched) if (!(keepTheme && (name === "transparency" || Object.values(THEMES).some(t => name in t)))) patch[name] = before[name] ?? null;
    if (Object.keys(patch).length === 0) return;
    await op("preferences.set", { patch });
    for (const name of Object.keys(patch)) touched.delete(name);
  };

  const ledgerPath = join(home, "parity-made.json");
  const ledger = () => (existsSync(ledgerPath) ? JSON.parse(readFileSync(ledgerPath, "utf8")) : {});
  const host = {
    fixture,
    home,
    wsp,
    op,
    pref: (name, value) => setPrefs({ [name]: value }),
    once: async (name, make) => {
      const had = ledger()[name];
      if (had !== undefined) return had;
      if (make === undefined) throw new Error(`once: ${name} is not in ${ledgerPath} and nothing here makes it`);
      const made = (await make()) ?? true;
      writeFileSync(ledgerPath, `${JSON.stringify({ ...ledger(), [name]: made }, null, 2)}\n`);
      return made;
    },
    thread: async prompt => {
      const id = ledger()[`thread:${prompt}`];
      if (id === undefined) throw new Error(`the thread "${prompt}" is not in ${ledgerPath}; baseline.mjs makes it`);
      return id;
    },
  };

  const started = [];
  const stopPid = async (child, what) => {
    if (child.exitCode !== null || child.signalCode !== null) return;
    const gone = new Promise(done => child.once("exit", done));
    child.kill("SIGTERM");
    if ((await Promise.race([gone.then(() => true), sleep(5000)])) !== true) {
      child.kill("SIGKILL");
      await gone;
    }
    say(`stopped ${what} (pid ${child.pid})`);
  };
  const startX = async () => {
    const x = spawn("Xvfb", [args.display, "-screen", "0", SCREEN, "-nolisten", "tcp"], { env: xEnv, stdio: "ignore" });
    started.push([x, "Xvfb"]);
    for (const end = Date.now() + 10_000; !existsSync(`/tmp/.X11-unix/X${args.display.slice(1)}`); await sleep(100))
      if (Date.now() > end || x.exitCode !== null) throw new Error(`Xvfb on ${args.display} did not start`);
    const wm = spawn("openbox", [], { env: xEnv, stdio: "ignore" });
    started.push([wm, "openbox"]);
    await sleep(1000);
    say(`Xvfb ${args.display} (pid ${x.pid}) and openbox (pid ${wm.pid})`);
  };
  const apps = new Set();
  const stopAll = async () => {
    for (const child of [...apps]) await stopPid(child, "the app");
    for (const [child, what] of started.reverse()) await stopPid(child, what);
    started.length = 0;
  };
  let ending = false;
  for (const [signal, code] of [["SIGINT", 130], ["SIGTERM", 143], ["SIGHUP", 129]])
    process.on(signal, async () => {
      if (ending) return;
      ending = true;
      say(`${signal}: stopping what this run started`);
      await stopAll();
      await restorePrefs().catch(() => {});
      process.exit(code);
    });

  const summary = { app: args.app, electron: args.electron, scale: SCALE, masked: "nothing; each entry's moves names what changes between runs inside its clip", shots: [] };
  const writeSummary = () => writeFileSync(join(args.out, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);

  async function shoot(shot, state, theme, clip) {
    const going = () => {
      if (shot.aborted) throw new Error(`${state.id} ${theme} was cut off`);
    };
    const data = electronStates.find(s => s.id === state.id)?.data;
    const made = data === undefined ? undefined : await data(host);
    going();
    const open = state.open === "data" ? made : state.open === undefined ? undefined : (fixture.threads[state.open] ?? state.open);
    if (state.open === "data" && typeof open !== "string") throw new Error(`the Electron state's data answered ${JSON.stringify(made)}, not a thread id`);
    const launch = ["--state", fixture.state, ...(open === undefined ? [] : ["--open", open])];
    const app = spawn(args.app, launch, { env: appEnv, stdio: "ignore" });
    shot.app = app;
    apps.add(app);
    app.once("exit", () => apps.delete(app));
    say(`${state.id} ${theme}: launched pid ${app.pid} ${launch.join(" ")}`);
    const win = xdo("search", "--sync", "--onlyvisible", "--name", "^wsp$").split("\n").pop();
    xdo("windowactivate", "--sync", win);
    xdo("mousemove", "2790", "1790");
    const geo = geometry(clip);
    const grab = name => {
      const file = join(scratch, `${name}.png`);
      magick("import", "-window", win, "-crop", geo, "+repage", file);
      return file;
    };
    // The window maps before the host's rows arrive: wait until the clip holds still for one beat.
    let last = grab("settle-0");
    for (let i = 1, end = Date.now() + 15_000; Date.now() < end; i++) {
      await sleep(700);
      const now = grab(`settle-${i % 2}`);
      if (differing(last, now, "0%") === 0) break;
      last = now;
    }
    going();
    const rest = grab("rest");
    for (const s of state.steps) {
      going();
      await s.do(xdo, host);
      await sleep(150);
    }
    let changed = false;
    for (const end = Date.now() + (state.wait ?? 10_000); Date.now() < end; await sleep(300)) if (differing(rest, grab("moving"), "0%") > 0) {
      changed = true;
      break;
    }
    await sleep(state.settle ?? 800);
    going();
    const file = join(args.out, `${state.id}-${theme}.png`);
    magick("convert", grab("shot"), "-units", "PixelsPerInch", "-density", String(72 * SCALE), file);
    for (const s of state.after ?? []) await s.do(xdo, host);
    await stopPid(app, "the app");
    const steps = state.steps.map(s => s.say).join(", then ");
    return { file, reason: changed ? undefined : `nothing in the clip changed within ${(state.wait ?? 10_000) / 1000} s of ${steps}, so the screen draws nothing there yet` };
  }

  function compareTo(electronFile, gpuiFile, stem) {
    const [w1, h1, w2, h2] = magick("identify", "-format", "%w %h ", electronFile, gpuiFile).stdout.trim().split(/\s+/).map(Number);
    const [w, h] = [Math.max(w1, w2), Math.max(h1, h2)];
    const a = join(scratch, "electron.png");
    const b = join(scratch, "gpui.png");
    magick("convert", electronFile, "-background", "magenta", "-extent", `${w}x${h}`, a);
    magick("convert", gpuiFile, "-background", "cyan", "-extent", `${w}x${h}`, b);
    magick("compare", "-fuzz", "3%", "-highlight-color", "red", a, b, join(args.out, `${stem}-diff.png`));
    magick("convert", a, b, "+append", join(args.out, `${stem}-pair.png`));
    const pixels = w * h;
    const box = inner(w, h);
    const [ai, bi] = [join(scratch, "electron-inner.png"), join(scratch, "gpui-inner.png")];
    magick("convert", a, "-crop", geometry(box), "+repage", ai);
    magick("convert", b, "-crop", geometry(box), "+repage", bi);
    const shares = Object.fromEntries(FUZZ.map(f => [f, share(differing(a, b, f), pixels)]));
    const inside = Object.fromEntries(FUZZ.map(f => [f, share(differing(ai, bi, f), box.width * box.height)]));
    return { size: `${w}x${h}`, shares, inside };
  }

  try {
    await startX();
    for (const id of ids) {
      const state = STATES.find(s => s.id === id);
      for (const theme of args.themes) {
        const entry = index.find(e => e.state === id && e.theme === theme);
        const row = { state: id, theme, reached: false, moves: state.moves ?? [], ...(state.caveat === undefined ? {} : { caveat: state.caveat }) };
        summary.shots.push(row);
        if (entry?.reached !== true || entry.file == null) {
          row.reason = `the Electron index holds no reached ${id} ${theme}, so there is no clip to cut`;
          say(`${id} ${theme}: not reached, ${row.reason}`);
          writeSummary();
          continue;
        }
        const clip = scaleClip(entry.clip);
        row.clip = clip;
        try {
          await setPrefs({ ...THEMES[theme], transparency: false });
          const shot = await deadline(STATE_MS, own => shoot(own, state, theme, clip), async own => {
            if (own.app !== undefined) await stopPid(own.app, "the app");
            tool("xdotool", ["keyup", "ctrl"]);
          });
          row.file = `${id}-${theme}.png`;
          row.reached = shot.reason === undefined;
          if (shot.reason !== undefined) row.reason = shot.reason;
          Object.assign(row, compareTo(join(args.electron, entry.file), shot.file, `${id}-${theme}`));
          const pct = `${FUZZ.map(f => `${row.shares[f].toFixed(2)}% at fuzz ${f}`).join(", ")}; inside the element ${FUZZ.map(f => `${row.inside[f].toFixed(2)}%`).join(", ")}`;
          say(`${id} ${theme}: ${row.reached ? "reached" : `not reached (${row.reason})`}; differing ${pct}${row.moves.length > 0 ? `; moves between runs: ${row.moves.join("; ")}` : ""}${row.reached && row.caveat !== undefined ? `; ${row.caveat}` : ""}`);
        } catch (e) {
          row.reason = e.message;
          say(`${id} ${theme}: not reached, ${e.message}`);
        } finally {
          await restorePrefs(true);
        }
        writeSummary();
      }
    }
  } finally {
    await stopAll();
    await restorePrefs();
    ws.close();
    rmSync(tmp, { recursive: true, force: true });
    writeSummary();
  }
  const reached = summary.shots.filter(s => s.reached).length;
  say(`${reached} of ${summary.shots.length} shots reached; ${join(args.out, "summary.json")}`);
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) await main();
