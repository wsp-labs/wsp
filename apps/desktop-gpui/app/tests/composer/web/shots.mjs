// SPDX-License-Identifier: AGPL-3.0-only
// The web app's chat composer in each of its states, photographed in Paper and Graphite off a throwaway host:
//   node shots.mjs --out <dir> [--only a,b] [--keep-host]
// The host runs from a copy of its bundle carrying a wsp-daemon ($WSP_TEST_DAEMON, else the checkout's, else the one
// a cloud machine keeps), in a temp folder that is its HOME, with stub-claude.mjs as its claude. Headless Chrome opens
// the host's own page; each state writes <state>-<theme>.png and <state>-<theme>.json (crop, rects, data) into <dir>.
import { spawn, spawnSync } from "node:child_process";
import { closeSync, copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, openSync, readdirSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { crc32, deflateSync } from "node:zlib";
import { turnsOf } from "../../../../bench/content.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(here, "../../../../../..");
const HOST = join(REPO, "packages/host");
const TRIPLE = `${process.arch === "arm64" ? "aarch64" : "x86_64"}-${process.platform === "darwin" ? "apple-darwin" : "unknown-linux-musl"}`;
const MARK = ".wsp-composer-web";

export const STATES = [
  "empty",
  "typing",
  "mention-open",
  "command-open",
  "model-picker-open",
  "effort-picker-open",
  "access-picker-open",
  "attachments",
  "stash-open",
  "running",
  "queued",
  "banner",
  "new-thread",
  "disabled",
  "history",
  "hash-open",
  "dollar-open",
  "fast-open",
  "favourites-open",
  "pinned-picker",
  "queue-bar",
  "drawer-question",
  "drawer-limit",
  "drawer-slot",
  "drawer-tasks",
  "resume-banner",
  "paste-file",
];
const THEMES = [
  { name: "paper", scheme: "light", dark: false },
  { name: "graphite", scheme: "dark", dark: true },
];

const PROJECT_FILES = {
  "README.md": "# bench\n\nA transcript view that keeps its place while tool output streams in.\n",
  "package.json": `${JSON.stringify({ name: "bench", private: true, type: "module", scripts: { test: "node --test test" } }, null, 2)}\n`,
  "src/scroll-anchor.ts": "export function anchorOf(list: HTMLElement): number {\n  return list.scrollHeight - list.scrollTop;\n}\n",
  "src/tool-output.tsx": "export function ToolOutput({ text }: { text: string }) {\n  return <pre>{text}</pre>;\n}\n",
  "src/transcript.ts": "export interface Row {\n  id: string;\n  height: number;\n}\n",
  "test/transcript.test.ts": 'import { test } from "node:test";\n\ntest("rows keep their place", () => {});\n',
};
const SKILL = "---\nname: tidy-imports\ndescription: Sort and group a file's imports the way this repo does, and drop the unused ones.\n---\n\nSort the imports into node, packages, then local files, and remove any that nothing uses.\n";
const CONVERSATION_TITLE = "Fix the flaky scroll test";
const TYPED = ["Look at why the transcript jumps", "when a long tool output lands", "and fix it."];
const STASHED = "Try pinning the scroll anchor before the tool output mounts.";
const QUEUED = "Then check the scroll anchor too.";
const NOTES = "The transcript jumps when a long tool output lands.\n";
const PASTED = Array.from({ length: 250 }, (_, i) => `line ${i + 1} of the pasted log, as a terminal printed it`).join("\n");
const MARGIN = 24;
const sleep = ms => new Promise(r => setTimeout(r, ms));
const log = line => console.log(`shots: ${line}`);

function usage(why) {
  console.error(`${why}\nusage: node shots.mjs --out <dir> [--only a,b] [--keep-host]`);
  process.exit(2);
}

function parseArgs(argv) {
  const args = { only: STATES, keepHost: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--out" && argv[i + 1] !== undefined) args.out = resolve(argv[++i]);
    else if (argv[i] === "--only" && argv[i + 1] !== undefined) args.only = argv[++i].split(",").filter(Boolean);
    else if (argv[i] === "--keep-host") args.keepHost = true;
    else usage(`unknown argument ${argv[i]}`);
  }
  if (args.out === undefined) usage("--out names the folder the shots land in");
  const unknown = args.only.filter(s => !STATES.includes(s));
  if (unknown.length > 0) usage(`no state ${unknown.join(", ")}; the states are ${STATES.join(", ")}`);
  return args;
}

/** The wsp-daemon the host carries: the one WSP_TEST_DAEMON names, else the checkout's, else a cloud machine's. */
function daemonBinary() {
  const named = process.env.WSP_TEST_DAEMON;
  const tried = named ? [named] : [join(REPO, "packages/wspx/daemon", TRIPLE, "wsp-daemon"), join("/root/wsp-daemon/wsp/assets/daemon", TRIPLE, "wsp-daemon")];
  const found = tried.find(p => existsSync(p));
  if (found === undefined) throw new Error(`no wsp-daemon at ${tried.join(" or ")} (WSP_TEST_DAEMON names one)`);
  return found;
}

/** A copy of the host's bundle in `dir` with the daemon linked where the bundle reads its assets first. */
function bundleWith(dir, daemon) {
  const pkg = join(dir, "hostpkg");
  cpSync(join(HOST, "dist"), join(pkg, "dist"), { recursive: true });
  copyFileSync(join(HOST, "package.json"), join(pkg, "package.json"));
  mkdirSync(join(pkg, "assets/daemon", TRIPLE), { recursive: true });
  symlinkSync(join(HOST, "node_modules"), join(pkg, "node_modules"));
  symlinkSync(join(REPO, "apps/web/dist"), join(pkg, "assets/web"));
  if (existsSync(join(REPO, "packages/wspx/assets/cli"))) symlinkSync(join(REPO, "packages/wspx/assets/cli"), join(pkg, "assets/cli"));
  symlinkSync(daemon, join(pkg, "assets/daemon", TRIPLE, "wsp-daemon"));
  return join(pkg, "dist/bin.js");
}

/** A 24 px square PNG of one colour. */
function swatchPng(rgb) {
  const chunk = (type, data) => {
    const out = Buffer.alloc(12 + data.length);
    out.writeUInt32BE(data.length, 0);
    out.write(type, 4, "ascii");
    data.copy(out, 8);
    out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
    return out;
  };
  const head = Buffer.alloc(13);
  head.writeUInt32BE(24, 0);
  head.writeUInt32BE(24, 4);
  head.set([8, 2, 0, 0, 0], 8);
  const row = Buffer.concat([Buffer.from([0]), Buffer.alloc(24 * 3).map((_, i) => rgb[i % 3])]);
  const pixels = Buffer.concat(Array.from({ length: 24 }, () => row));
  return Buffer.concat([Buffer.from("89504e470d0a1a0a", "hex"), chunk("IHDR", head), chunk("IDAT", deflateSync(pixels)), chunk("IEND", Buffer.alloc(0))]);
}

/** The throwaway host: started, seeded and stopped by pid. */
class Fixture {
  constructor() {
    this.dir = realpathSync(mkdtempSync(join(tmpdir(), "wsp-composer-web-")));
    writeFileSync(join(this.dir, MARK), "");
    this.env = { HOME: this.dir, PATH: [join(this.dir, "bin"), dirname(process.execPath), "/usr/local/bin", "/usr/bin", "/bin"].join(":"), LANG: "C.UTF-8", TZ: "UTC", WSP_UPDATE_CHECK: "0" };
    this.state = join(this.dir, "state", "state.json");
    this.threads = {};
    this.host = undefined;
  }

  start() {
    for (const built of [join(HOST, "dist/bin.js"), join(REPO, "apps/web/dist/index.html")]) {
      if (!existsSync(built)) throw new Error(`${built} is not built: run pnpm install && pnpm build`);
    }
    mkdirSync(join(this.dir, "bin"));
    mkdirSync(join(this.dir, "state"));
    symlinkSync(join(here, "stub-claude.mjs"), join(this.dir, "bin", "claude"));
    this.bin = bundleWith(this.dir, daemonBinary());
    const out = openSync(join(this.dir, "up.log"), "a");
    this.host = spawn(process.execPath, [this.bin, "up", "--port", "0", "--no-relay", "--state", this.state], { cwd: this.dir, env: this.env, stdio: ["ignore", out, out], detached: true });
    closeSync(out);
    return this.waitUp();
  }

  async waitUp() {
    for (const until = Date.now() + 30_000; !existsSync(join(this.dir, "state/host-token")) || this.port() === 0; await sleep(100)) {
      if (this.host.exitCode !== null || Date.now() > until) throw new Error(`no host came up:\n${this.log()}`);
    }
    this.token = readFileSync(join(this.dir, "state/host-token"), "utf8").trim();
    log(`host ${this.host.pid} on port ${this.port()}, in ${this.dir}`);
  }

  port() {
    try {
      return JSON.parse(readFileSync(join(this.dir, "state/host.lock"), "utf8")).port || 0;
    } catch {
      return 0;
    }
  }

  log() {
    return readFileSync(join(this.dir, "up.log"), "utf8").slice(-4000);
  }

  /** One wsp line against the host, which has to succeed; what it printed. */
  wsp(...args) {
    const ran = spawnSync(process.execPath, [this.bin, ...args, "--state", this.state], { cwd: this.dir, env: this.env, encoding: "utf8", timeout: 120_000 });
    const said = `${ran.stdout}${ran.stderr}`;
    if (ran.status !== 0) throw new Error(`wsp ${args.join(" ")} (${ran.status ?? ran.signal}): ${said}\nhost log:\n${this.log()}`);
    return said;
  }

  /** Every thread the host lists, as `wsp threads --json` prints them. */
  list() {
    return this.wsp("threads", "--json")
      .split("\n")
      .filter(line => line.startsWith("{"))
      .flatMap(line => JSON.parse(line).threads ?? []);
  }

  git(...args) {
    const ran = spawnSync("git", ["-c", "user.name=bench", "-c", "user.email=bench@example.com", "-c", "commit.gpgsign=false", ...args], { cwd: this.folder, env: this.env, encoding: "utf8" });
    if (ran.status !== 0) throw new Error(`git ${args.join(" ")}: ${ran.stderr}`);
  }

  /** The project, the skill, the conversation kept outside wsp and short-1's ten turns. */
  seed() {
    this.folder = join(this.dir, "bench");
    for (const [path, text] of Object.entries(PROJECT_FILES)) {
      mkdirSync(dirname(join(this.folder, path)), { recursive: true });
      writeFileSync(join(this.folder, path), text);
    }
    this.git("init", "-q", "-b", "main");
    this.git("add", "-A");
    this.git("commit", "-q", "-m", "bench");
    this.wsp("add", this.folder, "--name", "bench");
    mkdirSync(join(this.dir, ".claude/skills/tidy-imports"), { recursive: true });
    writeFileSync(join(this.dir, ".claude/skills/tidy-imports/SKILL.md"), SKILL);
    this.seedConversation();
    const turns = turnsOf("short-1");
    this.threads["short-1"] = this.run(turns[0].prompt);
    for (const turn of turns.slice(1)) this.wsp("send", this.threads["short-1"], turn.prompt);
    log(`short-1 is ${this.threads["short-1"]}, ${turns.length} turns`);
  }

  /** A conversation Claude Code kept in the project's folder, as a person's terminal session leaves one. */
  seedConversation() {
    const id = "5f0c9a52-7b1e-4d0a-9c3e-2a6b8d4e1f07";
    const dir = join(this.dir, ".claude/projects", this.folder.replace(/[^A-Za-z0-9]/g, "-"));
    const at = new Date(Date.now() - 3 * 60 * 60 * 1000);
    const line = (o, s) => JSON.stringify({ parentUuid: null, isSidechain: false, cwd: this.folder, sessionId: id, version: "2.1.295", entrypoint: "cli", gitBranch: "main", timestamp: new Date(at.getTime() + s * 1000).toISOString(), ...o });
    const reply = { id: "msg_1", type: "message", role: "assistant", model: "claude-sonnet-5", content: [{ type: "text", text: "The test scrolls before the rows are measured, so it passes only when the measure wins the race." }] };
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, `${id}.jsonl`), [line({ type: "user", uuid: "u1", message: { role: "user", content: CONVERSATION_TITLE } }, 0), line({ type: "assistant", uuid: "a1", parentUuid: "u1", message: reply }, 20)].join("\n") + "\n");
    this.conversation = id;
  }

  /** A new thread on the project with `prompt` as its first message; its id. */
  run(prompt, ...flags) {
    const said = this.wsp("run", "bench", "--agent", "claude", ...flags, prompt);
    const id = /\bthread ([0-9a-f-]{36})\b/.exec(said)?.[1];
    if (id === undefined) throw new Error(`no thread id in what wsp run said: ${said}`);
    return id;
  }

  /** The row `wsp threads` prints for a thread. */
  row(id) {
    return this.list().find(t => t.threadId === id || t.id === id);
  }

  async until(what, test, ms = 30_000) {
    for (const until = Date.now() + ms; ; await sleep(250)) {
      const got = test();
      if (got) return got;
      if (Date.now() > until) throw new Error(`${what} not seen in ${ms / 1000} s:\n${this.log()}`);
    }
  }

  /** A thread whose first turn runs `prompt` and is still running it. */
  async running(name, prompt, ...flags) {
    if (this.threads[name] !== undefined) return this.threads[name];
    const id = this.run(prompt, "--detach", ...flags);
    this.threads[name] = id;
    await this.until(`${name} running`, () => this.row(id)?.status === "running" && this.row(id)?.capped === undefined);
    log(`${name} is ${id}`);
    return id;
  }

  runningIds() {
    return this.list().filter(t => t.status === "running").map(t => t.threadId ?? t.id);
  }

  /** Interrupts every running turn until none runs or 20 s pass. */
  async interruptAll() {
    for (const until = Date.now() + 20_000; Date.now() < until; await sleep(300)) {
      let ids;
      try {
        ids = this.runningIds();
      } catch {
        return;
      }
      if (ids.length === 0) return;
      for (const id of ids) spawnSync(process.execPath, [this.bin, "stop", id, "--state", this.state], { cwd: this.dir, env: this.env, timeout: 20_000 });
    }
  }

  /** Interrupts the turns, stops the host by its pid, then any turn the stub marked as still running. */
  async stop() {
    if (this.host !== undefined && this.host.exitCode === null) {
      await this.interruptAll();
      process.kill(this.host.pid, "SIGTERM");
      for (const until = Date.now() + 20_000; this.host.exitCode === null && Date.now() < until; ) await sleep(100);
      if (this.host.exitCode === null) process.kill(this.host.pid, "SIGKILL");
    }
    const turns = join(this.dir, "turns");
    for (const pid of existsSync(turns) ? readdirSync(turns).map(Number) : []) {
      // A pid outlives its process and can be reused, so it is signalled only while it still runs the stub.
      const args = (() => {
        try {
          return readFileSync(`/proc/${pid}/cmdline`, "utf8");
        } catch {
          return "";
        }
      })();
      if (args.includes(join(this.dir, "bin", "claude"))) process.kill(pid, "SIGTERM");
    }
  }

  remove() {
    if (existsSync(join(this.dir, MARK))) rmSync(this.dir, { recursive: true, force: true });
  }
}

