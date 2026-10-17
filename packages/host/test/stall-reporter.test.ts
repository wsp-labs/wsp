// SPDX-License-Identifier: AGPL-3.0-only
// A run held by a case that never yields its event loop fails once nothing has moved for the stall budget, naming the
// file and the case, and stops the held worker; a run that keeps moving is left alone however long it takes. Driven
// through a real vitest run over a throwaway folder, with the reporter and the note setup CI's commands load.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));
const REPORTER = here("../../../vitest.stall.ts");
const NOTE = here("../../../vitest.stall-note.ts");
const VITEST = createRequire(here("../../../package.json")).resolve("vitest/vitest.mjs");

let dirs: string[] = [];
let groups: number[] = [];
afterEach(() => {
  for (const group of groups) {
    try {
      process.kill(-group, "SIGKILL");
    } catch {}
  }
  for (const dir of dirs) rmSync(dir, { recursive: true, force: true });
  [dirs, groups] = [[], []];
});

/** A folder run under the stall reporter with a budget of stallMs, for the test files the case writes into it. */
function suite(stallMs: number): string {
  const dir = mkdtempSync(join(tmpdir(), "wsp-stall-"));
  dirs.push(dir);
  const config = { test: { include: ["*.test.mjs"], setupFiles: [NOTE], reporters: ["default", [REPORTER, { stallMs }]] } };
  writeFileSync(join(dir, "vitest.config.mjs"), `export default ${JSON.stringify(config)};\n`);
  return dir;
}

function run(dir: string): Promise<{ code: number | null; out: string; ms: number }> {
  const started = Date.now();
  // A group of its own: a held worker ignores its parent going, so a case that times out kills the group.
  const child = spawn(process.execPath, [VITEST, "run", "--root", dir, "--config", join(dir, "vitest.config.mjs"), "--shard=1/1"], { cwd: dir, detached: true });
  groups.push(child.pid!);
  let out = "";
  child.stdout.on("data", chunk => (out += chunk));
  child.stderr.on("data", chunk => (out += chunk));
  return new Promise(resolve => child.on("close", code => resolve({ code, out, ms: Date.now() - started })));
}

const alive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

describe("the stall reporter", () => {
  it("fails a run held by a case that never yields, naming the file and the case, and stops the held worker", async () => {
    const dir = suite(2_000);
    writeFileSync(join(dir, "fine.test.mjs"), 'import { test } from "vitest";\ntest("finishes", () => {});\n');
    writeFileSync(
      join(dir, "held.test.mjs"),
      [
        'import { writeFileSync } from "node:fs";',
        'import { describe, test } from "vitest";',
        'describe("a suite", () => {',
        '  test("goes first", () => {});',
        '  test("holds its event loop", () => {',
        `    writeFileSync(${JSON.stringify(join(dir, "held.pid"))}, String(process.pid));`,
        "    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0);",
        "  });",
        "});",
      ].join("\n"),
    );
    writeFileSync(
      join(dir, "later.test.mjs"),
      [
        'import { beforeAll, describe, test } from "vitest";',
        'describe("first block", () => { test("ends", () => {}); });',
        'describe("second block", () => {',
        "  beforeAll(() => { Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0); });",
        '  test("never starts", () => {});',
        "});",
      ].join("\n"),
    );
    const result = await run(dir);
    const held = Number(readFileSync(join(dir, "held.pid"), "utf8"));

    expect(result.code).toBe(1);
    expect(result.out).toContain("no case started or finished for 2 s; 2 files unfinished:");
    expect(result.out).toMatch(/ {2}held\.test\.mjs: a suite > holds its event loop \(running \d+ s\)/);
    expect(result.out).toContain("  later.test.mjs: held after first block > ends ended, in an afterAll or a later block's beforeAll");
    expect(result.ms).toBeLessThan(30_000);
    // tinypool stops a worker with SIGTERM and a held one with SIGKILL a second later.
    const until = Date.now() + 5_000;
    while (alive(held) && Date.now() < until) await new Promise(r => setTimeout(r, 100));
    expect(alive(held)).toBe(false);
  }, 60_000);

  it("leaves a run alone that keeps moving for longer than the budget in all", async () => {
    const dir = suite(1_500);
    const step = "await new Promise(r => setTimeout(r, 600))";
    const cases = ["one", "two", "three"].map(name => `test("${name}", async () => { ${step}; });`);
    writeFileSync(join(dir, "slow.test.mjs"), ['import { test } from "vitest";', ...cases].join("\n"));
    const result = await run(dir);

    expect(result.out).not.toContain("no case started or finished");
    expect(result.code).toBe(0);
  }, 60_000);
});
