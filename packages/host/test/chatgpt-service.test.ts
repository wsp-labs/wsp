// SPDX-License-Identifier: AGPL-3.0-only
// Codex's roads through the host against a loopback stand-in for chatgpt.com and api.openai.com: the refresh and
// revoke answered at home, the websocket turn passed after its 101, connectors given the sign-in they never send,
// and no 401 ever reaching Codex.
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import WebSocket, { WebSocketServer } from "ws";
import { chatgptModels } from "../src/chatgpt/models.js";
import { CHATGPT_WORDS, chatgptService, OPENAI_WORDS, openaiService } from "../src/chatgpt/service.js";
import { codexAuthFile, codexKeyFile, codexPlaceholders, TURN_CLAIM } from "../src/chatgpt/placeholders.js";
import type { BrokerContext, BrokerService, ChatgptCredential, OpenaiCredential } from "../src/chatgpt/types.js";

const REAL_ACCESS = "eyJ-real-chatgpt-access-x";
const REAL_KEY = "sk-proj-real-key-x";
const CLAIMS = { accountId: "acct-real-1", planType: "plus", userId: "user-real-1", fedramp: false };

interface Seen {
  method: string;
  url: string;
  headers: IncomingMessage["headers"];
  raw: string[];
  body: Buffer;
}

let upstream: Server;
let upstreamUrl: string;
let wss: WebSocketServer;
const seen: Seen[] = [];
const wsSeen: IncomingMessage[] = [];
let answer: (req: IncomingMessage, res: ServerResponse, body: Buffer) => void;

let broker: Server;
let brokerUrl: string;
let chatgptCred: ChatgptCredential | undefined;
let openaiCred: OpenaiCredential | undefined;
const refusals: { status: number; said: string }[] = [];
let agent: "claude" | "codex" | undefined = "codex";

const ctx: BrokerContext = {
  credential: (async (kind: "chatgpt" | "openai") => (kind === "chatgpt" ? chatgptCred : openaiCred)) as BrokerContext["credential"],
  refuse(res, status, said) {
    refusals.push({ status, said });
    const text = JSON.stringify({ error: { message: said, type: "wsp_refused" } });
    res.writeHead(status, { "content-type": "application/json", "x-should-retry": "false", "content-length": Buffer.byteLength(text) });
    res.end(text);
  },
};

const CHATGPT = "https://chatgpt.com";
const GOV = "https://gov.example.test";
const dialTo = (stands: Record<string, string>) => (url: URL) => {
  const at = stands[url.origin];
  if (!at) throw new Error(`no stand-in for ${url.origin}`);
  return new URL(`${url.pathname}${url.search}`, at);
};

const listen = (server: Server) => new Promise<string>((resolve) => server.listen(0, "127.0.0.1", () => resolve(`127.0.0.1:${(server.address() as AddressInfo).port}`)));

function decode(jwt: string): Record<string, unknown> {
  return JSON.parse(Buffer.from(jwt.split(".")[1]!, "base64url").toString("utf8")) as Record<string, unknown>;
}

beforeAll(async () => {
  upstream = createServer((req, res) => {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => {
      const body = Buffer.concat(chunks);
      seen.push({ method: req.method!, url: req.url!, headers: req.headers, raw: req.rawHeaders, body });
      answer(req, res, body);
    });
  });
  wss = new WebSocketServer({ server: upstream, perMessageDeflate: true });
  wss.on("connection", (socket, req) => {
    wsSeen.push(req);
    socket.send("server-first");
    socket.on("message", (data, isBinary) => socket.send(isBinary ? Buffer.concat([Buffer.from("echo:"), data as Buffer]) : `echo:${String(data)}`, { binary: isBinary }));
  });
  upstreamUrl = `http://${await listen(upstream)}`;

  const services: BrokerService[] = [chatgptService({ dial: dialTo({ [CHATGPT]: upstreamUrl }), now: () => 1_760_000_000_000 }), openaiService()];
  const grant = { kind: "turn" as const, workspaceId: "ws-1", threadId: "t-1" };
  const pick = (req: IncomingMessage) => services.find((s) => s.claims(req, "http", agent));
  broker = createServer((req, res) => {
    const service = pick(req);
    if (!service) return void res.writeHead(404).end();
    void service.serve({ workspaceId: "ws-1", door: "http", grant, req, res, heldMs: 0 }, ctx);
  });
  broker.on("upgrade", (req: IncomingMessage, socket, head: Buffer) => {
    const service = pick(req);
    if (!service?.upgrade) return void socket.destroy();
    void service.upgrade({ workspaceId: "ws-1", door: "http", grant, req, res: undefined as never, heldMs: 0 }, socket, head, ctx);
  });
  brokerUrl = `http://${await listen(broker)}`;
});

