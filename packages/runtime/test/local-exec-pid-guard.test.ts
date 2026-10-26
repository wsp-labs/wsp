// SPDX-License-Identifier: AGPL-3.0-only
// The agent can write a run's .pid file, so what it names is read before it is signalled. Every factory here sends
// through a recorder: a run that signals a pid it did not start is a line in `sent`, never a signal. The recorder
// forwards only to the groups these tests launched, so a real leader still ends and nothing else is ever reached.
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ExecStream } from "@wsp/protocol";
import { groupExists, localExecStream, type LocalExecOptions } from "../src/local-exec.js";

describe("a run's .pid file names a pid before it is signalled", () => {
  let root: string;
  let runDir: string;
  let sent: [number, string][];
  let said: string[];
  let ours: Set<number>;
  let reading: Set<() => void>;
  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), "wsp-pidguard-"));
    runDir = join(root, "runs");
    sent = [];
    said = [];
    ours = new Set();
    reading = new Set();
  });
  afterEach(async () => {
    for (const drop of [...reading]) drop();
    for (const pid of ours) if (groupExists(pid)) process.kill(-pid, "SIGKILL");
    await vi.waitFor(() => expect([...ours].filter(groupExists)).toEqual([]), { timeout: 5_000 });
    rmSync(root, { recursive: true, force: true });
  });

  /** A poll that naps for a minute settles nothing while a case reads what teardown did; a kill is noticed by the poll. */
  const options = (pollMs = 10): LocalExecOptions => ({
    root,
    runDir,
    pollMs,
    reading,
    send: (pid, sig) => {
      sent.push([pid, sig]);
      if (ours.has(-pid)) process.kill(pid, sig);
    },
    log: line => said.push(line),
  });

  const launch = (pollMs?: number): ExecStream & { leader: number } => {
    const stream = localExecStream(options(pollMs))("sleep 300", { env: {} });
    ours.add(stream.pid!);
    return Object.assign(stream, { leader: stream.pid! });
  };

  /** A process this test started that leads a group of its own, as a bystander on the computer does. */
  const bystander = (): number => {
    const child = spawn("bash", ["-c", "sleep 300"], { detached: true, stdio: "ignore" });
    child.unref();
    ours.add(child.pid!);
    return child.pid!;
  };

  const backdate = (base: string, ms: number): void => {
    const at = new Date(Date.now() - ms);
    utimesSync(`${base}.d`, at, at);
  };

  /** When pid 1 started, as ps prints its age: the claim of a run is made at the same moment in a fresh pid namespace. */
  const initAgeMs = (): number => {
    const [span = "", days = "0"] = execFileSync("ps", ["-o", "etime=", "-p", "1"], { encoding: "utf8" }).trim().split("-").reverse();
    const parts = span.split(":").map(Number);
    const [hours = 0, minutes = 0, seconds = 0] = parts.length === 3 ? parts : [0, ...parts];
    return (((Number(days) * 24 + hours) * 60 + minutes) * 60 + seconds) * 1_000;
  };

  const refused = (): void => {
    expect(sent).toEqual([]);
    expect(said).toHaveLength(1);
  };

  const strangers: [string, () => string][] = [
    ["1", () => "1\n"],
    ["0", () => "0\n"],
    ["-1", () => "-1\n"],
    ["a word", () => "leader\n"],
    ["the pid of a process outside the run", () => `${bystander()}\n`],
  ];

  describe.each(strangers)("a file holding %s", (_name, text) => {
    const tampered = (pollMs?: number): ExecStream & { leader: number } => {
      const stream = launch(pollMs);
      const base = stream.run!;
      const word = text();
      // The claim's age is what says whose process a live pid is: a bystander is older than the run it is named in.
      backdate(base, 600_000);
      writeFileSync(`${base}.pid`, word);
      return stream;
    };

    it("sends nothing on teardown and says so once", async () => {
      const stream = tampered(60_000);
      stream.teardown();
      await vi.waitFor(() => expect(sent.length + said.length).toBeGreaterThan(0), { timeout: 3_000 });
      await new Promise(resolve => setTimeout(resolve, 150));
      refused();
      expect(groupExists(stream.leader)).toBe(true);
    });

    it("sends nothing on kill and says so", async () => {
      const stream = tampered();
      stream.kill();
      await stream.exited;
      expect(sent).toEqual([]);
      expect(said.length).toBeGreaterThan(0);
      expect(groupExists(stream.leader)).toBe(true);
    }, 20_000);
  });

  it("still signals a real leader: teardown and kill reach its group", async () => {
    const stream = launch(60_000);
    stream.teardown();
    await vi.waitFor(() => expect(sent).toEqual([[-stream.leader, "SIGTERM"]]), { timeout: 3_000 });
    await vi.waitFor(() => expect(groupExists(stream.leader)).toBe(false), { timeout: 5_000 });
    expect(said).toEqual([]);

    const next = launch();
    next.kill();
    await vi.waitFor(() => expect(sent).toContainEqual([-next.leader, "SIGKILL"]), { timeout: 3_000 });
    await next.exited;
    expect(said).toEqual([]);
  }, 20_000);

  describe("the reap", () => {
    const claimed = (text: string, age: number): string => {
      mkdirSync(runDir, { recursive: true, mode: 0o700 });
      const base = join(runDir, "aaaaaaaaaaaa");
      mkdirSync(`${base}.d`, { mode: 0o700 });
      writeFileSync(`${base}.pid`, text);
      backdate(base, age);
      return base;
    };

    it("keeps its age check: a live process that started long before the claim is left alone, and its files go", async () => {
      const stranger = bystander();
      const base = claimed(`${stranger}\n`, 600_000);

      expect(await localExecStream(options()).sweep!([])).toEqual([base]);

      expect(sent).toEqual([]);
      expect(readdirSync(runDir)).toEqual([]);
      expect(groupExists(stranger)).toBe(true);
    });

    it("refuses a file holding 1, 0, -1 or a word, and pid 1 even when its start sits inside the claim's window, with one line in the log each, and its files go", async () => {
      // In a fresh pid namespace pid 1 is as young as the claim, so the age check alone lets it through.
      const cases: [string, number][] = [["1\n", 1_000], ["0\n", 1_000], ["-1\n", 1_000], ["leader\n", 1_000], ["1\n", initAgeMs()]];
      for (const [text, age] of cases) {
        const base = claimed(text, age);

        expect(await localExecStream(options()).sweep!([])).toEqual([base]);

        refused();
        expect(readdirSync(runDir)).toEqual([]);
        sent.length = 0;
        said.length = 0;
      }
    }, 30_000);

    it("ends a real run's group, as before", async () => {
      const stream = launch();
      await localExecStream(options()).sweep!([]);
      expect(sent).toContainEqual([-stream.leader, "SIGTERM"]);
      await vi.waitFor(() => expect(groupExists(stream.leader)).toBe(false), { timeout: 5_000 });
      expect(said).toEqual([]);
    }, 20_000);
  });
});
