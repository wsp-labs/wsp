// SPDX-License-Identifier: AGPL-3.0-only
import { randomBytes } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, expect, it } from "vitest";
import type { Machine } from "@wsp/engine";
import { checkoutIgnoredHash, transferCheckout, transferReturnedBranch } from "../src/projects/checkout-transfer.js";

const dirs: string[] = [];
const temp = () => { const p = mkdtempSync(join(tmpdir(), "wsp-transfer-")); dirs.push(p); return p; };
afterEach(() => { for (const p of dirs.splice(0)) rmSync(p, { recursive: true, force: true }); });
const git = (folder: string, ...args: string[]) => execFileSync("git", ["-C", folder, ...args], { encoding: "utf8" }).trim();
function machine(corrupt = false): Machine {
  const run = async (cmd: string) => {
    const r = spawnSync("bash", ["-c", cmd], { encoding: "utf8", maxBuffer: 8 * 1024 * 1024 });
    return { exitCode: r.status ?? 1, stdout: r.stdout, stderr: r.stderr };
  };
  return { id: "test", exec: run, run, putBytes: async (file: string, bytes: Uint8Array) => { mkdirSync(dirname(file), { recursive: true }); writeFileSync(file, corrupt ? Buffer.from("bad") : bytes); } } as unknown as Machine;
}
function repo() {
  const p = temp(); git(p, "init", "-q", "-b", "main");
  git(p, "config", "user.name", "Test"); git(p, "config", "user.email", "test@example.com");
  writeFileSync(join(p, "tracked"), "base\n"); writeFileSync(join(p, ".gitignore"), ".env\nignored\n");
  git(p, "add", "."); git(p, "commit", "-qm", "base"); return p;
}
it("copies staged, unstaged, binary and untracked work from a real worktree without changing its index or branch", async () => {
  const source = repo(); const work = join(temp(), "work"); git(source, "worktree", "add", "-qb", "feature", work);
  writeFileSync(join(work, "tracked"), "staged\n"); git(work, "add", "tracked"); writeFileSync(join(work, "tracked"), "unstaged\n");
  writeFileSync(join(work, "binary"), Buffer.from([0, 1, 255])); git(work, "add", "binary");
  writeFileSync(join(work, "new file"), "new"); writeFileSync(join(work, ".env"), "selected"); writeFileSync(join(work, "ignored"), "stays");
  const before = git(work, "status", "--porcelain=v1"); const target = join(temp(), "copy");
  const made = await transferCheckout(machine(), machine(), { sourceFolder: work, destinationFolder: target, branch: "wsp/child", picked: [".env"] });
  expect(made.git).toBe(true); expect(git(target, "branch", "--show-current")).toBe("wsp/child");
  expect(git(target, "show", ":tracked")).toBe("staged"); expect(readFileSync(join(target, "tracked"), "utf8")).toBe("unstaged\n");
  expect(readFileSync(join(target, "binary"))).toEqual(Buffer.from([0, 1, 255])); expect(readFileSync(join(target, ".env"), "utf8")).toBe("selected");
  expect(() => readFileSync(join(target, "ignored"))).toThrow(); expect(git(work, "status", "--porcelain=v1")).toBe(before);
  expect(git(work, "branch", "--show-current")).toBe("feature"); expect(git(target, "remote")).toBe("");
});
it("returns committed work through a bundle with no remote and preserves the source subfolder", async () => {
  const source = repo(); mkdirSync(join(source, "sub")); const target = join(temp(), "copy");
  const made = await transferCheckout(machine(), machine(), { sourceFolder: join(source, "sub"), destinationFolder: target, branch: "wsp/child" });
  expect(made.folder).toBe(join(target, "sub")); writeFileSync(join(target, "result"), "done");
  git(target, "-c", "user.name=Test", "-c", "user.email=test@example.com", "add", ".");
  git(target, "-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-qm", "child");
  const back = await transferReturnedBranch(machine(), machine(), { sourceFolder: target, destinationFolder: source, branch: made.branch, base: made.head });
  try { git(source, "fetch", "-q", back.from, back.branch); git(source, "merge", "--ff-only", "FETCH_HEAD"); expect(readFileSync(join(source, "result"), "utf8")).toBe("done"); }
  finally { await back.cleanup(); }
});
it("refuses corrupted transport and leaves no destination checkout", async () => {
  const source = repo(); const target = join(temp(), "copy");
  await expect(transferCheckout(machine(), machine(true), { sourceFolder: source, destinationFolder: target, branch: "wsp/child" })).rejects.toThrow(/hash|bytes|transfer/i);
  expect(() => readFileSync(join(target, "tracked"))).toThrow();
});
it("refuses a selected path escaping the project before creating a checkout", async () => {
  const source = repo();
  await expect(transferCheckout(machine(), machine(), { sourceFolder: source, destinationFolder: join(temp(), "copy"), branch: "wsp/child", picked: ["../outside"] })).rejects.toThrow(/path|folder/i);
});

