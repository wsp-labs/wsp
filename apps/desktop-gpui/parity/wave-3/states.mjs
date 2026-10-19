// SPDX-License-Identifier: AGPL-3.0-only
// Pages come from Q, run in bench's folder so the thread's port watch lists it, and P, run here, held by no thread.
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { dirname, join } from "node:path";

const Q = 47801;
const C = 47802;
const P = 47803;
const PANEL = "[data-preview-panel-mode]";
const FIELD = "css:[data-preview-url-input]";
const css = selector => `css:${selector}`;
const homeOf = fixture => dirname(dirname(fixture.state));

/** Both servers' pages: one that never answers, a long title, one that opens a window, and the rest; Q gets its source. */
function answer(req, res) {
  const page = (title, body) =>
    `<!doctype html><title>${title}</title><body style="margin:0;padding:40px;font:16px/1.5 system-ui,sans-serif;color:#18181b;background:#fff"><h1 style="margin:0 0 12px;font-size:28px">${title}</h1>${body}</body>`;
  if (req.url === "/hold") return;
  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  if (req.url === "/popup") return res.end(page("Opens a window", "<script>window.open('/second')</script>"));
  if (req.url.startsWith("/long/")) return res.end(page(`A page whose title runs on ${"and on ".repeat(24)}`, `<p>${req.url}</p>`));
  res.end(page(req.url === "/second" ? "Second page" : "Parity page", `<p>${req.url}</p><p>Served for the parity baselines.</p>`));
}

const answers = port => fetch(`http://127.0.0.1:${port}/`, { signal: AbortSignal.timeout(2000) }).then(() => true, () => false);
for (const port of [Q, C]) if (await answers(port)) throw new Error(`port ${port} is held by another process; free it for the browser states`);
const own = createServer(answer);
await new Promise((done, fail) => own.once("error", fail).listen(P, "127.0.0.1", done));
own.unref();

let q;
/** Q in bench's folder, started once and again after a state stopped it; stopped by its pid when the driver ends. */
async function serveQ(fixture) {
  if (q !== undefined && q.exitCode === null && q.signalCode === null) return;
  q = spawn(process.execPath, ["-e", `require("node:http").createServer(${answer}).listen(${Q}, "127.0.0.1")`], {
    cwd: join(homeOf(fixture), "bench"),
    env: { PATH: "/usr/bin:/bin" },
    stdio: "ignore",
  });
  const child = q;
  process.once("exit", () => child.exitCode === null && child.signalCode === null && child.kill());
  for (let tries = 0; !(await answers(Q)); tries++) {
    if (child.exitCode !== null || tries > 50) throw new Error(`the page server on ${Q} did not come up`);
    await new Promise(done => setTimeout(done, 100));
  }
  // The daemon reads the thread's listening ports every 5 s.
  await new Promise(done => setTimeout(done, 6500));
}
async function stopQ() {
  if (q === undefined || q.exitCode !== null || q.signalCode !== null) return;
  const child = q;
  child.kill();
  await new Promise(done => child.once("exit", done));
}

/** A thread of the state's own, so no other state's tabs stand in its panel. */
const thread = (host, id) => host.thread(`Parity state ${id}: the browser pane.`);

/** The thread's Browser pane, opened by its key; `fresh` forgets the addresses earlier states visited. */
async function pane(app, id, fresh = false) {
  await app.route.thread(id);
  if (fresh) await app.page(() => Object.keys(localStorage).forEach(k => k.startsWith("wsp:browser-recents:") && localStorage.removeItem(k)));
  // The key toggles, and a fixture used before may hold this thread's panel open on Browser already.
  if (!(await app.page(() => document.querySelector("[data-browser-surface]") !== null))) await app.key("mod+shift+j");
  await app.waitFor(() => document.querySelector("[data-browser-surface]") !== null, 10_000);
}

async function go(app, place) {
  await app.click(FIELD);
  await app.type(place);
  await app.key("enter");
}

/** Until the shown guest has loaded; a page that never finishes runs out the time. */
async function loaded(app, ms = 15_000) {
  await app.waitFor(
    () => {
      const view = [...document.querySelectorAll("[data-browser-guest]")].find(g => g.getAttribute("aria-hidden") === null)?.querySelector("webview");
      try {
        return view !== undefined && !view.isLoading() && view.getURL() !== "";
      } catch {
        return false;
      }
    },
    ms,
  );
  await app.sleep(800);
}

async function opened(app, id, place, fresh = false) {
  await pane(app, id, fresh);
  await go(app, place);
  await loaded(app);
}

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

/** Closes the open menu by its button: after Escape the button keeps key focus, which shows its tooltip. */
async function shut(app) {
  await app.click("Preview menu");
  await app.park();
  await app.sleep(400);
}

