// SPDX-License-Identifier: AGPL-3.0-only
// A new thread's agent process started ahead of its first send, as the composer asks when it takes focus: the shipped
// Claude adapter over this computer's runs, driving a stand-in agent that says its pid and arguments as it starts and
// names the process and the turn token in every reply.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { LocalBackend } from "@wsp/engine";
import { AGENT_WARM_MS, AGENTS_KEPT, HERE_PLACE_ID, type Caller, type TurnResult } from "@wsp/protocol";
import { HARNESS_ADAPTERS } from "../src/adapters.js";
import { localExecStream } from "../src/local-exec.js";
import { createRuntime, type LocalWiring, type Runtime } from "../src/runtime.js";
import { memoryStore } from "../src/store.js";
import { fakeClock } from "./fake-clock.js";
import { claudeKeeper, pidOf, tokenOf } from "./kept-stubs.js";
import { stubBackend, copyingFake, createOn, testPlatform } from "./stub-backend.js";
import { alive, gone } from "./strays.js";
import { until } from "./until.js";

let root: string;
let runDir: string;
let heard: string;

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "wsp-warm-"));
  runDir = join(root, "runs");
  heard = join(root, "heard");
  mkdirSync(join(root, "bin"));
  claudeKeeper(join(root, "bin", "claude"));
});
const runtimes: Runtime[] = [];
afterEach(async () => {
  for (const rt of runtimes.splice(0)) await rt.close();
  await localExecStream({ root, runDir }).sweep!([]);
  rmSync(root, { recursive: true, force: true });
});

const host = (o: { clock?: ReturnType<typeof fakeClock>["clock"] } = {}): Runtime => {
  const local: LocalWiring = {
    backend: new LocalBackend({ root }),
    execStream: e => localExecStream({ root, runDir, pollMs: 20, ...e }),
    home: () => join(root, ".claude"),
    homeDir: root,
    rootsPath: join(root, "roots"),
    env: () => ({ PATH: `${join(root, "bin")}:${process.env["PATH"] ?? "/usr/bin:/bin"}`, STUB_HEARD: heard }),
    platform: testPlatform(),
    copier: copyingFake(),
  };
  const rt = createRuntime({
    backend: stubBackend(),
    store: memoryStore(),
    adapters: { claude: HARNESS_ADAPTERS.claude },
    local,
    ...(o.clock !== undefined ? { clock: o.clock } : {}),
    agents: { here: { url: "http://127.0.0.1:9" } },
  });
  runtimes.push(rt);
  return rt;
};

/** Every agent process the stand-in started, in the order it started them: its pid and the arguments it was given. */
const ups = (): { pid: number; args: string }[] =>
  (existsSync(heard) ? readFileSync(heard, "utf8") : "")
    .split("\n")
    .filter(l => l.startsWith("up "))
    .map(l => ({ pid: Number(l.split(" ")[1]), args: l.split(" ").slice(2).join(" ") }));
const upCount = async (n: number): Promise<{ pid: number; args: string }[]> => {
  await until(() => ups().length === n, 10_000);
  return ups();
};

const send = async (rt: Runtime, workspaceId: string, prompt: string, more: { thread?: string; harness?: string; model?: string; effort?: string; permissionMode?: string } = {}, origin?: Caller): Promise<{ result: TurnResult; threadId: string }> => {
  const handle = await rt.sessions.start(workspaceId, { prompt, ...more }, origin);
  const result = await handle.finished;
  return { result, threadId: handle.view().threadId! };
};

