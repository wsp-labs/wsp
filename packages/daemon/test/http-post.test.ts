// SPDX-License-Identifier: AGPL-3.0-only
// http.post on the daemon's wire: the daemon posts to an http MCP server on its own computer's loopback, with the
// bearer it reads out of the sign-in file Claude Code keeps under the home it serves, and answers what the server
// said. The token is in no frame the client sends or receives, and in no line the daemon logs.
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, describe, expect, it } from "vitest";
import WebSocket from "ws";
import { HttpPostReply } from "@wsp/protocol";
import { daemonUnderTest, type DaemonUnderTest } from "./harness.js";

const TOKEN = "http-post-token";
const SIGN_IN = "tok_TESTONLY_wire_3f8d";
const KEY = "cloudflare|2e40c71145c8b601";
const BEARER = { file: "~/.claude/.credentials.json", at: ["mcpOAuth", KEY], field: "accessToken" };
const home = mkdtempSync(join(tmpdir(), "wsp-http-post-home-"));
const root = mkdtempSync(join(tmpdir(), "wsp-http-post-root-"));
let daemon: DaemonUnderTest | undefined;
let server: Server | undefined;

afterAll(async () => {
  await daemon?.close();
  server?.close();
  rmSync(home, { recursive: true, force: true });
  rmSync(root, { recursive: true, force: true });
});

/** A server on this computer's loopback that answers initialize on an event stream to the one bearer it takes. */
async function loopback(): Promise<{ url: string; asked: string[] }> {
  const asked: string[] = [];
  server = createServer((req, res) => {
    asked.push(req.headers.authorization ?? "");
    req.resume();
    req.on("end", () => {
      if (req.headers.authorization !== `Bearer ${SIGN_IN}`) return void res.writeHead(401).end();
      res.writeHead(200, { "content-type": "text/event-stream", "mcp-session-id": "s-9" });
      res.end(`data: ${JSON.stringify({ jsonrpc: "2.0", id: 1, result: { serverInfo: { name: "zones" } } })}\n\n`);
    });
  });
  await new Promise<void>(r => server!.listen(0, "127.0.0.1", r));
  return { url: `http://127.0.0.1:${(server.address() as { port: number }).port}/mcp`, asked };
}

async function ask(port: number, frame: Record<string, unknown>): Promise<{ sent: string; got: string }> {
  const ws = new WebSocket(`ws://127.0.0.1:${port}/`);
  await new Promise<void>((resolve, reject) => {
    ws.once("open", resolve);
    ws.once("error", reject);
  });
  ws.send(JSON.stringify({ id: 0, op: "auth", token: TOKEN }));
  const sent = JSON.stringify({ id: 1, op: "http.post", ...frame });
  const got = await new Promise<string>(resolve => {
    ws.on("message", raw => {
      const m = JSON.parse(String(raw)) as { id?: unknown };
      if (m.id === 1) resolve(String(raw));
    });
    ws.send(sent);
  });
  ws.close();
  return { sent, got };
}

describe("http.post", () => {
  it("posts from the daemon's computer to its loopback with the bearer of the sign-in kept under its home, and the token never crosses the wire", async () => {
    mkdirSync(join(home, ".claude"));
    writeFileSync(join(home, ".claude/.credentials.json"), JSON.stringify({ mcpOAuth: { [KEY]: { serverName: "cloudflare", accessToken: SIGN_IN, expiresAt: 1 } } }), { mode: 0o600 });
    const { url, asked } = await loopback();
    daemon = await daemonUnderTest({ host: "127.0.0.1", port: 0, token: TOKEN, root, home });
    const body = JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: {} });
    const { sent, got } = await ask(daemon.port, { url, headers: { Accept: "application/json, text/event-stream" }, body, bearer: BEARER, timeoutMs: 10_000 });
    const reply = JSON.parse(got) as Record<string, unknown>;
    expect(reply["ok"]).toBe(true);
    expect(HttpPostReply.parse(reply)).toEqual({ status: 200, contentType: "text/event-stream", sessionId: "s-9", body: `data: ${JSON.stringify({ jsonrpc: "2.0", id: 1, result: { serverInfo: { name: "zones" } } })}\n\n` });
    expect(asked).toEqual([`Bearer ${SIGN_IN}`]);
    expect(sent).not.toContain(SIGN_IN);
    expect(got).not.toContain(SIGN_IN);
    expect(daemon.log().filter(l => l.includes(SIGN_IN))).toEqual([]);

    // With no entry for the server there, the request goes without one and the server's refusal comes back.
    const other = await ask(daemon.port, { url, headers: {}, body, bearer: { ...BEARER, at: ["mcpOAuth", "other|0"] } });
    expect(HttpPostReply.parse(JSON.parse(other.got))).toMatchObject({ status: 401 });
    expect(asked.at(-1)).toBe("");
  }, 30_000);
});
