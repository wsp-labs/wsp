// SPDX-License-Identifier: AGPL-3.0-only
// The host's broker: every connection a process on a brokered machine opens to
// one of its daemon's doors arrives here as a pipe's bytes, and is served by an
// http.Server that is never bound to a port. The broker owns the rules every
// call shares (who is calling, the off switch, no CONNECT, no destination
// outside the services, the caller's own auth dropped, never a 401, the log);
// each service owns its own upstream and what it allows there.
import { createServer, IncomingMessage, ServerResponse } from "node:http";
import type { Socket } from "node:net";
import type { Duplex } from "node:stream";
import type { BrokerStatus, PipeDoor, PipeOpen } from "@wsp/protocol";
import { brokerCredentials, type CredentialPorts } from "./credentials.js";
import type { BrokerLog } from "./log.js";
import { offSwitch, type OffSwitch } from "./off.js";
import { CONNECT_SAID, connectorSaid, DEAD_TOKEN_SAID, FAILED_SAID, NO_TOKEN_SAID, NOWHERE_SAID, OTHER_MACHINE_SAID, REVOKED_SAID, shapeOf, UNHELD_SAID, writeRefusal, type RefusalStatus } from "./refuse.js";
import { dropCallerAuth, hostOf, isConnectorCall, isRefreshCall, pathOf, REFRESH_BODY_MAX, refreshToken, tokenOf } from "./tokens.js";
import type { BrokerContext, BrokerLogLine, BrokerService, BrokerWorkspace, Grant, LiveGrant, PushedRef, RefusalShape } from "./types.js";

export interface BrokerOptions {
  /** Checked in order; the first whose `claims` answers serves the call. */
  services: readonly BrokerService[];
  /** What a token names in the host's own token store: a turn, op or terminal token's grant, and nothing for any
   * other token, one it does not hold, or one it revoked. */
  authorize(token: string): Promise<Grant | undefined>;
  workspace(id: string): Promise<BrokerWorkspace | undefined>;
  /** The grants live on a workspace now: a connector call with no token borrows its one Codex turn, and a storage
   * host's fetch on the gh door needs a turn or a terminal there. */
  live(workspaceId: string): Promise<readonly LiveGrant[]>;
  /** Whether a host is one of GitHub's storage hosts that gh's redirects name, whose fetches carry no token and get no
   * credential; some are numbered, so this is a test and not a list. No host is one where absent. */
  storageHost?(host: string): boolean;
  /** Revokes every live turn, op and terminal token of a workspace. */
  revokeTokens(workspaceId: string): Promise<void>;
  credentials: CredentialPorts;
  /** The machines switched off; in memory alone where absent. */
  off?: OffSwitch;
  log: BrokerLog;
  /** Told once per pipe how long the daemon held it before the host took it, against the thread that called. */
  waited?(grant: Grant, heldMs: number): void;
  now?(): number;
}

export interface Broker {
  /** One pipe the host's pool took: its door, its hold and the local connection's bytes both ways. */
  take(workspaceId: string, open: PipeOpen, pipe: Duplex): void;
  /** The per-machine off switch: every call from the machine's pipes is refused from now, the ones under way are
   * cut, and the workspace's live tokens are revoked. */
  revoke(workspaceId: string): Promise<void>;
  revoked(workspaceId: string): boolean;
  status(): Promise<BrokerStatus>;
  close(): void;
}

/** One pipe as the broker holds it: whose link it came on, and the bytes it has carried each way. */
interface Pipe {
  workspaceId: string;
  door: PipeDoor;
  heldMs: number;
  up: number;
  down: number;
  /** Where the last call's line stopped counting, so a kept-alive connection's next call counts from there. */
  upMark: number;
  downMark: number;
  /** The status line of the first bytes a service wrote raw, after an upgrade. */
  rawStatus?: number;
  sniff: boolean;
  waitedSaid: boolean;
}

const sizeOf = (chunk: unknown, encoding: unknown): number =>
  typeof chunk === "string" ? Buffer.byteLength(chunk, typeof encoding === "string" ? (encoding as BufferEncoding) : "utf8") : chunk instanceof Uint8Array ? chunk.byteLength : 0;

