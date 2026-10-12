// SPDX-License-Identifier: AGPL-3.0-only
// A setup's end over the real client and a scripted socket, while a places.list
// asked before the end is still in flight: the client shares reads in flight,
// and an answer read before the end must not be the last word on the row.
import { afterEach, describe, expect, it } from "vitest";
import type { PlaceSetup, PlaceView } from "@wsp/protocol";
import { makeApi, ProtocolClient } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { placeStateCell } from "../src/settings/places.js";
import { ScriptedSocket, type Frame } from "./scripted-socket.js";

const RUNNING: PlaceSetup = { state: "running", addId: "a_1", startedAt: "2026-10-12T10:00:00.000Z", steps: [{ step: "floor", state: "running" }], waiting: [] };
const box = (setup: PlaceSetup): PlaceView => ({ id: "p_2", kind: "computer", name: "lab", default: false, present: true, takesForks: true, setup });

let client: ProtocolClient | null = null;
afterEach(() => { client?.close(); client = null; });

async function until(cond: () => boolean, ms = 2000): Promise<void> {
  const deadline = Date.now() + ms;
  while (!cond()) {
    if (Date.now() > deadline) throw new Error("condition not met in time");
    await new Promise(r => setTimeout(r, 5));
  }
}

describe("a setup that ends while the places list is being read", () => {
  it("leaves the row on the ended state once every read lands, though the read in flight was answered before the end", async () => {
    const lists: Frame[] = [];
    ScriptedSocket.authOk = true;
    ScriptedSocket.serverUp = true;
    ScriptedSocket.reply = f => {
      const id = f["id"];
      switch (f["op"]) {
        case "events.subscribe": return { id, ok: true, seq: 0, stream: "stream-a" };
        case "places.list": return void lists.push(f);
        case "workspaces.list": return { id, ok: true, workspaces: [] };
        case "status.subscribe": return { id, ok: true, statuses: [] };
        default: return { id, ok: false, error: `unscripted op ${String(f["op"])}` };
      }
    };
    ScriptedSocket.instances.length = 0;
    client = new ProtocolClient({ url: "ws://test", token: "tok", WebSocketCtor: ScriptedSocket as unknown as typeof WebSocket });
    await client.connect();
    const sock = ScriptedSocket.instances[0]!;
    const deliver = (e: Record<string, unknown>) => sock.onmessage?.({ data: JSON.stringify(e) });
    const answer = (n: number, setup: PlaceSetup) => deliver({ id: lists[n]!["id"], ok: true, places: [box(setup)] });
    useStore.setState({ places: [], pending: [], placesRead: false });
    useStore.getState().bind(makeApi(client));
    useStore.getState().setConn("live");
    const row = () => placeStateCell(useStore.getState().places.find(p => p.id === "p_2")!, null, { canUpdate: false });

    await until(() => lists.length === 1);
    answer(0, RUNNING);
    await until(() => useStore.getState().places.length === 1);

    // A step's end asks the list for its rows; the host reads it while the setup still runs.
    deliver({ type: "place.setup", seq: 1, addId: "a_1", placeId: "p_2", line: { step: "floor", state: "done" } });
    expect(lists).toHaveLength(2);
    deliver({ type: "place.setup", seq: 2, addId: "a_1", placeId: "p_2", end: "ready" });
    expect(row()).toMatchObject({ word: "Ready" });
    answer(1, { ...RUNNING, steps: [{ step: "floor", state: "done" }, { step: "agents", state: "running" }] });

    // The read asked after the end goes out once the older one lands, and the host answers it with the end.
    await until(() => lists.length === 3);
    answer(2, { ...RUNNING, state: "done", steps: [{ step: "floor", state: "done" }, { step: "agents", state: "done" }] });
    await new Promise(r => setTimeout(r, 20));
    expect(row()).toMatchObject({ word: "Ready" });
    expect(lists).toHaveLength(3);
  });
});
