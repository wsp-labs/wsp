// SPDX-License-Identifier: AGPL-3.0-only
// The state sweep: today's routes (the hash and link forms in packages/protocol/src/app-address.ts), centre screens
// (WorkspaceCenter in apps/web/src/App.tsx), right panel panes (RightPanelKind in apps/web/src/panes.ts) and the
// empty and first-run states, read from today's source, so a kind added there is a row this script has not met. Each
// row is shot whole window at 1280x800, scale 2, in Paper and Graphite: today's app by baseline.mjs --wave sweep, then
// desktop-gpui launched to the same state under Xvfb. A row is not reached on a launch refused (exit 64, no pane or
// screen of that name), a panel or centre that draws only a short line where today's draws a screen (Nothing open here,
// Pick a thread), or a kind with no GPUI launch (MISSING); its surfaces differ on any ground that differs (surface.mjs).
// The fuzz shares are recorded for information and never decide a row: a pair can score low and still look wrong.
//   node sweep.mjs --fixture <fx.json> --app <desktop-gpui> --electron-app <Electron wsp> --out <dir> [--only <globs>] [--theme paper|graphite] [--display :97] [--electron-from <dir>] [--starting <out.json>] [--stood <out.json>]
import { spawn } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { readShot } from "./surface.mjs";
import { LONG_BRIEF } from "./wave-sweep/drives.mjs";
import { FUZZ, SCALE, THEMES, WINDOW, connectHost, deadline, globsOf, launchEnv, pairCompare, raiseApp, spawnEmptyHost, startDisplay, stopChild, surfaceWords, xTools } from "./wave-2/gpui-shots.mjs";

const here = dirname(fileURLToPath(import.meta.url));
export const USAGE =
  "usage: node sweep.mjs --fixture <fx.json> --app <desktop-gpui> --electron-app <Electron wsp> --out <dir> [--only <globs>] [--theme paper|graphite] [--display :97] [--electron-from <dir>] [--starting <out.json>] [--stood <out.json>]";
export const SOURCES = {
  panes: join(here, "../../web/src/panes.ts"),
  address: join(here, "../../../packages/protocol/src/app-address.ts"),
  centre: join(here, "../../web/src/App.tsx"),
};
const STATE_MS = 120_000;

/** RightPanelKind's members. */
export function paneKinds(source) {
  const union = /export type RightPanelKind = ([^;]+);/.exec(source)?.[1];
  if (union === undefined) throw new Error("panes.ts declares no RightPanelKind");
  return [...union.matchAll(/"([^"]+)"/g)].map(m => m[1]);
}

