// SPDX-License-Identifier: AGPL-3.0-only
// A send with files on desktop-gpui: the message "what's this?" sent from New thread on bench with a JSON file and a PNG, shot
// whole window as states.mjs W2-THR-SENT-FILES (before its turn starts) and W2-THR-SENT-FILES-STARTED (once it has) shoot Electron.
// xdotool cannot hand a window files, so they go in over XDND (../xdnd-drop.py), as a file manager's drag does.
//   node sent-files-gpui.mjs --fixture <fx.json> --app <desktop-gpui> --out <dir> [--electron <baseline.mjs out dir>] [--only <globs>] [--theme paper|graphite] [--display :96]
// With --electron, each shot is paired with that folder's shot of the same state and theme (diff, pair, surfaces).
import { spawn, spawnSync } from "node:child_process";
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readlinkSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { connectHost, launchEnv, pairCompare, picked, raiseApp, SCALE, startDisplay, stopChild, surfaceWords, THEMES, xTools } from "./gpui-shots.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const USAGE = "usage: node sent-files-gpui.mjs --fixture <fx.json> --app <desktop-gpui> --out <dir> [--electron <dir>] [--only <globs>] [--theme paper|graphite] [--display :96]";
const SENT = "what's this?";
const IDS = ["W2-THR-SENT-FILES", "W2-THR-SENT-FILES-STARTED"];
// New thread's composer in CSS px, given as device px: the vendored gpui takes an XDND drop's device px as logical px.
const COMPOSER = { x: 768, y: 440 };
// How long after the send the shot waits: the Starting line draws at once, the started turn's first line a beat later.
const AFTER_SEND = { "W2-THR-SENT-FILES": 2500, "W2-THR-SENT-FILES-STARTED": 4000 };
const sleep = ms => new Promise(done => setTimeout(done, ms));
const say = what => console.log(`sent-files-gpui: ${what}`);
const refuse = why => (console.error(`sent-files-gpui: ${why}`), process.exit(2));

const args = {};
for (let i = 2; i < process.argv.length; i += 2) {
  const name = process.argv[i]?.replace(/^--/, "");
  if (!["fixture", "app", "out", "electron", "only", "theme", "display"].includes(name) || process.argv[i + 1] === undefined) refuse(`${USAGE}; ${process.argv[i]} is not a flag it takes`);
  args[name] = process.argv[i + 1];
}
for (const name of ["fixture", "app", "out"]) if (args[name] === undefined) refuse(`${USAGE}; --${name} is missing`);
if (args.theme !== undefined && !(args.theme in THEMES)) refuse(`--theme takes paper or graphite, not ${args.theme}`);
const display = args.display ?? ":96";
if (existsSync(`/tmp/.X11-unix/X${display.slice(1)}`)) refuse(`display ${display} is taken; pass --display with a free one`);
const app = resolve(args.app);
if (!existsSync(app)) refuse(`no app at ${app}`);
const out = resolve(args.out);
const ids = picked(IDS, args.only);
if (ids.length === 0) refuse(`no state matches ${args.only}; it knows ${IDS.join(", ")}`);
const themes = args.theme === undefined ? Object.keys(THEMES) : [args.theme];

const fixture = JSON.parse(readFileSync(resolve(args.fixture), "utf8"));
const stateDir = dirname(fixture.state);
const home = dirname(stateDir);
if (!existsSync(join(home, ".wsp-bench-fixture"))) refuse(`${home} has no .wsp-bench-fixture mark, so fixture.sh did not make it`);
const hostArgs = existsSync(`/proc/${fixture.pid}/cmdline`) ? readFileSync(`/proc/${fixture.pid}/cmdline`, "utf8").split("\0").filter(Boolean) : [];
if (!hostArgs.includes(fixture.state)) refuse(`the fixture's host (pid ${fixture.pid}) is not running on ${fixture.state}`);
const [hostNode, hostBin] = hostArgs;
const wspEnv = { HOME: home, PATH: [join(home, "bin"), dirname(hostNode), "/usr/bin:/bin"].join(":"), LANG: "en_US.UTF-8", WSP_UPDATE_CHECK: "0" };
const wsp = (...argv) => spawnSync(hostNode, [hostBin, ...argv, "--state", fixture.state], { cwd: home, env: wspEnv, encoding: "utf8", timeout: 120_000 });

const { default: states } = await import(pathToFileURL(join(here, "states.mjs")).href);
const turnsPath = join(home, "parity-turns.json");
const shim = {
  home,
  script: (prompt, reply) => writeFileSync(turnsPath, `${JSON.stringify({ ...(existsSync(turnsPath) ? JSON.parse(readFileSync(turnsPath, "utf8")) : {}), [prompt]: reply }, null, 2)}\n`),
};

