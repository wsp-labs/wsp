// SPDX-License-Identifier: AGPL-3.0-only
// The done's result read against real git: a sub-thread's folder made by the
// preamble's own lines from a remote on disk, the host's command run in it, and
// the counts read back, so a clone that leaves no remote-tracking ref behind is
// caught by the number the person sees.
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { subPreamble, subResumePreamble, type SubResult } from "@wsp/protocol";
import { subResultCommand, subResultOf } from "../src/machines/sub-life.js";

const BRANCH = "wsp/review-the-servers-list";
const SIZE = { name: "small", cpu: 2, memMb: 4096, rateUsdPerHour: 0.05 };
const ID = ["-c", "user.name=t", "-c", "user.email=t@t"];

let dirs: string[] = [];
afterEach(() => {
  for (const d of dirs) rmSync(d, { recursive: true, force: true });
  dirs = [];
});

const git = (at: string, ...args: string[]): string => execFileSync("git", ["-C", at, ...ID, ...args], { encoding: "utf8" }).trim();
const sh = (at: string, line: string): string => execFileSync("sh", ["-c", line], { cwd: at, encoding: "utf8" });

/** A remote holding main and a pull request branch five commits deep, and the empty folder a sub-thread opens in. */
function remote(): { url: string; folder: string; head: string; root: string } {
  const root = mkdtempSync(join(tmpdir(), "wsp-sub-read-"));
  dirs.push(root);
  const src = join(root, "src");
  mkdirSync(src);
  git(src, "init", "-q", "-b", "main");
  for (let i = 1; i <= 5; i++) {
    writeFileSync(join(src, "f"), `${i}\n`);
    git(src, "add", "f");
    git(src, "commit", "-qm", `c${i}`);
  }
  git(src, "switch", "-qc", "acme-pr");
  writeFileSync(join(src, "g"), "pr\n");
  git(src, "add", "g");
  git(src, "commit", "-qm", "pr");
  execFileSync("git", ["clone", "-q", "--bare", src, join(root, "remote.git")]);
  const folder = join(root, "web");
  mkdirSync(folder);
  return { url: `file://${join(root, "remote.git")}`, folder, head: git(src, "rev-parse", "HEAD"), root };
}

/** The git lines of a preamble, run in the folder as the agent runs them. */
const runLines = (folder: string, preamble: string): void => {
  for (const line of preamble.split("\n").filter(l => l.startsWith("git "))) sh(folder, line);
};

const read = (folder: string, base: string): SubResult => subResultOf(sh(folder, subResultCommand(folder, BRANCH, base)), BRANCH, "2026-10-14T12:00:00.000Z")!;

describe("a brokered sub-thread's result read", () => {
  it("counts only the commits made on the machine, not the history it started from", () => {
    const r = remote();
    const base = { branch: "acme-pr", sha: r.head };
    runLines(r.folder, subPreamble({ on: "Boat", size: SIZE, lead: "0123456789abcdef", folder: r.folder, sent: [], setupGit: "", repo: r.url, base, branch: BRANCH, deleteAfterMinutes: 30, brokered: true }));
    sh(r.folder, `git fetch -q --deepen=4 origin ${r.head}`);
    expect(Number(sh(r.folder, "git rev-list --count HEAD"))).toBe(5);
    expect(read(r.folder, r.head)).toMatchObject({ unpushed: 0, changed: 0, pushedSha: null });

    writeFileSync(join(r.folder, "h"), "review\n");
    git(r.folder, "add", "h");
    git(r.folder, "commit", "-qm", "review notes");
    expect(read(r.folder, r.head)).toMatchObject({ unpushed: 1, changed: 0, pushedSha: null });

    git(r.folder, "push", "-q", "-u", "origin", BRANCH);
    expect(read(r.folder, r.head)).toMatchObject({ unpushed: 0, changed: 0, pushedSha: git(r.folder, "rev-parse", "HEAD"), commits: 1 });
  }, 20_000);

  it("still answers on a new machine that fetched its pushed branch and not the commit it started from", () => {
    const r = remote();
    const start = git(join(r.root, "src"), "rev-parse", "HEAD~1");
    execFileSync("git", ["-C", join(r.root, "src"), "push", "-q", join(r.root, "remote.git"), `HEAD:refs/heads/${BRANCH}`]);
    runLines(r.folder, subResumePreamble({ on: "Boat", size: SIZE, folder: r.folder, sent: [], setupGit: "", repo: r.url, base: { branch: "main", sha: start }, branch: BRANCH, pushed: true, deleteAfterMinutes: 30, brokered: true }));
    expect(read(r.folder, start)).toMatchObject({ unpushed: 0, changed: 0, pushedSha: r.head });
  }, 20_000);
});
