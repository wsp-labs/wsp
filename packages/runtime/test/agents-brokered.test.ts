// SPDX-License-Identifier: AGPL-3.0-only
// A workspace at a provider is a machine whose credentials the host keeps, and a
// sign-in asked of it is handed a target that says so; a workspace beside a
// provider that is not in the host's table is handed none.
import { describe, expect, it } from "vitest";
import { createRuntime, wiredPlace, type PlaceBackends } from "../src/runtime.js";
import type { AgentsActs, AgentsOn } from "../src/agents-read.js";
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
