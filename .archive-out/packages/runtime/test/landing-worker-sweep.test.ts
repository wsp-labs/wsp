// SPDX-License-Identifier: AGPL-3.0-only
// The machine a seeded add clones and installs in at a provider is claimed from the provider's first state until
// its stop, so a sweep of that place never kills it mid-install.
import { execFileSync } from "node:child_process";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { NoProviderBackend } from "@wsp/engine";
import type { SeedPlan } from "@wsp/protocol";
import { copyKey, createRuntime, type PlaceBackends } from "../src/runtime.js";
import { newPlaceKeyPair } from "../src/places.js";
import { memoryStore } from "../src/store.js";
import { fakeLocal, stubBackend } from "./stub-backend.js";
import { wiring } from "./place-join.js";
import { until } from "./until.js";

const VERSION = { version: 1, snapshotId: "snap_golden-v1", baseTemplate: "base", setupSha: "s1", createdAt: "2026-09-17T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 }, noSecrets: true };
const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});
const made = (prefix: string): string => {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), prefix)));
  roots.push(dir);
  return dir;
};
const menu = (folder: string): SeedPlan => ({ source: folder, remote: "https://github.com/acme/lab.git", branch: "main", defaultBranch: "main", unpushed: null, uncommitted: 0, memory: null, files: [], remembered: false });

describe("the machine an add seeds a project on at a provider", () => {
  for (const forksOnNone of [false, true]) {
    it(`is not killed by a sweep while it installs${forksOnNone ? ", on a host that holds Boat as a place" : ""}`, async () => {
      const folder = made("wsp-lab-");
      execFileSync("git", ["init", "-q", folder]);
      const boat = stubBackend();
      const store = memoryStore();
      const place = forksOnNone ? "box" : "default";
      await store.put("goldens", copyKey(place, "default"), { head: 1, versions: [VERSION] });
      if (forksOnNone) await store.put("images", "default", { name: "default", version: 1, hash: "a".repeat(64), recipeHash: "r", recipe: { version: 1, at: VERSION.createdAt, histories: [], rows: [] }, pins: [], logins: [], sealedAt: VERSION.createdAt, sealedFrom: "mac", noSecrets: true, place: "box" });
      const none = new NoProviderBackend();
      const host = forksOnNone ? { backend: none, places: { wired: "none", backend: (p: string) => (p === "box" ? boat : p === "none" ? none : undefined), list: () => ["box"] } satisfies PlaceBackends, placeLinks: wiring(newPlaceKeyPair()) } : { backend: boat };
      const rt = createRuntime({ ...host, store, adapters: {}, local: fakeLocal(made("wsp-home-")), seed: { plan: async f => menu(f), pack: async () => ({ tar: Buffer.from("seed"), files: 0, bytes: 1, commits: 0, left: [] }) } } as Parameters<typeof createRuntime>[0]);
      const answer = boat.execImpl;
      let release!: () => void;
      const installing = new Promise<void>(r => (release = r));
      let worker: { killed: boolean; spec: { labels: Record<string, string> } } | undefined;
      boat.execImpl = (async (m: Parameters<typeof answer>[0], cmd: string, stdin?: Uint8Array) => {
        if (worker === undefined && m.spec.labels?.["wsp-builder"] === "1") {
          worker = m as unknown as typeof worker;
          // The clone and install have run past the minute the engine grants a machine of this host's.
          worker!.spec.labels["createdAt"] = new Date(Date.now() - 2 * 60_000).toISOString();
          await installing;
        }
        return answer(m, cmd, stdin);
      }) as typeof answer;
      const adding = rt.projects.add({ source: folder, on: place, name: "lab-there", seed: { files: [], memory: false, commits: false } });
      await until(() => worker !== undefined);

      expect((await rt.reap()).reaped).toEqual([]);
      expect(worker!.killed).toBe(false);
      release();
      await adding;
      expect(worker!.killed).toBe(true);
      await rt.close();
    });
  }
});
