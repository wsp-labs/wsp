// SPDX-License-Identifier: AGPL-3.0-only
// The desktop app's processes through the host: the app's socket is told to
// read them only while a page watches, and each list it sends reaches the
// pages watching and leaves with the socket that sent it.
import type { AppProc } from "@wsp/protocol";
import { afterEach, describe, expect, it } from "vitest";
import { createRuntime, type Runtime } from "../src/runtime.js";
import { APP_PROCS_OFF_ROAD, APP_PROCS_UNOFFERED, APP_PROCS_WATCH_OFF_ROAD, appProcsHub } from "../src/serve-app-procs.js";
import { serveRuntime, type RuntimeServer } from "../src/serve.js";
import { memoryStore } from "../src/store.js";
import { stubBackend } from "./stub-backend.js";
import { until } from "./until.js";
import { WsClient } from "./ws-client.js";

const HOST_TOKEN = "secret";
const PROCS: AppProc[] = [
  { pid: 501, kind: "main" },
  { pid: 502, kind: "gpu" },
  { pid: 503, kind: "network" },
  { pid: 504, kind: "renderer" },
  { pid: 505, kind: "tab", origins: ["https://example.com"] },
];

let srv: RuntimeServer | undefined;
let rt: Runtime | undefined;

afterEach(async () => {
  await srv?.close();
  srv = undefined;
  await rt?.close();
  rt = undefined;
});

async function client(): Promise<WsClient> {
  if (srv === undefined) {
    rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: {} });
    srv = await serveRuntime(rt, { port: 0, authToken: HOST_TOKEN });
  }
  return WsClient.connect(srv.port, { token: HOST_TOKEN });
}

const wanted = (c: WsClient): unknown[] => c.events.filter(e => e.type === "app.procs.wanted").map(e => e["wanted"]);
const lists = (c: WsClient): unknown[] => c.events.filter(e => e.type === "app.procs").map(e => e["procs"]);

describe("the app's processes through the host", () => {
  it("are asked for while a page watches and no longer once its socket goes", async () => {
    const app = await client();
    expect(await app.request("appProcs.offer")).toMatchObject({ ok: true, wanted: false });

    const page = await client();
    expect(await page.request("appProcs.watch")).toMatchObject({ ok: true, procs: [] });
    await until(() => wanted(app).length === 1);
    expect(wanted(app)).toEqual([true]);

    // A second pane on the same socket and a second page each ride the one ask.
    expect(await page.request("appProcs.watch")).toMatchObject({ ok: true });
    const other = await client();
    expect(await other.request("appProcs.watch")).toMatchObject({ ok: true });

    expect(await app.request("appProcs.set", { procs: PROCS })).toMatchObject({ ok: true });
    await until(() => lists(page).length === 1 && lists(other).length === 1);
    expect(lists(page)).toEqual([PROCS]);

    expect(await other.request("appProcs.unwatch")).toMatchObject({ ok: true });
    page.close();
    await until(() => wanted(app).length === 2);
    expect(wanted(app)).toEqual([true, false]);

    // What the app read for pages that have all gone is not handed to the next one.
    expect(await app.request("appProcs.set", { procs: PROCS })).toMatchObject({ ok: true });
    expect(await other.request("appProcs.watch")).toMatchObject({ ok: true, procs: [] });
    expect(lists(other)).toHaveLength(1);
    app.close();
    other.close();
  });

  it("leave the pages with the socket that sent them, and come only from one that offered", async () => {
    const page = await client();
    await page.request("appProcs.watch");
    const app = await client();
    expect(await app.request("appProcs.set", { procs: PROCS })).toMatchObject({ ok: false, error: APP_PROCS_UNOFFERED });
    expect(await app.request("appProcs.offer")).toMatchObject({ ok: true, wanted: true });
    await app.request("appProcs.set", { procs: PROCS });
    await until(() => lists(page).length === 1);

    app.close();
    await until(() => lists(page).length === 2);
    expect(lists(page)[1]).toEqual([]);
    page.close();
  });

  it("refuse a list that is not a list of the app's processes", async () => {
    const app = await client();
    await app.request("appProcs.offer");
    expect(await app.request("appProcs.set", { procs: [{ pid: 7, kind: "agent" }] })).toMatchObject({ ok: false });
    expect(await app.request("appProcs.set", { procs: [{ pid: 7, kind: "tab", origins: ["x".repeat(300)] }] })).toMatchObject({ ok: false });
    app.close();
  });
});

describe("the hub", () => {
  it("takes no offer and no watch from a socket that came in through the relay", () => {
    const sent: unknown[] = [];
    const hub = appProcsHub();
    expect(() => hub.answer({ id: 1, op: "appProcs.offer" }, { send: f => sent.push(f), detaches: [], own: false })).toThrow(APP_PROCS_OFF_ROAD);
    expect(() => hub.answer({ id: 2, op: "appProcs.watch" }, { send: f => sent.push(f), detaches: [], own: false })).toThrow(APP_PROCS_WATCH_OFF_ROAD);
  });

  it("keeps one close per socket however often a pane opens and shuts", () => {
    const hub = appProcsHub();
    const socket = { send: () => undefined, detaches: [] as (() => void)[], own: true };
    for (let i = 0; i < 50; i++) {
      hub.answer({ id: i, op: "appProcs.watch" }, socket);
      hub.answer({ id: i, op: "appProcs.unwatch" }, socket);
    }
    expect(socket.detaches).toHaveLength(1);
  });

  it("names both apps' processes when two offer on one host", () => {
    const hub = appProcsHub();
    const seen: unknown[] = [];
    const page = { send: (f: unknown) => void seen.push(f), detaches: [] as (() => void)[], own: true };
    const one = { send: () => undefined, detaches: [] as (() => void)[], own: true };
    const two = { send: () => undefined, detaches: [] as (() => void)[], own: true };
    hub.answer({ id: 1, op: "appProcs.watch" }, page);
    hub.answer({ id: 2, op: "appProcs.offer" }, one);
    hub.answer({ id: 3, op: "appProcs.offer" }, two);
    hub.answer({ id: 4, op: "appProcs.set", procs: [PROCS[0]!] }, one);
    hub.answer({ id: 5, op: "appProcs.set", procs: [{ pid: 900, kind: "main" }] }, two);
    expect(seen.at(-1)).toEqual({ type: "app.procs", procs: [PROCS[0], { pid: 900, kind: "main" }] });
    for (const leave of one.detaches) leave();
    expect(seen.at(-1)).toEqual({ type: "app.procs", procs: [{ pid: 900, kind: "main" }] });
  });
});
