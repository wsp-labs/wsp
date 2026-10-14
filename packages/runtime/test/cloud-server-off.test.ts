// SPDX-License-Identifier: AGPL-3.0-only
// A server that needs the person's sign-in or key, on a cloud machine whose credentials the host keeps: a slate's run
// on it is refused as it is written, with the sentence the servers list gives, and a turn's launch there leaves it
// out, read off the machine's own config, at the provider this host is wired to and at another one whose record names
// its place. A turn on a computer the person joined reads nothing for it.
import { randomUUID } from "node:crypto";
import { afterEach, describe, expect, it } from "vitest";
import { MCP_READ_END, MCP_READ_MARK, type MachineBackend } from "@wsp/engine";
import { cloudServerOffLine, type Caller, type TurnResult } from "@wsp/protocol";
import { SERVER_NOT_SET_UP, SERVER_OFF, type AgentsOn, type AgentsReader } from "../src/agents-read.js";
import { createRuntime, type HarnessAdapterFactory, type HarnessStartOptions, type PlaceBackends, type Runtime } from "../src/runtime.js";
import { newPlaceKeyPair } from "../src/places.js";
import { memoryStore } from "../src/store.js";
import { stubBackend, createOn } from "./stub-backend.js";
import { wiring } from "./place-join.js";
import { answering, joined, type Started } from "./box-fixture.js";

let rt: Runtime | undefined;
afterEach(async () => {
  await rt?.close();
  rt = undefined;
});

/** A harness whose every turn ends at once, keeping what each was started with; `denies` is an adapter whose launch
 * leaves servers out. */
