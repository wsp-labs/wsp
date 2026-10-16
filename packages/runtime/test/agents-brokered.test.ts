// SPDX-License-Identifier: AGPL-3.0-only
// A workspace at a provider is a machine whose credentials the host keeps, and a
// sign-in asked of it is handed a target that says so; a workspace beside a
// provider that is not in the host's table is handed none.
import { describe, expect, it, onTestFinished } from "vitest";
import { createRuntime, wiredPlace, type PlaceBackends } from "../src/runtime.js";
import type { AgentsActs, AgentsOn, AgentsReader } from "../src/agents-read.js";
import { memoryStore } from "../src/store.js";
import { createOn, stubBackend } from "./stub-backend.js";

function planning(places?: (backend: ReturnType<typeof stubBackend>) => PlaceBackends) {
  const backend = stubBackend();
  const planned: AgentsOn[] = [];
  const acts: AgentsActs = {
    signInLine: async () => ({ command: "true" }),
    signIn: async on => {
      planned.push(on);
      return async run => run.emit({ state: "signed-in" });
    },
    key: async () => undefined,
    addTools: async () => ({ file: "~/.claude.json" }),
  };
  const rt = createRuntime({ backend, store: memoryStore(), adapters: {}, agentsActs: acts, ...(places !== undefined ? { places: places(backend) } : {}) });
  return { rt, planned };
}

describe("a sign-in asked of a workspace's machine", () => {
  it("is handed a brokered target at a provider, and none where the place is not in the host's provider table", async () => {
    for (const [places, brokered] of [[undefined, true], [(b: ReturnType<typeof stubBackend>) => ({ ...wiredPlace("default", b), list: () => [] }), false]] as const) {
      const { rt, planned } = planning(places);
      const ws = await createOn(rt, { name: "x", golden: "snap_base" });
      // The stub serves no daemon, so the run itself fails once the plan is made; the plan is what is read here.
      await rt.agents.signIn({ workspaceId: ws.id }, { agent: "claude", server: "notion" }, () => {}).catch(() => undefined);
      expect(planned[0]).toMatchObject({ kind: "machine" });
      expect(planned[0]!.kind === "machine" && planned[0]!.brokered === true).toBe(brokered);
      await rt.close();
    }
  });
});

describe("a read of a workspace's machine", () => {
  it("is handed the folders its turns' variables name: the config folder, and the plugins the image seeded", async () => {
    const asked: AgentsOn[] = [];
    const agentsReader: AgentsReader = { read: on => (asked.push(on), Promise.reject(new Error("not read here"))), tools: () => Promise.reject(new Error("not asked here")) };
    const rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: {}, agentsReader });
    onTestFinished(() => rt.close());
    const ws = await createOn(rt, { name: "x", golden: "snap_base" });
    await rt.agents.read({ workspaceId: ws.id }).catch(() => undefined);
    expect(asked[0]).toMatchObject({ kind: "machine", stores: { claude: "/root/.claude-cfg" }, seeds: { claude: "/root/.claude-plugin-seed" } });
  });
});
