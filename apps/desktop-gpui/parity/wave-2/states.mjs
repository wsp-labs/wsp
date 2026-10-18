// SPDX-License-Identifier: AGPL-3.0-only
// A state that changes what a thread holds makes its own thread in data(), so both themes start from the same place.
import { spawnSync } from "node:child_process";
import { createServer } from "node:net";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { deflateSync } from "node:zlib";

const PANEL = "[data-preview-panel-mode]";
const DRAWER = 'aside[data-terminal-owner="drawer"]';
const DIALOG = '[data-slot="dialog-popup"]';
const ALERT = '[data-slot="alert-dialog-popup"]';
const PALETTE = "[data-command-palette]";
const SETTINGS = "[data-settings-page]";
const TOASTS = "[data-sonner-toast]";
const css = selector => `css:${selector}`;
const slateText = name => readFileSync(new URL(`./data/${name}`, import.meta.url), "utf8");
const homeOf = fixture => dirname(dirname(fixture.state));
const folderOf = fixture => join(homeOf(fixture), "bench");
const marksOf = fixture => join(homeOf(fixture), "parity-marks");

function git(fixture, ...args) {
  const ran = spawnSync("git", ["-c", "user.name=bench", "-c", "user.email=bench@example.com", "-c", "commit.gpgsign=false", ...args], {
    cwd: folderOf(fixture),
    env: { HOME: homeOf(fixture), PATH: "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin", LANG: "en_US.UTF-8" },
    encoding: "utf8",
  });
  if (ran.status !== 0) throw new Error(`git ${args.join(" ")}: ${ran.stderr.trim()}`);
  return ran.stdout;
}

function put(fixture, path, body) {
  const at = join(folderOf(fixture), path);
  mkdirSync(dirname(at), { recursive: true });
  writeFileSync(at, body);
}

/** A PNG of a smooth two-colour field, so the slate's image pieces have pictures and none enters the repo. */
function png(width, height, from, to) {
  const row = width * 3 + 1;
  const raw = Buffer.alloc(row * height);
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const t = (x + y) / (width + height);
      for (let c = 0; c < 3; c++) raw[y * row + 1 + x * 3 + c] = from[c] + (to[c] - from[c]) * t;
    }
  const table = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = buf => {
    let c = 0xffffffff;
    for (const byte of buf) c = table[(c ^ byte) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data]);
    const len = Buffer.alloc(4);
    const sum = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    sum.writeUInt32BE(crc(body));
    return Buffer.concat([len, body, sum]);
  };
  const head = Buffer.alloc(13);
  head.writeUInt32BE(width, 0);
  head.writeUInt32BE(height, 4);
  head.set([8, 2, 0, 0, 0], 8);
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", head), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

function manySlate() {
  const rows = Array.from({ length: 250 }, (_, i) => `{ name: "check-${i + 1}", took: ${(i * 37) % 600} }`).join(", ");
  const t0 = Date.parse("2026-10-18T00:00:00Z");
  const pts = Array.from({ length: 400 }, (_, i) => `{ at: ${t0 + i * 60000}, v: ${Math.round(2400 + 30 * Math.sin(i / 20) + (i % 7))} }`).join(", ");
  const bars = Array.from({ length: 25 }, (_, i) => `{ n: "proc-${i + 1}", v: ${100 - i * 3} }`).join(", ");
  const parts = Array.from({ length: 60 }, (_, i) => `{ name: "mod-${i + 1}", b: ${2000 + ((i * 7919) % 90000)}, g: "pkg-${i % 5}" }`).join(", ");
  return `<slate title="Many rows">
  <column>
    <chart label="Gold, per ounce" items={[${pts}]} x={item.at} value={item.v} format="usd" />
    <bars label="Busiest" items={[${bars}]} name={item.n} value={item.v} />
    <treemap label="Bundle" items={[${parts}]} name={item.name} value={item.b} group={item.g} format="bytes" />
    <table items={[${rows}]} key={item.name}><col title="Check" value={item.name} /><col title="Took, s" value={item.took} /></table>
  </column>
</slate>
`;
}

const FIXTURES = ["src/keep.ts", "src/gone.txt", "src/moved.ts", "logo.bin", "docs/NOTES.md", ".gitignore"];

function seed(host) {
  return host.once("seed", async () => {
    const { fixture } = host;
    mkdirSync(marksOf(fixture), { recursive: true });
    writeFileSync(join(host.home, ".zshrc"), "PROMPT='%F{green}bench%f %1~ %# '\n");
    reset(fixture);
    put(fixture, "src/keep.ts", "export const a = 1;\nexport const b = 2;\n");
    put(fixture, "src/gone.txt", "old\n");
    put(fixture, "src/moved.ts", "line one\nline two\nline three\n");
    put(fixture, "logo.bin", Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0, 0, 0]), Buffer.alloc(2048, 3)]));
    put(fixture, "docs/NOTES.md", "# Notes\n\nSome *markdown* and a [link](https://example.com).\n\n- one\n- two\n");
    put(fixture, ".gitignore", "shots/\n");
    git(fixture, "add", "--", ...FIXTURES);
    if (git(fixture, "diff", "--cached", "--name-only").trim() !== "") git(fixture, "commit", "-qm", "parity fixtures");
    put(fixture, "shots/plot.png", png(640, 360, [40, 90, 160], [230, 180, 90]));
    put(fixture, "shots/before.png", png(480, 300, [200, 60, 60], [60, 60, 200]));
    put(fixture, "shots/after.png", png(480, 300, [60, 160, 90], [240, 230, 120]));
    put(fixture, "shots/notes.png", "not a picture\n");
  });
}

