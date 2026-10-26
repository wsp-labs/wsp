// SPDX-License-Identifier: AGPL-3.0-only
// One step of tests/cross_host.rs, run by it: the TS host's own store (packages/runtime/src/sqlite-store.ts) opens the
// state.db in CROSS_HOST_DIR, dumps everything the fixture names, applies the op sets of CROSS_HOST_APPLY (comma
// separated, may be empty) and dumps again, into CROSS_HOST_DIR/ts-dump.json as {before, after}.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

const R = "../../../../packages/runtime/src";
const sqlite = await import(`${R}/sqlite-store.ts`);
const { sqliteBinding } = await import("../../../../packages/engine/src/index.ts");

type Op = Record<string, unknown[]>;
const fixture = JSON.parse(readFileSync(fileURLToPath(new URL("../tests/fixtures/cross_host/ops.json", import.meta.url)), "utf8")) as Record<string, any>;
const WRITER = { wsp: "ts", daemon: 1, bin: "/ts/bin.js" };

const open = (dir: string) => sqlite.sqliteStore(join(dir, "state.json"), WRITER);

const apply = async (dir: string, sets: string[]): Promise<void> => {
  const store = open(dir);
  const rows = store.transcripts!;
  for (const set of sets) {
    for (const op of fixture[set] as Op[]) {
      const [name, args] = Object.entries(op)[0]!;
      const a = args as any[];
      if (name === "put") await store.put(a[0], a[1], a[2]);
      else if (name === "delete") await store.delete(a[0], a[1]);
      else if (name === "putBlob") await store.putBlob(a[0], a[1], Buffer.from(a[2], "hex"));
      else if (name === "deleteBlob") await store.deleteBlob(a[0], a[1]);
      else if (name === "append") rows.append(a[0], a[1]);
      else if (name === "remove") rows.remove(a[0], a[1]);
      else if (name === "putIndex") rows.putIndex(a[0], a[1]);
      else if (name === "clear") rows.clear(a[0]);
      else throw new Error(`unknown op ${name}`);
    }
  }
};

const dump = async (dir: string): Promise<unknown> => {
  const store = open(dir);
  const rows = store.transcripts!;
  const d = fixture.dump;
  const collections: Record<string, unknown> = {};
  for (const c of d.collections as string[]) {
    const get: Record<string, unknown> = {};
    for (const id of d.ids[c] ?? []) get[id] = (await store.get(c, id)) ?? null;
    collections[c] = { keys: await store.keys(c), list: await store.list(c), get };
  }
  const blobs: Record<string, unknown> = {};
  for (const [c, id] of d.blobs as string[][]) {
    const bytes = await store.getBlob(c, id);
    blobs[`${c}/${id}`] = { hex: bytes === undefined ? null : Buffer.from(bytes).toString("hex").toUpperCase(), mark: (await store.statBlob(c, id)) ?? null };
  }
  const events: Record<string, unknown> = {};
  for (const ws of d.workspaces as string[]) {
    const threads: Record<string, unknown> = {};
    for (const t of d.threads as string[]) {
      threads[t] = {
        thread: rows.thread(ws, t),
        count: rows.count(ws, t),
        total: rows.total(ws, t),
        sizes: rows.sizes(ws, t),
        words: rows.wordEvents(ws, t),
        newest: [...rows.newest(ws, t, 5)],
      };
    }
    events[ws] = { all: rows.all(ws), newestPos: rows.newestPos(ws), total: rows.total(ws), sizes: rows.sizes(ws), threads, at: rows.at(ws, d.positions), index: rows.index(ws) ?? null };
  }
  const { DatabaseSync } = sqliteBinding();
  const db = new DatabaseSync(join(dir, "state.db"));
  const all = (sql: string): unknown[][] => (db.prepare(sql).all() as Record<string, unknown>[]).map(r => Object.values(r));
  const shape = JSON.parse((db.prepare("select json from shape where one = 1").get() as { json: string }).json) as Record<string, unknown>;
  const raw = {
    collections: all("select collection, id, json from collections order by rowid"),
    blobs: all("select collection, id, hex(bytes), at from blobs order by rowid"),
    events: all("select workspace, pos, thread, type, size, json from events order by workspace, pos"),
    index: all("select workspace, json from transcript_index order by workspace"),
    shape: { shape: shape.shape, keys: Object.keys(shape) },
  };
  db.close();
  return { collections, blobs, events, raw };
};

it("the TS host reads and writes the state.db of CROSS_HOST_DIR", async () => {
  const dir = process.env.CROSS_HOST_DIR;
  expect(dir, "CROSS_HOST_DIR names the folder tests/cross_host.rs made").toBeTruthy();
  const sets = (process.env.CROSS_HOST_APPLY ?? "").split(",").filter(s => s !== "");
  try {
    const before = process.env.CROSS_HOST_FRESH === "1" ? null : await dump(dir!);
    await apply(dir!, sets);
    writeFileSync(join(dir!, "ts-dump.json"), JSON.stringify({ before, after: await dump(dir!) }));
  } catch (e) {
    writeFileSync(join(dir!, "ts-dump.json"), JSON.stringify({ error: e instanceof Error ? e.message : String(e) }));
  }
});
