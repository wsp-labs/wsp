// SPDX-License-Identifier: AGPL-3.0-only
// The deploy a runtime is handed writes the token the runtime passes. One of the
// deploy's own stood on the machine until the runtime wrote its own back, and the
// daemon closed 4401 every socket the host held in between, the pipes too.
import { describe, expect, it, vi } from "vitest";
import type { Machine } from "@wsp/engine";
import { DAEMON_DEPLOYED_LINE, deployDaemon } from "../src/doctor.js";
import { goldenRecipe } from "../src/cli/wiring.js";

vi.mock("../src/doctor.js", async original => ({
  ...(await original<typeof import("../src/doctor.js")>()),
  deployDaemon: vi.fn(async (_m: Machine, o: { token?: string } = {}) => ({ token: o.token ?? "f00d" })),
}));

describe("the deploy a runtime is handed", () => {
  it("writes the token the runtime passes, and mints one only where the runtime holds none", async () => {
    const machine = { id: "bx_acme" } as Machine;
    const deploy = goldenRecipe().deployDaemon!;
    expect(await deploy(machine, "ab12")).toBe(DAEMON_DEPLOYED_LINE);
    await deploy(machine);
    expect(vi.mocked(deployDaemon).mock.calls).toEqual([[machine, { token: "ab12" }], [machine, {}]]);
  });
});
