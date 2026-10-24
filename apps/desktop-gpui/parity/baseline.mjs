// SPDX-License-Identifier: AGPL-3.0-only
// Parity baselines of today's Electron app, driven over the Chrome DevTools Protocol:
//   node baseline.mjs --wave <n> --fixture <fixture.json> --app <executable> --out <dir> [--only <id glob>] [--theme paper|graphite] [--scale <n>]
// It launches the packaged app on the fixture's throwaway state, walks each state ./wave-<n>/states.mjs lists from the
// same home route in each theme, writes <id>-<theme>.png and index.json to --out, and stops the app by its pid tree.
// The fixture's claude is pointed at ./claude-stub.mjs for the run, so a state can script the agent's replies.
import { execFile, spawn, spawnSync } from "node:child_process";
import { existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, readlinkSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { lastWords as lastMarkdown } from "../bench/content.mjs";
import { crop, withPhys } from "./png.mjs";
import { EMPTY_SYSTEM, spawnEmptyHost } from "./wave-2/gpui-shots.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const USAGE = "usage: node baseline.mjs --wave <n> --fixture <fixture.json> --app <executable> --out <dir> [--only <id glob>] [--theme paper|graphite] [--scale <n>]";
const WIDTH = 1280;
const HEIGHT = 800;
const STATE_MS = 120_000;
/** Each theme as the preferences that draw it: the side, and that side's pick. */
const THEMES = {
  paper: { theme: "light", lightTheme: "paper" },
  graphite: { theme: "dark", darkTheme: "graphite" },
};
const MAC = process.platform === "darwin";
const sleep = ms => new Promise(done => setTimeout(done, ms));
const step = what => console.log(`baseline: ${what}`);
const refuse = why => {
  console.error(`baseline: ${why}`);
  process.exit(2);
};

const flag = (name, optional = false) => {
  const at = process.argv.indexOf(`--${name}`);
  if (at >= 0 && process.argv[at + 1] !== undefined) return process.argv[at + 1];
  if (optional) return undefined;
  refuse(`${USAGE}; --${name} is missing`);
};
const wave = flag("wave");
const out = resolve(flag("out"));
const appPath = resolve(flag("app"));
const only = flag("only", true);
const themeFlag = flag("theme", true);
if (themeFlag !== undefined && !(themeFlag in THEMES)) refuse(`--theme takes ${Object.keys(THEMES).join(" or ")}, not ${themeFlag}`);
const themes = themeFlag === undefined ? Object.keys(THEMES) : [themeFlag];
const scaleFlag = flag("scale", true);
if (scaleFlag !== undefined && !(Number(scaleFlag) > 0)) refuse(`--scale takes a number above 0, not ${scaleFlag}`);

// The fixture: a throwaway home fixture.sh made and marked, served by its own host, never the person's.
const fixture = JSON.parse(readFileSync(flag("fixture"), "utf8"));
const stateDir = dirname(fixture.state);
const home = dirname(stateDir);
// fixture.sh marks its home, and starting-host.mjs the one it seeds with a sub-thread still starting on Boat.
if (![".wsp-bench-fixture", ".wsp-starting-fixture"].some(mark => existsSync(join(home, mark)))) refuse(`${home} has no .wsp-bench-fixture or .wsp-starting-fixture mark, so neither fixture.sh nor starting-host.mjs made it, and this driver runs on nothing else`);
/** A process's arguments: exact off /proc where there is one, else as ps joins them. */
const argsOf = pid => {
  if (existsSync(`/proc/${pid}/cmdline`)) return readFileSync(`/proc/${pid}/cmdline`, "utf8").split("\0").filter(Boolean);
  const line = spawnSync("ps", ["-ww", "-o", "args=", "-p", String(pid)], { encoding: "utf8" }).stdout.trim();
  // The host runs as `<node> <.../bin.js> up ...`; either path may hold a space, which ps does not quote.
  const m = /^(.*?\S) (\/.*\/bin\.js) (up .*)$/.exec(line);
  return m === null ? [] : [m[1], m[2], ...m[3].split(" ")];
};
const hostArgs = argsOf(fixture.pid);
if (!hostArgs.includes(fixture.state)) refuse(`the fixture's host (pid ${fixture.pid}) is not running on ${fixture.state}; make a fixture with fixture.sh`);
const [hostNode, hostBin] = hostArgs;
if (!hostBin?.endsWith("bin.js")) refuse(`the fixture's host runs ${hostArgs.join(" ")}, which names no bin.js`);
const port = JSON.parse(readFileSync(join(stateDir, "host.lock"), "utf8")).port;
const token = readFileSync(join(stateDir, "host-token"), "utf8").trim();

// The app refuses a host of another release, so a mismatch is named here before anything launches.
const boot = await (await fetch(`http://127.0.0.1:${port}/`)).text();
const hostRelease = /"version":"([^"]+)"/.exec(boot)?.[1];
const wsPath = /"wsPath":"([^"]+)"/.exec(boot)?.[1] ?? "/ws";
const resources = appPath.includes(".app/Contents/MacOS/") ? join(appPath.slice(0, appPath.indexOf(".app/") + 4), "Contents", "Resources") : join(dirname(appPath), "resources");
const appPackage = join(resources, "app", "package.json");
if (!existsSync(appPackage)) refuse(`no ${appPackage}, so ${appPath} is not a packaged wsp app`);
const appRelease = JSON.parse(readFileSync(appPackage, "utf8")).version;
if (hostRelease !== appRelease) refuse(`the fixture's host is wsp ${hostRelease ?? "of no release its page names"} and the app is wsp ${appRelease}; run fixture.sh from a checkout whose packages/host matches the app`);

const { default: states, setup } = await import(pathToFileURL(join(here, `wave-${wave}`, "states.mjs")).href);
const seenIds = new Set();
for (const s of states) {
  if (typeof s.id !== "string" || (typeof s.go !== "function" && s.skip === undefined)) refuse(`wave ${wave}: every state needs an id and a go function (or a skip), ${JSON.stringify(s.id)} has not`);
  if (seenIds.has(s.id)) refuse(`wave ${wave}: ${s.id} is listed twice`);
  seenIds.add(s.id);
}
const globs = only?.split(",").map(g => new RegExp(`^${g.replace(/[.+^${}()|[\]\\]/g, "\\$&").replaceAll("*", ".*").replaceAll("?", ".")}$`));
const picked = states.filter(s => globs === undefined || globs.some(g => g.test(s.id)));
if (picked.length === 0) refuse(`wave ${wave} has no state matching ${only}`);

/** One socket on the fixture's host, authed with its token: `op(name, params)` answers the reply or throws its error. */
async function hostSocket(port, token, wsPath = "/ws") {
  const ws = new WebSocket(`ws://127.0.0.1:${port}${wsPath}`);
  await new Promise((done, fail) => ((ws.onopen = done), (ws.onerror = () => fail(new Error(`the host refused a socket on port ${port}`)))));
  let next = 0;
  const waiting = new Map();
  ws.onmessage = m => {
    const msg = JSON.parse(m.data);
    if (typeof msg.id === "number") waiting.get(msg.id)?.(msg), waiting.delete(msg.id);
  };
  ws.onclose = () => {
    for (const answer of waiting.values()) answer({ ok: false, error: "the host closed the socket" });
    waiting.clear();
  };
  const op = (name, params = {}) =>
    new Promise((done, fail) => {
      const id = next++;
      const late = setTimeout(() => (waiting.delete(id), fail(new Error(`${name}: no answer in 30 s`))), 30_000);
      waiting.set(id, msg => (clearTimeout(late), msg.ok === false ? fail(new Error(`${name}: ${msg.error}`)) : done(msg)));
      ws.send(JSON.stringify({ id, op: name, ...params }));
    });
  await op("auth", { token });
  return { op, close: () => ws.close() };
}

/** One wsp line through the fixture host's own command line, with none of this shell's environment. */
const cleanPath = [join(home, "bin"), dirname(hostNode), "/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"].join(":");
const cleanEnv = { HOME: home, PATH: cleanPath, LANG: "en_US.UTF-8", WSP_UPDATE_CHECK: "0" };
const run = (...args) =>
  new Promise(done =>
    execFile(hostNode, [hostBin, ...args, "--state", fixture.state], { cwd: home, env: cleanEnv, encoding: "utf8", maxBuffer: 64 << 20, timeout: 120_000 }, (e, stdout, stderr) => done({ code: e ? (e.code ?? 1) : 0, stdout, stderr })),
  );
