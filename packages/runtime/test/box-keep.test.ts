// SPDX-License-Identifier: AGPL-3.0-only
// A thread in a folder on a joined computer keeps its agent process between turns, as one on the computer the host
// runs on does: the real Claude adapter over a fake computer on the link whose launches start agent processes that
// answer message after message, so a case counts the launches its turns cost.
import { describe, expect, it } from "vitest";
import { AGENT_KEEP_MS, agentsKeptOn, type TurnResult } from "@wsp/protocol";
import { HARNESS_ADAPTERS } from "../src/adapters.js";
import { memoryStore } from "../src/store.js";
import { placesOf, relink, sockets } from "./places-fixture.js";
import { box, HETZNER, joined, type Box } from "./box-fixture.js";
import { report } from "./place-join.js";
import { fakeClock } from "./fake-clock.js";
import { until } from "./until.js";
import type { HarnessAdapterFactory, HarnessSession } from "../src/types/harness.js";

const ADAPTERS = { claude: HARNESS_ADAPTERS.claude };

/** The shipped Claude adapter, its kept process saying it still runs work a turn left going, as a Codex server with a
 * background command does. */
const working: HarnessAdapterFactory = ctx => {
  const adapter = HARNESS_ADAPTERS.claude(ctx);
  const keptWorking = (session: HarnessSession): HarnessSession =>
    new Proxy(session, {
      get: (target, key) => {
        if (key !== "kept") return Reflect.get(target, key);
        return () => {
          const kept = target.kept?.();
          return kept === undefined ? undefined : { ...kept, working: () => true, next: (turn: Parameters<typeof kept.next>[0]) => keptWorking(kept.next(turn)) };
        };
      },
    });
  return new Proxy(adapter, { get: (target, key) => (key === "start" ? (o: Parameters<typeof target.start>[0]) => keptWorking(target.start(o)) : Reflect.get(target, key)) });
};
/** What hetzner reports of itself as it joins, its memory among it. */
const HETZNER_SHAPE = report("hetzner").shape;

/** A host holding hetzner with its folder added, every launch there an agent that stays up for the next message. */
async function onHetzner(o: { clock?: ReturnType<typeof fakeClock>["clock"]; memMb?: number; adapters?: Record<string, HarnessAdapterFactory> } = {}) {
  const store = memoryStore();
  const held = await joined({ store, adapters: o.adapters ?? ADAPTERS, agent: true, ...(o.clock !== undefined ? { clock: o.clock } : {}), ...(o.memMb !== undefined ? { memMb: o.memMb } : {}) });
  const at = await held.rt.workspaces.folderFor({ project: held.project.id });
  const workspaceId = at.workspace.id;
  const send = async (prompt: string, thread?: string): Promise<{ result: TurnResult; threadId: string }> => {
    const handle = await held.rt.sessions.start(workspaceId, { prompt, harness: "claude", ...(thread !== undefined ? { thread } : {}) });
    const result = await handle.finished;
    return { result, threadId: handle.view().threadId! };
  };
  const launches = (): number => held.seen.order.filter(step => step === "launch").length;
  return { ...held, store, workspaceId, send, launches };
}

