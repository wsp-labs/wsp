// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { compileSlateText, parseSlate, sketchSlate, slateCatalog, slateFrameSource } from "../../src/slate/index.js";

const problems = (text: string) => { const r = parseSlate(text); return [...r.errors, ...r.warnings].map(p => `${p.code} ${p.message}`); };

describe("a slate frame's src", () => {
  it("reads an .html or .htm path as a file on the thread's computer, whole or under its folder", () => {
    for (const path of ["/tmp/dial.html", "figures/dial.html", "../pages/explain.HTM"]) expect(slateFrameSource(path)).toEqual({ path });
  });

  it.each([
    ["", "R900"],
    ["~/pages/dial.html", "R900"],
    ["https://acme.test/page.html", "R917"],
    ["file:///tmp/dial.html", "R917"],
    ["figures/dial.svg", "R918"],
    ["notes.md", "R918"],
  ])("refuses %j with %s", (src, code) => {
    const read = slateFrameSource(src);
    expect("problem" in read && read.problem.code).toBe(code);
  });

  it("refuses a page from the web at the write, and takes a path", () => {
    expect(problems(`<slate><column><frame src="https://acme.test/page.html" /><text>x</text></column></slate>`)).toEqual([expect.stringMatching(/^R917 https:\/\/acme.test\/page.html is an address/)]);
    expect(problems(`<slate><column><frame src="figures/dial.html" caption="Dial" /><text>Three blocks</text></column></slate>`)).toEqual([]);
  });

  it("warns on a slate that is only frames, and names the file in the sketch", () => {
    const only = `<slate><column><frame src="figures/dial.html" caption="Dial" /></column></slate>`;
    expect(problems(only)).toEqual([expect.stringMatching(/^W022 this slate is only frames/)]);
    expect(sketchSlate(compileSlateText(only).document!, {})).toContain("[frame figures/dial.html]  Dial");
  });

  it("stays out of the catalog's index until it is ruled on", () => {
    expect(slateCatalog()).not.toContain("frame");
  });
});
