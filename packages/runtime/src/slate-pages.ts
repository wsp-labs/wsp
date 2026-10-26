// SPDX-License-Identifier: AGPL-3.0-only
// A slate's frame piece, read for the window as an image is (slate-images.ts): an .html file from any folder on the
// thread's computer, this one's disk or another computer's through its daemon's fs.page, answered as text only where
// it is a regular file of UTF-8 under the cap. It is drawn sealed, so its text is all that leaves; never an address.
import { constants } from "node:fs";
import { open, stat } from "node:fs/promises";
import { isAbsolute, posix, resolve } from "node:path";
import { placeUpdateLine, unknownOpLine, type FsImageReply, type SlatesPageAnswer } from "@wsp/protocol";
import { slateFrameSource, slateImageMissing, slateImageNotAFile, slateNotAPage, slatePageTooBig, slateProblem, SLATE_PAGE_MAX_BYTES } from "@wsp/protocol/slate";
import { codeOf, inTurn, versionOf } from "./slate-images.js";

/** The thread's computer where it is not this one: its name, and a whole path read there through its daemon. */
export interface PageOn {
  name: string;
  read(path: string): Promise<FsImageReply>;
}

export interface SlatePageRoads {
  thread: string;
  have: string | undefined;
  folder: string | undefined;
  on: PageOn | undefined;
}

const text = new TextDecoder("utf-8", { fatal: true });

export async function slatePage(src: string, roads: SlatePageRoads): Promise<SlatesPageAnswer> {
  const read = slateFrameSource(src);
  if ("problem" in read) return read;
  const there = roads.on;
  const whole = there === undefined ? isAbsolute(read.path) : read.path.startsWith("/");
  const folder = roads.folder;
  if (!whole && folder === undefined) return { problem: slateProblem("R903", `${read.path} is not a whole path, and this host knows no folder for the thread to read it under`) };
  const under = (join: (a: string, b: string) => string): string => (whole || folder === undefined ? read.path : join(folder, read.path));
  return inTurn(roads.thread, () => (there !== undefined ? onDaemon(read.path, under(posix.join), there, roads.have) : readHere(read.path, under(resolve), roads.have)));
}

/** Bytes as the page's text, or nothing where they are not UTF-8. */
function asText(bytes: Uint8Array): string | undefined {
  try {
    return text.decode(bytes);
  } catch {
    return undefined;
  }
}

async function readHere(shown: string, path: string, have: string | undefined): Promise<SlatesPageAnswer> {
  let handle;
  try {
    handle = await open(path, constants.O_RDONLY | constants.O_NONBLOCK);
  } catch (e) {
    const code = codeOf(e);
    if (code === "ENOENT" || code === "ENOTDIR") return { problem: slateImageMissing(shown) };
    const found = await stat(path).catch(() => undefined);
    if (found !== undefined && !found.isFile()) return { problem: slateImageNotAFile(shown) };
    return { problem: slateProblem("R903", `${shown} could not be opened: ${code ?? String(e)}`) };
  }
  try {
    const st = await handle.stat({ bigint: true });
    if (!st.isFile()) return { problem: slateImageNotAFile(shown) };
    const size = Number(st.size);
    if (size > SLATE_PAGE_MAX_BYTES) return { problem: slatePageTooBig(shown, size) };
    const version = versionOf(Number(st.mtimeMs), size, st.ino, st.ctimeNs);
    if (version === have) return { unchanged: true, version };
    // One byte past the cap tells a file that grew since the stat.
    const bytes = Buffer.alloc(SLATE_PAGE_MAX_BYTES + 1);
    let got = 0;
    for (;;) {
      const { bytesRead } = await handle.read(bytes, got, bytes.length - got, null);
      if (bytesRead === 0) break;
      got += bytesRead;
      if (got === bytes.length) return { problem: slatePageTooBig(shown) };
    }
    const html = asText(bytes.subarray(0, got));
    return html === undefined ? { problem: slateNotAPage(shown) } : { html, version };
  } finally {
    await handle.close();
  }
}

async function onDaemon(shown: string, path: string, there: PageOn, have: string | undefined): Promise<SlatesPageAnswer> {
  let said: FsImageReply;
  try {
    said = await there.read(path);
  } catch (e) {
    const code = codeOf(e);
    const message = e instanceof Error ? e.message : String(e);
    if (code === "not-found") return { problem: slateImageMissing(shown) };
    if (code === "not-a-file") return { problem: slateImageNotAFile(shown) };
    if (message === unknownOpLine("fs.page")) return { problem: slateProblem("R903", `${there.name} runs a daemon too old to read pages; ${placeUpdateLine(there.name)} updates it`) };
    return { problem: slateProblem("R903", `${there.name} could not read ${shown}: ${message}`) };
  }
  if (said.size > SLATE_PAGE_MAX_BYTES) return { problem: slatePageTooBig(shown, said.size) };
  if (said.mediaType !== "text/html" || said.content === undefined) return { problem: slateNotAPage(shown) };
  const version = said.modified === undefined ? undefined : versionOf(said.modified, said.size, said.inode, said.changed);
  if (have !== undefined && version === have) return { unchanged: true, version };
  return { html: Buffer.from(said.content, "base64").toString("utf8"), ...(version !== undefined ? { version } : {}) };
}
