// SPDX-License-Identifier: AGPL-3.0-only
// A slate's frame piece, read for the window: an .html file from any folder on the thread's computer, a regular file
// at most the cap, handed over as text for the window to draw sealed. A thread on another computer is refused for
// now; the daemon's own read comes with the build.
import { constants } from "node:fs";
import { open, stat } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import { usageRefusal, type SlatesImageAnswer, type SlatesLookAnswer, type SlatesPageAnswer, type SlateView } from "@wsp/protocol";
import { slateFrameSource, slateImageMissing, slatePageTooBig, slateProblem, SLATE_PAGE_MAX_BYTES } from "@wsp/protocol/slate";

export interface SlatePageRoads {
  have: string | undefined;
  folder: string | undefined;
  /** The thread's computer's name where it is not this one. */
  elsewhere: string | undefined;
}

export async function slatePage(src: string, roads: SlatePageRoads): Promise<SlatesPageAnswer> {
  const read = slateFrameSource(src);
  if ("problem" in read) return read;
  if (roads.elsewhere !== undefined) return { problem: slateProblem("R903", `${read.path} is on ${roads.elsewhere}, and frames read files on this computer alone for now`) };
  if (!isAbsolute(read.path) && roads.folder === undefined) return { problem: slateProblem("R903", `${read.path} is not a whole path, and this host knows no folder for the thread to read it under`) };
  const path = isAbsolute(read.path) || roads.folder === undefined ? read.path : resolve(roads.folder, read.path);
  let handle;
  try {
    handle = await open(path, constants.O_RDONLY | constants.O_NONBLOCK);
  } catch (e) {
    const code = (e as { code?: string }).code;
    if (code === "ENOENT" || code === "ENOTDIR") return { problem: slateImageMissing(read.path) };
    const found = await stat(path).catch(() => undefined);
    if (found !== undefined && !found.isFile()) return { problem: slateProblem("R914", `${read.path} is not a file; a frame shows a regular file`) };
    return { problem: slateProblem("R903", `${read.path} could not be opened: ${code ?? String(e)}`) };
  }
  try {
    const st = await handle.stat({ bigint: true });
    if (!st.isFile()) return { problem: slateProblem("R914", `${read.path} is not a file; a frame shows a regular file`) };
    const size = Number(st.size);
    if (size > SLATE_PAGE_MAX_BYTES) return { problem: slatePageTooBig(read.path, size) };
    const version = [Math.round(Number(st.mtimeMs)), size, st.ino, st.ctimeNs].join(":");
    if (version === roads.have) return { unchanged: true, version };
    const bytes = Buffer.alloc(SLATE_PAGE_MAX_BYTES + 1);
    let got = 0;
    for (;;) {
      const { bytesRead } = await handle.read(bytes, got, bytes.length - got, null);
      if (bytesRead === 0) break;
      got += bytesRead;
      if (got > SLATE_PAGE_MAX_BYTES) return { problem: slatePageTooBig(read.path) };
    }
    return { html: bytes.subarray(0, got).toString("utf8"), version };
  } finally {
    await handle.close();
  }
}

/** A slate as a look draws it: the record a window holds, how the person's window last drew it, and the host's reads
 * of the files its images and frames name. */
export interface SlateDrawing {
  view: SlateView;
  theme: string;
  width: number;
  read(kind: "image" | "page", src: string): Promise<SlatesImageAnswer | SlatesPageAnswer>;
}
export type DrawSlate = (drawing: SlateDrawing) => Promise<{ png: Buffer; width: number; height: number }>;

export async function lookAt(draw: DrawSlate | undefined, drawing: SlateDrawing, text: string): Promise<SlatesLookAnswer> {
  if (draw === undefined) throw usageRefusal("this host runs without the wsp app, so nothing on this computer can draw the slate.", "Read the sketch with slate_read instead.");
  const drawn = await draw(drawing);
  return { mediaType: "image/png", bytes: drawn.png.toString("base64"), width: drawn.width, height: drawn.height, theme: drawing.theme, text };
}
