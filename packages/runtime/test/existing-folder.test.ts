// SPDX-License-Identifier: AGPL-3.0-only
// Read actual folders through the production inspection command, including quoted paths and a detached checkout.
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Machine } from "@wsp/engine";
import { expect, it } from "vitest";
import { existingFolder } from "../src/projects/existing-folder.js";

it("reads the canonical remote path, repository root and default branch without changing the checkout", async () => {
  const dir = mkdtempSync(join(tmpdir(), "wsp-existing-"));
  const repo = join(dir, "project's files");
  const git = (...args: string[]) => execFileSync("git", ["-C", repo, ...args], { encoding: "utf8" }).trim();
  const machine = { exec: async (cmd: string) => ({ exitCode: 0, stdout: execFileSync("bash", ["-c", cmd], { encoding: "utf8" }), stderr: "" }) } as unknown as Machine;
  try {
    mkdirSync(join(repo, "sub"), { recursive: true });
    git("init", "-q", "-b", "main");
    git("-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "--allow-empty", "-qm", "first");
    git("remote", "add", "origin", "https://example.com/project.git");
    git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main");
    git("checkout", "-qb", "feature");
    symlinkSync(repo, join(dir, "alias"));
    const read = await existingFolder(machine, dir, "~/alias/sub");
    expect(read.source.path).toBe(`${git("rev-parse", "--show-toplevel")}/sub`);
    expect(read.top).toBe(git("rev-parse", "--show-toplevel"));
    expect(read.resolved).toMatchObject({ remote: "https://example.com/project.git", defaultBranch: "main", name: "sub" });
    expect(git("symbolic-ref", "--short", "HEAD")).toBe("feature");
    git("checkout", "-q", "--detach");
    expect((await existingFolder(machine, dir, repo)).resolved.defaultBranch).toBe("main");
    mkdirSync(join(dir, "plain"));
    const plain = await existingFolder(machine, dir, join(dir, "plain"));
    expect(plain.top).toBeUndefined();
    expect(plain.resolved.remote).toBe("");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
