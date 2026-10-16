// SPDX-License-Identifier: AGPL-3.0-only
import { PassThrough } from "node:stream";
import { stripVTControlCharacters } from "node:util";
import { BOX_PRICING, SNAPSHOT_STORAGE, retentionPlan, snapshotStorage } from "@wsp/engine";
import { copyKey, createRuntime, memoryStore, type Runtime, type Store } from "@wsp/runtime";
import { describe, expect, it } from "vitest";
import { describeOrphanOffer, describeOrphans, describeRetention, describeStorage, noProviderStorageLine, retentionOffer } from "../src/storage.js";
import { stubBackend, type StubBackend } from "./stub-backend.js";
import { createOn, projectOn } from "./verbs-fixture.js";

const GB = 1e9;
const PRICING = SNAPSHOT_STORAGE;
const version = (n: number, parent?: number) => ({ version: n, snapshotId: `snap_golden-v${n}`, baseTemplate: "base", setupSha: `sha${n}`, createdAt: `2026-08-${10 + n}T00:00:00.000Z`, smoke: { cmd: "true", exitCode: 0 }, ...(parent !== undefined ? { parentSnapshotId: `snap_golden-v${parent}` } : {}) });

/** Every snapshot on the account is this host's and recorded, which is what the split leaves out of the line. */
const allRecorded = (count: number, bytes: number) => ({ kept: { count, bytes }, orphans: { count: 0, bytes: 0 }, others: { count: 0, bytes: 0 } });

describe("the storage line", () => {
  it("says the count, the size the listing reports and the monthly cost above the free GB at the published rate", () => {
    expect(describeStorage({ count: 6, totalBytes: 49.8 * GB, ...PRICING, monthlyUsd: 39.8 * 0.05, ...allRecorded(6, 49.8 * GB) })).toBe("storage: 6 snapshots, 49.8 GB; about $1.99/month above the free 10 GB from 2026-10-01");
  });

  it("inside the free GB it says there is nothing to pay, and one snapshot is singular", () => {
    expect(describeStorage({ count: 1, totalBytes: 7.8 * GB, ...PRICING, monthlyUsd: 0, ...allRecorded(1, 7.8 * GB) })).toBe("storage: 1 snapshot, 7.8 GB; inside the free 10 GB, nothing to pay from 2026-10-01");
  });

  it("when the account holds more than this host records, the line says who the rest belong to", () => {
    const s = { count: 6, totalBytes: 92.6 * GB, ...PRICING, monthlyUsd: 82.6 * 0.05, kept: { count: 1, bytes: 11.3 * GB }, orphans: { count: 3, bytes: 57.3 * GB }, others: { count: 2, bytes: 24.0 * GB } };
    expect(describeStorage(s)).toBe("storage: 6 snapshots, 92.6 GB; about $4.13/month above the free 10 GB from 2026-10-01. 1 kept here, 11.3 GB; 3 this host's with nothing recording them, 57.3 GB; 2 not this host's, 24.0 GB");
  });

  it("a group with nothing in it is left out rather than printed as zero", () => {
    const s = { count: 4, totalBytes: 40 * GB, ...PRICING, monthlyUsd: 30 * 0.05, kept: { count: 3, bytes: 30 * GB }, orphans: { count: 0, bytes: 0 }, others: { count: 1, bytes: 10 * GB } };
    expect(describeStorage(s)).toContain("3 kept here, 30.0 GB; 1 not this host's, 10.0 GB");
    expect(describeStorage(s)).not.toContain("0 this host's");
  });
});

