// SPDX-License-Identifier: AGPL-3.0-only
// wsp slate look, drawn here: the app offers on its socket to the host on this computer to draw slates, and draws each
// look the host pushes in a window nobody sees, on the host's own page at #slate-look=, through the same renderer and
// session as the window, so the theme, the fonts and the panel's width are the person's. The window is offscreen, so
// it paints with no screen, at the display's scale, and goes as soon as its picture is taken. The picture fits what
// the model API takes in a thread past 20 images: no side over 2,000 px, its base64 under Bedrock's 5 MB.
import type { SlatesLookAnswer } from "@wsp/protocol";
import type { OfferSocket } from "./app-procs.js";

/** The tallest slate a look takes, in CSS px; one past it is cut there, as the panel would scroll it, and the answer
 * names its whole height. */
export const LOOK_MAX_HEIGHT = 2_000;
/** The longest side of the picture, in its own pixels: past it the picture is scaled down. */
export const LOOK_MAX_SIDE = 2_000;
/** The PNG's bytes at most, so its base64 stays under 5 MB. */
export const LOOK_MAX_BYTES = 3_500_000;
/** How long a look waits for the page to say it is drawn. */
export const LOOK_WAIT_MS = 25_000;
const POLL_MS = 100;
/** Wide enough that the page lays out at the width it picks, before the window is cut to it. */
const FIRST_SIZE = { width: 1_600, height: 1_200 };

/** The parts of an offscreen BrowserWindow a look uses; a fake stands in for it under test. */
export interface LookWindow {
  loadURL(url: string): Promise<void>;
  setContentSize(width: number, height: number): void;
  webContents: {
    executeJavaScript(code: string): Promise<unknown>;
    capturePage(rect?: { x: number; y: number; width: number; height: number }): Promise<LookPicture>;
  };
  destroy(): void;
}

/** The parts of a NativeImage a look uses. */
export interface LookPicture {
  toPNG(): Buffer;
  getSize(): { width: number; height: number };
  resize(o: { width: number; height: number; quality: "best" }): LookPicture;
}

export interface LookDeps {
  /** The host page this app shows, on this computer. */
  url: string;
  /** The display's scale, which the window draws at. */
  scale: number;
  open(size: { width: number; height: number }, scale: number): LookWindow;
  waitMs?: number;
}

type PageSays = { state: "drawing" } | { state: "ready"; width: number; height: number; theme: string } | { state: "failed"; why: string };

const READ = "window.__wspSlateLook ?? null";
const sleep = (ms: number) => new Promise<void>(done => setTimeout(done, ms));

export const LOOK_WORDS = {
  late: (seconds: number) => `The app's page did not draw the slate within ${seconds} s; look again in a moment.`,
  noPicture: (why: string) => `The app drew the slate but could not take its picture (${why}); look again in a moment.`,
  tooBig: (bytes: number) => `The slate's picture stayed at ${Math.round(bytes / 1000)} kB after scaling down, over the ${LOOK_MAX_BYTES / 1_000_000} MB a look may weigh.`,
} as const;

/** The look page's address on the host page this app shows. */
export function lookUrl(page: string, threadId: string, width?: number): string {
  const at = new URL(page);
  at.hash = `slate-look=${encodeURIComponent(threadId)}${width !== undefined ? `&w=${width}` : ""}`;
  return at.toString();
}

export async function drawLook(threadId: string, width: number | undefined, deps: LookDeps): Promise<SlatesLookAnswer> {
  const win = deps.open(FIRST_SIZE, deps.scale);
  try {
    await win.loadURL(lookUrl(deps.url, threadId, width));
    const waitMs = deps.waitMs ?? LOOK_WAIT_MS;
    const began = Date.now();
    let said: PageSays | null = null;
    while (Date.now() - began < waitMs) {
      said = (await win.webContents.executeJavaScript(READ)) as PageSays | null;
      if (said?.state === "ready" || said?.state === "failed") break;
      await sleep(POLL_MS);
    }
    if (said?.state === "failed") throw new Error(said.why);
    if (said?.state !== "ready") throw new Error(LOOK_WORDS.late(Math.round(waitMs / 1000)));
    const whole = Math.max(1, said.height);
    const height = Math.min(whole, LOOK_MAX_HEIGHT);
    win.setContentSize(said.width, height);
    // One frame at the new size before the picture.
    await win.webContents.executeJavaScript("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))");
    const { png, size } = fitted(await capture(win, { x: 0, y: 0, width: said.width, height }));
    const scale = Math.round((size.width / said.width) * 1000) / 1000;
    return { png: png.toString("base64"), width: said.width, height, scale, theme: said.theme, ...(whole > height ? { whole } : {}) };
  } finally {
    win.destroy();
  }
}

/** The picture scaled down until no side passes LOOK_MAX_SIDE and its PNG fits LOOK_MAX_BYTES; PNG bytes go about
 * with the area, so each try past the bytes scales by the square root of how far over it was. */
export function fitted(shot: LookPicture): { png: Buffer; size: { width: number; height: number } } {
  const { width, height } = shot.getSize();
  let factor = Math.min(1, LOOK_MAX_SIDE / Math.max(width, height));
  for (let tried = 0; tried < 6; tried++) {
    const picture = factor < 1 ? shot.resize({ width: Math.max(1, Math.floor(width * factor)), height: Math.max(1, Math.floor(height * factor)), quality: "best" }) : shot;
    const png = picture.toPNG();
    if (png.length <= LOOK_MAX_BYTES) return { png, size: picture.getSize() };
    factor *= Math.sqrt(LOOK_MAX_BYTES / png.length) * 0.95;
  }
  throw new Error(LOOK_WORDS.tooBig(shot.toPNG().length));
}

/** How many times a picture is asked for: Chromium's compositor can refuse one under load (UnknownVizError, measured
 * on a busy Linux box) and give the next. */
const CAPTURE_TRIES = 3;

async function capture(win: LookWindow, rect: { x: number; y: number; width: number; height: number }): ReturnType<LookWindow["webContents"]["capturePage"]> {
  for (let tried = 1; ; tried++) {
    try {
      return await win.webContents.capturePage(rect);
    } catch (e) {
      if (tried >= CAPTURE_TRIES) throw new Error(LOOK_WORDS.noPicture(e instanceof Error ? e.message : String(e)));
      await sleep(150);
    }
  }
}

/** Offers this app's looks on one socket to the host: each slate.look.wanted is drawn, one at a time so at most one
 * hidden window stands, and answered by its id with the picture or the sentence that says why there is none. A host
 * that predates the offer refuses it, which leaves nothing running. Ends with the socket. */
export function offerSlateLooks(socket: OfferSocket, draw: (threadId: string, width?: number) => Promise<SlatesLookAnswer>, log?: (line: string) => void): void {
  const say = (e: unknown): void => log?.(`slate look: ${e instanceof Error ? e.message : String(e)}`);
  let queue: Promise<unknown> = Promise.resolve();
  const off = socket.onFrame(frame => {
    if (frame["type"] !== "slate.look.wanted" || typeof frame["lookId"] !== "string" || typeof frame["threadId"] !== "string") return;
    const lookId = frame["lookId"];
    const threadId = frame["threadId"];
    const width = typeof frame["width"] === "number" ? frame["width"] : undefined;
    queue = queue.then(() => draw(threadId, width)).then(
      look => socket.request("slateLook.answer", { lookId, look }),
      (e: unknown) => socket.request("slateLook.answer", { lookId, problem: e instanceof Error ? e.message : String(e) }),
    ).catch(say);
  });
  socket.request("slateLook.offer").catch(say);
  void socket.closed.then(off);
}
