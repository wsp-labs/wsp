// SPDX-License-Identifier: AGPL-3.0-only
// The close chords as the shell reads a press: mod+w closes a pane's tab and
// mod+shift+w the window, mod being Command on macOS and Control elsewhere.
import { describe, expect, it } from "vitest";
import { closeChordOf } from "../src/close.js";
import type { ShellKeyInput } from "../src/zoom.js";

const press = (key: string, over: Partial<ShellKeyInput> = {}): ShellKeyInput => ({ type: "keyDown", key, code: "KeyW", control: false, meta: false, shift: false, alt: false, ...over });

describe("the close chords", () => {
  it("read mod+w as the tab and mod+shift+w as the window, in either case of the key", () => {
    expect(closeChordOf(press("w", { meta: true }), "darwin")).toBe("tab");
    expect(closeChordOf(press("W", { meta: true, shift: true }), "darwin")).toBe("window");
    expect(closeChordOf(press("w", { control: true }), "linux")).toBe("tab");
    expect(closeChordOf(press("W", { control: true, shift: true }), "win32")).toBe("window");
  });

  it("are not the other platform's mod, both mods, an Option chord, a bare key, another key or a key up", () => {
    expect(closeChordOf(press("w", { control: true }), "darwin")).toBeUndefined();
    expect(closeChordOf(press("w", { meta: true }), "linux")).toBeUndefined();
    expect(closeChordOf(press("w", { meta: true, control: true }), "darwin")).toBeUndefined();
    expect(closeChordOf(press("w", { meta: true, alt: true }), "darwin")).toBeUndefined();
    expect(closeChordOf(press("w"), "darwin")).toBeUndefined();
    expect(closeChordOf(press("q", { meta: true }), "darwin")).toBeUndefined();
    expect(closeChordOf({ ...press("w", { meta: true }), type: "keyUp" }, "darwin")).toBeUndefined();
  });
});