/** Counts what a pipe carries at its own push, write and end, the calls every byte in and out goes through, so
 * nothing is copied and backpressure stays the pipe's own. */
function countBytes(duplex: Duplex, p: Pipe): void {
  const push = duplex.push.bind(duplex) as (chunk: unknown, encoding?: BufferEncoding) => boolean;
  duplex.push = ((chunk: unknown, encoding?: BufferEncoding) => {
    p.up += sizeOf(chunk, encoding);
    return push(chunk, encoding);
  }) as Duplex["push"];
  const write = duplex.write.bind(duplex) as (...a: unknown[]) => boolean;
  const end = duplex.end.bind(duplex) as (...a: unknown[]) => Duplex;
  const note = (chunk: unknown, encoding: unknown): void => {
    if (p.sniff && p.rawStatus === undefined) {
      const head = typeof chunk === "string" ? chunk.slice(0, 16) : chunk instanceof Uint8Array ? Buffer.from(chunk.subarray(0, 16)).toString("latin1") : "";
      const status = /^HTTP\/1\.[01] (\d{3})/.exec(head)?.[1];
      if (status !== undefined) p.rawStatus = Number(status);
    }
    p.down += sizeOf(chunk, encoding);
  };
  duplex.write = ((chunk: unknown, ...rest: unknown[]) => {
    note(chunk, rest[0]);
    return write(chunk, ...rest);
  }) as Duplex["write"];
  duplex.end = ((chunk?: unknown, ...rest: unknown[]) => {
    if (typeof chunk !== "function") note(chunk, rest[0]);
    return end(chunk, ...rest);
  }) as Duplex["end"];
}

/** One call's line while the call runs: the hold it carries, and what the broker and its service learn on the way. */
interface CallLine {
  heldMs: number;
  note: { pushes: PushedRef[] };
  grant(g: Grant): void;
  service(s: BrokerService["id"]): void;
  done(status: number): void;
}

/** A live grant as a call carries it, without the agent the host read it with. */
const grantOnly = ({ agent: _agent, ...grant }: LiveGrant): Grant => grant;

/** A request's body when it is no longer than `max`, and nothing for a longer one, whose rest is drained unread. */
function bodyUpTo(req: IncomingMessage, max: number): Promise<Buffer | undefined> {
  const declared = Number(req.headers["content-length"]);
  if (Number.isFinite(declared) && declared > max) {
    req.resume();
    return Promise.resolve(undefined);
  }
  return new Promise(resolve => {
    const chunks: Buffer[] = [];
    let size = 0;
    const done = (body: Buffer | undefined): void => {
      req.off("data", onData).off("end", onEnd).off("error", onError);
      if (body === undefined) req.resume();
      resolve(body);
    };
    const onData = (chunk: Buffer): void => {
      size += chunk.byteLength;
      if (size > max) return done(undefined);
      chunks.push(chunk);
    };
    const onEnd = (): void => done(Buffer.concat(chunks));
    const onError = (): void => done(undefined);
    req.on("data", onData).on("end", onEnd).on("error", onError);
  });
}

/** The request again with a body already read, for the service to read as if nothing had: same head, same socket. */
function replayed(req: IncomingMessage, body: Buffer): IncomingMessage {
  const again = new IncomingMessage(req.socket as Socket);
  again.httpVersion = req.httpVersion;
  again.httpVersionMajor = req.httpVersionMajor;
  again.httpVersionMinor = req.httpVersionMinor;
  again.method = req.method;
  again.url = req.url;
  again.headers = req.headers;
  again.headersDistinct = req.headersDistinct;
  again.rawHeaders = req.rawHeaders;
  again.complete = true;
  again.push(body);
  again.push(null);
  return again;
}

const AUTH_CHALLENGES = ["www-authenticate", "proxy-authenticate"];

