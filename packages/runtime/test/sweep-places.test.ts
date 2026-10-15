// SPDX-License-Identifier: AGPL-3.0-only
// The sweep reads every provider this host holds a key for, not only the one it
// forks on: a public build forks on none and holds Boat as a place, so a sweep
// of the wired provider alone never saw a Boat box. A box there with this host's
// owner label and a workspace id this host keeps no record or name for is named
// at each sweep with the time it goes, and deleted once that time is past.
import { describe, expect, it } from "vitest";
import { NoProviderBackend, type MachineBackend } from "@wsp/engine";
import { UNRECORDED_DELETE_MS, createRuntime, type PlaceBackends } from "../src/runtime.js";
import { fakeClock } from "./fake-clock.js";
import { memoryStore } from "../src/store.js";
import { createOn, stubBackend } from "./stub-backend.js";
import { until } from "./until.js";
import { newPlaceKeyPair } from "../src/places.js";
import { wiring } from "./place-join.js";

const ago = (ms: number): string => new Date(Date.now() - ms).toISOString();

/** The table a host with no provider of its own hands down once a Boat key is saved: the wired row answers for
 * itself, and only Boat is a place a person can name. */
const boatBeside = (box: MachineBackend): { wired: MachineBackend; places: PlaceBackends } => {
  const wired = new NoProviderBackend();
  return { wired, places: { wired: "none", backend: place => (place === "none" ? wired : place === "box" ? box : undefined), list: () => ["box"] } };
};

async function unrecordedBox(state: "paused" | "running" = "paused") {
  const box = stubBackend();
  const { wired, places } = boatBeside(box);
  const store = memoryStore();
  const time = fakeClock();
  const rt = createRuntime({ backend: wired, places, store, adapters: {}, clock: time.clock, placeLinks: wiring(newPlaceKeyPair()) });
  await rt.workspaces.list();
  const { id: owner } = (await store.get("owner", "id")) as { id: string };
  const machine = (await box.create({ kind: "sandbox", labels: { wsp: "1", "wsp-host": "1", "wsp-owner": owner, "wsp-workspace": "ws_d913623a", createdAt: ago(3 * 24 * 3_600_000) } })) as unknown as { id: string; paused: boolean; killed: boolean };
  machine.paused = state === "paused";
  return { box, store, rt, time, machine };
}

