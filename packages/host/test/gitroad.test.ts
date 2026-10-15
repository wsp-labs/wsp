// SPDX-License-Identifier: AGPL-3.0-only
// The git road against real git: a client configured as a cloud machine's turn is (github.com rewritten to plain http,
// the door as its proxy, the turn token as the proxy password), a stand-in broker on loopback, and `git http-backend`
// standing in for github.com behind a fake REST API that knows each repo's id and a renamed repo's old name.
import { execFile, spawn } from "node:child_process";
import { createReadStream, existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createServer, request, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createGzip } from "node:zlib";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { BranchOfRecord } from "@wsp/protocol";
import { gitRoad, spool, type GitContext, type GitLogLine, type GitRequest, type GuardedPush } from "../src/gitroad/proxy.js";

const TOKEN = "gho_wspTestPlaceholder0000000000000000";
const TURN = "wsp-turn-test";
const APP_ID = 101;

interface Ran {
  code: number;
  out: string;
}

let root: string;
let home: string;
let upstreamPort = 0;
let doorPort = 0;
let servers: Server[] = [];
let record: BranchOfRecord | undefined;
let credential: { token: string } | undefined;
let guardSays: string | undefined;
let gzipPushes = false;
let apiMode: "ok" | "down" | "401" | "403" | "404" | "502" = "ok";
const guarded: { push: GuardedPush; alone: boolean }[] = [];
const logged: GitLogLine[] = [];
const upstreamSeen: { method: string; path: string; headers: IncomingMessage["headers"]; body: Buffer }[] = [];

function run(cmd: string, args: string[], cwd: string, env: NodeJS.ProcessEnv): Promise<Ran> {
  return new Promise((resolve) => {
    execFile(cmd, args, { cwd, env, maxBuffer: 64 * 1024 * 1024 }, (err, stdout, stderr) => {
      const code = err ? (typeof (err as { code?: unknown }).code === "number" ? (err as { code: number }).code : 1) : 0;
      resolve({ code, out: `${stdout}${stderr}` });
    });
  });
}

function clientEnv(): NodeJS.ProcessEnv {
  const pairs: [string, string][] = [
    ["url.http://github.com/.insteadOf", "https://github.com/"],
    ["url.http://github.com/.insteadOf", "git@github.com:"],
    ["url.http://github.com/.insteadOf", "ssh://git@github.com/"],
    ["url.http://github.com/.insteadOf", "ssh://git@ssh.github.com:443/"],
    ["http.http://github.com/.proxy", `http://wsp:${TURN}@127.0.0.1:${doorPort}`],
    ["http.proxyAuthMethod", "basic"],
    ["credential.helper", ""],
  ];
  const env: NodeJS.ProcessEnv = {
    PATH: process.env.PATH,
    HOME: home,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_TERMINAL_PROMPT: "0",
    GIT_AUTHOR_NAME: "t",
    GIT_AUTHOR_EMAIL: "t@example.invalid",
    GIT_COMMITTER_NAME: "t",
    GIT_COMMITTER_EMAIL: "t@example.invalid",
    GIT_CONFIG_COUNT: String(pairs.length),
  };
  pairs.forEach(([key, value], i) => {
    env[`GIT_CONFIG_KEY_${i}`] = key;
    env[`GIT_CONFIG_VALUE_${i}`] = value;
  });
  return env;
}

const git = (args: string[], cwd = root) => run("git", args, cwd, clientEnv());
const bare = (repo: string, args: string[]) => run("git", ["--git-dir", join(root, "github", repo), ...args], root, { PATH: process.env.PATH, HOME: home });
const upstreamRef = async (ref: string, repo = "acme/app.git") => (await bare(repo, ["rev-parse", "--verify", "-q", ref])).out.trim();

async function readBody(req: IncomingMessage): Promise<Buffer> {
  const chunks: Buffer[] = [];
  for await (const chunk of req as AsyncIterable<Buffer>) chunks.push(chunk);
  return Buffer.concat(chunks);
}