const wsp = async (...args) => {
  const r = await run(...args);
  if (r.code !== 0) throw new Error(`wsp ${args.join(" ")}: ${(r.stderr || r.stdout).trim()}`);
  return r.stdout;
};

const host = await hostSocket(port, token, wsPath);
const { preferences: baseline } = await host.op("preferences.get");
/** What each state's preference changes, pref() or a click in Settings alike, are put back to after it. */
let current = { ...baseline };
const setPrefs = patch => host.op("preferences.set", { patch });
// preferences.set refuses these two: labs is the host's to say, and a project's icon has its own operation.
const UNPATCHABLE = new Set(["labs", "projectIcon"]);
// A patch names only the entries of these it moves, a null entry dropping one, so they are put back entry by entry.
const RECORDS = new Set(["terminalZoom", "access", "projectLook", "computerLook", "keybindings", "recipeLook", "threadsShut", "agentDefaults", "projectDefaults"]);
// What the record holds when it lacks a key: these clear with null, cloudCardDismissed reads false.
const NULLABLE = new Set(["sidebarWidth", "target", "textSize", "codeSize", "defaultAgent"]);
const LACKING = { cloudCardDismissed: false };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const nonEmpty = patch => Object.values(patch).some(v => typeof v !== "object" || v === null || Object.keys(v).length > 0);
const restorePrefs = async () => {
  const { preferences: now } = await host.op("preferences.get");
  const clear = {};
  const put = {};
  for (const name of new Set([...Object.keys(now), ...Object.keys(current)])) {
    if (UNPATCHABLE.has(name) || same(now[name], current[name])) continue;
    const was = current[name];
    if (RECORDS.has(name)) {
      const [nowRec, wasRec] = [now[name] ?? {}, was ?? {}];
      const moved = [...new Set([...Object.keys(nowRec), ...Object.keys(wasRec)])].filter(id => !same(nowRec[id], wasRec[id]));
      clear[name] = Object.fromEntries(moved.filter(id => id in nowRec).map(id => [id, null]));
      put[name] = Object.fromEntries(moved.filter(id => id in wasRec).map(id => [id, wasRec[id]]));
    } else if (was !== undefined) put[name] = was;
    else if (NULLABLE.has(name)) put[name] = null;
    else if (name in LACKING) put[name] = LACKING[name];
    else step(`the preference ${name} was set during the run and the record has no way to drop it`);
  }
  if (nonEmpty(clear)) await setPrefs(clear);
  if (nonEmpty(put)) await setPrefs(put);
};
// What a run made, by name, kept in the fixture's folder so a later run on the same fixture makes none of it again.
const ledgerPath = join(home, "parity-made.json");
const ledger = existsSync(ledgerPath) ? JSON.parse(readFileSync(ledgerPath, "utf8")) : {};
const keep = () => writeFileSync(ledgerPath, `${JSON.stringify(ledger, null, 2)}\n`);
const turnsPath = join(home, "parity-turns.json");
/** Scripts what the stub plays for a message holding `prompt`. */
const script = (prompt, reply) => {
  const turns = existsSync(turnsPath) ? JSON.parse(readFileSync(turnsPath, "utf8")) : {};
  turns[prompt] = reply;
  writeFileSync(turnsPath, `${JSON.stringify(turns, null, 2)}\n`);
};
const partsOf = reply => (Array.isArray(reply) ? reply : [reply]).map(p => (typeof p === "string" ? { kind: p } : p));
const HELD = new Set(["question", "permission", "hold"]);
const holds = reply => partsOf(reply).some(p => HELD.has(p.kind));
const threadList = async () =>
  (await wsp("threads", "--json"))
    .split("\n")
    .filter(l => l.trim() !== "")
    .flatMap(l => {
      const v = JSON.parse(l);
      return v.threads ?? [v];
    });
