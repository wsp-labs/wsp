// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { OWNER_LABEL, WSP_LABEL, type MachineBackend, type MachineSpec } from "@wsp/engine";
import { PROVIDER_LEFT_WORDS, providerKeptLine, providerKeyRefused, refusalLine } from "@wsp/protocol";
import { createRuntime, type PlaceBackends } from "../src/runtime.js";
import { newPlaceKeyPair } from "../src/places.js";
import { serveRuntime } from "../src/serve.js";
import { memoryStore } from "../src/store.js";
import { answersGoneOnce, stubBackend, createOn, type StubBackend } from "./stub-backend.js";
import { WsClient } from "./ws-client.js";
import { wiring } from "./place-join.js";
import { ctx, placesOf, remove } from "./places-fixture.js";

/** A host wired to Solari that also holds a Boat key, which `forgetKey` takes away: the row goes once it is gone. */
async function holdingBoat(box: StubBackend): Promise<{ forgot: string[]; aliveAtForget: () => number }> {
  const solari = stubBackend();
  const at: Record<string, MachineBackend> = { solari, box };
  const forgot: string[] = [];
  let alive = -1;
  const places: PlaceBackends = {
    get wired() {
      return "solari";
    },
    backend: place => (forgot.includes(place) ? undefined : at[place]),
    list: () => Object.keys(at).filter(id => !forgot.includes(id)),
  };
  ctx.runtime = createRuntime({
    backend: solari,
    store: memoryStore(),
    adapters: {},
    places,
    killConfirm: { graceMs: 20, pollMs: 1 },
    placeLinks: {
      ...wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 }),
      forgetKey: async placeId => {
        alive = box.machines.filter(m => !m.killed).length;
        forgot.push(placeId);
      },
    },
  });
  ctx.srv = await serveRuntime(ctx.runtime, { port: 0, authToken: "host-token", devices: ctx.runtime.devices });
  return { forgot, aliveAtForget: () => alive };
}

/** A remove with what it asks beyond the place, for a forget. */
async function removeWith(params: Record<string, unknown>): Promise<Record<string, unknown>> {
  const c = await WsClient.connect(ctx.srv!.port, { token: "host-token" });
  const answer = await c.request("places.remove", params);
  c.close();
  return answer;
}

async function workspaceIds(): Promise<string[]> {
  const c = await WsClient.connect(ctx.srv!.port, { token: "host-token" });
  const { workspaces } = (await c.request("workspaces.list")) as unknown as { workspaces: { id: string }[] };
  c.close();
  return workspaces.map(w => w.id);
}

/** Boat answering the key as refused, the way its API answers a revoked key. */
const refusesKey = (box: StubBackend): void => {
  box.list = async () => Promise.reject(Object.assign(new Error("unauthorized"), { kind: "auth", status: 401 }));
};

/** A box with this host's owner mark that no record names, as a create in flight leaves at Boat. */
const madeMeanwhile = (box: StubBackend): Promise<unknown> => {
  const owner = box.machines[0]!.spec.labels![OWNER_LABEL]!;
  return box.create({ kind: "sandbox", labels: { [WSP_LABEL]: "1", [OWNER_LABEL]: owner } } as MachineSpec);
};

const someoneElses = (box: StubBackend): Promise<unknown> => box.create({ kind: "sandbox", labels: { [WSP_LABEL]: "1", [OWNER_LABEL]: "another-host" } } as MachineSpec);