/** github.com's smart HTTP: `git http-backend` over the bare repos, which takes only the person's token. A renamed repo answers on its old name too. */
async function github(req: IncomingMessage, res: ServerResponse): Promise<void> {
  const body = await readBody(req);
  const url = new URL(req.url ?? "/", "http://github.com");
  upstreamSeen.push({ method: req.method ?? "", path: url.pathname, headers: req.headers, body });
  const basic = `Basic ${Buffer.from(`x-access-token:${TOKEN}`).toString("base64")}`;
  if (req.headers.authorization !== basic) {
    res.writeHead(401, { "www-authenticate": 'Basic realm="GitHub"' });
    return void res.end();
  }
  const env: NodeJS.ProcessEnv = {
    PATH: process.env.PATH,
    HOME: home,
    GIT_PROJECT_ROOT: join(root, "github"),
    GIT_HTTP_EXPORT_ALL: "1",
    PATH_INFO: url.pathname.replace(/^\/zed\/app\.git\//, "/acme/app.git/"),
    QUERY_STRING: url.search.slice(1),
    REQUEST_METHOD: req.method,
    CONTENT_TYPE: req.headers["content-type"] ?? "",
    CONTENT_LENGTH: String(body.length),
  };
  if (req.headers["content-encoding"]) env.HTTP_CONTENT_ENCODING = req.headers["content-encoding"];
  if (req.headers["git-protocol"]) env.GIT_PROTOCOL = req.headers["git-protocol"] as string;
  const cgi = spawn("git", ["http-backend"], { env });
  // http-backend answers a GET without reading stdin, so it can exit before the body is written.
  cgi.stdin.on("error", () => undefined);
  cgi.stdin.end(body);
  const out = await readBody(cgi.stdout as unknown as IncomingMessage);
  const split = out.indexOf("\r\n\r\n");
  const headers: Record<string, string> = {};
  let status = 200;
  for (const line of out.subarray(0, split).toString().split("\r\n")) {
    const [name = "", ...rest] = line.split(": ");
    if (name.toLowerCase() === "status") status = Number.parseInt(rest.join(": "), 10);
    else if (name) headers[name] = rest.join(": ");
  }
  res.writeHead(status, headers);
  res.end(out.subarray(split + 4));
}

/** GitHub's REST API, as much as the road asks: a repo's id by name, the old name answering a redirect. */
function restApi(req: IncomingMessage, res: ServerResponse): void {
  if (apiMode === "down") return void req.socket.destroy();
  if (apiMode !== "ok") {
    res.writeHead(Number(apiMode));
    return void res.end();
  }
  if (req.headers.authorization !== `Bearer ${TOKEN}`) {
    res.writeHead(401);
    return void res.end();
  }
  const ids: Record<string, number> = { "/repos/acme/app": APP_ID, "/repos/acme/other": 202, [`/repositories/${APP_ID}`]: APP_ID };
  if (req.url === "/repos/zed/app") {
    res.writeHead(301, { location: `/repositories/${APP_ID}` });
    return void res.end();
  }
  const id = ids[req.url ?? ""];
  res.writeHead(id ? 200 : 404, { "content-type": "application/json" });
  res.end(JSON.stringify(id ? { id, full_name: "acme/app" } : { message: "Not Found" }));
}

/** Indexes the pack in an empty repo with no `--fix-thin`, which succeeds only for a self-contained pack. */
async function indexesAlone(pack: string): Promise<boolean> {
  const scratch = mkdtempSync(join(root, "guard-"));
  await run("git", ["init", "-q", "--bare", scratch], root, { PATH: process.env.PATH, HOME: home });
  const code = await new Promise<number>((resolve) => {
    const child = spawn("git", ["--git-dir", scratch, "index-pack", "--stdin"], { stdio: ["pipe", "ignore", "ignore"] });
    createReadStream(pack).pipe(child.stdin);
    child.on("close", (c) => resolve(c ?? 1));
  });
  rmSync(scratch, { recursive: true, force: true });
  return code === 0;
}

const road = (packCap?: number, scratch = root) =>
  gitRoad({
    packCap,
    origin: `http://127.0.0.1:${upstreamPort}`,
    api: `http://127.0.0.1:${apiPort}`,
    scratch,
    guard: async (push) => {
      guarded.push({ push, alone: await indexesAlone(push.pack) });
      if (guardSays === "throw") throw new Error("the guard could not read the pack");
      return guardSays;
    },
  });
let apiPort = 0;

const context: GitContext = {
  credential: async () => credential,
  workspace: async () => ({ branchOfRecord: record }),
  refuse(res, status, said) {
    res.writeHead(status, { "content-type": "text/plain; charset=utf-8", "x-should-retry": "false" });
    res.end(`${said}\n`);
  },
  log: (line) => logged.push(line),
};

/** The broker's part, standing in: the turn token as the proxy password, then the first service that claims the request. */
function door(service: ReturnType<typeof gitRoad>) {
  return (req: IncomingMessage, res: ServerResponse) => {
    const auth = req.headers["proxy-authorization"] ?? "";
    if (auth !== `Basic ${Buffer.from(`wsp:${TURN}`).toString("base64")}` || !service.claims(req, "http")) {
      res.writeHead(403, { "content-type": "text/plain" });
      return void res.end("wsp: refused at the door\n");
    }
    let body: IncomingMessage = req;
    if (gzipPushes && req.method === "POST" && req.url?.endsWith("/git-receive-pack")) {
      body = Object.assign(req.pipe(createGzip()), { headers: { ...req.headers, "content-encoding": "gzip" }, method: req.method, url: req.url }) as unknown as IncomingMessage;
    }
    const r: GitRequest = { workspaceId: "ws1", door: "http", grant: { kind: "turn", workspaceId: "ws1", threadId: "th1" }, req: body, res, heldMs: 0 };
    service.serve(r, context).catch(() => res.destroy());
  };
}

function listen(handler: (req: IncomingMessage, res: ServerResponse) => void): Promise<number> {
  const server = createServer((req, res) => void handler(req, res));
  servers.push(server);
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve((server.address() as { port: number }).port)));
}

