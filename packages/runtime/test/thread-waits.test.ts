// SPDX-License-Identifier: AGPL-3.0-only
// A turn on a cloud machine whose requests the host held while it was away: nothing shows during the wait, and once
// the requests are let through the thread holds one row saying how long, written into its transcript.
import { describe, expect, it } from "vitest";
import { waitedLine, type SessionEvent, type TurnResult } from "@wsp/protocol";
import { createRuntime, type HarnessAdapterFactory } from "../src/runtime.js";
import { WAIT_SETTLE_MS } from "../src/threads/waits.js";
import { memoryStore } from "../src/store.js";
import { stubBackend, createOn } from "./stub-backend.js";
import { fakeClock } from "./fake-clock.js";

const answers: HarnessAdapterFactory = () => ({
  steers: false,
  start: ({ onEvent }) => {
    const sessionId = "44444444-4444-4444-8444-444444444444";
    const result: TurnResult = { status: "completed", text: "done" };
    onEvent({ type: "session.start", sessionId });
    onEvent({ type: "turn.done", sessionId, result });
    onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
    return { localId: sessionId, finished: Promise.resolve(result), interrupt: async () => {} };
  },
});

const MIN = 60_000;

async function thread() {
  const { clock, advance } = fakeClock();
  const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: answers }, clock });
  const ws = await createOn(rt, { golden: "snap_g", name: "web", cpu: 2, memMb: 4096 });
  const turn = await rt.sessions.start(ws.id, { prompt: "fix the upload test" });
  await turn.finished;
  const threadId = turn.view().threadId!;
  const waited = async (): Promise<Extract<SessionEvent, { type: "thread.waited" }>[]> =>
    (await rt.sessions.history(ws.id)).filter((e): e is Extract<SessionEvent, { type: "thread.waited" }> => e.type === "thread.waited");
  return { rt, advance, threadId, waited };
}

describe("a turn that waited for its host", () => {
  it("makes one row for a 14 minute hold and none for a 2 second one", async () => {
    const { rt, advance, threadId, waited } = await thread();
    await rt.sessions.noteWaited(threadId, 2_000);
    advance(WAIT_SETTLE_MS);
    expect(await waited()).toEqual([]);

    await rt.sessions.noteWaited(threadId, 14 * MIN);
    expect(await waited()).toEqual([]);
    advance(WAIT_SETTLE_MS);
    const rows = await waited();
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({ type: "thread.waited", threadId, ms: 14 * MIN });
    expect(rows[0]!.said).toBeUndefined();
    expect(waitedLine(rows[0]!.ms)).toBe("Waited 14 min for your computer");
  });

  it("makes one row of the longest for requests held together, and a new row for a later wait", async () => {
    const { rt, advance, threadId, waited } = await thread();
    await rt.sessions.noteWaited(threadId, 2 * MIN);
    await rt.sessions.noteWaited(threadId, 14 * MIN);
    advance(WAIT_SETTLE_MS - 1);
    await rt.sessions.noteWaited(threadId, 9 * MIN);
    advance(1);
    expect((await waited()).map(e => e.ms)).toEqual([14 * MIN]);
    advance(20 * MIN);
    await rt.sessions.noteWaited(threadId, 3 * MIN);
    advance(WAIT_SETTLE_MS);
    expect((await waited()).map(e => waitedLine(e.ms))).toEqual(["Waited 14 min for your computer", "Waited 3 min for your computer"]);
  });

  it("writes a sentence the host answered with once per turn, however short the hold", async () => {
    const { rt, advance, threadId, waited } = await thread();
    const said = "Codex's sign-in for cloud threads on acme's laptop ended. Sign in again in Settings, Agents.";
    await rt.sessions.noteWaited(threadId, 0, `wsp: ${said}`);
    await rt.sessions.noteWaited(threadId, 0, `wsp: ${said}`);
    advance(WAIT_SETTLE_MS);
    expect((await waited()).map(e => [e.ms, e.said])).toEqual([[0, said]]);
  });

  it("writes nothing for a thread it does not hold", async () => {
    const { rt, advance, waited } = await thread();
    await rt.sessions.noteWaited("thr_gone", 14 * MIN);
    advance(WAIT_SETTLE_MS);
    expect(await waited()).toEqual([]);
  });
});

describe("the waited row's words", () => {
  it("reads seconds under a minute and whole minutes after", () => {
    expect(waitedLine(6_000)).toBe("Waited 6 s for your computer");
    expect(waitedLine(59_400)).toBe("Waited 59 s for your computer");
    expect(waitedLine(59_600)).toBe("Waited 1 min for your computer");
    expect(waitedLine(3 * MIN + 20_000)).toBe("Waited 3 min for your computer");
    expect(waitedLine(60 * MIN)).toBe("Waited 60 min for your computer");
  });
});
