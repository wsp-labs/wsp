// SPDX-License-Identifier: AGPL-3.0-only
// `wsp delete <sub-thread>` and the delete tool, against a host holding Boat as
// a stub: the person is asked one question naming the machine and any work not
// pushed, then the thread and its machine go; a lead's own token is refused
// over work not pushed and answered over work that is.
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { afterEach, describe, expect, it } from "vitest";
import { HOST_TOKEN_ENV, TURN_TOKEN_ENV, subOpening } from "@wsp/protocol";
import { ctx, sockets } from "../../runtime/test/places-fixture.js";
import { PUSHED, UNPUSHED, answering, leadOnThisComputer } from "../../runtime/test/sub-fixture.js";
import { WsClient } from "../../runtime/test/ws-client.js";
import { CLI_VERBS, runVerb, type HostClient } from "../src/verbs.js";
import { mcpServer } from "../src/mcp.js";
import { captured } from "./verbs-fixture.js";

const NAME = "Fix the flaky upload test";
const BRANCH = "wsp/fix-the-flaky-upload-test";

let ended: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const end of ended) await end();
  ended = [];
});

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
    onFrame: () => () => {},
    closed: new Promise<void>(() => {}),
    closeWords: () => "closed",
    close: () => {},
    terminate: () => {},
  } as unknown as HostClient;
}

/** A sub-thread done once, its result read as `read` says, on a host served on a socket. */
async function doneSub(read: string) {
  const held = await leadOnThisComputer({ served: true });
  answering(held.boat, { read, session: null });
  const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat" } }, held.lead);
  const child = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Fix it."), harness: "claude", turnToken: held.token, title: NAME, cwd: made.sub!.folder }, held.lead);
  const threadId = child.view().threadId!;
  await child.finished;
  const end = Date.now() + 5_000;
  for (;;) {
    const r = (await held.store.get("workspaces", made.id)) as { phase: string } | undefined;
    if (r?.phase === "napping") break;
    if (Date.now() > end) throw new Error("the done never napped the machine");
    await new Promise(res => setTimeout(res, 10));
  }
  return { ...held, made, threadId, machine: held.boat.machines.at(-1)! };
}

/** A line the person types at a terminal, answering the question with `answer`. */
async function personTypes(answer: string, ...argv: string[]) {
  const io = captured();
  const asked: string[] = [];
  io.isTTY = true;
  io.ask = async (q: string) => (asked.push(q), answer);
  const verb = CLI_VERBS.find(v => v.name === argv[0])!;
  const client = await asToken("host-token");
  const code = await runVerb(verb, argv, io, () => "/nowhere/state.json", { env: {}, dial: async () => client });
  return { code, io, asked };
}

/** A line the lead runs, with the launch pair its turn was started with. */
async function leadRuns(launch: Readonly<Record<string, string>>, ...argv: string[]) {
  const io = captured();
  const verb = CLI_VERBS.find(v => v.name === argv[0])!;
  const env = { [HOST_TOKEN_ENV]: launch[HOST_TOKEN_ENV]!, [TURN_TOKEN_ENV]: launch[TURN_TOKEN_ENV]! };
  const client = await asToken(env[HOST_TOKEN_ENV]);
  const code = await runVerb(verb, argv, io, () => "/nowhere/state.json", { env, dial: async () => client });
  return { code, io };
}

describe("wsp delete <sub-thread>", () => {
  it("asks once, naming the machine on Boat and the work not pushed, then takes the thread and its machine", async () => {
    const held = await doneSub(UNPUSHED);
    const id8 = held.threadId.slice(0, 8);
    const said = await personTypes("yes", "delete", id8);
    expect(said.asked).toEqual([`Delete sub-thread ${id8} (${NAME}) and its machine on Boat?\n2 commits and 3 changed files on ${BRANCH} are not pushed and go with it.`]);
    expect(said.code, said.io.errors.join("\n")).toBe(0);
    expect(said.io.lines).toEqual([`deleted sub-thread ${id8} and its machine on Boat`]);
    expect(held.machine.killed).toBe(true);
    expect((await held.rt.sessions.list()).some(s => s.threadId === held.threadId)).toBe(false);
  });

  it("keeps everything when the person says no", async () => {
    const held = await doneSub(PUSHED);
    const id8 = held.threadId.slice(0, 8);
    const said = await personTypes("no", "delete", id8);
    expect(said.asked[0]).toBe(`Delete sub-thread ${id8} (${NAME}) and its machine on Boat?\nIts turns leave this computer and its machine is deleted; what it pushed to ${BRANCH} stays on the remote.`);
    expect(said.code).toBe(1);
    expect(held.machine.killed).toBe(false);
  });

  it("refuses the lead's own token over work not pushed, and answers machine deleted over work that is", async () => {
    const held = await doneSub(UNPUSHED);
    const refused = await leadRuns(held.launch, "delete", held.threadId, "--yes", "--json");
    expect(refused.code, refused.io.errors.join("\n")).toBe(3);
    expect(refused.io.errors.join("\n")).toContain(`sub-thread ${held.threadId.slice(0, 8)} holds 2 commits and 3 changed files not pushed on ${BRANCH}. Send it a push, or ask the person to delete it.`);
    expect(held.machine.killed).toBe(false);

    const pushed = await doneSub(PUSHED);
    const ok = await leadRuns(pushed.launch, "delete", pushed.threadId, "--yes", "--json");
    expect(ok.code, ok.io.errors.join("\n")).toBe(0);
    expect(JSON.parse(ok.io.lines[0]!)).toEqual({ threadId: pushed.threadId, workspaceId: pushed.made.id, threads: 1, machine: "deleted" });
    expect(pushed.machine.killed).toBe(true);
  });
});

describe("a sub-thread from before the machine's rate was kept", () => {
  it("lists through the threads tool", async () => {
    const sub = { state: "napping", on: "boat", machine: NAME, size: { cpu: 2, memMb: 4096 }, branch: BRANCH };
    const rows = [{ id: "s1", workspaceId: "ws_1", harness: "claude", status: "completed", threadId: "e17a90c3-0000", sub }];
    const client = {
      request: async (op: string) => {
        if (op === "sessions.list") return { sessions: rows };
        if (op === "workspaces.list") return { workspaces: [] };
        throw new Error(`unexpected op ${op}`);
      },
      events: async () => {},
      onFrame: () => () => {},
      closed: new Promise<void>(() => {}),
      closeWords: () => "closed",
      close: () => {},
      terminate: () => {},
    } as unknown as HostClient;
    const [toClient, toServer] = InMemoryTransport.createLinkedPair();
    await mcpServer("/nowhere/state.json", { env: {}, dial: Object.assign(async () => client, { close: async () => {} }) }).connect(toServer);
    const mcp = new Client({ name: "person", version: "0.0.0" });
    await mcp.connect(toClient);
    try {
      const listed = await mcp.callTool({ name: "threads", arguments: {} });
      expect(listed.isError, JSON.stringify(listed.content)).not.toBe(true);
      expect((listed.structuredContent as { threads: { sub?: unknown }[] }).threads[0]!.sub).toEqual(sub);
    } finally {
      await mcp.close();
    }
  });
});