/** Holds a response to rule 7: a 401 or a 407 from anywhere goes out as a 403 that tells the agent to stop, with no
 * challenge header, since either status sends a client to a login prompt on a machine that has nothing to log in
 * with. */
function neverUnauthorized(res: ServerResponse): void {
  const writeHead = res.writeHead.bind(res) as (...a: unknown[]) => ServerResponse;
  res.writeHead = ((status: number, ...rest: unknown[]) => {
    const refused = status === 401 || status === 407;
    // A refused answer drops the upstream's own retry word too, so the one set below is the only one sent.
    const dropped = refused ? [...AUTH_CHALLENGES, "x-should-retry"] : AUTH_CHALLENGES;
    for (const name of dropped) res.removeHeader(name);
    const headers = rest.find(r => typeof r === "object" && r !== null);
    if (Array.isArray(headers)) {
      const flat = headers.flat() as unknown[];
      const kept: unknown[] = [];
      for (let i = 0; i + 1 < flat.length; i += 2) if (!dropped.includes(String(flat[i]).toLowerCase())) kept.push(flat[i], flat[i + 1]);
      if (refused) kept.push("x-should-retry", "false");
      headers.splice(0, headers.length, ...kept);
    } else if (headers !== undefined) {
      const obj = headers as Record<string, unknown>;
      for (const name of Object.keys(obj)) if (dropped.includes(name.toLowerCase())) delete obj[name];
      if (refused) obj["x-should-retry"] = "false";
    } else if (refused) {
      res.setHeader("x-should-retry", "false");
    }
    if (!refused) return writeHead(status, ...rest);
    return writeHead(403, ...rest.filter(r => typeof r !== "string"));
  }) as ServerResponse["writeHead"];
}

