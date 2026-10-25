// SPDX-License-Identifier: AGPL-3.0-only
// A lead thread on this computer starting a sub-thread on Boat: every check
// that can refuse runs before Boat is asked for a start, the machine forks the
// image head at the size asked or the cloud's own, the picked files land in
// /root/<repo> through the file road, and the thread opens there with the
// preamble. A machine that fails before the first turn is deleted and replaced
// once. The lead's folder is a real repo here; Boat is a stub backend that
// keeps every create, command and file, and the lead's daemon a fake that keeps
// every frame.
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { gunzipSync } from "node:zlib";
import { afterEach, describe, expect, it } from "vitest";
import { BOX_WORK_DIR, SERVERS_OFF_LINE, placeLimitsOf } from "@wsp/engine";
import {
  placeSaid, SUB_SEED_MAX_BYTES, subAheadUnknownRefusal, subBesideRefusal, subOtherProjectRefusal, subBranchNameOk, subLeadOnMachineRefusal, subNotSentLine, subPickedElsewhereWhy, subStartsNothingRefusal, subStayedLine, TURN_TOKEN_ENV, exitClassOf, fmtBytes, refusalLine, spawnCapRefusal, subAheadRefusal, subBranchNameRefusal, subDetachedRefusal, startsUsedLine,
  subLeadBranchRefusal, subMachineTakenRefusal, subNoAgentRefusal, subNoCloudRefusal, subNoImageRefusal, subNoRemoteRefusal, subNoSignInRefusal, brokeredAgentRefusal, subHostNoSignInRefusal, subHostOtherGitRefusal, subNoThreadRefusal,
  subNoUpstreamRefusal, subFromNotOnRemoteRefusal, subOpening, subOtherHostRefusal, subSeedTooBigRefusal, subSizeRefusal, spendCapRefusal, type Caller,
} from "@wsp/protocol";
import type { Runtime } from "../src/runtime.js";
import { copyKey } from "../src/runtime.js";
import { threadPlaces } from "../src/serve-projects.js";
import { PAID, PAID_SIZES, TRIAL, TRIAL_SIZES, leadOnThisComputer, until, type Boat, type Fault, type Setup } from "./sub-fixture.js";
import { leadAndBox } from "./box-fixture.js";

let root: string | undefined;
afterEach(() => {
  if (root !== undefined) rmSync(root, { recursive: true, force: true });
  root = undefined;
});

const TASK = "Fix the flaky upload test in packages/upload.";
/** What every fork gets from the create road, none of it carrying a secret: its hostname, and the machine context
 * (the probe, then the archive of the wsp-machine skill and its facts landed by the file road and unpacked). */
const CONTEXT_ARCHIVE = "/tmp/wsp-vault-in-";
const forkRoad = (cmd: string): boolean => cmd.startsWith("hostname ") || cmd.includes("WSP_CTX") || cmd.includes(CONTEXT_ARCHIVE);
const seedFiles = (boat: Boat): string[] => boat.files.map(f => f.path).filter(p => !p.startsWith(CONTEXT_ARCHIVE));
const NAME = "Fix the flaky upload test";
const spawn = (rt: Runtime, project: string, lead: Caller | undefined, sub: { on?: string; size?: string; from?: string; branch?: string; harness?: string } = {}, name = NAME) =>
  rt.workspaces.create({ project, name, sub: { on: "boat", ...sub } }, lead);

/** The sub-thread's first turn, which its machine is made for, run to its end. */
const opened = async (rt: Runtime, made: Awaited<ReturnType<typeof spawn>>, lead: Caller | undefined): Promise<void> =>
  void (await (await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, TASK), harness: made.sub!.harness, cwd: made.sub!.folder }, lead)).finished);

/** What a refusal threw, as a terminal reads it, and the class it exits with. */
async function refused(p: Promise<unknown>): Promise<{ said: string; cls: string }> {
  const e = await p.then(
    () => undefined,
    (x: unknown) => x,
  );
  expect(e, "the spawn went ahead").toBeInstanceOf(Error);
  return { said: (e as Error).message, cls: exitClassOf(e) };
}