function bigText(seed: number): string {
  return Array.from({ length: 6000 }, (_, i) => `line ${i} of the big file, seeded ${seed * 7919 + i}`).join("\n") + "\n";
}

beforeAll(async () => {
  root = mkdtempSync(join(tmpdir(), "wsp-gitroad-"));
  home = join(root, "home");
  mkdirSync(home);
  upstreamPort = await listen((req, res) => void github(req, res));
  apiPort = await listen(restApi);
  doorPort = await listen(door(road()));

  const seed = join(root, "seed");
  const env = { ...clientEnv(), GIT_CONFIG_COUNT: "0" };
  await run("git", ["init", "-q", "-b", "main", seed], root, env);
  mkdirSync(join(seed, ".github", "workflows"), { recursive: true });
  for (let i = 0; i < 3; i++) {
    writeFileSync(join(seed, "README.md"), `app, version ${i}\n`);
    writeFileSync(join(seed, "big.txt"), bigText(i));
    writeFileSync(join(seed, ".github", "workflows", "ci.yml"), `on: push\n# ${i}\n`);
    await run("git", ["add", "-A"], seed, env);
    await run("git", ["commit", "-q", "-m", `commit ${i}`], seed, env);
  }
  for (const repo of ["acme/app.git", "acme/other.git"]) {
    await run("git", ["clone", "-q", "--bare", seed, join(root, "github", repo)], root, env);
    for (const [key, value] of [["http.receivepack", "true"], ["uploadpack.allowFilter", "true"], ["receive.advertisePushOptions", "true"], ["receive.certNonceSeed", "seed"], ["uploadpack.allowAnySHA1InWant", "true"]]) {
      await bare(repo, ["config", key as string, value as string]);
    }
  }
  startSha = await upstreamRef("refs/heads/main");
}, 60_000);

afterAll(async () => {
  await Promise.all(servers.map((s) => new Promise((resolve) => s.close(resolve))));
  servers = [];
  rmSync(root, { recursive: true, force: true });
});

const CLONES = {
  "depth 1": ["--depth", "1", "--branch", "main"],
  blobless: ["--filter=blob:none"],
  full: [],
} as const;

/** A clone as a brokered thread starts: the clone through the door, one fetch refspec line for the branch of record, the branch checked out. */
async function cloneAs(kind: keyof typeof CLONES, branch: string): Promise<string> {
  const dest = mkdtempSync(join(root, "clone-"));
  const cloned = await git(["clone", "-q", ...CLONES[kind], "https://github.com/acme/app", dest]);
  expect(cloned.code, cloned.out).toBe(0);
  await git(["config", "--add", "remote.origin.fetch", `+refs/heads/${branch}:refs/remotes/origin/${branch}`], dest);
  await git(["checkout", "-q", "-b", branch], dest);
  return dest;
}

async function commit(dir: string, file: string, text: string): Promise<string> {
  writeFileSync(join(dir, file), text);
  await git(["add", "-A"], dir);
  await git(["commit", "-q", "-m", `edit ${file}`], dir);
  return (await git(["rev-parse", "HEAD"], dir)).out.trim();
}

let startSha = "";

function resetState(branch: string): void {
  record = { repoId: APP_ID, owner: "acme", name: "app", branch, startSha };
  credential = { token: TOKEN };
  guardSays = undefined;
  gzipPushes = false;
  apiMode = "ok";
}

/** A door of its own, so the road's repo id cache starts empty; the suite's door comes back afterwards. */
async function withOwnDoor(packCap: number | undefined, body: () => Promise<void>, scratch?: string): Promise<void> {
  const shared = doorPort;
  doorPort = await listen(door(road(packCap, scratch)));
  try {
    await body();
  } finally {
    doorPort = shared;
  }
}

