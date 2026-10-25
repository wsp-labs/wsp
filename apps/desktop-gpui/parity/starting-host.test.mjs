// SPDX-License-Identifier: AGPL-3.0-only
// node --test apps/desktop-gpui/parity/starting-host.test.mjs: the starting fixture comes up with its sub-thread still
// starting on Boat and stops its host. It starts a real throwaway host, so it needs the packages built (pnpm build).
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));

test("the fixture's sub-thread reads starting on Boat with a running row and no events, and its host stops", () => {
  const dir = mkdtempSync(join(tmpdir(), "wsp-starting-test-"));
  const outDir = mkdtempSync(join(tmpdir(), "wsp-starting-out-"));
  const out = join(outDir, "out.json");
  try {
    // A host reached by these would be somebody else's; the fixture's host must never see them.
    const aimedWrong = { GIT_DIR: join(outDir, "not-a-repo"), WSP_HOST: "nobody", WSP_HOST_URL: "http://127.0.0.1:9", WSP_HOST_TOKEN: "wrong" };
    const ran = spawnSync(process.execPath, [join(here, "starting-host.mjs"), dir, out, "--hold", "0"], { env: { HOME: dir, PATH: "/usr/bin:/bin", ...aimedWrong }, encoding: "utf8", timeout: 90_000 });
    assert.equal(ran.status, 0, `${ran.stdout}\n${ran.stderr}`);
    assert.match(ran.stdout, /round 1 at \d+ s: row running, prompt "Move the cart.*sub starting on boat, spawn \{startedAt,branch,machine,log\}, events 0, projects whole: starting/);
    const { pid, state } = JSON.parse(readFileSync(out, "utf8"));
    assert.match(ran.stdout, new RegExp(`host ${pid} stopped, nothing left at that pid`));
    assert.equal(spawnSync("ps", ["-o", "args=", "-p", String(pid)], { encoding: "utf8" }).stdout.includes(state), false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
    rmSync(outDir, { recursive: true, force: true });
  }
});

test("with --stood the sub-thread's machine stood, its turn finished with four events, and its host stops", () => {
  const dir = mkdtempSync(join(tmpdir(), "wsp-starting-test-"));
  const outDir = mkdtempSync(join(tmpdir(), "wsp-starting-out-"));
  const out = join(outDir, "out.json");
  try {
    const ran = spawnSync(process.execPath, [join(here, "starting-host.mjs"), dir, out, "--hold", "0", "--stood"], { env: { HOME: dir, PATH: "/usr/bin:/bin" }, encoding: "utf8", timeout: 90_000 });
    assert.equal(ran.status, 0, `${ran.stdout}\n${ran.stderr}`);
    assert.match(ran.stdout, /round 1 at \d+ s: row completed, .*spawn \{startedAt,branch,machine,readyAt,log\}, events 4, projects whole: stood/);
    const { pid, stood, threads } = JSON.parse(readFileSync(out, "utf8"));
    assert.equal(stood, true);
    assert.equal(threads.sub, threads.starting);
    assert.match(ran.stdout, new RegExp(`host ${pid} stopped, nothing left at that pid`));
  } finally {
    rmSync(dir, { recursive: true, force: true });
    rmSync(outDir, { recursive: true, force: true });
  }
});