describe("removing a provider", () => {
  it("deletes every machine this host made there by its owner mark, reads each gone, then forgets the key", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    await createOn(ctx.runtime!, { golden: "snap_g", name: "b", on: "box" });
    await someoneElses(box);
    expect(box.machines.filter(m => !m.killed)).toHaveLength(3);

    const answer = await remove("box");
    expect(answer).toMatchObject({ removed: true, deleted: 2, swept: [] });
    // Another host's machine on the same account is not this host's to delete.
    expect(box.machines.filter(m => !m.killed).map(m => m.spec.labels?.[OWNER_LABEL])).toEqual(["another-host"]);
    // The key went after the deletes, with only the other host's machine left standing.
    expect(held.forgot).toEqual(["box"]);
    expect(held.aliveAtForget()).toBe(1);
    expect((await placesOf()).map(p => p.id)).not.toContain("box");
  });

  it("names a machine the provider will not delete and keeps the key until it goes", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    await createOn(ctx.runtime!, { golden: "snap_g", name: "b", on: "box" });
    const stubborn = box.machines[1]!;
    stubborn.kill = async () => {};

    const answer = await remove("box");
    expect(answer).toMatchObject({ removed: false, deleted: 1, note: providerKeptLine("Boat", 1, [{ id: stubborn.id, said: "still running after three asks" }]) });
    expect(String(answer["note"])).toContain(stubborn.id);
    expect(held.forgot).toEqual([]);
    expect((await placesOf()).map(p => p.id)).toContain("box");

    // Once the provider takes it, the same remove goes through and the key with it.
    stubborn.kill = async () => {
      stubborn.killed = true;
    };
    expect(await remove("box")).toMatchObject({ removed: true, deleted: 1 });
    expect(held.forgot).toEqual(["box"]);
  });

  it("deletes a machine its own records hold that the provider's listing missed, and counts none already gone", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    await createOn(ctx.runtime!, { golden: "snap_g", name: "b", on: "box" });
    await createOn(ctx.runtime!, { golden: "snap_g", name: "c", on: "box" });
    const [listed, missed, gone] = box.machines;
    gone!.killed = true;
    const list = box.list.bind(box);
    box.list = async labels => (await list(labels)).filter(m => m.id !== missed!.id);

    expect(await remove("box")).toMatchObject({ removed: true, deleted: 2 });
    expect([listed!.killed, missed!.killed]).toEqual([true, true]);
    expect(held.forgot).toEqual(["box"]);
  });

  it("a provider holding no machine of this host's forgets the key and says none went", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    expect(await remove("box")).toMatchObject({ removed: true, deleted: 0 });
    expect(held.forgot).toEqual(["box"]);
  });
  it("does not take one 404 from a copy that never held the machine as gone", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    await createOn(ctx.runtime!, { golden: "snap_g", name: "b", on: "box" });
    answersGoneOnce(box, box.machines[0]!);

    expect(await remove("box")).toMatchObject({ removed: true, deleted: 2 });
    expect(box.machines.filter(m => !m.killed)).toEqual([]);
    expect(held.aliveAtForget()).toBe(0);
  });

  it("deletes a machine a create in flight made after the first listing, before the key goes", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    const list = box.list.bind(box);
    let lists = 0;
    box.list = async labels => {
      if (++lists === 2) await madeMeanwhile(box);
      return list(labels);
    };

    expect(await remove("box")).toMatchObject({ removed: true, deleted: 2 });
    expect(box.machines).toHaveLength(2);
    expect(box.machines.filter(m => !m.killed)).toEqual([]);
    expect(held.forgot).toEqual(["box"]);
    expect(held.aliveAtForget()).toBe(0);
  });

  it("names a machine still showing after the third listing again and keeps the key", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    const list = box.list.bind(box);
    let lists = 0;
    box.list = async labels => {
      if (++lists > 1) await madeMeanwhile(box);
      return list(labels);
    };

    const answer = await remove("box");
    // The first listing and three more each delete what they found; the fifth listing's box is named.
    expect(lists).toBe(5);
    const late = box.machines.at(-1)!;
    expect(answer).toMatchObject({ removed: false, deleted: 4, note: providerKeptLine("Boat", 4, [{ id: late.id, said: PROVIDER_LEFT_WORDS.late }]) });
    expect(late.killed).toBeFalsy();
    expect(held.forgot).toEqual([]);
  });

  it("says a machine whose read failed could not be read, names five and counts the rest", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    for (const name of ["a", "b", "c", "d", "e", "f", "g"]) await createOn(ctx.runtime!, { golden: "snap_g", name, on: "box" });
    const get = box.get.bind(box);
    const failing = new Set(box.machines.map(m => m.id));
    box.get = async id => (failing.has(id) ? Promise.reject(Object.assign(new Error("Too Many Requests"), { kind: "concurrency", status: 429 })) : get(id));

    const answer = await remove("box");
    const note = String(answer["note"]);
    expect(answer).toMatchObject({ removed: false, deleted: 0 });
    expect(note).toContain(`(${PROVIDER_LEFT_WORDS.unread("Too Many Requests")})`);
    expect(note).not.toContain("would not");
    expect(note).toMatch(/^7 machines wsp made at Boat are not gone: /);
    expect(note).toContain(" and 2 more.");
    expect([...failing].filter(id => note.includes(id))).toHaveLength(5);
    expect(held.forgot).toEqual([]);
  });

  it("refuses in two halves when Boat refuses the key, which stays", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    refusesKey(box);

    const refused = providerKeyRefused("Boat", "unauthorized");
    const answer = await remove("box");
    expect(answer).toMatchObject({ ok: false, error: refusalLine(refused.said, refused.fix), kind: "auth" });
    expect(answer["error"]).toBe("Boat refused the key (unauthorized), so wsp cannot see or delete the machines it made there; the key is kept. Save a working key and remove Boat again, or remove it with --forget.");
    expect(held.forgot).toEqual([]);
    expect((await placesOf()).map(p => p.id)).toContain("box");
  });

  it("forgets a provider whose key it refuses: the key and its records go and nothing there is asked", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    const a = await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    refusesKey(box);
    // The listing already refuses, so a forget that asked Boat anything would fail on it or on a read.
    let reads = 0;
    const get = box.get.bind(box);
    box.get = async id => (reads++, get(id));

    expect(await removeWith({ placeId: "box", forget: true })).toMatchObject({ removed: true, swept: [] });
    expect(reads).toBe(0);
    expect(box.machines.filter(m => !m.killed)).toHaveLength(1);
    expect(held.forgot).toEqual(["box"]);
    expect(await workspaceIds()).not.toContain(a.id);
    expect((await placesOf()).map(p => p.id)).not.toContain("box");
  });
});
