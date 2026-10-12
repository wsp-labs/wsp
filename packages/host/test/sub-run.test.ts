// SPDX-License-Identifier: AGPL-3.0-only
// The line a lead thread on this computer runs to start a sub-thread on Boat,
// `wsp run --on boat --detach --notify me "<task>"`, and the run tool, each over
// the lead's own token against a host holding Boat as a stub: the answer carries
// 03's object, the thread opens in /root/<repo> on its own machine with the
// preamble above the task, and its stages and the lines it passes with go to
// stderr.
import { join } from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { afterEach, describe, expect, it } from "vitest";
import { EXIT_CODES, HOST_TOKEN_ENV, TURN_TOKEN_ENV, startsUsedLine, subOpenedLine, type SubOut } from "@wsp/protocol";
import { ctx, sockets } from "../../runtime/test/places-fixture.js";
import { PAID, leadOnThisComputer } from "../../runtime/test/sub-fixture.js";
import { WsClient } from "../../runtime/test/ws-client.js";
import { CLI_VERBS, runVerb, type HostClient } from "../src/verbs.js";
import { mcpServer } from "../src/mcp.js";
import { captured } from "./verbs-fixture.js";

let mcp: Client | undefined;
afterEach(async () => {
  await mcp?.close();
  mcp = undefined;
});

/** The host's socket as a thread's own wsp dials it, under that thread's token: a refusal thrown with its kind, and
 * the frames it pushes heard once events are asked for. */
async function asToken(token: string): Promise<HostClient> {
  const ws = await WsClient.connect(ctx.srv!.port, { token });
  sockets.push(ws.ws);
  return {
    request: async (op: string, params: Record<string, unknown> = {}) => {
      const reply = await ws.request(op, params);
      if (reply.ok !== true) throw Object.assign(new Error(String(reply["error"])), typeof reply["kind"] === "string" ? { kind: reply["kind"] } : {});
      return reply;
    },
    events: async (): Promise<void> => void (await ws.request("events.subscribe")),
    // Every frame that is no reply, to whoever listens now: the stages a create pushes.
    onFrame: (read: (f: Record<string, unknown>) => void) => {
      let on = true;
      ws.onFrame(f => {
        if (on && f["id"] === undefined) read(f);
      });
      return () => void (on = false);
    },
    closed: new Promise<void>(() => {}),
    closeWords: () => "closed",
    close: () => {},
    terminate: () => {},
  } as unknown as HostClient;
}

/** A line the lead runs, with the launch pair its turn was started with. */
async function wsp(launch: Readonly<Record<string, string>>, ...argv: string[]) {
  const io = captured();
  const verb = CLI_VERBS.find(v => v.name === argv[0])!;
  const env = { [HOST_TOKEN_ENV]: launch[HOST_TOKEN_ENV]!, [TURN_TOKEN_ENV]: launch[TURN_TOKEN_ENV]! };
  const client = await asToken(env[HOST_TOKEN_ENV]);
  const code = await runVerb(verb, argv, io, () => "/nowhere/state.json", { env, dial: async () => client });
  return { code, io };
}

const TASK = "Fix the flaky upload test";

