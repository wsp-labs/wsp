// SPDX-License-Identifier: AGPL-3.0-only
// The line wsp pause and wsp wake print names the nap off the computer the
// workspace's own machine stands on, which a sub-thread on another cloud does
// not share with its project.
import type { WorkspaceView } from "@wsp/protocol";
import { describe, expect, it } from "vitest";
import { ALL_VERBS, runVerb, type CliVerb, type HostClient } from "../src/verbs.js";
import { captured } from "./verbs-fixture.js";

const KID: WorkspaceView = {
  id: "ws_kid",
  name: "kid",
  machineId: "m_kid",
  project: { id: "pr_1", name: "web", path: "/root/web", computer: "solari" },
  phase: "running",
  golden: "snap_g",
  createdAt: "2026-09-01T00:00:00Z",
  parentThreadId: "thr_lead",
  rootThreadId: "thr_lead",
  place: "box",
  provider: "box",
  pauseMode: "disk",
};

/** A host whose project lands on a cloud that keeps the memory, holding a Boat sub-thread of it, in the phase given. */
function hostWith(phase: WorkspaceView["phase"]): HostClient {
  const kid = { ...KID, phase };
  const replies: Record<string, unknown> = {
    "workspaces.resolve": { workspace: kid },
    "workspaces.nap": { workspace: { ...kid, phase: "napping" } },
    "workspaces.wake": { workspace: kid },
    "status.list": { statuses: [{ ...kid, machineState: "paused", reach: { state: "napping" }, size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018 }] },
    "workspaces.landing": { name: "Solari", capabilities: { pauseMode: "memory", sizes: [] } },
  };
  return { request: async (op: string) => replies[op] ?? Promise.reject(new Error(`unexpected op ${op}`)), close: () => {} } as unknown as HostClient;
}

// Out of the whole table, so the line is read whether or not the cloud is on where the suite runs.
const verb = (name: string) => ALL_VERBS.find((v): v is CliVerb => v.name === name && "run" in v)!;

describe("the state line of a Boat sub-thread of a project on a cloud that keeps the memory", () => {
  it("wsp pause says it stopped", async () => {
    const io = captured();
    expect(await runVerb(verb("pause"), ["pause", "kid"], io, () => "/tmp/state.json", { env: {}, dial: async () => hostWith("running") })).toBe(0);
    expect(io.lines).toEqual(["kid stopped"]);
  });

  it("wsp wake on one the listing still reads asleep says it stopped", async () => {
    const io = captured();
    expect(await runVerb(verb("wake"), ["wake", "kid"], io, () => "/tmp/state.json", { env: {}, dial: async () => hostWith("napping") })).toBe(0);
    expect(io.lines).toEqual(["kid stopped"]);
  });
});