describe("the storage line on Boat, which bills named snapshots by count", () => {
  const rows = (named: number) => Array.from({ length: named }, (_, i) => ({ id: `wsp-h1-default-v${i}`, name: `wsp-h1-default-v${i}`, sizeBytes: 1 * GB }));
  const read = (r: { id: string }[]) => ({ hostId: "h1", recorded: new Set(r.map(x => x.id)), now: 0 });

  it("past the 10 free it counts the named snapshots and says what the ones past them cost, with no GB price and no billed-from", () => {
    const r = rows(14);
    expect(describeStorage(snapshotStorage(r, BOX_PRICING.snapshotStorage, read(r)))).toBe("storage: 14 named snapshots, 14.0 GB; about $6.80/month for the 4 past the 10 free");
  });

  it("inside the 10 free it says there is nothing to pay", () => {
    const r = rows(8);
    expect(describeStorage(snapshotStorage(r, BOX_PRICING.snapshotStorage, read(r)))).toBe("storage: 8 named snapshots, 8.0 GB; inside the 10 free, nothing to pay");
  });
});

describe("the line a host with no provider prints instead", () => {
  it("says nothing was listed, why, and where the key it has none of is read", () => {
    expect(noProviderStorageLine("/tmp/proof/state.json")).toBe("storage: nothing listed; this host is wired to no provider (no key beside /tmp/proof/state.json)");
    expect(noProviderStorageLine()).toBe("storage: nothing listed; this host is wired to no provider");
  });
});

describe("the orphan lines", () => {
  const snapshot = (id: string, name: string, gb: number) => ({ id, name, sizeBytes: gb * GB });

  it("names every orphan with the name it carries and the id a delete takes, then every row left alone and why", () => {
    const plan = {
      snapshots: [snapshot("snap_a", "wsp-h1-default-v9", 20)],
      templates: [{ id: "tpl_old", name: "wsp-h1-old-v1", status: "ready" as const }],
      freedBytes: 20 * GB,
      savesUsdPerMonth: 20 * 0.05,
      others: { snapshots: [snapshot("snap_b", "golden-v1", 21)], templates: [{ id: "base", name: "base", status: "ready" as const }] },
    };
    expect(describeOrphans(plan)).toEqual([
      "  orphan template wsp-h1-old-v1 (tpl_old)",
      "  orphan snapshot wsp-h1-default-v9 (snap_a), 20.0 GB",
      "  left alone, no mark of this host: template base (base)",
      "  left alone, no mark of this host: snapshot golden-v1 (snap_b), 21.0 GB",
    ]);
  });

  it("a template holds no snapshot bytes, so a template-only offer never blames the free GB for saving nothing", () => {
    const plan = { snapshots: [], templates: [{ id: "tpl_old", name: "wsp-h1-old-v1", status: "ready" as const }], freedBytes: 0, savesUsdPerMonth: 0, others: { snapshots: [], templates: [] } };
    expect(describeOrphanOffer(plan, false, PRICING)).toBe("1 orphan template, holding no snapshot bytes; wsp doctor --yes deletes them");
    expect(describeOrphanOffer(plan, true, PRICING)).toBe("deleting 1 orphan template, holding no snapshot bytes");
    expect(describeOrphanOffer(plan, false, PRICING)).not.toContain("free GB");
    expect(describeOrphanOffer(plan, false, PRICING)).not.toContain("0.0 GB");
    // Two of them read the same way; the clause never has to agree with a count.
    const two = { ...plan, templates: [...plan.templates, { id: "tpl_older", name: "wsp-h1-old-v0", status: "ready" as const }] };
    expect(describeOrphanOffer(two, false, PRICING)).toBe("2 orphan templates, holding no snapshot bytes; wsp doctor --yes deletes them");
  });

  it("snapshots inside the free GB still say nothing is saved yet, and a saving above it is named", () => {
    const snapshot = { id: "snap_a", name: "wsp-h1-default-v9", sizeBytes: 3 * GB };
    const free = { snapshots: [snapshot], templates: [], freedBytes: 3 * GB, savesUsdPerMonth: 0, others: { snapshots: [], templates: [] } };
    expect(describeOrphanOffer(free, false, PRICING)).toBe("1 orphan snapshot, 3.0 GB, inside the free GB, so nothing saved yet; wsp doctor --yes deletes them");
    const paid = { ...free, savesUsdPerMonth: 20 * 0.05 };
    expect(describeOrphanOffer(paid, true, PRICING)).toBe("deleting 1 orphan snapshot, 3.0 GB, saving about $1.00/month");
  });

  it("on Boat, which bills named snapshots by count, the offer names the count and never the GB the listing carries", () => {
    // Boat lists each named snapshot's restored content size: 62 B for a 7.3 GB image.
    const free = { snapshots: [{ id: "wsp-h1-default-v9", name: "wsp-h1-default-v9", sizeBytes: 62 }], templates: [], freedBytes: 62, savesUsdPerMonth: 0, others: { snapshots: [], templates: [] } };
    expect(describeOrphanOffer(free, false, BOX_PRICING.snapshotStorage)).toBe("1 orphan snapshot, inside the 10 kept free, so nothing saved yet; wsp doctor --yes deletes them");
    const past = { ...free, savesUsdPerMonth: 1.7 };
    expect(describeOrphanOffer(past, true, BOX_PRICING.snapshotStorage)).toBe("deleting 1 orphan snapshot, saving about $1.70/month");
  });

  it("a row the provider listed without a name reads as its id alone", () => {
    const plan = { snapshots: [], templates: [], freedBytes: 0, savesUsdPerMonth: 0, others: { snapshots: [{ id: "snap_c", sizeBytes: 3 * GB }], templates: [] } };
    expect(describeOrphans(plan)).toEqual(["  left alone, no mark of this host: snapshot snap_c, 3.0 GB"]);
  });
});

