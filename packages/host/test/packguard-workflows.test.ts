// SPDX-License-Identifier: AGPL-3.0-only
// The .github/ guard reads packs a real git push sends once the git road advertises no-thin: a stand-in receive-pack
// adds no-thin to the advertisement and records the request body, and the upstream's trees are read from the bare repo
// the push lands in, where the host reads them from GitHub.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deflateSync } from "node:zlib";
import { afterEach, describe, expect, it, vi } from "vitest";
import { writeStub } from "../../protocol/test/stub-script.js";
import { OLD_GIT_REFUSAL, OVERSIZE_REFUSAL, UNREAD_PACK_REFUSAL, WORKFLOWS_REFUSAL, githubEntryReader, guardWorkflows, type GithubEntryReader } from "../src/packguard/workflows.js";

const dirs: string[] = [];
afterEach(() => {
  vi.unstubAllEnvs();
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

const ENV = {
  PATH: process.env.PATH,
  HOME: tmpdir(),
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_AUTHOR_NAME: "a",
  GIT_AUTHOR_EMAIL: "a@example.invalid",
  GIT_COMMITTER_NAME: "a",
  GIT_COMMITTER_EMAIL: "a@example.invalid",
};

const git = (cwd: string, ...args: string[]): string => execFileSync("git", args, { cwd, env: ENV, encoding: "utf8", stdio: "pipe" }).trim();

const RECEIVE_PACK = `
import { spawn } from "node:child_process";
import { appendFileSync } from "node:fs";
const [log, dir] = process.argv.slice(2);
const rp = spawn("git-receive-pack", [dir], { stdio: ["pipe", "pipe", "inherit"] });
let head = Buffer.alloc(0), first = true;
rp.stdout.on("data", d => {
  if (!first) return process.stdout.write(d);
  head = Buffer.concat([head, d]);
  const n = head.length >= 4 ? parseInt(head.subarray(0, 4).toString(), 16) : Infinity;
  if (head.length < n) return;
  const line = head.subarray(4, n).toString("latin1").replace(/\\n$/, "") + " no-thin\\n";
  first = false;
  process.stdout.write(Buffer.concat([Buffer.from((line.length + 4).toString(16).padStart(4, "0")), Buffer.from(line, "latin1"), head.subarray(n)]));
});
process.stdin.on("data", d => { appendFileSync(log, d); rp.stdin.write(d); });
process.stdin.on("end", () => rp.stdin.end());
rp.on("exit", code => process.exit(code ?? 1));
`;

/** An upstream whose main holds a workflow, and a depth 1 clone of it on the thread's branch. */
function project(extra: Record<string, string> = {}): { root: string; origin: string; work: string; wrapper: string } {
  const root = mkdtempSync(join(tmpdir(), "wsp-packguard-test-"));
  dirs.push(root);
  const origin = join(root, "origin.git");
  const seed = join(root, "seed");
  const work = join(root, "work");
  git(root, "init", "-q", "--bare", "-b", "main", origin);
  git(root, "init", "-q", "-b", "main", seed);
  mkdirSync(join(seed, ".github", "workflows"), { recursive: true });
  writeFileSync(join(seed, ".github", "workflows", "ci.yml"), "on: push\n");
  writeFileSync(join(seed, "big.txt"), Array.from({ length: 20000 }, (_, i) => `${i}\n`).join(""));
  for (const [file, text] of Object.entries(extra)) writeFileSync(join(seed, file), text);
  git(seed, "add", "-A");
  git(seed, "commit", "-qm", "one");
  writeFileSync(join(seed, "README"), "hello\n");
  git(seed, "add", "-A");
  git(seed, "commit", "-qm", "two");
  git(seed, "push", "-q", origin, "HEAD:main");
  git(root, "clone", "-q", "--depth", "1", `file://${origin}`, work);
  git(work, "checkout", "-qb", "wsp/t");
  const wrapper = join(root, "receive-pack.mjs");
  writeFileSync(wrapper, RECEIVE_PACK);
  return { root, origin, work, wrapper };
}

/** Pushes through the stand-in and answers what the git road hands the guard: the new id and the pack. */
function push(p: { root: string; work: string; wrapper: string }, ...refspec: string[]): { head: string; pack: Buffer } {
  const log = join(p.root, `body-${Date.now()}-${Math.random()}`);
  git(p.work, "push", "-q", "-f", `--receive-pack=${JSON.stringify(process.execPath)} ${JSON.stringify(p.wrapper)} ${JSON.stringify(log)}`, "origin", ...refspec);
  const body = readFileSync(log);
  let at = 0;
  let head: string | undefined;
  for (;;) {
    const n = parseInt(body.subarray(at, at + 4).toString(), 16);
    if (n === 0) break;
    const line = body.subarray(at + 4, at + n).toString("latin1");
    if (!line.startsWith("shallow ")) head ??= line.split(" ")[1];
    at += n;
  }
  if (head === undefined) throw new Error("no command in the push");
  return { head, pack: body.subarray(at + 4) };
}

/** The upstream's `.github` entries, as GitHub's tree API answers them. */
const upstream = (origin: string): GithubEntryReader => async rev => {
  try {
    const line = git(origin, "ls-tree", rev, "--", ".github");
    if (line === "") return null;
    const [mode, , oid] = line.split(/[ \t]/);
    return `${mode} ${oid}`;
  } catch {
    return undefined;
  }
};

/** A pack of a 1 MiB blob and a chain of offset deltas, each copying its base twice: a few bytes per delta, and a
 * result twice its base's size. */
function doublingPack(deltas: number): Buffer {
  const MiB = 1 << 20;
  const header = (type: number, size: number): Buffer => {
    const bytes = [(type << 4) | (size & 15)];
    for (size = Math.floor(size / 16); size > 0; size = Math.floor(size / 128)) {
      bytes[bytes.length - 1]! |= 0x80;
      bytes.push(size & 127);
    }
    return Buffer.from(bytes);
  };
  const size = (n: number): number[] => {
    const bytes = [];
    for (; n >= 128; n = Math.floor(n / 128)) bytes.push((n & 127) | 0x80);
    return [...bytes, n];
  };
  const offset = (n: number): Buffer => {
    const bytes = [n & 127];
    for (n = Math.floor(n / 128); n > 0; n = Math.floor(n / 128)) bytes.unshift(0x80 | (--n & 127));
    return Buffer.from(bytes);
  };
  const parts = [Buffer.from("PACK"), Buffer.from([0, 0, 0, 2]), Buffer.alloc(4)];
  parts[2]!.writeUInt32BE(deltas + 1);
  let at = 12;
  let last = at;
  const blob = Buffer.alloc(MiB, "a");
  const first = Buffer.concat([header(3, MiB), deflateSync(blob)]);
  parts.push(first);
  at += first.length;
  for (let i = 0, base = MiB; i < deltas; i++, base *= 2) {
    const copies: number[] = [];
    for (let copied = 0; copied < 2 * base; copied += 8 * MiB > base ? base : 8 * MiB) {
      const from = copied % base;
      const len = Math.min(base, 8 * MiB);
      copies.push(0x80 | (from ? 0x0c : 0) | 0x40, ...(from ? [(from >>> 16) & 255, from >>> 24] : []), (len >>> 16) & 255);
    }
    const data = Buffer.from([...size(base), ...size(2 * base), ...copies]);
    const object = Buffer.concat([header(6, data.length), offset(at - last), deflateSync(data)]);
    last = at;
    parts.push(object);
    at += object.length;
  }
  const body = Buffer.concat(parts);
  return Buffer.concat([body, createHash("sha1").update(body).digest()]);
}

function edit(work: string, file: string, text: string): void {
  writeFileSync(join(work, file), text);
  git(work, "add", "-A");
  git(work, "commit", "-qm", `edit ${file}`);
}

describe("the .github/ guard", () => {
  it("passes a pack that adds a file outside .github/", async () => {
    const p = project();
    edit(p.work, "notes.md", "a change\n");
    const { head, pack } = push(p, "wsp/t");
    expect(pack.subarray(0, 4).toString()).toBe("PACK");
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: true });
  });

  it("passes a pack whose root tree names a .gitattributes and a .gitmodules the pack leaves out, as GitHub already holds them", async () => {
    const p = project({ ".gitattributes": "*.png binary\n", ".gitmodules": '[submodule "x"]\n\tpath = x\n\turl = https://example.invalid/x\n' });
    edit(p.work, "README", "hello from a machine\n");
    const { head, pack } = push(p, "wsp/t");
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: true });
  });

  /** A git 2.43 in front of the real one: usage (129) to fsck levels and its own version, what `also` answers, the rest
   * passed through. */
  const oldGit = (root: string, also = ""): void => {
    const real = execFileSync("sh", ["-c", "command -v git"], { encoding: "utf8" }).trim();
    mkdirSync(join(root, "git-2.43"));
    writeStub(join(root, "git-2.43", "git"), `#!/bin/sh\nfor a in "$@"; do case "$a" in --fsck-objects=*) exit 129;; --version) echo "git version 2.43.0"; exit 0;; ${also}esac; done\nexec ${real} "$@"\n`);
    vi.stubEnv("PATH", `${join(root, "git-2.43")}:${process.env.PATH}`);
  };

  it("passes an ordinary push on a git before 2.44, which answers usage to fsck levels", async () => {
    const p = project();
    edit(p.work, "README", "hello from a machine\n");
    const { head, pack } = push(p, "wsp/t");
    oldGit(p.root);
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: true });
  });

  it("passes a .gitmodules the pack leaves out on a git without fsck levels, as index-pack does", async () => {
    const p = project({ ".gitmodules": '[submodule "x"]\n\tpath = x\n\turl = https://example.com/x\n' });
    edit(p.work, "README", "hello from a machine\n");
    const { head, pack } = push(p, "wsp/t");
    oldGit(p.root);
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: true });
  });

  it("passes an ordinary push on a git before 2.39.1, which dies on the .gitattributes check's name", async () => {
    const p = project();
    edit(p.work, "README", "hello from a machine\n");
    const { head, pack } = push(p, "wsp/t");
    oldGit(p.root, "fsck.gitattributesMissing=ignore) exit 128;; ");
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: true });
  });

  it("names the git when its fsck cannot run on a git without fsck levels", async () => {
    const p = project();
    edit(p.work, "README", "hello from a machine\n");
    const { head, pack } = push(p, "wsp/t");
    oldGit(p.root, "fsck) exit 128;; ");
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: false, said: OLD_GIT_REFUSAL("2.43.0") });
  });

  it("passes a .gitattributes the pack leaves out on a git without fsck levels, which still runs that check", async () => {
    const p = project({ ".gitattributes": "*.png binary\n" });
    edit(p.work, "README", "hello from a machine\n");
    const { head, pack } = push(p, "wsp/t");
    oldGit(p.root);
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: true });
  });

  it("still refuses a root tree that names .github twice on a git without fsck levels", async () => {
    const p = project();
    const main = git(p.work, "rev-parse", "HEAD");
    edit(p.work, ".github/workflows/ci.yml", "on: [push, pull_request]\n");
    const entry = (rev: string): Buffer => Buffer.concat([Buffer.from("40000 .github\0"), Buffer.from(git(p.work, "rev-parse", `${rev}:.github`), "hex")]);
    const tree = execFileSync("git", ["hash-object", "-t", "tree", "--literally", "-w", "--stdin"], { cwd: p.work, env: ENV, input: Buffer.concat([entry(main), entry("HEAD")]) }).toString().trim();
    const head = git(p.work, "commit-tree", "-p", "HEAD", "-m", "two entries", tree);
    const pack = execFileSync("git", ["pack-objects", "--revs", "--stdout", "-q"], { cwd: p.work, env: ENV, input: `${head}\n^${main}\n` });
    oldGit(p.root);
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: false, said: UNREAD_PACK_REFUSAL });
  });

  it("refuses a root tree entry named .git on a git without fsck levels, as index-pack's strict fsck does", async () => {
    const p = project();
    const main = git(p.work, "rev-parse", "HEAD");
    const entry = (name: string): Buffer => Buffer.concat([Buffer.from(`40000 ${name}\0`), Buffer.from(git(p.work, "rev-parse", "HEAD:.github"), "hex")]);
    const tree = execFileSync("git", ["hash-object", "-t", "tree", "--literally", "-w", "--stdin"], { cwd: p.work, env: ENV, input: Buffer.concat([entry(".git"), entry(".github")]) }).toString().trim();
    const head = git(p.work, "commit-tree", "-p", "HEAD", "-m", "a .git entry", tree);
    const pack = execFileSync("git", ["pack-objects", "--revs", "--stdout", "-q"], { cwd: p.work, env: ENV, input: `${head}\n^${main}\n` });
    oldGit(p.root);
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: false, said: UNREAD_PACK_REFUSAL });
  });

  it("refuses a pack that edits .github/workflows/ci.yml, and one that adds a workflow", async () => {
    const p = project();
    edit(p.work, ".github/workflows/ci.yml", "on: [push, pull_request]\n");
    const edited = push(p, "wsp/t");
    expect(await guardWorkflows({ ...edited, entryAt: upstream(p.origin) })).toEqual({ allow: false, said: WORKFLOWS_REFUSAL });

    const q = project();
    edit(q.work, ".github/workflows/release.yml", "on: push\n");
    const added = push(q, "wsp/t");
    expect(await guardWorkflows({ ...added, entryAt: upstream(q.origin) })).toEqual({ allow: false, said: WORKFLOWS_REFUSAL });
  });

  it("passes a pack on an older base whose .github matches the base after main changed a workflow", async () => {
    const p = project();
    const base = git(p.work, "rev-parse", "HEAD");
    const other = join(p.root, "other");
    git(p.root, "clone", "-q", `file://${p.origin}`, other);
    edit(other, ".github/workflows/ci.yml", "on: [push]\n");
    git(other, "push", "-q", "origin", "main");
    edit(p.work, "notes.md", "work on the old base\n");
    const pushed = push(p, "wsp/t");
    const entryAt = upstream(p.origin);
    expect(await entryAt("HEAD")).not.toBe(await entryAt(base));
    expect(await guardWorkflows({ ...pushed, base, entryAt })).toEqual({ allow: true });
    expect(await guardWorkflows({ ...pushed, entryAt })).toEqual({ allow: false, said: WORKFLOWS_REFUSAL });
  });

  it("reads upstream for a commit the pack leaves out", async () => {
    const p = project();
    const pushed = push(p, "origin/main:refs/heads/wsp/u");
    expect(pushed.pack.readUInt32BE(8)).toBe(0);
    expect(await guardWorkflows({ ...pushed, entryAt: upstream(p.origin) })).toEqual({ allow: true });
    expect(await guardWorkflows({ ...pushed, entryAt: async rev => (rev === "HEAD" ? "040000 0000000000000000000000000000000000000001" : upstream(p.origin)(rev)) })).toEqual({ allow: false, said: WORKFLOWS_REFUSAL });
  });

  it("reads upstream for a root tree the pack leaves out", async () => {
    const p = project();
    git(p.work, "commit", "-q", "--allow-empty", "-m", "nothing changed");
    const pushed = push(p, "wsp/t");
    expect(pushed.pack.readUInt32BE(8)).toBe(1);
    expect(await guardWorkflows({ ...pushed, entryAt: upstream(p.origin) })).toEqual({ allow: true });
  });

  it("passes a message-only amend", async () => {
    const p = project();
    edit(p.work, "notes.md", "a change\n");
    push(p, "wsp/t");
    git(p.work, "commit", "-q", "--amend", "-m", "a better message");
    const pushed = push(p, "wsp/t");
    expect(await guardWorkflows({ ...pushed, entryAt: upstream(p.origin) })).toEqual({ allow: true });
  });

  it("refuses a 1.3 KB pack whose deltas declare a 256 MiB object, before git allocates it", async () => {
    const pack = doublingPack(8);
    expect(pack.length).toBeLessThan(1400);
    expect(await guardWorkflows({ pack, head: "1".repeat(40), entryAt: async () => null })).toEqual({ allow: false, said: OVERSIZE_REFUSAL });
  });

  it("passes a pack of deltas whose objects stay under the cap", async () => {
    const pack = doublingPack(2);
    expect(await guardWorkflows({ pack, head: "1".repeat(40), entryAt: async () => "040000 2007d6a0c6dfceeba4d005d67b2b28d6edf9d694" })).toEqual({ allow: true });
  });

  it("refuses a thin pack it cannot index alone, and a head it cannot find", async () => {
    const p = project();
    const main = git(p.work, "rev-parse", "HEAD");
    edit(p.work, "big.txt", `${readFileSync(join(p.work, "big.txt"), "utf8")}one more\n`);
    const head = git(p.work, "rev-parse", "HEAD");
    const thin = execFileSync("git", ["pack-objects", "--revs", "--thin", "--stdout", "-q"], { cwd: p.work, env: ENV, input: `${head}\n^${main}\n` });
    expect(await guardWorkflows({ pack: thin, head, entryAt: upstream(p.origin) })).toEqual({ allow: false, said: UNREAD_PACK_REFUSAL });
    const empty = push(p, "origin/main:refs/heads/wsp/u").pack;
    expect(await guardWorkflows({ pack: empty, head, entryAt: upstream(p.origin) })).toEqual({ allow: false, said: UNREAD_PACK_REFUSAL });
  });

  it("refuses a root tree that names .github twice, the first entry matching main", async () => {
    const p = project();
    const main = git(p.work, "rev-parse", "HEAD");
    edit(p.work, ".github/workflows/ci.yml", "on: [push, pull_request]\n");
    const entry = (rev: string): Buffer => Buffer.concat([Buffer.from("40000 .github\0"), Buffer.from(git(p.work, "rev-parse", `${rev}:.github`), "hex")]);
    const tree = execFileSync("git", ["hash-object", "-t", "tree", "--literally", "-w", "--stdin"], { cwd: p.work, env: ENV, input: Buffer.concat([entry(main), entry("HEAD")]) }).toString().trim();
    const head = git(p.work, "commit-tree", "-p", "HEAD", "-m", "two entries", tree);
    expect(git(p.work, "ls-tree", head, "--", ".github").split("\n")[0]).toBe(git(p.work, "ls-tree", main, "--", ".github"));
    const pack = execFileSync("git", ["pack-objects", "--revs", "--stdout", "-q"], { cwd: p.work, env: ENV, input: `${head}\n^${main}\n` });
    expect(await guardWorkflows({ pack, head, entryAt: upstream(p.origin) })).toEqual({ allow: false, said: UNREAD_PACK_REFUSAL });
  });

  it("passes a delete, which moves the branch to no commit", async () => {
    expect(await guardWorkflows({ pack: new Uint8Array(), head: "0".repeat(40), entryAt: async () => undefined })).toEqual({ allow: true });
  });
});

