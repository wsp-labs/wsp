// SPDX-License-Identifier: AGPL-3.0-only
import { copyFileSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DAEMON_VERSION, ProjectView, STATE_SHAPE, WorkspaceView } from "@wsp/protocol";
import { describe, expect, it } from "vitest";
import { createRuntime } from "../src/runtime.js";
import { sqliteStore } from "../src/sqlite-store.js";
import { jsonFileStore, type Store } from "../src/store.js";
import { stubBackend } from "./stub-backend.js";

/** Written by wsp 0.3.6 itself: its createRuntime over its jsonFileStore, adding one project and one cloud workspace. */
const FIXTURE = join(import.meta.dirname, "fixtures", "state-0.3.6.json");
const WRITER = { wsp: "test", daemon: DAEMON_VERSION, bin: "/usr/local/bin/wsp" };
const written = JSON.parse(readFileSync(FIXTURE, "utf8")) as Record<string, Record<string, unknown>>;

const STORES: [string, (path: string) => Store][] = [
  ["the JSON store", path => jsonFileStore(path, WRITER)],
  ["the SQLite store a host opens", path => sqliteStore(path, WRITER)],
];

describe("a state file wsp 0.3.6 wrote", () => {
  it("is in the shape this build reads", () => {
    expect(written["$shape"]).toMatchObject({ shape: STATE_SHAPE, wsp: "0.3.6", daemon: 157 });
  });

  for (const [name, open] of STORES) {
    it(`loads unchanged into ${name}: every record as 0.3.6 wrote it, and no branch of record appears`, async () => {
      const dir = mkdtempSync(join(tmpdir(), "wsp-state-036-"));
      copyFileSync(FIXTURE, join(dir, "state.json"));
      const store = open(join(dir, "state.json"));
      const rt = createRuntime({ backend: stubBackend(), store, adapters: {} });
      try {
        const projects = await rt.projects.list();
        const workspaces = await rt.workspaces.list();
        expect(projects.map(p => ProjectView.parse(p))).toEqual(Object.values(written["projects"]!));
        expect(workspaces).toHaveLength(1);
        expect(WorkspaceView.parse(workspaces[0]).branchOfRecord).toBeUndefined();
        for (const [collection, records] of Object.entries(written)) {
          if (collection === "$shape") continue;
          for (const [id, record] of Object.entries(records)) expect(await store.get(collection, id), `${collection}/${id}`).toEqual(record);
        }
      } finally {
        await rt.close();
        rmSync(dir, { recursive: true, force: true });
      }
    });
  }
});