describe("run --on boat from a thread on this computer", () => {
  it("tells the sub-thread's agents, in the machine context its fork writes, that a server needing the person's sign-in does not run there", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer({});
    try {
      await opened(rt, await spawn(rt, project.id, lead), lead);
      const context = boat.files.filter(f => f.path.startsWith(CONTEXT_ARCHIVE)).map(f => (f.bytes[0] === 0x1f && f.bytes[1] === 0x8b ? gunzipSync(f.bytes) : f.bytes).toString("utf8"));
      expect(context).toHaveLength(1);
      expect(context[0]).toContain(SERVERS_OFF_LINE);
    } finally {
      await rt.close();
    }
  }, 40_000);

  it("forks the image head at the cloud's size, lands the picked files in /root/<repo> by the file road, and opens the sub-thread there under the preamble", async () => {
    const { rt, store, boat, daemon, starts, project, threadId, lead, token, folder } = await leadOnThisComputer({ picks: ["CLAUDE.local.md", "local.properties"], git: { uncommitted: 2 } });
    const before = daemon.frames.length;
    const made = await spawn(rt, project.id, lead);
    // The record stands before the cloud is asked: the machine is made once the thread's start comes.
    expect(boat.creates).toEqual([]);
    expect(made.sub!.out).toEqual({
      on: "boat",
      machine: NAME,
      size: { cpu: 2, memMb: 4096 },
      rateUsdPerHour: 0.018,
      repo: "https://github.com/acme/web.git",
      base: { branch: "feat/upload", sha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" },
      branch: "wsp/fix-the-flaky-upload-test",
      sent: { paths: ["CLAUDE.local.md", "local.properties"], bytes: 1243, missing: [] },
      notSent: { uncommitted: 2 },
      deleteAfterMinutes: 30,
    });
    expect(made.sub!.folder).toBe("/root/web");
    expect(made.notice).toBe("2 uncommitted changes in web did not travel; the sub-thread starts at feat/upload as pushed (4be1c0a).");
    // The preamble, word for word as 03 gives it.
    expect(made.sub!.preamble).toBe(
      [
        `This thread runs on a machine of its own on Boat (small, 2 vCPU, 4 GB), started by thread ${threadId.slice(0, 8)}. The code is not here yet. The folder you are in, /root/web, holds only the git-ignored files the person picked: CLAUDE.local.md, local.properties. Get the code first, in this folder:`,
        "",
        "gh auth setup-git",
        "git init -q",
        "git remote add origin https://github.com/acme/web.git",
        "git fetch -q --filter=blob:none origin feat/upload",
        "git checkout -q -b wsp/fix-the-flaky-upload-test 4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a",
        "",
        `Work on wsp/fix-the-flaky-upload-test, then commit and run git push -u origin wsp/fix-the-flaky-upload-test before your turn ends. Thread ${threadId.slice(0, 8)} merges that branch. This machine is deleted 30 minutes after your last turn ends, and anything not pushed is lost with it.`,
        "",
        "The task:",
      ].join("\n"),
    );
    // The record names the lead's project and the lead, and stands in its root's tree.
    const record = (await store.get("workspaces", made.id)) as Record<string, unknown>;
    expect(record).toMatchObject({ kind: "cloud", project: project.id, lead: { threadId, workspaceId: folder.id }, rootThreadId: threadId, parentThreadId: threadId, golden: "wsp-host-default-v1", size: { cpu: 2, memMb: 4096 } });
    expect(record["sub"]).toEqual({ on: "boat", repo: "https://github.com/acme/web.git", base: { branch: "feat/upload", sha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" }, branch: "wsp/fix-the-flaky-upload-test", folder: "/root/web", rateUsdPerHour: 0.018, sizeName: "small", spawn: expect.any(Object) });
    // The thread opens in that folder, the preamble above the task, and is the lead's child.
    const child = await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, TASK), harness: made.sub!.harness, notify: ["me"], turnToken: token, title: NAME, cwd: made.sub!.folder }, lead);
    await child.finished;
    expect(boat.creates).toHaveLength(1);
    expect(boat.creates[0]).toMatchObject({ fromSnapshot: "wsp-host-default-v1", cpu: 2, memMb: 4096 });
    // The picked files went up as one archive by the file road, and only their names are in a command.
    const machine = boat.machines[0]!;
    expect(seedFiles(boat)).toEqual([`/tmp/wsp-seed-${made.id}.tgz`]);
    expect(machine.execLog).toContain("install -d -m 0755 '/root/web'");
    expect(machine.execLog).toContain(`tar -xzf '/tmp/wsp-seed-${made.id}.tgz' -C '/root/web' && rm -f '/tmp/wsp-seed-${made.id}.tgz'`);
    const opened = starts.at(-1)!.o;
    expect(opened.cwd).toBe("/root/web");
    expect(opened.prompt).toBe(`${made.sub!.preamble}\n\n${TASK}`);
    expect((await rt.sessions.list()).find(r => r.threadId === child.view().threadId)).toMatchObject({ parentThreadId: threadId, rootThreadId: threadId, workspaceId: made.id });
    // wsp asked the lead's daemon one read and pushed nothing.
    expect(daemon.frames.slice(before).map(f => f["op"])).toEqual(["git.status"]);
  });

  it("writes each step of the start on the record as it goes, the row carrying it, for the line under the brief", async () => {
    const { rt, boat, project, lead, token } = await leadOnThisComputer({ picks: ["CLAUDE.local.md", "local.properties"], git: { uncommitted: 3 } });
    boat.states = ["provisioning", "cloning", "ready"];
    const made = await spawn(rt, project.id, lead);
    const child = await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, TASK), harness: made.sub!.harness, notify: ["me"], turnToken: token, title: NAME, cwd: made.sub!.folder }, lead);
    await child.finished;
    const machine = boat.machines[0]!;
    const row = (await rt.sessions.list()).find(r => r.threadId === child.view().threadId)!;
    const spawned = row.sub!.spawn!;
    expect(spawned).toMatchObject({
      branch: { branch: "feat/upload", sha: "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a", ms: expect.any(Number) },
      machine: { id: machine.id, states: ["provisioning", "cloning", "ready"], ms: expect.any(Number) },
      files: { paths: ["CLAUDE.local.md", "local.properties"], bytes: 1243, ms: expect.any(Number) },
      uncommitted: 3,
    });
    expect(spawned.readyAt).toBeGreaterThanOrEqual(spawned.startedAt);
    expect(spawned.failed).toBeUndefined();
    // The log is the stage lines the command line prints, Boat's states among them.
    expect(spawned.log).toEqual(expect.arrayContaining([`starting ${NAME} on Boat`, `Boat's machine ${machine.id} is cloning`, "copying 2 files", "ready"]));
    expect(row.sub!.state).toBe("running");
    // The size by its class name, as Boat sells it, for the card and the pane.
    expect(row.sub!.size).toEqual({ cpu: 2, memMb: 4096, name: "small" });
  });

  it("leaves a file of values and a key on this computer whatever the saved picks say, since a sub-thread's machine is a provider's", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer({ picks: [".env", "dev.pem", "CLAUDE.local.md"], sizes: { ".env": 120, "dev.pem": 900, "CLAUDE.local.md": 1100 }, left: ["deploy/tls.key"] });
    const made = await spawn(rt, project.id, lead);
    expect(made.sub!.out.sent).toEqual({ paths: ["CLAUDE.local.md"], bytes: 1100, missing: [] });
    await opened(rt, made, lead);
    const archive = gunzipSync(boat.files.find(f => f.path.startsWith("/tmp/wsp-seed-"))!.bytes);
    expect([archive.includes("CLAUDE.local.md"), archive.includes(".env"), archive.includes("dev.pem")]).toEqual([true, false, false]);
    // Every picked path that stayed is named in one line, a key the pack found inside a picked folder among them.
    expect(made.notice?.split("\n")).toContain(subStayedLine([".env", "dev.pem", "deploy/tls.key"]));
  });

  it("says why a sub-thread starts with nothing where the only pick is a file of values", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer({ picks: [".env"], sizes: { ".env": 120 } });
    const made = await spawn(rt, project.id, lead);
    expect(made.sub!.out.sent).toEqual({ paths: [], bytes: 0, missing: [] });
    expect(made.notice).toBe(subStayedLine([".env"]));
    expect(seedFiles(boat)).toEqual([]);
  });

  it("sends nothing ignored without the person's picks, says why, and names a picked path no longer there", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer();
    const made = await spawn(rt, project.id, lead);
    expect(made.sub!.out.sent).toEqual({ paths: [], bytes: 0, missing: [] });
    expect(made.sub!.out.notSent).toEqual({ uncommitted: 0, why: "nothing is picked for web" });
    expect(made.notice).toBe("no git-ignored files were sent: nothing is picked for web. wsp projects seed web shows what could go.");
    expect(made.sub!.preamble).toContain("The folder you are in, /root/web, is empty.");
    await opened(rt, made, lead);
    expect(seedFiles(boat)).toEqual([]);
    expect(boat.machines[0]!.execLog.some(c => c.startsWith("tar "))).toBe(false);
    const again = await leadOnThisComputer({ picks: ["CLAUDE.local.md", "gone.json"] });
    const sent = await spawn(again.rt, again.project.id, again.lead);
    expect(sent.sub!.out.sent).toEqual({ paths: ["CLAUDE.local.md"], bytes: 1100, missing: ["gone.json"] });
  });

  it("takes the size the lead names, at its class's rate, and numbers the default branch past one another sub-thread pushes", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer({ agents: { spawn: true, maxMachines: 5 } });
    const big = await spawn(rt, project.id, lead, { size: "4x8" });
    await opened(rt, big, lead);
    expect(boat.creates[0]).toMatchObject({ cpu: 4, memMb: 8192 });
    expect(big.sub!.out).toMatchObject({ size: { cpu: 4, memMb: 8192 }, rateUsdPerHour: 0.036 });
    expect(big.sub!.preamble).toContain("on Boat (default, 4 vCPU, 8 GB)");
    const twin = await spawn(rt, project.id, lead, {}, `${NAME} 2`);
    expect(twin.sub!.out.branch).toBe("wsp/fix-the-flaky-upload-test-2");
    const third = await spawn(rt, project.id, lead, {}, `${NAME}.`);
    expect(third.sub!.out.branch).toBe("wsp/fix-the-flaky-upload-test-3");
    const named = await spawn(rt, project.id, lead, { branch: "wsp/upload-suite" }, "Run the suite");
    expect(named.sub!.out.branch).toBe("wsp/upload-suite");
  });

  it("numbers the default branch past one the remote already holds", async () => {
    const { rt, project, lead } = await leadOnThisComputer({ remoteHeads: ["wsp/fix-the-flaky-upload-test", "wsp/fix-the-flaky-upload-test-2"] });
    const made = await spawn(rt, project.id, lead);
    expect(made.sub!.out.branch).toBe("wsp/fix-the-flaky-upload-test-3");
    expect(made.sub!.preamble).toContain("git checkout -q -b wsp/fix-the-flaky-upload-test-3 ");
  });

  it("takes the lead's own project beside the cloud, by its id, its name or none, and refuses another naming the one it carries", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer();
    expect((await spawn(rt, project.name, lead)).sub!.out.repo).toBe((await spawn(rt, "", lead, {}, "Another one")).sub!.out.repo);
    const records = (await rt.workspaces.list()).length;
    const other = await refused(spawn(rt, "api", lead, {}, "A third one"));
    expect(other).toEqual({ said: subOtherProjectRefusal("boat", "api", "web").message, cls: "usage" });
    expect(other.said).toBe(refusalLine("run --on boat starts a sub-thread of the thread asking, on that thread's project web, and api is another project", "Leave the project out, or run the work on api without --on."));
    // Refused before the record is announced, so no window draws a row for it and nothing is asked of Boat.
    expect((await rt.workspaces.list()).length).toBe(records);
    expect(boat.creates).toEqual([]);
  });

  it("says the word beside --on it cannot take as each caller names it", () => {
    expect(subBesideRefusal("--beside").message).toBe(refusalLine("--on starts a sub-thread of the thread asking, in a folder of its own on its own machine, so it cannot also take --beside", "Drop --beside, or run the work without --on."));
    expect(subBesideRefusal("replaces").message).toBe(refusalLine("--on starts a sub-thread of the thread asking, in a folder of its own on its own machine, so it cannot also take replaces", "Drop replaces, or run the work without --on."));
  });

  it("refuses a size the plan does not sell, naming the ones it does, before any create", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer();
    boat.limitsRead = TRIAL;
    const big = await refused(spawn(rt, project.id, lead, { size: "8x16" }));
    expect(big).toEqual({ said: subSizeRefusal("Boat", "8x16", TRIAL_SIZES).message, cls: "usage" });
    expect(big.said).toBe("Boat sells small 2x4 ($0.018/h) and default 4x8 ($0.036/h) on this plan; 8x16 is not one of them. Name one of them, or leave the size out for Boat's own.");
    const odd = await refused(spawn(rt, project.id, lead, { size: "3x5" }));
    expect(odd).toEqual({ said: subSizeRefusal("Boat", "3x5", TRIAL_SIZES).message, cls: "usage" });
    expect(boat.creates).toEqual([]);
  });

  it("works in the sub-machine's own folder, counts against the root, runs no second thread, and its thread starts nothing but still sends to its lead", async () => {
    const { rt, boat, project, lead, threadId, token, folder, endLead, starts } = await leadOnThisComputer({ agents: { spawn: true, maxMachines: 1 } });
    const made = await spawn(rt, project.id, lead);
    // checkoutOf: the folder a turn and a command start in is the sub-machine's own.
    expect(made.folder).toBe("/root/web");
    // The root's machines count it: a second spawn meets the cap before any create.
    const capped = await refused(spawn(rt, project.id, lead, {}, "Another one"));
    expect(capped).toEqual({ said: spawnCapRefusal(threadId, 1, 1), cls: "usage" });
    const child = await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, TASK), harness: "claude", turnToken: token, cwd: made.sub!.folder }, lead);
    await child.finished;
    expect(boat.creates).toHaveLength(1);
    const childId = child.view().threadId!;
    const asChild: Caller = { origin: "relayed", by: { kind: "thread", threadId: childId, workspaceId: made.id, rootThreadId: threadId } };
    const second = await refused(rt.sessions.start(made.id, { prompt: "beside it", harness: "claude" }, lead));
    expect(second).toEqual({ said: subMachineTakenRefusal(NAME, childId).message, cls: "usage" });
    // A sub-thread starts nothing of its own, and still sends to its lead.
    const grand = await refused(rt.workspaces.create({ project: project.id, name: "grandchild", sub: { on: "boat" } }, asChild));
    expect(grand).toEqual({ said: subStartsNothingRefusal(childId, threadId).message, cls: "usage" });
    expect(boat.creates).toHaveLength(1);
    await endLead();
    const told = await rt.sessions.start(folder.id, { thread: threadId, prompt: "pushed wsp/fix-the-flaky-upload-test", harness: "claude" }, asChild);
    await told.finished;
    expect(told.view().threadId).toBe(threadId);
    expect(starts.at(-1)!.o.prompt).toContain("pushed wsp/fix-the-flaky-upload-test");
  });
});