describe("the git road's push rule, on real packs", () => {
  for (const kind of Object.keys(CLONES) as (keyof typeof CLONES)[]) {
    for (const atomic of [false, true]) {
      it(`${kind} clone, ${atomic ? "--atomic" : "plain"}: 03's verdicts, the allowed half of a mixed push lands`, async () => {
        const branch = `wsp/${kind.replace(" ", "")}-${atomic ? "atomic" : "plain"}`;
        resetState(branch);
        const flag = atomic ? ["--atomic"] : [];
        const push = (dir: string, ...args: string[]) => git(["push", ...flag, ...args], dir);
        const main = await upstreamRef("refs/heads/main");
        const dir = await cloneAs(kind, branch);
        const guardedBefore = guarded.length;

        let head = await commit(dir, "notes.txt", "first\n");
        let r = await push(dir, "-u", "origin", branch);
        expect(r.code, r.out).toBe(0);
        expect(r.out).toContain("[new branch]");
        expect(await upstreamRef(`refs/heads/${branch}`)).toBe(head);

        head = await commit(dir, "big.txt", bigText(2).replace("line 10 of", "line ten of"));
        r = await push(dir, "origin", branch);
        expect(r.code, r.out).toBe(0);
        expect(await upstreamRef(`refs/heads/${branch}`)).toBe(head);

        await git(["commit", "-q", "--amend", "-m", "rewritten"], dir);
        head = (await git(["rev-parse", "HEAD"], dir)).out.trim();
        r = await push(dir, "--force-with-lease", "origin", branch);
        expect(r.code, r.out).toBe(0);
        expect(r.out).toContain("(forced update)");
        expect(await upstreamRef(`refs/heads/${branch}`)).toBe(head);

        r = await push(dir, "origin", "HEAD:main");
        expect(r.code).not.toBe(0);
        expect(r.out).toContain(`remote: wsp: main is not this thread's branch; push your work to ${branch}`);
        expect(r.out).toContain(`! [remote rejected] HEAD -> main (wsp: main is not this thread's branch; push your work to ${branch})`);

        r = await push(dir, "origin", ":main");
        expect(r.out).toContain(`! [remote rejected] main (wsp: main is not this thread's branch; push your work to ${branch})`);

        await git(["tag", "v9"], dir);
        r = await push(dir, "origin", "v9");
        expect(r.out).toContain("! [remote rejected] v9 -> v9 (wsp: tags are made from your own computer, not from a cloud thread)");

        await git(["notes", "add", "-m", "a note"], dir);
        r = await push(dir, "origin", "refs/notes/commits");
        expect(r.out).toContain(`(wsp: refs/notes/commits is not a branch; a cloud thread pushes only ${branch})`);

        const before = await upstreamRef(`refs/heads/${branch}`);
        head = await commit(dir, "notes.txt", "mixed\n");
        r = await push(dir, "origin", `HEAD:${branch}`, "HEAD:refs/heads/other");
        expect(r.code).not.toBe(0);
        expect(r.out).toContain(`! [remote rejected] HEAD -> other (wsp: other is not this thread's branch; push your work to ${branch})`);
        expect(r.out).toContain(`remote: wsp: other is not this thread's branch; push your work to ${branch}`);
        if (atomic) {
          expect(r.out).toContain(`! [remote rejected] HEAD -> ${branch} (wsp: refused with the rest of this atomic push)`);
          expect(await upstreamRef(`refs/heads/${branch}`)).toBe(before);
        } else {
          expect(r.out).toMatch(new RegExp(`${before.slice(0, 7)}\\.\\.${head.slice(0, 7)}\\s+HEAD -> ${branch}`));
          expect(await upstreamRef(`refs/heads/${branch}`)).toBe(head);
        }

        r = await push(dir, "origin", `:${branch}`);
        expect(r.code, r.out).toBe(0);
        expect(r.out).toContain("[deleted]");
        expect(await upstreamRef(`refs/heads/${branch}`)).toBe("");

        expect(await upstreamRef("refs/heads/main")).toBe(main);
        expect(await upstreamRef("refs/heads/other")).toBe("");
        expect(await upstreamRef("refs/tags/v9")).toBe("");
        expect(await upstreamRef("refs/notes/commits")).toBe("");
        const packs = guarded.slice(guardedBefore);
        expect(packs.length).toBeGreaterThanOrEqual(3);
        expect(packs.every((g) => g.alone)).toBe(true);
        expect(packs.every((g) => g.push.commands.every((c) => c.ref === `refs/heads/${branch}`))).toBe(true);
        expect(packs.every((g) => g.push.base === startSha)).toBe(true);
      }, 120_000);
    }
  }

  it("a depth 1 push reaches GitHub with its shallow lines ahead of the commands", async () => {
    resetState("wsp/shallow");
    const dir = await cloneAs("depth 1", "wsp/shallow");
    await commit(dir, "notes.txt", "shallow\n");
    const seen = upstreamSeen.length;
    const r = await git(["push", "origin", "wsp/shallow", "HEAD:main"], dir);
    expect(r.out).toContain("[new branch]");
    const posted = upstreamSeen.slice(seen).find((s) => s.method === "POST" && s.path.endsWith("/git-receive-pack"));
    const lines = posted?.body.toString("latin1") ?? "";
    expect(lines.slice(4, 12)).toBe("shallow ");
    expect(lines).toContain(" refs/heads/wsp/shallow\0");
    expect(lines).not.toContain("refs/heads/main");
  }, 60_000);

  it("--follow-tags lands the branch and refuses only the tag", async () => {
    resetState("wsp/tags");
    const dir = await cloneAs("depth 1", "wsp/tags");
    const head = await commit(dir, "notes.txt", "tagged\n");
    await git(["tag", "-a", "-m", "release", "v10"], dir);
    const r = await git(["push", "--follow-tags", "origin", "wsp/tags"], dir);
    expect(r.out).toContain("[new branch]");
    expect(r.out).toContain("! [remote rejected] v10 -> v10 (wsp: tags are made from your own computer, not from a cloud thread)");
    expect(await upstreamRef("refs/heads/wsp/tags")).toBe(head);
    expect(await upstreamRef("refs/tags/v10")).toBe("");
  }, 60_000);

  it("a push option passes through with the upstream's capabilities", async () => {
    resetState("wsp/options");
    const dir = await cloneAs("depth 1", "wsp/options");
    const head = await commit(dir, "notes.txt", "options\n");
    const r = await git(["push", "-o", "ci.skip", "origin", "wsp/options"], dir);
    expect(r.code, r.out).toBe(0);
    expect(await upstreamRef("refs/heads/wsp/options")).toBe(head);
  }, 60_000);

  it("a push over git's post buffer sends its 0000 probe untouched, then lands", async () => {
    resetState("wsp/large");
    const dir = await cloneAs("depth 1", "wsp/large");
    const random = Buffer.alloc(3 * 1024 * 1024);
    for (let i = 0; i < random.length; i += 4) random.writeUInt32LE((Math.random() * 0xffffffff) >>> 0, i);
    writeFileSync(join(dir, "blob.bin"), random);
    await git(["add", "-A"], dir);
    await git(["commit", "-q", "-m", "large"], dir);
    const head = (await git(["rev-parse", "HEAD"], dir)).out.trim();
    const seen = upstreamSeen.length;
    const r = await git(["push", "origin", "wsp/large"], dir);
    expect(r.code, r.out).toBe(0);
    const posts = upstreamSeen.slice(seen).filter((s) => s.method === "POST");
    expect(posts.some((p) => p.body.toString() === "0000")).toBe(true);
    expect(await upstreamRef("refs/heads/wsp/large")).toBe(head);
  }, 60_000);

  it("a gzip push body is read like a plain one", async () => {
    resetState("wsp/gzip");
    const dir = await cloneAs("depth 1", "wsp/gzip");
    const head = await commit(dir, "notes.txt", "gzip\n");
    gzipPushes = true;
    const r = await git(["push", "origin", "wsp/gzip", "HEAD:main"], dir);
    gzipPushes = false;
    expect(r.out).toContain("[new branch]");
    expect(r.out).toContain("! [remote rejected] HEAD -> main (wsp: main is not this thread's branch; push your work to wsp/gzip)");
    expect(await upstreamRef("refs/heads/wsp/gzip")).toBe(head);
  }, 60_000);

  it("a push the .github/ guard refuses prints its sentence and lands nothing", async () => {
    resetState("wsp/guarded");
    const dir = await cloneAs("depth 1", "wsp/guarded");
    await commit(dir, ".github/workflows/ci.yml", "on: push\njobs: {}\n");
    guardSays = "wsp: this push changes .github/ (workflows run with the repo's secrets); push it from your own computer";
    const r = await git(["push", "origin", "wsp/guarded"], dir);
    expect(r.code).not.toBe(0);
    expect(r.out).toContain(`! [remote rejected] wsp/guarded -> wsp/guarded (${guardSays})`);
    expect(r.out).toContain(`remote: ${guardSays}`);
    expect(await upstreamRef("refs/heads/wsp/guarded")).toBe("");
  }, 60_000);

  it("hands the guard no base when the record holds no start commit", async () => {
    resetState("wsp/nobase");
    record = { repoId: APP_ID, owner: "acme", name: "app", branch: "wsp/nobase" };
    const dir = await cloneAs("depth 1", "wsp/nobase");
    await commit(dir, "notes.txt", "no base\n");
    const from = guarded.length;
    const r = await git(["push", "origin", "wsp/nobase"], dir);
    expect(r.code, r.out).toBe(0);
    expect(guarded.slice(from).map((g) => g.push.base)).toEqual([undefined]);
  }, 60_000);

  it("a guard that fails refuses the push with a sentence, never lets it through", async () => {
    resetState("wsp/guardfails");
    const dir = await cloneAs("depth 1", "wsp/guardfails");
    await commit(dir, "notes.txt", "guard fails\n");
    guardSays = "throw";
    const r = await git(["push", "origin", "wsp/guardfails"], dir);
    expect(r.out).toContain("! [remote rejected] wsp/guardfails -> wsp/guardfails (wsp: your computer could not check this push; push again)");
    expect(await upstreamRef("refs/heads/wsp/guardfails")).toBe("");
  }, 60_000);

  it("logs each command's ref, shas and verdict, never the credential", async () => {
    resetState("wsp/logged");
    const dir = await cloneAs("depth 1", "wsp/logged");
    const head = await commit(dir, "notes.txt", "logged\n");
    const from = logged.length;
    await git(["push", "origin", "wsp/logged", "HEAD:main"], dir);
    const line = logged.slice(from).find((l) => l.pushes.some((ref) => ref.ref === "refs/heads/wsp/logged"));
    expect(Object.keys(line ?? {})).toEqual(["pushes"]);
    expect(line?.pushes).toHaveLength(2);
    expect(line?.pushes).toEqual(expect.arrayContaining([
      { ref: "refs/heads/wsp/logged", old: "0".repeat(40), new: head, verdict: "allowed" },
      { ref: "refs/heads/main", old: await upstreamRef("refs/heads/main"), new: head, verdict: "refused" },
    ]));
    expect(JSON.stringify(logged)).not.toContain(TOKEN);
    expect(JSON.stringify(logged)).not.toContain(TURN);
  }, 60_000);
});

