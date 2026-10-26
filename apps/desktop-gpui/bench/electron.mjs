// SPDX-License-Identifier: AGPL-3.0-only
// The bench against today's Electron app, driven over the Chrome DevTools Protocol:
//   node electron.mjs --app <executable> --fixture <fixture.json> --out <result.json> --mem <mem.json>
// It launches the packaged app on the fixture's state (WSP_HOME names the home, so the app attaches to the fixture's
// host and to nothing else), reads its memory at rest and with `long` open, scrolls `long` top to bottom at 2,400 px
// per second, switches to each short thread as a click does, and stops the app it started by its pid.
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { firstPrompt, lastWords as lastMarkdown } from "./content.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const PX_PER_S = 2400;
const SETTLE_MS = 10_000;
/** How long after the scroll ends memory is read: GPUI's bench holds 10 s after its scroll before it switches, and its
 * reading, taken by run-mac.sh off the bench's log, has to land inside that hold; both apps wait the same. */
const SCROLLED_SETTLE_MS = 9_000;
const SWITCH = ["short-1", "short-2", "short-3"];
/** A thread's last words as the page draws them, its inline code without the backticks. */
const lastWords = name => lastMarkdown(name).replaceAll("`", "");

const flag = name => {
  const at = process.argv.indexOf(`--${name}`);
  if (at < 0 || process.argv[at + 1] === undefined) throw new Error(`usage: node electron.mjs --app <executable> --fixture <fixture.json> --out <result.json> --mem <mem.json>; --${name} is missing`);
  return process.argv[at + 1];
};
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const step = what => console.log(`electron: ${what}`);

const fixture = JSON.parse(readFileSync(flag("fixture"), "utf8"));
const home = dirname(dirname(fixture.state));
const appHome = join(home, "electron-home");
mkdirSync(appHome, { recursive: true });
// With no host serving its state the app installs a launchd or systemd service in the person's own login; these refuse.
const managers = join(appHome, "bin");
mkdirSync(managers, { recursive: true });
for (const name of ["launchctl", "systemctl"]) writeFileSync(join(managers, name), `#!/bin/sh\necho "the bench refuses ${name} $*" >&2\nexit 1\n`, { mode: 0o755 });
const shell = join(appHome, "login-shell");
// The app reads PATH off the login shell it is handed; this one hands it the fixture's, stub claude first.
const path = [managers, join(home, "bin"), dirname(process.execPath), "/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"].join(":");
writeFileSync(shell, `#!/bin/sh\nprintf 'PATH=%s\\0' ${JSON.stringify(path)}\n`, { mode: 0o755 });
const env = { HOME: appHome, WSP_HOME: dirname(fixture.state), PATH: path, LANG: "en_US.UTF-8", SHELL: shell, WSP_DESKTOP_SMOKE: "1" };
for (const name of ["DISPLAY", "XAUTHORITY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"]) if (process.env[name] !== undefined) env[name] = process.env[name];

step(`launching ${flag("app")} on ${fixture.state}`);
// Chromium refuses to run as root with its sandbox on, which is how a Linux cloud machine runs and a Mac never does.
// --inspect opens the main process to this script, which is how its window is raised above other apps (front below).
const app = spawn(flag("app"), ["--remote-debugging-port=0", "--inspect=0", ...(process.getuid?.() === 0 ? ["--no-sandbox"] : [])], { cwd: appHome, env, stdio: ["ignore", "pipe", "pipe"] });
const said = [];
app.stdout.on("data", chunk => said.push(chunk.toString()));
app.stderr.on("data", chunk => said.push(chunk.toString()));
/** What the app printed that `pattern` finds, once it has printed it. */
const printed = (pattern, what) =>
  new Promise((resolve, reject) => {
    const read = () => {
      const found = pattern.exec(said.join(""))?.[1];
      if (found !== undefined) resolve(found);
    };
    app.stdout.on("data", read);
    app.stderr.on("data", read);
    app.on("exit", code => reject(new Error(`the app exited (${code}) before ${what}:\n${said.join("").slice(-3000)}`)));
  });
const devtools = printed(/DevTools listening on ws:\/\/[^:]+:(\d+)\//, "DevTools listened").then(Number);
const inspector = printed(/Debugger listening on (ws:\/\/\S+)/, "its inspector listened");
// raise() may give up on it first; the app's exit then rejects it with no one awaiting, which would crash this script.
inspector.catch(() => {});
// Stopped by its pid whichever way this script is stopped, so no app is left with its inspector open.
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) process.once(signal, () => void stopApp().finally(() => process.exit(1)));

