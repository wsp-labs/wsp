// SPDX-License-Identifier: AGPL-3.0-only
// The broker's own rules, held against node's real http client over a pipe:
// who may call, the off switch, CONNECT, an address no service claims, an
// upstream's redirect and 401, and the caller's own auth never reaching an
// upstream. Each refusal is read in the shape its client parses.
import { request, type IncomingMessage, type ServerResponse } from "node:http";
import type { PipeDoor } from "@wsp/protocol";
import { pathOf } from "../src/broker/tokens.js";
import type { BrokerService, ServiceId } from "../src/broker/types.js";
import { afterEach, describe, expect, it } from "vitest";
import { CONNECT_SAID, DEAD_TOKEN_SAID, NO_TOKEN_SAID, NOWHERE_SAID, OTHER_MACHINE_SAID, REVOKED_SAID, UNHELD_SAID, connectorSaid } from "../src/broker/refuse.js";
import { CLAUDE, DOCTOR_OP, ENDED_TURN, GRANTS, STORAGE_HOST, OP, OTHER_TURN, OTHER_WS, REAL, TERMINAL, TURN, WS, pipePair, placeholderJwt, proxyLogin, rig, type CallOptions, type Rig } from "./broker-fixture.js";

let r: Rig;
afterEach(async () => {
  await r?.close();
});

const anthropicError = (body: string): unknown => JSON.parse(body);
/** The sentence a refusal stops Codex with: a plain-text 400, the one status codex-cli 0.155.1 does not retry. */
const codexSaid = (res: { status: number; headers: Record<string, unknown>; body: string }): string => {
  expect([res.status, res.headers["content-type"], res.headers["x-should-retry"], res.body.at(-1)]).toEqual([400, "text/plain; charset=utf-8", "false", "\n"]);
  return res.body.slice(0, -1);
};

