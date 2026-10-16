// SPDX-License-Identifier: AGPL-3.0-only
import { readFileSync } from "node:fs";
import { createServer, request, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";
import { gzipSync } from "node:zlib";
import { afterEach, describe, expect, it } from "vitest";
import { retryLine } from "@wsp/protocol";
import { anthropicService, type AnthropicContext, type AnthropicCredential } from "../src/anthropic/service.js";
import { writeRefusal } from "../src/broker/refuse.js";

interface Recorded { method: string; url: string; rawHeaders: string[]; body: string }
const RECORDED = JSON.parse(readFileSync(new URL("./anthropic-request.json", import.meta.url), "utf8")) as { oauth: Recorded; key: Recorded };
const COMPUTER = "acme's MacBook Pro";
const HOP = new Set(["connection", "keep-alive", "host"]);

interface Seen { method: string; url: string; rawHeaders: string[]; body: Buffer }
interface Reply { status: number; headers: string[]; chunks: Buffer[]; clientSaw?: Promise<void> }

const servers: Server[] = [];
afterEach(async () => {
  await Promise.all(servers.splice(0).map(server => new Promise(done => {
    server.closeAllConnections();
    server.close(done);
  })));
});

async function listen(handler: (req: IncomingMessage, res: ServerResponse) => void): Promise<string> {
  const server = createServer(handler);
  servers.push(server);
  await new Promise<void>(done => server.listen(0, "127.0.0.1", done));
  return `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
}

/** An upstream that keeps what reached it and answers with the reply given, chunk by chunk, holding the second
 * chunk until the client has the first so a buffered pass would hang instead of passing. */
async function upstream(reply: Reply): Promise<{ url: string; seen: Seen[] }> {
  const seen: Seen[] = [];
  const url = await listen((req, res) => {
    const body: Buffer[] = [];
    req.on("data", chunk => body.push(chunk));
    req.on("end", async () => {
      seen.push({ method: req.method!, url: req.url!, rawHeaders: req.rawHeaders, body: Buffer.concat(body) });
      res.writeHead(reply.status, reply.headers);
      const [first, ...rest] = reply.chunks;
      if (first !== undefined) res.write(first);
      if (reply.clientSaw !== undefined) await reply.clientSaw;
      for (const chunk of rest) res.write(chunk);
      res.end();
    });
  });
  return { url, seen };
}

/** The broker's side, as B1 hands a request over: the credential port and an Anthropic-shaped refusal that sets
 * nothing but the status, so whatever else a refusal carries is the service's own. */
function context(credential: AnthropicCredential | undefined): AnthropicContext {
  return {
    credential: async () => credential,
    refuse(res, status, said) {
      res.writeHead(status, { "content-type": "application/json" });
      res.end(JSON.stringify({ type: "error", error: { type: "permission_error", message: said } }));
    },
    retrying() {},
  };
}

async function door(upstreamUrl: string, credential: AnthropicCredential | undefined): Promise<string> {
  const service = anthropicService({ upstream: upstreamUrl, computer: () => COMPUTER });
  return listen((req, res) => void service.serve({ req, res, grant: { workspaceId: "ws_lab", threadId: "th_lab" } }, context(credential)));
}

interface Got { status: number; rawHeaders: string[]; body: Buffer }

/** Sends a recorded request with its own headers, case and order kept, as Claude Code sent it. */
function send(doorUrl: string, recorded: Recorded, onFirst: () => void = () => {}): Promise<Got> {
  const { port } = new URL(doorUrl);
  return new Promise((resolve, reject) => {
    const req = request({ host: "127.0.0.1", port, method: recorded.method, path: recorded.url, headers: recorded.rawHeaders, setHost: false }, res => {
      const body: Buffer[] = [];
      res.on("data", chunk => {
        body.push(chunk);
        onFirst();
      });
      res.on("end", () => resolve({ status: res.statusCode!, rawHeaders: res.rawHeaders, body: Buffer.concat(body) }));
    });
    req.on("error", reject);
    req.end(recorded.body);
  });
}

const pairs = (raw: readonly string[]): [string, string][] => raw.flatMap((name, i) => (i % 2 === 0 ? [[name, raw[i + 1]!] as [string, string]] : []));
const endToEnd = (raw: readonly string[]) => pairs(raw).filter(([name]) => !HOP.has(name.toLowerCase()));

const SSE = [
  'event: message_start\ndata: {"type":"message_start","message":{"id":"msg_01","type":"message","role":"assistant","model":"claude-opus-5-5","content":[],"stop_reason":null,"usage":{"input_tokens":12,"output_tokens":1}}}\n\n',
  'event: content_block_delta\ndata: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi é"}}\n\n',
  'event: message_stop\ndata: {"type":"message_stop"}\n\n',
].map(chunk => Buffer.from(chunk));
const STREAM_HEADERS = [
  "Content-Type", "text/event-stream; charset=utf-8",
  "Cache-Control", "no-cache",
  "request-id", "req_011CTest",
  "anthropic-ratelimit-unified-status", "allowed",
  "anthropic-ratelimit-unified-5h-utilization", "0.42",
  "anthropic-ratelimit-unified-reset", "1791925200",
  "retry-after", "7",
  "x-should-retry", "true",
];

describe("the Anthropic service passes Claude Code's calls through with only the credential swapped", () => {
  const cases: { name: string; recorded: Recorded; credential: AnthropicCredential; header: [string, string][]; extra?: [string, string][] }[] = [
    { name: "a setup-token or claude.ai login as Bearer", recorded: RECORDED.oauth, credential: { kind: "token", token: "token-for-test", expiresAt: Date.now() + 3_600_000 }, header: [["Authorization", "Bearer token-for-test"]] },
    { name: "an API key as x-api-key", recorded: RECORDED.key, credential: { kind: "key", key: "key-for-test" }, header: [["x-api-key", "key-for-test"]] },
    { name: "an apiKeyHelper's answer as both", recorded: RECORDED.key, credential: { kind: "helper", value: "helper-for-test" }, header: [["x-api-key", "helper-for-test"], ["Authorization", "Bearer helper-for-test"]] },
    {
      name: "a gateway's token as Bearer with its headers",
      recorded: RECORDED.oauth,
      credential: { kind: "gateway", url: "", token: "gateway-for-test", headers: { "x-litellm-team": "lab" } },
      header: [["Authorization", "Bearer gateway-for-test"]],
      extra: [["x-litellm-team", "lab"]],
    },
  ];

  it.each(cases)("a recorded 2.1.296 request and a streamed reply pass byte for byte but for $name", async ({ recorded, credential, header, extra }) => {
    let clientHas!: () => void;
    const up = await upstream({ status: 200, headers: STREAM_HEADERS, chunks: SSE, clientSaw: new Promise<void>(done => (clientHas = done)) });
    const gatewayed: AnthropicCredential = credential.kind === "gateway" ? { ...credential, url: `${up.url}/anthropic/` } : credential;
    const got = await send(await door(up.url, gatewayed), recorded, clientHas);

    expect(up.seen).toHaveLength(1);
    const [seen] = up.seen;
    expect(seen!.method).toBe(recorded.method);
    expect(seen!.url).toBe(credential.kind === "gateway" ? `/anthropic${recorded.url}` : recorded.url);
    expect(seen!.body.equals(Buffer.from(recorded.body))).toBe(true);
    const credentialNames = new Set(["authorization", "x-api-key"]);
    const theirs = endToEnd(recorded.rawHeaders);
    const ours = endToEnd(seen!.rawHeaders);
    expect(ours.filter(([name]) => !credentialNames.has(name.toLowerCase()) && !extra?.some(([e]) => e === name))).toEqual(theirs.filter(([name]) => !credentialNames.has(name.toLowerCase())));
    expect(ours.filter(([name]) => credentialNames.has(name.toLowerCase()))).toEqual(header);
    for (const pair of extra ?? []) expect(ours).toContainEqual(pair);
    expect(ours.some(([, value]) => value.includes("wsp-turn-placeholder"))).toBe(false);
    expect(pairs(seen!.rawHeaders).find(([name]) => name.toLowerCase() === "host")?.[1]).toBe(new URL(up.url).host);

    expect(got.status).toBe(200);
    expect(endToEnd(got.rawHeaders).filter(([name]) => !["date", "transfer-encoding"].includes(name.toLowerCase()))).toEqual(pairs(STREAM_HEADERS));
    expect(got.body.equals(Buffer.concat(SSE))).toBe(true);
  });

  it("drops every auth header and cookie the caller sent, whatever kind of placeholder it carries", async () => {
    const up = await upstream({ status: 200, headers: [], chunks: [] });
    const rawHeaders = [...RECORDED.oauth.rawHeaders, "x-api-key", "wsp-turn-placeholder", "Cookie", "session=wsp-turn-placeholder", "Proxy-Authorization", "Basic d3NwOnBsYWNlaG9sZGVy"];
    await send(await door(up.url, { kind: "key", key: "key-for-test" }), { ...RECORDED.oauth, rawHeaders });
    const seen = pairs(up.seen[0]!.rawHeaders);
    expect(seen.filter(([name]) => ["authorization", "x-api-key", "cookie", "proxy-authorization"].includes(name.toLowerCase()))).toEqual([["x-api-key", "key-for-test"]]);
  });

  it("sends a gateway whose key rides in its custom headers with no Authorization", async () => {
    const up = await upstream({ status: 200, headers: [], chunks: [] });
    await send(await door(up.url, { kind: "gateway", url: up.url, token: "", headers: { "x-litellm-api-key": "sk-for-test" } }), RECORDED.oauth);
    const seen = pairs(up.seen[0]!.rawHeaders);
    expect(seen.filter(([name]) => ["authorization", "x-api-key", "x-litellm-api-key"].includes(name.toLowerCase()))).toEqual([["x-litellm-api-key", "sk-for-test"]]);
  });

  it("stops a gateway call whose custom header name the reader refused, naming the header, and calls no gateway", async () => {
    const up = await upstream({ status: 200, headers: [], chunks: [] });
    const got = await send(await door(up.url, { kind: "gateway", url: up.url, token: "gateway-for-test", headers: { "x team": "lab" }, invalidHeader: 'header "x team"' }), RECORDED.oauth);
    expect(got.status).toBe(403);
    expect(pairs(got.rawHeaders)).toContainEqual(["x-should-retry", "false"]);
    expect(JSON.parse(got.body.toString()).error.message).toBe(`wsp: header "x team" in ANTHROPIC_CUSTOM_HEADERS on ${COMPUTER} has a name HTTP does not allow, so your gateway was not called. Fix that line there, which Claude Code refuses too.`);
    expect(up.seen).toEqual([]);
  });

  it("passes a compressed body as the upstream's bytes, never decoded", async () => {
    const models = gzipSync(JSON.stringify({ data: [{ id: "claude-opus-5-5", type: "model" }] }));
    const up = await upstream({ status: 200, headers: ["Content-Type", "application/json", "Content-Encoding", "gzip", "Content-Length", String(models.length)], chunks: [models] });
    const got = await send(await door(up.url, { kind: "key", key: "key-for-test" }), { ...RECORDED.key, method: "GET", url: "/v1/models?limit=1000", body: "", rawHeaders: RECORDED.key.rawHeaders.map((v, i, all) => (all[i - 1] === "Content-Length" ? "0" : v)) });
    expect(got.status).toBe(200);
    expect(got.body.equals(models)).toBe(true);
    expect(pairs(got.rawHeaders)).toContainEqual(["Content-Encoding", "gzip"]);
  });

  it("answers 404 for any path but the four, and never reaches the upstream", async () => {
    const up = await upstream({ status: 200, headers: [], chunks: [] });
    const doorUrl = await door(up.url, { kind: "key", key: "key-for-test" });
    for (const [method, url] of [["POST", "/v1/complete"], ["GET", "/v1/organizations/usage"], ["POST", "/api/oauth/claude_cli/create_api_key"], ["GET", "/v1/messages"], ["POST", "/v1/messages/batches"]] as const) {
      const got = await send(doorUrl, { ...RECORDED.key, method, url });
      expect(got.status, `${method} ${url}`).toBe(404);
      expect(JSON.parse(got.body.toString()).error.type).toBe("not_found_error");
    }
    expect(up.seen).toEqual([]);
    for (const [method, url] of [["HEAD", "/api/hello"], ["POST", "/v1/messages/count_tokens?beta=true"], ["GET", "/v1/models"]] as const) {
      const got = await send(doorUrl, { ...RECORDED.key, method, url, body: "", rawHeaders: RECORDED.key.rawHeaders.map((v, i, all) => (all[i - 1] === "Content-Length" ? "0" : v)) });
      expect(got.status, `${method} ${url}`).toBe(200);
    }
    expect(up.seen.map(seen => `${seen.method} ${seen.url}`)).toEqual(["HEAD /api/hello", "POST /v1/messages/count_tokens?beta=true", "GET /v1/models"]);
  });

  it("claims Claude's four calls on the HTTP door and nothing else, git's proxy requests included", () => {
    const service = anthropicService();
    const asked = (headers: Record<string, string>, call = "POST /v1/messages?beta=true") => {
      const [method, url] = call.split(" ");
      return { headers, method, url } as IncomingMessage;
    };
    const version = { "anthropic-version": "2023-06-01" };
    for (const call of ["POST /v1/messages?beta=true", "POST /v1/messages/count_tokens?beta=true", "GET /v1/models?limit=1000", "HEAD /api/hello"]) {
      expect(service.claims(asked({}, call), "http", "claude"), call).toBe(true);
    }
    expect(service.claims(asked(version), "http", undefined)).toBe(true);
    for (const call of ["GET http://github.com/acme/lab.git/info/refs?service=git-upload-pack", "POST http://github.com/acme/lab.git/git-receive-pack", "GET /v1/organizations/usage", "POST /v1/complete"]) {
      expect(service.claims(asked(version, call), "http", "claude"), call).toBe(false);
    }
    expect(service.claims(asked({}), "http", undefined)).toBe(false);
    expect(service.claims(asked(version), "http", "codex")).toBe(false);
    expect(service.claims(asked(version), "gh", "claude")).toBe(false);
  });
});

describe("a credential the host cannot add stops the turn with a sentence instead of a login prompt", () => {
  const stopped = (got: Got) => {
    expect(got.status).toBe(403);
    expect(pairs(got.rawHeaders)).toContainEqual(["x-should-retry", "false"]);
    const said = JSON.parse(got.body.toString()) as { type: string; error: { message: string } };
    expect(said.type).toBe("error");
    expect(said.error.message).toMatch(/^wsp: .*Settings, Agents\.$/);
    expect(said.error.message).toContain(COMPUTER);
    return said.error.message;
  };

  it("answers 403 with no credential and with a claude.ai login past its expiry, and reaches no upstream", async () => {
    const up = await upstream({ status: 200, headers: [], chunks: [] });
    expect(stopped(await send(await door(up.url, undefined), RECORDED.oauth))).toBe(`wsp: Claude is not signed in on ${COMPUTER}. Sign it in from Settings, Agents.`);
    expect(stopped(await send(await door(up.url, { kind: "token", token: "token-for-test", expiresAt: Date.now() - 1 }), RECORDED.oauth))).toContain("has expired");
    expect(up.seen).toEqual([]);
  });

  it.each([
    ["token", { kind: "token", token: "token-for-test" }, `wsp: Claude's token on ${COMPUTER} has expired. Make a new one in Settings, Agents.`],
    ["claude.ai login", { kind: "token", token: "token-for-test", expiresAt: Date.now() + 3_600_000 }, `wsp: Claude's sign-in on ${COMPUTER} has expired. Open Claude Code there to renew it, or sign in again from Settings, Agents.`],
    ["key", { kind: "key", key: "key-for-test" }, `wsp: Anthropic refused the API key on ${COMPUTER}. Set a working key in Settings, Agents.`],
    ["helper", { kind: "helper", value: "helper-for-test" }, "apiKeyHelper"],
    ["gateway", { kind: "gateway", url: "", token: "gateway-for-test" }, "gateway"],
  ] as const)("turns an upstream 401 on a %s into 403, never 401", async (_kind, credential, sentence) => {
    const refusal = Buffer.from('{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}');
    const up = await upstream({ status: 401, headers: ["Content-Type", "application/json", "x-should-retry", "true"], chunks: [refusal] });
    const given: AnthropicCredential = credential.kind === "gateway" ? { ...credential, url: up.url } : credential;
    const got = await send(await door(up.url, given), RECORDED.oauth);
    expect(up.seen).toHaveLength(1);
    expect(stopped(got)).toContain(sentence);
    expect(got.body.toString()).not.toContain("invalid x-api-key");
  });
});

