// SPDX-License-Identifier: AGPL-3.0-only
import { PassThrough } from "node:stream";
import { stripVTControlCharacters } from "node:util";
import { BOX_PRICING, SNAPSHOT_STORAGE, retentionPlan, snapshotStorage } from "@wsp/engine";
import { copyKey, createRuntime, memoryStore, type Runtime, type Store } from "@wsp/runtime";
import { describe, expect, it } from "vitest";
import { describeOrphanOffer, describeOrphans, describeStorage, noProviderStorageLine } from "../src/storage.js";
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