/** playwright-core, as the web app's playwright depends on it. */
function playwright() {
  const web = createRequire(join(REPO, "apps/web/package.json"));
  return createRequire(web.resolve("playwright/package.json"))("playwright-core");
}

const QUIET = "[data-sonner-toaster], [data-sonner-toast] { display: none !important; }";
const BARE = `
[data-chat-view] > :not([data-chat-composer-dock], [data-chat-composer], :has([data-chat-composer])),
[data-scroll-to-end] { visibility: hidden !important; }
`;

/** The browser and the page every state is photographed in, one context per state so no state sees another's
 * drafts, stash or favourites. */
class Browser {
  constructor(fixture) {
    this.fixture = fixture;
  }

  async launch() {
    const env = { PATH: "/usr/local/bin:/usr/bin:/bin", HOME: join(this.fixture.dir, "chrome-home"), TZ: "UTC" };
    if (process.env.DISPLAY) env.DISPLAY = process.env.DISPLAY;
    mkdirSync(env.HOME, { recursive: true });
    // A real device scale, as Electron on a Retina Mac draws: Playwright's emulated one rounds font metrics in CSS px.
    const args = ["--force-device-scale-factor=2", "--window-size=1280,800"];
    this.browser = await playwright().chromium.launch({ executablePath: process.env.CHROME ?? "/usr/bin/google-chrome", headless: true, env, args });
  }