describe("who may call", () => {
  it("refuses a call with no token, in Anthropic's shape, and sends nothing upstream", async () => {
    r = await rig();
    const res = await r.call({ method: "POST", path: "/v1/messages?beta=true", headers: CLAUDE, body: "{}" });
    expect(res.status).toBe(403);
    expect(res.headers["x-should-retry"]).toBe("false");
    expect(anthropicError(res.body)).toEqual({ type: "error", error: { type: "permission_error", message: NO_TOKEN_SAID } });
    expect(r.up.seen).toEqual([]);
    expect(r.served).toEqual([]);
  });

  it("refuses a token of another workspace and a token whose turn ended", async () => {
    r = await rig();
    const other = await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": OTHER_TURN }, body: "{}" });
    expect(other.status).toBe(403);
    expect(JSON.parse(other.body).error.message).toBe(OTHER_MACHINE_SAID);
    const ended = await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, authorization: `Bearer ${ENDED_TURN}` }, body: "{}" });
    expect(ended.status).toBe(403);
    expect(JSON.parse(ended.body).error.message).toBe(DEAD_TOKEN_SAID);
    expect(r.up.seen).toEqual([]);
    expect(r.served).toEqual([]);
  });

  it("refuses a revoked token, and after the off switch every call from that machine gets the revoked sentence", async () => {
    r = await rig();
    r.revokedTokens.push(TURN);
    const revokedToken = await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN }, body: "{}" });
    expect(revokedToken.status).toBe(403);
    expect(JSON.parse(revokedToken.body).error.message).toBe(DEAD_TOKEN_SAID);
    r.revokedTokens.length = 0;
    await r.broker.revoke(WS);
    expect(r.broker.revoked(WS)).toBe(true);
    expect(r.revokedTokens.sort()).toEqual([DOCTOR_OP, OP, TERMINAL, TURN].sort());
    const afterOff = await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN }, body: "{}" });
    expect(afterOff.status).toBe(403);
    expect(afterOff.headers["x-should-retry"]).toBe("false");
    expect(JSON.parse(afterOff.body).error.message).toBe(REVOKED_SAID);
    expect(REVOKED_SAID).toBe("wsp: this machine was revoked on your computer. Start a new thread.");
    expect(r.up.seen).toEqual([]);
    expect(r.broker.revoked(OTHER_WS)).toBe(false);
  });

  it("cuts a call under way when its machine is revoked", async () => {
    r = await rig({
      services: [
        {
          id: "anthropic",
          claims: () => true,
          serve: async ({ res }) => {
            res.writeHead(200, { "content-type": "text/event-stream" });
            res.write("event: ping\n\n");
          },
        },
      ],
    });
    const [host, client] = pipePair();
    r.broker.take(WS, { op: "pipe.open", door: "http", heldMs: 0 }, host);
    const ended = new Promise<string>(resolve => {
      const req = request({ createConnection: () => client as never, method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN } }, res => {
        res.once("data", () => void r.broker.revoke(WS));
        res.on("error", () => resolve("cut"));
        res.on("aborted", () => resolve("cut"));
        res.on("end", () => resolve("ended"));
      });
      req.on("error", () => resolve("cut"));
      req.end("{}");
    });
    expect(await ended).toBe("cut");
  });

  it("answers each client's refusal in its own shape", async () => {
    r = await rig();
    r.revokedTokens.push(TURN);
    const codex = await r.call({ method: "POST", path: "/backend-api/codex/responses", headers: { authorization: `Bearer ${placeholderJwt(TURN)}` }, body: "{}" });
    expect(codexSaid(codex)).toBe(DEAD_TOKEN_SAID);
    const claude = await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN }, body: "{}" });
    expect([claude.status, claude.headers["x-should-retry"]]).toEqual([403, "false"]);
    expect(anthropicError(claude.body)).toEqual({ type: "error", error: { type: "permission_error", message: DEAD_TOKEN_SAID } });
    const git = await r.call({ path: "http://github.com/acme/lab.git/info/refs?service=git-upload-pack", headers: { host: "github.com", "proxy-authorization": proxyLogin(TURN) } });
    expect([git.status, git.headers["x-should-retry"]]).toEqual([403, "false"]);
    expect(git.headers["content-type"]).toMatch(/^text\/plain/);
    expect(git.body).toBe(`${DEAD_TOKEN_SAID}\n`);
    const rest = await r.call({ door: "gh", path: "/repos/acme/lab", headers: { host: "api.github.com", authorization: `token ${TURN}` } });
    expect([rest.status, rest.headers["x-should-retry"]]).toEqual([403, "false"]);
    expect(JSON.parse(rest.body).message).toBe(DEAD_TOKEN_SAID);
    const graphql = await r.call({ door: "gh", method: "POST", path: "/graphql", headers: { host: "api.github.com", authorization: `token ${TURN}` }, body: '{"query":"{viewer{login}}"}' });
    expect([graphql.status, graphql.headers["x-should-retry"]]).toEqual([200, "false"]);
    expect(JSON.parse(graphql.body)).toEqual({ errors: [{ type: "FORBIDDEN", message: DEAD_TOKEN_SAID }] });
    expect(r.up.seen).toEqual([]);
  });

  it("reads the token where each client carries it and hands the service its grant", async () => {
    r = await rig();
    const calls: Array<{ kind: string; o: CallOptions }> = [
      { kind: "Claude key", o: { method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN }, body: "{}" } },
      { kind: "Claude OAuth", o: { method: "POST", path: "/v1/messages", headers: { ...CLAUDE, authorization: `Bearer ${TURN}` }, body: "{}" } },
      { kind: "Codex JWT", o: { method: "POST", path: "/backend-api/codex/responses", headers: { authorization: `Bearer ${placeholderJwt(TURN)}` }, body: "{}" } },
      { kind: "Codex key", o: { method: "POST", path: "/v1/responses", headers: { authorization: `Bearer sk-wsp-${TURN}` }, body: "{}" } },
      { kind: "git op", o: { path: "http://github.com/acme/lab.git/info/refs?service=git-receive-pack", headers: { host: "github.com", "proxy-authorization": proxyLogin(OP) } } },
      { kind: "gh terminal", o: { door: "gh", path: "/user", headers: { host: "api.github.com", authorization: `token ${TERMINAL}` } } },
    ];
    for (const c of calls) expect([c.kind, (await r.call(c.o)).status]).toEqual([c.kind, 200]);
    expect(r.served.map(s => [s.id, s.grant.kind])).toEqual([
      ["anthropic", "turn"],
      ["anthropic", "turn"],
      ["chatgpt", "turn"],
      ["openai", "turn"],
      ["git", "op"],
      ["gh", "terminal"],
    ]);
  });

  it("refuses a Codex JWT without the turn claim, and gh's token read anywhere but its own scheme", async () => {
    r = await rig();
    const noClaim = await r.call({ method: "POST", path: "/backend-api/codex/responses", headers: { authorization: `Bearer ${placeholderJwt(undefined)}` }, body: "{}" });
    expect(codexSaid(noClaim)).toBe(NO_TOKEN_SAID);
    const ghBearer = await r.call({ door: "gh", path: "/user", headers: { host: "api.github.com", authorization: `Bearer ${TURN}` } });
    expect(JSON.parse(ghBearer.body).message).toBe(NO_TOKEN_SAID);
    expect(r.up.seen).toEqual([]);
  });

  it("lends a connector call the one Codex turn on the workspace, and refuses it with none or two", async () => {
    r = await rig();
    const one = await r.call({ method: "POST", path: "/backend-api/ps/mcp", body: "{}" });
    expect(one.status).toBe(200);
    expect(r.served.at(-1)?.grant).toEqual(GRANTS[TURN]);
    r.live.push({ kind: "turn", workspaceId: WS, threadId: "th-acme-4", agent: "claude" }, { kind: "terminal", workspaceId: WS, threadId: "th-acme-1" });
    expect((await r.call({ method: "POST", path: "/backend-api/ps/mcp", body: "{}" })).status).toBe(200);
    r.live.push({ kind: "turn", workspaceId: WS, threadId: "th-acme-3", agent: "codex" });
    const two = await r.call({ method: "POST", path: "/backend-api/ps/mcp", body: "{}" });
    expect(codexSaid(two)).toBe(connectorSaid(2));
    r.live.length = 0;
    const none = await r.call({ method: "POST", path: "/backend-api/ps/mcp", body: "{}" });
    expect(codexSaid(none)).toBe(connectorSaid(0));
    r.live.push({ ...GRANTS[TURN]!, agent: "codex" });
    const dotted = await r.call({ method: "POST", path: "/backend-api/ps/mcp/../../codex/responses", body: "{}" });
    expect(codexSaid(dotted)).toBe(NO_TOKEN_SAID);
    const elsewhere = await r.call({ method: "POST", path: "/backend-api/codex/responses", body: "{}" });
    expect(codexSaid(elsewhere)).toBe(NO_TOKEN_SAID);
    expect(r.up.seen).toHaveLength(2);
  });

  it("reads Codex's refresh token from its body, only under 16 KB, and the service still reads the body", async () => {
    r = await rig();
    const body = JSON.stringify({ client_id: "app_acme", grant_type: "refresh_token", refresh_token: `rt_wsp_${TURN}` });
    const ok = await r.call({ method: "POST", path: "/oauth/token", headers: { "content-type": "application/json" }, body });
    expect(ok.status).toBe(200);
    expect(r.served.at(-1)?.grant).toEqual(GRANTS[TURN]);
    expect(r.up.seen.at(-1)?.body).toBe(body);
    const refused = async (b: string): Promise<string> => codexSaid(await r!.call({ method: "POST", path: "/oauth/token", headers: { "content-type": "application/json" }, body: b }));
    expect(await refused(JSON.stringify({ refresh_token: `rt_wsp_${OTHER_TURN}` }))).toBe(OTHER_MACHINE_SAID);
    expect(await refused(JSON.stringify({ refresh_token: `rt_wsp_${TURN}`, pad: "x".repeat(16 * 1024) }))).toBe(NO_TOKEN_SAID);
    expect(await refused(JSON.stringify({ refresh_token: TURN }))).toBe(NO_TOKEN_SAID);
    expect(await refused(`refresh_token=rt_wsp_${TURN}`)).toBe(NO_TOKEN_SAID);
    const get = await r.call({ path: "/oauth/token", headers: { "content-type": "application/json" } });
    expect(codexSaid(get)).toBe(NO_TOKEN_SAID);
    expect(r.up.seen).toHaveLength(1);
  });

  it("fetches a storage host on the gh door with no token and no credential while a turn or terminal lives", async () => {
    r = await rig();
    const fetch = (o: { method?: string; host?: string } = {}) => r!.call({ door: "gh", method: o.method ?? "GET", path: "/acme/lab/blob?sig=upstream-signed", headers: { host: o.host ?? STORAGE_HOST } });
    const ok = await fetch();
    expect(ok.status).toBe(200);
    expect(r.up.seen.at(-1)?.headers.authorization).toBeUndefined();
    expect(r.served.at(-1)?.grant).toEqual(GRANTS[TURN]);
    expect(JSON.parse((await fetch({ method: "POST" })).body).message).toBe(NO_TOKEN_SAID);
    expect(JSON.parse((await fetch({ host: "api.github.com" })).body).message).toBe(NO_TOKEN_SAID);
    expect(codexSaid(await r.call({ path: "/acme/lab/blob", headers: { host: STORAGE_HOST } }))).toBe(NO_TOKEN_SAID);
    r.live.splice(0, r.live.length, { kind: "op", workspaceId: WS, op: "git.push" });
    expect(JSON.parse((await fetch()).body).message).toBe(NO_TOKEN_SAID);
    r.live.splice(0, r.live.length, { kind: "terminal", workspaceId: WS, threadId: "th-acme-1" }, { kind: "turn", workspaceId: WS, threadId: "th-acme-5" });
    expect((await fetch()).status).toBe(200);
    expect(r.served.at(-1)?.grant).toEqual({ kind: "terminal", workspaceId: WS });
    await r.broker.revoke(WS);
    expect(JSON.parse((await fetch()).body).message).toBe(REVOKED_SAID);
    expect(r.up.seen).toHaveLength(2);
  });

  it("takes the doctor's op grant like any op token", async () => {
    r = await rig();
    expect((await r.call({ door: "gh", path: "/user", headers: { host: "api.github.com", authorization: `token ${DOCTOR_OP}` } })).status).toBe(200);
    expect(r.served.at(-1)?.grant).toEqual({ kind: "op", workspaceId: WS, op: "doctor" });
  });

  it("tells the waits port once per pipe how long the daemon held it", async () => {
    r = await rig();
    await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN }, body: "{}", heldMs: 14 * 60_000 });
    await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN }, body: "{}" });
    expect(r.waits).toEqual([{ grant: GRANTS[TURN], heldMs: 14 * 60_000 }]);
  });
});