beforeEach(() => {
  seen.length = 0;
  wsSeen.length = 0;
  refusals.length = 0;
  chatgptCred = { ...CLAIMS, accessToken: REAL_ACCESS };
  openaiCred = { key: REAL_KEY };
  agent = "codex";
  answer = (_req, res) => res.writeHead(200, { "content-type": "application/json" }).end("{}");
});

afterAll(async () => {
  for (const client of wss.clients) client.terminate();
  wss.close();
  broker.closeAllConnections();
  upstream.closeAllConnections();
  await new Promise((resolve) => broker.close(resolve));
  await new Promise((resolve) => upstream.close(resolve));
});

const placeholder = () => codexPlaceholders(CLAIMS, "turn-1", 1_760_000_000_000);

describe("refresh and revoke stay home", () => {
  it("answers a refresh with fresh placeholders carrying the same turn, and forwards nothing", async () => {
    chatgptCred = { ...CLAIMS, accessToken: REAL_ACCESS };
    const res = await fetch(`${brokerUrl}/oauth/token`, {
      method: "POST",
      headers: { "content-type": "application/json", originator: "codex_exec" },
      body: JSON.stringify({ client_id: "app_EMoamEEZ73f0CkXaXp7hrann", grant_type: "refresh_token", refresh_token: placeholder().refresh_token }),
    });
    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text).not.toContain(REAL_ACCESS);
    const body = JSON.parse(text) as { id_token: string; access_token: string; refresh_token: string };
    expect(body.refresh_token).toBe("rt_wsp_turn-1");
    const claims = decode(body.access_token);
    expect(claims[TURN_CLAIM]).toBe("turn-1");
    expect(claims["https://api.openai.com/auth"]).toEqual({ chatgpt_account_id: "acct-real-1", chatgpt_plan_type: "plus", chatgpt_user_id: "user-real-1", chatgpt_account_is_fedramp: false });
    expect(claims.exp).toBe(1_760_000_000 + 365 * 24 * 3600);
    expect(decode(body.id_token)[TURN_CLAIM]).toBe("turn-1");
    expect(seen).toEqual([]);
  });

  it("answers 503 while the host's sign-in is not ready, never 401, and forwards nothing", async () => {
    chatgptCred = undefined;
    const res = await fetch(`${brokerUrl}/oauth/token`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ grant_type: "refresh_token", refresh_token: "rt_wsp_turn-1" }),
    });
    expect(res.status).toBe(503);
    expect(refusals).toEqual([{ status: 503, said: CHATGPT_WORDS.notReady }]);
    expect(seen).toEqual([]);
  });

  it("refuses a refresh token wsp never minted, and forwards nothing", async () => {
    const res = await fetch(`${brokerUrl}/oauth/token`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ grant_type: "refresh_token", refresh_token: "rt_real_refresh_x" }),
    });
    expect(res.status).toBe(403);
    expect(refusals).toEqual([{ status: 403, said: CHATGPT_WORDS.notOurs }]);
    expect(seen).toEqual([]);
  });

  it("answers a revoke 200 and forwards nothing, so logout on a machine ends no sign-in", async () => {
    const res = await fetch(`${brokerUrl}/oauth/revoke`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ token: "rt_wsp_turn-1", token_type_hint: "refresh_token", client_id: "app_EMoamEEZ73f0CkXaXp7hrann" }),
    });
    expect(res.status).toBe(200);
    expect(seen).toEqual([]);
  });

  it("passes no upgrade on the refresh path", async () => {
    const socket = new WebSocket(`${brokerUrl.replace("http", "ws")}/oauth/token`);
    socket.on("error", () => undefined);
    const status = await new Promise<number>((resolve) => socket.on("unexpected-response", (_req, res) => resolve(res.statusCode ?? 0)));
    expect(status).toBe(404);
    expect(wsSeen).toEqual([]);
    socket.terminate();
  });
});

