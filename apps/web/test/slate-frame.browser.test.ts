// SPDX-License-Identifier: AGPL-3.0-only
// The frame piece drawn from slate text over a folder of HTML pages, at the panel's two widths in graphite and paper:
// a page sits on the panel with no ground of its own, follows its own height up to the cap and opens large; and a page
// that tries every forbidden thing reaches nothing. The hostile page aims at a listener this file owns (HTTP and
// WebSocket on one port, a UDP port for STUN), so a leak would show there and not on a host that happens to listen
// on the page's own 4400. The anatomy figure is its repo's dial indicator where SLATE_FRAME_ANATOMY names the file,
// else a stand-in; with SLATE_FRAME_OUT set each case is also shot there. Runs only when asked for (WSP_RENDER=1).
import { createSocket, type Socket } from "node:dgram";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { Browser, Frame, Page } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { launchRender, renderSkipped, stopRender } from "./render-browser";
import { FRAME_FILES } from "./slate-render/frames";
import { serveHarness, type Harness } from "./slate-render/serve";

const PAGES = join(dirname(fileURLToPath(import.meta.url)), "slate-render/pages");
const ANATOMY = process.env["SLATE_FRAME_ANATOMY"];
const OUT = process.env["SLATE_FRAME_OUT"];
const skipped = renderSkipped;
if (skipped !== undefined) console.info(`slate frame render test skipped: ${skipped}`);

/** What an agent changes in the committed figure to hand it to a slate: no ground or page margins of its own, and
 * the slate's scheme. */
const handed = (html: string): string =>
  html.replace("</head>", "<style>.iso.iso[data-theme]{--anatomy-page:transparent}.iso-page{width:100%;margin:0}</style></head>").replace(/<body class="iso" data-theme="[a-z]+"([^>]*)>/, `<body class="iso"$1><script>document.body.dataset.theme=document.documentElement.dataset.scheme</script>`);

const STAND_IN = `<!doctype html><html><head><style>body{font:13px var(--font-sans)}svg{width:100%;height:auto}</style></head><body class="iso" data-theme="dark"><svg viewBox="0 0 600 380"><rect x="40" y="40" width="520" height="300" rx="12" fill="none" stroke="var(--border)"/><circle cx="300" cy="190" r="90" fill="none" stroke="var(--foreground)"/></svg></body></html>`;

