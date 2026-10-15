// SPDX-License-Identifier: AGPL-3.0-only
import dns, { type LookupAddress } from "node:dns";
import { createServer, type Server } from "node:http";
import net, { type AddressInfo } from "node:net";
import { failureWords } from "@wsp/engine";
import { Agent, fetch as undiciFetch } from "undici";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { connectPatiently } from "../src/entry.js";

/** node's own TCP handle, so a test can hold an address's connect the way a slow road does: the attempt completes
 * after `late[address]` ms, or never. */
type Connect = (this: unknown, req: unknown, address: string, port: number) => number;
const { TCP } = (process as unknown as { binding(name: "tcp_wrap"): { TCP: { prototype: Record<"connect" | "connect6", Connect> } } }).binding("tcp_wrap");
const realConnect = TCP.prototype.connect;
const realConnect6 = TCP.prototype.connect6;
const realLookup = dns.lookup;
const realAttemptMs = net.getDefaultAutoSelectFamilyAttemptTimeout();
let late: Record<string, number> = {};
let server: Server | undefined;

function held(real: Connect): Connect {
  return function (req, address, port) {
    const ms = late[address];
    if (ms === undefined) return real.call(this, req, address, port);
    if (ms !== Infinity) setTimeout(() => real.call(this, req, address, port), ms);
    return 0;
  };
}

/** Every name answers with these addresses, in this order. */
function resolveTo(addresses: LookupAddress[]): void {
  (dns as { lookup: unknown }).lookup = (_host: string, _opts: unknown, cb: (e: null, a: LookupAddress[]) => void) => cb(null, addresses);
}

async function serve(): Promise<number> {
  server = createServer((_q, res) => res.end("boat"));
  await new Promise<void>(resolve => server!.listen(0, "127.0.0.1", resolve));
  return (server.address() as AddressInfo).port;
}

beforeEach(() => {
  TCP.prototype.connect = held(realConnect);
  TCP.prototype.connect6 = held(realConnect6);
  connectPatiently();
});

afterEach(async () => {
  TCP.prototype.connect = realConnect;
  TCP.prototype.connect6 = realConnect6;
  (dns as { lookup: unknown }).lookup = realLookup;
  net.setDefaultAutoSelectFamilyAttemptTimeout(realAttemptMs);
  late = {};
  await new Promise<void>(resolve => (server === undefined ? resolve() : server.close(() => resolve())));
  server = undefined;
});

describe("a host's connect on a road where each address takes over 250 ms", () => {
  it("reaches a server whose first address answers after 600 ms and whose second never answers", async () => {
    const port = await serve();
    late = { "127.0.0.1": 600, "::1": Infinity };
    resolveTo([{ address: "127.0.0.1", family: 4 }, { address: "::1", family: 6 }]);
    const res = await fetch(`http://first-slow.acme.test:${port}/`, { signal: AbortSignal.timeout(8_000) });
    expect(await res.text()).toBe("boat");
  }, 12_000);

  it("reaches a server whose first address does not answer and whose second answers after 600 ms", async () => {
    const port = await serve();
    late = { "::1": Infinity, "127.0.0.1": 600 };
    resolveTo([{ address: "::1", family: 6 }, { address: "127.0.0.1", family: 4 }]);
    const res = await fetch(`http://second-slow.acme.test:${port}/`, { signal: AbortSignal.timeout(8_000) });
    expect(await res.text()).toBe("boat");
  }, 12_000);
});

describe("a connect that fails names the addresses it tried", () => {
  it("when every address refuses", async () => {
    const port = await serve();
    await new Promise<void>(resolve => server!.close(() => resolve()));
    server = undefined;
    resolveTo([{ address: "127.0.0.1", family: 4 }, { address: "::1", family: 6 }]);
    const e = await fetch(`http://refused.acme.test:${port}/`).then(() => undefined, (err: unknown) => err);
    expect(failureWords(e)).toMatch(/^fetch failed: /);
    expect(failureWords(e)).toContain(`127.0.0.1:${port}`);
    expect(failureWords(e)).toContain(`::1:${port}`);
  });

  it("when the connect runs out of time", async () => {
    late = { "127.0.0.1": Infinity, "::1": Infinity };
    resolveTo([{ address: "127.0.0.1", family: 4 }, { address: "::1", family: 6 }]);
    const dispatcher = new Agent({ connect: { timeout: 1_000, autoSelectFamily: true, autoSelectFamilyAttemptTimeout: 300 } });
    try {
      const e = await undiciFetch("http://silent.acme.test:4000/", { dispatcher }).then(() => undefined, (err: unknown) => err);
      expect(failureWords(e)).toContain("attempted addresses: 127.0.0.1:4000, ::1:4000");
    } finally {
      await dispatcher.close();
    }
  });
});
