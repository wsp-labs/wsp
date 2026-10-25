// SPDX-License-Identifier: AGPL-3.0-only
// The look page's two readings: which thread and width a hash asks for, and whether a drawn slate has settled, that is
// nothing it loads is still on its way.
import { describe, expect, it } from "vitest";
import { slateLookOf, slateSettled } from "./SlateLook.js";

const holding = (html: string): HTMLElement => {
  const root = document.createElement("div");
  root.innerHTML = html;
  return root;
};

describe("the slate look page", () => {
  it("reads the thread and the width off its hash, and nothing off any other", () => {
    expect(slateLookOf("#slate-look=t-1&w=400")).toEqual({ threadId: "t-1", width: 400 });
    expect(slateLookOf("#slate-look=a%20b")).toEqual({ threadId: "a b" });
    expect(slateLookOf("#slate-look=")).toBeUndefined();
    expect(slateLookOf("#gallery")).toBeUndefined();
  });

  it("is settled once no image or frame is waiting and every frame has said its height", () => {
    expect(slateSettled(holding("<p>nothing drawn</p>"))).toBe(false);
    expect(slateSettled(holding(`<div data-slate-surface><span data-slate-image-loading></span></div>`))).toBe(false);
    expect(slateSettled(holding(`<div data-slate-surface><div data-slate-frame-waiting><svg></svg></div></div>`))).toBe(false);
    expect(slateSettled(holding(`<div data-slate-surface><iframe data-slate-frame-page></iframe></div>`))).toBe(false);
    expect(slateSettled(holding(`<div data-slate-surface><iframe data-slate-frame-page data-slate-frame-heard></iframe><div data-slate-frame-waiting>No file at a.html</div></div>`))).toBe(true);
  });
});
