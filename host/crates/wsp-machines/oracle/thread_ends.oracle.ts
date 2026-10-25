// SPDX-License-Identifier: AGPL-3.0-only
// machines/thread-ends.ts driven through scenes with a fake store, runtime and place door; every call, store row,
// answer and warning is written in order to tests/fixtures/oracle/thread_ends.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";
import { threadEnds } from "../../../../packages/runtime/src/machines/thread-ends.js";

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));

type Answer = { exitCode: number; stderr: string } | "throws" | "hangs";
interface World {
  linked: Set<string>;
  answers: Answer[];
  running: Set<string>;
  launching: Set<string>;
  holds: Set<string>;
}

const flush = async (): Promise<void> => {
  for (let i = 0; i < 20; i++) await new Promise<void>(r => setImmediate(r));
};

async function scene(name: string, seed: Record<string, unknown>, act: (t: ReturnType<typeof threadEnds>, w: World, fire: (e: unknown) => void, log: unknown[]) => Promise<void>, world: Partial<World> = {}) {
  const log: unknown[] = [];
  const rows = new Map<string, unknown>(Object.entries(seed));
  const w: World = { linked: new Set(), answers: [], running: new Set(), launching: new Set(), holds: new Set(), ...world };
  const store = {
    get: async (c: string, k: string) => (log.push(["store.get", c, k]), rows.get(k)),
    put: async (c: string, k: string, v: unknown) => (log.push(["store.put", c, k, v]), void rows.set(k, v)),
    delete: async (c: string, k: string) => (log.push(["store.delete", c, k]), void rows.delete(k)),
  };
  const ctx = {
    store,
    ready: async () => void log.push(["ready"]),
    runningOn: (t: string) => (log.push(["runningOn", t]), w.running.has(t) ? { id: "row" } : undefined),
    launchingOn: (t: string) => (log.push(["launchingOn", t]), w.launching.has(t) ? {} : undefined),
    stopHolds: (t: string) => (log.push(["stopHolds", t]), w.holds.has(t)),
  };
  const listeners: ((e: unknown) => void)[] = [];
  const door = {
    exec: async (placeId: string, script: string, o: unknown) => {
      log.push(["door.exec", placeId, script, o]);
      const a = w.answers.shift() ?? { exitCode: 0, stderr: "" };
      if (a === "throws") throw new Error("no answer");
      if (a === "hangs") return new Promise(() => {});
      return { exitCode: a.exitCode, stdout: "", stderr: a.stderr };
    },
    link: (placeId: string) => (log.push(["door.link", placeId]), w.linked.has(placeId) ? {} : undefined),
    nameOf: (placeId: string) => (placeId === "p1" ? "hetzner" : placeId),
    on: (l: (e: unknown) => void) => void listeners.push(l),
  };
  const warn = vi.spyOn(console, "warn").mockImplementation((line: unknown) => void log.push(["warn", String(line)]));
  try {
    const t = threadEnds(ctx as never, door as never);
    await act(t, w, e => listeners.forEach(l => l(e)), log);
    await flush();
  } finally {
    warn.mockRestore();
  }
  return { name, seed, world: { linked: [...w.linked], running: [...w.running], launching: [...w.launching], holds: [...w.holds] }, log, rows: [...rows] };
}

it("thread ends", async () => {
  const owedT1 = { p1: { threads: ["t1"] } };
  const scenes = [
    await scene("remove, computer away: owed, nothing said", {}, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", { remove: true })) ?? null])),
    await scene("stop, computer away: nothing owed, nothing said", {}, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", {})) ?? null])),
    await scene("away, no link: owed and the away line", {}, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", { away: true })) ?? null])),
    await scene("away, linked: owed, the late line, then paid in the background", {}, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", { away: true, remove: true })) ?? null]), { linked: new Set(["p1"]) }),
    await scene("away, linked, already owed: not owed twice", owedT1, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", { away: true })) ?? null]), { linked: new Set(["p1"]), answers: ["throws"] }),
    await scene("stop, linked, all ended", {}, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", {})) ?? null]), { linked: new Set(["p1"]) }),
    await scene("stop, linked, survivors named", owedT1, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", {})) ?? null]), { linked: new Set(["p1"]), answers: [{ exitCode: 1, stderr: "kill: 12\nprocesses 12 345 of thread-t1 still stand\n" }] }),
    await scene("stop, linked, nonzero with no pids named", {}, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", {})) ?? null]), { linked: new Set(["p1"]), answers: [{ exitCode: 3, stderr: "something else" }] }),
    await scene("stop, linked, the exec throws: the empty left line, nothing cleared", owedT1, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", {})) ?? null]), { linked: new Set(["p1"]), answers: ["throws"] }),
    await scene("remove, linked, all ended: cleared", owedT1, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", { remove: true })) ?? null]), { linked: new Set(["p1"]) }),
    await scene("remove, linked, survivors: owed and logged", {}, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", { remove: true })) ?? null]), { linked: new Set(["p1"]), answers: [{ exitCode: 1, stderr: "processes 7 of x" }] }),
    await scene("remove, linked, the exec throws: owed", {}, async (t, _w, _f, log) => void log.push(["answer", (await t.end("p1", "t1", { remove: true })) ?? null]), { linked: new Set(["p1"]), answers: ["throws"] }),
    await scene("owed: none owed answers nothing", {}, async (t, _w, _f, log) => void log.push(["answer", (await t.owed("p1", "t1")) === undefined ? null : "paid-promise"])),
    await scene("owed then paid at place.present", { p1: { threads: ["t1", "t2"] } }, async (t, _w, fire, log) => {
      const o = await t.owed("p1", "t1");
      log.push(["answer", o === undefined ? null : "paid-promise"]);
      void o?.paid.then(() => log.push(["paid", "t1"]));
      fire({ type: "place.present", placeId: "p1" });
      await flush();
    }, { linked: new Set(["p1"]), answers: [{ exitCode: 0, stderr: "" }, { exitCode: 1, stderr: "processes 9 of y" }] }),
    await scene("pay keeps a busy thread and one whose end did not answer", { p1: { threads: ["run", "launch", "held", "quiet"] } }, async (_t, _w, fire) => {
      fire({ type: "place.present", placeId: "p1" });
      await flush();
    }, { linked: new Set(["p1"]), running: new Set(["run"]), launching: new Set(["launch", "held"]), holds: new Set(["held"]), answers: ["throws", { exitCode: 0, stderr: "" }] }),
    await scene("place.removed drops the row and settles every waiter of that computer", { p1: { threads: ["t1"] }, p2: { threads: ["t1"] } }, async (t, _w, fire, log) => {
      const a = await t.owed("p1", "t1");
      const b = await t.owed("p2", "t1");
      void a?.paid.then(() => log.push(["paid", "p1"]));
      void b?.paid.then(() => log.push(["paid", "p2"]));
      fire({ type: "place.removed", placeId: "p1" });
      await flush();
    }),
    await scene("pay with nothing owed asks nothing", {}, async (_t, _w, fire) => {
      fire({ type: "place.present", placeId: "p1" });
      await flush();
    }, { linked: new Set(["p1"]) }),
    await scene("other events are ignored", { p1: { threads: ["t1"] } }, async (_t, _w, fire) => {
      fire({ type: "place.link", placeId: "p1" });
      await flush();
    }, { linked: new Set(["p1"]) }),
  ];
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "thread_ends.json"), `${JSON.stringify(scenes, null, 1)}\n`);
});
