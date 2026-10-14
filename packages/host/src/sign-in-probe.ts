// SPDX-License-Identifier: AGPL-3.0-only
// Whether a remote MCP server reached with no header of its own asks for a
// sign-in, which config alone cannot say: one unsigned initialize from this
// host, answered 401 or 403 by a server that wants one and with its answer by
// one that does not (measured 2026-10-14: cloudflare, posthog and notion 401,
// context7 and deepwiki 200). The answer is the server's whatever computer
// asks, so it is kept per address for the host's life, in a map capped at
// PROBES_KEPT. A probe that fails or runs out of time keeps only that it was
// asked, so the server stays on and the next read asks again. The address is
// written by the agent on the machine, so only a public https name is asked.
import { lookup, Resolver } from "node:dns/promises";
import { request } from "node:https";
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
  /** Whether an ask of the address has ended in this host's life, answered or not. */
  tried(url: string): boolean;
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

/** The address is the agent's to write, so this host asks only an https name every address of which is public, and
 * connects to the addresses it checked, never to a second lookup's. The name is read with c-ares, off the pool of two
 * that the system's lookup shares with the whole process: a lookup there cannot be stopped, so a name whose DNS never
 * answers would hold every other lookup of this host past any bound. */
export const publicPostWith = (resolvers: () => Resolver[], system: SystemLookup = SYSTEM): ProbePost => async (url, ms) => {
  const u = URL.parse(url);
  if (u?.protocol !== "https:" || isIP(u.hostname.replace(/^\[|\]$/g, "")) !== 0) return undefined;
  const found = await checkedAddresses(resolvers(), system, u.hostname, ms);
  if (found.length === 0 || found.some(a => NOT_PUBLIC.check(a, isIP(a) === 6 ? "ipv6" : "ipv4"))) return undefined;
  const all = found.map(address => ({ address, family: isIP(address) }));
  return new Promise(done => {
    // Answered on the next tick: answered at once, an address that fails at once threw out of request() and raised an
    // error on the socket that nothing listens to, which ends the host (Node 22.23).
    const answer = (o: { all?: boolean }, cb: (...a: unknown[]) => void): void => (o.all === true ? cb(null, all) : cb(null, all[0]!.address, all[0]!.family));
    const req = request(u, { method: "POST", timeout: ms, headers: { "content-type": "application/json", accept: "application/json, text/event-stream" }, lookup: (_n, o, cb) => process.nextTick(answer, o, cb) }, res => {
      done(res.statusCode);
      res.destroy();
    });
    req.on("timeout", () => req.destroy()).on("error", () => done(undefined)).end(INITIALIZE);
  });
};

/** The first answer with any address among `asks`; none where every one came back empty or failed. */
function firstAnswer(asks: readonly Promise<string[]>[]): Promise<string[]> {
  return new Promise<string[]>(done => {
    let left = asks.length;
    if (left === 0) return done([]);
    const none = (): void => void (--left === 0 && done([]));
    for (const asked of asks) asked.then(a => (a.length > 0 ? done(a) : none()), none);
  });
}

/** The addresses the probe dials, and so all it checks: the first IPv4 answer of any resolver, else the first IPv6 one.
 * IPv4 first, since some IPv6 addresses do not route from a home network that routes others (AWS's, 2026-10-14); and no
 * waiting on both families, which held a name with no IPv6 address for its AAAA query, 2 s or a timeout. */
export async function addressesOf(rs: readonly Resolver[], name: string): Promise<string[]> {
  const v6 = firstAnswer(rs.map(r => r.resolve6(name)));
  const v4 = await firstAnswer(rs.map(r => r.resolve4(name)));
  const found = v4.length > 0 ? v4 : await v6;
  for (const r of rs) r.cancel();
  return found;
}

/** The system's lookup of a name, its addresses with IPv4 first as `addressesOf()` takes them; none where the name had
 * no turn before `ms` ran out. */
export type SystemLookup = (name: string, ms: number) => Promise<string[]>;

