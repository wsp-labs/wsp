// SPDX-License-Identifier: AGPL-3.0-only
import { execFile, spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInflate } from "node:zlib";

export const WORKFLOWS_REFUSAL = "wsp: this push changes .github/ (workflows run with the repo's secrets); push it from your own computer";
export const UNREAD_PACK_REFUSAL = "wsp: this push's pack could not be read, so its .github/ could not be checked; push it from your own computer";
export const OVERSIZE_REFUSAL = "wsp: this push holds an object over 100 MiB, which GitHub refuses; keep it out of the push";

export type WorkflowVerdict = { allow: true } | { allow: false; said: string };

/** `.github`'s mode and id in the root tree of a commit or tree upstream ("HEAD" is the default branch's head):
 * null when that tree has none, undefined when the upstream does not know the rev. */
export type GithubEntryReader = (rev: string) => Promise<string | null | undefined>;

const OID = /^[0-9a-f]{40}$/;
const ZERO = "0".repeat(40);
const TREE_READ_MS = 10_000;
const INDEX_PACK_MS = 60_000;
/** `git index-pack` allocates each object's declared size, and a delta of a few bytes can double its base's, so a
 * pack of 1.3 KB made it take 495 MB; GitHub refuses a file over 100 MiB, so nothing real passes this. */
const OBJECT_MAX = 100 * 1024 * 1024;

/** A push may land when the `.github` entry of the commit it moves the branch to equals the default branch head's or
 * the thread's base commit's. The pack is indexed alone, which holds because the git road advertises `no-thin`. */
export async function guardWorkflows(o: {
  pack: Uint8Array | AsyncIterable<Uint8Array>;
  head: string;
  base?: string;
  entryAt: GithubEntryReader;
}): Promise<WorkflowVerdict> {
  if (o.head === ZERO) return { allow: true };
  if (!OID.test(o.head) || (o.base !== undefined && !OID.test(o.base))) return { allow: false, said: UNREAD_PACK_REFUSAL };
  const pack = await whole(o.pack);
  const sizes = await packSizes(pack);
  if (sizes !== "fits") return { allow: false, said: sizes === "over" ? OVERSIZE_REFUSAL : UNREAD_PACK_REFUSAL };
  const pushed = await pushedEntry(pack, o.head, o.entryAt);
  if (pushed === undefined) return { allow: false, said: UNREAD_PACK_REFUSAL };
  for (const rev of o.base === undefined ? ["HEAD"] : ["HEAD", o.base]) {
    if ((await o.entryAt(rev)) === pushed) return { allow: true };
  }
  return { allow: false, said: WORKFLOWS_REFUSAL };
}

/** Reads `.github` from the pack where it holds the head's root tree, else from upstream, which already holds what a
 * self-contained pack leaves out. */
