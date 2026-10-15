// SPDX-License-Identifier: AGPL-3.0-only
import { createServer, type Server } from "node:http";
import net, { type AddressInfo } from "node:net";
import { failureWords } from "@wsp/engine";
import { Agent, fetch as undiciFetch } from "undici";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { connectPatiently } from "../src/entry.js";
import { slowRoad } from "./held-connect.js";

const realAttemptMs = net.getDefaultAutoSelectFamilyAttemptTimeout();
let road: (() => void) | undefined;
let server: Server | undefined;

async function serve(): Promise<number> {
  server = createServer((_q, res) => res.end("boat"));
  await new Promise<void>(resolve => server!.listen(0, "127.0.0.1", resolve));
  return (server.address() as AddressInfo).port;
}

beforeEach(() => {
  connectPatiently();
});

afterEach(async () => {
  road?.();
  road = undefined;
  net.setDefaultAutoSelectFamilyAttemptTimeout(realAttemptMs);
  await new Promise<void>(resolve => (server === undefined ? resolve() : server.close(() => resolve())));
  server = undefined;
});

describe("a host's connect on a road where each address takes over 250 ms", () => {
  it("reaches a server whose first address answers after 600 ms and whose second never answers", async () => {
    const port = await serve();
    road = slowRoad([{ address: "127.0.0.1", family: 4 }, { address: "::1", family: 6 }], { "127.0.0.1": 600, "::1": Infinity });
    const res = await fetch(`http://first-slow.acme.test:${port}/`, { signal: AbortSignal.timeout(8_000) });
    expect(await res.text()).toBe("boat");
  }, 12_000);

  it("reaches a server whose first address does not answer and whose second answers after 600 ms", async () => {
    const port = await serve();
    road = slowRoad([{ address: "::1", family: 6 }, { address: "127.0.0.1", family: 4 }], { "::1": Infinity, "127.0.0.1": 600 });
    const res = await fetch(`http://second-slow.acme.test:${port}/`, { signal: AbortSignal.timeout(8_000) });
    expect(await res.text()).toBe("boat");
  }, 12_000);
});

describe("a connect that fails names the addresses it tried", () => {
  it("when every address refuses", async () => {
    const port = await serve();
    await new Promise<void>(resolve => server!.close(() => resolve()));
    server = undefined;
    road = slowRoad([{ address: "127.0.0.1", family: 4 }, { address: "::1", family: 6 }]);
    const e = await fetch(`http://refused.acme.test:${port}/`).then(() => undefined, (err: unknown) => err);
    expect(failureWords(e)).toMatch(/^fetch failed: /);
    expect(failureWords(e)).toContain(`127.0.0.1:${port}`);
    expect(failureWords(e)).toContain(`::1:${port}`);
  });

  it("when the connect runs out of time", async () => {
    road = slowRoad([{ address: "127.0.0.1", family: 4 }, { address: "::1", family: 6 }], { "127.0.0.1": Infinity, "::1": Infinity });
    const dispatcher = new Agent({ connect: { timeout: 1_000, autoSelectFamily: true, autoSelectFamilyAttemptTimeout: 300 } });
    try {
      const e = await undiciFetch("http://silent.acme.test:4000/", { dispatcher }).then(() => undefined, (err: unknown) => err);
      expect(failureWords(e)).toContain("attempted addresses: 127.0.0.1:4000, ::1:4000");
    } finally {
      await dispatcher.close();
    }
  });
});