/** A Chrome DevTools Protocol session on a websocket: `send(method, params)` answers the result or throws. */
async function session(url) {
  const ws = new WebSocket(url);
  await new Promise((resolve, reject) => ((ws.onopen = resolve), (ws.onerror = reject)));
  let next = 0;
  const waiting = new Map();
  ws.onmessage = m => {
    const msg = JSON.parse(m.data);
    if (msg.id !== undefined) waiting.get(msg.id)?.(msg), waiting.delete(msg.id);
  };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = ++next;
      waiting.set(id, msg => (msg.error ? reject(new Error(`${method}: ${msg.error.message}`)) : resolve(msg.result)));
      ws.send(JSON.stringify({ id, method, params }));
    });
  return { send, close: () => ws.close() };
}

/** Raises this app's windows above every other app's, the person's own wsp included, through its main process: a Mac
 * opens an app a script launched behind the frontmost one, Chromium reads an occluded window as hidden and throttles
 * it, and Page.bringToFront raises the tab inside the window, never the window. Nothing here goes through
 * LaunchServices, which knows the test app and the person's by one bundle id. Answers each window's state. */
let mainProcess;
async function raise() {
  if (mainProcess === undefined) {
    const url = await Promise.race([inspector, sleep(10_000)]);
    if (url === undefined) throw new Error("the app printed no inspector address in 10 s, so --inspect was not taken");
    mainProcess = await session(url);
  }
  for (const until = Date.now() + 30_000; ; await sleep(250)) {
    const got = await mainProcess.send("Runtime.evaluate", {
      returnByValue: true,
      // The main module is set once the app's entry has loaded; before that there is no window to raise.
      expression: `(() => {
        if (process.mainModule === undefined) return undefined;
        const { app, BrowserWindow } = process.mainModule.require("electron");
        const windows = BrowserWindow.getAllWindows().filter(w => w.webContents.getURL().startsWith("http://127.0.0.1:"));
        if (windows.length === 0) return undefined;
        if (process.platform === "darwin") app.focus({ steal: true });
        for (const w of windows) {
          if (w.isMinimized()) w.restore();
          w.show();
          w.moveTop();
          w.focus();
        }
        return windows.map(w => ({ visible: w.isVisible(), focused: w.isFocused() }));
      })()`,
    });
    if (got.exceptionDetails !== undefined) throw new Error(`raising the window: ${got.exceptionDetails.exception?.description ?? got.exceptionDetails.text}`);
    if (got.result.value !== undefined) return got.result.value;
    if (Date.now() > until) throw new Error("the app's main process showed no window on the host's page in 30 s");
  }
}

/** Every process under the app as last seen, stopped by these pids when the app is: nothing is found by name. */
let tree = [];
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
const bundle = flag("app").includes(".app/") ? flag("app").slice(0, flag("app").indexOf(".app/") + 4) : dirname(flag("app"));
/** Whether a pid seen under the app earlier still runs the app's files or the fixture's: a pid freed since may be anyone's. */
const ours = pid => {
  const args = spawnSync("ps", ["-ww", "-o", "args=", "-p", String(pid)], { encoding: "utf8" }).stdout;
  return args.includes(bundle) || args.includes(home);
};
async function stopApp() {
  tree = [...new Set([...tree, ...(alive(app.pid) ? treeOf(app.pid) : [])])];
  if (alive(app.pid)) app.kill("SIGTERM");
  for (const until = Date.now() + 10_000; alive(app.pid) && Date.now() < until; ) await sleep(100);
  for (const pid of tree.filter(alive).filter(ours)) process.kill(pid, "SIGKILL");
}