  async open(theme = THEMES[0]) {
    this.context = await this.browser.newContext({ viewport: null, colorScheme: theme.scheme, timezoneId: "UTC" });
    await this.context.addInitScript(token => window.localStorage.setItem("wsp:device-token", token), this.fixture.token);
    const page = await this.context.newPage();
    await page.goto(`http://127.0.0.1:${this.fixture.port()}/`, { waitUntil: "domcontentloaded" });
    // Headless Chrome keeps room for a window frame it does not draw, so the window grows by what the page lacks.
    const [w, h] = await page.evaluate(() => [innerWidth, innerHeight]);
    const cdp = await this.context.newCDPSession(page);
    const { windowId } = await cdp.send("Browser.getWindowForTarget");
    await cdp.send("Browser.setWindowBounds", { windowId, bounds: { width: 2560 - w, height: 1600 - h } });
    await cdp.detach();
    const screen = await page.evaluate(() => [innerWidth, innerHeight, devicePixelRatio]);
    if (screen.join() !== "1280,800,2") throw new Error(`the page is ${screen.join(" x ")}, not 1280 x 800 at 2`);
    await page.addStyleTag({ content: QUIET });
    await page.locator("[data-shell-center]").first().waitFor({ state: "visible", timeout: 30_000 });
    this.page = page;
    return page;
  }