describe("an agent process started ahead of a new thread's first send", () => {
  it("runs the send that opens a thread on it, the process up before the send, and keeps it for the thread's next", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    expect(await rt.sessions.warm(ws.id, { harness: "claude" })).toEqual({ warm: "started" });
    const [warm] = await upCount(1);
    expect(warm!.args).toContain("--model claude-opus-5-5");
    const one = await send(rt, ws.id, "one", { harness: "claude" });
    expect(one.result.text).toBe(`one from ${warm!.pid} turn 1 token ${tokenOf(one.result.text)}`);
    expect(ups()).toHaveLength(1);
    // The thread's next send runs on the same process, as any thread's kept one.
    const two = await send(rt, ws.id, "two", { thread: one.threadId });
    expect(pidOf(two.result.text)).toBe(warm!.pid);
    expect(tokenOf(two.result.text)).toBe(tokenOf(one.result.text));
    const row = (await rt.sessions.list(ws.id)).find(r => r.threadId === one.threadId);
    expect(row?.status).toBe("completed");
    expect(row?.claudeSessionId).toMatch(/^[0-9a-f-]{36}$/);
  }, 30_000);

  it("never outlives its window, and a send after it boots a process of its own", async () => {
    const fc = fakeClock();
    const rt = host({ clock: fc.clock });
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    await rt.sessions.warm(ws.id, { harness: "claude" });
    const [first] = await upCount(1);
    fc.advance(AGENT_WARM_MS - 1_000);
    expect(alive(first!.pid)).toBe(true);
    fc.advance(1_000);
    await gone(first!.pid);
    // Asked for again before its window is out, the window starts over.
    expect(await rt.sessions.warm(ws.id, { harness: "claude" })).toEqual({ warm: "started" });
    const [, second] = await upCount(2);
    fc.advance(AGENT_WARM_MS - 1_000);
    expect(await rt.sessions.warm(ws.id, { harness: "claude" })).toEqual({ warm: "standing" });
    fc.advance(AGENT_WARM_MS - 1_000);
    expect(alive(second!.pid)).toBe(true);
    fc.advance(1_000);
    await gone(second!.pid);
    const one = await send(rt, ws.id, "one", { harness: "claude" });
    expect(pidOf(one.result.text)).not.toBe(second!.pid);
    expect(ups()).toHaveLength(3);
  }, 30_000);

  it("a changed model, effort or access ends the one standing and starts one launched with it", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    await rt.sessions.warm(ws.id, { harness: "claude" });
    const [first] = await upCount(1);
    expect(await rt.sessions.warm(ws.id, { harness: "claude", model: "claude-sonnet-5" })).toEqual({ warm: "started" });
    const [, second] = await upCount(2);
    await gone(first!.pid);
    expect(second!.args).toContain("--model claude-sonnet-5");
    await rt.sessions.warm(ws.id, { harness: "claude", model: "claude-sonnet-5", effort: "low" });
    const [, , third] = await upCount(3);
    await gone(second!.pid);
    expect(third!.args).toContain("--effort low");
    await rt.sessions.warm(ws.id, { harness: "claude", model: "claude-sonnet-5", effort: "low", permissionMode: "acceptEdits" });
    const [, , , fourth] = await upCount(4);
    await gone(third!.pid);
    expect(fourth!.args).toContain("--permission-mode acceptEdits");
    const one = await send(rt, ws.id, "one", { harness: "claude", model: "claude-sonnet-5", effort: "low", permissionMode: "acceptEdits" });
    expect(pidOf(one.result.text)).toBe(fourth!.pid);
  }, 30_000);

  it("a send at other picks than the one standing boots cold and ends it, rather than run on the wrong model", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    await rt.sessions.warm(ws.id, { harness: "claude", model: "claude-sonnet-5" });
    const [warm] = await upCount(1);
    // The send names no model, so it opens on the default, which is not the model the process was launched at.
    const one = await send(rt, ws.id, "one", { harness: "claude" });
    expect(pidOf(one.result.text)).not.toBe(warm!.pid);
    expect(ups()[1]!.args).toContain("--model claude-opus-5-5");
    await gone(warm!.pid);
  }, 30_000);

  it("a send that leaves out a pick the one standing was launched with boots cold too, so fast never rides unasked", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    await rt.sessions.warm(ws.id, { harness: "claude", fast: true });
    const [warm] = await upCount(1);
    expect(warm!.args).toContain('"fastMode":true');
    const one = await send(rt, ws.id, "one", { harness: "claude" });
    expect(pidOf(one.result.text)).not.toBe(warm!.pid);
    expect(ups()[1]!.args).not.toContain("fastMode");
    await gone(warm!.pid);
  }, 30_000);

  it("a send that takes it and is refused before its launch ends it, so none stands claimed beside the next", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    await rt.sessions.warm(ws.id, { harness: "claude" });
    const [warm] = await upCount(1);
    await expect(rt.sessions.start(ws.id, { prompt: "one", harness: "claude", notify: ["no-such-thread"] })).rejects.toThrow("no thread no-such-thread to notify");
    await gone(warm!.pid);
    expect(await rt.sessions.warm(ws.id, { harness: "claude" })).toEqual({ warm: "started" });
    await upCount(2);
  }, 30_000);

  it("a thread's own start neither starts one nor takes the person's", async () => {
    const rt = host();
    const ws = await createOn(rt, { on: HERE_PLACE_ID, name: "mac" });
    const lead = await send(rt, ws.id, "lead", { harness: "claude" });
    const asLead: Caller = { origin: "here", by: { kind: "thread", threadId: lead.threadId, workspaceId: ws.id, rootThreadId: lead.threadId } };
    expect(await rt.sessions.warm(ws.id, { harness: "claude" }, asLead)).toEqual({ warm: "none" });
    expect(ups()).toHaveLength(1);
    await rt.sessions.warm(ws.id, { harness: "claude" });
    const [, warm] = await upCount(2);
    const child = await send(rt, ws.id, "child", { harness: "claude" }, asLead);
    expect(pidOf(child.result.text)).not.toBe(warm!.pid);
    expect(alive(warm!.pid)).toBe(true);
    const mine = await send(rt, ws.id, "mine", { harness: "claude" });
    expect(pidOf(mine.result.text)).toBe(warm!.pid);
  }, 30_000);

  it(`stands one per folder and agent, five folders holding five, and every process counts against the cap of ${AGENTS_KEPT}`, async () => {
    const rt = host();
    const folders = [];
    for (let n = 0; n < 5; n++) folders.push(await createOn(rt, { on: HERE_PLACE_ID, name: `folder${n}` }));
    for (const [n, ws] of folders.entries()) {
      await rt.sessions.warm(ws.id, { harness: "claude" });
      await rt.sessions.warm(ws.id, { harness: "claude" });
      await upCount(n + 1);
    }
    const warm = ups();
    expect(warm.every(w => alive(w.pid))).toBe(true);
    // Two threads' kept processes in a sixth folder on top of the five: the seventh process ends the one idle
    // longest, the first folder's.
    const sixth = await createOn(rt, { on: HERE_PLACE_ID, name: "folder5" });
    await send(rt, sixth.id, "a", { harness: "claude" });
    await send(rt, sixth.id, "b", { harness: "claude" });
    await gone(warm[0]!.pid);
    expect(warm.slice(1).every(w => alive(w.pid))).toBe(true);
  }, 60_000);
});
