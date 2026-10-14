// SPDX-License-Identifier: AGPL-3.0-only
// A slate's tool run on an http MCP server from a thread on this computer, where Claude Code keeps an OAuth sign-in for
// it: every message carries the bearer read here at that message, as the agent's own call does. A refusal for want of a
// sign-in asks the agent's own word once, which renews a token near its end, and sends again. The token reaches no
// record, view, event or log line of the host.
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LocalBackend } from "@wsp/engine";
import { DAEMON_VERSION, type AdapterEvent, type Caller, type SlateView, type TurnResult } from "@wsp/protocol";
import { createRuntime, type AgentsReader, type HarnessAdapterFactory, type LocalWiring, type Runtime } from "../src/index.js";
import { localExecStream } from "../src/local-exec.js";
import { signInGoneLine } from "../src/slate-mcp-on.js";
import { sqliteStore } from "../src/sqlite-store.js";
import { stubBackend, testPlatform } from "./stub-backend.js";

const OLD = "tok_TESTONLY_old_9d3f";
const NEW = "tok_TESTONLY_new_41ac";
const BEARER = { file: "~/.claude/.credentials.json", at: ["mcpOAuth", "cloudflare|2e40c71145c8b601"], field: "accessToken" };
const LINE = "claude mcp login 'cloudflare'";
const roots: string[] = [];
const servers: Server[] = [];
const runtimes: Runtime[] = [];
afterEach(async () => {
  vi.restoreAllMocks();
  for (const rt of runtimes.splice(0)) await rt.close();
  for (const s of servers.splice(0)) s.close();
  for (const at of roots.splice(0)) rmSync(at, { recursive: true, force: true });
});

function harness(): HarnessAdapterFactory {
  return () => ({
    steers: false,
    forkSession: async f => ({ resume: f.session, drop: async () => {} }),
    start: o => {
      const sessionId = o.resume ?? "66666666-6666-4666-8666-000000000002";
      const result: TurnResult = { status: "completed", text: "ok" };
      const finished = Promise.resolve().then(() => {
        const feed: AdapterEvent[] = [
          { type: "session.start", sessionId },
          { type: "turn.done", sessionId, result },
          { type: "session.end", sessionId, exitCode: 0, sawResult: true },
        ];
        for (const e of feed) o.onEvent(e);
        return result;
      });
      return { localId: sessionId, finished, interrupt: async () => {} };
    },
  });
}

/** An MCP server on this computer's loopback that takes one bearer, which `take` moves. */
async function whoami(): Promise<{ url: string; take: (token: string) => void; asked: string[] }> {
  let good = OLD;
  const asked: string[] = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", c => (body += c));
    req.on("end", () => {
      asked.push(req.headers.authorization ?? "");
      if (req.headers.authorization !== `Bearer ${good}`) return void res.writeHead(401, { "www-authenticate": 'Bearer realm="OAuth"' }).end();
      const m = JSON.parse(body) as { id?: number; method: string };
      if (m.id === undefined) return void res.writeHead(202).end();
      const result =
        m.method === "initialize"
          ? { protocolVersion: "2025-06-18", capabilities: { tools: {} }, serverInfo: { name: "cloudflare", version: "1" } }
          : m.method === "tools/list"
            ? { tools: [{ name: "whoami", inputSchema: { type: "object" }, annotations: { readOnlyHint: true } }] }
            : { content: [{ type: "text", text: "user: u_acme" }], structuredContent: { user: "u_acme" } };
      res.writeHead(200, { "content-type": "text/event-stream", "mcp-session-id": "s-1" }).end(`event: message\ndata: ${JSON.stringify({ jsonrpc: "2.0", id: m.id, result })}\n\n`);
    });
  });
  servers.push(server);
  await new Promise<void>(r => server.listen(0, "127.0.0.1", r));
  return { url: `http://127.0.0.1:${(server.address() as { port: number }).port}/mcp`, take: t => (good = t), asked };
}

/** Every file under `dir`, read whole. */
const filesUnder = (dir: string): string[] => readdirSync(dir, { recursive: true, encoding: "utf8" }).map(f => join(dir, f)).filter(f => statSync(f).isFile()).map(f => readFileSync(f, "latin1"));