  async close() {
    await this.context?.close();
    this.context = undefined;
  }
}

/** Opens a thread by its sidebar row and waits for its composer, or for what stands in the composer's place. */
async function openThread(page, id, ready = "[data-chat-composer] [contenteditable='true'], [data-composer-bar]") {
  const row = page.locator(`[data-row-id='thread:${id}']`).first();
  await row.waitFor({ state: "visible", timeout: 30_000 });
  await row.click();
  await page.locator(ready).first().waitFor({ state: "visible", timeout: 30_000 });
  await settle(page);
}

/** Opens the project's New thread page. */
async function openHome(page) {
  await page.getByRole("button", { name: /^New thread/ }).first().click();
  await page.locator("[data-k='project-home'] [data-chat-composer]").first().waitFor({ state: "visible", timeout: 30_000 });
  await settle(page);
}

const editor = page => page.locator("[data-chat-composer] [contenteditable='true']").first();

async function settle(page, ms = 400) {
  await page.evaluate(() => document.fonts.ready);
  await sleep(ms);
}

/** The boxes and the words a capture records, read in the page. */
function measure() {
  const box = el => {
    if (el === null || el === undefined) return null;
    const r = el.getBoundingClientRect();
    return r.width === 0 && r.height === 0 ? null : { x: Math.round(r.x * 100) / 100, y: Math.round(r.y * 100) / 100, width: Math.round(r.width * 100) / 100, height: Math.round(r.height * 100) / 100 };
  };
  const shown = el => el.checkVisibility({ visibilityProperty: true, opacityProperty: true }) && el.closest("[aria-hidden='true'],[inert]") === null;
  const shell = document.querySelector("[data-chat-composer-dock] [data-slot='composer-shell'], [data-chat-composer] [data-slot='composer-shell']");
  const composer = shell?.closest("[data-chat-composer]") ?? shell?.parentElement ?? null;
  const popupSelectors = ["[data-composer-drawer-layer]", "[data-slot='menu-popup']", "[data-slot='popover-popup']", "[data-slot='select-popup']", "[data-slot='combobox-popup']", "[role='menu']", "[role='listbox']", "[role='dialog']"];
  const popups = [];
  for (const el of document.querySelectorAll(popupSelectors.join(","))) {
    if (!shown(el) || composer?.contains(el) && !el.matches("[data-composer-drawer-layer]")) continue;
    if (popups.some(p => p.contains(el))) continue;
    for (let i = popups.length - 1; i >= 0; i--) if (el.contains(popups[i])) popups.splice(i, 1);
    popups.push(el);
  }
  const words = root => {
    const out = [];
    const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
    for (let n = walk.currentNode; n !== null; n = walk.nextNode()) {
      if (n.nodeType === Node.ELEMENT_NODE) {
        if ((n.tagName === "INPUT" || n.tagName === "TEXTAREA") && shown(n)) {
          const said = n.value || n.placeholder;
          if (said?.trim()) out.push(said.trim());
        }
        if (n.dataset?.placeholder !== undefined && shown(n) && n.textContent === "") out.push(n.dataset.placeholder);
        continue;
      }
      const text = n.textContent.replace(/\s+/g, " ").trim();
      if (text === "" || !shown(n.parentElement)) continue;
      const range = document.createRange();
      range.selectNodeContents(n);
      const r = range.getBoundingClientRect();
      if (r.width < 1 || r.height < 1) continue;
      out.push(text);
    }
    return out;
  };
  const inShell = sel => [...(composer?.querySelectorAll(sel) ?? [])].find(shown);
  const banner = inShell("[data-composer-drawer]") ?? inShell("[data-slot='composer-banner']") ?? inShell("[data-composer-stash-drawer]");
  const rects = {
    composer: box(composer),
    shell: box(shell),
    host: box(inShell("[data-slot='composer-host']")),
    tray: box(inShell("[data-slot='composer-tray']")),
    banner: box(banner),
    editor: box(inShell("[contenteditable]")),
    pickers: box(inShell("[data-flip='pickers']")),
    actions: box(inShell("[data-chat-composer-actions='right']")),
    triggers: [...(composer?.querySelectorAll("[data-composer-picker]") ?? [])].filter(shown).map(box),
    popups: popups.map(box).filter(Boolean),
  };
  const outer = [rects.shell ?? rects.composer, ...rects.popups].filter(Boolean);
  let crop = null;
  if (outer.length > 0) {
    const x0 = Math.max(0, Math.min(...outer.map(r => r.x)) - 24);
    const y0 = Math.max(0, Math.min(...outer.map(r => r.y)) - 24);
    const x1 = Math.min(innerWidth, Math.max(...outer.map(r => r.x + r.width)) + 24);
    const y1 = Math.min(innerHeight, Math.max(...outer.map(r => r.y + r.height)) + 24);
    crop = { x: Math.floor(x0), y: Math.floor(y0), width: Math.ceil(x1 - Math.floor(x0)), height: Math.ceil(y1 - Math.floor(y0)) };
  }
  const apart = popups.filter(p => !composer?.contains(p));
  const above = p => shell !== null && p.getBoundingClientRect().bottom <= shell.getBoundingClientRect().top + 1;
  const data = [...apart.filter(above).flatMap(words), ...(composer === null ? [] : words(composer)), ...apart.filter(p => !above(p)).flatMap(words)];
  return { crop, rects, data, dark: document.documentElement.classList.contains("dark"), shown: shell !== null && shown(shell) };
}