describe("a thread on a joined computer keeps its agent process between turns", () => {
  it("answers the second send from the process its first turn left up: two turns, one launch", async () => {
    const { send, launches } = await onHetzner();
    const one = await send("one");
    expect(one.result).toMatchObject({ status: "completed", text: "one from run 1 turn 1" });
    const two = await send("two", one.threadId);
    expect(two.result).toMatchObject({ status: "completed", text: "two from run 1 turn 2" });
    expect(launches()).toBe(1);
  }, 20_000);

  it(`keeps at most ${agentsKeptOn(HETZNER_SHAPE.memMb)} there for its ${HETZNER_SHAPE.memMb} MB, the one idle longest ended first`, async () => {
    const { send, launches, seen } = await onHetzner();
    const cap = agentsKeptOn(HETZNER_SHAPE.memMb);
    const threads: string[] = [];
    for (let n = 0; n <= cap; n++) threads.push((await send(`thread ${n}`)).threadId);
    expect(launches()).toBe(cap + 1);
    // The first thread's process went to make room, its input closed, and its next send boots again.
    await until(() => [...seen.runs.values()].find(run => run.n === 1)?.exit !== undefined);
    expect((await send("again", threads[0])).result.text).toBe(`again from run ${cap + 2} turn 1`);
    // The last one's is still up.
    expect((await send("still", threads[cap])).result.text).toBe(`still from run ${cap + 1} turn 2`);
  }, 30_000);

  it("ends the kept process once the keep runs out, and the next send boots again", async () => {
    const fc = fakeClock();
    const { send, launches, seen } = await onHetzner({ clock: fc.clock });
    const one = await send("one");
    await until(() => fc.dueFor(fc.clock.now() + AGENT_KEEP_MS));
    fc.advance(AGENT_KEEP_MS);
    await until(() => seen.runs.get([...seen.runs.keys()][0]!)?.exit !== undefined);
    expect((await send("two", one.threadId)).result.text).toBe("two from run 2 turn 1");
    expect(launches()).toBe(2);
  }, 20_000);

  it("boots again where the session file changed there since the last turn, as a terminal resuming it does", async () => {
    const { send, launches, seen } = await onHetzner();
    seen.sessionBytes = 1_000;
    const one = await send("one");
    // The turn's end reads the file there before anything changes it.
    await until(() => seen.execs.some(e => e.cmd.includes("wc -c <")));
    seen.sessionBytes = 1_400;
    expect((await send("two", one.threadId)).result.text).toBe("two from run 2 turn 1");
    // Left as the last turn found it, the next send runs on the process that turn left.
    expect((await send("three", one.threadId)).result.text).toBe("three from run 2 turn 2");
    expect(launches()).toBe(2);
  }, 20_000);

  it("runs on the kept process though the session file changed there, while that process still runs work its turn left", async () => {
    const { send, launches, seen } = await onHetzner({ adapters: { claude: working } });
    seen.sessionBytes = 1_000;
    const one = await send("one");
    await until(() => seen.execs.some(e => e.cmd.includes("wc -c <")));
    seen.sessionBytes = 1_400;
    expect((await send("two", one.threadId)).result.text).toBe("two from run 1 turn 2");
    expect(launches()).toBe(1);
  }, 20_000);

  it("closes the agent at the computer's next link when the keep runs out while it is away, and ends nothing else the thread left there", async () => {
    const fc = fakeClock();
    const held = await onHetzner({ clock: fc.clock });
    await held.send("one");
    await away(held.placeId);
    await until(() => fc.dueFor(fc.clock.now() + AGENT_KEEP_MS));
    fc.advance(AGENT_KEEP_MS);
    await nothingOwed(held);
  }, 20_000);

  it("a send while the computer is away boots cold, owes it no thread end, and the old agent closes at its next link", async () => {
    const held = await onHetzner();
    const one = await held.send("one");
    await away(held.placeId);
    const two = held.rt.sessions.start(held.workspaceId, { prompt: "two", harness: "claude", thread: one.threadId });
    await nothingOwed(held);
    expect((await (await two).finished).text).toMatch(/^two from run \d+ turn 1$/);
  }, 20_000);

  it("waits out the host's own title write into the session file before reading it, so the send runs on the kept agent", async () => {
    const { rt, seen, send, launches, workspaceId } = await onHetzner();
    seen.sessionBytes = 1_000;
    const handle = await rt.sessions.start(workspaceId, { prompt: "one", harness: "claude" });
    await handle.finished;
    const threadId = handle.view().threadId!;
    await until(() => seen.execs.some(e => e.cmd.includes("wc -c <")));
    // The title lands in the file at once and its answer comes later, so a send meanwhile reads a grown file.
    seen.titleMs = 300;
    const titled = rt.sessions.rename(handle.id, "Box keep");
    await until(() => seen.execs.some(e => e.cmd.includes("custom-title")));
    expect((await send("two", threadId)).result.text).toBe("two from run 1 turn 2");
    await titled;
    expect(launches()).toBe(1);
  }, 20_000);
});

/** The computer's link gone from this host, as its socket closing tells it. */
async function away(placeId: string): Promise<void> {
  for (const ws of sockets.splice(0)) ws.close();
  await until(async () => (await placesOf()).find(p => p.id === placeId)!.present === false);
}

/** While the computer is away nothing is owed it; at its next link the first run's input closes and its own group is
 * reaped, and the end of the thread's cgroup is never sent. */
async function nothingOwed(held: Awaited<ReturnType<typeof onHetzner>>): Promise<void> {
  const [run] = [...held.seen.runs.keys()];
  await new Promise(resolve => setTimeout(resolve, 300));
  expect(await held.store.get("thread-ends", held.placeId)).toBeUndefined();
  let back!: Box;
  const linked = await relink(held.hostKey, held.placeId, held.pair, report("hetzner", { login: { HOME: HETZNER.home, USER: "root", PATH: "/usr/bin" } }), c => {
    back = box(c, HETZNER, { agent: true, runs: held.seen.runs });
  });
  sockets.push(linked.client.ws);
  await until(() => back.kills.some(cmd => cmd.includes(`'${run}'`)));
  expect(held.seen.runs.get(run!)?.exit ?? "reaped").not.toBe("143");
  expect([...held.seen.execs, ...back.execs].filter(e => e.cmd.includes("wsp_end")).map(e => e.cmd)).toEqual([]);
  expect(await held.store.get("thread-ends", held.placeId)).toBeUndefined();
}

describe("a new thread's agent started ahead of its first send on a joined computer", () => {
  it("launches as the composer asks, and the send that opens the thread runs on it", async () => {
    const { rt, send, launches, workspaceId } = await onHetzner();
    expect(await rt.sessions.warm(workspaceId, { harness: "claude" })).toEqual({ warm: "started" });
    await until(() => launches() === 1);
    const opened = await send("first");
    expect(opened.result.text).toBe("first from run 1 turn 1");
    expect(launches()).toBe(1);
  }, 20_000);

  it("starts none on a computer too small to keep one, and its threads launch at each send", async () => {
    const { rt, send, launches, workspaceId } = await onHetzner({ memMb: 1_024 });
    expect(agentsKeptOn(1_024)).toBe(0);
    expect(await rt.sessions.warm(workspaceId, { harness: "claude" })).toEqual({ warm: "none" });
    const one = await send("one");
    expect((await send("two", one.threadId)).result.text).toBe("two from run 2 turn 1");
    expect(launches()).toBe(2);
  }, 20_000);
});
