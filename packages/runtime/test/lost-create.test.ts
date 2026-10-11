// SPDX-License-Identifier: AGPL-3.0-only
// A create whose answer never came back: the attempt keeps the bytes it sent, and the sweep asks again under its key
// and settles the machine the provider made, or deletes the box named after one of this host's images that nothing
// names. The provider is the stub with the three calls a key-keeping provider carries.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { goldenName, type MachineSpec, type NamedMachine } from "@wsp/engine";
import { createRuntime } from "../src/runtime.js";
import { memoryStore } from "../src/store.js";
import { templateHost } from "../src/host-id.js";
import type { PendingCreate } from "../src/types/internal.js";
import { createOn, stubBackend, type StubBackend, type StubMachine } from "./stub-backend.js";

const HOST = "boat-test";
const T0 = Date.parse("2026-10-11T10:00:00.000Z");
/** Above any pid the kernel hands out, so no process holds it. */
const DEAD_PID = 4_194_305;
const minutesAgo = (n: number): string => new Date(Date.now() - n * 60_000).toISOString();

interface Sent {
  key: string;
  bytes: string;
}

/** The stub as a provider that keeps a key: the bytes a create sends carry the moment they were made, a create may
 * lose its answer after the machine is made, a replay answers the machine its key made and names it after the
 * bytes, and the listing by name sees a machine made from an image under that image's name until then. */
function keyKeeping(backend: StubBackend) {
  const real = backend.create.bind(backend);
  const sent: Sent[] = [];
  const replays: Sent[] = [];
  const madeBy = new Map<string, StubMachine>();
  const names = new Map<string, string>();
  const createdAt = new Map<string, string>();
  const asked: string[][] = [];
  const kk = { sent, replays, madeBy, names, createdAt, asked, lose: 0, hold: undefined as Promise<void> | undefined };
  backend.createRequest = async (spec: MachineSpec) => JSON.stringify({ labels: spec.labels ?? {}, from: spec.fromSnapshot ?? spec.template, made: `${Date.now()}.${Math.random()}` });
  backend.create = async (spec, waiting, bytes) => {
    const body = bytes ?? (await backend.createRequest!(spec));
    sent.push({ key: spec.idempotencyKey!, bytes: body });
    await kk.hold;
    if (kk.lose > 0) {
      kk.lose--;
      const m = (await real({ ...spec, labels: {} }, waiting)) as StubMachine;
      madeBy.set(spec.idempotencyKey!, m);
      names.set(m.id, spec.fromSnapshot ?? spec.template ?? "");
      throw new TypeError("fetch failed");
    }
    return real(spec, waiting);
  };
  backend.replayCreate = async (key, bytes) => {
    replays.push({ key, bytes });
    const m = madeBy.get(key) ?? ((await real({ kind: "sandbox" })) as StubMachine);
    const labels = (JSON.parse(bytes) as { labels: Record<string, string> }).labels;
    m.spec = { ...m.spec, labels };
    (m as { labels?: Record<string, string> }).labels = labels;
    names.delete(m.id);
    return m;
  };
  backend.namedAfter = async (wanted): Promise<NamedMachine[]> => {
    asked.push([...wanted].sort());
    return backend.machines
      .filter(m => !m.killed && wanted.has(names.get(m.id) ?? ""))
      .map(m => ({ id: m.id, name: names.get(m.id)!, state: "running", ...(createdAt.has(m.id) ? { createdAt: createdAt.get(m.id)! } : {}) }));
  };
  /** A machine the provider lists under `name` and no labels, made `age` minutes ago. */
  const stray = async (name: string, age: number): Promise<StubMachine> => {
    const m = (await real({ kind: "sandbox" })) as StubMachine;
    names.set(m.id, name);
    createdAt.set(m.id, minutesAgo(age));
    return m;
  };
  return Object.assign(kk, { stray });
}