const tmp = mkdtempSync(join(tmpdir(), "sent-files-gpui-"));
const scratch = join(tmp, "shots");
mkdirSync(scratch);
mkdirSync(join(tmp, "home"));
mkdirSync(out, { recursive: true });
const xEnv = { HOME: join(tmp, "home"), PATH: "/usr/bin:/bin", DISPLAY: display };
const tools = xTools(xEnv);
const { xdo, magick, differing } = tools;

const ws = await connectHost(stateDir);
const { preferences: before } = await ws.op("preferences.get");
// The fixture's claude plays the parity stub for the run, as baseline.mjs points it, and is put back after.
const claudeLink = join(home, "bin", "claude");
const parityStub = join(here, "..", "claude-stub.mjs");
const hadLink = lstatSync(claudeLink, { throwIfNoEntry: false })?.isSymbolicLink() ? readlinkSync(claudeLink) : undefined;
if (hadLink !== parityStub) rmSync(claudeLink, { force: true }), symlinkSync(parityStub, claudeLink);

/** Stops and deletes every thread the send made, so the next shot's sidebar is the one the Electron shot had. */
async function removeSent() {
  const rows = JSON.parse(wsp("threads", "--json").stdout || "{}").threads ?? [];
  for (const t of rows.filter(r => r.title === SENT)) {
    if (t.status === "running") wsp("stop", t.id);
    await ws.op("sessions.delete", { threadId: t.id });
    say(`removed the thread ${t.id}`);
  }
}

const started = [];
const summary = [];
let child;
try {
  await startDisplay(display, xEnv, started);
  for (const id of ids)
    for (const theme of themes) {
      const row = { state: id, theme, reached: false };
      summary.push(row);
      try {
        await ws.op("preferences.set", { patch: { ...THEMES[theme], transparency: false } });
        const files = await states.find(s => s.id === id).data(shim);
        const env = launchEnv(tmp, { side: "gpui", id, theme }, display);
        mkdirSync(env.HOME, { recursive: true });
        child = spawn(app, ["--state", fixture.state], { env, stdio: "ignore" });
        say(`${id} ${theme}: launched pid ${child.pid}`);
        const win = raiseApp(xdo);
        const grab = name => (magick("import", "-window", win, join(scratch, `${name}.png`)), join(scratch, `${name}.png`));
        // New thread's background field redraws every 120 ms, so the window never holds still; the host's rows land in this.
        await sleep(5000);
        const dropped = spawnSync("python3", ["-I", join(here, "..", "xdnd-drop.py"), win, String(COMPOSER.x), String(COMPOSER.y), ...files], { env: xEnv, encoding: "utf8", timeout: 20_000 });
        if (dropped.status !== 0) throw new Error(`the drop failed: ${(dropped.stderr || dropped.stdout).trim()}`);
        await sleep(1500);
        xdo("windowactivate", "--sync", win);
        xdo("type", "--delay", "40", SENT);
        xdo("key", "--clearmodifiers", "Return");
        await sleep(AFTER_SEND[id]);
        xdo("mousemove", "2790", "1790");
        await sleep(600);
        const file = join(out, `${id}-${theme}.png`);
        magick("convert", grab("shot"), "-units", "PixelsPerInch", "-density", String(72 * SCALE), `PNG24:${file}`);
        row.file = file;
        row.reached = true;
        say(`${id} ${theme}: ${file}`);
        const electron = args.electron === undefined ? undefined : join(resolve(args.electron), `${id}-${theme}.png`);
        if (electron !== undefined && existsSync(electron)) {
          Object.assign(row, pairCompare(tools, electron, file, { out, stem: `${id}-${theme}`, scratch, whole: true }));
          say(`${id} ${theme}: differing ${Object.entries(row.shares).map(([f, s]) => `${s.toFixed(2)}% at fuzz ${f}`).join(", ")}; surfaces: ${surfaceWords(row.surface)}`);
        }
      } catch (e) {
        row.reason = e.message;
        say(`${id} ${theme}: not reached, ${e.message}`);
      } finally {
        if (child !== undefined) await stopChild(child, "the app");
        child = undefined;
        await removeSent().catch(e => say(`removing the sent thread: ${e.message}`));
      }
    }
} finally {
  if (child !== undefined) await stopChild(child, "the app");
  for (const [c, what] of started.reverse()) await stopChild(c, what);
  if (hadLink !== parityStub) rmSync(claudeLink, { force: true }), hadLink !== undefined && symlinkSync(hadLink, claudeLink);
  await ws.op("preferences.set", { patch: { theme: before.theme ?? null, lightTheme: before.lightTheme ?? null, darkTheme: before.darkTheme ?? null, transparency: before.transparency ?? null } }).catch(e => say(`putting the preferences back: ${e.message}`));
  ws.close();
  rmSync(tmp, { recursive: true, force: true });
  writeFileSync(join(out, "sent-files-gpui.json"), `${JSON.stringify(summary, null, 2)}\n`);
}
say(`${summary.filter(r => r.reached).length} of ${summary.length} shots reached into ${out}`);