describe("an upstream the host cannot reach reads to Claude Code as Anthropic not answering, and a real overload as one", () => {
  /** The broker's own refusal, so a refusal reaches the caller in the shape and status Claude Code reads. */
  function brokerContext(credential: AnthropicCredential): AnthropicContext & { refusals: string[] } {
    const refusals: string[] = [];
    return { refusals, credential: async () => credential, refuse: (res, status, said, shape) => (refusals.push(said), writeRefusal(res, status, said, shape)), retrying() {} };
  }

  it.each([
    ["Anthropic", { kind: "key", key: "key-for-test" }],
    ["a gateway", { kind: "gateway", url: "", token: "gateway-for-test" }],
  ] as const)("cuts the call with no answer when %s refuses the connection, since Claude Code reads any status as an answer and a 503 overloaded_error as overloaded", async (_who, credential) => {
    const up = await upstream({ status: 200, headers: [], chunks: [] });
    const gone = up.url;
    await new Promise(done => {
      const server = servers.pop()!;
      server.closeAllConnections();
      server.close(done);
    });
    const ctx = brokerContext(credential.kind === "gateway" ? { ...credential, url: gone } : credential);
    const service = anthropicService({ upstream: gone, computer: () => COMPUTER });
    const doorUrl = await listen((req, res) => void service.serve({ req, res, grant: { workspaceId: "ws_lab", threadId: "th_lab" } }, ctx));
    const answered = await send(doorUrl, RECORDED.key).then(got => `${got.status} ${got.body.toString()}`, (e: Error) => e.message);
    expect(answered).toMatch(/^(socket hang up|read ECONNRESET)$/);
    expect(ctx.refusals).toEqual([]);
  });

  it.each([529, 503])("passes Anthropic's own %s through whole, which Claude Code reads as overloaded", async status => {
    const overloaded = Buffer.from('{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}');
    const up = await upstream({ status, headers: ["Content-Type", "application/json", "x-should-retry", "true"], chunks: [overloaded] });
    const ctx = brokerContext({ kind: "key", key: "key-for-test" });
    const service = anthropicService({ upstream: up.url, computer: () => COMPUTER });
    const doorUrl = await listen((req, res) => void service.serve({ req, res, grant: { workspaceId: "ws_lab", threadId: "th_lab" } }, ctx));
    const got = await send(doorUrl, RECORDED.key);
    expect(got.status).toBe(status);
    expect(got.body.equals(overloaded)).toBe(true);
    expect(pairs(got.rawHeaders)).toContainEqual(["x-should-retry", "true"]);
    expect(ctx.refusals).toEqual([]);
  });
});