async function pushedEntry(pack: Buffer, head: string, entryAt: GithubEntryReader): Promise<string | null | undefined> {
  const dir = await mkdtemp(join(tmpdir(), "wsp-packguard-"));
  try {
    if ((await git(dir, ["init", "--quiet", "--bare", dir])) === undefined) return undefined;
    if (!(await indexPack(dir, pack))) return undefined;
    if ((await git(dir, ["cat-file", "-e", "--end-of-options", head])) === undefined) return entryAt(head);
    const tree = (await git(dir, ["cat-file", "commit", "--end-of-options", head]))?.match(/^tree ([0-9a-f]{40})\n/)?.[1];
    if (tree === undefined) return undefined;
    if ((await git(dir, ["cat-file", "-e", "--end-of-options", tree])) === undefined) return entryAt(tree);
    const line = await git(dir, ["ls-tree", "--full-tree", "--end-of-options", tree, "--", ".github"]);
    if (line === undefined) return undefined;
    if (line === "") return null;
    const [mode, , oid] = line.split(/[ \t]/);
    return `${mode} ${oid}`;
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

/** Neither the person's git config nor the system's applies to a scratch repo that only reads what a machine sent. */
function gitEnv(): NodeJS.ProcessEnv {
  return { PATH: process.env.PATH, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null", GIT_TERMINAL_PROMPT: "0", LC_ALL: "C" };
}

function git(dir: string, args: string[]): Promise<string | undefined> {
  return new Promise(resolve =>
    execFile("git", ["-C", dir, ...args], { env: gitEnv(), timeout: 30_000, maxBuffer: 1 << 20 }, (error, stdout) =>
      resolve(error === null ? stdout.trim() : undefined),
    ),
  );
}

async function indexPack(dir: string, pack: Buffer): Promise<boolean> {
  // index-pack resolves one delta chain per thread, so 8 chains under the cap took 463 MB on 2 threads and 233 MB on 1.
  const child = spawn("git", ["-C", dir, "index-pack", "--stdin", "--fsck-objects", "--threads=1"], { env: gitEnv(), stdio: ["pipe", "ignore", "ignore"], timeout: INDEX_PACK_MS });
  child.stdin.on("error", () => undefined);
  child.stdin.end(pack);
  return new Promise(resolve => {
    child.on("error", () => resolve(false));
    child.on("close", code => resolve(code === 0));
  });
}

async function whole(pack: Uint8Array | AsyncIterable<Uint8Array>): Promise<Buffer> {
  if (pack instanceof Uint8Array) return Buffer.from(pack.buffer, pack.byteOffset, pack.byteLength);
  const chunks: Uint8Array[] = [];
  for await (const chunk of pack) chunks.push(chunk);
  return Buffer.concat(chunks);
}

/** Walks every object's header, and each delta's declared result size, before anything allocates them. */
async function packSizes(pack: Buffer): Promise<"fits" | "over" | "broken"> {
  if (pack.length < 12 || pack.toString("latin1", 0, 4) !== "PACK") return "broken";
  let at = 12;
  for (let left = pack.readUInt32BE(8); left > 0; left--) {
    const header = varint(pack, at, "type");
    if (header === undefined) return "broken";
    const type = (pack[at]! >> 4) & 7;
    if (header.value > OBJECT_MAX) return "over";
    at = header.next;
    if (type === 6) {
      while (at < pack.length && pack[at]! & 0x80) at++;
      at++;
    } else if (type === 7) at += 20;
    else if (type < 1 || type > 4) return "broken";
    const body = await inflated(pack.subarray(at), header.value);
    if (body === undefined) return "broken";
    if (type >= 6) {
      const base = varint(body.head, 0, "plain");
      const result = base && varint(body.head, base.next, "plain");
      if (result === undefined) return "broken";
      if (result.value > OBJECT_MAX) return "over";
    }
    at += body.used;
  }
  return "fits";
}

/** Pack sizes run low bits first; an object header keeps its type in the first byte's bits 4 to 6. Reading stops once
 * a size passes the cap. */
function varint(bytes: Buffer, from: number, kind: "type" | "plain"): { value: number; next: number } | undefined {
  let at = from;
  let value = 0;
  let scale = 1;
  let first = kind === "type";
  while (value <= OBJECT_MAX) {
    const byte = bytes[at++];
    if (byte === undefined) return undefined;
    value += (byte & (first ? 0x0f : 0x7f)) * scale;
    scale *= first ? 16 : 128;
    first = false;
    if ((byte & 0x80) === 0) return { value, next: at };
  }
  return { value, next: at };
}

/** Inflates one object's stream, keeping only its first bytes, and answers how much of the pack it used. */
function inflated(rest: Buffer, size: number): Promise<{ used: number; head: Buffer } | undefined> {
  return new Promise(resolve => {
    const inflate = createInflate();
    let out = 0;
    let head = Buffer.alloc(0);
    inflate.on("data", (chunk: Buffer) => {
      if (head.length < 20) head = Buffer.concat([head, chunk.subarray(0, 20 - head.length)]);
      out += chunk.length;
      if (out > size) inflate.destroy();
    });
    inflate.on("error", () => resolve(undefined));
    inflate.on("close", () => resolve(undefined));
    inflate.on("end", () => resolve(out === size ? { used: inflate.bytesWritten, head } : undefined));
    inflate.end(rest);
  });
}

/** Reads `.github` through GitHub's tree API by repo id, so a renamed repo reads the same. */
export function githubEntryReader(o: { repoId: number; token: string; api?: string }): GithubEntryReader {
  const api = o.api ?? "https://api.github.com";
  return async rev => {
    const res = await fetch(`${api}/repositories/${o.repoId}/git/trees/${encodeURIComponent(rev)}`, {
      headers: { authorization: `Bearer ${o.token}`, accept: "application/vnd.github+json", "user-agent": "wsp" },
      redirect: "error",
      signal: AbortSignal.timeout(TREE_READ_MS),
    });
    if (res.status === 404) return undefined;
    if (!res.ok) throw new Error(`GitHub answered ${res.status} reading a tree`);
    const body: unknown = await res.json();
    const tree = typeof body === "object" && body !== null && "tree" in body && Array.isArray(body.tree) ? body.tree : [];
    for (const entry of tree) {
      if (isTreeEntry(entry) && entry.path === ".github") return `${entry.mode} ${entry.sha}`;
    }
    return null;
  };
}

function isTreeEntry(v: unknown): v is { path: string; mode: string; sha: string } {
  return typeof v === "object" && v !== null && "path" in v && "mode" in v && "sha" in v
    && typeof v.path === "string" && typeof v.mode === "string" && typeof v.sha === "string";
}
