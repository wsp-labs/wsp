// SPDX-License-Identifier: AGPL-3.0-only
// A process kept between a thread's turns on a joined computer's road: this computer's bash stands in for that
// computer, and an agent that answers each line of its input serves one turn after another from one process.
import { execFile } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ExecResult, Machine } from "@wsp/engine";
import { keepRun, shellQuote, type ExecStream } from "@wsp/protocol";
import { machineExecStream } from "../src/machine-exec.js";
import { LINUX_SHELL_PRELUDE } from "../../protocol/test/linux-shell.js";

const guests: { machine: Machine; runDir: string; root: string }[] = [];
const pids: number[] = [];

/** Ends every run a case started, closed or not, with its whole group: a case that failed or timed out before its
 * close leaves its agent and input pump waiting on a message that never comes. */
async function endGuests(): Promise<void> {
  for (const pid of pids.splice(0)) {
    try {
      process.kill(pid, "SIGKILL");
    } catch {
      // gone already, as the case asserts
    }
  }
  for (const { machine, runDir, root } of guests.splice(0)) {
    await machineExecStream(machine, { runDir }).sweep!([]);
    rmSync(root, { recursive: true, force: true });
  }
}
afterEach(endGuests);

/** This computer's bash as the computer the run is on, every exec's text kept. */
function guest(): { machine: Machine; runDir: string; root: string; calls: string[] } {
  const root = mkdtempSync(join(tmpdir(), "wsp-kept-road-"));
  const calls: string[] = [];
  const machine = {
    id: "box",
    exec: (cmd: string) =>
      new Promise<ExecResult>(resolve => {
        calls.push(cmd);
        execFile("bash", ["-c", `${LINUX_SHELL_PRELUDE}${cmd}`], { maxBuffer: 16 * 1024 * 1024 }, (e, stdout, stderr) => {
          resolve({ exitCode: e === null ? 0 : ((e as { code?: number }).code ?? 1), stdout, stderr });
        });
      }),
  } as unknown as Machine;
  const runDir = join(root, "run");
  guests.push({ machine, runDir, root });
  return { machine, runDir, root, calls };
}

/** An agent that answers every line it reads with `got <line>` and its own pid, and on "leave" first starts a sleep
 * whose parent exits, so it stays behind in the run's group as a command a turn left going does. */
const agent = (left: string): string =>
  `while IFS= read -r l; do if [ "$l" = leave ]; then ( sleep 300 & echo $! > ${shellQuote(left)} ); fi; echo "got $l $$"; done`;

/** Lines off a turn's view until one matches. */
async function until(lines: AsyncIterator<string>, match: (line: string) => boolean): Promise<string[]> {
  const seen: string[] = [];
  for (;;) {
    const next = await lines.next();
    if (next.done === true) throw new Error(`the turn ended before a match, after ${JSON.stringify(seen)}`);
    seen.push(next.value);
    if (match(next.value)) return seen;
  }
}

const alive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