describe("what the policy refuses for every call", () => {
  it("refuses CONNECT with no service asked and the connection closed", async () => {
    r = await rig();
    const [host, client] = pipePair();
    r.broker.take(WS, { op: "pipe.open", door: "http", heldMs: 0 }, host);
    const answer = await new Promise<{ status: number; body: string; closed: boolean }>((resolve, reject) => {
      const req = request({ createConnection: () => client as never, method: "CONNECT", path: "github.com:443", headers: { host: "github.com:443", "proxy-authorization": proxyLogin(TURN) } });
      req.on("response", res => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () => resolve({ status: res.statusCode ?? 0, body: Buffer.concat(chunks).toString("utf8"), closed: true }));
      });
      // node's client hands any answer to CONNECT over as a socket, a refusal too, with its body still on it.
      req.on("connect", (res, socket, head) => {
        const chunks: Buffer[] = [head];
        socket.on("data", (c: Buffer) => chunks.push(c));
        socket.on("end", () => resolve({ status: res.statusCode ?? 0, body: Buffer.concat(chunks).toString("utf8"), closed: true }));
      });
      req.on("error", reject);
      req.end();
    });
    expect(answer.status).toBe(403);
    expect(answer.body).toBe(`${CONNECT_SAID}\n`);
    expect(answer.closed).toBe(true);
    expect(r.served).toEqual([]);
    expect(r.up.seen).toEqual([]);
  });

  it("refuses an address no service claims: 404 for a path on the HTTP door, 403 for another host", async () => {
    r = await rig();
    const path = await r.call({ method: "POST", path: "/v1/complete", headers: { ...CLAUDE, "x-api-key": TURN, "anthropic-version": "2023-06-01" }, body: "{}" });
    expect(path.status).toBe(404);
    expect(JSON.parse(path.body)).toEqual({ type: "error", error: { type: "not_found_error", message: NOWHERE_SAID } });
    const host = await r.call({ path: "http://evil.example/acme/lab.git/info/refs?service=git-upload-pack", headers: { host: "evil.example", "proxy-authorization": proxyLogin(TURN) } });
    expect(host.status).toBe(403);
    expect(host.body).toBe(`${NOWHERE_SAID}\n`);
    const ghHost = await r.call({ door: "gh", path: "/anything", headers: { host: "evil.example", authorization: `token ${TURN}` } });
    expect(ghHost.status).toBe(403);
    expect(JSON.parse(ghHost.body).message).toBe(NOWHERE_SAID);
    expect(r.up.seen).toEqual([]);
  });

  it("hands an upstream's redirect back as it came and follows none of it", async () => {
    r = await rig();
    const res = await r.call({ door: "gh", path: "/repos/acme/lab/actions/runs/1/logs/redirect", headers: { host: "api.github.com", authorization: `token ${TURN}` } });
    expect(res.status).toBe(302);
    expect(res.headers.location).toBe("https://objects.githubusercontent.com/acme/lab/blob?sig=upstream-signed");
    expect(r.up.seen).toHaveLength(1);
  });

  it("turns an upstream 401 into a 403 that stops the agent, and keeps the gh token read for its five minutes", async () => {
    r = await rig();
    const first = await r.call({ door: "gh", path: "/user/unauthorized", headers: { host: "api.github.com", authorization: `token ${TURN}` } });
    expect(first.status).toBe(403);
    expect(first.headers["x-should-retry"]).toBe("false");
    expect(first.headers["www-authenticate"]).toBeUndefined();
    const tokenReads = (): number => r.gh.runs.filter(a => a[1] === "token").length;
    expect(tokenReads()).toBe(1);
    await r.call({ door: "gh", path: "/user", headers: { host: "api.github.com", authorization: `token ${TURN}` } });
    expect(tokenReads()).toBe(1);
  });

  it("sends every 401 a service writes as a 403 with x-should-retry false, whatever form its headers take", async () => {
    const ways: Record<string, (res: ServerResponse) => void> = {
      "flat array, upstream says retry": res => res.writeHead(401, ["www-authenticate", "Bearer", "x-should-retry", "true", "content-type", "text/plain"]).end("no"),
      "flat array, no retry word": res => res.writeHead(401, ["www-authenticate", "Bearer"]).end("no"),
      "object, upstream says retry": res => res.writeHead(401, { "www-authenticate": "Bearer", "x-should-retry": "true" }).end("no"),
      "set headers, then status": res => {
        res.setHeader("x-should-retry", "true");
        res.setHeader("www-authenticate", "Bearer");
        res.statusCode = 401;
        res.end("no");
      },
      "a 407 as a flat array": res => res.writeHead(407, ["proxy-authenticate", "Basic", "x-should-retry", "true"]).end("no"),
    };
    r = await rig({ services: [{ id: "anthropic", claims: () => true, serve: async ({ req, res }) => ways[req.headers["x-way"] as string]!(res) }] });
    for (const way of Object.keys(ways)) {
      const got = await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN, "x-way": way }, body: "{}" });
      expect([way, got.status, got.headers["x-should-retry"], got.headers["www-authenticate"], got.headers["proxy-authenticate"]]).toEqual([way, 403, "false", undefined, undefined]);
    }
  });

  it("leaves a service every way node reads headers, the caller's auth gone from each", async () => {
    const read: Array<Record<string, string[] | undefined>> = [];
    r = await rig({
      services: [
        {
          id: "chatgpt",
          claims: () => true,
          serve: async ({ req, res }) => {
            read.push(req.headersDistinct);
            req.resume();
            req.on("end", () => res.end("{}"));
          },
        },
      ],
    });
    await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, "x-api-key": TURN, cookie: "session=caller-cookie", "x-kept": "yes" }, body: "{}" });
    await r.call({ method: "POST", path: "/oauth/token", headers: { "content-type": "application/json", cookie: "session=caller-cookie" }, body: JSON.stringify({ refresh_token: `rt_wsp_${TURN}` }) });
    expect(read).toHaveLength(2);
    expect(read[0]!["x-kept"]).toEqual(["yes"]);
    expect(read[1]!["content-type"]).toEqual(["application/json"]);
    for (const distinct of read) for (const name of ["x-api-key", "cookie", "authorization"]) expect(distinct[name]).toBeUndefined();
  });

  it("drops the caller's auth headers and cookies before a service or an upstream sees the call", async () => {
    r = await rig();
    const caller = { cookie: "session=caller-cookie", cookie2: "$Version=1", "x-api-key": TURN, authorization: `Bearer ${TURN}`, "proxy-authorization": proxyLogin(TURN) };
    await r.call({ method: "POST", path: "/v1/messages", headers: { ...CLAUDE, ...caller }, body: "{}" });
    await r.call({ path: "http://github.com/acme/lab.git/info/refs?service=git-upload-pack", headers: { host: "github.com", cookie: "session=caller-cookie", "proxy-authorization": proxyLogin(TURN) } });
    await r.call({ door: "gh", path: "/user", headers: { host: "api.github.com", cookie: "session=caller-cookie", authorization: `token ${TURN}` } });
    expect(r.served).toHaveLength(3);
    for (const s of r.served) {
      for (const name of ["authorization", "x-api-key", "proxy-authorization", "cookie", "cookie2"]) expect(s.headers[name]).toBeUndefined();
      expect(s.raw.filter((_, i) => i % 2 === 0).map(n => n.toLowerCase())).not.toEqual(expect.arrayContaining(["authorization", "cookie"]));
    }
    expect(r.up.seen).toHaveLength(3);
    const upstreamText = JSON.stringify(r.up.seen.map(s => s.headers));
    for (const caller of [TURN, "caller-cookie", proxyLogin(TURN)]) expect(upstreamText).not.toContain(caller);
    expect(r.up.seen[0]!.headers["x-api-key"]).toBe(REAL.anthropic);
    expect(r.up.seen[0]!.headers.cookie).toBeUndefined();
  });

  it("passes an upgrade the service accepts, frames both ways, and refuses one with no token", async () => {
    r = await rig();
    const upgrade = (headers: Record<string, string>) =>
      new Promise<{ status: number; echo?: string }>((resolve, reject) => {
        const [host, client] = pipePair();
        r.broker.take(WS, { op: "pipe.open", door: "http", heldMs: 0 }, host);
        const req = request({ createConnection: () => client as never, path: "/backend-api/codex/responses", headers: { host: "127.0.0.1:7071", connection: "Upgrade", upgrade: "websocket", ...headers } });
        req.on("upgrade", (res, socket) => {
          socket.once("data", (d: Buffer) => resolve({ status: res.statusCode ?? 0, echo: d.toString("utf8") }));
          socket.write("frame-1");
        });
        req.on("response", res => {
          res.resume();
          resolve({ status: res.statusCode ?? 0 });
        });
        req.on("error", reject);
        req.end();
      });
    expect(await upgrade({ authorization: `Bearer ${placeholderJwt(TURN)}` })).toEqual({ status: 101, echo: "echo:frame-1" });
    expect(r.served.at(-1)?.id).toBe("chatgpt");
    expect(await upgrade({})).toEqual({ status: 400 });
    expect(r.served).toHaveLength(1);
  });

  it("keeps the bytes a client sends while its upgrade is being admitted, for the service to read", async () => {
    // A workspace read that takes a while is the window: the client's first frame lands while the call is admitted.
    const workspace = async () => {
      await new Promise(res => setTimeout(res, 50));
      return { agent: "codex" as const, repo: { owner: "acme", name: "lab", id: 7 }, trackers: [] };
    };
    r = await rig({ workspace });
    const [host, client] = pipePair();
    r.broker.take(WS, { op: "pipe.open", door: "http", heldMs: 0 }, host);
    const got = new Promise<string>(resolve => {
      let seen = "";
      client.on("data", (d: Buffer) => {
        seen += d.toString("utf8");
        if (seen.includes("echo:early")) resolve(seen);
      });
    });
    client.write(`GET /backend-api/codex/responses HTTP/1.1\r\nHost: 127.0.0.1:7071\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nAuthorization: Bearer ${placeholderJwt(TURN)}\r\n\r\n`);
    await new Promise(res => setImmediate(res));
    client.write("early");
    expect(await got).toMatch(/^HTTP\/1\.1 101 [^]*echo:early$/);
  });
});

