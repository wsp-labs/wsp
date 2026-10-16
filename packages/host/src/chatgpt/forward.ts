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

/** How long a model call may go with no reply started, plus a second per 32 KB it sends, and what to do once it is cut. */
export interface FirstByteWait {
  ms: number;
  cut: () => void;
}

const waitFor = (wait: FirstByteWait, bytes: number) => wait.ms + Math.floor(bytes / 32_768) * 1_000;

/** Reads the frame headers off one direction of a websocket, calling onData with each message's payload bytes as
 * they pass, so a ping, a pong or a close is never taken for a call or for its reply. */
function messageBytes(onData: (bytes: number) => void): (chunk: Buffer) => void {
  let head: number[] = [];
  let left = 0;
  let data = false;
  return (chunk) => {
    for (let at = 0; at < chunk.length; ) {
      if (left > 0) {
        const n = Math.min(left, chunk.length - at);
        left -= n;
        at += n;
        if (data) onData(n);
        continue;
      }
      head.push(chunk[at++]!);
      if (head.length < 2) continue;
      const short = head[1]! & 0x7f;
      if (head.length < 2 + (short === 126 ? 2 : short === 127 ? 8 : 0) + (head[1]! & 0x80 ? 4 : 0)) continue;
      left = short === 126 ? (head[2]! << 8) | head[3]! : short === 127 ? Number(Buffer.from(head.slice(2, 10)).readBigUInt64BE()) : short;
      // Opcodes 8 and up are control frames.
      data = (head[0]! & 0x0f) < 8;
      head = [];
      if (data && left === 0) onData(0);
    }
  };
}

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

/** wait: once it passes with no reply started, the call is cut and answered as an event stream that ends before its
 * last event, which Codex retries at once and says so. */
export function passHttp(req: IncomingMessage, res: ServerResponse, target: Target, headers: string[], refuse: Refusal, said: Sentences, opts: { rewrite?: Rewrite; wait?: FirstByteWait } = {}): Promise<void> {
  const { rewrite, wait } = opts;
  return new Promise((resolve) => {
    const up = open(target, req.method ?? "GET", headers, true);
    const timer = wait === undefined ? undefined : setTimeout(() => {
      wait.cut();
      res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-store" });
      res.end();
      up.destroy();
    }, waitFor(wait, Number(req.headers["content-length"] ?? 0)));
    res.on("close", () => {
      clearTimeout(timer);
      if (!res.writableFinished) up.destroy();
      resolve();
    });
    up.on("response", (u) => {
      clearTimeout(timer);
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
      if (res.writableEnded) return;
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
  // Node's server stops passing the socket's drain to its response once it upgrades, so a pipe into res would stall.
  socket.on("drain", () => void (res.writableNeedDrain && res.emit("drain")));
  res.on("finish", () => {
    res.detachSocket(socket as Socket);
    socket.destroy();
  });
  return res;
}

/** Cuts a websocket, both ways, once the caller has sent a message and the upstream has sent none back within the
 * message's wait, which Codex reads as a dropped stream and retries. A ping, a pong or a close is never a message.
 * The wait is asked for as each message starts, since one socket carries a thread's calls from turn to turn. */
function cutUnanswered(socket: Duplex, upSocket: Duplex, next: () => FirstByteWait | undefined): { sent: (chunk: Buffer) => void; got: (chunk: Buffer) => void } {
  let since: number | undefined;
  let wait: FirstByteWait | undefined;
  let bytes = 0;
  let timer: NodeJS.Timeout | undefined;
  socket.on("close", () => clearTimeout(timer));
  const sent = messageBytes((n) => {
    if (since === undefined) {
      since = Date.now();
      wait = next();
    }
    bytes += n;
    const w = wait;
    if (w === undefined) return;
    clearTimeout(timer);
    timer = setTimeout(() => {
      w.cut();
      socket.destroy();
      upSocket.destroy();
    }, since + waitFor(w, bytes) - Date.now());
  });
  const got = messageBytes(() => {
    since = undefined;
    bytes = 0;
    clearTimeout(timer);
  });
  return { sent, got };
}

/** wait: the wait for a reply to start after each message the caller sends, as passHttp's, asked for as it starts. */
export function passUpgrade(req: IncomingMessage, socket: Duplex, head: Buffer, target: Target, headers: string[], refuse: Refusal, said: Sentences, wait?: () => FirstByteWait | undefined): Promise<void> {
  return new Promise((resolve) => {
    const up = open(target, req.method ?? "GET", headers, false);
    let answer: ServerResponse | undefined;
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
      if (wait !== undefined) {
        const watch = cutUnanswered(socket, upSocket, wait);
        watch.got(upHead);
        watch.sent(head);
        upSocket.on("data", watch.got);
        socket.on("data", watch.sent);
      }
      upSocket.pipe(socket);
      socket.pipe(upSocket);
    });
    up.on("response", (u) => {
      const res = (answer = responseOn(req, socket));
      if (u.statusCode === 401) {
        u.resume();
        refuse(res, 403, said.refused);
        return;
      }
      u.on("error", () => res.destroy());
      u.on("aborted", () => res.destroy());
      res.writeHead(u.statusCode ?? 502, u.statusMessage, responseHeaders(u.rawHeaders));
      u.pipe(res);
    });
    up.on("error", () => {
      if (answer !== undefined) answer.destroy();
      else if (!socket.destroyed) refuse(responseOn(req, socket), 503, said.unreachable);
    });
    up.end();
  });
}
