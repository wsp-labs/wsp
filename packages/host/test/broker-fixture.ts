// SPDX-License-Identifier: AGPL-3.0-only
// A broker driven the way a machine's pipes drive it: each client is node's own
// http client over an in-memory duplex that the broker takes as a pipe, and the
// upstream is a loopback server that records every request that reaches it.
// The services here are stand-ins that forward the way a real one would, so
// what the tests hold is the broker's own rules and nothing of a vendor's.
import { mkdtempSync, rmSync } from "node:fs";
import { createServer, request, type IncomingHttpHeaders, type IncomingMessage, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Duplex, PassThrough, Writable } from "node:stream";
import { createBroker, type Broker, type BrokerOptions } from "../src/broker/broker.js";
import type { RunCommand } from "../src/broker/credentials.js";
import { brokerLogFile } from "../src/broker/log.js";
import { pathOf } from "../src/broker/tokens.js";
import type { PipeDoor } from "@wsp/protocol";
import type { BrokerService, CredentialKind, Grant, LiveGrant, ServiceId } from "../src/broker/types.js";

export const WS = "ws-acme-lab";
export const OTHER_WS = "ws-acme-other";
export const TURN = "wspturn-acme-lab-0001";
export const OP = "wspop-acme-lab-0002";
export const TERMINAL = "wspterm-acme-lab-0003";
export const OTHER_TURN = "wspturn-acme-other-0004";
export const ENDED_TURN = "wspturn-acme-lab-ended";
export const DOCTOR_OP = "wspop-acme-lab-doctor";
export const STORAGE_HOST = "objects.githubusercontent.com";

// Built at run time so no literal in the tree looks like a live key to a scanner.
export const REAL = {
  anthropic: ["sk", "ant", "api03", `fake${"0".repeat(28)}fake`].join("-"),
  openai: ["sk", "proj", `fake${"1".repeat(28)}fake`].join("-"),
  github: `gho_${"f".repeat(18)}${"2".repeat(18)}`,
} as const;

export const GRANTS: Readonly<Record<string, Grant>> = {
  [TURN]: { kind: "turn", workspaceId: WS, threadId: "th-acme-1" },
  [OP]: { kind: "op", workspaceId: WS, op: "git.push", branch: "wsp/fix-tests" },
  [TERMINAL]: { kind: "terminal", workspaceId: WS, threadId: "th-acme-1", branch: "wsp/fix-tests" },
  [OTHER_TURN]: { kind: "turn", workspaceId: OTHER_WS, threadId: "th-acme-2" },
  [DOCTOR_OP]: { kind: "op", workspaceId: WS, op: "doctor" },
};

/** The secret in any of the ruled credential shapes, for a stand-in to send. */
const secretOf = (c: object): string => {
  const o = c as Record<string, string | undefined>;
  return o.key ?? o.token ?? o.accessToken ?? o.value ?? "";
};

/** An unsigned JWT as Codex's placeholder access token is one, with the turn token in its claim. */
export function placeholderJwt(turn: string | undefined): string {
  const part = (v: object) => Buffer.from(JSON.stringify(v)).toString("base64url");
  return `${part({ alg: "none", typ: "JWT" })}.${part({ exp: 4102444800, ...(turn !== undefined ? { "https://wsp.dev/turn": turn } : {}) })}.c2lnbmF0dXJl`;
}

export interface Seen {
  method: string;
  url: string;
  headers: IncomingHttpHeaders;
  body: string;
}

