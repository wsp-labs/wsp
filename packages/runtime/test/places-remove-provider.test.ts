// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import { OWNER_LABEL, WSP_LABEL, type MachineBackend, type MachineSpec } from "@wsp/engine";
import { PROVIDER_LEFT_WORDS, providerImagesKeptLine, providerKeptLine, providerKeyRefused, providerKeyStillLine, providerUnlistedLine, refusalLine, type GoldenManifest } from "@wsp/protocol";
import { createRuntime, type PlaceBackends } from "../src/runtime.js";
import { newPlaceKeyPair } from "../src/places.js";
import { serveRuntime } from "../src/serve.js";
import { memoryStore, type Store } from "../src/store.js";
import { GOLDENS, PROJECTS, PROJECT_GOLDENS, copyKey } from "../src/types/internal.js";
import { answersGoneOnce, stubBackend, createOn, type StubBackend } from "./stub-backend.js";
import { WsClient } from "./ws-client.js";
import { wiring } from "./place-join.js";
import { ctx, placesOf, remove } from "./places-fixture.js";

/** A host wired to Solari that also holds a Boat key, which `forgetKey` takes away: the row goes once it is gone. */
async function holdingBoat(box: StubBackend, still?: string, o: { store?: Store; solari?: StubBackend } = {}): Promise<{ forgot: string[]; aliveAtForget: () => number; snapshotsAtForget: () => string[] }> {
  const solari = o.solari ?? stubBackend();
  const at: Record<string, MachineBackend> = { solari, box };
  const forgot: string[] = [];
  let alive = -1;
  let snapshots: string[] = [];
  const places: PlaceBackends = {
    get wired() {
      return "solari";
    },
    backend: place => (forgot.includes(place) ? undefined : at[place]),
    list: () => Object.keys(at).filter(id => !forgot.includes(id)),
  };
  ctx.runtime = createRuntime({
    backend: solari,
    store: o.store ?? memoryStore(),
    adapters: {},
    places,
    killConfirm: { graceMs: 20, pollMs: 1 },
    placeLinks: {
      ...wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 }),
      forgetKey: async placeId => {
        alive = box.machines.filter(m => !m.killed).length;
        snapshots = box.snapshots.map(r => r.id);
        forgot.push(placeId);
        return still;
      },
    },
  });
  ctx.srv = await serveRuntime(ctx.runtime, { port: 0, authToken: "host-token", devices: ctx.runtime.devices });
  return { forgot, aliveAtForget: () => alive, snapshotsAtForget: () => snapshots };
}

const sealed = (snapshotId: string): GoldenManifest => ({ head: 1, versions: [{ version: 1, snapshotId, baseTemplate: "base", setupSha: "s", createdAt: "2026-10-16T13:33:52.413Z", smoke: { cmd: "true", exitCode: 0 } }] });

/** This host's image sealed at Boat with a project image beside it, and the wired cloud's own image, each listed by its provider. */
async function imagesAt(box: StubBackend, solari: StubBackend): Promise<Store> {
  const store = memoryStore();
  await store.put(GOLDENS, copyKey("box", "default"), sealed("wsp-acme-default-v1"));
  await store.put(PROJECT_GOLDENS, "wsp-acme-project-api", { snapshotId: "wsp-acme-project-api", projects: [], golden: "default", workspaceId: "w1", workspaceName: "api", createdAt: "2026-10-16T14:00:00.000Z", place: "box" });
  await store.put(GOLDENS, "default", sealed("snap_solari"));
  box.snapshots.push({ id: "wsp-acme-default-v1", name: "wsp-acme-default-v1", sizeBytes: 345_000_000 }, { id: "wsp-acme-project-api", name: "wsp-acme-project-api", sizeBytes: 9_120_485_376 });
  solari.snapshots.push({ id: "snap_solari", sizeBytes: 8_000_000_000 });
  return store;
}