/** A host whose thread runs on this computer, its agent's config naming the server, the store holding `token`. */
async function hereThread(o: { renews: (keep: (token: string) => void) => boolean; token?: string }) {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "wsp-slate-here-")));
  roots.push(root);
  const folder = join(root, "project");
  mkdirSync(folder);
  mkdirSync(join(root, "state"));
  const server = await whoami();
  let held = o.token ?? OLD;
  let reads = 0;
  let renewed = 0;
  const agentsReader: AgentsReader = {
    read: () => Promise.reject(new Error("not read here")),
    tools: () => Promise.reject(new Error("not asked here")),
    server: async on => {
      if (on.kind !== "here") throw new Error("a thread here read another computer's config");
      return {
        transport: { kind: "http", url: server.url, headers: {} },
        cwd: folder,
        env: {},
        secrets: [],
        signIn: { bearer: BEARER, line: LINE, renew: async () => (renewed++, o.renews(t => (held = t))), read: async () => (reads++, held) },
      };
    },
  };
  const local: LocalWiring = {
    backend: new LocalBackend({ root }),
    execStream: e => localExecStream({ root, runDir: join(root, "runs"), ...e }),
    home: () => join(root, ".claude"),
    homeDir: root,
    rootsPath: join(root, "roots"),
    env: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin", HOME: root }),
    platform: testPlatform(),
    daemonRoad: async () => ({ url: "http://127.0.0.1:1", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "t" }),
  };
  const state = join(root, "state");
  const store = sqliteStore(join(state, "state.json"), { wsp: "test", daemon: DAEMON_VERSION, bin: "/usr/local/bin/wsp" });
  const events: unknown[] = [];
  const rt = createRuntime({ backend: stubBackend(), store, adapters: { claude: harness() }, local, statePath: join(state, "state.json"), agentsReader });
  rt.events.on("*", e => events.push(e));
  runtimes.push(rt);
  const project = await rt.projects.add({ source: folder });
  const { workspace } = await rt.workspaces.folderFor({ project: project.id });
  const first = await rt.sessions.start(workspace.id, { prompt: "who am I on cloudflare", harness: "claude", permissionMode: "default" });
  await first.finished;
  const threadId = first.view().threadId!;
  const caller: Caller = { origin: "here", by: { kind: "thread", threadId, workspaceId: workspace.id, rootThreadId: threadId } };
  const view = async (): Promise<SlateView> => {
    await rt.slates.settled();
    return (await rt.slates.get(threadId))!;
  };
  await rt.slates.write({ text: `<slate title="Who"><run name="who" tool="cloudflare.whoami" description="Names the signed-in account" /><column><button id="go" label="Go" onPress={start($who)} /></column></slate>` }, caller);
  const press = async (n: number) => rt.slates.event({ threadId, version: (await view()).version, piece: "go", event: "press", requestId: `go-${n}` });
  return { rt, server, state, events, view, press, threadId, reads: () => reads, renewed: () => renewed };
}

describe("a slate's tool run on an http MCP server from a thread on this computer", () => {
  it("sends the bearer of the sign-in Claude Code keeps here on every message, and the token reaches no record, view, event or log line", async () => {
    const logged: string[] = [];
    for (const level of ["log", "info", "warn", "error", "debug"] as const) vi.spyOn(console, level).mockImplementation((...a: unknown[]) => void logged.push(a.map(String).join(" ")));
    const { rt, server, state, events, view, press, threadId, reads, renewed } = await hereThread({ renews: () => false });

    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["who"]).toMatchObject({ state: "done", runs: 1, json: { user: "u_acme" } }), { timeout: 15_000 });
    expect(server.asked.length).toBeGreaterThanOrEqual(3);
    expect(server.asked.every(a => a === `Bearer ${OLD}`)).toBe(true);
    expect(reads()).toBe(server.asked.length);
    expect(renewed()).toBe(0);

    await rt.close();
    expect(JSON.stringify(await view().catch(() => null))).not.toContain(OLD);
    expect(JSON.stringify(events)).not.toContain(OLD);
    expect(filesUnder(state).length).toBeGreaterThan(0);
    expect(filesUnder(state).filter(text => text.includes(OLD))).toEqual([]);
    expect(logged.filter(line => line.includes(OLD))).toEqual([]);
  }, 40_000);

  it("asks the agent here again when the server wants a sign-in, which renews its token, and sends the message once more", async () => {
    const { rt, server, view, press, threadId, renewed } = await hereThread({
      renews: keep => {
        keep(NEW);
        return true;
      },
    });
    server.take(NEW);
    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["who"]).toMatchObject({ state: "done", runs: 1, json: { user: "u_acme" } }), { timeout: 15_000 });
    expect(renewed()).toBe(1);
    expect(server.asked[0]).toBe(`Bearer ${OLD}`);
    expect(server.asked.slice(1).every(a => a === `Bearer ${NEW}`)).toBe(true);
  }, 40_000);

  it("says the person must sign in again here, with the line, where the agent holds no sign-in now", async () => {
    const { rt, server, view, press, threadId, renewed } = await hereThread({ renews: () => false });
    server.take(NEW);
    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["who"]).toMatchObject({ state: "failed" }), { timeout: 15_000 });
    const [before, after] = signInGoneLine("\0", LINE).split("\0");
    const why = String(((await view()).values["who"] as { why?: string }).why);
    expect(why.startsWith(`cloudflare did not start: ${before}`) && why.endsWith(after!) && why.length > `cloudflare did not start: ${before}${after}`.length, why).toBe(true);
    expect(renewed()).toBe(2);
  }, 40_000);

  it("sends no bearer that would break the request line, and its words reach no answer", async () => {
    const torn = `${OLD}\r\nX-Injected: 1`;
    const { rt, server, view, press, threadId } = await hereThread({ renews: () => false, token: torn });
    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["who"]).toMatchObject({ state: "failed" }), { timeout: 15_000 });
    expect(server.asked.every(a => a === "")).toBe(true);
    expect(JSON.stringify(await view())).not.toContain(OLD);
  }, 40_000);
});