describe("the retention line", () => {
  it("names what goes, what it holds, what it saves and what stays, in one line", () => {
    const plan = { keep: [version(4, 3), version(3, 2)], drop: [version(1), version(2, 1)], abandoned: [], guarded: [], parentAssumed: false, namedFreed: 0, freedBytes: 15.8 * GB, savesUsdPerMonth: 15.8 * 0.05, pricing: PRICING };
    expect(describeRetention(plan)).toBe("Delete image v1 and v2, 15.8 GB, saving about $0.79/month from 2026-10-01? v4 and v3 stay.");
  });

  it("names the abandoned branches in the same line, after what stays", () => {
    const plan = { keep: [version(5, 2), version(2, 1)], drop: [version(1), version(3, 2), version(4, 3)], abandoned: [version(3, 2), version(4, 3)], guarded: [], parentAssumed: false, namedFreed: 0, freedBytes: 24.5 * GB, savesUsdPerMonth: 24.5 * 0.05, pricing: PRICING };
    expect(describeRetention(plan)).toBe("Delete image v1, v3 and v4, 24.5 GB, saving about $1.23/month from 2026-10-01? v5 and v2 stay. v3 and v4 are abandoned branches: v5 was not built through them.");
    const one = { ...plan, drop: [version(4, 3)], abandoned: [version(4, 3)], freedBytes: 8.5 * GB, savesUsdPerMonth: 8.5 * 0.05 };
    expect(describeRetention(one)).toBe("Delete image v4, 8.5 GB, saving about $0.43/month from 2026-10-01? v5 and v2 stay. v4 is an abandoned branch: v5 was not built through it.");
  });

  it("says when the kept parent was taken by version order, since the head was sealed before parents were recorded", () => {
    const plan = { keep: [version(4), version(3)], drop: [version(1), version(2)], abandoned: [], guarded: [], parentAssumed: true, namedFreed: 0, freedBytes: 15.8 * GB, savesUsdPerMonth: 15.8 * 0.05, pricing: PRICING };
    expect(describeRetention(plan)).toBe("Delete image v1 and v2, 15.8 GB, saving about $0.79/month from 2026-10-01? v4 and v3 stay. v3 is taken as the parent by version order: v4 was sealed before parents were recorded.");
  });

  it("a drop inside the free GB says nothing is saved yet; three versions read as a list", () => {
    const plan = { keep: [version(5, 4), version(4, 3)], drop: [version(1), version(2, 1), version(3, 2)], abandoned: [], guarded: [], parentAssumed: false, namedFreed: 0, freedBytes: 3 * GB, savesUsdPerMonth: 0, pricing: PRICING };
    expect(describeRetention(plan)).toBe("Delete image v1, v2 and v3, 3.0 GB, inside the free 10 GB, so nothing saved yet? v5 and v4 stay.");
  });
});