/** A loopback upstream that records what reaches it and answers by path: a redirect, a 401, a stream, or 200. */
export async function upstream(): Promise<{ port: number; seen: Seen[]; close(): Promise<void> }> {
  const seen: Seen[] = [];
  const server: Server = createServer((req, res) => {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => {
      seen.push({ method: req.method ?? "", url: req.url ?? "", headers: req.headers, body: Buffer.concat(chunks).toString("utf8") });
      const path = pathOf(req.url);
      if (path.endsWith("/redirect")) {
        res.writeHead(302, { location: "https://objects.githubusercontent.com/acme/lab/blob?sig=upstream-signed", "set-cookie": "upstream=1" });
        return res.end();
      }
      if (path.endsWith("/unauthorized")) {
        res.writeHead(401, { "www-authenticate": 'Basic realm="GitHub"', "content-type": "text/plain" });
        return res.end("Bad credentials");
      }
      res.writeHead(200, { "content-type": "application/json", "x-upstream": "yes" });
      res.end(JSON.stringify({ ok: true, path }));
    });
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  return {
    port: (server.address() as AddressInfo).port,
    seen,
    close: () => new Promise<void>(resolve => server.close(() => resolve())),
  };
}

/** A stand-in service: claims by door and path, adds the real credential the way its vendor wants it, forwards to
 * the recorded upstream with node's own client (which follows no redirect), and passes the answer back. */
export function forwarding(id: ServiceId, port: number, o: {
  door: PipeDoor;
  claims: (req: IncomingMessage) => boolean;
  /** Nothing for a fetch that carries no credential, as a storage host's does. */
  credential?: CredentialKind;
  header: (secret: string) => Record<string, string>;
  served: Array<{ id: ServiceId; grant: Grant; headers: IncomingHttpHeaders; raw: string[] }>;
}): BrokerService {
  return {
    id,
    claims: (req, door) => door === o.door && o.claims(req),
    async serve(r, ctx) {
      o.served.push({ id, grant: r.grant, headers: { ...r.req.headers }, raw: [...r.req.rawHeaders] });
      const cred = o.credential === undefined ? undefined : await ctx.credential(o.credential);
      if (o.credential !== undefined && cred === undefined) return ctx.refuse(r.res, 403, "wsp: no credential on your computer.", "anthropic");
      const target = /^https?:\/\//.test(r.req.url ?? "") ? new URL(r.req.url!) : undefined;
      await new Promise<void>((resolve, reject) => {
        const up = request(
          { host: "127.0.0.1", port, method: r.req.method, path: target !== undefined ? `${target.pathname}${target.search}` : r.req.url, headers: { ...r.req.headers, ...(cred !== undefined ? o.header(secretOf(cred)) : {}) } },
          answer => {
            r.res.writeHead(answer.statusCode ?? 502, answer.headers);
            answer.pipe(r.res);
            answer.on("end", resolve);
          },
        );
        up.on("error", reject);
        r.req.pipe(up);
      });
    },
    async upgrade(r, socket) {
      o.served.push({ id, grant: r.grant, headers: { ...r.req.headers }, raw: [...r.req.rawHeaders] });
      socket.write("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n");
      socket.on("data", (d: Buffer) => socket.write(Buffer.concat([Buffer.from("echo:"), d])));
    },
  };
}

/** Two ends of one in-memory connection: the broker takes one as the pipe, the client writes on the other. */
export function pipePair(): [Duplex, Duplex] {
  const toHost = new PassThrough();
  const toClient = new PassThrough();
  // The daemon shuts the machine's side down however the pipe ends, so the client reads what was sent, then its end.
  const down = new Writable({
    write: (chunk: Buffer, _encoding, done) => void toClient.write(chunk, done),
    final: done => void toClient.end(done),
    destroy: (err, done) => {
      if (!toClient.writableEnded) toClient.end();
      done(err);
    },
  });
  const up = toHost.pipe(new PassThrough());
  toHost.once("close", () => up.destroy());
  const host = Duplex.from({ readable: up, writable: down });
  const client = Duplex.from({ readable: toClient, writable: toHost });
  // Destroying the client's end aborts the streams under both, which is the cut a dropped pipe is and no failure of a test.
  for (const s of [toHost, toClient, up, down, host, client]) s.on("error", () => undefined);
  return [host, client];
}

export interface Rig {
  broker: Broker;
  up: Awaited<ReturnType<typeof upstream>>;
  served: Array<{ id: ServiceId; grant: Grant; headers: IncomingHttpHeaders; raw: string[] }>;
  gh: { runs: string[][] };
  revokedTokens: string[];
  live: LiveGrant[];
  logDir: string;
  waits: Array<{ grant: Grant; heldMs: number }>;
  /** Sends one request through a fresh pipe of `workspaceId` at `door`, answering the status, headers and body. */
  call(o: CallOptions): Promise<{ status: number; headers: IncomingHttpHeaders; body: string }>;
  close(): Promise<void>;
}

export interface CallOptions {
  door?: PipeDoor;
  workspaceId?: string;
  method?: string;
  path: string;
  headers?: Record<string, string>;
  body?: string;
  heldMs?: number;
}

export async function rig(extra: Partial<BrokerOptions> = {}): Promise<Rig> {
  const up = await upstream();
  const served: Rig["served"] = [];
  const gh = { runs: [] as string[][] };
  const revokedTokens: string[] = [];
  const live: LiveGrant[] = [{ ...GRANTS[TURN]!, agent: "codex" }];
  const waits: Rig["waits"] = [];
  const logDir = mkdtempSync(join(tmpdir(), "wsp-broker-log-"));
  const run: RunCommand = async (_file, args) => {
    gh.runs.push([...args]);
    if (args[1] === "token") return { code: 0, stdout: `${REAL.github}\n` };
    return { code: 0, stdout: JSON.stringify({ hosts: { "github.com": [{ state: "success", active: true, login: "acme-dev" }] } }) };
  };
  const services = [
    forwarding("anthropic", up.port, { door: "http", claims: req => /^\/(v1\/messages|v1\/models|api\/hello)/.test(pathOf(req.url)) && req.headers["anthropic-version"] !== undefined, credential: "anthropic", header: s => ({ "x-api-key": s }), served }),
    forwarding("chatgpt", up.port, { door: "http", claims: req => pathOf(req.url).startsWith("/backend-api/") || pathOf(req.url).startsWith("/oauth/"), credential: "chatgpt", header: s => ({ authorization: `Bearer ${s}` }), served }),
    forwarding("openai", up.port, { door: "http", claims: req => pathOf(req.url).startsWith("/v1/") && req.headers["anthropic-version"] === undefined, credential: "openai", header: s => ({ authorization: `Bearer ${s}` }), served }),
    forwarding("git", up.port, { door: "http", claims: req => (req.url ?? "").startsWith("http://github.com/"), credential: "github", header: s => ({ authorization: `Basic ${Buffer.from(`x-access-token:${s}`).toString("base64")}` }), served }),
    forwarding("gh", up.port, { door: "gh", claims: req => req.headers.host === "api.github.com", credential: "github", header: s => ({ authorization: `token ${s}` }), served }),
    forwarding("gh", up.port, { door: "gh", claims: req => req.headers.host === STORAGE_HOST, header: () => ({}), served }),
  ];
  const broker = createBroker({
    services,
    authorize: async token => (revokedTokens.includes(token) ? undefined : GRANTS[token]),
    workspace: async id => (id === WS || id === OTHER_WS ? { agent: "claude", repo: { owner: "acme", name: "lab", id: 7 }, trackers: [] } : undefined),
    live: async () => live,
    storageHost: host => host === STORAGE_HOST,
    revokeTokens: async id => {
      for (const [token, grant] of Object.entries(GRANTS)) if (grant.workspaceId === id) revokedTokens.push(token);
    },
    credentials: {
      vault: () => ({ ANTHROPIC_API_KEY: REAL.anthropic, OPENAI_API_KEY: REAL.openai }),
      chatgpt: async () => ({ accessToken: placeholderJwt("real-chatgpt-access"), accountId: "acct-acme" }),
      run,
    },
    log: brokerLogFile(logDir),
    waited: (grant, heldMs) => waits.push({ grant, heldMs }),
    ...extra,
  });
  const call: Rig["call"] = o =>
    new Promise((resolve, reject) => {
      const [host, client] = pipePair();
      broker.take(o.workspaceId ?? WS, { op: "pipe.open", door: o.door ?? "http", heldMs: o.heldMs ?? 0 }, host);
      const req = request({ createConnection: () => client as never, method: o.method ?? "GET", path: o.path, headers: { host: "127.0.0.1:7071", ...o.headers } }, res => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () => resolve({ status: res.statusCode ?? 0, headers: res.headers, body: Buffer.concat(chunks).toString("utf8") }));
      });
      req.on("error", reject);
      req.end(o.body);
    });
  return {
    broker,
    up,
    served,
    gh,
    revokedTokens,
    live,
    logDir,
    waits,
    call,
    async close() {
      broker.close();
      await up.close();
      rmSync(logDir, { recursive: true, force: true });
    },
  };
}

export const CLAUDE = { "anthropic-version": "2023-06-01", "content-type": "application/json" };
export const proxyLogin = (token: string): string => `Basic ${Buffer.from(`wsp:${token}`).toString("base64")}`;