describe("which repo a push may reach", () => {
  it("a renamed repo's old name lands when GitHub's id for it matches the record", async () => {
    resetState("wsp/renamed");
    const dir = await cloneAs("depth 1", "wsp/renamed");
    await git(["remote", "set-url", "origin", "https://github.com/zed/app"], dir);
    const head = await commit(dir, "notes.txt", "renamed\n");
    const r = await git(["push", "origin", "wsp/renamed"], dir);
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain("[new branch]");
    expect(await upstreamRef("refs/heads/wsp/renamed")).toBe(head);
  }, 60_000);

  it("the record's own name is refused when GitHub's id for it is another repo's", async () => {
    resetState("wsp/impostor");
    record = { repoId: 555, owner: "acme", name: "app", branch: "wsp/impostor" };
    const dir = await cloneAs("depth 1", "wsp/impostor");
    await commit(dir, "notes.txt", "impostor\n");
    const r = await git(["push", "origin", "wsp/impostor"], dir);
    expect(r.code).not.toBe(0);
    expect(r.out).toContain("remote: wsp: github.com/acme/app is not this thread's project");
    expect(await upstreamRef("refs/heads/wsp/impostor")).toBe("");
  }, 60_000);

  it("another repo fetches but refuses a push at the advertisement with its sentence", async () => {
    resetState("wsp/other");
    const dir = await cloneAs("depth 1", "wsp/other");
    const fetched = await git(["fetch", "--depth", "1", "https://github.com/acme/other", "main"], dir);
    expect(fetched.code, fetched.out).toBe(0);
    await commit(dir, "notes.txt", "other\n");
    const r = await git(["push", "https://github.com/acme/other", "HEAD:wsp/other"], dir);
    expect(r.code).not.toBe(0);
    expect(r.out).toContain("remote: wsp: github.com/acme/other is not this thread's project; pushes go only to github.com/acme/app");
    expect(await upstreamRef("refs/heads/wsp/other", "acme/other.git")).toBe("");
  }, 60_000);

  for (const [mode, status, said] of [
    ["401", 403, "remote: wsp: GitHub refused the gh sign-in on this computer; run gh auth login there"],
    ["502", 503, "remote: wsp: GitHub did not answer your computer; try again"],
    ["down", 503, "remote: wsp: GitHub did not answer your computer; try again"],
    ["404", 403, "remote: wsp: GitHub does not show github.com/acme/app to the gh sign-in on this computer; check gh auth status there"],
    ["403", 403, "remote: wsp: GitHub refused github.com/acme/app to the gh sign-in on this computer (SSO or a rate limit); check gh auth status there"],
  ] as const) {
    it(`GitHub's API answering ${mode} on the record's own repo reads as that, never as another project`, async () => {
      resetState(`wsp/api${mode}`);
      const dir = await cloneAs("depth 1", `wsp/api${mode}`);
      await commit(dir, "notes.txt", `api ${mode}\n`);
      await withOwnDoor(undefined, async () => {
        apiMode = mode;
        const r = await git(["push", "origin", `wsp/api${mode}`], dir);
        expect(r.code).not.toBe(0);
        expect(r.out).toContain(said);
        expect(r.out).toContain(`${status}`);
        expect(r.out).not.toContain("is not this thread's project");
      });
      expect(await upstreamRef(`refs/heads/wsp/api${mode}`)).toBe("");
    }, 60_000);
  }

  it("a thread with no branch of record pushes nothing", async () => {
    resetState("wsp/none");
    const dir = await cloneAs("depth 1", "wsp/none");
    record = undefined;
    const r = await git(["push", "origin", "wsp/none"], dir);
    expect(r.code).not.toBe(0);
    expect(r.out).toContain("remote: wsp: this thread has no branch to push from a cloud machine");
  }, 60_000);
});