describe("the retention line on Boat, which bills named snapshots by count", () => {
  const manifest = { head: 5, versions: [version(1), version(2, 1), version(3, 2), version(4, 3), version(5, 4)] };
  const named = (n: number) => Array.from({ length: n }, (_, i) => ({ id: i < 5 ? `snap_golden-v${i + 1}` : `wsp-other-${i}`, name: i < 5 ? `golden-v${i + 1}` : `wsp-other-${i}`, sizeBytes: 0 }));

  it("names how many named snapshots go and what that saves, with no GB and no billed-from", () => {
    const plan = retentionPlan(manifest, named(14), () => [], BOX_PRICING.snapshotStorage);
    expect(describeRetention(plan)).toBe("Delete image v1, v2 and v3, 3 named snapshots, saving about $5.10/month? v5 and v4 stay.");
  });

  it("a drop inside the 10 kept free says nothing is saved yet", () => {
    const plan = retentionPlan(manifest, named(8), () => [], BOX_PRICING.snapshotStorage);
    expect(describeRetention(plan)).toBe("Delete image v1, v2 and v3, 3 named snapshots, inside the 10 kept free, so nothing saved yet? v5 and v4 stay.");
  });
});

describe("the storage line after the offer deletes at a place", () => {
  it("is the start line with its place named, so Boat's line reads one way at start and after the deletes", () => {
    const named = (n: number) => Array.from({ length: n }, (_, i) => ({ id: `acme-${i}`, name: `acme-${i}`, sizeBytes: GB }));
    const read = { hostId: "h1", recorded: new Set<string>(), now: 0 };
    const past = snapshotStorage(named(12), BOX_PRICING.snapshotStorage, read);
    expect(describeStorage(past, "Boat")).toBe(describeStorage(past).replace(/^storage:/, "storage at Boat:"));
    expect(describeStorage(past, "Boat")).toBe("storage at Boat: 12 named snapshots, 12.0 GB; about $3.40/month for the 2 past the 10 free. 12 not this host's, 12.0 GB");
    expect(describeStorage(snapshotStorage(named(8), BOX_PRICING.snapshotStorage, read), "Boat")).toBe("storage at Boat: 8 named snapshots, 8.0 GB; inside the 10 free, nothing to pay. 8 not this host's, 8.0 GB");
    expect(describeStorage({ count: 1, totalBytes: 7.8 * GB, ...PRICING, monthlyUsd: 0, ...allRecorded(1, 7.8 * GB) }, "Solari")).toBe("storage at Solari: 1 snapshot, 7.8 GB; inside the free 10 GB, nothing to pay from 2026-10-01");
  });
});