describe("a box at a provider this host is not wired to", () => {
  it("is named at every sweep when it wears this host's label and no record or name of this host claims it, with the time it is deleted", async () => {
    const { rt, time, machine } = await unrecordedBox();
    const goesAt = new Date(time.clock.now() + UNRECORDED_DELETE_MS).toISOString();

    for (const _ of [1, 2]) {
      const result = await rt.reap();
      expect(result.failed).toEqual([{ id: machine.id, message: `not recorded: no workspace of this host is ws_d913623a, so nothing here reaches this paused machine of yours; deleted at ${goesAt}, a day after this host first saw it` }]);
      expect(result.reaped).toEqual([]);
      time.advance(10 * 60_000);
    }
    expect(machine.killed).toBe(false);
    await rt.close();
  });

  it("is deleted at the first sweep past its delay, and named once more as it goes", async () => {
    const { store, rt, time, machine } = await unrecordedBox();
    await rt.reap();
    time.advance(UNRECORDED_DELETE_MS - 1);
    expect((await rt.reap()).reaped).toEqual([]);
    expect(machine.killed).toBe(false);

    time.advance(1);
    const result = await rt.reap();

    expect(machine.killed).toBe(true);
    expect(result.failed).toBeUndefined();
    expect(result.reaped).toMatchObject([{ id: machine.id, reason: "unrecorded", labels: { "wsp-workspace": "ws_d913623a" } }]);
    expect(await store.keys("unrecorded-machines")).toEqual([]);
    await rt.close();
  });

  it("a running one is named and deleted the same way, since nothing else ends a machine of this host's whose project it cannot name", async () => {
    const { rt, time, machine } = await unrecordedBox("running");
    expect((await rt.reap()).failed).toMatchObject([{ id: machine.id, message: expect.stringContaining("this running machine of yours; deleted at") }]);
    time.advance(UNRECORDED_DELETE_MS);
    expect((await rt.reap()).reaped).toMatchObject([{ id: machine.id, reason: "unrecorded" }]);
    expect(machine.killed).toBe(true);
    await rt.close();
  });

  it("a machine the listing stops carrying is forgotten, so the first sighting of a box that comes back starts a new day", async () => {
    const { box, store, rt, time, machine } = await unrecordedBox();
    await rt.reap();
    const realList = box.list.bind(box);
    box.list = async labels => (await realList(labels)).filter(row => row.id !== machine.id);
    time.advance(60_000);
    await rt.reap();
    expect(await store.keys("unrecorded-machines")).toEqual([]);
    await rt.close();
  });

  it("a box whose workspace this host still names is not deleted: its project is the person's to bring back", async () => {
    const { store, rt, time, machine } = await unrecordedBox();
    await store.put("workspace-names", "ws_d913623a", { workspaceId: "ws_d913623a", name: "kept", project: "p_gone" });
    await rt.reap();
    time.advance(UNRECORDED_DELETE_MS);
    const result = await rt.reap();
    expect(result.failed).toMatchObject([{ id: machine.id, message: expect.stringMatching(/this host holds no project for it.*a machine of yours still paused$/) }]);
    expect(machine.killed).toBe(false);
    await rt.close();
  });

  it("a paused body a settled record no longer names is named and deleted the same way: the engine kills only a running one", async () => {
    const { box, store, rt, time, machine } = await unrecordedBox();
    const ws = await createOn(rt, { golden: "snap_g", name: "lead", on: "box" });
    const { id: owner } = (await store.get("owner", "id")) as { id: string };
    const body = (await box.create({ kind: "sandbox", labels: { wsp: "1", "wsp-owner": owner, "wsp-workspace": ws.id, createdAt: ago(3_600_000) } })) as unknown as { id: string; paused: boolean; killed: boolean };
    body.paused = true;
    const held = (await rt.workspaces.get(ws.id)).machineId;
    // Napped first, so the day the clock moves takes no idle nap that would still be in flight at the sweep.
    await rt.workspaces.nap(ws.id);

    expect((await rt.reap()).failed?.find(f => f.id === body.id)?.message).toContain(`not recorded: ${ws.id} runs on ${held}, so nothing here reaches this paused machine of yours; deleted at`);
    time.advance(UNRECORDED_DELETE_MS);
    expect((await rt.reap()).reaped.map(r => r.id).sort()).toEqual([machine.id, body.id].sort());
    expect(body.killed).toBe(true);
    expect((await rt.workspaces.get(ws.id)).machineId).toBe(held);
    await rt.close();
  });

  it("a place whose listing fails is named and the rest of the sweep still runs", async () => {
    const { box, rt, machine } = await unrecordedBox();
    box.list = async () => {
      throw new Error("fetch failed");
    };
    const result = await rt.reap();
    expect(result.failed).toEqual([{ message: "Boat: fetch failed" }]);
    expect(machine.killed).toBe(false);
    await rt.close();
  });

  it("a lost record of a workspace at the place is recorded again there, not at the provider this host forks on", async () => {
    const solari = stubBackend();
    const box = stubBackend();
    const store = memoryStore();
    const places: PlaceBackends = { wired: "solari", backend: place => ({ solari, box })[place], list: () => ["solari", "box"] };
    const links = wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 });
    const first = createRuntime({ backend: solari, places, store, adapters: {}, placeLinks: links });
    const ws = await createOn(first, { golden: "snap_g", name: "there", on: "box" });
    await first.close();
    const machine = box.machines[0]!;
    machine.spec.labels!["createdAt"] = ago(2 * 60_000);
    machine.paused = true;
    await store.delete("workspaces", ws.id);
    const rt = createRuntime({ backend: solari, places, store, adapters: {}, placeLinks: links });

    expect((await rt.reap()).adopted).toEqual([{ id: machine.id, workspaceId: ws.id, name: "there", phase: "napping" }]);
    expect(await store.get("workspaces", ws.id)).toMatchObject({ place: "box", machineId: machine.id });
    await rt.close();
  });
});

