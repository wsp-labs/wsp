// SPDX-License-Identifier: AGPL-3.0-only
// A page sealed where its own realm has no Trusted Types. A handler on the page's <html> start tag is dropped by the
// seal, so a pointer over the frame runs nothing and nothing reaches a listener, with Trusted Types and without. Runs with WSP_RENDER=1.
import { createSocket, type Socket } from "node:dgram";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import type { Browser, Page } from "playwright";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { launchRender, renderSkipped, stopRender } from "./render-browser";
import { FRAME_FILES } from "./slate-render/frames";
import { serveHarness, type Harness } from "./slate-render/serve";

const PROBE = (stun: number) =>
  `<!doctype html><html style="display:block;min-height:100vh" onmouseover="if(window.ran)return;window.ran=1;parent.postMessage({wspFrame:'height',height:7777},'*');const pc=new RTCPeerConnection({iceServers:[{urls:'stun:127.0.0.1:${stun}'}]});pc.createDataChannel('x');pc.createOffer().then(o=>pc.setLocalDescription(o))"><head><title>probe</title></head><body><p>hi</p><script>parent.postMessage({wspFrame:'height',height:3210},'*')</script></body></html>`;

describe.skipIf(renderSkipped !== undefined)("a page sealed where its realm has no Trusted Types", () => {
  let harness: Harness | undefined;
  let browser: Browser | undefined;
  let folder: string | undefined;
  let stun: Socket | undefined;
  const reached: string[] = [];

  beforeAll(async () => {
    stun = createSocket("udp4");
    stun.on("message", () => reached.push("udp stun"));
    await new Promise<void>(done => stun!.bind(0, "127.0.0.1", done));
    folder = mkdtempSync(join(tmpdir(), "wsp-sealed-anyway-"));
    mkdirSync(dirname(join(folder, FRAME_FILES.hostile)), { recursive: true });
    writeFileSync(join(folder, FRAME_FILES.hostile), PROBE(stun.address().port));
    browser = await launchRender();
    harness = await serveHarness(folder);
  }, 120_000);
  afterAll(async () => {
    await stopRender(browser, undefined);
    harness?.stop();
    stun?.close();
    if (folder !== undefined) rmSync(folder, { recursive: true, force: true });
  });

  const run = async (withoutTrustedTypes: boolean): Promise<{ ran: unknown; height: number; reached: string[] }> => {
    reached.length = 0;
    const page: Page = await browser!.newPage({ viewport: { width: 1200, height: 1100 }, reducedMotion: "reduce" });
    try {
      if (withoutTrustedTypes)
        await page.addInitScript(() => {
          if (window !== window.top) delete (window as { trustedTypes?: unknown }).trustedTypes;
        });
      await page.goto(`${harness!.base}?w=640&doc=frame-hostile&theme=graphite`);
      await page.waitForSelector("[data-slate-frame-page]");
      await page.waitForTimeout(2_000);
      const before = await page.evaluate(() => Math.round(document.querySelector("[data-slate-frame-page]")!.getBoundingClientRect().height));
      const box = (await page.locator("[data-slate-frame-page]").boundingBox())!;
      await page.mouse.move(box.x + 20, box.y + 20);
      await page.mouse.move(box.x + box.width / 2, box.y + Math.min(box.height, 120) / 2, { steps: 5 });
      await page.waitForTimeout(3_000);
      const inner = page.frames().filter(f => f.parentFrame() !== null && f.parentFrame() !== page.mainFrame());
      const ran = await inner[0]!.evaluate(() => (window as { ran?: unknown }).ran);
      const height = await page.evaluate(() => Math.round(document.querySelector("[data-slate-frame-page]")!.getBoundingClientRect().height));
      console.info(`withoutTrustedTypes=${withoutTrustedTypes}: height before the pointer ${before}, after ${height}, ran ${String(ran)}, reached ${JSON.stringify(reached)}`);
      return { ran, height, reached: [...reached] };
    } finally {
      await page.close();
    }
  };

  it("runs none of the page's own script when a pointer crosses the frame", async () => {
    const without = await run(true);
    expect(without.ran).toBeUndefined();
    expect(without.reached).toEqual([]);
  }, 60_000);

  it("with Trusted Types too, a handler on the page's html tag is gone and nothing reaches the listener", async () => {
    const withTT = await run(false);
    expect(withTT.ran).toBeUndefined();
    expect(withTT.reached).toEqual([]);
  }, 60_000);
});
