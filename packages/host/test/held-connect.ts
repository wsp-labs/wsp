// SPDX-License-Identifier: AGPL-3.0-only
// A road slow to connect, for a test in this process: every name resolves to
// the addresses given, and node's own TCP handle holds the connect to each
// address the test names, the way a NAT64 network holds GitHub's.
import dns, { type LookupAddress } from "node:dns";

type Connect = (this: unknown, req: unknown, address: string, port: number) => number;
const { TCP } = (process as unknown as { binding(name: "tcp_wrap"): { TCP: { prototype: Record<"connect" | "connect6", Connect> } } }).binding("tcp_wrap");
const realConnect = TCP.prototype.connect;
const realConnect6 = TCP.prototype.connect6;
const realLookup = dns.lookup;

function held(real: Connect, late: Record<string, number>): Connect {
  return function (req, address, port) {
    const ms = late[address];
    if (ms === undefined) return real.call(this, req, address, port);
    if (ms !== Infinity) setTimeout(() => real.call(this, req, address, port), ms);
    return 0;
  };
}

/** Every name answers with `addresses`, in this order, and a connect to an address in `late` completes after that
 * many ms, or never; the returned call puts node's own back. */
export function slowRoad(addresses: LookupAddress[], late: Record<string, number> = {}): () => void {
  TCP.prototype.connect = held(realConnect, late);
  TCP.prototype.connect6 = held(realConnect6, late);
  (dns as { lookup: unknown }).lookup = (_host: string, _opts: unknown, cb: (e: null, a: LookupAddress[]) => void) => cb(null, addresses);
  return () => {
    TCP.prototype.connect = realConnect;
    TCP.prototype.connect6 = realConnect6;
    (dns as { lookup: unknown }).lookup = realLookup;
  };
}