describe.skipIf(skipped !== undefined)("the slate's frame piece in Chromium", () => {
  let harness: Harness | undefined;
  let browser: Browser | undefined;
  let folder: string | undefined;
  let target: Server | undefined;
  let stun: Socket | undefined;
  /** Everything that reached this file's listener from any page: the ground truth of a leak. */
  const reached: string[] = [];

  beforeAll(async () => {
    target = createServer((req, res) => {
      reached.push(`http ${req.method} ${req.url}`);
      res.end("x");
    });
    target.on("upgrade", (req, socket) => {
      reached.push(`websocket ${req.url}`);
      socket.destroy();
    });
    await new Promise<void>(done => target!.listen(0, "127.0.0.1", done));
    stun = createSocket("udp4");
    stun.on("message", () => reached.push("udp stun"));
    await new Promise<void>(done => stun!.bind(0, "127.0.0.1", done));
    const port = (target.address() as { port: number }).port;
    folder = mkdtempSync(join(tmpdir(), "wsp-slate-frames-"));
    const put = (path: string, text: string) => {
      mkdirSync(dirname(join(folder!, path)), { recursive: true });
      writeFileSync(join(folder!, path), text);
    };
    put(FRAME_FILES.hostile, readFileSync(join(PAGES, "hostile.html"), "utf8").replaceAll("127.0.0.1:4400", `127.0.0.1:${port}`).replace("stun:127.0.0.1:3478", `stun:127.0.0.1:${stun.address().port}`));
    put(FRAME_FILES.explainer, readFileSync(join(PAGES, "explainer.html"), "utf8"));
    const anatomy = ANATOMY !== undefined ? readFileSync(ANATOMY, "utf8") : STAND_IN;
    put(FRAME_FILES.anatomyAsCommitted, anatomy);
    put(FRAME_FILES.anatomy, handed(anatomy));
    browser = await launchRender();
    harness = await serveHarness(folder);
    if (OUT !== undefined) mkdirSync(OUT, { recursive: true });
  }, 120_000);
  afterAll(async () => {
    await stopRender(browser, undefined);
    harness?.stop();
    target?.close();
    stun?.close();
    if (folder !== undefined) rmSync(folder, { recursive: true, force: true });
  });

  const open = async (doc: string, w: number, theme: string, before?: () => void): Promise<Page> => {
    const page = await browser!.newPage({ viewport: { width: Math.max(w + 40, 1200), height: 1100 }, deviceScaleFactor: OUT === undefined ? 1 : 2, reducedMotion: "reduce" });
    if (before !== undefined) await page.addInitScript(before);
    await page.goto(`${harness!.base}?w=${w}&doc=${doc}&theme=${theme}`);
    await page.waitForSelector("[data-slate-frame]");
    await page.waitForFunction(() => document.querySelector("[data-slate-frame-page], [data-slate-frame-waiting]:not(:has(svg))") !== null, undefined, { timeout: 15_000 });
    return page;
  };
  /** The page itself, two frames under the panel. */
  const pageFrame = async (page: Page, has: string): Promise<Frame> => {
    for (let i = 0; i < 100; i++) {
      for (const f of page.frames()) if (f !== page.mainFrame() && (await f.$(has).catch(() => null)) !== null) return f;
      await page.waitForTimeout(100);
    }
    throw new Error(`no frame holds ${has}`);
  };
  const shoot = async (page: Page, name: string, whole = false) => {
    if (OUT === undefined) return;
    if (whole) await page.screenshot({ path: join(OUT, `${name}.png`) });
    else await page.locator("[data-panel]").screenshot({ path: join(OUT, `${name}.png`) });
  };

  for (const [doc, has] of [["frame-anatomy", "svg"], ["frame-explainer", ".steps button"]] as const)
    for (const w of [400, 640])
      for (const theme of ["graphite", "paper"])
        it(`${doc} at ${w} in ${theme} sits on the panel with no ground and follows its height`, async () => {
          const page = await open(doc, w, theme);
          try {
            const inner = await pageFrame(page, has);
            await page.waitForFunction(() => (document.querySelector("[data-slate-frame-page]") as HTMLElement).getBoundingClientRect().height > 170, undefined, { timeout: 10_000 });
            await page.waitForTimeout(300);
            const ground = await inner.evaluate(() => [getComputedStyle(document.documentElement).backgroundColor, getComputedStyle(document.body).backgroundColor]);
            const box = await page.evaluate(() => {
              const panel = document.querySelector("[data-panel]")!.getBoundingClientRect();
              const frame = document.querySelector("[data-slate-frame-page]")!.getBoundingClientRect();
              return { inside: frame.left >= panel.left - 0.5 && frame.right <= panel.right + 0.5, height: Math.round(frame.height) };
            });
            await shoot(page, `${doc.slice(6)}-${w}-${theme}`);
            expect(ground).toEqual(["rgba(0, 0, 0, 0)", "rgba(0, 0, 0, 0)"]);
            expect(box.inside).toBe(true);
            expect(box.height).toBeLessThanOrEqual(560);
            if (theme === "paper") expect(await inner.evaluate(() => document.documentElement.dataset["scheme"])).toBe("light");
            if (w === 640) {
              await page.getByRole("button", { name: "Expand" }).click();
              await page.waitForSelector("[data-slate-frame-expanded] [data-slate-frame-page]");
              await page.waitForTimeout(600);
              await shoot(page, `${doc.slice(6)}-open-${theme}`, true);
            }
          } finally {
            await page.close();
          }
        }, 40_000);

  it("draws the committed figure with the ground it paints itself", async () => {
    for (const theme of ["graphite", "paper"]) {
      const page = await open("frame-anatomy-committed", 640, theme);
      try {
        await pageFrame(page, "svg");
        await page.waitForTimeout(500);
        await shoot(page, `anatomy-committed-640-${theme}`);
      } finally {
        await page.close();
      }
    }
  }, 40_000);

  it("says why a frame shows nothing, in the frame's place", async () => {
    for (const theme of ["graphite", "paper"]) {
      const page = await open("frame-states", 400, theme);
      try {
        await page.waitForFunction(() => document.querySelectorAll("[data-slate-frame-waiting]:not(:has(svg))").length === 2);
        const said = await page.locator("[data-slate-frame-waiting]").allTextContents();
        const heights = await page.locator("[data-slate-frame-waiting]").evaluateAll(els => els.map(e => Math.round(e.getBoundingClientRect().height)));
        await shoot(page, `states-400-${theme}`);
        expect(said).toEqual(["", "No file at pages/gone.html", "too-big/trace.html is 2.3 MB, over the 1 MB a frame's page may weigh"]);
        expect(heights.every(h => h === heights[0])).toBe(true);
      } finally {
        await page.close();
      }
    }
  }, 40_000);

  it("lets a hostile page reach nothing: every attempt fails and no request leaves the frame", async () => {
    for (const theme of ["graphite", "paper"]) {
      reached.length = 0;
      const page = await open("frame-hostile", 640, theme);
      const left: string[] = [];
      const harnessOrigin = new URL(harness!.base).origin;
      page.on("response", r => {
        if (!r.url().startsWith(harnessOrigin) && !r.url().startsWith("data:") && !r.url().startsWith("blob:")) left.push(`${r.status()} ${r.url()}`);
      });
      page.on("popup", p => left.push(`popup ${p.url()}`));
      try {
        const inner = await pageFrame(page, "#log");
        await inner.waitForSelector("body[data-done]", { timeout: 20_000 });
        const outcomes = await inner.$$eval("#log li", lis => lis.map(li => [(li as HTMLElement).dataset["attempt"], (li as HTMLElement).dataset["outcome"], li.textContent]));
        await page.waitForTimeout(400);
        await shoot(page, `hostile-640-${theme}`);
        // The page navigating itself, the one thing a frame does that no policy inside it refuses.
        await inner.evaluate(port => void (location.href = `http://127.0.0.1:${port}/self`), (target!.address() as { port: number }).port).catch(() => {});
        await page.waitForTimeout(1_500);
        const attempts = outcomes.map(o => o[0]);
        for (const want of ["fetch to", "WebSocket", "window.top", "the app's document", "localStorage", "cookies", "IndexedDB", "form post", "outside image", "remote script", "remote stylesheet", "remote font", "sendBeacon", "EventSource", "popup", "top navigation", "WebRTC to a STUN server", "a nested page, written", "a nested page, set as srcdoc", "a javascript: frame", "XSLT", "the static nested pages", "Node", "a fresh realm's attachShadow", "a nested page in a declarative shadow root", "a nested page written while parsing", "a slate action"]) expect(attempts.some(a => a?.startsWith(want)), want).toBe(true);
        expect(outcomes.filter(o => o[1] !== "blocked")).toEqual([]);
        expect(reached).toEqual([]);
        expect(left).toEqual([]);
        expect(await page.evaluate(() => (window as unknown as { hostEvents: unknown[] }).hostEvents)).toEqual([]);
        expect(await page.evaluate(() => Math.round(document.querySelector("[data-slate-frame-page]")!.getBoundingClientRect().height))).toBeLessThanOrEqual(560);
        expect(page.url()).toContain(harnessOrigin);
        const cookies = await page.context().cookies();
        expect(cookies.filter(c => c.name === "wsp_probe")).toEqual([]);
      } finally {
        await page.close();
      }
    }
  }, 90_000);

  it("draws no page where the window's engine has no Trusted Types, and says why", async () => {
    reached.length = 0;
    const page = await open("frame-hostile", 640, "graphite", () => void delete (window as { trustedTypes?: unknown }).trustedTypes);
    try {
      await page.waitForSelector("[data-slate-frame-waiting]:not(:has(svg))");
      await page.waitForTimeout(1_500);
      await shoot(page, "unsealable-640-graphite");
      expect(await page.locator("[data-slate-frame-waiting]").textContent()).toBe("This window's web engine cannot draw a page sealed; Safari 26 or later on macOS 14 or later can.");
      expect(await page.locator("[data-slate-frame-page]").count()).toBe(0);
      expect(page.frames()).toHaveLength(1);
      expect(reached).toEqual([]);
    } finally {
      await page.close();
    }
  }, 40_000);

  it("stops a sealed page whose own realm has no Trusted Types before any of its script runs", async () => {
    reached.length = 0;
    // The panel keeps them and seals the page; only the frames under it lose them, so the seal alone must hold.
    const page = await open("frame-hostile", 640, "graphite", () => {
      if (window !== window.top) delete (window as { trustedTypes?: unknown }).trustedTypes;
    });
    try {
      await page.waitForSelector("[data-slate-frame-page]");
      await page.waitForTimeout(3_000);
      const inner = page.frames().filter(f => f.parentFrame() !== null && f.parentFrame() !== page.mainFrame());
      expect(inner).toHaveLength(1);
      const held = await inner[0]!.evaluate(() => ({ trustedTypes: typeof (window as { trustedTypes?: unknown }).trustedTypes, early: typeof (window as { early?: unknown }).early, attempts: document.querySelectorAll("#log li").length, frames: document.querySelectorAll("iframe").length }));
      expect(held).toEqual({ trustedTypes: "undefined", early: "undefined", attempts: 0, frames: 0 });
      expect(page.frames()).toHaveLength(3);
      expect(reached).toEqual([]);
    } finally {
      await page.close();
    }
  }, 40_000);
});
