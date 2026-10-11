// SPDX-License-Identifier: AGPL-3.0-only
// Add a cloud as a public build folds it: the bundler fills in __WSP_CLOUD__ as
// false there, so the tables are read again here with the constant set as it
// would be, and Boat is the one cloud the sheet offers.
import { afterEach, describe, expect, it, vi } from "vitest";
import type { InitSetup } from "@wsp/protocol";

const setupOf = (keys: Record<string, boolean>): InitSetup => ({ keys, home: "/Users/dev", agents: [], pricing: null, job: null }) as InitSetup;

/** The sheet's list and the marks as a build with the constant at `cloud` reads them. */
async function foldedAs(cloud: boolean | undefined) {
  vi.resetModules();
  vi.stubGlobal("__WSP_CLOUD__", cloud);
  const { cloudsOffered } = await import("../src/settings/providers.js");
  const { PROVIDER_KEY_WORDS } = await import("@wsp/protocol");
  const { PROVIDER_MARKS } = await import("@wsp/catalog");
  return { cloudsOffered, PROVIDER_KEY_WORDS, marks: PROVIDER_MARKS.map(m => m.provider) };
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
});

describe("Add a cloud in a public build", () => {
  it("offers Boat and not Solari, whatever keys the host says it registered", async () => {
    const folded = await foldedAs(false);
    expect(folded.cloudsOffered(setupOf({ box: false }))).toEqual(["box"]);
    expect(folded.cloudsOffered(setupOf({ box: false, solari: false }))).toEqual(["box"]);
    expect(Object.keys(folded.PROVIDER_KEY_WORDS)).toEqual(["box"]);
    expect(folded.PROVIDER_KEY_WORDS["box"]!.name).toBe("Boat");
    expect(folded.marks).toEqual(["box"]);
  });

  it("offers both in any other build, where the host registers both", async () => {
    const folded = await foldedAs(undefined);
    expect(folded.cloudsOffered(setupOf({ box: false, solari: false }))).toEqual(["box", "solari"]);
    expect(folded.cloudsOffered(setupOf({ box: false }))).toEqual(["box"]);
    expect(folded.marks.sort()).toEqual(["box", "solari"]);
  });
});
