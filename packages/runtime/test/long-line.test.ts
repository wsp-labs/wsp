// SPDX-License-Identifier: AGPL-3.0-only
// A detached run that prints a line longer than a reader may hold, and a reader its consumer let go of: a
// machine run printed a 425 MB base64 tar as one line, its reader held it whole twice, and the wsp exec that read it
// left the reader at its last line, so the host kept 850 MB until it restarted.
import { execFile } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setFlagsFromString } from "node:v8";
import { runInNewContext } from "node:vm";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ExecResult, Machine } from "@wsp/engine";
import { EXEC_CHUNK_BYTES, EXEC_LINE_MAX_BYTES, subOpening, type TurnResult } from "@wsp/protocol";
import { LINUX_SHELL_PRELUDE } from "../../protocol/test/linux-shell.js";
import { machineExecStream } from "../src/machine-exec.js";
import type { HarnessAdapterFactory } from "../src/runtime.js";
import { scriptGuest } from "./script-guest.js";
import { stubBackend } from "./stub-backend.js";
import { leadOnThisComputer } from "./sub-fixture.js";

setFlagsFromString("--expose-gc");
const gc = runInNewContext("gc") as () => void;
/** What this process still holds outside V8's heap once everything it can let go of is gone. */
const externalAfterGc = async (): Promise<number> => {
  for (let i = 0; i < 4; i++) {
    await new Promise(resolve => setImmediate(resolve));
    gc();
  }
  return process.memoryUsage().external;
};
const MB = 1024 * 1024;
/** A base64 tarball as a run prints it: one line with no newline, under the cap so the reader holds it whole. */
const TARBALL = "H4sI".repeat(3 * MB);