describe("run --on boat --from a ref the remote holds", () => {
  /** A commit on the remote alone, made off the lead's head without moving it, pushed to `ref` there. */
  const pushedElsewhere = (repo: string, ref: string, message: string): string => {
    const sha = execFileSync("git", ["-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", message]).toString().trim();
    execFileSync("git", ["-C", repo, "push", "-q", "origin", `${sha}:${ref}`]);
    return sha;
  };

  it("starts at origin/main's commit while the lead's branch holds commits the remote does not, and says nothing of the lead's uncommitted changes", async () => {
    const { rt, boat, daemon, project, lead, repo } = await leadOnThisComputer({ picks: ["CLAUDE.local.md"], git: { ahead: 1, uncommitted: 2 } });
    const main = pushedElsewhere(repo, "refs/heads/main", "the suite moved to tests/k6");
    const before = daemon.frames.length;
    const made = await spawn(rt, project.id, lead, { from: "origin/main" });
    // The record stands before the cloud is asked: the machine is made once the thread's start comes.
    expect(boat.creates).toEqual([]);
    expect(made.sub!.out).toMatchObject({ base: { branch: "main", sha: main }, branch: "wsp/fix-the-flaky-upload-test", notSent: { uncommitted: 0 } });
    expect(made.sub!.preamble).toContain(["gh auth setup-git", "git init -q", "git remote add origin https://github.com/acme/web.git", "git fetch -q --filter=blob:none origin main", `git checkout -q -b wsp/fix-the-flaky-upload-test ${main}`].join("\n"));
    expect(made.notice).toBeUndefined();
    expect(daemon.frames.slice(before).map(f => f["op"])).toEqual(["git.status"]);
    await opened(rt, made, lead);
    expect(boat.creates).toHaveLength(1);
  });

  it("takes a tag by the commit it names and a commit by the remote branch holding it, the default branch first", async () => {
    const { rt, project, lead, repo } = await leadOnThisComputer({ git: { upstream: undefined, defaultBranch: "trunk" }, agents: { spawn: true, maxMachines: 5 } });
    const tagged = pushedElsewhere(repo, "refs/heads/trunk", "the release");
    execFileSync("git", ["-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", "tag", "-a", "-m", "v1", "v1", tagged]);
    execFileSync("git", ["-C", repo, "push", "-q", "origin", "v1", `${tagged}:refs/heads/a-side`]);
    // A branch whose name ends in v1 is not the tag v1, though ls-remote lists it for that name.
    pushedElsewhere(repo, "refs/heads/release/v1", "not the tag");
    const byTag = await spawn(rt, project.id, lead, { from: "v1" }, "From the tag");
    expect(byTag.sub!.out.base).toEqual({ branch: "v1", sha: tagged });
    execFileSync("git", ["-C", repo, "fetch", "-q", "origin"]);
    const byCommit = await spawn(rt, project.id, lead, { from: tagged.slice(0, 9) }, "From the commit");
    expect(byCommit.sub!.out.base).toEqual({ branch: "trunk", sha: tagged });
    expect(byCommit.sub!.preamble).toContain(`git fetch -q --filter=blob:none origin trunk\ngit checkout -q -b wsp/from-the-commit ${tagged}`);
    // A branch deleted on the remote that this folder still tracks, sorting first, is no branch to fetch the commit by.
    const aside = pushedElsewhere(repo, "refs/heads/z-live", "on a live branch alone");
    execFileSync("git", ["-C", repo, "push", "-q", "origin", `${aside}:refs/heads/a-gone`]);
    execFileSync("git", ["-C", repo, "fetch", "-q", "origin"]);
    execFileSync("git", ["-C", join(dirname(repo), "remote.git"), "update-ref", "-d", "refs/heads/a-gone"]);
    expect(execFileSync("git", ["-C", repo, "branch", "-r", "--contains", aside]).toString()).toContain("origin/a-gone");
    const byLive = await spawn(rt, project.id, lead, { from: aside.slice(0, 9) }, "From the live branch");
    expect(byLive.sub!.out.base).toEqual({ branch: "z-live", sha: aside });
  });

  it("refuses a commit no remote branch holds, and a ref the remote lacks, with the push line, before any create", async () => {
    const { rt, boat, project, lead, repo } = await leadOnThisComputer();
    const local = execFileSync("git", ["-C", repo, "rev-parse", "HEAD"]).toString().trim();
    const commit = await refused(spawn(rt, project.id, lead, { from: local.slice(0, 12) }));
    expect(commit).toEqual({ said: subFromNotOnRemoteRefusal(local.slice(0, 12), undefined).message, cls: "usage" });
    const branch = (await spawn(rt, project.id, lead, { from: "feat/later" }).then(() => undefined, (x: unknown) => x)) as Error;
    expect(branch.message).toBe("feat/later is not on the remote. Run git push -u origin feat/later, then start the sub-thread again.");
    // The form the refusals teach: the push line names the branch, which git takes, and not origin/, which it refuses.
    const tracked = (await spawn(rt, project.id, lead, { from: "origin/feat/later" }).then(() => undefined, (x: unknown) => x)) as Error;
    expect(tracked.message).toBe("origin/feat/later is not on the remote. Run git push -u origin feat/later, then start the sub-thread again.");
    expect(boat.creates).toEqual([]);
  });

  it("names --from beside the push line where the lead's branch is not pushed", async () => {
    const { rt, project, lead } = await leadOnThisComputer({ git: { ahead: 1, defaultBranch: "trunk" } });
    const e = (await spawn(rt, project.id, lead).then(() => undefined, (x: unknown) => x)) as Error & { fix: string };
    expect(e.message).toBe("feat/upload has 1 commit the remote does not. Run git push and start the sub-thread again, or start it from a ref the remote holds with --from origin/trunk.");
  });
});

