// SPDX-License-Identifier: AGPL-3.0-only
// wsp remove boat: Boat is named by the word a person types for it, the host is
// asked to remove the provider row, and the line says how many machines went;
// a machine Boat would not delete fails the line with the host's own words.
import { describe, expect, it } from "vitest";
import { exitClassOf, providerKeptLine, providerRemovedLine } from "@wsp/protocol";
import { removeCommand } from "../src/places.js";
import { captured } from "./verbs-fixture.js";

type Dial = NonNullable<Parameters<typeof removeCommand>[4]>["dial"];

function boatHost(answer: Record<string, unknown>): { dial: Dial; asked: Record<string, unknown>[] } {
  const asked: Record<string, unknown>[] = [];
  const dial: Dial = () =>
    Promise.resolve({
      request: (op: string, params?: Record<string, unknown>) => {
        if (op === "places.list") return Promise.resolve({ places: [{ id: "here", kind: "computer", name: "zingzys-mac", default: true }, { id: "box", kind: "provider", name: "box", default: false }] } as never);
        if (op === "places.remove") {
          asked.push(params ?? {});
          return Promise.resolve(answer as never);
        }
        return Promise.reject(new Error(`unexpected op ${op}`));
      },
      events: () => Promise.resolve(),
      onFrame: () => () => {},
      closed: Promise.resolve(),
      closeWords: () => "",
      close: () => {},
      drop: () => {},
    } as never);
  return { dial, asked };
}

/** A provider's remove only dials the host; nothing here runs, signs in or reads a file. */
const deps = (dial: Dial): Parameters<typeof removeCommand>[4] => ({ dial }) as unknown as Parameters<typeof removeCommand>[4];
const opts = { statePath: "/nowhere/state.json", home: "/nowhere", env: {} };

describe("wsp remove boat", () => {
  for (const word of ["boat", "Boat", "box"]) {
    it(`takes "${word}" for Boat and says how many machines went`, async () => {
      const io = captured();
      const host = boatHost({ removed: true, swept: [], deleted: 2 });
      expect(await removeCommand(io, opts, [word], { yes: true }, deps(host.dial))).toBe(0);
      expect(host.asked).toEqual([{ placeId: "box" }]);
      expect(io.lines).toEqual([providerRemovedLine("Boat", 2)]);
      expect(io.lines[0]).toBe("Boat removed: 2 machines deleted, the key forgotten.");
    });
  }

  it("fails as the provider's when Boat would not delete a machine, naming it", async () => {
    const note = providerKeptLine("Boat", 1, [{ id: "bx_rb3j87ys", said: "still running after three asks" }]);
    const host = boatHost({ removed: false, swept: [], deleted: 1, note });
    const failed = await removeCommand(captured(), opts, ["boat"], { yes: true }, deps(host.dial)).catch((e: unknown) => e);
    expect(failed).toBeInstanceOf(Error);
    expect((failed as Error).message).toBe(note);
    expect((failed as Error).message).toContain("bx_rb3j87ys");
    expect(exitClassOf(failed)).toBe("provider");
  });
});
