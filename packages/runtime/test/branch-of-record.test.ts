// SPDX-License-Identifier: AGPL-3.0-only
// git at a thread's start where the host keeps the machine's credentials: the
// branch of record on the workspace's first record, a sub-thread's preamble that
// fetches its one start commit by id at depth 1, and a clone one commit deep with
// one refspec for that branch, run through real git against a repo here. With the
// broker not ready, every line is today's.
import { execFileSync, spawn as spawnProcess, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer, type Socket } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { BROKER_PORT, childStartedLine, ENV_FROM_INPUT, forkNotPushableRefusal, SUB_SHALLOW_LINE, subOpening, subResumePreamble, type Caller, type DaemonResponse, type WorkspaceView } from "@wsp/protocol";
import type { Runtime } from "../src/runtime.js";
import { brokeredGitEnv, cloneLines, onRecordLines } from "../src/project-landing.js";
import { projectSource } from "../src/project-sources.js";
import { LocalBackend } from "@wsp/engine";
import { copyKey, createRuntime, type HarnessAdapterFactory } from "../src/runtime.js";
import { memoryStore } from "../src/store.js";
import { writeStub } from "../../protocol/test/stub-script.js";
import { freeBranchName } from "../src/projects/branch-of-record.js";
import { leadOnThisComputer } from "./sub-fixture.js";
import { fakeLocal, projectOn, stubBackend, tokenGuest, type StubMachine } from "./stub-backend.js";

const roots: string[] = [];
const scratch = (): string => {
  const root = mkdtempSync(join(tmpdir(), "wsp-branch-of-record-"));
  roots.push(root);
  return root;
};
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

/** gh on this computer as GitHub answers it for acme/web, renamed from acme/old: its id and its name now. Signed in, so
 * a start the host brokers gets past the check of gh's login here. */
const GH = `#!/bin/sh
[ "$*" = "auth token --hostname github.com" ] && { echo gho_TESTONLY; exit 0; }
case "$2" in
  repos/acme/web|repos/acme/old) printf '4242\\tacme\\tweb\\n' ;;
  repos/acme/web/pulls/7) printf '9001\\tana\\tweb-fork\\tfix-typo\\ttrue\\t4242\\tec5c10de663bd1860925ad42e9580bab4eb1d377\\n' ;;
  repos/acme/web/pulls/8) printf '9001\\tana\\tweb-fork\\tfix-typo\\tfalse\\t4242\\tec5c10de663bd1860925ad42e9580bab4eb1d377\\n' ;;
  repos/acme/web/pulls/9) printf '4242\\tacme\\tweb\\tlab/review-me\\tfalse\\t4242\\t9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e\\n' ;;
  repos/acme/web/commits/main) echo 1d2c3b4a5f6e7d8c9b0a1d2c3b4a5f6e7d8c9b0a ;;
  repos/acme/web/commits/feat/parent) echo 7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b ;;
  repos/acme/web/pulls/10) printf '\\t\\t\\tfix-typo\\tfalse\\t4242\\n' ;;
  *) echo "gh: Not Found (HTTP 404)" >&2; exit 1 ;;
esac
`;
const BRANCH = "wsp/fix-the-flaky-upload-test";
const NAME = "Fix the flaky upload test";
const spawn = (rt: Runtime, project: string, lead: Caller, from: string, name = NAME) => rt.workspaces.create({ project, name, sub: { on: "boat", from } }, lead);

const git = (args: readonly string[], cwd?: string, env: Record<string, string> = {}): string =>
  execFileSync("git", [...(cwd !== undefined ? ["-C", cwd] : []), ...args], { env: { ...process.env, GIT_TERMINAL_PROMPT: "0", ...env } }).toString().trim();

/** A commit on the remote alone, off the lead's head or the parent given, pushed to `ref` there. */
const pushedElsewhere = (repo: string, ref: string, message: string, parent = "HEAD"): string => {
  const sha = git(["-c", "user.name=t", "-c", "user.email=t@t", "commit-tree", "HEAD^{tree}", "-p", parent, "-m", message], repo);
  git(["push", "-q", "origin", `${sha}:${ref}`], repo);
  return sha;
};

/** The preamble's git lines run in an empty folder as the agent runs them, the remote's github.com url answered by the
 * bare repo here: what the folder holds after. */
function runPreamble(preamble: string, bare: string): { head: string; depth: string; branch: string; fetch: string; plainFetch: number } {
  const lines = preamble.split("\n").filter(l => l.startsWith("git "));
  const dir = scratch();
  const env = { GIT_CONFIG_COUNT: "1", GIT_CONFIG_KEY_0: `url.file://${bare}.insteadOf`, GIT_CONFIG_VALUE_0: "https://github.com/acme/web.git" };
  const ran = spawnSync("sh", ["-ec", lines.join("\n")], { cwd: dir, env: { ...process.env, ...env, GIT_TERMINAL_PROMPT: "0" }, encoding: "utf8" });
  expect(ran.stderr, `the preamble's lines failed: ${lines.join(" ; ")}`).toBe("");
  // A plain fetch before the branch is pushed: a refspec naming the branch exactly fails it.
  const plainFetch = spawnSync("git", ["fetch", "-q", "origin"], { cwd: dir, env: { ...process.env, ...env, GIT_TERMINAL_PROMPT: "0" } }).status ?? -1;
  return { head: git(["rev-parse", "HEAD"], dir), depth: git(["rev-list", "--count", "HEAD"], dir), branch: git(["branch", "--show-current"], dir), fetch: git(["config", "--get-all", "remote.origin.fetch"], dir), plainFetch };
}

