// SPDX-License-Identifier: AGPL-3.0-only
// Byte-for-byte pass of one request to a fixed upstream, and of a websocket upgrade after its 101. Bodies stream
// both ways unread; a redirect goes back as it came, since node's client never follows one.
import { Agent as HttpAgent, request as httpRequest, ServerResponse, type IncomingMessage } from "node:http";
import { Agent as HttpsAgent, request as httpsRequest } from "node:https";
import type { Socket } from "node:net";
import type { Duplex } from "node:stream";

const HOP = new Set(["connection", "keep-alive", "proxy-connection", "transfer-encoding", "te", "trailer", "upgrade", "host"]);
const CALLER_AUTH = new Set(["authorization", "x-api-key", "proxy-authorization", "cookie"]);
const UPGRADE_KEEPS = new Set(["connection", "upgrade"]);

const httpAgent = new HttpAgent({ keepAlive: true, maxFreeSockets: 8 });
const httpsAgent = new HttpsAgent({ keepAlive: true, maxFreeSockets: 8 });

export type Refusal = (res: ServerResponse, status: 403 | 503, said: string) => void;
export interface Sentences {
  refused: string;
  unreachable: string;
}

// The caller's headers in their order and case, less its auth, hop-by-hop headers and every name the host owns.
export function outgoingHeaders(req: IncomingMessage, owned: readonly string[], set: Record<string, string>, upgrade: boolean): string[] {
  const replaced = new Set([...owned, ...Object.keys(set)].map((name) => name.toLowerCase()));
  const out: string[] = [];
  for (let i = 0; i < req.rawHeaders.length; i += 2) {
    const name = req.rawHeaders[i]!;
    const lower = name.toLowerCase();
    if (CALLER_AUTH.has(lower) || replaced.has(lower)) continue;
    if (HOP.has(lower) && !(upgrade && UPGRADE_KEEPS.has(lower))) continue;
    out.push(name, req.rawHeaders[i + 1]!);
  }
  for (const [name, value] of Object.entries(set)) out.push(name, value);
  return out;
}

const RESPONSE_HOP = new Set(["connection", "keep-alive", "transfer-encoding"]);

function responseHeaders(raw: string[], also: readonly string[] = []): string[] {
  const out: string[] = [];
  for (let i = 0; i < raw.length; i += 2) {
    const lower = raw[i]!.toLowerCase();
    if (RESPONSE_HOP.has(lower) || also.includes(lower)) continue;
    out.push(raw[i]!, raw[i + 1]!);
  }
  return out;
}

// Where a request goes, and the address dialed for it when that differs (a stand-in in tests).
export interface Target {
  url: URL;
  via?: URL;
}

function open(target: Target, method: string, headers: string[], pooled: boolean) {
  const at = target.via ?? target.url;
  const https = at.protocol === "https:";
  const agent = pooled ? (https ? httpsAgent : httpAgent) : false;
  // Node adds no Host of its own when the headers are a raw list.
  return (https ? httpsRequest : httpRequest)(at, { method, headers: ["Host", target.url.host, ...headers], agent });
}

// A rewrite reads a 200 reply whole, so it is only for small JSON answers; past the cap the reply is cut.
const REWRITE_MAX = 1024 * 1024;
export type Rewrite = (body: Buffer) => Buffer | undefined;

function answerRewritten(u: IncomingMessage, res: ServerResponse, rewrite: Rewrite): void {
  const chunks: Buffer[] = [];
  let size = 0;
  u.on("data", (chunk: Buffer) => {
    size += chunk.length;
    if (size > REWRITE_MAX) return void res.destroy();
    chunks.push(chunk);
  });
  u.on("end", () => {
    const body = Buffer.concat(chunks);
    const out = (u.statusCode === 200 && rewrite(body)) || body;
    res.writeHead(u.statusCode ?? 502, u.statusMessage, [...responseHeaders(u.rawHeaders, ["content-length"]), "Content-Length", String(out.length)]);
    res.end(out);
  });
}

export function passHttp(req: IncomingMessage, res: ServerResponse, target: Target, headers: string[], refuse: Refusal, said: Sentences, rewrite?: Rewrite): Promise<void> {
  return new Promise((resolve) => {
    const up = open(target, req.method ?? "GET", headers, true);
    res.on("close", () => {
      if (!res.writableFinished) up.destroy();
      resolve();
    });
    up.on("response", (u) => {
      if (u.statusCode === 401) {
        u.resume();
        refuse(res, 403, said.refused);
        return;
      }
      u.on("error", () => res.destroy());
      u.on("aborted", () => res.destroy());
      if (rewrite) return answerRewritten(u, res, rewrite);
      res.writeHead(u.statusCode ?? 502, u.statusMessage, responseHeaders(u.rawHeaders));
      res.flushHeaders();
      u.pipe(res);
    });
    up.on("error", () => {
      if (res.headersSent) res.destroy();
      else refuse(res, 503, said.unreachable);
    });
    req.on("error", () => up.destroy());
    req.pipe(up);
  });
}

// A response written on an upgrade's socket, for a refusal or an upstream that answered without switching.
export function responseOn(req: IncomingMessage, socket: Duplex): ServerResponse {
  const res = new ServerResponse(req);
  res.shouldKeepAlive = false;
  res.assignSocket(socket as Socket);
  res.on("finish", () => {
    res.detachSocket(socket as Socket);
    socket.end();
  });
  return res;
}

export function passUpgrade(req: IncomingMessage, socket: Duplex, head: Buffer, target: Target, headers: string[], refuse: Refusal, said: Sentences): Promise<void> {
  return new Promise((resolve) => {
    const up = open(target, req.method ?? "GET", headers, false);
    let done = false;
    const finish = () => {
      if (!done) resolve();
      done = true;
    };
    socket.on("error", () => up.destroy());
    socket.on("close", () => {
      up.destroy();
      finish();
    });
    up.on("upgrade", (u, upSocket, upHead) => {
      const lines = [`HTTP/1.1 ${u.statusCode} ${u.statusMessage ?? ""}`];
      for (let i = 0; i < u.rawHeaders.length; i += 2) lines.push(`${u.rawHeaders[i]}: ${u.rawHeaders[i + 1]}`);
      socket.write(`${lines.join("\r\n")}\r\n\r\n`);
      if (upHead.length > 0) socket.write(upHead);
      if (head.length > 0) upSocket.write(head);
      upSocket.on("error", () => socket.destroy());
      upSocket.on("close", () => socket.destroy());
      socket.on("close", () => upSocket.destroy());
      upSocket.pipe(socket);
      socket.pipe(upSocket);
    });
    up.on("response", (u) => {
      const res = responseOn(req, socket);
      if (u.statusCode === 401) {
        u.resume();
        refuse(res, 403, said.refused);
        return;
      }
      res.writeHead(u.statusCode ?? 502, u.statusMessage, responseHeaders(u.rawHeaders));
      u.pipe(res);
    });
    up.on("error", () => {
      if (socket.destroyed) return;
      refuse(responseOn(req, socket), 503, said.unreachable);
    });
    up.end();
  });
}
