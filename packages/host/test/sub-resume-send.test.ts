// SPDX-License-Identifier: AGPL-3.0-only
// `wsp send` and the send tool into a sub-thread whose machine Boat deleted,
// against a host holding Boat as a stub: the send asks for no wake, makes a new
// machine and resumes the same session; the command line says on stderr what
// the new machine's stand-up said, a replaced box included.
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { afterEach, describe, expect, it } from "vitest";
import { BOX_WORK_DIR } from "@wsp/engine";
import { HOST_TOKEN_ENV, TURN_TOKEN_ENV, subOpening, subReplacedLine } from "@wsp/protocol";
import { ctx, sockets } from "../../runtime/test/places-fixture.js";
import { PUSHED, answering, jumpClock, leadOnThisComputer, until } from "../../runtime/test/sub-fixture.js";
import { WsClient } from "../../runtime/test/ws-client.js";
import { CLI_VERBS, runVerb, type HostClient } from "../src/verbs.js";
import { mcpServer } from "../src/mcp.js";
import { captured } from "./verbs-fixture.js";

const NAME = "Fix the flaky upload test";
const PREAMBLE_HEAD = "This thread's machine was deleted after its last turn";

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
    events: async (): Promise<void> => {},
    onFrame: () => () => {},
    closed: new Promise<void>(() => {}),
    closeWords: () => "closed",
    close: () => {},
    terminate: () => {},
  } as unknown as HostClient;
}

/** A sub-thread done once and its machine deleted at Boat, on a host served on a socket. */
async function releasedSub() {
  const { clock, jump } = jumpClock();
  const held = await leadOnThisComputer({ served: true, clock });
  answering(held.boat, { read: PUSHED, session: Buffer.from("gzipped claude session") });
  const made = await held.rt.workspaces.create({ project: held.project.id, name: NAME, sub: { on: "boat" } }, held.lead);
  const child = await held.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Remember the word lantern."), harness: "claude", turnToken: held.token, title: NAME, cwd: made.sub!.folder }, held.lead);
  const threadId = child.view().threadId!;
  await child.finished;
  const phase = async (): Promise<string | undefined> => ((await held.store.get("workspaces", made.id)) as { phase: string } | undefined)?.phase;
  await until(async () => ((await phase()) === "napping" ? true : undefined), "the nap at the done");
  jump(31 * 60_000);
  await until(async () => ((await phase()) === "released" ? true : undefined), "the release");
  return { ...held, made, threadId };
}

async function runs(env: Record<string, string>, token: string, ...argv: string[]) {
  const io = captured();
  const verb = CLI_VERBS.find(v => v.name === argv[0])!;
  const client = await asToken(token);
  const code = await runVerb(verb, argv, io, () => "/nowhere/state.json", { env, dial: async () => client });
  return { code, io };
}

describe("wsp send into a sub-thread whose machine was deleted", () => {
  it("as the person, makes a new machine with no wake asked, resumes the same session, and says the replaced box on stderr", async () => {
    const held = await releasedSub();
    held.boat.faults.push({ kind: "unusable" });
    const creates = held.boat.creates.length;
    const said = await runs({}, "host-token", "send", held.threadId.slice(0, 8), "what was the word?", "--detach");
    expect(said.code, said.io.errors.join("\n")).toBe(0);
    expect(held.boat.creates).toHaveLength(creates + 2);
    const launch = held.starts.at(-1)!.o;
    expect(launch.resume).toBe("s2");
    expect(launch.prompt.startsWith(PREAMBLE_HEAD)).toBe(true);
    const broken = held.boat.machines.at(-2)!;
    expect(said.io.errors).toContain(subReplacedLine({ kind: "unusable", box: broken.id, why: `${BOX_WORK_DIR} is missing` }, NAME, "Boat"));
  });

  it("with the lead's own launch tokens, does the same", async () => {
    const held = await releasedSub();
    const env = { [HOST_TOKEN_ENV]: held.launch[HOST_TOKEN_ENV]!, [TURN_TOKEN_ENV]: held.launch[TURN_TOKEN_ENV]! };
    const said = await runs(env, env[HOST_TOKEN_ENV], "send", held.threadId, "what was the word?", "--detach");
    expect(said.code, said.io.errors.join("\n")).toBe(0);
    expect(held.starts.at(-1)!.o).toMatchObject({ resume: "s2" });
    expect(held.starts.at(-1)!.o.prompt.startsWith(PREAMBLE_HEAD)).toBe(true);
  });
});

describe("the send tool into a sub-thread whose machine was deleted", () => {
  it("makes a new machine and resumes the same session, its answer the turn's as ever", async () => {
    const held = await releasedSub();
    const client = await asToken("host-token");
    const [toClient, toServer] = InMemoryTransport.createLinkedPair();
    await mcpServer("/nowhere/state.json", { env: {}, dial: Object.assign(async () => client, { close: async () => {} }) }).connect(toServer);
    const mcp = new Client({ name: "person", version: "0.0.0" });
    await mcp.connect(toClient);
    try {
      const sent = await mcp.callTool({ name: "send", arguments: { thread: held.threadId, message: "what was the word?", detach: true } });
      expect(sent.isError, JSON.stringify(sent.content)).not.toBe(true);
      expect(held.starts.at(-1)!.o).toMatchObject({ resume: "s2" });
      expect(held.starts.at(-1)!.o.prompt.startsWith(PREAMBLE_HEAD)).toBe(true);
      expect(Object.keys(sent.structuredContent as object)).not.toContain("notice");
    } finally {
      await mcp.close();
    }
  });
});