describe("a service that throws stops its client with what failed, never with a wait", () => {
  const said = (call: string): string => `wsp: your computer failed on its side of this ${call} (ERR_INVALID_HTTP_TOKEN) and would fail the same way on every try, so it stopped the call.`;
  /** A service whose call throws the way node's client does on a header name with a space, before anything is written. */
  const throwing = (id: ServiceId, door: PipeDoor, claims: (path: string, req: IncomingMessage) => boolean): BrokerService => {
    const fault = (): never => {
      request({ host: "127.0.0.1", port: 9, headers: { "x team": "lab" } });
      throw new Error("node took the header name");
    };
    return { id, claims: (req, d) => d === door && claims(pathOf(req.url), req), serve: async () => fault(), upgrade: async () => fault() };
  };
  const services = [
    throwing("anthropic", "http", (_path, req) => req.headers["anthropic-version"] !== undefined),
    throwing("chatgpt", "http", path => path.startsWith("/backend-api/")),
    throwing("openai", "http", path => path.startsWith("/v1/")),
    throwing("git", "http", (_path, req) => (req.url ?? "").startsWith("http://github.com/")),
    throwing("gh", "gh", () => true),
  ];

  it.each<[string, CallOptions, number, string, unknown]>([
    ["Claude Code", { method: "POST", path: "/v1/messages?beta=true", headers: { ...CLAUDE, "x-api-key": TURN }, body: "{}" }, 403, "application/json", { type: "error", error: { type: "permission_error", message: said("Claude model call") } }],
    ["Codex on ChatGPT", { method: "POST", path: "/backend-api/codex/responses", headers: { authorization: `Bearer ${placeholderJwt(TURN)}` }, body: "{}" }, 400, "text/plain; charset=utf-8", `${said("Codex ChatGPT call")}\n`],
    ["Codex on OpenAI", { method: "POST", path: "/v1/responses", headers: { authorization: `Bearer sk-wsp-${TURN}` }, body: "{}" }, 400, "text/plain; charset=utf-8", `${said("Codex OpenAI call")}\n`],
    ["git", { path: "http://github.com/acme/lab.git/info/refs?service=git-upload-pack", headers: { host: "github.com", "proxy-authorization": proxyLogin(TURN) } }, 403, "text/plain; charset=utf-8", `${said("git call to GitHub")}\n`],
    ["gh's REST", { door: "gh", path: "/repos/acme/lab", headers: { host: "api.github.com", authorization: `token ${TURN}` } }, 403, "application/json; charset=utf-8", { message: said("gh call to GitHub"), status: "403" }],
    ["gh's GraphQL", { door: "gh", method: "POST", path: "/graphql", headers: { host: "api.github.com", authorization: `token ${TURN}` }, body: '{"query":"{viewer{login}}"}' }, 200, "application/json; charset=utf-8", { errors: [{ type: "FORBIDDEN", message: said("gh call to GitHub") }] }],
  ])("answers %s in its own shape, naming the call and the fault, with x-should-retry false", async (_client, call, status, type, body) => {
    r = await rig({ services });
    const got = await r.call(call);
    expect([got.status, got.headers["x-should-retry"], got.headers["content-type"]]).toEqual([status, "false", type]);
    expect(type.startsWith("text/") ? got.body : JSON.parse(got.body)).toEqual(body);
    expect(got.body).not.toContain("x team");
  });

  it("answers an upgrade whose service throws before its handshake a 400, the one status codex-cli does not retry", async () => {
    r = await rig({ services });
    const [host, client] = pipePair();
    r.broker.take(WS, { op: "pipe.open", door: "http", heldMs: 0 }, host);
    const answer = await new Promise<{ status: number; retry?: string; body: string }>((resolve, reject) => {
      const req = request({ createConnection: () => client as never, path: "/backend-api/codex/responses", headers: { host: "127.0.0.1:7071", connection: "Upgrade", upgrade: "websocket", authorization: `Bearer ${placeholderJwt(TURN)}` } });
      req.on("upgrade", res => resolve({ status: res.statusCode ?? 0, body: "" }));
      req.on("response", res => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () => resolve({ status: res.statusCode ?? 0, retry: res.headers["x-should-retry"] as string, body: Buffer.concat(chunks).toString("utf8") }));
      });
      req.on("error", reject);
      req.end();
    });
    expect(answer).toEqual({ status: 400, retry: "false", body: `${said("Codex ChatGPT call")}\n` });
  });
});

