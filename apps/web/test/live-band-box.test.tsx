// SPDX-License-Identifier: AGPL-3.0-only
// The band's watch on a box, end to end: the Computers list and the page's own client against a real host, with the
// box a fake on its link keeping every frame the host sends it. Opening the list asks the box for one sys.watch and
// leaving it sends sys.unwatch, with no exec in between, so nothing starts a program there.
import { act, cleanup, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DAEMON_VERSION } from "@wsp/protocol";
import { makeApi, ProtocolClient } from "../src/protocol/client.js";
import { useStore } from "../src/protocol/store.js";
import { forgetComputerReadings } from "../src/machine/computerLive.js";
import { useSettingsStore } from "../src/settings/settingsStore.js";
import { mountSettings, resetSettings, settingsApi, settle } from "./settings-harness.js";
import { KEEPS_NO_IMAGE, code, ctx, join, placesOf, serving, sockets } from "../../../packages/runtime/test/places-fixture.js";
import { report } from "../../../packages/runtime/test/place-join.js";

// jsdom puts its own Uint8Array on the global, so the link's seal refuses every Buffer ws hands it until node's is back.
globalThis.Uint8Array = Object.getPrototypeOf(Buffer.prototype).constructor as Uint8ArrayConstructor;

let client: ProtocolClient | undefined;

beforeEach(() => {
  resetSettings();
  forgetComputerReadings();
});

afterEach(() => {
  cleanup();
  client?.close();
  client = undefined;
});

describe("the band's watch on a box the person joined", () => {
  it("asks the box for one sys.watch when the list opens, draws its samples, and sends sys.unwatch when the list is left", async () => {
    const { hostKey } = await serving();
    const frames: Record<string, unknown>[] = [];
    const sent = report("lab", { login: { HOME: "/root", USER: "root", PATH: "/usr/bin" }, daemonVersion: DAEMON_VERSION });
    const { client: box, placeId } = await join(hostKey, {
      code: await code(),
      report: sent,
      answers: c =>
        c.onFrame(raw => {
          const frame = raw as unknown as Record<string, unknown>;
          if (typeof frame["op"] !== "string") return;
          frames.push(frame);
          c.say({ id: frame["id"], ok: true, ...(frame["op"] === "machine.backend" ? KEEPS_NO_IMAGE : {}) });
        }),
    });
    sockets.push(box.ws);
    const ops = (op: string): number => frames.filter(f => f["op"] === op).length;

    client = new ProtocolClient({ url: `ws://127.0.0.1:${ctx.srv!.port}`, token: "host-token" });
    await client.connect();
    const real = makeApi(client);
    const lab = (await placesOf()).find(p => p.id === placeId)!;
    useStore.setState({ places: [lab] });
    const before = frames.length;
    mountSettings({
      api: settingsApi({ watchPlaceSys: real.watchPlaceSys!, unwatchPlaceSys: real.unwatchPlaceSys!, onPlaceSysSample: real.onPlaceSysSample!, placesReadings: real.placesReadings! }).api,
      at: { kind: "group", group: "computers" },
    });
    await settle();

    await waitFor(() => expect(ops("sys.watch")).toBe(1));
    box.say({ type: "sys.sample", cpu: 37, load1: 0.2, mem: { used: 1024 ** 3, total: 4 * 1024 ** 3 }, disk: { used: 1, total: 2 }, at: Date.now() });
    await waitFor(() => expect(document.querySelector(`[data-place-row='${placeId}'] [data-live-band] [data-k='cpu']`)?.textContent).toBe("CPU37%"));
    expect(ops("sys.unwatch")).toBe(0);

    act(() => useSettingsStore.getState().go({ kind: "group", group: "appearance" }));
    await settle();
    await waitFor(() => expect(ops("sys.unwatch")).toBe(1));
    expect(ops("sys.watch")).toBe(1);
    expect(frames.slice(before).filter(f => f["op"] === "exec")).toEqual([]);
  });
});