it("transfers several bounded frames and protects an existing destination", async () => {
  const source = repo(); const bytes = randomBytes(700_000); writeFileSync(join(source, "large"), bytes);
  const target = join(temp(), "copy"); const progress: number[] = [];
  await transferCheckout(machine(), machine(), { sourceFolder: source, destinationFolder: target, branch: "wsp/chunks", onProgress: sent => progress.push(sent) });
  expect(readFileSync(join(target, "large"))).toEqual(bytes); expect(progress.length).toBeGreaterThan(3);
  await expect(transferCheckout(machine(), machine(), { sourceFolder: source, destinationFolder: target, branch: "wsp/duplicate" })).rejects.toThrow(/transfer/i);
  expect(readFileSync(join(target, "large"))).toEqual(bytes);
});
it("does not copy nested Git metadata from selected ignored directories", async () => {
  const source = repo(); writeFileSync(join(source, ".gitignore"), "ignored/\n");
  mkdirSync(join(source, "ignored", ".git", "hooks"), { recursive: true });
  writeFileSync(join(source, "ignored", ".git", "hooks", "post-checkout"), "untrusted"); writeFileSync(join(source, "ignored", "keep"), "selected");
  const target = join(temp(), "copy");
  await transferCheckout(machine(), machine(), { sourceFolder: source, destinationFolder: target, branch: "wsp/safe", picked: ["ignored"] });
  expect(readFileSync(join(target, "ignored", "keep"), "utf8")).toBe("selected");
  expect(() => readFileSync(join(target, "ignored", ".git", "hooks", "post-checkout"))).toThrow();
});

it("excludes catalog login metadata even inside a selected directory or a plain folder", async () => {
  const source = repo(); const folder = join(source, "config"); mkdirSync(join(folder, ".aws"), { recursive: true });
  for (const name of [".netrc", ".git-credentials", "auth.json", "hosts.yml", ".aws/credentials"]) writeFileSync(join(folder, name), "private login");
  writeFileSync(join(folder, "allowed"), "project config"); writeFileSync(join(source, ".gitignore"), "config/\n");
  for (const gitSource of [true, false]) {
    if (!gitSource) rmSync(join(source, ".git"), { recursive: true });
    const target = join(temp(), "copy");
    await transferCheckout(machine(), machine(), { sourceFolder: source, destinationFolder: target, branch: "wsp/safe", picked: ["config"] });
    expect(readFileSync(join(target, "config", "allowed"), "utf8")).toBe("project config");
    for (const name of [".netrc", ".git-credentials", "auth.json", "hosts.yml", ".aws/credentials"]) expect(() => readFileSync(join(target, "config", name))).toThrow();
  }
});
it("detects changed and new ignored work while excluding rebuilt output", async () => {
  const source = repo(); writeFileSync(join(source, ".env"), "original");
  const before = await checkoutIgnoredHash(machine(), source);
  writeFileSync(join(source, ".env"), "changed"); expect(await checkoutIgnoredHash(machine(), source)).not.toBe(before);
  writeFileSync(join(source, ".env"), "original");
  writeFileSync(join(source, ".gitignore"), ".env\nignored\nnode_modules/\n");
  const same = await checkoutIgnoredHash(machine(), source);
  mkdirSync(join(source, "node_modules")); writeFileSync(join(source, "node_modules", "generated"), "rebuilt");
  expect(await checkoutIgnoredHash(machine(), source)).toBe(same);
  writeFileSync(join(source, "ignored"), "new work"); expect(await checkoutIgnoredHash(machine(), source)).not.toBe(same);
});

it("copies the current commit and tree without unrelated Git history", async () => {
  const source = repo(); writeFileSync(join(source, "old-large"), randomBytes(2_000_000)); git(source, "add", "."); git(source, "commit", "-qm", "old blob");
  git(source, "rm", "old-large"); git(source, "commit", "-qm", "current tree");
  const target = join(temp(), "copy");
  const made = await transferCheckout(machine(), machine(), { sourceFolder: source, destinationFolder: target, branch: "wsp/compact" });
  expect(made.head).toBe(git(source, "rev-parse", "HEAD")); expect(git(target, "rev-list", "--count", "HEAD")).toBe("1");
  expect(made.bytes).toBeLessThan(50_000);
});
