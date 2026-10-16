// SPDX-License-Identifier: AGPL-3.0-only
// Each workspace's view says how the computer its own machine stands on naps,
// read off the backend that holds that machine: never the provider the host is
// wired to, and never the computer its project lands on, which a sub-thread on
// another cloud does not share.
import { afterEach, describe, expect, it } from "vitest";
import type { MachineBackend } from "@wsp/engine";
import { createRuntime, type PlaceBackends, type Runtime } from "../src/runtime.js";
import { newPlaceKeyPair } from "../src/places.js";
import { memoryStore } from "../src/store.js";
import { stubBackend, createOn } from "./stub-backend.js";
import { leadOnThisComputer } from "./sub-fixture.js";
import { report, wiring } from "./place-join.js";
import { asRoot, code, ctx, forks, join, KEEPS_NO_IMAGE, placesOf, relink, ROOT_LOGIN, serving, sockets } from "./places-fixture.js";
import { until } from "./until.js";

let open: Runtime[] = [];
afterEach(async () => {
  for (const rt of open) await rt.close().catch(() => undefined);
  open = [];
});

const twoProviders = (wired: string, at: Record<string, MachineBackend>): PlaceBackends => ({
  get wired() {
    return wired;
  },
  backend: place => at[place],
  list: () => Object.keys(at),
});

describe("a workspace's pause mode", () => {
  it("is its own provider's on a host wired to another: a fork at a cloud whose nap ends every process says so beside one at the wired cloud that keeps the memory", async () => {
    const solari = stubBackend();
    const box = stubBackend();
    box.capabilities.pauseMode = "disk";
    const rt = createRuntime({ backend: solari, store: memoryStore(), adapters: {}, places: twoProviders("solari", { solari, box }), placeLinks: wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 }) });
    open.push(rt);
    const there = await createOn(rt, { golden: "snap_g", name: "on-box", on: "box" });
    const here = await createOn(rt, { golden: "snap_g", name: "on-solari", on: "solari" });
    expect([there.place, here.place]).toEqual(["box", undefined]);
    expect((await rt.workspaces.get(there.id)).pauseMode).toBe("disk");
    expect((await rt.workspaces.get(here.id)).pauseMode).toBe("memory");
    expect(Object.fromEntries((await rt.workspaces.list()).map(w => [w.name, w.pauseMode]))).toEqual({ "on-box": "disk", "on-solari": "memory" });
  });

  it("is a Boat sub-thread's own on a host wired to a cloud that keeps the memory, whatever its project's computer does", async () => {
    const solari = stubBackend();
    // Boat joins the table once the fixture has made it, and the host starts again on the same state to hold both.
    const at: Record<string, MachineBackend> = { solari };
    const held = await leadOnThisComputer({ host: { backend: solari, places: twoProviders("solari", at), placeLinks: wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 }) } });
    await held.rt.close();
    held.boat.capabilities.pauseMode = "disk";
    at["box"] = held.boat;
    const rt = held.restart();
    open.push(rt);
    const made = await rt.workspaces.create({ project: held.project.id, name: "Kid", sub: { on: "boat" } }, held.lead);
    expect(made.place).toBe("box");
    expect((await rt.workspaces.get(made.id)).pauseMode).toBe("disk");
    expect((await rt.workspaces.get(held.folder.id)).pauseMode).toBeUndefined();
  }, 40_000);

  it("is none on an older fork of a joined computer while its updated daemon has not said again what it forks with, and the list still answers", async () => {
    const { hostKey } = await serving();
    const { client, placeId, pair } = await join(hostKey, { code: await code(), report: report("srv", { login: ROOT_LOGIN }), answers: c => forks(c, undefined, asRoot, KEEPS_NO_IMAGE) });
    sockets.push(client.ws);
    // A fork made there before a project on a joined computer was a folder in its login's home, which a host still holds.
    const door = ctx.runtime!.places!;
    const joinedAt = door.joined;
    door.joined = () => false;
    const fork = await createOn(ctx.runtime!, { name: "x", on: "srv" }).finally(() => (door.joined = joinedAt));
    expect(fork.kind).toBe("cloud");
    expect((await ctx.runtime!.workspaces.get(fork.id)).pauseMode).toBe("memory");
    // The computer's daemon updates and dials back: the host drops what the old one said it forks with, and the new
    // one has not answered the ask yet.
    client.close();
    await until(async () => (await placesOf()).find(p => p.id === placeId)!.present === false);
    const back = await relink(hostKey, placeId, pair, report("srv", { login: ROOT_LOGIN, daemonVersion: report().daemonVersion + 1 }), c => forks(c, undefined, asRoot, KEEPS_NO_IMAGE).swallow.add("machine.backend"));
    sockets.push(back.client.ws);
    await until(async () => (await placesOf()).find(p => p.id === placeId)!.present === true);
    const listed = (await ctx.runtime!.workspaces.list()).find(w => w.id === fork.id);
    expect(listed).toMatchObject({ id: fork.id, place: placeId });
    expect(listed!.pauseMode).toBeUndefined();
  }, 40_000);
});