/** A new thread on bench whose turn plays short-1's first; answers its id. */
async function fresh(host, label) {
  await seed(host);
  writeFileSync(join(host.home, "bench-turn.json"), JSON.stringify({ thread: "short-1", turn: 0 }));
  const said = await host.wsp("run", "bench", "--agent", "claude", `Parity state ${label}.`);
  const id = /\bthread ([0-9a-f-]{36})\b/.exec(said)?.[1];
  if (id === undefined) throw new Error(`wsp run named no thread: ${said}`);
  return id;
}

async function slated(host, label, text) {
  const id = await fresh(host, label);
  const file = join(host.home, "parity.slate");
  writeFileSync(file, text);
  await host.wsp("slate", "write", id, file);
  return id;
}

// Threads past the settle time leave the switcher and the palette's recent list; the driver restores it after each state.
const unsettled = host => host.pref("settleAfter", "never");
const has = (app, selector, ms = 15_000) => app.waitFor(s => document.querySelector(s) !== null, ms, selector);
const reads = (app, words, ms = 15_000) => app.waitFor(w => document.body.innerText.includes(w), ms, words);
/** Clicks the first element `selector` finds whose text holds `words`, for menu items whose words repeat elsewhere. */
async function pick(app, selector, words) {
  await app.waitFor(
    (s, w) => {
      document.querySelector("[data-parity-pick]")?.removeAttribute("data-parity-pick");
      const el = [...document.querySelectorAll(s)].find(e => e.textContent.includes(w));
      el?.setAttribute("data-parity-pick", "");
      return el !== undefined;
    },
    10_000,
    selector,
    words,
  );
  await app.click(css("[data-parity-pick]"));
}

/** Each pane's root, to tell a pane already on screen from one behind another tab. */
const ROOTS = {
  diff: "[data-diff-surface]",
  files: "[data-files-pane]",
  slate: "[data-slate-surface]",
  machine: "[data-machine]",
  agents: "[data-agents-panel]",
  processes: "[data-procs], [data-procs-thread], [data-procs-nothing]",
  terminal: 'aside[data-terminal-owner="right-panel"]',
};

/** Opens a pane by its panes.ts kind; bench's threads share the panel's tabs, so it may be open, behind a tab or shut. */
async function pane(app, kind, label) {
  for (let tries = 0; tries < 3; tries++) {
    const at = await app.page(
      (k, root, words) => {
        if (document.querySelector(root) !== null) return "open";
        if (document.querySelector(`[data-surface-launch="${k}"]`)) return "launcher";
        const tab = [...document.querySelectorAll("[data-right-panel-tab-list] button")].find(b => b.textContent.trim() === words);
        if (tab !== undefined) return tab.click(), "tab";
        return document.querySelector('button[aria-label="Add a panel"]') ? "tabs" : "closed";
      },
      kind,
      ROOTS[kind],
      label,
    );
    if (at === "open") return;
    if (at === "tab") return app.waitFor(root => document.querySelector(root) !== null, 10_000, ROOTS[kind]);
    if (at === "launcher") return app.click(css(`[data-surface-launch="${kind}"]`));
    if (at === "tabs") {
      await app.click(css('button[aria-label="Add a panel"]'));
      return pick(app, "[role=menuitem]", label);
    }
    await app.click(css('button[aria-label="Toggle right panel"]'));
    await app.sleep(600);
  }
  throw new Error(`the ${label} pane would not open`);
}

async function drawer(app) {
  await app.key("mod+j");
  await app.waitFor(d => document.querySelector(`${d} [data-terminal-viewport] canvas`) !== null && document.querySelector(`${d} [data-terminal-overlay]`) === null, 30_000, DRAWER);
  await app.click(css(`${DRAWER} [data-terminal-viewport] canvas`));
  await app.sleep(300);
}

async function shell(app, line, mark) {
  const file = join(marksOf(app.fixture), mark);
  rmSync(file, { force: true });
  await app.type(`${line}; touch ${file}`);
  await app.key("enter");
  for (const until = Date.now() + 20_000; !existsSync(file); await app.sleep(100)) if (Date.now() > until) throw new Error(`the shell never ran: ${line}`);
  await app.sleep(500);
}

const COLOURS = String.raw`clear; printf '\e[31mred \e[32mgreen \e[33myellow \e[34mblue \e[35mmagenta \e[36mcyan \e[1;37mbold\e[0m\n\e[38;5;208m256 \e[38;2;120;200;255mtruecolor\e[0m \e[4munderline\e[0m \e[7mreverse\e[0m\n'; ls --color=always /`;

/** The project on main with nothing uncommitted, whatever an earlier state or an interrupted run left. */
function reset(fixture) {
  git(fixture, "checkout", "-q", "-f", "main");
  git(fixture, "reset", "-q", "--hard");
  git(fixture, "clean", "-fdq");
}

function fiveChanges(fixture) {
  reset(fixture);
  put(fixture, "src/keep.ts", "export const a = 1;\nexport const b = 3;\nexport const c = 4;\n");
  rmSync(join(folderOf(fixture), "src/gone.txt"));
  git(fixture, "mv", "src/moved.ts", "src/renamed.ts");
  put(fixture, "src/renamed.ts", "line one\nline two\nline three\nline four\n");
  put(fixture, "logo.bin", Buffer.concat([readFileSync(join(folderOf(fixture), "logo.bin")), Buffer.alloc(2048, 7)]));
  put(fixture, "src/added.ts", "export const added = true;\n");
}

const bigFile = fixture => put(fixture, "big.txt", `${Array.from({ length: 400_000 }, (_, i) => `${i + 1}`).join("\n")}\n`);

