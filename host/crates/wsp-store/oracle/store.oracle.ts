// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the wsp-store tests pin, for runtime/src/{store,sqlite-store,sqlite-transcripts,transcript-reader}.ts
// at 65f6b89c7: one JSON file per unit under tests/fixtures/store/. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-store/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it } from "vitest";

const R = "../../../../packages/runtime/src";
const store = await import(`${R}/store.ts`);
const sqlite = await import(`${R}/sqlite-store.ts`);
const rowsOf = await import(`${R}/sqlite-transcripts.ts`);
const reader = await import(`${R}/transcript-reader.ts`);

const out = fileURLToPath(new URL("../tests/fixtures/store/", import.meta.url));
const write = (name: string, value: unknown): void => {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, `${name}.json`), `${JSON.stringify(value, null, 1)}\n`);
};
const tried = async (f: () => unknown): Promise<unknown> => {
  try {
    const v = await f();
    return { ok: v === undefined ? "<undefined>" : v };
  } catch (e) {
    return { err: e instanceof Error ? e.message : String(e) };
  }
};
const WRITER = { wsp: "test", daemon: 1, bin: "/usr/local/bin/wsp" };

// Events in the schema's key order (type, scope, then the variant's own), so the Rust struct writes the same text.
const delta = (threadId: string | undefined, pos: number | undefined, text: string, extra: Record<string, unknown> = {}, kind = "text") => ({
  type: "session.delta",
  workspaceId: "w",
  sessionId: "s",
  ...(threadId !== undefined ? { threadId } : {}),
  ...(pos !== undefined ? { pos } : {}),
  kind,
  text,
  ...extra,
});

it("store", async () => {
  const m = store.memoryStore();
  for (const id of ["b", "2", "a", "1", "01", "4294967295", "4294967294"]) await m.put("c", id, { id });
  const memKeys = await m.keys("c");
  const memList = await m.list("c");
  await m.putBlob("v", "x", Buffer.from("one"));
  await m.putBlob("v", "y", Buffer.from("two!"));
  await m.putBlob("v", "x", Buffer.from("three"));
  const marks = { x: await m.statBlob("v", "x"), y: await m.statBlob("v", "y") };
  await m.deleteBlob("v", "x");
  const gone = (await m.statBlob("v", "x")) ?? "<undefined>";
  await m.putBlob("v", "x", Buffer.alloc(0));
  const empty = { mark: await m.statBlob("v", "x"), bytes: [...((await m.getBlob("v", "x")) as Buffer)] };
  await m.put("c", "b", { again: true });
  await m.delete("c", "a");
  await m.put("c", "a", { back: true });
  const after = await m.keys("c");
  const shape = { shape: 2, wsp: "0.3.0", daemon: 160, bin: "/Applications/wsp.app/Contents/Resources/bin.js", at: "2026-09-18T15:15:00.000Z" };
  write("store", {
    memKeys,
    memList,
    marks,
    gone,
    empty,
    afterReputDeleteAndPut: after,
    lines: {
      newer: store.stateWrittenByNewerLine("/h/state.db", shape),
      olderNone: store.stateWrittenByOlderLine("/h/state.db", undefined),
      older: store.stateWrittenByOlderLine("/h/state.db", shape),
      unreadable: store.stateUnreadableLine("/h/state.db", "file is not a database"),
      shapeUnreadable: [3, null, { shape: 2 }, "not json {"].map(d => store.stateShapeUnreadableLine("/h/state.db", d)),
    },
  });
});