describe("a record the start left as it was, since its place forked nothing then", () => {
  it("keeps every field once the key is saved and the sweep lists its box: nothing adopts it over the stored one", async () => {
    const box = stubBackend();
    const none = new NoProviderBackend();
    let keyed = true;
    const places: PlaceBackends = { wired: "none", backend: p => (p === "box" && keyed ? box : p === "none" ? none : undefined), list: () => (keyed ? ["box"] : []) };
    const store = memoryStore();
    const links = wiring(newPlaceKeyPair());
    const first = createRuntime({ backend: none, places, store, adapters: {}, placeLinks: links });
    const ws = await createOn(first, { golden: "snap_g", name: "there", on: "box" });
    await first.workspaces.nap(ws.id);
    await store.put("workspaces", ws.id, { ...((await store.get("workspaces", ws.id)) as Record<string, unknown>), theme: "rose" });
    await first.close();
    box.machines[0]!.spec.labels!["createdAt"] = ago(2 * 60_000);
    const before = (await store.get("workspaces", ws.id)) as Record<string, unknown>;
    expect(before).toMatchObject({ phase: "napping", theme: "rose", nappedAt: expect.any(Number), shape: expect.any(Object), vaultedAt: expect.anything() });

    keyed = false;
    const rt = createRuntime({ backend: none, places, store, adapters: {}, placeLinks: links });
    await rt.workspaces.list();
    keyed = true;
    const result = await rt.reap();

    expect(result.adopted).toBeUndefined();
    expect(result.failed).toEqual([{ id: box.machines[0]!.id, message: `not recorded: ${ws.id} is stored here but was not read at start, so its paused machine is left as it is until the host starts again` }]);
    expect(await store.get("workspaces", ws.id)).toEqual(before);
    expect(await store.keys("unrecorded-machines")).toEqual([]);
    expect(box.machines[0]!.killed).toBe(false);
    await rt.close();
  });
});

describe("a recorded machine at a provider this host is not wired to", () => {
  it("is found on that provider's listing, so the sweep reads no machine of it one by one", async () => {
    const solari = stubBackend();
    const box = stubBackend();
    const places: PlaceBackends = { wired: "solari", backend: place => ({ solari, box })[place], list: () => ["solari", "box"] };
    const rt = createRuntime({ backend: solari, places, store: memoryStore(), adapters: {}, placeLinks: wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 }) });
    const ws = await createOn(rt, { golden: "snap_g", name: "there", on: "box" });
    await rt.workspaces.nap(ws.id);
    const machine = box.machines[0]!;
    let reads = 0;
    const state = machine.state.bind(machine);
    machine.state = async () => (reads++, state());

    await rt.reap();

    expect(reads).toBe(0);
    await rt.close();
  });
});

describe("a box whose create has not returned", () => {
  for (const wiredToBoat of [false, true]) {
    it(`is not killed while its create runs past its minute with its labels already on it${wiredToBoat ? ", where Boat is the provider this host forks on" : ""}`, async () => {
      const solari = stubBackend();
      const box = stubBackend();
      const places: PlaceBackends = wiredToBoat ? { wired: "box", backend: place => (place === "box" ? box : undefined), list: () => ["box"] } : { wired: "solari", backend: place => ({ solari, box })[place], list: () => ["solari", "box"] };
      const rt = createRuntime({ backend: wiredToBoat ? box : solari, places, store: memoryStore(), adapters: {}, placeLinks: wiring(newPlaceKeyPair(), wiredToBoat ? { id: "box", rateUsdPerHour: 0.018 } : { id: "solari", rateUsdPerHour: 0.11 }) });
      let release: () => void = () => {};
      const held = new Promise<void>(resolve => (release = resolve));
      const realCreate = box.create.bind(box);
      box.create = async (spec, waiting) => {
        const m = await realCreate(spec, waiting);
        (m as unknown as { spec: { labels: Record<string, string> } }).spec.labels["createdAt"] = ago(5 * 60_000);
        await held;
        return m;
      };
      const making = createOn(rt, { golden: "snap_g", name: "slow", on: "box" });
      await until(() => box.machines.length === 1);

      const result = await rt.reap();

      expect(box.machines[0]!.killed).toBe(false);
      expect(result.reaped).toEqual([]);
      release();
      await making;
      await rt.close();
    });
  }
});

describe("a box at the provider this host forks on", () => {
  it("is named and deleted after the same day when no record or name of this host claims it", async () => {
    const box = stubBackend();
    const store = memoryStore();
    const time = fakeClock();
    const rt = createRuntime({ backend: box, store, adapters: {}, clock: time.clock });
    await rt.workspaces.list();
    const { id: owner } = (await store.get("owner", "id")) as { id: string };
    const machine = (await box.create({ kind: "sandbox", labels: { wsp: "1", "wsp-owner": owner, "wsp-workspace": "ws_07165147", createdAt: ago(3_600_000) } })) as unknown as { id: string; paused: boolean; killed: boolean };
    machine.paused = true;

    expect((await rt.reap()).failed).toMatchObject([{ id: machine.id, message: expect.stringContaining("no workspace of this host is ws_07165147") }]);
    time.advance(UNRECORDED_DELETE_MS);
    expect((await rt.reap()).reaped).toMatchObject([{ id: machine.id, reason: "unrecorded" }]);
    expect(machine.killed).toBe(true);
    await rt.close();
  });
});