describe("a pack the host cannot write", () => {
  async function* chunks(...sizes: number[]): AsyncGenerator<Buffer> {
    for (const [i, size] of sizes.entries()) yield Buffer.concat([i === 0 ? Buffer.from("PACK") : Buffer.alloc(0), Buffer.alloc(size, 0x5a)]);
  }
  const settles = <T>(p: Promise<T>) => Promise.race([p.then(() => "resolved", () => "rejected"), new Promise((r) => setTimeout(() => r("hung"), 3000))]);

  it("a spool into a folder that is not there ends, with a small or a large first chunk", async () => {
    const missing = join(root, "no-such-folder", "pack");
    expect(await settles(spool(chunks(16), missing, 1 << 30))).toBe("rejected");
    expect(await settles(spool(chunks(64 * 1024, 64 * 1024), missing, 1 << 30))).toBe("rejected");
  });

  it("a spool onto a path that is a folder ends", async () => {
    const folder = mkdtempSync(join(root, "is-a-folder-"));
    expect(await settles(spool(chunks(64 * 1024, 64 * 1024), folder, 1 << 30))).toBe("rejected");
    rmSync(folder, { recursive: true, force: true });
  });

  it.skipIf(!existsSync("/dev/full"))("a spool onto a full disk ends", async () => {
    expect(await settles(spool(chunks(64 * 1024, 256 * 1024, 256 * 1024), "/dev/full", 1 << 30))).toBe("rejected");
  });

  it("a push whose pack cannot be spooled ends with a sentence and leaves nothing behind", async () => {
    resetState("wsp/nodisk");
    const dir = await cloneAs("depth 1", "wsp/nodisk");
    await commit(dir, "notes.txt", "no disk\n");
    const scratch = join(root, "scratch-is-a-file");
    writeFileSync(scratch, "");
    await withOwnDoor(undefined, async () => {
      const r = await git(["push", "origin", "wsp/nodisk"], dir);
      expect(r.code).not.toBe(0);
      expect(r.out).toContain("wsp: your computer could not hold this push on its disk; free some space there and push again");
    }, scratch);
    expect(await upstreamRef("refs/heads/wsp/nodisk")).toBe("");
    expect(readdirSync(root).filter((name) => name.startsWith("wsp-push-"))).toEqual([]);
  }, 60_000);
});