async function holdsOf(placeId: string): Promise<Record<string, unknown>> {
  const c = await WsClient.connect(ctx.srv!.port, { token: "host-token" });
  const answer = await c.request("places.holds", { placeId });
  c.close();
  return answer;
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
  it("says a machine whose read failed after the delete could not be read, and one whose delete failed could not be deleted", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    await createOn(ctx.runtime!, { golden: "snap_g", name: "b", on: "box" });
    const [unread, undeleted] = box.machines;
    // The first read finds it; every read once it was asked to go answers 429.
    const get = box.get.bind(box);
    box.get = async id => (id === unread!.id && unread!.killed ? Promise.reject(Object.assign(new Error("Too Many Requests"), { kind: "concurrency", status: 429 })) : get(id));
    undeleted!.kill = async () => Promise.reject(Object.assign(new Error("Internal Server Error"), { kind: "unknown", status: 500 }));

    const answer = await remove("box");
    expect(answer).toMatchObject({ removed: false, deleted: 0 });
    const note = String(answer["note"]);
    expect(note).toContain(`${unread!.id} (${PROVIDER_LEFT_WORDS.unread("Too Many Requests")})`);
    expect(note).toContain(`${undeleted!.id} (${PROVIDER_LEFT_WORDS.undeleted("Internal Server Error")})`);
    expect(held.forgot).toEqual([]);
  });

  it("says a listing that failed after the deletes in the kept line's way, with how many went, and keeps the key", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    await createOn(ctx.runtime!, { golden: "snap_g", name: "b", on: "box" });
    const list = box.list.bind(box);
    let lists = 0;
    box.list = async labels => (++lists === 2 ? Promise.reject(Object.assign(new Error("Internal Server Error"), { kind: "unknown", status: 500 })) : list(labels));

    expect(await remove("box")).toMatchObject({ removed: false, deleted: 2, note: providerUnlistedLine("Boat", 2, "Internal Server Error") });
    expect(box.machines.filter(m => !m.killed)).toEqual([]);
    expect(held.forgot).toEqual([]);
  });

  it("answers where the key still is when the host also reads it from beyond its own file", async () => {
    const box = stubBackend();
    const still = providerKeyStillLine([{ file: "/work/acme/.env" }]);
    const held = await holdingBoat(box, still);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    expect(await remove("box")).toMatchObject({ removed: true, deleted: 1, note: still });
    expect(held.forgot).toEqual(["box"]);
  });
  it("drops the count from a failed relist when nothing was deleted", async () => {
    const box = stubBackend();
    const held = await holdingBoat(box);
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    box.machines[0]!.killed = true;
    const list = box.list.bind(box);
    let lists = 0;
    box.list = async labels => (++lists === 2 ? Promise.reject(Object.assign(new Error("Too Many Requests"), { kind: "concurrency", status: 429 })) : list(labels));

    const answer = await remove("box");
    expect(answer).toMatchObject({ removed: false, deleted: 0, note: providerUnlistedLine("Boat", 0, "Too Many Requests") });
    expect(answer["note"]).toBe("Boat's machines could not be listed again (Too Many Requests), so one made meanwhile may still be there and the key is kept. Remove Boat again to finish.");
    expect(held.forgot).toEqual([]);
  });

  it("names this host's image snapshots there with their sizes, and deletes each before the key goes", async () => {
    const box = stubBackend();
    const solari = stubBackend();
    const store = await imagesAt(box, solari);
    const held = await holdingBoat(box, undefined, { store, solari });
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });

    expect((await holdsOf("box"))["images"]).toEqual([
      { id: "wsp-acme-default-v1", sizeBytes: 345_000_000 },
      { id: "wsp-acme-project-api", sizeBytes: 9_120_485_376 },
    ]);

    expect(await remove("box")).toMatchObject({ removed: true, deleted: 1, snapshots: 2 });
    expect(held.forgot).toEqual(["box"]);
    expect(held.snapshotsAtForget()).toEqual([]);
    expect(box.snapshots).toEqual([]);
    // The wired cloud's image is another provider's, and its record stays.
    expect(solari.snapshots.map(r => r.id)).toEqual(["snap_solari"]);
    expect(await store.get(GOLDENS, copyKey("solari", "default"))).toBeDefined();
    expect(await store.get(GOLDENS, copyKey("box", "default"))).toBeUndefined();
    expect(await store.get(PROJECT_GOLDENS, "wsp-acme-project-api")).toBeUndefined();
  });

  it("names a snapshot the provider will not delete and keeps the key and its record until it goes", async () => {
    const box = stubBackend();
    const solari = stubBackend();
    const store = await imagesAt(box, solari);
    const held = await holdingBoat(box, undefined, { store, solari });
    await createOn(ctx.runtime!, { golden: "snap_g", name: "a", on: "box" });
    const deleteSnapshot = box.deleteSnapshot.bind(box);
    box.deleteSnapshot = async id => (id === "wsp-acme-default-v1" ? Promise.reject(Object.assign(new Error("Internal Server Error"), { kind: "unknown", status: 500 })) : deleteSnapshot(id));

    const answer = await remove("box");
    expect(answer).toMatchObject({ removed: false, deleted: 1, note: providerImagesKeptLine("Boat", 1, [{ id: "wsp-acme-default-v1", said: PROVIDER_LEFT_WORDS.undeleted("Internal Server Error") }]) });
    expect(String(answer["note"])).toContain("wsp-acme-default-v1");
    expect(held.forgot).toEqual([]);
    expect((await placesOf()).map(p => p.id)).toContain("box");
    expect(box.snapshots.map(r => r.id)).toEqual(["wsp-acme-default-v1"]);
    expect(await store.get(GOLDENS, copyKey("box", "default"))).toBeDefined();
    expect(await store.get(PROJECT_GOLDENS, "wsp-acme-project-api")).toBeUndefined();

    // Once the provider takes it, the same remove goes through and the key with it.
    box.deleteSnapshot = deleteSnapshot;
    expect(await remove("box")).toMatchObject({ removed: true, deleted: 0 });
    expect(box.snapshots).toEqual([]);
    expect(held.forgot).toEqual(["box"]);
  });

  it("deletes the image a project seeded there holds before the key goes, and the project keeps no image", async () => {
    const box = stubBackend();
    const store = memoryStore();
    await store.put(PROJECTS, "p_api", { id: "p_api", name: "api", computer: "box", source: { kind: "git", url: "https://github.com/acme/api.git" }, path: "/root/api", remote: "https://github.com/acme/api.git", defaultBranch: "main", memoryKey: "-root-api", memoryDir: "/root/.claude/projects/-root-api", image: { snapshotId: "api-p_api", builtAt: "2026-10-16T14:00:00.000Z" }, createdAt: "2026-10-16T14:00:00.000Z" });
    box.snapshots.push({ id: "api-p_api", name: "api-p_api", sizeBytes: 2_000_000_000 });
    const held = await holdingBoat(box, undefined, { store });

    expect((await holdsOf("box"))["images"]).toEqual([{ id: "api-p_api", sizeBytes: 2_000_000_000 }]);
    expect(await remove("box")).toMatchObject({ removed: true, snapshots: 1 });
    expect(held.snapshotsAtForget()).toEqual([]);
    expect(((await store.get(PROJECTS, "p_api")) as { image?: unknown }).image).toBeUndefined();
  });
});