const quietHarness = (starts: HarnessStartOptions[] = [], denies = false): HarnessAdapterFactory => () => ({
  steers: false,
  ...(denies ? { deniesServers: true as const } : {}),
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

/** Claude Code's own file on the machine: cloudflare signs in with its agent, linear sends a fixed header, stub runs a
 * command. */
const CLAUDE_JSON = JSON.stringify({ mcpServers: { cloudflare: { type: "http", url: "https://mcp.cloudflare.com/mcp" }, linear: { type: "http", url: "https://mcp.linear.app/mcp", headers: { "X-Client": "acme" } }, stub: { command: "node", args: ["s.js"] } } });
/** What the config read prints with that file as its first scope's, and no other scope's file there. */
const configRead = (): string => `${MCP_READ_MARK} 0 0 ${Buffer.from(CLAUDE_JSON).toString("base64")}\n${MCP_READ_END}\n`;

const twoProviders = (wired: string, at: Record<string, MachineBackend>): PlaceBackends => ({
  get wired() {
    return wired;
  },
  backend: place => at[place],
  list: () => Object.keys(at),
});

describe("a server that needs the person's sign-in or key, on a cloud machine", () => {
  it("refuses a slate's run on a server that needs the person's sign-in there, with the servers list's sentence, and keeps a server that runs there", async () => {
    const asked: AgentsOn[] = [];
    // The cloud machine's config as the host reads it: cloudflare signs in with its agent, stub is a command.
    const agentsReader: AgentsReader = {
      read: () => Promise.reject(new Error("not read here")),
      tools: () => Promise.reject(new Error("not asked here")),
      server: async (on, ask) => {
        asked.push(on);
        if (ask.name === "cloudflare" && on.kind === "machine" && on.brokered === true) throw Object.assign(new Error(cloudServerOffLine("cloudflare")), { code: SERVER_OFF });
        if (ask.name !== "stub" && ask.name !== "cloudflare") throw Object.assign(new Error(`no MCP server called ${ask.name} is in Claude Code's config there`), { code: SERVER_NOT_SET_UP });
        return { transport: { kind: "stdio", command: "stub-mcp", args: [], env: {} }, cwd: "/root", env: {}, secrets: [] };
      },
    };
    const solari = stubBackend();
    const boat = stubBackend();
    rt = createRuntime({ backend: solari, store: memoryStore(), adapters: { claude: quietHarness() }, places: twoProviders("solari", { solari, boat }), placeLinks: wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 }), agentsReader });
    for (const on of ["boat", "solari"]) {
      const fork = await createOn(rt, { golden: "snap_g", name: `x-${on}`, on });
      expect(fork.place).toBe(on === "boat" ? "boat" : undefined);
      const turn = await rt.sessions.start(fork.id, { prompt: "zones", harness: "claude" });
      await turn.finished;
      const threadId = turn.view().threadId!;
      const caller: Caller = { origin: "relayed", by: { kind: "thread", threadId, workspaceId: fork.id, rootThreadId: threadId } };
      const slate = `<slate title="Zones"><run name="zones" tool="cloudflare.zones_list" description="Lists the account's zones" /><run name="items" tool="stub.list_items" description="Lists the newest items" /><column><output run={$zones} /></column></slate>`;
      await expect(rt.slates.write({ text: slate }, caller), on).rejects.toMatchObject({ kind: "invalid", message: `slate refused: 1 error: K714 server-off ${cloudServerOffLine("cloudflare")}` });
      expect(await rt.slates.get(threadId)).toBeNull();
      await rt.slates.write({ text: `<slate title="Items"><run name="items" tool="stub.list_items" description="Lists the newest items" /><column><output run={$items} /></column></slate>` }, caller);
    }
    expect(asked.length).toBeGreaterThan(0);
    expect(asked.every(on => on.kind === "machine" && on.brokered === true)).toBe(true);
  }, 40_000);

  it("a turn's launch there leaves out each server the machine's config names that needs the person's sign-in, read off that config", async () => {
    const starts: HarnessStartOptions[] = [];
    const reads: string[] = [];
    let readFails = false;
    const solari = stubBackend();
    const boat = stubBackend();
    for (const backend of [solari, boat]) {
      const answer = backend.execImpl;
      backend.execImpl = (m, cmd, stdin) => (cmd.includes("wsp_mcp_read") ? (reads.push(cmd), readFails ? { exitCode: 1, stdout: "", stderr: "read failed" } : { exitCode: 0, stdout: configRead(), stderr: "" }) : answer(m, cmd, stdin));
    }
    rt = createRuntime({ backend: solari, store: memoryStore(), adapters: { claude: quietHarness(starts, true) }, places: twoProviders("solari", { solari, boat }), placeLinks: wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 }) });
    for (const on of ["boat", "solari"]) {
      const fork = await createOn(rt, { golden: "snap_g", name: `x-${on}`, on });
      await (await rt.sessions.start(fork.id, { prompt: "zones", harness: "claude" })).finished;
    }
    expect(starts.map(o => o.offServers)).toEqual([["cloudflare"], ["cloudflare"]]);
    expect(reads.length).toBe(2);
    // A config that cannot be read costs the launch its list, never the turn.
    readFails = true;
    const unread = await createOn(rt, { golden: "snap_g", name: "x-unread", on: "boat" });
    expect((await (await rt.sessions.start(unread.id, { prompt: "zones", harness: "claude" })).finished).status).toBe("completed");
    expect(starts).toHaveLength(3);
    expect(starts[2]!.offServers).toBeUndefined();
  }, 40_000);

  it("a turn on a computer the person joined reads no config for it and leaves nothing out", async () => {
    const started: Started[] = [];
    const denying: HarnessAdapterFactory = hctx => ({ ...answering(started)(hctx), deniesServers: true });
    const { rt: box, project, seen } = await joined({ adapters: { claude: denying } });
    const at = await box.workspaces.folderFor({ project: project.id });
    await (await box.sessions.start(at.workspace.id, { prompt: "zones", harness: "claude" })).finished;
    expect(started).toHaveLength(1);
    expect(started[0]!.o.offServers).toBeUndefined();
    expect(JSON.stringify(seen.frames)).not.toContain("wsp_mcp_read");
    await box.close();
  }, 40_000);
});