/** Writes the state's PNG and JSON in each theme, off the page as the state left it. */
async function capture(page, out, state) {
  await page.addStyleTag({ content: BARE });
  const seen = {};
  for (const theme of THEMES) {
    await page.emulateMedia({ colorScheme: theme.scheme });
    await page.waitForFunction(dark => document.documentElement.classList.contains("dark") === dark, theme.dark, { timeout: 10_000 });
    await settle(page, 500);
    const got = await page.evaluate(measure);
    if (got.crop === null || !got.shown) throw new Error(`${state}: no composer shown on the page`);
    await page.screenshot({ path: join(out, `${state}-${theme.name}.png`), animations: "disabled", caret: "initial" });
    const { dark: _dark, shown: _shown, ...kept } = got;
    writeFileSync(join(out, `${state}-${theme.name}.json`), `${JSON.stringify(kept, null, 2)}\n`);
    seen[theme.name] = kept;
  }
  await page.emulateMedia({ colorScheme: THEMES[0].scheme });
  return seen;
}

async function type(page, lines) {
  await editor(page).click();
  for (const [i, line] of lines.entries()) {
    if (i > 0) await page.keyboard.press("Shift+Enter");
    await page.keyboard.type(line);
  }
}

/** Clicks the model picker, then waits for its popup. */
async function openPicker(page, which) {
  await page.locator(`[data-chat-composer] [data-composer-picker='${which}']`).first().click();
  await settle(page);
}

