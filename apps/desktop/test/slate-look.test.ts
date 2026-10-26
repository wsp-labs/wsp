// SPDX-License-Identifier: AGPL-3.0-only
// wsp slate look in the app: the look page's address, the window drawn until the page says it is ready and cut to
// its size, a page that failed or never settled said in words, the window gone either way, and looks answered on
// the host's socket one at a time by their ids.
import { describe, expect, it } from "vitest";
import type { SlatesLookAnswer } from "@wsp/protocol";
import type { OfferSocket } from "../src/app-procs.js";
import { drawLook, fitted, lookUrl, LOOK_MAX_BYTES, LOOK_MAX_HEIGHT, LOOK_MAX_SIDE, LOOK_WORDS, offerSlateLooks, type LookPicture, type LookWindow } from "../src/slate-look.js";

/** A picture of its size whose PNG weighs `perPixel` bytes a pixel, as a NativeImage's does give or take. */
function picture(width: number, height: number, perPixel = 0): LookPicture {
  return {
    toPNG: () => (perPixel === 0 ? Buffer.from(`png ${width}x${height}`) : Buffer.alloc(Math.ceil(width * height * perPixel))),
    getSize: () => ({ width, height }),
    resize: o => picture(o.width, o.height, perPixel),
  };
}

/** A window whose page says each of `says` in turn, then the last for ever, drawn at `scale`. */
function fakeWindow(says: unknown[], scale = 1, perPixel = 0): LookWindow & { loaded: string[]; sized: [number, number][]; shot: unknown[]; gone: boolean } {
  const w = {
    loaded: [] as string[],
    sized: [] as [number, number][],
    shot: [] as unknown[],
    gone: false,
    loadURL: async (url: string) => void w.loaded.push(url),
    setContentSize: (width: number, height: number) => void w.sized.push([width, height]),
    webContents: {
      executeJavaScript: async (code: string) => (code.startsWith("window.__wspSlateLook") ? (says.length > 1 ? says.shift() : says[0]) : true),
      capturePage: async (rect?: { width: number; height: number }) => (w.shot.push(rect), picture((rect?.width ?? 1) * scale, (rect?.height ?? 1) * scale, perPixel)),
    },
    destroy: () => void (w.gone = true),
  };
  return w;
}