describe("Boat's row", () => {
  it("carries the account's machines running, the hours its credit buys and what Boat spent today, as its plan panel and wsp computers read them", async () => {
    const { rt, boat } = await leadOnThisComputer({ spendPerDayUsd: 10 });
    Object.assign(boat, { placeLimits: async () => placeLimitsOf({ ...PAID, activeBoxes: 7, creditBalanceHours: 41.5 }) });
    await rt.places!.list(Date.now());
    const row = await until(async () => (await rt.places!.list(Date.now())).find(p => p.id === "box" && p.plan !== undefined), "Boat's limits read on its row");
    expect(row.plan).toMatchObject({ machinesAtOnce: 100, running: 7 });
    expect(row.creditHours).toBe(41.5);
    expect(row.spentTodayUsd).toBe((await rt.status.spend(await rt.places!.rows())).find(s => s.place === "box")!.todayUsd);
  });
});

describe("every check refuses before Boat is asked for a start", () => {
  const cases: { name: string; setup?: Setup; sub?: { on?: string; size?: string; from?: string; branch?: string; harness?: string }; asked?: "terminal"; expect: (f: Awaited<ReturnType<typeof leadOnThisComputer>>) => { said: string; cls: string }; before?: (f: Awaited<ReturnType<typeof leadOnThisComputer>>) => void | Promise<void> }[] = [
    { name: "S1 a line from no thread", asked: "terminal", expect: () => ({ said: subNoThreadRefusal("boat").message, cls: "usage" }) },
    { name: "S3 a cloud this host holds no key for", sub: { on: "solari" }, expect: () => ({ said: subNoCloudRefusal("solari", ["Boat"]).message, cls: "usage" }) },
    { name: "S4 a folder with no remote", setup: { remote: null }, expect: () => ({ said: subNoRemoteRefusal("web", "Boat").message, cls: "usage" }) },
    { name: "S4 a remote on another host", setup: { remote: "https://git.example.com/acme/web.git" }, expect: () => ({ said: subOtherHostRefusal("web", "git.example.com", "Boat").message, cls: "usage" }) },
    { name: "S5 a lead on no branch", setup: { git: { head: "(detached)" } }, expect: () => ({ said: subDetachedRefusal("web", "origin/main").message, cls: "usage" }) },
    { name: "S5 a branch the remote lacks", setup: { git: { upstream: undefined } }, expect: () => ({ said: subNoUpstreamRefusal("feat/upload", "origin/main").message, cls: "usage" }) },
    { name: "S5 commits the remote lacks", setup: { git: { ahead: 2 } }, expect: () => ({ said: subAheadRefusal("feat/upload", 2, "origin/main").message, cls: "usage" }) },
    { name: "S5 commits git could not count", setup: { git: { countsUnknown: true } }, expect: () => ({ said: subAheadUnknownRefusal("feat/upload", "origin/main").message, cls: "usage" }) },
    { name: "S5 a ref --from names the remote lacks", sub: { from: "origin/nope" }, expect: () => ({ said: subFromNotOnRemoteRefusal("origin/nope", "nope").message, cls: "usage" }) },
    { name: "S6 no image on Boat", setup: { image: false }, expect: () => ({ said: subNoImageRefusal("Boat").message, cls: "usage" }) },
    { name: "S6 an image with no GitHub sign-in", setup: { image: { logins: [{ name: "GitHub CLI login", state: "skipped" }] } }, expect: () => ({ said: subNoSignInRefusal("Boat", { label: "GitHub", cli: "gh", login: "GitHub CLI login" }, "acme/web").message, cls: "usage" }) },
    { name: "S6 gh on this computer signed in to nothing, the broker ready", setup: { broker: true, hostGh: false }, expect: () => ({ said: subHostNoSignInRefusal("Boat", "acme/web").message, cls: "usage" }) },
    { name: "S6 a remote on GitLab, the broker ready", setup: { broker: true, hostGh: true, remote: "git@gitlab.com:acme/web.git" }, expect: () => ({ said: subHostOtherGitRefusal("Boat", "GitLab", "acme/web").message, cls: "usage" }) },
    { name: "S7 a size the plan does not sell", sub: { size: "16x32" }, expect: () => ({ said: subSizeRefusal("Boat", "16x32", PAID_SIZES).message, cls: "usage" }) },
    { name: "S8 a name git refuses", sub: { branch: "wsp/bad..name" }, expect: () => ({ said: subBranchNameRefusal("wsp/bad..name").message, cls: "usage" }) },
    { name: "S8 the lead's own branch", sub: { branch: "feat/upload" }, expect: () => ({ said: subLeadBranchRefusal("feat/upload").message, cls: "usage" }) },
    { name: "S8 HEAD", sub: { branch: "HEAD" }, expect: () => ({ said: subBranchNameRefusal("HEAD").message, cls: "usage" }) },
    { name: "S8 a part ending in .lock", sub: { branch: "feat/x.lock/y" }, expect: () => ({ said: subBranchNameRefusal("feat/x.lock/y").message, cls: "usage" }) },
    // Today's line, which names the row as the place door does; C2 makes it say Boat.
    { name: "S10 the day's spend reached", setup: { spendPerDayUsd: 0 }, expect: () => ({ said: spendCapRefusal("box", 0, 0), cls: "provider" }) },
    { name: "S11 an agent the broker adds no key for, the broker ready", setup: { broker: true, hostGh: true, image: { agents: ["claude", "codex", "opencode"] } }, sub: { harness: "opencode" }, expect: () => ({ said: brokeredAgentRefusal("OpenCode", ["Claude Code", "Codex"]).message, cls: "usage" }) },
    { name: "S11 an agent the image does not carry", setup: { image: { agents: ["codex"] } }, sub: { harness: "claude" }, expect: () => ({ said: subNoAgentRefusal("Boat", "claude", ["codex"]).message, cls: "usage" }) },
    {
      name: "S12 picks past what a sub-thread carries",
      setup: { picks: ["data.bin", "CLAUDE.local.md"], sizes: { "data.bin": SUB_SEED_MAX_BYTES + 1024, "CLAUDE.local.md": 100 } },
      expect: () => ({ said: "", cls: "usage" }),
    },
    {
      name: "S13 the hour's starts used",
      before: f => void (f.boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, hour: { limit: 60, used: 60 } } }),
      expect: () => ({ said: startsUsedLine("hour", 60, 60, "the $20 plan", `sub-thread ${NAME}`), cls: "provider" }),
    },
    {
      name: "S13 the day's starts used",
      before: f => void (f.boat.limitsRead = { ...PAID, starts: { ...PAID.starts!, day: { limit: 200, used: 200 } } }),
      expect: () => ({ said: startsUsedLine("day", 200, 200, "the $20 plan", `sub-thread ${NAME}`), cls: "provider" }),
    },
  ];
  for (const c of cases) {
    it(c.name, async () => {
      const f = await leadOnThisComputer(c.setup);
      await c.before?.(f);
      const got = await refused(spawn(f.rt, f.project.id, c.asked === "terminal" ? undefined : f.lead, c.sub ?? {}));
      const want = c.expect(f);
      if (want.said !== "") expect(got).toEqual(want);
      else expect(got.cls).toBe(want.cls);
      expect(f.boat.creates).toEqual([]);
      expect(f.daemon.frames.filter(fr => fr["op"] === "git.push")).toEqual([]);
    });
  }

  const skipped = { logins: [{ name: "GitHub CLI login", state: "skipped" as const }] };
  it("S6 asks gh on this computer for its GitHub login in place of the image's while the broker is ready", async () => {
    const f = await leadOnThisComputer({ broker: true, hostGh: true, image: skipped });
    expect((await spawn(f.rt, f.project.id, f.lead)).sub).toBeDefined();
    expect(readFileSync(join(dirname(f.repo), "gh-asked"), "utf8").split("\n")[0]).toBe("auth token --hostname github.com");
  });

  it("S6 asks the image while the broker is not ready, whatever gh on this computer holds", async () => {
    const f = await leadOnThisComputer({ broker: false, hostGh: true, image: skipped });
    expect(await refused(spawn(f.rt, f.project.id, f.lead))).toEqual({ said: subNoSignInRefusal("Boat", { label: "GitHub", cli: "gh", login: "GitHub CLI login" }, "acme/web").message, cls: "usage" });
    expect(existsSync(join(dirname(f.repo), "gh-asked"))).toBe(false);
  });

  it("S12 names the largest picks and the sizes", async () => {
    const big = SUB_SEED_MAX_BYTES + 1024;
    const f = await leadOnThisComputer({ picks: ["data.bin", "CLAUDE.local.md"], sizes: { "data.bin": big, "CLAUDE.local.md": 100 } });
    const got = await refused(spawn(f.rt, f.project.id, f.lead));
    expect(got.cls).toBe("usage");
    expect(got.said).toMatch(new RegExp(`^the files picked for web come to [^,]+, over the ${fmtBytes(SUB_SEED_MAX_BYTES).replace(".", "\\.")} a sub-thread carries; the largest are data\\.bin \\(${fmtBytes(big).replace(".", "\\.")}\\), CLAUDE\\.local\\.md \\(100 B\\)\\. Pick again with wsp projects seed web\\.$`));
    expect(subSeedTooBigRefusal("web", "1 MB", "20 MB", ["data/"]).message).toBe(refusalLine("the files picked for web come to 1 MB, over the 20 MB a sub-thread carries; the largest are data/", "Pick again with wsp projects seed web."));
    expect(f.boat.creates).toEqual([]);
  });
});