describe("reading .github upstream", () => {
  it("asks GitHub's tree API by repo id with the person's token and reads the root entry", async () => {
    const asked: { url?: string; auth?: string }[] = [];
    const server = createServer((req, res) => {
      asked.push({ url: req.url, auth: req.headers.authorization });
      if (req.url?.endsWith("/missing")) return res.writeHead(404).end('{"message":"Not Found","status":"404"}');
      const tree = req.url?.endsWith("/bare") ? [] : [{ path: ".github", mode: "040000", type: "tree", sha: "00fbbb8666d04a3a1cf56d00fae38e2bbced90e1" }];
      res.writeHead(200, { "content-type": "application/json" }).end(JSON.stringify({ sha: "285b8f48cc8c2874eb194c8e8dd3780d21255f39", tree: [{ path: ".agents", mode: "040000", type: "tree", sha: "1".repeat(40) }, ...tree], truncated: false }));
    });
    await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
    try {
      const read = githubEntryReader({ repoId: 42, token: "gho_test", api: `http://127.0.0.1:${(server.address() as AddressInfo).port}` });
      expect(await read("HEAD")).toBe("040000 00fbbb8666d04a3a1cf56d00fae38e2bbced90e1");
      expect(await read("bare")).toBeNull();
      expect(await read("missing")).toBeUndefined();
      expect(asked[0]).toEqual({ url: "/repositories/42/git/trees/HEAD", auth: "Bearer gho_test" });
    } finally {
      await new Promise(resolve => server.close(resolve));
    }
  });
});
