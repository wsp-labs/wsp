// SPDX-License-Identifier: AGPL-3.0-only
// The first Claude turn on a cloud machine after this host starts, with the host's own reader and probe: an address
// nobody has asked yet is asked before the launch and waited on, so the first turn already leaves out a server that
// wants a sign-in, as every later one does.
import { randomUUID } from "node:crypto";
import { afterEach, describe, expect, it } from "vitest";
import { MCP_READ_END, MCP_READ_MARK } from "@wsp/engine";
import type { TurnResult } from "@wsp/protocol";
import { createRuntime, memoryStore, type HarnessAdapterFactory, type HarnessStartOptions, type Runtime } from "@wsp/runtime";
import { createOn, stubBackend } from "../../runtime/test/stub-backend.js";
import { agentsReader } from "../src/agents-reader.js";
import { signInProbes } from "../src/sign-in-probe.js";

let rt: Runtime | undefined;
afterEach(async () => {
  await rt?.close();
  rt = undefined;
});

/** Claude Code's own file on the machine: cloudflare wants a sign-in, context7 does not. */
const CLAUDE_JSON = JSON.stringify({ mcpServers: { cloudflare: { type: "http", url: "https://mcp.cloudflare.com/mcp" }, context7: { type: "http", url: "https://mcp.context7.com/mcp" } } });

describe("the first cloud turn after the host starts", () => {
  it("waits on the host's probe for an address it has no answer for, and leaves out the one that wants a sign-in", async () => {
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
    // The addresses answer as they did from Boat, cloudflare 401 and context7 200, each after 50 ms.
    const posted: string[] = [];
    const probes = signInProbes({ post: async url => (posted.push(url), await new Promise(done => setTimeout(done, 50)), url.includes("cloudflare") ? 401 : 200) });
    const backend = stubBackend();
    const answer = backend.execImpl;
    backend.execImpl = (m, cmd, stdin) => (cmd.includes("wsp_mcp_read") ? { exitCode: 0, stdout: `${MCP_READ_MARK} 0 0 ${Buffer.from(CLAUDE_JSON).toString("base64")}\n${MCP_READ_END}\n`, stderr: "" } : answer(m, cmd, stdin));
    rt = createRuntime({ backend, store: memoryStore(), adapters: { claude }, places: { wired: "solari", backend: () => backend, list: () => ["solari"] }, agentsReader: agentsReader({ vault: () => ({}), probes }) });
    const fork = await createOn(rt, { golden: "snap_g", name: "x", on: "solari" });
    for (const prompt of ["first", "second"]) await (await rt.sessions.start(fork.id, { prompt, harness: "claude" })).finished;
    expect(starts.map(o => o.offServers ?? null)).toEqual([["cloudflare"], ["cloudflare"]]);
    expect(posted.sort()).toEqual(["https://mcp.cloudflare.com/mcp", "https://mcp.context7.com/mcp"]);
  }, 40_000);
});