describe("a process kept between turns on a joined computer's road", () => {
  it("rests past the idle limit between turns and answers the next turn's message from the same process", async () => {
    const { machine, runDir, root } = guest();
    // Every wait short, the resting one too, so the idle limit is read many times while no turn is open.
    const stream = machineExecStream(machine, { pollMs: 20, runDir, idleMs: 400, sleep: ms => new Promise(resolve => setTimeout(resolve, Math.min(ms, 50))) })(agent(join(root, "left")), { env: {}, input: ["one"] });
    const keeper = keepRun(stream);
    const first = keeper.turn();
    expect(first.from).toBeUndefined();
    const one = await until(first.stream.lines[Symbol.asyncIterator](), line => line.startsWith("got one"));
    first.stream.closeInput();
    // Twice the idle limit with no turn open: a turn this quiet would be cut, a resting process is not.
    await new Promise(resolve => setTimeout(resolve, 900));
    expect(keeper.up).toBe(true);

    const second = keeper.turn();
    expect(second.from).toBeGreaterThan(0);
    expect(await second.stream.write("two")).toBe("written");
    const two = await until(second.stream.lines[Symbol.asyncIterator](), line => line.startsWith("got two"));
    expect(two).toEqual([`got two ${one.at(-1)!.split(" ").at(-1)}`]);

    await keeper.close(1_000, 500);
    await vi.waitFor(() => expect(readdirSync(runDir)).toEqual([]), { timeout: 5_000 });
  }, 20_000);

  it("wakes the resting reader for the next turn's message rather than waiting out its slow poll", async () => {
    const { machine, runDir, root } = guest();
    // The resting poll's wait never ends, so the next turn's lines come only if its start wakes the reader.
    const sleep = (ms: number): Promise<void> => (ms >= 10_000 ? new Promise<void>(() => {}) : new Promise<void>(resolve => setTimeout(resolve, ms)));
    const stream = machineExecStream(machine, { pollMs: 20, runDir, sleep })(agent(join(root, "left")), { env: {}, input: ["one"] });
    const keeper = keepRun(stream);
    const first = keeper.turn();
    await until(first.stream.lines[Symbol.asyncIterator](), line => line.startsWith("got one"));
    first.stream.closeInput();
    await new Promise(resolve => setTimeout(resolve, 200));
    const second = keeper.turn();
    await second.stream.write("two");
    expect(await until(second.stream.lines[Symbol.asyncIterator](), line => line.startsWith("got two"))).toHaveLength(1);
    await keeper.close(1_000, 500);
  }, 20_000);

  // What a turn left is read off /proc, which a joined computer has and a Mac standing in for one does not.
  it.skipIf(!existsSync("/proc/1/stat"))("ends what a turn left running beside the agent once that turn is over, and keeps the agent up", async () => {
    const { machine, runDir, root } = guest();
    const left = join(root, "left");
    const stream = machineExecStream(machine, { pollMs: 20, runDir })(agent(left), { env: {}, input: ["leave"] });
    const keeper = keepRun(stream);
    const first = keeper.turn();
    await until(first.stream.lines[Symbol.asyncIterator](), line => line.startsWith("got leave"));
    const stray = Number(readFileSync(left, "utf8").trim());
    pids.push(stray);
    expect(alive(stray)).toBe(true);
    first.stream.closeInput();
    await vi.waitFor(() => expect(alive(stray)).toBe(false), { timeout: 5_000 });
    expect(keeper.up).toBe(true);

    const second = keeper.turn();
    await second.stream.write("still");
    expect(await until(second.stream.lines[Symbol.asyncIterator](), line => line.startsWith("got still"))).toHaveLength(1);
    await keeper.close(1_000, 500);
    await vi.waitFor(() => expect(readdirSync(runDir)).toEqual([]), { timeout: 5_000 });
  }, 20_000);

  it("re-opens a turn on a kept process from where that turn began, past the turns before it", async () => {
    const { machine, runDir, root } = guest();
    const factory = machineExecStream(machine, { pollMs: 20, runDir });
    const stream = factory(agent(join(root, "left")), { env: {}, input: ["one"] });
    const keeper = keepRun(stream);
    const first = keeper.turn();
    await until(first.stream.lines[Symbol.asyncIterator](), line => line.startsWith("got one"));
    first.stream.closeInput();
    const second = keeper.turn();
    await second.stream.write("two");
    await until(second.stream.lines[Symbol.asyncIterator](), line => line.startsWith("got two"));

    // A host that restarts in the second turn re-opens it from its own first byte.
    const reopened = (await factory.attach!(stream.run!, { input: true, startedAt: Date.now(), from: second.from! })) as ExecStream;
    const read = reopened.lines[Symbol.asyncIterator]();
    expect((await read.next()).value).toMatch(/^got two /);
    await keeper.close(1_000, 500);
    reopened.kill();
    await vi.waitFor(() => expect(readdirSync(runDir)).toEqual([]), { timeout: 5_000 });
  }, 20_000);

  it("a run a case never closed is ended with its input pump by the teardown every case runs", async () => {
    const { machine, runDir, root } = guest();
    const stream = machineExecStream(machine, { pollMs: 20, runDir })(agent(join(root, "left")), { env: {}, input: ["one"] });
    const keeper = keepRun(stream);
    await until(keeper.turn().stream.lines[Symbol.asyncIterator](), line => line.startsWith("got one"));
    const leader = Number(readFileSync(`${stream.run!}.pid`, "utf8").trim());
    const pump = Number(readFileSync(`${stream.run!}.tail`, "utf8").trim());
    expect([alive(leader), alive(pump)]).toEqual([true, true]);
    await endGuests();
    await vi.waitFor(() => expect([alive(leader), alive(pump)]).toEqual([false, false]), { timeout: 5_000 });
    stream.kill();
  }, 20_000);
});