describe("a sub-thread started where the host keeps its machine's credentials", () => {
  it("fetches the one commit --from names at depth 1, by its id, onto wsp/<slug>, for a branch, a tag and a sha", async () => {
    const { rt, project, lead, repo } = await leadOnThisComputer({ broker: true, gh: GH, git: { ahead: 1 }, agents: { spawn: true, maxMachines: 5 } });
    const bare = join(dirname(repo), "remote.git");
    const parent = pushedElsewhere(repo, "refs/heads/main", "the parent");
    const main = pushedElsewhere(repo, "refs/heads/main", "the suite moved to tests/k6", parent);
    const tagged = pushedElsewhere(repo, "refs/heads/release", "the release");
    git(["-c", "user.name=t", "-c", "user.email=t@t", "tag", "-a", "-m", "v1", "v1", tagged], repo);
    git(["push", "-q", "origin", "v1"], repo);
    // The tag's commit is then left behind by its branch, so it is no branch's tip.
    pushedElsewhere(repo, "refs/heads/release", "after the release", tagged);
    git(["fetch", "-q", "origin"], repo);
    const cases = [
      { from: "origin/main", sha: main, name: NAME, branch: BRANCH },
      { from: "v1", sha: tagged, name: "From the tag", branch: "wsp/from-the-tag" },
      { from: tagged.slice(0, 9), sha: tagged, name: "From the commit", branch: "wsp/from-the-commit" },
    ];
    for (const c of cases) {
      const made = await spawn(rt, project.id, lead, c.from, c.name);
      expect(made.sub!.out.base.sha).toBe(c.sha);
      const lines = ["git init -q", `git remote add -t '${c.branch}*' origin https://github.com/acme/web.git`, `git fetch -q --depth 1 origin ${c.sha}`, `git checkout -q -b ${c.branch} FETCH_HEAD`];
      expect(made.sub!.preamble, c.from).toContain(lines.join("\n"));
      expect(made.sub!.preamble).toContain(SUB_SHALLOW_LINE);
      expect(made.sub!.preamble).not.toContain("gh auth setup-git");
      expect(made.sub!.preamble).not.toContain("blob:none");
      expect(runPreamble(made.sub!.preamble, bare), c.from).toEqual({ head: c.sha, depth: "1", branch: c.branch, fetch: `+refs/heads/${c.branch}*:refs/remotes/origin/${c.branch}*`, plainFetch: 0 });
      expect((await rt.workspaces.get(made.id)).branchOfRecord).toEqual({ repoId: 4242, owner: "acme", name: "web", branch: c.branch, startSha: c.sha });
    }
  });

  it("names its lead and its branch of record in the first event, the record, before the cloud is asked", async () => {
    const { rt, boat, project, lead, threadId, token } = await leadOnThisComputer({ broker: true, gh: GH });
    const heard: { workspace: WorkspaceView; creates: number }[] = [];
    rt.events.on("*", e => void (e.type === "workspace.created" && heard.push({ workspace: structuredClone(e.workspace), creates: boat.creates.length })));
    const made = await rt.workspaces.create({ project: project.id, name: NAME, sub: { on: "boat" } }, lead);
    const first = heard.find(h => h.workspace.id === made.id);
    expect(first).toMatchObject({ creates: 0, workspace: { parentThreadId: threadId, rootThreadId: threadId, branchOfRecord: { repoId: 4242, owner: "acme", name: "web", branch: BRANCH, startSha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" } } });
    await (await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, "Run the suite."), harness: made.sub!.harness, turnToken: token, cwd: made.sub!.folder }, lead)).finished;
    expect(boat.creates).toHaveLength(1);
  });

  it("is refused, naming the repo, where gh on this computer cannot say which repository the remote is", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer({ broker: true, gh: `#!/bin/sh\n[ "$*" = "auth token --hostname github.com" ] && { echo gho_TESTONLY; exit 0; }\necho "gh: To get started with GitHub CLI, please run: gh auth login" >&2\nexit 4\n` });
    await expect(rt.workspaces.create({ project: project.id, name: NAME, sub: { on: "boat" } }, lead)).rejects.toThrow(
      "gh on this computer could not read acme/web (gh: To get started with GitHub CLI, please run: gh auth login), and a machine whose credentials this host keeps pushes only to a repository GitHub names. Run gh auth status on this computer, then start it again.",
    );
    expect(boat.creates).toEqual([]);
    expect((await rt.workspaces.list()).map(w => w.name)).not.toContain(NAME);
  });

  it("with the broker not ready, keeps today's lines and records no branch of record", async () => {
    const { rt, project, lead } = await leadOnThisComputer({ gh: GH });
    const made = await rt.workspaces.create({ project: project.id, name: NAME, sub: { on: "boat" } }, lead);
    expect(made.sub!.preamble).toContain(["gh auth setup-git", "git init -q", "git remote add origin https://github.com/acme/web.git", "git fetch -q --filter=blob:none origin feat/upload"].join("\n"));
    expect(made.sub!.preamble).not.toContain(SUB_SHALLOW_LINE);
    expect((await rt.workspaces.get(made.id)).branchOfRecord).toBeUndefined();
  });

  it("gets its pushed branch back at depth 1 on a new machine, or its start commit where it pushed nothing", () => {
    const o = { on: "Boat", size: { name: "small", cpu: 2, memMb: 4096 }, folder: "/root/web", sent: [], setupGit: "gh auth setup-git", repo: "https://github.com/acme/web.git", base: { branch: "main", sha: "4be1c0a9" }, branch: BRANCH, deleteAfterMinutes: 0, brokered: true as const };
    expect(subResumePreamble({ ...o, pushed: true })).toContain(["git init -q", `git remote add -t '${BRANCH}*' origin ${o.repo}`, `git fetch -q --depth 1 origin ${BRANCH}`, `git checkout -q -b ${BRANCH} FETCH_HEAD`].join("\n"));
    expect(subResumePreamble({ ...o, pushed: false })).toContain([`git fetch -q --depth 1 origin 4be1c0a9`, `git checkout -q -b ${BRANCH} FETCH_HEAD`].join("\n"));
    const { brokered: _, ...today } = o;
    expect(subResumePreamble({ ...today, pushed: true })).toContain(["gh auth setup-git", "git init -q", `git remote add origin ${o.repo}`, `git fetch -q --filter=blob:none origin ${BRANCH}`, `git checkout -q -b ${BRANCH} FETCH_HEAD`].join("\n"));
  });
});