async function home(app) {
  for (let tries = 0; tries < 3; tries++) {
    const at = await app.page(() => {
      if (document.querySelector("[data-browser-surface]") !== null) return "open";
      if (document.querySelector('[data-surface-launch="preview"]') !== null) return "launcher";
      const tab = [...document.querySelectorAll("[data-right-panel-tab-list] button")].find(b => b.textContent.trim() === "Browser");
      if (tab !== undefined) return tab.click(), "tab";
      return document.querySelector('button[aria-label="Add a panel"]') !== null ? "tabs" : "closed";
    });
    if (at === "open") return;
    if (at === "tab") return app.waitFor(() => document.querySelector("[data-browser-surface]") !== null, 10_000);
    if (at === "launcher") return app.click(css('[data-surface-launch="preview"]'));
    if (at === "tabs") {
      await app.click(css('button[aria-label="Add a panel"]'));
      return pick(app, "[role=menuitem]", "Browser");
    }
    await app.click(css('button[aria-label="Toggle right panel"]'));
    await app.sleep(600);
  }
  throw new Error("the Browser pane would not open");
}

async function startup(app, width) {
  await app.route.settings("general");
  if (width !== undefined) await app.width(width);
  await app.waitFor(() => document.querySelector("section[data-settings-card='startup']") !== null, 10_000);
  await app.page(() => document.querySelector("section[data-settings-card='startup']").scrollIntoView({ block: "start" }));
  await app.park();
  await app.sleep(600);
}
const startupCard = async app => {
  await app.page(() => document.querySelector("section[data-settings-card='startup']").scrollIntoView({ block: "start" }));
  return app.page(() => {
    const r = document.querySelector("section[data-settings-card='startup']").getBoundingClientRect();
    return { x: r.left - 16, y: r.top - 16, width: r.width + 32, height: r.height + 32 };
  });
};

const withQ = id => async host => (await serveQ(host.fixture), thread(host, id));