describe("a slate look drawn in the app", () => {
  it("opens the host page's look for the thread, waits for ready, and takes the picture at the page's size", async () => {
    const win = fakeWindow([null, { state: "drawing" }, { state: "ready", width: 400, height: 812, theme: "graphite" }], 2);
    const drawn = await drawLook("t-1", undefined, { url: "http://127.0.0.1:4400", scale: 2, open: () => win });
    expect(win.loaded).toEqual(["http://127.0.0.1:4400/#slate-look=t-1"]);
    expect(win.sized).toEqual([[400, 812]]);
    expect(win.shot).toEqual([{ x: 0, y: 0, width: 400, height: 812 }]);
    expect(drawn).toEqual({ png: Buffer.from("png 800x1624").toString("base64"), width: 400, height: 812, scale: 2, theme: "graphite" });
    expect(win.gone).toBe(true);
  });

  it("names a width when one is asked for, cuts a slate taller than the cap, and scales its picture to what the model API takes in a thread past 20 images", async () => {
    expect(lookUrl("http://127.0.0.1:4400/#t/x", "a b", 640)).toBe("http://127.0.0.1:4400/#slate-look=a%20b&w=640");
    const win = fakeWindow([{ state: "ready", width: 640, height: 20_000, theme: "paper" }]);
    expect(await drawLook("t", 640, { url: "http://127.0.0.1:4400", scale: 1, open: () => win })).toMatchObject({ width: 640, height: LOOK_MAX_HEIGHT, whole: 20_000, scale: 1 });
    // A 1,600 px panel at 2x, its slate 9,000 px tall, its PNG 4 bytes a pixel before any scaling.
    const wide = fakeWindow([{ state: "ready", width: 1_600, height: 9_000, theme: "graphite" }], 2, 4);
    const drawn = await drawLook("t", 1_600, { url: "http://127.0.0.1:4400", scale: 2, open: () => wide });
    const png = Buffer.from(drawn.png, "base64");
    expect(drawn).toMatchObject({ width: 1_600, height: LOOK_MAX_HEIGHT, whole: 9_000 });
    expect(png.length).toBeLessThanOrEqual(LOOK_MAX_BYTES);
    expect(drawn.png.length).toBeLessThan(5 * 1024 * 1024);
    const side = Math.max(drawn.width, drawn.height) * drawn.scale;
    expect(side).toBeLessThanOrEqual(LOOK_MAX_SIDE);
    expect(LOOK_MAX_SIDE).toBe(2_000);
    expect(fitted(picture(1_080, 1_800)).size).toEqual({ width: 1_080, height: 1_800 });
    expect(fitted(picture(1_080, 4_000)).size).toEqual({ width: 540, height: 2_000 });
    const heavy = fitted(picture(1_500, 1_500, 4));
    expect(heavy.png.length).toBeLessThanOrEqual(LOOK_MAX_BYTES);
    expect(heavy.size.width).toBeLessThan(1_500);
  });

  it("says the page's own sentence, or that it never drew, and closes the window either way", async () => {
    const failed = fakeWindow([{ state: "failed", why: "This slate needs a newer wsp app to draw it (schema 3); update the app and look again." }]);
    await expect(drawLook("t", undefined, { url: "http://127.0.0.1:4400", scale: 1, open: () => failed })).rejects.toThrow("needs a newer wsp app");
    expect(failed.gone).toBe(true);
    const never = fakeWindow([{ state: "drawing" }]);
    await expect(drawLook("t", undefined, { url: "http://127.0.0.1:4400", scale: 1, open: () => never, waitMs: 300 })).rejects.toThrow(LOOK_WORDS.late(0));
    expect(never.gone).toBe(true);
  });

  it("asks for the picture again when the compositor refuses one, and says so in words when it keeps refusing", async () => {
    const win = fakeWindow([{ state: "ready", width: 400, height: 300, theme: "graphite" }]);
    let refusals = 1;
    const take = win.webContents.capturePage;
    win.webContents.capturePage = async rect => (refusals-- > 0 ? Promise.reject(new Error("UnknownVizError")) : take(rect));
    expect((await drawLook("t", undefined, { url: "http://127.0.0.1:4400", scale: 1, open: () => win })).width).toBe(400);
    const stuck = fakeWindow([{ state: "ready", width: 400, height: 300, theme: "graphite" }]);
    stuck.webContents.capturePage = async () => Promise.reject(new Error("UnknownVizError"));
    await expect(drawLook("t", undefined, { url: "http://127.0.0.1:4400", scale: 1, open: () => stuck })).rejects.toThrow(LOOK_WORDS.noPicture("UnknownVizError"));
    expect(stuck.gone).toBe(true);
  });
});

describe("the app's offer of looks", () => {
  it("offers on the socket and answers each look by its id, one at a time, the picture or the sentence", async () => {
    let push: (frame: Record<string, unknown>) => void = () => {};
    const asked: [string, Record<string, unknown> | undefined][] = [];
    const socket: OfferSocket = { request: async (op, params) => (asked.push([op, params]), {}), onFrame: fn => ((push = fn), () => {}), closed: new Promise(() => {}) };
    let running = 0;
    let most = 0;
    const draw = async (threadId: string, width?: number): Promise<SlatesLookAnswer> => {
      most = Math.max(most, ++running);
      await new Promise(r => setTimeout(r, 20));
      running--;
      if (threadId === "gone") throw new Error("This thread has no slate yet.");
      return { png: "cG5nIQ==", width: width ?? 540, height: 300, scale: 1, theme: "graphite" };
    };
    offerSlateLooks(socket, draw);
    push({ type: "slate.look.wanted", lookId: "l1", threadId: "t1", width: 400 });
    push({ type: "slate.look.wanted", lookId: "l2", threadId: "gone" });
    push({ type: "session.row" });
    await expect.poll(() => asked.length).toBe(3);
    expect(asked).toEqual([
      ["slateLook.offer", undefined],
      ["slateLook.answer", { lookId: "l1", look: { png: "cG5nIQ==", width: 400, height: 300, scale: 1, theme: "graphite" } }],
      ["slateLook.answer", { lookId: "l2", problem: "This thread has no slate yet." }],
    ]);
    expect(most).toBe(1);
  });
});
