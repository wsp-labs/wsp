// SPDX-License-Identifier: AGPL-3.0-only
// A slate's tool run on an http MCP server from a thread on a computer the person joined: every message goes from
// that computer through its daemon's http.post, so a server on that computer's own loopback answers, and the bearer
// of the sign-in its Claude Code keeps there is read there by the daemon, which this fake stands in for. The token
// never reaches this host: no frame it sends, no record it writes and no line it logs holds it. A refusal for want of
// a sign-in asks the agent's own word again there, which renews a token near its end, and says so where it cannot.
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createServer, request, type Server } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DAEMON_VERSION, HTTP_POST_DAEMON_VERSION, slateServerBehindLine, type SlateView } from "@wsp/protocol";
import type { AgentsOn, AgentsReader } from "../src/agents-read.js";
import { signInGoneLine } from "../src/slate-mcp-on.js";
import { sqliteStore } from "../src/sqlite-store.js";
import { answering, asThread, joined, type Posts, type Started } from "./box-fixture.js";

const OLD = "tok_TESTONLY_old_5b1a";
const NEW = "tok_TESTONLY_new_c7e2";
const KEY = "cloudflare|2e40c71145c8b601";
const BEARER = { file: "~/.claude/.credentials.json", at: ["mcpOAuth", KEY], field: "accessToken" };
const roots: string[] = [];
const servers: Server[] = [];
afterEach(() => {
  vi.restoreAllMocks();
  for (const s of servers.splice(0)) s.close();
  for (const at of roots.splice(0)) rmSync(at, { recursive: true, force: true });
});

const temp = (name: string): string => {
  const at = realpathSync(mkdtempSync(join(tmpdir(), name)));
  roots.push(at);
  return at;
};

/** An MCP server on hetzner's loopback that takes one bearer, which `take` moves. */
async function zones(): Promise<{ url: string; take: (token: string) => void; asked: string[] }> {
  let good = OLD;
  const asked: string[] = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", c => (body += c));
    req.on("end", () => {
      asked.push(req.headers.authorization ?? "");
      if (req.headers.authorization !== `Bearer ${good}`) return void res.writeHead(401).end();
      const m = JSON.parse(body) as { id?: number; method: string };
      if (m.id === undefined) return void res.writeHead(202).end();
      const result =
        m.method === "initialize"
          ? { protocolVersion: "2025-06-18", capabilities: { tools: {} }, serverInfo: { name: "zones", version: "1" } }
          : m.method === "tools/list"
            ? { tools: [{ name: "zones_list", inputSchema: { type: "object" }, annotations: { readOnlyHint: true } }] }
            : { content: [{ type: "text", text: "acme.dev" }], structuredContent: { zones: ["acme.dev"] } };
      res.writeHead(200, { "content-type": "text/event-stream", "mcp-session-id": "s-1" }).end(`event: message\ndata: ${JSON.stringify({ jsonrpc: "2.0", id: m.id, result })}\n\n`);
    });
  });
  servers.push(server);
  await new Promise<void>(r => server.listen(0, "127.0.0.1", r));
  const port = (server.address() as { port: number }).port;
  return { url: `http://127.0.0.1:${port}/mcp`, take: t => (good = t), asked };
}

/** hetzner's daemon as http.post makes a request: the bearer read off its own home, the request sent from there. */
function daemonThere(home: string): Posts {
  return frame =>
    new Promise((resolve, reject) => {
      const b = frame["bearer"] as typeof BEARER | undefined;
      let token: string | undefined;
      if (b !== undefined) {
        let at: unknown = JSON.parse(readFileSync(join(home, b.file.slice(2)), "utf8"));
        for (const k of b.at) at = (at as Record<string, unknown> | undefined)?.[k];
        token = (at as Record<string, string> | undefined)?.[b.field];
      }
      const headers = { ...(frame["headers"] as Record<string, string>), ...(token !== undefined && token !== "" ? { Authorization: `Bearer ${token}` } : {}) };
      const req = request(String(frame["url"]), { method: "POST", headers }, res => {
        let body = "";
        res.on("data", c => (body += c));
        res.on("end", () => resolve({ status: res.statusCode, ...(res.headers["content-type"] !== undefined ? { contentType: res.headers["content-type"] } : {}), ...(typeof res.headers["mcp-session-id"] === "string" ? { sessionId: res.headers["mcp-session-id"] } : {}), body }));
      });
      req.on("error", reject);
      req.end(String(frame["body"]));
    });
}

const writeStore = (home: string, token: string): void => {
  mkdirSync(join(home, ".claude"), { recursive: true });
  writeFileSync(join(home, ".claude/.credentials.json"), JSON.stringify({ mcpOAuth: { [KEY]: { serverName: "cloudflare", serverUrl: "https://mcp.cloudflare.com/mcp", accessToken: token, refreshToken: "ref_TESTONLY", expiresAt: 1 } } }));
};

/** Every file under `dir`, read whole. */
const filesUnder = (dir: string): string[] => readdirSync(dir, { recursive: true, encoding: "utf8" }).map(f => join(dir, f)).filter(f => statSync(f).isFile()).map(f => readFileSync(f, "latin1"));