/** The system's lookup through one place for the whole host: at most one runs at a time, and a name whose lookup still
 * runs or waits starts no second. A system lookup cannot be stopped and the process runs two at once, so a name whose
 * DNS never answers holds one of the two, never both. A name still waiting when its `ms` runs out answers none. */
export function oneSystemLookup(look: (name: string) => Promise<string[]>): SystemLookup {
  const waiting: { name: string; done: (found: string[]) => void }[] = [];
  const asked = new Map<string, Promise<string[]>>();
  let busy = false;
  const next = (): void => {
    const item = busy ? undefined : waiting.shift();
    if (item === undefined) return;
    busy = true;
    void look(item.name)
      .catch(() => [])
      .then(found => {
        busy = false;
        item.done(found);
        next();
      });
  };
  return (name, ms) => {
    const going = asked.get(name);
    if (going !== undefined) return going;
    const answer = new Promise<string[]>(done => {
      const item = { name, done };
      waiting.push(item);
      // The caller is gone by then; a lookup it never started is dropped rather than left to run for nobody.
      setTimeout(() => {
        const at = waiting.indexOf(item);
        if (at >= 0) waiting.splice(at, 1), done([]);
      }, ms).unref();
      next();
    }).finally(() => asked.delete(name));
    asked.set(name, answer);
    return answer;
  };
}

/** This host's one place for the system's lookup, IPv4 addresses first. */
const SYSTEM = oneSystemLookup(async name => {
  const all = await lookup(name, { all: true });
  const v4 = all.filter(a => a.family === 4).map(a => a.address);
  return v4.length > 0 ? v4 : all.map(a => a.address);
});

/** The addresses the probe dials and checks: c-ares first, and where it has none, the system's lookup, which answers off
 * the computer's own cache (on a home network whose DNS drops c-ares' queries, the system answered all five live
 * addresses five times in five and c-ares none of eight times, 2026-10-14). */
export async function checkedAddresses(rs: readonly Resolver[], system: SystemLookup, name: string, ms: number): Promise<string[]> {
  const found = await addressesOf(rs, name);
  return found.length > 0 ? found : system(name, ms);
}

/** One resolver per DNS server this computer names now, asked side by side: c-ares asks a resolver's servers one after
 * another, and one that drops a query held the whole ask on a home network (2026-10-14). A try of one second leaves the
 * system's lookup the rest of the probe's time (with two, half the live runs missed an address). Made per ask, since a
 * resolver made at start keeps the first network's servers. */
export const publicPost = publicPostWith(() =>
  new Resolver().getServers().map(server => {
    const r = new Resolver({ timeout: 1_000, tries: 1 });
    r.setServers([server]);
    return r;
  }),
);

export function signInProbes(o: { post?: ProbePost; ms?: number; cap?: number } = {}): SignInProbes {
  const post = o.post ?? publicPost;
  const cap = o.cap ?? PROBES_KEPT;
  const kept = new Map<string, boolean | null>();
  const asking = new Map<string, Promise<void>>();
  const keep = (url: string, signs: boolean | null): void => {
    if (kept.size >= cap && !kept.has(url)) kept.delete(kept.keys().next().value!);
    kept.set(url, signs);
  };
  const one = (url: string): Promise<void> => {
    const going = asking.get(url);
    if (going !== undefined) return going;
    const ms = o.ms ?? PROBE_MS;
    // The whole ask is bounded, since a launch waits on it.
    let timer: NodeJS.Timeout | undefined;
    const late = new Promise<undefined>(done => (timer = setTimeout(done, ms, undefined)));
    const asked = Promise.race([post(url, ms).catch(() => undefined), late])
      .then(status => {
        keep(url, status !== undefined && status < 500 && (status < 300 || status >= 400) ? status === 401 || status === 403 : null);
      })
      .finally(() => {
        clearTimeout(timer);
        asking.delete(url);
      });
    asking.set(url, asked);
    return asked;
  };
  return {
    known: url => kept.get(url) ?? undefined,
    tried: url => kept.has(url),
    ask: async urls => void (await Promise.all(urls.filter(u => typeof kept.get(u) !== "boolean").map(one))),
    get size() {
      return kept.size;
    },
  };
}