describe("a clone where the host keeps the machine's credentials", () => {
  const source = projectSource("github");

  const lab = (branch: string, startSha?: string, repo: { owner: string; name: string } = { owner: "acme", name: "lab" }, held = false) => ({ ofRecord: { repoId: 7, ...repo, branch, ...(startSha !== undefined ? { startSha } : {}) }, origin: "acme/lab", held });
  const SHA = "1d2c3b4a5f6e7d8c9b0a1d2c3b4a5f6e7d8c9b0a";

  it("clones one commit deep at the base, adds one refspec for the branch of record and checks it out at its start commit; with no broker, today's lines", () => {
    const o = { source, remote: "https://github.com/acme/lab", checkout: "/root/lab", computer: "Boat", branch: "main" };
    expect(cloneLines({ ...o, brokered: lab("wsp/pricing-page", SHA) })).toEqual([
      "set -e",
      "mkdir -p '/root'",
      "git clone --depth 1 --branch main -- https://github.com/acme/lab /root/lab",
      "git -C /root/lab config --add remote.origin.fetch '+refs/heads/wsp/pricing-page*:refs/remotes/origin/wsp/pricing-page*'",
      `git -C /root/lab cat-file -e '${SHA}^{commit}' 2>/dev/null || git -C /root/lab fetch -q --depth 1 origin ${SHA}`,
      `git -C /root/lab checkout -q -B wsp/pricing-page ${SHA}`,
      "git -C /root/lab rev-parse HEAD",
    ]);
    // A fork's head: a remote of its own, named for its owner, which the branch pushes to.
    expect(cloneLines({ ...o, brokered: lab("main", SHA, { owner: "ana", name: "lab-fork" }) }).slice(3)).toEqual([
      "git -C /root/lab remote add -t 'main*' ana https://github.com/ana/lab-fork.git",
      `git -C /root/lab cat-file -e '${SHA}^{commit}' 2>/dev/null || git -C /root/lab fetch -q --depth 1 ana ${SHA}`,
      `git -C /root/lab checkout -q -B main ${SHA}`,
      "git -C /root/lab config branch.main.remote ana",
      "git -C /root/lab config branch.main.merge refs/heads/main",
      "git -C /root/lab rev-parse HEAD",
    ]);
    const today = cloneLines(o);
    expect(today).toContain("gh auth setup-git");
    expect(today.at(-1)).toBe("gh repo clone acme/lab /root/lab");
  });

  /** A bare repo of acme/lab here with commits one and two on main, and a commit helper. */
  const labRepo = () => {
    const root = scratch();
    const bare = join(root, "lab.git");
    const seed = join(root, "seed");
    git(["init", "-q", "--bare", "-b", "main", bare]);
    git(["init", "-q", "-b", "main", seed]);
    const commit = (at: string, file: string): string => {
      writeFileSync(join(at, file), file);
      git(["add", file], at);
      git(["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", file], at);
      return git(["rev-parse", "HEAD"], at);
    };
    for (const file of ["one", "two"]) commit(seed, file);
    git(["push", "-q", bare, "main"], seed);
    return { root, bare, seed, commit };
  };
  const run = (lines: readonly string[], env: Record<string, string> = {}): string => execFileSync("sh", ["-ec", lines.join("\n")], { env: { ...process.env, GIT_TERMINAL_PROMPT: "0", ...env } }).toString().trim();

  it("leaves a real checkout one commit deep on the branch of record, whose lease holds after a push and a rebase on the base", () => {
    const { root, bare, seed, commit } = labRepo();
    const checkout = join(root, "work", "lab");
    const lines = cloneLines({ source, remote: `file://${bare}`, checkout, computer: "Boat", branch: "main", brokered: lab("wsp/pricing-page", git(["rev-parse", "main"], bare)) });
    expect(run(lines)).toBe(git(["rev-parse", "main"], bare));
    expect(git(["rev-list", "--count", "HEAD"], checkout)).toBe("1");
    expect(git(["branch", "--show-current"], checkout)).toBe("wsp/pricing-page");
    // A new branch tracks nothing the remote does not hold, so its first lease expects no branch there.
    expect(spawnSync("git", ["-C", checkout, "rev-parse", "--verify", "-q", "refs/remotes/origin/wsp/pricing-page"]).status).not.toBe(0);
    expect(git(["config", "--get-all", "remote.origin.fetch"], checkout).split("\n")).toEqual(["+refs/heads/main:refs/remotes/origin/main", "+refs/heads/wsp/pricing-page*:refs/remotes/origin/wsp/pricing-page*"]);
    // A plain fetch before the first push finds nothing to refuse.
    git(["fetch", "-q", "origin"], checkout);
    // Push, amend, lease, the base moves, fetch it, rebase, lease again.
    const as = ["-c", "user.name=t", "-c", "user.email=t@t"];
    writeFileSync(join(checkout, "work"), "one");
    git(["add", "work"], checkout);
    git([...as, "commit", "-q", "-m", "work"], checkout);
    git(["push", "-q", "-u", "origin", "wsp/pricing-page"], checkout);
    git([...as, "commit", "-q", "--amend", "-m", "work again"], checkout);
    git(["push", "-q", "--force-with-lease", "origin", "wsp/pricing-page"], checkout);
    commit(seed, "three");
    git(["push", "-q", bare, "main"], seed);
    git(["fetch", "-q", "--depth", "1", "origin", "main"], checkout);
    git([...as, "rebase", "-q", "origin/main"], checkout);
    git(["push", "-q", "--force-with-lease", "origin", "wsp/pricing-page"], checkout);
    expect(git(["rev-parse", "wsp/pricing-page^"], bare)).toBe(git(["rev-parse", "main"], bare));
  });

  it("puts a pull request thread on the head itself, its own repo's or a fork's, tracking it, so a plain push and a first lease land where the head lives", () => {
    const { root, bare, seed, commit } = labRepo();
    // The pull request's head: a branch off two, then main moves on, so the head is no descendant of main's tip.
    git(["checkout", "-q", "-b", "fix-typo"], seed);
    const head = commit(seed, "typo");
    git(["push", "-q", bare, "fix-typo"], seed);
    git(["checkout", "-q", "main"], seed);
    commit(seed, "three");
    git(["push", "-q", bare, "main"], seed);
    const as = ["-c", "user.name=t", "-c", "user.email=t@t"];
    /** A commit, a plain push, then an amend pushed with a lease and nothing fetched between: what lands upstream. */
    const pushes = (checkout: string, env: Record<string, string> = {}): void => {
      writeFileSync(join(checkout, "more"), "more");
      git(["add", "more"], checkout);
      git([...as, "commit", "-q", "-m", "more"], checkout);
      git(["push", "-q"], checkout, env);
      git([...as, "commit", "-q", "--amend", "-m", "more again"], checkout);
      git(["push", "-q", "--force-with-lease"], checkout, env);
    };
    const own = join(root, "own", "lab");
    expect(run(cloneLines({ source, remote: `file://${bare}`, checkout: own, computer: "Boat", branch: "main", brokered: lab("fix-typo", head, undefined, true) }))).toBe(head);
    expect(git(["status", "-sb"], own).split("\n")[0]).toBe("## fix-typo...origin/fix-typo");
    pushes(own);
    expect(git(["rev-parse", "fix-typo"], bare)).toBe(git(["rev-parse", "HEAD"], own));
    // A fork whose head branch is named main, as many are: the fork's main, not the base's.
    const fork = join(root, "fork.git");
    git(["init", "-q", "--bare", "-b", "main", fork]);
    git(["push", "-q", fork, `${head}:refs/heads/main`], seed);
    const env = { GIT_CONFIG_COUNT: "1", GIT_CONFIG_KEY_0: `url.file://${fork}.insteadOf`, GIT_CONFIG_VALUE_0: "https://github.com/ana/lab-fork.git" };
    const theirs = join(root, "theirs", "lab");
    expect(run(cloneLines({ source, remote: `file://${bare}`, checkout: theirs, computer: "Boat", branch: "main", brokered: lab("main", head, { owner: "ana", name: "lab-fork" }, true) }), env)).toBe(head);
    expect(git(["status", "-sb"], theirs).split("\n")[0]).toBe("## main...ana/main");
    pushes(theirs, env);
    expect(git(["rev-parse", "main"], fork)).toBe(git(["rev-parse", "HEAD"], theirs));
    expect(git(["rev-parse", "main"], bare)).not.toBe(git(["rev-parse", "HEAD"], theirs));
  });

  it("puts a checkout an image already holds on its branch of record where it stands, and says the commit", () => {
    const { root, bare } = labRepo();
    const held = join(root, "held");
    git(["clone", "-q", bare, held]);
    expect(run(["set -e", ...onRecordLines({ checkout: held, ...lab("wsp/pricing-page") })])).toBe(git(["rev-parse", "HEAD"], held));
    expect(git(["branch", "--show-current"], held)).toBe("wsp/pricing-page");
  });

  it("sends every GitHub remote form to the daemon's door as git's proxy, the token its password, with no prompt and no helper", async () => {
    const env = brokeredGitEnv("wsp-op-token");
    expect(env["GIT_CONFIG_COUNT"]).toBe("7");
    expect(env[`GIT_CONFIG_VALUE_4`]).toBe(`http://wsp:wsp-op-token@127.0.0.1:${BROKER_PORT}`);
    expect(env["GIT_TERMINAL_PROMPT"]).toBe("0");
    // The same pairs with the door on a port of this test's own: what git sends it for each remote form.
    const heard: string[] = [];
    const door = createServer((s: Socket) => {
      s.once("data", d => {
        heard.push(d.toString("latin1").split("\r\n").filter(l => /^(GET|Proxy-Authorization)/.test(l)).join(" | "));
        s.end("HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
      });
    });
    await new Promise<void>(r => door.listen(0, "127.0.0.1", r));
    const port = (door.address() as { port: number }).port;
    const local = Object.fromEntries(Object.entries(env).map(([k, v]) => [k, v.replace(`:${BROKER_PORT}`, `:${port}`)]));
    const dir = scratch();
    try {
      for (const remote of ["https://github.com/acme/web.git", "git@github.com:acme/web.git", "ssh://git@github.com/acme/web.git", "ssh://git@ssh.github.com:443/acme/web.git"]) {
        const ran = await new Promise<number>(resolve => {
          const p = spawnProcess("git", ["ls-remote", remote], { cwd: dir, env: { ...process.env, ...local, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null" }, stdio: "ignore" });
          p.on("exit", code => resolve(code ?? -1));
        });
        expect(ran).not.toBe(0);
      }
    } finally {
      door.close();
    }
    const auth = `Proxy-Authorization: Basic ${Buffer.from("wsp:wsp-op-token").toString("base64")}`;
    expect(heard).toEqual(Array(4).fill(`GET http://github.com/acme/web.git/info/refs?service=git-upload-pack HTTP/1.1 | ${auth}`));
  });
});


/** An agent that answers each turn at once. */
const answering: HarnessAdapterFactory = () => ({
  steers: false,
  start: o => {
    const sessionId = `s-${Math.random().toString(16).slice(2)}`;
    o.onEvent({ type: "session.start", sessionId });
    o.onEvent({ type: "turn.done", sessionId, result: { status: "completed", text: "ok" } });
    o.onEvent({ type: "session.end", sessionId, exitCode: 0, sawResult: true });
    return { localId: sessionId, finished: Promise.resolve({ status: "completed", text: "ok" }), interrupt: async () => {} };
  },
});

/** A host whose provider takes input on each command and answers where a checkout stands, gh on this computer
 * answering for acme/web, every daemon frame kept, and a broker that answers ready when told; one project of
 * acme/web at the provider, recorded on main, whose image holds its checkout where `image` says so. */
async function onProvider(o: { leadBranch?: string; image?: true } = {}) {
  const root = scratch();
  const bin = join(root, "bin");
  mkdirSync(bin);
  writeStub(join(bin, "gh"), GH);
  const backend = stubBackend();
  backend.execImpl = tokenGuest;
  const stdin: { machine: string; cmd: string; input: string; scope?: unknown }[] = [];
  const frames: Record<string, unknown>[] = [];
  let rt!: Runtime;
  const create = backend.create.bind(backend);
  backend.create = async spec => {
    const m = (await create(spec)) as StubMachine & { takesStdin?: boolean };
    m.takesStdin = true;
    const exec = m.exec.bind(m);
    m.exec = async (cmd, opts) => {
      if (opts?.stdin === undefined) return exec(cmd, opts);
      const input = Buffer.from(opts.stdin).toString();
      const token = /GIT_CONFIG_VALUE_4=http:\/\/wsp:([^@]+)@/.exec(input)?.[1];
      const held = token === undefined ? undefined : await rt.devices.match(token);
      stdin.push({ machine: m.id, cmd, input, ...(held?.scope !== undefined ? { scope: held.scope } : {}) });
      m.execLog.push(cmd);
      return { exitCode: 0, stdout: cmd.endsWith("git -C /root/web rev-parse HEAD") ? `${IMAGE_SHA}\n` : "", stderr: "" };
    };
    return m;
  };
  const broker = { on: false };
  const local = { ...fakeLocal(join(root, "home")), daemonRoad: async () => ({ url: "ws://this-computer", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "cafef00d".repeat(3) }), backend: new LocalBackend({ root: join(root, "home"), env: { ...process.env, PATH: `${bin}:${process.env["PATH"] ?? ""}` } }) };
  // Each machine's daemon answers where its branch is, a child's start on a branch, and a pull request's reads.
  const daemonChannel = async () => ({
    send: async (frame: unknown) => {
      const f = frame as Record<string, unknown>;
      frames.push(f);
      if (f["op"] === "git.status") return { id: 1, ok: true, branch: { oid: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a", head: o.leadBranch ?? "main", upstream: `origin/${o.leadBranch ?? "main"}`, ahead: 0, behind: 0 }, entries: [], root: String(f["cwd"]) } as unknown as DaemonResponse;
      if (f["op"] === "git.startOn") return { id: 1, ok: true, branch: String(f["branch"]) } as unknown as DaemonResponse;
      const number = Number(f["number"]);
      const fork = number === 9 ? {} : { fork: { owner: "ana", pushable: number !== 8 } };
      if (f["op"] === "git.prRead") return { id: 1, ok: true, pr: { number, url: `https://github.com/acme/web/pull/${number}`, state: "open", host: "github.com", draft: false, base: "main", branch: number === 9 ? "lab/review-me" : "fix-typo", headOid: "ec5c10de663bd1860925ad42e9580bab4eb1d377", headSubject: "Fix a typo", mergeable: "mergeable", mergeState: "clean", review: "none", checks: [], additions: 1, deletions: 1, changedFiles: 1, commits: 1, author: "ana", ...fork } } as unknown as DaemonResponse;
      if (f["op"] === "git.issueRead") return { id: 1, ok: true, issue: { number, url: `https://github.com/acme/web/pull/${number}`, title: "Fix a typo", body: "One word.", state: "OPEN", comments: [] } } as unknown as DaemonResponse;
      if (f["op"] === "git.prCheckout") return { id: 1, ok: true, branch: "fix-typo" } as unknown as DaemonResponse;
      if (f["op"] === "git.prDiff") return { id: 1, ok: true, diff: "", truncated: false, left: [] } as unknown as DaemonResponse;
      return { id: 1, ok: false, error: `${String(f["op"])} was not answered` } as unknown as DaemonResponse;
    },
    close: () => {},
    closed: new Promise<{ code: number; reason: string }>(() => {}),
  });
  const store = memoryStore();
  await store.put("goldens", copyKey("default", "default"), { head: 1, versions: [{ version: 1, snapshotId: "snap_g", kind: "desktop", baseTemplate: "base", setupSha: "abc", createdAt: "2026-09-01T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 } }] });
  const options = { backend, store, adapters: { claude: answering }, local, daemonToken: "cafef00d".repeat(3), daemonChannel, broker: { ready: () => broker.on, codexClaims: async () => undefined } };
  const boot = (): Runtime => {
    const made = createRuntime(options);
    made.events.on("workspace.created", e => {
      if (e.type !== "workspace.created") return;
      const m = backend.machines.find(x => x.id === e.workspace.machineId);
      if (m !== undefined) m.previewUrl = async () => ({ url: "http://127.0.0.1:7070", token: "e", expiresAt: Date.now() + 3_600_000 });
    });
    return made;
  };
  rt = boot();
  let project = await projectOn(rt, undefined, "https://github.com/acme/web.git", { name: "web", base: "main" });
  if (o.image === true) {
    // The project's own image, which holds its checkout: the host started again reads it off the record.
    project = { ...project, image: { snapshotId: "snap_g", builtAt: "2026-10-01T00:00:00.000Z" } };
    await store.put("projects", project.id, project);
    await rt.close();
    rt = boot();
  }
  return { rt, backend, stdin, frames, broker, project };
}

const PARENT_SHA = "7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b";
const IMAGE_SHA = "5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f";
const cloneOf = (m: StubMachine): string => m.execLog.find(c => c.includes("git clone")) ?? "";

/** A lead workspace at the provider with its thread run, and the broker then ready: the caller its thread asks as. */
async function leadThread(rt: Runtime, broker: { on: boolean }, project: string): Promise<{ lead: string; threadId: string; asking: Caller }> {
  const lead = await rt.workspaces.create({ project, golden: "snap_g", name: "lead", agents: { spawn: true } });
  const turn = await rt.sessions.start(lead.id, { prompt: "coordinate", harness: "claude" });
  await turn.finished;
  const threadId = turn.view().threadId!;
  broker.on = true;
  return { lead: lead.id, threadId, asking: { origin: "here", by: { kind: "thread", threadId, workspaceId: lead.id, rootThreadId: threadId } } };
}

describe("a workspace's clone at a provider", () => {
  it("with the broker not ready, runs today's lines and records no branch of record", async () => {
    const { rt, backend, project } = await onProvider();
    const ws = await rt.workspaces.create({ project: project.id, golden: "snap_g", name: "pricing page" });
    expect(cloneOf(backend.machines.at(-1)!)).toContain("git clone --branch main -- https://github.com/acme/web.git /root/web");
    expect((await rt.workspaces.get(ws.id)).branchOfRecord).toBeUndefined();
  });

  it("asked by a thread, starts one commit deep on its parent's branch, on its own wsp/<slug>, under a token minted for the clone and taken back after", async () => {
    const { rt, backend, stdin, frames, broker, project } = await onProvider({ leadBranch: "feat/parent" });
    const { asking } = await leadThread(rt, broker, project.id);
    const ws = await rt.workspaces.create({ project: project.id, name: "pricing page" }, asking);
    expect((await rt.workspaces.get(ws.id)).branchOfRecord).toEqual({ repoId: 4242, owner: "acme", name: "web", branch: "wsp/pricing-page", startSha: PARENT_SHA });
    const m = backend.machines.find(x => x.id === ws.machineId)!;
    const clone = stdin.find(s => s.machine === m.id && s.cmd.includes("git clone"))!;
    expect(clone.cmd.split("\n")).toEqual([
      ENV_FROM_INPUT,
      "set -e",
      "mkdir -p '/root'",
      "git clone --depth 1 --branch feat/parent -- https://github.com/acme/web.git /root/web",
      "git -C /root/web config --add remote.origin.fetch '+refs/heads/wsp/pricing-page*:refs/remotes/origin/wsp/pricing-page*'",
      `git -C /root/web cat-file -e '${PARENT_SHA}^{commit}' 2>/dev/null || git -C /root/web fetch -q --depth 1 origin ${PARENT_SHA}`,
      `git -C /root/web checkout -q -B wsp/pricing-page ${PARENT_SHA}`,
      "git -C /root/web rev-parse HEAD",
    ]);
    // Nothing moves the child onto its parent's branch after the clone: git.startOn would find no ref to put it on.
    expect(frames.filter(f => f["op"] === "git.startOn")).toEqual([]);
    expect(ws.notice).toContain(childStartedLine("pricing page", "wsp/pricing-page"));
    const env = Object.fromEntries(clone.input.split("\0").filter(Boolean).map(kv => [kv.slice(0, kv.indexOf("=")), kv.slice(kv.indexOf("=") + 1)]));
    const token = new RegExp(`^http://wsp:([^@]+)@127\\.0\\.0\\.1:${BROKER_PORT}$`).exec(env["GIT_CONFIG_VALUE_4"] ?? "")?.[1];
    expect(token, "the clone's proxy carries a token").toBeDefined();
    expect(m.execLog.join("\n")).not.toContain(token!);
    expect(await rt.devices.match(token!)).toBeUndefined();
  });

  it("made by the person for a new thread, clones under a token scoped to the thread it opens, which the first start on it takes", async () => {
    const { rt, backend, stdin, broker, project } = await onProvider();
    broker.on = true;
    const ws = await rt.workspaces.create({ project: project.id, golden: "snap_g", name: "pricing page" });
    const clone = stdin.find(s => s.machine === ws.machineId && s.cmd.includes("git clone --depth 1"));
    const scope = clone?.scope as { threadId: string } | undefined;
    expect(scope).toEqual({ kind: "thread", threadId: expect.any(String), workspaceId: ws.id, rootThreadId: scope?.threadId });
    const first = await rt.sessions.start(ws.id, { prompt: "price it", harness: "claude" });
    await first.finished;
    expect(first.view().threadId).toBe(scope!.threadId);
    const second = await rt.sessions.start(ws.id, { prompt: "and again", harness: "claude" });
    await second.finished;
    expect(second.view().threadId).not.toBe(scope!.threadId);
    expect(backend.machines.filter(m => m.id === ws.machineId)).toHaveLength(1);
  });

  it("names wsp/<slug> numbered past a branch another thread of this host holds, a sub-thread's or a branch of record, or the remote has", async () => {
    const { rt, broker, project } = await onProvider();
    const { asking } = await leadThread(rt, broker, project.id);
    const first = await rt.workspaces.create({ project: project.id, name: "pricing page" }, asking);
    const second = await rt.workspaces.create({ project: project.id, name: "Pricing  page!" }, asking);
    expect((await rt.workspaces.get(first.id)).branchOfRecord?.branch).toBe("wsp/pricing-page");
    expect((await rt.workspaces.get(second.id)).branchOfRecord?.branch).toBe("wsp/pricing-page-2");
    const live = new Map([
      ["a", { record: { branchOfRecord: { branch: "wsp/x" } } }],
      ["b", { record: { sub: { branch: "wsp/x-2" } } }],
    ]) as unknown as Parameters<typeof freeBranchName>[0]["live"];
    expect(freeBranchName({ live }, "wsp/x", "abc\trefs/heads/wsp/x-3\n")).toBe("wsp/x-4");
    expect(freeBranchName({ live }, "wsp/y", undefined)).toBe("wsp/y");
  });

  it("whose project image holds the checkout, puts that checkout on its branch of record and records the commit it stood on", async () => {
    const { rt, backend, stdin, broker, project } = await onProvider({ image: true });
    broker.on = true;
    const ws = await rt.workspaces.create({ project: project.id, name: "pricing page" });
    const landed = stdin.find(s => s.machine === ws.machineId)!;
    expect(landed.cmd.split("\n").slice(1)).toEqual([
      "set -e",
      "git -C /root/web config --add remote.origin.fetch '+refs/heads/wsp/pricing-page*:refs/remotes/origin/wsp/pricing-page*'",
      "git -C /root/web checkout -q -B wsp/pricing-page",
      "git -C /root/web rev-parse HEAD",
    ]);
    expect(cloneOf(backend.machines.find(m => m.id === ws.machineId)!)).toBe("");
    // Only the machine can say where the image's checkout stands: the commit comes back off its last line.
    expect((await rt.workspaces.get(ws.id)).branchOfRecord).toEqual({ repoId: 4242, owner: "acme", name: "web", branch: "wsp/pricing-page", startSha: IMAGE_SHA });
  });

  it("whose project image holds the checkout, still starts a thread-asked fork on its parent's commit", async () => {
    const { rt, stdin, broker, project } = await onProvider({ image: true, leadBranch: "feat/parent" });
    const { asking } = await leadThread(rt, broker, project.id);
    const ws = await rt.workspaces.create({ project: project.id, name: "pricing page" }, asking);
    expect((await rt.workspaces.get(ws.id)).branchOfRecord?.startSha).toBe(PARENT_SHA);
    expect(stdin.find(s => s.machine === ws.machineId)!.cmd).toContain(`git -C /root/web checkout -q -B wsp/pricing-page ${PARENT_SHA}`);
  });
});

describe("a thread started from a pull request at a provider", () => {
  const start = (rt: Runtime, number: number) => rt.workspaces.start({ url: `https://github.com/acme/web/pull/${number}`, agent: "claude" });

  it("records a fork's head by repo id, branch and commit in the first write, and clones onto that head from the fork, with no checkout after", async () => {
    const { rt, broker, stdin, frames } = await onProvider();
    broker.on = true;
    const heard: WorkspaceView[] = [];
    rt.events.on("workspace.created", e => void (e.type === "workspace.created" && heard.push(structuredClone(e.workspace))));
    const started = await start(rt, 7);
    // The clone ran under a token scoped to the thread this start opened, and the token is gone.
    const clone = stdin.find(s => s.cmd.includes("git clone --depth 1"))!;
    expect(clone.scope).toEqual({ kind: "thread", threadId: started.threadId, workspaceId: started.workspace.id, rootThreadId: started.threadId });
    const sha = "ec5c10de663bd1860925ad42e9580bab4eb1d377";
    expect(clone.cmd.split("\n").slice(4)).toEqual([
      "git -C /root/web remote add -t 'fix-typo*' ana https://github.com/ana/web-fork.git",
      `git -C /root/web cat-file -e '${sha}^{commit}' 2>/dev/null || git -C /root/web fetch -q --depth 1 ana ${sha}`,
      `git -C /root/web checkout -q -B fix-typo ${sha}`,
      `git -C /root/web update-ref refs/remotes/ana/fix-typo ${sha}`,
      "git -C /root/web config branch.fix-typo.remote ana",
      "git -C /root/web config branch.fix-typo.merge refs/heads/fix-typo",
      "git -C /root/web rev-parse HEAD",
    ]);
    expect(frames.filter(f => f["op"] === "git.prCheckout")).toEqual([]);
    const head = { repoId: 9001, owner: "ana", name: "web-fork", branch: "fix-typo", startSha: sha };
    expect(heard.find(w => w.id === started.workspace.id)?.branchOfRecord).toEqual(head);
    expect((await rt.workspaces.get(started.workspace.id)).branchOfRecord).toEqual(head);
  });

  it("refuses a start on a fork whose author allowed no edits from maintainers, and one whose fork is gone, before anything is made", async () => {
    const { rt, backend, broker } = await onProvider();
    broker.on = true;
    const before = backend.machines.length;
    await expect(start(rt, 8)).rejects.toMatchObject({ kind: "usage", message: forkNotPushableRefusal("ana", 8).message });
    await expect(start(rt, 10)).rejects.toThrow("pull request #10's fork was deleted, so there is no branch to push to");
    expect(backend.machines.length).toBe(before);
    expect(await rt.workspaces.list()).toEqual([]);
  });

  it("reviews a fork whose author allowed no edits from maintainers, since a review pushes nothing", async () => {
    const { rt, broker } = await onProvider();
    broker.on = true;
    const reviewing = await rt.workspaces.review({ url: "https://github.com/acme/web/pull/8", agent: "claude" });
    expect((await rt.workspaces.get(reviewing.workspace.id)).branchOfRecord).toMatchObject({ owner: "ana", name: "web-fork", branch: "fix-typo" });
  });

  it("records the head branch on the project's own repo for a pull request that is not a fork's, and clones onto it from origin", async () => {
    const { rt, broker, stdin } = await onProvider();
    broker.on = true;
    const started = await start(rt, 9);
    expect((await rt.workspaces.get(started.workspace.id)).branchOfRecord).toEqual({ repoId: 4242, owner: "acme", name: "web", branch: "lab/review-me", startSha: "9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e" });
    expect(stdin.find(s => s.cmd.includes("git clone --depth 1"))!.cmd.split("\n").slice(-5)).toEqual([
      "git -C /root/web checkout -q -B lab/review-me 9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e",
      "git -C /root/web update-ref refs/remotes/origin/lab/review-me 9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e",
      "git -C /root/web config branch.lab/review-me.remote origin",
      "git -C /root/web config branch.lab/review-me.merge refs/heads/lab/review-me",
      "git -C /root/web rev-parse HEAD",
    ]);
  });

  it("with the broker not ready, starts a fork's pull request as today and records no branch of record", async () => {
    const { rt, frames } = await onProvider();
    const started = await start(rt, 8);
    expect((await rt.workspaces.get(started.workspace.id)).branchOfRecord).toBeUndefined();
    expect(frames.filter(f => f["op"] === "git.prCheckout")).toHaveLength(1);
  });
});