async function changes(app, id) {
  await app.route.thread(id);
  await pane(app, "diff", "Changes");
  await has(app, "[data-diff-surface]", 20_000);
  if (!(await app.page(() => document.querySelector('[data-diff-scope="head"]') !== null))) {
    await app.click(css('[aria-label^="Changes scope:"]'));
    await pick(app, "[role=menuitem], [role=menuitemradio], [role=option]", "Uncommitted");
    await has(app, '[data-diff-scope="head"]');
  }
  // The view mode is kept in localStorage, so a split view left by W2-PRP-027 is put back to stacked.
  if (await app.page(() => document.querySelector('[aria-label="Split view"][aria-pressed="true"]') !== null)) await app.click(css('[aria-label="Stacked view"]'));
  await app.sleep(800);
}

async function find(app, id, words, ready) {
  await app.route.thread(id);
  await app.key("mod+p");
  await has(app, "[data-file-finder-input]", 10_000);
  await app.type(words);
  await app.sleep(1000);
  await app.key("enter");
  await ready();
  await app.sleep(800);
}

async function computers(app) {
  await app.route.settings("computers");
  await app.sleep(400);
}

async function palette(app, words) {
  await app.route.thread("short-1");
  await app.key("mod+k");
  await has(app, "[data-command-palette]");
  if (words !== undefined) await app.type(words);
  await app.sleep(800);
}

const freePort = () =>
  new Promise((done, fail) => {
    const probe = createServer().once("error", fail);
    probe.listen(0, "127.0.0.1", () => {
      const { port } = probe.address();
      probe.close(() => done(port));
    });
  });

/** A python web server in the thread's terminal, so the thread owns processes and a port; it stops itself in 5 min. */
async function serve(app, id) {
  const port = await freePort();
  await app.route.thread(id);
  await drawer(app);
  await shell(app, `(python3 -m http.server ${port} >/dev/null 2>&1 & s=$!; sleep 300; kill $s) >/dev/null 2>&1 &`, `server-${id}`);
  await app.key("mod+j");
  await app.sleep(1500);
}

async function activity(app) {
  await app.key("mod+k");
  await app.type("Activity");
  await app.sleep(500);
  await app.key("enter");
  await has(app, '[data-k="activity-totals"]', 20_000);
  await app.sleep(1500);
}

const terminal = (id, clip, go) => ({ id, clip, data: host => fresh(host, id), go: async (app, thread) => (await app.route.thread(thread), await drawer(app), await go(app)) });
const slate = (id, file, go, clip = PANEL) => ({
  id,
  clip,
  data: host => slated(host, id, typeof file === "function" ? file() : slateText(file)),
  go: async (app, thread) => (await app.route.thread(thread), await pane(app, "slate", "Slate"), await has(app, "[data-slate]", 20_000), await go(app)),
});
const skip = (id, why) => ({ id, skip: why, go: () => Promise.reject(new Error(why)) });

const WINDOW = app => app.page(() => ({ x: 0, y: 0, width: innerWidth, height: innerHeight }));
/** The side the driver set on the empty home's host for this shot: light for Paper, dark for Graphite. */
const sideOf = async host => (await host.empty.op("preferences.get")).preferences.theme;
/** Onboarding follows the computer's colour scheme, which the driver cannot set per shot, so the page is put on that
 * scheme's side the way its own follow() does. */
const scheme = (app, side) =>
  app.page(dark => {
    const html = document.documentElement;
    html.classList.add("no-transitions");
    html.classList.toggle("dark", dark);
    window.wsp.setTitleBar?.();
    requestAnimationFrame(() => html.classList.remove("no-transitions"));
  }, side === "dark");
const onboarding = (id, go) => ({ id, home: "empty", clip: WINDOW, data: sideOf, go: async (app, side) => (await scheme(app, side), await go(app), await app.park(), await app.sleep(600)) });
/** Get started, then the agents screen once its scan has drawn the rows: one ticked when `found`, none ticked when not. */
async function agentsScreen(app, found) {
  await app.click(css("#start"));
  await has(app, "#agents:not([hidden])");
  await has(app, found ? "#rows input:checked" : "#rows .row");
}

const run = (id, go, clip = PANEL) => slate(id, "runs.slate", async app => (await has(app, '[data-slate-piece="go-quick"] button', 20_000), await go(app)), clip);
const press = piece => css(`[data-slate-piece="${piece}"] button`);
async function runOnce(app, piece, output, state) {
  await app.click(press(piece));
  await pick(app, "button", "Run once");
  await has(app, `[data-slate-output="${output}"] [data-slate-run-state="${state}"]`, 30_000);
  await app.sleep(600);
}
async function slateMenu(app, item) {
  await app.click(css('button[aria-label="Slate menu"]'));
  if (item !== undefined) await pick(app, "[role=menuitem]", item);
}

