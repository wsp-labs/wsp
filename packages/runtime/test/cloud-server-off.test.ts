// SPDX-License-Identifier: AGPL-3.0-only
// A server that needs the person's sign-in or key, on a cloud machine whose credentials the host keeps: a slate's run
// on it is refused as it is written, with the sentence the servers list gives, and a turn's launch there leaves it
// out, read off the machine's own config, at the provider this host is wired to and at another one whose record names
// its place. A turn on a computer the person joined reads nothing for it.
import { randomUUID } from "node:crypto";
import { afterEach, describe, expect, it } from "vitest";
import { MCP_READ_END, MCP_READ_MARK, type MachineBackend } from "@wsp/engine";
import { cloudServerOffLine, type Caller, type TurnResult } from "@wsp/protocol";
import { SERVER_NOT_SET_UP, SERVER_OFF, type AgentsOn, type AgentsReader, type ServersActs } from "../src/agents-read.js";
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

/** Claude Code's own file on the machine: cloudflare signs in with its agent, context7 needs nothing, linear sends a
 * fixed header, stub runs a command. */
const CLAUDE_JSON = JSON.stringify({ mcpServers: { cloudflare: { type: "http", url: "https://mcp.cloudflare.com/mcp" }, context7: { type: "http", url: "https://mcp.context7.com/mcp" }, linear: { type: "http", url: "https://mcp.linear.app/mcp", headers: { "X-Client": "acme" } }, stub: { command: "node", args: ["s.js"] } } });
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

  it("a turn's launch there leaves out each server the machine's config names that needs the person's sign-in, read off that config once per machine", async () => {
    const starts: HarnessStartOptions[] = [];
    // The host's probe as it stands: cloudflare answered 401, context7 answered, and nothing else has been asked.
    const asked: string[] = [];
    let cloudflareSigns = true;
    const agentsReader: AgentsReader = {
      read: () => Promise.reject(new Error("not read here")),
      tools: () => Promise.reject(new Error("not asked here")),
      signsIn: async urls => (asked.push(...urls), url => (url.includes("cloudflare") ? cloudflareSigns : url.includes("context7") ? false : undefined)),
    };
    const serversActs: ServersActs = { add: async () => ({ file: "~/.claude.json" }), remove: async () => ({ file: "~/.claude.json" }), toggle: async () => ({ file: "~/.claude.json" }) };
    const reads: string[] = [];
    let readFails = false;
    const solari = stubBackend();
    const boat = stubBackend();
    for (const backend of [solari, boat]) {
      const answer = backend.execImpl;
      backend.execImpl = (m, cmd, stdin) => (cmd.includes("wsp_mcp_read") ? (reads.push(cmd), readFails ? { exitCode: 1, stdout: "", stderr: "read failed" } : { exitCode: 0, stdout: configRead(), stderr: "" }) : answer(m, cmd, stdin));
    }
    rt = createRuntime({ backend: solari, store: memoryStore(), adapters: { claude: quietHarness(starts, true) }, places: twoProviders("solari", { solari, boat }), placeLinks: wiring(newPlaceKeyPair(), { id: "solari", rateUsdPerHour: 0.11 }), agentsReader, serversActs });
    const forks = [];
    for (const on of ["boat", "solari"]) {
      const fork = await createOn(rt, { golden: "snap_g", name: `x-${on}`, on });
      forks.push(fork);
      await (await rt.sessions.start(fork.id, { prompt: "zones", harness: "claude" })).finished;
    }
    expect(starts.map(o => o.offServers)).toEqual([["cloudflare"], ["cloudflare"]]);
    expect(reads.length).toBe(2);
    expect([...new Set(asked)].sort()).toEqual(["https://mcp.cloudflare.com/mcp", "https://mcp.context7.com/mcp"]);
    // A second turn on a machine reuses what its first read there, until wsp changes a server there.
    const boatFork = forks[0]!;
    await (await rt.sessions.start(boatFork.id, { prompt: "again", harness: "claude" })).finished;
    expect(starts[2]!.offServers).toEqual(["cloudflare"]);
    expect(reads.length).toBe(2);
    await rt.agents.serversToggle({ workspaceId: boatFork.id }, { agent: "claude", name: "linear", on: false });
    await (await rt.sessions.start(boatFork.id, { prompt: "after the change", harness: "claude" })).finished;
    expect(reads.length).toBe(3);
    // A rebuild replaces the machine, maybe from an image with other servers: the next turn reads its config again.
    await rt.workspaces.rebuild(boatFork.id);
    await (await rt.sessions.start(boatFork.id, { prompt: "after the rebuild", harness: "claude" })).finished;
    expect(reads.length).toBe(4);
    // A config that cannot be read costs the launch its list, never the turn.
    readFails = true;
    const unread = await createOn(rt, { golden: "snap_g", name: "x-unread", on: "boat" });
    expect((await (await rt.sessions.start(unread.id, { prompt: "zones", harness: "claude" })).finished).status).toBe("completed");
    expect(starts).toHaveLength(6);
    expect(starts[5]!.offServers).toBeUndefined();
    // A probe that no longer asks for a sign-in leaves nothing out, and says so, which clears the last launch's list.
    cloudflareSigns = false;
    await (await rt.sessions.start(boatFork.id, { prompt: "open now", harness: "claude" })).finished;
    expect(starts[6]!.offServers).toEqual([]);
  }, 40_000);

  it("a turn on a machine from an image built at a provider leaves out aws-mcp, whose AWS login stayed on the person's computer, and keeps it where it sends unsigned", async () => {
    const starts: HarnessStartOptions[] = [];
    const image = JSON.stringify({
      mcpServers: {
        "aws-mcp": { command: "uvx", args: ["mcp-proxy-for-aws@latest", "https://aws-mcp.us-east-1.api.aws/mcp", "--metadata", "INSTALL_SOURCE=aws-cli"] },
        "aws-installed": { command: "/root/.local/bin/mcp-proxy-for-aws", args: ["https://aws-mcp.us-east-1.api.aws/mcp"] },
        "aws-readme": { command: "uvx", args: ["mcp-proxy-for-aws-cli@latest", "https://aws-mcp.us-east-1.api.aws/mcp"] },
        "aws-wrapped": { command: "sh", args: ["-c", "uvx mcp-proxy-for-aws@latest https://aws-mcp.us-east-1.api.aws/mcp"] },
        "aws-unsigned": { command: "uvx", args: ["mcp-proxy-for-aws==1.1.0", "https://aws-mcp.us-east-1.api.aws/mcp", "--skip-auth"] },
        "aws-wrapped-unsigned": { command: "sh", args: ["-c", "uvx mcp-proxy-for-aws@latest https://aws-mcp.us-east-1.api.aws/mcp --skip-auth"] },
        stub: { command: "node", args: ["s.js"] },
      },
    });
    const agentsReader: AgentsReader = { read: () => Promise.reject(new Error("not read here")), tools: () => Promise.reject(new Error("not asked here")), signsIn: async () => () => undefined };
    const boat = stubBackend();
    const answer = boat.execImpl;
    boat.execImpl = (m, cmd, stdin) => (cmd.includes("wsp_mcp_read") ? { exitCode: 0, stdout: `${MCP_READ_MARK} 0 0 ${Buffer.from(image).toString("base64")}\n${MCP_READ_END}\n`, stderr: "" } : answer(m, cmd, stdin));
    rt = createRuntime({ backend: boat, store: memoryStore(), adapters: { claude: quietHarness(starts, true) }, agentsReader });
    const fork = await createOn(rt, { golden: "snap_g", name: "x-aws" });
    await (await rt.sessions.start(fork.id, { prompt: "buckets", harness: "claude" })).finished;
    expect(starts.map(o => o.offServers)).toEqual([["aws-mcp", "aws-installed", "aws-readme", "aws-wrapped"]]);
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