export default [
  { id: "W3-BRW-028", clip: PANEL, data: host => thread(host, "W3-BRW-028"), go: async (app, id) => (await pane(app, id, true), await app.park(), await app.sleep(800)) },
  {
    id: "W3-BRW-029",
    clip: PANEL,
    data: withQ("W3-BRW-029"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${P}/settings`, true);
      await app.click("Preview menu");
      await pick(app, "[role=menuitem]", "New tab");
      await app.waitFor(() => document.querySelector("[data-server-port]") !== null && document.querySelector("[data-recent-url]") !== null, 15_000);
      await app.park();
      await app.sleep(800);
    },
  },
  {
    id: "W3-BRW-030",
    clip: PANEL,
    data: withQ("W3-BRW-030"),
    go: async (app, id) => {
      await pane(app, id);
      await app.click(FIELD);
      await app.type("zzz");
      await app.sleep(500);
    },
  },
  { id: "W3-BRW-031", clip: PANEL, data: withQ("W3-BRW-031"), go: async (app, id) => (await opened(app, id, `localhost:${Q}/`), await app.park()) },
  { id: "W3-BRW-032", clip: PANEL, data: host => thread(host, "W3-BRW-032"), go: async (app, id) => (await opened(app, id, `localhost:${P}/`), await app.park(), await app.sleep(1500)) },
  {
    id: "W3-BRW-033",
    clip: PANEL,
    data: withQ("W3-BRW-033"),
    go: async (app, id) => {
      await pane(app, id);
      await go(app, `localhost:${Q}/hold`);
      await app.park();
      await app.sleep(6000);
    },
  },
  {
    id: "W3-BRW-034",
    clip: PANEL,
    data: host => thread(host, "W3-BRW-034"),
    go: async (app, id) => {
      await pane(app, id);
      await go(app, `localhost:${C}/`);
      await app.park();
      await app.sleep(3000);
    },
  },
  {
    id: "W3-BRW-035",
    clip: PANEL,
    data: withQ("W3-BRW-035"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/`);
      await stopQ();
      await app.waitFor(() => document.querySelector("[data-browser-surface]")?.textContent.includes("is not answering"), 15_000);
      await app.park();
      await app.sleep(800);
    },
  },
  { id: "W3-BRW-036", clip: PANEL, data: host => thread(host, "W3-BRW-036"), go: async (app, id) => (await opened(app, id, `parity.localhost:${P}/`), await app.park()) },
  {
    id: "W3-BRW-037",
    clip: PANEL,
    data: withQ("W3-BRW-037"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/`);
      await app.click("Preview menu");
      await app.park();
      await app.sleep(500);
    },
  },
  {
    id: "W3-BRW-111",
    clip: PANEL,
    data: host => thread(host, "W3-BRW-111"),
    go: async (app, id) => {
      await pane(app, id, true);
      await app.click("Preview menu");
      await app.park();
      await app.sleep(500);
    },
  },
  {
    id: "W3-BRW-038",
    clip: PANEL,
    data: withQ("W3-BRW-038"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/`);
      await app.click("Preview menu");
      await app.click("Zoom in");
      await app.click("Zoom in");
      await shut(app);
      await app.sleep(2500);
    },
  },
  {
    id: "W3-BRW-112",
    clip: PANEL,
    data: withQ("W3-BRW-112"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/`);
      await app.click("Preview menu");
      await app.click("Zoom in");
      await shut(app);
      await app.sleep(300);
    },
  },
  {
    id: "W3-BRW-039",
    clip: PANEL,
    data: withQ("W3-BRW-039"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/`);
      await app.hover(FIELD);
    },
  },
  {
    id: "W3-BRW-113",
    clip: PANEL,
    data: withQ("W3-BRW-113"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/`);
      await app.click("Copy URL");
      await app.sleep(200);
    },
  },
  {
    id: "W3-BRW-040",
    clip: PANEL,
    data: withQ("W3-BRW-040"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/`);
      await app.click(FIELD);
      await app.park();
      await app.sleep(400);
    },
  },
  {
    id: "W3-BRW-120",
    clip: PANEL,
    data: withQ("W3-BRW-120"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${P}/settings`, true);
      await app.click("Preview menu");
      await pick(app, "[role=menuitem]", "New tab");
      await app.waitFor(() => document.querySelector("[data-recent-url]") !== null, 15_000);
      await app.hover(css("[data-recent-url]"));
    },
  },
  { id: "W3-BRW-041", clip: PANEL, data: host => thread(host, "W3-BRW-041"), go: async (app, id) => (await opened(app, id, `parity.localhost:${P}/long/${"x".repeat(300)}`), await app.park()) },
  {
    id: "W3-BRW-042",
    clip: PANEL,
    data: withQ("W3-BRW-042"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/1`);
      for (let n = 2; n <= 6; n++) {
        await app.click(css('button[aria-label="Add a panel"]'));
        await pick(app, "[role=menuitem]", "Browser");
        await go(app, `localhost:${Q}/${n}`);
        await loaded(app);
      }
      await app.park();
    },
  },
  {
    id: "W3-BRW-043",
    clip: PANEL,
    data: withQ("W3-BRW-043"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/popup`);
      await app.sleep(2000);
      await app.park();
    },
  },
  {
    id: "W3-BRW-044",
    data: withQ("W3-BRW-044"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/1`);
      for (let n = 2; n <= 10; n++) {
        await app.click(css('button[aria-label="Add a panel"]'));
        await pick(app, "[role=menuitem]", "Browser");
        await go(app, `localhost:${Q}/${n}`);
        await loaded(app);
      }
      await app.click(css('button[aria-label="Add a panel"]'));
      await pick(app, "[role=menuitem]", "Browser");
      await app.waitFor(() => document.querySelector("[data-sonner-toast]")?.textContent.includes("the most wsp keeps"), 10_000);
      await app.park();
      await app.sleep(400);
    },
  },
  {
    id: "W3-BRW-045",
    width: 980,
    data: withQ("W3-BRW-045"),
    go: async (app, id) => (await opened(app, id, `localhost:${Q}/`), await app.park()),
  },
  {
    id: "W3-BRW-046",
    width: 980,
    data: withQ("W3-BRW-046"),
    go: async (app, id) => {
      await opened(app, id, `localhost:${Q}/`);
      await app.click("Preview menu");
      await app.park();
      await app.sleep(500);
    },
  },
  {
    id: "W3-BRW-114",
    data: withQ("W3-BRW-114"),
    go: async (app, id) => (await opened(app, id, `localhost:${Q}/`), await app.width(759), await app.page(() => document.activeElement?.blur()), await app.park(), await app.sleep(1000)),
  },
  {
    id: "W3-BRW-048",
    clip: PANEL,
    go: async app => {
      await app.route.home();
      await home(app);
      await go(app, `127.0.0.1:${P}`);
      await app.park();
      await app.sleep(2000);
    },
  },
  {
    id: "W3-SHL-066",
    clip: "[data-sonner-toast]",
    go: async app => {
      await app.route.home();
      // The page reads a link off its hash only as it loads.
      await app.page(() => ((location.hash = "#open/thread/nope"), setTimeout(() => location.reload(), 50)));
      await app.sleep(1500);
      await app.waitFor(() => document.body.textContent.includes("names a thread this wsp does not have"), 10_000);
      await app.park();
      await app.sleep(500);
    },
  },
  { id: "W3-SWT-011", clip: startupCard, go: app => startup(app) },
  { id: "W3-SWT-012", clip: startupCard, data: host => host.pref("keepAwake", false), go: app => startup(app) },
  { id: "W3-SWT-016", clip: startupCard, go: app => startup(app, 600) },
];