describe("every refusal the broker writes stops Codex at its first answer", () => {
  const CHATGPT_ROAD = (token: string | undefined): CallOptions => ({ method: "POST", path: "/backend-api/codex/responses", headers: token === undefined ? {} : { authorization: `Bearer ${placeholderJwt(token)}` }, body: "{}" });
  const OPENAI_ROAD = (token: string | undefined, path = "/v1/responses"): CallOptions => ({ method: "POST", path, headers: token === undefined ? {} : { authorization: `Bearer sk-wsp-${token}` }, body: "{}" });

  it.each<[string, CallOptions, string]>([
    ["no token on the ChatGPT road", CHATGPT_ROAD(undefined), NO_TOKEN_SAID],
    ["no token on the OpenAI road", OPENAI_ROAD(undefined), NO_TOKEN_SAID],
    ["a dead token on the ChatGPT road", CHATGPT_ROAD(ENDED_TURN), DEAD_TOKEN_SAID],
    ["a dead token on the OpenAI road", OPENAI_ROAD(ENDED_TURN), DEAD_TOKEN_SAID],
    ["another machine's token on the ChatGPT road", CHATGPT_ROAD(OTHER_TURN), OTHER_MACHINE_SAID],
    ["another machine's token on the OpenAI road", OPENAI_ROAD(OTHER_TURN), OTHER_MACHINE_SAID],
    ["a path no service claims", OPENAI_ROAD(TURN, "/v2/responses"), NOWHERE_SAID],
  ])("%s", async (_name, call, said) => {
    r = await rig();
    expect(codexSaid(await r.call(call))).toBe(said);
    expect(r.served).toEqual([]);
    expect(r.up.seen).toEqual([]);
  });

  it("a revoked machine, on both roads", async () => {
    r = await rig();
    await r.broker.revoke(WS);
    expect(codexSaid(await r.call(CHATGPT_ROAD(TURN)))).toBe(REVOKED_SAID);
    expect(codexSaid(await r.call(OPENAI_ROAD(TURN)))).toBe(REVOKED_SAID);
    expect(r.up.seen).toEqual([]);
  });

  it("a machine the computer holds no workspace for, on both roads", async () => {
    r = await rig({ workspace: async () => undefined });
    expect(codexSaid(await r.call(CHATGPT_ROAD(TURN)))).toBe(UNHELD_SAID);
    expect(codexSaid(await r.call(OPENAI_ROAD(TURN)))).toBe(UNHELD_SAID);
    expect(r.up.seen).toEqual([]);
  });

  it("a CONNECT with no proxy login", async () => {
    r = await rig();
    const [host, client] = pipePair();
    r.broker.take(WS, { op: "pipe.open", door: "http", heldMs: 0 }, host);
    const answer = await new Promise<{ status: number; headers: Record<string, unknown>; body: string }>((resolve, reject) => {
      const req = request({ createConnection: () => client as never, method: "CONNECT", path: "chatgpt.com:443", headers: { host: "chatgpt.com:443" } });
      req.on("connect", (res, socket, head) => {
        const chunks: Buffer[] = [head];
        socket.on("data", (c: Buffer) => chunks.push(c));
        socket.on("end", () => resolve({ status: res.statusCode ?? 0, headers: res.headers, body: Buffer.concat(chunks).toString("utf8") }));
      });
      req.on("error", reject);
      req.end();
    });
    expect(codexSaid(answer)).toBe(CONNECT_SAID);
  });
});
