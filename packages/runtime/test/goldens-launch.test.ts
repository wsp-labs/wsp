// SPDX-License-Identifier: AGPL-3.0-only
// A launch reads the goldens collection for the kind of every record that stands on an image, and the re-read pass
// does it again for every record at once, so what one read of goldens costs is paid hundreds of times over at the
// same moment. Goldens holds image versions and nothing a fork never reads.
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { HERE_PLACE_ID } from "@wsp/protocol";
import { LocalBackend } from "@wsp/engine";
import { copyKey, createRuntime, type LocalWiring } from "../src/runtime.js";
import { localExecStream } from "../src/local-exec.js";
import { memoryStore, type Store } from "../src/store.js";
import { copyingFake, createOn, stubBackend, testPlatform } from "./stub-backend.js";

const sha = (n: number): string => n.toString(16).padStart(64, "0");
const owned = (files: number, tag: string) =>
  Array.from({ length: files }, (_, i) => ({ path: `.claude/plugins/cache/${tag}/node_modules/pkg-${i % 97}/dist/file-${i}.js`, sha256: sha(i), ...(i % 50 === 0 ? { volatile: true } : {}) }));
const version = (n: number, snapshotId: string, files: number) => ({
  version: n,
  snapshotId,
  templateId: `tpl_${snapshotId}`,
  baseTemplate: "base",
  kind: "sandbox",
  setupSha: "s",
  createdAt: "2026-10-01T00:00:00.000Z",
  smoke: { cmd: "true", exitCode: 0 },
  size: { cpu: 2, memMb: 4096 },
  owned: owned(files, snapshotId),
});
/** Two manifests the way a state that sealed four box versions and one solari version holds them: 7,276 and 1,432
 * owned files on their heads, 8,708 together. */
const manifests = (): Record<string, { head: number; versions: ReturnType<typeof version>[] }> => ({
  [copyKey("default", "default")]: { head: 4, versions: [version(1, "snap_1", 0), version(2, "snap_2", 0), version(3, "snap_3", 0), version(4, "snap_4", 7276)] },
  [copyKey("other", "default")]: { head: 1, versions: [version(1, "snap_o1", 1432)] },
});
const without = <T extends { versions: { owned?: unknown }[] }>(m: T) => ({ ...m, versions: m.versions.map(({ owned: _, ...v }) => v) });

/** A store that says how many bytes of JSON each read of goldens handed back, and every write to it. */
function countingStore(inner: Store) {
  const sizes = new WeakMap<object, number>();
  const sizeOf = (doc: unknown): number => {
    const o = doc as object;
    let n = sizes.get(o);
    if (n === undefined) sizes.set(o, (n = JSON.stringify(doc).length));
    return n;
  };
  const tally = { listed: 0, lists: 0, puts: [] as string[] };
  const store: Store = {
    ...inner,
    async list(collection) {
      const docs = await inner.list(collection);
      if (collection === "goldens") {
        tally.lists++;
        for (const d of docs) tally.listed += sizeOf(d);
      }
      return docs;
    },
    async put(collection, id, value) {
      if (collection === "goldens") tally.puts.push(id);
      return inner.put(collection, id, value);
    },
  };
  return { store, tally };
}

/** Waits until the goldens reads stop for a quarter of a second: the re-read pass runs after ready() answers. */
async function settled(tally: { lists: number }): Promise<void> {
  let last = -1;
  while (last !== tally.lists) {
    last = tally.lists;
    await new Promise(r => setTimeout(r, 250));
  }
}

describe("goldens at launch", () => {
  let root: string;
  let local: LocalWiring;
  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), "wsp-goldens-launch-"));
    local = {
      backend: new LocalBackend({ root }),
      execStream: o => localExecStream({ root, runDir: join(root, "runs"), ...o }),
      home: () => join(root, ".claude"),
      homeDir: root,
      rootsPath: join(root, "roots"),
      env: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
      platform: testPlatform(),
      copier: copyingFake(),
    };
  });
  afterEach(() => rmSync(root, { recursive: true, force: true }));

  /** A state of `cloud` records on the box head and `here` records on this computer, cloned off one of each made
   * through the runtime; each case puts the goldens it launches over. */
  async function stateOf(cloud: number, here: number): Promise<Store> {
    const backend = stubBackend();
    const store = memoryStore();
    const first = createRuntime({ backend, store, adapters: {}, local });
    const onCloud = await createOn(first, { golden: "snap_4", name: "c" });
    const onHere = await createOn(first, { on: HERE_PLACE_ID, name: "h" });
    await first.close();
    const cloudRecord = (await store.get("workspaces", onCloud.id)) as Record<string, unknown>;
    const hereRecord = (await store.get("workspaces", onHere.id)) as Record<string, unknown>;
    expect(hereRecord["golden"]).toBe("");
    for (let i = 0; i < cloud; i++) await store.put("workspaces", `wc${i}`, { ...cloudRecord, id: `wc${i}`, name: `c${i}`, machineId: onCloud.machineId });
    for (let i = 0; i < here; i++) await store.put("workspaces", `wh${i}`, { ...hereRecord, id: `wh${i}`, name: `h${i}` });
    return store;
  }

  it("a seal's owned lists leave every stored manifest at the first launch, written once, every other field as it was", async () => {
    const store = await stateOf(0, 0);
    for (const [key, m] of Object.entries(manifests())) await store.put("goldens", key, m);
    const { store: counted, tally } = countingStore(store);

    const rt = createRuntime({ backend: stubBackend(), store: counted, adapters: {}, local });
    await rt.workspaces.list();
    await rt.close();

    for (const [key, m] of Object.entries(manifests())) expect(await store.get("goldens", key)).toEqual(without(m));
    expect(tally.puts.sort()).toEqual(Object.keys(manifests()).sort());

    tally.puts.length = 0;
    const again = createRuntime({ backend: stubBackend(), store: counted, adapters: {}, local });
    await again.workspaces.list();
    await again.close();
    expect(tally.puts).toEqual([]);
  });

  it("600 records read under 10 MB of goldens through the whole launch, and golden.get answers under 20 KB", async () => {
    const store = await stateOf(414, 186);
    for (const [key, m] of Object.entries(manifests())) await store.put("goldens", key, m);
    const whole = Object.values(manifests()).reduce((n, m) => n + JSON.stringify(m).length, 0);
    expect(whole).toBeGreaterThan(1_300_000);
    const { store: counted, tally } = countingStore(store);

    const rt = createRuntime({ backend: stubBackend(), store: counted, adapters: {}, local });
    await rt.workspaces.list();
    await settled(tally);

    expect(tally.listed).toBeLessThan(10_000_000);
    expect(JSON.stringify({ id: "1", ok: true, manifest: await rt.golden.get() }).length).toBeLessThan(20_000);
    await rt.close();
  }, 60_000);

  it("records on this computer stand on no image, and launching them reads no goldens", async () => {
    const store = await stateOf(0, 186);
    for (const id of await store.keys("workspaces")) if (!id.startsWith("wh")) await store.delete("workspaces", id);
    const { store: counted, tally } = countingStore(store);

    const rt = createRuntime({ backend: stubBackend(), store: counted, adapters: {}, local });
    await rt.workspaces.list();
    await settled(tally);
    await rt.close();

    expect(tally.lists).toBe(0);
  });
});