/** What each state does to a fresh page before it is photographed. */
const STEPS = {
  async empty(page, f) {
    await openThread(page, f.threads["short-1"]);
  },
  async typing(page, f) {
    await openThread(page, f.threads["short-1"]);
    await type(page, TYPED);
  },
  async "mention-open"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await type(page, ["@"]);
    await page.locator("[data-composer-item-id]").first().waitFor({ timeout: 15_000 });
  },
  async "command-open"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await type(page, ["/"]);
    await page.locator("[data-composer-command-drawer]").first().waitFor({ timeout: 15_000 });
  },
  async "model-picker-open"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await openPicker(page, "model");
  },
  async "effort-picker-open"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await openPicker(page, "reasoning");
  },
  async "access-picker-open"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await page.locator("[data-slot='composer-tray'] [data-composer-picker='access']").first().click();
    await settle(page);
  },
  async attachments(page, f) {
    await openThread(page, f.threads["short-1"]);
    const files = [join(f.dir, "files/swatch.png"), join(f.dir, "files/notes.txt")];
    mkdirSync(join(f.dir, "files"), { recursive: true });
    writeFileSync(files[0], swatchPng([217, 119, 87]));
    writeFileSync(files[1], NOTES);
    const cdp = await page.context().newCDPSession(page);
    const { root } = await cdp.send("DOM.getDocument", { depth: -1, pierce: true });
    const { nodeId } = await cdp.send("DOM.querySelector", { nodeId: root.nodeId, selector: "[data-composer-file-input]" });
    await cdp.send("DOM.setFileInputFiles", { nodeId, files });
    await cdp.detach();
    await page.locator("[data-composer-files] li").nth(1).waitFor({ timeout: 15_000 });
    await settle(page);
  },
  async "stash-open"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await type(page, [STASHED]);
    await page.keyboard.press("Control+s");
    await page.locator("[data-composer-stash-word]").first().click();
    await page.locator("[data-composer-stash-drawer]").first().waitFor({ timeout: 15_000 });
    await settle(page);
  },
  async running(page, f) {
    await openThread(page, await f.running("slow", "SLOW: look at why the transcript jumps"));
  },
  async queued(page, f) {
    await openThread(page, await f.running("slow", "SLOW: look at why the transcript jumps"));
    await type(page, [QUEUED]);
    await page.keyboard.press("Enter");
    await page.locator("[data-drawer-row='queue']").first().waitFor({ timeout: 15_000 });
    await settle(page);
  },
  async "queue-bar"(page, f) {
    await STEPS.queued(page, f);
    await page.locator("[data-drawer-row='queue']").first().click();
    await page.locator("[data-composer-bar='queue']").first().waitFor({ timeout: 15_000 });
    await settle(page);
  },
  async banner(page, f) {
    await openThread(page, f.threads["short-1"]);
    await forkLast(page);
  },
  async "new-thread"(page) {
    await openHome(page);
  },
  async history(page, f) {
    await openThread(page, f.threads["short-1"]);
    await editor(page).click();
    await page.keyboard.press("ArrowUp");
    await settle(page);
  },
  async "hash-open"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await type(page, ["#"]);
    await page.locator("[data-composer-command-drawer]").first().waitFor({ timeout: 15_000 });
    await settle(page, 1500);
  },
  async "dollar-open"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await type(page, ["$"]);
    await page.locator("[data-composer-command-drawer]").first().waitFor({ timeout: 15_000 });
  },
  async "fast-open"(page) {
    await openHome(page);
    await openPicker(page, "model");
    await page.locator("[data-composer-option='claude-opus-5-5']").first().click();
    await page.keyboard.press("Escape");
    await settle(page);
    await openPicker(page, "reasoning");
    await page.locator("[data-composer-fast]").first().waitFor({ timeout: 15_000 });
  },
  async "favourites-open"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await openPicker(page, "model");
    await page.locator("[data-composer-favourite='claude-opus-5-5']").first().click();
    await page.getByRole("tab", { name: "Favourites" }).first().click();
    await settle(page);
  },
  async "pinned-picker"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await openPicker(page, "model");
    await page.locator("[data-composer-harness='codex']").first().click();
    await settle(page);
  },
  async "drawer-question"(page, f) {
    await openThread(page, await f.running("question", "QUESTION: clear the build folder", "--access", "ask"), "[data-chat-composer-dock] [data-dock-title]");
    await page.keyboard.press("Escape");
    await page.locator("[data-drawer-row='question']").first().waitFor({ timeout: 15_000 });
    await settle(page);
  },
  async "drawer-limit"(page, f) {
    if (f.threads.limit === undefined) {
      f.threads.limit = f.run("LIMIT: run the long transcript test", "--detach");
      await f.until("limit's turn stopped on the usage limit", () => f.row(f.threads.limit)?.limit !== undefined);
    }
    await openThread(page, f.threads.limit);
    await page.locator("[data-drawer-row='usage']").first().waitFor({ timeout: 15_000 });
    await settle(page);
  },
  async "drawer-slot"(page, f) {
    if (f.threads.slot === undefined) {
      await f.running("slow", "SLOW: look at why the transcript jumps");
      f.wsp("computers", "set", "here", "--threads", String(f.runningIds().length));
      f.threads.slot = f.run("SLOW: pin the scroll anchor before the tool output mounts", "--detach");
      await f.until("slot's turn held back", () => f.row(f.threads.slot)?.capped !== undefined);
    }
    await openThread(page, f.threads.slot);
    await page.locator("[data-drawer-row='capped']").first().waitFor({ timeout: 15_000 });
    await settle(page);
  },
  async "drawer-tasks"(page, f) {
    await openThread(page, await f.running("tasks", "TASKS: pin the scroll anchor"));
    await page.locator("[data-drawer-row='tasks']").first().waitFor({ timeout: 15_000 });
    await settle(page);
  },
  async "resume-banner"(page, f) {
    await openHome(page);
    await page.keyboard.press("Control+k");
    const search = page.locator("[role='dialog'] input").first();
    await search.waitFor({ timeout: 15_000 });
    await search.fill("Resume a conversation");
    await page.locator("[role='dialog'] [role='option']", { hasText: "Resume a conversation" }).first().click();
    const row = page.locator("[data-resume-picker] [role='option']", { hasText: CONVERSATION_TITLE }).first();
    await row.waitFor({ timeout: 30_000 });
    await row.click();
    await page.locator("[data-k='resume-draft'] [data-resume-banner]").first().waitFor({ timeout: 15_000 });
    await settle(page, 1000);
  },
  async "paste-file"(page, f) {
    await openThread(page, f.threads["short-1"]);
    await editor(page).click();
    await page.evaluate(text => {
      const data = new DataTransfer();
      data.setData("text/plain", text);
      document.activeElement.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
    }, PASTED);
    await page.locator("[data-composer-files] li").first().waitFor({ timeout: 15_000 });
    await settle(page);
  },
};

