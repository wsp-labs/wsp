// SPDX-License-Identifier: AGPL-3.0-only
// The host's one unsigned initialize to a remote MCP address with no header: a 401 or 403 says it asks for a sign-in,
// any other answer under 500 says it does not, and neither a failure nor a redirect nor a hang past the timeout is
// kept, so the next read asks again. Answers are kept per address under a cap, and one address is asked once at a time.
import { createServer, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";
import { describe, expect, it, onTestFinished } from "vitest";
import { signInProbes } from "../src/sign-in-probe.js";

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
    const probes = signInProbes({ ms: 300 });
    const urls = ["/401", "/403", "/200", "/404", "/500", "/302", "/hang"].map(p => `${base}${p}`);
    await probes.ask(urls);
    expect(urls.map(u => probes.known(u))).toEqual([true, true, false, false, undefined, undefined, undefined]);
    expect(JSON.parse(bodies[0]!)).toMatchObject({ jsonrpc: "2.0", method: "initialize" });
    // What was kept is not asked again; what was not is.
    await probes.ask(urls);
    expect(asked.filter(p => p === "/401")).toHaveLength(1);
    expect(asked.filter(p => p === "/500")).toHaveLength(2);
    expect(asked.filter(p => p === "/hang")).toHaveLength(2);
    expect(probes.size).toBe(4);
  });

  it("asks one address once while an ask of it goes, and keeps no more answers than its cap, the oldest going first", async () => {
    const { base, asked } = await answering();
    const probes = signInProbes({ cap: 2 });
    await Promise.all([probes.ask([`${base}/401`]), probes.ask([`${base}/401`])]);
    expect(asked).toEqual(["/401"]);
    await probes.ask([`${base}/200`, `${base}/403`]);
    expect(probes.size).toBe(2);
    expect(probes.known(`${base}/401`)).toBeUndefined();
    expect(probes.known(`${base}/403`)).toBe(true);
  });
});
