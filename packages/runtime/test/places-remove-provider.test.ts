// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { OWNER_LABEL, WSP_LABEL, type MachineBackend, type MachineSpec } from "@wsp/engine";
import { providerKeptLine } from "@wsp/protocol";
import { createRuntime, type PlaceBackends } from "../src/runtime.js";
import { newPlaceKeyPair } from "../src/places.js";
import { serveRuntime } from "../src/serve.js";
import { memoryStore } from "../src/store.js";
import { stubBackend, createOn, type StubBackend } from "./stub-backend.js";
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
});
