// SPDX-License-Identifier: AGPL-3.0-only
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { readFileSync } from "node:fs";
import { THREAD_OPS } from "@wsp/protocol";
import { THREAD_TOOLS, THREAD_COMPUTERS_WORDS } from "../src/thread-tools.js";
import { VERBS, hasTool, verbPage } from "../src/verbs.js";
import { wspSkill } from "../src/skill.js";
import { expect, it } from "vitest";
import { mcpServer } from "../src/mcp.js";

it("serves thread tools without advertising account management, without dialing a host", async () => {
  const dial = Object.assign(async () => { throw new Error("discovery must not dial"); }, { close: async () => {} });
  const server = mcpServer("/fixture/state.json", { env: {}, scoped: true, dial });
  const client = new Client({ name: "scope", version: "1" });
  const [a, b] = InMemoryTransport.createLinkedPair();
  try {
    await server.connect(a);
    await client.connect(b);
    const names = (await client.listTools()).tools.map(t => t.name);
    expect(names).toEqual(expect.arrayContaining(["computers", "projects", "run", "send", "stop", "threads", "slate_catalog"]));
    for (const name of ["agents", "skills", "servers", "add", "computers_set", "restart", "setup", "image_build", "agents_setup"]) expect(names).not.toContain(name);
    expect((await client.callTool({ name: "agents", arguments: {} })).isError).toBe(true);
  } finally {
    await client.close();
    await server.close();
  }
});

it("every advertised thread tool has a recorded path using only permitted host operations", () => {
  for (const name of THREAD_TOOLS) {
    const fixture = JSON.parse(readFileSync(new URL(`../../../daemon/crates/wsp-mcp/tests/answers/${name}.json`, import.meta.url), "utf8")) as { cases: { asked: { op: string }[] }[] };
    expect(fixture.cases.some(c => c.asked.length > 0 && c.asked.every(a => THREAD_OPS.includes(a.op))), name).toBe(true);
  }
});

it("CLI help, MCP discovery and the skill explain the same project-scoped computer listing", () => {
  const computers = VERBS.find(v => v.name === "computers")!;
  expect("run" in computers && verbPage(computers, "here").replace(/\s+/g, " ")).toContain(THREAD_COMPUTERS_WORDS);
  expect(hasTool(computers) && computers.tool.description).toContain(THREAD_COMPUTERS_WORDS);
  expect(wspSkill()).toContain(THREAD_COMPUTERS_WORDS);
  expect(wspSkill()).toContain("switching from MCP to the command line does not grant access");
});
