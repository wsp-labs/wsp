// SPDX-License-Identifier: AGPL-3.0-only
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { SWEEP_STAGED_CMD, sweepLeftovers } from "../src/golden-tools.js";
import type { ExecResult, Machine } from "../src/machine.js";

let dir: string | undefined;
afterEach(() => {
  if (dir !== undefined) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

describe("what wsp made on a machine, cleared when its disk runs short", () => {
  it("takes the staging a road left an hour ago and more, and never a road's still in flight, the live slates or anyone else's file", () => {
    dir = mkdtempSync(join(tmpdir(), "wsp-sweep-"));
    const hourAgo = new Date(Date.now() - 2 * 3_600_000);
    const old = (name: string, folder = false): string => {
      const path = join(dir!, name);
      if (folder) mkdirSync(path);
      else writeFileSync(path, "x");
      utimesSync(path, hourAgo, hourAgo);
      return path;
    };
    const gone = [old("wsp-put-a1b2c3.0"), old("wsp-get-d4e5f6.0001"), old("wsp-seed-ws_1.tgz"), old("wsp-session-t1.gz"), old("wsp-vault-1.tgz"), old("wsp-land-a1b2c3", true), old("wsp-road-Xy12ab", true)];
    const kept = [old("wsp-slates", true), old("other-file"), old("tmp.Abc123", true)];
    const inFlight = join(dir, "wsp-put-ffffff.0");
    writeFileSync(inFlight, "x");
    execFileSync("bash", ["-c", SWEEP_STAGED_CMD.replace("find /tmp ", `find ${dir} `)]);
    expect(gone.filter(existsSync)).toEqual([]);
    expect([...kept, inFlight].filter(p => !existsSync(p))).toEqual([]);
  });

  it("sweeps the staging, then the build caches, on the machine's own roads", async () => {
    const said: string[] = [];
    const answer = (cmd: string): ExecResult => (said.push(cmd), { exitCode: 0, stdout: "", stderr: "" });
    const machine = { id: "bx_1", exec: async (cmd: string) => answer(cmd), run: async (script: string) => answer(script) } as unknown as Machine;
    await sweepLeftovers(machine);
    expect(said[0]).toBe(SWEEP_STAGED_CMD);
    expect(said.some(c => c.includes("rm -rf /root/.npm /root/.cache/uv /root/.cache/go-build /root/.cache/node-gyp"))).toBe(true);
  });
});
