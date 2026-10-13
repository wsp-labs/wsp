// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's agent ended by a signal on its Boat machine: wsp reads the
// machine's kernel log once, and an out-of-memory end is said as one in the
// finished line its lead gets, with the size to start again at. The agent is
// the real Claude adapter, its run a scripted guest on the stub Boat.
import { describe, expect, it } from "vitest";
import { subOpening, type TurnResult } from "@wsp/protocol";
import { HARNESS_ADAPTERS } from "../src/adapters.js";
import type { HarnessAdapterFactory } from "../src/runtime.js";
import { scriptGuest, type Step } from "./script-guest.js";
import { leadOnThisComputer } from "./sub-fixture.js";

const NAME = "Fix the flaky upload test";
const OOM = "the agent on Fix the flaky upload test was killed for memory: its machine is small, 2 vCPU, 4 GB, and Boat's own processes hold part of that. Start the work again with size 4x8.";
const INIT = `${JSON.stringify({ type: "system", subtype: "init", session_id: "0b5e2c1a-7d4f-4e8a-9c3b-1f2e3d4c5b6a", model: "claude-opus-5-5", cwd: "/root/web", tools: [] })}\n`;

/** A sub-thread whose agent starts and then ends as `end` says, on a machine whose kernel log counts `kernel`. */
async function subTurn(end: Step, kernel: string, size?: string) {
  const f = await leadOnThisComputer({ subAdapter: HARNESS_ADAPTERS.claude, agents: { spawn: true, maxMachines: 3 } });
  const made = await f.rt.workspaces.create({ project: f.project.id, name: NAME, sub: { on: "boat", ...(size !== undefined ? { size } : {}) } }, f.lead);
  await f.endLead();
  const others = f.boat.execImpl;
  const readings: string[] = [];
  scriptGuest(f.boat, [{ append: INIT }, end, {}, {}], async (m, cmd) => {
    if (!cmd.includes("journalctl")) return others(m, cmd);
    readings.push(cmd);
    return { exitCode: 0, stdout: kernel, stderr: "" };
  });
  const child = await f.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Run the upload suite."), harness: "claude", notify: [f.threadId], turnToken: f.token, title: NAME, cwd: made.sub!.folder }, f.lead);
  const result = await child.finished;
  const row = (await f.rt.sessions.list()).find(r => r.threadId === child.view().threadId);
  // The lead was idle, so the finished line opens its next turn.
  await expect.poll(() => f.starts.length).toBe(2);
  return { result, row, line: f.starts.at(-1)!.o.prompt, readings };
}

describe("a sub-thread's agent ended by a signal", () => {
  it("exit 137 with the kernel's out-of-memory kill reads the log once and says the out-of-memory line in the finished line and the row", async () => {
    const { result, row, line, readings } = await subTurn({ exit: 137 }, "1\n");
    expect(readings).toHaveLength(1);
    expect(readings[0]).toMatch(/^journalctl -k --since @\d+ --no-pager \| grep -ci 'out of memory'/);
    expect(result).toMatchObject({ status: "failed", error: OOM });
    expect(row?.failure).toBe(OOM);
    expect(line).toContain(OOM);
  });

  it("a run gone with no exit code is a signal end too", async () => {
    const { line, readings } = await subTurn({ dead: true }, "2\n");
    expect(readings).toHaveLength(1);
    expect(line).toContain(OOM);
  });

  it("a command run with wsp exec on the machine keeps its exit code and reads nothing", async () => {
    // The sub-thread's agent works on, so its machine stands for the command.
    const busy: HarnessAdapterFactory = () => ({
      steers: false,
      start: o => (o.onEvent({ type: "session.start", sessionId: "busy" }), { localId: "busy", finished: new Promise<TurnResult>(() => {}), interrupt: async () => {} }),
    });
    const f = await leadOnThisComputer({ subAdapter: busy, agents: { spawn: true, maxMachines: 3 } });
    const made = await f.rt.workspaces.create({ project: f.project.id, name: NAME, sub: { on: "boat" } }, f.lead);
    await f.endLead();
    await f.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Run the upload suite."), harness: "claude", cwd: made.sub!.folder }, f.lead);
    const others = f.boat.execImpl;
    const readings: string[] = [];
    scriptGuest(f.boat, [{ append: "building\n" }, { exit: 137 }, {}], async (m, cmd) => (cmd.includes("journalctl") ? (readings.push(cmd), { exitCode: 0, stdout: "1\n", stderr: "" }) : others(m, cmd)));
    const ran = await f.rt.workspaces.execStream(made.id, ["pnpm", "test"], undefined, f.lead);
    const lines: string[] = [];
    for await (const l of ran.lines) lines.push(l);
    expect(lines).toEqual(["building"]);
    expect(await ran.exited).toBe(137);
    expect(readings).toEqual([]);
  });

  it("the kernel counting no out-of-memory kill keeps the agent's own exit line", async () => {
    const { result, line, readings } = await subTurn({ exit: 137 }, "0\n");
    expect(readings).toHaveLength(1);
    expect(result.error).toBe("claude exited with code 137 before emitting a result");
    expect(line).not.toContain("killed for memory");
  });

  it("an agent that exited with a code of its own reads nothing", async () => {
    const { result, readings } = await subTurn({ exit: 1 }, "1\n");
    expect(readings).toEqual([]);
    expect(result.error).toBe("claude exited with code 1 before emitting a result");
  });

  it("names the next size up the plan sells, and says none is bigger on the largest", async () => {
    const { result } = await subTurn({ exit: 137 }, "1\n", "4x8");
    expect(result.error).toBe("the agent on Fix the flaky upload test was killed for memory: its machine is default, 4 vCPU, 8 GB, and Boat's own processes hold part of that. Start the work again with size 8x16.");
    // The plan sells small, default and large: 16x32 is not one of them.
    const largest = await subTurn({ exit: 137 }, "1\n", "8x16");
    expect(largest.result.error).toBe("the agent on Fix the flaky upload test was killed for memory: its machine is large, 8 vCPU, 16 GB, and Boat's own processes hold part of that. Boat sells no bigger size, so the work has to use less memory.");
  });
});