describe("the websocket turn", () => {
  it("passes the upgrade on /backend-api/codex/responses and frames flow both ways", async () => {
    const socket = new WebSocket(`${brokerUrl.replace("http", "ws")}/backend-api/codex/responses`, {
      headers: { Authorization: `Bearer ${placeholder().access_token}`, "ChatGPT-Account-ID": "acct-real-1", "x-codex-turn-state": "s1" },
      perMessageDeflate: true,
    });
    const got: string[] = [];
    const upgraded = new Promise<IncomingMessage>((resolve) => socket.on("upgrade", resolve));
    const three = new Promise<void>((resolve) =>
      socket.on("message", (data, isBinary) => {
        got.push(isBinary ? `bin:${(data as Buffer).toString("hex")}` : String(data));
        if (got.length === 3) resolve();
      }),
    );
    await new Promise((resolve, reject) => {
      socket.on("open", resolve);
      socket.on("error", reject);
    });
    socket.send("client-hello");
    socket.send(Buffer.from([0, 1, 2, 255]), { binary: true });
    await three;
    expect(got).toEqual(["server-first", "echo:client-hello", `bin:${Buffer.from("echo:").toString("hex")}000102ff`]);
    const res = await upgraded;
    expect(res.headers["sec-websocket-extensions"]).toContain("permessage-deflate");
    expect(wsSeen).toHaveLength(1);
    expect(wsSeen[0]!.url).toBe("/backend-api/codex/responses");
    expect(wsSeen[0]!.headers.authorization).toBe(`Bearer ${REAL_ACCESS}`);
    expect(wsSeen[0]!.headers["chatgpt-account-id"]).toBe("acct-real-1");
    expect(wsSeen[0]!.headers["x-codex-turn-state"]).toBe("s1");
    const closed = new Promise((resolve) => socket.on("close", resolve));
    socket.close();
    await closed;
  });

  it("answers an upgrade refused upstream with 401 as a 403 sentence", async () => {
    chatgptCred = { ...CLAIMS, accessToken: REAL_ACCESS };
    const refusing = createServer((_req, res) => res.writeHead(500).end());
    refusing.on("upgrade", (_req, socket) => socket.on("error", () => undefined).end("HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"));
    const at = await listen(refusing);
    const service = chatgptService({ dial: dialTo({ [CHATGPT]: `http://${at}` }) });
    const local = createServer();
    local.on("upgrade", (req: IncomingMessage, socket, head: Buffer) => {
      void service.upgrade!({ workspaceId: "ws-1", door: "http", grant: { kind: "turn", workspaceId: "ws-1" }, req, res: undefined as never, heldMs: 0 }, socket, head, ctx);
    });
    const localAt = await listen(local);
    const socket = new WebSocket(`ws://${localAt}/backend-api/codex/responses`);
    socket.on("error", () => undefined);
    const status = await new Promise<number>((resolve) => socket.on("unexpected-response", (_req, res) => resolve(res.statusCode ?? 0)));
    expect(status).toBe(403);
    expect(refusals).toEqual([{ status: 403, said: CHATGPT_WORDS.refused }]);
    socket.terminate();
    local.closeAllConnections();
    refusing.closeAllConnections();
    await new Promise((resolve) => local.close(resolve));
    await new Promise((resolve) => refusing.close(resolve));
  });
});