function setup(store = memoryStore(), backend = stubBackend()) {
  const kk = keyKeeping(backend);
  const rt = createRuntime({ backend, store, adapters: {}, hostId: HOST });
  return { rt, store, backend, kk };
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(T0);
  vi.spyOn(console, "warn").mockImplementation(() => {});
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("a create whose answer is lost", () => {
  it("keeps the exact bytes it sent beside its key, and a create asked again for the record sends those bytes", async () => {
    const { rt, store, kk } = setup();
    const ws = await createOn(rt, { golden: "snap_g", name: "x" });
    kk.lose = 1;
    await expect(rt.workspaces.rebuild(ws.id)).rejects.toThrow("fetch failed");
    const first = kk.sent.at(-1)!;
    expect((await store.list("creates")) as PendingCreate[]).toEqual([expect.objectContaining({ key: first.key, request: first.bytes })]);
    vi.setSystemTime(T0 + 30_000);
    await rt.workspaces.rebuild(ws.id);
    const again = kk.sent.at(-1)!;
    expect(again).toEqual(first);
    expect(await store.list("creates")).toEqual([]);
  });

  it("a workspace whose create lost its answer leaves the attempt for the sweep, which replays it after two minutes and deletes the box no record waits for", async () => {
    const { rt, store, kk } = setup();
    kk.lose = 1;
    await expect(createOn(rt, { golden: "snap_g", name: "x" })).rejects.toThrow("fetch failed");
    const [lost] = [...kk.madeBy.values()];
    const sent = kk.sent.at(-1)!;
    expect((await store.list("creates")) as PendingCreate[]).toEqual([expect.objectContaining({ key: sent.key, request: sent.bytes })]);

    vi.setSystemTime(T0 + 119_000);
    await rt.reap();
    expect(kk.replays).toEqual([]);
    expect(lost!.killed).toBe(false);

    vi.setSystemTime(T0 + 120_000);
    const swept = await rt.reap();
    expect(kk.replays).toEqual([sent]);
    expect(lost!.killed).toBe(true);
    expect(swept.reaped).toContainEqual(expect.objectContaining({ id: lost!.id, reason: "lost" }));
    expect(await store.list("creates")).toEqual([]);
  });

  it("the sweep adopts the box when its record still waits for it: the name was kept and no record holds the workspace", async () => {
    const store = memoryStore();
    const backend = stubBackend();
    const first = setup(store, backend);
    const ws = await createOn(first.rt, { golden: "snap_g", name: "kept" });
    const labels = { ...backend.machines.at(-1)!.spec.labels!, createdAt: minutesAgo(3) };
    await first.rt.close();
    backend.machines.at(-1)!.killed = true;
    await store.delete("workspaces", ws.id);
    // The earlier process asked for a machine for this workspace and died before the answer came.
    const key = `workspace/${ws.id}:00112233aabbccdd`;
    const bytes = JSON.stringify({ labels, from: "snap_g", made: "earlier" });
    const lost = await first.kk.stray("snap_g", 3);
    first.kk.madeBy.set(key, lost);
    await store.put("creates", `workspace/${ws.id}`, { key, createdAt: minutesAgo(3), body: "earlier", request: bytes, image: "snap_g", host: HOST, pid: DEAD_PID } satisfies PendingCreate);

    const second = createRuntime({ backend, store, adapters: {}, hostId: HOST });
    try {
      const swept = await second.reap();
      expect(first.kk.replays).toEqual([{ key, bytes }]);
      expect(lost.killed).toBe(false);
      expect(swept.adopted).toContainEqual(expect.objectContaining({ id: lost.id, workspaceId: ws.id }));
      expect(await second.workspaces.get(ws.id)).toMatchObject({ machineId: lost.id });
      expect(await store.list("creates")).toEqual([]);
    } finally {
      await second.close();
    }
  });

  it("a create still in flight in this process is never replayed, however old its attempt", async () => {
    const { rt, store, kk } = setup();
    let release!: () => void;
    kk.hold = new Promise<void>(resolve => (release = resolve));
    const making = createOn(rt, { golden: "snap_g", name: "slow" });
    await vi.waitFor(async () => expect(await store.list("creates")).toHaveLength(1));
    vi.setSystemTime(T0 + 5 * 60_000);
    await rt.reap();
    expect(kk.replays).toEqual([]);
    release();
    await making;
    expect(await store.list("creates")).toEqual([]);
  });
});

describe("a box named after one of this host's images", () => {
  it("is deleted once it is ten minutes old and no record or attempt names it; a box under any other name is never touched", async () => {
    const { rt, store, kk } = setup();
    const image = goldenName(templateHost(HOST) + templateHost(await rt.owner()), "default", 4);
    const ws = await createOn(rt, { golden: image, name: "w" });
    const own = (await rt.workspaces.get(ws.id)).machineId;
    kk.names.set(own, image);
    kk.createdAt.set(own, minutesAgo(30));

    const stray = await kk.stray(image, 11);
    const young = await kk.stray(image, 9);
    const untouched = [
      await kk.stray(`${image}-copy`, 60),
      await kk.stray(goldenName("otherhost", "default", 4), 60),
      await kk.stray("my box", 60),
      await kk.stray("", 60),
    ];
    // An attempt of this host that asked for the same image, not yet two minutes old: the box may be its.
    await store.put("creates", "workspace/ws_pending", { key: "workspace/ws_pending:00", createdAt: minutesAgo(1), body: "b", request: "{}", image, host: HOST, pid: DEAD_PID } satisfies PendingCreate);
    await rt.reap();
    expect(stray.killed).toBe(false);
    await store.delete("creates", "workspace/ws_pending");

    const swept = await rt.reap();
    expect(stray.killed).toBe(true);
    expect(swept.reaped).toContainEqual(expect.objectContaining({ id: stray.id, reason: "stray", name: image }));
    expect(young.killed).toBe(false);
    expect((await rt.workspaces.get(ws.id)).machineId).toBe(own);
    expect(kk.names.has(own)).toBe(true);
    expect(untouched.map(m => m.killed)).toEqual([false, false, false, false]);
    // The listing is asked for this host's image names alone, so no other box is even read.
    expect(kk.asked.at(-1)).toEqual([image]);
  });
});
