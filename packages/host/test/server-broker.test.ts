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
import { LocalBackend } from "@wsp/engine";
import { writeStub } from "../../protocol/test/stub-script.js";
import { createOn, fakeLocal, stubBackend, tokenGuest, type StubBackend, type StubMachine } from "../../runtime/test/stub-backend.js";
import { WsClient } from "../../runtime/test/ws-client.js";
import { anthropicService } from "../src/anthropic/service.js";
import { chatgptService } from "../src/chatgpt/service.js";
import { DEAD_TOKEN_SAID, REVOKED_SAID, UNHELD_SAID } from "../src/broker/refuse.js";
import type { BrokerService } from "../src/broker/types.js";
import { brokerLogDir, hostBroker, type HostBroker } from "../src/server-broker.js";
import { startHost, type HostHandle } from "../src/server.js";

// Built at run time so no literal in the tree looks like a live key to a scanner.
const REAL_KEY = ["sk", "ant", "api03", `fake${"0".repeat(28)}fake`].join("-");
const VAULT = { ANTHROPIC_API_KEY: REAL_KEY };
const CODEX_CLAIMS = { accountId: "acct-acme", planType: "plus" };
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

/** gh on this computer as GitHub answers it for any repo: its id and name, and the tip a branch of record starts from. */
const GH = `#!/bin/sh
[ "$*" = "auth token --hostname github.com" ] && { echo gho_TESTONLY; exit 0; }
case "$2" in
  repos/*/commits/*) echo 1d2c3b4a5f6e7d8c9b0a1d2c3b4a5f6e7d8c9b0a ;;
  repos/*) r=\${2#repos/}; printf '4242\\t%s\\t%s\\n' "\${r%%/*}" "\${r#*/}" ;;
  *) echo "gh: Not Found (HTTP 404)" >&2; exit 1 ;;
esac
`;

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
        destroy(err, done) {
          ws.close();
          done(err);
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

/** One request over a connection to the broker. */
function callOn(local: Duplex, o: { method: string; path: string; headers: Record<string, string>; body?: string }): Promise<{ status: number; body: string }> {
  return new Promise((resolve, reject) => {
    const req = request({ createConnection: () => local as never, method: o.method, path: o.path, headers: o.headers }, res => {
      const chunks: Buffer[] = [];
      res.on("data", (c: Buffer) => chunks.push(c));
      res.on("end", () => resolve({ status: res.statusCode ?? 0, body: Buffer.concat(chunks).toString("utf8") }));
    });
    req.on("error", reject);
    req.end(o.body);
  });
}

/** A pipe handed straight to the runtime's pipe port, as one the daemon took before its machine went: its client end. */
function handed(broker: HostBroker, workspaceId: string): Duplex {
  const toHost = new PassThrough();
  const toClient = new PassThrough();
  const host = Duplex.from({ readable: toHost, writable: toClient });
  const client = Duplex.from({ readable: toClient, writable: toHost });
  for (const end of [toHost, toClient, host, client]) end.on("error", () => undefined);
  broker.port.pipe!(workspaceId, { op: "pipe.open", door: "http", heldMs: 0 }, host);
  return client;
}

/** A model call on a pipe handed straight to the runtime's pipe port. */
const heldPipeCall = (broker: HostBroker, workspaceId: string, token: string, headers: Record<string, string> = {}): Promise<{ status: number; body: string }> =>
  callOn(handed(broker, workspaceId), { method: "POST", path: "/v1/messages", headers: { host: "127.0.0.1:7071", "anthropic-version": "2023-06-01", "x-api-key": token, ...headers }, body: "{}" });

/** git's first call of a clone through the host's git road, as brokeredGitEnv sends it: an absolute-form URL to the
 * proxy at the daemon's door, the token as the proxy's password. */
const gitCall = (local: Duplex, token: string): Promise<{ status: number; body: string }> =>
  callOn(local, { method: "GET", path: "http://github.com/acme/lab.git/info/refs?service=git-upload-pack", headers: { host: "github.com", "proxy-authorization": `Basic ${Buffer.from(`wsp:${token}`).toString("base64")}` } });

/** The git road as the broker hands it a call: a stand-in that answers every github.com fetch it is given. */
const GIT_DOOR_SAID = "the git door answered";
const gitDoor: BrokerService = {
  id: "git",
  claims: (req, door) => door === "http" && req.url?.startsWith("http://github.com/") === true,
  serve: async ({ res }) => void res.writeHead(200, { "content-type": "text/plain" }).end(GIT_DOOR_SAID),
};

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

/** A Codex stand-in whose brokered turn polls ChatGPT's plugin list through its machine's door, as Codex does about
 * once a second, and whose process ends with its result while the last poll is still held upstream. */
function codexTurns(daemon: Daemon, seen: IncomingHttpHeaders[]): HarnessAdapterFactory {
  return ctx => ({
    steers: false,
    start: ({ onEvent }) => {
      const sessionId = randomUUID();
      const finished = (async (): Promise<TurnResult> => {
        onEvent({ type: "session.start", sessionId });
        await expect.poll(() => daemon.parked.length, { timeout: 10_000 }).toBeGreaterThan(0);
        const local = daemon.door(0);
        const poll = request({ createConnection: () => local as never, method: "GET", path: "/backend-api/ps/plugins/list", headers: { host: "127.0.0.1:7071", authorization: `Bearer ${ctx.env[HOST_TOKEN_ENV] ?? ""}`, "x-acme-hold": "1" } });
        poll.on("error", () => undefined);
        poll.end();
        await expect.poll(() => seen.length, { timeout: 10_000 }).toBe(1);
        const result: TurnResult = { status: "completed", text: "ok" };
        onEvent({ type: "turn.done", sessionId, result });
        onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
        poll.destroy();
        return result;
      })();
      return { localId: sessionId, finished, interrupt: async () => {} };
    },
  });
}

async function served(o: { gh?: string; codex?: boolean } = {}): Promise<{ rt: Runtime; broker: HostBroker; brokerOn: () => HostBroker; daemon: Daemon; host: () => Promise<HostHandle>; turns: Turn[]; seen: IncomingHttpHeaders[]; release(): void; ran: string[][]; statePath: string; backend: StubBackend }> {
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
  const brokerOn = (): HostBroker =>
    hostBroker({
      statePath,
      vault: () => VAULT,
      services: [anthropicService({ upstream: up.url, computer: () => "acme's laptop" }), chatgptService({ dial: url => new URL(`${url.pathname}${url.search}`, up.url) }), gitDoor],
      signIns: { anthropic: { gateway: none, helper: none, login: none }, codex: o.codex === true ? { credential: async () => ({ ...CODEX_CLAIMS, accessToken: "acme-chatgpt-access" }), claims: async () => CODEX_CLAIMS } : { credential: none, claims: none } },
      run: async (file, args) => (ran.push([file, ...args]), { code: 1, stdout: "" }),
    });
  const broker = brokerOn();
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
  const bin = join(dir, "bin");
  mkdirSync(bin, { recursive: true });
  if (o.gh !== undefined) writeStub(join(bin, "gh"), o.gh);
  const local = o.gh === undefined ? {} : { local: { ...fakeLocal(join(dir, "home")), backend: new LocalBackend({ root: join(dir, "home"), env: { ...process.env, PATH: `${bin}:${process.env["PATH"] ?? ""}` } }) } };
  const rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: factory, codex: codexTurns(daemon, up.seen) }, vault: () => VAULT, broker: broker.port, ...local });
  const host = async (): Promise<HostHandle> => {
    const handle = await startHost({ runtime: rt, webDir, port: 0, statePath, broker, checkKey: async () => ({ works: true, account: "acme-dev", plan: "$20 plan", running: 0, limit: 100 }) });
    let closed: Promise<void> | undefined;
    const close = (): Promise<void> => (closed ??= handle.close());
    cleanups.push(close);
    return { ...handle, close };
  };
  return { rt, broker, brokerOn, daemon, host, turns, seen: up.seen, release: up.release, ran, statePath, backend };
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

  it("writes nothing into a Codex thread for a poll its process cut as its turn completed", async () => {
    const s = await served({ codex: true });
    const ws = await createOn(s.rt, { golden: "snap_g", name: "acme-lab" });
    await s.host();
    const waited: unknown[] = [];
    s.rt.events.on("thread.waited", e => void waited.push(e));
    const turn = await s.rt.sessions.start(ws.id, { prompt: "one", harness: "codex" });
    expect(await turn.finished).toMatchObject({ status: "completed" });
    // A call cut before any answer is logged with status 0.
    const polls = () => readdirSync(brokerLogDir(s.statePath)).flatMap(day => readFileSync(join(brokerLogDir(s.statePath), day), "utf8").trim().split("\n")).map(line => JSON.parse(line) as Record<string, unknown>).filter(line => line["path"] === "/backend-api/ps/plugins/list");
    await expect.poll(polls, { timeout: 5_000 }).toEqual([expect.objectContaining({ machine: ws.id, token: "turn", service: "chatgpt", status: 0 })]);
    await new Promise(resolve => setTimeout(resolve, 250));
    expect(waited).toEqual([]);
  });

  it("carries a create's own clone to the git door while the machine is still being made", async () => {
    const s = await served({ gh: GH });
    await s.host();
    const answered: Array<{ status: number; body: string }> = [];
    // The machine's git reaches github.com through a pipe the create dialled to its daemon's door, with the op token
    // the create minted for its clone, read off the proxy its git config names.
    s.backend.execImpl = async (m, cmd) => {
      const token = cmd.includes("clone") ? /wsp:([^@]+)@127\.0\.0\.1/.exec(cmd)?.[1] : undefined;
      if (token === undefined) return tokenGuest(m, cmd);
      await expect.poll(() => s.daemon.parked.length, { timeout: 10_000 }).toBeGreaterThan(0);
      const got = await gitCall(s.daemon.door(0), token);
      answered.push(got);
      return got.status === 200 ? { exitCode: 0, stdout: "", stderr: "" } : { exitCode: 128, stdout: "", stderr: got.body };
    };
    const ws = await createOn(s.rt, { golden: "snap_g", name: "acme-lab" });
    expect(ws.branchOfRecord?.branch).toBe("wsp/acme-lab");
    expect(answered).toEqual([{ status: 200, body: GIT_DOOR_SAID }]);
  });

  it("tells a machine whose workspace it does not hold from one it revoked", async () => {
    const s = await served();
    await s.host();
    const gone = await s.rt.devices.mint("terminal th-acme-gone", { kind: "thread", threadId: "th-acme-gone", workspaceId: "ws_acme_gone", rootThreadId: "th-acme-gone" }, Date.now());
    const said = await gitCall(handed(s.broker, "ws_acme_gone"), gone.deviceToken);
    expect(said.status).toBe(403);
    expect(said.body).toBe(`${UNHELD_SAID}\n`);
  });

  it("refuses as revoked a machine switched off while its create still runs", async () => {
    const s = await served({ gh: GH });
    await s.host();
    const answered: Array<{ status: number; body: string }> = [];
    s.backend.execImpl = async (m, cmd) => {
      const token = cmd.includes("clone") ? /wsp:([^@]+)@127\.0\.0\.1/.exec(cmd)?.[1] : undefined;
      if (token === undefined) return tokenGuest(m, cmd);
      const id = (await s.rt.devices.list()).find(d => d.name.startsWith("op clone"))!.scope!.workspaceId;
      await s.broker.port.revoke!(id);
      const got = await gitCall(handed(s.broker, id), token);
      answered.push(got);
      return { exitCode: 128, stdout: "", stderr: got.body };
    };
    await expect(createOn(s.rt, { golden: "snap_g", name: "acme-lab" })).rejects.toThrow(REVOKED_SAID);
    expect(answered).toEqual([{ status: 403, body: `${REVOKED_SAID}\n` }]);
  });

  it("keeps a machine whose provider delete failed cut off after the host restarts, and lets a lifted one through", async () => {
    const s = await served();
    const ws = await createOn(s.rt, { golden: "snap_g", name: "acme-lab" });
    const kept = await createOn(s.rt, { golden: "snap_g", name: "acme-kept" });
    const handle = await s.host();
    s.backend.machines[0]!.kill = async () => {
      throw new Error("Boat answered 503");
    };
    await expect(s.rt.workspaces.delete(ws.id)).rejects.toThrow();
    await handle.close();
    // The next host start: a new broker on the same state, whose token store holds nothing of the last one.
    const again = s.brokerOn();
    await again.start({ rt: s.rt, server: { authorize: async () => undefined, revokeDevice: async () => false } });
    cleanups.push(() => again.stop());
    const revoked = await heldPipeCall(again, ws.id, "acme-old-token");
    expect(revoked.status).toBe(403);
    expect(revoked.body).toContain(REVOKED_SAID);
    // A machine never switched off is judged on its token, which the new host does not hold.
    const other = await heldPipeCall(again, kept.id, "acme-old-token");
    expect(other.status).toBe(403);
    expect(other.body).toContain(DEAD_TOKEN_SAID);
    // A switch lifted for a new machine under the same record stays lifted on the host after.
    again.port.lift!(ws.id);
    again.stop();
    const third = s.brokerOn();
    await third.start({ rt: s.rt, server: { authorize: async () => undefined, revokeDevice: async () => false } });
    cleanups.push(() => third.stop());
    expect((await heldPipeCall(third, ws.id, "acme-old-token")).body).toContain(DEAD_TOKEN_SAID);
    expect(s.seen).toHaveLength(0);
  });

  it("keeps the host's revoke on disk when wsp doctor's own runtime on the same state cuts its machine off", async () => {
    const s = await served();
    const ws = await createOn(s.rt, { golden: "snap_g", name: "acme-lab" });
    const handle = await s.host();
    // wsp doctor's terminal road builds its own runtime, and so its own broker, on this state.
    const doctor = s.brokerOn();
    s.backend.machines[0]!.kill = async () => {
      throw new Error("Boat answered 503");
    };
    await expect(s.rt.workspaces.delete(ws.id)).rejects.toThrow();
    await doctor.port.revoke!("ws_d0c70r00");
    await handle.close();
    const again = s.brokerOn();
    await again.start({ rt: s.rt, server: { authorize: async () => undefined, revokeDevice: async () => false } });
    cleanups.push(() => again.stop());
    expect((await heldPipeCall(again, ws.id, "acme-old-token")).body).toContain(REVOKED_SAID);
  });

  it("keeps the host's lift on disk when wsp doctor's own runtime on the same state cuts its machine off", async () => {
    const s = await served();
    const back = await createOn(s.rt, { golden: "snap_g", name: "acme-back" });
    const handle = await s.host();
    await s.broker.port.revoke!(back.id);
    const doctor = s.brokerOn();
    s.broker.port.lift!(back.id);
    await doctor.port.revoke!("ws_d0c70r00");
    await handle.close();
    const again = s.brokerOn();
    await again.start({ rt: s.rt, server: { authorize: async () => undefined, revokeDevice: async () => false } });
    cleanups.push(() => again.stop());
    expect((await heldPipeCall(again, back.id, "acme-old-token")).body).toContain(DEAD_TOKEN_SAID);
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
