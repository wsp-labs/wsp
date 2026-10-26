// SPDX-License-Identifier: AGPL-3.0-only
// Where a slate's frame comes from and the sentences that refuse one. A frame shows an HTML file on the thread's
// computer, read by the host as images are, and never an address: the page is drawn sealed, so nothing it names
// loads, and a page from the web would only draw broken.
import { fmtBytes, fmtBytesOver } from "../format.js";
import { slateProblem } from "./problems.js";
import type { SlateProblem } from "./types.js";

export const SLATE_PAGE_MAX_BYTES = 1024 * 1024;

export type SlateFrameSource = { path: string } | { problem: SlateProblem };

export function slateFrameSource(src: string): SlateFrameSource {
  const said = src.trim();
  if (said === "") return { problem: slateProblem("R900", "a frame names no file; give a path such as figures/dial.html") };
  if (/^[a-z][a-z0-9+.-]*:/i.test(said) && !/^[a-z]:[\\/]/i.test(said)) return { problem: slateProblem("R917", `${said} is an address; a frame shows an .html file on the thread's computer, never a page from the web`) };
  if (said.startsWith("~")) return { problem: slateProblem("R900", `${said} starts at a home folder, which wsp does not guess; write the whole path`) };
  if (!/\.html?$/i.test(said)) return { problem: slateNotAPage(said) };
  return { path: said };
}

export const slateNotAPage = (where: string): SlateProblem => slateProblem("R918", `${where} is not an .html file; a frame shows an HTML page`);

export const slatePageTooBig = (where: string, bytes?: number): SlateProblem =>
  slateProblem("R919", `${where} is ${bytes === undefined ? "more than" : `${fmtBytesOver(bytes)}, over`} the ${fmtBytes(SLATE_PAGE_MAX_BYTES)} a frame's page may weigh`);

export { SLATE_FRAME_POLICY } from "./frame-policy.js";
