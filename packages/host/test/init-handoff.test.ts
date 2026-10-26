// SPDX-License-Identifier: AGPL-3.0-only
// What a sign-in on a machine reads off the tool and hands it: the code beside a page, the display a callback road
// runs with, and how often the status is asked.
import { describe, expect, it } from "vitest";
import { CALLBACK_DISPLAY, cadence, codeIn, signInEnv } from "../src/init-handoff.js";
import { signInFor } from "./signin-questions.js";

const DEVICE = "https://github.com/login/device";
const CODE = "8F4A-C21B";
const GH_CODE = (() => {
  const s = signInFor("gh");
  return s.kind === "device" ? s.code : undefined;
})();

describe("the code beside a page", () => {
  it("reads a device code in the shape the row names, never out of the page's own query string, and nothing at all for a row with no shape", () => {
    expect(GH_CODE).toBeInstanceOf(RegExp);
    expect(codeIn(`! First copy your one-time code: ${CODE}\nOpen ${DEVICE}`, GH_CODE)).toBe(CODE);
    expect(codeIn(`visit: https://claude.com/oauth/authorize?state=ABCD-1234&code=true\nPaste code here > `, GH_CODE)).toBeUndefined();
    expect(codeIn(`Opening ${DEVICE} in your browser`, GH_CODE)).toBeUndefined();
    // claude's row shows no code, so a token of that shape in its output is never announced as one.
    expect(codeIn(`! First copy your one-time code: ${CODE}`, undefined)).toBeUndefined();
  });
});

describe("the display a sign-in is handed", () => {
  it("only the callback road hands the tool a display; the others take the pty as it comes", () => {
    expect(signInEnv("callback")).toEqual({ DISPLAY: CALLBACK_DISPLAY });
    expect(signInEnv("code")).toBeUndefined();
    expect(signInEnv("none")).toBeUndefined();
  });
});

describe("how often the status is asked", () => {
  it("every pollMs for the first minute, then every twenty seconds or the caller's slower cadence", () => {
    expect(cadence(0, 5_000)).toBe(5_000);
    expect(cadence(59_999, 5_000)).toBe(5_000);
    expect(cadence(60_000, 5_000)).toBe(20_000);
    expect(cadence(15 * 60_000, 5_000)).toBe(20_000);
    expect(cadence(60_000, 30_000)).toBe(30_000);
  });
});