async function boxThread(o: { renews: (home: string) => boolean; daemonVersion?: number }) {
  const boxHome = temp("wsp-box-home-");
  const state = temp("wsp-host-state-");
  writeStore(boxHome, OLD);
  const server = await zones();
  const asked: AgentsOn[] = [];
  let renewed = 0;
  const agentsReader: AgentsReader = {
    read: () => Promise.reject(new Error("not read here")),
    tools: () => Promise.reject(new Error("not asked here")),
    server: async on => {
      asked.push(on);
      if (on.kind === "here") throw new Error("this computer's config was read for a thread on hetzner");
      return {
        transport: { kind: "http", url: server.url, headers: {} },
        cwd: "/root",
        env: {},
        secrets: [],
        login: { platform: "linux", home: "/root", user: "root" },
        signIn: { bearer: BEARER, line: "claude mcp login cloudflare", renew: async () => (renewed++, o.renews(boxHome)) },
      };
    },
  };
  const store = sqliteStore(join(state, "state.json"), { wsp: "test", daemon: DAEMON_VERSION, bin: "/usr/local/bin/wsp" });
  const started: Started[] = [];
  const { rt, project, seen } = await joined({ adapters: { claude: answering(started) }, agentsReader, store, posts: daemonThere(boxHome), daemonVersion: o.daemonVersion ?? HTTP_POST_DAEMON_VERSION });
  const at = await rt.workspaces.folderFor({ project: project.id });
  const first = await rt.sessions.start(at.workspace.id, { prompt: "list my zones", harness: "claude" });
  await first.finished;
  const threadId = first.view().threadId!;
  const caller = asThread({ kind: "thread", threadId, workspaceId: at.workspace.id, rootThreadId: threadId });
  const view = async (): Promise<SlateView> => {
    await rt.slates.settled();
    return (await rt.slates.get(threadId))!;
  };
  await rt.slates.write({ text: `<slate title="Zones"><run name="zones" tool="cloudflare.zones_list" /><column><button id="go" label="Go" onPress={start($zones)} /></column></slate>` }, caller);
  const press = async (n: number) => rt.slates.event({ threadId, version: (await view()).version, piece: "go", event: "press", requestId: `go-${n}` });
  return { rt, seen, server, state, asked, view, press, threadId, renewed: () => renewed };
}

describe("a slate's tool run on an http MCP server from a thread on a computer you joined", () => {
  it("asks it from that computer through its daemon with the sign-in Claude Code keeps there, reaches its loopback, and the token never reaches this host", async () => {
    const logged: string[] = [];
    for (const level of ["log", "info", "warn", "error", "debug"] as const) vi.spyOn(console, level).mockImplementation((...a: unknown[]) => void logged.push(a.map(String).join(" ")));
    const fetched = vi.spyOn(globalThis, "fetch");
    const { rt, seen, server, state, asked, view, press, threadId, renewed } = await boxThread({ renews: () => false });

    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["zones"]).toMatchObject({ state: "done", runs: 1, json: { zones: ["acme.dev"] } }), { timeout: 15_000 });

    // Every message went from hetzner, to the address on its own loopback, naming where the sign-in is and never what.
    const posts = seen.frames.filter(f => f["op"] === "http.post");
    expect(posts.length).toBeGreaterThanOrEqual(3);
    expect(posts.every(f => f["url"] === server.url && JSON.stringify(f["bearer"]) === JSON.stringify(BEARER))).toBe(true);
    expect(server.asked.every(a => a === `Bearer ${OLD}`)).toBe(true);
    expect(fetched).not.toHaveBeenCalled();
    expect(renewed()).toBe(0);
    expect(asked.every(on => on.kind === "box" && on.name === "hetzner")).toBe(true);

    // Nothing this host sent, wrote or logged holds the token.
    await rt.close();
    expect(JSON.stringify(seen.frames)).not.toContain(OLD);
    expect(JSON.stringify(await view().catch(() => null))).not.toContain(OLD);
    expect(filesUnder(state).length).toBeGreaterThan(0);
    expect(filesUnder(state).filter(text => text.includes(OLD))).toEqual([]);
    expect(logged.filter(line => line.includes(OLD))).toEqual([]);
  }, 40_000);

  it("asks the agent there again when the server wants a sign-in, which renews its token, and sends the message once more", async () => {
    const { rt, server, view, press, threadId, renewed } = await boxThread({
      renews: home => {
        writeStore(home, NEW);
        return true;
      },
    });
    server.take(NEW);
    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["zones"]).toMatchObject({ state: "done", runs: 1, json: { zones: ["acme.dev"] } }), { timeout: 15_000 });
    expect(renewed()).toBe(1);
    expect(server.asked[0]).toBe(`Bearer ${OLD}`);
    expect(server.asked.slice(1).every(a => a === `Bearer ${NEW}`)).toBe(true);
    await rt.close();
  }, 40_000);

  it("says the person must sign in again on that computer, with the line, where the agent there holds no sign-in now", async () => {
    const { rt, server, view, press, threadId, renewed } = await boxThread({ renews: () => false });
    server.take(NEW);
    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["zones"]).toMatchObject({ state: "failed", why: `cloudflare did not start: ${signInGoneLine("hetzner", "claude mcp login cloudflare")}` }), { timeout: 15_000 });
    // Once for the tool list the consent sheet showed and once for the run: each connection asks again once.
    expect(renewed()).toBe(2);
    await rt.close();
  }, 40_000);

  it("is refused on a computer whose daemon cannot post from there, naming the update, and posts nothing", async () => {
    const { rt, seen, view, press, threadId } = await boxThread({ renews: () => false, daemonVersion: HTTP_POST_DAEMON_VERSION - 1 });
    await rt.slates.approve({ threadId, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["zones"]).toMatchObject({ state: "failed", why: `cloudflare did not start: ${slateServerBehindLine("hetzner")}` }), { timeout: 15_000 });
    expect(seen.frames.filter(f => f["op"] === "http.post")).toEqual([]);
    await rt.close();
  }, 40_000);
});