describe("the retention offer over the stub backend", () => {
  /** Four sealed versions, sized as the listing reports them, and the streams wsp init would ask on. */
  async function golden(): Promise<{ rt: Runtime; backend: StubBackend; store: Store; input: PassThrough; output: PassThrough; text: () => string; press: (k: string) => Promise<void> }> {
    const store = memoryStore();
    const backend = stubBackend();
    await store.put("goldens", copyKey("default", "default"), { head: 4, versions: [1, 2, 3, 4].map(n => version(n, n > 1 ? n - 1 : undefined)) });
    for (const n of [1, 2, 3, 4]) backend.snapshots.push({ id: `snap_golden-v${n}`, sizeBytes: (7 + n) * GB });
    const rt = createRuntime({ backend, store, adapters: {} });
    const input = new PassThrough();
    const output = new PassThrough();
    const chunks: string[] = [];
    output.on("data", (c: Buffer) => chunks.push(c.toString()));
    const press = async (k: string): Promise<void> => {
      input.write(k);
      await new Promise(r => setTimeout(r, 5));
    };
    return { rt, backend, store, input, output, text: () => stripVTControlCharacters(chunks.join("")), press };
  }
  const streams = (g: { input: PassThrough; output: PassThrough }): { input: PassThrough; output: PassThrough } => ({ input: g.input, output: g.output });

  it("under --yes the line is printed, taken as yes, the older ancestors are deleted, and the storage line follows", async () => {
    const g = await golden();
    await retentionOffer({ rt: g.rt, interactive: false, takenAs: "--yes", ...streams(g) });
    const out = g.text();
    expect(out).toContain("Delete image v1 and v2, 17.0 GB, saving about $0.85/month from 2026-10-01? v4 and v3 stay. Taken as yes (--yes).");
    expect(out).toContain("Deleted image v1 and v2.");
    expect(out).toContain("storage at default: 2 snapshots, 21.0 GB; about $0.55/month above the free 10 GB from 2026-10-01");
    expect(g.backend.snapshots.map(r => r.id)).toEqual(["snap_golden-v3", "snap_golden-v4"]);
    expect((await g.rt.golden.get())!.versions.map(v => v.version)).toEqual([3, 4]);
  });

  it("a version a running or paused workspace was forked from is named and never offered", async () => {
    const g = await golden();
    const alpha = await createOn(g.rt, { golden: "snap_golden-v1", name: "alpha" });
    await g.rt.workspaces.nap(alpha.id);
    await retentionOffer({ rt: g.rt, interactive: false, takenAs: "no terminal", ...streams(g) });
    const out = g.text();
    expect(out).toContain("Image v1 stays: workspace alpha was forked from it.");
    expect(out).toContain("Delete image v2, 9.0 GB, saving about $0.45/month from 2026-10-01? v4 and v3 stay. Taken as yes (no terminal).");
    expect(g.backend.snapshots.map(r => r.id)).toEqual(["snap_golden-v1", "snap_golden-v3", "snap_golden-v4"]);
    expect((await g.rt.golden.get())!.versions.map(v => v.version)).toEqual([1, 3, 4]);
  });

  it("right after a rollback nothing is offered; once an update seals past the branch rolled back from it is offered as abandoned, and a workspace forked from it keeps its version", async () => {
    const g = await golden();
    await g.rt.golden.rollback(2);
    await retentionOffer({ rt: g.rt, interactive: false, takenAs: "--yes", ...streams(g) });
    expect(g.text()).toBe("");
    // An update seals v5 from v2; the manifest is written as the runtime records it.
    const sealed = async (h: { rt: Runtime; backend: StubBackend; store: Store }): Promise<void> => {
      const manifest = (await h.rt.golden.get())!;
      await h.store.put("goldens", copyKey("default", "default"), { head: 5, versions: [...manifest.versions, version(5, 2)] });
      h.backend.snapshots.push({ id: "snap_golden-v5", sizeBytes: 12 * GB });
    };
    await sealed(g);
    await retentionOffer({ rt: g.rt, interactive: false, takenAs: "--yes", ...streams(g) });
    expect(g.text()).toContain("Delete image v1, v3 and v4, 29.0 GB, saving about $1.45/month from 2026-10-01? v5 and v2 stay. v3 and v4 are abandoned branches: v5 was not built through them. Taken as yes (--yes).");
    expect(g.text()).toContain("Deleted image v1, v3 and v4.");
    expect(g.backend.snapshots.map(r => r.id)).toEqual(["snap_golden-v2", "snap_golden-v5"]);

    const h = await golden();
    await h.rt.golden.rollback(2);
    await sealed(h);
    await createOn(h.rt, { golden: "snap_golden-v4", name: "alpha" });
    await retentionOffer({ rt: h.rt, interactive: false, takenAs: "--yes", ...streams(h) });
    expect(h.text()).toContain("Image v4 stays: workspace alpha was forked from it.");
    expect(h.text()).toContain("Delete image v1 and v3, 18.0 GB, saving about $0.90/month from 2026-10-01? v5 and v2 stay. v3 is an abandoned branch: v5 was not built through it. Taken as yes (--yes).");
    expect(h.backend.snapshots.map(r => r.id)).toEqual(["snap_golden-v2", "snap_golden-v4", "snap_golden-v5"]);
    expect((await h.rt.golden.get())!.versions.map(v => v.version)).toEqual([2, 4, 5]);
  });

  it("interactive: No keeps every version; y deletes", async () => {
    const g = await golden();
    const asked = retentionOffer({ rt: g.rt, interactive: true, takenAs: "no terminal", ...streams(g) });
    await new Promise(r => setTimeout(r, 20));
    // The prompt wraps the question to the terminal's width, so the line is read back unwrapped.
    const asOneLine = (): string => g.text().replace(/\n┃\s+/g, " ");
    expect(asOneLine()).toContain("Delete image v1 and v2, 17.0 GB, saving about $0.85/month from 2026-10-01? v4 and v3 stay.");
    expect(g.text()).toContain("No keeps every version.");
    await g.press("n");
    await asked;
    expect(g.text()).toContain("Every image version is kept.");
    expect(g.backend.snapshots).toHaveLength(4);

    const again = retentionOffer({ rt: g.rt, interactive: true, takenAs: "no terminal", ...streams(g) });
    await new Promise(r => setTimeout(r, 20));
    await g.press("y");
    await again;
    expect(g.text()).toContain("Deleted image v1 and v2.");
    expect(g.backend.snapshots.map(r => r.id)).toEqual(["snap_golden-v3", "snap_golden-v4"]);
  });

  it("with two versions, or a backend that cannot list snapshots, nothing is said", async () => {
    const g = await golden();
    await g.rt.golden.prune();
    await retentionOffer({ rt: g.rt, interactive: false, takenAs: "--yes", ...streams(g) });
    expect(g.text()).toBe("");
    const bare = stubBackend();
    const { listSnapshots: _l, ...rest } = bare;
    const rt = createRuntime({ backend: { ...rest, capabilities: { ...bare.capabilities, snapshotListing: false } }, store: memoryStore(), adapters: {} });
    await retentionOffer({ rt, interactive: false, takenAs: "--yes", ...streams(g) });
    expect(g.text()).toBe("");
  });

  it("an image sealed at another place has its copies at the place this host forks on offered and deleted there, at that place's rate, and its own versions at its own", async () => {
    const store = memoryStore();
    const backend = stubBackend();
    const boat = Object.assign(stubBackend(), { pricing: BOX_PRICING });
    await store.put("goldens", copyKey("default", "default"), { head: 4, versions: [1, 2, 3, 4].map(n => ({ ...version(n, n > 1 ? n - 1 : undefined), snapshotId: `snap_copy-v${n}`, ...(n > 1 ? { parentSnapshotId: `snap_copy-v${n - 1}` } : {}) })) });
    for (const n of [1, 2, 3, 4]) backend.snapshots.push({ id: `snap_copy-v${n}`, sizeBytes: (7 + n) * GB });
    await store.put("goldens", copyKey("boat", "default"), { head: 3, versions: [1, 2, 3].map(n => version(n, n > 1 ? n - 1 : undefined)) });
    for (let i = 1; i <= 10; i += 1) boat.snapshots.push({ id: `snap_acme-${i}`, name: `acme-${i}`, sizeBytes: GB });
    for (const n of [1, 2, 3]) boat.snapshots.push({ id: `snap_golden-v${n}`, name: `golden-v${n}`, sizeBytes: GB });
    await store.put("images", "default", { name: "default", version: 3, hash: "a".repeat(64), recipeHash: "h1", recipe: {}, pins: [], logins: [], sealedAt: "2026-09-01T00:00:00.000Z", sealedFrom: "h1", place: "boat" });
    const rt = createRuntime({ backend, store, adapters: {}, places: { wired: "default", backend: p => ({ default: backend, boat })[p], list: () => ["default", "boat"] } });
    const g = { input: new PassThrough(), output: new PassThrough() };
    const chunks: string[] = [];
    g.output.on("data", (c: Buffer) => chunks.push(c.toString()));
    try {
      await retentionOffer({ rt, interactive: false, takenAs: "--yes", ...g });
      const out = stripVTControlCharacters(chunks.join(""));
      expect(out).toContain("Delete image v1, 1 named snapshot, saving about $1.70/month? v3 and v2 stay. Taken as yes (--yes).");
      expect(out).toContain("Delete v1 and v2 of the copy of your image at default, 17.0 GB, saving about $0.85/month from 2026-10-01? v4 and v3 stay. Taken as yes (--yes).");
      expect(out).toContain("Deleted v1 and v2 of the copy of your image at default.");
      expect(boat.snapshots.filter(r => r.id.startsWith("snap_golden")).map(r => r.id)).toEqual(["snap_golden-v2", "snap_golden-v3"]);
      expect(backend.snapshots.map(r => r.id)).toEqual(["snap_copy-v3", "snap_copy-v4"]);
    } finally {
      await rt.close();
    }
  });

  it("an image sealed where this host forks has its copies at another provider offered and deleted there, at that provider's rate", async () => {
    const store = memoryStore();
    const backend = stubBackend();
    const boat = Object.assign(stubBackend(), { pricing: BOX_PRICING });
    await store.put("goldens", copyKey("default", "default"), { head: 2, versions: [1, 2].map(n => version(n, n > 1 ? n - 1 : undefined)) });
    for (const n of [1, 2]) backend.snapshots.push({ id: `snap_golden-v${n}`, sizeBytes: (7 + n) * GB });
    await store.put("goldens", copyKey("boat", "default"), { head: 4, versions: [1, 2, 3, 4].map(n => ({ ...version(n, n > 1 ? n - 1 : undefined), snapshotId: `snap_copy-v${n}`, ...(n > 1 ? { parentSnapshotId: `snap_copy-v${n - 1}` } : {}) })) });
    for (let i = 1; i <= 10; i += 1) boat.snapshots.push({ id: `snap_acme-${i}`, name: `acme-${i}`, sizeBytes: GB });
    for (const n of [1, 2, 3, 4]) boat.snapshots.push({ id: `snap_copy-v${n}`, name: `copy-v${n}`, sizeBytes: GB });
    await store.put("images", "default", { name: "default", version: 2, hash: "a".repeat(64), recipeHash: "h1", recipe: {}, pins: [], logins: [], sealedAt: "2026-09-01T00:00:00.000Z", sealedFrom: "h1", place: "default" });
    const rt = createRuntime({ backend, store, adapters: {}, places: { wired: "default", backend: p => ({ default: backend, boat })[p], list: () => ["default", "boat"] } });
    const g = { input: new PassThrough(), output: new PassThrough() };
    const chunks: string[] = [];
    g.output.on("data", (c: Buffer) => chunks.push(c.toString()));
    try {
      await retentionOffer({ rt, interactive: false, takenAs: "--yes", ...g });
      const out = stripVTControlCharacters(chunks.join(""));
      expect(out).toContain("Delete v1 and v2 of the copy of your image at boat, 2 named snapshots, saving about $3.40/month? v4 and v3 stay. Taken as yes (--yes).");
      expect(out).toContain("Deleted v1 and v2 of the copy of your image at boat.");
      expect(boat.snapshots.filter(r => r.id.startsWith("snap_copy")).map(r => r.id)).toEqual(["snap_copy-v3", "snap_copy-v4"]);
      expect(backend.snapshots.map(r => r.id)).toEqual(["snap_golden-v1", "snap_golden-v2"]);
    } finally {
      await rt.close();
    }
  });

  it("a copy's guarded and refused versions are named as the copy's, with the version written as it is", async () => {
    const store = memoryStore();
    const backend = stubBackend();
    const boat = Object.assign(stubBackend(), { pricing: BOX_PRICING });
    await store.put("goldens", copyKey("default", "default"), { head: 2, versions: [1, 2].map(n => version(n, n > 1 ? n - 1 : undefined)) });
    await store.put("goldens", copyKey("boat", "default"), { head: 5, versions: [1, 2, 3, 4, 5].map(n => ({ ...version(n, n > 1 ? n - 1 : undefined), snapshotId: `snap_copy-v${n}`, ...(n > 1 ? { parentSnapshotId: `snap_copy-v${n - 1}` } : {}) })) });
    for (const n of [1, 2, 3, 4, 5]) boat.snapshots.push({ id: `snap_copy-v${n}`, name: `copy-v${n}`, sizeBytes: GB });
    const deleteSnapshot = boat.deleteSnapshot.bind(boat);
    boat.deleteSnapshot = async (id: string) => {
      if (id === "snap_copy-v3") throw new Error("busy");
      await deleteSnapshot(id);
    };
    const rt = createRuntime({ backend, store, adapters: {}, places: { wired: "default", backend: p => ({ default: backend, boat })[p], list: () => ["default", "boat"] } });
    const g = { input: new PassThrough(), output: new PassThrough() };
    const chunks: string[] = [];
    g.output.on("data", (c: Buffer) => chunks.push(c.toString()));
    try {
      await createOn(rt, { golden: "snap_copy-v1", name: "alpha" });
      await store.put("images", "default", { name: "default", version: 2, hash: "a".repeat(64), recipeHash: "h1", recipe: {}, pins: [], logins: [], sealedAt: "2026-09-01T00:00:00.000Z", sealedFrom: "h1", place: "default" });
      await retentionOffer({ rt, interactive: false, takenAs: "--yes", ...g });
      const out = stripVTControlCharacters(chunks.join(""));
      expect(out).toContain("The copy of your image at boat keeps v1: workspace alpha was forked from it.");
      expect(out).toContain("The copy of your image at boat keeps v3, which was not deleted (busy); it stays in the lineage.");
      expect(out).toContain("Deleted v2 of the copy of your image at boat.");
      expect(boat.snapshots.map(r => r.id)).toEqual(["snap_copy-v1", "snap_copy-v3", "snap_copy-v4", "snap_copy-v5"]);
    } finally {
      await rt.close();
    }
  });

  it("a place that refuses its listing after deletes at another is named, and only its own versions are said to stay", async () => {
    const store = memoryStore();
    const backend = stubBackend();
    const boat = Object.assign(stubBackend(), { pricing: BOX_PRICING });
    await store.put("goldens", copyKey("default", "default"), { head: 4, versions: [1, 2, 3, 4].map(n => version(n, n > 1 ? n - 1 : undefined)) });
    for (const n of [1, 2, 3, 4]) backend.snapshots.push({ id: `snap_golden-v${n}`, sizeBytes: (7 + n) * GB });
    await store.put("goldens", copyKey("boat", "default"), { head: 3, versions: [1, 2, 3].map(n => ({ ...version(n, n > 1 ? n - 1 : undefined), snapshotId: `snap_copy-v${n}`, ...(n > 1 ? { parentSnapshotId: `snap_copy-v${n - 1}` } : {}) })) });
    boat.listSnapshots = async () => {
      throw new Error("unauthorized");
    };
    const rt = createRuntime({ backend, store, adapters: {}, places: { wired: "default", backend: p => ({ default: backend, boat })[p], list: () => ["default", "boat"] } });
    const g = { input: new PassThrough(), output: new PassThrough() };
    const chunks: string[] = [];
    g.output.on("data", (c: Buffer) => chunks.push(c.toString()));
    try {
      await retentionOffer({ rt, interactive: false, takenAs: "--yes", ...g });
      const out = stripVTControlCharacters(chunks.join(""));
      expect(out).toContain("Deleted image v1 and v2.");
      expect(out).toContain("Snapshot storage at boat was not read (unauthorized); every version there is kept.");
      expect(out).not.toContain("every image version is kept");
    } finally {
      await rt.close();
    }
  });

  it("a listing the provider refuses keeps every version and says so", async () => {
    const g = await golden();
    g.backend.listSnapshots = async () => {
      throw new Error("502 Bad Gateway");
    };
    await retentionOffer({ rt: g.rt, interactive: false, takenAs: "--yes", ...streams(g) });
    expect(g.text()).toContain("Snapshot storage at default was not read (502 Bad Gateway); every version there is kept.");
    expect((await g.rt.golden.get())!.versions).toHaveLength(4);
  });
});