/** Forks short-1 from its last message, as the message's own action does. */
async function forkLast(page) {
  const last = page.locator("[data-message-role='assistant']").last();
  await last.hover();
  await last.getByRole("button", { name: "Fork from here" }).click();
  await page.locator("[data-k='fork-confirm']").click();
  await page.locator("[data-fork-banner]").first().waitFor({ timeout: 15_000 });
  await settle(page);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  mkdirSync(args.out, { recursive: true });
  const fixture = new Fixture();
  const browser = new Browser(fixture);
  let stopping = false;
  const stop = async () => {
    if (stopping) return;
    stopping = true;
    await browser.browser?.close().catch(() => {});
    if (!args.keepHost) {
      await fixture.stop();
      fixture.remove();
    }
  };
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) process.once(signal, () => void stop().finally(() => process.exit(1)));
  const done = [];
  const failed = [];
  try {
    await fixture.start();
    fixture.seed();
    await browser.launch();
    const order = [...args.only.filter(s => s !== "disabled"), ...args.only.filter(s => s === "disabled")];
    for (const state of order) {
      if (state === "disabled" && args.keepHost) {
        failed.push({ state, why: "--keep-host leaves the host up, and disabled needs it stopped" });
        continue;
      }
      try {
        const page = await browser.open();
        if (state === "disabled") {
          await openThread(page, fixture.threads["short-1"]);
          await fixture.stop();
          await page.locator("[data-chat-composer] [contenteditable='false']").first().waitFor({ timeout: 30_000 });
          await settle(page, 1000);
        } else {
          await STEPS[state](page, fixture);
        }
        await capture(page, args.out, state);
        done.push(state);
        log(`${state} captured`);
      } catch (e) {
        failed.push({ state, why: e instanceof Error ? e.message.split("\n")[0] : String(e) });
        log(`${state} failed: ${e instanceof Error ? e.message : e}`);
        if (browser.page !== undefined) await browser.page.screenshot({ path: join(args.out, `${state}-failed.png`) }).catch(() => {});
      } finally {
        if (state === "drawer-slot" && fixture.host.exitCode === null) fixture.wsp("computers", "set", "here", "--reset", "threads");
        await browser.close();
      }
    }
  } finally {
    if (args.keepHost) log(`host ${fixture.host?.pid} left serving ${fixture.state} on port ${fixture.port()}`);
    await stop();
  }
  log(`captured ${done.length}: ${done.join(", ") || "none"}`);
  for (const { state, why } of failed) log(`not captured: ${state}: ${why}`);
  process.exit(failed.length === 0 ? 0 : 1);
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();

export { Browser, Fixture, capture, measure, openHome, openThread };
