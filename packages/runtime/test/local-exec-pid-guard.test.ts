// SPDX-License-Identifier: AGPL-3.0-only
// The agent can write a run's .pid file, so a live run never reads it and an attach after a restart reads it under a
// check. Every signal goes through a spy on process.kill that records it and sends it only to a group these tests
// launched: this file depends on no option of local-exec, so on a tree with no guard, the one before this fix
// included, it sends nothing it should not.
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ExecStream } from "@wsp/protocol";
import { groupExists, localExecStream, type LocalExecOptions } from "../src/local-exec.js";

const real = process.kill.bind(process);

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
    vi.spyOn(console, "warn").mockImplementation((line: unknown) => void said.push(String(line)));
    vi.spyOn(process, "kill").mockImplementation(((pid: number, sig?: string | number) => {
      if (sig === 0 || sig === undefined) return real(pid, 0);
      sent.push([pid, String(sig)]);
      if (ours.has(-pid)) real(pid, sig);
      return true;
    }) as typeof process.kill);
  });
  afterEach(async () => {
    for (const drop of [...reading]) drop();
    for (const pid of ours) if (groupExists(pid)) real(-pid, "SIGKILL");
    await vi.waitFor(() => expect([...ours].filter(groupExists)).toEqual([]), { timeout: 5_000 });
    vi.restoreAllMocks();
    rmSync(root, { recursive: true, force: true });
  });

  /** A poll that naps for a minute settles nothing while a case reads what teardown did; a kill is noticed by the poll. */
  const options = (pollMs = 10): LocalExecOptions => ({ root, runDir, pollMs, reading });

  const launch = (pollMs?: number): ExecStream & { leader: number } => {
    const stream = localExecStream(options(pollMs))("sleep 300", { env: {} });
    ours.add(stream.pid!);
    return Object.assign(stream, { leader: stream.pid! });
  };

  /** A process this test started that leads a group of its own, as a bystander on the computer does. */
  const bystander = (command = "exec sleep 300"): number => {
    const child = spawn("bash", ["-c", command], { detached: true, stdio: "ignore" });
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

  it("a live run signals the pid its launch holds, whatever its pid file says", async () => {
    // The agent controls the run folder and its times, so no value in it may aim a Stop at another group.
    const aims: ((base: string) => number | undefined)[] = [
      base => void writeFileSync(`${base}.pid`, "1\n"),
      // A bystander that started inside the claim's window, which the age check alone takes for the run's own.
      base => {
        const other = bystander();
        writeFileSync(`${base}.pid`, `${other}\n`);
        return other;
      },
      // A group whose leader is gone and a child holds open.
      base => {
        const other = bystander("sleep 300 & exit");
        writeFileSync(`${base}.pid`, `${other}\n`);
        return other;
      },
    ];
    for (const aim of aims) {
      for (const verb of ["teardown", "kill"] as const) {
        sent.length = 0;
        const stream = launch(verb === "teardown" ? 60_000 : 10);
        const other = aim(stream.run!);
        if (other !== undefined) await vi.waitFor(() => expect(groupExists(other)).toBe(true));
        stream[verb]();
        await vi.waitFor(() => expect(sent.length).toBeGreaterThan(0), { timeout: 3_000 });
        if (verb === "kill") await stream.exited;
        expect(new Set(sent.map(([pid]) => pid))).toEqual(new Set([-stream.leader]));
        expect(sent[0]).toEqual([-stream.leader, verb === "teardown" ? "SIGTERM" : "SIGKILL"]);
        if (other !== undefined) expect(groupExists(other)).toBe(true);
        await vi.waitFor(() => expect(groupExists(stream.leader)).toBe(false), { timeout: 5_000 });
      }
    }
    expect(said).toEqual([]);
  }, 60_000);

  /** A run the host that launched it let go of, as a restart leaves one: the pid file is all an attach has. */
  const orphaned = async (word: string, pollMs: number): Promise<ExecStream & { leader: number }> => {
    const first = launch(60_000);
    const base = first.run!;
    for (const drop of [...reading]) drop();
    backdate(base, 600_000);
    writeFileSync(`${base}.pid`, word);
    const attached = await localExecStream(options(pollMs)).attach!(base, { input: false, startedAt: Date.now() });
    expect(attached).not.toBe("gone");
    return Object.assign(attached as ExecStream, { leader: first.leader });
  };

  const strangers: [string, () => string][] = [
    ["1", () => "1\n"],
    ["0", () => "0\n"],
    ["-1", () => "-1\n"],
    ["a word", () => "leader\n"],
    ["the pid of a process outside the run", () => `${bystander()}\n`],
  ];

  describe.each(strangers)("an attach to a run whose pid file holds %s", (_name, text) => {
    it("sends nothing on teardown and says so once", async () => {
      const stream = await orphaned(text(), 60_000);
      stream.teardown();
      await vi.waitFor(() => expect(sent.length + said.length).toBeGreaterThan(0), { timeout: 3_000 });
      await new Promise(resolve => setTimeout(resolve, 150));
      refused();
      expect(groupExists(stream.leader)).toBe(true);
    });

    it("sends nothing on kill and says so", async () => {
      const stream = await orphaned(text(), 10);
      stream.kill();
      await stream.exited;
      expect(sent).toEqual([]);
      expect(said.length).toBeGreaterThan(0);
      expect(groupExists(stream.leader)).toBe(true);
    }, 20_000);
  });

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
      const stream = launch(60_000);
      await localExecStream(options()).sweep!([]);
      expect(sent).toContainEqual([-stream.leader, "SIGTERM"]);
      await vi.waitFor(() => expect(groupExists(stream.leader)).toBe(false), { timeout: 5_000 });
      expect(said).toEqual([]);
    }, 20_000);
  });
});
