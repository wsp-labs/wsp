// SPDX-License-Identifier: AGPL-3.0-only
// A session row keeps the head of the message its thread opened with, never the whole of it: an agent opens a
// sub-thread with a brief tens of thousands of characters long, and every row was held in the host, written whole to
// the session index at every save and sent to every window and the menu bar, each keeping all of them.
import { foldThreads, OPENING_KEPT, type EventUnion, type SessionView, type TurnResult } from "@wsp/protocol";
import { afterEach, describe, expect, it } from "vitest";
import { createRuntime, type HarnessAdapterFactory, type Runtime } from "../src/runtime.js";
import { serveRuntime, type RuntimeServer } from "../src/serve.js";
import { memoryStore, type Store } from "../src/store.js";
import { createOn, stubBackend } from "./stub-backend.js";
import { until } from "./until.js";
import { WsClient } from "./ws-client.js";

const OPENING = `Build the ticket, and say what you find. ${"Read every comment first, then the code it names. ".repeat(700)}`;

const quick: HarnessAdapterFactory = () => ({
  steers: false,
  start: ({ onEvent, resume }) => {
    const sessionId = resume ?? "22222222-2222-4222-8222-222222222222";
    const result: TurnResult = { status: "completed", text: "done" };
    onEvent({ type: "session.start", sessionId });
    onEvent({ type: "turn.done", sessionId, result });
    onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
    return { localId: sessionId, finished: Promise.resolve(result), interrupt: async () => {} };
  },
});

const indexRows = async (store: Store, workspaceId: string): Promise<SessionView[]> => ((await store.get("sessions", workspaceId)) as { sessions?: SessionView[] } | undefined)?.sessions ?? [];
const kept = (prompt: string | undefined): boolean => prompt !== undefined && prompt.length <= OPENING_KEPT && OPENING.startsWith(prompt);

let rt: Runtime | undefined;
let srv: RuntimeServer | undefined;
let client: WsClient | undefined;
afterEach(async () => {
  client?.close();
  await srv?.close();
  await rt?.close();
  client = srv = rt = undefined;
});

describe("the opening on a session row", () => {
  it("is its first 600 characters in the host, the session index, every listing and every pushed row, and the title stands across a restart", async () => {
    expect(OPENING.length).toBeGreaterThan(35_000);
    const store = memoryStore();
    const backend = stubBackend();
    rt = createRuntime({ backend, store, adapters: { claude: quick } });
    const heard: EventUnion[] = [];
    rt.events.on("*", e => void heard.push(e));
    srv = await serveRuntime(rt, { port: 0, authToken: "host-token", devices: rt.devices });
    client = await WsClient.connect(srv.port, { token: "host-token" });
    const ws = await createOn(rt, { golden: "snap_g", name: "a" });

    await (await rt.sessions.start(ws.id, { prompt: OPENING })).finished;
    const rows = await rt.sessions.list(ws.id);
    expect(rows.map(r => kept(r.prompt))).toEqual([true]);
    const listed = await client.request("sessions.list", { workspaceId: ws.id });
    expect(JSON.stringify(listed).length).toBeLessThan(OPENING_KEPT + 2_000);
    const pushed = heard.filter(e => e.type === "session.row");
    expect(pushed.length).toBeGreaterThan(0);
    for (const e of pushed) expect(JSON.stringify(e).length).toBeLessThan(OPENING_KEPT + 2_000);
    await until(async () => (await indexRows(store, ws.id)).length === 1);
    expect((await indexRows(store, ws.id)).map(r => kept(r.prompt))).toEqual([true]);
    // The transcript keeps the message whole: the thread reads back as it was sent.
    expect((await rt.sessions.history(ws.id)).find(e => e.type === "session.start")).toMatchObject({ prompt: OPENING });
    const title = foldThreads(rows)[0]!.title;

    client.close();
    await srv.close();
    await rt.close();
    client = srv = undefined;
    rt = createRuntime({ backend, store, adapters: { claude: quick } });
    expect(foldThreads(await rt.sessions.list(ws.id))[0]!.title).toBe(title);
  });

  it("a session index an older host wrote with the whole opening is read back cut, under the same title, and written back cut once", async () => {
    const store = memoryStore();
    const backend = stubBackend();
    rt = createRuntime({ backend, store, adapters: { claude: quick } });
    const ws = await createOn(rt, { golden: "snap_g", name: "a" });
    await (await rt.sessions.start(ws.id, { prompt: OPENING })).finished;
    const title = foldThreads(await rt.sessions.list(ws.id))[0]!.title;
    await rt.close();
    const index = (await store.get("sessions", ws.id)) as { sessions: SessionView[] };
    await store.put("sessions", ws.id, { ...index, sessions: index.sessions.map(r => ({ ...r, prompt: OPENING })) });

    let writes = 0;
    const put = store.put.bind(store);
    store.put = (collection, id, value) => ((writes += collection === "sessions" ? 1 : 0), put(collection, id, value));

    rt = createRuntime({ backend, store, adapters: { claude: quick } });
    const rows = await rt.sessions.list(ws.id);
    expect(rows.map(r => kept(r.prompt))).toEqual([true]);
    expect(foldThreads(rows)[0]!.title).toBe(title);
    // Nothing else may ever save this workspace's rows again, and every launch would parse the whole index.
    await until(async () => (await indexRows(store, ws.id)).every(r => kept(r.prompt)));
    await rt.close();
    const written = writes;
    rt = createRuntime({ backend, store, adapters: { claude: quick } });
    await rt.sessions.list(ws.id);
    await new Promise(r => setTimeout(r, 100));
    expect(writes).toBe(written);
  });
});