it("sqlite_store", async () => {
  const h = mkdtempSync(join(tmpdir(), "wsp-oracle-"));
  try {
    const s = sqlite.sqliteStore(join(h, "state.json"), WRITER);
    await s.put("r", "nums", { z: 1, a: 2, "2": 0, n: 1.5, big: 1e21, small: 1e-7, neg0: -0, whole: 10.0, nested: { "10": 1, b: [1, "x", null] } });
    await s.put("r", "null", null);
    await s.put("r", "text", "héllo   \u0001 / \"q\"");
    await s.put("r", "gone", { a: 1 });
    await s.put("r", "gone", undefined);
    for (const id of ["a", "b", "c"]) await s.put("o", id, { id });
    await s.put("o", "a", { id: "a", again: true });
    await s.delete("o", "b");
    await s.put("o", "b", { id: "b", back: true });
    await s.delete("o", "missing");
    await s.putBlob("v", "e", Buffer.alloc(0));
    const { DatabaseSync } = process.getBuiltinModule("node:sqlite") as typeof import("node:sqlite");
    const d = new DatabaseSync(join(h, "state.db"), { readOnly: true });
    const raw = d.prepare("select collection, id, json from collections order by rowid").all().map(r => ({ ...r }));
    const shapeRow = JSON.parse((d.prepare("select json from shape").get() as { json: string }).json);
    d.close();
    write("sqlite_store", {
      raw,
      shapeKeys: Object.keys(shapeRow),
      getNull: await tried(() => s.get("r", "null")),
      getGone: await tried(() => s.get("r", "gone")),
      keys: await s.keys("o"),
      list: await s.list("o"),
      unknown: { list: await s.list("nothing"), keys: await s.keys("nothing") },
      emptyBlob: { bytes: [...((await s.getBlob("v", "e")) as Buffer)], mark: (await s.statBlob("v", "e"))?.bytes },
    });
  } finally {
    rmSync(h, { recursive: true, force: true });
  }
});

it("sqlite_transcripts", async () => {
  const { DatabaseSync } = process.getBuiltinModule("node:sqlite") as typeof import("node:sqlite");
  const fresh = () => {
    const d = new DatabaseSync(":memory:");
    sqlite.migrate(d, ":memory:");
    return { d, rows: rowsOf.transcriptRows(() => d) };
  };
  const { d, rows } = fresh();
  const events = [
    delta("a", 1, "line 1"),
    delta("b", 2, "😀 é"),
    delta(undefined, 3, "no thread"),
    delta("", 4, "empty thread"),
    { type: "session.start", workspaceId: "w", sessionId: "s", threadId: "a", pos: 5, prompt: "go" },
    { type: "session.steer", workspaceId: "w", sessionId: "s", threadId: "a", pos: 6, prompt: "more" },
    delta("a", 7, "thinking", {}, "thinking"),
    { type: "session.earlier", workspaceId: "w", sessionId: "s", threadId: "a", pos: 8, who: "person", text: "before" },
    delta("a", 9, "result", {}, "tool_result"),
    { type: "session.end", workspaceId: "w", sessionId: "s", threadId: "a", pos: 10, exitCode: 0, sawResult: true },
  ];
  rows.append("w", events);
  const raw = d.prepare("select workspace, pos, thread, type, size, json from events order by pos").all().map(r => ({ ...r }));
  const noPos = await tried(() => rows.append("w", [delta("a", undefined, "no pos"), delta("a", 11, "after")]));
  const twice = await tried(() => rows.append("w", [delta("a", 12, "new"), delta("a", 1, "again")]));
  let caught = "";
  rows.atomically(() => {
    rows.putIndex("w", "outer");
    try {
      rows.atomically(() => {
        rows.append("w", [delta("a", 20, "inner")]);
        throw new Error("inner stop");
      });
    } catch (e) {
      caught = (e as Error).message;
    }
  });
  write("sqlite_transcripts", {
    raw,
    all: rows.all("w"),
    thread: rows.thread("w", "a"),
    threadEmpty: rows.thread("w", ""),
    wordEvents: rows.wordEvents("w", "a").map((e: { pos: number }) => e.pos),
    newest: [...rows.newest("w", "a")].map((e: { pos: number }) => e.pos),
    newestBelow7: [...rows.newest("w", "a", 7)].map((e: { pos: number }) => e.pos),
    newestBelow0: [...rows.newest("w", "a", 0)].map((e: { pos: number }) => e.pos),
    count: { a: rows.count("w", "a"), empty: rows.count("w", ""), none: rows.count("w", "zzz") },
    total: { all: rows.total("w"), a: rows.total("w", "a"), empty: rows.total("w", ""), unknownWs: rows.total("nope") },
    newestPos: { w: rows.newestPos("w"), unknownWs: rows.newestPos("nope") },
    sizes: rows.sizes("w"),
    sizesEmpty: rows.sizes("w", ""),
    at: rows.at("w", [4, 4, 99, 1]).map((e: { pos: number }) => e.pos),
    noPos,
    afterNoPos: rows.all("w").events.map((e: { pos: number }) => e.pos),
    twice,
    afterTwice: rows.all("w").events.map((e: { pos: number }) => e.pos),
    nested: { caught, index: rows.index("w"), pos: rows.all("w").events.map((e: { pos: number }) => e.pos) },
  });
});

