// SPDX-License-Identifier: AGPL-3.0-only
// Whether a remote MCP server reached with no header of its own asks for a
// sign-in, which config alone cannot say: one unsigned initialize from this
// host, answered 401 or 403 by a server that wants one and with its answer by
// one that does not (measured 2026-10-14: cloudflare, posthog and notion 401,
// context7 and deepwiki 200). The answer is the server's whatever computer
// asks, so it is kept per address for the host's life, in a map capped at
// PROBES_KEPT. A probe that fails or runs out of time keeps no answer, so the
// server stays on and the next read asks again. The address is written by the
// agent on the machine, so only a public https name is asked.
import { lookup } from "node:dns/promises";
import { BlockList, isIP } from "node:net";
import { INITIALIZE } from "./server-tools.js";

export const PROBE_MS = 3_000;
export const PROBES_KEPT = 256;

/** The status an unsigned initialize to the address answered; nothing where none came back. */
export type ProbePost = (url: string, ms: number) => Promise<number | undefined>;

export interface SignInProbes {
  /** True where the address answered 401 or 403, false where it answered anything else under 500, nothing where it
   * has not answered yet. */
  known(url: string): boolean | undefined;
  /** Asks each address with no kept answer once, at most one ask per address at a time; settles when every ask has. */
  ask(urls: readonly string[]): Promise<void>;
  readonly size: number;
}

export const fetchPost: ProbePost = async (url, ms) => {
  try {
    // No redirect is followed, so nothing of a sign-in page is fetched in the server's name.
    const res = await fetch(url, { method: "POST", redirect: "manual", signal: AbortSignal.timeout(ms), headers: { "content-type": "application/json", accept: "application/json, text/event-stream" }, body: INITIALIZE });
    void res.body?.cancel().catch(() => {});
    return res.status;
  } catch {
    return undefined;
  }
};

/** Loopback, private, shared, link-local, unique-local and multicast ranges: the person's own computer and network. An
 * IPv4 rule also holds for the address's IPv4-mapped IPv6 form (Node 22.23), so no ::ffff:0:0/96 rule is added. */
const NOT_PUBLIC = new BlockList();
for (const [net, bits] of [["0.0.0.0", 8], ["10.0.0.0", 8], ["100.64.0.0", 10], ["127.0.0.0", 8], ["169.254.0.0", 16], ["172.16.0.0", 12], ["192.168.0.0", 16], ["224.0.0.0", 3]] as const) NOT_PUBLIC.addSubnet(net, bits, "ipv4");
for (const [net, bits] of [["::", 127], ["fc00::", 7], ["fe80::", 10], ["ff00::", 8]] as const) NOT_PUBLIC.addSubnet(net, bits, "ipv6");

/** The address is the agent's to write, so this host asks only an https name every address of which is public: a name
 * resolved again to a private address meets a certificate that is not its own, and no status comes back. */
export const publicPost: ProbePost = async (url, ms) => {
  const u = URL.parse(url);
  if (u?.protocol !== "https:" || isIP(u.hostname.replace(/^\[|\]$/g, "")) !== 0) return undefined;
  const found = await lookup(u.hostname, { all: true }).catch(() => []);
  return found.length > 0 && found.every(a => !NOT_PUBLIC.check(a.address, a.family === 6 ? "ipv6" : "ipv4")) ? fetchPost(url, ms) : undefined;
};

export function signInProbes(o: { post?: ProbePost; ms?: number; cap?: number } = {}): SignInProbes {
  const post = o.post ?? publicPost;
  const cap = o.cap ?? PROBES_KEPT;
  const kept = new Map<string, boolean>();
  const asking = new Map<string, Promise<void>>();
  const keep = (url: string, signs: boolean): void => {
    if (kept.size >= cap && !kept.has(url)) kept.delete(kept.keys().next().value!);
    kept.set(url, signs);
  };
  const one = (url: string): Promise<void> => {
    const going = asking.get(url);
    if (going !== undefined) return going;
    const ms = o.ms ?? PROBE_MS;
    // The whole ask is bounded, the name's lookup with it: a launch waits on it (measured 7 s once without the bound).
    let timer: NodeJS.Timeout | undefined;
    const late = new Promise<undefined>(done => (timer = setTimeout(done, ms, undefined)));
    const asked = Promise.race([post(url, ms).catch(() => undefined), late])
      .then(status => {
        if (status !== undefined && status < 500 && (status < 300 || status >= 400)) keep(url, status === 401 || status === 403);
      })
      .finally(() => {
        clearTimeout(timer);
        asking.delete(url);
      });
    asking.set(url, asked);
    return asked;
  };
  return {
    known: url => kept.get(url),
    ask: async urls => void (await Promise.all(urls.filter(u => !kept.has(u)).map(one))),
    get size() {
      return kept.size;
    },
  };
}
