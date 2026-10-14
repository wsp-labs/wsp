// SPDX-License-Identifier: AGPL-3.0-only
// The broker as the host serves it: a turn on a machine at a provider calls
// one of its daemon's doors, the daemon hands the connection to a pipe the
// runtime's pool dialled, the host's broker reads the token it carries off the
// host's own store, and the Anthropic service adds the person's key on the way
// to a recorded upstream. Before the host serves and once the broker stops the
// switch reads not ready, and a turn is back on the vault's road.
import { randomUUID } from "node:crypto";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer, request, type IncomingHttpHeaders, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Duplex, PassThrough } from "node:stream";
import { afterEach, describe, expect, it } from "vitest";
import { WebSocketServer, type WebSocket } from "ws";
import { HOST_TOKEN_ENV, PIPE_DAEMON_VERSION, THREAD_ENV, WS_PATH, type TurnResult } from "@wsp/protocol";
import { createRuntime, memoryStore, type HarnessAdapterContext, type HarnessAdapterFactory, type Runtime } from "@wsp/runtime";
import { createOn, stubBackend, tokenGuest, type StubBackend, type StubMachine } from "../../runtime/test/stub-backend.js";
import { WsClient } from "../../runtime/test/ws-client.js";
import { anthropicService } from "../src/anthropic/service.js";
import { REVOKED_SAID } from "../src/broker/refuse.js";
import { brokerLogDir, hostBroker, type HostBroker } from "../src/server-broker.js";
import { startHost, type HostHandle } from "../src/server.js";

// Built at run time so no literal in the tree looks like a live key to a scanner.
const REAL_KEY = ["sk", "ant", "api03", `fake${"0".repeat(28)}fake`].join("-");
const VAULT = { ANTHROPIC_API_KEY: REAL_KEY };
/** Over the five seconds a hold must pass to be written into the thread. */
const HELD_MS = 6_000;

const cleanups: Array<() => unknown> = [];
afterEach(async () => {
  for (const fn of cleanups.splice(0).reverse()) await fn();
});

/** A loopback upstream that keeps the headers of every request that reaches it and answers 200, holding a request
 * that asks to be held until `release` and refusing one that asks to be refused. */