describe("a machine that fails before the first turn", () => {
  for (const fault of [{ kind: "unusable" }, { kind: "env" }, { kind: "cancelled" }, { kind: "unusable", replayed: true }, { kind: "env", replayed: true }] as Fault[]) {
    it(`is deleted and replaced once: ${fault.kind}${fault.replayed === true ? ", on a replayed create" : ""}`, async () => {
      const { rt, boat, project, lead } = await leadOnThisComputer();
      boat.faults = [fault];
      const made = await spawn(rt, project.id, lead);
      await opened(rt, made, lead);
      expect(boat.creates).toHaveLength(2);
      const [bad, good] = boat.machines;
      expect(bad!.killed).toBe(true);
      expect(good!.killed).toBe(false);
      expect((await rt.workspaces.get(made.id)).machineId).toBe(good!.id);
      const line =
        fault.kind === "unusable"
          ? `Boat gave ${bad!.id} with nothing able to run on it (${BOX_WORK_DIR} is missing); wsp deleted it and started another.`
          : fault.kind === "env"
            ? `Boat's machine ${bad!.id} did not take wsp's setup within 2 minutes (exit 124); wsp deleted it and started another.`
            : `Boat could not get a machine for ${NAME} and cancelled the create (no capacity); wsp started another.`;
      const row = (await rt.sessions.list()).find(r => r.workspaceId === made.id)!;
      expect(row.sub!.spawn!.replaced).toEqual([line]);
    });
  }

  it("fails the spawn on a second failure, naming both machines, and leaves neither standing, the record released under its lead", async () => {
    const { rt, boat, project, lead, threadId } = await leadOnThisComputer();
    boat.faults = [{ kind: "unusable", replayed: true }, { kind: "unusable" }];
    const got = await refused(spawn(rt, project.id, lead).then(made => opened(rt, made, lead)));
    expect(got.cls).toBe("provider");
    expect(got.said).toBe(`Boat gave two machines in a row that could not run commands (${boat.machines[0]!.id}, ${boat.machines[1]!.id}); Boat is restoring badly right now. Start it again in a few minutes.`);
    expect(boat.machines.every(m => m.killed)).toBe(true);
    expect((await rt.workspaces.list()).find(w => w.name === NAME)).toMatchObject({ phase: "released", machineId: "", parentThreadId: threadId });
  });

  it("names both faults when they differ", async () => {
    const { rt, boat, project, lead } = await leadOnThisComputer();
    boat.faults = [{ kind: "env" }, { kind: "cancelled" }];
    const got = await refused(spawn(rt, project.id, lead).then(made => opened(rt, made, lead)));
    expect(got.said).toBe(`Boat gave two machines in a row that failed before the sub-thread started (${boat.machines[0]!.id}: setup not taken, exit 124; ${boat.machines[1]!.id}: no capacity). Start it again in a few minutes.`);
    expect(boat.machines.every(m => m.killed)).toBe(true);
  });
});