describe("a model call no reply has started for is cut at a wait, so Claude Code says it is trying again", () => {
  const key: AnthropicCredential = { kind: "key", key: "key-for-test" };
  const lab = { workspaceId: "ws_lab", threadId: "th_one" };

  /** An upstream that starts each reply only after the wait given for that call, in order. */
  async function slowUpstream(waits: number[]): Promise<{ url: string; calls: () => number; left: () => number }> {
    let calls = 0;
    let left = 0;
    const url = await listen((req, res) => {
      const wait = waits[calls++] ?? Infinity;
      req.resume();
      res.on("close", () => void (res.writableFinished || left++));
      if (wait === Infinity) return;
      setTimeout(() => {
        res.writeHead(200, STREAM_HEADERS);
        res.end(Buffer.concat(SSE));
      }, wait);
    });
    return { url, calls: () => calls, left: () => left };
  }

  async function doorFor(upstreamUrl: string, credential: AnthropicCredential, firstByteMs: number) {
    const service = anthropicService({ upstream: upstreamUrl, computer: () => COMPUTER, firstByteMs });
    const refusals: string[] = [];
    const retries: string[] = [];
    const ctx: AnthropicContext = { credential: async () => credential, refuse: (res, status, said) => (refusals.push(said), context(credential).refuse(res, status, said, "anthropic")), retrying: r => void retries.push(retryLine(r)) };
    let grant = lab;
    const url = await listen((req, res) => void service.serve({ req, res, grant }, ctx));
    return { url, refusals, retries, as: (g: typeof lab) => void (grant = g) };
  }

  const withText = (body: string): Recorded => {
    const rawHeaders = RECORDED.key.rawHeaders.map((v, i, all) => (i % 2 === 1 && all[i - 1]!.toLowerCase() === "content-length" ? String(Buffer.byteLength(body)) : v));
    return { ...RECORDED.key, body, rawHeaders };
  };
  const withBody = (bytes: number): Recorded => withText(RECORDED.key.body.padEnd(bytes, " "));

  it("cuts a call whose upstream took it and never answered, as Claude Code's own wait does, so it reads that Anthropic did not answer", async () => {
    const up = await slowUpstream([Infinity]);
    const door = await doorFor(up.url, key, 150);
    await expect(send(door.url, RECORDED.key)).rejects.toThrow(/socket hang up|ECONNRESET/);
    await expect.poll(up.left).toBe(1);
    expect(door.refusals).toEqual([]);
    expect(door.retries).toEqual(["Anthropic did not answer, trying again (1)"]);
  });

  it("lets a cut call's retry wait as long as Claude Code does, so a reply slow to start costs a retry, never the reply", async () => {
    const up = await slowUpstream([Infinity, 400, 400]);
    const door = await doorFor(up.url, key, 150);
    await expect(send(door.url, RECORDED.key)).rejects.toThrow();
    const retried = await send(door.url, RECORDED.key);
    expect(retried.status).toBe(200);
    expect(retried.body.equals(Buffer.concat(SSE))).toBe(true);
    door.as({ workspaceId: "ws_lab", threadId: "th_two" });
    await expect(send(door.url, RECORDED.key)).rejects.toThrow();
    expect(door.retries).toEqual(["Anthropic did not answer, trying again (1)", "Anthropic did not answer, trying again (1)"]);
  });

  it("tells the thread each answer Claude Code retries, counting a call's attempts by its bytes, and nothing for one it does not", async () => {
    const answers = [529, 529, 500, 200, 529, 400, 503];
    const url = await listen((req, res) => {
      req.resume();
      const status = answers.shift()!;
      res.writeHead(status, { "content-type": "application/json", ...(status === 503 ? { "x-should-retry": "false" } : {}) }).end(JSON.stringify({ type: "error", error: { type: status === 503 ? "api_error" : "overloaded_error" } }));
    });
    const door = await doorFor(url, key, 150);
    for (let i = 0; i < 4; i++) await send(door.url, RECORDED.key);
    expect(door.retries).toEqual(["Anthropic was overloaded, trying again (1)", "Anthropic was overloaded, trying again (2)", "Anthropic answered with a server error (500), trying again (3)"]);
    for (let i = 0; i < 3; i++) await send(door.url, RECORDED.key);
    expect(door.retries.slice(3)).toEqual(["Anthropic was overloaded, trying again (1)"]);
  });

  it("counts two calls with one body apiece and spares each retry the wait, as subagents given one prompt send it", async () => {
    const up = await slowUpstream([Infinity, Infinity, 400, 400]);
    const door = await doorFor(up.url, key, 150);
    await Promise.all([expect(send(door.url, RECORDED.key)).rejects.toThrow(), expect(send(door.url, RECORDED.key)).rejects.toThrow()]);
    const retried = await Promise.all([send(door.url, RECORDED.key), send(door.url, RECORDED.key)]);
    expect(retried.map(got => got.status)).toEqual([200, 200]);
    expect(door.retries).toEqual(["Anthropic did not answer, trying again (1)", "Anthropic did not answer, trying again (1)"]);
  });

  it("tells the thread no retry Claude Code gives up on: a 429 at a claude.ai login's limit, a key's third 529 in a row, an eleventh failure", async () => {
    const answers = [429, 529, 529, 529, ...Array<number>(11).fill(529)];
    const url = await listen((req, res) => {
      req.resume();
      const status = answers.shift()!;
      res.writeHead(status, { "content-type": "application/json", ...(status === 429 ? { "anthropic-ratelimit-unified-status": "rejected", "anthropic-ratelimit-unified-representative-claim": "five_hour" } : {}) }).end(JSON.stringify({ type: "error", error: { type: status === 429 ? "rate_limit_error" : "overloaded_error" } }));
    });
    const login = await doorFor(url, { kind: "token", token: "token-for-test" }, 150);
    await send(login.url, withText(`${RECORDED.key.body} `));
    expect(login.retries).toEqual([]);
    const keyed = await doorFor(url, key, 150);
    for (let i = 0; i < 3; i++) await send(keyed.url, RECORDED.key);
    expect(keyed.retries).toEqual(["Anthropic was overloaded, trying again (1)", "Anthropic was overloaded, trying again (2)"]);
    for (let i = 0; i < 11; i++) await send(login.url, RECORDED.key);
    expect(login.retries).toHaveLength(10);
    expect(login.retries.at(-1)).toBe("Anthropic was overloaded, trying again (10)");
  });

  it("tells the thread nothing of a call whose caller left before its reply started", async () => {
    const up = await slowUpstream([Infinity]);
    const door = await doorFor(up.url, key, 5_000);
    const { port } = new URL(door.url);
    const req = request({ host: "127.0.0.1", port, method: RECORDED.key.method, path: RECORDED.key.url, headers: RECORDED.key.rawHeaders, setHost: false });
    req.on("error", () => undefined);
    req.end(RECORDED.key.body);
    await expect.poll(up.calls).toBe(1);
    req.destroy();
    await expect.poll(up.left).toBe(1);
    expect(door.retries).toEqual([]);
  });

  it("cuts each of a thread's calls at most once when several wait at once, as a turn's subagents do", async () => {
    const up = await slowUpstream([Infinity, Infinity, 400, 400]);
    const door = await doorFor(up.url, key, 150);
    const [main, subagent] = [withText(`${RECORDED.key.body}  `), withText(`${RECORDED.key.body}\n\n`)];
    await Promise.all([expect(send(door.url, main)).rejects.toThrow(), expect(send(door.url, subagent)).rejects.toThrow()]);
    const retried = await Promise.all([send(door.url, main), send(door.url, subagent)]);
    expect(retried.map(got => got.status)).toEqual([200, 200]);
  });

  it("never cuts a call that asks for a whole reply, which starts only once it is done", async () => {
    const up = await slowUpstream([400]);
    const door = await doorFor(up.url, key, 150);
    expect((await send(door.url, withText(RECORDED.key.body.replace('"stream":true', '"stream":false')))).status).toBe(200);
  });

  it("adds a second per 32 KB of the request to the wait, as Claude Code's own does, since the API starts no reply before its first token", async () => {
    const up = await slowUpstream([600]);
    const door = await doorFor(up.url, key, 150);
    const got = await send(door.url, withBody(70_000));
    expect(got.status).toBe(200);
  });

  it("never cuts a reply that has started, however long it runs", async () => {
    let clientHas!: () => void;
    const clientSaw = new Promise<void>(done => (clientHas = done)).then(() => new Promise<void>(done => setTimeout(done, 400)));
    const up = await upstream({ status: 200, headers: STREAM_HEADERS, chunks: SSE, clientSaw });
    const door = await doorFor(up.url, key, 150);
    const got = await send(door.url, RECORDED.key, clientHas);
    expect(got.body.equals(Buffer.concat(SSE))).toBe(true);
  });

  it("leaves a gateway's call to the wait Claude Code gives it, since a gateway may hold a reply until it is whole", async () => {
    const up = await slowUpstream([400]);
    const door = await doorFor(up.url, { kind: "gateway", url: up.url, token: "gateway-for-test" }, 150);
    expect((await send(door.url, RECORDED.key)).status).toBe(200);
  });
});