const dirs: string[] = [];
afterEach(() => {
  vi.restoreAllMocks();
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

/** This computer's bash as the guest, given what a Linux machine has. */
function bashGuest(): { machine: Machine; runDir: string } {
  const dir = mkdtempSync(join(tmpdir(), "wsp-long-line-"));
  dirs.push(dir);
  const machine = {
    id: "local",
    exec: (cmd: string) =>
      new Promise<ExecResult>(resolve => {
        execFile("bash", ["-c", `${LINUX_SHELL_PRELUDE}${cmd}`], { maxBuffer: 16 * MB }, (e, stdout, stderr) => {
          resolve({ exitCode: e === null ? 0 : ((e as { code?: number }).code ?? 1), stdout, stderr });
        });
      }),
  } as unknown as Machine;
  return { machine, runDir: join(dir, "run") };
}

describe("a detached run's line longer than the cap", () => {
  it("reaches the reader whole in pieces of the cap, and the reader never holds more than the cap and one chunk", async () => {
    const { machine, runDir } = bashGuest();
    let held = 0;
    const concat = Buffer.concat.bind(Buffer);
    vi.spyOn(Buffer, "concat").mockImplementation((list, total) => {
      const joined = concat(list, total);
      held = Math.max(held, joined.length);
      return joined;
    });
    const stream = machineExecStream(machine, { pollMs: 5, runDir })(`head -c ${50 * MB} /dev/zero | tr '\\0' a`, { env: {} });
    const pieces: number[] = [];
    let total = 0;
    let other = 0;
    for await (const line of stream.lines) {
      pieces.push(Buffer.byteLength(line));
      total += line.length;
      other += line.replaceAll("a", "").length;
    }
    expect(await stream.exited).toBe(0);
    expect(total).toBe(50 * MB);
    expect(other).toBe(0);
    expect(Math.max(...pieces)).toBeLessThanOrEqual(EXEC_LINE_MAX_BYTES);
    expect(held).toBeLessThanOrEqual(EXEC_LINE_MAX_BYTES + EXEC_CHUNK_BYTES);
  }, 120_000);

  it("cuts its pieces where a character starts, so a line of two-byte characters comes back as it was", async () => {
    const backend = stubBackend();
    await backend.create({ kind: "sandbox" });
    // One byte ahead of the two-byte characters, so the cap falls inside one.
    const line = `a${"é".repeat(EXEC_LINE_MAX_BYTES / 2 + 1000)}`;
    scriptGuest(backend, [{ append: `${line}\nend\n`, exit: 0 }]);
    const stream = machineExecStream(backend.machines[0]!, { pollMs: 5 })("true", { env: {} });
    const lines: string[] = [];
    for await (const l of stream.lines) lines.push(l);
    expect(lines.length).toBe(3);
    expect(lines.slice(0, 2).join("")).toBe(line);
    expect(lines[2]).toBe("end");
  }, 60_000);
});

describe("a reader its consumer let go of", () => {
  it("is finished: the set of runs being read lets go of it, its exit settles, and what it held is freed", async () => {
    const backend = stubBackend();
    await backend.create({ kind: "sandbox" });
    // The guest's log is a Buffer of this process too, so it is full before the measure starts.
    const guest = scriptGuest(backend, []);
    guest.append(`${"a".repeat(24 * MB)}\nmore\n`);
    const reading = new Set<() => void>();
    const before = await externalAfterGc();
    await (async () => {
      const stream = machineExecStream(backend.machines[0]!, { pollMs: 5, reading })("true", { env: {} });
      for await (const line of stream.lines) {
        expect(line.length).toBeGreaterThan(MB);
        break;
      }
      expect(reading.size).toBe(0);
      expect(await Promise.race([stream.exited, new Promise(resolve => setTimeout(resolve, 2_000, "unsettled"))])).toBeNull();
    })();
    await new Promise(resolve => setTimeout(resolve, 50));
    expect(((await externalAfterGc()) - before) / MB).toBeLessThan(4);
  }, 60_000);

  it("on a wsp exec whose machine paused mid-run, holds nothing of the line it was reading once the exec has ended", async () => {
    const busy: HarnessAdapterFactory = () => ({
      steers: false,
      start: o => (o.onEvent({ type: "session.start", sessionId: "busy" }), { localId: "busy", finished: new Promise<TurnResult>(() => {}), interrupt: async () => {} }),
    });
    const f = await leadOnThisComputer({ subAdapter: busy, agents: { spawn: true, maxMachines: 3 } });
    try {
      const made = await f.rt.workspaces.create({ project: f.project.id, name: "Pack the build", sub: { on: "boat" } }, f.lead);
      await f.endLead();
      // The sub-thread's agent works on, so its machine stands for the command.
      await f.rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Pack the build."), harness: "claude", cwd: made.sub!.folder }, f.lead);
      const others = f.boat.execImpl;
      const guest = scriptGuest(f.boat, [{ append: "packing\n" }], others);
      const before = await externalAfterGc();
      await (async () => {
        const ran = await f.rt.workspaces.execStream(made.id, ["sh", "-c", "tar cz . | base64 -w0"], undefined, f.lead);
        const lines: string[] = [];
        const read = (async () => {
          for await (const l of ran.lines) lines.push(l);
        })();
        await vi.waitFor(() => expect(lines).toEqual(["packing"]));
        // The tarball as one line with no newline, read whole, and the machine pausing while the reader holds it.
        guest.append(TARBALL);
        await vi.waitFor(() => expect(guest.calls.some(c => c.includes(`tail -c +${"packing\n".length + TARBALL.length + 1} `))).toBe(true), { timeout: 30_000 });
        await f.rt.workspaces.nap(made.id);
        await expect(read).rejects.toThrow("machine paused");
        expect(await ran.exited).toBeNull();
      })();
      await new Promise(resolve => setTimeout(resolve, 200));
      // What the guest's own log took is no part of what the host kept.
      expect(((await externalAfterGc()) - before - TARBALL.length) / MB).toBeLessThan(4);
    } finally {
      await f.rt.close();
    }
  }, 120_000);
});