describe("what wsp does to the lead and the machine", () => {
  it("pushes nothing for the lead and runs nothing on the machine past 10's list", async () => {
    const { rt, boat, daemon, project, lead, token } = await leadOnThisComputer({ picks: ["CLAUDE.local.md"] });
    const before = daemon.frames.length;
    const made = await spawn(rt, project.id, lead);
    expect(daemon.frames.slice(before).map(f => f["op"])).toEqual(["git.status"]);
    const child = await rt.sessions.start(made.id, { prompt: subOpening(made.sub!.preamble, TASK), harness: "claude", turnToken: token, cwd: made.sub!.folder }, lead);
    const machine = boat.machines[0]!;
    const spawned = machine.execLog.slice(0, machine.execLog.indexOf(`tar -xzf '/tmp/wsp-seed-${made.id}.tgz' -C '/root/web' && rm -f '/tmp/wsp-seed-${made.id}.tgz'`) + 1);
    // The fork road's three, then the folder and the archive, and nothing else; no command carries a byte of a file.
    expect(spawned.filter(forkRoad)).toHaveLength(3);
    expect(spawned.filter(c => !forkRoad(c))).toEqual(["install -d -m 0755 '/root/web'", `tar -xzf '/tmp/wsp-seed-${made.id}.tgz' -C '/root/web' && rm -f '/tmp/wsp-seed-${made.id}.tgz'`]);
    expect(seedFiles(boat)).toEqual([`/tmp/wsp-seed-${made.id}.tgz`]);
    const seed = boat.files.find(f => f.path.startsWith("/tmp/wsp-seed-"))!.bytes.toString("base64");
    expect(spawned.some(c => c.includes(seed.slice(0, 32)))).toBe(false);
    await child.finished;
    expect(machine.execLog.some(c => /\bgit (clone|push|fetch)\b|\b(npm|pnpm) (install|ci)\b/.test(c))).toBe(false);
  });
});