it("transcript_reader", async () => {
  const events = [
    delta("a", 1, "x".repeat(10)),
    delta("b", 2, "other"),
    delta("a", 3, "é".repeat(10)),
    delta(undefined, 4, "none"),
    delta("a", undefined, "unnumbered"),
    delta("a", 6, "y".repeat(30)),
  ];
  const sizeOf = (e: unknown) => Buffer.byteLength(JSON.stringify(e)) + 1;
  const reads: Record<string, unknown> = {};
  const cases: Record<string, object> = {
    all: { limit: Infinity, bytes: 1e9 },
    limit2: { limit: 2, bytes: 1e9 },
    limit0: { limit: 0, bytes: 1e9 },
    before6: { limit: Infinity, bytes: 1e9, before: 6 },
    before1: { limit: Infinity, bytes: 1e9, before: 1 },
    before0: { limit: Infinity, bytes: 1e9, before: 0 },
    bytes0: { limit: Infinity, bytes: 0 },
    bytes0strict: { limit: Infinity, bytes: 0, strict: true },
    bytesExactTwo: { limit: Infinity, bytes: sizeOf(events[5]) + sizeOf(events[4]) },
    bytesOneShort: { limit: Infinity, bytes: sizeOf(events[5]) + sizeOf(events[4]) - 1 },
    strictExactOne: { limit: Infinity, bytes: sizeOf(events[5]), strict: true },
    strictOneShort: { limit: Infinity, bytes: sizeOf(events[5]) - 1, strict: true },
  };
  for (const [name, o] of Object.entries(cases)) {
    const r = reader.readThread(events, "a", o);
    reads[name] = { pos: r.events.map((e: { pos?: number; text: string }) => e.pos ?? e.text), total: r.total };
  }
  reads["threadEmpty"] = reader.readThread(events, "", { limit: Infinity, bytes: 1e9 });
  reads["sizes"] = events.map(sizeOf);
  const long = delta("a", 1, "z".repeat(2049), {}, "tool_result");
  const exact = delta("a", 1, "z".repeat(2048), {}, "tool_result");
  const emoji = delta("a", 1, `${"z".repeat(2047)}😀tail`, {}, "tool_result");
  const lines = Array.from({ length: 40 }, (_, i) => `+line ${i} ${"p".repeat(90)}`);
  const patched = delta("a", 1, "short", { patch: [{ path: "a.ts", hunks: [{ oldStart: 1, oldLines: 0, newStart: 1, newLines: 40, lines }] }, { path: "b.ts", hunks: [] }] }, "tool_result");
  const textLong = delta("a", 1, "z".repeat(5000));
  const shaped = (e: object) => {
    const s = reader.headShape(e);
    return { keys: Object.keys(s), textLength: s.text?.length, cut: s.cut, patchCut: s.patchCut, patch: s.patch, tail: [...(s.text?.slice(-3) ?? "")].map((c: string) => c.charCodeAt(0)) };
  };
  write("transcript_reader", {
    reads,
    head: { long: shaped(long), exact: shaped(exact), emoji: shaped(emoji), patched: shaped(patched), textLong: shaped(textLong) },
    eventJson: [delta("a", 1, "😀 é \u0001"), { type: "session.end", workspaceId: "w", sessionId: "s", pos: 2, exitCode: null, sawResult: false, at: 1792923718163.5 }].map(e => reader.eventJson(e)),
  });
});