describe("connectors and every /backend-api/ path", () => {
  it("adds the sign-in to a connector call that carries no Authorization", async () => {
    answer = (_req, res) => res.writeHead(200, { "content-type": "application/json", "mcp-session-id": "m1" }).end('{"jsonrpc":"2.0","id":1,"result":{}}');
    const body = '{"jsonrpc":"2.0","id":1,"method":"initialize"}';
    const res = await fetch(`${brokerUrl}/backend-api/ps/mcp`, { method: "POST", headers: { "content-type": "application/json" }, body });
    expect(res.status).toBe(200);
    expect(res.headers.get("mcp-session-id")).toBe("m1");
    expect(seen).toHaveLength(1);
    expect(seen[0]!.headers.authorization).toBe(`Bearer ${REAL_ACCESS}`);
    expect(seen[0]!.headers["chatgpt-account-id"]).toBe("acct-real-1");
    expect(seen[0]!.body.toString()).toBe(body);
  });

  it("never hands Codex a 401 on connectors: the upstream's 401 reaches it as a 403 sentence", async () => {
    answer = (_req, res) => res.writeHead(401, { "www-authenticate": 'Bearer resource_metadata="x"' }).end("{}");
    const res = await fetch(`${brokerUrl}/backend-api/ps/mcp`, { method: "POST", headers: { "content-type": "application/json" }, body: "{}" });
    expect(res.status).toBe(403);
    expect(res.headers.get("www-authenticate")).toBeNull();
    expect(((await res.json()) as { error: { message: string } }).error.message).toBe(CHATGPT_WORDS.refused);
  });

  it("refuses with 403, not 401, when the host holds no ChatGPT sign-in, and reaches no upstream", async () => {
    chatgptCred = undefined;
    const res = await fetch(`${brokerUrl}/backend-api/ps/mcp`, { method: "POST", body: "{}" });
    expect(res.status).toBe(403);
    expect(refusals).toEqual([{ status: 403, said: CHATGPT_WORDS.signedOut }]);
    expect(CHATGPT_WORDS.signedOut).toContain('cli_auth_credentials_store = "file"');
    expect(seen).toEqual([]);
  });

  it("swaps the caller's auth and cookies for the host's, passes the rest and the body byte for byte, and streams the reply", async () => {
    chatgptCred = { ...CLAIMS, fedramp: true, accessToken: REAL_ACCESS };
    answer = (_req, res) => {
      res.writeHead(200, { "content-type": "text/event-stream", "x-codex-primary-used-percent": "12" });
      res.write("event: response.created\ndata: {}\n\n");
      setTimeout(() => res.end("event: response.completed\ndata: {}\n\n"), 30);
    };
    const body = Buffer.from([0x28, 0xb5, 0x2f, 0xfd, 0, 1, 2, 3]);
    const res = await fetch(`${brokerUrl}/backend-api/codex/responses?client_version=0.162.1`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${placeholder().access_token}`,
        "chatgpt-account-id": "acct-placeholder",
        "x-openai-fedramp": "false",
        cookie: "a=b",
        "x-api-key": "k",
        "content-encoding": "zstd",
        "x-codex-turn-state": "s2",
      },
      body,
    });
    expect(res.status).toBe(200);
    expect(res.headers.get("x-codex-primary-used-percent")).toBe("12");
    expect(await res.text()).toBe("event: response.created\ndata: {}\n\nevent: response.completed\ndata: {}\n\n");
    const got = seen[0]!;
    expect(got.url).toBe("/backend-api/codex/responses?client_version=0.162.1");
    expect(got.headers.authorization).toBe(`Bearer ${REAL_ACCESS}`);
    expect(got.headers["chatgpt-account-id"]).toBe("acct-real-1");
    expect(got.headers["x-openai-fedramp"]).toBe("true");
    expect(got.headers.cookie).toBeUndefined();
    expect(got.headers["x-api-key"]).toBeUndefined();
    expect(got.headers["content-encoding"]).toBe("zstd");
    expect(got.headers["x-codex-turn-state"]).toBe("s2");
    expect(got.raw.filter((_, i) => i % 2 === 0 && got.raw[i]!.toLowerCase() === "authorization")).toHaveLength(1);
    expect(got.body.equals(body)).toBe(true);
  });

  it("sends a redirect back as it came and follows none", async () => {
    answer = (_req, res) => res.writeHead(302, { location: "https://files.example.test/x" }).end();
    const res = await fetch(`${brokerUrl}/backend-api/files/x`, { redirect: "manual" });
    expect(res.status).toBe(302);
    expect(res.headers.get("location")).toBe("https://files.example.test/x");
    expect(seen).toHaveLength(1);
  });

  it("answers 503 when the upstream cannot be reached", async () => {
    const dead = createServer();
    const at = await listen(dead);
    await new Promise((resolve) => dead.close(resolve));
    const service = chatgptService({ dial: dialTo({ [CHATGPT]: `http://${at}` }) });
    const local = createServer((req, res) => void service.serve({ workspaceId: "ws-1", door: "http", grant: { kind: "turn", workspaceId: "ws-1" }, req, res, heldMs: 0 }, ctx));
    const localAt = await listen(local);
    const res = await fetch(`http://${localAt}/backend-api/wham/usage`);
    expect(res.status).toBe(503);
    expect(refusals).toEqual([{ status: 503, said: CHATGPT_WORDS.unreachable }]);
    local.closeAllConnections();
    await new Promise((resolve) => local.close(resolve));
  });
});

describe("the workspace's own origin", () => {
  it("answers accounts/check with an https origin for every account, and sends the sign-in's later calls to its own", async () => {
    const govSeen: IncomingMessage[] = [];
    const gov = createServer((req, res) => {
      govSeen.push(req);
      req.resume();
      res.writeHead(200, { "content-type": "application/json" }).end('{"gov":true}');
    });
    const govWss = new WebSocketServer({ server: gov });
    govWss.on("connection", (socket, req) => {
      govSeen.push(req);
      socket.send("gov-first");
    });
    const govAt = `http://${await listen(gov)}`;
    const service = chatgptService({ dial: dialTo({ [CHATGPT]: upstreamUrl, [GOV]: govAt }) });
    const grant = { kind: "turn" as const, workspaceId: "ws-1" };
    const local = createServer((req, res) => void service.serve({ workspaceId: "ws-1", door: "http", grant, req, res, heldMs: 0 }, ctx));
    local.on("upgrade", (req: IncomingMessage, socket, head: Buffer) => void service.upgrade!({ workspaceId: "ws-1", door: "http", grant, req, res: undefined as never, heldMs: 0 }, socket, head, ctx));
    const localAt = await listen(local);

    answer = (_req, res) =>
      res.writeHead(200, { "content-type": "application/json" }).end(
        JSON.stringify({
          accounts: [
            { id: "other-1", workspace_backend_origin: "NO_CONSTRAINT", account_routing_override: "NO_CONSTRAINT" },
            { id: "acct-real-1", workspace_backend_origin: `${GOV}/`, account_routing_override: "us" },
            { id: "other-2" },
            { id: "other-3", workspace_backend_origin: "http://evil.example.test" },
            { id: "other-4", workspace_backend_origin: "https://user:pw@evil.example.test" },
          ],
          default_account_id: "other-1",
        }),
      );
    const check = await fetch(`http://${localAt}/backend-api/wham/accounts/check`, { headers: { "accept-encoding": "gzip, br, zstd" } });
    expect(check.status).toBe(200);
    const text = await check.text();
    expect(Number(check.headers.get("content-length"))).toBe(Buffer.byteLength(text));
    const body = JSON.parse(text) as { accounts: { id: string; workspace_backend_origin: string; account_routing_override?: string }[]; default_account_id: string };
    expect(body.accounts.map((a) => [a.id, a.workspace_backend_origin])).toEqual([
      ["other-1", CHATGPT],
      ["acct-real-1", GOV],
      ["other-2", CHATGPT],
      ["other-3", CHATGPT],
      ["other-4", CHATGPT],
    ]);
    expect(body.accounts[1]!.account_routing_override).toBe("us");
    expect(body.default_account_id).toBe("other-1");
    expect(seen).toHaveLength(1);
    expect(seen[0]!.headers.host).toBe("chatgpt.com");
    expect(seen[0]!.headers.authorization).toBe(`Bearer ${REAL_ACCESS}`);
    expect(seen[0]!.headers["accept-encoding"]).toBe("identity");

    const usage = await fetch(`http://${localAt}/backend-api/wham/usage`);
    expect(await usage.text()).toBe('{"gov":true}');
    expect(govSeen[0]!.headers.host).toBe("gov.example.test");
    expect(govSeen[0]!.headers.authorization).toBe(`Bearer ${REAL_ACCESS}`);

    const socket = new WebSocket(`ws://${localAt}/backend-api/codex/responses`);
    const first = await new Promise<string>((resolve, reject) => {
      socket.on("message", (data) => resolve(String(data)));
      socket.on("error", reject);
    });
    expect(first).toBe("gov-first");
    socket.terminate();

    await fetch(`http://${localAt}/backend-api/wham/accounts/check?origin=${encodeURIComponent("https://evil.example.test")}`);
    expect(seen).toHaveLength(2);
    expect(seen[1]!.headers.host).toBe("chatgpt.com");
    expect(govSeen).toHaveLength(2);

    for (const client of govWss.clients) client.terminate();
    govWss.close();
    local.closeAllConnections();
    gov.closeAllConnections();
    await new Promise((resolve) => local.close(resolve));
    await new Promise((resolve) => gov.close(resolve));
  });
});

describe("the OpenAI road", () => {
  it("carries /v1/ to the key's upstream with the key, the organization and the project", async () => {
    openaiCred = { key: REAL_KEY, baseUrl: `${upstreamUrl}/v1`, organization: "org-1", project: "proj-1" };
    const res = await fetch(`${brokerUrl}/v1/responses`, { method: "POST", headers: { authorization: "Bearer sk-wsp-turn-1", "openai-project": "p-placeholder" }, body: "{}" });
    expect(res.status).toBe(200);
    expect(seen[0]!.url).toBe("/v1/responses");
    expect(seen[0]!.headers.authorization).toBe(`Bearer ${REAL_KEY}`);
    expect(seen[0]!.headers["openai-organization"]).toBe("org-1");
    expect(seen[0]!.headers["openai-project"]).toBe("proj-1");
  });

  it("carries a custom provider to its recorded base_url, query and all", async () => {
    openaiCred = { key: REAL_KEY, baseUrl: `${upstreamUrl}/openai/` };
    await fetch(`${brokerUrl}/v1/responses?api-version=2025-04-01-preview`, { method: "POST", body: "{}" });
    expect(seen[0]!.url).toBe("/openai/responses?api-version=2025-04-01-preview");
    expect(seen[0]!.headers["openai-project"]).toBeUndefined();
  });

  it("passes the websocket on /v1/responses with the key", async () => {
    openaiCred = { key: REAL_KEY, baseUrl: `${upstreamUrl}/v1` };
    const socket = new WebSocket(`${brokerUrl.replace("http", "ws")}/v1/responses`, { headers: { Authorization: "Bearer sk-wsp-turn-1" } });
    const first = await new Promise<string>((resolve, reject) => {
      socket.on("message", (data) => resolve(String(data)));
      socket.on("error", reject);
    });
    expect(first).toBe("server-first");
    expect(wsSeen[0]!.headers.authorization).toBe(`Bearer ${REAL_KEY}`);
    socket.terminate();
  });

  it("refuses with 403 when the host holds no key", async () => {
    openaiCred = undefined;
    const res = await fetch(`${brokerUrl}/v1/responses`, { method: "POST", body: "{}" });
    expect(res.status).toBe(403);
    expect(refusals).toEqual([{ status: 403, said: OPENAI_WORDS.noKey }]);
    expect(seen).toEqual([]);
  });
});

describe("what each road claims", () => {
  const req = (url: string) => ({ url }) as IncomingMessage;
  const chatgpt = chatgptService();
  const openai = openaiService();

  it("claims Codex's paths on the HTTP door only, and leaves Claude's /v1/ alone", () => {
    expect(chatgpt.claims(req("/backend-api/codex/responses"), "http", "codex")).toBe(true);
    expect(chatgpt.claims(req("/oauth/token"), "http", "codex")).toBe(true);
    expect(chatgpt.claims(req("/oauth/revoke"), "http", "codex")).toBe(true);
    expect(openai.claims(req("/v1/responses"), "http", "codex")).toBe(true);
    expect(openai.claims(req("/v1/messages"), "http", "claude")).toBe(false);
    expect(chatgpt.claims(req("/backend-api/codex/responses"), "http", "claude")).toBe(false);
    expect(chatgpt.claims(req("/backend-api/codex/responses"), "gh", "codex")).toBe(false);
  });

  it("never lets the request name the upstream or climb out of its path", () => {
    expect(chatgpt.claims(req("http://evil.example.test/backend-api/x"), "http", "codex")).toBe(false);
    expect(chatgpt.claims(req("/backend-api/../api/auth/session"), "http", "codex")).toBe(false);
    expect(chatgpt.claims(req("/backend-api/%2e%2e/api/auth/session"), "http", "codex")).toBe(false);
    expect(openai.claims(req("/v1/../admin"), "http", "codex")).toBe(false);
  });
});

describe("placeholders", () => {
  it("mint a file Codex reads as a ChatGPT sign-in with the real claims, a far exp and the turn, and no real token", () => {
    const file = codexAuthFile({ ...CLAIMS, fedramp: true }, "turn-9", 1_760_000_000_000) as {
      auth_mode: string;
      tokens: { id_token: string; access_token: string; refresh_token: string; account_id: string };
      last_refresh: string;
    };
    expect(file.auth_mode).toBe("chatgpt");
    expect(file.tokens.account_id).toBe("acct-real-1");
    expect(file.last_refresh).toBe(new Date(1_760_000_000_000).toISOString());
    expect(file.tokens.access_token.split(".").every((p) => p.length > 0)).toBe(true);
    const claims = decode(file.tokens.access_token);
    expect(claims[TURN_CLAIM]).toBe("turn-9");
    expect((claims["https://api.openai.com/auth"] as { chatgpt_account_is_fedramp: boolean }).chatgpt_account_is_fedramp).toBe(true);
    expect(JSON.stringify(file)).not.toContain(REAL_ACCESS);
  });

  it("mint an apikey file holding the turn as an sk-wsp- key", () => {
    expect(codexKeyFile("turn-9")).toEqual({ auth_mode: "apikey", OPENAI_API_KEY: "sk-wsp-turn-9" });
  });
});

describe("the account's Codex models, read by the host", () => {
  // upstreamUrl is set in beforeAll, so the stand-in is named at each read.
  const read = (version: string) => chatgptModels(async () => chatgptCred, version, { dial: url => dialTo({ [CHATGPT]: upstreamUrl })(url) });

  it("asks chatgpt.com for the version named with the host's own login, as codex asks at a turn", async () => {
    chatgptCred = { ...CLAIMS, fedramp: true, accessToken: REAL_ACCESS };
    answer = (_req, res) => res.writeHead(200, { "content-type": "application/json" }).end(JSON.stringify({ models: [{ slug: "gpt-6-sol", visibility: "list" }] }));
    expect(await read("0.155.1")).toEqual({ models: [{ slug: "gpt-6-sol", visibility: "list" }] });
    expect(seen.map(s => `${s.method} ${s.url}`)).toEqual(["GET /backend-api/codex/models?client_version=0.155.1"]);
    expect(seen[0]!.headers).toMatchObject({ authorization: `Bearer ${REAL_ACCESS}`, "chatgpt-account-id": "acct-real-1", "x-openai-fedramp": "true" });
  });

  it("answers nothing with no login to read with, or when chatgpt.com refuses or answers no JSON", async () => {
    chatgptCred = undefined;
    expect(await read("0.155.1")).toBeUndefined();
    expect(seen).toEqual([]);
    chatgptCred = { ...CLAIMS, accessToken: REAL_ACCESS };
    answer = (_req, res) => res.writeHead(401, { "content-type": "application/json" }).end('{"detail":"Unauthorized"}');
    expect(await read("0.155.1")).toBeUndefined();
    answer = (_req, res) => res.writeHead(200, { "content-type": "text/html" }).end("<html>");
    expect(await read("0.155.1")).toBeUndefined();
  });
});

describe("a model call no reply has started for is cut at a wait, so Codex says it is reconnecting", () => {
  const WAIT = 150;
  const servers: Server[] = [];
  const socketServers: WebSocketServer[] = [];
  const clients: WebSocket[] = [];
  afterEach(async () => {
    for (const client of clients.splice(0)) client.terminate();
    for (const wss of socketServers.splice(0)) {
      for (const client of wss.clients) client.terminate();
      wss.close();
    }
    for (const server of servers.splice(0)) {
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
    }
  });

  /** An upstream answering each call and each socket as the test says, counting the calls it saw cut before their end. */
  async function stand(onCall: (req: IncomingMessage, res: ServerResponse) => void = () => undefined, onSocket: (socket: WebSocket) => void = () => undefined) {
    let cut = 0;
    const server = createServer((req, res) => {
      res.on("close", () => void (res.writableFinished || cut++));
      req.resume();
      req.on("end", () => onCall(req, res));
    });
    const wss = new WebSocketServer({ server });
    wss.on("connection", onSocket);
    servers.push(server);
    socketServers.push(wss);
    return { url: `http://${await listen(server)}`, cut: () => cut };
  }

  async function doorTo(upstream: string): Promise<string> {
    openaiCred = { key: REAL_KEY, baseUrl: `${upstream}/v1` };
    const services = [chatgptService({ dial: dialTo({ [CHATGPT]: upstream }), firstByteMs: WAIT }), openaiService()];
    const grant = (req: IncomingMessage) => ({ kind: "turn" as const, workspaceId: "ws-1", threadId: String(req.headers["x-lab-thread"] ?? "t-1") });
    const pick = (req: IncomingMessage) => services.find((s) => s.claims(req, "http", "codex"))!;
    const door = createServer((req, res) => void pick(req).serve({ workspaceId: "ws-1", door: "http", grant: grant(req), req, res, heldMs: 0 }, ctx));
    door.on("upgrade", (req: IncomingMessage, socket, head: Buffer) => void pick(req).upgrade!({ workspaceId: "ws-1", door: "http", grant: grant(req), req, res: undefined as never, heldMs: 0 }, socket, head, ctx));
    servers.push(door);
    return `http://${await listen(door)}`;
  }

  const call = (url: string, body = "{}", accept = "text/event-stream", thread = "t-1") => fetch(url, { method: "POST", headers: { accept, "content-type": "application/json", "x-lab-thread": thread }, body });
  const events = (n: number) => Array.from({ length: n }, (_, i) => `event: response.output_text.delta\ndata: {"delta":"${i}"}\n\n`).join("");

  async function socketTo(url: string): Promise<{ socket: WebSocket; got: string[]; closed: Promise<void> }> {
    const socket = new WebSocket(url.replace("http", "ws"));
    clients.push(socket);
    const got: string[] = [];
    socket.on("message", (data) => got.push(String(data)));
    const closed = new Promise<void>((resolve) => socket.on("close", () => resolve()));
    await new Promise((resolve, reject) => {
      socket.on("open", resolve);
      socket.on("error", reject);
    });
    return { socket, got, closed };
  }

  it("answers a Responses call its upstream took and never answered with an event stream that ends, which Codex retries at once and says so", async () => {
    const up = await stand();
    const res = await call(`${await doorTo(up.url)}/backend-api/codex/responses`);
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toBe("text/event-stream");
    expect(await res.text()).toBe("");
    await expect.poll(up.cut).toBe(1);
    expect(refusals).toEqual([]);
  });

  it("lets a thread's calls wait as long as Codex does once two were cut in the last minute, so a reply slow to start costs retries, never the reply", async () => {
    const up = await stand((_req, res) => setTimeout(() => res.writeHead(200, { "content-type": "text/event-stream" }).end(events(1)), WAIT * 3));
    const door = `${await doorTo(up.url)}/backend-api/codex/responses`;
    expect(await (await call(door)).text()).toBe("");
    expect(await (await call(door)).text()).toBe("");
    expect(await (await call(door)).text()).toBe(events(1));
    expect(await (await call(door, "{}", "text/event-stream", "t-2")).text()).toBe("");
  });

  it("keeps each agent's two cuts its own, as a turn's subagents call at once on its grant, each named by its thread-id", async () => {
    const up = await stand((_req, res) => setTimeout(() => res.writeHead(200, { "content-type": "text/event-stream" }).end(events(1)), WAIT * 3));
    const door = `${await doorTo(up.url)}/backend-api/codex/responses`;
    const agent = (id: string) => fetch(door, { method: "POST", headers: { accept: "text/event-stream", "thread-id": id }, body: "{}" }).then((res) => res.text());
    expect(await Promise.all([agent("subagent-a"), agent("subagent-b")])).toEqual(["", ""]);
    expect(await agent("lead")).toBe("");
  });

  it("never cuts a reply that has started, however long it runs", async () => {
    const up = await stand((_req, res) => {
      res.writeHead(200, { "content-type": "text/event-stream" });
      let sent = 0;
      const next = () => (sent++ < 4 ? (res.write(events(1)), setTimeout(next, WAIT)) : res.end());
      next();
    });
    const res = await call(`${await doorTo(up.url)}/backend-api/codex/responses`);
    expect((await res.text()).match(/response\.output_text\.delta/g)).toHaveLength(4);
    expect(up.cut()).toBe(0);
  });

  it("adds a second per 32 KB of the call to the wait, the time a large call takes to send", async () => {
    const up = await stand((_req, res) => setTimeout(() => res.writeHead(200, { "content-type": "text/event-stream" }).end(events(1)), WAIT * 4));
    const res = await call(`${await doorTo(up.url)}/backend-api/codex/responses`, `{"input":"${"x".repeat(70_000)}"}`);
    expect(await res.text()).toBe(events(1));
  });

  it("never cuts a call that asks for a whole reply, which starts only once it is done", async () => {
    const up = await stand((_req, res) => setTimeout(() => res.writeHead(200, { "content-type": "application/json" }).end('{"output":[]}'), WAIT * 3));
    const res = await call(`${await doorTo(up.url)}/backend-api/codex/responses/compact`, "{}", "application/json");
    expect(await res.text()).toBe('{"output":[]}');
  });

  it("cuts a websocket whose upstream sends nothing back for a message within the wait, which Codex reads as a dropped stream", async () => {
    const up = await stand(undefined, (socket) => socket.on("message", () => undefined));
    const { socket, closed } = await socketTo(`${await doorTo(up.url)}/backend-api/codex/responses`);
    await new Promise((resolve) => setTimeout(resolve, WAIT * 2));
    expect(socket.readyState).toBe(WebSocket.OPEN);
    socket.send('{"type":"response.create"}');
    await closed;
  });

  it("takes neither the upstream's pings nor the caller's pongs for a call or its reply, as chatgpt.com pings an idle socket every 20 s", async () => {
    const pinging = (socket: WebSocket) => {
      const timer = setInterval(() => socket.ping(), WAIT / 3);
      socket.on("close", () => clearInterval(timer));
    };
    const up = await stand(undefined, pinging);
    const idle = await socketTo(`${await doorTo(up.url)}/backend-api/codex/responses`);
    await new Promise((resolve) => setTimeout(resolve, WAIT * 3));
    expect(idle.socket.readyState).toBe(WebSocket.OPEN);
    idle.socket.send('{"type":"response.create"}');
    await idle.closed;
  });

  it("never cuts a websocket reply that has started, and gives the socket's next message a wait of its own", async () => {
    const up = await stand(undefined, (socket) =>
      socket.on("message", (data) => {
        if (String(data) === "held") return;
        let sent = 0;
        const next = () => sent++ < 4 && (socket.send(`delta${sent}`), setTimeout(next, WAIT));
        next();
      }),
    );
    const { socket, got, closed } = await socketTo(`${await doorTo(up.url)}/backend-api/codex/responses`);
    socket.send('{"type":"response.create"}');
    await expect.poll(() => got.length, { timeout: WAIT * 10 }).toBe(4);
    expect(socket.readyState).toBe(WebSocket.OPEN);
    socket.send("held");
    await closed;
  });

  it("asks for a websocket message's wait as the message starts, as one socket carries a thread's calls from turn to turn", async () => {
    const slow = (_req: IncomingMessage, res: ServerResponse) => setTimeout(() => res.writeHead(200, { "content-type": "text/event-stream" }).end(events(1)), WAIT * 3);
    const up = await stand(slow, (socket) => socket.on("message", () => setTimeout(() => socket.send("started"), WAIT * 3)));
    const door = await doorTo(up.url);
    const { socket, got } = await socketTo(`${door}/backend-api/codex/responses`);
    expect(await (await call(`${door}/backend-api/codex/responses`)).text()).toBe("");
    expect(await (await call(`${door}/backend-api/codex/responses`)).text()).toBe("");
    socket.send('{"type":"response.create"}');
    await expect.poll(() => got, { timeout: WAIT * 10 }).toEqual(["started"]);
  });

  it("leaves the OpenAI key road's calls to the wait Codex gives them, as api.openai.com's first byte is not measured", async () => {
    const up = await stand(
      (_req, res) => setTimeout(() => res.writeHead(200, { "content-type": "text/event-stream" }).end(events(1)), WAIT * 3),
      (socket) => socket.on("message", () => setTimeout(() => socket.send("started"), WAIT * 3)),
    );
    const door = await doorTo(up.url);
    expect(await (await call(`${door}/v1/responses`)).text()).toBe(events(1));
    const { socket, got } = await socketTo(`${door}/v1/responses`);
    socket.send('{"type":"response.create"}');
    await expect.poll(() => got, { timeout: WAIT * 10 }).toEqual(["started"]);
    expect(socket.readyState).toBe(WebSocket.OPEN);
  });

  it("adds a second per 32 KB of a websocket message to its wait", async () => {
    const up = await stand(undefined, (socket) => socket.on("message", () => setTimeout(() => socket.send("started"), WAIT * 4)));
    const { socket, got } = await socketTo(`${await doorTo(up.url)}/backend-api/codex/responses`);
    socket.send(`{"input":"${"x".repeat(70_000)}"}`);
    await expect.poll(() => got, { timeout: WAIT * 10 }).toEqual(["started"]);
    expect(socket.readyState).toBe(WebSocket.OPEN);
  });
});