describe("wsp run --on boat from a lead thread on this computer", () => {
  it("answers 03's object at once, opens the sub-thread in /root/web under the lead with the preamble, and says its stages on stderr", async () => {
    const { rt, boat, starts, threadId, launch } = await leadOnThisComputer({ served: true, picks: [".env", ".env.local"], git: { uncommitted: 2 } });
    const ran = await wsp(launch, "run", "--on", "boat", "--detach", "--notify", "me", TASK, "--json");
    expect(ran.io.lines.length, ran.io.errors.join("\n")).toBe(1);
    expect(ran.code).toBe(0);
    const out = JSON.parse(ran.io.lines[0]!) as { threadId: string; workspaceId: string; harness: string; outcome: string; sub: SubOut };
    expect(out).toMatchObject({ threadId: expect.any(String), workspaceId: expect.any(String), harness: "claude", outcome: "started" });
    expect(out.sub).toEqual({
      on: "boat",
      machine: "Fix the flaky upload test",
      size: { cpu: 2, memMb: 4096 },
      rateUsdPerHour: 0.018,
      repo: "https://github.com/acme/web.git",
      base: { branch: "feat/upload", sha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" },
      branch: "wsp/fix-the-flaky-upload-test",
      sent: { paths: [".env", ".env.local"], bytes: 1243, missing: [] },
      notSent: { uncommitted: 2 },
      deleteAfterMinutes: 30,
    });
    expect(boat.creates).toHaveLength(1);
    expect(ran.io.errors).toContain("starting Fix the flaky upload test on Boat");
    expect(ran.io.errors).toContain("copying 2 files");
    expect(ran.io.errors).toContain("2 uncommitted changes in web did not travel; the sub-thread starts at feat/upload as pushed (4be1c0a).");
    await expect.poll(() => starts.length).toBe(2);
    const opened = starts[1]!.o;
    expect(opened.cwd).toBe("/root/web");
    expect(opened.prompt.startsWith("This thread runs on a machine of its own on Boat (small, 2 vCPU, 4 GB)")).toBe(true);
    expect(opened.prompt.endsWith(`The task:\n\n${TASK}`)).toBe(true);
    const row = (await rt.sessions.list()).find(r => r.threadId === out.threadId)!;
    expect(row).toMatchObject({ parentThreadId: threadId, rootThreadId: threadId, workspaceId: out.workspaceId });
    // Without --json the first line says where it runs.
    const plain = await wsp(launch, "run", "--on", "boat", "--size", "4x8", "--title", "Run the suite", "--detach", "Run the whole upload suite.");
    expect(plain.code).toBe(0);
    expect(plain.io.lines[0]).toMatch(/^thread [0-9a-f]{8} on boat in \/root\/web /);
    expect(boat.creates.at(-1)).toMatchObject({ cpu: 4, memMb: 8192 });
  });

  it("past Boat's machines at once answers held with capped, says the wait on stderr, and starts the sub-thread once a machine naps", async () => {
    const { rt, boat, starts, launch } = await leadOnThisComputer({ served: true, machines: 1, agents: { spawn: true, maxMachines: 5 } });
    const first = JSON.parse((await wsp(launch, "run", "--on", "boat", "--detach", "--notify", "me", TASK, "--json")).io.lines[0]!) as { workspaceId: string };
    await expect.poll(() => starts.length).toBe(2);
    const ran = await wsp(launch, "run", "--on", "boat", "--title", "Run the suite", "--detach", "--notify", "me", "Run the whole upload suite.", "--json");
    expect(ran.code, ran.io.errors.join("\n")).toBe(0);
    const out = JSON.parse(ran.io.lines[0]!) as { threadId: string; workspaceId: string; outcome: string; capped: unknown; sub: SubOut };
    expect(out).toMatchObject({ outcome: "held", capped: { placeId: "box", place: "Boat", running: 1, atOnce: 1, why: "slot" }, sub: { machine: "Run the suite" } });
    expect(ran.io.errors).toContain("waiting for a machine on Boat: 1 of 1 running");
    expect(boat.creates).toHaveLength(1);
    await rt.workspaces.nap(first.workspaceId);
    await expect.poll(() => boat.creates.length).toBe(2);
    await expect.poll(() => starts.length).toBe(3);
    expect(starts[2]!.o.prompt.endsWith("Run the whole upload suite.")).toBe(true);
    expect((await rt.sessions.list()).find(r => r.threadId === out.threadId)).toMatchObject({ workspaceId: out.workspaceId });
  });

  it("refuses the hour's starts used as the provider's class, exit 1, with no create", async () => {
    const { boat, launch } = await leadOnThisComputer({ served: true });
    boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, hour: { limit: 60, used: 60 } } };
    const ran = await wsp(launch, "run", "--on", "boat", "--detach", TASK, "--json");
    expect(ran.code).toBe(EXIT_CODES.provider);
    expect(JSON.parse(ran.io.errors.at(-1)!)).toEqual({ error: startsUsedLine("hour", 60, 60, "the $20 plan", "sub-thread Fix the flaky upload test"), class: "provider", exit: 1 });
    expect(boat.creates).toEqual([]);
  });

  it("the run tool with on answers the same object from the lead's own tool server", async () => {
    const { boat, starts, launch } = await leadOnThisComputer({ served: true });
    const env = { [HOST_TOKEN_ENV]: launch[HOST_TOKEN_ENV]!, [TURN_TOKEN_ENV]: launch[TURN_TOKEN_ENV]! };
    const client = await asToken(env[HOST_TOKEN_ENV]);
    const [toClient, toServer] = InMemoryTransport.createLinkedPair();
    await mcpServer(join("/nowhere", "state.json"), { env, scoped: true, dial: Object.assign(async () => client, { close: async () => {} }) }).connect(toServer);
    mcp = new Client({ name: "lead", version: "0.0.0" });
    await mcp.connect(toClient);
    const ran = await mcp.callTool({ name: "run", arguments: { on: "boat", size: "4x8", message: TASK, notify: ["me"], detach: true } });
    expect(ran.isError, JSON.stringify(ran.content)).not.toBe(true);
    const out = ran.structuredContent as { threadId: string; sub: SubOut };
    expect(out.sub).toMatchObject({ on: "boat", size: { cpu: 4, memMb: 8192 }, rateUsdPerHour: 0.036, sent: { paths: [], bytes: 0, missing: [] }, notSent: { uncommitted: 0, why: "nothing is picked for web" } });
    expect(ran.content).toEqual([{ type: "text", text: subOpenedLine(out.threadId, "boat", "/root/web") }]);
    expect(boat.creates).toHaveLength(1);
    await expect.poll(() => starts.length).toBe(2);
    expect(starts[1]!.o.cwd).toBe("/root/web");
    // A word beside on is refused before anything is made.
    const beside = await mcp.callTool({ name: "run", arguments: { on: "boat", cwd: "/tmp", message: TASK } });
    expect(beside.isError).toBe(true);
    expect(boat.creates).toHaveLength(1);
  });
});
