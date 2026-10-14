// SPDX-License-Identifier: AGPL-3.0-only
// What a cloud machine's Claude Code loads once a turn's launch has run there: its own config less the managed
// drop-in the launch writes. No server `wsp servers` lists as off there is left in it, so neither the turn nor a
// `claude mcp list` the agent runs finds one to send the person to sign in to.
import { execFile, execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { afterEach, describe, expect, it, onTestFinished } from "vitest";
import { MCP_READ_END, MCP_READ_MARK, type ExecResult } from "@wsp/engine";
import type { TurnResult } from "@wsp/protocol";
import { createRuntime, memoryStore, type HarnessAdapterFactory, type HarnessStartOptions, type Runtime } from "@wsp/runtime";
import { OFF_SERVERS_FILE, buildCommand } from "../../adapter-claude/src/landmines.js";
import { agentHome } from "../../collect/test/agent-home.js";
import { writeStub } from "../../protocol/test/stub-script.js";
import { createOn, stubBackend } from "../../runtime/test/stub-backend.js";
import { agentsReader } from "../src/agents-reader.js";
import { fetchPost, signInProbes } from "../src/sign-in-probe.js";

let rt: Runtime | undefined;
const roots: string[] = [];
afterEach(async () => {
  await rt?.close();
  rt = undefined;
  for (const r of roots.splice(0)) rmSync(r, { recursive: true, force: true });
});

describe("the configs a cloud machine's Claude Code reads after a turn's launch", () => {
  it("hold no server that wsp servers lists as off there, and keep every server it lists as on", async () => {
    const srv = createServer((req, res) => {
      if (req.url === "/signs") res.writeHead(401, { "www-authenticate": 'Bearer realm="OAuth"' }).end();
      else res.writeHead(200, { "content-type": "application/json" }).end('{"jsonrpc":"2.0","id":1,"result":{}}');
    });
    await new Promise<void>(done => srv.listen(0, "127.0.0.1", done));
    onTestFinished(() => void srv.close());
    const local = `http://127.0.0.1:${(srv.address() as AddressInfo).port}`;
    const root = mkdtempSync(join(tmpdir(), "wsp-handed-"));
    roots.push(root);
    const at = agentHome(root);
    const file = join(at.home, ".claude.json");
    const config = JSON.parse(readFileSync(file, "utf8")) as { mcpServers: Record<string, unknown> };
    config.mcpServers["signs"] = { type: "http", url: `${local}/signs` };
    config.mcpServers["open"] = { type: "http", url: `${local}/open` };
    config.mcpServers["cf"] = { type: "http", url: "https://mcp.cloudflare.com/mcp", headers: { Authorization: "Bearer ${CF_TOKEN}" } };
    config.mcpServers["keyed"] = { command: "node", args: ["keyed.js"], env: { API_TOKEN: "${SOME_VAR}" } };
    writeFileSync(file, JSON.stringify(config));
    const claudeJson = readFileSync(file, "utf8");

    // The host's reader, as wsp servers reads that machine; only the local server is asked.
    const probes = signInProbes({ post: (url, ms) => (url.startsWith(local) ? fetchPost(url, ms) : Promise.resolve(undefined)) });
    const reader = agentsReader({ vault: () => ({}), probes });
    const env = { PATH: `${at.bin}:/usr/bin:/bin`, HOME: at.home };
    const machine = {
      id: "m_cloud",
      uploadUrl: () => Promise.reject(new Error("this backend mints no signed urls")),
      exec: (cmd: string): Promise<ExecResult> =>
        new Promise(resolve =>
          execFile("/bin/bash", ["-c", cmd], { env, maxBuffer: 4 * 1024 * 1024, timeout: 30_000 }, (e, stdout, stderr) => resolve({ exitCode: e === null ? 0 : typeof e.code === "number" ? e.code : 1, stdout: String(stdout), stderr: String(stderr) })),
        ),
    };
    const listed = (await reader.read({ kind: "machine", machine, projects: [], brokered: true })).servers.filter(s => s.agent === "claude" && s.scope === "user");
    const off = listed.filter(s => s.off !== undefined).map(s => s.name);
    const on = listed.filter(s => s.off === undefined).map(s => s.name);
    expect(off.sort()).toEqual(["cf", "keyed", "signs"]);
    expect(on).toContain("open");

    // A turn on that machine through the runtime, reading the same config with the same reader.
    const starts: HarnessStartOptions[] = [];
    const claude: HarnessAdapterFactory = () => ({
      steers: false,
      deniesServers: true,
      start: o => {
        starts.push(o);
        const sessionId = randomUUID();
        const result: TurnResult = { status: "completed", text: "ok" };
        o.onEvent({ type: "session.start", sessionId });
        o.onEvent({ type: "turn.done", sessionId, result });
        o.onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
        return { localId: sessionId, finished: Promise.resolve(result), interrupt: async () => {} };
      },
    });
    const backend = stubBackend();
    const answer = backend.execImpl;
    backend.execImpl = (m, cmd, stdin) => (cmd.includes("wsp_mcp_read") ? { exitCode: 0, stdout: `${MCP_READ_MARK} 0 0 ${Buffer.from(claudeJson).toString("base64")}\n${MCP_READ_END}\n`, stderr: "" } : answer(m, cmd, stdin));
    rt = createRuntime({ backend, store: memoryStore(), adapters: { claude }, places: { wired: "solari", backend: () => backend, list: () => ["solari"] }, agentsReader: reader });
    const fork = await createOn(rt, { golden: "snap_g", name: "x", on: "solari" });
    await (await rt.sessions.start(fork.id, { prompt: "zones", harness: "claude" })).finished;

    // That turn's launch line run on a stand-in machine: the managed folder under the test's own, claude a stub.
    const bin = join(root, "launch-bin");
    mkdirSync(bin);
    writeStub(join(bin, "claude"), "#!/bin/sh\nexit 0\n");
    const managed = join(root, "etc-claude-code", "managed-settings.d");
    const line = buildCommand({ sessionId: randomUUID(), cwd: root, offServers: starts[0]!.offServers });
    execFileSync("/bin/bash", ["-c", line.replaceAll(dirname(OFF_SERVERS_FILE), managed)], { env: { PATH: `${bin}:/usr/bin:/bin` } });
    const dropIn = join(managed, basename(OFF_SERVERS_FILE));
    const denied = existsSync(dropIn) ? (JSON.parse(readFileSync(dropIn, "utf8")) as { deniedMcpServers: { serverName: string }[] }).deniedMcpServers.map(d => d.serverName) : [];
    const loaded = Object.keys(config.mcpServers).filter(name => !denied.includes(name));
    expect(loaded.filter(name => off.includes(name))).toEqual([]);
    expect(on.filter(name => !loaded.includes(name))).toEqual([]);
  }, 40_000);
});