async function upstream(): Promise<{ url: string; seen: IncomingHttpHeaders[]; release(): void }> {
  const seen: IncomingHttpHeaders[] = [];
  const held: Array<() => void> = [];
  const server: Server = createServer((req, res) => {
    seen.push(req.headers);
    req.resume();
    const answer = (): void => void res.writeHead(200, { "content-type": "application/json" }).end(JSON.stringify({ type: "message", content: [] }));
    const refuse = (): void => void res.writeHead(401, { "content-type": "application/json" }).end(JSON.stringify({ type: "error", error: { type: "authentication_error" } }));
    req.on("end", () => (req.headers["x-acme-hold"] !== undefined ? held.push(answer) : req.headers["x-acme-refuse"] !== undefined ? refuse() : answer()));
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  cleanups.push(() => new Promise<void>(resolve => server.close(() => resolve())));
  return { url: `http://127.0.0.1:${(server.address() as AddressInfo).port}`, seen, release: () => held.splice(0).forEach(answer => answer()) };
}

type Daemon = Awaited<ReturnType<typeof standInDaemon>>;

/** A machine's daemon as the pool meets it: a link is answered with its hello at a version that serves pipes, and a
 * pipe is parked until a local connection reaches the HTTP door, which `door` plays. */
async function standInDaemon(): Promise<{ url: string; parked: WebSocket[]; door(heldMs: number): Duplex }> {
  const http = createServer();
  const wss = new WebSocketServer({ noServer: true });
  const parked: WebSocket[] = [];
  http.on("upgrade", (req, socket, head) => wss.handleUpgrade(req, socket, head, ws => wss.emit("connection", ws, req)));
  wss.on("connection", ws => {
    ws.on("error", () => undefined);
    ws.once("message", data => {
      const frame = JSON.parse(data.toString()) as Record<string, unknown>;
      ws.send(JSON.stringify({ id: frame["id"], ok: true }));
      if (frame["pipe"] === true) {
        parked.push(ws);
        ws.once("close", () => void (parked.includes(ws) ? parked.splice(parked.indexOf(ws), 1) : undefined));
        return;
      }
      ws.send(JSON.stringify({ type: "daemon.hello", root: "/root", version: PIPE_DAEMON_VERSION }));
      ws.on("message", more => {
        const asked = JSON.parse(more.toString()) as { id?: number };
        if (typeof asked.id === "number") ws.send(JSON.stringify({ id: asked.id, ok: true }));
      });
    });
  });
  await new Promise<void>(resolve => http.listen(0, "127.0.0.1", resolve));
  cleanups.push(async () => {
    for (const ws of wss.clients) ws.terminate();
    wss.close();
    await new Promise<void>(resolve => http.close(() => resolve()));
  });
  return {
    url: `http://127.0.0.1:${(http.address() as AddressInfo).port}/`,
    parked,
    door(heldMs) {
      const ws = parked.shift();
      if (ws === undefined) throw new Error("no pipe parked");
      ws.send(JSON.stringify({ op: "pipe.open", door: "http", heldMs }));
      const local = new Duplex({
        read() {},
        write(chunk: Buffer, _encoding, done) {
          ws.send(chunk, { binary: true }, done);
        },
        final(done) {
          ws.close();
          done();
        },
      });
      local.on("error", () => undefined);
      ws.on("message", (data: Buffer, binary) => void (binary ? local.push(data) : undefined));
      ws.on("close", () => local.push(null));
      return local;
    },
  };
}

/** One model call the way Claude Code makes it on such a machine, at the daemon's HTTP door: the turn token in the
 * header its placeholder's kind rides. */
function modelCall(daemon: Daemon, ctx: HarnessAdapterContext, more: Record<string, string> = {}): Promise<{ status: number; body: string }> {
  const token = ctx.env[HOST_TOKEN_ENV] ?? "";
  const header = ctx.brokered?.kind === "key" ? { "x-api-key": token } : { authorization: `Bearer ${token}` };
  return new Promise((resolve, reject) => {
    const local = daemon.door(HELD_MS);
    const req = request(
      { createConnection: () => local as never, method: "POST", path: "/v1/messages", headers: { host: "127.0.0.1:7071", "anthropic-version": "2023-06-01", "content-type": "application/json", ...header, ...more } },
      res => {
        const chunks: Buffer[] = [];
        res.on("data", (c: Buffer) => chunks.push(c));
        res.on("end", () => resolve({ status: res.statusCode ?? 0, body: Buffer.concat(chunks).toString("utf8") }));
      },
    );
    req.on("error", reject);
    req.end(JSON.stringify({ model: "claude-haiku", max_tokens: 1, messages: [{ role: "user", content: "hi" }] }));
  });
}

/** The same call on a pipe handed straight to the runtime's pipe port, as one the daemon took before its machine went. */
function heldPipeCall(broker: HostBroker, workspaceId: string, token: string, headers: Record<string, string> = {}): Promise<{ status: number; body: string }> {
  const toHost = new PassThrough();
  const toClient = new PassThrough();
  const host = Duplex.from({ readable: toHost, writable: toClient });
  const client = Duplex.from({ readable: toClient, writable: toHost });
  for (const end of [toHost, toClient, host, client]) end.on("error", () => undefined);
  broker.port.pipe!(workspaceId, { op: "pipe.open", door: "http", heldMs: 0 }, host);
  return new Promise((resolve, reject) => {
    const req = request({ createConnection: () => client as never, method: "POST", path: "/v1/messages", headers: { host: "127.0.0.1:7071", "anthropic-version": "2023-06-01", "x-api-key": token, ...headers } }, res => {
      const chunks: Buffer[] = [];
      res.on("data", (c: Buffer) => chunks.push(c));
      res.on("end", () => resolve({ status: res.statusCode ?? 0, body: Buffer.concat(chunks).toString("utf8") }));
    });
    req.on("error", reject);
    req.end("{}");
  });
}

interface Turn {
  ctx: HarnessAdapterContext;
  answer?: { status: number; body: string };
  refused?: { status: number; body: string };
}

/** A Claude Code stand-in whose brokered turn makes two model calls through its machine's door while it runs: one the
 * upstream answers, one it refuses. */
function claudeTurns(daemon: Daemon): { factory: HarnessAdapterFactory; turns: Turn[] } {
  const turns: Turn[] = [];
  const factory: HarnessAdapterFactory = ctx => ({
    steers: false,
    start: ({ onEvent }) => {
      const sessionId = randomUUID();
      const turn: Turn = { ctx };
      if (HOST_TOKEN_ENV in ctx.env) turns.push(turn);
      const finished = (async (): Promise<TurnResult> => {
        onEvent({ type: "session.start", sessionId });
        if (ctx.brokered !== undefined) {
          await expect.poll(() => daemon.parked.length, { timeout: 10_000 }).toBeGreaterThan(0);
          turn.answer = await modelCall(daemon, ctx);
          await expect.poll(() => daemon.parked.length, { timeout: 10_000 }).toBeGreaterThan(0);
          turn.refused = await modelCall(daemon, ctx, { "x-acme-refuse": "1" });
        }
        const result: TurnResult = { status: "completed", text: "ok" };
        onEvent({ type: "turn.done", sessionId, result });
        onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
        return result;
      })();
      return { localId: sessionId, finished, interrupt: async () => {} };
    },
  });
  return { factory, turns };
}

async function served(): Promise<{ rt: Runtime; broker: HostBroker; daemon: Daemon; host: () => Promise<HostHandle>; turns: Turn[]; seen: IncomingHttpHeaders[]; release(): void; ran: string[][]; statePath: string; backend: StubBackend }> {
  const dir = mkdtempSync(join(tmpdir(), "wsp-server-broker-"));
  cleanups.push(() => rmSync(dir, { recursive: true, force: true }));
  const statePath = join(dir, "state.json");
  const webDir = join(dir, "web");
  mkdirSync(webDir, { recursive: true });
  writeFileSync(join(webDir, "index.html"), "<!doctype html><title>wsp</title><script>window.__WSP__ = {};</script>");
  const up = await upstream();
  const daemon = await standInDaemon();
  // The person's own sign-ins beyond the vault stand empty here, so the vault's key is the one added; gh is never run.
  const none = async () => undefined;
  const ran: string[][] = [];
  const broker = hostBroker({
    statePath,
    vault: () => VAULT,
    services: [anthropicService({ upstream: up.url, computer: () => "acme's laptop" })],
    signIns: { anthropic: { gateway: none, helper: none, login: none }, codex: { credential: none, claims: none } },
    run: async (file, args) => (ran.push([file, ...args]), { code: 1, stdout: "" }),
  });
  const { factory, turns } = claudeTurns(daemon);
  // Every machine the provider makes is reached at the stand-in daemon.
  const backend = stubBackend();
  backend.execImpl = tokenGuest;
  const make = backend.create.bind(backend);
  backend.create = async spec => {
    const m = (await make(spec)) as StubMachine;
    m.previewUrl = async () => ({ url: daemon.url, token: "e", expiresAt: Date.now() + 3_600_000 });
    return m;
  };
  const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: factory }, vault: () => VAULT, broker: broker.port });
  const host = async (): Promise<HostHandle> => {
    const handle = await startHost({ runtime: rt, webDir, port: 0, statePath, broker, checkKey: async () => ({ works: true, account: "acme-dev", plan: "$20 plan", running: 0, limit: 100 }) });
    let closed: Promise<void> | undefined;
    const close = (): Promise<void> => (closed ??= handle.close());
    cleanups.push(close);
    return { ...handle, close };
  };
  return { rt, broker, daemon, host, turns, seen: up.seen, release: up.release, ran, statePath, backend };
}