export function createBroker(o: BrokerOptions): Broker {
  const now = o.now ?? Date.now;
  const credentials = brokerCredentials(o.credentials);
  const pipes = new WeakMap<Duplex, Pipe>();
  const live = new Map<string, Set<Duplex>>();
  const off = o.off ?? offSwitch();
  let closed = false;

  const server = createServer({ requestTimeout: 0 });

  const isOff = (workspaceId: string): boolean => off.has(workspaceId);

  /** The grant a token names, held to the workspace whose link the pipe came on. */
  const granted = async (token: string, p: Pipe): Promise<Grant | { said: string }> => {
    const grant = await o.authorize(token);
    if (grant === undefined) return { said: DEAD_TOKEN_SAID };
    return grant.workspaceId === p.workspaceId ? grant : { said: OTHER_MACHINE_SAID };
  };

  /** The grant a call runs under and the request its service reads, or the sentence it is refused with. Three calls
   * carry no token where every other does: a connector call borrows the workspace's one Codex turn, Codex's refresh
   * carries it in its body, and a storage host's fetch on the gh door, which gets no credential, needs a live turn
   * or terminal on the workspace. */
  const grantOf = async (req: IncomingMessage, p: Pipe): Promise<{ grant: Grant; req: IncomingMessage } | { said: string }> => {
    const token = tokenOf(req, p.door);
    const withReq = (g: Grant | { said: string }, r: IncomingMessage = req) => ("said" in g ? g : { grant: g, req: r });
    if (token !== undefined) return withReq(await granted(token, p));
    if (isConnectorCall(req, p.door)) {
      const turns = (await o.live(p.workspaceId)).filter(g => g.workspaceId === p.workspaceId && g.kind === "turn" && g.agent === "codex");
      return turns.length === 1 ? withReq(grantOnly(turns[0]!)) : { said: connectorSaid(turns.length) };
    }
    if (isRefreshCall(req, p.door)) {
      const body = await bodyUpTo(req, REFRESH_BODY_MAX);
      const inBody = body === undefined ? undefined : refreshToken(body);
      return inBody === undefined ? { said: NO_TOKEN_SAID } : withReq(await granted(inBody, p), replayed(req, body!));
    }
    if (p.door === "gh" && req.method === "GET" && o.storageHost?.(hostOf(req)) === true) {
      const held = (await o.live(p.workspaceId)).filter(g => g.workspaceId === p.workspaceId && (g.kind === "turn" || g.kind === "terminal"));
      if (held.length === 0) return { said: NO_TOKEN_SAID };
      return withReq(held.length === 1 ? grantOnly(held[0]!) : { kind: held[0]!.kind, workspaceId: p.workspaceId });
    }
    return { said: NO_TOKEN_SAID };
  };

  /** The context one call's service is handed: credentials, its own workspace only, refusals and its log fields. */
  const contextFor = (p: Pipe, note: CallLine["note"]): BrokerContext => ({
    credential: kind => credentials.credential(kind),
    async workspace(id) {
      const w = id === p.workspaceId ? await o.workspace(id) : undefined;
      if (w === undefined) throw new Error(`${id} is not the machine this pipe came from, or that machine is gone`);
      return w;
    },
    refuse: (res, status, said, shape) => writeRefusal(res, status, said, shape),
    log(line) {
      if (Array.isArray(line.pushes)) note.pushes.push(...line.pushes);
    },
  });

  /** Writes the call's one line when its response is done, counting the bytes since the last call on this pipe. */
  const lineFor = (req: IncomingMessage, p: Pipe): CallLine => {
    const started = now();
    const heldMs = p.heldMs;
    p.heldMs = 0;
    const note: CallLine["note"] = { pushes: [] };
    let grant: Grant | undefined;
    let service: BrokerService["id"] | undefined;
    let written = false;
    return {
      note,
      heldMs,
      grant: g => (grant = g),
      service: s => (service = s),
      done(status) {
        if (written) return;
        written = true;
        const line: BrokerLogLine = {
          at: new Date(started).toISOString(),
          machine: p.workspaceId,
          ...(grant?.threadId !== undefined ? { thread: grant.threadId } : {}),
          ...(grant !== undefined ? { token: grant.kind } : {}),
          ...(service !== undefined ? { service } : {}),
          method: req.method ?? "",
          path: pathOf(req.url),
          status,
          bytesUp: p.up - p.upMark,
          bytesDown: p.down - p.downMark,
          ms: now() - started,
          heldMs,
          ...(note.pushes.length > 0 ? { pushes: note.pushes } : {}),
        };
        p.upMark = p.up;
        p.downMark = p.down;
        try {
          o.log.write(line);
        } catch {
          // A log that cannot be written never fails the call it records.
        }
      },
    };
  };

  /** The checks every call passes before a service sees it, in the broker policy's order: the off switch, the token,
   * its workspace, then the caller's own auth dropped. Answers the grant and the service, or refuses and answers
   * nothing. */
  const admit = async (
    req: IncomingMessage,
    res: ServerResponse,
    p: Pipe,
    line: CallLine,
  ): Promise<{ grant: Grant; req: IncomingMessage; service: BrokerService } | undefined> => {
    const shape: RefusalShape = shapeOf(req, p.door);
    const refuse = (status: RefusalStatus, said: string): undefined => {
      dropCallerAuth(req);
      writeRefusal(res, status, said, shape);
      return undefined;
    };
    if (isOff(p.workspaceId)) return refuse(403, REVOKED_SAID);
    const admitted = await grantOf(req, p);
    dropCallerAuth(req);
    if (isOff(p.workspaceId)) return refuse(403, REVOKED_SAID);
    if ("said" in admitted) return refuse(403, admitted.said);
    const { grant } = admitted;
    line.grant(grant);
    const workspace = await o.workspace(p.workspaceId);
    if (isOff(p.workspaceId)) return refuse(403, REVOKED_SAID);
    if (workspace === undefined) return refuse(403, UNHELD_SAID);
    if (!p.waitedSaid) {
      p.waitedSaid = true;
      if (line.heldMs > 0) o.waited?.(grant, line.heldMs);
    }
    const service = o.services.find(s => s.claims(req, p.door, workspace.agent));
    if (service === undefined) return refuse(p.door === "gh" || /^[a-z][a-z0-9+.-]*:\/\//i.test(req.url ?? "") ? 403 : 404, NOWHERE_SAID);
    line.service(service.id);
    return { grant, req: admitted.req, service };
  };

  const brokerRequest = (p: Pipe, grant: Grant, req: IncomingMessage, res: ServerResponse, heldMs: number) => ({ workspaceId: p.workspaceId, door: p.door, grant, req, res, heldMs });

  server.on("request", (req: IncomingMessage, res: ServerResponse) => {
    const p = pipes.get(req.socket as Duplex);
    if (p === undefined) return res.destroy();
    const line = lineFor(req, p);
    neverUnauthorized(res);
    res.on("close", () => line.done(res.headersSent ? res.statusCode : 0));
    void (async () => {
      const admitted = await admit(req, res, p, line);
      if (admitted === undefined) return;
      try {
        await admitted.service.serve(brokerRequest(p, admitted.grant, admitted.req, res, line.heldMs), contextFor(p, line.note));
      } catch {
        writeRefusal(res, 503, FAILED_SAID, shapeOf(req, p.door));
      }
    })().catch(() => res.destroy());
  });

  server.on("upgrade", (req: IncomingMessage, socket: Duplex, head: Buffer) => {
    const p = pipes.get(socket);
    if (p === undefined) return socket.destroy();
    const line = lineFor(req, p);
    const res = new ServerResponse(req);
    res.assignSocket(socket as Parameters<ServerResponse["assignSocket"]>[0]);
    p.sniff = true;
    socket.on("close", () => line.done(res.headersSent ? res.statusCode : (p.rawStatus ?? 0)));
    // An ended pipe's ws stream emits no close until something destroys it (measured on ws 8.21), and the line waits on that close.
    res.on("finish", () => socket.destroy());
    void (async () => {
      const admitted = await admit(req, res, p, line);
      if (admitted === undefined) return;
      const upgrade = admitted.service.upgrade;
      if (upgrade === undefined) return writeRefusal(res, 404, NOWHERE_SAID, shapeOf(req, p.door));
      res.detachSocket(socket as Parameters<ServerResponse["detachSocket"]>[0]);
      try {
        await upgrade.call(admitted.service, brokerRequest(p, admitted.grant, req, res, line.heldMs), socket, head, contextFor(p, line.note));
      } catch {
        socket.destroy();
      }
    })().catch(() => socket.destroy());
  });

  server.on("connect", (req: IncomingMessage, socket: Duplex) => {
    const p = pipes.get(socket);
    if (p === undefined) return socket.destroy();
    const line = lineFor(req, p);
    const res = new ServerResponse(req);
    res.assignSocket(socket as Parameters<ServerResponse["assignSocket"]>[0]);
    socket.on("close", () => line.done(res.statusCode));
    const shape = shapeOf(req, p.door);
    dropCallerAuth(req);
    writeRefusal(res, 403, isOff(p.workspaceId) ? REVOKED_SAID : CONNECT_SAID, shape);
    res.on("finish", () => socket.destroy());
  });

  return {
    take(workspaceId, open, pipe) {
      pipe.on("error", () => undefined);
      if (closed) return void pipe.destroy();
      const p: Pipe = { workspaceId, door: open.door, heldMs: Math.max(0, open.heldMs), up: 0, down: 0, upMark: 0, downMark: 0, sniff: false, waitedSaid: false };
      pipes.set(pipe, p);
      countBytes(pipe, p);
      const set = live.get(workspaceId) ?? new Set<Duplex>();
      live.set(workspaceId, set);
      set.add(pipe);
      pipe.once("close", () => {
        set.delete(pipe);
        if (set.size === 0 && live.get(workspaceId) === set) live.delete(workspaceId);
      });
      server.emit("connection", pipe);
    },
    async revoke(workspaceId) {
      off.add(workspaceId);
      for (const pipe of live.get(workspaceId) ?? []) pipe.destroy();
      await o.revokeTokens(workspaceId);
    },
    revoked: isOff,
    status: () => credentials.status(),
    close() {
      closed = true;
      for (const set of live.values()) for (const pipe of set) pipe.destroy();
      live.clear();
    },
  };
}