describe("the branch a sub-thread pushes, by git's own rules", () => {
  it("takes exactly the names git check-ref-format --branch takes", () => {
    const names = ["wsp/ok", "feat/upload", "a/b/c", "HEADx", "refs/heads/x", "a@b", "@", "HEAD", "feat/x.lock/y", "feat/x.lock", "x.lock", "wsp/bad..name", "-x", "-", "/x", "x/", "x.", ".x", "x/.y", "x//y", "a b", "a~b", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", "a@{b", "@{-1}", "a\tb", ""];
    // Read outside any repository, so the checkout the test runs in has no say in what git answers.
    root = mkdtempSync(join(tmpdir(), "wsp-ref-names-"));
    const git = (name: string): boolean => spawnSync("git", ["check-ref-format", "--branch", name], { cwd: root, stdio: "ignore" }).status === 0;
    expect(names.filter(n => subBranchNameOk(n) !== git(n))).toEqual([]);
  });
});

describe("a lead off this computer", () => {
  it("refuses a lead on a machine wsp forked, before any create", async () => {
    const { rt, boat } = await leadOnThisComputer();
    const cloud = await rt.projects.add({ source: "https://github.com/acme/web", on: "box", name: "web-cloud" });
    const machine = await rt.workspaces.create({ project: cloud.id, name: "cloud-lead", agents: { } });
    const opened = await rt.sessions.start(machine.id, { prompt: "look around", harness: "claude" });
    await opened.finished;
    const onMachine: Caller = { origin: "relayed", by: { kind: "thread", threadId: opened.view().threadId!, workspaceId: machine.id, rootThreadId: opened.view().threadId! } };
    const creates = boat.creates.length;
    const got = await refused(spawn(rt, cloud.id, onMachine));
    expect(got).toEqual({ said: subLeadOnMachineRefusal("Boat", opened.view().threadId!, "cloud-lead").message, cls: "usage" });
    expect(boat.creates).toHaveLength(creates);
  });

  it("starts from a lead on a computer the person added, and says no picks went since its folder is there", async () => {
    root = mkdtempSync(join(tmpdir(), "wsp-sub-box-"));
    const held = await leadAndBox(root);
    await held.store.put("goldens", copyKey("default", "default"), { head: 1, versions: [{ version: 1, snapshotId: "snap_image-v1", baseTemplate: "base", setupSha: "abc", createdAt: "2026-09-01T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 }, logins: [{ name: "GitHub CLI login", state: "copied" }] }] });
    const at = await held.rt.workspaces.folderFor({ project: held.onBox.id }, held.lead);
    const child = await held.rt.sessions.start(at.workspace.id, { prompt: "build it", harness: "claude", turnToken: held.launch[TURN_TOKEN_ENV]! }, held.lead);
    await child.finished;
    const onBox: Caller = { origin: "relayed", by: { kind: "thread", threadId: child.view().threadId!, workspaceId: at.workspace.id, rootThreadId: held.threadId } };
    const made = await held.rt.workspaces.create({ project: "", name: NAME, sub: { on: "default" } }, onBox);
    expect(made.sub!.out.notSent).toEqual({ uncommitted: 0, why: subPickedElsewhereWhy("lab-box", "hetzner") });
    expect(made.notice).toBe(subNotSentLine(subPickedElsewhereWhy("lab-box", "hetzner"), "lab-box"));
    expect(made.notice).toBe("no git-ignored files were sent: lab-box lives on hetzner, and picks are read from folders on this computer only.");
    expect(made.sub!.folder).toBe("/root/lab");
  });

  it("answers its sub-thread on the cloud the cloud rows alone, and no row a thread reads carries the road to it", async () => {
    root = mkdtempSync(join(tmpdir(), "wsp-sub-box-"));
    const held = await leadAndBox(root);
    await held.store.put("goldens", copyKey("default", "default"), { head: 1, versions: [{ version: 1, snapshotId: "snap_image-v1", baseTemplate: "base", setupSha: "abc", createdAt: "2026-09-01T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 }, logins: [{ name: "GitHub CLI login", state: "copied" }] }] });
    const at = await held.rt.workspaces.folderFor({ project: held.onBox.id }, held.lead);
    const child = await held.rt.sessions.start(at.workspace.id, { prompt: "build it", harness: "claude", turnToken: held.launch[TURN_TOKEN_ENV]! }, held.lead);
    await child.finished;
    const onBox: Caller = { origin: "relayed", by: { kind: "thread", threadId: child.view().threadId!, workspaceId: at.workspace.id, rootThreadId: held.threadId } };
    const made = await held.rt.workspaces.create({ project: "", name: NAME, sub: { on: "default" } }, onBox);
    const onCloud: Caller = { origin: "relayed", by: { kind: "thread", threadId: "t_sub", workspaceId: made.id, rootThreadId: held.threadId } };
    const boxRows = await threadPlaces(held.rt, onBox, Date.now());
    expect(boxRows.map(p => p.id)).toEqual([held.placeId]);
    expect(await threadPlaces(held.rt, onCloud, Date.now())).toEqual((await held.rt.places!.list(Date.now())).filter(p => p.kind === "provider").map(p => ({ ...p, label: placeSaid(p) })));
    expect([...boxRows, ...(await threadPlaces(held.rt, held.lead, Date.now()))].filter(p => p.road !== undefined)).toEqual([]);
  });

  it("numbers the default branch past the branches the remote holds, read in the lead's folder on its box", async () => {
    root = mkdtempSync(join(tmpdir(), "wsp-sub-box-"));
    const held = await leadAndBox(root);
    held.seen.remoteHeads = ["main", "wsp/fix-the-flaky-upload-test"];
    await held.store.put("goldens", copyKey("default", "default"), { head: 1, versions: [{ version: 1, snapshotId: "snap_image-v1", baseTemplate: "base", setupSha: "abc", createdAt: "2026-09-01T00:00:00.000Z", smoke: { cmd: "true", exitCode: 0 }, logins: [{ name: "GitHub CLI login", state: "copied" }] }] });
    const at = await held.rt.workspaces.folderFor({ project: held.onBox.id }, held.lead);
    const child = await held.rt.sessions.start(at.workspace.id, { prompt: "build it", harness: "claude", turnToken: held.launch[TURN_TOKEN_ENV]! }, held.lead);
    await child.finished;
    const onBox: Caller = { origin: "relayed", by: { kind: "thread", threadId: child.view().threadId!, workspaceId: at.workspace.id, rootThreadId: held.threadId } };
    const made = await held.rt.workspaces.create({ project: "", name: NAME, sub: { on: "default" } }, onBox);
    expect(made.sub!.out.branch).toBe("wsp/fix-the-flaky-upload-test-2");
    // Read in the lead's own folder there, as root through the box's road, with no prompt.
    expect(held.seen.execs.map(e => e.cmd).filter(c => c.includes("ls-remote"))).toEqual([expect.stringContaining("GIT_TERMINAL_PROMPT=0 git -C /root/lab-box ls-remote --heads origin")]);
  });
});