describe("the road's wire", () => {
  /** One request through the door as git would send it: absolute-form, the turn token as the proxy password. */
  function viaDoor(method: string, target: string, headers: Record<string, string> = {}, body?: Buffer): Promise<{ status: number; headers: IncomingMessage["headers"]; body: Buffer }> {
    return new Promise((resolve, reject) => {
      let answered = false;
      const req = request({ host: "127.0.0.1", port: doorPort, method, path: target, headers: { "proxy-authorization": `Basic ${Buffer.from(`wsp:${TURN}`).toString("base64")}`, ...headers } }, (res) => {
        answered = true;
        void readBody(res).then((b) => resolve({ status: res.statusCode ?? 0, headers: res.headers, body: b }));
      });
      req.on("error", (err) => (answered ? undefined : reject(err)));
      req.end(body);
    });
  }

  it("serves the receive-pack advertisement with no-thin added and push-cert taken out", async () => {
    resetState("wsp/caps");
    const direct = (await bare("acme/app.git", ["receive-pack", "--advertise-refs", join(root, "github", "acme/app.git")])).out;
    expect(direct).toContain("push-cert=");
    const r = await viaDoor("GET", "http://github.com/acme/app.git/info/refs?service=git-receive-pack");
    expect(r.status).toBe(200);
    const text = r.body.toString("latin1");
    const caps = text.slice(text.indexOf("\0") + 1, text.indexOf("\n", text.indexOf("\0")));
    expect(caps.split(" ")).toEqual(expect.arrayContaining(["report-status", "side-band-64k", "atomic", "push-options", "no-thin"]));
    expect(caps).not.toContain("push-cert");
    expect(text).toContain("refs/heads/main");
  });

  it("sends GitHub its own credential and none of the caller's auth headers or cookies", async () => {
    resetState("wsp/headers");
    const seen = upstreamSeen.length;
    const r = await viaDoor("GET", "http://github.com/acme/app.git/info/refs?service=git-upload-pack", { authorization: "Bearer caller-token", cookie: "user_session=caller" });
    expect(r.status).toBe(200);
    const sent = upstreamSeen.slice(seen);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.headers.authorization).toBe(`Basic ${Buffer.from(`x-access-token:${TOKEN}`).toString("base64")}`);
    expect(sent[0]?.headers.cookie).toBeUndefined();
    expect(sent[0]?.headers["proxy-authorization"]).toBeUndefined();
  });

  it("never answers 401: GitHub refusing the credential reads as a 403 sentence, and git does not prompt", async () => {
    resetState("wsp/badtoken");
    credential = { token: "gho_wspWrongPlaceholder" };
    const r = await viaDoor("GET", "http://github.com/acme/app.git/info/refs?service=git-upload-pack");
    expect(r.status).toBe(403);
    expect(r.body.toString()).toContain("wsp: GitHub refused the gh sign-in on this computer");
    const cloned = await git(["clone", "-q", "https://github.com/acme/app", join(root, "never")]);
    expect(cloned.code).not.toBe(0);
    expect(cloned.out).toContain("remote: wsp: GitHub refused the gh sign-in on this computer; run gh auth login there");
    expect(cloned.out).not.toContain("Username");
  });

  it("refuses with the sign-in sentence when this computer has no gh login", async () => {
    resetState("wsp/nologin");
    credential = undefined;
    const r = await viaDoor("GET", "http://github.com/acme/app.git/info/refs?service=git-upload-pack");
    expect(r.status).toBe(403);
    expect(r.body.toString()).toBe("wsp: sign gh in on this computer with gh auth login\n");
  });

  it("refuses LFS and every path that is not a fetch or a push, and claims only absolute-form github.com on the HTTP door", async () => {
    resetState("wsp/paths");
    const lfs = await viaDoor("POST", "http://github.com/acme/app.git/info/lfs/objects/batch", {}, Buffer.from("{}"));
    expect(lfs.status).toBe(403);
    expect(lfs.body.toString()).toContain("wsp: Git LFS does not run from a cloud thread");
    const dumb = await viaDoor("GET", "http://github.com/acme/app.git/HEAD");
    expect(dumb.status).toBe(403);
    const upload = await viaDoor("GET", "http://github.com/acme/app.git/info/refs?service=git-upload-archive");
    expect(upload.status).toBe(403);
    const service = road();
    const claims = (url: string, door: "http" | "gh" = "http") => service.claims({ url } as IncomingMessage, door);
    expect(claims("http://github.com/acme/app.git/info/refs?service=git-upload-pack")).toBe(true);
    expect(claims("http://github.com/acme/app.git/info/refs", "gh")).toBe(false);
    expect(claims("/acme/app.git/info/refs")).toBe(false);
    expect(claims("http://github.com.evil.example/acme/app.git/info/refs")).toBe(false);
    expect(claims("https://github.com/acme/app.git/info/refs")).toBe(false);
    expect(claims("http://github.com:8080/acme/app.git/info/refs")).toBe(false);
  });

  const pushLines = (ref: string) => {
    const command = Buffer.from(`${"0".repeat(40)} ${"1".repeat(40)} ${ref}\0report-status side-band-64k`);
    return Buffer.concat([Buffer.from((command.length + 4).toString(16).padStart(4, "0")), command, Buffer.from("0000")]);
  };

  it("a delete-only push sends GitHub no pack, even when a refused command brought one", async () => {
    resetState("wsp/delmix");
    const dir = await cloneAs("depth 1", "wsp/delmix");
    await commit(dir, "notes.txt", "delmix\n");
    expect((await git(["push", "origin", "wsp/delmix"], dir)).code).toBe(0);
    await commit(dir, "notes.txt", "delmix again\n");
    const seen = upstreamSeen.length;
    const r = await git(["push", "origin", ":wsp/delmix", "HEAD:main"], dir);
    expect(r.out).toContain("[deleted]");
    expect(r.out).toContain("! [remote rejected] HEAD -> main");
    const posted = upstreamSeen.slice(seen).filter((x) => x.method === "POST");
    expect(posted).toHaveLength(1);
    expect(posted[0]?.body.includes("PACK")).toBe(false);
    expect(await upstreamRef("refs/heads/wsp/delmix")).toBe("");
  }, 60_000);

  it("a pack past the cap is refused with a sentence and stops filling the disk", async () => {
    resetState("wsp/cap");
    const before = guarded.length;
    const seen = upstreamSeen.length;
    await withOwnDoor(64 * 1024, async () => {
      const junk = Buffer.concat([Buffer.from("PACK"), Buffer.alloc(256 * 1024, 0x5a)]);
      const r = await viaDoor("POST", "http://github.com/acme/app.git/git-receive-pack", { "content-type": "application/x-git-receive-pack-request" }, Buffer.concat([pushLines("refs/heads/wsp/cap"), junk]));
      expect(r.status).toBe(200);
      expect(r.body.toString("latin1")).toContain("ng refs/heads/wsp/cap wsp: this push is larger than GitHub takes in one push; push it in smaller parts");
    });
    expect(guarded.length).toBe(before);
    expect(upstreamSeen.slice(seen).some((x) => x.method === "POST")).toBe(false);
  });

  it("a push whose body after the commands is not a pack is refused before the guard or GitHub sees it", async () => {
    resetState("wsp/junk");
    const before = guarded.length;
    const seen = upstreamSeen.length;
    const r = await viaDoor("POST", "http://github.com/acme/app.git/git-receive-pack", { "content-type": "application/x-git-receive-pack-request" }, Buffer.concat([pushLines("refs/heads/wsp/junk"), Buffer.alloc(4096, 0x5a)]));
    expect(r.status).toBe(200);
    expect(r.body.toString("latin1")).toContain("ng refs/heads/wsp/junk wsp: this push could not be read; push again with git");
    expect(guarded.length).toBe(before);
    expect(upstreamSeen.slice(seen).some((x) => x.method === "POST")).toBe(false);
  });

  it("a push body that is not pkt-lines is refused, never forwarded", async () => {
    resetState("wsp/garbage");
    const seen = upstreamSeen.length;
    const r = await viaDoor("POST", "http://github.com/acme/app.git/git-receive-pack", { "content-type": "application/x-git-receive-pack-request" }, Buffer.from("zzzz not a pkt line"));
    expect(r.status).toBe(403);
    expect(upstreamSeen.slice(seen).some((s) => s.method === "POST")).toBe(false);
  });
});