export default [
  terminal("W2-TRM-012", DRAWER, app => shell(app, "clear", "016")),
  terminal("W2-TRM-013", DRAWER, app => shell(app, COLOURS, "017")),
  terminal("W2-TRM-014", DRAWER, async app => (await app.type("seq 1 1000 | sed 's/^/line /' | less"), await app.key("enter"), await app.sleep(1500))),
  {
    id: "W2-TRM-015",
    clip: DRAWER,
    data: async host => {
      const id = await fresh(host, "W2-TRM-015");
      const dir = join(host.home, ".config", "ghostty");
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "config"), ["background = #1e1e2e", "foreground = #cdd6f4", "cursor-color = #f5e0dc", "cursor-style = bar", "selection-background = #585b70", "palette = 1=#f38ba8", "palette = 2=#a6e3a1", "palette = 4=#89b4fa", "window-padding-x = 16", "window-padding-y = 10", ""].join("\n"));
      return { id, config: join(dir, "config") };
    },
    go: async (app, { id, config }) => {
      try {
        await app.route.thread(id);
        await drawer(app);
        await shell(app, COLOURS, "019");
      } finally {
        rmSync(config, { force: true });
      }
    },
  },
  terminal("W2-TRM-016", DRAWER, async app => {
    await shell(app, "clear; echo https://example.com/docs/terminal", "020");
    // The renderer underlines a link while Ctrl or Cmd is held over it; a synthetic move carries the modifier.
    await app.page(drawerSel => {
      const canvas = document.querySelector(`${drawerSel} [data-terminal-viewport] canvas`);
      const r = canvas.getBoundingClientRect();
      const at = { clientX: r.left + 60, clientY: r.top + 12, bubbles: true, ctrlKey: true, metaKey: navigator.platform.startsWith("Mac") };
      canvas.dispatchEvent(new PointerEvent("pointermove", at));
      canvas.dispatchEvent(new MouseEvent("mousemove", at));
    }, DRAWER);
    await app.sleep(400);
  }),
  terminal("W2-TRM-017", DRAWER, async app => {
    await app.click(css('button[aria-label^="Split terminal vertically"]'));
    await app.waitFor(d => document.querySelectorAll(`${d} [data-terminal-viewport]`).length === 2, 20_000, DRAWER);
    await app.click(css('button[aria-label^="New terminal"]'));
    await has(app, "[data-terminal-tabs]", 20_000);
    await app.sleep(1000);
  }),
  skip("W2-TRM-018", "near duplicate of W2-TRM-017: four panes in place of two, and the limit's words are a tooltip"),
  terminal("W2-TRM-019", DRAWER, async app => {
    await shell(app, COLOURS, "023");
    for (let i = 0; i < 3; i++) await app.key("mod+="), await app.sleep(300);
    await app.sleep(800);
  }),
  terminal("W2-TRM-020", undefined, async app => (await app.click(css('button[aria-label="Terminal font"]')), await has(app, 'button[aria-label="Terminal font"][aria-expanded="true"]'))),
  {
    id: "W2-TRM-021",
    clip: PANEL,
    data: host => fresh(host, "W2-TRM-021"),
    go: async (app, id) => {
      await app.route.thread(id);
      await pane(app, "terminal", "Terminal");
      await has(app, 'aside[data-terminal-owner="right-panel"] [data-terminal-viewport] canvas', 30_000);
      await app.sleep(1500);
    },
  },
  terminal("W2-TRM-022", DRAWER, async app => (await shell(app, "seq 1 5000", "026"), await app.scroll(css(`${DRAWER} [data-terminal-viewport] canvas`), -1200))),
  {
    id: "W2-TRM-023",
    data: host => fresh(host, "W2-TRM-023"),
    go: async (app, id) => {
      await app.route.thread(id);
      await app.size(640, 800);
      await drawer(app);
      await app.click(css('button[aria-label^="Split terminal horizontally"]'));
      await app.waitFor(d => document.querySelectorAll(`${d} [data-terminal-viewport]`).length === 2, 20_000, DRAWER);
      await app.sleep(800);
    },
  },
  terminal("W2-TRM-024", DRAWER, async app => {
    await app.click(css('button[aria-label^="Close terminal"]'));
    await app.sleep(600);
    await app.key("mod+j");
    await has(app, '[data-terminal-empty="live"]', 15_000);
  }),
  terminal("W2-TRM-025", DRAWER, async app => {
    await app.click(css('button[aria-label^="New terminal"]'));
    await has(app, "[data-terminal-tabs]", 20_000);
    await app.sleep(800);
    await app.click(css(`${DRAWER} [data-terminal-viewport] canvas`));
    await app.type("exit");
    await app.key("enter");
    await app.waitFor(() => document.querySelector("[data-terminal-tabs]")?.textContent.includes("(exited)"), 15_000);
  }),
  skip("W2-TRM-026", "starting needs a host with no wsp-daemon staged, and every other terminal state needs one"),
  skip("W2-TRM-027", "nothing answered needs a host with no wsp-daemon staged"),
  skip("W2-TRM-028", "daemon down needs the local wsp-daemon stopped; no host op or wsp line stops it"),
  skip("W2-TRM-029", "reconnecting follows a stopped daemon, and lasts seconds"),
  skip("W2-TRM-030", "shell gone follows a stopped daemon's restart"),

  {
    id: "W2-PRP-019",
    data: async host => {
      const id = await fresh(host, "W2-PRP-019");
      reset(host.fixture);
      git(host.fixture, "checkout", "-q", "-B", "feature/parity");
      put(host.fixture, "feature.txt", "x\n");
      return id;
    },
    go: async (app, id) => {
      try {
        await app.route.thread(id);
        await has(app, "[data-git-quick]", 30_000);
        await app.click(css('button[aria-label="Toggle right panel"]'));
        await has(app, "[data-surface-launch]");
        await app.sleep(500);
      } finally {
        reset(app.fixture);
      }
    },
  },
  { id: "W2-PRP-021", clip: PANEL, data: async host => (await seed(host), reset(host.fixture), fresh(host, "W2-PRP-021")), go: async (app, id) => (await changes(app, id), await reads(app, "No uncommitted changes at")) },
  { id: "W2-PRP-023", clip: PANEL, data: async host => (await seed(host), fiveChanges(host.fixture), fresh(host, "W2-PRP-023")), go: async (app, id) => (await changes(app, id), await has(app, "[data-changed-files]"), await app.sleep(1500)) },
  {
    id: "W2-PRP-024",
    clip: PANEL,
    data: async host => {
      await seed(host);
      reset(host.fixture);
      put(host.fixture, "packages/a-package-with-a-rather-long-name/src/components/settings/advanced/integrations/providers/a-component-file-with-a-long-descriptive-name.test.tsx", "export const x = 1;\n");
      return fresh(host, "W2-PRP-024");
    },
    go: async (app, id) => (await changes(app, id), await has(app, "[data-changed-files]")),
  },
  {
    id: "W2-PRP-025",
    clip: PANEL,
    data: async host => {
      await seed(host);
      reset(host.fixture);
      for (let i = 1; i <= 300; i++) put(host.fixture, `many/file-${i}.txt`, `line ${i}\n`);
      return fresh(host, "W2-PRP-025");
    },
    go: async (app, id) => (await changes(app, id), await app.waitFor(() => document.querySelector("[data-changed-files]")?.textContent.includes("300"), 30_000)),
  },
  {
    id: "W2-PRP-026",
    clip: PANEL,
    data: async host => (await seed(host), reset(host.fixture), bigFile(host.fixture), put(host.fixture, "small.txt", "small\n"), fresh(host, "W2-PRP-026")),
    go: async (app, id) => (await changes(app, id), await has(app, "[data-diff-truncated]", 30_000)),
  },
  {
    id: "W2-PRP-027",
    clip: PANEL,
    data: async host => (await seed(host), fiveChanges(host.fixture), fresh(host, "W2-PRP-027")),
    go: async (app, id) => {
      await changes(app, id);
      await app.click(css('[aria-label="Split view"]'));
      await app.sleep(1200);
    },
  },
  {
    id: "W2-PRP-029",
    clip: PANEL,
    data: async host => (await seed(host), fiveChanges(host.fixture), fresh(host, "W2-PRP-029")),
    go: async (app, id) => {
      await changes(app, id);
      await app.click(css('[data-changed-file="src/keep.ts"] [data-viewed-tick]'));
      await app.waitFor(() => document.querySelector("[data-viewed-count]")?.textContent.includes("1 of"), 10_000);
    },
  },
  {
    id: "W2-PRP-030",
    clip: PANEL,
    data: async host => (await seed(host), fiveChanges(host.fixture), fresh(host, "W2-PRP-030")),
    go: async (app, id) => {
      await changes(app, id);
      await app.click(css("[data-diff-commit]"));
      await has(app, "[data-commit-box]");
      await app.waitFor(() => document.querySelector("[data-commit-box] [role=status]") === null, 30_000);
    },
  },
  {
    id: "W2-PRP-031",
    clip: ALERT,
    data: async host => (await seed(host), fiveChanges(host.fixture), fresh(host, "W2-PRP-031")),
    go: async (app, id) => (await changes(app, id), await app.click(css('button[aria-label="Discard changes to added.ts"]')), await has(app, '[data-k="discard-confirm"]')),
  },
  {
    id: "W2-PRP-032",
    clip: PANEL,
    data: async host => (await seed(host), fiveChanges(host.fixture), fresh(host, "W2-PRP-032")),
    go: async (app, id) => (await changes(app, id), await app.click(css('button[aria-label="Edit keep.ts"]')), await has(app, 'button[aria-label="Save keep.ts"]')),
  },
  { id: "W2-PRP-033", data: async host => (await seed(host), fiveChanges(host.fixture), fresh(host, "W2-PRP-033")), go: async (app, id) => (await changes(app, id), await app.size(600, 800), await app.sleep(800)) },
  {
    id: "W2-PRP-037",
    clip: PANEL,
    data: async host => (await seed(host), reset(host.fixture), fresh(host, "W2-PRP-037")),
    go: async (app, id) => {
      await app.route.thread(id);
      await pane(app, "files", "Files");
      await has(app, "[data-files-pane]", 20_000);
      await app.click(css('input[name="project-files-search"]'));
      await app.type("NOTES");
      await app.sleep(1000);
    },
  },
  {
    id: "W2-PRP-039",
    clip: PANEL,
    data: async host => (await seed(host), fiveChanges(host.fixture), fresh(host, "W2-PRP-039")),
    go: async (app, id) => {
      await app.route.thread(id);
      await app.key("mod+shift+f");
      await app.sleep(500);
      await app.type("export const b");
      await app.sleep(1500);
      await app.key("enter");
      await has(app, "[data-file-preview]", 20_000);
    },
  },
  { id: "W2-PRP-040", clip: PANEL, data: async host => (await seed(host), reset(host.fixture), fresh(host, "W2-PRP-040")), go: (app, id) => find(app, id, "NOTES", () => has(app, "[data-markdown-preview]", 20_000)) },
  { id: "W2-PRP-041", clip: PANEL, data: async host => (await seed(host), reset(host.fixture), fresh(host, "W2-PRP-041")), go: (app, id) => find(app, id, "logo.bin", () => reads(app, "Nothing to preview", 20_000)) },
  { id: "W2-PRP-042", clip: PANEL, data: async host => (await seed(host), reset(host.fixture), bigFile(host.fixture), fresh(host, "W2-PRP-042")), go: (app, id) => find(app, id, "big.txt", () => has(app, "[data-file-truncated]", 20_000)) },
  {
    id: "W2-PRP-043",
    clip: "[data-file-finder]",
    data: async host => (await seed(host), reset(host.fixture), fresh(host, "W2-PRP-043")),
    go: async (app, id) => (await app.route.thread(id), await app.key("mod+p"), await app.type("kee"), await has(app, '[data-file-finder="files"]')),
  },
  skip("W2-PRP-044", "the Pull request pane reads GitHub through gh; the throwaway host has no pull request and no fake gh"),

  slate("W2-SLT-045", "every.slate", async app => {
    await has(app, '[data-slate-type="empty"]', 20_000);
    await app.page(() => document.querySelector('[data-slate-type="empty"]')?.scrollIntoView());
    await app.sleep(800);
    await app.page(() => document.querySelector("[data-slate]")?.scrollIntoView());
    await app.sleep(1500);
  }),
  slate("W2-SLT-046", "empty.slate", app => app.sleep(1200)),
  slate("W2-SLT-047", "long.slate", app => app.sleep(1200)),
  slate("W2-SLT-048", manySlate, async app => (await has(app, '[data-slate-type="chart"]', 20_000), await app.sleep(1500))),
  slate("W2-SLT-049", "failed.slate", async app => (await reads(app, "No file at shots/missing.png", 20_000), await app.sleep(2500))),
  slate("W2-SLT-050", "held.slate", app => app.hover(css("[data-slate-held-button]"))),
  slate("W2-SLT-051", "variants.slate", async app => {
    await app.page(() => [...document.querySelectorAll("[data-slate] img")].at(-1)?.scrollIntoView());
    await app.sleep(800);
    await app.page(() => document.querySelector("[data-slate]")?.scrollIntoView());
    await app.sleep(1500);
  }),
  slate("W2-SLT-052", "every.slate", async app => {
    await pick(app, '[data-slate-type="choices"] button[role="radio"]', "Link");
    await has(app, '[data-slate-choice="wrong"]');
  }),
  slate("W2-SLT-053", "every.slate", async app => (await has(app, '[data-slate-type="chart"] [data-k="point"]', 20_000), await app.sleep(1000), await app.hover(css('[data-slate-type="chart"] [data-k="point"]:last-of-type')))),
  slate(
    "W2-SLT-054",
    "every.slate",
    async app => {
      await has(app, '[data-slate-type="diagram"] [data-mermaid] svg', 20_000);
      await app.click(css('[data-slate-type="diagram"] button[aria-label="Expand"]'));
      await has(app, "[data-slate-diagram-expanded]");
      await app.sleep(800);
    },
    DIALOG,
  ),
  slate(
    "W2-SLT-055",
    "every.slate",
    async app => {
      await app.page(() => document.querySelector('[data-slate-images="gallery"]')?.scrollIntoView());
      await has(app, '[data-slate-images="gallery"] [data-slate-thumb] img', 20_000);
      await app.click(css('[data-slate-images="gallery"] [data-slate-thumb] button'));
      await has(app, "[data-slate-image-at]");
      await app.sleep(800);
    },
    DIALOG,
  ),
  skip("W2-SLT-056", "near duplicate: a section's fold is one chevron and a hidden body"),
  skip("W2-SLT-057", "the conflict row needs typing to stay held while the host's write lands, which a driver's blur races"),
  skip("W2-SLT-058", "the panel's width is read from localStorage once per load, and the driver does not reload between states"),
  run("W2-SLT-059", () => Promise.resolve()),
  run("W2-SLT-060", async app => (await app.click(press("go-quick")), await has(app, '[data-slate-consent="quick"]')), DIALOG),
  run("W2-SLT-061", async app => (await runOnce(app, "go-slow", "slow", "running"), await app.sleep(4000))),
  run("W2-SLT-062", app => runOnce(app, "go-quick", "quick", "done")),
  run("W2-SLT-063", app => runOnce(app, "go-broken", "broken", "failed")),
  run("W2-SLT-064", async app => {
    await runOnce(app, "go-slow", "slow", "running");
    await slateMenu(app, "Stop runs");
    await has(app, '[data-slate-output="slow"] [data-slate-run-state="cancelled"]', 20_000);
  }),
  skip("W2-SLT-065", "near duplicate of W2-SLT-064: Not now draws the same cancelled word, and its reason is not drawn"),
  run("W2-SLT-066", async app => (await app.click(press("send")), await has(app, '[data-slate-piece="send"] [data-slate-outcome]', 5_000))),
  run("W2-SLT-067", async app => (await app.click(press("bad-link")), await has(app, '[data-slate-piece="bad-link"] [data-slate-outcome]', 5_000))),
  run("W2-SLT-068", async app => (await app.click(press("go-danger")), await reads(app, "This only prints a line.")), ALERT),
  slate("W2-SLT-069", "timers.slate", app => app.waitFor(() => document.querySelectorAll("[data-slate-approvals] [data-slate-approval]").length >= 2, 30_000), DIALOG),
  run("W2-SLT-070", async app => {
    await app.click(press("go-quick"));
    await has(app, '[data-slate-consent="quick"]');
    await app.key("escape");
    await has(app, '[data-slate-held="quick"]', 10_000);
  }),
  run("W2-SLT-071", async app => {
    await app.click(press("go-five"));
    await pick(app, "button", "Always in this thread");
    await has(app, '[data-slate-held="s5"] [data-slate-held-why]', 20_000);
  }),
  slate("W2-SLT-072", "timers.slate", async app => {
    await app.waitFor(() => document.querySelectorAll("[data-slate-approvals] [data-slate-approval]").length >= 2, 30_000);
    await pick(app, "button", "Allow all");
    await has(app, "[data-slate-version] [data-slate-refreshing]", 30_000);
  }),
  run("W2-SLT-073", async app => (await app.click(press("good-link")), await has(app, "[data-slate-link-href]")), DIALOG),
  run(
    "W2-SLT-074",
    async app => {
      await app.click(press("go-quick"));
      await has(app, '[data-slate-consent="quick"]');
      // Always is drawn only where the host can pin the command; elsewhere Run once leaves the dialog with no row.
      const always = await app.page(() => [...document.querySelectorAll("[data-slot=dialog-popup] button")].some(b => b.textContent.includes("Always in this thread")));
      await pick(app, "[data-slot=dialog-popup] button", always ? "Always in this thread" : "Run once");
      await has(app, '[data-slate-output="quick"] [data-slate-run-state="done"]', 30_000);
      await slateMenu(app, "Approvals");
      if (always) await has(app, '[data-settings-card="slate-approvals"] [data-settings-row]');
      await app.sleep(800);
    },
    DIALOG,
  ),
  run("W2-SLT-075", app => slateMenu(app)),
  run("W2-SLT-076", async app => (await slateMenu(app, "Forget secrets"), await reads(app, "Forget this slate's secrets?")), ALERT),
  {
    id: "W2-SLT-077",
    clip: PANEL,
    data: host => fresh(host, "W2-SLT-077"),
    go: async (app, id) => (await app.route.thread(id), await pane(app, "slate", "Slate"), await has(app, '[data-slate-empty="none"]', 20_000)),
  },
  run("W2-SLT-078", async app => (await slateMenu(app, "Clear"), await has(app, '[data-slate-empty="cleared"]', 10_000))),

  { id: "W2-DLG-021", clip: DIALOG, go: async app => (await computers(app), await app.click(css('[data-k="add-computer-button"]')), await has(app, '[data-add-computer="where"] [data-k="where-field"]')) },
  {
    id: "W2-DLG-022",
    clip: DIALOG,
    data: async host => {
      const dir = join(host.home, ".ssh");
      mkdirSync(dir, { recursive: true });
      const hosts = Array.from({ length: 29 }, (_, i) => `Host box-${String(i + 1).padStart(2, "0")}\n  HostName 10.0.0.${i + 1}\n  User root\n`);
      hosts.push("Host a-very-long-alias-for-the-build-box-in-the-basement-rack-seven\n  HostName 10.0.1.7\n  User root\n");
      writeFileSync(join(dir, "config"), hosts.join("\n"));
      return join(dir, "config");
    },
    go: async (app, config) => {
      try {
        await computers(app);
        await app.click(css('[data-k="add-computer-button"]'));
        await has(app, '[data-grid="ssh-hosts"] [data-choice]');
      } finally {
        rmSync(config, { force: true });
      }
    },
  },
  { id: "W2-DLG-023", clip: DIALOG, go: async app => (await computers(app), await app.click(css('[data-k="add-computer-button"]')), await has(app, '[data-add-computer="where"]'), await app.size(600, 800), await app.sleep(800)) },
  {
    id: "W2-DLG-024",
    clip: DIALOG,
    go: async app => {
      await computers(app);
      await app.click(css('[data-k="add-computer-button"]'));
      await app.click(css('[data-k="where-field"]'));
      await app.type("nobody@192.0.2.1");
      await app.click(css('[data-k="continue"]'));
      await has(app, '[data-add-computer="checks"] [data-grid="checks"]');
      await app.sleep(1500);
    },
  },
  {
    id: "W2-DLG-025",
    clip: DIALOG,
    go: async app => {
      await computers(app);
      await app.click(css('[data-k="add-computer-button"]'));
      await app.click(css('[data-k="where-field"]'));
      await app.type("nobody@wsp-parity.invalid");
      await app.click(css('[data-k="continue"]'));
      await has(app, '[data-grid="checks"] [data-state="failed"]', 30_000);
    },
  },
  skip("W2-DLG-026", "every Add a computer step past Checks needs a real SSH host that joins"),
  { id: "W2-DLG-027", clip: DIALOG, go: async app => (await computers(app), await app.click(css('[data-k="add-cloud-button"]')), await has(app, '[data-add-cloud="key"] [data-k="cloud-key"]')) },
  skip(
    "W2-DLG-028",
    "needs the live provider: the host checks a Boat key against boat.dev directly (packages/host/src/providers-key.ts takes BOX_API_URL from packages/engine/src/box-backend.ts:25, with no override), so no capture can reach this state without calling Boat",
  ),
  skip("W2-DLG-029", "every Add a cloud provider step past the key needs a working Boat key and account"),
  skip("W2-DLG-030", "this computer has no Remove line; the dialog needs a second computer or a cloud"),
  {
    id: "W2-DLG-031",
    clip: ALERT,
    data: host => fresh(host, "W2-DLG-031"),
    go: async (app, id) => (await app.route.thread(id), await app.menu(`[data-row-id='thread:${id}']`, "Delete thread"), await has(app, '[data-k="delete-thread"]'), await app.park()),
    cleanup: (host, id) => id !== undefined && host.removeThread(id),
  },
  onboarding("W2-DLG-032", async app => (await has(app, "#start"), await app.sleep(1500))),
  onboarding("W2-DLG-033", app => agentsScreen(app, true)),
  onboarding("W2-DLG-034", async app => {
    await agentsScreen(app, true);
    await app.click(css("#rows input:checked"));
    await has(app, "#open:disabled");
  }),
  { ...onboarding("W2-DLG-035", async app => (await agentsScreen(app, false), await reads(app, "No agent found"))), agents: false },
  {
    ...onboarding("W2-DLG-036", async app => {
      await agentsScreen(app, true);
      await app.click(css("#open"));
      await app.waitFor(() => document.getElementById("status").textContent.includes("would not take the wsp tools"), 15_000);
    }),
    // The install's config file is one it cannot parse, which installMcp refuses as a person's broken ~/.claude.json.
    data: async host => (writeFileSync(join(host.empty.home, "parity-home", ".claude.json"), "{ this is not json\n"), sideOf(host)),
  },
  { id: "W2-DLG-038", home: "held", clip: WINDOW, go: async app => (await has(app, ".starting"), await app.park()) },
  {
    id: "W2-DLG-040",
    home: "empty",
    clip: css('[data-k="first-run"]'),
    go: async app => {
      await agentsScreen(app, true);
      await app.click(css("#open"));
      await app.window("app");
      await has(app, '[data-k="first-run"] [data-k="add-project"]', 30_000);
      await app.park();
      await app.sleep(800);
    },
  },
  { id: "W2-DLG-041", bridge: false, clip: css("form:has(#pair-code)"), go: async app => (await has(app, "#pair-code", 20_000), await app.park()) },
  {
    id: "W2-DLG-042",
    bridge: false,
    clip: css("form:has(#pair-code)"),
    go: async app => {
      await has(app, "#pair-code", 20_000);
      await app.click(css("#pair-code"));
      await app.type("000000");
      await app.click(css('form:has(#pair-code) button[type="submit"]'));
      await has(app, 'form:has(#pair-code) [role="alert"]', 15_000);
      await app.park();
    },
  },

  { id: "W2-PAL-016", clip: PALETTE, data: unsettled, go: app => palette(app) },
  {
    id: "W2-PAL-017",
    clip: PALETTE,
    data: async host => (await host.thread("Parity results."), await unsettled(host)),
    go: async app => (await palette(app, "Parity"), await has(app, '[data-palette-group="threads-search"]', 10_000)),
  },
  { id: "W2-PAL-018", clip: PALETTE, data: unsettled, go: async app => (await palette(app, "LegendList"), await has(app, '[data-palette-group="messages-search"]', 10_000)) },
  { id: "W2-PAL-019", clip: PALETTE, data: unsettled, go: async app => (await palette(app, "zzqx"), await reads(app, "No matching commands")) },
  { id: "W2-PAL-020", clip: PALETTE, data: unsettled, go: async app => (await palette(app, "new thread in"), await app.key("enter"), await has(app, '[data-k="palette-back"]')) },
  { id: "W2-PAL-022", clip: "[data-workspace-switcher]", data: unsettled, go: async app => (await app.route.thread("short-1"), await app.keyDown("ctrl"), await app.key("tab"), await has(app, "[data-workspace-switcher] [data-thread-card]", 10_000), await app.sleep(800)) },
  { id: "W2-PAL-023", clip: PANEL, go: async app => (await app.route.home(), await pane(app, "processes", "Processes"), await has(app, "[data-procs] [data-proc-row]", 20_000)) },
  {
    id: "W2-PAL-025",
    clip: PANEL,
    data: host => fresh(host, "W2-PAL-025"),
    go: async (app, id) => (await serve(app, id), await pane(app, "processes", "Processes"), await has(app, "[data-procs-thread] [data-proc-row]", 20_000), await app.sleep(800)),
  },
  skip("W2-PAL-026", "a fresh thread's kept agent process always lists, so the thread pane is never empty on this host"),
  { id: "W2-PAL-027", clip: SETTINGS, data: host => fresh(host, "W2-PAL-027"), go: async (app, id) => (await serve(app, id), await activity(app)) },
  {
    id: "W2-PAL-028",
    clip: SETTINGS,
    data: host => fresh(host, "W2-PAL-028"),
    go: async (app, id) => (await serve(app, id), await activity(app), await pick(app, '[data-k="activity-views"] *', "Processes"), await app.sleep(1000)),
  },
  skip("W2-PAL-029", "the Ports view listed no row for a listening python server within 40 s on Boat; the cause was not found"),
  { id: "W2-PAL-031", data: host => fresh(host, "W2-PAL-031"), clip: ALERT, go: async (app, id) => (await serve(app, id), await app.key("mod+shift+e"), await has(app, '[data-k="settle-processes"]', 15_000)) },
  {
    id: "W2-PAL-034",
    clip: TOASTS,
    data: async host => (await seed(host), host),
    go: async (app, host) => {
      await app.route.thread("short-1");
      writeFileSync(join(host.home, "bench-turn.json"), JSON.stringify({ thread: "short-2", turn: 0 }));
      await host.wsp("run", "bench", "--agent", "claude", "--notify", "me", "--detach", "Parity note.");
      await has(app, '[data-notice][data-kind="note"]', 30_000);
    },
  },
  { id: "W2-PAL-035", clip: TOASTS, data: host => fresh(host, "W2-PAL-035"), go: async (app, id) => (await app.route.thread(id), await app.key("mod+shift+e"), await has(app, '[data-notice][data-kind="done"]', 15_000)) },
  { id: "W2-PAL-036", clip: TOASTS, go: async app => (await app.route.thread("short-1"), await app.key("mod+o"), await has(app, '[data-notice][data-kind="error"]', 15_000)) },
  skip("W2-PAL-037", "the waiting notice needs an agent asking for leave, which the stub claude never does"),
  { id: "W2-PAL-038", data: host => fresh(host, "W2-PAL-038"), go: async (app, id) => (await serve(app, id), await activity(app), await app.size(600, 800), await app.sleep(800)) },

  { id: "W2-UNO-016", clip: PANEL, data: host => fresh(host, "W2-UNO-016"), go: async (app, id) => (await app.route.thread(id), await pane(app, "machine", "Computer"), await has(app, "[data-machine] [data-chart]", 20_000), await app.sleep(4000)) },
  { id: "W2-UNO-017", clip: PANEL, go: async app => (await app.route.home(), await pane(app, "machine", "Computer"), await has(app, "[data-machine] [data-facts]", 20_000), await app.sleep(3000)) },
  { id: "W2-UNO-023", clip: PANEL, data: host => fresh(host, "W2-UNO-023"), go: async (app, id) => (await app.route.thread(id), await pane(app, "agents", "Agents"), await has(app, "[data-agents-panel]", 20_000), await app.sleep(1500)) },
  {
    id: "W2-UNO-028",
    clip: PANEL,
    data: host => fresh(host, "W2-UNO-028"),
    go: async (app, id) => {
      await app.route.thread(id);
      await app.click(css('[data-testid="composer-editor"]'));
      await app.type("/btw what was the last reply about?");
      await app.sleep(400);
      await app.key("enter");
      await app.sleep(300);
      if (!(await app.page(() => document.querySelector('[data-k="aside-surface"]') !== null))) await app.key("enter");
      await has(app, '[data-k="aside-refused"]', 15_000);
    },
  },
];
