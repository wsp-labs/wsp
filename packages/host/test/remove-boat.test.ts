// SPDX-License-Identifier: AGPL-3.0-only
// wsp remove boat: Boat is named by the word a person types for it, the host is
// asked to remove the provider row, and the line says how many machines went;
// a machine Boat would not delete fails the line with the host's own words.
import { describe, expect, it } from "vitest";
import { exitClassOf, providerForgetQuestion, providerForgotLine, providerKeptLine, providerKeyStillLine, providerRemoveQuestion, providerRemovedLine } from "@wsp/protocol";
import { removeCommand } from "../src/places.js";
import { captured } from "./verbs-fixture.js";

type Dial = NonNullable<Parameters<typeof removeCommand>[4]>["dial"];

function boatHost(answer: Record<string, unknown>, rows: Record<string, unknown>[] = []): { dial: Dial; asked: Record<string, unknown>[] } {
  const asked: Record<string, unknown>[] = [];
  const dial: Dial = () =>
    Promise.resolve({
      request: (op: string, params?: Record<string, unknown>) => {
        if (op === "places.list") return Promise.resolve({ places: [{ id: "here", kind: "computer", name: "zingzys-mac", default: true }, { id: "box", kind: "provider", name: "box", default: false }] } as never);
        // Two tasks on Boat and one here: the question counts Boat's alone, as the app's dialog does.
        if (op === "workspaces.list") return Promise.resolve({ workspaces: [{ id: "w1", kind: "box", provider: "box", machineId: "m1" }, { id: "w2", kind: "box", provider: "box", machineId: "m2" }, { id: "w3", kind: "local", machineId: "" }] } as never);
        if (op === "sessions.list") return Promise.resolve({ sessions: rows } as never);
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
  for (const word of ["boat", "Boat", "BOAT", "box", "BOX"]) {
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

  it("asks with the count of Boat's tasks before anything goes", async () => {
    const io = captured();
    const asked: string[] = [];
    Object.assign(io, { isTTY: true, ask: async (q: string) => (asked.push(q), "yes") });
    const host = boatHost({ removed: true, swept: [], deleted: 2 });
    expect(await removeCommand(io, opts, ["boat"], {}, deps(host.dial))).toBe(0);
    expect(asked).toEqual([providerRemoveQuestion("Boat", 2)]);
    expect(asked[0]).toBe("Remove Boat?\nIts 2 tasks are deleted at Boat, and once Boat reads them gone the key is forgotten.");
  });

  it("names the sub-threads whose machines hold work not pushed in the question, since it goes with them", async () => {
    const io = captured();
    const asked: string[] = [];
    Object.assign(io, { isTTY: true, ask: async (q: string) => (asked.push(q), "yes") });
    const sub = (held?: { files: number; commits: number }, state = "napping") => ({ state, on: "boat", machine: "m", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, branch: "wsp/fix-it", ...(held !== undefined ? { held } : {}) });
    const rows = [
      { id: "s1", workspaceId: "w1", harness: "claude", status: "completed", threadId: "e17a90c3-0000", sub: sub({ files: 3, commits: 2 }) },
      { id: "s2", workspaceId: "w2", harness: "claude", status: "completed", threadId: "5b2d8e41-0000", sub: sub() },
    ];
    const host = boatHost({ removed: true, swept: [], deleted: 2 }, rows);
    expect(await removeCommand(io, opts, ["boat"], {}, deps(host.dial))).toBe(0);
    expect(asked).toEqual([`${providerRemoveQuestion("Boat", 2)}\nA sub-thread holds work not pushed, which goes with its machine: e17a90c3 (2 commits and 3 changed files on wsp/fix-it).`]);
    // A forget leaves both sub-threads' records, released, so it counts them among no records that leave.
    const forgot = captured();
    expect(await removeCommand(forgot, opts, ["boat"], { yes: true, forget: true }, deps(boatHost({ removed: true, swept: [] }, rows).dial))).toBe(0);
    expect(forgot.lines).toEqual([providerForgotLine("Boat", 0)]);
  });

  it("refuses --force on Boat as usage, asking the host nothing", async () => {
    const host = boatHost({ removed: true, swept: [], deleted: 0 });
    const failed = await removeCommand(captured(), opts, ["boat"], { yes: true, force: true }, deps(host.dial)).catch((e: unknown) => e);
    expect(exitClassOf(failed)).toBe("usage");
    expect((failed as Error).message).toContain("Boat is not a computer");
    expect(host.asked).toEqual([]);
  });

  it("forgets Boat with --forget: the host is asked to forget, and the line says machines there may bill", async () => {
    const io = captured();
    const asked: string[] = [];
    Object.assign(io, { isTTY: true, ask: async (q: string) => (asked.push(q), "yes") });
    const host = boatHost({ removed: true, swept: [] });
    expect(await removeCommand(io, opts, ["boat"], { forget: true }, deps(host.dial))).toBe(0);
    expect(host.asked).toEqual([{ placeId: "box", forget: true }]);
    expect(asked).toEqual([providerForgetQuestion("Boat", 2)]);
    expect(io.lines).toEqual([providerForgotLine("Boat", 2)]);
    expect(io.lines[0]).toBe("Boat forgotten: the key and 2 tasks' records left this host. Nothing at Boat was touched, so machines wsp made there may bill until Boat's own timer ends them.");
  });
  it("says where the key still is in place of the key forgotten, on a remove and on a forget", async () => {
    const still = providerKeyStillLine([{ file: "/work/acme/.env" }]);
    const removed = captured();
    expect(await removeCommand(removed, opts, ["boat"], { yes: true }, deps(boatHost({ removed: true, swept: [], deleted: 2, note: still }).dial))).toBe(0);
    expect(removed.lines).toEqual(["Boat removed: 2 machines deleted. Its key is still in /work/acme/.env, which a host started in that folder reads first; take it out there."]);
    const forgot = captured();
    expect(await removeCommand(forgot, opts, ["boat"], { yes: true, forget: true }, deps(boatHost({ removed: true, swept: [], note: still }).dial))).toBe(0);
    expect(forgot.lines).toEqual([`Boat forgotten: 2 tasks' records left this host. ${still} Nothing at Boat was touched, so machines wsp made there may bill until Boat's own timer ends them.`]);
  });
});

describe("the provider remove words at 0 and 1 tasks", () => {
  it("reads at 0", () => {
    expect(providerRemoveQuestion("Boat", 0)).toBe("Remove Boat?\nEvery machine wsp made there is deleted, and once Boat reads them gone the key is forgotten.");
    expect(providerForgetQuestion("Boat", 0)).toBe("Forget Boat?\nThe key leaves this host. Nothing at Boat is touched, so machines wsp made there may bill until Boat's own timer ends them.");
    expect(providerForgotLine("Boat", 0)).toBe("Boat forgotten: the key left this host. Nothing at Boat was touched, so machines wsp made there may bill until Boat's own timer ends them.");
    expect(providerForgotLine("Boat", 0, "S.")).toBe("Boat forgotten. S. Nothing at Boat was touched, so machines wsp made there may bill until Boat's own timer ends them.");
  });

  it("reads at 1", () => {
    expect(providerRemoveQuestion("Boat", 1)).toBe("Remove Boat?\nIts task is deleted at Boat, and once Boat reads it gone the key is forgotten.");
    expect(providerForgetQuestion("Boat", 1)).toBe("Forget Boat?\nThe key and its task's record leave this host. Nothing at Boat is touched, so machines wsp made there may bill until Boat's own timer ends them.");
    expect(providerForgotLine("Boat", 1)).toBe("Boat forgotten: the key and 1 task's record left this host. Nothing at Boat was touched, so machines wsp made there may bill until Boat's own timer ends them.");
    expect(providerRemovedLine("Boat", 1)).toBe("Boat removed: 1 machine deleted, the key forgotten.");
  });
});