/** The address's string pieces (`const NAME = "..."`) and LINK_KINDS. */
export function addressForms(source) {
  const pieces = [...source.matchAll(/^const ([A-Z_]+) = "[^"]*";/gm)].map(m => m[1]);
  const links = /export const LINK_KINDS = \[([^\]]*)\]/.exec(source)?.[1];
  if (pieces.length === 0 || links === undefined) throw new Error("app-address.ts holds no address pieces or no LINK_KINDS");
  return { pieces, links: [...links.matchAll(/"([^"]+)"/g)].map(m => m[1]) };
}

/** Every component WorkspaceCenter can put in the centre, and each `data-k` it draws itself. */
export function centreScreens(source) {
  const at = source.indexOf("function WorkspaceCenter()");
  if (at < 0) throw new Error("App.tsx has no WorkspaceCenter");
  const body = source.slice(at, source.indexOf("\n}\n", at));
  return [...new Set([...body.matchAll(/<([A-Z][A-Za-z]+)\b/g)].map(m => m[1]).concat([...body.matchAll(/data-k="([^"]+)"/g)].map(m => m[1])))];
}

/** The empty and first-run states no table in today's source lists, each by the file that draws it. */
export const KEPT = [
  { kind: "empty:EmptyThread", file: "apps/web/src/components/chat/ChatView.tsx", holds: "export function EmptyThread" },
  { kind: "empty:DaemonDown", file: "apps/web/src/components/DaemonDown.tsx", holds: "export function DaemonDown" },
  { kind: "empty:DisconnectedBanner", file: "apps/web/src/shell/DisconnectedBanner.tsx", holds: "export function DisconnectedBanner" },
  { kind: "first:Onboarding", file: "apps/desktop/src/onboarding.html", holds: 'id="start"' },
  { kind: "name:hover", file: "packages/protocol/src/place-name.ts", holds: "if (place.label !== undefined) return place.label;" },
];

/** Every kind the sweep must have a row for, read from today's source. */
export function kindsOf(read = path => readFileSync(path, "utf8")) {
  const { pieces, links } = addressForms(read(SOURCES.address));
  return [
    ...paneKinds(read(SOURCES.panes)).map(k => `pane:${k}`),
    ...pieces.map(p => `hash:${p}`),
    ...links.map(k => `link:${k}`),
    ...centreScreens(read(SOURCES.centre)).map(c => `centre:${c}`),
    ...KEPT.map(k => k.kind),
  ];
}

const pane = (kind, name) => ({ kind: `pane:${kind}`, electron: `SWP-pane-${kind}`, gpui: { args: ["--open", "{thread:panes}", "--pane", name] }, bare: ["centre", "panel"], regions: { panel: null } });
// The New thread page's ASCII field is redrawn every 120 ms over the centre, so no point there holds still.
const NEW_THREAD = { "thread pane": null, panel: null, composer: null };
const DRAWER = { composer: null, "thread pane": { x: 262, y: 300 } };
const ONE_WINDOW = { "sidebar header": null, sidebar: null, "thread header": null, "panel header": null, panel: null, composer: null, "thread pane": { x: 40, y: 760 } };

/** Each kind as the sweep drives it: `electron`, the wave-sweep state that shoots today's app (or `{ skip }`), and
 * `gpui`, the launch that reaches the same state (`args`, with `{thread:<name>}`, `{ws:<project>}` and
 * `{project:<name>}` read off the fixture; `plain` for a launch that names nothing to open; `empty` on a host with no
 * home yet; `steps` typed after it settles), `{ missing }` where the GPUI app has no such screen, or `{ skip }`. `same`
 * names a row that shoots the same state. `bare` names the boxes where a short line alone fails; `regions` moves the
 * surface check's points (surface.mjs). */
export const TABLE = [
  pane("preview", "browser"),
  pane("terminal", "terminal"),
  pane("diff", "changes"),
  { kind: "pane:pr", electron: { skip: "the Pull request pane opens only on a workspace with a pull request (panes.ts onlyWhenAvailable), and the fixture has no GitHub remote" }, gpui: { skip: "as today's" } },
  pane("files", "files"),
  pane("machine", "computer"),
  pane("processes", "processes"),
  pane("agents", "agents"),
  { ...pane("aside", "aside"), gpui: { args: ["--open", "{thread:panes}"], steps: [{ click: [330, 722] }, { type: "/btw what was the last reply about?" }, { key: "Return" }, { wait: 300 }, { key: "Return" }], wait: 4000 } },
  pane("slate", "slate"),
  { kind: "hash:PREFIX", same: "link:workspace" },
  { kind: "hash:THREAD", electron: "SWP-thread", gpui: { args: ["--open", "{thread:short-1}"] } },
  { kind: "hash:SUBAGENT", electron: { skip: "the subagent page is W1-FRM-029, whose drive holds a Task turn on the long thread" }, gpui: { skip: "as today's" } },
  { kind: "hash:NEW", electron: "SWP-new-thread", gpui: { args: ["--screen", "composer:{ws:bench}"] }, regions: NEW_THREAD },
  { kind: "hash:CODE", electron: { skip: "a browser tab's pairing code (W2-DLG-041); neither desktop app reads one" }, gpui: { skip: "as today's" } },
  { kind: "hash:CODE_ALONE", same: "hash:CODE" },
  { kind: "hash:LINK_SCHEME", same: "link:workspace" },
  { kind: "hash:LINK_PREFIX", same: "link:workspace" },
  { kind: "link:thread", same: "hash:THREAD" },
  { kind: "link:workspace", electron: "SWP-link-workspace", gpui: { plain: true, args: ["wsp://workspace/{ws:bench}"] } },
  { kind: "link:project", electron: "SWP-link-project", gpui: { plain: true, args: ["wsp://project/{project:quiet}"] }, regions: NEW_THREAD },
  { kind: "link:settings", electron: "SWP-link-settings", gpui: { plain: true, args: ["wsp://settings/general"] }, regions: { "thread pane": null, composer: null, panel: null, "panel header": null }, bare: [] },
  { kind: "centre:WorkspaceCreation", electron: { skip: "a create held mid-flight, which the fixture cannot hold" }, gpui: { skip: "as today's" } },
  { kind: "centre:FirstRun", electron: "SWP-first-run", gpui: { empty: true, plain: true, steps: [{ key: "Return" }, { wait: 6000 }, { key: "Return" }], wait: 20_000 }, regions: { composer: null, panel: null, "panel header": null }, bare: [] },
  { kind: "centre:ComputerTerminalDrawer", electron: "SWP-drawer-computer", gpui: { plain: true, steps: [{ wait: 1500 }, { key: "ctrl+j" }], wait: 15_000 }, regions: DRAWER },
  { kind: "centre:ProjectHome", electron: "SWP-home", gpui: { plain: true }, regions: NEW_THREAD },
  { kind: "centre:FailedStart", electron: { skip: "a fork on a cloud whose machine never stood" }, gpui: { skip: "as today's" } },
  { kind: "centre:WorkspaceThread", same: "hash:THREAD" },
  { kind: "centre:WorkspaceTerminalDrawer", electron: "SWP-drawer-thread", gpui: { args: ["--open", "{thread:short-1}"], steps: [{ wait: 1500 }, { key: "ctrl+j" }], wait: 15_000 }, regions: DRAWER },
  { kind: "centre:centre-refused", electron: { skip: "a host that refuses the projects list (App.tsx:107)" }, gpui: { missing: "no centre line for a refused projects list: nothing in app/src says PROJECT_WORDS.notRead" } },
  { kind: "empty:EmptyThread", same: "link:project" },
  { kind: "empty:DaemonDown", electron: { skip: "no host operation stops a daemon (W2-UNO-029)" }, gpui: { skip: "as today's" } },
  { kind: "empty:DisconnectedBanner", electron: "SWP-reconnecting", gpui: { missing: "no banner: the window draws no 'wsp is not running' bar (DisconnectedBanner.tsx:13); only the composer's hold says it (composer/model.rs:756)" } },
  { kind: "first:Onboarding", electron: "SWP-onboarding", gpui: { empty: true, plain: true }, regions: ONE_WINDOW, bare: [] },
  { kind: "name:hover", electron: "SWP-hover", gpui: { args: ["--open", "{thread:panes}"], steps: [{ move: [130, 242] }, { wait: 1200 }], wait: 4000, park: false } },
];

/** Whole-window pairs beyond today's kinds, as rows of the same shape. `fixture: "starting"` runs on
 * the host starting-host.mjs seeds (--starting), and `"stood"` on one it seeds with --stood (--stood); the sidebar
 * lists each one's sub-thread under its lead. */
export const PAIRS = [
  { kind: "pair:long-message", electron: "SWP-long-message", gpui: { args: ["--open", "{thread:long-message}"] } },
  { kind: "pair:agents", same: "pane:agents" },
  // The stood sub-thread waits on its unpushed work, so Needs you stands above the tree and its row sits lower.
  { kind: "pair:started", fixture: "stood", electron: "SWP-started", gpui: { args: ["--open", "{thread:lead}"], steps: [{ click: [140, 386] }, { wait: 1500 }, { move: [1000, 600] }], wait: 5000 }, regions: { panel: null } },
  { kind: "pair:starting", fixture: "starting", electron: "SWP-starting", gpui: { args: ["--open", "{thread:lead}"], steps: [{ click: [140, 294] }, { wait: 1500 }, { move: [1000, 600] }], wait: 5000 }, regions: { panel: null } },
];

const rowOf = kind => TABLE.find(r => r.kind === kind) ?? PAIRS.find(r => r.kind === kind);
/** The row that shoots `kind`, following `same`. */
export function shotRow(kind) {
  const seen = new Set();
  let row = rowOf(kind);
  while (row?.same !== undefined) {
    if (seen.has(row.kind)) throw new Error(`${kind}: same goes round in a circle`);
    seen.add(row.kind);
    row = rowOf(row.same);
  }
  if (row === undefined) throw new Error(`no row for ${kind}`);
  return row;
}

/** The launch words of a row's GPUI side, its placeholders read off the fixture: `ctx` holds `state`, `threads`,
 * `workspaces` (by project name) and `projects` (by name). */
export function launchArgs(gpui, ctx) {
  const fill = word =>
    word.replace(/\{(thread|ws|project):([^}]+)\}/g, (_, kind, name) => {
      const got = { thread: ctx.threads, ws: ctx.workspaces, project: ctx.projects }[kind]?.[name];
      if (got === undefined) throw new Error(`the fixture names no ${kind} ${name}`);
      return got;
    });
  return ["--state", ctx.state, ...(gpui.args ?? []).map(fill)];
}

/** The threads baseline.mjs made on a fixture for a sweep state, by the names the launches use, off its ledger. */
export function madeThreads(home) {
  const path = join(home, "parity-made.json");
  const ledger = existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : {};
  const id = ledger[`thread:${LONG_BRIEF}`];
  return id === undefined ? {} : { "long-message": id };
}

/** Why a launch that ended early failed: the app's own refusal of a pane or a screen it has no screen for. */
export function refusal(code, stderr) {
  const named = /no (right panel pane|screen) is named (\S+)/.exec(stderr);
  if (code === 64 && named !== null) return `no screen: desktop-gpui refused ${named[1] === "screen" ? "--screen" : "--pane"} ${named[2]} (exit 64, "${named[0]}")`;
  return `desktop-gpui ended with ${code} before its window showed: ${stderr.trim().split("\n").filter(l => !l.includes("libEGL")).at(-1) ?? "it said nothing"}`;
}

/** Where a short line alone means an empty screen, in CSS px: the centre column and the right panel under their bars. */
export const BOXES = { centre: { x: 260, y: 56, width: 476, height: 740 }, panel: { x: 744, y: 56, width: 532, height: 740 } };

/** The bounding box, in CSS px, of what is drawn in `box` apart from its ground (its most common colour); null for
 * none. A pixel more than `ink` levels off the ground in any channel counts, so the web's grain does not. */
export function inkBox(image, scale, box, ink = 12) {
  const [x0, y0, x1, y1] = [box.x, box.y, box.x + box.width, box.y + box.height].map(v => Math.round(v * scale));
  const count = new Map();
  for (let y = y0; y < y1; y += 4)
    for (let x = x0; x < x1; x += 4) {
      const at = (y * image.width + x) * image.channels;
      const key = (image.pixels[at] << 16) | (image.pixels[at + 1] << 8) | image.pixels[at + 2];
      count.set(key, (count.get(key) ?? 0) + 1);
    }
  const ground = [...count].sort((a, b) => b[1] - a[1])[0][0];
  const g = [(ground >> 16) & 255, (ground >> 8) & 255, ground & 255];
  let [l, t, r, b] = [Infinity, Infinity, -Infinity, -Infinity];
  for (let y = y0; y < y1; y++)
    for (let x = x0; x < x1; x++) {
      const at = (y * image.width + x) * image.channels;
      if (Math.abs(image.pixels[at] - g[0]) > ink || Math.abs(image.pixels[at + 1] - g[1]) > ink || Math.abs(image.pixels[at + 2] - g[2]) > ink) {
        if (x < l) l = x;
        if (x > r) r = x;
        if (y < t) t = y;
        if (y > b) b = y;
      }
    }
  if (r < 0) return null;
  return { x: l / scale, y: t / scale, width: (r - l + 1) / scale, height: (b - t + 1) / scale };
}

/** A box holding a line or two of words and nothing else: "Nothing open here." under its hint, or "Pick a thread". */
export const isBare = ink => ink === null || (ink.width <= 360 && ink.height <= 60);

/** The boxes where GPUI draws only a short line and today's app draws more. */
export function bareFailures(electron, gpui, boxes, scale = SCALE) {
  return boxes.flatMap(name => {
    const [e, g] = [inkBox(electron, scale, BOXES[name]), inkBox(gpui, scale, BOXES[name])];
    if (!isBare(g) || isBare(e)) return [];
    const what = g === null ? "nothing" : `only ${Math.round(g.width)}x${Math.round(g.height)} px of words at ${Math.round(g.x)},${Math.round(g.y)}`;
    return [`bare ${name}: GPUI draws ${what} where today's draws ${Math.round(e.width)}x${Math.round(e.height)} px of screen`];
  });
}

/** A row's standing, which no fuzz figure enters: `reached` (a launch, a window, no refusal, no centre or panel that
 * draws a line or two where today's draws a screen, a GPUI screen for the kind at all) and `surfaces` (every region's
 * ground equal, surface.mjs). Neither says the two windows look alike; the pair image is for the eye. */
export function standingOf({ notReached = [], surface }) {
  const reached = notReached.length === 0;
  const surfaces = surface === undefined ? "not compared" : surface.ok ? "equal" : "differ";
  return { reached, notReached, surfaces, status: `${reached ? "reached" : "not reached"}, surfaces ${surfaces}` };
}

/** Whether a row holds up: reached, and every surface equal. */
export const holds = row => row.reached === true && row.surfaces === "equal";

/** The xdotool words of one step, with points in CSS px. */
export function stepWords(step, win) {
  if (step.key !== undefined) return [["key", "--clearmodifiers", step.key]];
  if (step.type !== undefined) return [["type", "--delay", "40", step.type]];
  if (step.click !== undefined) return [["mousemove", "--window", win, String(step.click[0] * SCALE), String(step.click[1] * SCALE), "click", "1"]];
  if (step.move !== undefined) return [["mousemove", "--window", win, String(step.move[0] * SCALE), String(step.move[1] * SCALE)]];
  if (step.wait !== undefined) return [];
  throw new Error(`a step of ${JSON.stringify(step)}`);
}

export function parseArgs(argv) {
  const known = new Set(["fixture", "app", "electron-app", "out", "only", "theme", "display", "electron-from", "starting", "stood"]);
  const got = {};
  for (let i = 0; i < argv.length; i += 2) {
    const name = argv[i]?.startsWith("--") ? argv[i].slice(2) : undefined;
    if (name === undefined || !known.has(name)) throw new Error(`${USAGE}; ${argv[i]} is not a flag it takes`);
    if (argv[i + 1] === undefined || argv[i + 1].startsWith("--")) throw new Error(`${USAGE}; --${name} takes a value`);
    got[name] = argv[i + 1];
  }
  for (const name of ["fixture", "app", "electron-app", "out"]) if (got[name] === undefined) throw new Error(`${USAGE}; --${name} is missing`);
  if (got.theme !== undefined && !(got.theme in THEMES)) throw new Error(`--theme takes ${Object.keys(THEMES).join(" or ")}, not ${got.theme}`);
  const display = got.display ?? ":97";
  if (!/^:\d+$/.test(display)) throw new Error(`--display takes a display such as :97, not ${display}`);
  return {
    fixture: resolve(got.fixture),
    app: resolve(got.app),
    electronApp: resolve(got["electron-app"]),
    out: resolve(got.out),
    only: got.only,
    themes: got.theme === undefined ? Object.keys(THEMES) : [got.theme],
    display,
    electronFrom: got["electron-from"] === undefined ? undefined : resolve(got["electron-from"]),
    starting: got.starting === undefined ? undefined : resolve(got.starting),
    stood: got.stood === undefined ? undefined : resolve(got.stood),
  };
}

const sleep = ms => new Promise(done => setTimeout(done, ms));
const say = what => console.log(`sweep: ${what}`);

async function main() {
  let args;
  try {
    args = parseArgs(process.argv.slice(2));
  } catch (e) {
    console.error(`sweep: ${e.message}`);
    process.exit(2);
  }
  const refuse = why => (console.error(`sweep: ${why}`), process.exit(2));
  /** A fixture's host as the run drives it, checked before anything launches. */
  const siteOf = (path, kind) => {
    const fixture = JSON.parse(readFileSync(path, "utf8"));
    const stateDir = dirname(fixture.state);
    const home = dirname(stateDir);
    const mark = kind === "main" ? ".wsp-bench-fixture" : ".wsp-starting-fixture";
    if (!existsSync(join(home, mark))) refuse(`${home} has no ${mark} mark, so ${kind === "main" ? "fixture.sh" : "starting-host.mjs"} did not make it`);
    if ((kind === "stood") !== (fixture.stood === true) && kind !== "main") refuse(`${path} is starting-host.mjs's ${fixture.stood ? "--stood" : "starting"} host, not the ${kind} one`);
    if (kind === "main" && fixture.realHost === undefined) refuse(`${path} has no realHost: make the fixture with this checkout's bench/fixture.sh`);
    const hostArgs = existsSync(`/proc/${fixture.pid}/cmdline`) ? readFileSync(`/proc/${fixture.pid}/cmdline`, "utf8").split("\0").filter(Boolean) : [];
    if (!hostArgs.includes(fixture.state)) refuse(`the ${kind} fixture's host (pid ${fixture.pid}) is not running on ${fixture.state}`);
    return { kind, path, fixture, stateDir, hostArgs };
  };
  const sites = { main: siteOf(args.fixture, "main") };
  for (const kind of ["starting", "stood"]) if (args[kind] !== undefined) sites[kind] = siteOf(args[kind], kind);
  const [hostNode, hostBin] = sites.main.hostArgs;
  if (!existsSync(args.app)) refuse(`no app at ${args.app}`);
  if (existsSync(`/tmp/.X11-unix/X${args.display.slice(1)}`)) refuse(`display ${args.display} is taken; pass --display with a free one`);
  const kinds = kindsOf();
  const unlisted = kinds.filter(k => rowOf(k) === undefined);
  const globs = globsOf(args.only);
  const picked = [...TABLE, ...PAIRS].filter(r => globs === undefined || globs.some(g => g.test(r.kind)));
  if (picked.length === 0) refuse(`no row matches ${args.only}`);

  const tmp = mkdtempSync(join(tmpdir(), "parity-sweep-"));
  const scratch = join(tmp, "shots");
  mkdirSync(scratch);
  mkdirSync(join(tmp, "home"));
  mkdirSync(args.out, { recursive: true });
  const electronDir = args.electronFrom ?? join(args.out, "electron");
  const xEnv = { HOME: join(tmp, "home"), PATH: "/usr/bin:/bin", DISPLAY: args.display };
  const tools = xTools(xEnv);
  const { tool, xdo, magick } = tools;
  for (const site of Object.values(sites)) {
    site.host = await connectHost(site.stateDir);
    site.before = (await site.host.op("preferences.get")).preferences;
    site.touched = new Set();
    const { workspaces } = await site.host.op("workspaces.list");
    const { projects } = await site.host.op("projects.list");
    const real = site.fixture.realHost;
    site.ctx = {
      threads: { ...site.fixture.threads, ...(real === undefined ? {} : { panes: real.panesThread }) },
      projects: Object.fromEntries(projects.map(p => [p.name, p.id])),
      workspaces: Object.fromEntries(projects.flatMap(p => workspaces.filter(w => w.project?.id === p.id && w.kind === "local" && w.worktree === undefined).map(w => [p.name, w.id]))),
    };
    // The app keeps its panel, picks and drafts beside the state it runs on; each launch starts from the run's first, as
    // baseline.mjs puts back the page's first storage before each shot.
    const store = join(site.stateDir, "desktop-gpui-local-store.json");
    const first = existsSync(store) ? readFileSync(store) : undefined;
    site.putStoreBack = () => (first === undefined ? rmSync(store, { force: true }) : writeFileSync(store, first));
  }
  const setPrefs = async (op, patch, site) => {
    if (site !== undefined) for (const name of Object.keys(patch)) site.touched.add(name);
    await op("preferences.set", { patch });
  };

  const started = [];
  const children = new Map();
  const stopAll = async () => {
    for (const [child, what] of [...children]) await stopChild(child, what, what === "baseline.mjs" ? 30_000 : 5000);
    for (const [child, what] of started.reverse()) await stopChild(child, what);
    started.length = 0;
  };
  const emptyHomes = new Set();
  /** A host on a fresh, empty, marked home, started as the fixture's host is and with the clean environment. */
  const emptyHome = async () => {
    const h = spawnEmptyHost({ hostNode, hostBin, stub: join(here, "claude-stub.mjs") });
    const { dir, child } = h;
    emptyHomes.add(h);
    for (const end = Date.now() + 30_000; !existsSync(join(dir, "state", "host-token")) || !existsSync(join(dir, "state", "host.lock")); await sleep(100))
      if (child.exitCode !== null || Date.now() > end) throw new Error(`the empty host did not come up: ${readFileSync(join(dir, "up.log"), "utf8").slice(-800)}`);
    await sleep(500);
    h.socket = await connectHost(join(dir, "state"));
    h.stop = async () => {
      emptyHomes.delete(h);
      h.socket?.close();
      await stopChild(child, "the empty host", 20_000);
      rmSync(dir, { recursive: true, force: true });
    };
    return h;
  };
  let ending = false;
  for (const [signal, code] of [["SIGINT", 130], ["SIGTERM", 143], ["SIGHUP", 129]])
    process.on(signal, async () => {
      if (ending) return;
      ending = true;
      say(`${signal}: stopping what this run started`);
      await stopAll();
      for (const h of [...emptyHomes]) await h.stop();
      process.exit(code);
    });

  // The build under test is copied first, since a rebuild of the same path mid-run would mix two builds in one sweep.
  const appCopy = join(tmp, "desktop-gpui");
  copyFileSync(args.app, appCopy);
  chmodSync(appCopy, 0o700);
  const built = statSync(args.app);
  const summary = { reads: "reached: the GPUI side has a screen for the state (a launch, a window, no refusal, no bare centre or panel); surfaces: every region's ground equal. Neither is visual parity: fuzz is information only and the pair image is for the eye.", app: args.app, appBuilt: built.mtime.toISOString(), appBytes: built.size, electronApp: args.electronApp, electron: electronDir, scale: SCALE, window: WINDOW, fixture: args.fixture, starting: args.starting, stood: args.stood, unlisted, rows: [] };
  const writeSummary = () => writeFileSync(join(args.out, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);

  /** Today's side of every picked row on one fixture in one baseline.mjs run; answers how it ended. */
  async function electron(site) {
    const ids = [...new Set(picked.map(r => shotRow(r.kind)).filter(r => (r.fixture ?? "main") === site.kind && typeof r.electron === "string").map(r => r.electron))];
    if (ids.length === 0) return 0;
    const env = launchEnv(tmp, { side: "electron", id: "sweep", theme: "both" }, args.display);
    mkdirSync(env.HOME, { recursive: true });
    const argv = [join(here, "baseline.mjs"), "--wave", "sweep", "--fixture", site.path, "--app", args.electronApp, "--out", electronDir, "--only", ids.join(","), "--scale", String(SCALE), ...(args.themes.length === 1 ? ["--theme", args.themes[0]] : [])];
    const child = spawn(process.execPath, argv, { env, stdio: ["ignore", "inherit", "inherit"] });
    children.set(child, "baseline.mjs");
    const ended = await new Promise(done => child.once("exit", (code, signal) => done(code ?? signal)));
    children.delete(child);
    return ended;
  }

  async function shootGpui(shot, row, theme, file, site) {
    const going = () => {
      if (shot.aborted) throw new Error(`${row.kind} ${theme} was cut off`);
    };
    const g = row.gpui;
    const empty = g.empty ? await emptyHome() : undefined;
    shot.empty = empty;
    await setPrefs(empty?.socket.op ?? site.host.op, { ...THEMES[theme], transparency: false }, empty === undefined ? site : undefined);
    if (empty === undefined) site.putStoreBack();
    Object.assign(site.ctx.threads, madeThreads(dirname(site.stateDir)));
    const argv = launchArgs(g, { ...site.ctx, state: empty?.state ?? site.fixture.state });
    const env = launchEnv(tmp, { side: "gpui", id: row.kind.replace(/[^A-Za-z0-9-]/g, "-"), theme }, args.display);
    mkdirSync(env.HOME, { recursive: true });
    const app = spawn(appCopy, argv, { env, stdio: ["ignore", "ignore", "pipe"] });
    shot.app = app;
    children.set(app, "the app");
    let stderr = "";
    app.stderr.on("data", d => (stderr += d));
    const ended = new Promise(done => app.once("exit", code => (children.delete(app), done(code))));
    say(`${row.kind} ${theme}: launched pid ${app.pid} ${argv.join(" ")}`);
    /** The app's window, raised, once one shows; a reason instead if the app ends or none shows. */
    const windowOf = async () => {
      for (const end = Date.now() + 30_000; ; await sleep(300)) {
        if (app.exitCode !== null) return { reason: refusal(await ended, stderr) };
        if (Date.now() > end) return { reason: "no window named wsp showed within 30 s" };
        if (tool("xdotool", ["search", "--onlyvisible", "--name", "^wsp$"]).stdout.trim() !== "") return { win: raiseApp(xdo) };
      }
    };
    let { win, reason } = await windowOf();
    if (reason !== undefined) return { reason };
    const grab = name => {
      const at = join(scratch, `${name}.png`);
      magick("import", "-window", win, `PNG24:${at}`);
      return at;
    };
    let last = grab("settle-0");
    for (let i = 1, end = Date.now() + 15_000; Date.now() < end; i++) {
      await sleep(700);
      const now = grab(`settle-${i % 2}`);
      if (tools.differing(last, now, "0%") === 0) break;
      last = now;
    }
    going();
    for (const step of g.steps ?? []) {
      for (const words of stepWords(step, win)) xdo(...words);
      await sleep(step.wait ?? 400);
      going();
    }
    if ((g.steps ?? []).length > 0) {
      await sleep(g.wait ?? 3000);
      // Onboarding's Open wsp closes its window and opens the app's, so the window is found again.
      ({ win, reason } = await windowOf());
      if (reason !== undefined) return { reason };
    }
    if (g.park !== false) xdo("mousemove", "2790", "1790");
    await sleep(1200);
    going();
    magick("convert", grab("shot"), "-units", "PixelsPerInch", "-density", String(72 * SCALE), `PNG24:${file}`);
    await stopChild(app, "the app");
    return {};
  }

  try {
    await startDisplay(args.display, xEnv, started);
    let electronEnded = 0;
    if (args.electronFrom === undefined)
      for (const site of Object.values(sites)) {
        say(`today's app on the ${site.kind} fixture: baseline.mjs --wave sweep into ${electronDir}`);
        electronEnded ||= await electron(site);
      }
    const indexPath = join(electronDir, "index.json");
    const index = existsSync(indexPath) ? JSON.parse(readFileSync(indexPath, "utf8")) : [];
    for (const listed of picked) {
      const row = shotRow(listed.kind);
      for (const theme of args.themes) {
        const out = { kind: listed.kind, theme, ...(listed.same !== undefined ? { same: row.kind } : {}) };
        summary.rows.push(out);
        if (listed.same !== undefined && picked.some(r => r.kind === row.kind)) {
          out.status = `shot as ${row.kind}`;
          say(`${listed.kind} ${theme}: shot as ${row.kind}`);
          continue;
        }
        const site = sites[row.fixture ?? "main"];
        if (site === undefined) {
          out.status = "not driven";
          out.reason = `no --${row.fixture} fixture: start one with starting-host.mjs${row.fixture === "stood" ? " --stood" : ""} and pass its out.json`;
          say(`${listed.kind} ${theme}: not driven, ${out.reason}`);
          continue;
        }
        if (typeof row.electron !== "string" && row.gpui.missing === undefined) {
          out.status = "not driven";
          out.reason = row.electron.skip;
          say(`${listed.kind} ${theme}: not driven, ${out.reason}`);
          continue;
        }
        const stem = `${row.electron?.replace?.(/^SWP-/, "") ?? row.kind.replace(/[^A-Za-z0-9-]/g, "-")}-${theme}`;
        const entry = typeof row.electron === "string" ? index.find(e => e.state === row.electron && e.theme === theme) : undefined;
        const electronFile = join(args.out, `${stem}-electron.png`);
        if (entry?.reached === true) {
          copyFileSync(join(electronDir, entry.file), electronFile);
          out.electron = `${stem}-electron.png`;
        } else out.electronReason = typeof row.electron !== "string" ? row.electron.skip : entry?.reason ?? `baseline.mjs ended with ${electronEnded} and left no shot of ${row.electron} ${theme}`;
        const notReached = [];
        let surface;
        if (row.gpui.missing !== undefined) notReached.push(`MISSING: ${row.gpui.missing}`);
        else {
          const gpuiFile = join(args.out, `${stem}-gpui.png`);
          try {
            const got = await deadline(STATE_MS, shot => shootGpui(shot, row, theme, gpuiFile, site), async shot => {
              if (shot.app !== undefined) await stopChild(shot.app, "the app");
            }).finally(async () => {
              for (const h of [...emptyHomes]) await h.stop();
            });
            if (got.reason !== undefined) notReached.push(got.reason);
            else out.gpui = `${stem}-gpui.png`;
          } catch (e) {
            notReached.push(`the GPUI side did not finish: ${e.message}`);
          }
          if (out.electron !== undefined && out.gpui !== undefined) {
            const { shares, size, ...compared } = pairCompare(tools, electronFile, gpuiFile, { out: args.out, stem, scratch, whole: true, regions: row.regions });
            Object.assign(out, { size, fuzz: shares, pair: `${stem}-pair.png`, diff: `${stem}-diff.png` });
            surface = compared.surface;
            out.surface = surface;
            notReached.push(...bareFailures(readShot(readFileSync(electronFile)), readShot(readFileSync(gpuiFile)), row.bare ?? ["centre"]));
          }
        }
        Object.assign(out, standingOf({ notReached, surface }));
        const fuzz = out.fuzz === undefined ? "" : `; fuzz, for information only: ${FUZZ.map(f => `${out.fuzz[f].toFixed(2)}% at ${f}`).join(", ")}`;
        const off = [...out.notReached, ...(out.surfaces === "differ" ? [`surfaces: ${surfaceWords(surface)}`] : [])];
        say(`${listed.kind} ${theme}: ${out.status}${off.length > 0 ? `; ${off.join("; ")}` : ""}${fuzz}${out.electronReason !== undefined ? `; no Electron shot: ${out.electronReason}` : ""}`);
        writeSummary();
      }
    }
  } finally {
    await stopAll();
    for (const h of [...emptyHomes]) await h.stop();
    for (const site of Object.values(sites)) {
      site.putStoreBack?.();
      const patch = Object.fromEntries([...(site.touched ?? [])].map(name => [name, site.before[name] ?? null]));
      if (Object.keys(patch).length > 0) await site.host.op("preferences.set", { patch }).catch(e => say(`putting the ${site.kind} fixture's preferences back: ${e.message}`));
      site.host?.close();
    }
    rmSync(tmp, { recursive: true, force: true });
    writeSummary();
  }
  const driven = summary.rows.filter(r => r.reached !== undefined);
  const failed = driven.filter(r => !holds(r));
  const counts = {};
  for (const r of summary.rows) counts[r.status.startsWith("shot as") ? "shot as another row" : r.status] = (counts[r.status.startsWith("shot as") ? "shot as another row" : r.status] ?? 0) + 1;
  summary.counts = counts;
  writeSummary();
  if (unlisted.length > 0) say(`today's source has kinds with no row here: ${unlisted.join(", ")}`);
  say(`${Object.entries(counts).map(([k, n]) => `${n} ${k}`).join("; ")}; ${failed.length} of ${driven.length} driven rows are not reached or differ in a surface; ${join(args.out, "summary.json")}`);
  process.exit(failed.length > 0 || unlisted.length > 0 ? 1 : 0);
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) await main();
