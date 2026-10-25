// SPDX-License-Identifier: AGPL-3.0-only
// The cloud setup on the wire: the one view the app's key step and wsp setup read, and the words it prints.
import { describe, expect, it } from "vitest";
import { InitSetup, LOGIN_CHOICES, NEEDS_YOU, SIGN_IN_ANSWERS, credentialOnBuilderLine, initSetupLines, initStoppedLine, titleWithNeed } from "../src/index.js";

describe("the cloud setup view", () => {
  it("carries the provider key's presence alone, never a key, and the price", () => {
    const setup = InitSetup.parse({ keys: { solari: false }, home: "/Users/me", pricing: { size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.11 } });
    expect(Object.keys(setup.keys)).toEqual(["solari"]);
    expect(InitSetup.safeParse({ keys: { solari: "slr_live_x" }, home: "/Users/me", pricing: null }).success).toBe(false);
  });

  it("wsp setup prints the keys as held or not and the price, and no key", () => {
    const setup = { keys: { box: false, solari: true }, home: "/Users/me", pricing: { size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.11 } };
    expect(initSetupLines(setup).map(line => line.replace(/\u00a0/g, " "))).toEqual(["Boat API key: not set", "Solari API key: saved", "A 2 vCPU, 4 GB workspace costs about $0.11 an hour while it runs and naps when idle"]);
    expect(JSON.stringify(initSetupLines(setup))).not.toMatch(/slr_live|sk-ant/);
  });

  it("a stopped build's sentence is this computer's own word for what happened, and a reason said twice is said once", () => {
    expect(initStoppedLine("fetch failed; fetch failed")).toBe("This computer lost its network");
    expect(initStoppedLine("Sandbox limit reached")).toBe("Sandbox limit reached");
    expect(initStoppedLine("the snapshot failed; the snapshot failed")).toBe("the snapshot failed");
  });
});

describe("the need mark", () => {
  it("a title carries the mark once however often it is set", () => {
    expect(NEEDS_YOU).toBe("wsp needs you");
    expect(titleWithNeed("wsp", true)).toBe("\u2022 wsp");
    expect(titleWithNeed("wsp", false)).toBe("wsp");
    expect(titleWithNeed(titleWithNeed("wsp", true), true)).toBe("\u2022 wsp");
    expect(titleWithNeed(titleWithNeed("wsp", true), false)).toBe("wsp");
  });
});

describe("the sign-in words", () => {
  it("the sign-in answers offer six words, each with the words its row and its counts print", () => {
    expect(LOGIN_CHOICES).toEqual(["copy", "machine", "later", "key", "skip", "token"]);
    for (const value of LOGIN_CHOICES) expect(SIGN_IN_ANSWERS[value], value).toBeDefined();
  });

  it("a builder holding a sign-in file is refused in one sentence that names the paths", () => {
    expect(credentialOnBuilderLine(["/root/.claude-cfg/.credentials.json", "/root/.codex/auth.json"])).toBe(
      "the builder holds a sign-in file at /root/.claude-cfg/.credentials.json, /root/.codex/auth.json; sign-ins never sit in an image, so nothing was sealed",
    );
  });
});