async function main() {
  const port = await devtools;
  let target;
  for (const until = Date.now() + 60_000; target === undefined; await sleep(250)) {
    if (Date.now() > until) throw new Error("no window showing the host's page in 60 s");
    const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    target = pages.find(p => p.type === "page" && /^http:\/\/127\.0\.0\.1:\d+\//.test(p.url));
  }
  const tab = await session(target.webSocketDebuggerUrl);
  const cdp = tab.send;
  step(`raised the window: ${JSON.stringify(await raise())}`);
  /** Runs a function in the page with JSON arguments and answers what it returned. */
  const page = async (fn, ...args) => {
    const got = await cdp("Runtime.evaluate", { expression: `(${fn})(...${JSON.stringify(args)})`, awaitPromise: true, returnByValue: true });
    if (got.exceptionDetails !== undefined) throw new Error(got.exceptionDetails.exception?.description ?? got.exceptionDetails.text);
    return got.result.value;
  };
  const waitFor = async (what, fn, ms, ...args) => {
    for (const until = Date.now() + ms; ; await sleep(100)) {
      const got = await page(fn, ...args);
      if (got) return got;
      if (Date.now() > until) throw new Error(`${what} not seen in ${ms / 1000} s: ${await seen().catch(() => "the page had no bench helpers yet")}`);
    }
  };
  const rowSelector = id => `[data-row-id='thread:${id}']`;
  const click = async selector => {
    const box = await page(sel => {
      const el = document.querySelector(sel);
      el.scrollIntoView({ block: "nearest" });
      const r = el.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    }, selector);
    await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: box.x, y: box.y });
    await cdp("Input.dispatchMouseEvent", { type: "mousePressed", x: box.x, y: box.y, button: "left", clickCount: 1 });
    await cdp("Input.dispatchMouseEvent", { type: "mouseReleased", x: box.x, y: box.y, button: "left", clickCount: 1 });
  };
  /** What the page sees, for an error that has to say why it stalled. */
  const seen = async () => JSON.stringify(await page(() => window.__bench.state()));
  /** Raises the window and holds the run until the page is visible and focused, failing it if that never comes. */
  const front = async () => {
    const windows = await raise();
    await cdp("Page.bringToFront");
    for (const until = Date.now() + 5_000; ; await sleep(100)) {
      const unfit = await page(() => window.__bench.unfit());
      if (unfit === undefined) return;
      if (Date.now() > until) throw new Error(`${unfit} after it was raised (windows ${JSON.stringify(windows)}, page ${await seen()}), so requestAnimationFrame would be throttled`);
    }
  };
  const mem = () => {
    tree = [...new Set([...tree, ...treeOf(app.pid)])];
    const ran = spawnSync(join(here, "mem.sh"), [String(app.pid)], { encoding: "utf8" });
    if (ran.status !== 0) throw new Error(`mem.sh ${app.pid}: ${ran.stderr}`);
    process.stderr.write(ran.stderr);
    return Number(ran.stdout.trim());
  };

  step("waiting for the sidebar");
  await waitFor("the long thread's row in the sidebar", sel => document.querySelector(sel) !== null, 60_000, rowSelector(fixture.threads.long));
  await page(() => {
    window.__bench = {
      /** The transcript's scroller: the first scrolling box above a row of it. */
      scroller() {
        let el = document.querySelector("[data-timeline-row-id]")?.parentElement;
        while (el && !(el.scrollHeight > el.clientHeight && /auto|scroll/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
        return el ?? undefined;
      },
      shows: words => [...document.querySelectorAll("[data-timeline-row-id]")].some(el => el.textContent.includes(words)),
      // A hidden window runs no requestAnimationFrame callbacks, so a frame that does not come fails the run, not hangs it.
      frame: () =>
        new Promise((resolve, reject) => {
          const late = setTimeout(() => reject(new Error(`no frame in 5 s: ${window.__bench.unfit() ?? "the page stopped drawing"}`)), 5_000);
          requestAnimationFrame(t => (clearTimeout(late), resolve(t)));
        }),
      /** Why the window cannot be measured now, or nothing: Chromium throttles requestAnimationFrame in a window that
       * is hidden or behind others. */
      state() {
        const scroller = window.__bench.scroller();
        return { visibilityState: document.visibilityState, hasFocus: document.hasFocus(), scrollTop: scroller?.scrollTop ?? null, scrollHeight: scroller?.scrollHeight ?? null, rows: document.querySelectorAll("[data-timeline-row-id]").length };
      },
      unfit: () => (document.visibilityState !== "visible" ? `the window is ${document.visibilityState}` : !document.hasFocus() ? "the window has no focus" : undefined),
      /** Watches for the window going out of view or losing focus, until stopped; answers what happened, if anything. */
      watch() {
        let lost;
        const blur = () => (lost ??= "the window lost focus");
        const seen = () => (lost ??= document.visibilityState === "visible" ? undefined : `the window became ${document.visibilityState}`);
        addEventListener("blur", blur);
        document.addEventListener("visibilitychange", seen);
        return () => {
          removeEventListener("blur", blur);
          document.removeEventListener("visibilitychange", seen);
          return lost ?? window.__bench.unfit();
        };
      },
    };
  });
  // The app opens on the newest thread; New thread is the view with the sidebar and no transcript.
  await page(() => [...document.querySelectorAll("button")].find(b => b.textContent.trim().startsWith("New thread"))?.click());
  await waitFor("a view with no transcript", () => document.querySelectorAll("[data-timeline-row-id]").length === 0, 10_000);
  await front();
  await sleep(SETTLE_MS);
  const rest = mem();
  step(`memory at rest ${rest} MB`);

  step("opening long and loading its whole transcript");
  await front();
  const opened = Date.now();
  await click(rowSelector(fixture.threads.long));
  await waitFor("long's last words", words => window.__bench.shows(words), 60_000, lastWords("long"));
  // The app opens a thread at its end and reads older pages as the reader nears the top, so the top is held until
  // the thread's first message is drawn.
  // A hidden window pages nothing, so the wait stops at once when the page stops being visible and focused.
  for (const until = Date.now() + 120_000; ; await sleep(100)) {
    const at = await page(prompt => {
      const unfit = window.__bench.unfit();
      const scroller = window.__bench.scroller();
      if (unfit === undefined && scroller !== undefined) scroller.scrollTop = 0;
      return { shown: window.__bench.shows(prompt), unfit };
    }, firstPrompt("long"));
    if (at.shown) break;
    if (at.unfit !== undefined) throw new Error(`${at.unfit} while long paged back to its first message: ${await seen()}`);
    if (Date.now() > until) throw new Error(`long's first message not seen in 120 s: ${await seen()}`);
  }
  step(`whole transcript loaded in ${((Date.now() - opened) / 1000).toFixed(1)} s`);
  await sleep(SETTLE_MS);
  const long = mem();
  step(`memory with long open ${long} MB`);

  await front();
  if (!(await page(prompt => document.querySelector("[data-timeline-row-id]")?.textContent.includes(prompt) === true, firstPrompt("long")))) throw new Error("the scroll would not start at long's first message");
  step("the first row drawn is long's first message");
  step(`scrolling long top to bottom at ${PX_PER_S} px/s`);
  const scrolled = await page(async pxPerS => {
    const { scroller: find, frame, watch } = window.__bench;
    const scroller = find();
    // The list's own row count, off the props React handed the virtualized list.
    const fiberKey = Object.keys(scroller).find(k => k.startsWith("__reactFiber$"));
    let fiber = scroller[fiberKey];
    while (fiber && !(Array.isArray(fiber.memoizedProps?.data) && typeof fiber.memoizedProps?.renderItem === "function")) fiber = fiber.return;
    const rows = fiber ? fiber.memoizedProps.data.length : -1;
    scroller.scrollTop = 0;
    await frame();
    await frame();
    const stop = watch();
    const t0 = await frame();
    const frames = [0];
    for (;;) {
      const t = await frame();
      const lost = window.__bench.unfit();
      if (lost !== undefined) throw new Error(`${lost} during the scroll, so its frame times are throttled`);
      frames.push(t - t0);
      const end = scroller.scrollHeight - scroller.clientHeight;
      const offset = ((t - t0) * pxPerS) / 1000;
      scroller.scrollTop = Math.min(offset, end);
      if (offset >= end) break;
      if (t - t0 > 300_000) throw new Error(`the scroll had not reached the end after 300 s, at ${scroller.scrollTop} of ${end} px`);
    }
    const lost = stop();
    if (lost !== undefined) throw new Error(`${lost} during the scroll, so its frame times are throttled`);
    return { rows, frames, height: scroller.scrollHeight };
  }, PX_PER_S);
  step(`${scrolled.frames.length} frames over ${scrolled.height} px, the list holding ${scrolled.rows} rows`);
  await sleep(SCROLLED_SETTLE_MS);
  const after = mem();
  step(`memory after scrolling long ${after} MB`);
  writeFileSync(flag("mem"), `${JSON.stringify({ rest, long, scrolled: after })}\n`);

  const switches = [];
  for (const name of SWITCH) {
    const id = fixture.threads[name];
    await front();
    await page(words => {
      const { frame, shows, watch } = window.__bench;
      window.__benchSwitch = new Promise((resolve, reject) => {
        // The row opens its thread on click; the press before it is a separate DevTools round trip, about 4 ms.
        addEventListener(
          "click",
          async e => {
            const t0 = e.timeStamp;
            const stop = watch();
            for (;;) {
              await frame();
              if (shows(words)) break;
            }
            // A requestAnimationFrame callback runs before Chromium lays out and paints its frame, so the frame where
            // the rows first exist has not drawn them yet; the clock is read inside the next frame's callback, which
            // runs only once that layout and paint are done, as GPUI stamps after its own.
            await frame();
            const ms = performance.now() - t0;
            const lost = stop();
            if (lost !== undefined) reject(new Error(`${lost} during the switch`));
            else resolve(ms);
          },
          { capture: true, once: true },
        );
      });
    }, lastWords(name));
    await click(rowSelector(id));
    const ms = await page(() => Promise.race([window.__benchSwitch, new Promise(resolve => setTimeout(() => resolve(-1), 30_000))]));
    if (ms < 0) throw new Error(`${name} did not show in 30 s after its click`);
    switches.push({ thread: id, ms: Math.round(ms * 10) / 10 });
    step(`switch to ${name} ${switches.at(-1).ms} ms`);
    await sleep(1000);
  }

  writeFileSync(flag("out"), `${JSON.stringify({ app: "electron", thread_rows: scrolled.rows, scroll: { px_per_s: PX_PER_S, frame_ms: scrolled.frames.map(t => Math.round(t * 100) / 100) }, switch: switches })}\n`);
  tab.close();
  mainProcess?.close();
}

try {
  await main();
  await stopApp();
} catch (e) {
  await stopApp();
  console.error(`electron: ${e instanceof Error ? e.message : String(e)}`);
  process.exit(1);
}
