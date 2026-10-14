// SPDX-License-Identifier: AGPL-3.0-only
// The host's one unsigned initialize to a remote MCP address with no header: a 401 or 403 says it asks for a sign-in,
// any other answer under 500 says it does not, and neither a failure nor a redirect nor a hang past the timeout is
// kept, so the next read asks again. Answers are kept per address under a cap, and one address is asked once at a time.
// Only a public https name is asked, since the address is the agent's to write.
import { createSocket } from "node:dgram";
import { lookup, Resolver } from "node:dns/promises";
import { createServer, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";
import { describe, expect, it, onTestFinished } from "vitest";
import { addressesOf, checkedAddresses, fetchPost, oneSystemLookup, publicPost, publicPostWith, signInProbes } from "../src/sign-in-probe.js";

/** A server answering each path with the status it names, `/hang` never, and every request it got. */
async function answering(): Promise<{ base: string; asked: string[]; bodies: string[] }> {
  const asked: string[] = [];
  const bodies: string[] = [];
  const hung: ServerResponse[] = [];
  const srv = createServer((req, res) => {
    asked.push(req.url ?? "");
    let body = "";
    req.on("data", (c: Buffer) => (body += c.toString("utf8")));
    req.on("end", () => {
      bodies.push(body);
      if (req.url === "/hang") return void hung.push(res);
      const status = Number((req.url ?? "/200").slice(1).split("-")[0]);
      res.writeHead(status, status >= 300 && status < 400 ? { location: "https://login.example/authorize" } : {}).end();
    });
  });
  await new Promise<void>(done => srv.listen(0, "127.0.0.1", done));
  onTestFinished(() => {
    for (const res of hung) res.destroy();
    srv.close();
  });
  return { base: `http://127.0.0.1:${(srv.address() as AddressInfo).port}`, asked, bodies };
}

describe("asking a remote address whether it wants a sign-in", () => {
  it("keeps 401 and 403 as asking, other answers as not, and nothing for a 500, a redirect or a hang", async () => {
    const { base, asked, bodies } = await answering();
    const probes = signInProbes({ ms: 300, post: fetchPost });
    const urls = ["/401", "/403", "/200", "/404", "/500", "/302", "/hang"].map(p => `${base}${p}`);
    await probes.ask(urls);
    expect(urls.map(u => probes.known(u))).toEqual([true, true, false, false, undefined, undefined, undefined]);
    expect(JSON.parse(bodies[0]!)).toMatchObject({ jsonrpc: "2.0", method: "initialize" });
    // What was kept is not asked again; what was not is.
    await probes.ask(urls);
    expect(asked.filter(p => p === "/401")).toHaveLength(1);
    expect(asked.filter(p => p === "/500")).toHaveLength(2);
    expect(asked.filter(p => p === "/hang")).toHaveLength(2);
    expect(probes.size).toBe(7);
  });

  it("asks one address once while an ask of it goes, and keeps no more answers than its cap, the oldest going first", async () => {
    const { base, asked } = await answering();
    const probes = signInProbes({ cap: 2, post: fetchPost });
    await Promise.all([probes.ask([`${base}/401`]), probes.ask([`${base}/401`])]);
    expect(asked).toEqual(["/401"]);
    await probes.ask([`${base}/200`, `${base}/403`]);
    expect(probes.size).toBe(2);
    expect(probes.known(`${base}/401`)).toBeUndefined();
    expect(probes.known(`${base}/403`)).toBe(true);
  });

  it("ends an ask whose post never settles at its time, the name's lookup included, and keeps nothing for it", async () => {
    // The post never settles, so the ask settling at all is the bound at work.
    const probes = signInProbes({ ms: 100, post: () => new Promise<never>(() => {}) });
    await probes.ask(["https://mcp.example.com/mcp"]);
    expect(probes.known("https://mcp.example.com/mcp")).toBeUndefined();
  });

  it("asks nothing on this computer or its network, nor any address but an https name", async () => {
    const { base, asked } = await answering();
    const port = new URL(base).port;
    for (const url of [`${base}/401`, `https://127.0.0.1:${port}/401`, `https://localhost:${port}/401`, `https://[::1]:${port}/401`, "https://10.0.0.1/401", "https://169.254.169.254/401", "http://mcp.example.com/mcp"]) expect(await publicPost(url, 300), url).toBeUndefined();
    const probes = signInProbes();
    await probes.ask([`${base}/401`, `https://localhost:${port}/401`]);
    expect([probes.known(`${base}/401`), probes.known(`https://localhost:${port}/401`)]).toEqual([undefined, undefined]);
    expect(asked).toEqual([]);
  });

  it("answers nothing, and throws nothing past itself, for an address that fails as it is dialled", async () => {
    // 2001:db8::/32 is for documentation: no route reaches it, so the connect fails at once.
    const unreachable = { resolve4: async () => [], resolve6: async () => ["2001:db8::1"], cancel: () => {} } as unknown as Resolver;
    expect(await publicPostWith(() => [unreachable])("https://mcp.example.com/mcp", 2_000)).toBeUndefined();
  });

  it("dials IPv4 addresses where a name has them, and IPv6 ones only where it has none", async () => {
    const stub = (v4: string[], v6: string[]) => ({ resolve4: async () => v4, resolve6: async () => v6, cancel: () => {} }) as unknown as Resolver;
    expect(await addressesOf([stub(["192.0.2.1"], ["2001:db8::1"])], "mcp.example.com")).toEqual(["192.0.2.1"]);
    expect(await addressesOf([stub([], ["2001:db8::1"])], "mcp.example.com")).toEqual(["2001:db8::1"]);
    expect(await addressesOf([stub([], []), stub(["192.0.2.7"], [])], "mcp.example.com")).toEqual(["192.0.2.7"]);
  });

  it("runs one system lookup at a time and never a second for a name whose lookup still runs", async () => {
    const looked: string[] = [];
    let answer: (found: string[]) => void = () => {};
    const system = oneSystemLookup(name => (looked.push(name), new Promise<string[]>(done => (answer = done))));
    const first = system("mcp-a.example.com", 5_000);
    const again = system("mcp-a.example.com", 5_000);
    // Another name waits its turn, and gives up with nothing once its time runs out.
    expect(await system("mcp-b.example.com", 50)).toEqual([]);
    expect(looked).toEqual(["mcp-a.example.com"]);
    answer(["192.0.2.1"]);
    expect([await first, await again]).toEqual([["192.0.2.1"], ["192.0.2.1"]]);
    const third = system("mcp-c.example.com", 5_000);
    answer(["192.0.2.3"]);
    expect(await third).toEqual(["192.0.2.3"]);
    expect(looked).toEqual(["mcp-a.example.com", "mcp-c.example.com"]);
  });

  it("answers a name c-ares has no address for through the system's lookup, and not one it has", async () => {
    const stub = (v4: string[]) => ({ resolve4: async () => v4, resolve6: async () => [], cancel: () => {} }) as unknown as Resolver;
    const looked: string[] = [];
    const system = oneSystemLookup(async name => (looked.push(name), ["192.0.2.9"]));
    expect(await checkedAddresses([stub([])], system, "mcp.example.com", 1_000)).toEqual(["192.0.2.9"]);
    expect(await checkedAddresses([stub(["192.0.2.1"])], system, "mcp.example.org", 1_000)).toEqual(["192.0.2.1"]);
    expect(looked).toEqual(["mcp.example.com"]);
  });

  it("holds none of this host's own lookups while a name's DNS never answers", async () => {
    // The system's lookup runs two at a time for the whole process and is never stopped, so a probe stuck there would
    // hold this lookup until the resolver gave up.
    const silent = createSocket("udp4");
    await new Promise<void>(done => silent.bind(0, "127.0.0.1", done));
    const resolvers: Resolver[] = [];
    onTestFinished(() => {
      for (const r of resolvers) r.cancel();
      silent.close();
    });
    // The system's lookup behind it hangs too, as it does where the computer's own DNS never answers.
    const looked: string[] = [];
    const system = oneSystemLookup(name => (looked.push(name), new Promise<never>(() => {})));
    const post = publicPostWith(() => {
      const r = new Resolver({ timeout: 100, tries: 1 });
      r.setServers([`127.0.0.1:${silent.address().port}`]);
      resolvers.push(r);
      return [r];
    }, system);
    const probes = signInProbes({ ms: 200, post });
    const names = ["a", "b", "c", "d"].map(n => `https://mcp-${n}.example.com/mcp`);
    await probes.ask(names);
    await probes.ask(names);
    expect(resolvers).toHaveLength(8);
    expect(looked).toEqual(["mcp-a.example.com"]);
    expect(await Promise.race([lookup("localhost").then(() => "answered"), new Promise(done => setTimeout(done, 1_000, "waited"))])).toBe("answered");
  });
});