const threadOf = async id => (await threadList()).find(t => t.id === id || t.threadId === id);
/** A thread by the fixture's name for it (short-1), the prompt a state made it with, or its id. */
const idOf = which => fixture.threads[which] ?? ledger[`thread:${which}`] ?? which;
const waitThread = async (id, what, fits, ms = 30_000) => {
  for (const end = Date.now() + ms; ; await sleep(250)) {
    const t = await threadOf(id);
    if (t !== undefined && fits(t)) return t;
    if (Date.now() > end) throw new Error(`thread ${id} was not ${what} in ${ms / 1000} s (it is ${t?.status ?? "not listed"})`);
  }
};
/** The threads this run left a turn running on, stopped when it ends. */
const running = new Set();
const heldOn = async (id, reply) => {
  running.add(id);
  const asks = partsOf(reply).some(p => p.kind === "question" || p.kind === "permission");
  await waitThread(id, asks ? "asking" : "running", t => t.status === "running" && (!asks || t.asking !== undefined));
};
const once = async (name, make = MAKERS[name]) => {
  if (ledger[name] !== undefined) return ledger[name];
  if (make === undefined) throw new Error(`once: nothing makes ${name}`);
  ledger[name] = (await make()) ?? true;
  keep();
  return ledger[name];
};
const threadIdIn = said => /\bthread ([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b/.exec(said)?.[1];
/** A thread on `project` whose first message is `prompt` and whose agent plays `reply`; made once, answers its id. */
const thread = (prompt, { project = "bench", reply = "markdown", title, access, file } = {}) =>
  once(`thread:${prompt}`, async () => {
    script(prompt, reply);
    const held = holds(reply);
    // A permission prompt reaches the window only on a thread that asks; a bypass thread's adapter allows it unseen.
    access ??= partsOf(reply).some(p => p.kind === "permission") ? "ask" : undefined;
    const flags = [...(title !== undefined ? ["--title", title] : []), ...(access !== undefined ? ["--access", access] : []), ...(file !== undefined ? ["--file", file] : []), ...(held ? ["--detach"] : [])];
    const r = await run("run", project, "--agent", "claude", ...flags, prompt);
    const id = threadIdIn(`${r.stdout}\n${r.stderr}`);
    if (id === undefined) throw new Error(`wsp run printed no thread: ${(r.stderr || r.stdout).trim()}`);
    if (held) await heldOn(id, reply);
    else await waitThread(id, "finished", t => t.status !== "running");
    return id;
  });
/** One more turn on a thread, its agent playing `reply`; made once per thread and prompt. */
const send = (which, prompt, { reply = "markdown" } = {}) =>
  once(`send:${idOf(which)}:${prompt}`, async () => {
    const id = idOf(which);
    script(prompt, reply);
    const held = holds(reply);
    await wsp("send", id, ...(held ? ["--detach"] : []), prompt);
    if (held) await heldOn(id, reply);
    else await waitThread(id, "finished", t => t.status !== "running");
    return id;
  });
/** A turn left running on a thread, playing `reply` and then holding; nothing new where one already runs there. */
const working = async (which, { reply = [], prompt = "Run the tests again and fix what fails." } = {}) => {
  const id = idOf(which);
  if ((await threadOf(id))?.status === "running") return id;
  const spec = partsOf(reply);
  if (spec.length === 0) spec.push({ kind: "text", text: "I will run the tests first." });
  if (!holds(spec)) spec.push({ kind: "hold" });
  script(prompt, spec);
  await wsp("send", id, "--detach", prompt);
  await heldOn(id, spec);
  return id;
};
/** Stops the thread's running turn, as the app's Stop does. */
const stop = async which => {
  const id = idOf(which);
  running.delete(id);
  if ((await threadOf(id))?.status !== "running") return;
  await run("stop", id);
  await waitThread(id, "stopped", t => t.status !== "running");
};
/** Deletes a thread a state made, and forgets it, so the next shot makes it again. */
const removeThread = async which => {
  const id = idOf(which);
  await stop(id);
  // Through the socket: wsp delete first asks the computer's daemon about processes, and a fixture may have none.
  await host.op("sessions.delete", { threadId: id });
  for (const [name, value] of Object.entries(ledger)) if (value === id) delete ledger[name];
  keep();
};
/** A project on a new git folder in the fixture's folder; made once, answers its name. */
const project = name =>
  once(`project:${name}`, async () => {
    const folder = join(home, "projects", `${Object.keys(ledger).length}-${name.replace(/[^A-Za-z0-9._-]+/g, "-").slice(0, 40)}`);
    mkdirSync(folder, { recursive: true });
    const git = (...args) => spawnSync("git", ["-c", "user.name=parity", "-c", "user.email=parity@example.com", "-c", "commit.gpgsign=false", ...args], { cwd: folder, env: cleanEnv, stdio: "ignore" });
    git("init", "-q", "-b", "main");
    git("commit", "-q", "--allow-empty", "-m", "parity");
    await wsp("add", folder, "--name", name);
    return name;
  });
const projectRows = async () =>
  (await wsp("projects", "--json"))
    .split("\n")
    .filter(l => l.trim() !== "")
    .flatMap(l => {
      const v = JSON.parse(l);
      return v.projects ?? [v];
    });
/** Takes a project off the host by its name, and out of what this run made. */
const removeProject = async name => {
  const row = (await projectRows()).find(p => p.name === name);
  if (row !== undefined) await host.op("projects.remove", { projectId: row.id, force: true });
  delete ledger[`project:${name}`];
  keep();
};
/** What `once(name)` makes for the names states share. */
const MAKERS = {
  manyThreads: async () => {
    const ids = [];
    for (let i = 1; i <= 14; i++) ids.push(await thread(`Check sidebar case ${i} and say what you find.`, { reply: { kind: "text", text: `Case ${i} reads fine.` } }));
    return ids;
  },
  longTitle: async () => {
    const name = "a project whose name runs well past the sidebar edge, 60 chars".slice(0, 60);
    await project(name);
    const title = "A thread title long enough to run past every edge the app gives it, so the tile, the crumb and the card each have to cut it somewhere; it keeps going with more words until two hundred.".padEnd(200, " x").slice(0, 200);
    const id = await thread("Read the long names and report back.", { project: name, title, reply: { kind: "text", text: "The long names read fine." } });
    return { project: name, thread: id, title };
  },
  second: () => project("second"),
};
const hostHelpers = { wsp, op: host.op, once, pref: (name, value) => setPrefs({ [name]: value }), script, thread, send, working, stop, removeThread, project, removeProject, threads: threadList, fixture, home };

// The fixture's claude plays the parity stub for the run; the link it had is kept in the ledger and put back at the end.
const claudeLink = join(home, "bin", "claude");
const parityStub = join(here, "claude-stub.mjs");
const hasClaude = (() => {
  try {
    return lstatSync(claudeLink).isSymbolicLink();
  } catch {
    return false;
  }
})();
if (hasClaude && readlinkSync(claudeLink) !== parityStub) {
  ledger["claude:fixture"] = readlinkSync(claudeLink);
  keep();
  rmSync(claudeLink);
  symlinkSync(parityStub, claudeLink);
}
// The host keeps each thread's agent up 30 minutes after its turn and hands it the next message whatever the link
// says; one ended while idle is dropped with no failed status, so each kept agent of the fixture's claude is ended.
const runsDir = join(stateDir, "runs");
for (const name of existsSync(runsDir) ? readdirSync(runsDir) : []) {
  if (!name.endsWith(".agent") || existsSync(join(runsDir, name.replace(/\.agent$/, ".exit")))) continue;
  const pid = Number(readFileSync(join(runsDir, name), "utf8").trim());
  if (!(pid > 0) || !spawnSync("ps", ["-ww", "-o", "args=", "-p", String(pid)], { encoding: "utf8" }).stdout.includes(claudeLink)) continue;
  process.kill(pid, "SIGTERM");
  step(`ended the kept agent ${pid} (${name})`);
}

const SYSTEM_PATH = ["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"];
const treeOf = root => {
  const rows = spawnSync("ps", ["-A", "-o", "pid=", "-o", "ppid="], { encoding: "utf8" }).stdout.trim().split("\n").map(l => l.trim().split(/\s+/).map(Number));
  const keep = new Set([root]);
  for (let grew = true; grew; ) {
    grew = false;
    for (const [pid, ppid] of rows) if (!keep.has(pid) && keep.has(ppid)) keep.add(pid), (grew = true);
  }
  return [...keep];
};
const alive = pid => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
const argsLine = pid => spawnSync("ps", ["-ww", "-o", "args=", "-p", String(pid)], { encoding: "utf8" }).stdout;
const bundle = appPath.includes(".app/") ? appPath.slice(0, appPath.indexOf(".app/") + 4) : dirname(appPath);

/** The app this run has open; there is never more than one. */
let active;
/** Launches the app on a wsp home (`wspHome` its state's folder) with the clean environment electron.mjs gives it. */
function launchApp({ wspHome, ownHome, system = [dirname(hostNode), ...SYSTEM_PATH] }) {
  const appHome = join(ownHome, "parity-home");
  const managers = join(appHome, "bin");
  mkdirSync(managers, { recursive: true });
  // With no host serving its state the app installs a launchd or systemd service in the person's own login; these refuse.
  for (const name of ["launchctl", "systemctl"]) writeFileSync(join(managers, name), `#!/bin/sh\necho "the parity driver refuses ${name} $*" >&2\nexit 1\n`, { mode: 0o755 });
  const shell = join(appHome, "login-shell");
  const path = [managers, join(ownHome, "bin"), ...system].join(":");
  // The app reads PATH off the login shell it is handed; this one hands it the home's, its claude first.
  writeFileSync(shell, `#!/bin/sh\nprintf 'PATH=%s\\0' ${JSON.stringify(path)}\n`, { mode: 0o755 });
  const env = { HOME: appHome, WSP_HOME: wspHome, PATH: path, LANG: "en_US.UTF-8", SHELL: shell, WSP_DESKTOP_SMOKE: "1" };
  for (const name of ["DISPLAY", "XAUTHORITY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"]) if (process.env[name] !== undefined) env[name] = process.env[name];
  step(`launching ${appPath} (wsp ${appRelease}) on ${wspHome}`);
  // Chromium refuses to run as root with its sandbox on, which is how a Boat machine runs and a Mac never does.
  // The scale is the display's own, never emulated: emulation rounds Blink's font metrics in CSS px, unlike a Retina Mac.
  const child = spawn(
    appPath,
    ["--remote-debugging-port=0", "--inspect=0", ...(process.getuid?.() === 0 ? ["--no-sandbox"] : []), ...(scaleFlag !== undefined ? [`--force-device-scale-factor=${scaleFlag}`] : [])],
    { cwd: appHome, env, stdio: ["ignore", "pipe", "pipe"] },
  );
  child.once("error", e => refuse(`${appPath} did not start: ${e.message}`));
  const said = [];
  child.stdout.on("data", chunk => said.push(chunk.toString()));
  child.stderr.on("data", chunk => said.push(chunk.toString()));
  const printed = (pattern, what) =>
    new Promise((done, fail) => {
      const read = () => {
        const found = pattern.exec(said.join(""))?.[1];
        if (found !== undefined) done(found);
      };
      child.stdout.on("data", read);
      child.stderr.on("data", read);
      child.on("exit", code =>
        // The app quits at once, 0, when another app already holds this home's single instance lock.
        fail(new Error(code === 0 ? `the app quit before ${what}: another wsp app is open on ${wspHome}, close it first` : `the app exited (${code}) before ${what}:\n${said.join("").slice(-3000)}`)),
      );
    });
  const devtools = printed(/DevTools listening on ws:\/\/[^:]+:(\d+)\//, "DevTools listened").then(Number);
  const inspector = printed(/Debugger listening on (ws:\/\/\S+)/, "its inspector listened");
  devtools.catch(() => {});
  inspector.catch(() => {});
  active = { child, ownHome, tree: [], devtools, inspector, mainProcess: undefined, stopping: undefined };
  return active;
}
/** Stops an app by its pid tree; a pid seen under it earlier is signalled only while it still runs the app's files or
 * its home's, since a pid freed since may be anyone's. */
function stopApp(a = active) {
  if (a === undefined) return Promise.resolve();
  return (a.stopping ??= (async () => {
    a.mainProcess?.close();
    a.tree = [...new Set([...a.tree, ...(alive(a.child.pid) ? treeOf(a.child.pid) : [])])];
    if (alive(a.child.pid)) a.child.kill("SIGTERM");
    for (const until = Date.now() + 10_000; alive(a.child.pid) && Date.now() < until; ) await sleep(100);
    for (const pid of a.tree.filter(alive)) {
      const args = argsLine(pid);
      if (args.includes(bundle) || args.includes(a.ownHome)) process.kill(pid, "SIGKILL");
    }
  })());
}

/** The empty hosts this run started, stopped by their pids when the run ends either way. */
const emptyHosts = new Set();
/** Puts the fixture's preferences back as the run found them, theme included, and stops the app; once, either way. */
let finishing;
const finish = () =>
  (finishing ??= (async () => {
    await stopApp();
    for (const h of emptyHosts) await h.stop();
    for (const id of running) await run("stop", id);
    if (ledger["claude:fixture"] !== undefined) {
      rmSync(claudeLink, { force: true });
      symlinkSync(ledger["claude:fixture"], claudeLink);
    }
    current = { ...baseline };
    await restorePrefs().catch(e => console.error(`baseline: putting the fixture's preferences back: ${e.message}`));
    host.close();
  })());
let interrupted = false;
for (const [signal, code] of [
  ["SIGINT", 130],
  ["SIGTERM", 143],
  ["SIGHUP", 129],
])
  process.once(signal, () => {
    interrupted = true;
    void finish().finally(() => process.exit(code));
  });

/** A Chrome DevTools Protocol session on a websocket: `send(method, params)` answers the result or throws. */
async function session(url) {
  const ws = new WebSocket(url);
  await new Promise((done, fail) => ((ws.onopen = done), (ws.onerror = fail)));
  let next = 0;
  const waiting = new Map();
  const heard = new Set();
  ws.onmessage = m => {
    const msg = JSON.parse(m.data);
    if (msg.id !== undefined) waiting.get(msg.id)?.(msg), waiting.delete(msg.id);
    else for (const hear of heard) hear(msg);
  };
  ws.onclose = () => {
    for (const answer of waiting.values()) answer({ error: { message: "the DevTools socket closed" } });
    waiting.clear();
  };
  const send = (method, params = {}) =>
    new Promise((done, fail) => {
      const id = ++next;
      const late = setTimeout(() => (waiting.delete(id), fail(new Error(`${method}: no answer in 30 s`))), 30_000);
      waiting.set(id, msg => (clearTimeout(late), msg.error ? fail(new Error(`${method}: ${msg.error.message}`)) : done(msg.result)));
      ws.send(JSON.stringify({ id, method, params }));
    });
  /** Calls `fn` with each `method` event's params until the answered function is called. */
  const on = (method, fn) => {
    const hear = msg => msg.method === method && fn(msg.params);
    heard.add(hear);
    return () => heard.delete(hear);
  };
  return { send, on, close: () => ws.close() };
}

/** The windows a state can drive, by the address their page shows; the main process finds its BrowserWindow by the
 * same test the DevTools target is found by. The pair window is the host's page opened with no preload. */
const WINDOWS = {
  app: url => url.startsWith("http://127.0.0.1:") && !url.includes("parity-pair"),
  pair: url => url.startsWith("http://127.0.0.1:") && url.includes("parity-pair"),
  onboarding: url => url.startsWith("file:") && url.endsWith("onboarding.html"),
  starting: url => url.startsWith("file:") && url.endsWith("starting.html"),
};

/** Runs an expression in the app's main process on the window `kind` names, as `w`. */
async function onWindow(body, kind = "app") {
  if (active.mainProcess === undefined) {
    const url = await Promise.race([active.inspector, sleep(10_000)]);
    if (url === undefined) throw new Error("the app printed no inspector address in 10 s, so --inspect was not taken");
    active.mainProcess = await session(url);
  }
  const got = await active.mainProcess.send("Runtime.evaluate", {
    returnByValue: true,
    // The main module is set once the app's entry has loaded; before that there is no window.
    expression: `(() => {
      if (process.mainModule === undefined) return undefined;
      const { app, BrowserWindow } = process.mainModule.require("electron");
      const w = BrowserWindow.getAllWindows().find(w => (${WINDOWS[kind]})(w.webContents.getURL()));
      if (w === undefined) return undefined;
      ${body}
    })()`,
  });
  if (got.exceptionDetails !== undefined) throw new Error(`in the main process: ${got.exceptionDetails.exception?.description ?? got.exceptionDetails.text}`);
  return got.result.value;
}

// A Mac opens a scripted launch behind the owner's app and Chromium throttles a hidden window; LaunchServices cannot tell the two apps apart.
async function raise(kind = "app") {
  for (const until = Date.now() + 30_000; ; await sleep(250)) {
    const shown = await onWindow(
      `
      if (process.platform === "darwin") app.focus({ steal: true });
      if (w.isMinimized()) w.restore();
      w.show();
      w.moveTop();
      w.focus();
      return { visible: w.isVisible(), focused: w.isFocused() };`,
      kind,
    );
    if (shown !== undefined) return shown;
    if (Date.now() > until) throw new Error(`the app showed no ${kind} window in 30 s`);
  }
}

/** A DevTools session on the active app's page that `kind` names, once one shows. */
async function attach(kind, ms = 60_000) {
  const port = await active.devtools;
  for (const until = Date.now() + ms; ; await sleep(250)) {
    const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    const target = pages.find(p => p.type === "page" && WINDOWS[kind](p.url));
    if (target !== undefined) return session(target.webSocketDebuggerUrl);
    if (Date.now() > until) throw new Error(`no ${kind} window in ${ms / 1000} s (pages: ${pages.map(p => p.url).join(", ") || "none"})`);
  }
}

/** Every helper a state's go is handed, for the page `tab` shows in the window `win` names (WINDOWS). */
function tools(tab, win) {
  const cdp = tab.send;
  /** Runs a function in the page with JSON arguments and answers what it returned. */
  const page = async (fn, ...args) => {
    const got = await cdp("Runtime.evaluate", { expression: `(${fn})(...${JSON.stringify(args)})`, awaitPromise: true, returnByValue: true });
    if (got.exceptionDetails !== undefined) throw new Error(got.exceptionDetails.exception?.description ?? got.exceptionDetails.text);
    return got.result.value;
  };
  const waitFor = async (what, fn, ms = 10_000, ...args) => {
    for (const until = Date.now() + ms; ; await sleep(100)) {
      const got = await page(fn, ...args);
      if (got) return got;
      if (Date.now() > until) throw new Error(`${what} not seen in ${ms / 1000} s`);
    }
  };

  /** Where `target` is, waited for up to 5 s: a selector if it starts with [ . # or css:, else the words on it. */
  const locate = async target => {
    for (const until = Date.now() + 5_000; ; await sleep(100)) {
      const at = await find(target);
      if (at !== null) return at;
      if (Date.now() > until) throw new Error(`nothing on the page for ${JSON.stringify(target)} in 5 s`);
    }
  };
  const find = target =>
    page(target => {
      const visible = el => {
        const r = el.getBoundingClientRect();
        const s = getComputedStyle(el);
        return r.width > 0 && r.height > 0 && s.visibility !== "hidden" && s.display !== "none";
      };
      const norm = s => (s ?? "").replace(/\s+/g, " ").trim().toLowerCase();
      const control = "button, a[href], input, select, textarea, summary, label, [role=button], [role=menuitem], [role=menuitemcheckbox], [role=menuitemradio], [role=option], [role=tab], [role=checkbox], [role=switch], [role=radio], [role=link], [data-sidebar-row]";
      let found;
      if (/^(\[|\.|#|css:)/.test(target)) found = [...document.querySelectorAll(target.replace(/^css:/, ""))];
      else {
        const words = norm(target);
        const labelled = [...document.body.querySelectorAll("[aria-label], [title]")].filter(el => norm(el.getAttribute("aria-label")) === words || norm(el.getAttribute("title")) === words);
        const holders = [];
        const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        for (let node = walk.nextNode(); node !== null; node = walk.nextNode()) if (norm(node.data).includes(words)) holders.push(node.parentElement);
        // Words split over several elements: the deepest element whose text holds them all.
        if (holders.length === 0) {
          const all = [...document.body.querySelectorAll("*")].filter(el => norm(el.textContent).includes(words));
          holders.push(...all.filter(el => !all.some(other => other !== el && el.contains(other))));
        }
        found = [...labelled, ...holders.filter(el => norm(el.textContent) === words), ...holders];
      }
      const el = found.filter(visible).map(el => el.closest(control) ?? el)[0];
      if (el === undefined) return null;
      el.scrollIntoView({ block: "nearest", inline: "nearest" });
      const r = el.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    }, target);
  const mouse = (type, x, y, more = {}) => cdp("Input.dispatchMouseEvent", { type, x, y, ...more });
  const nativeMenus = () => onWindow("return globalThis.parityMenus.splice(0);", win);
  const click = async (target, { count = 1, button = "left" } = {}) => {
    const { x, y } = await locate(target);
    if (button === "right") await nativeMenus();
    await mouse("mouseMoved", x, y);
    for (let i = 1; i <= count; i++) {
      await mouse("mousePressed", x, y, { button, clickCount: i });
      await mouse("mouseReleased", x, y, { button, clickCount: i });
    }
    if (button !== "right") return;
    await sleep(300);
    const asked = await nativeMenus();
    if (asked.length > 0) throw new Error(`the app draws this menu natively through window.wsp.contextMenu, which a page capture does not hold (its rows: ${asked.flat().join(", ")})`);
  };
  /** Right-clicks `target` and picks rows of the native menu by their words, one per menu it opens in turn. */
  const menu = async (target, ...rows) => {
    await nativeMenus();
    await onWindow(`globalThis.parityPicks = ${JSON.stringify(rows)}; return true;`, win);
    const { x, y } = await locate(target);
    await mouse("mouseMoved", x, y);
    await mouse("mousePressed", x, y, { button: "right", clickCount: 1 });
    await mouse("mouseReleased", x, y, { button: "right", clickCount: 1 });
    await sleep(500);
    const asked = await nativeMenus();
    await onWindow("globalThis.parityPicks = []; return true;", win);
    if (asked.length === 0) throw new Error(`no native menu opened on ${JSON.stringify(target)}`);
    for (const [i, row] of rows.entries()) if (!asked[i]?.includes(row)) throw new Error(`the menu on ${JSON.stringify(target)} has no row ${JSON.stringify(row)} (its rows: ${(asked[i] ?? []).join(", ")})`);
  };
  /** Rests the pointer on what `target` names for `wait` ms: a tooltip opens after 600, a thread's card after 450. */
  const hover = async (target, { wait = 800 } = {}) => {
    const { x, y } = await locate(target);
    await mouse("mouseMoved", x, y);
    await sleep(wait);
  };
  /** Takes the pointer off the window, so no row is hovered and no tooltip is open in a capture that did not ask. */
  const park = () => mouse("mouseMoved", -10, -10);
  /** Clicks what a selector finds through the DOM, with no pointer over it: how the routes move, so no hover card opens. */
  const press = selector =>
    page(selector => {
      const el = document.querySelector(selector);
      if (el === null) throw new Error(`nothing on the page for ${selector}`);
      el.click();
    }, selector);

  const NAMED = { escape: ["Escape", 27], enter: ["Enter", 13, "\r"], tab: ["Tab", 9], backspace: ["Backspace", 8], delete: ["Delete", 46], space: [" ", 32, " ", "Space"], arrowup: ["ArrowUp", 38], arrowdown: ["ArrowDown", 40], arrowleft: ["ArrowLeft", 37], arrowright: ["ArrowRight", 39], home: ["Home", 36], end: ["End", 35], pageup: ["PageUp", 33], pagedown: ["PageDown", 34] };
  const PUNCT = { ",": ["Comma", 188], ".": ["Period", 190], "/": ["Slash", 191], ";": ["Semicolon", 186], "'": ["Quote", 222], "[": ["BracketLeft", 219], "]": ["BracketRight", 221], "\\": ["Backslash", 220], "-": ["Minus", 189], "=": ["Equal", 187], "`": ["Backquote", 192] };
  const MODS = { alt: ["Alt", "AltLeft", 18, 1], ctrl: ["Control", "ControlLeft", 17, 2], meta: ["Meta", "MetaLeft", 91, 4], shift: ["Shift", "ShiftLeft", 16, 8] };
  const modName = m => ({ mod: MAC ? "meta" : "ctrl", control: "ctrl", cmd: "meta", option: "alt" })[m] ?? m;
  /** A chord as the keybinding table spells it ("mod+shift+k", "escape", "mod+,", "ctrl"): its modifiers and its key. */
  const chordOf = chord => {
    const parts = chord.toLowerCase().split(/\+(?!$)/).map(modName);
    const name = parts.pop();
    for (const m of parts) if (MODS[m] === undefined) throw new Error(`key ${chord}: no modifier ${m}`);
    const shift = parts.includes("shift");
    let k;
    if (MODS[name] !== undefined) k = { key: MODS[name][0], code: MODS[name][1], windowsVirtualKeyCode: MODS[name][2], bit: MODS[name][3] };
    else if (NAMED[name === "esc" ? "escape" : name] !== undefined) {
      const [key, code, text, codeName] = NAMED[name === "esc" ? "escape" : name];
      k = { key, code: codeName ?? key, windowsVirtualKeyCode: code, ...(text !== undefined ? { text } : {}) };
    } else if (/^f([1-9]|1[0-2])$/.test(name)) k = { key: name.toUpperCase(), code: name.toUpperCase(), windowsVirtualKeyCode: 111 + Number(name.slice(1)) };
    else if (/^[a-z]$/.test(name)) k = { key: shift ? name.toUpperCase() : name, code: `Key${name.toUpperCase()}`, windowsVirtualKeyCode: name.toUpperCase().charCodeAt(0), text: shift ? name.toUpperCase() : name };
    else if (/^[0-9]$/.test(name)) k = { key: name, code: `Digit${name}`, windowsVirtualKeyCode: 48 + Number(name), text: name };
    else if (PUNCT[name] !== undefined) k = { key: name, code: PUNCT[name][0], windowsVirtualKeyCode: PUNCT[name][1], text: name };
    else throw new Error(`key ${chord}: no key ${name}`);
    return { mods: parts.map(m => MODS[m]), k };
  };
  /** The modifiers held down now, and the chords keyDown left down, released by the next reset. */
  let pressed = 0;
  const heldChords = [];
  const keyEvent = (type, k, extra = {}) => cdp("Input.dispatchKeyEvent", { type, modifiers: pressed, key: k.key, code: k.code, windowsVirtualKeyCode: k.windowsVirtualKeyCode, ...extra });
  /** Presses a chord's modifiers and then its key and holds them; mod is Cmd on a Mac, else Ctrl. */
  const keyDown = async chord => {
    const { mods, k } = chordOf(chord);
    for (const [key, code, windowsVirtualKeyCode, bit] of mods) {
      pressed |= bit;
      await keyEvent("rawKeyDown", { key, code, windowsVirtualKeyCode });
    }
    if (k.bit !== undefined) pressed |= k.bit;
    // A chord with Ctrl, Alt or Cmd types nothing, so its key goes down raw, without its text.
    const typed = (pressed & 7) === 0 && k.text !== undefined;
    await keyEvent(typed ? "keyDown" : "rawKeyDown", k, typed ? { text: k.text, unmodifiedText: k.text } : {});
    heldChords.push(chord);
  };
  const keyUp = async chord => {
    const { mods, k } = chordOf(chord);
    await keyEvent("keyUp", k);
    if (k.bit !== undefined) pressed &= ~k.bit;
    for (const [key, code, windowsVirtualKeyCode, bit] of mods.reverse()) {
      pressed &= ~bit;
      await keyEvent("keyUp", { key, code, windowsVirtualKeyCode });
    }
    const at = heldChords.lastIndexOf(chord);
    if (at >= 0) heldChords.splice(at, 1);
  };
  const key = async chord => {
    await keyDown(chord);
    await keyUp(chord);
  };
  const type = text => cdp("Input.insertText", { text });
  /** Turns the wheel over what `target` names, `dy` pixels down (negative is up). */
  const scroll = async (target, dy = 400) => {
    const { x, y } = await locate(target);
    await mouse("mouseWheel", x, y, { deltaX: 0, deltaY: dy });
    await sleep(300);
  };

  /** A drag the last state left held, let go by the next reset. */
  let dragHeld;
  /** Presses on `from`, moves the pointer to `to` (a target or { x, y }) and lets go, or with hold keeps it there. An
   * HTML drag (a draggable tile) is played as the drag events Chromium hands over; anything else is plain moves. */
  const drag = async (from, to, { hold = false, steps = 12 } = {}) => {
    const a = await locate(from);
    const b = typeof to === "object" && "x" in to ? to : await locate(to);
    let data;
    const off = tab.on("Input.dragIntercepted", p => (data = p.data));
    await cdp("Input.setInterceptDrags", { enabled: true });
    try {
      await mouse("mouseMoved", a.x, a.y);
      await mouse("mousePressed", a.x, a.y, { button: "left", clickCount: 1 });
      for (let i = 1; i <= steps && data === undefined; i++) {
        await mouse("mouseMoved", a.x + ((b.x - a.x) * i) / steps, a.y + ((b.y - a.y) * i) / steps, { button: "left", buttons: 1 });
        await sleep(16);
      }
      if (data !== undefined) {
        for (const type of ["dragEnter", "dragOver"]) await cdp("Input.dispatchDragEvent", { type, x: b.x, y: b.y, data });
        if (!hold) {
          await cdp("Input.dispatchDragEvent", { type: "drop", x: b.x, y: b.y, data });
          await mouse("mouseReleased", b.x, b.y, { button: "left", clickCount: 1 });
        }
      } else if (!hold) await mouse("mouseReleased", b.x, b.y, { button: "left", clickCount: 1 });
      if (hold) dragHeld = { ...b, data };
    } finally {
      off();
      await cdp("Input.setInterceptDrags", { enabled: false });
    }
  };
  const letGo = async () => {
    if (dragHeld === undefined) return;
    const { x, y, data } = dragHeld;
    dragHeld = undefined;
    if (data !== undefined) await cdp("Input.dispatchDragEvent", { type: "dragCancel", x, y, data });
    await mouse("mouseReleased", x, y, { button: "left", clickCount: 1 });
  };
  /** Drops files from this computer on what `target` names, as a drag from the file manager does. */
  const dropFiles = async (target, paths) => {
    const { x, y } = await locate(target);
    const data = { items: [], files: paths, dragOperationsMask: 1 };
    for (const type of ["dragEnter", "dragOver", "drop"]) await cdp("Input.dispatchDragEvent", { type, x, y, data });
  };

  /** Sets the window's content size through the main process and holds until the page has it. */
  const size = async (width, height) => {
    await onWindow(`
      if (w.isFullScreen()) w.setFullScreen(false);
      if (w.isMaximized()) w.unmaximize();
      w.setContentSize(${width}, ${height});
      return true;`, win);
    try {
      await waitFor(`the page at ${width}x${height}`, ([w, h]) => innerWidth === w && innerHeight === h, 5_000, [width, height]);
    } catch {
      const [w, h] = await page(() => [innerWidth, innerHeight]);
      throw new Error(`the window was set to ${width}x${height} and the page is ${w}x${h}`);
    }
  };

  const threadRow = id => `[data-row-id='thread:${id}']`;
  const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
  const route = {
    /** New thread on the bench project: the home route every state starts from. */
    async home() {
      await press("[data-new-thread-row]:not([aria-disabled]), [data-new-thread-glyph]:not([aria-disabled])");
      await waitFor("the New thread view", () => document.querySelector("[data-k=project-home]") !== null);
    },
    newThread: () => route.home(),
    /** A thread by the fixture's name for it (short-1), its id, or words of its title in the sidebar. */
    async thread(which) {
      const id = fixture.threads[which] ?? (UUID.test(which) ? which : undefined);
      const selector =
        id !== undefined
          ? threadRow(id)
          : await page(words => {
              const row = [...document.querySelectorAll("[data-row-id^='thread:']")].find(el => el.textContent.includes(words));
              return row === undefined ? undefined : `[data-row-id='${row.dataset.rowId}']`;
            }, which);
      if (selector === undefined) throw new Error(`no thread in the sidebar reads ${JSON.stringify(which)}`);
      // A narrow window keeps the sidebar in a shut sheet, whose rows are not drawn until its toggle opens it.
      const sheet = !(await page(sel => document.querySelector(sel) !== null, selector)) && (await page(() => document.querySelector('[data-sidebar="trigger"]') !== null));
      if (sheet) await press('[data-sidebar="trigger"]');
      await waitFor(`the thread row ${which}`, sel => document.querySelector(sel) !== null, 10_000, selector);
      await press(selector);
      if (sheet && (await page(sel => document.querySelector(sel) !== null, selector))) await key("escape");
      await waitFor(`${which} picked in the sidebar`, sel => document.querySelector(sel)?.getAttribute("data-active") === "true", 10_000, selector);
      if (fixture.threads[which] !== undefined) {
        const words = lastMarkdown(which).replaceAll("`", "");
        await waitFor(`${which}'s last words`, words => [...document.querySelectorAll("[data-timeline-row-id]")].some(el => el.textContent.includes(words)), 30_000, words);
      } else {
        await waitFor(`${which}'s transcript`, () => document.querySelectorAll("[data-timeline-row-id]").length > 0, 30_000);
      }
    },
    /** A page of Settings by its id: a group (general, agents, usage) or one page in it (project:<id>, agent:claude). */
    async settings(at = "general") {
      if (!(await page(() => document.querySelector("[data-settings-page]") !== null))) await press("[data-k=settings-row]");
      await waitFor("Settings", () => document.querySelector("[data-settings-page]") !== null);
      const [kind, id] = at.split(":");
      await press(`[data-k=settings-${id === undefined ? kind : { computer: "computers", project: "projects", agent: "agents" }[kind]}]`);
      if (id !== undefined) {
        await waitFor(`the Settings row ${at}`, row => document.querySelector(row) !== null, 10_000, `[data-row-id='${at}']`);
        await press(`[data-row-id='${at}']`);
      }
      await waitFor(`the Settings page ${at}`, at => document.querySelector("[data-settings-page]")?.getAttribute("data-settings-at") === at, 10_000, at);
    },
    usage: () => route.settings("usage"),
  };

  /** Holds until no element has changed for `ms` (`cap` at most), so what the host answers late is drawn. */
  const quiet = (ms, cap) =>
    page(
      async (ms, cap) => {
        let last = performance.now();
        const seen = new MutationObserver(() => (last = performance.now()));
        seen.observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
        for (const until = performance.now() + cap; performance.now() - last < ms && performance.now() < until; ) await new Promise(done => setTimeout(done, 50));
        seen.disconnect();
      },
      ms,
      cap,
    );

  /** Holds for fonts, a quiet page and every animation that ends (3 s at most), then two frames. */
  const settle = async () => {
    await quiet(300, 3_000);
    const got = await page(async () => {
      await document.fonts.ready;
      const frame = () => new Promise(done => (requestAnimationFrame(() => done(true)), setTimeout(() => done(false), 1_000)));
      await frame();
      await frame();
      const moving = () => document.getAnimations().filter(a => a.playState === "running" && a.effect?.getComputedTiming().iterations !== Infinity).length;
      for (const until = performance.now() + 3_000; moving() > 0 && performance.now() < until; ) await new Promise(done => setTimeout(done, 50));
      await frame();
      return (await frame()) || "no frame came, so the window is hidden or throttled";
    });
    if (got !== true) throw new Error(got);
  };

  /** The region a state keeps, in CSS px of the window: a selector's (or a list of selectors') box padded 16 px, or a
   * { x, y, width, height }, or `clip(app, data)` answering either; clamped to the window and to whole pixels. */
  const clipOf = async (given, data, drive) => {
    const picked = typeof given === "function" ? await given(drive, data) : given;
    const clip = typeof picked === "string" ? picked.replace(/^css:/, "") : Array.isArray(picked) ? picked.map(c => c.replace(/^css:/, "")) : picked;
    if (clip === undefined) return undefined;
    const rect =
      typeof clip === "string" || Array.isArray(clip)
          ? await page(selectors => {
              const boxes = selectors.flatMap(sel => [...document.querySelectorAll(sel)]).map(el => el.getBoundingClientRect()).filter(r => r.width > 0 && r.height > 0);
              if (boxes.length === 0) return undefined;
              const x = Math.min(...boxes.map(r => r.left)) - 16;
              const y = Math.min(...boxes.map(r => r.top)) - 16;
              return { x, y, width: Math.max(...boxes.map(r => r.right)) + 16 - x, height: Math.max(...boxes.map(r => r.bottom)) + 16 - y };
            }, [clip].flat())
          : clip;
    if (rect === undefined || rect === null) throw new Error(`nothing on the page for the clip ${JSON.stringify(clip)}`);
    const [w, h] = await page(() => [innerWidth, innerHeight]);
    const x = Math.max(0, Math.floor(rect.x));
    const y = Math.max(0, Math.floor(rect.y));
    return { x, y, width: Math.min(w, Math.ceil(rect.x + rect.width)) - x, height: Math.min(h, Math.ceil(rect.y + rect.height)) - y };
  };

  /** The whole window as a PNG at its own scale. A clip is cut from it in node: Page.captureScreenshot's own clip moves
   * the page's pointer to the window's centre before it paints, which drops a hover, and runs through a viewport override. */
  const capture = async () => Buffer.from((await cdp("Page.captureScreenshot", { format: "png", fromSurface: true, captureBeyondViewport: false })).data, "base64");
  return { tab, cdp, win, page, waitFor, locate, press, quiet, settle, clipOf, capture, letGo, heldChords, threadRow, route, click, menu, hover, key, keyDown, keyUp, type, scroll, drag, dropFiles, park, size, width: px => size(px, HEIGHT), height: px => size(WIDTH, px) };
}

/** The `app` a state's go is handed: each call goes to the helpers of the window it drives now, which `window(kind)`
 * moves to another window of the app (WINDOWS). */
function driveOf(current, moveTo) {
  const drive = { sleep, fixture, mod: MAC ? "cmd" : "ctrl", window: kind => moveTo(kind) };
  for (const name of ["click", "menu", "hover", "key", "keyDown", "keyUp", "type", "scroll", "drag", "dropFiles", "park", "size", "width", "height", "page"]) drive[name] = (...args) => current()[name](...args);
  drive.waitFor = (fn, ms, ...args) => current().waitFor("the state's wait", fn, ms, ...args);
  Object.defineProperty(drive, "route", { get: () => current().route });
  return drive;
}

/** A throwaway host on the empty home `spawnEmptyHost` makes; the claude on its PATH is the parity stub unless `agents` is
 * false. `stop()` ends it by its pid. */
async function emptyHost({ agents = true } = {}) {
  const { dir, state, child } = spawnEmptyHost({ hostNode, hostBin, stub: agents ? parityStub : undefined, detached: true });
  const h = {
    dir,
    state,
    pid: child.pid,
    held: false,
    async stop() {
      emptyHosts.delete(h);
      h.socket?.close();
      if (h.held && alive(h.pid)) process.kill(h.pid, "SIGCONT");
      if (alive(h.pid) && argsLine(h.pid).includes(state)) {
        process.kill(h.pid, "SIGTERM");
        for (const until = Date.now() + 20_000; alive(h.pid) && Date.now() < until; ) await sleep(100);
        if (alive(h.pid)) process.kill(h.pid, "SIGKILL");
      }
      rmSync(dir, { recursive: true, force: true });
    },
  };
  emptyHosts.add(h);
  const lockedPort = () => {
    try {
      return JSON.parse(readFileSync(join(dir, "state", "host.lock"), "utf8")).port || 0;
    } catch {
      return 0;
    }
  };
  for (const until = Date.now() + 30_000; !existsSync(join(dir, "state", "host-token")) || lockedPort() === 0; await sleep(100)) {
    if (child.exitCode !== null || Date.now() > until) throw new Error(`the empty host did not come up:\n${readFileSync(join(dir, "up.log"), "utf8").slice(-2000)}`);
  }
  h.socket = await hostSocket(lockedPort(), readFileSync(join(dir, "state", "host-token"), "utf8").trim());
  return h;
}

async function main() {
  if (typeof setup === "function") {
    step("wave setup");
    await setup(hostHelpers);
  }
  mkdirSync(out, { recursive: true });
  const indexPath = join(out, "index.json");
  /** Entries of an earlier run into the same folder stay, so an --only run updates its own states and no others. */
  const index = existsSync(indexPath) ? JSON.parse(readFileSync(indexPath, "utf8")) : [];
  const record = entry => {
    const at = index.findIndex(e => e.state === entry.state && e.theme === entry.theme);
    if (at >= 0) index[at] = entry;
    else index.push(entry);
    index.sort((a, b) => a.state.localeCompare(b.state) || a.theme.localeCompare(b.theme));
    writeFileSync(indexPath, `${JSON.stringify(index, null, 2)}\n`);
  };
  /** One shot: `work` brings the state up and answers { tools, drive, after }; then the capture, recorded either way,
   * then cleanup and `after`, which also run after a failure. */
  const shoot = async (state, name, scale, work) => {
    const file = `${state.id}-${name}.png`;
    const began = Date.now();
    let data;
    let t;
    try {
      if (state.skip !== undefined) throw new Error(state.skip);
      let late;
      t = await Promise.race([
        work(got => (data = got), up => (t = up)),
        new Promise((_, fail) => (late = setTimeout(() => fail(new Error(`the state ran past ${STATE_MS / 1000} s`)), STATE_MS))),
      ]).finally(() => clearTimeout(late));
      await t.tools().settle();
      const clip = await t.tools().clipOf(state.clip, data, t.drive);
      const whole = await t.tools().capture();
      const shot = clip === undefined ? withPhys(whole, scale) : crop(whole, clip, scale);
      writeFileSync(join(out, file), shot.png);
      record({ state: state.id, theme: name, file, bytes: shot.png.length, width: shot.width, height: shot.height, scale, clip: clip ?? null, reached: true });
      step(`${file} ${shot.width}x${shot.height}${clip !== undefined ? ` (clip ${clip.x},${clip.y} ${clip.width}x${clip.height})` : ""} in ${((Date.now() - began) / 1000).toFixed(1)} s`);
    } catch (e) {
      if (interrupted) return;
      const reason = (e instanceof Error ? e.message : String(e)).replace(/\s+/g, " ").trim().slice(0, 400);
      rmSync(join(out, file), { force: true });
      record({ state: state.id, theme: name, file: null, bytes: 0, width: 0, height: 0, scale, clip: null, reached: false, reason });
      step(`${state.id} ${name} not reached: ${reason}`);
    } finally {
      if (state.cleanup !== undefined && state.skip === undefined) await Promise.resolve().then(() => state.cleanup(hostHelpers, data)).catch(e => step(`cleanup after ${state.id} ${name}: ${e instanceof Error ? e.message : String(e)}`));
      await t?.after?.().catch(e => step(`closing after ${state.id} ${name}: ${e instanceof Error ? e.message : String(e)}`));
      await restorePrefs().catch(e => step(`putting the preferences back after ${state.id}: ${e.message}`));
    }
  };
  // The states on an empty home run last, each in an app and a host of its own, so the fixture's app opens once.
  const onFixture = picked.filter(s => s.home === undefined);
  const onEmpty = picked.filter(s => s.home !== undefined);
  let scale = scaleFlag === undefined ? 1 : Number(scaleFlag);
  if (onFixture.length > 0) scale = await fixtureSession(onFixture, shoot);
  if (onEmpty.length > 0) await stopApp();
  for (const state of onEmpty) for (const name of themes) if (!interrupted) await shoot(state, name, scale, (setData, started) => emptySession(state, name, setData, started));
  const missed = index.filter(e => picked.some(s => s.id === e.state) && themes.includes(e.theme) && !e.reached).length;
  step(`${picked.length * themes.length - missed} of ${picked.length * themes.length} captured into ${out}`);
}

/** The states on the fixture's host, in one app on it; answers the scale it draws at. */
async function fixtureSession(states, shoot) {
  launchApp({ wspHome: stateDir, ownHome: home });
  const mainTools = tools(await attach("app"), "app");
  let cur = mainTools;
  const moveTo = async kind => {
    cur = tools(await attach(kind, 20_000), kind);
    await cur.waitFor(`the ${kind} window`, () => document.readyState === "complete", 20_000);
  };
  const drive = driveOf(() => cur, moveTo);
  step(`raised the window: ${JSON.stringify(await raise())}`);
  // The desktop app draws its right-click menus natively, which no page capture holds; this answers them instead,
  // with the row a state picked by its words, and keeps the rows each was asked to show.
  await onWindow(`
    const { ipcMain } = process.mainModule.require("electron");
    if (globalThis.parityMenus === undefined) {
      globalThis.parityMenus = [];
      globalThis.parityPicks = [];
      ipcMain.removeHandler("menu:context");
      ipcMain.handle("menu:context", (_e, items) => {
        globalThis.parityMenus.push(items.map(i => i.label));
        const pick = globalThis.parityPicks.shift();
        return items.find(i => i.label === pick && i.enabled)?.id ?? null;
      });
    }
    return true;`);
  const { page, waitFor, route, quiet, press, key, keyUp, park, size, letGo, heldChords, threadRow, cdp } = mainTools;
  const firstThread = Object.values(fixture.threads)[0];
  /** Holds until the page shows the sidebar with the fixture's threads, then visits New thread once. */
  const ready = async () => {
    await waitFor(
      "the New thread row and the fixture's threads in the sidebar",
      sel => document.querySelector("[data-new-thread-row]:not([aria-disabled])") !== null && (sel === undefined || document.querySelector(sel) !== null),
      60_000,
      firstThread === undefined ? undefined : threadRow(firstThread),
    );
    // New thread's first visit asks the host for the agent's sign-in and keeps it; this visit makes the first state like the rest.
    await route.home();
    await quiet(1_500, 15_000);
  };
  step("waiting for the sidebar");
  await ready();
  // The keys the app reads back as layout, picks or drafts; the rest (stamps, the last thread open) change on their own.
  const KEPT_KEYS = /^wsp:(composer-drafts|composer-favourites|composer-modes|prompt-stash|creations|sidebar-|settings-at|right-panel-|terminal-|diff-surface|render-markdown)|^wsp\.terminal\./;
  /** The page's own state as the first visit left it: the page it is on and the localStorage keys above. */
  const pageState = () =>
    page(kept => {
      const keys = Object.keys(localStorage).filter(k => new RegExp(kept).test(k)).sort();
      return { page: location.origin + location.pathname, storage: Object.fromEntries(keys.map(k => [k, localStorage.getItem(k)])) };
    }, KEPT_KEYS.source);
  const pristine = await pageState();
  const pristineAll = await page(() => Object.fromEntries(Object.keys(localStorage).map(k => [k, localStorage.getItem(k)])));
  const homeUrl = `${new URL(pristine.page).origin}/`;
  // Each load starts from that storage, so what the app writes on its way out of the last page is overwritten too.
  await cdp("Page.enable");
  await cdp("Page.addScriptToEvaluateOnNewDocument", { source: `if (location.href.startsWith(${JSON.stringify(new URL(homeUrl).origin)})) { localStorage.clear(); for (const [k, v] of Object.entries(${JSON.stringify(pristineAll)})) localStorage.setItem(k, v); }` });
  let reloads = 0;

  /** Puts the window back where every state starts: the default size first (a narrow window shuts the sidebar), the
   * first visit's page state, nothing open, New thread, the pointer away and no key or drag held. */
  const reset = async () => {
    cur = mainTools;
    await raise();
    await cdp("Page.bringToFront");
    await letGo();
    for (const chord of heldChords.toReversed()) await keyUp(chord);
    await park();
    await size(WIDTH, HEIGHT);
    for (let tries = 0; JSON.stringify(await pageState()) !== JSON.stringify(pristine); tries++) {
      if (tries === 2) throw new Error("the page kept its own state through two reloads");
      reloads++;
      const before = await page(() => performance.timeOrigin);
      // The storage is written here too, in case the script above did not run before the app read it.
      await page(all => {
        localStorage.clear();
        for (const [k, v] of Object.entries(all)) localStorage.setItem(k, v);
      }, pristineAll);
      await cdp("Page.navigate", { url: homeUrl });
      for (const until = Date.now() + 2_000; (await page(() => performance.timeOrigin).catch(() => before)) === before && Date.now() < until; ) await sleep(100);
      if ((await page(() => performance.timeOrigin).catch(() => before)) === before) await cdp("Page.reload", { ignoreCache: false });
      await waitFor("the page loaded again", was => performance.timeOrigin !== was && document.readyState === "complete", 30_000, before);
      await ready();
    }
    for (let i = 0; i < 2; i++) await key("escape"), await sleep(100);
    // Escape leaves a message standing while the composer has focus, so each is closed by its own button.
    const messages = await page(() => [...document.querySelectorAll("[data-notice] button[aria-label='Close this message']")].map(b => b.click()).length);
    if (messages > 0) await waitFor("the standing messages closed", () => document.querySelector("[data-sonner-toast] [data-notice]") === null, 5_000);
    if (await page(() => document.querySelector("[data-settings-page]") !== null)) await press("[data-k=settings-back]");
    await waitFor("Settings closed", () => document.querySelector("[data-settings-page]") === null);
    await route.home();
    await quiet(300, 3_000);
    // A draft typed on New thread stays there for the next visit, so it is cleared as a person would; one select-all
    // and Backspace can leave a paragraph's last letter behind, so it goes on until the editor is empty.
    for (let tries = 0; ; tries++) {
      const drafted = await page(() => {
        const editor = document.querySelector("[data-k=project-home] [data-chat-composer] [contenteditable=true]");
        if (editor === null || editor.textContent.trim() === "") return false;
        editor.focus();
        document.execCommand("selectAll");
        return true;
      });
      if (!drafted) break;
      if (tries === 10) throw new Error("the New thread draft would not clear");
      await key("backspace");
      await sleep(50);
    }
  };

  /** The host's page in a window of its own with no preload and storage of its own, as a browser tab opens it. */
  const openPair = async (width, height) => {
    await onWindow(`
      const p = new BrowserWindow({ width: ${width}, height: ${height}, useContentSize: true, show: true, webPreferences: { partition: "parity-pair", contextIsolation: true, sandbox: true } });
      void p.loadURL(new URL("/?parity-pair", w.webContents.getURL()).href);
      return true;`);
    await moveTo("pair");
    await raise("pair");
  };
  const closePair = () => onWindow("w.destroy(); return true;", "pair").finally(() => (cur = mainTools));

  const scale = await page(() => devicePixelRatio);
  if (scaleFlag !== undefined && scale !== Number(scaleFlag)) throw new Error(`the app was launched at scale ${scaleFlag} and its page draws at ${scale}`);
  // Every state is shot solid: a page capture holds no native material, and what glass shows depends on what is behind.
  await setPrefs({ transparency: false });
  current = { ...current, transparency: false };
  let drawn;
  const theme = async name => {
    if (drawn === name) return;
    await setPrefs(THEMES[name]);
    current = { ...current, ...THEMES[name] };
    await waitFor(`the ${name} theme`, id => document.documentElement.dataset.theme === id, 10_000, name);
    drawn = name;
  };
  // State by state, each in every theme before the next: data, go, capture, cleanup for each shot.
  for (const state of states) {
    for (const name of themes) {
      if (interrupted) return scale;
      await shoot(state, name, scale, async setData => {
        await theme(name);
        await reset();
        if (state.width !== undefined || state.height !== undefined) await size(state.width ?? WIDTH, state.height ?? HEIGHT);
        const data = state.data === undefined ? undefined : await state.data(hostHelpers);
        setData(data);
        if (state.bridge === false) await openPair(state.width ?? WIDTH, state.height ?? HEIGHT);
        await state.go(drive, data);
        return { tools: () => cur, drive, after: state.bridge === false ? closePair : undefined };
      });
    }
  }
  if (reloads > 0) step(`reloaded the page ${reloads} times to put its own state back`);
  return scale;
}

/** One shot on an empty home: a throwaway host of its own (stopped with SIGSTOP for home "held", so the app shows its
 * starting window), the app launched on it, and the window the state names (onboarding, or starting when held). */
async function emptySession(state, name, setData, started) {
  await stopApp();
  const h = await emptyHost({ agents: state.agents !== false });
  const after = async () => {
    await stopApp();
    await h.stop();
  };
  started({ tools: () => undefined, after });
  try {
    await h.socket.op("preferences.set", { patch: { ...THEMES[name], transparency: false } });
    if (state.home === "held") {
      process.kill(h.pid, "SIGSTOP");
      h.held = true;
    }
    launchApp({ wspHome: join(h.dir, "state"), ownHome: h.dir, system: EMPTY_SYSTEM });
    const first = state.window ?? (state.home === "held" ? "starting" : "onboarding");
    let cur = tools(await attach(first), first);
    const moveTo = async kind => {
      cur = tools(await attach(kind, 30_000), kind);
      await cur.waitFor(`the ${kind} window`, () => document.readyState === "complete", 20_000);
    };
    const drive = driveOf(() => cur, moveTo);
    await cur.waitFor(`the ${first} window`, () => document.readyState === "complete", 20_000);
    await raise(first).catch(() => {});
    await cur.quiet(500, 5_000);
    if (state.width !== undefined || state.height !== undefined) await cur.size(state.width ?? WIDTH, state.height ?? HEIGHT);
    const data = state.data === undefined ? undefined : await state.data({ ...hostHelpers, empty: { home: h.dir, state: h.state, op: h.socket.op } });
    setData(data);
    await state.go(drive, data);
    return { tools: () => cur, drive, after };
  } catch (e) {
    await after();
    throw e;
  }
}

let failed = false;
try {
  await main();
} catch (e) {
  failed = true;
  console.error(`baseline: ${e instanceof Error ? e.message : String(e)}`);
}
await finish();
process.exit(failed ? 1 : 0);
