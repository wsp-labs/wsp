// SPDX-License-Identifier: AGPL-3.0-only
// What each agent on a cloud machine loads once a turn's launch has run there: Claude Code its own config less the
// managed drop-in the launch writes, Codex its own config less the servers the launch turns off. No server `wsp
// servers` lists as off there is left in either, so no turn finds one to send the person to sign in to.
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
import { OFF_SERVERS_FILE, buildCommand as claudeLine } from "../../adapter-claude/src/landmines.js";
import { buildCommand as codexLine } from "../../adapter-codex/src/command.js";
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

/** A harness whose every turn ends at once, keeping what each was started with. */
const quiet = (starts: HarnessStartOptions[]): HarnessAdapterFactory => () => ({
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

describe("the configs a cloud machine's agents read after a turn's launch", () => {
  it("hold no server that wsp servers lists as off there, and keep every server it lists as on, for Claude Code and for Codex", async () => {
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
    const claudeFile = join(at.home, ".claude.json");
    const config = JSON.parse(readFileSync(claudeFile, "utf8")) as { mcpServers: Record<string, unknown> };
    config.mcpServers["signs"] = { type: "http", url: `${local}/signs` };
    config.mcpServers["open"] = { type: "http", url: `${local}/open` };
    config.mcpServers["cf"] = { type: "http", url: "https://mcp.cloudflare.com/mcp", headers: { Authorization: "Bearer ${CF_TOKEN}" } };
    config.mcpServers["keyed"] = { command: "node", args: ["keyed.js"], env: { API_TOKEN: "${SOME_VAR}" } };
    writeFileSync(claudeFile, JSON.stringify(config));
    // Codex's: linear's header as the copy to a cloud machine leaves it, and an address that answers 401.
    const codexHome = join(at.home, ".codex");
    const codexFile = join(codexHome, "config.toml");
    const toml = readFileSync(codexFile, "utf8").replace(/Bearer [^"]+"/, 'Bearer "');
    writeFileSync(codexFile, `${toml}\n[mcp_servers.signs]\nurl = "${local}/signs"\n\n[mcp_servers.open]\nurl = "${local}/open"\n`);
    const texts = { claude: readFileSync(claudeFile, "utf8"), codex: readFileSync(codexFile, "utf8") };

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
    const listed = (await reader.read({ kind: "machine", machine, projects: [], brokered: true })).servers.filter(s => s.scope === "user" && s.launch !== true);
    const rows = (agent: string) => ({ off: listed.filter(s => s.agent === agent && s.off !== undefined).map(s => s.name).sort(), on: listed.filter(s => s.agent === agent && s.off === undefined).map(s => s.name) });
    expect(rows("claude").off).toEqual(["cf", "keyed", "signs"]);
    expect(rows("codex").off).toEqual(["linear", "signs"]);

    // A turn of each agent on that machine through the runtime, reading the same configs with the same reader.
    const starts = { claude: [] as HarnessStartOptions[], codex: [] as HarnessStartOptions[] };
    const backend = stubBackend();
    const answer = backend.execImpl;
    backend.execImpl = (m, cmd, stdin) => {
      if (!cmd.includes("wsp_mcp_read")) return answer(m, cmd, stdin);
      const text = cmd.includes(".codex/config.toml") ? texts.codex : texts.claude;
      return { exitCode: 0, stdout: `${MCP_READ_MARK} 0 0 ${Buffer.from(text).toString("base64")}\n${MCP_READ_END}\n`, stderr: "" };
    };
    rt = createRuntime({ backend, store: memoryStore(), adapters: { claude: quiet(starts.claude), codex: quiet(starts.codex) }, places: { wired: "solari", backend: () => backend, list: () => ["solari"] }, agentsReader: reader });
    const fork = await createOn(rt, { golden: "snap_g", name: "x", on: "solari" });
    for (const harness of ["claude", "codex"] as const) await (await rt.sessions.start(fork.id, { prompt: "zones", harness })).finished;

    // Each turn's launch line run on a stand-in machine: Claude's managed folder under the test's own, each agent a
    // stub that prints the words it was started with.
    const bin = join(root, "launch-bin");
    mkdirSync(bin);
    writeStub(join(bin, "claude"), "#!/bin/sh\nexit 0\n");
    writeStub(join(bin, "codex"), `#!/bin/sh\nprintf '%s\\n' "$@"\n`);
    const managed = join(root, "etc-claude-code", "managed-settings.d");
    execFileSync("/bin/bash", ["-c", claudeLine({ sessionId: randomUUID(), cwd: root, offServers: starts.claude[0]!.offServers }).replaceAll(dirname(OFF_SERVERS_FILE), managed)], { env: { PATH: `${bin}:/usr/bin:/bin` } });
    const dropIn = join(managed, basename(OFF_SERVERS_FILE));
    const denied = existsSync(dropIn) ? (JSON.parse(readFileSync(dropIn, "utf8")) as { deniedMcpServers: { serverName: string }[] }).deniedMcpServers.map(d => d.serverName) : [];
    const words = execFileSync("/bin/bash", ["-c", codexLine({ cwd: root, ...(starts.codex[0]!.offServers !== undefined ? { offServers: starts.codex[0]!.offServers } : {}) })], { env: { PATH: `${bin}:/usr/bin:/bin`, CODEX_HOME: codexHome }, encoding: "utf8" }).split("\n");
    const disabled = words.flatMap(w => /^mcp_servers\.([\w-]+)\.enabled=false$/.exec(w)?.[1] ?? []);
    const loaded = {
      claude: Object.keys(config.mcpServers).filter(name => !denied.includes(name)),
      codex: [...texts.codex.matchAll(/^\[mcp_servers\.([\w-]+)\]$/gm)].map(m => m[1]!).filter(name => name !== "old" && !disabled.includes(name)),
    };
    for (const agent of ["claude", "codex"] as const) {
      expect(loaded[agent].filter(name => rows(agent).off.includes(name)), agent).toEqual([]);
      expect(rows(agent).on.filter(name => name !== "old" && !loaded[agent].includes(name)), agent).toEqual([]);
    }
  }, 40_000);
});