const runTurn = async (rt: Runtime, workspaceId: string): Promise<void> => {
  await (await rt.sessions.start(workspaceId, { prompt: "one", harness: "claude" })).finished;
};

describe("the broker the host serves", () => {
  it("carries a brokered turn's model call through the pipe and the broker, and reads not ready once stopped", async () => {
    const s = await served();
    // A runtime nobody serves yet keeps today's road: the machine is made as today, the vault's key rides the turn and
    // nothing is brokered.
    expect(s.broker.port.ready()).toBe(false);
    const ws = await createOn(s.rt, { golden: "snap_g", name: "acme-lab" });
    await runTurn(s.rt, ws.id);
    expect(s.turns).toHaveLength(1);
    expect(s.turns[0]!.ctx.brokered).toBeUndefined();
    expect(s.turns[0]!.ctx.vault).toEqual(VAULT);

    const handle = await s.host();
    expect(s.broker.port.ready()).toBe(true);
    const waited: unknown[] = [];
    s.rt.events.on("thread.waited", e => void waited.push(e));
    // The same machine's next turn, once the host serves, goes through the broker.
    await runTurn(s.rt, ws.id);
    const turn = s.turns[1]!;
    const token = turn.ctx.env[HOST_TOKEN_ENV]!;
    // The turn holds the turn token in the key's place, and nothing of the vault.
    expect(turn.ctx.brokered).toEqual({ token, kind: "key" });
    expect(JSON.stringify(turn.ctx.vault)).not.toContain(REAL_KEY);
    expect(JSON.stringify(turn.ctx.env)).not.toContain(REAL_KEY);
    // From the daemon's door through the pool's pipe and the broker, the Anthropic service sent the person's key
    // upstream and never the token.
    expect(turn.answer?.status).toBe(200);
    expect(s.seen).toHaveLength(2);
    expect(s.seen[0]!["x-api-key"]).toBe(REAL_KEY);
    expect(JSON.stringify(s.seen[0])).not.toContain(token);
    // The upstream's refusal reaches the agent as the service's 403 sentence.
    expect(turn.refused?.status).toBe(403);
    expect(turn.refused?.body).toContain("Anthropic refused the API key on acme's laptop");
    // The pool dials a replacement for the pipe the call took.
    await expect.poll(() => s.daemon.parked.length, { timeout: 10_000 }).toBeGreaterThanOrEqual(4);
    // The broker's log beside the state file holds the call against its machine.
    const days = readdirSync(brokerLogDir(s.statePath));
    const lines = days.flatMap(day => readFileSync(join(brokerLogDir(s.statePath), day), "utf8").trim().split("\n")).map(line => JSON.parse(line) as Record<string, unknown>);
    expect(lines).toEqual([
      expect.objectContaining({ machine: ws.id, token: "turn", service: "anthropic", method: "POST", path: "/v1/messages", status: 200, heldMs: HELD_MS }),
      expect.objectContaining({ machine: ws.id, token: "turn", service: "anthropic", status: 403 }),
    ]);
    // The holds the pipes carried are one row in the thread once the wait settles, and the refusal's sentence is one
    // more, in its words.
    const thread = turn.ctx.env[THREAD_ENV]!;
    await expect.poll(() => waited, { timeout: 5_000 }).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ type: "thread.waited", threadId: thread, ms: HELD_MS }),
        expect.objectContaining({ type: "thread.waited", threadId: thread, said: expect.stringMatching(/^Anthropic refused the API key on acme's laptop/) }),
      ]),
    );
    expect(waited).toHaveLength(2);
    // The turn's token died with the turn, so a call carrying it now is refused by the host's own store.
    const late = await modelCall(s.daemon, turn.ctx);
    expect(late.status).toBe(403);
    expect(s.seen).toHaveLength(2);
    // A Terminal pane's token reads as a terminal's, not a turn's.
    const pane = await s.rt.devices.mint(`terminal ${thread}`, { kind: "thread", threadId: thread, workspaceId: ws.id, rootThreadId: thread }, Date.now());
    expect((await heldPipeCall(s.broker, ws.id, pane.deviceToken)).status).toBe(200);
    await s.rt.devices.revoke(pane.deviceId);
    const paneLine = readdirSync(brokerLogDir(s.statePath)).flatMap(day => readFileSync(join(brokerLogDir(s.statePath), day), "utf8").trim().split("\n")).map(line => JSON.parse(line) as Record<string, unknown>).at(-1);
    expect(paneLine).toMatchObject({ machine: ws.id, thread, token: "terminal", status: 200 });

    // The broker ops answer on the host's socket.
    const client = await WsClient.connectTo(`ws://127.0.0.1:${handle.port}${WS_PATH}`, { token: handle.authToken });
    cleanups.push(() => client.close());
    expect(await client.request("broker.status")).toMatchObject({ ok: true, claude: "key" });
    expect(await client.request("providers.checkKey", { provider: "box", key: "bx_acme" })).toMatchObject({ ok: true, works: true, account: "acme-dev" });
    // A cloud sign-in runs the agent's own login on this computer, for the agent the request names.
    const signIns: unknown[] = [];
    Object.assign(s.rt.agents, { signIn: async (target: unknown, ask: unknown) => (signIns.push({ target, ask }), { signInId: "si-acme", leave: () => {} }) });
    expect(await client.request("agents.cloudSignIn", { agent: "claude" })).toMatchObject({ ok: true, signInId: "si-acme" });
    expect(signIns).toEqual([{ target: { placeId: "here" }, ask: { agent: "claude", login: true } }]);

    // A delete cuts the machine off as it starts, even where the provider then refuses to delete it: a call under way on
    // the doctor's grant is cut rather than answered, and a pipe it still holds is refused with the revoked sentence.
    const doctor = await s.broker.doctor.grant(ws.id);
    const underWay = heldPipeCall(s.broker, ws.id, doctor.token, { "x-acme-hold": "1" }).catch((e: Error) => e);
    await expect.poll(() => s.seen.length).toBe(4);
    // The provider refuses the delete: its first machine is this workspace's.
    s.backend.machines[0]!.kill = async () => {
      throw new Error("Boat answered 503");
    };
    await expect(s.rt.workspaces.delete(ws.id)).rejects.toThrow();
    s.release();
    expect(await underWay).toBeInstanceOf(Error);
    const revoked = await heldPipeCall(s.broker, ws.id, doctor.token);
    expect(revoked.status).toBe(403);
    expect(revoked.body).toContain(REVOKED_SAID);
    expect(s.seen).toHaveLength(4);

    // The broker stopped: the switch reads not ready, the next turn is back on the vault's road, and a pipe the
    // daemon hands over now is closed unanswered.
    s.broker.stop();
    expect(s.broker.port.ready()).toBe(false);
    const later = await createOn(s.rt, { golden: "snap_g", name: "acme-later" });
    await runTurn(s.rt, later.id);
    const after = s.turns[2]!;
    expect(after.ctx.brokered).toBeUndefined();
    expect(after.ctx.vault).toEqual(VAULT);
    const cut = await heldPipeCall(s.broker, later.id, token).catch((e: Error) => e);
    expect(cut).toBeInstanceOf(Error);
    expect(s.seen).toHaveLength(4);
    // The broker reads the workspace on every call; the project repo's id gh could not read was asked for once.
    expect(s.ran.filter(args => args[1] === "api")).toEqual([["gh", "api", expect.stringMatching(/^repos\//), "--jq", ".id"]]);
  });

  it("stops the broker first when the host closes", async () => {
    const s = await served();
    const handle = await s.host();
    expect(s.broker.port.ready()).toBe(true);
    await handle.close();
    expect(s.broker.port.ready()).toBe(false);
    await expect(s.broker.port.status!()).rejects.toThrow("not serving");
  });
});
